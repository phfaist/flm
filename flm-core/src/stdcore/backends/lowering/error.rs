//! The conditions that lowering reports, `flm.lowering.*`, one condition about
//! verbatim content, and the recoveries after them.
//!
//! A **condition** is the typed body of a diagnostic: a value that says what
//! happened and holds the facts describing it, identified as
//! `flm.<feature>.<condition>` (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). A
//! **recovery** is what a feature does to go on after a problem it reported,
//! for example ignoring an option it cannot use, writing a stand-in for one
//! node, or giving up on a whole tree. A lowering rule that leaves out part
//! of the content it lowers may stage a `flm.pres.dropped` node, the dropped
//! mark ([`DroppedPayload`](crate::stdcore::backends::pres::DroppedPayload)),
//! where that content was. A diagnostic's message states the problem only;
//! the documentation of each condition below states its recovery, and each
//! feature's own conditions state theirs the same way.
//!
//! Lowering never stops the run because of a problem in a document: it
//! reports every problem as a diagnostic, never by panicking or by returning
//! an error to the caller, and recovers from it in one of two ways:
//!
//! - **Recovered at one node.** The node lowers to a childless
//!   `flm.pres.unsupported` stand-in, one Error diagnostic is reported at the
//!   node's span, and the rest of the tree lowers normally.
//!   [`UnsupportedElement`], [`RuleFailed`], [`RegionNotFound`] and [`NoRule`]
//!   are recovered this way.
//! - **The whole tree fails.** Lowering of that tree stops, the tree gets no
//!   entry in the backend's lowered-trees table ([`super::LoweredTrees`]), so
//!   nothing is emitted for it, and one Error diagnostic at the tree's root
//!   says why. [`RoundsExceeded`], [`LoweringFailed`] and [`NestingTooDeep`]
//!   are recovered this way.
//!
//! Three conditions are reported by lowering rules rather than by the driver,
//! each with a recovery of its own: [`FieldNotRead`], by the rule of the
//! field element, [`ContentNotCharacters`], by every rule that stages
//! verbatim content through
//! [`read_verbatim_content`](super::read_verbatim_content), and
//! [`MathUnprocessed`], by the rule of last resort of the math groups
//! ([`MathGroupRule`](super::MathGroupRule)) and by the `math` feature's rule
//! of last resort of the display-math environments. The second is identified as
//! `flm.verbatim.…`, after the structural element `flm.verbatim` (the
//! verbatim group) whose content it is about, and the third as `flm.math.…`,
//! after the structural element `flm.math` (the math group) in the same way.

use techy::error::DiagnosticInfo;

/// Condition `flm.lowering.unsupported-element`: the lowering reached a node
/// of an accepted unrenderable element, an element the plan request accepts
/// that the backend cannot render
/// ([`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported)).
/// The backend neither writes the element natively nor has a lowering rule
/// for it; the request accepted that instead of letting the plan build fail.
///
/// Recovery: the node lowers to a childless `flm.pres.unsupported` stand-in,
/// and one Error is reported per occurrence, at the node's span. The source
/// text of what could not be rendered stays in the diagnostic and never
/// reaches the output.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.unsupported-element",
    message = "the backend `{backend}` cannot render the element `{element}`: the backend \
               does not write it directly and has no lowering rule for it"
)]
pub struct UnsupportedElement {
    /// The name of the backend.
    pub backend: String,
    /// The name of the element that could not be rendered.
    pub element: String,
}

/// Condition `flm.lowering.rule-failed`: a lowering rule failed at a node. It
/// read a table or an option it did not declare, staged a presentation
/// element its `produces` declaration does not list, reached a node shape it
/// did not expect, or failed to bring a spliced [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) up to date
/// ([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)).
///
/// A spliced synthesized value fails to come up to date when it states a
/// fact about the document as a whole, such as a label: a synthesized value
/// is a tree built during the run, and bringing it up to date must add
/// nothing to what the document says, so a document fact in it is refused. A
/// rule that splices a value built from what an author wrote may instead
/// report the problem in the author's terms
/// ([`LowerError::Refused`](crate::core::lowering::LowerError::Refused)). For
/// example, a kept user definition whose template declares a label is
/// reported by the user-definitions feature's own condition,
/// [`KeptTemplateDeclaresPoint`](crate::stdcore::features::userdefs::kept::KeptTemplateDeclaresPoint),
/// which gives the fix, and not by this one.
///
/// Recovery: the node lowers to a childless `flm.pres.unsupported` stand-in,
/// as for [`UnsupportedElement`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.rule-failed",
    message = "the lowering rule `{rule}` failed at a node of the element `{element}` for the \
               backend `{backend}`: {detail}"
)]
pub struct RuleFailed {
    /// The name of the backend.
    pub backend: String,
    /// The name of the rule.
    pub rule: String,
    /// The name of the node's element.
    pub element: String,
    /// The failure, as text.
    pub detail: String,
}

/// Condition `flm.lowering.region-not-found`: a lowering rule asked to lower,
/// where its own node is placed, a node region (an argument or a slot) of
/// another node
/// ([`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region)),
/// and that region does not exist: no tree with the given identifier is
/// reachable, the tree holds no node at the given address, or the node has
/// no argument or slot at the given position. The mistake belongs to the
/// stage or rule that built the node referring to the region (for example a
/// stage that synthesized a value holding a [content
/// pointer](super::ContentPointer)), never to a document's author.
///
/// Recovery: the node whose rule asked lowers to a childless
/// `flm.pres.unsupported` stand-in, as for [`RuleFailed`], and nothing of the
/// region is shown. [`lower_content_pointer`](super::lower_content_pointer)
/// instead reports this condition at the pointer and writes nothing for the
/// pointer alone.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.region-not-found",
    message = "the lowering rule `{rule}`, at a node of the element `{element}` for the backend \
               `{backend}`, must refer to an existing argument or body of an existing node: \
               {detail}"
)]
pub struct RegionNotFound {
    /// The name of the backend.
    pub backend: String,
    /// The name of the rule that asked.
    pub rule: String,
    /// The name of the element of the node whose rule asked.
    pub element: String,
    /// What could not be found, as text.
    pub detail: String,
}

/// Condition `flm.lowering.no-rule`: the lowering reached a node whose
/// element the backend does not write natively, for which the plan selected
/// no lowering rule, and which is not an accepted unrenderable element.
///
/// Plan construction checks that the backend can render every element that
/// can occur in a document, so this condition is reachable only through a
/// tree holding an element that check did not cover, for instance a template
/// placeholder that remained after expansion.
///
/// Recovery: as for [`UnsupportedElement`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.no-rule",
    message = "the backend `{backend}` cannot render the element `{element}`: the backend \
               does not write it directly, no lowering rule is selected for it, and the request \
               for this output does not accept it as an element left unrendered"
)]
pub struct NoRule {
    /// The name of the backend.
    pub backend: String,
    /// The name of the element.
    pub element: String,
}

/// Condition `flm.lowering.rounds-exceeded`: lowering ran all the rounds it
/// is allowed ([`rank`](super::rank)) and the tree still holds elements the
/// backend does not write natively.
///
/// A rule that returns
/// [`Lowering::Default`](crate::core::lowering::Lowering::Default) for an
/// element the backend does not write natively, or whose `produces`
/// declaration does not list what it actually stages, makes the number of
/// rounds allowed too small and causes this condition.
///
/// Recovery: the tree has no lowered form.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.rounds-exceeded",
    message = "lowering for the backend `{backend}` must finish within {rank} rounds, but after \
               them the tree still holds elements the backend cannot write directly: {remaining}"
)]
pub struct RoundsExceeded {
    /// The name of the backend.
    pub backend: String,
    /// The number of rounds allowed ([`rank`](super::rank)).
    pub rank: u32,
    /// The elements the backend does not write natively that remain after
    /// the last round, in name order, separated by commas.
    pub remaining: String,
}

/// Condition `flm.lowering.failed`: lowering a tree failed for a reason that
/// no recovery at one node covers: a node could not be staged, by a rule or
/// by the driver itself, or the lowered tree could not be written into the
/// backend's table.
///
/// Recovery: the tree has no lowered form.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.failed",
    message = "lowering a tree for the backend `{backend}` failed: {detail}"
)]
pub struct LoweringFailed {
    /// The name of the backend.
    pub backend: String,
    /// What failed, as text.
    pub detail: String,
}

/// Condition `flm.lowering.nesting-too-deep`: the tree nests too deeply to
/// lower. The recursion guard, which bounds every recursive tree walk of the
/// library (see [the recursion guard and worker
/// threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)),
/// refused to descend one level further.
///
/// Each walk reports its own condition of this kind, so a diagnostic says
/// which step ran out of room: parsing and the rewrites report
/// `flm.blocks.nesting-too-deep`, bringing a spliced synthesized value up to
/// date reports `flm.synth.nesting-too-deep`, and lowering reports this one.
///
/// Recovery: the tree has no lowered form.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.nesting-too-deep",
    message = "the content nests too deeply to be lowered for the backend `{backend}`: {detail}"
)]
pub struct NestingTooDeep {
    /// The name of the backend.
    pub backend: String,
    /// The guard's explanation of which limit was reached.
    pub detail: String,
}

/// Condition `flm.lowering.field-not-read`: a field node reached the
/// lowering. A **field** is a macro that a construct absorbs as data rather
/// than as content, such as a `\label` written after a heading or a `\tag` on
/// an equation line (see [fields: data a construct
/// reads](crate::guide::concepts_overview#fields-data-a-construct-reads)).
///
/// A field is data that the construct it is written after reads; it
/// represents no content at its own position, so nothing can be rendered for
/// it. A field node that reaches the lowering means the construct that
/// absorbed it did not read it: the fault is that construct's, not the
/// document's.
///
/// Recovery: nothing is written for the node, and one Error is reported at
/// its span, on every backend.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.field-not-read",
    message = "the command `\\{field}` must be read as data by the construct it follows, but \
               that construct did not read it (backend `{backend}`)"
)]
pub struct FieldNotRead {
    /// The name of the backend.
    pub backend: String,
    /// The field's name as written, without the escape character.
    pub field: String,
}

/// Condition `flm.verbatim.content-not-characters`: the content of a
/// verbatim region holds something that is not characters (a command, an
/// environment, a special character sequence, mathematics), which a user
/// definition's template substituted there. A **verbatim region** is a run of
/// raw text in which no markup character means anything (see [verbatim
/// regions](crate::guide::concepts_overview#verbatim-regions)). For example,
/// `\code{\emph{x}}` puts `\emph{x}` into a verbatim region when `\code` is
/// a user definition written `\verbcode{#1}` with an ordinary argument.
///
/// [`read_verbatim_content`](super::read_verbatim_content) reports it at the
/// command or environment that owns the region, once per node lowered.
///
/// Recovery: the region shows its whole content written as its source text,
/// the parts that are not characters included, as characters: `\emph{x}` is
/// shown as those seven characters. The same on every backend.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.verbatim.content-not-characters",
    message = "the verbatim content of {name} must be plain characters, but it holds a command \
               or other markup"
)]
pub struct ContentNotCharacters {
    /// The command or environment that owns the verbatim region, as it is
    /// written (`\verbcode`, `{verbatimcode}`, or an extension's own command),
    /// or `a verbatim group` when no command or environment owns it.
    pub name: String,
}

/// Condition `flm.math.unprocessed` (Error): a math group (`\(…\)`, `\[…\]`,
/// `$…$`) or a display-math environment reached a backend on which no
/// registered feature processes mathematics, so a rule of last resort lowered
/// it: [`MathGroupRule`](super::MathGroupRule) for a math group, the `math`
/// feature's
/// [`UnprocessedEnvironmentRule`](crate::stdcore::features::math::unprocessed::UnprocessedEnvironmentRule)
/// for an environment. A feature that processes mathematics registers rules
/// for both, for example the feature of the package
/// [`math-passthrough`](crate::math_passthrough).
///
/// It is reported in a registry without a processing feature, with or without
/// the `math` feature, and in a registry whose processing feature has no rule
/// for the formula's element on the backend at hand. Reported once per math
/// group or environment, at its span.
///
/// Recovery: the content is lowered through the rules as ordinary content
/// (for an environment, the content of its equation lines), inside a
/// `flm.pres.span` of role `flm.math` and kind `inline` or `display`: a
/// character run is text, an unknown command its
/// `flm.pres.unknown`, a text command its `flm.pres.math-text`. Nothing is
/// relocated or dropped.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.math.unprocessed",
    message = "mathematics cannot be rendered: no loaded feature processes it; rendering it needs \
               the feature `math` and a package that processes mathematics, for example \
               `math-passthrough` or `math-mathjax`"
)]
pub struct MathUnprocessed;
