//! The binding's side of a loaded extension: the module its instances are
//! created from ([`WasmModule`], one per extension found in a folder, one per
//! collection), what every instance of one extension shares
//! ([`WasmExtension`], the [`Binding`]), the thread's instances of a module,
//! and one instance ([`WasmInstance`]), entered for one handler call (the
//! [`Instance`]). The handler objects, `run_handler` and the natives' bodies
//! are the shared ones ([`crate::binding`]).
//!
//! # One handler call
//!
//! [`Binding::enter`] finds this thread's instance of the extension's module
//! (creating it first if needed), defines the extension in it when the
//! instance has not defined it for this load, gives it the call's fuel, and
//! runs the shared code on it; `run_handler` pushes the frame and calls the
//! entry points through [`Instance::call_entry`]: the request written,
//! `flm_entry` called as a resumable call, each request of the guest answered
//! with the store free (a nested handler call takes the same instance from
//! the thread's map), then the reply read. A nested entry into an instance
//! runs with fuel of its own, and the outer call's remainder is restored when
//! it returns. At the outermost level, an instance a limit or a trap
//! poisoned, or any in strict mode, is dropped.
//!
//! # The extensions of one instance
//!
//! The instances of a module are keyed, on a thread, by the host and the
//! module, so that every extension of a collection runs in one instance. An
//! instance keeps, for each stem it defined, the load it defined it for and
//! the features whose `register` it ran for that extension ([`Registered`]):
//! an extension is defined in an instance when it is first called there, and
//! defined again when another load of the same stem is called. The memory
//! limit of an instance is that of the extension whose call creates it
//! ([`WasmExtension::settings`]): for an extension of a collection, the
//! limit of the collection's instances, the same for each of its extensions.
//!
//! # A cancelled build
//!
//! A stage's or a citation provider's call runs under the scope of its row of
//! the build's progress sheet. The call asks whether that build is cancelled
//! at each request of the guest, before answering it. When it is, the call
//! fails and the instance is dropped, as when the fuel runs out. Between two
//! requests the guest runs on: its fuel is given to it whole.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use flm_core::progress::Scope;
use serde_json::{json, Value};
use wasmi::{ExternType, Memory, Module, Store, TypedFunc, TypedResumableCall, Val};

use super::abi::{self, Guest, Limiter};
use super::host::{WasmHost, WasmSettings, WeakHost};
use crate::binding::{inner_data, next_extension_id, Binding, EntryArg, Instance, Registered, Reply};
use crate::engine::ExtensionIdentity;
use crate::runtime::ExtError;

/// What every instance of one module, on every thread, is created from: the
/// compiled module and the engine it is compiled for. One per load of an
/// extension found in a folder, one per collection
/// ([`Collection`](super::Collection)).
pub(crate) struct WasmModule {
    /// From the process-wide counter of loads ([`next_extension_id`]): with
    /// the host's identity, it keys the module's instances on a thread.
    pub id: u64,
    /// The engine every instance runs on.
    pub engine: wasmi::Engine,
    /// The module, compiled once.
    pub module: Module,
    /// The module's initial memory, in bytes.
    pub initial_memory: u64,
}

impl WasmModule {
    /// The module `module`, compiled for `engine`, from which no extension
    /// is loaded yet.
    pub(crate) fn new(engine: wasmi::Engine, module: Module) -> WasmModule {
        let initial_memory = match module.get_export("memory") {
            Some(ExternType::Memory(memory)) => memory.minimum().saturating_mul(1 << 16),
            _ => 0,
        };
        WasmModule { id: next_extension_id(), engine, module, initial_memory }
    }
}

/// Where an extension's module comes from, for the messages and the entry
/// `define`.
#[derive(Debug, Clone)]
pub(crate) enum ModuleOrigin {
    /// The module at this path of the extension's folder, as the manifest
    /// writes it (`./wordcount.wasm`).
    Folder(String),
    /// The module of the collection of this label.
    Collection(String),
}

impl ModuleOrigin {
    /// The text the entry `define` receives as its module: the path, or the
    /// collection's label.
    fn text(&self) -> &str {
        match self {
            ModuleOrigin::Folder(path) => path,
            ModuleOrigin::Collection(label) => label,
        }
    }
}

/// One extension the wasm engine loaded: what every instance of it on every
/// thread shares. Held by `Arc` from the runtime and from every handler
/// object.
pub(crate) struct WasmExtension {
    /// From a process-wide counter, one per load
    /// ([`next_extension_id`]): an instance defines the extension again
    /// when another load of its stem is called in it.
    pub id: u64,
    /// The host the extension was loaded with.
    pub host: WasmHost,
    /// The settings of the extension: the host's, with the limits of the
    /// extension's manifest ([`WasmSettings::with_limits`]). For an extension
    /// of a collection, the memory limit is that of the collection's
    /// instances ([`WasmEngine::with_collection`](super::WasmEngine::with_collection)).
    pub settings: WasmSettings,
    /// The identity and the stem, for the messages.
    pub extension: ExtensionIdentity,
    /// Where the module comes from, for the messages and the entry `define`.
    pub origin: ModuleOrigin,
    /// The module the extension's instances are created from.
    pub module: Arc<WasmModule>,
}

impl WasmExtension {
    /// The fuel of one handler call: the setting, or as much as wasmi counts
    /// when it is unlimited.
    fn fuel(&self) -> u64 {
        self.settings.fuel.unwrap_or(u64::MAX)
    }

    /// What the messages call the load entry `entry` (`define`, `packages`).
    fn load_entry(&self, entry: &str) -> String {
        let stem = &self.extension.stem;
        match &self.origin {
            ModuleOrigin::Folder(path) => format!("the entry `{entry}` of the module `{path}` of the extension `{stem}`"),
            ModuleOrigin::Collection(label) => {
                format!("the entry ‘{entry}’ of the extension ‘{stem}’ in the collection ‘{label}’")
            }
        }
    }

    /// The key of this extension's instances on a thread.
    fn key(&self) -> InstanceKey {
        InstanceKey { host: self.host.id(), module: self.module.id }
    }
}

/// Why a call into an instance stopped: the fuel ran out, the build was
/// cancelled, a trap (with its text), or a buffer the guest gave outside its
/// memory (with the failure's text).
enum Stop {
    /// The fuel ran out.
    Fuel,
    /// The build of the call was cancelled.
    Cancelled,
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
/// binding calls, the extensions defined in it, how many entries into it are
/// running, and whether a limit or a trap poisoned it.
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
    /// The extensions defined in this instance, by stem.
    defined: RefCell<HashMap<String, Defined>>,
    /// How many entries into this instance are running on this thread.
    depth: Cell<u32>,
    /// Set when the instance must not be used again (the fuel ran out, the
    /// memory limit was reached, a trap); the outermost call drops it on its
    /// way out.
    poisoned: Cell<bool>,
}

/// An extension defined in an instance: the load it was defined for, and the
/// features whose `register` the instance ran for it (for real or
/// rehearsed).
struct Defined {
    /// The load ([`WasmExtension::id`]).
    load: u64,
    /// The features whose `register` ran.
    registered: Rc<Registered>,
}

impl WasmInstance {
    /// Creates an instance of `extension`'s module on this thread: the store
    /// with the extension's memory limit and one call's fuel, the module
    /// instantiated with the two host functions, its exports found. No
    /// extension is defined in it yet ([`WasmInstance::define`]).
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: the module's initial memory is above the
    /// limit, or it cannot be instantiated.
    fn create(extension: &WasmExtension) -> Result<WasmInstance, ExtError> {
        let stem = &extension.extension.stem;
        let module = &extension.module;
        let failed = |detail: String| ExtError::Handler {
            detail: format!("the engine of the extension `{stem}` could not start: {detail}"),
            data: Value::Null,
        };
        let limit = extension.settings.memory_limit;
        if let Some(limit) = limit.filter(|limit| module.initial_memory > *limit as u64) {
            let detail = match &extension.origin {
                ModuleOrigin::Folder(path) => format!(
                    "the initial memory of the module `{path}` of the extension `{stem}` must be at most {limit} bytes, its \
                     memory limit"
                ),
                ModuleOrigin::Collection(label) => format!(
                    "the initial memory of the module of the collection ‘{label}’ must be at most {limit} bytes (the memory \
                     limit of its instances)"
                ),
            };
            return Err(ExtError::Handler {
                detail,
                data: json!({ "limit": "memory", "limit_bytes": limit }),
            });
        }
        let guest = Guest { limiter: Limiter { limit, refused: false }, request: None, reply: None };
        let mut store = Store::new(&module.engine, guest);
        store.limiter(|guest| &mut guest.limiter);
        store.set_fuel(extension.fuel()).map_err(|error| failed(error.to_string()))?;
        let linker = abi::linker(&module.engine).map_err(failed)?;
        let instance = linker.instantiate_and_start(&mut store, &module.module).map_err(|error| failed(error.to_string()))?;
        let memory = instance.get_memory(&store, "memory").ok_or_else(|| failed("the module exports no memory `memory`".into()))?;
        let entry = instance.get_typed_func(&store, "flm_entry").map_err(|error| failed(error.to_string()))?;
        let alloc = instance.get_typed_func(&store, "flm_alloc").map_err(|error| failed(error.to_string()))?;
        let free = instance.get_typed_func(&store, "flm_free").map_err(|error| failed(error.to_string()))?;
        let last_panic = instance.get_typed_func(&store, "flm_last_panic").ok();
        Ok(WasmInstance {
            store: RefCell::new(store),
            memory,
            entry,
            alloc,
            free,
            last_panic,
            defined: RefCell::new(HashMap::new()),
            depth: Cell::new(0),
            poisoned: Cell::new(false),
        })
    }

    /// The features whose `register` this instance ran for `extension`,
    /// after calling its entry `define(stem, module)` (outside any handler
    /// call, with one call's fuel of its own,
    /// [`WasmInstance::call_load_entry`]) when the instance has not defined
    /// the extension's stem for this load; a stem defined again starts with
    /// no feature registered.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: `define` fails or answers an error; the
    /// instance is then poisoned.
    fn define(&self, extension: &WasmExtension) -> Result<Rc<Registered>, ExtError> {
        let stem = &extension.extension.stem;
        if let Some(defined) = self.defined.borrow().get(stem).filter(|defined| defined.load == extension.id) {
            return Ok(Rc::clone(&defined.registered));
        }
        let args = [EntryArg::Text(stem.clone()), EntryArg::Text(extension.origin.text().to_string())];
        if let Err(error) = self.call_load_entry(extension, "define", &args) {
            self.poisoned.set(true);
            return Err(error);
        }
        let registered = Rc::new(Registered::default());
        self.defined
            .borrow_mut()
            .insert(stem.clone(), Defined { load: extension.id, registered: Rc::clone(&registered) });
        Ok(registered)
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
    /// handler call `what`, under `progress`: the request written into a
    /// buffer of the guest's, `flm_entry` driven to its end (each request of
    /// the guest answered), the buffer freed, the reply read and freed.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: the fuel ran out, the build was cancelled, the
    /// memory limit was reached, a trap; the reply is not of the ABI's shape; the guest's own error
    /// (read by [`ExtError::from_data`]); [`ExtError::Value`]: an argument
    /// cannot be written as JSON ([`EntryArg::to_value`]), or the reply is not
    /// JSON.
    pub(crate) fn call_entry(
        &self,
        extension: &WasmExtension,
        entry: &str,
        args: &[EntryArg<'_>],
        what: &str,
        progress: &Scope,
    ) -> Result<Value, ExtError> {
        self.answer(extension, entry, args, what, progress)?
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
    pub(crate) fn call_load_entry(&self, extension: &WasmExtension, entry: &str, args: &[EntryArg<'_>]) -> Result<Value, ExtError> {
        let what = extension.load_entry(entry);
        self.set_fuel(extension.fuel());
        self.answer(extension, entry, args, &what, &Scope::none())?.map_err(|error| ExtError::Handler {
            detail: format!("{what} must answer without an error: {error}"),
            data: inner_data(&error),
        })
    }

    /// The answer of the entry `entry` with `args`, for the call `what` under
    /// `progress`: the
    /// guest's own answer (its value, or its own error), or the call's
    /// failure (the outer error: a limit, a trap, a reply not of the ABI's
    /// shape).
    fn answer(
        &self,
        extension: &WasmExtension,
        entry: &str,
        args: &[EntryArg<'_>],
        what: &str,
        progress: &Scope,
    ) -> Result<Result<Value, ExtError>, ExtError> {
        let request = abi::entry_request(entry, &extension.extension.stem, args)?;
        let reply = self.run(extension, request.as_bytes(), what, progress).map_err(|stop| self.fail(extension, what, stop))?;
        abi::read_reply(&reply, what)
    }

    /// Runs one entry under `progress`: the request `bytes` written, the call
    /// driven, the reply's bytes read; both buffers freed.
    fn run(&self, extension: &WasmExtension, bytes: &[u8], what: &str, progress: &Scope) -> Result<Vec<u8>, Stop> {
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
        let packed = self.drive(extension, at, length, progress)?;
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
    /// call resumed with the reply's length. Answers the packed reply. Before
    /// it answers a request, the call asks whether the build of `progress` is
    /// cancelled.
    fn drive(&self, extension: &WasmExtension, at: i32, length: i32, progress: &Scope) -> Result<i64, Stop> {
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
                    if progress.cancelled() {
                        return Err(Stop::Cancelled);
                    }
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
            Stop::Cancelled => ExtError::Handler {
                detail: format!("{what} was stopped: the build was cancelled"),
                data: json!({ "cancelled": true }),
            },
            Stop::Outside(detail) => ExtError::Handler { detail, data: Value::Null },
            Stop::Trap(_) if self.store.borrow().data().limiter.refused => {
                let limit = self.store.borrow().data().limiter.limit.unwrap_or(0);
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

/// The key of an instance on a thread: one per host, per module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InstanceKey {
    /// The host's identity.
    host: u64,
    /// The module's number ([`WasmModule::id`]).
    module: u64,
}

/// An instance in a thread's map, with its module and its host, each held
/// weakly: once either is gone, the next entry into any instance on the
/// thread, and the next load of an extension on the thread, drop it
/// ([`sweep`]).
struct Kept {
    /// The instance.
    instance: Rc<WasmInstance>,
    /// The module it is an instance of.
    module: Weak<WasmModule>,
    /// Its host.
    host: WeakHost,
}

thread_local! {
    /// The instances of this thread. Each is shared (`Rc`) between the map
    /// and the entries running on it, so the map is borrowed only for a
    /// moment and a nested entry can find or create an instance.
    static INSTANCES: RefCell<HashMap<InstanceKey, Kept>> = RefCell::new(HashMap::new());
}

/// Drops this thread's instances of the modules or the hosts no one holds
/// any more, unless a call is running on them.
///
/// Every entry into an instance sweeps first ([`enter_instance`]), the load
/// entries among them, so that the map never holds more than the instances
/// of one generation of dropped extensions. A thread that only loads
/// extensions (a preparation that fails after the load runs no handler)
/// therefore drops the instances of the previous load at each load.
fn sweep() {
    let swept = INSTANCES.with(|instances| {
        let mut instances = instances.borrow_mut();
        let mut swept = Vec::new();
        instances.retain(|_, kept| {
            let keep = (kept.module.strong_count() > 0 && kept.host.exists()) || kept.instance.depth.get() > 0;
            if !keep {
                swept.push(Rc::clone(&kept.instance));
            }
            keep
        });
        swept
    });
    // Dropped after the map's borrow ends.
    drop(swept);
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

/// Leaves an instance on the way out of [`enter_instance`]: restores the
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

/// Enters this thread's instance of `extension`'s module and runs `run` on
/// it, with the features the instance ran for the extension, after dropping
/// the instances no one holds any more ([`sweep`]). The instance is created
/// when the thread has none, or a fresh one in strict mode, unless an entry
/// into it runs; at the outermost entry, the memory limit's mark is
/// cleared. The extension is then defined in it when it is not
/// ([`WasmInstance::define`]), and the call is given its fuel (a nested
/// entry keeps the outer call's remainder, restored on the way out).
///
/// # Errors
///
/// The failure to create the instance, as `created` words it, or to define
/// the extension, as `defined` words it; otherwise what `run` answers.
fn enter_instance<R>(
    extension: &Arc<WasmExtension>,
    created: impl FnOnce(ExtError) -> ExtError,
    defined: impl FnOnce(ExtError) -> ExtError,
    run: impl FnOnce(&WasmInstance, Rc<Registered>) -> Result<R, ExtError>,
) -> Result<R, ExtError> {
    let key = extension.key();
    let strict = extension.settings.strict;
    sweep();
    let existing = INSTANCES.with(|instances| instances.borrow().get(&key).map(|kept| Rc::clone(&kept.instance)));
    let instance = match existing {
        Some(instance) if instance.depth.get() > 0 || !strict => instance,
        _ => {
            let instance = Rc::new(WasmInstance::create(extension).map_err(created)?);
            let kept = Kept {
                instance: Rc::clone(&instance),
                module: Arc::downgrade(&extension.module),
                host: extension.host.downgrade(),
            };
            INSTANCES.with(|instances| instances.borrow_mut().insert(key, kept));
            instance
        }
    };
    let outer = if instance.depth.get() > 0 { Some(instance.fuel()) } else { None };
    if outer.is_none() {
        instance.store.borrow_mut().data_mut().limiter.refused = false;
    }
    instance.depth.set(instance.depth.get() + 1);
    let _leave = Leave { key, instance: &instance, strict, outer };
    let registered = instance.define(extension).map_err(defined)?;
    instance.set_fuel(extension.fuel());
    run(&instance, registered)
}

/// Calls the load entry `entry` (`packages`) of `extension`, without
/// arguments, in this thread's instance of its module, after defining the
/// extension there ([`enter_instance`]): the instance the load uses is the
/// one the registry build on the loading thread uses.
///
/// # Errors
///
/// The failure to create the instance, to define the extension, or of the
/// entry ([`WasmInstance::call_load_entry`]).
pub(crate) fn call_load_entry(extension: &Arc<WasmExtension>, entry: &str) -> Result<Value, ExtError> {
    enter_instance(extension, |error| error, |error| error, |instance, _| instance.call_load_entry(extension, entry, &[]))
}

/// An instance entered for one handler call: the instance, its extension,
/// the features the instance ran for it, and the call's scope.
struct Entered<'a> {
    /// The instance.
    instance: &'a WasmInstance,
    /// Its extension.
    extension: &'a WasmExtension,
    /// The features whose `register` the instance ran for the extension.
    registered: Rc<Registered>,
    /// The scope of the row the handler works under.
    progress: &'a Scope,
}

impl Instance for Entered<'_> {
    fn registered(&self) -> &Registered {
        &self.registered
    }

    fn call_entry(&mut self, entry: &str, args: &[EntryArg<'_>], what: &str) -> Result<Reply, ExtError> {
        self.instance.call_entry(self.extension, entry, args, what, self.progress).map(Reply::Json)
    }
}

impl Binding for WasmExtension {
    fn identity(&self) -> &ExtensionIdentity {
        &self.extension
    }

    fn id(&self) -> u64 {
        self.id
    }

    /// Enters this thread's instance of the extension's module
    /// ([`enter_instance`]), creating it, or the extension's definition in
    /// it, when needed, and runs `run` on it, under `progress`.
    fn enter(
        self: Arc<Self>,
        progress: &Scope,
        run: &mut dyn FnMut(&mut dyn Instance) -> Result<Reply, ExtError>,
    ) -> Result<Reply, ExtError> {
        let stem = &self.extension.stem;
        let created = |error: ExtError| ExtError::Handler {
            detail: format!("the engine of the extension `{stem}` could not be created on this thread: {error}"),
            data: inner_data(&error),
        };
        let defined = |error: ExtError| ExtError::Handler {
            detail: format!("Failed to define the extension ‘{stem}’ in its instance on this thread: {error}"),
            data: inner_data(&error),
        };
        enter_instance(&self, created, defined, |instance, registered| {
            run(&mut Entered { instance, extension: &self, registered, progress })
        })
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
            extension: ExtensionIdentity { identity: "t".to_string(), stem: "t".to_string(), name: "t".to_string() },
            origin: ModuleOrigin::Folder("m.wasm".to_string()),
            module: Arc::new(WasmModule::new(engine, module)),
        })
    }

    #[test]
    fn an_instance_that_cannot_be_created_on_a_thread_fails_the_handler_call() {
        let extension = extension(r#"(module (memory (export "memory") 2))"#, 1 << 16);
        let error = Arc::clone(&extension).enter(&Scope::none(), &mut |_| Ok(Reply::Json(Value::Null))).expect_err("the instance cannot be created");
        let ExtError::Handler { data, .. } = &error else { panic!("a handler error: {error:?}") };
        assert_eq!(data, &json!({ "limit": "memory", "limit_bytes": 65536 }));
        assert_eq!(extension.host.instances_on_this_thread(), 0);
    }
}
