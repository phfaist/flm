//! The table of every xnote of a document, `flm.xnotes` ([`Xnotes`]), the
//! collect stage that writes it, `flm.xnotes.collect`, and the standard
//! builder of an xnote kind's list, [`ListBuilder`].
//!
//! A table is typed data that one stage writes beside the frozen trees and
//! other stages read ([`Table`]); a stage is one unit of work of the pipeline
//! ([`Stage`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::piece::Piece;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::copy::{SynthesizedPiece, ValueContent, ValuePiece, copy_pieces_into_value};
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::{Flm, FlmOrigin, FlmSpan, nowhere};
use crate::model::addr::NodeAddr;
use crate::model::aspects::PointId;
use crate::model::names::{Name, RefKind};
use crate::core::lowering::NodeRegion;
use crate::stdcore::backends::lowering::ContentPointer;
use crate::stdcore::features::numbering::{self, COUNTERS};
use crate::stdcore::features::refpoints::{REFPOINTS, point_key};

use super::config::{XnoteKind, XnotePlacement};
use super::diagnostics::{NoNumbering, Unpaired};
use super::marks::{Compiled, FixedMarks};
use super::{
    builder_name, contents_of_kind, list_element_name, piece_table, COLLECT, CONTENT_POINTER, ENTRY,
    FEATURE, KINDS_SLOT, mark_point_kind,
};

/// The name of the content slot of a list node and of an entry node, `body`.
const BODY_SLOT: &str = "body";

/// The typed name of the table of xnotes, `flm.xnotes` ([`Xnotes`]); like
/// every table name, it is also a capability
/// ([`CapabilityDef`](crate::core::registry::CapabilityDef): a registered
/// dotted name for a state of processing that a stage brings about), the one
/// that a stage reading the table requires.
pub const XNOTES: TableName<Xnotes> = TableName::new("flm.xnotes");

/// One xnote of the document, as the collect stage records it: its two
/// reference points, where it was written, and the region of its node that
/// holds its content.
///
/// The record holds identities, never what is shown: where the xnote is shown,
/// its number is read from the numbering feature's counter table for
/// [`point`], and the anchors of the xnote and of its mark from the
/// references feature's anchors table for [`point`] and [`mark`].
///
/// [`point`]: XnoteRecord::point
/// [`mark`]: XnoteRecord::mark
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XnoteRecord {
    /// The xnote's own reference point: the one a `\ref` to it resolves to,
    /// numbered for a numbered kind.
    pub point: PointId,
    /// The reference point of its mark, the point after [`point`] on the
    /// same node: the target of the link back from the entry of a listed
    /// xnote to where the xnote was written.
    ///
    /// [`point`]: XnoteRecord::point
    pub mark: PointId,
    /// The source span of the node carrying the two points.
    pub span: FlmSpan,
    /// The region of the node that holds the xnote's content, as the node's
    /// [`XnoteSource`](super::XnoteSource) aspect returns it for this xnote,
    /// under the pairing that aspect states: `None` where the aspect returns
    /// fewer xnotes of the kind than the node carries pairs of points for.
    pub content: Option<NodeRegion>,
}

/// The table of xnotes, `flm.xnotes`: every xnote of the document, by xnote
/// kind, each kind's xnotes in document order (the order of the run's trees,
/// and within a tree the order in which its nodes are written).
///
/// The collect stage ([`COLLECT`](super::COLLECT)) writes it. Every xnote
/// kind of the service slot of xnote kinds has an entry, empty when the
/// document holds none of its xnotes. An xnote is what its reference points
/// say it is: a point whose reference kind is an xnote kind's name, followed
/// on the same node by a point whose reference kind is that name and `-mark`
/// ([`ref_points`](super::ref_points) returns the two). The stage consults the
/// element carrying the points only for the region of each xnote's content
/// ([`XnoteRecord::content`]), so the nodes of another feature that return
/// those points are recorded like any `\footnote`. A node may carry several
/// xnotes, of one kind or of several: each pair of points is one record, and
/// the records of one node are in the order of its points. An xnote inside
/// another xnote's content is recorded like any other, in document order.
///
/// The table records the document's own nodes only. An xnote node in content
/// that the run builds itself, apart from the document's trees (such as a
/// list's own label template), has an address
/// of its own and no record here, and a reader can tell from this that such an
/// xnote is not one the document wrote ([`record_at`](Xnotes::record_at)
/// returns `None`). Content shown again elsewhere (a heading's title that a
/// reference shows) holds no xnote node but a node that represents it
/// ([`MarkOf`](super::MarkOf)), which finds the records of the xnotes it
/// represents by the span at which their node was written
/// ([`records_written_at`](Xnotes::records_written_at)).
///
/// The table is classed as a document fact ([`PieceClass::DocumentFact`]):
/// its records describe the document's own nodes, and a stage that runs over
/// a [synthesized value](crate::guide::concepts_overview#synthesized-values)
/// cannot add records to it (the store refuses them with
/// [`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue)).
///
/// The table also carries the content of each unnumbered kind's fixed mark,
/// compiled when the registry was built ([`fixed_mark`](Xnotes::fixed_mark)).
/// It belongs to the registry, is the same for every document and is no fact
/// about this one, so it is neither compared nor projected.
#[derive(Debug, Clone, Default)]
pub struct Xnotes {
    /// The records, by the xnote kind's name, each kind's in document order.
    pub by_kind: BTreeMap<Name, Vec<XnoteRecord>>,
    /// The content that each unnumbered kind's fixed mark writes, by the
    /// kind's name.
    fixed_marks: Compiled,
}

impl PartialEq for Xnotes {
    /// Compares the records alone: the fixed marks belong to the registry.
    fn eq(&self, other: &Xnotes) -> bool {
        self.by_kind == other.by_kind
    }
}

impl Eq for Xnotes {}

impl Xnotes {
    /// The records of the xnote kind `kind`, in document order; empty for a
    /// kind with no xnote, and for a name that is no kind's.
    pub fn of_kind(&self, kind: &str) -> &[XnoteRecord] {
        self.by_kind.get(kind).map_or(&[], Vec::as_slice)
    }

    /// The xnote kind and the record of the xnote carried by the node at
    /// `addr`, or `None` for a node that carries no recorded xnote (a node
    /// that is no xnote, or one in content copied during a run, whose address
    /// is not one of the document's).
    ///
    /// The lookup goes through every record; a lowering rule makes it once per
    /// xnote it writes. For a node carrying several xnotes, it returns the
    /// first record of the first kind in name order, which tells a rule that
    /// the node's xnotes are recorded with a kind other than the one it
    /// writes. [`records_at`](Xnotes::records_at) returns the records of one
    /// kind.
    pub fn record_at(&self, addr: NodeAddr) -> Option<(&Name, &XnoteRecord)> {
        self.by_kind.iter().find_map(|(kind, records)| {
            records.iter().find(|record| record.point.addr == addr).map(|record| (kind, record))
        })
    }

    /// The records of the xnote kind `kind` carried by the node at `addr`, in
    /// the node's order: the first is the node's first xnote of that kind,
    /// which the pairing stated on [`XnoteSource`](super::XnoteSource) pairs
    /// with the first region of that kind that the node's aspect
    /// returns, and so on. Empty for a node that carries no recorded xnote of
    /// that kind.
    pub fn records_at(&self, kind: &str, addr: NodeAddr) -> impl Iterator<Item = &XnoteRecord> {
        self.of_kind(kind).iter().filter(move |record| record.point.addr == addr)
    }

    /// The records of the xnote kind `kind` whose node was written at `span`
    /// (the same range of the same source), in document order; empty where no
    /// xnote of that kind was written there.
    ///
    /// A node that represents a node's xnotes in content shown again
    /// ([`MarkOf`](super::MarkOf)) finds them with this function: every
    /// copied node keeps the span of the node it was copied from, and a copy's
    /// node has no address among the document's.
    pub fn records_written_at<'s>(
        &'s self,
        kind: &str,
        span: &'s FlmSpan,
    ) -> impl Iterator<Item = &'s XnoteRecord> {
        self.of_kind(kind).iter().filter(move |record| {
            record.span.start() == span.start() && record.span.end() == span.end() && record.span.same_source(span)
        })
    }

    /// Whether the table holds no record at all.
    pub fn is_empty(&self) -> bool {
        self.by_kind.values().all(Vec::is_empty)
    }

    /// The content that the fixed mark of the unnumbered xnote kind `kind`
    /// writes, as synthesized value trees ready to be spliced
    /// ([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)),
    /// compiled as FLM text from the kind's [`mark`](XnoteKind::mark)
    /// template when the registry was built. Empty for a kind with an empty
    /// mark, for a numbered kind (whose mark is its number, from the numbering
    /// feature's counter table), and for a name that is no kind of the
    /// registry.
    pub fn fixed_mark(&self, kind: &str) -> &[Arc<NodeTree<Flm>>] {
        self.fixed_marks.get(kind).map_or(&[], Vec::as_slice)
    }
}

impl Table for Xnotes {
    /// Appends the table piece's records after this table's, kind by kind.
    fn absorb(&mut self, piece: Xnotes) {
        // The fixed marks are the registry's, the same map in every piece.
        if !piece.fixed_marks.is_empty() {
            self.fixed_marks = piece.fixed_marks;
        }
        for (kind, records) in piece.by_kind {
            self.by_kind.entry(kind).or_default().extend(records);
        }
    }

    /// `{<kind>: [{point, mark}]}`, the kinds in name order, each kind's
    /// records in document order, each point keyed `<node>#<index>` as the
    /// table of reference points keys it.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.by_kind
                .iter()
                .map(|(kind, records)| {
                    let records = records
                        .iter()
                        .map(|record| {
                            SerialValue::Map(vec![
                                ("point".to_string(), SerialValue::Str(point_key(cx, record.point))),
                                ("mark".to_string(), SerialValue::Str(point_key(cx, record.mark))),
                            ])
                        })
                        .collect();
                    (kind.to_string(), SerialValue::List(records))
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for Xnotes {
    const NAME: &'static str = "flm.xnotes";
}

/// Condition `flm.xnotes.stage-failed`: a stage of the feature (the collect
/// stage, or an xnote kind's list builder) could not read or write a table it
/// declared, which means the plan does not match the stage's declarations; or
/// a list builder could not assemble the list. Either is a fault in the
/// program, not something a document causes. Recovery: the stage's table is
/// absent (`flm.xnotes`, or the kind's list, which is then not placed).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.xnotes.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The stage.
    pub stage: String,
    /// What failed, rendered.
    pub detail: String,
}

/// Reports [`StageFailed`] for the stage `stage`.
fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// The collect stage, [`COLLECT`]: a computing stage of document scope that
/// writes the table [`XNOTES`] (see [`Xnotes`] for what it records). It
/// carries the compiled fixed marks `marks` into the table.
pub(super) struct Collect {
    pub(super) marks: Arc<FixedMarks>,
}

impl Stage for Collect {
    fn decl(&self) -> StageDecl {
        StageDecl::new(COLLECT, Effect::Compute, Scope::Document).writes(XNOTES).docs(
            "Records every xnote of the document (a reference point whose kind is an xnote \
             kind's name, followed on the same node by the point of its mark) into `flm.xnotes`, \
             by kind and in document order, each with the region of its content. Reports the \
             numbered kinds that no numbering feature numbers, and the nodes whose number of \
             xnotes of a kind differs from their number of regions holding an xnote's content.",
        )
    }

    /// Reads the table of reference points, and the counter table
    /// `flm.counters` wherever the numbering feature is registered; the stage
    /// reports a numbered kind when the counter table is absent. The read of
    /// the counter table is a required read where it is made, as in the
    /// references feature's anchors stage, so that the counter table exists in
    /// every plan that schedules this stage. It is made only where the
    /// numbering feature is registered, because the construction of a plan
    /// fails for a stage that reads a table no feature declares.
    // See decision rationale at [§flm-dd-dr:std-xnote-collect].
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let needs = Needs::new().reads(REFPOINTS);
        if cx.has_feature(numbering::FEATURE) { needs.reads(COUNTERS) } else { needs }
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let failed = |out: &mut StageOut<'_>, detail: String| stage_failed(out, COLLECT, detail);
        let points = match cx.get(REFPOINTS) {
            Ok(Some(points)) => points,
            Ok(None) => return,
            Err(error) => return failed(out, error.to_string()),
        };
        let numbered = cx.registry().has_feature(numbering::FEATURE)
            && match cx.get(COUNTERS) {
                Ok(counters) => counters.is_some(),
                Err(error) => return failed(out, error.to_string()),
            };
        let kinds: BTreeMap<&str, (&XnoteKind, RefKind)> = cx
            .registry()
            .services()
            .services_named::<XnoteKind>(KINDS_SLOT)
            .map(|kind| (kind.name.as_str(), (kind, mark_point_kind(kind.name.as_str()))))
            .collect();
        // Every kind of the slot has an entry, empty where no xnote is of it.
        // See decision rationale at [§flm-dd-dr:std-xnote-every-kind].
        let mut xnotes = Xnotes {
            by_kind: kinds.keys().map(|name| (Name::from(*name), Vec::new())).collect(),
            fixed_marks: self.marks.compiled(),
        };
        // The xnotes of each kind met so far on each node, with the node's
        // span: the position of the next one among the node's xnotes of that
        // kind, which pairs it with its content (`XnoteSource`).
        let mut met: BTreeMap<(NodeAddr, &str), (usize, FlmSpan)> = BTreeMap::new();
        for record in points.in_document_order(cx) {
            let Some((kind, mark_kind)) = kinds.get(record.point.kind.as_str()) else { continue };
            let Some(index) = record.id.index.checked_add(1) else { continue };
            let mark = PointId { addr: record.id.addr, index };
            // A point of a kind's name with no mark point after it is no
            // xnote: `ref_points` answers the two together.
            // See decision rationale at [§flm-dd-dr:std-xnote-pairing].
            if points.point(mark).is_none_or(|next| next.point.kind != *mark_kind) {
                continue;
            }
            let name = kind.name.as_str();
            let nth = &mut met.entry((record.id.addr, name)).or_insert_with(|| (0, record.span.clone())).0;
            // See decision rationale at [§flm-dd-dr:std-xnote-several].
            let content = cx
                .frozen_tree(record.id.addr.tree)
                .and_then(|tree| tree.get(record.id.addr.node))
                .and_then(|node| contents_of_kind(&node, name).get(*nth).copied());
            *nth += 1;
            if let Some(records) = xnotes.by_kind.get_mut(name) {
                records.push(XnoteRecord { point: record.id, mark, span: record.span.clone(), content });
            }
        }
        for ((addr, kind), (pairs, span)) in &met {
            let Some(node) = cx.frozen_tree(addr.tree).and_then(|tree| tree.get(addr.node)) else { continue };
            let contents = contents_of_kind(&node, kind).len();
            if contents != *pairs {
                out.diags.push(Diagnostic::error(
                    Unpaired {
                        element: node.ext().element.name.to_string(),
                        kind: (*kind).to_string(),
                        points: *pairs,
                        contents,
                    },
                    span.clone(),
                ));
            }
        }
        if !numbered {
            for (name, (kind, _)) in &kinds {
                let first = xnotes.of_kind(name).first();
                if let (true, Some(first)) = (kind.numbered, first) {
                    out.diags.push(Diagnostic::error(
                        NoNumbering { kind: (*name).to_string() },
                        first.span.clone(),
                    ));
                }
            }
        }
        match out.table(XNOTES) {
            Ok(table) => *table = xnotes,
            Err(error) => failed(out, error.to_string()),
        }
    }
}

/// The standard builder of the list of one xnote kind: the stage
/// `flm.xnotes.build-list.<kind>` ([`builder_name`]), a computing stage of
/// document scope, which writes the kind's table `flm.xnotes.list.<kind>`
/// ([`piece_table`]) from the table [`XNOTES`] and the document's trees.
///
/// [`contribute_kind`](super::contribute_kind) registers one builder per xnote
/// kind, so that every kind, configured or contributed by another feature,
/// has its own list, and so that replacing one kind's builder replaces that
/// kind's list and no other. A user or a package builds the list differently
/// by registering another document-scope stage that writes the same table and
/// preferring it in the plan request (see [`piece_table`]).
///
/// What it writes:
///
/// - for an in-place kind ([`XnotePlacement::InPlace`]), and for a kind with
///   no xnote in the document, a [`Piece`] with no content: nothing is
///   placed;
/// - otherwise one synthesized value made of document elements: a root
///   holding one node of the kind's list element `flm.xnotes.<kind>.list`
///   ([`list_element_name`], carrying an
///   [`XnotesListElement`](super::XnotesListElement)), whose children are one
///   node of the entry element [`ENTRY`](super::ENTRY) per xnote of the kind,
///   in document order. Each entry holds one content pointer
///   ([`CONTENT_POINTER`](super::CONTENT_POINTER)) whose payload
///   ([`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer))
///   refers to the node carrying the xnote and to the region of its content,
///   as the table records it ([`XnoteRecord::content`]); nothing of the
///   content is copied. A node carrying several xnotes of the kind has one
///   entry for each, each pointing to its own region. An entry whose record
///   holds no region holds no pointer. The value is made by
///   [`copy_pieces_into_value`] of synthesized parts only, and each entry and
///   pointer is placed at its xnote's source span, so that a diagnostic about
///   it points into the document.
///
/// The list's rule lowers each pointer where the pointer is placed: the
/// content is lowered once, there, as the document's own content (its
/// reference points numbered and given anchors as the document's, a labeled
/// point inside it accepted), and the xnote's own rule writes its mark alone
/// ([`ListRule`](super::ListRule)).
///
/// A list that cannot be assembled, or a table that cannot be read or
/// written, is reported as [`StageFailed`], and the list is not placed.
///
/// The builder reads the table [`XNOTES`] where the `xnotes` feature is
/// registered. Where it is not (a feature contributed the kind to a registry
/// without it), no xnote is collected, and the builder writes no content.
#[derive(Debug)]
pub struct ListBuilder {
    kind: Arc<XnoteKind>,
}

impl ListBuilder {
    /// The builder of the list of the xnote kind `kind`.
    pub fn new(kind: Arc<XnoteKind>) -> ListBuilder {
        ListBuilder { kind }
    }

    /// The synthesized value the builder writes, or `None` for nothing to
    /// place; a failure is reported into `out`.
    fn build(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) -> Option<Arc<NodeTree<Flm>>> {
        let stage = builder_name(self.kind.name.as_str());
        if self.kind.placement == XnotePlacement::InPlace {
            return None;
        }
        // Without the `xnotes` feature, the table of xnotes is not read
        // (`needs`), and no xnote is collected.
        if !cx.registry().has_feature(FEATURE) {
            return None;
        }
        let xnotes = match cx.get(XNOTES) {
            Ok(Some(xnotes)) => xnotes,
            Ok(None) => return None,
            Err(error) => {
                stage_failed(out, stage.as_str(), error);
                return None;
            }
        };
        let records = xnotes.of_kind(self.kind.name.as_str());
        if records.is_empty() {
            return None;
        }
        let specs = self.specs(cx).map_err(|detail| stage_failed(out, stage.as_str(), detail)).ok()?;
        let guard = cx.registry().lang().guard();
        let mut nodes = Vec::with_capacity(records.len());
        for record in records {
            let Some(node) = cx
                .frozen_tree(record.point.addr.tree)
                .and_then(|tree| tree.get(record.point.addr.node))
            else {
                stage_failed(out, stage.as_str(), "a recorded xnote is no node of the document");
                return None;
            };
            nodes.push(node);
        }
        // One pointer per record, naming the region the record pairs it
        // with; nothing of the content is copied.
        // See decision rationale at [§flm-dd-dr:rend-lower-region].
        let pointers: Vec<Option<SynthesizedPiece<'_, '_>>> = records
            .iter()
            .zip(&nodes)
            .map(|(record, node)| {
                let region = record.content?;
                Some(SynthesizedPiece {
                    spec: Arc::clone(&specs.pointer),
                    slot: BODY_SLOT,
                    span: record.span.clone(),
                    state: Arc::clone(node.parsing_state()),
                    body: Vec::new(),
                    payload: Some(Arc::new(ContentPointer { target: record.point.addr, region })),
                })
            })
            .collect();
        let entries: Vec<SynthesizedPiece<'_, '_>> = records
            .iter()
            .zip(&nodes)
            .zip(&pointers)
            .map(|((record, node), pointer)| SynthesizedPiece {
                spec: Arc::clone(&specs.entry),
                slot: BODY_SLOT,
                span: record.span.clone(),
                state: Arc::clone(node.parsing_state()),
                body: pointer.iter().map(ValuePiece::Synthesized).collect(),
                payload: None,
            })
            .collect();
        let (Some(first), Some(anchor)) = (entries.first(), nodes.first()) else { return None };
        let list = SynthesizedPiece {
            spec: Arc::clone(&specs.list),
            slot: BODY_SLOT,
            span: first.span.clone(),
            state: Arc::clone(&first.state),
            body: entries.iter().map(ValuePiece::Synthesized).collect(),
            payload: None,
        };
        match copy_pieces_into_value(&[ValuePiece::Synthesized(&list)], anchor, ValueContent::Own, guard) {
            Ok(value) => Some(value),
            Err(error) => {
                stage_failed(out, stage.as_str(), format!("the list could not be assembled: {error}"));
                None
            }
        }
    }

    /// The registered list element of the kind, the entry element and the
    /// content pointer, of whose nodes the value is made.
    fn specs(&self, cx: &StageCx<'_>) -> Result<ListSpecs, String> {
        let spec = |name: &str| match cx.registry().element_value(name) {
            Ok(Some(element)) => Ok(Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))),
            Ok(None) => Err(format!("no element `{name}` is registered")),
            Err(error) => Err(error.to_string()),
        };
        Ok(ListSpecs {
            list: spec(list_element_name(self.kind.name.as_str()).as_str())?,
            entry: spec(ENTRY)?,
            pointer: spec(CONTENT_POINTER)?,
        })
    }
}

impl Stage for ListBuilder {
    fn decl(&self) -> StageDecl {
        let name = self.kind.name.as_str();
        StageDecl::new(builder_name(name), Effect::Compute, Scope::Document)
            .writes(piece_table(name))
            .docs(format!(
                "Builds the list of the `{name}` xnotes of the document: a synthesized value \
                 holding one entry per xnote, each a pointer to the xnote's content, written into \
                 the piece `flm.xnotes.list.{name}`."
            ))
    }

    /// Reads the table of xnotes wherever the `xnotes` feature is registered,
    /// as a required read, so that the collect stage is scheduled; and reads
    /// nothing where it is not, because the construction of a plan fails for
    /// a stage that reads a table no feature declares.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        if cx.has_feature(FEATURE) { Needs::new().reads(XNOTES) } else { Needs::new() }
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let content = self.build(cx, out);
        let name = self.kind.name.as_str();
        match out.table(&piece_table(name)) {
            Ok(table) => *table = Piece { content },
            Err(error) => stage_failed(out, builder_name(name).as_str(), error),
        }
    }
}

/// The specifications of the synthesized nodes of a list: the xnote kind's
/// list element, the entry element and the content pointer.
struct ListSpecs {
    list: Arc<FlmSynthesizedSpec>,
    entry: Arc<FlmSynthesizedSpec>,
    pointer: Arc<FlmSynthesizedSpec>,
}

