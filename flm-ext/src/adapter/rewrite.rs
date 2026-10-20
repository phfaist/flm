//! `ExtRewriteStage`, `flm-core`'s rewrite stage over the extension API's
//! [`RewriteStage`], the visitor that drives it ([`ExtVisitor`]), and the
//! façades it lends the handler ([`RewriteCxFacade`], [`RestagerFacade`],
//! and the bundles [`RestagedArgumentFacade`], [`RestagedSlotFacade`]).
//!
//! # The run of one tree
//!
//! [`ExtRewriteStage::rewrite`] runs techy's driver
//! ([`TreeRestager`](flm_core::techy::transform::TreeRestager)) under the
//! registry's recursion guard ([`guard::restager`]) on a worker thread
//! ([`guard::on_worker_thread`]), as the `input` feature's splice does. The
//! visitor is an [`ExtVisitor`]: for every node the driver reaches, it lends
//! the handler the node, the context and a restager for one call, and maps the
//! handler's answer. The diagnostics the handler reports are collected in the
//! visitor and handed to the run once the driver is done.
//!
//! The restager's region operations (`restage_subtree`, `restage_children`,
//! `restage_argument`, …) call techy's with the same visitor, so the nodes of
//! the region go through the handler again: nested calls, each with façades of
//! its own.
//!
//! # What a failure leaves behind
//!
//! - The handler fails (an `Err`, an `Emit` naming a node this call did not
//!   stage, or naming one twice or one already given as content): the Error
//!   `flm.ext.handler-failed` at the node, and the node is kept as the
//!   driver's `Descend` keeps it.
//! - An operation is refused by techy for a reason the handler can recover
//!   from (an index out of range, an unknown argument name, a node that is not
//!   a callable, an absent argument given content): [`ExtError::Core`] of kind
//!   `restage`, which the handler may catch. The text is worded in the
//!   author's terms: the construct as written (`\name`,
//!   `\begin{name}`, a specials sequence) with its line, an argument or a slot
//!   by its index, never techy's internal node identifiers.
//! - Content given to `restage_argument_with_content` or
//!   `restage_slot_with_content` must have been staged by the same call; it is
//!   recorded as placed once techy accepts the operation, so content given
//!   twice is the method's refusal ([`ExtError::Argument`]); content given
//!   and also emitted is caught when the answer is placed, which is the
//!   handler's failure above (`flm.ext.handler-failed`, the node kept).
//! - An operation meets an error after which the rewrite cannot go on (a
//!   refusal of the output builder, a dropped content parent, the descent
//!   guard's refusal, or such an error in a nested call): the restager keeps
//!   it, and the visitor returns it whatever the handler answers, which ends
//!   the driver's run. The driver can also meet such an error by itself after
//!   the handler answered, when it places the answer or finishes the tree (the
//!   root replaced by other than one node; an argument's content inside a node
//!   the handler removed). The guard's refusal is
//!   [`RewriteError::NestingTooDeep`] (`flm.blocks.nesting-too-deep`); any
//!   other is [`RewriteError::Failed`] with the Error `flm.ext.rewrite-failed`
//!   at the tree's root, worded the same way. `flm-core` then leaves the
//!   tree out of every later table.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use flm_core::core::plan::PlanCx;
use flm_core::core::run::RewriteCx as CoreRewriteCx;
use flm_core::core::stage::{
    Needs as CoreNeeds, RewriteDecl as CoreRewriteDecl, RewriteError, RewriteStage as CoreRewriteStage,
};
use flm_core::lang::guard::{self, GuardPolicy, NestingTooDeep};
use flm_core::lang::parse::WorkerThreadUnavailable;
use flm_core::lang::{Flm, FlmCallableType, FlmDiagnostic, FlmDiagnostics, FlmSpan};
use flm_core::model::addr::TreeId;
use flm_core::model::doc::FragmentLayout;
use flm_core::model::names::CapName;
use flm_core::model::options::OptionRef;
use flm_core::model::element::element_of;
use flm_core::techy::core::node::{BuildId, NodeBuildError, NodeId, NodeKind, NodeRef, NodeTree};
use flm_core::techy::core::Lang;
use flm_core::techy::source::TextContent;
use flm_core::techy::transform::{
    Restage as CoreRestage, RestageContext, RestageError, RestageVisitor, RestagedArgument as CoreRestagedArgument,
    RestagedSlot as CoreRestagedSlot,
};
use serde_json::Value;

use crate::adapter::diag::ExtCondition;
use crate::adapter::names::Owner;
use crate::adapter::node::NodeFacade;
use crate::adapter::stage::read_ref;
use crate::adapter::values::diagnostic_to_core;
use crate::api::core::run::RewriteCx;
use crate::api::core::stage::{Needs, RewriteDecl, RewriteStage};
use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::Node;
use crate::api::techy::transform::{Restage, RestagedArgument, RestagedSlot, Restager, Staged};
use crate::runtime::ExtError;
use crate::sealed::{RestagedArgumentCore, RestagedSlotCore, RestagerCore, RewriteCxCore, Token};

// --- the stage ---------------------------------------------------------------------------------

/// `flm-core`'s rewrite stage over an extension's [`RewriteStage`]: what
/// [`FeatureBuilder::rewrite_stage`](crate::api::core::registry::FeatureBuilder::rewrite_stage)
/// registers. The declaration and the needs were read once, when the stage
/// was registered, and are answered from that copy.
pub struct ExtRewriteStage {
    /// The extension's handler.
    stage: Arc<dyn RewriteStage>,
    /// Its declaration, read at registration.
    decl: RewriteDecl,
    /// Its needs, read at registration.
    needs: Needs,
    /// The extension and the package that registered the stage: the
    /// identifiers of the diagnostics the handler reports must begin with the
    /// extension's stem.
    owner: Owner,
}

impl ExtRewriteStage {
    /// The stage over `stage`, whose declaration `decl` and needs `needs` were
    /// read at registration, registered by `owner`.
    pub(crate) fn new(stage: Arc<dyn RewriteStage>, decl: RewriteDecl, needs: Needs, owner: Owner) -> ExtRewriteStage {
        ExtRewriteStage { stage, decl, needs, owner }
    }

    /// The driver's run over `tree`, on the calling thread: the new tree or
    /// the failure, and the diagnostics the handler reported.
    fn run(
        &self,
        tree_id: TreeId,
        tree: &NodeTree<Flm>,
        cx: &CoreRewriteCx<'_>,
        policy: GuardPolicy,
    ) -> (Result<Arc<NodeTree<Flm>>, RewriteError>, Vec<FlmDiagnostic>) {
        let mut visitor = ExtVisitor {
            stage: Arc::clone(&self.stage),
            name: &self.decl.name,
            owner: &self.owner,
            cx,
            diagnostics: Vec::new(),
        };
        let result = guard::restager(&mut visitor, policy).restage(tree);
        let at = tree.root().span().clone();
        let result = result.map(Arc::new).map_err(|error| self.failure(flatten(error), Some(tree), tree_id, at));
        (result, visitor.diagnostics)
    }

    /// The [`RewriteError`] for a run the driver abandoned, at `at`: the
    /// guard's refusal is [`RewriteError::NestingTooDeep`], as every guarded
    /// pass reports it; anything else is the Error `flm.ext.rewrite-failed`,
    /// worded by [`describe`] over the input tree `tree`.
    fn failure(
        &self,
        error: RestageError<RewriteFailure>,
        tree: Option<&NodeTree<Flm>>,
        tree_id: TreeId,
        at: FlmSpan,
    ) -> RewriteError {
        match error {
            RestageError::DescentLimitExceeded { detail } => {
                RewriteError::NestingTooDeep(FlmDiagnostic::error(NestingTooDeep::new(detail), at))
            }
            other => {
                let condition =
                    ExtCondition::rewrite_failed(&self.decl.name, &tree_id.to_string(), &describe(&other, tree));
                RewriteError::Failed(FlmDiagnostic::error(condition, at))
            }
        }
    }
}

impl CoreRewriteStage for ExtRewriteStage {
    /// The declaration read at registration.
    fn decl(&self) -> CoreRewriteDecl {
        let decl = &self.decl;
        let mut core = CoreRewriteDecl::new(decl.name.as_str(), decl.publishes.as_str()).docs(decl.docs.as_str());
        if let Some(parent) = &decl.refines {
            core = core.refines(parent.as_str());
        }
        if decl.default {
            core = core.default();
        }
        core
    }

    /// The needs read at registration, every field passed through: a table
    /// read is named by its name alone, typed as the registry declares it
    /// (and the plan refuses a rewrite stage's read); an option type is read
    /// by its name ([`OptionRef::named`]).
    fn needs(&self, cx: &PlanCx<'_>) -> CoreNeeds {
        let registry = cx.registry();
        let needs = &self.needs;
        let mut core = CoreNeeds::new();
        core.requires = needs.requires.iter().map(|name| CapName::from(name.as_str())).collect();
        core.reads = needs.reads.iter().map(|name| read_ref(registry, name)).collect();
        core.reads_opt = needs.reads_opt.iter().map(|name| read_ref(registry, name)).collect();
        core.after = needs.after.iter().map(|name| CapName::from(name.as_str())).collect();
        core.before = needs.before.iter().map(|name| CapName::from(name.as_str())).collect();
        core.options = needs.options.iter().map(|name| OptionRef::named(name.as_str())).collect();
        core
    }

    /// Drives the handler over `tree` on a worker thread (see the [module
    /// documentation](self)); the handler's diagnostics are added to `diags`.
    fn rewrite(
        &self,
        tree_id: TreeId,
        tree: &Arc<NodeTree<Flm>>,
        cx: &CoreRewriteCx<'_>,
        diags: &mut FlmDiagnostics,
    ) -> Result<Arc<NodeTree<Flm>>, RewriteError> {
        let at = tree.root().span().clone();
        let policy = cx.registry().lang().guard();
        match guard::on_worker_thread(|| self.run(tree_id, tree, cx, policy)) {
            Ok((result, found)) => {
                for diagnostic in found {
                    diags.push(diagnostic);
                }
                result
            }
            Err(error) => Err(RewriteError::Failed(FlmDiagnostic::error(
                WorkerThreadUnavailable::new(error.to_string()),
                at,
            ))),
        }
    }
}

// --- the visitor -------------------------------------------------------------------------------

/// A failure that ends the rewrite of a tree, carried through the visitor's
/// own error type (techy wraps it in [`RestageError::Visitor`] on its way
/// out of a nested call, from which the adapter takes it out again).
#[derive(Debug)]
pub struct RewriteFailure(Box<RestageError<RewriteFailure>>);

impl fmt::Display for RewriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// `error`, the failure of a nested call taken out of its
/// [`RestageError::Visitor`] wrapper (the failure kept is always the
/// innermost, never wrapped).
fn flatten(error: RestageError<RewriteFailure>) -> RestageError<RewriteFailure> {
    match error {
        RestageError::Visitor(RewriteFailure(inner)) => *inner,
        other => other,
    }
}

/// Whether techy's `error` leaves the rewrite unable to go on: every error but
/// the refusals of an operation's arguments, which leave the builder as it was.
fn ends_the_run(error: &RestageError<RewriteFailure>) -> bool {
    !matches!(
        error,
        RestageError::UnknownArgumentName { .. }
            | RestageError::ArgumentIndexOutOfRange { .. }
            | RestageError::SlotIndexOutOfRange { .. }
            | RestageError::NotACallable { .. }
            | RestageError::ArgumentAbsent { .. }
    )
}

/// techy's error as the handler sees it: [`ExtError::Core`] of kind
/// `restage`, worded by [`describe`] over the input tree `tree`.
fn restage_error(error: &RestageError<RewriteFailure>, tree: &NodeTree<Flm>) -> ExtError {
    ExtError::Core { kind: "restage".to_string(), detail: describe(error, Some(tree)), data: Value::Null }
}

/// techy's `error` in an extension author's terms: the construct named as
/// written, with its line (from the input tree `tree`, when given), an
/// argument or a slot by its index, and no internal identifier of a node.
/// An error that carries no identifier keeps techy's text.
fn describe(error: &RestageError<RewriteFailure>, tree: Option<&NodeTree<Flm>>) -> String {
    let construct = |id: NodeId| construct(tree, id);
    match error {
        RestageError::UnknownArgumentName { node, name } => format!("{} has no argument named `{name}`", construct(*node)),
        RestageError::ArgumentIndexOutOfRange { node, index, count } => {
            format!("{} has no argument at index {index}: it has {}", construct(*node), counted(*count, "argument"))
        }
        RestageError::SlotIndexOutOfRange { node, index, count } => {
            format!("{} has no slot at index {index}: it has {}", construct(*node), counted(*count, "slot"))
        }
        RestageError::NotACallable { node } => {
            format!("{} is not a macro, an environment or specials, so it has no arguments and no slots", construct(*node))
        }
        RestageError::ArgumentAbsent { node, index } => {
            format!("the argument at index {index} of {} must be given in the document to receive new content", construct(*node))
        }
        RestageError::ContentParentDropped { callable, .. } => format!(
            "the content of an argument or a slot of {} lies inside a node the rewrite stage removed or replaced by \
             other than one node; replace the whole call instead, with a copy made by `restage_invocation`",
            construct(*callable)
        ),
        RestageError::Build(build) => describe_build(build),
        other => other.to_string(),
    }
}

/// The output builder's refusal in an extension author's terms; a refusal
/// that carries no identifier keeps techy's text.
fn describe_build(error: &NodeBuildError) -> String {
    match error {
        NodeBuildError::ChildNotStaged { .. } => "a node placed in the new tree was not created by this rewrite".to_string(),
        NodeBuildError::ChildAlreadyClaimed { .. } => "a node created by the rewrite stage was placed twice".to_string(),
        NodeBuildError::RootNotStaged { .. } | NodeBuildError::RootClaimed { .. } => {
            "the root of the new tree must be one node, placed nowhere else".to_string()
        }
        NodeBuildError::ContentParentNotStaged { .. }
        | NodeBuildError::ContentParentUnreachable { .. }
        | NodeBuildError::ContentParentOutsideSubtree { .. }
        | NodeBuildError::ContentParentOutsideRegion { .. }
        | NodeBuildError::ContentParentUnmapped { .. }
        | NodeBuildError::StagedContentParentUnmapped { .. } => {
            "the content of a restaged argument or slot must lie inside the call it belongs to".to_string()
        }
        other => other.to_string(),
    }
}

/// How a message names the node `id` of `tree`, by its callable's type: a
/// macro as written (`` `\sdquote` ``, a control symbol with its backslash,
/// `` `\,` ``), an environment by its opening (`` `\begin{itemize}` ``),
/// specials as the sequence written (`` `---` ``), any other node by its
/// element's name; with the line it starts on. "a construct" without the
/// tree.
fn construct(tree: Option<&NodeTree<Flm>>, id: NodeId) -> String {
    let Some(node) = tree.and_then(|tree| tree.get(id)) else { return "a construct".to_string() };
    let what = match (node.callable_type(), node.name()) {
        (Some(FlmCallableType::Macro), Some(name)) => format!("`\\{name}`"),
        (Some(FlmCallableType::Environment), Some(name)) => format!("`\\begin{{{name}}}`"),
        (Some(FlmCallableType::Specials), Some(name)) => format!("`{name}`"),
        _ => format!("the element `{}`", element_of(&node).name),
    };
    let span = node.span();
    match span.source().line_index().line_col(span.range().start) {
        Some((line, _)) => format!("{what} on line {line}"),
        None => what,
    }
}

/// `count` items of `noun`, with the plural when needed.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 { format!("1 {noun}") } else { format!("{count} {noun}s") }
}

/// An argument error of the method `method`.
fn argument(method: &str, detail: &str) -> ExtError {
    ExtError::Argument { method: method.to_string(), detail: detail.to_string() }
}

/// techy's visitor over an extension's handler: called by the driver once per
/// node, it lends the handler the node, a [`RewriteCxFacade`] and a
/// [`RestagerFacade`] for one call.
pub struct ExtVisitor<'x> {
    /// The handler.
    stage: Arc<dyn RewriteStage>,
    /// The stage's name, for the message of the handler's failure.
    name: &'x str,
    /// The extension and the package that registered the stage.
    owner: &'x Owner,
    /// The context of the rewrite.
    cx: &'x CoreRewriteCx<'x>,
    /// The diagnostics of the run, handed to `flm-core` when the driver is
    /// done.
    diagnostics: Vec<FlmDiagnostic>,
}

impl RestageVisitor<Flm, (), ()> for ExtVisitor<'_> {
    type Error = RewriteFailure;

    /// Lends the handler its three façades and maps its answer: `Descend` as
    /// it is, `Emit` with the nodes it names (each staged by this call, each
    /// named once); a failure of the handler is the Error
    /// `flm.ext.handler-failed` at the node and the node kept (`Descend`); a
    /// failure the restager kept ends the run.
    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<CoreRestage<()>, RewriteFailure> {
        // Cloned out before the visitor is lent into the restager.
        let stage = Arc::clone(&self.stage);
        let site = NodeFacade::lent(node);
        let cx_facade = RewriteCxFacade { cx: self.cx };
        let mut rs = RestagerFacade {
            cx,
            visitor: &mut *self,
            site: node,
            staged: HashSet::new(),
            placed: HashSet::new(),
            failure: None,
        };
        let answer = stage.restage(&site, &cx_facade, &mut rs);
        if let Some(failure) = rs.failure.take() {
            return Err(RewriteFailure(Box::new(failure)));
        }
        let answer = answer.and_then(|restage| match restage {
            Restage::Descend => Ok(CoreRestage::Descend(())),
            Restage::Emit(staged) => rs.placeable("restage", &staged).map(CoreRestage::Emit),
        });
        match answer {
            Ok(restage) => Ok(restage),
            Err(error) => {
                let handler = format!("the rewrite stage `{}`", self.name);
                let condition = ExtCondition::handler_failed(&error, &handler);
                self.diagnostics.push(FlmDiagnostic::error(condition, node.span().clone()));
                Ok(CoreRestage::Descend(()))
            }
        }
    }
}

// --- the context -------------------------------------------------------------------------------

/// The context lent to one call of a rewrite stage's handler: `flm-core`'s
/// context of the rewrite, the one implementation of [`RewriteCx`].
pub struct RewriteCxFacade<'a> {
    cx: &'a CoreRewriteCx<'a>,
}

impl RewriteCxCore for RewriteCxFacade<'_> {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&CoreRewriteCx<'_>)) {
        f(self.cx);
    }
}

impl RewriteCx for RewriteCxFacade<'_> {
    fn tree_id(&self) -> String {
        self.cx.tree_id().to_string()
    }

    fn layout(&self) -> String {
        match self.cx.layout() {
            FragmentLayout::Blocks => "blocks",
            FragmentLayout::Inline => "inline",
            FragmentLayout::InlineOrBlocks => "inline-or-blocks",
        }
        .to_string()
    }

    fn option(&self, name: &str) -> Result<Value, ExtError> {
        self.cx.option_named(name).cloned().map_err(|error| ExtError::Core {
            kind: "option".to_string(),
            detail: error.to_string(),
            data: Value::Null,
        })
    }
}

// --- the restager ------------------------------------------------------------------------------

/// The restager lent to one call of a rewrite stage's handler: techy's restage
/// context, the visitor (lent back to techy by every region operation), what
/// the call staged and placed, and the failure that ends the run; the one
/// implementation of [`Restager`].
pub struct RestagerFacade<'a, 'r, 'x, 'n> {
    cx: &'a mut RestageContext<'r, Flm, (), ()>,
    visitor: &'a mut ExtVisitor<'x>,
    /// The node of the call.
    site: NodeRef<'n, Flm>,
    /// The nodes this call staged.
    staged: HashSet<BuildId>,
    /// The nodes this call's answer placed.
    placed: HashSet<BuildId>,
    /// The first error met after which the rewrite cannot go on.
    failure: Option<RestageError<RewriteFailure>>,
}

impl RestagerFacade<'_, '_, '_, '_> {
    /// techy's answer as the handler sees it; an error that ends the run is
    /// kept.
    fn techy<T>(&mut self, result: Result<T, RestageError<RewriteFailure>>) -> Result<T, ExtError> {
        result.map_err(|error| {
            let error = flatten(error);
            let reported = restage_error(&error, self.site.tree());
            if ends_the_run(&error) && self.failure.is_none() {
                self.failure = Some(error);
            }
            reported
        })
    }

    /// Records `ids` as staged by this call; answers them.
    fn mint(&mut self, ids: Vec<BuildId>) -> Vec<Staged> {
        self.staged.extend(ids.iter().copied());
        ids.into_iter().map(Staged).collect()
    }

    /// Records `id` as staged by this call; answers it.
    fn mint_one(&mut self, id: BuildId) -> Staged {
        self.staged.insert(id);
        Staged(id)
    }

    /// The ids of `content`, each staged by this call and not yet placed:
    /// what replaces the content of an argument or a slot. The caller records
    /// them as placed once techy accepts the operation (they are then placed
    /// there as an `Emit` places its nodes, so content given twice is the
    /// method's refusal, and content given and also emitted is refused when
    /// the answer is placed); a refused operation leaves
    /// them free to be placed otherwise.
    fn content(&self, method: &str, content: &[Staged]) -> Result<Vec<BuildId>, ExtError> {
        self.unplaced(method, content)
    }

    /// The ids of `staged`, each staged by this call and not yet placed,
    /// recorded as placed; the method's failure otherwise.
    fn placeable(&mut self, method: &str, staged: &[Staged]) -> Result<Vec<BuildId>, ExtError> {
        let ids = self.unplaced(method, staged)?;
        self.placed.extend(ids.iter().copied());
        Ok(ids)
    }

    /// The ids of `staged`, each staged by this call, not yet placed and not
    /// given twice; the method's failure otherwise. Records nothing.
    fn unplaced(&self, method: &str, staged: &[Staged]) -> Result<Vec<BuildId>, ExtError> {
        let mut ids: Vec<BuildId> = Vec::with_capacity(staged.len());
        let mut seen = HashSet::new();
        for Staged(id) in staged {
            if !self.staged.contains(id) {
                return Err(argument(method, "each node placed must have been created during this same rewrite call"));
            }
            if self.placed.contains(id) || !seen.insert(*id) {
                return Err(argument(method, "a node may be placed only once"));
            }
            ids.push(*id);
        }
        Ok(ids)
    }
}

/// The error of a bundle given to `restage_invocation` again: what the
/// dispatcher answers when the bundle's entry was taken already.
pub(crate) fn spent() -> ExtError {
    argument("restage_invocation", "each restaged argument or slot may be given to `restage_invocation` once")
}

impl RestagerCore for RestagerFacade<'_, '_, '_, '_> {}

impl Restager for RestagerFacade<'_, '_, '_, '_> {
    fn restage_subtree(&mut self, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
        let result = self.cx.restage_subtree(node.core_node_ref(Token::new()), &mut *self.visitor);
        let ids = self.techy(result)?;
        Ok(self.mint(ids))
    }

    fn restage_children(&mut self, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
        let result = self.cx.restage_children(node.core_node_ref(Token::new()), &mut *self.visitor);
        let ids = self.techy(result)?;
        Ok(self.mint(ids))
    }

    fn restage_argument(&mut self, node: &dyn Node<'_>, index: usize) -> Result<Box<dyn RestagedArgument>, ExtError> {
        let result = self.cx.restage_argument(node.core_node_ref(Token::new()), index, &mut *self.visitor);
        Ok(Box::new(RestagedArgumentFacade::new(self.techy(result)?)))
    }

    fn restage_argument_named(&mut self, node: &dyn Node<'_>, name: &str) -> Result<Box<dyn RestagedArgument>, ExtError> {
        let result = self.cx.restage_argument_named(node.core_node_ref(Token::new()), name, &mut *self.visitor);
        Ok(Box::new(RestagedArgumentFacade::new(self.techy(result)?)))
    }

    fn restage_slot(&mut self, node: &dyn Node<'_>, index: usize) -> Result<Box<dyn RestagedSlot>, ExtError> {
        let result = self.cx.restage_slot(node.core_node_ref(Token::new()), index, &mut *self.visitor);
        Ok(Box::new(RestagedSlotFacade::new(self.techy(result)?)))
    }

    fn restage_invocation(
        &mut self,
        node: &dyn Node<'_>,
        args: Vec<Box<dyn RestagedArgument>>,
        slots: Vec<Box<dyn RestagedSlot>>,
    ) -> Result<Staged, ExtError> {
        let mut core_args = Vec::with_capacity(args.len());
        for mut bundle in args {
            core_args.push(bundle.take_core(Token::new()).ok_or_else(spent)?);
        }
        let mut core_slots = Vec::with_capacity(slots.len());
        for mut bundle in slots {
            core_slots.push(bundle.take_core(Token::new()).ok_or_else(spent)?);
        }
        let result =
            self.cx.restage_invocation::<RewriteFailure>(node.core_node_ref(Token::new()), core_args, core_slots, ());
        let id = self.techy(result)?;
        Ok(self.mint_one(id))
    }

    fn restage_argument_with_content(
        &mut self,
        node: &dyn Node<'_>,
        index: usize,
        content: Vec<Staged>,
    ) -> Result<Box<dyn RestagedArgument>, ExtError> {
        let ids = self.content("restage_argument_with_content", &content)?;
        let result = self.cx.restage_argument_with_content::<RewriteFailure>(
            node.core_node_ref(Token::new()),
            index,
            ids.clone(),
            (),
        );
        let restaged = self.techy(result)?;
        self.placed.extend(ids);
        Ok(Box::new(RestagedArgumentFacade::new(restaged)))
    }

    fn restage_slot_with_content(
        &mut self,
        node: &dyn Node<'_>,
        index: usize,
        content: Vec<Staged>,
    ) -> Result<Box<dyn RestagedSlot>, ExtError> {
        let ids = self.content("restage_slot_with_content", &content)?;
        let result =
            self.cx.restage_slot_with_content::<RewriteFailure>(node.core_node_ref(Token::new()), index, ids.clone(), ());
        let restaged = self.techy(result)?;
        self.placed.extend(ids);
        Ok(Box::new(RestagedSlotFacade::new(restaged)))
    }

    /// Stages the node as a pass stages a characters node, and as
    /// [`ExpandCx::stage_chars`](crate::api::lang::rewrites::userdefs::ExpandCx::stage_chars)
    /// does: `NodeKind::chars` over owned text, the span and the parsing state
    /// of `like`, its ext made by `Flm::make_node_ext`, no children.
    fn stage_chars(&mut self, text: &str, like: &dyn Node<'_>) -> Result<Staged, ExtError> {
        let like = like.core_node_ref(Token::new());
        let span = like.span().clone();
        let state = Arc::clone(like.parsing_state());
        let kind = NodeKind::<Flm>::chars(TextContent::Owned(text.into()));
        let builder = self.cx.builder();
        let result = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))
            .and_then(|ext| builder.add(kind, span, state, Vec::new(), ext, ()))
            .map_err(RestageError::Build);
        let id = self.techy(result)?;
        Ok(self.mint_one(id))
    }

    fn diagnose(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError> {
        self.visitor.owner.check("diagnostic", &diagnostic.id)?;
        let diagnostic = diagnostic_to_core(diagnostic, self.site.span().clone())?;
        self.visitor.diagnostics.push(diagnostic);
        Ok(())
    }
}

// --- the bundles -------------------------------------------------------------------------------

/// A restaged argument a [`RestagerFacade`] answered: techy's bundle, until
/// [`restage_invocation`](Restager::restage_invocation) takes it; the one
/// implementation of [`RestagedArgument`].
pub struct RestagedArgumentFacade {
    bundle: Option<CoreRestagedArgument<Flm>>,
    provided: bool,
}

impl RestagedArgumentFacade {
    /// The façade over `bundle`.
    pub(crate) fn new(bundle: CoreRestagedArgument<Flm>) -> RestagedArgumentFacade {
        let provided = bundle.is_provided();
        RestagedArgumentFacade { bundle: Some(bundle), provided }
    }
}

impl RestagedArgumentCore for RestagedArgumentFacade {
    fn take_core(&mut self, _: Token) -> Option<CoreRestagedArgument<Flm>> {
        self.bundle.take()
    }
}

impl RestagedArgument for RestagedArgumentFacade {
    fn is_provided(&self) -> bool {
        self.provided
    }
}

/// A restaged slot a [`RestagerFacade`] answered: techy's bundle, until
/// [`restage_invocation`](Restager::restage_invocation) takes it; the one
/// implementation of [`RestagedSlot`].
pub struct RestagedSlotFacade {
    bundle: Option<CoreRestagedSlot<Flm>>,
    name: Option<String>,
}

impl RestagedSlotFacade {
    /// The façade over `bundle`.
    pub(crate) fn new(bundle: CoreRestagedSlot<Flm>) -> RestagedSlotFacade {
        let name = bundle.name().map(str::to_string);
        RestagedSlotFacade { bundle: Some(bundle), name }
    }
}

impl RestagedSlotCore for RestagedSlotFacade {
    fn take_core(&mut self, _: Token) -> Option<CoreRestagedSlot<Flm>> {
        self.bundle.take()
    }
}

impl RestagedSlot for RestagedSlotFacade {
    fn name(&self) -> Option<String> {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::stage::nowhere;

    /// A stage `sd.dash` of the package `_` of the extension `sd`.
    fn dash() -> ExtRewriteStage {
        struct Nothing;
        impl RewriteStage for Nothing {
            fn decl(&self) -> RewriteDecl {
                RewriteDecl::new("sd.dash", "sd.dashed")
            }
            fn needs(&self) -> Needs {
                Needs::new()
            }
            fn restage<'c>(&self, _: &'c dyn Node<'c>, _: &'c dyn RewriteCx, _: &'c mut dyn Restager) -> Result<Restage, ExtError> {
                Ok(Restage::Descend)
            }
        }
        let owner = Owner::new("sd", "_", "dashes");
        ExtRewriteStage::new(Arc::new(Nothing), RewriteDecl::new("sd.dash", "sd.dashed"), Needs::new(), owner)
    }

    #[test]
    fn the_guard_s_refusal_is_the_nesting_condition_even_from_a_nested_call() {
        let refusal = RestageError::DescentLimitExceeded { detail: "too deep".to_string() };
        let nested = RestageError::Visitor(RewriteFailure(Box::new(refusal)));
        let error = dash().failure(flatten(nested), None, TreeId::tree(0), nowhere());
        let RewriteError::NestingTooDeep(diagnostic) = error else { panic!("{error:?}") };
        assert_eq!(diagnostic.identifier(), "flm.blocks.nesting-too-deep");
        assert_eq!(diagnostic.message(), "the content nests too deeply to process: too deep");
    }

    #[test]
    fn any_other_abandoned_run_is_a_failed_rewrite_at_the_root() {
        let error = dash().failure(RestageError::RootNotSingular { count: 2 }, None, TreeId::tree(3), nowhere());
        let RewriteError::Failed(diagnostic) = error else { panic!("{error:?}") };
        assert_eq!(diagnostic.identifier(), "flm.ext.rewrite-failed");
        assert!(
            diagnostic.message().starts_with("the rewrite stage `sd.dash` must produce one tree from the tree `tree#3`: "),
            "{}",
            diagnostic.message()
        );
    }

    #[test]
    fn only_the_refusals_of_an_operation_s_arguments_leave_the_run_going() {
        assert!(ends_the_run(&RestageError::DescentLimitExceeded { detail: String::new() }));
        assert!(ends_the_run(&RestageError::RootNotSingular { count: 0 }));
    }
}
