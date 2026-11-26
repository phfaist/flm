//! This module defines the table of xnotes [`XNOTES`], the collect stage that
//! writes this table, and the stage [`ListBuilder`] that builds the list of an
//! xnote kind, along with the error [`StageFailed`] that these stages report.
//!
//! The collect stage finds the xnotes of the document among its reference
//! points and records them in the table of xnotes. The lowering rules of the
//! xnotes and the stages that build the lists read this table.

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
use crate::stdlib::backends::lowering::ContentPointer;
use crate::stdlib::features::numbering::{self, COUNTERS};
use crate::stdlib::features::refpoints::{REFPOINTS, point_key};

use super::config::{XnoteKind, XnotePlacement};
use super::diagnostics::{NoNumbering, Unpaired};
use super::marks::{Compiled, FixedMarks};
use super::{
    builder_name, contents_of_kind, list_element_name, piece_table, COLLECT, CONTENT_POINTER, ENTRY,
    FEATURE, KINDS_SLOT, mark_point_kind,
};

/// The name of the content slot, `body`, of the synthesized nodes that
/// [`ListBuilder`] creates: the list node, the entry nodes and the content
/// pointers.
const BODY_SLOT: &str = "body";

/// The name of the table of xnotes, `flm.xnotes`, whose value is an
/// [`Xnotes`]. The collect stage [`COLLECT`] writes this table. To read the
/// table in a stage, declare the read with [`Needs::reads`]`(XNOTES)`; in a
/// lowering rule, declare it with
/// [`LoweringDecl::reads`](crate::core::lowering::LoweringDecl::reads)`(XNOTES)`.
pub const XNOTES: TableName<Xnotes> = TableName::new("flm.xnotes");

/// The record of one xnote of the document in the table of xnotes
/// ([`Xnotes`]). The record identifies the two reference points of the xnote
/// and the region of its node that contains the content of the xnote.
///
/// The record contains no number and no anchor. The number of an xnote of a
/// numbered kind is stored under [`point`] in the counter table of the
/// numbering feature ([`COUNTERS`]). The anchors of the xnote and of its mark
/// are stored under [`point`] and [`mark`] in the anchors table of the
/// references feature
/// ([`ANCHORS`](crate::stdlib::features::refs::ANCHORS)).
///
/// [`point`]: XnoteRecord::point
/// [`mark`]: XnoteRecord::mark
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XnoteRecord {
    /// The reference point of the xnote itself. Its reference kind is the
    /// name of the xnote kind, and it carries the labels of the xnote: a
    /// `\ref` to one of these labels refers to this point. For a numbered
    /// xnote kind, this point is numbered.
    pub point: PointId,
    /// The reference point of the mark of the xnote, which follows [`point`]
    /// on the same node. Its reference kind is `<kind>-mark`. Its anchor
    /// identifies the mark in the output, and on the HTML page the entry of
    /// the xnote in the list links back to it.
    ///
    /// [`point`]: XnoteRecord::point
    pub mark: PointId,
    /// The source span of the node carrying the two points.
    pub span: FlmSpan,
    /// The region of the node that contains the content of the xnote, as the
    /// [`XnoteSource`](super::XnoteSource) aspect of the node's element
    /// returns it. The regions that this aspect returns are paired in order
    /// with the xnotes of the node, xnote kind by xnote kind (see
    /// [`XnoteSource`](super::XnoteSource)). `None` if the aspect returns
    /// fewer regions of this kind than the node has xnotes of this kind; the
    /// collect stage then reports [`Unpaired`].
    pub content: Option<NodeRegion>,
}

/// The table of xnotes ([`XNOTES`], `flm.xnotes`), which contains a record
/// ([`XnoteRecord`]) for each xnote of the document, grouped by xnote kind.
///
/// The collect stage [`COLLECT`] writes this table. It finds the xnotes in the
/// table of reference points ([`REFPOINTS`]). An xnote is a reference point
/// whose reference kind is the name of an xnote kind, immediately followed on
/// the same node by a reference point of the reference kind `<kind>-mark`.
/// The function [`ref_points`](super::ref_points) returns these two reference
/// points. The xnotes that the elements of other features carry are therefore
/// recorded in the same way as the xnotes written with `\footnote`.
///
/// The records of each xnote kind are in document order, which is the order
/// of [`RefPoints::in_document_order`]. A node can carry several xnotes, of
/// one xnote kind or of several. Each xnote has a record of its own, and the
/// records of one node are in the order of its reference points.
///
/// Every xnote kind of the registry (every kind in the service slot
/// [`KINDS_SLOT`]) has an entry in [`by_kind`](Xnotes::by_kind). This entry is
/// empty if the document contains no xnote of the kind.
///
/// The table contains records only for the nodes of the document trees. A
/// node of a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) has no record.
/// When content is copied to be shown again, for instance the title of a
/// heading that a reference displays, each node that carries xnotes is
/// replaced in the copy by a [`MarkOf`](super::MarkOf) node. This node finds
/// the records of the original xnotes with
/// [`records_written_at`](Xnotes::records_written_at).
///
/// The table also contains the content of the fixed mark of each unnumbered
/// xnote kind ([`fixed_mark`](Xnotes::fixed_mark)). These marks are compiled
/// once, when the registry is built. The collect stage copies them into the
/// table so that the lowering rules, which have no access to the registry,
/// can read them.
///
/// [`RefPoints::in_document_order`]: crate::stdlib::features::refpoints::RefPoints::in_document_order
#[derive(Debug, Clone, Default)]
pub struct Xnotes {
    /// The records of the xnotes, by the name of their xnote kind. The
    /// records of each kind are in document order.
    pub by_kind: BTreeMap<Name, Vec<XnoteRecord>>,
    /// The compiled content of the fixed mark of each unnumbered xnote kind,
    /// by the name of the kind (see [`Xnotes::fixed_mark`]).
    fixed_marks: Compiled,
}

impl PartialEq for Xnotes {
    /// Compares the records only. The fixed marks are not compared, because
    /// they depend only on the registry.
    fn eq(&self, other: &Xnotes) -> bool {
        self.by_kind == other.by_kind
    }
}

impl Eq for Xnotes {}

impl Xnotes {
    /// Returns the records of the xnote kind `kind`, in document order.
    /// Returns an empty slice if the document contains no xnote of this kind,
    /// and if `kind` is not the name of an xnote kind.
    pub fn of_kind(&self, kind: &str) -> &[XnoteRecord] {
        self.by_kind.get(kind).map_or(&[], Vec::as_slice)
    }

    /// Returns the record of an xnote that the node at `addr` carries,
    /// together with the name of its xnote kind. Returns `None` if the table
    /// contains no record for this node: the node carries no xnote, or it is
    /// not a node of a document tree.
    ///
    /// If the node carries several xnotes, this method returns the first
    /// record of the xnote kind whose name comes first in the order of the
    /// names. To obtain the records of one xnote kind for a node, call
    /// [`records_at`](Xnotes::records_at).
    pub fn record_at(&self, addr: NodeAddr) -> Option<(&Name, &XnoteRecord)> {
        self.by_kind.iter().find_map(|(kind, records)| {
            records.iter().find(|record| record.point.addr == addr).map(|record| (kind, record))
        })
    }

    /// Returns the records of the xnotes of the kind `kind` that the node at
    /// `addr` carries, in the order of the reference points of the node. The
    /// first record belongs to the first xnote of this kind on the node, whose
    /// content is the first region of this kind that the
    /// [`XnoteSource`](super::XnoteSource) aspect of the node returns, and so
    /// on. Returns an empty iterator if the table contains no record of this
    /// kind for the node.
    pub fn records_at(&self, kind: &str, addr: NodeAddr) -> impl Iterator<Item = &XnoteRecord> {
        self.of_kind(kind).iter().filter(move |record| record.point.addr == addr)
    }

    /// Returns the records of the xnote kind `kind` whose node was written at
    /// `span`, in document order. A record matches if the span of its node
    /// covers the same range of the same source as `span`. Returns an empty
    /// iterator if no xnote of this kind was written at `span`.
    ///
    /// Use this method to find the xnotes of a copied node. A copy of a node
    /// keeps the span of the original node, but it has an address of its own,
    /// for which the table contains no record. For instance, a
    /// [`MarkOf`](super::MarkOf) node, which replaces a node that carries
    /// xnotes in a copy of content shown again, finds the records of these
    /// xnotes with this method.
    pub fn records_written_at<'s>(
        &'s self,
        kind: &str,
        span: &'s FlmSpan,
    ) -> impl Iterator<Item = &'s XnoteRecord> {
        self.of_kind(kind).iter().filter(move |record| {
            record.span.start() == span.start() && record.span.end() == span.end() && record.span.same_source(span)
        })
    }

    /// Returns `true` if the table contains no record.
    pub fn is_empty(&self) -> bool {
        self.by_kind.values().all(Vec::is_empty)
    }

    /// Returns the content of the fixed mark of the unnumbered xnote kind
    /// `kind`. The fixed mark is the [`mark`](XnoteKind::mark) template of the
    /// kind, which contains no number and is therefore the same for every
    /// xnote of the kind. It is compiled as FLM text when the registry is
    /// built.
    ///
    /// The content is a list of synthesized values, which a lowering rule
    /// inserts into its output with
    /// [`Lowerer::splice`](crate::core::lowering::Lowerer::splice).
    /// [`stage_mark_text`](super::stage_mark_text) writes the mark of an xnote
    /// of any kind, numbered or not.
    ///
    /// Returns an empty slice for a kind with an empty mark, for a numbered
    /// kind, and if `kind` is not the name of an xnote kind of the registry.
    pub fn fixed_mark(&self, kind: &str) -> &[Arc<NodeTree<Flm>>] {
        self.fixed_marks.get(kind).map_or(&[], Vec::as_slice)
    }
}

impl Table for Xnotes {
    /// Appends the records of `piece` after the records of `self`, kind by
    /// kind. The fixed marks are the same in every table piece: if `piece`
    /// has fixed marks, they replace those of `self`.
    fn absorb(&mut self, piece: Xnotes) {
        // The fixed marks are the registry's, the same map in every piece.
        if !piece.fixed_marks.is_empty() {
            self.fixed_marks = piece.fixed_marks;
        }
        for (kind, records) in piece.by_kind {
            self.by_kind.entry(kind).or_default().extend(records);
        }
    }

    /// Serializes the table as a map from the name of each xnote kind, in the
    /// order of the names, to the list of its records, in document order.
    /// Each record is a map with two keys: `point`, the key of the reference
    /// point of the xnote, and `mark`, the key of the reference point of its
    /// mark. A reference point has the key `<node>#<index>`, as in the
    /// serialized table of reference points. The other fields of the records
    /// and the fixed marks are not serialized.
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

/// The error that is reported when a stage of the `xnotes` feature fails
/// because of an error in the program (id `flm.xnotes.stage-failed`).
///
/// A document cannot cause this error. The collect stage [`COLLECT`] reports
/// it when it cannot read the table of reference points or the counter table,
/// or cannot write the table of xnotes. The stage that builds the list of an
/// xnote kind ([`ListBuilder`]) reports it when it cannot read the table of
/// xnotes or write the table of the list, when one of the elements of which
/// the list is made is not registered, when the node of a recorded xnote is
/// missing in the document, or when the list cannot be assembled.
///
/// Reported at no position in the document. If the collect stage fails, the
/// table of xnotes contains no record: each xnote then expands to nothing,
/// with the error `flm.run.missing-entry`, and no list of xnotes has content.
/// If the stage that builds the list of an xnote kind fails, the list of this
/// kind has no content, so the list does not appear in the output.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.xnotes.stage-failed", message = "Stage ‘{stage}’ failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage: `flm.xnotes.collect`, or
    /// `flm.xnotes.build-list.<kind>` for the stage that builds the list of
    /// the xnote kind `<kind>`.
    pub stage: String,
    /// The description of the failure.
    pub detail: String,
}

/// Reports [`StageFailed`] for the stage `stage`.
fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// The collect stage `flm.xnotes.collect` ([`COLLECT`]) records every xnote
/// of the document in the table [`XNOTES`] (see [`Xnotes`]). The stage runs
/// once for the whole document, and finds the xnotes in the table of
/// reference points ([`REFPOINTS`]).
///
/// The stage also reports the following errors:
///
/// - [`NoNumbering`], once for each numbered xnote kind that has xnotes in the
///   document, when the registry does not contain the numbering feature;
/// - [`Unpaired`], for each node that carries xnotes of a kind, when the
///   reference points of the node and its [`XnoteSource`](super::XnoteSource)
///   aspect return different numbers of xnotes of this kind;
/// - [`StageFailed`], when the stage cannot read or write a table.
pub(super) struct Collect {
    /// The fixed marks of the unnumbered xnote kinds, compiled when the
    /// registry is built. The stage copies them into the table of xnotes.
    pub(super) marks: Arc<FixedMarks>,
}

impl Stage for Collect {
    fn decl(&self) -> StageDecl {
        StageDecl::new(COLLECT, Effect::Compute, Scope::Document).writes(XNOTES).docs(
            "Writes every xnote of the document to the table `flm.xnotes`, by xnote kind and in \
             document order. Finds the xnotes among the reference points in the table \
             `flm.refpoints`.",
        )
    }

    /// Declares a read of the table of reference points, and a read of the
    /// counter table [`COUNTERS`] when the registry contains the numbering
    /// feature.
    ///
    /// The stage reads no value from the counter table: it only checks that
    /// the table exists. The read is nevertheless a required read, so that
    /// every plan that contains this stage also computes the numbers of the
    /// xnotes. Without the numbering feature, the counter table is not
    /// declared in the registry, and the stage does not declare the read: the
    /// construction of a plan fails for a stage that reads a table that is not
    /// declared
    /// ([`PlanError::NeedsUndeclared`](crate::core::plan::PlanError::NeedsUndeclared)).
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
        // Every kind of the slot has an entry, empty where no xnote is of it:
        // a reader tells "no xnote of this kind" from "no such kind" by the
        // slot, and a kind's list finds its entry with no special case.
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
            // xnote: `ref_points` answers the two together, and reading them
            // back as a pair checks that the point is an xnote without reading
            // its element.
            if points.point(mark).is_none_or(|next| next.point.kind != *mark_kind) {
                continue;
            }
            let name = kind.name.as_str();
            let nth = &mut met.entry((record.id.addr, name)).or_insert_with(|| (0, record.span.clone())).0;
            // The node's n-th xnote of a kind takes the node's n-th content of
            // that kind; the pairing is made once, here, and stored with the
            // xnote, so that no reader repeats it.
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

/// The stage `flm.xnotes.build-list.<kind>` ([`builder_name`]) builds the
/// list of the xnotes of one xnote kind. The stage runs once for the whole
/// document. It reads the table of xnotes [`XNOTES`], and writes the list as a
/// [piece](crate::guide::concepts_overview#pieces) to the table
/// `flm.xnotes.list.<kind>` ([`piece_table`]). This piece is placed after the
/// content of the document.
///
/// [`contribute_kind`](super::contribute_kind) registers one such stage for
/// each xnote kind, including the xnote kinds that other features contribute.
/// To build the list of one xnote kind differently, register a stage of your
/// own that writes the same table, and prefer this stage in the plan request,
/// as described in [`piece_table`]. The lists of the other xnote kinds are not
/// affected.
///
/// The piece has no content, and no list appears in the output, in the
/// following cases:
///
/// - the xnotes of the kind are shown in place
///   ([`XnotePlacement::InPlace`]);
/// - the document contains no xnote of the kind;
/// - the registry does not contain the `xnotes` feature, which can happen
///   when another feature contributes the xnote kind. The table of xnotes is
///   then not declared, and the stage does not read it.
///
/// Otherwise, the content of the piece is a synthesized value. This value
/// contains one node of the list element of the kind, `flm.xnotes.<kind>.list`
/// ([`list_element_name`]). The children of the list node are the entry nodes
/// ([`ENTRY`]), one for each xnote of the kind, in document order. Each entry
/// node contains a content pointer ([`CONTENT_POINTER`]). The payload of the
/// content pointer, a [`ContentPointer`], refers to the node that carries the
/// xnote and to the region of this node that contains the content of the
/// xnote ([`XnoteRecord::content`]).
///
/// A node that carries several xnotes of the kind has one entry for each of
/// them. An entry whose record has no content region contains no content
/// pointer. Each entry and each content pointer has the source span of the
/// node that carries its xnote, so that a diagnostic about it points to the
/// place where the xnote was written.
///
/// The content of the xnotes is not copied into the list. The lowering rule of
/// the list ([`ListRule`](super::ListRule)) lowers the content that each
/// content pointer refers to at the place of the entry, as content of the
/// document. The reference points in this content therefore keep the numbers
/// and anchors computed for the document: for instance, a footnote written
/// inside the content of another footnote keeps its number. The content is
/// lowered only in the list: the lowering rule of the xnote writes only the
/// mark of the xnote in the text.
///
/// If the list cannot be built, the stage reports [`StageFailed`], and the
/// piece has no content.
#[derive(Debug)]
pub struct ListBuilder {
    kind: Arc<XnoteKind>,
}

impl ListBuilder {
    /// Returns the stage that builds the list of the xnote kind `kind`.
    pub fn new(kind: Arc<XnoteKind>) -> ListBuilder {
        ListBuilder { kind }
    }

    /// Builds the content of the list. Returns the synthesized value, or
    /// `None` if the list has no content. If the list cannot be built, reports
    /// [`StageFailed`] into `out` and returns `None`.
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
                stage_failed(out, stage.as_str(), "missing node of a recorded xnote in the document");
                return None;
            };
            nodes.push(node);
        }
        // One pointer per record, naming the region the record pairs it
        // with; nothing of the content is copied, so the content is lowered at
        // its addresses in the document and reads what the stages wrote for it
        // there (its numbers, its anchors), which a copy's nodes would not.
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
                stage_failed(out, stage.as_str(), error);
                None
            }
        }
    }

    /// Returns the specifications of the three synthesized elements of which
    /// the list is made: the list element of the xnote kind, the entry element
    /// [`ENTRY`] and the content pointer [`CONTENT_POINTER`]. Returns the
    /// description of the error if one of these elements is not registered,
    /// or is registered more than once.
    fn specs(&self, cx: &StageCx<'_>) -> Result<ListSpecs, String> {
        let spec = |name: &str| match cx.registry().element_value(name) {
            Ok(Some(element)) => Ok(Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))),
            Ok(None) => Err(format!("missing element ‘{name}’ in the registry")),
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
                "Builds the list of the xnotes of kind `{name}` in the document, with one entry \
                 for each xnote, and writes the list to the piece `flm.xnotes.list.{name}`. The \
                 piece has no content if the xnotes of this kind are shown in place."
            ))
    }

    /// Declares a required read of the table of xnotes when the registry
    /// contains the `xnotes` feature, so that every plan that contains this
    /// stage also contains the collect stage. Declares no read otherwise: the
    /// table of xnotes is then not declared in the registry, and the
    /// construction of a plan fails for a stage that reads a table that is not
    /// declared.
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

/// The specifications of the three synthesized elements of which a list of
/// xnotes is made: the list element of the xnote kind, the entry element
/// [`ENTRY`] and the content pointer [`CONTENT_POINTER`].
struct ListSpecs {
    list: Arc<FlmSynthesizedSpec>,
    entry: Arc<FlmSynthesizedSpec>,
    pointer: Arc<FlmSynthesizedSpec>,
}

