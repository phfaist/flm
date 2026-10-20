//! The [`Expand`] aspect, the [`Expander`] interface through which an `Expand`
//! implementation works, and the [`PlaceholderBindings`] that say what each
//! placeholder of a template is replaced by at one use site.
//!
//! An element whose use sites the [expand pass](super::expand) rewrites carries
//! the [`Expand`] aspect (an
//! [aspect](crate::guide::concepts_overview#elements-and-aspects) is a piece of
//! behavior attached to an element and read back through a trait object). The
//! pass visits every node of the tree, from the root down. At a node whose
//! element carries the aspect, and whose expansion policy ([`ExpandPolicy`]) is
//! among the policies this run of the pass expands ([`ExpandSet`]), the pass
//! calls [`Expand::expand`]. The implementation works through the [`Expander`]
//! it is handed: it instantiates a compiled template at the use site, or it
//! replaces the use site by its literal source text and reports a problem. It
//! then returns either the nodes that replace the use site
//! ([`Expansion::Emit`]), which the pass inserts in place of the use site
//! without a wrapper node, or the decision to leave the node as it is
//! ([`Expansion::Default`]).
//!
//! The standard implementation of the aspect is
//! [`UserDefinition`](super::UserDefinition), which instantiates the
//! definition's template with the placeholder bindings of the use site
//! ([`DefinitionBindings`](super::DefinitionBindings)). An extension crate may
//! write its own implementation over the same interface, for example to compute
//! the content of placeholder names of its own, with placeholder bindings that
//! pass every other placeholder to `DefinitionBindings`. A template that refers
//! to no argument of a use site (a value a feature compiled and fills by name)
//! is instantiated with [`ValueBindings`](super::ValueBindings), which give each
//! extra placeholder a finished tree, through the public
//! [`instantiate`](fn@super::instantiate), from an expansion hook or from a
//! lowering rule.

use std::fmt;

use techy::core::node::{BuildId, NodeBuildError, NodeRef, NodeSlice, NodeTree};
use techy::transform::{RestageContext, RestageError};

use crate::lang::flm::Flm;
use crate::lang::BuiltLang;
use crate::lang::origin::FlmDiagnostic;
use crate::lang::rewrites::userdefs::template::{CompiledTemplate, PlaceholderTarget};

/// The context through which the expand pass, and an [`Expand`]
/// implementation, build nodes into the expanded tree (to *stage* them, in
/// techy's terms; see [`Lowerer`](crate::core::lowering::Lowerer)): techy's
/// restage context over the FLM language. The two unit type parameters say that
/// FLM trees carry no annotations.
pub type ExpandCx<'t> = RestageContext<'t, Flm, (), ()>;

/// When a definition's use sites are expanded: `always`, in every plan, or
/// `per-plan`, as each plan decides.
///
/// The `flm.userdefs.expand-always` stage expands the `always` definitions in
/// every plan. The `flm.userdefs.expand` stage expands the `per-plan`
/// definitions in a plan that asks for expanded definitions
/// (`flm.userdefs.expanded`, the default choice); in a plan that asks for kept
/// definitions (`flm.userdefs.kept`), their use sites stay in the tree, and a
/// lowering rule either writes each one to the output as a call of the
/// definition or writes the instantiated template in its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExpandPolicy {
    /// Expanded in every plan.
    Always,
    /// Expanded or kept, as each plan decides: expanded unless the plan asks
    /// for kept definitions. This is the default.
    #[default]
    PerPlan,
}

/// Which definitions one run of the [expand pass](super::expand) expands, by
/// their expansion policy.
///
/// The two rewrite stages of the
/// [`userdefs` feature](crate::stdcore::features::userdefs) differ in this
/// alone: `flm.userdefs.expand-always` runs the pass with
/// [`Always`](ExpandSet::Always), and `flm.userdefs.expand` with
/// [`PerPlan`](ExpandSet::PerPlan). A caller outside these stages, such as a
/// lowering rule that expands one kept use site, uses [`Both`](ExpandSet::Both).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpandSet {
    /// The definitions whose policy is [`ExpandPolicy::Always`].
    Always,
    /// The definitions whose policy is [`ExpandPolicy::PerPlan`].
    PerPlan,
    /// Every definition.
    Both,
}

impl ExpandSet {
    /// Whether this set expands a definition whose expansion policy is `policy`.
    pub fn includes(self, policy: ExpandPolicy) -> bool {
        match self {
            ExpandSet::Always => policy == ExpandPolicy::Always,
            ExpandSet::PerPlan => policy == ExpandPolicy::PerPlan,
            ExpandSet::Both => true,
        }
    }
}

/// What an [`Expand`] implementation decided about one use site.
#[derive(Debug)]
pub enum Expansion {
    /// Leave the node as it is: it is copied into the expanded tree, and the pass
    /// continues into its children.
    Default,
    /// These nodes, which the implementation built through the [`ExpandCx`],
    /// replace the use site: the pass inserts them, in order, in place of the use
    /// site. An empty list removes the use site.
    Emit(Vec<BuildId>),
}

/// A failure of an [`Expand`] implementation, or of the [`Expander`] it works
/// through: the error type of the expand pass's traversal. The boxed
/// [`Restage`](ExpandError::Restage) variant follows techy's pattern for errors
/// of a recursive traversal.
///
/// Every variant is an implementation failure, never a problem in the document:
/// a problem in the document is reported as a diagnostic, and the use site is
/// replaced by its literal source text ([`Expander::recover`]). An
/// `ExpandError` that reaches the pass ends it with no tree, reported as
/// `flm.userdefs.expansion-failed`, or as `flm.blocks.nesting-too-deep` for a
/// refusal of the recursion guard ([`ExpandFailure`](super::ExpandFailure)).
#[derive(Debug)]
pub enum ExpandError {
    /// An operation of the restage context failed: a region of a node could not be
    /// attached to its new nodes, the [recursion guard](crate::lang::guard)
    /// refused, or a node could not be built.
    Restage(Box<RestageError<ExpandError>>),
    /// Building a node directly through the context's node builder failed.
    Build(NodeBuildError),
    /// A placeholder that the placeholder bindings could not give a value. The
    /// substitution pass returns it, and [`Expander::instantiate`] turns it into the
    /// use site's recovery; an implementation receives this error only if it
    /// instantiates a template by other means.
    BadPlaceholder {
        /// The placeholder as written.
        reference: String,
    },
    /// A failure of the implementation's own code (an operational error in
    /// extension code).
    Hook {
        /// The failure, as text.
        detail: String,
    },
}

impl ExpandError {
    /// The detail of a refusal of the [recursion guard](crate::lang::guard), when
    /// this error, or a restage error it wraps at any depth, is one.
    pub fn descent_refusal(&self) -> Option<&str> {
        match self {
            ExpandError::Restage(error) => match &**error {
                RestageError::DescentLimitExceeded { detail } => Some(detail),
                RestageError::Visitor(inner) => inner.descent_refusal(),
                _ => None,
            },
            _ => None,
        }
    }

    /// The placeholder that could not be given a value, when this error is a
    /// [`BadPlaceholder`](ExpandError::BadPlaceholder) or wraps one, at any depth,
    /// inside a restage error. A placeholder inside a conditional's branch or inside
    /// a default value reaches the pass through the restage driver's own wrapping.
    pub fn bad_placeholder(&self) -> Option<&str> {
        match self {
            ExpandError::BadPlaceholder { reference } => Some(reference),
            ExpandError::Restage(error) => match &**error {
                RestageError::Visitor(inner) => inner.bad_placeholder(),
                _ => None,
            },
            _ => None,
        }
    }
}

impl From<RestageError<ExpandError>> for ExpandError {
    fn from(error: RestageError<ExpandError>) -> ExpandError {
        ExpandError::Restage(Box::new(error))
    }
}

impl From<NodeBuildError> for ExpandError {
    fn from(error: NodeBuildError) -> ExpandError {
        ExpandError::Build(error)
    }
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpandError::Restage(error) => write!(f, "{error}"),
            ExpandError::Build(error) => write!(f, "building the expanded nodes failed: {error}"),
            ExpandError::BadPlaceholder { reference } => {
                write!(f, "placeholder ‘#{reference}’ has no value")
            }
            ExpandError::Hook { detail } => {
                write!(f, "the expansion code of a definition failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ExpandError {}

/// The expansion aspect: the behavior of an element whose use sites are
/// rewritten during the [expand pass](super::expand). It is attached by its trait-object type, like
/// every aspect: `.with::<dyn Expand>(Arc::new(definition))`.
///
/// [`UserDefinition`](super::UserDefinition) is the standard implementation.
pub trait Expand: Send + Sync {
    /// When this element's use sites are expanded ([`ExpandPolicy`]): per plan,
    /// unless the implementation says otherwise.
    fn policy(&self) -> ExpandPolicy {
        ExpandPolicy::PerPlan
    }

    /// Expands one use site. The pass calls this, from the root of the tree down,
    /// at every node of this element whose policy is among the policies the run
    /// expands ([`ExpandSet`]). `node` is the use site in the input tree; `cx` is
    /// the context through which the replacement nodes are built; `xp` is the
    /// [`Expander`] through which the implementation instantiates a compiled
    /// template or reports a problem.
    ///
    /// # Errors
    ///
    /// An [`ExpandError`] ends the pass and leaves no tree, so it is for an
    /// implementation failure only ([`ExpandError::Hook`] for the implementation's
    /// own). A problem in the *document* is reported by replacing the use site
    /// through [`Expander::recover`], which returns normally.
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError>;
}

/// The operations an [`Expand`] implementation uses at a use site: instantiate a
/// compiled template, replace the use site by its literal source text with a
/// diagnostic, or record a diagnostic. The expand pass implements this trait,
/// and an implementation never sees the pass itself.
pub trait Expander {
    /// The current expansion depth: 0 at the top level of the tree, one more inside
    /// each instantiated template.
    fn depth(&self) -> usize;

    /// The expansion depth limit that applies
    /// ([`ExpandOptions::recursion_limit`](super::ExpandOptions::recursion_limit)).
    fn recursion_limit(&self) -> usize;

    /// Instantiates `template` at the use site `site`, and returns the nodes that
    /// replace the use site.
    ///
    /// Each placeholder is replaced by the value that `bindings` gives it (already
    /// parsed nodes, never parsed again), the conditionals are evaluated, and the
    /// template's nodes are copied onto a new source created for this use: a
    /// source with the template's text that records the use site's position as
    /// the place where it was created; each copied node keeps its byte range in
    /// that text. The result then goes
    /// through the pass one level deeper, so that definitions used in the template
    /// or in the substituted arguments are expanded in turn.
    ///
    /// The expansion depth limit is checked here, before any work: at the limit,
    /// the use site is replaced by its literal source text and reported with
    /// `flm.userdefs.recursion-limit`. A placeholder that `bindings` cannot give a
    /// value, wherever it is (at the template's top level, inside a conditional's
    /// branch, inside a default value), is a problem in the document: the use site
    /// is replaced by its literal source text and reported with
    /// `flm.userdefs.bad-placeholder`, and the replacement node is returned.
    ///
    /// # Errors
    ///
    /// An [`ExpandError`] for an implementation failure only: a broken invariant
    /// while building nodes, a refusal of the recursion guard, or a failure of an
    /// expansion hook reached inside the instantiated template. Every problem in the
    /// document is handled by replacing the use site instead.
    fn instantiate(
        &mut self,
        cx: &mut ExpandCx<'_>,
        site: NodeRef<'_, Flm>,
        template: &CompiledTemplate,
        bindings: &dyn PlaceholderBindings,
    ) -> Result<Vec<BuildId>, ExpandError>;

    /// Reports a problem in the document at the use site `site`: records
    /// `diagnostic` and builds a text node holding the use site's literal source
    /// text, spanned into the use site's own source so that its origin is unchanged.
    /// Returns that node, as the replacement of the use site.
    ///
    /// # Errors
    ///
    /// [`ExpandError::Build`] when building the text node fails, which is an
    /// implementation failure and ends the pass.
    fn recover(
        &mut self,
        cx: &mut ExpandCx<'_>,
        site: NodeRef<'_, Flm>,
        diagnostic: FlmDiagnostic,
    ) -> Result<Vec<BuildId>, ExpandError>;

    /// Records `diagnostic` without replacing anything: what an implementation that
    /// keeps its node ([`Expansion::Default`]) uses to report a condition there.
    fn diagnose(&mut self, diagnostic: FlmDiagnostic);

    /// The built language the expansion runs under: what a hook compiles a
    /// template it computed at the use site against
    /// ([`compile_template_with`](super::compile_template_with)).
    fn lang(&self) -> &BuiltLang;
}

/// The value of a placeholder at one use site, as
/// [`PlaceholderBindings::value`] returns it.
#[derive(Debug, Clone, Copy)]
pub enum PlaceholderValue<'a> {
    /// The placeholder refers to an argument that was not written and has no
    /// default value (the `-NoValue-` of LaTeX's `xparse` package): nothing is
    /// substituted, and `\IfNoValue…` holds.
    NoValue,
    /// Content written at the use site (an argument's or the body's content
    /// nodes), or any other nodes the bindings can lend for the lifetime `'a`,
    /// such as the nodes of a tree an expansion hook built. They are substituted as they are,
    /// with their own positions in their sources.
    Provided(NodeSlice<'a, Flm>),
    /// The argument was not written, and its declared default value applies: a
    /// compiled template, with placeholders of its own, instantiated in place.
    Default(&'a CompiledTemplate),
    /// Content an expansion hook computed, as **finished content**: the children of
    /// the tree's root are substituted. They go through the substitution pass, so
    /// that no placeholder node remains in the expanded tree. A placeholder node
    /// among them belongs to no compiled template the pass entered, so the use site
    /// is replaced by its literal source text and reported with
    /// `flm.userdefs.bad-placeholder`; a conditional among them is evaluated. A hook
    /// that wants placeholders given values returns a compiled template through
    /// [`Default`](PlaceholderValue::Default) instead. The tree's nodes keep their
    /// own positions in their sources (they are not the template's).
    Tree(&'a NodeTree<Flm>),
    /// The placeholder's own substitution argument (the bracketed content of
    /// `#[…]{name}`, as parsed in the template) is the value: its nodes are the
    /// template's, copied like content written at the use site, so they carry this
    /// use's own source in the expanded tree. Written without a substitution
    /// argument, the placeholder substitutes nothing.
    ///
    /// [`PlaceholderBindings::value`] receives these nodes for reading only (they
    /// exist for the duration of the call, not as long as the returned value), which
    /// is why returning them takes this variant rather than
    /// [`Provided`](PlaceholderValue::Provided).
    Substitution,
    /// The placeholder has no value here (an expansion hook does not compute it).
    /// The use site is replaced by its literal source text and reported with
    /// `flm.userdefs.bad-placeholder`.
    Unresolvable,
}

/// What the placeholders of a template are replaced by at one use site.
///
/// The template compiler resolves each placeholder to its target
/// ([`PlaceholderTarget`]) in a fixed order: a name an expansion hook computes
/// first, then the body, then a declared argument name, then an argument number
/// ([`PlaceholderVocabulary::resolve`](super::PlaceholderVocabulary::resolve)).
/// At each use, the bindings give each target its value.
/// [`DefinitionBindings`](super::DefinitionBindings) are the bindings of a user
/// definition's use site; an extension's [`Expand`] implementation supplies its
/// own, usually built on top of them. [`ValueBindings`](super::ValueBindings)
/// are the bindings of a template that has no use site's arguments: they give
/// each extra placeholder a finished tree held by name, for a feature that
/// instantiates a template it compiled itself
/// ([`instantiate`](fn@super::instantiate)).
pub trait PlaceholderBindings {
    /// The value of `target` at this use site. `substitution` is the placeholder's
    /// substitution argument (`#[…]{…}`) as parsed in the template, when one is
    /// written: the data that a computed placeholder name reads. The arguments and
    /// the body take none (the template compiler refuses one on them).
    ///
    /// **The substitution nodes** are nodes of the compiled template being
    /// instantiated, before they are copied onto the use's own source, and they are
    /// lent for this call alone: they do not exist as long as the returned value, so
    /// they cannot be returned as [`Provided`](PlaceholderValue::Provided). An
    /// implementation that wants them substituted returns
    /// [`Substitution`](PlaceholderValue::Substitution); the pass copies them, and
    /// they carry this use's own source in the expanded tree.
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_per_plan_set_excludes_always_definitions() {
        assert!(!ExpandSet::PerPlan.includes(ExpandPolicy::Always));
        assert!(ExpandSet::PerPlan.includes(ExpandPolicy::PerPlan));
    }

    #[test]
    fn the_both_set_includes_every_policy() {
        assert!(ExpandSet::Both.includes(ExpandPolicy::Always));
        assert!(ExpandSet::Both.includes(ExpandPolicy::PerPlan));
    }

    #[test]
    fn the_default_policy_is_per_plan() {
        assert_eq!(ExpandPolicy::default(), ExpandPolicy::PerPlan);
    }
}
