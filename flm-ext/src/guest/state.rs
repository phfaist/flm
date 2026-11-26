//! The guest runtime's state, gathered into one value ([`State`]): what
//! `define` declared, what each feature's `register` recorded, and the record
//! of the `register` running now; compiled into a program (the cargo feature
//! `native`), also the loaded extension the instance belongs to ([`Loaded`]:
//! its identity, its load's number and where its console output goes),
//! which the typed route reads to find the frame of the running handler
//! call and to write a line of `console` output.
//!
//! **One state per extension.** In a WebAssembly module, the instance keeps a
//! state for each extension defined in it, by the extension's stem
//! ([`defined`], [`keep`]): the entry `define` makes the state of its stem,
//! and each entry, which names its extension, installs that state as the
//! current state ([`install`]) for its length; the state current before is
//! restored afterwards, also when the code unwinds. Compiled into a program
//! (the engine `native`, cargo feature `native`, on any target, `wasm32`
//! included), several extensions share the guest client's code: each
//! instance of each extension owns a state of its own, which the engine
//! installs likewise for the length of one entry, and while it drops the
//! objects the instance recorded. A nested entry (a handler of another
//! extension, or of the same one, run while a request of this one runs)
//! installs its own state over it and restores it on its way out, so every
//! extension finds its own state whenever its code runs.
//!
//! When no entry's state is installed (outside the entries, in the guest
//! client's own tests, on a thread an extension started) the current state
//! is the thread's own, which belongs to no extension.
//!
//! **The thread's end.** Compiled into a program, an extension's code can
//! still run while the thread's thread-locals are being destroyed: the `Drop`
//! of an object an instance recorded, when the engine `native` drops the
//! instances left on the thread. Once this module's thread-local is gone,
//! [`current`] answers a fresh state of no loaded extension and [`install`]
//! changes nothing: such code finds no instance's state, and a request it sends
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

#[cfg(feature = "native")]
use crate::engine::ExtensionIdentity;
#[cfg(feature = "native")]
use crate::host::Console;
use crate::guest::entry::Defined;
use crate::guest::registry::Shared;

/// The guest runtime's state of one extension in one instance.
#[derive(Default)]
pub(crate) struct State {
    /// What `define` declared.
    pub(crate) defined: RefCell<Defined>,
    /// What each feature's last `register` recorded, by package and feature.
    pub(crate) records: RefCell<BTreeMap<(String, String), Shared>>,
    /// The record of the `register` (or the finalizer) running now.
    pub(crate) running: RefCell<Option<Shared>>,
    /// The loaded extension this instance belongs to (the engine `native`);
    /// `None` for the thread's own state, whose requests answer
    /// `Unsupported` (with the feature `native` the module's import is not
    /// compiled).
    #[cfg(feature = "native")]
    pub(crate) loaded: Option<Loaded>,
}

/// The loaded extension a native instance belongs to, as the engine `native`
/// gives it when it creates the instance: its identity, whose stem the
/// refusal of a request sent outside its handler calls names; the load's
/// number, which finds the frame of its running handler call
/// ([`frames::with_current`](crate::binding::frames::with_current)), where
/// its requests and lendings run (`abi::in_frame`); and where its `console`
/// output goes.
#[cfg(feature = "native")]
pub(crate) struct Loaded {
    /// The extension's identity.
    pub(crate) extension: ExtensionIdentity,
    /// The load's number ([`Binding::id`](crate::binding::Binding::id)).
    pub(crate) load: u64,
    /// Where the extension's `console` output goes.
    pub(crate) console: Console,
}

#[cfg(feature = "native")]
impl State {
    /// A fresh state of an instance of the loaded extension `loaded`.
    pub(crate) fn for_load(loaded: Loaded) -> State {
        State { loaded: Some(loaded), ..State::default() }
    }

    /// Moves what the extension's code recorded in this state (`defined`,
    /// `records`, `running`) into a new state of no loaded extension, which
    /// the caller drops when and under the current state it chooses; this
    /// state keeps its loaded extension. Each cell is borrowed for a moment
    /// only.
    pub(crate) fn take_recorded(&self) -> State {
        State {
            defined: RefCell::new(self.defined.take()),
            records: RefCell::new(self.records.take()),
            running: RefCell::new(self.running.take()),
            loaded: None,
        }
    }
}

thread_local! {
    /// The current state: the one an entry installed, else the thread's own.
    static CURRENT: RefCell<Rc<State>> = RefCell::new(Rc::new(State::default()));

    /// The state of each extension defined in this instance of a module, by
    /// its stem.
    static DEFINED: RefCell<BTreeMap<String, Rc<State>>> = const { RefCell::new(BTreeMap::new()) };
}

/// The current state, shared; the thread-local's borrow is released before
/// it is answered. Once the thread-local is destroyed (at the thread's end),
/// a fresh state of no loaded extension.
pub(crate) fn current() -> Rc<State> {
    CURRENT.try_with(|current| Rc::clone(&current.borrow())).unwrap_or_else(|_| Rc::new(State::default()))
}

/// The state of the extension `stem` that the entry `define` made in this
/// instance of a module, if it made one. Once the thread-local is destroyed
/// (at the thread's end), `None`.
pub(crate) fn defined(stem: &str) -> Option<Rc<State>> {
    DEFINED.try_with(|defined| defined.borrow().get(stem).cloned()).ok().flatten()
}

/// Keeps `state` as the state of the extension `stem` in this instance of a
/// module, in place of the one kept before. Once the thread-local is
/// destroyed (at the thread's end), changes nothing.
pub(crate) fn keep(stem: &str, state: Rc<State>) {
    let _ = DEFINED.try_with(|defined| defined.borrow_mut().insert(stem.to_string(), state));
}

/// Restores the state that was current before [`install`] when dropped.
#[must_use = "the state is current only until this guard drops"]
pub(crate) struct Installed {
    /// The state current before.
    previous: Option<Rc<State>>,
}

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
pub(crate) fn install(state: Rc<State>) -> Installed {
    let previous = CURRENT.try_with(|current| std::mem::replace(&mut *current.borrow_mut(), state)).ok();
    Installed { previous }
}

#[cfg(test)]
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
