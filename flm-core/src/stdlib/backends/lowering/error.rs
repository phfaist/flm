//! This module defines the diagnostic conditions that are reported while a
//! document is lowered for a backend (see [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! The identifiers of most of these conditions begin with `flm.lowering.`.
//! Two conditions have other identifiers: [`ContentNotCharacters`]
//! (`flm.verbatim.content-not-characters`) and [`MathUnprocessed`]
//! (`flm.math.unprocessed`).
//!
//! Each of these conditions is reported as an Error diagnostic, and the
//! lowering then continues with the rest of the document. The lowering driver
//! ([`driver`](mod@super::driver)) handles a problem in one of two ways:
//!
//! - **At one node.** The node is replaced by a `flm.pres.unsupported` node
//!   without children, the error is reported at the node, and the rest of
//!   the tree is lowered as usual. [`UnsupportedElement`], [`NoRule`],
//!   [`RuleFailed`] and [`RegionNotFound`] are handled this way.
//! - **For the whole tree.** The lowering of the tree stops, and the error is
//!   reported at the root of the tree. The tree has no entry in the table of
//!   lowered trees of the backend ([`LoweredTrees`](super::LoweredTrees)), so
//!   the backend writes no output for this tree. [`RoundsExceeded`],
//!   [`NestingTooDeep`] and [`LoweringFailed`] are handled this way.
//!
//! The other three conditions, [`FieldNotRead`], [`ContentNotCharacters`] and
//! [`MathUnprocessed`], are reported by lowering rules. The documentation of
//! each of them states what is written in place of the content.

use techy::error::DiagnosticInfo;

/// The error that is reported when a document contains an element that the
/// backend cannot render (`flm.lowering.unsupported-element`).
///
/// A backend cannot render an element when it neither writes the element
/// natively nor has a lowering rule for it. If a document can contain such
/// an element, building the plan for the backend fails
/// ([`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete)),
/// unless the element is listed for this backend in
/// [`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported).
/// In that case the plan builds, and this error is reported for each
/// occurrence of the element in the document.
///
/// Reported by the lowering driver at each node of the element. The node is
/// replaced by a `flm.pres.unsupported` node without children, so its
/// content does not appear in the output.
///
/// An element that the backend cannot render and that is not listed in
/// `PlanRequest::unsupported` is reported as [`NoRule`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.unsupported-element",
    message = "Unsupported element ‘{element}’ for backend ‘{backend}’"
)]
pub struct UnsupportedElement {
    /// The name of the backend.
    pub backend: String,
    /// The name of the element that could not be rendered.
    pub element: String,
}

/// The error that is reported when a lowering rule fails at a node
/// (`flm.lowering.rule-failed`).
///
/// A lowering rule fails when it returns a
/// [`LowerError`](crate::core::lowering::LowerError). This error is usually
/// caused by a fault of the rule. For instance, the rule reads a table or an
/// option that it did not declare, stages a presentation element that is not
/// listed in its `produces` declaration, or finds a node structure that it
/// does not expect. A rule also fails when a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) that it splices
/// ([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)) cannot be
/// brought up to date.
///
/// Reported by the lowering driver at the node. The node is replaced by a
/// `flm.pres.unsupported` node without children.
///
/// Some kinds of `LowerError` are handled differently:
///
/// - [`LowerError::Refused`](crate::core::lowering::LowerError::Refused): the
///   diagnostic that the error contains is reported in place of this error,
///   and the node is replaced in the same way. A rule returns this error to
///   report a problem of the document in terms that the author can act on,
///   as the user-definitions feature does with
///   [`KeptTemplateDeclaresPoint`](crate::stdlib::features::userdefs::kept::KeptTemplateDeclaresPoint).
/// - [`LowerError::RegionNotFound`](crate::core::lowering::LowerError::RegionNotFound)
///   is reported as [`RegionNotFound`].
/// - [`LowerError::Build`](crate::core::lowering::LowerError::Build) stops the
///   lowering of the whole tree, and is reported as [`LoweringFailed`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.rule-failed",
    message = "Lowering rule ‘{rule}’ in backend ‘{backend}’ failed for ‘{element}’: {detail}"
)]
pub struct RuleFailed {
    /// The name of the backend.
    pub backend: String,
    /// The name of the lowering rule.
    pub rule: String,
    /// The name of the element of the node.
    pub element: String,
    /// The message of the [`LowerError`](crate::core::lowering::LowerError)
    /// that the rule returned.
    pub detail: String,
}

/// The error that is reported when a lowering rule calls
/// [`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region)
/// for a node region that does not exist (id
/// `flm.lowering.region-not-found`).
///
/// A node region is an argument or a content slot of a node
/// ([`NodeRegion`](crate::core::lowering::NodeRegion)). A rule calls
/// `lower_region` to lower a node region of another node at the position of
/// its own node, for instance the content that a
/// [`ContentPointer`](super::ContentPointer) refers to. The node region does
/// not exist when no document tree and no synthesized value that is being
/// lowered has the given tree identifier, when that tree has no node at the
/// given address, or when the node has no such argument or slot.
///
/// A document cannot cause this error: it is caused by the stage or the rule
/// that created the reference to the node region, for instance a stage that
/// built a synthesized value containing a content pointer.
///
/// Reported by the lowering driver at the node whose rule called
/// `lower_region`. The node is replaced by a `flm.pres.unsupported` node
/// without children. The function
/// [`lower_content_pointer`](super::lower_content_pointer) handles this error
/// differently: it reports the error at the content pointer node and returns
/// an empty list of staged nodes, so that the content pointer node is not
/// replaced.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.region-not-found",
    message = "Invalid content region reference in lowering rule ‘{rule}’ of backend ‘{backend}’ while lowering ‘{element}’: \
               {detail}"
)]
pub struct RegionNotFound {
    /// The name of the backend.
    pub backend: String,
    /// The name of the lowering rule that called `lower_region`.
    pub rule: String,
    /// The name of the element of the node whose rule called `lower_region`.
    pub element: String,
    /// A text that states which part of the node region is missing: the
    /// tree, the node, or the argument or slot.
    pub detail: String,
}

/// The error that is reported when the lowering reaches a node of an element
/// that the backend cannot render and that is not listed as unrenderable in
/// the plan request (id `flm.lowering.no-rule`).
///
/// The backend does not write the element natively, and no lowering rule is
/// selected for the element. Building the plan checks that the backend can
/// render every element that can occur in a document
/// ([`FlmRegistry::occurrable_elements`](crate::core::registry::FlmRegistry::occurrable_elements)).
/// This error can therefore occur only for an element that this check does
/// not cover, for instance a template placeholder that remains in the tree
/// after the expansion of user definitions.
///
/// A document cannot cause this error: it is caused by a fault of the
/// program, in a feature, a rewrite or an extension. This is the difference
/// from [`UnsupportedElement`], which is reported for an element that the
/// plan request lists as unrenderable, and whose message is addressed to the
/// author of the document.
///
/// Reported by the lowering driver at the node. The node is replaced by a
/// `flm.pres.unsupported` node without children.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.no-rule",
    message = "Missing lowering rule for element ‘{element}’ in backend ‘{backend}’"
)]
pub struct NoRule {
    /// The name of the backend.
    pub backend: String,
    /// The name of the element.
    pub element: String,
}

/// The error that is reported when the lowering of a tree reaches the maximum
/// number of rounds and the tree still contains elements that the backend
/// does not write natively (id `flm.lowering.rounds-exceeded`).
///
/// The lowering rewrites a tree in rounds. Each round applies the selected
/// lowering rules to the nodes of the elements that the backend does not
/// write natively, and the next round processes the resulting tree. The
/// maximum number of rounds is computed from the `produces` declarations of
/// the selected rules ([`rank`](super::rank)).
///
/// This error indicates a fault of a lowering rule. For instance, a rule that
/// returns [`Lowering::Default`](crate::core::lowering::Lowering::Default) for
/// an element that the backend does not write natively leaves the element in
/// the tree for the next round.
///
/// Reported by the lowering driver at the root of the tree. The tree has no
/// lowered form.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.rounds-exceeded",
    message = "Too many lowering rounds ({rank}) while running backend ‘{backend}’; elements that need further rounds: {remaining}"
)]
pub struct RoundsExceeded {
    /// The name of the backend.
    pub backend: String,
    /// The maximum number of rounds ([`rank`](super::rank)).
    pub rank: u32,
    /// The names of the elements that remain in the tree after the last
    /// round and that the backend does not write natively, sorted by name
    /// and separated by commas.
    pub remaining: String,
}

/// The error that is reported when the lowering of a tree fails as a whole
/// (`flm.lowering.failed`).
///
/// This happens when an error that cannot be handled at one node occurs
/// during the lowering, such as
/// [`LowerError::Build`](crate::core::lowering::LowerError::Build) for a node
/// that cannot be built. It also happens when the lowered tree cannot be
/// stored in the table of lowered trees of the backend
/// ([`LoweredTrees`](super::LoweredTrees)).
///
/// Reported at the root of the tree, by the lowering driver or by the
/// lowering stage ([`LoweringStage`](super::LoweringStage)). The tree has no
/// lowered form.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.failed",
    message = "Lowering failed in backend ‘{backend}’: {detail}"
)]
pub struct LoweringFailed {
    /// The name of the backend.
    pub backend: String,
    /// The message of the error that stopped the lowering of the tree.
    pub detail: String,
}

/// The error that is reported when a tree is nested too deeply to be lowered
/// (`flm.lowering.nesting-too-deep`).
///
/// The lowering walks the tree recursively. The recursion guard limits the
/// depth of every recursive walk of a tree in the library (see [the recursion
/// guard and worker
/// threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).
/// This error is reported when the guard does not allow the lowering to
/// descend one more level. The other walks report conditions of their own:
/// parsing and the rewrites report `flm.blocks.nesting-too-deep`, and
/// bringing a synthesized value up to date reports
/// `flm.synth.nesting-too-deep`.
///
/// Reported by the lowering driver at the root of the tree. The tree has no
/// lowered form.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.nesting-too-deep",
    message = "Content too deeply nested with backend ‘{backend}’: {detail}"
)]
pub struct NestingTooDeep {
    /// The name of the backend.
    pub backend: String,
    /// A text that states which limit of the recursion guard was reached.
    pub detail: String,
}

/// The error that is reported when a field reaches the lowering
/// (`flm.lowering.field-not-read`).
///
/// A field is a macro that a construct absorbs as data, not as content, such
/// as a `\label` written after a heading or a `\tag` on an equation line (see
/// [fields: data a construct
/// reads](crate::guide::concepts_overview#fields-data-a-construct-reads)).
/// The fields of a construct are read by the feature that defines the
/// construct, and the lowering rule of the construct must not lower them. A
/// field that reaches the lowering has therefore not been read.
///
/// A document cannot cause this error: it is caused by the lowering rule of
/// the construct that absorbed the field.
///
/// Reported by [`FieldRule`](super::FieldRule), the lowering rule of fields
/// on every backend, at the field. The field expands to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lowering.field-not-read",
    message = "Unread field ‘\\{field}’ left in the content (backend ‘{backend}’)"
)]
pub struct FieldNotRead {
    /// The name of the backend.
    pub backend: String,
    /// The field's name as written, without the escape character.
    pub field: String,
}

/// The error that is reported when the content of a verbatim region is not
/// plain characters (`flm.verbatim.content-not-characters`).
///
/// A verbatim region is text in which no character has a markup meaning, such
/// as the argument of `\verbcode` (see [verbatim
/// regions](crate::guide::concepts_overview#verbatim-regions)). Its content
/// is plain characters when it consists only of text, comments and brace
/// groups whose content is plain characters in turn. A brace group
/// contributes its content without the braces.
///
/// The text that an author writes in a verbatim region is always plain
/// characters. Other content gets there through a user definition whose
/// template places one of its arguments in a verbatim region. A call of
/// such a definition must pass plain characters in this argument. For
/// instance, if the template of `\code` is `\verbcode{#1}`, the call can be
/// `\code{x = y + 1}`.
///
/// Reported by [`read_verbatim_content`](super::read_verbatim_content), which
/// every lowering rule that stages verbatim content calls, at the command or
/// environment that contains the verbatim region. The verbatim region then
/// contains the source text of its whole content, markup included, as
/// characters. The same happens on every backend.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.verbatim.content-not-characters",
    message = "Invalid content in {name}: verbatim content must consist of plain characters"
)]
pub struct ContentNotCharacters {
    /// The name of the command or environment that contains the verbatim
    /// region, as written in the document: `\verbcode` for a command,
    /// `{verbatimcode}` for an environment. When no command or environment
    /// contains the region, this field contains the text `a verbatim group`.
    pub name: String,
}

/// The error that is reported when a math group or a display-math
/// environment is lowered on a backend for which no feature processes
/// mathematics (id `flm.math.unprocessed`).
///
/// A math group is mathematics written as `\(…\)`, `$…$` or `\[…\]`. Math
/// groups and display-math environments are lowered by the rules of a
/// feature that processes mathematics, for example the feature of the
/// package [`math-passthrough`](crate::math_passthrough). When no such rule
/// is registered for the backend, the mathematics is lowered by a rule of
/// last resort, which reports this error. A rule of last resort is selected
/// only when it is the only rule for the element on the backend. The rule of
/// last resort is [`MathGroupRule`](super::MathGroupRule) for a math group,
/// and
/// [`UnprocessedEnvironmentRule`](crate::stdlib::features::math::unprocessed::UnprocessedEnvironmentRule)
/// of the `math` feature for a display-math environment.
///
/// Reported at each math group or display-math environment that a rule of
/// last resort lowers. The content of the mathematics is lowered as ordinary
/// content, inside a `flm.pres.span` node of role `flm.math` and kind
/// `inline` or `display`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.math.unprocessed",
    message = "Mathematics processing is not configured for this document"
)]
pub struct MathUnprocessed;
