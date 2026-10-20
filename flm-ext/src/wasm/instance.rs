//! The binding's side of a loaded extension: what every instance of one
//! module shares ([`WasmExtension`], the [`Binding`]), the thread's instances
//! of it, and one instance ([`WasmInstance`]), entered for one handler call
//! (the [`Instance`]). The handler objects, `run_handler` and the natives'
//! bodies are the shared ones ([`crate::binding`]).
//!
//! # One handler call
//!
//! [`Binding::enter`] finds this thread's instance of the extension (creating
//! it first if needed), gives it the call's fuel, and runs the shared code on
//! it; `run_handler` pushes the frame and calls the entry points through
//! [`Instance::call_entry`]: the request written, `flm_entry` called as a
//! resumable call, each request of the guest answered with the store free
//! (a nested handler call takes the same instance from the thread's map),
//! then the reply read. A nested entry into an instance runs with fuel of its
//! own, and the outer call's remainder is restored when it returns. At the
//! outermost level, an instance a limit or a trap poisoned, or any in strict
//! mode, is dropped.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use serde_json::{json, Value};
use wasmi::{Memory, Module, Store, TypedFunc, TypedResumableCall, Val};

use super::abi::{self, Guest, Limiter};
use super::host::{WasmHost, WasmSettings};
use crate::binding::{inner_data, Binding, EntryArg, Instance, Registered};
use crate::engine::ExtensionIdentity;
use crate::runtime::ExtError;

/// One extension the wasm engine loaded: what every instance of its module
/// on every thread shares. Held by `Arc` from the runtime and from every
/// handler object.
pub(crate) struct WasmExtension {
    /// From a process-wide counter, one per load
    /// ([`next_extension_id`](crate::binding::next_extension_id)): with the
    /// host's, it keys the extension's instances on a thread.
    pub id: u64,
    /// The host the extension was loaded with.
    pub host: WasmHost,
    /// The settings every instance of the extension is created with: the
    /// host's, with the limits of the extension's manifest
    /// ([`WasmSettings::with_limits`]).
    pub settings: WasmSettings,
    /// The identity and the stem, for the messages.
    pub extension: ExtensionIdentity,
    /// The module's normalized path (`wordcount.wasm`), for the messages.
    pub path: String,
    /// The engine every instance runs on.
    pub engine: wasmi::Engine,
    /// The module, compiled once at load.
    pub module: Module,
    /// The module's initial memory, in bytes.
    pub initial_memory: u64,
}

impl WasmExtension {
    /// The fuel of one handler call: the setting, or as much as wasmi counts
    /// when it is unlimited.
    fn fuel(&self) -> u64 {
        self.settings.fuel.unwrap_or(u64::MAX)
    }

    /// What the messages call the load entry `entry` (`define`, `packages`).
    fn load_entry(&self, entry: &str) -> String {
        format!("the entry `{entry}` of the module `{}` of the extension `{}`", self.path, self.extension.stem)
    }
}

/// Why a call into an instance stopped: the fuel ran out, a trap (with its
/// text), or a buffer the guest gave outside its memory (with the failure's
/// text).
enum Stop {
    /// The fuel ran out.
    Fuel,
    /// A trap, or a host function's failure.
    Trap(String),
    /// A buffer the guest answered lies outside its memory.
    Outside(String),
}

impl From<wasmi::Error> for Stop {
    fn from(error: wasmi::Error) -> Stop {
        match error.as_trap_code() {
            Some(wasmi::TrapCode::OutOfFuel) => Stop::Fuel,
            _ => Stop::Trap(error.to_string()),
        }
    }
}

/// One instance of a module on this thread: the store, the exports the
/// binding calls, the features whose `register` it ran, how many entries
/// into it are running, and whether a limit or a trap poisoned it.
pub(crate) struct WasmInstance {
    /// The store. Borrowed only while the guest runs or its memory is read
    /// or written, never while a request of the guest runs (a nested handler
    /// call borrows it again).
    store: RefCell<Store<Guest>>,
    /// The export `memory`.
    memory: Memory,
    /// The export `flm_entry`.
    entry: TypedFunc<(i32, i32), i64>,
    /// The export `flm_alloc`.
    alloc: TypedFunc<i32, i32>,
    /// The export `flm_free`.
    free: TypedFunc<(i32, i32), ()>,
    /// The export `flm_last_panic`, when the module has it.
    last_panic: Option<TypedFunc<(), i64>>,
    /// The features whose `register` this instance has run (for real or
    /// rehearsed).
    registered: Registered,
    /// How many entries into this instance are running on this thread.
    depth: Cell<u32>,
    /// Set when the instance must not be used again (the fuel ran out, the
    /// memory limit was reached, a trap); the outermost call drops it on its
    /// way out.
    poisoned: Cell<bool>,
}

impl WasmInstance {
    /// Creates an instance of `extension`'s module on this thread: the store
    /// with the memory limit and one call's fuel, the module instantiated
    /// with the two host functions, its exports found, then its entry
    /// `define(stem, module)` called (outside any handler call) with one
    /// call's fuel of its own ([`WasmInstance::call_load_entry`]).
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: the module's initial memory is above the
    /// limit, or it cannot be instantiated, or `define` fails or answers an
    /// error.
    pub(crate) fn create(extension: &WasmExtension) -> Result<WasmInstance, ExtError> {
        let stem = &extension.extension.stem;
        let failed = |detail: String| ExtError::Handler {
            detail: format!("the engine of the extension `{stem}` could not start: {detail}"),
            data: Value::Null,
        };
        let limit = extension.settings.memory_limit;
        if let Some(limit) = limit.filter(|limit| extension.initial_memory > *limit as u64) {
            return Err(ExtError::Handler {
                detail: format!(
                    "the initial memory of the module `{}` of the extension `{stem}` must be at most {limit} bytes, its \
                     memory limit",
                    extension.path
                ),
                data: json!({ "limit": "memory", "limit_bytes": limit }),
            });
        }
        let guest = Guest { limiter: Limiter { limit, refused: false }, request: None, reply: None };
        let mut store = Store::new(&extension.engine, guest);
        store.limiter(|guest| &mut guest.limiter);
        store.set_fuel(extension.fuel()).map_err(|error| failed(error.to_string()))?;
        let linker = abi::linker(&extension.engine).map_err(failed)?;
        let instance = linker.instantiate_and_start(&mut store, &extension.module).map_err(|error| failed(error.to_string()))?;
        let memory = instance.get_memory(&store, "memory").ok_or_else(|| failed("the module exports no memory `memory`".into()))?;
        let entry = instance.get_typed_func(&store, "flm_entry").map_err(|error| failed(error.to_string()))?;
        let alloc = instance.get_typed_func(&store, "flm_alloc").map_err(|error| failed(error.to_string()))?;
        let free = instance.get_typed_func(&store, "flm_free").map_err(|error| failed(error.to_string()))?;
        let last_panic = instance.get_typed_func(&store, "flm_last_panic").ok();
        let created = WasmInstance {
            store: RefCell::new(store),
            memory,
            entry,
            alloc,
            free,
            last_panic,
            registered: Registered::default(),
            depth: Cell::new(0),
            poisoned: Cell::new(false),
        };
        let args = [EntryArg::Text(stem.clone()), EntryArg::Text(extension.path.clone())];
        created.call_load_entry(extension, "define", &args)?;
        Ok(created)
    }

    /// The fuel left in the store. (Fuel metering is always on: wasmi
    /// answers an error only when it is off.)
    fn fuel(&self) -> u64 {
        self.store.borrow().get_fuel().unwrap_or(0)
    }

    /// Sets the fuel left in the store.
    fn set_fuel(&self, fuel: u64) {
        // Fuel metering is always on: wasmi refuses only when it is off.
        let _ = self.store.borrow_mut().set_fuel(fuel);
    }

    /// Calls the entry point `entry` of the guest with `args`, for the
    /// handler call `what`: the request written into a buffer of the guest's,
    /// `flm_entry` driven to its end (each request of the guest answered),
    /// the buffer freed, the reply read and freed.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: the fuel ran out, the memory limit was reached,
    /// a trap; the reply is not of the ABI's shape; the guest's own error
    /// (read by [`ExtError::from_data`]); [`ExtError::Value`]: the reply is
    /// not JSON.
    pub(crate) fn call_entry(
        &self,
        extension: &WasmExtension,
        entry: &str,
        args: &[EntryArg],
        what: &str,
    ) -> Result<Value, ExtError> {
        self.answer(extension, entry, args, what)?
    }

    /// Calls the load entry `entry` (`define`, `packages`) of the guest with
    /// `args`, with one call's fuel, as [`WasmInstance::call_entry`] calls a
    /// handler's entry; the guest's own error is answered as the entry's
    /// failure, "the entry `packages` of the module `P` of the extension `S`
    /// must answer without an error: DETAIL".
    ///
    /// # Errors
    ///
    /// [`WasmInstance::call_entry`]'s, the guest's own error worded as above.
    pub(crate) fn call_load_entry(&self, extension: &WasmExtension, entry: &str, args: &[EntryArg]) -> Result<Value, ExtError> {
        let what = extension.load_entry(entry);
        self.set_fuel(extension.fuel());
        self.answer(extension, entry, args, &what)?.map_err(|error| ExtError::Handler {
            detail: format!("{what} must answer without an error: {error}"),
            data: inner_data(&error),
        })
    }

    /// The answer of the entry `entry` with `args`, for the call `what`: the
    /// guest's own answer (its value, or its own error), or the call's
    /// failure (the outer error: a limit, a trap, a reply not of the ABI's
    /// shape).
    fn answer(
        &self,
        extension: &WasmExtension,
        entry: &str,
        args: &[EntryArg],
        what: &str,
    ) -> Result<Result<Value, ExtError>, ExtError> {
        let request = abi::entry_request(entry, args);
        let reply = self.run(extension, request.as_bytes(), what).map_err(|stop| self.fail(extension, what, stop))?;
        abi::read_reply(&reply, what)
    }

    /// Runs one entry: the request `bytes` written, the call driven, the
    /// reply's bytes read; both buffers freed.
    fn run(&self, extension: &WasmExtension, bytes: &[u8], what: &str) -> Result<Vec<u8>, Stop> {
        let length =
            i32::try_from(bytes.len()).map_err(|_| Stop::Outside(format!("the request of {what} must be below 2 GiB")))?;
        let at = {
            let mut store = self.store.borrow_mut();
            let at = self.alloc.call(&mut *store, length)?;
            self.memory
                .write(&mut *store, at as u32 as usize, bytes)
                .map_err(|_| Stop::Outside(format!("the buffer `flm_alloc` answered for {what} must lie within the module's memory")))?;
            at
        };
        let packed = self.drive(extension, at, length)?;
        let mut store = self.store.borrow_mut();
        self.free.call(&mut *store, (at, length))?;
        let (start, size) = ((packed as u64 >> 32) as usize, (packed as u64 & 0xffff_ffff) as usize);
        let reply = self
            .memory
            .data(&*store)
            .get(start..start.saturating_add(size))
            .ok_or_else(|| Stop::Outside(format!("the answer of {what} must lie within the module's memory")))?
            .to_vec();
        self.free.call(&mut *store, (start as i32, size as i32))?;
        Ok(reply)
    }

    /// Calls `flm_entry(at, length)` as a resumable call and drives it to its
    /// end: each time the guest suspends it with a request (`flm.call`), the
    /// store is released, the request answered (a nested handler call may
    /// enter this instance meanwhile), the reply kept for `flm.take`, and the
    /// call resumed with the reply's length. Answers the packed reply.
    fn drive(&self, extension: &WasmExtension, at: i32, length: i32) -> Result<i64, Stop> {
        let mut step = {
            let mut store = self.store.borrow_mut();
            store.data_mut().request = None;
            self.entry.call_resumable(&mut *store, (at, length))
        };
        loop {
            match step? {
                TypedResumableCall::Finished(packed) => return Ok(packed),
                TypedResumableCall::OutOfFuel(_) => return Err(Stop::Fuel),
                TypedResumableCall::HostTrap(suspended) => {
                    let request = self.store.borrow_mut().data_mut().request.take();
                    let Some(request) = request else {
                        return Err(Stop::Trap(suspended.host_error().to_string()));
                    };
                    let reply = abi::answer_request(extension, &request);
                    let size = i32::try_from(reply.len()).unwrap_or(i32::MAX);
                    let mut store = self.store.borrow_mut();
                    store.data_mut().reply = Some(reply);
                    step = suspended.resume(&mut *store, &[Val::I32(size)]);
                }
            }
        }
    }

    /// The failure of the handler call `what` stopped by `stop`, its text and
    /// data; the instance poisoned (a guest that answers a buffer outside its
    /// memory is not trusted further either). A trap after a refused growth is the
    /// memory limit; after any other trap, the guest's panic message, if
    /// `flm_last_panic` answers one, is appended.
    fn fail(&self, extension: &WasmExtension, what: &str, stop: Stop) -> ExtError {
        self.poisoned.set(true);
        match stop {
            Stop::Fuel => {
                let fuel = extension.fuel();
                ExtError::Handler {
                    detail: format!("{what} must finish within its budget of {fuel} fuel units"),
                    data: json!({ "limit": "fuel", "fuel": fuel }),
                }
            }
            Stop::Outside(detail) => ExtError::Handler { detail, data: Value::Null },
            Stop::Trap(_) if self.store.borrow().data().limiter.refused => {
                let limit = extension.settings.memory_limit.unwrap_or(0);
                ExtError::Handler {
                    detail: format!("{what} must stay within the memory limit of {limit} bytes"),
                    data: json!({ "limit": "memory", "limit_bytes": limit }),
                }
            }
            Stop::Trap(trap) => {
                let panic = self.panic_message(extension);
                let mut detail = format!("{what} must run without a trap: {trap}");
                if let Some(message) = &panic {
                    detail.push_str(&format!("; the guest panicked: {message}"));
                }
                ExtError::Handler { detail, data: json!({ "trap": trap, "panic": panic }) }
            }
        }
    }

    /// The guest's last panic message, from `flm_last_panic` (with one call's
    /// fuel); `None` when the module lacks it, answers `0`, or fails.
    fn panic_message(&self, extension: &WasmExtension) -> Option<String> {
        let last_panic = self.last_panic.as_ref()?;
        let mut store = self.store.borrow_mut();
        store.set_fuel(extension.fuel()).ok()?;
        let packed = last_panic.call(&mut *store, ()).ok().filter(|packed| *packed != 0)?;
        let (start, size) = ((packed as u64 >> 32) as usize, (packed as u64 & 0xffff_ffff) as usize);
        let bytes = self.memory.data(&*store).get(start..start.saturating_add(size))?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// The key of an instance on a thread: one per host, per loaded extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InstanceKey {
    /// The host's identity.
    host: u64,
    /// The loaded extension's number ([`WasmExtension::id`]).
    extension: u64,
}

/// An instance in a thread's map, with the loaded extension it is an instance
/// of, held weakly: once every holder of the extension is gone, the next
/// entry into any instance on the thread drops it.
struct Kept {
    /// The instance.
    instance: Rc<WasmInstance>,
    /// The loaded extension.
    extension: Weak<WasmExtension>,
}

thread_local! {
    /// The instances of this thread. Each is shared (`Rc`) between the map
    /// and the entries running on it, so the map is borrowed only for a
    /// moment and a nested entry can find or create an instance.
    static INSTANCES: RefCell<HashMap<InstanceKey, Kept>> = RefCell::new(HashMap::new());
}

/// Keeps `instance`, the one the load of `extension` created, in this
/// thread's map, so that the registry build on the loading thread uses it.
pub(crate) fn keep(extension: &Arc<WasmExtension>, instance: WasmInstance) {
    let key = InstanceKey { host: extension.host.id(), extension: extension.id };
    let kept = Kept { instance: Rc::new(instance), extension: Arc::downgrade(extension) };
    INSTANCES.with(|instances| instances.borrow_mut().insert(key, kept));
}

/// Drops every instance of the host `host` on this thread.
pub(crate) fn discard_host(host: u64) {
    let discarded: Vec<Kept> = INSTANCES.with(|instances| {
        let mut instances = instances.borrow_mut();
        let keys: Vec<InstanceKey> = instances.keys().filter(|key| key.host == host).copied().collect();
        keys.into_iter().filter_map(|key| instances.remove(&key)).collect()
    });
    drop(discarded);
}

/// How many instances of the host `host` this thread's map holds.
pub(crate) fn count_host(host: u64) -> usize {
    INSTANCES.with(|instances| instances.borrow().keys().filter(|key| key.host == host).count())
}

/// Leaves an instance on the way out of [`Binding::enter`]: restores the
/// outer call's fuel (a nested entry), lowers the depth, and at the outermost
/// level drops a poisoned instance, or any in strict mode.
struct Leave<'a> {
    /// The instance's key.
    key: InstanceKey,
    /// The instance.
    instance: &'a Rc<WasmInstance>,
    /// Strict mode.
    strict: bool,
    /// The outer call's fuel, for a nested entry.
    outer: Option<u64>,
}

impl Drop for Leave<'_> {
    fn drop(&mut self) {
        if let (Some(fuel), Ok(mut store)) = (self.outer, self.instance.store.try_borrow_mut()) {
            // Fuel metering is always on: wasmi refuses only when it is off.
            let _ = store.set_fuel(fuel);
        }
        let depth = self.instance.depth.get().saturating_sub(1);
        self.instance.depth.set(depth);
        if depth == 0 && (self.strict || self.instance.poisoned.get()) {
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
    instance: &'a WasmInstance,
    /// Its extension.
    extension: &'a WasmExtension,
}

impl Instance for Entered<'_> {
    fn registered(&self) -> &Registered {
        &self.instance.registered
    }

    fn call_entry(&mut self, entry: &str, args: &[EntryArg], what: &str) -> Result<Value, ExtError> {
        self.instance.call_entry(self.extension, entry, args, what)
    }
}

impl Binding for WasmExtension {
    fn identity(&self) -> &ExtensionIdentity {
        &self.extension
    }

    fn id(&self) -> u64 {
        self.id
    }

    /// Finds this thread's instance of the extension, after dropping the
    /// instances of extensions no one holds any more; creates it when the
    /// thread has none, or a fresh one in strict mode, unless an entry into
    /// it runs. Gives the call its fuel (a nested entry keeps the outer call's
    /// remainder, restored on the way out); at the outermost entry, clears
    /// the memory limit's mark. Then runs `run` on it.
    fn enter(
        self: Arc<Self>,
        run: &mut dyn FnMut(&mut dyn Instance) -> Result<Value, ExtError>,
    ) -> Result<Value, ExtError> {
        let key = InstanceKey { host: self.host.id(), extension: self.id };
        let strict = self.settings.strict;
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
        let existing = INSTANCES.with(|instances| instances.borrow().get(&key).map(|kept| Rc::clone(&kept.instance)));
        let instance = match existing {
            Some(instance) if instance.depth.get() > 0 || !strict => instance,
            _ => {
                let created = WasmInstance::create(&self).map_err(|error| ExtError::Handler {
                    detail: format!(
                        "the engine of the extension `{}` could not be created on this thread: {error}",
                        self.extension.stem
                    ),
                    data: inner_data(&error),
                })?;
                let instance = Rc::new(created);
                let kept = Kept { instance: Rc::clone(&instance), extension: Arc::downgrade(&self) };
                INSTANCES.with(|instances| instances.borrow_mut().insert(key, kept));
                instance
            }
        };
        let outer = if instance.depth.get() > 0 {
            Some(instance.fuel())
        } else {
            instance.store.borrow_mut().data_mut().limiter.refused = false;
            None
        };
        instance.set_fuel(self.fuel());
        instance.depth.set(instance.depth.get() + 1);
        let _leave = Leave { key, instance: &instance, strict, outer };
        run(&mut Entered { instance: &instance, extension: &self })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An extension of stem `t` over the module `text`, whose instances may
    /// have `memory_limit` bytes.
    fn extension(text: &str, memory_limit: usize) -> Arc<WasmExtension> {
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = wasmi::Engine::new(&config);
        let module = Module::new(&engine, wat::parse_str(text).expect("the text parses")).expect("the module compiles");
        let settings = WasmSettings { memory_limit: Some(memory_limit), ..WasmSettings::default() };
        Arc::new(WasmExtension {
            id: crate::binding::next_extension_id(),
            host: WasmHost::new(settings.clone()),
            settings,
            extension: ExtensionIdentity { identity: "t".to_string(), stem: "t".to_string() },
            path: "m.wasm".to_string(),
            engine,
            module,
            initial_memory: 2 << 16,
        })
    }

    #[test]
    fn an_instance_that_cannot_be_created_on_a_thread_fails_the_handler_call() {
        let extension = extension(r#"(module (memory (export "memory") 2))"#, 1 << 16);
        let error = Arc::clone(&extension).enter(&mut |_| Ok(Value::Null)).expect_err("the instance cannot be created");
        let inner = ExtError::Handler {
            detail: "the initial memory of the module `m.wasm` of the extension `t` must be at most 65536 bytes, its \
                     memory limit"
                .to_string(),
            data: json!({ "limit": "memory", "limit_bytes": 65536 }),
        };
        assert_eq!(
            error,
            ExtError::Handler {
                detail: format!("the engine of the extension `t` could not be created on this thread: {inner}"),
                data: json!({ "limit": "memory", "limit_bytes": 65536 }),
            }
        );
        assert_eq!(extension.host.instances_on_this_thread(), 0);
    }
}
