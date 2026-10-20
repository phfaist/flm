//! Mirrors techy's `transform` module, reached as
//! [`flm_core::techy::transform`]: restaging, techy's way of editing a frozen
//! tree.
//!
//! A driver reads the input tree and stages a copy of each node into a fresh
//! builder; a visitor decides, per node, whether the node is copied over its
//! restaged children ([`Restage::Descend`]) or replaced by nodes the visitor
//! staged itself ([`Restage::Emit`]). A rewrite stage of an extension
//! ([`RewriteStage`](crate::api::core::stage::RewriteStage)) is such a
//! visitor: the adapter is the driver, and the handler is called once per
//! node with a [`Restager`], the façade of techy's
//! [`RestageContext`](flm_core::techy::transform::RestageContext), through
//! which it stages what replaces the node.
//!
//! Staged nodes are [`Staged`] values, the build identifiers the expand and
//! lowering façades answer too. A restaged argument or slot is a bundle
//! ([`RestagedArgument`], [`RestagedSlot`]) that the handler passes back to
//! [`Restager::restage_invocation`] by value.

use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::sealed::{RestagedArgumentCore, RestagedSlotCore, RestagerCore};

pub use crate::api::lang::rewrites::userdefs::Staged;

/// Mirrors techy's [`Restage`](flm_core::techy::transform::Restage): what a
/// rewrite stage's handler answers for one node.
///
/// What differs: [`Descend`](Restage::Descend) carries no annotation (FLM's
/// trees carry none); the nodes of [`Emit`](Restage::Emit) are [`Staged`]
/// values, each of which the same handler call must have staged, and each
/// named once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restage {
    /// Keep the node: the driver stages a copy of it over its restaged
    /// children, whose subtrees all go through the handler.
    Descend,
    /// Replace the node with these nodes, which the handler staged during
    /// the same call (an empty list drops the node); the driver does not
    /// visit the node's children.
    Emit(Vec<Staged>),
}

/// Mirrors techy's
/// [`RestageContext`](flm_core::techy::transform::RestageContext): the
/// staging context of a rewrite, lent to a rewrite stage's handler for one
/// call.
///
/// What differs: no visitor argument, since the visitor is the adapter's,
/// which calls the handler again for every node a region operation
/// restages (nested calls, each with façades of its own); no annotation
/// argument (FLM's trees carry none); build identifiers are [`Staged`]
/// values, and a node given as content must have been staged by the same
/// handler call and is then placed, as the nodes of an `Emit` are;
/// bundles are owned values ([`RestagedArgument`],
/// [`RestagedSlot`]); errors are [`ExtError`], and techy's
/// [`RestageError`](flm_core::techy::transform::RestageError) is
/// [`ExtError::Core`] of kind `restage`, worded in the author's terms (a
/// construct as written, with its line; an argument or a slot by its index)
/// where techy's text names internal node identifiers. Added:
/// [`stage_chars`](Restager::stage_chars), with no techy counterpart, and
/// [`diagnose`](Restager::diagnose), the diagnostics of `flm-core`'s
/// [`RewriteStage::rewrite`](flm_core::core::stage::RewriteStage::rewrite).
/// Left out: `builder()`, the output tree's builder, whose `add` takes a node
/// kind, a span, a parsing state and an ext, objects of `flm-core` an
/// extension cannot make; [`stage_chars`](Restager::stage_chars) covers the
/// one new node a rewrite stages from scratch.
///
/// A techy error that leaves the rewrite unable to go on (a refusal of the
/// output builder, a dropped content parent, the descent guard's refusal)
/// ends the call: the tree's rewrite is then abandoned, whatever the handler
/// answers ([`RewriteStage`](crate::api::core::stage::RewriteStage)).
///
/// The trait is sealed: the adapter alone implements it.
pub trait Restager: RestagerCore {
    /// Mirrors techy's
    /// [`RestageContext::restage_subtree`](flm_core::techy::transform::RestageContext::restage_subtree):
    /// restages `node` and its whole subtree through the handler, and answers
    /// the nodes that replace `node`.
    ///
    /// # Errors
    ///
    /// A techy error ([`ExtError::Core`] of kind `restage`). A refusal of the output
    /// builder, a dropped content parent or the
    /// descent guard's refusal met while the nodes are restaged ends the call,
    /// and the tree's rewrite is abandoned.
    fn restage_subtree(&mut self, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_children`](flm_core::techy::transform::RestageContext::restage_children):
    /// restages every child subtree of `node`, the node itself excluded, and
    /// answers their replacements concatenated.
    ///
    /// # Errors
    ///
    /// As [`restage_subtree`](Restager::restage_subtree).
    fn restage_children(&mut self, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_argument`](flm_core::techy::transform::RestageContext::restage_argument):
    /// restages argument `index` of the callable `node` through the handler,
    /// and answers the bundle to pass to
    /// [`restage_invocation`](Restager::restage_invocation). An argument not
    /// provided is answered as an absent bundle.
    ///
    /// # Errors
    ///
    /// `node` is not a callable, or has no argument at `index`, plus the
    /// errors of [`restage_subtree`](Restager::restage_subtree)
    /// ([`ExtError::Core`] of kind `restage`). A refusal of the output builder, a dropped
    /// content parent or the
    /// descent guard's refusal met while the nodes are restaged ends the call,
    /// and the tree's rewrite is abandoned.
    fn restage_argument(&mut self, node: &dyn Node<'_>, index: usize) -> Result<Box<dyn RestagedArgument>, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_argument_named`](flm_core::techy::transform::RestageContext::restage_argument_named):
    /// as [`restage_argument`](Restager::restage_argument), for the argument
    /// named `name`.
    ///
    /// # Errors
    ///
    /// No argument of the callable is named `name`, plus the errors of
    /// [`restage_argument`](Restager::restage_argument)
    /// ([`ExtError::Core`] of kind `restage`). A refusal of the output builder, a dropped
    /// content parent or the
    /// descent guard's refusal met while the nodes are restaged ends the call,
    /// and the tree's rewrite is abandoned.
    fn restage_argument_named(&mut self, node: &dyn Node<'_>, name: &str) -> Result<Box<dyn RestagedArgument>, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_slot`](flm_core::techy::transform::RestageContext::restage_slot):
    /// restages slot `index` of the callable `node` through the handler, and
    /// answers the bundle to pass to
    /// [`restage_invocation`](Restager::restage_invocation).
    ///
    /// # Errors
    ///
    /// `node` is not a callable, or has no slot at `index`, plus the errors of
    /// [`restage_subtree`](Restager::restage_subtree) ([`ExtError::Core`] of
    /// kind `restage`). A refusal of the output builder, a dropped content parent or the
    /// descent guard's refusal met while the nodes are restaged ends the call,
    /// and the tree's rewrite is abandoned.
    fn restage_slot(&mut self, node: &dyn Node<'_>, index: usize) -> Result<Box<dyn RestagedSlot>, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_invocation`](flm_core::techy::transform::RestageContext::restage_invocation):
    /// stages a copy of the callable `node` whose arguments and slots are the
    /// bundles given, in the order given, and answers it. What differs: no
    /// annotation; the bundles are those this handler call was answered, each
    /// given once.
    ///
    /// # Errors
    ///
    /// `node` is not a callable ([`ExtError::Core`] of kind `restage`); a
    /// bundle was given before ([`ExtError::Argument`]); a bundle's nodes are
    /// already placed, or the output builder refuses the copy otherwise: the
    /// call then ends, and the tree's rewrite is abandoned. The bundles are
    /// taken one at a time, the arguments first, then the slots: after a
    /// refusal, a bundle given may be spent (those before the one at fault
    /// when a bundle is refused, all of them when techy refuses the call),
    /// so the handler stages new ones.
    fn restage_invocation(
        &mut self,
        node: &dyn Node<'_>,
        args: Vec<Box<dyn RestagedArgument>>,
        slots: Vec<Box<dyn RestagedSlot>>,
    ) -> Result<Staged, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_argument_with_content`](flm_core::techy::transform::RestageContext::restage_argument_with_content):
    /// restages argument `index` of the callable `node` with its content
    /// replaced by `content`, its surroundings copied unchanged. What
    /// differs: no annotation; `content` holds nodes this handler call staged.
    ///
    /// # Errors
    ///
    /// `node` is not a callable, has no argument at `index`, or the argument
    /// is not provided ([`ExtError::Core`] of kind `restage`); a node of
    /// `content` was not staged by this call, or is placed already, as
    /// content given before or as a node of an `Emit`
    /// ([`ExtError::Argument`]); the output builder refuses the copy: the
    /// call then ends, and the tree's rewrite is abandoned.
    fn restage_argument_with_content(
        &mut self,
        node: &dyn Node<'_>,
        index: usize,
        content: Vec<Staged>,
    ) -> Result<Box<dyn RestagedArgument>, ExtError>;

    /// Mirrors techy's
    /// [`RestageContext::restage_slot_with_content`](flm_core::techy::transform::RestageContext::restage_slot_with_content):
    /// restages slot `index` of the callable `node` with its content replaced
    /// by `content`, its surroundings copied unchanged. What differs: as
    /// [`restage_argument_with_content`](Restager::restage_argument_with_content).
    ///
    /// # Errors
    ///
    /// `node` is not a callable or has no slot at `index`
    /// ([`ExtError::Core`] of kind `restage`), and the other errors of
    /// [`restage_argument_with_content`](Restager::restage_argument_with_content).
    fn restage_slot_with_content(
        &mut self,
        node: &dyn Node<'_>,
        index: usize,
        content: Vec<Staged>,
    ) -> Result<Box<dyn RestagedSlot>, ExtError>;

    /// No techy counterpart: stages one characters node holding `text`, with
    /// the span and the parsing state of the node `like`, as a pass stages
    /// a characters node through the output tree's builder (and as
    /// [`ExpandCx::stage_chars`](crate::api::lang::rewrites::userdefs::ExpandCx::stage_chars)
    /// does at a use site).
    ///
    /// # Errors
    ///
    /// The output builder refuses the node: the call then ends, and the
    /// tree's rewrite is abandoned.
    fn stage_chars(&mut self, text: &str, like: &dyn Node<'_>) -> Result<Staged, ExtError>;

    /// Reports `diagnostic` (the diagnostics of `flm-core`'s
    /// [`RewriteStage::rewrite`](flm_core::core::stage::RewriteStage::rewrite)),
    /// at the node of the call unless it has a place of its own, as
    /// [`Lowerer::diagnose`](crate::api::core::lowering::Lowerer::diagnose)
    /// does. What differs: the diagnostic is data, and the method answers a
    /// `Result`, since it checks the diagnostic's identifier.
    ///
    /// # Errors
    ///
    /// The diagnostic's identifier does not begin with the extension's stem
    /// and a dot ([`ExtError::NameOutsideExtension`]), or its data cannot
    /// cross ([`ExtError::Value`]); nothing is reported then.
    fn diagnose(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError>;
}

/// Mirrors techy's
/// [`RestagedArgument`](flm_core::techy::transform::RestagedArgument): one
/// restaged argument of a callable, answered by a [`Restager`] method and
/// passed back to [`Restager::restage_invocation`] by value.
///
/// What differs: an owned handle that borrows nothing of the [`Restager`]
/// that answered it, valid for the handler call it was answered in; one
/// reading, [`is_provided`](RestagedArgument::is_provided). Left out: the
/// constructors `provided` and `absent`, which take an argument spec and an
/// ext, objects of `flm-core` an extension cannot make; the readers `spec`,
/// `nodes`, `content` and `ext`, which answer objects of `flm-core` that
/// have no façade.
///
/// The trait is sealed: the adapter alone implements it.
pub trait RestagedArgument: RestagedArgumentCore {
    /// Mirrors techy's
    /// [`RestagedArgument::is_provided`](flm_core::techy::transform::RestagedArgument::is_provided):
    /// whether the argument was provided (an emptied argument is provided;
    /// an absent one is not).
    fn is_provided(&self) -> bool;
}

/// Mirrors techy's [`RestagedSlot`](flm_core::techy::transform::RestagedSlot):
/// one restaged slot of a callable, answered by a [`Restager`] method and
/// passed back to [`Restager::restage_invocation`] by value.
///
/// What differs: an owned handle that borrows nothing of the [`Restager`]
/// that answered it, valid for the handler call it was answered in; one
/// reading, [`name`](RestagedSlot::name). Left out: the constructors `new`
/// and `new_unnamed`, which take a slot ext, an object of `flm-core` an
/// extension cannot make; the readers `role`, `nodes`, `content` and `ext`,
/// which answer objects of `flm-core` that have no façade.
///
/// The trait is sealed: the adapter alone implements it.
pub trait RestagedSlot: RestagedSlotCore {
    /// Mirrors techy's
    /// [`RestagedSlot::name`](flm_core::techy::transform::RestagedSlot::name):
    /// the slot's name, if it has one. What differs: owned text.
    fn name(&self) -> Option<String>;
}

fn _object_safe(_: &dyn Restager, _: &dyn RestagedArgument, _: &dyn RestagedSlot) {}
