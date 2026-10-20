//! The tables of the [`refs`](super) feature. A table is typed data that one
//! stage writes beside the frozen trees and other stages read
//! ([`Table`]); each of these is identified by its dotted name:
//!
//! - [`LABELS`] (`flm.labels`, a [`Labels`]): for every label, the reference
//!   point that keeps it; the labels written more than once; and the labels
//!   refused because their point does not accept their type. The label scan
//!   writes it, from the reference points that the
//!   [`refpoints`](crate::stdcore::features::refpoints) feature recorded.
//! - [`ANCHORS`] (`flm.anchors`, an [`Anchors`]): the anchor of every
//!   reference point, or none. The anchors stage writes it.
//! - [`REF_TARGETS`] (`flm.refs.targets`, a [`RefTargets`]): for every label
//!   whose point received an anchor, the point that keeps it, its link target
//!   and what a reference to it
//!   shows. The anchors stage writes it, and the resolve stage reads it.
//! - [`REF_RESOLUTIONS`] (`flm.refs.resolutions`, a [`RefResolutions`]): the
//!   [`Resolution`] of every reference site. The resolve stage writes it, and
//!   the lowering rules of `\ref` and `\hyperref` read it.
//! - [`EXTERNAL_INDEX`] (`flm.refs.external-index`, an [`ExternalIndex`]):
//!   labels of *other* documents. It is an external table, a table that no
//!   stage of the run writes: the caller installs its value when the run is
//!   constructed ([`Externals`](crate::core::run::Externals)), and the resolve
//!   stage reads it when the caller has installed it.
//!
//! Every table iterates in an order fixed by its content (a map in key
//! order, a list in document order), so no hash-map order can reach a
//! projection, a diagnostic or a golden file. In a projection, a node address
//! is written by the projection context (`elem@line:col`), a point or a site
//! as `<node>#<index>`, and content held as a synthesized value as its plain
//! text, which keeps a projection readable.

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::serialize::SerialValue;

use crate::model::addr::{NodeAddr, TreeId};
use crate::model::aspects::{Label, NumberSource, PointId};
use crate::model::names::RefKind;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::lang::{Flm, FlmSpan};
use crate::stdcore::features::refpoints::{RefPointRecord, node_key, point_key, project_number};

use crate::lang::text::plain_text;

/// The typed name of the label table, `flm.labels`, a [`Labels`], which the
/// label scan ([`SCAN`](super::SCAN)) writes.
pub const LABELS: TableName<Labels> = TableName::new("flm.labels");

/// The typed name of the anchors table, `flm.anchors`, an [`Anchors`], which
/// the anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) writes.
///
/// An anchor is the identifier a link points at for a reference point:
/// `<type>-<label>` from the first label the point keeps (for a point whose
/// element asks for its label, or a labeled point whose element asks for its
/// position), `<prefix>--<title>` for an unlabeled heading,
/// `<prefix>-<numbers>` for a point identified by its number, labeled or not,
/// an
/// identifier derived from the position otherwise
/// (`<prefix>--<position><four letters>`), or an identifier its element gives
/// itself. The anchors stage of the `refs` feature computes it once for every
/// point, and the backends write it as the link target (on HTML, the `id` of
/// the element and the `href` of a link after a `#`). See the [module
/// documentation of the feature](super#anchors) for the rules and
/// [`encode_anchor`](super::encode_anchor) for the encoding.
pub const ANCHORS: TableName<Anchors> = TableName::new("flm.anchors");

/// The typed name of the targets table, `flm.refs.targets`, a
/// [`RefTargets`], which the anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) writes and the
/// resolve stage ([`RESOLVE`](super::RESOLVE)) reads.
pub const REF_TARGETS: TableName<RefTargets> = TableName::new("flm.refs.targets");

/// The typed name of the resolutions table, `flm.refs.resolutions`, a
/// [`RefResolutions`], which the resolve stage ([`RESOLVE`](super::RESOLVE)) writes and the
/// lowering rules
/// of references read.
pub const REF_RESOLUTIONS: TableName<RefResolutions> = TableName::new("flm.refs.resolutions");

/// The typed name of the external index, `flm.refs.external-index`, an
/// [`ExternalIndex`]: an external table, whose value the caller installs when
/// it constructs the run ([`Externals`](crate::core::run::Externals)), and
/// which no stage writes.
pub const EXTERNAL_INDEX: TableName<ExternalIndex> = TableName::new("flm.refs.external-index");

/// The identity of one reference site: the address of the node that declares
/// it and the site's index among that node's sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SiteId {
    /// The address of the node that declares the site.
    pub addr: NodeAddr,
    /// The site's index among the node's sites, in the order the node's
    /// [`Referencing`](crate::model::aspects::Referencing) aspect returns
    /// them.
    pub index: u32,
}

/// The two things a reference to a point may show, as the anchors stage
/// ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) records them: the number source
/// and the content that the
/// [`RefPoint`](crate::model::aspects::RefPoint) states, with the content
/// copied out of the document.
///
/// A reference shows the first of them that exists: the point's number,
/// formatted where the reference is written, where the numbering feature
/// numbered the point; else the content; else the label the reference was
/// written with, as written. Whether the point was numbered is not recorded
/// here, because the numbering stage decides it for each run (a point that
/// states a number source is numbered wherever it can be); this holds the
/// number *source*, the counter and the counter style, and never a formatted
/// number.
///
/// The content is stored as a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values), a small tree
/// held in the table, which the `\ref` rule brings up to date and splices into
/// the lowered tree.
#[derive(Debug, Clone, Default)]
pub struct PointDisplay {
    /// The point's number source (the counter and the counter style it is
    /// numbered by), where it can be numbered at all.
    pub number: Option<NumberSource>,
    /// The content the point shows where it has no number, as a synthesized
    /// value.
    pub content: Option<Arc<NodeTree<Flm>>>,
}

impl PointDisplay {
    /// Projects the display as `{number?: {counter, instances?, style,
    /// advance, number_format?}, content?: <plain text>}`. Each key is present
    /// only where the point states that thing: a point that can show nothing
    /// of its own projects as an empty map, a number source counted within no
    /// scoped instance has no `instances`, and one without a number template
    /// of its own has no `number_format`.
    ///
    /// `tree` is the tree of the node that carries the point, in which the
    /// instances (nodes of that tree) are rendered; a point of no tree of this
    /// run projects no instances.
    fn project(&self, cx: &dyn ProjectCx, tree: Option<TreeId>) -> SerialValue {
        let mut entries = Vec::with_capacity(2);
        if let Some(number) = &self.number {
            entries.push(("number".to_string(), project_number(cx, tree, number)));
        }
        if let Some(content) = &self.content {
            entries.push(("content".to_string(), SerialValue::Str(plain_text(content))));
        }
        SerialValue::Map(entries)
    }
}

/// A label written for two reference points, as [`Labels`] records it. The
/// label scan ([`SCAN`](super::SCAN)) records the fact; the label check
/// ([`CHECK_LABELS`](super::CHECK_LABELS)) reports it as
/// `flm.refs.duplicate-label`.
#[derive(Debug, Clone)]
pub struct DuplicateLabel {
    /// The label.
    pub label: Label,
    /// The point that keeps the label (the first in document order).
    pub first: PointId,
    /// The later point that also wrote it.
    pub second: PointId,
    /// The span of the later point's node.
    pub span: FlmSpan,
}

/// A label whose type the point it was written for does not accept, as
/// [`Labels`] records it. The label scan ([`SCAN`](super::SCAN)) records the fact;
/// the label check ([`CHECK_LABELS`](super::CHECK_LABELS)) reports it as
/// `flm.refs.label-type-not-accepted`.
///
/// A refused label defines nothing: no reference resolves to the point through
/// it, and a point whose every label was refused gets no anchor.
#[derive(Debug, Clone)]
pub struct RefusedLabel {
    /// The label as written.
    pub label: Label,
    /// The point it was written for.
    pub point: PointId,
    /// That point's reference kind (`section`), which says what sort of place
    /// the label was written on.
    pub kind: RefKind,
    /// The label types the point does accept, in the order it declares them.
    pub accepted: Vec<String>,
    /// The span of the point's node.
    pub span: FlmSpan,
}

/// The label table, `flm.labels` ([`LABELS`]): for every label, the first
/// point that wrote it; the labels written more than once; and the labels
/// refused because the point they were written for does not accept their
/// type.
///
/// The points themselves (their kinds, their labels as written, their anchor
/// policies) are held in the table of the
/// [`refpoints`](crate::stdcore::features::refpoints) feature,
/// [`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS), which the
/// label scan ([`SCAN`](super::SCAN)) reads; this table holds what the labels make of them.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    /// From a label to the point that keeps it.
    pub by_label: BTreeMap<Label, PointId>,
    /// The labels written more than once, in document order of the later
    /// point.
    pub duplicates: Vec<DuplicateLabel>,
    /// The labels whose type the point they were written for does not accept,
    /// in document order of that point.
    pub refused: Vec<RefusedLabel>,
}

impl Labels {
    /// Records the labels of a point: each label joins the index unless it is
    /// a written label whose type the point does not accept (a refusal is
    /// then recorded; the point's
    /// [computed](crate::model::aspects::RefPoint::computed) labels are not
    /// checked) or an earlier point already keeps it (a duplicate is then
    /// recorded). The label scan records the points in document order, so
    /// that the first point to write a label keeps it.
    pub fn insert(&mut self, record: &RefPointRecord) {
        let point = &record.point;
        for (at, label) in point.labels.iter().enumerate() {
            if at >= point.computed && !point.accepts.accepts(&label.ref_type) {
                self.refused.push(RefusedLabel {
                    label: label.clone(),
                    point: record.id,
                    kind: point.kind.clone(),
                    accepted: point.accepts.listed().unwrap_or(&[]).to_vec(),
                    span: record.span.clone(),
                });
                continue;
            }
            match self.by_label.get(label) {
                Some(first) if *first == record.id => {}
                Some(first) => self.duplicates.push(DuplicateLabel {
                    label: label.clone(),
                    first: *first,
                    second: record.id,
                    span: record.span.clone(),
                }),
                None => {
                    self.by_label.insert(label.clone(), record.id);
                }
            }
        }
    }
}

impl Table for Labels {
    /// The label scan runs once for the document and writes one table piece;
    /// a table piece absorbed after another keeps the labels of the earlier
    /// one, as a later point would.
    fn absorb(&mut self, piece: Labels) {
        for (label, point) in piece.by_label {
            self.by_label.entry(label).or_insert(point);
        }
        self.duplicates.extend(piece.duplicates);
        self.refused.extend(piece.refused);
    }

    /// Projects the table as `{labels: {<label>: <point>}, duplicates: […],
    /// refused: […]}`.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        let labels = self
            .by_label
            .iter()
            .map(|(label, id)| (label.to_string(), SerialValue::Str(point_key(cx, *id))))
            .collect();
        let duplicates = self
            .duplicates
            .iter()
            .map(|dup| {
                SerialValue::Map(vec![
                    ("label".to_string(), SerialValue::Str(dup.label.to_string())),
                    ("first".to_string(), SerialValue::Str(point_key(cx, dup.first))),
                    ("second".to_string(), SerialValue::Str(point_key(cx, dup.second))),
                ])
            })
            .collect();
        let refused = self
            .refused
            .iter()
            .map(|refusal| {
                let accepted = refusal
                    .accepted
                    .iter()
                    .map(|one| SerialValue::Str(one.clone()))
                    .collect();
                SerialValue::Map(vec![
                    ("label".to_string(), SerialValue::Str(refusal.label.to_string())),
                    ("point".to_string(), SerialValue::Str(point_key(cx, refusal.point))),
                    ("kind".to_string(), SerialValue::Str(refusal.kind.to_string())),
                    ("accepted".to_string(), SerialValue::List(accepted)),
                ])
            })
            .collect();
        SerialValue::Map(vec![
            ("labels".to_string(), SerialValue::Map(labels)),
            ("duplicates".to_string(), SerialValue::List(duplicates)),
            ("refused".to_string(), SerialValue::List(refused)),
        ])
    }

    /// A label states a fact about the document, which a [synthesized
    /// value](crate::guide::concepts_overview#synthesized-values) (a small
    /// tree a stage builds during a run) may never state.
    fn piece_class(&self) -> PieceClass {
        if self.by_label.is_empty() && self.duplicates.is_empty() && self.refused.is_empty() {
            PieceClass::Empty
        } else {
            PieceClass::DocumentFact
        }
    }
}

impl SingletonTable for Labels {
    const NAME: &'static str = "flm.labels";
}

/// The anchors table, `flm.anchors` ([`ANCHORS`]): every reference point of
/// the document with the anchor the anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE))
/// computed for it, or no
/// anchor when the point has nothing to form one from (no label it keeps,
/// under a policy that needs one) or when an earlier point already holds the
/// same one.
///
/// A point that the table does not hold at all is a point the stage never
/// saw, which is a different case: a consumer that finds one is reading data
/// that was never computed for the content it is looking at, and reports it
/// as such rather than treating the point as one without an anchor.
/// [`Anchors::lookup`] tells the two cases apart.
#[derive(Debug, Clone, Default)]
pub struct Anchors {
    /// From a point to its anchor; `None` for a point that received none.
    pub ids: BTreeMap<PointId, Option<String>>,
}

impl Anchors {
    /// The anchor of a point: `None` both for a point the stage never saw and
    /// for one that received no anchor. A consumer that must tell the two
    /// apart calls [`Anchors::lookup`].
    pub fn id(&self, point: PointId) -> Option<&str> {
        self.ids.get(&point).and_then(|id| id.as_deref())
    }

    /// What the table holds for a point: `None` when the stage never saw the
    /// point, `Some(None)` when it saw it and gave it no anchor (no label to
    /// form it from, or a collision with an earlier point), and
    /// `Some(Some(id))` when it gave it the anchor `id`.
    pub fn lookup(&self, point: PointId) -> Option<Option<&str>> {
        self.ids.get(&point).map(|id| id.as_deref())
    }
}

impl Table for Anchors {
    fn absorb(&mut self, piece: Anchors) {
        self.ids.extend(piece.ids);
    }

    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.ids
                .iter()
                .map(|(id, anchor)| {
                    let value = match anchor {
                        Some(anchor) => SerialValue::Str(anchor.clone()),
                        None => SerialValue::Null,
                    };
                    (point_key(cx, *id), value)
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.ids.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for Anchors {
    const NAME: &'static str = "flm.anchors";
}

/// What a label resolves to inside the document: the point that keeps it,
/// the link target (`#` followed by the anchor) and what a reference to it
/// shows.
#[derive(Debug, Clone)]
pub struct RefTarget {
    /// The point that keeps the label.
    pub point: PointId,
    /// The link target, `#<anchor>`.
    pub href: String,
    /// What a reference to the point shows.
    pub display: PointDisplay,
}

/// The targets table, `flm.refs.targets` ([`REF_TARGETS`]): from a label to
/// what it resolves to inside the document. It combines the label table and
/// the anchors, and the anchors stage
/// ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)), which runs once for the
/// document, writes it. A label whose point received no anchor (a point that
/// lost an anchor collision, or that has nothing to form an anchor from) has
/// no entry, so a reference to it resolves to nothing in the document.
///
/// The combination is a table of its own so that the resolve stage
/// ([`RESOLVE`](super::RESOLVE)) can read
/// it over a synthesized value. The resolve stage runs once per tree, so
/// [value catch-up](crate::guide::concepts_overview#value-catch-up) runs it
/// again over a synthesized value, and the per-tree tables it then reads are
/// those of that value. The table of reference points, `flm.refpoints`, is
/// one of them: inside a synthesized value it holds the value's own points,
/// usually none. This table is written by a stage that runs once for the
/// document, which value catch-up never runs again, so the resolve stage
/// reads the document's targets even over a synthesized value. Any other
/// table that holds facts about the whole document and that a per-tree stage
/// must read over a synthesized value needs the same arrangement.
#[derive(Debug, Clone, Default)]
pub struct RefTargets {
    /// From a label to its target.
    pub by_label: BTreeMap<Label, RefTarget>,
}

impl Table for RefTargets {
    fn absorb(&mut self, piece: RefTargets) {
        self.by_label.extend(piece.by_label);
    }

    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.by_label
                .iter()
                .map(|(label, target)| {
                    (
                        label.to_string(),
                        SerialValue::Map(vec![
                            ("point".to_string(), SerialValue::Str(point_key(cx, target.point))),
                            ("href".to_string(), SerialValue::Str(target.href.clone())),
                            (
                                "display".to_string(),
                                target.display.project(cx, Some(target.point.addr.tree)),
                            ),
                        ]),
                    )
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.by_label.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for RefTargets {
    const NAME: &'static str = "flm.refs.targets";
}

/// How one reference site resolved.
#[derive(Debug, Clone)]
pub enum Resolution {
    /// The target was found: in the document (`point` set) or in the external
    /// index (`point` absent).
    Resolved {
        /// The point, for a target inside the document.
        point: Option<PointId>,
        /// The link target: `#<anchor>` for a point, the index's target
        /// otherwise.
        href: String,
        /// What the site displays.
        display: PointDisplay,
    },
    /// No point of the document and no target of the external index carries
    /// the label; the resolve stage ([`RESOLVE`](super::RESOLVE)) reported
    /// `flm.refs.unresolved`.
    Unresolved {
        /// The label the site points at.
        target: Label,
    },
}

/// The resolutions table, `flm.refs.resolutions` ([`REF_RESOLUTIONS`]): how
/// every reference site resolved, for every node whose element carries the
/// [`Referencing`](crate::model::aspects::Referencing) aspect.
#[derive(Debug, Clone, Default)]
pub struct RefResolutions {
    /// From a site to its resolution.
    pub sites: BTreeMap<SiteId, Resolution>,
}

impl RefResolutions {
    /// The resolution of a site, or `None` for a site this table does not
    /// hold.
    pub fn site(&self, site: SiteId) -> Option<&Resolution> {
        self.sites.get(&site)
    }
}

impl Table for RefResolutions {
    fn absorb(&mut self, piece: RefResolutions) {
        self.sites.extend(piece.sites);
    }

    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.sites
                .iter()
                .map(|(id, resolution)| {
                    let key = format!("{}#{}", node_key(cx, id.addr), id.index);
                    let value = match resolution {
                        Resolution::Resolved { point, href, display } => SerialValue::Map(vec![(
                            "resolved".to_string(),
                            SerialValue::Map(vec![
                                (
                                    "point".to_string(),
                                    match point {
                                        Some(point) => SerialValue::Str(point_key(cx, *point)),
                                        None => SerialValue::Str("external".to_string()),
                                    },
                                ),
                                ("href".to_string(), SerialValue::Str(href.clone())),
                                (
                                    "display".to_string(),
                                    display.project(cx, point.map(|point| point.addr.tree)),
                                ),
                            ]),
                        )]),
                        Resolution::Unresolved { target } => SerialValue::Map(vec![(
                            "unresolved".to_string(),
                            SerialValue::Str(target.to_string()),
                        )]),
                    };
                    (key, value)
                })
                .collect(),
        )
    }
}

impl SingletonTable for RefResolutions {
    const NAME: &'static str = "flm.refs.resolutions";
}

/// A target of the external index: where a link to a label of another
/// document leads, and what a reference to it shows. What it shows is content
/// (a synthesized value), not a string, so that a reference to another
/// document is rendered the same way as one inside this document.
#[derive(Debug, Clone)]
pub struct ExternalTarget {
    /// The link target, as the caller wants it written (`other.html#sec`).
    pub href: String,
    /// The content a reference shows, as a synthesized value. Build it by
    /// parsing the text with the registry's language
    /// ([`parse_fragment`](crate::lang::parse_fragment)) and copying the
    /// parsed nodes with [`value_of`](super::value_of), as in
    /// `value_of(&nodes, &root, registry.lang().guard())`, where `root` is the
    /// root of the parsed tree and `nodes` its children.
    pub display: Arc<NodeTree<Flm>>,
}

/// The external index, `flm.refs.external-index` ([`EXTERNAL_INDEX`]): from
/// a label of another document to its target.
///
/// An embedding program that processes several documents installs it when it
/// constructs a run ([`Externals`](crate::core::run::Externals)); no stage
/// writes it. The resolve stage ([`RESOLVE`](super::RESOLVE)) reads it when the caller
/// has installed it and looks a
/// label up there only when no point of the document carries it; a run
/// without it has no external labels.
///
/// To install one, build it with [`ExternalIndex::new`] and
/// [`with`](ExternalIndex::with), one [`ExternalTarget`] per label, and pass
/// it as `Externals::none().install_unstamped(EXTERNAL_INDEX, index)`
/// ([`Externals::install_unstamped`](crate::core::run::Externals::install_unstamped))
/// when the run is constructed.
#[derive(Debug, Clone, Default)]
pub struct ExternalIndex {
    /// From a label to its target.
    pub targets: BTreeMap<Label, ExternalTarget>,
}

impl ExternalIndex {
    /// An empty index.
    pub fn new() -> ExternalIndex {
        ExternalIndex::default()
    }

    /// Builder: adds a target, replacing any target already held for that
    /// label.
    pub fn with(mut self, label: Label, target: ExternalTarget) -> ExternalIndex {
        self.targets.insert(label, target);
        self
    }
}

impl Table for ExternalIndex {
    fn absorb(&mut self, piece: ExternalIndex) {
        self.targets.extend(piece.targets);
    }

    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.targets
                .iter()
                .map(|(label, target)| {
                    (
                        label.to_string(),
                        SerialValue::Map(vec![
                            ("href".to_string(), SerialValue::Str(target.href.clone())),
                            ("display".to_string(), SerialValue::Str(plain_text(&target.display))),
                        ]),
                    )
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.targets.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for ExternalIndex {
    const NAME: &'static str = "flm.refs.external-index";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::aspects::{AcceptedRefTypes, AnchorPolicy, RefPoint};
    use crate::model::doc::FragmentLayout;
    use crate::lang::{FlmOrigin, LangInputs, ParseOptions, build_language, parse_fragment};
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

    /// A point of the root node at the given point index (distinct points of
    /// one node, since the test needs distinct identities only).
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
    fn a_repeated_label_is_recorded_as_a_duplicate_keeping_the_first_point() {
        let root = root_id();
        let (first, second) = (record(root, 1, &["sec:a"]), record(root, 2, &["sec:a", "sec:b"]));
        let mut labels = Labels::default();
        labels.insert(&first);
        labels.insert(&second);
        assert_eq!(labels.by_label[&Label::new("sec", "a")], first.id);
        assert_eq!(labels.by_label[&Label::new("sec", "b")], second.id);
        assert_eq!(labels.duplicates.len(), 1);
        assert_eq!(labels.duplicates[0].second, second.id);
    }

    #[test]
    fn a_labels_piece_states_a_document_fact_only_when_it_holds_a_label() {
        let root = root_id();
        assert_eq!(Labels::default().piece_class(), PieceClass::Empty);
        let mut unlabeled = Labels::default();
        unlabeled.insert(&record(root, 1, &[]));
        assert_eq!(unlabeled.piece_class(), PieceClass::Empty);
        let mut labeled = Labels::default();
        labeled.insert(&record(root, 1, &["sec:a"]));
        assert_eq!(labeled.piece_class(), PieceClass::DocumentFact);
    }

    #[test]
    fn the_anchors_table_tells_an_unseen_point_from_one_without_an_anchor() {
        let root = root_id();
        let seen = PointId { addr: NodeAddr::new(TreeId::tree(0), root), index: 1 };
        let collided = PointId { addr: NodeAddr::new(TreeId::tree(0), root), index: 2 };
        let unseen = PointId { addr: NodeAddr::new(TreeId::tree(0), root), index: 3 };
        let mut anchors = Anchors::default();
        anchors.ids.insert(seen, Some("sec-intro".to_string()));
        anchors.ids.insert(collided, None);
        assert_eq!(anchors.id(seen), Some("sec-intro"));
        assert_eq!(anchors.id(collided), None);
        assert_eq!(anchors.lookup(collided), Some(None));
        assert_eq!(anchors.lookup(unseen), None);
    }
}
