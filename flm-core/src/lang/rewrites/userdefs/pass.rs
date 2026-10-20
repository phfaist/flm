//! The expand pass: [`expand`], a plain function over a tree that replaces every
//! use site of a user definition by the definition's template, instantiated with
//! the arguments written at the use site.
//!
//! [`expand`] runs one traversal over a tree, from the root down, and builds a
//! new tree. At every node whose element carries the [`Expand`] aspect and whose
//! [expansion policy](super::ExpandPolicy) is in the run's [`ExpandSet`], it
//! calls the aspect; every other node is copied. The traversal also implements
//! the [`Expander`] interface that the aspect works through: it keeps track of
//! the expansion depth, instantiates compiled templates (the substitution and
//! re-sourcing passes of [`super::instantiate`](mod@super::instantiate), after which the result goes
//! through this pass again, one level deeper), and replaces a use site that has
//! a problem by its literal source text, with a diagnostic.
//!
//! The [`userdefs` feature](crate::stdcore::features::userdefs) runs the pass as
//! two rewrite stages, `flm.userdefs.expand-always` and `flm.userdefs.expand`.
//! A caller may also run it on any other tree, such as the tree of a short text
//! parsed on its own.
//!
//! # The expansion depth limit, the recursion guard and the worker thread
//!
//! Every tree traversal of the pass runs under the
//! [recursion guard](crate::lang::guard) policy that the caller passes in
//! [`ExpandOptions`], normally the built language's. Content nested too deeply
//! therefore becomes the condition `flm.blocks.nesting-too-deep` and no tree,
//! never a stack overflow and never a panic. The guard compares the stack a run
//! has used with a fixed budget, which protects only a thread with enough stack,
//! so [`expand`] runs itself on a
//! [worker thread](crate::lang::guard::on_worker_thread) when the calling thread
//! is not one.
//!
//! Separately from the guard, the pass limits how many expansions one chain may
//! nest inside one another: [`DEFAULT_RECURSION_LIMIT`] (32), unless the caller
//! sets another limit. A use site written inside an argument is expanded inside
//! the instance that substitutes the argument, so it counts one level deeper.
//! The limit is checked in [`Expander::instantiate`], so every `Expand`
//! implementation, including one from another crate, is limited without checking
//! anything itself. Reaching the limit replaces the use site by its literal
//! source text and reports `flm.userdefs.recursion-limit`; it does not end the
//! pass.
//!
//! # Failures that end the pass
//!
//! A failure that no recovery covers (an internal error while building the
//! expanded tree, or a failing expansion hook) ends the pass with no tree and
//! the condition `flm.userdefs.expansion-failed`, reported at the tree's root
//! ([`ExpandFailure`]). Every failure of the substitution pass other than a
//! placeholder without a value is such a failure.

use std::fmt;
use std::sync::Arc;

use techy::core::node::{BuildId, NodeKind, NodeRef, NodeTree};
use techy::core::Lang;
use techy::error::Diagnostic;
use techy::source::TextContent;
use techy::transform::{Restage, RestageError, RestageVisitor};

use crate::model::element::element_of;
use crate::lang::flm::Flm;
use crate::lang::BuiltLang;
use crate::lang::guard::{self, GuardPolicy, NestingTooDeep};
use crate::lang::origin::{FlmDiagnostic, FlmDiagnostics, FlmSpan};
use crate::lang::parse::WorkerThreadUnavailable;
use crate::lang::rewrites::userdefs::diagnostics::{
    BadPlaceholder, ExpansionFailed, PlaceholderProblem, RecursionLimit,
};
use crate::lang::rewrites::userdefs::hook::{
    Expand, ExpandCx, ExpandError, ExpandSet, Expander, Expansion, PlaceholderBindings,
};
use crate::lang::rewrites::userdefs::instantiate::{instance_nodes, instantiate};
use crate::lang::rewrites::userdefs::template::CompiledTemplate;

/// The default expansion depth limit: how many expansions one chain may nest
/// inside one another before a use site is replaced by its literal source text
/// and reported with `flm.userdefs.recursion-limit`. The configuration of the
/// `userdefs` feature
/// ([`UserdefsConfig`](crate::stdcore::features::userdefs::UserdefsConfig)) uses
/// it unless it sets another limit.
pub const DEFAULT_RECURSION_LIMIT: usize = 32;

/// The options of one run of the pass: the expansion depth limit and the
/// recursion guard policy. The default is [`DEFAULT_RECURSION_LIMIT`] and
/// [`GuardPolicy::StackBudget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandOptions {
    /// How many expansions one chain may nest inside one another before a use site
    /// is replaced by its literal source text and reported with
    /// `flm.userdefs.recursion-limit`.
    pub recursion_limit: usize,
    /// The [recursion guard](crate::lang::guard) policy that every tree traversal of
    /// the pass runs under: normally the built language's
    /// ([`crate::lang::BuiltLang::guard`]).
    pub guard: GuardPolicy,
}

impl Default for ExpandOptions {
    fn default() -> ExpandOptions {
        ExpandOptions { recursion_limit: DEFAULT_RECURSION_LIMIT, guard: GuardPolicy::StackBudget }
    }
}

/// A run of the pass that returned no tree. Each variant carries the one Error
/// diagnostic that reports the missing tree, which a rewrite stage records in
/// place of the tree.
#[derive(Debug)]
pub enum ExpandFailure {
    /// The recursion guard refused: the content nests too deeply
    /// (`flm.blocks.nesting-too-deep`).
    NestingTooDeep(FlmDiagnostic),
    /// The pass failed for a reason that no recovery covers
    /// (`flm.userdefs.expansion-failed`).
    Failed(FlmDiagnostic),
    /// The pass did not run: the calling thread is not a worker thread, and the
    /// operating system refused to create one (`flm.lang.worker-thread-unavailable`).
    WorkerThread(FlmDiagnostic),
}

impl ExpandFailure {
    /// The diagnostic that reports the missing tree.
    pub fn diagnostic(&self) -> &FlmDiagnostic {
        match self {
            ExpandFailure::NestingTooDeep(diagnostic)
            | ExpandFailure::Failed(diagnostic)
            | ExpandFailure::WorkerThread(diagnostic) => diagnostic,
        }
    }
}

impl fmt::Display for ExpandFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.diagnostic().message())
    }
}

impl std::error::Error for ExpandFailure {}

/// Expands, throughout `tree`, the use sites of every definition whose expansion
/// policy `set` includes, under the depth limit and recursion guard policy of
/// `options`, and returns the expanded tree. `lang` is the built language the
/// tree was parsed under, which the expansion hooks reach through
/// [`Expander::lang`].
///
/// The expand pass replaces every use site of a user definition by its template
/// instantiated with the use site's arguments (substitute, re-source, then
/// expand one level deeper), as a plain function that the
/// [`userdefs` feature](crate::stdcore::features::userdefs) wraps in two
/// rewrite stages.
///
/// It can be called on a document's tree and on any other tree alike. A use site
/// that has a problem (a definition that was refused, a use in a parsing mode
/// where its template did not parse, a placeholder without a value, the
/// expansion depth limit) is replaced by its literal source text, and its diagnostic is pushed onto
/// `diags`; the expanded tree is still returned. The pass runs on a
/// [worker thread](crate::lang::guard::on_worker_thread): on the calling thread
/// when it is one, otherwise on a thread created for the run.
///
/// # Errors
///
/// [`ExpandFailure`] means there is no expanded tree at all:
/// [`NestingTooDeep`](ExpandFailure::NestingTooDeep) when the content nests
/// deeper than the recursion guard allows, [`Failed`](ExpandFailure::Failed)
/// when an internal error occurred while building the tree or an expansion hook
/// failed, and [`WorkerThread`](ExpandFailure::WorkerThread) when no worker
/// thread could be created. Each variant carries the one Error diagnostic
/// ([`ExpandFailure::diagnostic`]) that a rewrite stage records in place of the
/// tree.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::lang::rewrites::userdefs::{expand, ExpandOptions, ExpandSet};
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmDiagnostics, FlmSource, LangInputs, ParseOptions,
/// };
///
/// // A language with no user definition declared: the pass finds no use site
/// // and hands back a tree equal to the one it was given.
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("plain text, nothing to expand"));
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::default(), &ParseOptions::default(),
/// ).expect("the input is shallow enough to parse");
///
/// let mut diags = FlmDiagnostics::new();
/// let expanded = expand(
///     fragment.tree(), &lang, &ExpandOptions::default(), ExpandSet::Both, &mut diags,
/// ).expect("nothing here can fail the pass");
/// assert_eq!(expanded.node_count(), fragment.tree().node_count());
/// assert!(diags.is_empty());
/// ```
pub fn expand(
    tree: &NodeTree<Flm>,
    lang: &BuiltLang,
    options: &ExpandOptions,
    set: ExpandSet,
    diags: &mut FlmDiagnostics,
) -> Result<Arc<NodeTree<Flm>>, ExpandFailure> {
    let run = || run_pass(tree, lang, options, set, diags);
    match guard::on_worker_thread(run) {
        Ok(result) => result,
        Err(error) => Err(ExpandFailure::WorkerThread(Diagnostic::error(
            WorkerThreadUnavailable::new(error.to_string()),
            tree.root().span().clone(),
        ))),
    }
}

/// The pass, on a thread that satisfies the worker-thread requirement.
fn run_pass(
    tree: &NodeTree<Flm>,
    lang: &BuiltLang,
    options: &ExpandOptions,
    set: ExpandSet,
    diags: &mut FlmDiagnostics,
) -> Result<Arc<NodeTree<Flm>>, ExpandFailure> {
    let mut pass = ExpandPass::new(lang, options, set, diags);
    let result = guard::restager(&mut pass, options.guard).restage(tree);
    match result {
        Ok(expanded) => Ok(Arc::new(expanded)),
        Err(error) => Err(failure(error, tree.root().span().clone())),
    }
}

/// The failure of the pass for a restage error: a refusal of the recursion
/// guard, at any depth of the error, is the nesting condition; anything else is
/// the pass's own failure.
fn failure(error: RestageError<ExpandError>, at: FlmSpan) -> ExpandFailure {
    let refusal = match &error {
        RestageError::DescentLimitExceeded { detail } => Some(detail.clone()),
        RestageError::Visitor(inner) => inner.descent_refusal().map(String::from),
        _ => None,
    };
    match refusal {
        Some(detail) => {
            ExpandFailure::NestingTooDeep(Diagnostic::error(NestingTooDeep::new(detail), at))
        }
        None => {
            ExpandFailure::Failed(Diagnostic::error(ExpansionFailed::new(error.to_string()), at))
        }
    }
}

/// The traversal of the pass, and the [`Expander`] that the `Expand`
/// implementations work through.
struct ExpandPass<'d> {
    lang: &'d BuiltLang,
    diags: &'d mut FlmDiagnostics,
    recursion_limit: usize,
    guard: GuardPolicy,
    set: ExpandSet,
    depth: usize,
}

impl<'d> ExpandPass<'d> {
    fn new(
        lang: &'d BuiltLang,
        options: &ExpandOptions,
        set: ExpandSet,
        diags: &'d mut FlmDiagnostics,
    ) -> ExpandPass<'d> {
        ExpandPass {
            lang,
            diags,
            recursion_limit: options.recursion_limit,
            guard: options.guard,
            set,
            depth: 0,
        }
    }
}

impl RestageVisitor<Flm, (), ()> for ExpandPass<'_> {
    type Error = ExpandError;

    /// One aspect lookup per node: the pass asks every node's element for the
    /// [`Expand`] aspect, and consults no table of names and no type of
    /// specification.
    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
    ) -> Result<Restage<()>, ExpandError> {
        if let Some(hook) = element_of(&node).aspect::<dyn Expand>() {
            if self.set.includes(hook.policy()) {
                return Ok(match hook.expand(node, cx, self)? {
                    Expansion::Default => Restage::Descend(()),
                    Expansion::Emit(ids) => Restage::Emit(ids),
                });
            }
        }
        Ok(Restage::Descend(()))
    }
}

impl Expander for ExpandPass<'_> {
    fn depth(&self) -> usize {
        self.depth
    }

    fn recursion_limit(&self) -> usize {
        self.recursion_limit
    }

    // Each instantiation runs two nested restage drivers, and each driver takes a
    // fresh stack anchor under the full budget, so at the expansion depth limit the
    // cumulative stack use of the nested drivers is not bounded by one budget. The
    // worker stack is generous and template trees are shallow, so this is safe as
    // things stand; a restager that continues under the caller's remaining budget
    // would remove the caveat. See the design rationale at
    // [§flm-dd-dr:lang-nested-restage-budget].
    fn instantiate(
        &mut self,
        cx: &mut ExpandCx<'_>,
        site: NodeRef<'_, Flm>,
        template: &CompiledTemplate,
        bindings: &dyn PlaceholderBindings,
    ) -> Result<Vec<BuildId>, ExpandError> {
        // The depth bound, enforced here so that every hook is bounded alike.
        if self.depth >= self.recursion_limit {
            let condition = RecursionLimit::new(template.definition(), self.recursion_limit);
            let diagnostic = Diagnostic::error(condition, site.span().clone());
            return self.recover(cx, site, diagnostic);
        }
        // Substitute, then re-source: the use site's parsed nodes at the
        // placeholders, the conditionals' branches selected, and the compiled
        // trees' nodes onto this use's synthesized sources. The recomposing
        // backend runs the same function for an omitted argument's default,
        // so the two agree about what a use site substitutes.
        let instance = match instantiate(site.span(), template, bindings, self.guard) {
            Ok(instance) => instance,
            Err(error) => {
                // An unresolved placeholder, at whatever depth of the driver's
                // wrapping (a conditional's branch, a default value), is the
                // use site's recovery; anything else is the pass's failure.
                // Among those, techy's `ContentParentDropped` — a callable's
                // content region anchored on a child the pass dropped — is
                // unreachable: the pass drops only placeholder and conditional
                // nodes, both callables, and techy anchors such regions on groups
                // and body lists, never on a callable. A change that made a
                // placeholder or a conditional a content anchor would turn a
                // document condition into a whole-tree failure.
                let Some(reference) = error.bad_placeholder().map(String::from) else {
                    return Err(error);
                };
                let condition = BadPlaceholder::new(
                    template.definition(),
                    reference,
                    PlaceholderProblem::Unresolvable,
                );
                let diagnostic = Diagnostic::error(condition, site.span().clone());
                return self.recover(cx, site, diagnostic);
            }
        };
        // Then expand: the instance flows through the pass one level deeper,
        // spliced into the use site's sibling run.
        self.depth += 1;
        let mut ids = Vec::new();
        let mut outcome = Ok(());
        for child in instance_nodes(&instance).iter() {
            match cx.restage_subtree(child, self) {
                Ok(staged) => ids.extend(staged),
                Err(error) => {
                    outcome = Err(ExpandError::from(error));
                    break;
                }
            }
        }
        self.depth -= 1;
        outcome?;
        Ok(ids)
    }

    fn recover(
        &mut self,
        cx: &mut ExpandCx<'_>,
        site: NodeRef<'_, Flm>,
        diagnostic: FlmDiagnostic,
    ) -> Result<Vec<BuildId>, ExpandError> {
        self.diags.push(diagnostic);
        let span = site.span().clone();
        let kind = NodeKind::<Flm>::chars(TextContent::Spanned(span.span()));
        let state = Arc::clone(site.parsing_state());
        let builder = cx.builder();
        let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))?;
        Ok(vec![builder.add(kind, span, state, Vec::new(), ext, ())?])
    }

    fn diagnose(&mut self, diagnostic: FlmDiagnostic) {
        self.diags.push(diagnostic);
    }

    fn lang(&self) -> &BuiltLang {
        self.lang
    }
}
