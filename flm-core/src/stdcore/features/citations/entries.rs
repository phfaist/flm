//! The citation entries of the cited works: the table `flm.citations.entries`
//! ([`ENTRIES`]), and the retrieve stage, which asks the citation providers
//! for them and then numbers the cited works.
//!
//! The retrieve stage reads the cited keys ([`KEYS`](super::KEYS)), asks the
//! citation providers of the service slot
//! [`PROVIDERS_SLOT`](super::PROVIDERS_SLOT), checks each citation entry,
//! and writes [`ENTRIES`] and [`CITATIONS`](super::CITATIONS). The list of
//! cited works, the citation marks under the in-place placement and the
//! check stage read [`ENTRIES`].

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

/// The name of the table of the citation entries, `flm.citations.entries`
/// ([`CitationEntries`]): for each cited key, its citation entry or the reason
/// it has none.
pub const ENTRIES: TableName<CitationEntries> = TableName::new("flm.citations.entries");

/// The name of the retrieve stage, `flm.citations.retrieve`, which writes
/// [`ENTRIES`] and [`CITATIONS`](super::CITATIONS).
pub const RETRIEVE: &str = "flm.citations.retrieve";

/// Why a cited key has no citation entry.
///
/// New reasons may be added.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissingEntry {
    /// The registry holds no citation provider.
    NoProvider,
    /// Every citation provider answered "not mine" for the citation key: none
    /// of them serves it.
    Unknown,
    /// The citation provider that claimed the citation key could not answer;
    /// the error holds its words.
    Failed(CitationProviderError),
    /// The citation entry that the citation provider returned holds a
    /// citation, and was refused.
    Cites,
    /// The citation entry that the citation provider returned holds a
    /// paragraph break or a block (a list, a heading), while a citation entry
    /// must be inline content, and was refused.
    NotInline,
    /// The citation entry that the citation provider returned could not be
    /// processed into the content the document shows (for example, a user
    /// definition in it expands without end), and was refused.
    Unprocessable,
}

impl MissingEntry {
    /// Returns the reason in a few plain words, as the check stage's
    /// diagnostic [`NoEntry`](super::NoEntry) states it: `no source of
    /// bibliography entries knows it`.
    pub fn reason(&self) -> String {
        match self {
            MissingEntry::NoProvider => "no source of bibliography entries is set up".to_string(),
            MissingEntry::Unknown => "no source of bibliography entries knows it".to_string(),
            MissingEntry::Failed(error) => format!("retrieving it failed: {error}"),
            MissingEntry::Cites => "its entry cites other works".to_string(),
            MissingEntry::NotInline => "its entry holds a paragraph break or a block".to_string(),
            MissingEntry::Unprocessable => "its entry could not be processed".to_string(),
        }
    }

    /// Returns the token that the table's serialized form writes for the
    /// reason.
    fn token(&self) -> &'static str {
        match self {
            MissingEntry::NoProvider => "no-provider",
            MissingEntry::Unknown => "unknown",
            MissingEntry::Failed(_) => "failed",
            MissingEntry::Cites => "cites",
            MissingEntry::NotInline => "not-inline",
            MissingEntry::Unprocessable => "unprocessable",
        }
    }
}

/// What the retrieve stage found for one cited key: a citation provider's
/// answer, or the reason the citation key has no citation entry.
#[derive(Debug, Clone)]
pub enum Retrieval {
    /// The citation provider's answer for the citation key: its citation
    /// entry and its work's identity.
    Found(CitationAnswer),
    /// The citation key has no citation entry, for this reason.
    Missing(MissingEntry),
}

/// The value of the table [`ENTRIES`]: every citation key the document cites,
/// in order of first citation, with its citation entry or the reason it has
/// none.
///
/// The retrieve stage writes it. The bibliography's builder stage, the check
/// stage and the rules of `\cite` and `\R` on every backend except the
/// recomposed LaTeX declare that they read it (the rules use it only under the
/// in-place placement). Because the check runs under every workflow, the
/// citation providers are asked under every workflow.
///
/// A citation key's citation entry is a fact about the document as a whole,
/// so a synthesized value may not contribute to this table
/// ([`PieceClass::DocumentFact`]). The serialized form ([`Table::project`]) is
/// a list of maps holding `key` and either `entry`, the citation entry's
/// plain text, or `missing`, a token for the reason.
#[derive(Debug, Clone, Default)]
pub struct CitationEntries {
    /// The citation keys, in order of first citation.
    keys: Vec<String>,
    /// What was found for each citation key.
    retrieved: BTreeMap<String, Retrieval>,
}

impl CitationEntries {
    /// Returns what was found for `key`, or `None` when the document does not
    /// cite `key` in the trees the collect stage read.
    pub fn get(&self, key: &str) -> Option<&Retrieval> {
        self.retrieved.get(key)
    }

    /// Returns the citation entry of `key`, or `None` for a citation key with
    /// no citation entry or not cited.
    pub fn entry(&self, key: &str) -> Option<&CitationEntry> {
        match self.retrieved.get(key)? {
            Retrieval::Found(answer) => Some(&answer.entry),
            Retrieval::Missing(_) => None,
        }
    }

    /// Returns every cited key with what was found for it, in order of first
    /// citation.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Retrieval)> {
        self.keys.iter().filter_map(|key| self.retrieved.get(key).map(|found| (key.as_str(), found)))
    }

    /// Returns how many citation keys the table holds.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns whether the table holds no citation key.
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
    /// The citation keys of the table piece that this table does not hold yet
    /// are added after this table's, in their order.
    fn absorb(&mut self, piece: CitationEntries) {
        let CitationEntries { keys, mut retrieved } = piece;
        for key in keys {
            if let Some(found) = retrieved.remove(&key) {
                self.record(&key, found);
            }
        }
    }

    /// `[{key, entry}]` for a citation key with a citation entry, its plain
    /// text; `[{key, missing}]` for one without, the reason's token; in order
    /// of first citation.
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

/// Whether `tree` holds a citation: a node of `\cite` or of `\R`.
fn cites(tree: &NodeTree<Flm>) -> bool {
    tree.root().descendants().any(|node| element_value::<CitationElement>(&node).is_some())
}

/// Returns what the citation providers answer for `keys`, one result per
/// citation key, in the same order. Each citation provider, in the order
/// given (the slot's order of precedence), receives in one call the citation
/// keys that no citation provider before it claimed; a citation provider's
/// answer or error decides a citation key.
fn retrieve(keys: &[&str], providers: &[&dyn CitationProvider], cx: &ProviderCx<'_>) -> Vec<Retrieval> {
    if providers.is_empty() {
        return keys.iter().map(|_| Retrieval::Missing(MissingEntry::NoProvider)).collect();
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
                "a source of bibliography entries must return one result for each key it is \
                 given, and it returned {} for {}",
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
    decided.into_iter().map(|found| found.unwrap_or(Retrieval::Missing(MissingEntry::Unknown))).collect()
}

/// Returns `count` followed by the noun, `one` for a count of one and `many`
/// otherwise: `1 answer`, `2 answers`.
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// Returns why `entry` is refused, or `None` when it is accepted. Its content
/// is brought up to date as the document would show it (value catch-up: the
/// plan's rewrites run over it, user definitions expanded among them; see
/// [value catch-up](crate::guide::concepts_overview#value-catch-up)). A
/// citation entry that then holds a citation, or holds block content rather
/// than inline content, is refused, and so is one that cannot be brought up
/// to date. The diagnostics reported while bringing it up to date go to
/// `diags`.
fn refusal(cx: &StageCx<'_>, entry: &CitationEntry, diags: &mut FlmDiagnostics) -> Option<MissingEntry> {
    // See decision rationale at [§flm-dd-dr:std-citations-stages].
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

/// The retrieve stage, [`RETRIEVE`]: a compute stage of document scope. It
/// reads the cited keys ([`KEYS`]), asks the citation providers of the service
/// slot [`PROVIDERS_SLOT`] about them, writes what it finds in [`ENTRIES`],
/// then numbers the cited works by the identities the answers give and writes
/// them in [`CITATIONS`].
///
/// The citation providers are asked in the order of the service slot, each
/// receiving in one call the citation keys that no citation provider before
/// it claimed ([`CitationProvider::answer_all`]). The first citation provider
/// that claims a citation key decides it: with an answer, or with an error
/// that is final for that citation key; the citation providers after it are
/// not asked about it. The answer's citation entry is then brought up to date as the
/// document would show it (the plan's rewrites run over it, user definitions
/// expanded among them) and refused when it holds a citation or block content
/// (a paragraph break, a list, a heading): a citation entry must be inline
/// content.
///
/// The cited works are numbered once every citation key has been asked about
/// ([`number_works`]). A citation key refers to the work whose identity its
/// citation provider's answer gives ([`CitationAnswer::identity_for`]), even
/// where its citation entry was refused; a citation key that no citation
/// provider answered refers to the work whose identity is the citation key as
/// written, so a citation key spelled exactly like a work's identity is a
/// citation key of that work.
///
/// It reports, at the first citation of the citation key: a citation
/// provider's failure ([`ProviderFailed`]), each warning of an answer
/// ([`ProviderWarning`]) and a refused citation entry ([`EntryCites`],
/// [`EntryNotInline`]). It reports once, at the first citation of the
/// document, that the document cites works and the registry holds no citation
/// provider ([`NoProvider`]). The diagnostics reported while bringing a
/// citation entry up to date are reported at the citation entry's own
/// content.
#[derive(Debug, Default)]
pub(super) struct Retrieve;

impl Stage for Retrieve {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RETRIEVE, Effect::Compute, Scope::Document).writes(ENTRIES).writes(CITATIONS).docs(
            "Asks the citation providers of `flm.citations.providers`, in order, about every \
             citation key the document cites and writes their citation entries into \
             `flm.citations.entries`; then numbers the cited works, in order of first citation, \
             into `flm.citations`.",
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
        let provider_cx = ProviderCx::new(cx.registry().lang());
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
