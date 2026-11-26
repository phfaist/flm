//! The façades of the progress handles ([`crate::api::progress`]) over
//! `flm-core`'s.
//!
//! Each façade owns the `flm-core` handle. A façade whose method ends or
//! starts its row takes the handle out, so that a handle used twice has no
//! row behind it the second time; a façade dropped with its handle inside
//! drops it, which ends the row as `flm-core`'s handle does (a row declared
//! and never started as not run, a running row as failed). A handler's
//! handles are entries of the call's table, dropped when the call ends.

use std::cell::{Cell, RefCell};

use flm_core::progress::{
    Counted as CoreCounted, Kind, Pending as CorePending, Scope as CoreScope, Task as CoreTask, Unit,
};

use crate::api::progress::{Counted, Pending, ProgressScope, Task};
use crate::sealed::{ScopeCore, Sealed, Token};

/// The façade of `flm-core`'s [`Scope`](CoreScope).
pub(crate) struct ScopeFacade(CoreScope);

impl ScopeFacade {
    /// The façade over `scope`, boxed as a handler receives it.
    pub(crate) fn boxed(scope: CoreScope) -> Box<dyn ProgressScope> {
        Box::new(ScopeFacade(scope))
    }
}

impl ScopeCore for ScopeFacade {
    fn core_scope(&self, _token: Token) -> CoreScope {
        self.0.clone()
    }
}

impl ProgressScope for ScopeFacade {
    fn child(&self, kind: &str, title: Option<String>, label: Option<String>) -> Box<dyn Pending> {
        let kind = Kind::shared(kind);
        let kind = match title {
            Some(title) => kind.with_title(title),
            None => kind,
        };
        Box::new(PendingFacade(Cell::new(Some(self.0.child(&kind, label.as_deref())))))
    }

    fn cancelled(&self) -> bool {
        self.0.cancelled()
    }
}

/// The façade of `flm-core`'s [`Pending`](CorePending): empty once started.
struct PendingFacade(Cell<Option<CorePending>>);

impl Sealed for PendingFacade {}

impl Pending for PendingFacade {
    fn start(&self) -> Box<dyn Task> {
        let task = self.0.take().map(CorePending::start);
        Box::new(TaskFacade(RefCell::new(task)))
    }

    fn start_counted(&self, unit: Unit, total: u64) -> Box<dyn Counted> {
        let counted = self.0.take().map(|pending| pending.start_counted(unit, total));
        Box::new(CountedFacade(RefCell::new(counted)))
    }
}

/// The façade of `flm-core`'s [`Task`](CoreTask): empty once finished.
struct TaskFacade(RefCell<Option<CoreTask>>);

impl Sealed for TaskFacade {}

impl Task for TaskFacade {
    fn scope(&self) -> Box<dyn ProgressScope> {
        ScopeFacade::boxed(self.0.borrow().as_ref().map_or_else(CoreScope::none, CoreTask::scope))
    }

    fn finish(&self) {
        if let Some(task) = self.0.borrow_mut().take() {
            task.finish();
        }
    }
}

/// The façade of `flm-core`'s [`Counted`](CoreCounted): empty once
/// finished.
struct CountedFacade(RefCell<Option<CoreCounted>>);

impl Sealed for CountedFacade {}

impl Counted for CountedFacade {
    fn advance(&self, n: u64) {
        if let Some(counted) = self.0.borrow().as_ref() {
            counted.advance(n);
        }
    }

    fn scope(&self) -> Box<dyn ProgressScope> {
        ScopeFacade::boxed(self.0.borrow().as_ref().map_or_else(CoreScope::none, CoreCounted::scope))
    }

    fn finish(&self) {
        if let Some(counted) = self.0.borrow_mut().take() {
            counted.finish();
        }
    }
}
