//! The cited keys and the numbers of the cited works: the table
//! `flm.citations.keys` ([`KEYS`]) with the collect stage that writes it, the
//! table `flm.citations` ([`CITATIONS`]), and [`number_works`], the only
//! function that numbers cited works.
//!
//! The collect stage records which citation keys the document cites. The
//! retrieve stage ([`RETRIEVE`](super::RETRIEVE)) then asks the citation
//! providers about them and calls [`number_works`] with the identity each
//! answer gives, which decides which citation keys refer to one cited work.
//! This module also holds the condition [`StageFailed`], which every stage of
//! the feature reports when it cannot read or write a table it declared.

use std::collections::BTreeMap;

use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmOrigin, FlmSpan, nowhere};
use crate::stdcore::features::index::{ElementIndex, ELEMENT_INDEX, walk_values_of};

use super::elements::{read_chunk, CitationElement, Chunk, Reading};

/// The name of the table of the cited keys, `flm.citations.keys`
/// ([`CitedKeys`]): every distinct citation key the document cites, in order
/// of first citation, with where it is first cited. The collect stage
/// ([`COLLECT`]) writes it.
pub const KEYS: TableName<CitedKeys> = TableName::new("flm.citations.keys");

/// The name of the table of the cited works, `flm.citations` ([`CitedWorks`]):
/// for each citation key, the work it refers to, with the work's number,
/// identifier and identity. The retrieve stage
/// ([`RETRIEVE`](super::RETRIEVE)) writes it.
pub const CITATIONS: TableName<CitedWorks> = TableName::new("flm.citations");

/// The name of the collect stage, `flm.citations.collect`, which writes
/// [`KEYS`].
pub const COLLECT: &str = "flm.citations.collect";

/// The prefix of a cited work's identifier, `citation-`: the work numbered 3
/// has the identifier `citation-3`. A citation mark's links point at it, and
/// the list of cited works places it on the work's entry.
///
/// The prefix belongs to this feature, as `fn-` belongs to the footnotes and
/// `eq-` to the equations: no other feature creates identifiers with it, so the
/// identifiers of the cited works never coincide with any other identifier of
/// a document.
pub const IDENTIFIER_PREFIX: &str = "citation-";

/// The value of the table [`KEYS`]: every distinct citation key the document
/// cites, in order of first citation, each with the span of the node that
/// first cites it. A diagnostic about the citation key is reported at that
/// span.
///
/// Which citation keys a document cites is a fact about the document as a
/// whole, so a synthesized value may not contribute to this table
/// ([`PieceClass::DocumentFact`]). The serialized form ([`Table::project`]) is
/// the list of the citation keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CitedKeys {
    /// The citation keys, in order of first citation, each with the span of
    /// its first citation.
    keys: Vec<(String, FlmSpan)>,
}

impl CitedKeys {
    /// Returns the citation keys cited in `chunks`, a sequence of citation
    /// chunks in document order, each with the span of the node carrying it:
    /// each citation key once, in order of first citation, with the span of
    /// the citation chunk that first cites it.
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

    /// Returns the span of the first citation of `key`, or `None` when the
    /// collect stage found no citation of `key` in the document's trees.
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
    /// The citation keys of the table piece that this table does not hold yet
    /// are added after this table's, in their order.
    fn absorb(&mut self, piece: CitedKeys) {
        for (key, span) in &piece.keys {
            self.cite(key, span);
        }
    }

    /// `[key]`, in order of first citation.
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

/// One cited work, as the table [`CITATIONS`] records it for each citation
/// key that refers to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedWork {
    /// The work's number: its position, from 1, among the cited works ordered
    /// by the first citation of any of their citation keys.
    pub number: usize,
    /// The work's identifier, [`IDENTIFIER_PREFIX`] followed by the number:
    /// `citation-1`. A citation mark links to it, and the work's entry in a
    /// list of cited works carries it.
    pub identifier: String,
    /// The work's identity, as a citation provider gave it for the citation
    /// keys that refer to the work, or the citation key as written where no
    /// citation provider gave one ([`CitationAnswer`](super::CitationAnswer)).
    pub identity: String,
}

/// The value of the table [`CITATIONS`]: every citation key the document
/// cites, with the cited work it refers to. Several citation keys refer to one
/// work when their citation providers' answers gave the same identity.
/// [`number_works`] builds it.
///
/// A citation key is looked up by its text ([`work`](CitedWorks::work)),
/// never by the node that cites it. A citation mark reads its citation chunk's
/// keys from its own node and looks up each one, so a citation mark shown
/// again away from where it is written (in a heading's title that a reference
/// shows, for example) shows the same numbers.
///
/// Which number a citation key has is a fact about the document as a whole,
/// so a synthesized value may not contribute to this table
/// ([`PieceClass::DocumentFact`]). The serialized form ([`Table::project`]) is
/// a list of maps with the keys `key`, `number`, `id` and `identity`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CitedWorks {
    /// Every citation key, in order of first citation.
    keys: Vec<String>,
    /// Each citation key's work.
    works: BTreeMap<String, CitedWork>,
    /// For each number, from 1, the citation key under which its work was
    /// first cited.
    firsts: Vec<String>,
    /// Each identity's number.
    numbers: BTreeMap<String, usize>,
}

impl CitedWorks {
    /// Returns the work cited under `key`, or `None` when the document does not
    /// cite `key` in the trees the retrieve stage read.
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

    /// Returns how many works are cited.
    pub fn len(&self) -> usize {
        self.firsts.len()
    }

    /// Returns whether no work is cited.
    pub fn is_empty(&self) -> bool {
        self.firsts.is_empty()
    }

    /// Records `key` as referring to the work `identity`, unless `key` is
    /// already recorded: with that work's number where the identity already
    /// has one, else with the next number.
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
    /// The citation keys of the table piece that this table does not hold yet
    /// are recorded after this table's, each referring to its work's identity
    /// (and numbered as [`number_works`] numbers).
    fn absorb(&mut self, piece: CitedWorks) {
        for (key, work) in piece.keys() {
            self.cite(key, &work.identity);
        }
    }

    /// `[{key, number, id, identity}]`, every citation key in order of first
    /// citation.
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

/// Numbers the cited works of `keys` and returns them as the value of the
/// table [`CITATIONS`].
///
/// `keys` holds pairs of a citation key and the identity of the work it
/// refers to, in order of first citation. The function makes one work per
/// distinct identity and numbers the works in the order of the first
/// citation of any of their citation keys: the work of the first citation key
/// is 1, and a citation key whose identity already has a number refers to
/// that work. Identities are compared as characters. A citation key given
/// twice keeps its first pair.
///
/// This is the only function that gives a cited work its number. The
/// retrieve stage calls it once, after the citation providers have been
/// asked, over every citation key of the document with the identity its
/// citation provider's answer gave (the citation key itself where no answer
/// gave one). A caller that lists the works of one part of a document calls
/// it over that part's citation keys, makes a
/// [`ListedWork`](super::ListedWork) of each work of the result
/// ([`CitedWorks::works`]) with its citation entry, and passes them to
/// [`build_list`](super::build_list).
pub fn number_works<'k>(keys: impl IntoIterator<Item = (&'k str, &'k str)>) -> CitedWorks {
    let mut works = CitedWorks::default();
    for (key, identity) in keys {
        works.cite(key, identity);
    }
    works
}

/// Every citation chunk of the run's frozen trees, in the run's document
/// order, each with the span of the node carrying it, read with
/// [`Reading::InStage`]. The collect stage records the citation keys of these
/// chunks.
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

/// Reports [`StageFailed`] for the stage `stage`, which could not read or
/// write a table it declared; `detail` says what failed.
pub(super) fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// Condition `flm.citations.stage-failed`, an Error: a stage of the
/// `citations` feature could not read or write a table it declared. This
/// happens only when the run's plan does not match the stage's declarations,
/// a fault in the program rather than something a document causes. The table
/// the stage writes is then absent. Reported with no position in the source.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.citations.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage.
    pub stage: String,
    /// What failed, rendered.
    pub detail: String,
}

/// The collect stage, [`COLLECT`]: a compute stage of document scope. It
/// walks the element index ([`ELEMENT_INDEX`]) for the `\cite` and `\R`
/// nodes, in the run's document order, reads each citation chunk's items and
/// records every distinct citation key at its first citation in [`KEYS`].
///
/// It reads no citation entry and numbers nothing, because the cited works
/// are numbered only once the citation providers have given their
/// identities. It reports no mistake in the writing of a citation chunk: the
/// lowering rules that lower the citation chunk report those.
#[derive(Debug, Default)]
pub(super) struct Collect;

impl Stage for Collect {
    fn decl(&self) -> StageDecl {
        StageDecl::new(COLLECT, Effect::Compute, Scope::Document).writes(KEYS).docs(
            "Records every citation key that the document's `\\cite` and `\\R` commands cite, in \
             order of first citation, with where it is first cited, into `flm.citations.keys`.",
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
