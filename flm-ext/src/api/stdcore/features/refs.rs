//! Mirrors [`flm_core::stdcore::features::refs`]: the names of the
//! references feature and the parts a referenceable construct of an
//! extension declares and lowers its labels with.
//!
//! A construct that carries reference points declares the `\label{…}` fields
//! written after it with [`label_fields_arg`], reads them back on an
//! occurrence with [`label_uses`], turns them into the labels of its points
//! with [`point_labels`] (an extension's
//! [`Referenceable`](crate::api::model::aspects::Referenceable) answers the
//! points), and writes them on the recomposed LaTeX backend with
//! [`stage_label`] and [`stage_label_group`], the two parts the `headings`
//! and `theorems` features' rules call.
//!
//! What differs from `flm-core`, for every item: nodes are the extension
//! API's [`Node`]s; a span is the extension API's [`Span`], on the source of
//! the node it was read from; the parts that stage take the extension API's
//! [`LowerCx`] and [`Lowerer`], answer the call's own [`Staged`] nodes, and
//! fail with [`ExtError`]; the table names are `&str` constants.
//!
//! Left out, each with the reason:
//!
//! - `label_uses_in`: no construct reads labels outside the fields argument;
//!   [`label_uses`] covers them.
//! - `auto_anchor`: the anchors stage computes an automatic anchor itself.
//! - `Anchors::lookup`: its answer, an option of an option, has no form
//!   across a JSON binding (an absent value and a value that is absent are
//!   both `null`); the [`Anchors`] façade's `id` answers what a construct
//!   needs.
//! - `label_field`, `label_package`, `field_text`, `label_use`,
//!   `split_targets`, `ref_targets`, `ref_display_options`,
//!   `hyperref_target`, `reference_targets`, `ref_element`, `ref_args`,
//!   `hyperref_element`, `hyperref_args`: the parts of `\ref` and
//!   `\hyperref` themselves, which an extension does not declare again.
//! - `stage_spelling`, `stage_ref_options`, `stage_labels_with_display_text`,
//!   `reference_type_words`, `stage_reference_type_setup`, `label_spelling`,
//!   `target_spelling`, `read_target_at_lowering`: the parts of the `\ref`
//!   rule; [`stage_label`] and [`stage_label_group`] are the two a
//!   construct calls.
//! - the stages' names and the condition types: nothing an extension
//!   declares.

use flm_core::core::lowering::{LowerCx as CoreLowerCx, LowerError, Lowerer as CoreLowerer};
use flm_core::stdcore::features::refs as core_refs;
use flm_core::techy::core::node::BuildId;

use crate::api::core::lowering::{LowerCx, Lowerer, Staged};
use crate::api::lang::mint::ArgDef;
use crate::api::model::aspects::{Label, MalformedLabel};
use crate::api::techy::node::{Node, Span};
use crate::adapter::tables::PointsCore;
use crate::api::model::aspects::PointId;
use crate::runtime::ExtError;
use crate::sealed::{Sealed, Token};

// --- the names ---------------------------------------------------------------------------------

/// Mirrors [`ANCHORS`](flm_core::stdcore::features::refs::ANCHORS): the
/// table of the anchors of the reference points, `flm.anchors`. What
/// differs: the name alone.
pub const ANCHORS: &str = core_refs::ANCHORS.name();

/// Mirrors [`SAFE_LABELS`](flm_core::stdcore::features::refs::SAFE_LABELS):
/// the table of the labels' spellings safe to write in LaTeX,
/// `flm.labels.safe`, which [`stage_label`] reads. What differs: the name
/// alone.
pub const SAFE_LABELS: &str = core_refs::SAFE_LABELS.name();

/// Mirrors [`LABELS`](flm_core::stdcore::features::refs::LABELS): the table
/// of the labels of the document's reference points, `flm.labels`. What
/// differs: the name alone.
pub const LABELS: &str = core_refs::LABELS.name();

/// Mirrors [`REF_RESOLUTIONS`](flm_core::stdcore::features::refs::REF_RESOLUTIONS):
/// the table of the resolved references, `flm.refs.resolutions`. What
/// differs: the name alone.
pub const REF_RESOLUTIONS: &str = core_refs::REF_RESOLUTIONS.name();

/// Mirrors [`FIELDS_ARG`](flm_core::stdcore::features::refs::FIELDS_ARG):
/// the name of the argument [`label_fields_arg`] declares, `fields`. Nothing
/// differs.
pub const FIELDS_ARG: &str = core_refs::FIELDS_ARG;

/// Mirrors [`LABEL_FIELD`](flm_core::stdcore::features::refs::LABEL_FIELD):
/// the name of the `\label` field, `label`. Nothing differs.
pub const LABEL_FIELD: &str = core_refs::LABEL_FIELD;

/// Mirrors [`TARGET_ARG`](flm_core::stdcore::features::refs::TARGET_ARG):
/// the name of the argument of `\ref` that holds its targets, `target`.
/// Nothing differs.
pub const TARGET_ARG: &str = core_refs::TARGET_ARG;

/// Mirrors [`LABEL_PACKAGE`](flm_core::stdcore::features::refs::LABEL_PACKAGE):
/// the scoped package of the `\label` field, `flm.refs.label`. Nothing
/// differs.
pub const LABEL_PACKAGE: &str = core_refs::LABEL_PACKAGE;

/// Mirrors [`TARGET_ARG_ROLE`](flm_core::stdcore::features::refs::TARGET_ARG_ROLE):
/// the argument role of a label that a reference points to,
/// `flm.refs.target`, which the references feature declares. Nothing
/// differs.
pub const TARGET_ARG_ROLE: &str = core_refs::TARGET_ARG_ROLE;

/// Mirrors [`DEFINED_LABEL_ARG_ROLE`](flm_core::stdcore::features::refs::DEFINED_LABEL_ARG_ROLE):
/// the argument role of a label that a `\label` attaches to a construct,
/// `flm.refs.defined-label`, which the references feature declares and the
/// argument of the `\label` field has. Nothing differs.
pub const DEFINED_LABEL_ARG_ROLE: &str = core_refs::DEFINED_LABEL_ARG_ROLE;

// --- the labels of a construct -----------------------------------------------------------------

/// Mirrors [`label_fields_arg`](flm_core::stdcore::features::refs::label_fields_arg):
/// the argument that reads the `\label{…}` fields written after a
/// construct, named [`FIELDS_ARG`], to place last in the construct's
/// argument list. Nothing differs; across a JSON binding the argument is a
/// handle of the kind `arg_def`, which an argument list takes in place of a
/// code.
///
/// The argument of the `\label` field has the argument role
/// [`DEFINED_LABEL_ARG_ROLE`], which the references feature declares. A
/// feature whose declarations use this argument also declares that role
/// with
/// [`FeatureBuilder::peer_argument_role`](crate::api::core::registry::FeatureBuilder::peer_argument_role),
/// so that a registry without the references feature still builds; without
/// the peer declaration, building such a registry fails.
pub fn label_fields_arg() -> ArgDef {
    core_refs::label_fields_arg()
}

/// Mirrors [`LabelUse`](flm_core::stdcore::features::refs::LabelUse): one
/// `\label{…}` written on a construct. What differs: the span is the
/// extension API's [`Span`], on the source of the construct's node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelUse {
    /// The text between the braces, trimmed.
    pub text: String,
    /// The span of the `\label` field, on the source of the construct's
    /// node.
    pub span: Span,
    /// The parsed label, when the text is of the form `type:label`.
    pub label: Option<Label>,
}

impl LabelUse {
    /// The mirror of `flm-core`'s `use_`.
    pub(crate) fn of(use_: &core_refs::LabelUse) -> LabelUse {
        let range = use_.span.range();
        LabelUse { text: use_.text.clone(), span: Span { start: range.start, end: range.end }, label: use_.label.clone() }
    }
}

/// Mirrors [`label_uses`](flm_core::stdcore::features::refs::label_uses):
/// the labels written in the [`FIELDS_ARG`] argument of `node`, in order;
/// none when the node has no such argument. What differs: see
/// [`LabelUse`].
pub fn label_uses(node: &dyn Node<'_>) -> Vec<LabelUse> {
    let core = node.core_node_ref(Token::new());
    core_refs::label_uses(&core).iter().map(LabelUse::of).collect()
}

/// Mirrors [`point_labels`](flm_core::stdcore::features::refs::point_labels):
/// the labels of a reference point from the labels written on its
/// construct: the parsed ones, in order, and the texts that do not parse,
/// each with its span and its position among `uses`. What differs: it takes
/// a slice of the mirror's [`LabelUse`] and answers the mirror's
/// [`MalformedLabel`]s, computed here over that data as `flm-core` computes
/// them, since the mirror's spans carry no source.
pub fn point_labels(uses: &[LabelUse]) -> (Vec<Label>, Vec<MalformedLabel>) {
    let mut labels = Vec::new();
    let mut malformed = Vec::new();
    for (position, use_) in uses.iter().enumerate() {
        match &use_.label {
            Some(label) => labels.push(label.clone()),
            None => malformed.push(MalformedLabel { text: use_.text.clone(), span: use_.span, position }),
        }
    }
    (labels, malformed)
}

// --- the recomposed LaTeX ----------------------------------------------------------------------

/// Stages through `flm-core`'s part `f`, as the call of `lw`.
fn stage(
    method: &str,
    cx: &mut dyn LowerCx,
    lw: &mut dyn Lowerer,
    mut f: impl FnMut(&mut CoreLowerCx<'_>, &mut dyn CoreLowerer) -> Result<Vec<BuildId>, LowerError>,
) -> Result<Vec<Staged>, ExtError> {
    lw.stage_core(Token::new(), method, cx, &mut f)
}

/// Mirrors [`stage_label`](flm_core::stdcore::features::refs::stage_label):
/// stages the `\label{…}` of one label use on the recomposed LaTeX backend,
/// its spelling the one the table [`SAFE_LABELS`] holds for it. The rule
/// must read [`SAFE_LABELS`] (optionally) and produce `flm.pres.latex.macro`
/// and `flm.pres.latex.raw`. What differs: the label is owned, and the node
/// is the call's own staged node.
///
/// # Errors
///
/// `cx` is not the call's context ([`ExtError::Argument`]), or `flm-core`'s
/// refusal ([`ExtError::Core`]): the rule does not declare the read or the
/// constructs.
pub fn stage_label(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, text: &str, label: Option<Label>) -> Result<Staged, ExtError> {
    let staged =
        stage("stage_label", cx, lw, |cx, lw| core_refs::stage_label(cx, lw, text, label.as_ref()).map(|id| vec![id]))?;
    staged.into_iter().next().ok_or_else(|| ExtError::Argument {
        method: "stage_label".to_string(),
        detail: "the label was not staged".to_string(),
    })
}

/// Mirrors [`stage_label_group`](flm_core::stdcore::features::refs::stage_label_group):
/// the nodes to emit for the labels `labels` of a construct on the
/// recomposed LaTeX backend: the labels themselves, or, when the document's
/// options ask for the label hooks, the labels inside the group that the
/// hooks read (with `empty_group`, a group even when there is no label). The
/// rule must read the option `LatexRecomposedRenderOptions` and require
/// `flm.latex.def.labels` when the hooks are on. What differs: the labels and
/// the answer are the call's own staged nodes.
///
/// # Errors
///
/// As [`stage_label`].
pub fn stage_label_group(
    cx: &mut dyn LowerCx,
    lw: &mut dyn Lowerer,
    labels: Vec<Staged>,
    empty_group: bool,
) -> Result<Vec<Staged>, ExtError> {
    let ids: Vec<BuildId> = labels.iter().map(|Staged(id)| *id).collect();
    // The labels are placed inside the group: checked and recorded as the
    // lowerer's own methods that take content check and record it.
    let mut f = |cx: &mut CoreLowerCx<'_>, lw: &mut dyn CoreLowerer| core_refs::stage_label_group(cx, lw, ids.clone(), empty_group);
    lw.stage_core_placing(Token::new(), "stage_label_group", cx, &labels, &mut f)
}

// --- the anchors table -------------------------------------------------------------------------

/// Mirrors [`Anchors`](flm_core::stdcore::features::refs::Anchors): the table
/// of the reference points' anchors, as a stage's context or a rule's
/// lowerer answers it for [`ANCHORS`]
/// ([`TableValue::Anchors`](crate::api::core::data::table::TableValue::Anchors)).
///
/// What differs: [`id`](Anchors::id) alone (`lookup` is left out, see the
/// module's text). Across a JSON binding a point is read back from its data
/// form `{ addr: { tree, node }, index }` against the trees the reader can
/// see: in a rule, the tree being lowered; in a stage, the trees of the
/// document. The trait is sealed: the adapter alone implements it.
pub trait Anchors: Sealed + PointsCore {
    /// Mirrors [`Anchors::id`](flm_core::stdcore::features::refs::Anchors::id):
    /// the anchor of `point`, or `None` both for a point the stage never saw
    /// and for one that received no anchor. What differs: the anchor is
    /// owned.
    fn id(&self, point: PointId) -> Option<String>;
}

fn _object_safe(_: &dyn Anchors) {}
