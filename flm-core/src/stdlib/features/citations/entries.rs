//! This module defines the table of the citation entries,
//! `flm.citations.entries` ([`ENTRIES`]), and the retrieve stage
//! ([`RETRIEVE`]), which writes this table.
//!
//! The retrieve stage reads the cited keys from the table
//! [`KEYS`](super::KEYS) and calls the citation providers of the service slot
//! [`PROVIDERS_SLOT`](super::PROVIDERS_SLOT) to obtain their citation entries.
//! It checks each citation entry and writes the results to [`ENTRIES`]. Then
//! it numbers the cited works and writes the numbers to the table
//! [`CITATIONS`](super::CITATIONS).
//!
//! The table [`ENTRIES`] is read by the stage that builds the list of cited
//! works, by the check stage, and by the lowering rules of `\cite` and `\R`
//! under the in-place placement, where a citation mark contains the citation
//! entries.

use std::collections::BTreeMap;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::Diagnostic;
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::rewrites::blocks::run_holds_blocks;
use crate::lang::text::plain_text;
use crate::lang::{Flm, FlmDiagnostics};
use crate::model::element::element_value;

use super::diagnostics::{EntryCites, EntryNotInline, NoProvider, ProviderFailed, ProviderWarning};
use super::elements::CitationElement;
use super::numbers::{number_works, stage_failed, CITATIONS, KEYS};
use super::providers::{
    CitationAnswer, CitationEntry, CitationProvider, CitationProviderError, CitationRequest,
    ProviderCx, PROVIDERS_SLOT,
};

/// The name of the table of the citation entries, `flm.citations.entries`.
/// The table ([`CitationEntries`]) contains, for each cited key, its citation
/// entry or the reason why it has none.
pub const ENTRIES: TableName<CitationEntries> = TableName::new("flm.citations.entries");

/// The name of the retrieve stage, `flm.citations.retrieve`. This stage
/// obtains the citation entries from the citation providers, and writes the
/// tables [`ENTRIES`] and [`CITATIONS`](super::CITATIONS).
pub const RETRIEVE: &str = "flm.citations.retrieve";

/// The reason why a cited key has no citation entry.
///
/// The table [`ENTRIES`] contains this reason for each cited key without a
/// citation entry ([`Retrieval::Missing`]). The check stage reports each such
/// citation key in the error [`NoEntry`](super::NoEntry), with the text that
/// [`MissingEntry::reason`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissingEntry {
    /// No citation provider has been contributed to the service slot
    /// [`PROVIDERS_SLOT`].
    NoProviderInstalled,
    /// Every citation provider returned `Ok(None)` for the citation key, that
    /// is, no citation provider handles this citation key.
    UnknownProvider,
    /// The [`CitationProviderError`] structure contains the details (including
    /// the error message) that a citation provider reports when it fails to
    /// retrieve a particular citation key. The retrieve stage also creates such
    /// an error itself when [`CitationProvider::answer_all`] returns a
    /// different number of results than it received requests.
    Failed(CitationProviderError),
    /// The citation entry contains a citation (`\cite` or `\R`), and the
    /// retrieve stage refused it. The retrieve stage reports this case as the
    /// error [`EntryCites`].
    Cites,
    /// The citation entry contains a paragraph break or a block, and the
    /// retrieve stage refused it, because a citation entry must be inline
    /// content. The retrieve stage reports this case as the error
    /// [`EntryNotInline`].
    NotInline,
    /// The retrieve stage could not bring the citation entry up to date as the
    /// document would show it ([value
    /// catch-up](crate::guide::concepts_overview#value-catch-up)), and refused
    /// it. For instance, a rewrite stage failed over the content of the
    /// citation entry.
    Unprocessable,
}

impl MissingEntry {
    /// Returns the text that the error [`NoEntry`](super::NoEntry) contains
    /// after the citation key to state this reason. For instance,
    /// [`MissingEntry::UnknownProvider`] returns `no appropriate citation
    /// provider found`.
    pub fn reason(&self) -> String {
        match self {
            MissingEntry::NoProviderInstalled => "no bibliography source set up".to_string(),
            MissingEntry::UnknownProvider => "no appropriate citation provider found".to_string(),
            MissingEntry::Failed(error) => format!("failed to retrieve the bibliography entry ({error})"),
            MissingEntry::Cites => "the bibliography entry may not use ‘\\cite’ recursively".to_string(),
            MissingEntry::NotInline => {
                "the bibliography entry must hold inline content only (no paragraph breaks or blocks)".to_string()
            }
            MissingEntry::Unprocessable => "failed to process the bibliography entry".to_string(),
        }
    }

    /// Returns the token that represents this reason in the serialized form of
    /// the table [`ENTRIES`], for instance `unknown` for
    /// [`MissingEntry::UnknownProvider`].
    fn token(&self) -> &'static str {
        match self {
            MissingEntry::NoProviderInstalled => "no-provider",
            MissingEntry::UnknownProvider => "unknown",
            MissingEntry::Failed(_) => "failed",
            MissingEntry::Cites => "cites",
            MissingEntry::NotInline => "not-inline",
            MissingEntry::Unprocessable => "unprocessable",
        }
    }
}

/// The result of the retrieve stage for one cited key: the answer of a
/// citation provider, or the reason why the citation key has no citation
/// entry.
#[derive(Debug, Clone)]
pub enum Retrieval {
    /// A citation provider returned this answer for the citation key, and the
    /// retrieve stage accepted its citation entry.
    Found(CitationAnswer),
    /// The citation key has no citation entry, for the given reason.
    Missing(MissingEntry),
}

/// The value of the table [`ENTRIES`]: the cited keys of the document, in
/// order of first citation, each with the result of the retrieve stage
/// ([`Retrieval`]).
///
/// The serialized form of the table ([`Table::project`]) is a list with one
/// map for each cited key, in order of first citation. Each map contains the
/// field `key`, with the citation key, and one of two other fields: `entry`,
/// with the plain text of the citation entry, or `missing`, with a token that
/// represents the reason why the citation key has no citation entry (for
/// instance `failed`).
#[derive(Debug, Clone, Default)]
pub struct CitationEntries {
    /// The citation keys, in order of first citation.
    keys: Vec<String>,
    /// The result of the retrieve stage for each citation key.
    retrieved: BTreeMap<String, Retrieval>,
}

impl CitationEntries {
    /// Returns the result of the retrieve stage for the citation key `key`, or
    /// `None` if `key` is not among the cited keys of the document
    /// ([`KEYS`]).
    pub fn get(&self, key: &str) -> Option<&Retrieval> {
        self.retrieved.get(key)
    }

    /// Returns the citation entry of the citation key `key`, or `None` if `key`
    /// has no citation entry or is not among the cited keys of the document.
    pub fn entry(&self, key: &str) -> Option<&CitationEntry> {
        match self.retrieved.get(key)? {
            Retrieval::Found(answer) => Some(&answer.entry),
            Retrieval::Missing(_) => None,
        }
    }

    /// Returns an iterator over the cited keys, in order of first citation,
    /// each with the result of the retrieve stage for it.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Retrieval)> {
        self.keys.iter().filter_map(|key| self.retrieved.get(key).map(|found| (key.as_str(), found)))
    }

    /// Returns the number of cited keys in the table.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the table contains no cited key.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Records `retrieval` for `key`, unless `key` already has one.
    fn record(&mut self, key: &str, retrieval: Retrieval) {
        if self.retrieved.contains_key(key) {
            return;
        }
        self.keys.push(key.to_string());
        self.retrieved.insert(key.to_string(), retrieval);
    }
}

impl Table for CitationEntries {
    /// Appends the citation keys of `piece` that this table does not contain
    /// yet, with their results, in their order in `piece`. A citation key
    /// that this table already contains keeps its result.
    fn absorb(&mut self, piece: CitationEntries) {
        let CitationEntries { keys, mut retrieved } = piece;
        for key in keys {
            if let Some(found) = retrieved.remove(&key) {
                self.record(&key, found);
            }
        }
    }

    /// Returns the serialized form of the table, which the documentation of
    /// [`CitationEntries`] describes.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.iter()
                .map(|(key, found)| {
                    let (field, value) = match found {
                        Retrieval::Found(answer) => ("entry", plain_text(&answer.entry.content)),
                        Retrieval::Missing(missing) => ("missing", missing.token().to_string()),
                    };
                    SerialValue::Map(vec![
                        ("key".to_string(), SerialValue::Str(key.to_string())),
                        (field.to_string(), SerialValue::Str(value)),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for CitationEntries {
    const NAME: &'static str = "flm.citations.entries";
}

/// Returns `true` if `tree` contains a citation, that is, a node of `\cite` or
/// of `\R`.
fn cites(tree: &NodeTree<Flm>) -> bool {
    tree.root().descendants().any(|node| element_value::<CitationElement>(&node).is_some())
}

/// Calls the citation providers `providers` for the citation keys `keys`, and
/// returns one result for each citation key, in the order of `keys`.
///
/// The citation providers are called in the order of `providers`, which is
/// the order of precedence of the service slot. Each citation provider
/// receives, in one call of [`CitationProvider::answer_all`], the citation
/// keys for which every citation provider before it returned `Ok(None)`. The
/// first citation provider that returns an answer or an error for a citation
/// key determines the result for this citation key.
///
/// If a citation provider returns a different number of results than it
/// received citation keys, the result for each of these citation keys is
/// [`MissingEntry::Failed`], and no later citation provider is called. A
/// citation key for which every citation provider returned `Ok(None)` has the
/// result [`MissingEntry::UnknownProvider`]. If `providers` is empty, every
/// citation key has the result [`MissingEntry::NoProviderInstalled`].
fn retrieve(keys: &[&str], providers: &[&dyn CitationProvider], cx: &ProviderCx<'_>) -> Vec<Retrieval> {
    if providers.is_empty() {
        return keys.iter().map(|_| Retrieval::Missing(MissingEntry::NoProviderInstalled)).collect();
    }
    let mut decided: Vec<Option<Retrieval>> = keys.iter().map(|_| None).collect();
    let mut pending: Vec<usize> = (0..keys.len()).collect();
    for provider in providers {
        if pending.is_empty() {
            break;
        }
        let requests: Vec<CitationRequest> = pending.iter().map(|&at| CitationRequest::new(keys[at])).collect();
        let answers = provider.answer_all(&requests, cx);
        if answers.len() != requests.len() {
            let error = CitationProviderError::new(format!(
                "the citation provider returned {} for {}; expected one result per key",
                counted(answers.len(), "result", "results"),
                counted(requests.len(), "key", "keys")
            ));
            for &at in &pending {
                decided[at] = Some(Retrieval::Missing(MissingEntry::Failed(error.clone())));
            }
            pending.clear();
            break;
        }
        let mut unclaimed = Vec::new();
        for (&at, answer) in pending.iter().zip(answers) {
            match answer {
                Ok(None) => unclaimed.push(at),
                Ok(Some(answer)) => decided[at] = Some(Retrieval::Found(answer)),
                Err(error) => decided[at] = Some(Retrieval::Missing(MissingEntry::Failed(error))),
            }
        }
        pending = unclaimed;
    }
    decided.into_iter().map(|found| found.unwrap_or(Retrieval::Missing(MissingEntry::UnknownProvider))).collect()
}

/// Returns `count` followed by the noun, `one` for a count of one and `many`
/// otherwise: `1 answer`, `2 answers`.
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// Checks the citation entry `entry`, and returns the reason for refusing it,
/// or `None` if the citation entry is accepted.
///
/// The content of the citation entry is first brought up to date as the
/// document would show it ([value
/// catch-up](crate::guide::concepts_overview#value-catch-up)): the rewrite
/// stages of the plan run over it, and expand the user definitions that it
/// uses. The citation entry is refused if this fails
/// ([`MissingEntry::Unprocessable`]), if its content then contains a citation
/// ([`MissingEntry::Cites`]), or if its content contains a paragraph break or
/// a block ([`MissingEntry::NotInline`]). The diagnostics reported while the
/// content is brought up to date are added to `diags`.
fn refusal(cx: &StageCx<'_>, entry: &CitationEntry, diags: &mut FlmDiagnostics) -> Option<MissingEntry> {
    // The entry is tested as the document would show it, so that a citation or
    // block content reached through a user definition is refused as it is when
    // written directly.
    let caught = match cx.resolve_value(&entry.content) {
        Ok(caught) => caught,
        Err(error) => {
            if let Some(diagnostic) = error.diagnostic() {
                diags.push(diagnostic.clone());
            }
            return Some(MissingEntry::Unprocessable);
        }
    };
    for diagnostic in caught.diags {
        diags.push(diagnostic);
    }
    if cites(&caught.tree) {
        return Some(MissingEntry::Cites);
    }
    let content: Vec<NodeRef<'_, Flm>> = caught.tree.root().children().iter().collect();
    if run_holds_blocks(&content) {
        return Some(MissingEntry::NotInline);
    }
    None
}

/// The retrieve stage [`RETRIEVE`], a compute stage that runs once for the
/// whole document.
///
/// The stage reads the cited keys from the table [`KEYS`], and calls the
/// citation providers of the service slot [`PROVIDERS_SLOT`] for them
/// ([`retrieve`]). It checks the citation entry of each answer ([`refusal`])
/// and writes the results to the table [`ENTRIES`]. Then it numbers the cited
/// works ([`number_works`]) and writes the numbers to the table [`CITATIONS`].
///
/// The cited works are numbered by the identities that the answers give
/// ([`CitationAnswer::identity_for`]). The identity in an answer counts also
/// when the stage refuses the citation entry of the answer. The identity of a
/// citation key without an answer is the citation key itself.
///
/// The stage reports the following diagnostics at the first citation of the
/// citation key concerned:
///
/// - [`ProviderFailed`] for an error of a citation provider;
/// - [`ProviderWarning`] for each warning text of an answer;
/// - [`EntryCites`] or [`EntryNotInline`] for a refused citation entry.
///
/// If the document cites works and the service slot contains no citation
/// provider, the stage reports [`NoProvider`] once, at the first citation of
/// the document. The diagnostics reported while a citation entry is brought up
/// to date are reported at the content of the citation entry.
#[derive(Debug, Default)]
pub(super) struct Retrieve;

impl Stage for Retrieve {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RETRIEVE, Effect::Compute, Scope::Document).writes(ENTRIES).writes(CITATIONS).docs(
            "Retrieves the citation entries of the cited keys from the citation providers in \
             `flm.citations.providers` and writes them to `flm.citations.entries`, then numbers \
             the cited works in order of first citation and writes the numbers to `flm.citations`.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(KEYS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let cited = match cx.get(KEYS) {
            Ok(Some(cited)) => cited,
            Ok(None) => return,
            Err(error) => return stage_failed(out, RETRIEVE, error),
        };
        let providers: Vec<&dyn CitationProvider> =
            cx.registry().services().services_by_precedence_named::<dyn CitationProvider>(PROVIDERS_SLOT).collect();
        let provider_cx = ProviderCx::new(cx.registry().lang(), cx.progress());
        if providers.is_empty() {
            if let Some((key, span)) = cited.iter().next() {
                out.diags.push(Diagnostic::error(NoProvider { key: key.to_string() }, span.clone()));
            }
        }
        let keys: Vec<&str> = cited.iter().map(|(key, _)| key).collect();
        let answers = retrieve(&keys, &providers, &provider_cx);
        let mut entries = CitationEntries::default();
        let mut identities: Vec<(&str, String)> = Vec::with_capacity(keys.len());
        for ((key, span), mut found) in cited.iter().zip(answers) {
            let identity = match &found {
                Retrieval::Found(answer) => answer.identity_for(key).to_string(),
                Retrieval::Missing(_) => key.to_string(),
            };
            identities.push((key, identity));
            if let Retrieval::Found(answer) = &found {
                for detail in &answer.warnings {
                    let warning = ProviderWarning { key: key.to_string(), detail: detail.clone() };
                    out.diags.push(Diagnostic::warning(warning, span.clone()));
                }
                if let Some(missing) = refusal(cx, &answer.entry, &mut out.diags) {
                    found = Retrieval::Missing(missing);
                }
            }
            let (owned, span) = (key.to_string(), span.clone());
            match &found {
                Retrieval::Missing(MissingEntry::Failed(error)) => out.diags.push(Diagnostic::error(
                    ProviderFailed { key: owned, detail: error.detail.clone() },
                    span,
                )),
                Retrieval::Missing(MissingEntry::Cites) => out.diags.push(Diagnostic::error(EntryCites { key: owned }, span)),
                Retrieval::Missing(MissingEntry::NotInline) => {
                    out.diags.push(Diagnostic::error(EntryNotInline { key: owned }, span))
                }
                _ => {}
            }
            entries.record(key, found);
        }
        let works = number_works(identities.iter().map(|(key, identity)| (*key, identity.as_str())));
        match out.table(ENTRIES) {
            Ok(table) => *table = entries,
            Err(error) => stage_failed(out, RETRIEVE, error),
        }
        match out.table(CITATIONS) {
            Ok(table) => *table = works,
            Err(error) => stage_failed(out, RETRIEVE, error),
        }
    }
}
