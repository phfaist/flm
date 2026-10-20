//! The splicing pass: [`splice`], a plain function over a tree that replaces
//! the node of every `\input` command by the content of the included file,
//! when the parse read that file.
//!
//! When a parse reads `\input{file}` and the host supplies the file, the parse
//! reads the file's content and places it in a node slot of the command's node
//! (techy's `ParsedSlot`, a part of a node's content outside its arguments,
//! such as an environment's body), the slot
//! [`ATTACHED_SLOT`](crate::lang::input::ATTACHED_SLOT) (see
//! [`crate::lang::input`]). This pass removes that node and puts the included
//! nodes in its place, among the sibling nodes that held the command. A
//! paragraph begun before the command therefore continues into the included
//! content and out of it again. The included nodes keep their spans, which
//! point into the included file. An `\input` command inside the included
//! content is replaced in the same way, at every depth.
//!
//! Two kinds of `\input` node have no attached content and stay in the tree
//! unchanged: a command whose file was not brought in (the parse reported the
//! problem), and the command of a heading input, which places the included
//! file as a tree of its own ([`crate::lang::input::is_placing`]).
//!
//! The pass runs before every other rewrite, so that user definitions, block
//! structure and every later stage see the included content as part of the
//! surrounding text. The `input` feature ([`crate::stdcore::features::input`])
//! places the pass in a plan as its rewrite stage `flm.input.splice`.

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::transform::{Restage, RestageContext, RestageError, RestageVisitor};

use crate::lang::flm::Flm;
use crate::lang::guard::{self, GuardPolicy, NestingTooDeep};
use crate::lang::input::attached_content;
use crate::lang::origin::{FlmDiagnostic, FlmSpan};
use crate::lang::parse::WorkerThreadUnavailable;

/// Condition `flm.input.splice-failed`: the splicing pass failed for a reason
/// that lies in the program, not in the document (an operation of techy's tree
/// builder refused, for example), and returned no tree.
///
/// Recovery: none. [`SpliceFailure::Failed`] carries this diagnostic, which
/// is recorded in place of the absent tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.splice-failed",
    message = "inserting the content of included files into the document failed: {detail}"
)]
pub struct SpliceFailed {
    /// The failure, as text.
    pub detail: String,
}

/// The failure of a call to [`splice`] that returned no tree.
///
/// Each variant carries the one Error diagnostic that explains the absent tree,
/// positioned at the root of the tree the pass was called on;
/// [`diagnostic`](SpliceFailure::diagnostic) returns it for every variant.
/// The `input` feature's rewrite stage records it in place of the tree.
#[derive(Debug)]
pub enum SpliceFailure {
    /// The [recursion guard](crate::lang::guard) refused to descend further:
    /// the content is nested too deeply. The diagnostic carries
    /// [`NestingTooDeep`] (`flm.blocks.nesting-too-deep`).
    NestingTooDeep(FlmDiagnostic),
    /// The pass failed for a reason in the program, not in the document. The
    /// diagnostic carries [`SpliceFailed`] (`flm.input.splice-failed`).
    Failed(FlmDiagnostic),
    /// The pass was not attempted: the calling thread is not a [worker
    /// thread](crate::lang::guard::on_worker_thread) and the operating system
    /// refused to create one. The diagnostic carries
    /// [`WorkerThreadUnavailable`] (`flm.lang.worker-thread-unavailable`).
    WorkerThread(FlmDiagnostic),
}

impl SpliceFailure {
    /// The Error diagnostic that explains the absent tree.
    pub fn diagnostic(&self) -> &FlmDiagnostic {
        match self {
            SpliceFailure::NestingTooDeep(diagnostic)
            | SpliceFailure::Failed(diagnostic)
            | SpliceFailure::WorkerThread(diagnostic) => diagnostic,
        }
    }
}

/// Replaces, throughout `tree`, the node of every `\input` command by the
/// content of the included file (for each command whose file the parse read),
/// and returns the new tree.
///
/// `tree` is a parsed tree, or a tree an earlier rewrite produced. The nodes of
/// the included content take the place of the command's node among its
/// siblings, at every depth; the module documentation says which `\input`
/// nodes stay. A tree with no `\input` command comes back as an identical copy.
///
/// The walk runs under the recursion-guard policy `guard`, normally the built
/// language's ([`BuiltLang::guard`](crate::lang::BuiltLang::guard)). It runs on
/// a [worker thread](crate::lang::guard::on_worker_thread): on the calling
/// thread when that thread is already one, otherwise on a thread created for
/// the call.
///
/// # Errors
///
/// [`SpliceFailure`] when no tree could be produced: the content is nested too
/// deeply for the guard, the operating system refused to create a worker
/// thread, or an operation of the tree builder failed. Each variant carries the
/// one Error diagnostic to record in place of the tree.
pub fn splice(tree: &NodeTree<Flm>, guard: GuardPolicy) -> Result<Arc<NodeTree<Flm>>, SpliceFailure> {
    let at = tree.root().span().clone();
    let run = || run_pass(tree, guard);
    match guard::on_worker_thread(run) {
        Ok(result) => result,
        Err(error) => Err(SpliceFailure::WorkerThread(Diagnostic::error(
            WorkerThreadUnavailable::new(error.to_string()),
            at,
        ))),
    }
}

/// The pass itself, on a thread that meets the worker-thread requirement.
fn run_pass(tree: &NodeTree<Flm>, policy: GuardPolicy) -> Result<Arc<NodeTree<Flm>>, SpliceFailure> {
    let mut pass = SplicePass;
    match guard::restager(&mut pass, policy).restage(tree) {
        Ok(spliced) => Ok(Arc::new(spliced)),
        Err(error) => Err(failure(error, tree.root().span().clone())),
    }
}

/// The [`SpliceFailure`] for a failed restage (techy's walk that builds the new
/// tree), positioned at `at`: a refusal of the guard, possibly wrapped in the
/// visitor's error, becomes [`SpliceFailure::NestingTooDeep`]; anything else
/// becomes [`SpliceFailure::Failed`].
fn failure(error: RestageError<RestageFailure>, at: FlmSpan) -> SpliceFailure {
    let refusal = match &error {
        RestageError::DescentLimitExceeded { detail } => Some(detail.clone()),
        RestageError::Visitor(RestageFailure(inner)) => match inner.as_ref() {
            RestageError::DescentLimitExceeded { detail } => Some(detail.clone()),
            _ => None,
        },
        _ => None,
    };
    match refusal {
        Some(detail) => SpliceFailure::NestingTooDeep(Diagnostic::error(NestingTooDeep::new(detail), at)),
        None => SpliceFailure::Failed(Diagnostic::error(SpliceFailed::new(error.to_string()), at)),
    }
}

/// A failure of a context operation inside the visitor, boxed so that the
/// visitor can carry it through its own error type.
#[derive(Debug)]
struct RestageFailure(Box<RestageError<RestageFailure>>);

impl std::fmt::Display for RestageFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<RestageError<RestageFailure>> for RestageFailure {
    fn from(error: RestageError<RestageFailure>) -> RestageFailure {
        RestageFailure(Box::new(error))
    }
}

/// The visitor of the restage: at an `\input` node with attached content it
/// stages (builds into the new tree) the included nodes in place of the node;
/// every other node is copied and its children visited. It has no state.
struct SplicePass;

impl RestageVisitor<Flm, (), ()> for SplicePass {
    type Error = RestageFailure;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, RestageFailure> {
        let Some(content) = attached_content(&node) else { return Ok(Restage::Descend(())) };
        let mut ids = Vec::new();
        for child in content {
            ids.extend(cx.restage_subtree(child, self)?);
        }
        Ok(Restage::Emit(ids))
    }
}
