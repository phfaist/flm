//! The recursion guard, which refuses deeply nested content instead of
//! overflowing the stack, and the worker threads every guarded operation runs
//! on.
//!
//! Parsing, walking, rewriting and serializing a tree are recursive, so deeply
//! nested input uses stack. techy's **descent guard** measures how much stack a
//! recursive operation has used (the distance from a reference point taken when
//! the operation starts) and refuses to descend one level further once a
//! budget is spent; the library then reports the condition
//! `flm.blocks.nesting-too-deep`. Without the guard, deeply nested input would
//! overflow the stack, which ends the process; no embedding program can
//! recover from that. See
//! [the recursion guard and worker
//! threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads).
//!
//! # Guarded construction
//!
//! techy's default budget is small, and it cannot be changed on a traversal or
//! a language once constructed, so the library constructs every recursive
//! operation through a function of this module, which sets the budget
//! [`STACK_BUDGET`]: [`language`] (parsing), [`walker`], [`restager`] and
//! [`recomposer`] (traversals), and [`serde_session`] (serialization). Each
//! takes a [`GuardPolicy`]. The stack budget is the policy for normal use; a
//! limit on the number of open descents is the policy for tests, because it
//! refuses at the same input on every platform. A count of descents does not
//! measure how much stack each level uses, so it gives no protection and is
//! not used outside tests.
//!
//! # The worker-thread requirement
//!
//! The budget limits how much stack a guarded operation *may use*; it does not
//! check how much stack the thread has. On a thread with less stack than the
//! budget, a guarded operation would overflow before the guard refused. A
//! guarded operation must therefore run on a **worker thread**: a thread created
//! with at least [`WORKER_STACK_SIZE`] bytes of stack.
//!
//! The library meets this requirement for its callers. A thread-local marker
//! records that the current thread is a worker thread ([`mark_worker_thread`];
//! [`run_with_worker_stack`] sets it on the thread it creates). Before a guarded
//! operation, the library's entry points call [`on_worker_thread`], which runs
//! the work on the current thread if it is marked and otherwise on a new worker
//! thread. The [parse entry points](crate::lang::parse), the [rewrite
//! passes](crate::lang::rewrites), the run of a plan
//! ([`Run`](crate::core::run::Run)) and the emitters of the standard backends
//! do so, so an embedding program does not need to know about this
//! requirement. When a run uses a thread pool, the pool's threads are created
//! with the worker stack size and marked as they start, so no further thread
//! is created on that path.
//!
//! A caller that runs a guarded operation that does not make this check itself
//! (one built with the functions of this module, or
//! [`copy_run_into_value`](crate::lang::copy::copy_run_into_value)) must run it
//! inside [`on_worker_thread`].
//!
//! # A refusal is never a panic
//!
//! When the guard refuses to descend, the operation produces the condition
//! [`NestingTooDeep`] (`flm.blocks.nesting-too-deep`) and no tree: a parse
//! returns [`FragmentError::NestingTooDeep`](crate::lang::FragmentError::NestingTooDeep),
//! and a rewrite pass produces nothing for the tree it was working on and
//! records the diagnostic instead.

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

/// The stack size, in bytes, of a worker thread: 64 MiB. Every guarded
/// operation may assume that this much stack is available.
///
/// A thread marked with [`mark_worker_thread`] must have at least this much
/// stack; see [the worker-thread
/// requirement](self#the-worker-thread-requirement).
pub const WORKER_STACK_SIZE: usize = 64 * 1024 * 1024;

/// The number of bytes of stack a guarded operation may use before it refuses
/// to descend further: 48 MiB.
///
/// It is smaller than [`WORKER_STACK_SIZE`], because the stack used between two
/// checks of the guard, and by the refusal itself, must still fit on the worker
/// thread.
pub const STACK_BUDGET: usize = 48 * 1024 * 1024;

// The budget must leave headroom below the worker stack (checked at compile time).
const _: () = assert!(STACK_BUDGET < WORKER_STACK_SIZE);

thread_local! {
    /// Whether the current thread is known to carry a worker stack.
    static WORKER_THREAD: Cell<bool> = const { Cell::new(false) };
}

/// Marks the current thread as a worker thread: a thread on which a guarded
/// operation may run directly.
///
/// Call this **only** on a thread created with a stack of at least
/// [`WORKER_STACK_SIZE`] bytes (`std::thread::Builder::stack_size`, or a thread
/// pool's equivalent). A run's thread pool calls it once in each thread, as
/// the thread starts.
///
/// Marking a thread with a smaller stack allows the stack overflow that [the
/// worker-thread requirement](self#the-worker-thread-requirement) prevents,
/// because the guard's budget limits how much stack an operation uses and does
/// not check how much the thread has. [`run_with_worker_stack`] marks the
/// thread it creates, so its callers need not call this function.
pub fn mark_worker_thread() {
    WORKER_THREAD.with(|marker| marker.set(true));
}

/// Returns whether the current thread has been marked as a worker thread with
/// [`mark_worker_thread`].
pub fn is_worker_thread() -> bool {
    WORKER_THREAD.with(|marker| marker.get())
}

/// Runs `f` on a worker thread and returns its result: on the current thread
/// when it is already one ([`is_worker_thread`]), and otherwise on a new
/// thread created through [`run_with_worker_stack`].
///
/// A worker thread is a thread created with at least [`WORKER_STACK_SIZE`]
/// bytes of stack; every guarded operation (a parse, a rewrite, an emit) must
/// execute on one, and the library's entry points create one when the calling
/// thread is not marked as a worker thread. The library's guarded entry points
/// call this function for their callers; call it yourself around any guarded
/// operation you build with this module's functions.
///
/// # Errors
///
/// The operating system's error when it refuses to create the thread. `f` did
/// not run in that case.
///
/// # Panics
///
/// A panic inside `f` propagates to the caller, whichever thread `f` ran on.
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

/// Condition `flm.blocks.nesting-too-deep`: the recursion guard refused to
/// descend one level further, because the content is nested too deeply.
///
/// The parse, the rewrite passes and the other guarded operations report this
/// same condition, so an embedding program has one identifier to recognize.
///
/// Reported at error severity. Recovery: none. A parse that hits the limit
/// returns no fragment; a rewrite pass that hits it produces no tree for the
/// tree it was working on. This diagnostic is the only record of why.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.nesting-too-deep",
    message = "the content nests too deeply to process: {detail}"
)]
pub struct NestingTooDeep {
    /// techy's description of the limit that was reached.
    pub detail: String,
}

/// How the recursion guard limits a guarded operation: by stack use (for
/// normal use) or by the number of open descents (for tests). The [module
/// documentation](self#guarded-construction) explains why there are two
/// policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuardPolicy {
    /// Refuse to descend once the operation has used [`STACK_BUDGET`] bytes of
    /// stack. This is the policy for normal use, and the default.
    #[default]
    StackBudget,
    /// Refuse to descend once this many descents are open at the same time
    /// (roughly two per nesting level of a parse, one per tree level of a
    /// traversal).
    ///
    /// A count of descents does not measure how much stack each level uses, so
    /// it does not protect against a stack overflow. It exists because it
    /// refuses at the same input on every platform and build profile, which a
    /// test needs. Use it in tests only.
    DepthLimit(usize),
}

impl GuardPolicy {
    /// Returns this policy as techy's descent-guard configuration, which
    /// techy's constructors take.
    pub fn init(self) -> StdDescentGuardInit {
        match self {
            GuardPolicy::StackBudget => StdDescentGuardInit::fixed_stack_budget(STACK_BUDGET),
            GuardPolicy::DepthLimit(levels) => StdDescentGuardInit::depth_limit(levels),
        }
    }
}

/// Returns `language` with the recursion guard of `policy` installed, so that
/// every parse it runs is guarded.
/// [`build_language`](crate::lang::build_language) applies this to the language
/// it builds.
pub fn language(language: Language<Flm>, policy: GuardPolicy) -> Language<Flm> {
    language.with_descent_guard_init(policy.init())
}

/// Returns a tree walker (a read-only traversal) that drives `visitor` under
/// the recursion guard of `policy`. Use it instead of techy's constructor.
pub fn walker<V: ?Sized>(visitor: &mut V, policy: GuardPolicy) -> TreeWalker<'_, V> {
    TreeWalker::new(visitor).with_descent_guard_init(policy.init())
}

/// Returns a tree restager (a traversal that builds a new tree as it goes) that
/// drives `visitor` under the recursion guard of `policy`. Use it instead of
/// techy's constructor.
pub fn restager<V: ?Sized>(visitor: &mut V, policy: GuardPolicy) -> TreeRestager<'_, V> {
    TreeRestager::new(visitor).with_descent_guard_init(policy.init())
}

/// Returns a tree recomposer (a traversal that rebuilds a tree with edits
/// applied) that drives `recomposer` under the recursion guard of `policy`. Use
/// it instead of techy's constructor.
pub fn recomposer<R: ?Sized>(recomposer: &mut R, policy: GuardPolicy) -> TreeRecomposer<'_, R> {
    TreeRecomposer::new(recomposer).with_descent_guard_init(policy.init())
}

/// Returns `session` with the recursion guard of `policy` installed, so that
/// reading or writing a deeply nested tree is refused instead of overflowing
/// the stack.
pub fn serde_session<L: SerializableLang>(
    session: SerdeSession<L>,
    policy: GuardPolicy,
) -> SerdeSession<L> {
    session.with_descent_guard_init(policy.init())
}

/// Runs `f` on a new thread created with a stack of [`WORKER_STACK_SIZE`]
/// bytes and marked as a worker thread ([`mark_worker_thread`]), and returns its
/// result.
///
/// This meets [the worker-thread
/// requirement](self#the-worker-thread-requirement) from a thread whose stack
/// size is unknown (a test thread, the main thread of a program). It always
/// creates a thread; [`on_worker_thread`] creates one only when the current
/// thread is not a worker thread, and is usually the better choice.
///
/// # Errors
///
/// The operating system's error when it refuses to create the thread. `f` did
/// not run in that case.
///
/// # Panics
///
/// A panic inside `f` is caught on the worker thread and resumed on the calling
/// thread, so it reaches the caller as if `f` had run there.
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
