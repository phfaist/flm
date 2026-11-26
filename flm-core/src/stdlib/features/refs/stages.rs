//! This module defines four of the six stages of the [`refs`](super) feature,
//! along with the render options [`RefsRenderOptions`] and the error
//! conditions of the feature. The two other stages compute the safe labels of
//! the recomposed LaTeX and are defined in [`safe_labels`](super::safe_labels).
//!
//! Most conditions are reported by the stages of this module. The conditions
//! [`UndeclaredVariant`] and [`NothingToShow`] are reported by the lowering
//! rule of `\ref`, in [`lower_reference`](super::lower_reference).
//!
//! # The stages
//!
//! The four stages run in the following order, because each of them reads a
//! table that the previous one writes. The documentation of each constant
//! lists the tables that its stage reads and writes.
//!
//! 1. The label scan [`SCAN`] runs once for the document. It builds the label
//!    table [`LABELS`] from the reference points that the
//!    [`refpoints`](crate::stdlib::features::refpoints) feature collected.
//!    The scan does not look for `\label` nodes: the element of a labeled
//!    construct reads its `\label` fields and returns the labels in its
//!    reference points
//!    ([`Referenceable`](crate::model::aspects::Referenceable)). The scan
//!    records the duplicate labels and the labels of a type that their point
//!    does not accept, and the label check reports them.
//! 2. The anchors stage [`ANCHORS_STAGE`] runs once for the document. It
//!    computes the anchor of every reference point. For every label, it
//!    records the link target and the content that a reference to the label
//!    displays.
//! 3. The resolve stage [`RESOLVE`] runs once for each tree. It looks up the
//!    label of every reference site, and reports the labels that are not
//!    defined ([`Unresolved`]).
//! 4. The label check [`CHECK_LABELS`] runs once for the document. It reports
//!    the duplicate labels ([`DuplicateLabelCondition`]) and the labels of a
//!    type that their point does not accept ([`LabelTypeNotAccepted`]).
//!
//! # Anchors
//!
//! The anchors stage forms the anchor of each reference point according to the
//! policy set by the element of the point ([`AnchorPolicy`]). The [feature
//! documentation](super#anchors) describes the resulting forms of an anchor.
//! The stage visits the points in document order, and an anchor belongs to the
//! first point that receives it:
//!
//! - An anchor formed from a title ([`AnchorPolicy::Slug`]) that an earlier
//!   point already has receives a number suffix ([`free_number`]). Two
//!   headings with the same title are common, so this is not reported.
//! - An anchor of any other form that an earlier point already has is a
//!   collision. [`AnchorCollision`] is reported, and the later point receives
//!   no anchor. The labels of the later point get no entry in
//!   [`REF_TARGETS`], so a reference to them is unresolved.
//! - A point that is anchored on its label receives no anchor if each of its
//!   labels belongs to an earlier point or has a type that the point does not
//!   accept. The anchors stage does not report this, because the label check
//!   reports these labels.
//!
//! # References inside synthesized values
//!
//! The resolve stage is the only one of the four stages that runs once for
//! each tree. [Value catch-up](crate::guide::concepts_overview#value-catch-up)
//! therefore runs it again over a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), such as the
//! title of a heading that a `\ref` displays, and a reference inside the value
//! resolves like any other. This works because the resolve stage reads the
//! labels from [`REF_TARGETS`], which a document-scope stage writes and which
//! value catch-up does not compute again.
//!
//! Over a synthesized value, the resolve stage reports every undefined label,
//! as it does for the trees of the document. A reference with an undefined
//! label is therefore reported where it is written, and again wherever a
//! synthesized value that contains it is brought up to date. No label scan
//! runs over a synthesized value, so the resolve stage also reports there the
//! targets that do not have the form `type:label` ([`BadLabel`]).
//!
//! # Severity of an unresolved reference
//!
//! The label check reads the table [`REF_RESOLUTIONS`], although it does not
//! use it. This read makes every plan that runs the label check also run the
//! resolve stage, so that the `check` workflow reports the undefined labels
//! too.
//!
//! In a plan without an active backend, such as a plan of the `check`
//! workflow, the resolve stage reports an undefined label as a Warning,
//! regardless of [`RefsRenderOptions`]. Such a plan renders nothing, and it
//! cannot know whether the label would be defined in a render: a render may
//! install an external index ([`EXTERNAL_INDEX`]) that defines the label. In a
//! plan that renders, the severity is the one set in [`RefsRenderOptions`], an
//! Error by default. Duplicate labels and labels of a type that their point
//! does not accept depend on the document alone, and are Errors in every plan.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use techy::error::{Diagnostic, DiagnosticInfo, Severity};

use crate::model::addr::TreeId;
use crate::model::aspects::{AnchorPolicy, ContentRef, Label, PointId, Referencing};
use crate::model::names::RefKind;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::core::data::store::StageOut;
use crate::lang::text::plain_text_of;
use crate::lang::{nowhere, FlmOrigin};
use crate::stdlib::features::index::{ELEMENT_INDEX, walk_nodes_with};
use crate::stdlib::features::numbering::{self, COUNTERS, Counters, identifier_text};
use crate::stdlib::features::refpoints::{REFPOINTS, RefPointRecord};

use super::content::{
    auto_anchor, content_value, label_anchor, title_anchor, value_of,
    value_anchor,
};
use super::elements::reference_targets;
use super::tables::{
    ANCHORS, Anchors, EXTERNAL_INDEX, LABELS, Labels,
    PointDisplay, REF_RESOLUTIONS, REF_TARGETS, RefResolutions, RefTarget, RefTargets,
    Resolution, SiteId,
};

/// The name of the label scan stage, `flm.refs.scan`.
///
/// The label scan runs once for the document. It reads the reference points
/// of the table [`REFPOINTS`] in document order, and writes the label table
/// [`LABELS`]: the reference point of each label, the labels written for more
/// than one point, and the labels of a type that their point does not accept.
/// The label scan also reads the element index ([`ELEMENT_INDEX`]) to find the
/// `\ref` and `\hyperref` nodes of the document.
///
/// The label scan reports [`BadLabel`] for each label and each target of a
/// `\ref` or a `\hyperref` that does not have the form `type:label`.
pub const SCAN: &str = "flm.refs.scan";

/// The name of the anchors stage, `flm.refs.anchors`.
///
/// The anchors stage runs once for the document. It computes the anchor of
/// every reference point by the rules described in the [feature
/// documentation](super#anchors), and writes the anchors to the table
/// [`ANCHORS`]. For every label whose point receives an anchor, it writes to
/// the table [`REF_TARGETS`] the point, the link target, and what a reference
/// to the label displays ([`PointDisplay`]).
///
/// The anchors stage reads the reference points ([`REFPOINTS`]) and the label
/// table ([`LABELS`]). Where the [`numbering`] feature is registered, it also
/// reads the counter table ([`COUNTERS`]), because the anchor of some points
/// is formed from their number. The anchors stage reports [`AnchorCollision`]
/// and [`DisplayUnavailable`].
pub const ANCHORS_STAGE: &str = "flm.refs.anchors";

/// The name of the resolve stage, `flm.refs.resolve`.
///
/// The resolve stage runs once for each tree, including each synthesized value
/// that [value catch-up](crate::guide::concepts_overview#value-catch-up)
/// brings up to date. For every reference site of the tree, such as the
/// target of a `\ref`, it looks up the label in the table [`REF_TARGETS`], and
/// then in the external index [`EXTERNAL_INDEX`] if the caller installed one.
/// It writes the results to the table [`REF_RESOLUTIONS`]. It also reads the
/// element index ([`ELEMENT_INDEX`]) and the render options
/// [`RefsRenderOptions`].
///
/// The resolve stage reports [`Unresolved`] for each label that it finds in
/// neither table. In a synthesized value, it also reports [`BadLabel`] for
/// each target of a `\ref` or a `\hyperref` that does not have the form
/// `type:label`, because the label scan does not run over synthesized values.
pub const RESOLVE: &str = "flm.refs.resolve";

/// The name of the label check stage, `flm.refs.check-labels`, and of the
/// capability that this stage provides.
///
/// The capability is a member of the
/// [`flm.validated`](crate::stdlib::VALIDATED) collection, so every plan that
/// demands this collection runs the label check. The label check reads the
/// label table [`LABELS`]. It reports [`DuplicateLabelCondition`] for each
/// label written for more than one reference point, and
/// [`LabelTypeNotAccepted`] for each label of a type that its point does not
/// accept.
///
/// The label check also reads the table [`REF_RESOLUTIONS`], without using it.
/// This read makes every plan that runs the label check also run the resolve
/// stage ([`RESOLVE`]), so that the `check` workflow reports the undefined
/// labels too.
pub const CHECK_LABELS: &str = "flm.refs.check-labels";

/// The severity with which a reference to an undefined label is reported
/// ([`Unresolved`]). This is the type of the option `unresolved` of
/// [`RefsRenderOptions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnresolvedRefs {
    /// Reports the reference as an Error. A run that has recorded an Error
    /// stops before it writes any output file, unless the caller lets the run
    /// continue
    /// ([`RunOptions::continue_with_errors`](crate::core::run::RunOptions::continue_with_errors)).
    #[default]
    Error,
    /// Reports the reference as a Warning, which does not stop the run.
    Allow,
}

/// The render options of the [`refs`](super) feature. They are set per render
/// request ([`crate::model::options`]).
///
/// In a run that renders nothing, such as a run of the `check` workflow, these
/// options have no effect, and a reference to an undefined label is always
/// reported as a Warning. Such a run cannot know whether the label would be
/// defined in a render, because a render may install an index of the labels
/// defined in other documents ([`EXTERNAL_INDEX`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RefsRenderOptions {
    /// The severity with which a reference to an undefined label is reported:
    /// an Error with `error` (the default), and a Warning with `allow`.
    pub unresolved: UnresolvedRefs,
}

/// The error that is reported when a label, or a target of a `\ref` or a
/// `\hyperref`, does not have the form `type:label` (id `flm.refs.bad-label`).
///
/// A label must have a nonempty label type before its first colon and a
/// nonempty text after it, as in `\label{sec:intro}` and `\ref{sec:intro}`.
///
/// Reported by the label scan ([`SCAN`]) at the `\label` or at the reference.
/// A `\label` with such a text defines no label. A reference with such a
/// target expands like a reference to an undefined label: without a link,
/// and marked as unresolved. When the reference is part of content that another reference displays,
/// such as the title of a heading, the resolve stage ([`RESOLVE`]) reports the
/// target once more.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.bad-label",
    message = "Invalid label ‘{text}’: labels must have the form ‘type:label’"
)]
pub struct BadLabel {
    /// The text as written.
    pub text: String,
}

/// The error that is reported when a label is defined twice in a document
/// (`flm.refs.duplicate-label`). This is the case when the same label is
/// written for two different reference points, such as two headings.
///
/// Reported by the label check ([`CHECK_LABELS`]) at each reference point
/// after the first one. The first reference point in document order keeps the
/// label, and every reference to the label leads to it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.duplicate-label",
    message = "Duplicate definition of label ‘{label}’"
)]
pub struct DuplicateLabelCondition {
    /// The label.
    pub label: String,
}

/// The error that is reported when a label has a type that the labeled
/// construct does not accept (id `flm.refs.label-type-not-accepted`).
///
/// Each reference point accepts a fixed set of label types
/// ([`RefPoint::accepts`](crate::model::aspects::RefPoint::accepts)). For
/// instance, a heading accepts the types `sec` and `topic`, as in
/// `\section{Introduction}\label{sec:intro}`. Some constructs, such as a
/// proof, accept no label at all.
///
/// Reported by the label check ([`CHECK_LABELS`]) at the labeled construct.
/// The label is not defined, so a reference to it is unresolved
/// ([`Unresolved`]).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.refs.label-type-not-accepted")]
pub struct LabelTypeNotAccepted {
    /// The label as written.
    pub label: String,
    /// The reference kind of the labeled reference point, such as `section`.
    pub kind: String,
    /// The label types that the reference point accepts, in the order of its
    /// declaration. Empty when the reference point accepts no label.
    pub accepted: Vec<String>,
}

impl std::fmt::Display for LabelTypeNotAccepted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.accepted.is_empty() {
            return write!(
                f,
                "Invalid label ‘{}’: ‘{}’ accepts no label",
                self.label, self.kind
            );
        }
        write!(
            f,
            "Invalid label ‘{}’: ‘{}’ labels must be of the form {}",
            self.label,
            self.kind,
            self.accepted.iter().map(|prefix| format!("‘{prefix}:…’")).collect::<Vec<_>>().join(", ")
        )
    }
}

/// The error that is reported when a reference refers to an undefined label
/// (`flm.refs.unresolved`). A label is defined when it is written for a
/// reference point of the document, or when it is listed in the external
/// index ([`EXTERNAL_INDEX`]) that the caller installed for the run.
///
/// Reported by the resolve stage ([`RESOLVE`]) at the reference, once for each
/// undefined label of the reference. The severity is an Error by default, and
/// a Warning under the render option `unresolved: allow`
/// ([`RefsRenderOptions`]). In a run that renders nothing, such as a run of
/// the `check` workflow, the severity is always a Warning.
///
/// The reference expands to its text without a link, marked as unresolved: a
/// `\ref` displays the label, and a `\hyperref` displays its text argument.
/// When the reference is part of content that another reference displays,
/// such as the title of a heading, it is reported again there.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.unresolved",
    message = "Label ‘{label}’ is undefined"
)]
pub struct Unresolved {
    /// The label that the reference refers to.
    pub label: String,
}

/// The error that is reported when a `\ref` asks for a wording variant that
/// the counter style of a target does not declare (id
/// `flm.refs.undeclared-variant`).
///
/// Each key in the options of `\ref`, other than `noprefix` and `S`, asks for
/// the wording variant of that name for the word before a number, as in
/// `\ref[abbreviated]{lem:x}`. The [counter
/// style](crate::stdlib::features::numbering::CounterStyleDecl) of every
/// numbered target must declare this variant. The key `S` asks for the
/// variant `capital` only where the counter style declares it.
///
/// Reported by the lowering rule of `\ref` when it is invoked, once for each
/// counter style that does not declare the variant. Each such counter style
/// writes its usual wording.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.undeclared-variant",
    message = "Invalid counter variant ‘{variant}’ for counter style ‘{style}’"
)]
pub struct UndeclaredVariant {
    /// The name of the variant, as written in the key.
    pub variant: String,
    /// The name of the counter style that does not declare the variant.
    pub style: String,
}

/// The error that is reported when a reference refers to a reference point
/// that has neither a number nor content to display (id
/// `flm.refs.nothing-to-show`).
///
/// A `\ref` displays the number of its target, or, for a target without a
/// number, the content of the target, such as the title of a heading. Some
/// targets have neither, for instance an unnumbered figure without a caption,
/// or an equation in a registry without the [`numbering`] feature.
///
/// Reported by the lowering rule of `\ref` when it is invoked, once for each
/// such target. The other commands that display a reference through
/// [`lower_reference`](super::lower_reference), such as `\eqref`, report it in
/// the same way. The reference displays the label as written, with a link to
/// the target.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.nothing-to-show",
    message = "The reference point with label ‘{label}’ has neither number nor display text to show"
)]
pub struct NothingToShow {
    /// The label of the target, as written in the reference.
    pub label: String,
}

/// The error that is reported when two reference points receive the same
/// anchor (`flm.refs.anchor-collision`).
///
/// An anchor is the identifier that a link points to, such as the `id`
/// attribute of an HTML element ([`ANCHORS`]). Two points receive the same
/// anchor, for instance, when two different labels give the same anchor
/// ([`label_anchor`]), or when the element of a point sets the anchor itself
/// ([`AnchorPolicy::Explicit`]). An anchor formed from a title never
/// collides: it receives a number suffix instead, as in `sec--Examples-2`.
///
/// Reported by the anchors stage ([`ANCHORS_STAGE`]) at the later point. The
/// first point in document order keeps the anchor. The later point receives no
/// anchor, so no link leads to it, and a reference to one of its labels is
/// unresolved ([`Unresolved`]).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.anchor-collision",
    message = "Duplicate link identifier ‘{anchor}’ for both ‘{first}’ and ‘{second}’"
)]
pub struct AnchorCollision {
    /// The anchor that both points receive.
    pub anchor: String,
    /// The point that keeps the anchor. It is given by its first label, by the
    /// title that its anchor is formed from, or by its reference kind followed
    /// by its position among the points of that kind, as in `equation 3`.
    pub first: String,
    /// The later point, which receives no anchor, given in the same way.
    pub second: String,
}

/// The error that is reported when the content that a reference displays for
/// a reference point cannot be copied (id `flm.refs.display-unavailable`).
///
/// A reference to a point without a number displays the content of the
/// point, such as the title of a heading. The anchors stage
/// ([`ANCHORS_STAGE`]) copies this content when it records the point as the
/// target of its labels. The copy fails when the content is nested too deeply
/// ([`CopyError::NestingTooDeep`](super::CopyError::NestingTooDeep)), or
/// because of an error in the program
/// ([`CopyError::Failed`](super::CopyError::Failed)).
///
/// Reported by the anchors stage at the reference point. The content of the
/// point is replaced by an empty text: a reference to the point displays the
/// number of the point if it has one, and an empty text otherwise.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.display-unavailable",
    message = "Failed to process reference display text: {detail}"
)]
pub struct DisplayUnavailable {
    /// The description of the failure.
    pub detail: String,
}

/// The error that is reported when a stage of the [`refs`](super) feature
/// fails because of an error in the program (id `flm.refs.stage-failed`).
///
/// A document cannot cause this error. A stage reports it, for instance, when
/// it cannot access a table or an option type that it declared. This means
/// that the plan does not match the declarations of the stage. Reported at no
/// position in the document. When a stage cannot access a table or an option
/// type that it reads, it stops and writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.refs.stage-failed", message = "Stage ‘{stage}’ failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage.
    pub stage: String,
    /// The description of the failure.
    pub detail: String,
}

fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// Converts the index of a reference site among the sites of its node to the
/// `u32` of [`SiteId::index`]. An index above `u32::MAX` becomes `u32::MAX`;
/// a document cannot produce that many sites in one node.
fn point_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

// --- the scan ------------------------------------------------------------------------

/// This struct implements the label scan stage, [`SCAN`]. See the module
/// documentation.
pub(super) struct Scan;

impl Stage for Scan {
    fn decl(&self) -> StageDecl {
        StageDecl::new(SCAN, Effect::Compute, Scope::Document)
            .writes(LABELS)
            .docs(
                "Builds the label table `flm.labels` from the reference points in \
                 `flm.refpoints`: the reference point of each label, the labels written for \
                 more than one reference point, and the labels of a type that their reference \
                 point does not accept. Reports each label and each reference target that does \
                 not have the form `type:label`.",
            )
    }

    /// Declares required reads of the reference points ([`REFPOINTS`]) and of
    /// the element index ([`ELEMENT_INDEX`]). The scan uses the element index
    /// to find the `\ref` and `\hyperref` nodes. Because both reads are
    /// required, a plan that includes the scan cannot be built in a registry
    /// without the [`refpoints`](crate::stdlib::features::refpoints) feature
    /// ([`PlanError::NeedsUndeclared`](crate::core::plan::PlanError::NeedsUndeclared)).
    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(REFPOINTS).reads(ELEMENT_INDEX)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let points = match cx.get(REFPOINTS) {
            Ok(Some(points)) => points,
            Ok(None) => return,
            Err(error) => return stage_failed(out, SCAN, error),
        };
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return stage_failed(out, SCAN, error),
        };
        let mut labels = Labels::default();
        for record in points.in_document_order(cx) {
            // A label text the construct's reader could not parse is
            // reported here, at the field it was written in: the reader
            // parsed it once, as it read the point's data.
            for malformed in &record.point.malformed {
                let condition = BadLabel { text: malformed.text.clone() };
                out.diags.push(Diagnostic::error(condition, malformed.span.clone()));
            }
            labels.insert(record);
        }
        // Every referencing node of this feature's, `\ref` and `\hyperref`
        // alike, in document order: a target that is not spelled
        // `type:label` names nothing, and this is where it is reported.
        // Another feature's referencing element reads no target here and is
        // silently passed over, its own reader being its own business.
        for (_, node, _) in walk_nodes_with::<dyn Referencing>(cx, index) {
            // A `\ref` may name several targets at once, and each of them is
            // reported on its own.
            for target in reference_targets(&node).into_iter().flatten() {
                if target.label.is_none() {
                    let condition = BadLabel { text: target.text };
                    out.diags.push(Diagnostic::error(condition, node.span().clone()));
                }
            }
        }
        match out.table(LABELS) {
            Ok(table) => *table = labels,
            Err(error) => stage_failed(out, SCAN, error),
        }
    }
}

// --- the anchors ---------------------------------------------------------------------

/// This struct implements the anchors stage, [`ANCHORS_STAGE`]. See the
/// module documentation.
pub(super) struct AnchorsStage;

impl Stage for AnchorsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(ANCHORS_STAGE, Effect::Compute, Scope::Document)
            .writes(ANCHORS)
            .writes(REF_TARGETS)
            .docs(
                "Computes the anchor of every reference point (the identifier that a link \
                 points to) and writes it to `flm.anchors`. For every label, writes the link \
                 target and the content that a reference to the label displays to \
                 `flm.refs.targets`. Reports two reference points with the same anchor; an \
                 anchor formed from a title receives a number suffix instead.",
            )
    }

    /// Declares required reads of the reference points ([`REFPOINTS`]) and of
    /// the label table ([`LABELS`]). Where the [`numbering`] feature is
    /// registered, also declares a required read of the counter table
    /// ([`COUNTERS`]): the anchor of a point with the policy
    /// [`AnchorPolicy::FromValue`] is formed from the number of the point.
    ///
    /// The read of the counter table is required, and not optional, so that
    /// the numbering stage runs in every plan that includes the anchors stage.
    /// A point therefore receives the same anchor in every plan. The read is
    /// declared only where the numbering feature is registered, for two
    /// reasons. Plan construction fails for a stage that reads a table that is
    /// not declared in any feature. A stage, unlike a lowering rule, cannot
    /// declare a table of another feature as a peer.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let needs = Needs::new().reads(REFPOINTS).reads(LABELS);
        if cx.has_feature(numbering::FEATURE) { needs.reads(COUNTERS) } else { needs }
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let points = match cx.get(REFPOINTS) {
            Ok(Some(points)) => points,
            Ok(None) => return,
            Err(error) => return stage_failed(out, ANCHORS_STAGE, error),
        };
        let labels = match cx.get(LABELS) {
            Ok(Some(labels)) => labels,
            Ok(None) => return,
            Err(error) => return stage_failed(out, ANCHORS_STAGE, error),
        };
        let guard = cx.registry().lang().guard();
        // `None` is a plan in which nothing numbers: the table is read only
        // where the numbering feature is registered, which is the same
        // condition `needs` asks for it under, so no point is identified by a
        // number in a registry without that feature.
        let counters: Option<&Counters> = if cx.registry().has_feature(numbering::FEATURE) {
            match cx.get(COUNTERS) {
                Ok(counters) => counters,
                Err(error) => return stage_failed(out, ANCHORS_STAGE, error),
            }
        } else {
            None
        };
        // The anchor a point computes, and how the point is named in a
        // collision report; both keyed so that the first claimant of an
        // anchor can be named when a later point computes the same one.
        let mut claimed: BTreeMap<String, PointId> = BTreeMap::new();
        let mut named: BTreeMap<PointId, String> = BTreeMap::new();
        let mut ordinals: BTreeMap<RefKind, u32> = BTreeMap::new();
        let mut anchors = Anchors::default();
        let mut targets = RefTargets::default();
        for record in points.in_document_order(cx) {
            let point = &record.point;
            let counter = ordinals.entry(point.kind.clone()).or_insert(0);
            *counter += 1;
            let ordinal = *counter;
            let mut title = String::new();
            let kept = kept_label(labels, record);
            // `None` is a point that gets no anchor at all: every label it
            // wrote belongs to an earlier point, so it has no label of its
            // own to encode. The duplicate-label check reports that fact; the
            // anchors stage stays silent about it (one report per document
            // fact).
            let candidate: Option<String> = match &point.anchor {
                AnchorPolicy::Explicit(id) => Some(id.clone()),
                // A *labeled* point never falls back to its position: its
                // identifier would move when a point is inserted before it.
                AnchorPolicy::FromFirstLabel => kept.map(label_anchor),
                // `Auto` is `FromFirstLabel` for a labeled point, and the
                // point's position among the points of its kind for an
                // unlabeled one, so that every such point has an identifier.
                AnchorPolicy::Auto { prefix } => match kept {
                    Some(label) => Some(label_anchor(label)),
                    None if point.labels.is_empty() => Some(auto_anchor(prefix, ordinal)),
                    None => None,
                },
                AnchorPolicy::Slug { prefix, content } => {
                    title = frozen_text(cx, record.id.addr.tree, record, content);
                    // Numbered when the title repeats, so that no heading is
                    // left without an identifier.
                    Some(free_number(&claimed, title_anchor(prefix, &title)))
                }
                // The number this document gave the point. A point it did not
                // number has no number to be identified by and falls back to
                // its first surviving label, exactly as `FromFirstLabel`
                // does; with no label it gets no identifier at all.
                AnchorPolicy::FromValue { prefix } => {
                    match counters.and_then(|counters| counters.record(record.id)) {
                        Some(number) => Some(value_anchor(
                            prefix,
                            &identifier_text(&number.ancestors, &number.value),
                        )),
                        None => kept.map(label_anchor),
                    }
                }
            };
            let Some(candidate) = candidate else {
                anchors.ids.insert(record.id, None);
                continue;
            };
            let name = match kept.or_else(|| point.labels.first()) {
                Some(label) => label.to_string(),
                None if matches!(point.anchor, AnchorPolicy::Slug { .. }) => title,
                None => format!("{} {ordinal}", point.kind),
            };
            match claimed.entry(candidate.clone()) {
                Entry::Occupied(first) => {
                    let condition = AnchorCollision {
                        anchor: candidate,
                        first: named.get(first.get()).cloned().unwrap_or_default(),
                        second: name,
                    };
                    out.diags.push(Diagnostic::error(condition, record.span.clone()));
                    // No counter, no second anchor: the point is recorded
                    // without one, so a consumer tells it from a point the
                    // stage never saw.
                    anchors.ids.insert(record.id, None);
                }
                Entry::Vacant(slot) => {
                    slot.insert(record.id);
                    named.insert(record.id, name);
                    // What a reference to the point shows, made once for the
                    // point and only where a label names it: nothing else
                    // refers to a point.
                    let mut display: Option<PointDisplay> = None;
                    for label in &point.labels {
                        if labels.by_label.get(label) == Some(&record.id) {
                            let display = match &mut display {
                                Some(display) => display.clone(),
                                None => {
                                    let made = point_display(cx, guard, record, out);
                                    display.insert(made).clone()
                                }
                            };
                            targets.by_label.insert(
                                label.clone(),
                                RefTarget {
                                    point: record.id,
                                    href: format!("#{candidate}"),
                                    display,
                                },
                            );
                        }
                    }
                    anchors.ids.insert(record.id, Some(candidate));
                }
            }
        }
        match out.table(ANCHORS) {
            Ok(table) => *table = anchors,
            Err(error) => stage_failed(out, ANCHORS_STAGE, error),
        }
        match out.table(REF_TARGETS) {
            Ok(table) => *table = targets,
            Err(error) => stage_failed(out, ANCHORS_STAGE, error),
        }
    }
}

/// Returns the anchor `base` if it is not in `claimed`, the anchors that
/// earlier points received. Otherwise returns `base` followed by a hyphen and
/// the smallest number, from 2 up, that gives an anchor not in `claimed`.
///
/// For instance, the first heading titled `Examples` receives
/// `sec--Examples`, the second `sec--Examples-2`, and the third
/// `sec--Examples-3`.
fn free_number(claimed: &BTreeMap<String, PointId>, base: String) -> String {
    if !claimed.contains_key(&base) {
        return base;
    }
    // The loop always ends: a document holds finitely many claimed
    // identifiers, so some number is free.
    let mut number = 2u64;
    loop {
        let candidate = format!("{base}-{number}");
        if !claimed.contains_key(&candidate) {
            return candidate;
        }
        number += 1;
    }
}

/// Returns the first label of the point of `record` that belongs to this
/// point in the label table `labels`. The anchor of a point that is anchored
/// on its label is formed from this label.
///
/// The labels that belong to an earlier point are skipped, and so are the
/// labels of a type that the point does not accept.
fn kept_label<'a>(labels: &Labels, record: &'a RefPointRecord) -> Option<&'a Label> {
    record.point.labels.iter().find(|label| labels.by_label.get(*label) == Some(&record.id))
}

/// Returns the plain text of the content `content` of the node of `record`,
/// in the frozen tree `tree`. The anchors stage forms the anchor of a heading
/// from this text. Returns an empty string if the tree, the node or the
/// content does not exist.
fn frozen_text(
    cx: &StageCx<'_>,
    tree: TreeId,
    record: &RefPointRecord,
    content: &ContentRef,
) -> String {
    let Some(tree) = cx.frozen_tree(tree) else { return String::new() };
    let Some(node) = tree.get(record.id.addr.node) else { return String::new() };
    content.nodes(&node).map(plain_text_of).unwrap_or_default()
}

/// Returns what a reference to the point of `record` displays: the number
/// source of the point, and the content of the point as a synthesized value.
/// The content is copied out of the frozen tree, or shared when the element of
/// the point built it itself ([`content_value`]).
///
/// If the content reference of the point designates no node (an argument
/// that was not written), or if the content cannot be copied
/// ([`DisplayUnavailable`] is then reported), the content is an empty
/// synthesized value. Without any content,
/// a reference to the point would display its label instead and report
/// [`NothingToShow`].
fn point_display(
    cx: &StageCx<'_>,
    guard: crate::lang::guard::GuardPolicy,
    record: &RefPointRecord,
    out: &mut StageOut<'_>,
) -> PointDisplay {
    let number = record.point.number.clone();
    let Some(content) = &record.point.content else { return PointDisplay { number, content: None } };
    let Some(node) =
        cx.frozen_tree(record.id.addr.tree).and_then(|tree| tree.get(record.id.addr.node))
    else {
        return PointDisplay { number, content: None };
    };
    let copied = match content_value(&node, content, guard) {
        Ok(value) => value,
        Err(error) => {
            let condition = DisplayUnavailable { detail: error.to_string() };
            out.diags.push(Diagnostic::error(condition, record.span.clone()));
            None
        }
    };
    let content = match copied {
        Some(value) => Some(value),
        None => match value_of(&[], &node, guard) {
            Ok(empty) => Some(empty),
            Err(error) => {
                stage_failed(out, ANCHORS_STAGE, error);
                None
            }
        },
    };
    PointDisplay { number, content }
}

// --- the resolver --------------------------------------------------------------------

/// Returns the severity of [`Unresolved`] in the plan being run: a Warning in
/// a plan without an active backend, and otherwise the severity set in
/// `options`. The module documentation gives the reason, under *Severity of
/// an unresolved reference*.
fn unresolved_severity(cx: &StageCx<'_>, options: RefsRenderOptions) -> Severity {
    if cx.plan().active_backends().is_empty() {
        return Severity::Warning;
    }
    match options.unresolved {
        UnresolvedRefs::Error => Severity::Error,
        UnresolvedRefs::Allow => Severity::Warning,
    }
}


/// This struct implements the resolve stage, [`RESOLVE`]. See the module
/// documentation.
pub(super) struct Resolve;

impl Stage for Resolve {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RESOLVE, Effect::Compute, Scope::PerTree).writes(REF_RESOLUTIONS).docs(
            "Looks up the label of every reference in the labels of the document and in the \
             external index, and writes the results to `flm.refs.resolutions`. Reports every \
             reference to an undefined label.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(ELEMENT_INDEX)
            .reads(REF_TARGETS)
            .reads_opt(EXTERNAL_INDEX)
            .option::<RefsRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (Some(id), Some(tree)) = (cx.tree_id(), cx.my_tree()) else { return };
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return stage_failed(out, RESOLVE, error),
        };
        let targets = match cx.get(REF_TARGETS) {
            Ok(Some(targets)) => targets,
            Ok(None) => return,
            Err(error) => return stage_failed(out, RESOLVE, error),
        };
        let external = match cx.get(EXTERNAL_INDEX) {
            Ok(external) => external,
            Err(error) => return stage_failed(out, RESOLVE, error),
        };
        let options = match cx.option::<RefsRenderOptions>() {
            Ok(options) => *options,
            Err(error) => return stage_failed(out, RESOLVE, error),
        };
        let severity = unresolved_severity(cx, options);
        let mut resolutions = RefResolutions::default();
        let registry = cx.registry();
        for (addr, node, referencing) in index.nodes_with::<dyn Referencing>(registry, id, &tree)
        {
            // A target that is not spelled `type:label` declares no site. The
            // scan reports the document's trees'; a value is seen by no scan,
            // and its are reported here.
            if id.is_value() {
                for target in reference_targets(&node).into_iter().flatten() {
                    if target.label.is_none() {
                        let condition = BadLabel { text: target.text };
                        out.diags.push(Diagnostic::error(condition, node.span().clone()));
                    }
                }
            }
            for (index, site) in referencing.sites(&node).into_iter().enumerate() {
                let key = SiteId { addr, index: point_index(index) };
                let resolution = match targets.by_label.get(&site.target) {
                    Some(target) => Resolution::Resolved {
                        point: Some(target.point),
                        href: target.href.clone(),
                        display: target.display.clone(),
                    },
                    None => match external.and_then(|index| index.targets.get(&site.target)) {
                        Some(target) => Resolution::Resolved {
                            point: None,
                            href: target.href.clone(),
                            display: PointDisplay {
                                number: None,
                                content: Some(target.display.clone()),
                            },
                        },
                        None => {
                            let condition = Unresolved { label: site.target.to_string() };
                            let span = node.span().clone();
                            out.diags.push(Diagnostic::new(severity, condition, span));
                            Resolution::Unresolved { target: site.target }
                        }
                    },
                };
                resolutions.sites.insert(key, resolution);
            }
        }
        match out.table(REF_RESOLUTIONS) {
            Ok(table) => *table = resolutions,
            Err(error) => stage_failed(out, RESOLVE, error),
        }
    }
}

// --- the check -----------------------------------------------------------------------

/// This struct implements the label check stage, [`CHECK_LABELS`]. See the
/// module documentation.
pub(super) struct CheckLabels;

impl Stage for CheckLabels {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK_LABELS, Effect::Check, Scope::Document).provides(CHECK_LABELS).docs(
            "Reports every label written for more than one reference point, and every label of \
             a type that its reference point does not accept. Also reads \
             `flm.refs.resolutions`, so that the resolve stage runs in the same plan and \
             reports the references to undefined labels.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        // The resolutions are read so that the resolver joins every `check`
        // plan and its unresolved-reference diagnostics come with it; the
        // check itself reads only the labels.
        Needs::new().reads(LABELS).reads(REF_RESOLUTIONS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let labels = match cx.get(LABELS) {
            Ok(Some(labels)) => labels,
            Ok(None) => return,
            Err(error) => return stage_failed(out, CHECK_LABELS, error),
        };
        for duplicate in &labels.duplicates {
            out.diags.push(Diagnostic::error(
                DuplicateLabelCondition { label: duplicate.label.to_string() },
                duplicate.span.clone(),
            ));
        }
        for refusal in &labels.refused {
            out.diags.push(Diagnostic::error(
                LabelTypeNotAccepted {
                    label: refusal.label.to_string(),
                    kind: refusal.kind.to_string(),
                    accepted: refusal.accepted.clone(),
                },
                refusal.span.clone(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stages_declare_their_shapes() {
        assert_eq!(Scan.decl().scope, Scope::Document);
        assert_eq!(Scan.decl().writes.len(), 1);
        assert_eq!(AnchorsStage.decl().scope, Scope::Document);
        assert_eq!(AnchorsStage.decl().writes.len(), 2);
        assert_eq!(Resolve.decl().scope, Scope::PerTree);
        let check = CheckLabels.decl();
        assert_eq!(check.effect, Effect::Check);
        assert_eq!(check.provides, [crate::model::names::CapName::from(CHECK_LABELS)]);
    }

    #[test]
    fn the_default_options_make_an_unresolved_reference_an_error() {
        assert_eq!(RefsRenderOptions::default().unresolved, UnresolvedRefs::Error);
    }
}
