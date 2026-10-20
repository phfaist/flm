//! The `refpoints` feature: the table [`REFPOINTS`] of every reference point
//! of a run, and the stage that fills it.
//!
//! A reference point ([`RefPoint`]) is one place in a document that can be
//! referred to, numbered, or given an anchor (a heading, an equation line, a
//! theorem, a footnote); an element declares its reference points per
//! occurrence through the [`Referenceable`] aspect, and this feature records
//! them per tree. The concepts overview presents reference points together
//! with labels and anchors under [Reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors).
//!
//! The feature declares no command or environment; it serves other features.
//! Each point an element declares carries its kind, its labels, its anchor
//! policy ([`AnchorPolicy`]), its number source ([`NumberSource`]) where it
//! can be numbered, and the content that a reference to it shows. The
//! [`refs`](crate::stdcore::features::refs) feature builds its label index
//! and its anchors from the points, and the
//! [`numbering`](crate::stdcore::features::numbering) feature counts them.
//! This feature asks every node for its points once and records them in one
//! table, so that every reader reads the same points, and content that an
//! element builds for a point itself ([`ContentRef::Synthesized`]) is built
//! once per run and shared by every reader.
//!
//! # What the feature registers
//!
//! - **A stage.** [`SCAN`] (`flm.refpoints.scan`) computes the table, once per
//!   tree. It reads the [element index](crate::stdcore::features::index), asks
//!   every node whose element carries [`Referenceable`] for its points, once,
//!   and records each point whole, in tree order (the root, then its
//!   descendants in preorder).
//! - **A table.** [`REFPOINTS`] (`flm.refpoints`), a [`RefPoints`], is what
//!   the stage writes: one [`RefPointRecord`] per point, with lookup by the
//!   point's identity ([`PointId`]) and a read in document order across the
//!   trees of the run ([`RefPoints::in_document_order`]).
//!
//! # Which features need it
//!
//! The stages of the `refs` and `numbering` features read the table as a
//! required read, so a registry that holds either of them must hold this
//! feature, and this feature in turn reads the element index of the
//! [`index`](crate::stdcore::features::index) feature. A registry that lacks
//! this feature still builds, because the tables a stage reads are checked
//! when a plan is made; every plan made from it then fails with an error that
//! identifies the stage and the table `flm.refpoints`, which no feature
//! declares. This holds even for
//! a plan that schedules none of those stages, because making a plan asks
//! every registered stage what it needs. The standard package lists this
//! feature right after `index`.
//!
//! # Synthesized values
//!
//! The stage runs once per tree, so that [value
//! catch-up](crate::guide::concepts_overview#value-catch-up) runs it again over
//! a [synthesized value](crate::guide::concepts_overview#synthesized-values) (a
//! small tree a stage builds during a run), as it runs again every per-tree
//! stage a consumer depends on: the value then holds the points of its own
//! nodes. A labeled point states a fact about the document, which a
//! synthesized value may not state; see *What a table piece states* in
//! [`RefPoints`].

// Why a feature of its own, read by both consumers, rather than a collection
// in the element index or two aspects: see decision rationale at
// [§flm-dd-dr:std-refpoints].

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
use crate::stdcore::features::index::ELEMENT_INDEX;

/// The feature's registered name, `refpoints`.
pub const FEATURE: &str = "refpoints";

/// The name of the stage that records the reference points,
/// `flm.refpoints.scan`: it reads the element index and writes
/// [`REFPOINTS`].
pub const SCAN: &str = "flm.refpoints.scan";

/// The typed name of the table of reference points, `flm.refpoints`, a
/// [`RefPoints`]. The name is also the capability that a stage reading the
/// table requires.
pub const REFPOINTS: TableName<RefPoints> = TableName::new("flm.refpoints");

/// One reference point as the stage [`SCAN`] recorded it: its identity, the
/// point exactly as its element's [`Referenceable`] aspect returned it, and
/// the span of the node that carries it.
#[derive(Debug, Clone)]
pub struct RefPointRecord {
    /// The point's identity: the node carrying it and its index among that
    /// node's points.
    pub id: PointId,
    /// The point, exactly as the node's [`Referenceable`] aspect returned it.
    pub point: RefPoint,
    /// The span of the node carrying the point, for a diagnostic about it.
    pub span: FlmSpan,
}

impl RefPointRecord {
    /// Every text written as a label for this point, in the order the
    /// document wrote them: each text with its parsed label, or with `None`
    /// where the text is not spelled `type:label`.
    ///
    /// A point holds its parsed labels ([`RefPoint::labels`]) and the texts
    /// that are not labels ([`RefPoint::malformed`]) in two lists, because
    /// almost every reader wants the labels alone. A reader that needs the
    /// written order calls this method, which interleaves the two lists by
    /// the [position](crate::model::aspects::MalformedLabel::position) of each
    /// malformed text. The safe-labels stage of the `refs` feature
    /// ([`SafeLabelsStage`](crate::stdcore::features::refs::SafeLabelsStage))
    /// is such a reader: it numbers the labels in the order the document
    /// mentions them.
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

/// The table of reference points, `flm.refpoints` ([`REFPOINTS`]): every
/// reference point of every tree of the run, with lookup by the point's
/// identity.
///
/// The table holds the points tree after tree, in the run's order of the trees
/// ([`StageCx::trees`]); within a tree, in tree order (the root, then its
/// descendants in preorder); within a node, in the order the node's aspect returns them.
/// [`RefPoints::in_document_order`] returns them in document order across the
/// trees: the points of a tree placed in another tree come where that tree is
/// placed, between the points of the tree that places it.
///
/// Each record holds the whole point ([`RefPointRecord::point`]), its content
/// included: a tree that an element built for a point
/// ([`ContentRef::Synthesized`]) is held here, built once per run and shared
/// by every reader.
///
/// # What a table piece states
///
/// A table piece (the part of the table that one invocation of the stage
/// produced; [`PieceClass`]) that holds a labeled point states a fact about
/// the document ([`PieceClass::DocumentFact`]), because a label is what
/// another place of the document refers to. A synthesized value may never
/// state such a fact: a value holding a labeled point is refused when it is
/// brought up to date. A table piece holding only unlabeled points is data
/// keyed by node ([`PieceClass::NodeKeyed`]), which a synthesized value may
/// hold.
#[derive(Debug, Clone, Default)]
pub struct RefPoints {
    records: Vec<RefPointRecord>,
    by_id: BTreeMap<PointId, usize>,
}

impl RefPoints {
    /// Appends a point after every point already recorded. The stage
    /// [`SCAN`] records the points of a tree in tree order, and the table
    /// pieces of the trees are absorbed in the run's
    /// order of the trees.
    pub fn insert(&mut self, record: RefPointRecord) {
        self.by_id.insert(record.id, self.records.len());
        self.records.push(record);
    }

    /// Every point, tree after tree, in the order the table holds them (see
    /// [`RefPoints`]). A reader that needs document order calls
    /// [`RefPoints::in_document_order`].
    pub fn records(&self) -> &[RefPointRecord] {
        &self.records
    }

    /// Every point, in the run's document order ([`StageCx::walk`]): sorted by
    /// the position of its node ([`StageCx::position`]), the points of one
    /// node in the order the node's aspect returned them. Points of a
    /// synthesized value have no position and come first, in the order the
    /// table holds them. A stage of document scope that counts or orders the
    /// points calls this.
    pub fn in_document_order<'t>(&'t self, cx: &StageCx<'_>) -> Vec<&'t RefPointRecord> {
        // See decision rationale at [§flm-dd-dr:dm-run-order-position].
        // Every node of a tree of the run has a position; only a synthesized
        // value's records have none, and the stable sort keeps their order
        // (first).
        let mut records: Vec<&RefPointRecord> = self.records.iter().collect();
        records.sort_by_key(|record| cx.position(record.id.addr));
        records
    }

    /// The points of the tree `tree`, in tree order; none
    /// for a tree the table does not hold.
    pub fn of_tree(&self, tree: TreeId) -> impl Iterator<Item = &RefPointRecord> {
        self.records.iter().filter(move |record| record.id.addr.tree == tree)
    }

    /// The record of a point, or `None` for a point this table does not hold.
    pub fn point(&self, id: PointId) -> Option<&RefPointRecord> {
        self.by_id.get(&id).and_then(|index| self.records.get(*index))
    }

    /// The number of points recorded.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the table records no point.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl Table for RefPoints {
    /// The piece's points are recorded after this table's, in order.
    fn absorb(&mut self, piece: RefPoints) {
        for record in piece.records {
            self.insert(record);
        }
    }

    /// Projects the table as `{<point>: {kind, labels, malformed?, anchor,
    /// display: {number?, content?}}}`, the points in the order the table
    /// holds them, each keyed `<node>#<index>`.
    ///
    /// A point's `malformed` list holds the texts written as labels for it
    /// that are not spelled `type:label`, and is present only for a point
    /// that has one. Under `display`, `number` is the point's number source,
    /// present only where the point states one, and `content` is the content
    /// the point states: the plain text of a tree the element built, and
    /// otherwise the reference into the node's tree as text (`argument
    /// title`, `body`, a node), because the table holds the reference and not
    /// the content it designates.
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

/// A node address as the projection renders it, as a map key or a text.
pub(crate) fn node_key(cx: &dyn ProjectCx, addr: NodeAddr) -> String {
    match cx.node_ref(addr) {
        SerialValue::Str(text) => text,
        other => format!("{other:?}"),
    }
}

/// A point's key in a projection: `<node>#<index>`.
pub(crate) fn point_key(cx: &dyn ProjectCx, id: PointId) -> String {
    format!("{}#{}", node_key(cx, id.addr), id.index)
}

/// A number source as a projection shows it: `{counter, instances?, style,
/// advance, number_format?}`, with `instances` present only for a point
/// counted within a scoped instance and `number_format` only for a point
/// carrying a number template of its own.
///
/// `tree` is the tree of the node carrying the point, which the instances,
/// nodes of that tree, are rendered in; a point of no tree of this run
/// projects no instances.
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

/// An anchor policy as a short text, for projections.
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

/// A content reference as a short text, for projections: the reference as
/// text, and the plain text of a tree the aspect built.
pub(crate) fn content_text(cx: &dyn ProjectCx, tree: TreeId, content: &ContentRef) -> String {
    match content {
        ContentRef::Argument(index) => format!("argument {index}"),
        ContentRef::ArgumentNamed(name) => format!("argument {name}"),
        ContentRef::Body => "body".to_string(),
        ContentRef::Node(id) => format!("node {}", node_key(cx, NodeAddr::new(tree, *id))),
        ContentRef::Synthesized(value) => plain_text(value),
    }
}

/// Condition `flm.refpoints.stage-failed`, an Error: the stage [`SCAN`] could
/// not read or write a table it declared, which means that the plan does not
/// match the stage's declarations (a fault of the program, not of the
/// document). Recovery: the table piece of that tree is not written.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refpoints.stage-failed",
    message = "the stage `flm.refpoints.scan` failed: {detail}"
)]
pub struct StageFailed {
    /// What failed, rendered.
    pub detail: String,
}

/// A point's index among its node's, as the table keys it: a node with more
/// than `u32::MAX` points is not reachable from document input, and the
/// conversion saturates rather than truncates.
fn point_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

/// The stage [`SCAN`] (`flm.refpoints.scan`). See the module documentation.
struct Scan;

impl Stage for Scan {
    fn decl(&self) -> StageDecl {
        StageDecl::new(SCAN, Effect::Compute, Scope::PerTree).writes(REFPOINTS).docs(
            "Asks every node whose element carries the `Referenceable` aspect for its \
             reference points, once, and records each point whole, in document order, into \
             `flm.refpoints`.",
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

/// The `refpoints` feature ([`FlmFeature`]): it registers the table of
/// reference points [`REFPOINTS`] and the stage [`SCAN`] that fills it. See
/// the module documentation.
///
/// The feature reads the element index, so a registry that holds it must hold
/// the [`index`](crate::stdcore::features::index) feature too.
#[derive(Debug, Default, Clone, Copy)]
pub struct Refpoints;

impl FlmFeature for Refpoints {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Reference points").docs(
            "The table `flm.refpoints`: every reference point of the run (a place that can \
             be referred to, numbered or given an anchor), as its element declared it, in \
             document order; the `refs` and `numbering` features read it.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table(
            REFPOINTS,
            "Every reference point that a node's element declares through the \
             `Referenceable` aspect, whole, in document order, with lookup by the point's \
             identity.",
        )?;
        b.stage(Arc::new(Scan))?;
        Ok(())
    }
}

/// The [`Refpoints`] feature, ready to be listed in a package entry
/// ([`PackageEntry`](crate::core::registry::PackageEntry)).
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
