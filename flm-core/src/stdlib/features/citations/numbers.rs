//! This module defines the table of cited keys [`KEYS`], the table of cited
//! works [`CITATIONS`], the collect stage that writes the table of cited keys,
//! and the function [`number_works`] that numbers the cited works, along with
//! the error [`StageFailed`] that the stages of the `citations` feature report.
//!
//! The collect stage [`COLLECT`] records every citation key that the document
//! cites. The retrieve stage [`RETRIEVE`](super::RETRIEVE) then obtains the
//! citation entries of these citation keys from the citation providers. The
//! answer of a citation provider can give the *identity* of the cited work
//! ([`CitationAnswer`](super::CitationAnswer)), and citation keys with the same
//! identity refer to the same work. The retrieve stage numbers the cited works
//! with [`number_works`] and writes the result to the table of cited works.

use std::collections::BTreeMap;

use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmOrigin, FlmSpan, nowhere};
use crate::stdlib::features::index::{ElementIndex, ELEMENT_INDEX, walk_values_of};

use super::elements::{read_chunk, CitationElement, Chunk, Reading};

/// The name of the table of cited keys, `flm.citations.keys`, whose value is a
/// [`CitedKeys`]. The collect stage [`COLLECT`] writes this table. The retrieve
/// stage [`RETRIEVE`](super::RETRIEVE) and the check stage
/// [`CHECK`](super::CHECK) read it.
pub const KEYS: TableName<CitedKeys> = TableName::new("flm.citations.keys");

/// The name of the table of cited works, `flm.citations`, whose value is a
/// [`CitedWorks`]. The retrieve stage [`RETRIEVE`](super::RETRIEVE) writes this
/// table. The lowering rules of `\cite` and `\R` read the number and the
/// identifier of each cited work from this table, and the stage
/// [`BUILD_BIBLIOGRAPHY`](super::BUILD_BIBLIOGRAPHY) reads it to build the list
/// of cited works.
pub const CITATIONS: TableName<CitedWorks> = TableName::new("flm.citations");

/// The name of the collect stage, `flm.citations.collect`. This stage records
/// every citation key that the document cites in the table [`KEYS`].
pub const COLLECT: &str = "flm.citations.collect";

/// The prefix of the identifier of a cited work, `citation-`. The identifier
/// of the work numbered 3 is `citation-3`.
///
/// In the output, the entry of a work in the list of cited works has the
/// identifier of the work as its anchor, and the numbers in the citation marks
/// link to this anchor. For instance, in HTML output, the entry of the work
/// numbered 3 has the attribute `id="citation-3"`, and the number 3 in a
/// citation mark is a link to `#citation-3`.
pub const IDENTIFIER_PREFIX: &str = "citation-";

/// The value of the table of cited keys ([`KEYS`]). It contains every citation
/// key that the document cites, once, in order of first citation, that is, in
/// the order in which the citation keys first appear in the document.
///
/// Each citation key is stored with the span of its first citation: the span
/// of the `\cite` or `\R` node whose citation chunk ([`Chunk`]) first contains
/// the citation key. The stages of the `citations` feature report the
/// diagnostics about a citation key at this span, for instance
/// [`NoEntry`](super::NoEntry) for a citation key without a citation entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CitedKeys {
    /// The citation keys, in order of first citation, each with the span of
    /// its first citation.
    keys: Vec<(String, FlmSpan)>,
}

impl CitedKeys {
    /// Returns the cited keys of the citation chunks `chunks`. Each item of
    /// `chunks` is a citation chunk with the span of the `\cite` or `\R` node
    /// from which it was read, and the items must be in document order. The
    /// result contains each citation key once, with the span of the first
    /// citation chunk that contains it.
    pub fn of<'c, 't: 'c>(chunks: impl IntoIterator<Item = (&'c FlmSpan, &'c Chunk<'t>)>) -> CitedKeys {
        let mut keys = CitedKeys::default();
        for (span, chunk) in chunks {
            for item in &chunk.items {
                keys.cite(&item.key, span);
            }
        }
        keys
    }

    /// Returns every citation key with the span of its first citation, in
    /// order of first citation.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &FlmSpan)> {
        self.keys.iter().map(|(key, span)| (key.as_str(), span))
    }

    /// Returns the span of the first citation of `key`, or `None` if the table
    /// does not contain `key`.
    pub fn first_citation(&self, key: &str) -> Option<&FlmSpan> {
        self.keys.iter().find(|(cited, _)| cited == key).map(|(_, span)| span)
    }

    /// Returns how many distinct citation keys are cited.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns whether no citation key is cited.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Records `key`, first cited at `span`, unless it is already recorded.
    fn cite(&mut self, key: &str, span: &FlmSpan) {
        if !self.keys.iter().any(|(cited, _)| cited == key) {
            self.keys.push((key.to_string(), span.clone()));
        }
    }
}

impl Table for CitedKeys {
    /// Appends the citation keys of `piece` that this table does not contain
    /// yet, in their order and with their spans.
    fn absorb(&mut self, piece: CitedKeys) {
        for (key, span) in &piece.keys {
            self.cite(key, span);
        }
    }

    /// Serializes the table as the list of the citation keys, in order of
    /// first citation.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(self.iter().map(|(key, _)| SerialValue::Str(key.to_string())).collect())
    }

    fn piece_class(&self) -> PieceClass {
        if self.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for CitedKeys {
    const NAME: &'static str = "flm.citations.keys";
}

/// A `CitedWork` contains the number, the identifier and the identity of a
/// cited work. The table of cited works ([`CitedWorks`]) stores a `CitedWork`
/// for each citation key, and the citation keys that refer to the same work
/// have equal `CitedWork` values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedWork {
    /// The number of the work. The cited works are numbered from 1, in the
    /// order of the first citation of any of their citation keys.
    pub number: usize,
    /// The identifier of the work, as in `citation-1`: the prefix
    /// [`IDENTIFIER_PREFIX`] followed by the number. In the output, it is the
    /// anchor of the entry of the work in the list of cited works.
    pub identifier: String,
    /// The identity of the work. Citation keys with the same identity refer to
    /// the same work. The identity is the one that the citation provider gives
    /// in its answer for the citation key
    /// ([`CitationAnswer::identity`](super::CitationAnswer::identity)). If the
    /// citation key has no answer, or its answer gives no identity, the
    /// identity is the citation key itself.
    pub identity: String,
}

/// The value of the table of cited works ([`CITATIONS`]). It contains every
/// citation key that the document cites, in order of first citation, each
/// with the cited work ([`CitedWork`]) that it refers to. Citation keys with
/// the same identity refer to the same work (see [`CitedWork::identity`]). The
/// function [`number_works`] creates this value.
///
/// The table is keyed by the text of the citation keys
/// ([`work`](CitedWorks::work)), not by nodes. Therefore a citation that is
/// copied, for instance with the title of a heading that a reference displays,
/// expands to the same numbers as the original citation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CitedWorks {
    /// Every citation key, in order of first citation.
    keys: Vec<String>,
    /// Each citation key's work.
    works: BTreeMap<String, CitedWork>,
    /// The citation key of the first citation of each work, in the order of
    /// the numbers. The work numbered `n` is at index `n - 1`.
    firsts: Vec<String>,
    /// Each identity's number.
    numbers: BTreeMap<String, usize>,
}

impl CitedWorks {
    /// Returns the work that the citation key `key` refers to, or `None` if the
    /// table does not contain `key`. This is the case for a citation key that
    /// is cited only outside the trees of the document (see
    /// [`Uncollected`](super::Uncollected)).
    pub fn work(&self, key: &str) -> Option<&CitedWork> {
        self.works.get(key)
    }

    /// Returns every cited work once, in the order of the numbers, with the
    /// citation key under which it was first cited.
    pub fn works(&self) -> impl Iterator<Item = (&str, &CitedWork)> {
        self.firsts.iter().filter_map(|key| self.works.get(key).map(|work| (key.as_str(), work)))
    }

    /// Returns every cited key with its work, in order of first citation.
    pub fn keys(&self) -> impl Iterator<Item = (&str, &CitedWork)> {
        self.keys.iter().filter_map(|key| self.works.get(key).map(|work| (key.as_str(), work)))
    }

    /// Returns the number of cited works. Citation keys that refer to the same
    /// work count as one work.
    pub fn len(&self) -> usize {
        self.firsts.len()
    }

    /// Returns whether no work is cited.
    pub fn is_empty(&self) -> bool {
        self.firsts.is_empty()
    }

    /// Records `key` as a citation key of the work with the identity
    /// `identity`, unless the table already contains `key`. If a work with
    /// this identity is already numbered, `key` refers to this work.
    /// Otherwise, `key` refers to a new work with the next number.
    fn cite(&mut self, key: &str, identity: &str) {
        if self.works.contains_key(key) {
            return;
        }
        let number = match self.numbers.get(identity) {
            Some(number) => *number,
            None => {
                self.firsts.push(key.to_string());
                let number = self.firsts.len();
                self.numbers.insert(identity.to_string(), number);
                number
            }
        };
        let identifier = format!("{IDENTIFIER_PREFIX}{number}");
        self.keys.push(key.to_string());
        self.works.insert(key.to_string(), CitedWork { number, identifier, identity: identity.to_string() });
    }
}

impl Table for CitedWorks {
    /// Records the citation keys of `piece` that this table does not contain
    /// yet, in their order, each with the identity of its work. The works are
    /// numbered in the same way as by [`number_works`].
    fn absorb(&mut self, piece: CitedWorks) {
        for (key, work) in piece.keys() {
            self.cite(key, &work.identity);
        }
    }

    /// Serializes the table as a list with one map for each citation key, in
    /// order of first citation. Each map has the keys `key`, `number`, `id`
    /// (the identifier of the work) and `identity`.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.keys()
                .map(|(key, work)| {
                    SerialValue::Map(vec![
                        ("key".to_string(), SerialValue::Str(key.to_string())),
                        ("number".to_string(), SerialValue::Int(work.number as i64)),
                        ("id".to_string(), SerialValue::Str(work.identifier.clone())),
                        ("identity".to_string(), SerialValue::Str(work.identity.clone())),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for CitedWorks {
    const NAME: &'static str = "flm.citations";
}

/// Numbers the cited works of the citation keys `keys`, and returns the result
/// as the value of the table of cited works ([`CITATIONS`]).
///
/// Each item of `keys` is a pair of a citation key and the identity of the
/// work that the citation key refers to (see [`CitedWork::identity`]).
/// Citation keys with the same identity refer to the same work. The identities
/// are compared as exact strings. The works are numbered from 1, in the order
/// in which their identities first appear in `keys`. If `keys` contains a
/// citation key more than once, only its first pair counts.
///
/// The retrieve stage [`RETRIEVE`](super::RETRIEVE) calls this function once,
/// after it has obtained the answers of the citation providers. It passes
/// every cited key of the document, in order of first citation, so that the
/// works are numbered in the order of their first citation.
pub fn number_works<'k>(keys: impl IntoIterator<Item = (&'k str, &'k str)>) -> CitedWorks {
    let mut works = CitedWorks::default();
    for (key, identity) in keys {
        works.cite(key, identity);
    }
    works
}

/// Returns every citation chunk in the trees of the document, in document
/// order, each with the span of the `\cite` or `\R` node from which it was
/// read. The citation chunks are read with [`Reading::InStage`]: a `\cite`
/// call whose citation keys contain a placeholder of a user definition
/// contributes no citation key.
fn document_chunks<'t>(
    cx: &StageCx<'t>,
    index: &'t ElementIndex,
) -> Vec<(FlmSpan, Chunk<'t>)> {
    let mut chunks = Vec::new();
    for (_, node, _) in walk_values_of::<CitationElement>(cx, index) {
        // Read in a stage, a chunk is never refused.
        if let Ok(chunk) = read_chunk(&node, Reading::InStage) {
            chunks.push((node.span().clone(), chunk));
        }
    }
    chunks
}

/// Reports the error [`StageFailed`] for the stage named `stage`, with the
/// description `detail` of the failure.
pub(super) fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// The error that is reported when a stage of the `citations` feature fails
/// (id `flm.citations.stage-failed`).
///
/// A stage fails when it cannot read or write one of its tables. This happens
/// only when the code of a feature is faulty: a stage accesses a table that it
/// did not declare, or accesses a table with a value type that differs from
/// the declared one (see
/// [`AccessError`](crate::core::data::store::AccessError)). The stage
/// [`BUILD_BIBLIOGRAPHY`](super::BUILD_BIBLIOGRAPHY) also fails when it cannot
/// build the list of cited works (see [`build_list`](super::build_list)).
///
/// Reported at no position in the document. A stage that cannot read a table
/// stops, and each table that a failed stage did not write is empty. The
/// consequences for the document depend on the stage:
///
/// - If the collect stage [`COLLECT`] fails, no citation key is numbered.
///   Each `\cite` and `\R` then reports [`Uncollected`](super::Uncollected)
///   for each of its citation keys, and expands to a dropped mark
///   (`flm.pres.dropped`).
/// - If the retrieve stage [`RETRIEVE`](super::RETRIEVE) fails, the table of
///   cited works ([`CITATIONS`]) or the table of citation entries
///   ([`ENTRIES`](super::ENTRIES)) is empty. Without the table of cited works,
///   the citations expand to dropped marks, as after a failure of the collect
///   stage. Without the table of citation entries, the check stage reports
///   [`NoEntry`](super::NoEntry) for each citation key.
/// - If the check stage [`CHECK`](super::CHECK) fails, no citation key is
///   reported for a missing citation entry.
/// - If the stage [`BUILD_BIBLIOGRAPHY`](super::BUILD_BIBLIOGRAPHY) fails, the
///   list of cited works has no content and does not appear in the output.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.citations.stage-failed", message = "Stage ‘{stage}’ failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage: `flm.citations.collect`,
    /// `flm.citations.retrieve`, `flm.citations.check` or
    /// `flm.citations.build-bibliography`.
    pub stage: String,
    /// The description of the failure.
    pub detail: String,
}

/// The collect stage `flm.citations.collect` ([`COLLECT`]) records every
/// citation key that the document cites in the table of cited keys
/// ([`KEYS`]). The stage runs once for the whole document. It finds the
/// `\cite` and `\R` nodes in the element index ([`ELEMENT_INDEX`]), reads the
/// citation chunk of each node, and records each citation key at its first
/// citation.
///
/// The stage does not number the cited works, because the numbers depend on
/// the identities of the works, which the retrieve stage obtains later from
/// the citation providers. The stage does not report mistakes in the
/// citations; the lowering rules of `\cite` and `\R` report them.
///
/// The stage reports [`StageFailed`] when it cannot read the element index or
/// write the table of cited keys.
#[derive(Debug, Default)]
pub(super) struct Collect;

impl Stage for Collect {
    fn decl(&self) -> StageDecl {
        StageDecl::new(COLLECT, Effect::Compute, Scope::Document).writes(KEYS).docs(
            "Writes every citation key that the document cites to the table \
             `flm.citations.keys`, in order of first citation, with the position of its first \
             citation in the source.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(ELEMENT_INDEX)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let failed = |out: &mut StageOut<'_>, detail: String| stage_failed(out, COLLECT, detail);
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return failed(out, error.to_string()),
        };
        let chunks = document_chunks(cx, index);
        let keys = CitedKeys::of(chunks.iter().map(|(span, chunk)| (span, chunk)));
        match out.table(KEYS) {
            Ok(table) => *table = keys,
            Err(error) => failed(out, error.to_string()),
        }
    }
}
