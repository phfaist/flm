//! This module defines the `refpoints` feature, which collects the reference
//! points of a document in the table [`REFPOINTS`] (`flm.refpoints`).
//!
//! A *reference point* ([`RefPoint`]) is a place in a document that can be
//! referred to, numbered, or given an anchor, such as a heading, an equation
//! line, a theorem or a footnote. An element declares the reference points of
//! each of its nodes through the [`Referenceable`] aspect. A node can have
//! several reference points; for instance, a display-math environment has one
//! reference point for each equation line. The concepts overview explains how
//! reference points relate to labels and anchors ([Reference points, labels
//! and anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//!
//! Several features work on the reference points of a document. The
//! [`refs`](crate::stdlib::features::refs) feature collects the labels of the
//! reference points and computes their anchors, the
//! [`numbering`](crate::stdlib::features::numbering) feature numbers them,
//! and the [`xnotes`](crate::stdlib::features::xnotes) feature finds the
//! footnotes and other notes among them. These features read the reference
//! points from the table [`REFPOINTS`] and do not call the [`Referenceable`]
//! aspect themselves. Therefore the aspect is called only once for each node
//! in a run, and content that an element builds for a reference point
//! ([`ContentRef::Synthesized`]) is built only once and shared by all readers
//! of the table.
//!
//! # What the feature registers
//!
//! The `refpoints` feature offers no command or environment. It registers the
//! following items:
//!
//! - The table [`REFPOINTS`] (`flm.refpoints`), whose value is a
//!   [`RefPoints`]. The table contains one [`RefPointRecord`] for each
//!   reference point of the run. A reference point is identified by a
//!   [`PointId`]: the address of its node and its index among the reference
//!   points of that node.
//! - The stage [`SCAN`] (`flm.refpoints.scan`), which writes this table. The
//!   stage runs once for each tree. It uses the element index
//!   ([`ELEMENT_INDEX`]) to find the nodes of the tree whose element carries
//!   the [`Referenceable`] aspect, and records their reference points in tree
//!   order: the root, then its descendants in preorder.
//!
//! A stage reads the table like any other table: it declares the read with
//! [`Needs::reads`]`(REFPOINTS)` and obtains the table with [`StageCx::get`].
//! A stage that runs once for the whole document and needs the reference
//! points in document order calls [`RefPoints::in_document_order`].
//!
//! # Features that require the table
//!
//! The stages of the `refs`, `numbering` and `xnotes` features require the
//! table [`REFPOINTS`]. A registry that contains one of these features must
//! therefore also contain the `refpoints` feature. The `refpoints` feature in
//! turn requires the element index, so the registry must also contain the
//! [`index`](crate::stdlib::features::index) feature.
//!
//! A registry that lacks the `refpoints` feature still builds. However, every
//! plan made from it fails with
//! [`PlanError::NeedsUndeclared`](crate::core::plan::PlanError::NeedsUndeclared):
//! a stage reads the table `flm.refpoints`, and this table is not declared in
//! any feature. This holds even for a plan that does not schedule the stages
//! that read the table, because the needs of every registered stage are
//! resolved when a plan is made.
//!
//! # Synthesized values
//!
//! A [*synthesized value*](crate::guide::concepts_overview#synthesized-values)
//! is a small tree that a stage builds during a run, outside the document's
//! trees, for instance the text displayed for a reference. Before a stage
//! renders a synthesized value, it calls [`StageCx::resolve_value`], which
//! performs [value catch-up](crate::guide::concepts_overview#value-catch-up):
//! the stages that run once for each tree and that the calling stage depends
//! on run over the synthesized value. The stage [`SCAN`] is among these stages
//! if the calling stage depends on it, directly or through other stages. The
//! table [`REFPOINTS`] computed during the catch-up then contains only the
//! reference points of the synthesized value.
//!
//! A synthesized value is not part of the document, so it may not define a
//! label. If the table computed during the catch-up contains a reference
//! point with a label, the catch-up fails with
//! [`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue).
//! A synthesized value may contain reference points without a label.

// A feature of its own, read by both consumers: content an aspect builds for a
// point is built once and stored, where two stages asking the aspect would build
// it twice. A collection in the element index would make the scan part of every
// registry, and two aspects, one for what may be referenced and one for what may
// be numbered, would have to agree on count and order by convention.

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::text::plain_text;
use crate::lang::{nowhere, FlmOrigin, FlmSpan};
use crate::model::addr::{NodeAddr, TreeId};
use crate::model::aspects::{
    AnchorPolicy, ContentRef, Label, NumberSource, PointId, RefPoint, Referenceable,
};
use crate::stdlib::features::index::ELEMENT_INDEX;

/// The feature's registered name, `refpoints`.
pub const FEATURE: &str = "refpoints";

/// The name of the stage that writes the table of reference points
/// [`REFPOINTS`], `flm.refpoints.scan`.
pub const SCAN: &str = "flm.refpoints.scan";

/// The name of the table of reference points, `flm.refpoints`, whose value is
/// a [`RefPoints`]. To read the table in a stage, declare the read with
/// [`Needs::reads`]`(REFPOINTS)`.
pub const REFPOINTS: TableName<RefPoints> = TableName::new("flm.refpoints");

/// A `RefPointRecord` is one entry of the table [`REFPOINTS`]. It contains a
/// reference point as the [`Referenceable`] aspect of its node returned it,
/// together with the identifier of the reference point and the span of the
/// node.
#[derive(Debug, Clone)]
pub struct RefPointRecord {
    /// The identifier of the reference point: the address of its node and the
    /// index of the reference point among the reference points of that node.
    pub id: PointId,
    /// The reference point, as the [`Referenceable`] aspect of the node
    /// returned it.
    pub point: RefPoint,
    /// The span of the node that has the reference point. The stages that
    /// report a diagnostic about the reference point report it at this span.
    pub span: FlmSpan,
}

impl RefPointRecord {
    /// Returns the labels of this reference point and the texts that were
    /// written as labels but are not of the form `type:label`, in one list.
    ///
    /// Each item contains the text of a label and the parsed label, or a
    /// malformed text and `None`. The labels that the element computed come
    /// first. The labels written in the document and the malformed texts
    /// follow, in the order in which the document wrote them.
    ///
    /// A [`RefPoint`] stores its labels ([`RefPoint::labels`]) and its
    /// malformed texts ([`RefPoint::malformed`]) in two separate lists. This
    /// method merges the two lists by the
    /// [position](crate::model::aspects::MalformedLabel::position) of each
    /// malformed text. Call it when you need the order in which the document
    /// mentions the labels. For instance, the safe labels stage of the `refs`
    /// feature ([`SafeLabelsStage`](crate::stdlib::features::refs::SafeLabelsStage))
    /// numbers the labels in this order.
    pub fn mentions(&self) -> Vec<(String, Option<&Label>)> {
        let point = &self.point;
        let mut mentions = Vec::with_capacity(point.labels.len() + point.malformed.len());
        let mut labels = point.labels.iter();
        let mut malformed = point.malformed.iter().peekable();
        loop {
            let next_is_malformed =
                malformed.peek().is_some_and(|one| one.position <= mentions.len());
            if next_is_malformed {
                let one = malformed.next().expect("peeked");
                mentions.push((one.text.clone(), None));
            } else if let Some(label) = labels.next() {
                mentions.push((label.to_string(), Some(label)));
            } else if let Some(one) = malformed.next() {
                // A position past the end: nothing is dropped.
                mentions.push((one.text.clone(), None));
            } else {
                return mentions;
            }
        }
    }
}

/// A `RefPoints` is the value of the table [`REFPOINTS`] (`flm.refpoints`). It
/// contains one [`RefPointRecord`] for each reference point of the trees of
/// the run.
///
/// The table stores the records tree after tree, in the run's order of the
/// trees ([`StageCx::trees`]). Within a tree, the records follow tree order:
/// the root, then its descendants in preorder. The records of one node follow
/// the order in which the [`Referenceable`] aspect of the node returned the
/// reference points. [`RefPoints::records`] returns the records in this order,
/// and [`RefPoints::in_document_order`] returns them in document order.
///
/// [`RefPoints::point`] returns the record of a reference point, given its
/// identifier ([`PointId`]). [`RefPoints::of_tree`] returns the records of one
/// tree.
#[derive(Debug, Clone, Default)]
pub struct RefPoints {
    records: Vec<RefPointRecord>,
    by_id: BTreeMap<PointId, usize>,
}

impl RefPoints {
    /// Appends `record` after the records already in the table.
    pub fn insert(&mut self, record: RefPointRecord) {
        self.by_id.insert(record.id, self.records.len());
        self.records.push(record);
    }

    /// Returns all records in the order in which the table stores them: tree
    /// after tree, and within a tree in tree order (see [`RefPoints`]). To
    /// obtain the records in document order, call
    /// [`RefPoints::in_document_order`].
    pub fn records(&self) -> &[RefPointRecord] {
        &self.records
    }

    /// Returns all records in the document order of the run, which is the
    /// order of [`StageCx::walk`].
    ///
    /// The records are sorted by the position of their node in the document
    /// ([`StageCx::position`]). The records of one node keep the order in
    /// which the [`Referenceable`] aspect of the node returned them. When a
    /// tree is placed inside another tree, the records of the placed tree come
    /// immediately after the records of the node at which the tree is placed.
    /// Records whose node has no position in the document, such as the
    /// reference points of a synthesized value, come first, in the order of
    /// the table.
    ///
    /// A stage that runs once for the whole document calls this method when
    /// it counts or orders the reference points. For instance, the
    /// [`numbering`](crate::stdlib::features::numbering) feature numbers the
    /// reference points in this order.
    pub fn in_document_order<'t>(&'t self, cx: &StageCx<'_>) -> Vec<&'t RefPointRecord> {
        // The table holds no place in the document, which a per-tree stage
        // cannot know: the run's position of each node gives the order.
        // Every node of a tree of the run has a position; only a synthesized
        // value's records have none, and the stable sort keeps their order
        // (first).
        let mut records: Vec<&RefPointRecord> = self.records.iter().collect();
        records.sort_by_key(|record| cx.position(record.id.addr));
        records
    }

    /// Returns the records of the tree `tree`, in tree order. Returns no
    /// records if the table contains no reference point of this tree.
    pub fn of_tree(&self, tree: TreeId) -> impl Iterator<Item = &RefPointRecord> {
        self.records.iter().filter(move |record| record.id.addr.tree == tree)
    }

    /// Returns the record of the reference point `id`, or `None` if the table
    /// does not contain this reference point.
    pub fn point(&self, id: PointId) -> Option<&RefPointRecord> {
        self.by_id.get(&id).and_then(|index| self.records.get(*index))
    }

    /// Returns the number of records in the table.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if the table contains no record.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl Table for RefPoints {
    /// Appends the records of `piece` after the records of this table, in
    /// their order.
    fn absorb(&mut self, piece: RefPoints) {
        for record in piece.records {
            self.insert(record);
        }
    }

    /// Serializes the table as a map with one entry for each reference point,
    /// in the order of the table.
    ///
    /// The key of an entry has the form `<node>#<index>`: the node as
    /// [`ProjectCx::node_ref`] writes it, and the index of the reference point
    /// among the reference points of the node. The value of an entry is a map
    /// with the following entries:
    ///
    /// - `kind`: the reference kind, such as `section`.
    /// - `labels`: the list of the labels.
    /// - `malformed`: the list of the texts written as labels that are not of
    ///   the form `type:label`. This entry is present only if the reference
    ///   point has such a text.
    /// - `anchor`: the anchor policy as a short text, such as `auto:sec` or
    ///   `from-first-label`.
    /// - `display`: a map with the entries `number` and `content`. `number` is
    ///   the number source, with the entries `counter`, `style` and `advance`,
    ///   and with `instances` and `number_format` where the number source has
    ///   them. `content` is the content that a reference to the reference
    ///   point displays when the reference point has no number. Each of the
    ///   two entries is present only if the reference point has it.
    ///
    /// The content is written as a short text that refers to a part of the
    /// node, such as `argument 1` or `body`. Content that the element built as
    /// a tree ([`ContentRef::Synthesized`]) is written as the plain text of
    /// that tree.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.records
                .iter()
                .map(|record| {
                    let point = &record.point;
                    let tree = record.id.addr.tree;
                    let labels = point
                        .labels
                        .iter()
                        .map(|label| SerialValue::Str(label.to_string()))
                        .collect();
                    let mut entries = vec![
                        ("kind".to_string(), SerialValue::Str(point.kind.to_string())),
                        ("labels".to_string(), SerialValue::List(labels)),
                    ];
                    if !point.malformed.is_empty() {
                        let malformed = point
                            .malformed
                            .iter()
                            .map(|one| SerialValue::Str(one.text.clone()))
                            .collect();
                        entries.push(("malformed".to_string(), SerialValue::List(malformed)));
                    }
                    entries.push((
                        "anchor".to_string(),
                        SerialValue::Str(policy_text(cx, tree, &point.anchor)),
                    ));
                    let mut display = Vec::with_capacity(2);
                    if let Some(number) = &point.number {
                        display.push(("number".to_string(), project_number(cx, Some(tree), number)));
                    }
                    if let Some(content) = &point.content {
                        display.push((
                            "content".to_string(),
                            SerialValue::Str(content_text(cx, tree, content)),
                        ));
                    }
                    entries.push(("display".to_string(), SerialValue::Map(display)));
                    (point_key(cx, record.id), SerialValue::Map(entries))
                })
                .collect(),
        )
    }

    /// Returns [`PieceClass::DocumentFact`] for a table piece that contains a
    /// reference point with a label, [`PieceClass::NodeKeyed`] for a table
    /// piece whose reference points have no label, and [`PieceClass::Empty`]
    /// for an empty table piece.
    ///
    /// A label is a fact about the document, because other places of the
    /// document refer to it. A synthesized value may therefore not contain a
    /// reference point with a label (see [Synthesized
    /// values](crate::stdlib::features::refpoints#synthesized-values)).
    fn piece_class(&self) -> PieceClass {
        if self.records.is_empty() {
            PieceClass::Empty
        } else if self.records.iter().any(|record| !record.point.labels.is_empty()) {
            PieceClass::DocumentFact
        } else {
            PieceClass::NodeKeyed
        }
    }
}

impl SingletonTable for RefPoints {
    const NAME: &'static str = "flm.refpoints";
}

/// Returns the text that [`ProjectCx::node_ref`] writes for the node at
/// `addr`. The projections of the tables about reference points use this text
/// in map keys and in texts. If `node_ref` writes a value that is not a
/// string, this function returns the debug form of the value.
pub(crate) fn node_key(cx: &dyn ProjectCx, addr: NodeAddr) -> String {
    match cx.node_ref(addr) {
        SerialValue::Str(text) => text,
        other => format!("{other:?}"),
    }
}

/// Returns the key of the reference point `id` in a projection, of the form
/// `<node>#<index>`. `<node>` is the text that [`node_key`] returns for the
/// node, and `<index>` is the index of the reference point among the reference
/// points of the node.
pub(crate) fn point_key(cx: &dyn ProjectCx, id: PointId) -> String {
    format!("{}#{}", node_key(cx, id.addr), id.index)
}

/// Serializes the number source `number` of a reference point as a map with
/// the entries `counter`, `instances`, `style`, `advance` and
/// `number_format`.
///
/// The entry `instances` lists the scoped instances of the counter in which
/// the reference point is counted. It is present only if there are such
/// instances and `tree` is given. The entry `number_format` is present only
/// if the reference point has a number template of its own.
///
/// `tree` is the tree of the node that has the reference point. The scoped
/// instances are nodes of this tree.
pub(crate) fn project_number(
    cx: &dyn ProjectCx,
    tree: Option<TreeId>,
    number: &NumberSource,
) -> SerialValue {
    let advance = number.advance.iter().map(|level| SerialValue::Int(i64::from(*level))).collect();
    let mut source = vec![("counter".to_string(), SerialValue::Str(number.counter.to_string()))];
    if let (Some(tree), false) = (tree, number.instances.is_empty()) {
        let instances =
            number.instances.iter().map(|node| cx.node_ref(NodeAddr::new(tree, *node))).collect();
        source.push(("instances".to_string(), SerialValue::List(instances)));
    }
    source.push(("style".to_string(), SerialValue::Str(number.style.to_string())));
    source.push(("advance".to_string(), SerialValue::List(advance)));
    if let Some(format) = &number.number_format {
        source.push(("number_format".to_string(), SerialValue::Str(format.clone())));
    }
    SerialValue::Map(source)
}

/// Returns the anchor policy `policy` as a short text for a projection, such
/// as `auto:sec` or `from-first-label`. `tree` is the tree of the node that
/// has the reference point; [`content_text`] uses it to write the content of
/// an [`AnchorPolicy::Slug`] policy.
pub(crate) fn policy_text(cx: &dyn ProjectCx, tree: TreeId, policy: &AnchorPolicy) -> String {
    match policy {
        AnchorPolicy::Explicit(id) => format!("explicit:{id}"),
        AnchorPolicy::FromFirstLabel => "from-first-label".to_string(),
        AnchorPolicy::Slug { prefix, content } => {
            format!("slug:{prefix}:{}", content_text(cx, tree, content))
        }
        AnchorPolicy::FromValue { prefix } => format!("from-value:{prefix}"),
        AnchorPolicy::Auto { prefix } => format!("auto:{prefix}"),
    }
}

/// Returns the content reference `content` as a short text for a projection.
///
/// A reference to a part of a node is written as a short description of that
/// part, such as `argument 1` or `body`. Content that the element built as a
/// tree ([`ContentRef::Synthesized`]) is written as the plain text of that
/// tree. `tree` is the tree of the node that has the reference point; it is
/// needed to write a [`ContentRef::Node`] reference.
pub(crate) fn content_text(cx: &dyn ProjectCx, tree: TreeId, content: &ContentRef) -> String {
    match content {
        ContentRef::Argument(index) => format!("argument {index}"),
        ContentRef::ArgumentNamed(name) => format!("argument {name}"),
        ContentRef::Body => "body".to_string(),
        ContentRef::Node(id) => format!("node {}", node_key(cx, NodeAddr::new(tree, *id))),
        ContentRef::Synthesized(value) => plain_text(value),
    }
}

/// The error that is reported when the stage [`SCAN`] cannot read the element
/// index or cannot write the table of reference points (id
/// `flm.refpoints.stage-failed`).
///
/// A document cannot cause this error: it is caused by a fault of the
/// program. Reported at no position in the document. The stage then writes no
/// reference points for the tree, so the table [`REFPOINTS`] contains no
/// reference point of that tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refpoints.stage-failed",
    message = "Stage ‘flm.refpoints.scan’ failed: {detail}"
)]
pub struct StageFailed {
    /// The error that the stage received when it accessed the table, as text.
    pub detail: String,
}

/// Converts the index of a reference point among the reference points of its
/// node to the type of [`PointId::index`]. An index above `u32::MAX` becomes
/// `u32::MAX`; a document cannot produce a node with that many reference
/// points.
fn point_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

/// The stage [`SCAN`] (`flm.refpoints.scan`), which writes the table
/// [`REFPOINTS`]. The stage runs once for each tree. It uses the element index
/// ([`ELEMENT_INDEX`]) to find the nodes of the tree whose element carries the
/// [`Referenceable`] aspect, calls the aspect once for each of these nodes,
/// and records the returned reference points in tree order.
struct Scan;

impl Stage for Scan {
    fn decl(&self) -> StageDecl {
        StageDecl::new(SCAN, Effect::Compute, Scope::PerTree).writes(REFPOINTS).docs(
            "Records in `flm.refpoints` the reference points of each node of the tree whose \
             element carries the `Referenceable` aspect, in tree order.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(ELEMENT_INDEX)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (Some(id), Some(tree)) = (cx.tree_id(), cx.my_tree()) else { return };
        let failed = |out: &mut StageOut<'_>, detail: String| {
            out.diags.push(Diagnostic::error(StageFailed { detail }, nowhere(FlmOrigin::none())));
        };
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return failed(out, error.to_string()),
        };
        let mut points = RefPoints::default();
        for (addr, node, referenceable) in
            index.nodes_with::<dyn Referenceable>(cx.registry(), id, &tree)
        {
            for (index, point) in referenceable.points(&node).into_iter().enumerate() {
                points.insert(RefPointRecord {
                    id: PointId { addr, index: point_index(index) },
                    point,
                    span: node.span().clone(),
                });
            }
        }
        match out.table(REFPOINTS) {
            Ok(table) => *table = points,
            Err(error) => failed(out, error.to_string()),
        }
    }
}

/// The `refpoints` feature, which registers the table [`REFPOINTS`] and the
/// stage [`SCAN`] that writes this table. The [module
/// documentation](crate::stdlib::features::refpoints) describes the table and
/// the features that read it.
///
/// The stage [`SCAN`] reads the element index, so a registry that contains the
/// `refpoints` feature must also contain the
/// [`index`](crate::stdlib::features::index) feature.
#[derive(Debug, Default, Clone, Copy)]
pub struct Refpoints;

impl FlmFeature for Refpoints {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Reference points").docs(
            "The table of reference points (table `flm.refpoints`), which lists the places of \
             the document that can be referred to, numbered or given an anchor, such as \
             headings and equation lines. The `refs`, `numbering` and `xnotes` features read \
             this table.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table(
            REFPOINTS,
            "For each tree, the reference points that the `Referenceable` aspect returns for \
             the nodes of the tree, in tree order, each with the identifier of the reference \
             point.",
        )?;
        b.stage(Arc::new(Scan))?;
        Ok(())
    }
}

/// Returns the [`Refpoints`] feature as a shared feature object, for instance
/// to list it in a [`PackageEntry`](crate::core::registry::PackageEntry).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Refpoints)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{LangInputs, ParseOptions, build_language, parse_fragment};
    use crate::model::aspects::{AcceptedRefTypes, MalformedLabel};
    use crate::model::doc::FragmentLayout;
    use crate::model::names::RefKind;
    use techy::core::node::NodeId;
    use techy::source::{Source, SourceSpan};

    /// The root's id of a parse under the bare language: a point needs a node
    /// address, and node ids come from trees only.
    fn root_id() -> NodeId {
        let lang = build_language(LangInputs::new());
        let source = Arc::new(Source::new("x").with_origin(FlmOrigin::none()));
        let fragment =
            parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
                .expect("a parse of `x`");
        fragment.tree().root().id()
    }

    fn span() -> FlmSpan {
        let source = Arc::new(Source::new("").with_origin(FlmOrigin::none()));
        SourceSpan::new(&source, 0..0)
    }

    /// A point of the root node at the given point index.
    fn record(root: NodeId, index: u32, labels: &[&str]) -> RefPointRecord {
        RefPointRecord {
            id: PointId { addr: NodeAddr::new(TreeId::tree(0), root), index },
            point: RefPoint {
                kind: RefKind::from("section"),
                labels: labels.iter().map(|l| Label::parse(l).unwrap()).collect(),
                malformed: Vec::new(),
                computed: 0,
                anchor: AnchorPolicy::Auto { prefix: "sec".to_string() },
                number: None,
                content: None,
                accepts: AcceptedRefTypes::Any,
            },
            span: span(),
        }
    }

    #[test]
    fn the_table_keeps_the_points_in_order_and_finds_one_by_identity() {
        let root = root_id();
        let mut points = RefPoints::default();
        points.insert(record(root, 0, &["sec:a"]));
        points.insert(record(root, 1, &[]));
        assert_eq!(points.len(), 2);
        assert_eq!(points.records()[1].id.index, 1);
        let second = PointId { addr: NodeAddr::new(TreeId::tree(0), root), index: 1 };
        assert!(points.point(second).is_some_and(|record| record.point.labels.is_empty()));
        assert_eq!(points.of_tree(TreeId::tree(0)).count(), 2);
        assert_eq!(points.of_tree(TreeId::tree(1)).count(), 0);
    }

    #[test]
    fn a_piece_holding_a_label_states_a_document_fact() {
        let root = root_id();
        let mut unlabeled = RefPoints::default();
        unlabeled.insert(record(root, 0, &[]));
        assert_eq!(unlabeled.piece_class(), PieceClass::NodeKeyed);
        let mut labeled = RefPoints::default();
        labeled.insert(record(root, 0, &["sec:a"]));
        assert_eq!(labeled.piece_class(), PieceClass::DocumentFact);
    }

    #[test]
    fn a_points_mentions_come_back_in_the_order_the_document_wrote_them() {
        // The point keeps the parsed labels and the texts that are no label in
        // two lists; `mentions` puts them back in the written order, which is
        // what decides the key a recomposed output allocates for each.
        let root = root_id();
        let malformed = |text: &str, position: usize| MalformedLabel {
            text: text.to_string(),
            span: span(),
            position,
        };
        let written = |point: &RefPointRecord| {
            point.mentions().into_iter().map(|(text, _)| text).collect::<Vec<_>>()
        };

        // `\label{bad}` before `\label{sec:a}`.
        let mut first = record(root, 1, &["sec:a"]);
        first.point.malformed = vec![malformed("bad", 0)];
        assert_eq!(written(&first), ["bad", "sec:a"]);
        assert_eq!(first.mentions()[1].1.map(Label::to_string), Some("sec:a".to_string()));
        assert_eq!(first.mentions()[0].1, None);

        // `\label{sec:a}` before `\label{bad}`: a trailing text, whose
        // position is the end of the list.
        let mut last = record(root, 2, &["sec:a"]);
        last.point.malformed = vec![malformed("bad", 1)];
        assert_eq!(written(&last), ["sec:a", "bad"]);

        // Interleaved, and a point with no text that is no label.
        let mut mixed = record(root, 3, &["sec:a", "sec:b"]);
        mixed.point.malformed = vec![malformed("one", 1), malformed("two", 3)];
        assert_eq!(written(&mixed), ["sec:a", "one", "sec:b", "two"]);
        assert_eq!(written(&record(root, 4, &["sec:a"])), ["sec:a"]);
    }

    #[test]
    fn the_projection_shows_a_text_that_is_no_label_under_a_key_of_its_own() {
        // A tool reading the dump must never find, in a point's `labels`, a
        // text that appears in no index and resolves nowhere.
        struct NoCx;
        impl ProjectCx for NoCx {
            fn node_ref(&self, _: NodeAddr) -> SerialValue {
                SerialValue::Null
            }
            fn tree_ref(&self, _: TreeId) -> SerialValue {
                SerialValue::Null
            }
        }
        let keys = |points: &RefPoints| {
            let SerialValue::Map(points) = points.project(&NoCx) else { panic!("a map") };
            let (_, point) = points.into_iter().next().expect("one point");
            let SerialValue::Map(point) = point else { panic!("a map") };
            point
        };

        let root = root_id();
        let mut plain = RefPoints::default();
        plain.insert(record(root, 1, &["sec:a"]));
        let entries = keys(&plain);
        assert!(entries.iter().all(|(key, _)| key != "malformed"), "{entries:?}");

        let mut with_text = RefPoints::default();
        let mut point = record(root, 1, &["sec:a"]);
        point.point.malformed =
            vec![MalformedLabel { text: "bad".to_string(), span: span(), position: 0 }];
        with_text.insert(point);
        let entries = keys(&with_text);
        let labels = entries.iter().find(|(key, _)| key == "labels").expect("the labels");
        assert_eq!(labels.1, SerialValue::List(vec![SerialValue::Str("sec:a".to_string())]));
        let malformed =
            entries.iter().find(|(key, _)| key == "malformed").expect("the malformed texts");
        assert_eq!(malformed.1, SerialValue::List(vec![SerialValue::Str("bad".to_string())]));
    }

    #[test]
    fn the_scan_is_per_tree_and_writes_the_table() {
        let decl = Scan.decl();
        assert_eq!(decl.name, SCAN);
        assert_eq!(decl.effect, Effect::Compute);
        assert_eq!(decl.scope, Scope::PerTree);
        assert_eq!(decl.writes[0].name, REFPOINTS.name());
        assert_eq!(RefPoints::NAME, REFPOINTS.name());
        assert_eq!(Refpoints.info().name, FEATURE);
    }

    #[test]
    fn an_empty_table_is_an_empty_piece() {
        assert_eq!(RefPoints::default().piece_class(), PieceClass::Empty);
        assert!(RefPoints::default().is_empty());
    }
}
