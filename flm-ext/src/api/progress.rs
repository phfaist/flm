//! Mirrors [`flm_core::progress`]: the handles through which a handler
//! reports its work on the progress sheet of a build and asks whether the
//! build was asked to stop.
//!
//! A stage's context ([`StageCx::progress`](crate::api::core::run::StageCx::progress))
//! and a citation provider's
//! ([`ProviderCx::progress`](crate::api::stdlib::features::citations::ProviderCx::progress))
//! lend the [`ProgressScope`] of the row the handler works under. Under it
//! the handler declares rows of its own ([`ProgressScope::child`]), starts
//! them ([`Pending::start`], [`Pending::start_counted`]), advances a count
//! ([`Counted::advance`]) and ends them ([`Task::finish`],
//! [`Counted::finish`]). A build that reports nowhere lends a scope with no
//! row behind it: every method then does nothing, and the handler calls them
//! all the same.
//!
//! What differs: a kind is given as its name and, optionally, its title, as
//! texts; [`ProgressScope`] is `flm-core`'s `Scope` under another name, since
//! [`Scope`](crate::api::core::stage::Scope) names the scope of a stage; the
//! handles are façades whose methods take them by reference (a handle that
//! was started or ended already has no row behind it, and does nothing). Left
//! out: the log line (an extension writes on its console), the depth limit,
//! and `start_counted_with`. Every handle is valid for the call that lent it:
//! a row the handler leaves running when the call ends ends as failed, and a
//! row it declared and never started ends as not run, as `flm-core`'s handle
//! does when it is dropped.

use crate::sealed::{ScopeCore, Sealed};

pub use flm_core::progress::Unit;

/// Mirrors [`Scope`](flm_core::progress::Scope): lent to the code that works
/// under a running row, to declare rows under it and to ask whether the
/// build was asked to stop.
///
/// What differs: the name; the log line and the depth limit are left out.
///
/// The trait is sealed: the adapter alone implements it.
pub trait ProgressScope: ScopeCore {
    /// Mirrors [`Scope::child`](flm_core::progress::Scope::child): declares a
    /// row of the kind named `kind`, of title `title`, labeled `label`, under
    /// this scope's row, and returns it, not started. What differs: the kind
    /// as its name (a dotted name, such as `math-mathjax.formulas`) and its
    /// title (the fixed text a display shows for the rows of the kind, such
    /// as "Rendering formulas").
    fn child(&self, kind: &str, title: Option<String>, label: Option<String>) -> Box<dyn Pending>;

    /// Mirrors [`Scope::cancelled`](flm_core::progress::Scope::cancelled):
    /// whether the build was asked to stop. A handler asks at the grain of
    /// its units (once for a formula, a file), and returns as soon as it can
    /// when the answer is `true`: what the step under way produces is
    /// discarded.
    fn cancelled(&self) -> bool;
}

/// Mirrors [`Pending`](flm_core::progress::Pending): a declared row, not
/// started.
///
/// What differs: the methods take the handle by reference, and a handle
/// started once has no row behind it; `with_depth_limit` and
/// `start_counted_with` are left out.
///
/// The trait is sealed: the adapter alone implements it.
pub trait Pending: Sealed {
    /// Mirrors [`Pending::start`](flm_core::progress::Pending::start): starts
    /// the row, with nothing to count.
    fn start(&self) -> Box<dyn Task>;

    /// Mirrors [`Pending::start_counted`](flm_core::progress::Pending::start_counted):
    /// starts the row with a count of `total` units of `unit`. The total
    /// never changes after the row starts.
    fn start_counted(&self, unit: Unit, total: u64) -> Box<dyn Counted>;
}

/// Mirrors [`Task`](flm_core::progress::Task): a running row with nothing
/// to count.
///
/// What differs: [`finish`](Task::finish) takes the handle by reference, and
/// a handle ended once has no row behind it.
///
/// The trait is sealed: the adapter alone implements it.
pub trait Task: Sealed {
    /// Mirrors [`Task::scope`](flm_core::progress::Task::scope): the scope of
    /// the row, for the work under it.
    fn scope(&self) -> Box<dyn ProgressScope>;

    /// Mirrors [`Task::finish`](flm_core::progress::Task::finish): ends the
    /// row as finished.
    fn finish(&self);
}

/// Mirrors [`Counted`](flm_core::progress::Counted): a running row with a
/// count.
///
/// What differs: [`finish`](Counted::finish) takes the handle by reference,
/// and a handle ended once has no row behind it.
///
/// The trait is sealed: the adapter alone implements it.
pub trait Counted: Sealed {
    /// Mirrors [`Counted::advance`](flm_core::progress::Counted::advance):
    /// adds `n` to the number done, which never goes past the total.
    fn advance(&self, n: u64);

    /// Mirrors [`Counted::scope`](flm_core::progress::Counted::scope): the
    /// scope of the row, for the work under it.
    fn scope(&self) -> Box<dyn ProgressScope>;

    /// Mirrors [`Counted::finish`](flm_core::progress::Counted::finish): ends
    /// the row as finished.
    fn finish(&self);
}

fn _object_safe(_: &dyn ProgressScope, _: &dyn Pending, _: &dyn Task, _: &dyn Counted) {}
