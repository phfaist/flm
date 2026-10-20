//! The seal of the façade traits: a public supertrait in a private module.
//!
//! Every façade trait the adapter alone implements has one of the supertraits
//! below. Code outside this crate cannot name them, so it cannot implement a
//! façade trait: every `dyn Node`, `dyn Expander`, … a handler receives is the
//! adapter's. The supertraits that carry an accessor to the `flm-core` object
//! behind a façade take a [`Token`] argument. Code outside the crate can call a
//! supertrait's method on a trait object (Rust resolves it without the trait in
//! scope), but it cannot make a `Token`, so it cannot pass one: the accessors
//! are reachable from inside the crate alone.

use std::sync::Arc;

use flm_core::lang::rewrites::userdefs::{
    CompiledTemplate as CoreCompiledTemplate, ExpandCx as CoreExpandCx, Expander as CoreExpander,
    ModeVariant as CoreModeVariant, PlaceholderBindings as CorePlaceholderBindings,
    UserDefinition as CoreUserDefinition,
};
use flm_core::core::lowering::{LowerCx as CoreLowerCx, LowerError, Lowerer as CoreLowerer};
use flm_core::core::run::{RewriteCx as CoreRewriteCx, StageCx as CoreStageCx};
use flm_core::techy::core::node::{BuildId, NodeId, NodeTree};
use flm_core::lang::{BuiltLang as CoreBuiltLang, Flm};
use flm_core::techy::core::node::NodeRef;
use flm_core::techy::transform::{RestagedArgument as CoreRestagedArgument, RestagedSlot as CoreRestagedSlot};

use crate::api::core::lowering::{LowerCx, Staged};
use flm_core::techy::latexlike::Mode;

use crate::adapter::durable::{DurableTable, DurableTemplateFacade, DurableToken};
use crate::api::core::data::table::TableValue;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;

/// The proof that a caller is inside this crate: the argument of every
/// accessor. Only this crate can make one.
pub struct Token(pub(crate) ());

impl Token {
    /// A token, for a call to an accessor from inside the crate.
    pub(crate) fn new() -> Token {
        Token(())
    }
}

/// The seal of a façade trait that carries no accessor.
pub trait Sealed {}

/// The seal of [`Node`](crate::api::techy::node::Node), with its two
/// accessors (plan A, A16).
pub trait NodeCore<'t> {
    /// The `flm-core` node behind the façade, borrowing the façade: a façade
    /// over a tree it owns (an `Arc` or an `Rc`) can lend its node only for as
    /// long as it is itself borrowed, and every `flm-core` call that takes a
    /// node takes a `NodeRef<'_, Flm>`, so this borrow suffices for all of
    /// them.
    fn core_node_ref(&self, token: Token) -> NodeRef<'_, Flm>;

    /// An owned copy of the façade, for whoever must keep the node beyond one
    /// borrow: the dispatcher, holding a node in its table or passing the nodes
    /// it holds to a function that takes a list
    /// (`&[Box<dyn Node<'t> + 't>]`), and the façade of a use site's bindings.
    /// The adapter's node type is `Clone` (it holds a `Copy` `NodeRef`, an
    /// `Arc` or an `Rc`), so the copy is cheap.
    fn clone_box(&self, token: Token) -> Box<dyn Node<'t> + 't>;

    /// The façade of the node `id` of the same tree as this one, owning what
    /// this one owns: how the mirror of a library function answers the nodes
    /// the function found in the tree of a node it was given. `id` must be a
    /// node of that tree.
    fn relative(&self, token: Token, id: NodeId) -> Box<dyn Node<'t> + 't>;

    /// The tree of the node, when the façade shares it through an `Arc` (a
    /// frozen tree, a synthesized value): the mirror hands such a tree back to
    /// `flm-core` as the same value, never a copy.
    fn shared_tree(&self, token: Token) -> Option<Arc<NodeTree<Flm>>>;
}

/// The seal of [`StageCx`](crate::api::core::run::StageCx), with its
/// accessor.
pub trait StageCxCore: DurableOwner {
    /// Calls `f` with the `flm-core` context behind the façade.
    fn with_core(&self, token: Token, f: &mut dyn FnMut(&CoreStageCx<'_>));
}

/// The seal of [`Expander`](crate::api::lang::rewrites::userdefs::Expander),
/// with its accessor.
pub trait ExpanderCore: DurableOwner {
    /// Calls `f` with the `flm-core` expander behind the façade.
    fn with_core(&mut self, token: Token, f: &mut dyn FnMut(&mut dyn CoreExpander));
}

/// The seal of [`ExpandCx`](crate::api::lang::rewrites::userdefs::ExpandCx),
/// with its accessor.
pub trait ExpandCxCore {
    /// Calls `f` with the `flm-core` context behind the façade.
    fn with_core(&mut self, token: Token, f: &mut dyn FnMut(&mut CoreExpandCx<'_>));
}

/// The seal of
/// [`ModeVariant`](crate::api::lang::rewrites::userdefs::ModeVariant), with its
/// accessors.
pub trait ModeVariantCore {
    /// Calls `f` with the `flm-core` variant behind the façade; `f` is not
    /// called when the definition has no compiled variant for the mode.
    fn with_core(&self, token: Token, f: &mut dyn FnMut(&CoreModeVariant));

    /// An owned copy of what the façade holds, the definition's `Arc` and the
    /// mode: what the façade of a use site's bindings keeps from the variant
    /// it is built for (plan A, A23).
    fn definition_and_mode(&self, token: Token) -> (Arc<CoreUserDefinition>, Mode);
}

/// The seal of
/// [`CompiledTemplate`](crate::api::lang::rewrites::userdefs::CompiledTemplate),
/// with its accessor.
pub trait CompiledTemplateCore {
    /// Calls `f` with the `flm-core` template behind the façade.
    fn with_core(&self, token: Token, f: &mut dyn FnMut(&CoreCompiledTemplate));
}

/// The seal of
/// [`PlaceholderBindings`](crate::api::lang::rewrites::userdefs::PlaceholderBindings),
/// with its accessor.
pub trait PlaceholderBindingsCore {
    /// Calls `f` with the `flm-core` bindings behind the façade.
    fn with_core(&self, token: Token, f: &mut dyn FnMut(&dyn CorePlaceholderBindings));
}

/// The seal of [`LowerCx`](crate::api::core::lowering::LowerCx), with its
/// accessor.
pub trait LowerCxCore {
    /// Calls `f` with the `flm-core` context behind the façade.
    fn with_core(&mut self, token: Token, f: &mut dyn FnMut(&mut CoreLowerCx<'_>));
}

/// The seal of [`Lowerer`](crate::api::core::lowering::Lowerer), with the
/// read the dispatcher makes in place of
/// [`Lowerer::get`](crate::api::core::lowering::Lowerer::get).
pub trait LowererCore: DurableOwner {
    /// The table `name`, as [`Lowerer::get`](crate::api::core::lowering::Lowerer::get)
    /// answers it, but owning what it holds (an extension's own table as a
    /// copy of its items, a rewrite's trees as their shared roots), so that the
    /// dispatcher can lend it into the call's table beside the lowerer it was
    /// asked on.
    ///
    /// # Errors
    ///
    /// As [`Lowerer::get`](crate::api::core::lowering::Lowerer::get).
    fn get_owned<'s>(&self, token: Token, name: &str) -> Result<Option<TableValue<'s>>, ExtError>;

    /// Calls `f` with the `flm-core` lowerer behind the façade, for a read.
    fn with_core(&self, token: Token, f: &mut dyn FnMut(&dyn CoreLowerer));

    /// Stages through `f`, a `flm-core` function given the context behind
    /// `cx` (which must be the one lent to the call) and the lowerer, and
    /// answers what it staged as the call's own staged nodes: how the mirror
    /// of a library part stages. `method` names the function in the errors.
    ///
    /// # Errors
    ///
    /// As the lowerer's staging methods: `cx` is not the call's context, or
    /// `f` fails (the call then ends with that error).
    fn stage_core(
        &mut self,
        token: Token,
        method: &str,
        cx: &mut dyn LowerCx,
        f: &mut CorePart<'_>,
    ) -> Result<Vec<Staged>, ExtError>;

    /// [`stage_core`](LowererCore::stage_core) for a function that places
    /// `content`, nodes the call staged, inside what it stages: each node of
    /// `content` must have been staged by this call and not yet placed, and
    /// is recorded as placed once `f` succeeds, as the lowerer's own methods
    /// that take content check and record it.
    ///
    /// # Errors
    ///
    /// As [`stage_core`](LowererCore::stage_core); a node of `content` was
    /// not staged by this call, or is already placed ([`ExtError::Argument`]).
    fn stage_core_placing(
        &mut self,
        token: Token,
        method: &str,
        cx: &mut dyn LowerCx,
        content: &[Staged],
        f: &mut CorePart<'_>,
    ) -> Result<Vec<Staged>, ExtError>;
}

/// A `flm-core` function that stages, given the context and the lowerer, as
/// [`LowererCore::stage_core`] calls it.
pub type CorePart<'f> =
    dyn FnMut(&mut CoreLowerCx<'_>, &mut dyn CoreLowerer) -> Result<Vec<BuildId>, LowerError> + 'f;

/// The seal of [`RewriteCx`](crate::api::core::run::RewriteCx), with its
/// accessor.
pub trait RewriteCxCore {
    /// Calls `f` with the `flm-core` context behind the façade.
    fn with_core(&self, token: Token, f: &mut dyn FnMut(&CoreRewriteCx<'_>));
}

/// The seal of [`Restager`](crate::api::techy::transform::Restager). It
/// carries no accessor yet: every operation of the façade is a method of the
/// façade itself; an accessor to techy's restage context behind it comes
/// with the first library function that restages through it.
pub trait RestagerCore {}

/// The seal of
/// [`RestagedArgument`](crate::api::techy::transform::RestagedArgument),
/// with its accessor.
pub trait RestagedArgumentCore {
    /// The techy bundle behind the façade, taken out of it: what
    /// [`Restager::restage_invocation`](crate::api::techy::transform::Restager::restage_invocation)
    /// hands to techy's. `None` once taken: the dispatcher takes the bundle
    /// out of the call's table through this, leaving the entry spent, so
    /// that a handle given twice is refused.
    fn take_core(&mut self, token: Token) -> Option<CoreRestagedArgument<Flm>>;
}

/// The seal of [`RestagedSlot`](crate::api::techy::transform::RestagedSlot),
/// with its accessor.
pub trait RestagedSlotCore {
    /// The techy bundle behind the façade, taken out of it: what
    /// [`Restager::restage_invocation`](crate::api::techy::transform::Restager::restage_invocation)
    /// hands to techy's. `None` once taken, as for
    /// [`RestagedArgumentCore::take_core`].
    fn take_core(&mut self, token: Token) -> Option<CoreRestagedSlot<Flm>>;
}

/// The seal of
/// [`CitationEntry`](crate::api::stdcore::features::citations::CitationEntry),
/// with its accessor.
pub trait CitationEntryCore {
    /// The `flm-core` citation entry behind the façade, owned (a clone of its
    /// `Arc`): what the adapter hands to `flm-core` in a provider's answer,
    /// and what the dispatcher copies out of the call's table.
    fn core_entry(&self, token: Token) -> flm_core::stdcore::features::citations::CitationEntry;
}

/// No `flm-core` counterpart: the façades that lend a call the durable table
/// of the running handler's feature ([`DurableTable`]): the builder, a stage's
/// context, the expander, the lowerer, and the built language and the
/// diagnostics lent to a late-initialization hook (the language a rule or a
/// hook answers carries the table too). The table is shared (`Arc`), not
/// borrowed: a durable handle is the one handle that outlives its call.
///
/// The trait is re-exported by the extension API (as
/// `flm_ext::api::lang::rewrites::userdefs::DurableOwner`) so that its
/// method [`durable_template`](DurableOwner::durable_template) shows on every
/// façade that offers it; it stays sealed through its supertrait, so only the
/// adapter implements it.
pub trait DurableOwner: Sealed {
    /// The durable table of the feature whose handler the call runs.
    fn durable_table(&self, token: Token) -> Arc<DurableTable>;

    /// No `flm-core` counterpart: the compiled template the durable token
    /// `token` names, as [`compile_template_with`](crate::api::lang::rewrites::userdefs::compile_template_with)
    /// answered it, resolved against the table of the running handler's
    /// feature: how a Rust handler that kept a token (in a `static`, say)
    /// finds its template again in a later call.
    ///
    /// # Errors
    ///
    /// [`ExtError::StaleDurable`]: the token names no compiled template of
    /// that table (it was made under another registry, by another feature,
    /// or is of another kind).
    fn durable_template(&self, token: DurableToken) -> Result<DurableTemplateFacade, ExtError> {
        self.durable_table(Token::new()).compiled_template(token)
    }
}

/// The seal of [`BuiltLang`](crate::api::lang::BuiltLang), with its accessor.
pub trait BuiltLangCore: DurableOwner {
    /// The `flm-core` language behind the façade.
    fn core_lang(&self, token: Token) -> &CoreBuiltLang;
}
