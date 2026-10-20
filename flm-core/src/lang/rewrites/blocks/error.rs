//! The problems the [block-structure pass](super) reports: the conditions it
//! recovers from, and the failure that leaves no tree.
//!
//! The pass reports a problem in one of two ways, as every part of FLM does
//! (see [diagnostics](crate::guide::concepts_overview#diagnostics)):
//!
//! - A problem in the document's content is reported as an Error diagnostic
//!   and the pass goes on: it returns the structured tree, and the diagnostic
//!   marks the place of the problem. The conditions are [`BlockInInline`],
//!   [`ParbreakInInline`] and [`DecoratorWithoutHome`]; each one's
//!   documentation says what the pass does with the content.
//! - A problem after which the pass cannot continue ends the pass with a
//!   [`StructureError`], and there is **no tree**. One Error diagnostic
//!   explains the absent tree ([`StructureError::diagnostic`]): the
//!   [recursion guard](crate::lang::guard) refused the nesting depth, the pass
//!   or an element's aspect broke an invariant of the tree builder, or no
//!   [worker thread](crate::lang::guard::on_worker_thread) could be created.
//!
//! No input document makes the pass panic.
//!
//! The pass has an error type of its own rather than
//! [`RewriteError`](crate::core::stage::RewriteError), the error type of
//! rewrite stages, because the pass is a
//! plain function that can be called with no stage around it, and
//! `RewriteError` belongs to the stages. The rewrite stage of the `blocks`
//! feature ([`crate::stdcore::features::blocks`]) converts one into the other
//! with [`StructureError::diagnostic`].

use std::fmt;

use techy::core::node::{NodeBuildError, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::transform::RestageError;

use crate::model::names::ElementName;
use crate::lang::flm::Flm;
use crate::lang::guard::NestingTooDeep;
use crate::lang::origin::FlmDiagnostic;

// --- recovered conditions -------------------------------------------------------------

/// Condition `flm.blocks.block-in-inline`: a node whose element has the block
/// role [`Block`](crate::model::aspects::BlockRole::Block) is placed in a
/// [content region](super) (a run of sibling nodes laid out together) laid out
/// inline, such as the top level of a fragment laid out
/// [`Inline`](crate::model::doc::FragmentLayout::Inline), a `{…}` group, an
/// argument laid out inline, or a body laid out inline.
///
/// Recovery: the node is kept where it is written, among the inline content;
/// the diagnostic is placed at the node. A lowering rule that renders
/// such a node differently in an inline content region asks
/// [`stands_in_blocks`](super::stands_in_blocks).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.block-in-inline",
    message = "‘{element}’ is a block-level construct and must not appear inside inline content"
)]
pub struct BlockInInline {
    /// The stable name of the node's element.
    pub element: String,
}

/// Condition `flm.blocks.parbreak-in-inline`: a paragraph break (a blank line)
/// is written in a [content region](super) (a run of sibling nodes laid out
/// together) laid out inline: a `{…}` group, an argument
/// laid out inline, a body laid out inline, or the top level of a fragment laid
/// out [`Inline`](crate::model::doc::FragmentLayout::Inline).
///
/// Recovery: the paragraph break is read as one space; the diagnostic is
/// placed at the blank line.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.parbreak-in-inline",
    message = "a paragraph break (a blank line) must not appear inside inline content, such \
               as a `{{…}}` group or an argument or body laid out inline"
)]
pub struct ParbreakInInline {
    /// The kind of content region the paragraph break was found in: `group`,
    /// `argument`, `body`, `fragment` (the top level), `list` (the content of a
    /// container node inside inline content), or `run` (a run of nodes that a
    /// block-structure hook laid out).
    pub context: String,
}

/// Condition `flm.blocks.decorator-without-home`: an element's
/// [block-structure hook](super::hook::BlockStructure) created a node through
/// [`BlockStructurer::synthesize_callable`](super::hook::BlockStructurer::synthesize_callable)
/// and returned without placing that node in any run of nodes it laid out, so
/// the node has no place in the structured tree.
///
/// This is a programming error in the hook. It is reported as a diagnostic
/// rather than as a failure, so that one faulty element does not make the
/// document lose its tree.
///
/// Recovery: the node is dropped; the diagnostic is placed at the node whose
/// hook created it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.decorator-without-home",
    message = "the block-structure hook of ‘{hook}’ must place every node it creates, \
               and it did not place its node of ‘{element}’"
)]
pub struct DecoratorWithoutHome {
    /// The stable name of the created node's element.
    pub element: String,
    /// The stable name of the element whose hook created the node.
    pub hook: String,
}

// --- failures ---------------------------------------------------------------------------

/// Condition `flm.blocks.structure-failed`: the pass failed for a reason in the
/// program, not in the document: the pass itself, an element's
/// [`BlockTransparent`](super::transparent::BlockTransparent) aspect or its
/// [block-structure hook](super::hook::BlockStructure) broke an invariant of
/// techy's tree builder, or the aspect or the hook reported a failure of its
/// own.
///
/// Recovery: none; there is no tree. [`StructureError::diagnostic`] builds this
/// diagnostic at the root of the tree and the caller records it in place of
/// the tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.structure-failed",
    message = "dividing the content into paragraphs and blocks failed: {detail}"
)]
pub struct StructureFailed {
    /// What failed, as text: the [`Display`](fmt::Display) text of the
    /// [`StructureError`].
    pub detail: String,
}

/// Condition `flm.blocks.worker-thread-unavailable`: the pass was not run,
/// because the calling thread is not a [worker
/// thread](crate::lang::guard::on_worker_thread) and the operating system
/// refused to create one.
///
/// The pass runs only on a thread with a large enough stack, so that the
/// [recursion guard](crate::lang::guard) can refuse deeply nested content with
/// a diagnostic before the stack overflows.
///
/// Recovery: none; there is no tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.worker-thread-unavailable",
    message = "dividing the content into paragraphs and blocks needs a thread with a large \
               stack, and the operating system could not create one: {detail}"
)]
pub struct NoWorkerThread {
    /// The operating system's error.
    pub detail: String,
}

/// The failure of a call to the [block-structure pass](super::pass) that
/// returned no tree.
///
/// [`structure`](super::structure) and
/// [`structure_with_guard`](super::structure_with_guard) return it; an element's
/// [`BlockTransparent`](super::transparent::BlockTransparent) aspect or
/// [block-structure hook](super::hook::BlockStructure) returns it to report a
/// failure of its own ([`StructureError::cut`], [`StructureError::hook`]), and
/// the operations of [`BlockCutter`](super::transparent::BlockCutter) and
/// [`BlockStructurer`](super::hook::BlockStructurer) return it when they fail.
///
/// Every variant corresponds to one Error diagnostic, which
/// [`diagnostic`](StructureError::diagnostic) builds, so an absent tree is
/// always explained.
#[derive(Debug, Clone)]
pub enum StructureError {
    /// The [recursion guard](crate::lang::guard) refused to descend further:
    /// the content is nested too deeply. The diagnostic carries
    /// [`NestingTooDeep`] (`flm.blocks.nesting-too-deep`).
    NestingTooDeep {
        /// The guard's explanation of which limit was reached.
        detail: String,
    },
    /// The pass was not run, because the operating system refused to create a
    /// worker thread. The diagnostic carries [`NoWorkerThread`]
    /// (`flm.blocks.worker-thread-unavailable`).
    WorkerThread {
        /// The operating system's error.
        detail: String,
    },
    /// An operation of techy's tree builder failed: the pass, an aspect or a
    /// hook broke an invariant of the builder, or asked for an edit of an
    /// argument or a node slot (techy's `ParsedSlot`, a part of a node's
    /// content outside its arguments) that the builder refuses. The diagnostic
    /// carries [`StructureFailed`].
    ///
    /// The error is boxed because the type is recursive: techy's restage error
    /// can hold an error of the walk, which is this type again.
    Restage(Box<RestageError<StructureError>>),
    /// An element's [block-structure hook](super::hook::BlockStructure)
    /// reported a failure of its own ([`StructureError::hook`]). The diagnostic
    /// carries [`StructureFailed`].
    Hook {
        /// The element whose hook failed.
        element: ElementName,
        /// The hook's explanation.
        detail: String,
    },
    /// A block-structure hook asked for the body of a node that has no body
    /// slot (the node slot marked by its body marker as the node's body, such
    /// as an environment's body; see
    /// [`FlmNodeExts`](crate::lang::flm::FlmNodeExts)), through
    /// [`body_run`](super::hook::BlockStructurer::body_run) or
    /// [`with_body`](super::hook::BlockStructurer::with_body). This is a
    /// programming error in the hook. The diagnostic carries
    /// [`StructureFailed`].
    NoBody {
        /// The element whose hook asked for the body.
        element: ElementName,
    },
    /// An element's [`BlockTransparent`](super::transparent::BlockTransparent)
    /// aspect reported a failure of its own ([`StructureError::cut`]), or
    /// called an operation of [`BlockCutter`](super::transparent::BlockCutter)
    /// wrongly, for example asked it to rebuild a part the node does not have.
    /// The diagnostic carries [`StructureFailed`].
    Cut {
        /// The element whose aspect failed.
        element: ElementName,
        /// What failed.
        detail: String,
    },
}

impl StructureError {
    /// Returns a [`StructureError::Hook`] for the element `element` whose
    /// block-structure hook failed, with the hook's explanation `detail`. A hook
    /// returns this to end the pass; a problem in the document is reported
    /// through [`BlockStructurer::diagnose`](super::hook::BlockStructurer::diagnose)
    /// instead.
    pub fn hook(element: ElementName, detail: impl Into<String>) -> StructureError {
        StructureError::Hook {
            element,
            detail: detail.into(),
        }
    }

    /// Returns a [`StructureError::Cut`] for the element `element` whose
    /// [`BlockTransparent`](super::transparent::BlockTransparent) aspect
    /// failed, with the aspect's explanation `detail`. An aspect returns this to
    /// end the pass; a problem in the document is reported through
    /// [`BlockCutter::diagnose`](super::transparent::BlockCutter::diagnose)
    /// instead.
    pub fn cut(element: ElementName, detail: impl Into<String>) -> StructureError {
        StructureError::Cut {
            element,
            detail: detail.into(),
        }
    }

    /// Returns the Error diagnostic that explains the absent tree, positioned at
    /// the root of `tree`, the tree the pass was called on. A rewrite stage
    /// records it in place of that tree.
    ///
    /// [`StructureError::NestingTooDeep`] gives the condition
    /// [`NestingTooDeep`], and [`StructureError::WorkerThread`] the condition
    /// [`NoWorkerThread`]; every other variant gives [`StructureFailed`], with
    /// this error's [`Display`](fmt::Display) text as its detail.
    pub fn diagnostic(&self, tree: &NodeTree<Flm>) -> FlmDiagnostic {
        let span = tree.root().span().clone();
        match self {
            StructureError::NestingTooDeep { detail } => {
                Diagnostic::error(NestingTooDeep::new(detail.clone()), span)
            }
            StructureError::WorkerThread { detail } => {
                Diagnostic::error(NoWorkerThread::new(detail.clone()), span)
            }
            other => Diagnostic::error(StructureFailed::new(other.to_string()), span),
        }
    }
}

impl fmt::Display for StructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StructureError::NestingTooDeep { detail } => {
                write!(f, "the content is nested too deeply: {detail}")
            }
            StructureError::WorkerThread { detail } => {
                write!(f, "the operating system could not create a thread with a large stack: {detail}")
            }
            StructureError::Restage(error) => write!(f, "rebuilding the tree failed: {error}"),
            StructureError::Hook { element, detail } => {
                write!(
                    f,
                    "the block-structure hook of ‘{element}’ failed: {detail}"
                )
            }
            StructureError::NoBody { element } => {
                write!(
                    f,
                    "the block-structure hook of ‘{element}’ asked for the body of a node \
                           that has no body"
                )
            }
            StructureError::Cut { element, detail } => {
                write!(f, "dividing ‘{element}’ at the paragraph breaks and blocks of its content failed: {detail}")
            }
        }
    }
}

impl std::error::Error for StructureError {}

impl From<RestageError<StructureError>> for StructureError {
    /// Converts techy's restage error as techy documents it for a walk that
    /// calls itself again: an error of the walk itself is returned unchanged,
    /// a refusal of the recursion guard becomes
    /// [`NestingTooDeep`](StructureError::NestingTooDeep), and any other error
    /// is boxed as [`Restage`](StructureError::Restage).
    fn from(error: RestageError<StructureError>) -> StructureError {
        match error {
            RestageError::Visitor(error) => error,
            RestageError::DescentLimitExceeded { detail } => {
                StructureError::NestingTooDeep { detail }
            }
            other => StructureError::Restage(Box::new(other)),
        }
    }
}

impl From<NodeBuildError> for StructureError {
    /// Wraps a failure of techy's tree builder as
    /// [`Restage`](StructureError::Restage).
    fn from(error: NodeBuildError) -> StructureError {
        StructureError::Restage(Box::new(RestageError::Build(error)))
    }
}
