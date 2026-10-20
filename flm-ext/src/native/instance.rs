//! The engine's side of a loaded native extension: what every instance of it
//! shares ([`NativeLoaded`], the [`Binding`]), the thread's instances of it,
//! one instance ([`NativeInstance`]), entered for one handler call (the
//! [`Instance`]), and the in-process transport of its requests. The handler
//! objects, `run_handler` and the natives' bodies are the shared ones
//! ([`crate::binding`]).
//!
//! # One handler call
//!
//! [`Binding::enter`] finds this thread's instance of the extension (creating
//! it first if needed) and runs the shared code on it; `run_handler` pushes
//! the frame and calls the entry points through [`Instance::call_entry`]:
//! the instance's guest state installed as the thread's current state, the
//! guest's entry routing called with the arguments as values, under
//! `catch_unwind`. Each request the guest sends meanwhile reaches
//! [`answer_request`] through the state's host, and from there the shared
//! natives, in the same process. At the outermost level, an instance a panic
//! poisoned is dropped.
//!
//! # Dropping an instance
//!
//! The objects an instance's guest state holds (what `define` declared, the
//! stages, rule bodies and hooks each `register` recorded) are Rust values of
//! the extension's crate, and their `Drop` is code of the extension. The
//! engine drops an instance outside its entries: after a panic, at the
//! outermost level, which may be inside another extension's request; when
//! its extension is no longer held (the sweep at the next entry on the
//! thread); when its `define` fails, or the load's `packages`; and at the
//! thread's end. So [`NativeInstance`]'s `Drop` takes those objects out of
//! the state and drops them while the instance's own state is current. A request one of them
//! sends is then answered for this extension, never for the extension whose
//! state was current; one that needs a running handler call (an API method,
//! the lending of a handler object) is refused, since no handler call of this
//! extension runs on the thread at that moment (the frames' invariant 2,
//! [`crate::binding::frames`]). At the thread's end (this module's
//! thread-local being destroyed) the objects drop under a fresh state with no
//! host instead: the shared host side's thread-locals may already be gone,
//! and a request answers [`ExtError::Unsupported`](crate::ExtError::Unsupported)
//! without reaching them. A panic in such a `Drop` is caught, so that it
//! fails no other extension's call and does not abort the program at the
//! thread's end; the panic hook has seen it (Rust's default hook prints it
//! to standard error).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{Arc, Weak};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::binding::natives::{self, Native};
use crate::binding::{inner_data, Binding, EntryArg, Instance, Registered};
use crate::engine::ExtensionIdentity;
use crate::guest::state::{self, InProcess, State};
use crate::guest::{entry, CompiledExtension};
use crate::host::ConsoleSink;
use crate::runtime::ExtError;

/// One extension the native engine loaded: what every instance of it on
/// every thread shares. Held by `Arc` from the runtime and from every
/// handler object.
pub(crate) struct NativeLoaded {
    /// From a process-wide counter, one per load
    /// ([`next_extension_id`](crate::binding::next_extension_id)): it keys the
    /// extension's instances on a thread.
    pub id: u64,
    /// The identity and the stem, for the messages and the natives.
    pub extension: ExtensionIdentity,
    /// The extension's code.
    pub compiled: &'static CompiledExtension,
    /// Where `console` output goes.
    pub console: ConsoleSink,
}

impl NativeLoaded {
    /// What the messages call the load entry `entry` (`define`, `packages`).
    fn load_entry(&self, entry: &str) -> String {
        format!("the entry `{entry}` of the extension `{}`", self.extension.stem)
    }
}

/// One instance of a native extension on this thread: the guest runtime's
/// state, the features whose `register` it ran, how many entries into it are
/// running, and whether a panic poisoned it.
pub(crate) struct NativeInstance {
    /// The guest runtime's state, installed while an entry runs; its host
    /// answers the extension's requests.
    state: Rc<State>,
    /// The features whose `register` this instance has run (for real or
    /// rehearsed).
    registered: Registered,
    /// How many entries into this instance are running on this thread.
    depth: Cell<u32>,
    /// Set when a panic ran through one of the instance's entries; the
    /// outermost call drops it on its way out.
    poisoned: Cell<bool>,
}

impl NativeInstance {
    /// Creates an instance of `extension` on this thread: a fresh guest state
    /// whose requests [`answer_request`] answers for the extension, then its
    /// entry `define(stem)` called (outside any handler call).
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: `define` panics or answers an error.
    pub(crate) fn create(extension: &NativeLoaded) -> Result<NativeInstance, ExtError> {
        let (identity, load) = (extension.extension.clone(), extension.id);
        let console = Arc::clone(&extension.console);
        let host: InProcess = Rc::new(move |request| answer_request(&identity, load, &console, request));
        let created = NativeInstance {
            state: Rc::new(State::answered_by(host)),
            registered: Registered::default(),
            depth: Cell::new(0),
            poisoned: Cell::new(false),
        };
        created.call_load_entry(extension, "define", &[EntryArg::Text(extension.extension.stem.clone())])?;
        Ok(created)
    }

    /// Calls the load entry `entry` (`define`, `packages`) with `args`, as
    /// [`Instance::call_entry`] calls a handler's entry; the guest's own
    /// error is answered as the entry's failure, "the entry `packages` of the
    /// extension `S` must answer without an error: DETAIL".
    ///
    /// # Errors
    ///
    /// The entry panics, or the guest's own error worded as above.
    pub(crate) fn call_load_entry(&self, extension: &NativeLoaded, entry: &str, args: &[EntryArg]) -> Result<Value, ExtError> {
        let what = extension.load_entry(entry);
        self.answer(extension, entry, args, &what)?.map_err(|error| ExtError::Handler {
            detail: format!("{what} must answer without an error: {error}"),
            data: inner_data(&error),
        })
    }

    /// The answer of the entry `entry` with `args`, for the call `what`: the
    /// guest's own answer (its value, or its own error), or the call's
    /// failure (the outer error: a panic). The instance's state is the
    /// thread's current state while the entry runs, and the state current
    /// before is restored afterwards.
    fn answer(
        &self,
        extension: &NativeLoaded,
        entry: &str,
        args: &[EntryArg],
        what: &str,
    ) -> Result<Result<Value, ExtError>, ExtError> {
        let values: Vec<Value> = args
            .iter()
            .map(|arg| match arg {
                EntryArg::Text(text) => Value::String(text.clone()),
                EntryArg::Json(value) => value.clone(),
            })
            .collect();
        let define = extension.compiled.define();
        // The instance's own guest state, current for the entry and restored after it. See
        // decision rationale at [§flm-dd-dr:ext-native-engine]
        let _installed = state::install(Rc::clone(&self.state));
        panic::catch_unwind(AssertUnwindSafe(|| entry::call(entry, &values, define))).map_err(|payload| {
            self.poisoned.set(true);
            let message = panic_message(payload.as_ref());
            ExtError::Handler { detail: format!("{what} must run without a panic: {message}"), data: json!({ "panic": message }) }
        })
    }
}

/// The message a panic carries: its text, or "a panic without a message"
/// when its payload is not a text.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| text.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_string())
}

/// The envelope answering the request `request` of a guest of the extension
/// `extension`, the loaded extension of number `load`: the request read as one
/// of the natives' shapes, then run by [`natives::answer`] on the running
/// call's frame, `console` receiving its `console` output. A request that
/// cannot be read answers an `err` envelope.
fn answer_request(extension: &ExtensionIdentity, load: u64, console: &ConsoleSink, request: &Value) -> Value {
    // The guest client's request answered by the dispatcher every binding shares, as a
    // value. See decision rationale at [§flm-dd-dr:ext-native-engine]
    match Native::deserialize(request) {
        Ok(native) => natives::answer(extension, load, console, native),
        Err(error) => natives::envelope(Err(ExtError::Argument {
            method: String::new(),
            detail: format!("a request of the extension `{}` must be one of the requests the host answers: {error}", extension.stem),
        })),
    }
}

/// An instance in a thread's map, with the loaded extension it is an instance
/// of, held weakly: once every holder of the extension is gone, the next
/// entry into any native instance on the thread drops it.
struct Kept {
    /// The instance.
    instance: Rc<NativeInstance>,
    /// The loaded extension.
    extension: Weak<NativeLoaded>,
}

impl Drop for NativeInstance {
    /// Drops what the extension's code recorded in the instance's state, its
    /// own state current, or at the thread's end a fresh state with no host
    /// (the module documentation, "Dropping an instance").
    fn drop(&mut self) {
        let ending = INSTANCES.try_with(|_| ()).is_err();
        let current = if ending { Rc::new(State::default()) } else { Rc::clone(&self.state) };
        let _installed = state::install(current);
        let recorded = self.state.take_recorded();
        let _ = panic::catch_unwind(AssertUnwindSafe(move || drop(recorded)));
    }
}

thread_local! {
    /// The native instances of this thread, by the loaded extension's number.
    /// Each is shared (`Rc`) between the map and the entries running on it,
    /// so the map is borrowed only for a moment and a nested entry can find
    /// or create an instance.
    static INSTANCES: RefCell<HashMap<u64, Kept>> = RefCell::new(HashMap::new());
}

/// Keeps `instance`, the one the load of `extension` created, in this
/// thread's map, so that the registry build on the loading thread uses it.
pub(crate) fn keep(extension: &Arc<NativeLoaded>, instance: NativeInstance) {
    let kept = Kept { instance: Rc::new(instance), extension: Arc::downgrade(extension) };
    INSTANCES.with(|instances| instances.borrow_mut().insert(extension.id, kept));
}

/// Leaves an instance on the way out of [`Binding::enter`]: lowers the depth,
/// and at the outermost level drops a poisoned instance.
struct Leave<'a> {
    /// The loaded extension's number.
    key: u64,
    /// The instance.
    instance: &'a Rc<NativeInstance>,
}

impl Drop for Leave<'_> {
    fn drop(&mut self) {
        let depth = self.instance.depth.get().saturating_sub(1);
        self.instance.depth.set(depth);
        if depth == 0 && self.instance.poisoned.get() {
            let removed = INSTANCES.with(|instances| {
                let mut instances = instances.borrow_mut();
                match instances.get(&self.key) {
                    Some(kept) if Rc::ptr_eq(&kept.instance, self.instance) => instances.remove(&self.key),
                    _ => None,
                }
            });
            drop(removed);
        }
    }
}

/// An instance entered for one handler call: the instance and its extension.
struct Entered<'a> {
    /// The instance.
    instance: &'a NativeInstance,
    /// Its extension.
    extension: &'a NativeLoaded,
}

impl Instance for Entered<'_> {
    fn registered(&self) -> &Registered {
        &self.instance.registered
    }

    fn call_entry(&mut self, entry: &str, args: &[EntryArg], what: &str) -> Result<Value, ExtError> {
        self.instance.answer(self.extension, entry, args, what)?
    }
}

impl Binding for NativeLoaded {
    fn identity(&self) -> &ExtensionIdentity {
        &self.extension
    }

    fn id(&self) -> u64 {
        self.id
    }

    /// Finds this thread's instance of the extension, after dropping the
    /// instances of extensions no one holds any more; creates it when the
    /// thread has none. Then runs `run` on it.
    fn enter(
        self: Arc<Self>,
        run: &mut dyn FnMut(&mut dyn Instance) -> Result<Value, ExtError>,
    ) -> Result<Value, ExtError> {
        let swept = INSTANCES.with(|instances| {
            let mut instances = instances.borrow_mut();
            let mut swept = Vec::new();
            instances.retain(|_, kept| {
                let keep = kept.extension.strong_count() > 0 || kept.instance.depth.get() > 0;
                if !keep {
                    swept.push(Rc::clone(&kept.instance));
                }
                keep
            });
            swept
        });
        // Dropped after the map's borrow ends.
        drop(swept);
        let existing = INSTANCES.with(|instances| instances.borrow().get(&self.id).map(|kept| Rc::clone(&kept.instance)));
        let instance = match existing {
            Some(instance) => instance,
            None => {
                let created = NativeInstance::create(&self).map_err(|error| ExtError::Handler {
                    detail: format!(
                        "the engine of the extension `{}` could not be created on this thread: {error}",
                        self.extension.stem
                    ),
                    data: inner_data(&error),
                })?;
                let instance = Rc::new(created);
                let kept = Kept { instance: Rc::clone(&instance), extension: Arc::downgrade(&self) };
                INSTANCES.with(|instances| instances.borrow_mut().insert(self.id, kept));
                instance
            }
        };
        instance.depth.set(instance.depth.get() + 1);
        let _leave = Leave { key: self.id, instance: &instance };
        run(&mut Entered { instance: &instance, extension: &self })
    }
}
