//! This module provides the recursion guard of FLM, which reports an error for
//! deeply nested content before the content overflows the stack, along with the
//! worker threads on which every guarded operation runs.
//!
//! Parsing, traversing, rewriting and serializing a tree are recursive
//! operations, so deeply nested content uses much stack. A stack overflow ends
//! the process, and the program that embeds FLM cannot recover from it. To
//! prevent the overflow, techy provides a *descent guard*: the guard measures
//! the stack that a recursive operation has used since it started, and stops the
//! operation when this amount exceeds a budget. The operation then reports an
//! error, for instance [`NestingTooDeep`] (`flm.blocks.nesting-too-deep`) for a
//! parse. See also [the recursion guard and worker
//! threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads).
//!
//! # Guarded construction
//!
//! techy's default budget is small, and the budget of a language or of a
//! traversal can only be set when the language or the traversal is constructed.
//! FLM therefore constructs every recursive operation with one of the following
//! functions of this module, which install the guard with the budget
//! [`STACK_BUDGET`]:
//!
//! - [`language`], for parsing;
//! - [`walker`], [`restager`] and [`recomposer`], for traversals of a tree;
//! - [`serde_session`], for serialization.
//!
//! The limit is selected with the [`GuardPolicy`] argument that each function
//! takes.
//! [`GuardPolicy::StackBudget`] limits the stack use and is the policy for
//! normal use. [`GuardPolicy::DepthLimit`] limits the number of nested descents
//! and is used in tests only.
//!
//! # The worker-thread requirement
//!
//! The guard limits the stack that an operation uses, but it does not check the
//! size of the stack of the current thread. On a thread with less stack than the
//! budget, an operation overflows the stack before the guard stops it. Every
//! guarded operation must therefore run on a *worker thread*: a thread created
//! with at least [`WORKER_STACK_SIZE`] bytes of stack.
//!
//! The entry points of the library meet this requirement for their callers. They
//! run each guarded operation through [`on_worker_thread`], which runs the
//! operation on the current thread if this thread is marked as a worker thread,
//! and otherwise on a new worker thread. The [parse functions](crate::lang::parse),
//! the [rewrite passes](crate::lang::rewrites), the run of a plan
//! ([`Run`](crate::core::run::Run)) and the emitters of the standard backends work
//! in this way. A program that embeds FLM therefore does not need to create
//! worker threads.
//!
//! If you construct a guarded operation with a function of this module, or if
//! you call [`copy_run_into_value`](crate::lang::copy::copy_run_into_value), run
//! the operation inside [`on_worker_thread`].
//!
//! # Content that exceeds the limit
//!
//! When the guard stops an operation, the operation does not panic. It reports
//! an error and produces no result. For instance, a parse returns
//! [`FragmentError::NestingTooDeep`](crate::lang::FragmentError::NestingTooDeep),
//! which contains the error [`NestingTooDeep`]. A rewrite pass records the same
//! error, and the tree that the pass was rewriting is absent from the output of
//! the run.

use std::cell::Cell;
use std::thread;

use techy::core::Language;
use techy::core::StdDescentGuardInit;
use techy::error::DiagnosticInfo;
use techy::recompose::TreeRecomposer;
use techy::serialize::{SerdeSession, SerializableLang};
use techy::transform::TreeRestager;
use techy::visit::TreeWalker;

use crate::lang::flm::Flm;

/// The stack size of a worker thread, in bytes (64 MiB).
///
/// A thread that is marked as a worker thread with [`mark_worker_thread`] must
/// have at least this much stack (see [the worker-thread
/// requirement](self#the-worker-thread-requirement)).
pub const WORKER_STACK_SIZE: usize = 64 * 1024 * 1024;

/// The amount of stack, in bytes, that a guarded operation may use before the
/// recursion guard stops it (48 MiB).
///
/// This budget is smaller than [`WORKER_STACK_SIZE`]. The difference leaves room
/// for the stack that the operation uses between two checks of the guard, and
/// for the stack that is used to report the error.
pub const STACK_BUDGET: usize = 48 * 1024 * 1024;

// The budget must leave headroom below the worker stack (checked at compile time).
const _: () = assert!(STACK_BUDGET < WORKER_STACK_SIZE);

thread_local! {
    /// Whether the current thread is marked as a worker thread.
    static WORKER_THREAD: Cell<bool> = const { Cell::new(false) };
}

/// Marks the current thread as a worker thread. [`on_worker_thread`] then runs
/// guarded operations directly on this thread.
///
/// Call this function only on a thread that was created with at least
/// [`WORKER_STACK_SIZE`] bytes of stack, for instance with
/// `std::thread::Builder::stack_size` or with the corresponding setting of a
/// thread pool. On a thread with a smaller stack, a guarded operation can
/// overflow the stack before the recursion guard stops it (see [the
/// worker-thread requirement](self#the-worker-thread-requirement)).
///
/// [`run_with_worker_stack`] marks the thread that it creates, so you do not
/// need to call this function on that thread.
pub fn mark_worker_thread() {
    WORKER_THREAD.with(|marker| marker.set(true));
}

/// Returns whether the current thread has been marked as a worker thread with
/// [`mark_worker_thread`].
pub fn is_worker_thread() -> bool {
    WORKER_THREAD.with(|marker| marker.get())
}

/// Runs `f` on a worker thread and returns its result.
///
/// If the current thread is marked as a worker thread ([`is_worker_thread`]),
/// `f` runs on the current thread. Otherwise, `f` runs on a new thread created
/// with [`run_with_worker_stack`].
///
/// The entry points of the library call this function before they run a
/// guarded operation (see [the worker-thread
/// requirement](self#the-worker-thread-requirement)). Call it yourself around a
/// guarded operation that you construct with the functions of this module.
///
/// # Errors
///
/// Returns the error of the operating system if a new thread is needed and the
/// operating system fails to create it. In this case, `f` does not run.
///
/// # Panics
///
/// A panic inside `f` propagates to the caller, on whichever thread `f` ran.
pub fn on_worker_thread<T, F>(f: F) -> std::io::Result<T>
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    if is_worker_thread() {
        Ok(f())
    } else {
        run_with_worker_stack(f)
    }
}

/// The error that is reported when content is nested too deeply to be
/// processed (`flm.blocks.nesting-too-deep`).
///
/// Reported by a parse and by the rewrite passes when the recursion guard stops
/// them (see the [module documentation](self)).
///
/// A parse that reports this error returns no fragment. When a rewrite pass
/// reports this error, the tree that the pass was rewriting is absent from the
/// output of the run.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.nesting-too-deep",
    message = "Content too deeply nested: {detail}"
)]
pub struct NestingTooDeep {
    /// techy's description of the limit that was reached.
    pub detail: String,
}

/// A `GuardPolicy` is the kind of limit that the recursion guard sets on a
/// guarded operation. The default policy,
/// [`StackBudget`](GuardPolicy::StackBudget), is the policy for normal use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuardPolicy {
    /// Stops the operation once it has used more than [`STACK_BUDGET`] bytes of
    /// stack.
    #[default]
    StackBudget,
    /// Stops the operation when it would open more than the given number of
    /// descents at the same time. A parse opens about two descents for each
    /// level of nesting, and a traversal opens one descent for each level of the
    /// tree.
    ///
    /// Use this policy in tests only. It stops an operation at the same input on
    /// every platform and with every build profile, which a test needs. However,
    /// the number of descents does not measure the stack that each level uses, so
    /// this policy does not protect against a stack overflow.
    DepthLimit(usize),
}

impl GuardPolicy {
    /// Returns this policy as a [`StdDescentGuardInit`], the configuration of
    /// the descent guard that techy's constructors take.
    pub fn init(self) -> StdDescentGuardInit {
        match self {
            GuardPolicy::StackBudget => StdDescentGuardInit::fixed_stack_budget(STACK_BUDGET),
            GuardPolicy::DepthLimit(levels) => StdDescentGuardInit::depth_limit(levels),
        }
    }
}

/// Returns `language` with a recursion guard that applies `policy` to every
/// parse with this language.
///
/// [`build_language`](crate::lang::build_language) calls this function on the
/// language that it builds.
pub fn language(language: Language<Flm>, policy: GuardPolicy) -> Language<Flm> {
    language.with_descent_guard_init(policy.init())
}

/// Returns a [`TreeWalker`] for `visitor`, with a recursion guard that applies
/// `policy`.
///
/// A tree walker traverses a tree without changing it. Use this function in
/// place of `TreeWalker::new`, which installs techy's small default budget.
pub fn walker<V: ?Sized>(visitor: &mut V, policy: GuardPolicy) -> TreeWalker<'_, V> {
    TreeWalker::new(visitor).with_descent_guard_init(policy.init())
}

/// Returns a [`TreeRestager`] for `visitor`, with a recursion guard that
/// applies `policy`.
///
/// A tree restager traverses a tree and builds a new tree from it, with
/// changes applied. Use this function in place of `TreeRestager::new`, which
/// installs techy's small default budget.
pub fn restager<V: ?Sized>(visitor: &mut V, policy: GuardPolicy) -> TreeRestager<'_, V> {
    TreeRestager::new(visitor).with_descent_guard_init(policy.init())
}

/// Returns a [`TreeRecomposer`] for `recomposer`, with a recursion guard that
/// applies `policy`.
///
/// A tree recomposer traverses a tree and combines it into one value, for
/// instance a string of source text. Use this function in place of
/// `TreeRecomposer::new`, which installs techy's small default budget.
pub fn recomposer<R: ?Sized>(recomposer: &mut R, policy: GuardPolicy) -> TreeRecomposer<'_, R> {
    TreeRecomposer::new(recomposer).with_descent_guard_init(policy.init())
}

/// Returns `session` with a recursion guard that applies `policy` when the
/// session serializes or deserializes a tree.
pub fn serde_session<L: SerializableLang>(
    session: SerdeSession<L>,
    policy: GuardPolicy,
) -> SerdeSession<L> {
    session.with_descent_guard_init(policy.init())
}

/// Runs `f` on a new worker thread and returns its result.
///
/// The new thread is created with [`WORKER_STACK_SIZE`] bytes of stack, and it
/// is marked as a worker thread with [`mark_worker_thread`]. This function
/// always creates a thread. In most cases, prefer [`on_worker_thread`], which
/// creates a thread only if the current thread is not a worker thread.
///
/// # Errors
///
/// Returns the error of the operating system if the operating system fails to
/// create the thread. In this case, `f` does not run.
///
/// # Panics
///
/// A panic inside `f` is caught on the new thread and resumed on the calling
/// thread. The caller therefore observes the panic as if `f` had run on the
/// calling thread.
pub fn run_with_worker_stack<T, F>(f: F) -> std::io::Result<T>
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    thread::scope(|scope| {
        let handle = thread::Builder::new().stack_size(WORKER_STACK_SIZE).spawn_scoped(scope, || {
            mark_worker_thread();
            f()
        })?;
        match handle.join() {
            Ok(value) => Ok(value),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worker_run_returns_its_value() {
        assert_eq!(run_with_worker_stack(|| 41 + 1).unwrap(), 42);
    }

    #[test]
    fn a_worker_run_is_marked_and_the_caller_is_not() {
        assert!(!is_worker_thread());
        assert!(run_with_worker_stack(is_worker_thread).unwrap());
        assert!(!is_worker_thread());
    }

    #[test]
    fn a_marked_thread_runs_the_closure_in_place() {
        let outside = thread::current().id();
        let (ran_on, spawned) = run_with_worker_stack(|| {
            let inner = thread::current().id();
            let ran_on = on_worker_thread(|| thread::current().id()).unwrap();
            (ran_on == inner, inner != outside)
        })
        .unwrap();
        assert!(ran_on);
        assert!(spawned);
    }
}
