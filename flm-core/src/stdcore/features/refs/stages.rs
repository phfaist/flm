//! Four of the six stages of the [`refs`](super) feature, its render options
//! [`RefsRenderOptions`], and the conditions it reports. The two other stages
//! write the safe labels of the recomposed LaTeX and are defined in
//! [`safe_labels`](super::safe_labels).
//!
//! # The stages
//!
//! The four stages run in this order, each reading what the previous ones
//! wrote:
//!
//! - [`SCAN`] (`flm.refs.scan`) runs once for the document. It reads the
//!   reference points that the [`refpoints`](crate::stdcore::features::refpoints)
//!   feature recorded ([`REFPOINTS`]), in document order, and records in
//!   `flm.labels` which point keeps each label, the labels written twice, and
//!   the labels refused because their point does not accept their type. It
//!   reports, as [`BadLabel`], every text written as a label for a point that
//!   is not spelled `type:label` (at the `\label` where it was written), and
//!   every `\ref` or `\hyperref` target that is not spelled that way. It never
//!   looks for `\label` nodes itself: a label is a field, and the construct
//!   that declared it reads it.
//! - [`ANCHORS_STAGE`] (`flm.refs.anchors`) runs once for the document.
//!   It reads the reference points and the labels, gives every point its
//!   anchor (`flm.anchors`), and writes `flm.refs.targets`: for every label
//!   whose point received an anchor, the point that keeps it, its link target,
//!   and the content a reference to
//!   it shows, copied into a [synthesized
//!   value](crate::guide::concepts_overview#synthesized-values). See *Anchors*
//!   below.
//! - [`RESOLVE`] (`flm.refs.resolve`) runs once per tree. It reads the
//!   element index and the targets, and the external index where the caller
//!   installed one, and resolves every reference site of every node whose
//!   element carries the [`Referencing`](crate::model::aspects::Referencing)
//!   aspect into `flm.refs.resolutions`, reporting a site that nothing
//!   resolves ([`Unresolved`]). Because it runs per tree and reads only tables
//!   written once for the document, [value
//!   catch-up](crate::guide::concepts_overview#value-catch-up) runs it again
//!   over a synthesized value, so a `\ref` inside such a value resolves too;
//!   over a value it also reports the malformed targets, which no scan sees
//!   there (*Reports over a synthesized value*, below).
//! - [`CHECK_LABELS`] (`flm.refs.check-labels`) is a check, run once for the
//!   document. It reports every duplicate label ([`DuplicateLabelCondition`])
//!   and every label of a type its point does not accept
//!   ([`LabelTypeNotAccepted`]). It also reads the resolutions, so that a plan
//!   that schedules the check schedules the resolve stage too, and the
//!   unresolved references are reported with the check. It provides the
//!   capability of its own name, a member of the
//!   [`flm.validated`](crate::stdcore::VALIDATED) collection.
//!
//! # Anchors
//!
//! An anchor is the identifier a link points at for a reference point
//! ([`ANCHORS`](super::ANCHORS)). The element of each point chooses how its
//! anchor is formed ([`AnchorPolicy`]):
//!
//! - A point whose element asks for its first label
//!   ([`AnchorPolicy::FromFirstLabel`]), and a labeled point whose element
//!   asks for its position ([`AnchorPolicy::Auto`]), is anchored on the
//!   encoding of the first label it keeps
//!   ([`label_anchor`](super::label_anchor)). A point that keeps no label
//!   gets no anchor under the first policy.
//! - An unlabeled heading's anchor is the prefix its element states, a double
//!   hyphen, and the encoding of its title (`sec--Examples`;
//!   [`title_anchor`](super::title_anchor)).
//! - An unlabeled point whose element asks for its position (an equation
//!   line) is anchored on the prefix, a double hyphen, its position among the
//!   points of its kind and four letters
//!   ([`auto_anchor`](super::auto_anchor)), so that every such point has an
//!   anchor a link can reach.
//! - A point whose element asks to be identified by its number
//!   ([`AnchorPolicy::FromValue`]) is anchored on the prefix, a hyphen and
//!   the number the document gave it, the numbers of the counters it is
//!   numbered within first ([`value_anchor`](super::value_anchor)). The stage
//!   reads that number from `flm.counters`, which it requires wherever the
//!   [`numbering`](crate::stdcore::features::numbering) feature is
//!   registered. A point the document did not number (it states no number
//!   source, its number needs a counter with no value yet, or no numbering
//!   feature is registered) takes the anchor of the first label it keeps
//!   instead, and has no anchor when it keeps no label.
//! - A point whose element gives an anchor itself
//!   ([`AnchorPolicy::Explicit`]) takes that anchor as given.
//!
//! The prefix, a label's type and text, a title and a number text go through
//! [`encode_anchor`](super::encode_anchor); the position, the four letters and
//! an explicit anchor are written unchanged.
//!
//! An anchor made from a title is numbered whenever an earlier point already
//! holds the anchor it would take, whichever point that is: the second
//! heading titled `Examples` takes `sec--Examples-2`, the third
//! `sec--Examples-3`, and a number another point has taken is skipped.
//! Writing the same title twice is common and loses nothing, so it is not
//! reported.
//!
//! Any other anchor cannot be numbered, so an anchor that an earlier point
//! already holds is a collision: the first point keeps the anchor, the second
//! gets none, and the Error [`AnchorCollision`] is reported. A point anchored
//! on its label (its element asks for its first label, or for its position
//! and the point is labeled) all of whose labels an earlier point already
//! keeps gets no anchor and no report of its own: the check reports the
//! duplicate labels.
//!
//! # Reports over a synthesized value
//!
//! The resolve stage reports over a synthesized value exactly as over a
//! document tree: every reference that nothing resolves is reported as
//! [`Unresolved`], regardless of what the value holds (content copied out of
//! the document and shown again, such as a heading's title shown by a
//! reference; a use site of a user definition expanded at lowering; a user
//! definition's template lowered unchanged at each of its use sites; or
//! content that the embedding program supplied). A reference that the
//! document writes and a synthesized value shows again is therefore reported
//! where it is written and once more wherever the value is brought up to
//! date; each report points at the place the reference was written. No scan
//! sees a synthesized value, so the resolve stage also reports there the
//! targets not spelled `type:label` ([`BadLabel`]), which the scan reports
//! for the document's trees.
//!
//! # Severity of an unresolved reference in a plan that renders nothing
//!
//! The label check reads the resolutions, so the resolve stage runs in every
//! plan of the `check` workflow, and its reports of unresolved references
//! appear beside the duplicate labels. In a plan that renders nothing, their
//! severity is a Warning, regardless of the render options: whether a label
//! resolves is decided at render time, and a run that installs an external
//! index may resolve it, so a plan that renders nothing can only say
//! that a reference *might* fail. The resolve stage decides this from the
//! plan being run: a plan without an active backend renders nothing. A plan
//! that renders keeps the severity that [`RefsRenderOptions`] sets, an Error
//! by default.
//!
//! Duplicate labels are Errors in both cases, because a label written twice
//! depends on the document alone. So is a label of a type that its point does
//! not accept.
//!
//! # Label types
//!
//! A label is written `type:label`, and a reference point declares which
//! label types may be written for it
//! ([`RefPoint::accepts`](crate::model::aspects::RefPoint::accepts)): a
//! heading accepts `sec` and `topic`, and a point declaring
//! [`AcceptedRefTypes::Any`](crate::model::aspects::AcceptedRefTypes) accepts
//! every type. A label of another type is refused: the scan records the
//! refusal and leaves the label out of the label index, so nothing resolves
//! through it, and a point whose every label was refused gets no anchor from
//! a label. The label check reports it; the scan does not, so that the fact
//! is reported once.

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
use crate::stdcore::features::index::{ELEMENT_INDEX, walk_nodes_with};
use crate::stdcore::features::numbering::{self, COUNTERS, Counters, identifier_text};
use crate::stdcore::features::refpoints::{REFPOINTS, RefPointRecord};

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

/// The name of the label scan, `flm.refs.scan`: it reads `flm.refpoints` and
/// the element index, writes `flm.labels`, and reports labels and reference
/// targets that are not spelled `type:label`. See the module documentation.
pub const SCAN: &str = "flm.refs.scan";

/// The name of the anchors stage, `flm.refs.anchors`: it reads
/// `flm.refpoints`, `flm.labels` and, where the numbering feature is
/// registered, `flm.counters`, and writes `flm.anchors` and
/// `flm.refs.targets`. See the module documentation.
pub const ANCHORS_STAGE: &str = "flm.refs.anchors";

/// The name of the resolve stage, `flm.refs.resolve`: it reads the element
/// index, `flm.refs.targets`, the render options [`RefsRenderOptions`] and,
/// where it is installed, `flm.refs.external-index`, and writes
/// `flm.refs.resolutions`. See the module documentation.
pub const RESOLVE: &str = "flm.refs.resolve";

/// The name of the label check, and of the capability it provides,
/// `flm.refs.check-labels`: a member of the
/// [`flm.validated`](crate::stdcore::VALIDATED) collection. The check reads
/// `flm.labels` and `flm.refs.resolutions` and reports duplicate labels and
/// labels of a type their point does not accept.
pub const CHECK_LABELS: &str = "flm.refs.check-labels";

/// The severity of a reference that points at no label. The value is set per
/// render request, as part of [`RefsRenderOptions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnresolvedRefs {
    /// An Error diagnostic. A run that has recorded an Error stops before it
    /// writes its outputs, unless the caller lets it continue
    /// ([`RunOptions`](crate::core::run::RunOptions)).
    #[default]
    Error,
    /// A Warning diagnostic: the output is written, and the reference shows
    /// its label text, marked unresolved.
    Allow,
}

/// The render options of the [`refs`](super) feature, an option type
/// ([`crate::model::options`]) set per render request.
///
/// In a plan that renders nothing, such as one of the `check` workflow, an
/// unresolved reference is always a Warning, and these options do not apply;
/// see the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RefsRenderOptions {
    /// The severity of a reference that points at no label: `error` (the
    /// default) or `allow` (a Warning).
    pub unresolved: UnresolvedRefs,
}

/// Condition `flm.refs.bad-label`, an Error: a text written as a label for a
/// reference point, or as the target of a `\ref` or a `\hyperref`, is not
/// spelled `type:label`. The label scan ([`SCAN`]) reports it for the
/// document's trees, the resolve stage ([`RESOLVE`]) for a synthesized value.
/// Recovery: the text defines no
/// label and declares no reference site; a reference to it is shown
/// unresolved.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.bad-label",
    message = "a label must have the form `type:label`, as in `sec:intro`, and `{text}` does not"
)]
pub struct BadLabel {
    /// The text as written.
    pub text: String,
}

/// Condition `flm.refs.duplicate-label`, an Error reported by the label check
/// ([`CHECK_LABELS`]): a label was written for two reference points. It is reported at the later
/// point. Recovery: the first point in document order keeps the label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.duplicate-label",
    message = "the label `{label}` must be written only once in a document, and it was \
               already written earlier"
)]
pub struct DuplicateLabelCondition {
    /// The label.
    pub label: String,
}

/// Condition `flm.refs.label-type-not-accepted`, an Error reported by the
/// label check ([`CHECK_LABELS`]): a label's type is not one that the labeled construct accepts
/// (`eq:x` written on a heading, which accepts `sec` and `topic`), or a label
/// is written on a construct that accepts none, such as a proof. Recovery:
/// the label defines nothing, so no reference resolves through it, and a
/// point whose every label was refused gets no anchor from a label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.refs.label-type-not-accepted")]
pub struct LabelTypeNotAccepted {
    /// The label as written.
    pub label: String,
    /// The reference kind of the point the label was written for (`section`).
    pub kind: String,
    /// The label types that point accepts, in the order it declares them;
    /// empty where it accepts none, and the message then says that the point
    /// accepts no label instead of listing types.
    pub accepted: Vec<String>,
}

impl std::fmt::Display for LabelTypeNotAccepted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.accepted.is_empty() {
            return write!(
                f,
                "the label `{}` cannot be written on this {}, which accepts no label",
                self.label, self.kind
            );
        }
        write!(
            f,
            "the label `{}` cannot be written on this {}: its type must be one of {}",
            self.label,
            self.kind,
            self.accepted.join(", ")
        )
    }
}

/// Condition `flm.refs.unresolved`: a reference points at a label that no
/// reference point of the document and no target of the external index
/// carries. The resolve stage ([`RESOLVE`]) reports it, as an Error or a Warning
/// ([`RefsRenderOptions`]; always a Warning in a plan that renders nothing).
/// Recovery: the reference shows its label text, without a link and marked
/// unresolved.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.unresolved",
    message = "the reference points at the label `{label}`, which is not defined"
)]
pub struct Unresolved {
    /// The label the reference points at.
    pub label: String,
}

/// Condition `flm.refs.undeclared-variant`, an Error: a `\ref` asks, by a key
/// of its options such as `capital`, for a wording variant of the word before
/// a number that the [counter
/// style](crate::stdcore::features::numbering::CounterStyleDecl) (how the
/// numbering feature shows a counter's value) of one of its numbered targets
/// does not declare. The key `S`, which asks for `capital` only where the counter style
/// declares it, never raises this condition. The `\ref` lowering rule reports
/// it, once per reference and per counter style that lacks the variant.
/// Recovery: that counter style writes its usual wording.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.undeclared-variant",
    message = "the reference asks for the wording `{variant}`, which the counter style \
               `{style}` does not declare"
)]
pub struct UndeclaredVariant {
    /// The variant asked for, as the key wrote it.
    pub variant: String,
    /// The name of the counter style that does not declare it.
    pub style: String,
}

/// Condition `flm.refs.nothing-to-show`, an Error: a reference resolves to a
/// point that has neither a number nor content to show (an unnumbered float
/// with no caption, a numbered equation in a registry without the numbering
/// feature). The lowering rule that shows the reference reports it, once per
/// such target. Recovery: the reference shows the label as it was written,
/// linked to the point.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.nothing-to-show",
    message = "the place labeled `{label}` must have a number or a text for this reference to \
               show, and has neither"
)]
pub struct NothingToShow {
    /// The label the reference points at, as written.
    pub label: String,
}

/// Condition `flm.refs.anchor-collision`, an Error reported by the anchors
/// stage ([`ANCHORS_STAGE`]): two reference points produce the same anchor (the identifier a link
/// points at), for instance two labels that encode alike. It is reported at
/// the later point. Recovery: the first point in document order keeps the
/// anchor; the second gets none, so no link leads to it. An anchor made from a
/// title never raises this condition: it is numbered instead.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.anchor-collision",
    message = "`{first}` and `{second}` produce the same link identifier `{anchor}`, which must \
               be unique"
)]
pub struct AnchorCollision {
    /// The anchor both points produced.
    pub anchor: String,
    /// The point that keeps the anchor: its first label, the title its
    /// anchor was made from, or its reference kind followed by its position
    /// among the points of that kind.
    pub first: String,
    /// The later point, which gets no anchor, given in the same way.
    pub second: String,
}

/// Condition `flm.refs.display-unavailable`, an Error reported by the anchors
/// stage ([`ANCHORS_STAGE`]), which copies the content of every labeled point into a synthesized
/// value: the content that a reference to a point shows could not be copied
/// (the recursion guard refused to descend further, or the copy could not be
/// built). Recovery: the point's content is an empty text, so a reference to
/// it shows its number where the document numbered it, and nothing otherwise.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.refs.display-unavailable",
    message = "the text that a reference to this place shows could not be prepared: {detail}"
)]
pub struct DisplayUnavailable {
    /// What failed.
    pub detail: String,
}

/// Condition `flm.refs.stage-failed`, an Error: a stage of the feature could
/// not read or write a table it declared, or read an option type it declared,
/// which means that the plan does not match the stage's declarations (a fault
/// of the program, not of the document). Recovery: the stage writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.refs.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage.
    pub stage: String,
    /// What failed, rendered.
    pub detail: String,
}

fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// The index of a point or a site among those of its node, as the tables key
/// it. Document input cannot produce a node with more than `u32::MAX` points,
/// and the conversion saturates rather than truncates.
fn point_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

// --- the scan ------------------------------------------------------------------------

/// The label scan [`SCAN`] (`flm.refs.scan`). See the module documentation.
pub(super) struct Scan;

impl Stage for Scan {
    fn decl(&self) -> StageDecl {
        StageDecl::new(SCAN, Effect::Compute, Scope::Document)
            .writes(LABELS)
            .docs(
                "Records in `flm.labels`, from the reference points of `flm.refpoints` in \
                 document order, which point keeps each label, the labels written twice and \
                 the labels whose type their point does not accept; reports labels and \
                 reference targets not spelled `type:label`.",
            )
    }

    /// Reads the reference points, which the
    /// [`refpoints`](crate::stdcore::features::refpoints) feature records (a
    /// required read: in a registry without that feature, every plan fails,
    /// and the error gives the table's name), and the element index, to find the references whose
    /// targets it checks.
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

/// The anchors stage [`ANCHORS_STAGE`] (`flm.refs.anchors`). See the module
/// documentation.
pub(super) struct AnchorsStage;

impl Stage for AnchorsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(ANCHORS_STAGE, Effect::Compute, Scope::Document)
            .writes(ANCHORS)
            .writes(REF_TARGETS)
            .docs(
                "Gives every reference point its anchor (the identifier a link points at), \
                 formed from its label, its title, its position or its number, into \
                 `flm.anchors`, reporting two points with the same anchor; writes, for every \
                 label whose point received an anchor, its point, its link target and the \
                 content a reference shows, into `flm.refs.targets`.",
            )
    }

    /// Reads the reference points, the labels, and the counter table
    /// `flm.counters` wherever the
    /// [`numbering`](crate::stdcore::features::numbering) feature is
    /// registered: a point anchored at its number
    /// ([`AnchorPolicy::FromValue`]) is identified by the number the document
    /// gave it, so the numbering stage must run first.
    ///
    /// The read of the counter table is required rather than optional, so
    /// that the numbering stage is scheduled and a point's anchor is the same
    /// in every plan over the document. It is declared only where the
    /// numbering feature is registered, because a stage that reads a table no
    /// feature declares makes plan construction fail, and a stage, unlike a
    /// lowering rule, cannot declare a table of another feature through a
    /// peer declaration.
    // Why the read is required and asked for under the feature's presence:
    // see design rationale at [§flm-dd-dr:std-anchor-counters].
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
                // See design rationale at [§flm-dd-dr:std-anchor-encoding].
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
                    // left without an identifier. See design rationale at
                    // [§flm-dd-dr:std-anchor-encoding].
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

/// `base` itself when no point holds it, and otherwise `base` followed by a
/// hyphen and the lowest number from 2 up such that no point holds the
/// result either.
///
/// This tells two headings with one title apart: the first keeps
/// `sec--Examples`, the next takes `sec--Examples-2`, the next
/// `sec--Examples-3`, and a number that a heading already holds under a title
/// of its own is skipped.
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

/// The first label of `record` that the label index assigns to `record`
/// itself: the label the point's anchor is encoded from. A label that an
/// earlier point already keeps is skipped (that point keeps both the label
/// and the anchor, and the label check reports the duplicate), and so is a
/// label of a type the point does not accept.
fn kept_label<'a>(labels: &Labels, record: &'a RefPointRecord) -> Option<&'a Label> {
    record.point.labels.iter().find(|label| labels.by_label.get(*label) == Some(&record.id))
}

/// The plain text of a content reference of a frozen node, for a
/// title-derived anchor; empty when the tree or the content is not there.
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

/// What a reference to the point of `record` shows: its number source, and
/// its content copied out of the frozen tree into a synthesized value, or
/// shared where the element built the content itself.
///
/// A point that states content but has none to show (an absent argument, or
/// a copy that failed, which is reported as [`DisplayUnavailable`]) shows an
/// empty synthesized value rather than no content, because no content would
/// make a reference show its label instead.
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

/// The severity of an unresolved reference in the plan being run. See the
/// module documentation, *Severity of an unresolved reference in a plan that
/// renders nothing*.
///
/// A plan with no active backend renders nothing: it is a plan of the `check`
/// workflow, whose resolutions exist only so that the label check can read
/// them. Whether a label resolves is decided per plan at render time and may
/// depend on an external index a render installs, so such a plan can only
/// warn that a reference *might* fail to resolve: a Warning, regardless of
/// the render options. A plan that renders keeps the severity that
/// [`RefsRenderOptions`] sets.
fn unresolved_severity(cx: &StageCx<'_>, options: RefsRenderOptions) -> Severity {
    if cx.plan().active_backends().is_empty() {
        return Severity::Warning;
    }
    match options.unresolved {
        UnresolvedRefs::Error => Severity::Error,
        UnresolvedRefs::Allow => Severity::Warning,
    }
}


/// The resolve stage [`RESOLVE`] (`flm.refs.resolve`). See the module
/// documentation.
pub(super) struct Resolve;

impl Stage for Resolve {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RESOLVE, Effect::Compute, Scope::PerTree).writes(REF_RESOLUTIONS).docs(
            "Resolves every reference site against the labels of the document and of the \
             external index, into `flm.refs.resolutions`; reports every reference that points \
             at no label.",
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

/// The label check [`CHECK_LABELS`] (`flm.refs.check-labels`): it reports
/// duplicate labels and labels of a type their point does not accept. See
/// the module documentation.
pub(super) struct CheckLabels;

impl Stage for CheckLabels {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK_LABELS, Effect::Check, Scope::Document).provides(CHECK_LABELS).docs(
            "Reports every label written for two reference points and every label whose type \
             its point does not accept; it reads the resolutions so that the resolve stage \
             runs and its reports of unresolved references come with the check.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        // The resolutions are read so that the resolver joins every `check`
        // plan and its unresolved-reference diagnostics come with it; the
        // check itself reads only the labels. See design rationale at
        // [§flm-dd-dr:std-check-unresolved].
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
