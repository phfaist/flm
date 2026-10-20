//! The guest runtime's state, gathered into one value ([`State`]): what
//! `define` declared, what each feature's `register` recorded, and the record
//! of the `register` running now.
//!
//! **One state per instance.** In an extension's own WebAssembly module, one
//! module is one extension, and the thread's one state is the module's.
//! Compiled into a program (the engine `native`, cargo feature `native`, on any
//! target, `wasm32` included), several extensions
//! share the guest client's code: each instance of each extension owns a
//! state of its own, which the engine installs as the thread's current state
//! (`install`) for the length of one entry, and while it drops the objects
//! the instance recorded; the previous state is restored afterwards, also
//! when the code unwinds. A nested entry (a handler of another extension, or
//! of the same one, run while a request of this one runs) installs its own
//! state over it and restores it on its way out, so every instance finds its
//! own state whenever its code runs.
//!
//! When no native instance's state is installed (in an extension's own
//! WebAssembly module, in the guest client's own tests, on a thread an
//! extension started) the current state is the thread's own, which has no
//! host.
//!
//! **The thread's end.** Compiled into a program, an extension's code can
//! still run while the thread's thread-locals are being destroyed: the `Drop`
//! of an object an instance recorded, when the engine `native` drops the
//! instances left on the thread. Once this module's thread-local is gone,
//! [`current`] answers a fresh state with no host and [`install`] changes
//! nothing: such code finds no instance's state, and a request it sends
//! answers [`ExtError::Unsupported`](crate::ExtError::Unsupported)
//! ([`crate::guest::abi`]), instead of a panic inside a thread-local's
//! destructor, which would abort the program.
//!
//! **Re-entry.** No borrow of a state's cells is held across a request
//! ([`crate::guest::abi`]'s rule): [`current`] answers the state shared, the
//! thread-local's borrow already released, and each function of the runtime
//! borrows one cell for a moment.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::guest::entry::Defined;
use crate::guest::registry::Shared;

/// The guest runtime's state on one instance.
#[derive(Default)]
pub(crate) struct State {
    /// What `define` declared.
    pub(crate) defined: RefCell<Defined>,
    /// What each feature's last `register` recorded, by package and feature.
    pub(crate) records: RefCell<BTreeMap<(String, String), Shared>>,
    /// The record of the `register` (or the finalizer) running now.
    pub(crate) running: RefCell<Option<Shared>>,
    /// The host that answers this instance's requests in the same process
    /// (the engine `native`); `None` for the thread's own state, whose
    /// requests answer `Unsupported` (with the feature `native` the module's
    /// import is not compiled).
    #[cfg(feature = "native")]
    pub(crate) host: Option<InProcess>,
}

/// A host answering a guest's requests in the same process: a request
/// (`{"native": NAME, …}`) in, its envelope out.
#[cfg(feature = "native")]
pub(crate) type InProcess = Rc<dyn Fn(&serde_json::Value) -> serde_json::Value>;

#[cfg(feature = "native")]
impl State {
    /// A fresh state whose requests `host` answers.
    pub(crate) fn answered_by(host: InProcess) -> State {
        State { host: Some(host), ..State::default() }
    }

    /// Moves what the extension's code recorded in this state (`defined`,
    /// `records`, `running`) into a new state with no host, which the caller
    /// drops when and under the current state it chooses; this state keeps
    /// its host. Each cell is borrowed for a moment only.
    pub(crate) fn take_recorded(&self) -> State {
        State {
            defined: RefCell::new(self.defined.take()),
            records: RefCell::new(self.records.take()),
            running: RefCell::new(self.running.take()),
            host: None,
        }
    }
}

thread_local! {
    /// The current state: the one an entry of the engine `native` installed,
    /// else the thread's own.
    static CURRENT: RefCell<Rc<State>> = RefCell::new(Rc::new(State::default()));
}

/// The current state, shared; the thread-local's borrow is released before
/// it is answered. Once the thread-local is destroyed (at the thread's end),
/// a fresh state with no host.
pub(crate) fn current() -> Rc<State> {
    CURRENT.try_with(|current| Rc::clone(&current.borrow())).unwrap_or_else(|_| Rc::new(State::default()))
}

/// Restores the state that was current before [`install`] when dropped.
#[cfg(feature = "native")]
#[must_use = "the state is current only until this guard drops"]
pub(crate) struct Installed {
    /// The state current before.
    previous: Option<Rc<State>>,
}

#[cfg(feature = "native")]
impl Drop for Installed {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            // The thread-local outlives the guard, unless both drop at the thread's end.
            let _ = CURRENT.try_with(|current| *current.borrow_mut() = previous);
        }
    }
}

/// Makes `state` the current state until the answered guard is dropped,
/// which restores the state current before (also when the entry unwinds).
/// Once the thread-local is destroyed (at the thread's end), changes nothing.
#[cfg(feature = "native")]
pub(crate) fn install(state: Rc<State>) -> Installed {
    // See decision rationale at [§flm-dd-dr:ext-native-engine]
    let previous = CURRENT.try_with(|current| std::mem::replace(&mut *current.borrow_mut(), state)).ok();
    Installed { previous }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;

    #[test]
    fn an_installed_state_is_current_until_its_guard_drops() {
        let own = current();
        let first = Rc::new(State::default());
        let second = Rc::new(State::default());
        {
            let _first = install(Rc::clone(&first));
            assert!(Rc::ptr_eq(&current(), &first));
            {
                let _second = install(Rc::clone(&second));
                assert!(Rc::ptr_eq(&current(), &second));
            }
            assert!(Rc::ptr_eq(&current(), &first));
        }
        assert!(Rc::ptr_eq(&current(), &own));
    }

    #[test]
    fn an_unwinding_entry_restores_the_state_before_it() {
        let own = current();
        let state = Rc::new(State::default());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _installed = install(Rc::clone(&state));
            panic!("the entry unwinds");
        }));
        assert!(caught.is_err());
        assert!(Rc::ptr_eq(&current(), &own));
    }
}
