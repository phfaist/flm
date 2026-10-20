//! The WebAssembly binding: extensions compiled to WebAssembly, run by the
//! interpreter wasmi (cargo feature `wasm`).
//!
//! # What the binding is
//!
//! The engine `wasm` ([`WasmEngine`], added to a loader by
//! [`ExtensionLoader::with_wasm`](crate::ExtensionLoader::with_wasm)) loads an
//! extension whose code is one WebAssembly module, and runs the module's
//! handlers on an instance of it per thread. The module sees FLM through one
//! import, a request in JSON text, and answers the binding's entry points in
//! JSON text: the same requests the JavaScript prelude makes of its host
//! (the twelve natives) and the same entry points (`register`, `run_stage`,
//! `expand`, …), so the handler objects, their lending and the natives'
//! bodies are the ones the JavaScript binding uses. The engine's own half is
//! here: the instances, the entry calls, the limits, the loading, the digest.
//! An extension written in Rust is compiled against the guest client
//! (`flm_ext::guest`, cargo feature `guest`), which writes the module's side
//! of the ABI below.
//!
//! # An extension in WebAssembly
//!
//! ```yaml
//! extension:
//!   engine: wasm
//!   wasm:
//!     module: wordcount.wasm
//!     limits: { memory_mib: 128, fuel: 5000000000 }
//! ```
//!
//! `extension.wasm.module` is the module's path inside the extension's folder
//! ([`WasmManifest`](crate::manifest::WasmManifest)); `limits` is optional
//! ([`WasmLimits`](crate::manifest::WasmLimits)).
//!
//! **The ABI.** JSON text in the guest's linear memory, both ways.
//!
//! - *Imports*, module `flm`: `call(ptr: i32, len: i32) -> i32` sends the
//!   request at `[ptr, ptr + len)` and answers the length of its reply, which
//!   the host keeps; `take(ptr: i32, len: i32)` copies the kept reply to `ptr`
//!   (`len` must be its length, else the call traps). `call` may be called
//!   only while `flm_entry` runs (else it traps), and a range outside the
//!   memory traps. Nothing else is offered.
//! - *Exports*: the memory `memory`; `flm_alloc(len: i32) -> i32`;
//!   `flm_free(ptr: i32, len: i32)`; `flm_entry(ptr: i32, len: i32) -> i64`;
//!   optionally `flm_last_panic() -> i64`.
//! - *An entry*: the host writes `{"entry": NAME, "args": [ … ]}` into a
//!   buffer it took with `flm_alloc`, calls `flm_entry`, and frees the buffer
//!   with `flm_free`. The arguments are JSON values: for a handler, the
//!   package, the feature, then the handler's own (a name as a string, a
//!   handle as `{"@handle": …}`); `define`'s are the stem and the module's
//!   path; `packages`'s none. The answer is `(ptr << 32) | len` of the reply,
//!   a buffer of the guest's the host frees after reading it: `{"ok":
//!   value}` (`null` for nothing), or `{"err": data, "message": text}`, the
//!   data being an [`ExtError`](crate::ExtError)'s
//!   ([`data`](crate::ExtError::data)). `flm_last_panic` answers the same
//!   packing for the last panic's message, `0` when there is none.
//! - *A request*: `{"native": NAME, …fields}`, one of the twelve natives:
//!   `call` (`handle`, `method`, `args`), `module_handle` (`path`),
//!   `lend_stage` (`decl`, `needs`), `lend_expand` (`element`, `policy`),
//!   `lend_lower` (`rule`), `lend_rewrite` (`decl`, `needs`), `lend_provider`
//!   (`name`), `lend_template` (`name`), `lend_aspect` (`kind`, `element`,
//!   `methods`), `lend_late_init` (`index`), `lend_finalizer` (`index`),
//!   `console` (`level`, `text`). The reply is the envelope `{"ok": value}` or
//!   `{"err": data, "message": text}`. A request that is not JSON, nests lists
//!   and maps more than 127 levels deep, or is of no native's shape, answers
//!   an `err` envelope; it is not a trap.
//!
//! The entries a module answers: `define(stem, module)` once per instance,
//! then `packages()` at load (the package objects, in the shape the
//! JavaScript prelude's `packages` answers); `register(settings, b)` and
//! `rehearse(settings)`; the handlers' entries, `run_stage`, `expand`,
//! `lower`, `restage`, `answer_all`, `template_describe`, `template_files`,
//! `template_render`, `points`, `level`, `in_toc`, `toc_title`, `late_init`,
//! `finalize`, each with the package and the feature first.
//!
//! **What a module reaches.** Nothing but the two imports: no file, network,
//! clock, random source or process, since the binding offers no other import
//! and refuses a module that asks for one. What it needs from outside it asks
//! of the host's services ([`crate::host`]) through `call`, as any call of
//! the extension API, once its package declares them.
//!
//! # Loading and the instances
//!
//! At load the module's bytes are read once through the extension's files and
//! compiled once; its imports and exports are checked against the ABI; an
//! instance is created on the loading thread (instantiated, then its entry
//! `define` called), its entry `packages` called, and the answer read as the
//! package objects. Every other thread creates its own instance from the
//! compiled module when it first runs a handler of the extension, and calls
//! `define` once. An instance is a wasmi store and instance; its memory is
//! its own. `define` and `packages` run outside any handler call of the
//! extension: a request that needs one (any but `module_handle` and
//! `console`) answers a refusal, also when the instance is created while
//! another extension's handler call runs on the thread.
//!
//! # The mapping rules
//!
//! The values crossing are those of the JavaScript binding: JSON values,
//! a façade object as its handle `{"@handle": {"kind", "call", "index"}}`,
//! valid during the handler call that lent it, a durable object as its token
//! `{"@durable": …}`, valid for the life of the registry. A handler call that
//! runs while a request of the guest runs (a hook run while a stage's request
//! runs) is a nested entry into the same instance: wasmi suspends the guest's
//! call at each request (a resumable call) and the binding answers the
//! request with the instance free, so the nested call runs on it and returns
//! before the guest is resumed with the reply. The twelve mapping rules for
//! a guest written in Rust are on the page of `flm_ext::guest`.
//!
//! # The limits
//!
//! | Limit ([`WasmSettings`]) | Default | Exceeded |
//! |---|---|---|
//! | `fuel`, per handler call | 1,000,000,000 units (about one per instruction) | the call stops; the handler fails; the instance is replaced after the call |
//! | `memory_limit`, per instance | 64 MiB | `memory.grow` answers -1; when the guest then traps (a Rust guest's allocator aborts), the handler fails with the limit's text; the instance is replaced after the call |
//! | the call depth | wasmi's (1,000 nested calls), not configurable | a trap: the handler fails; the instance is replaced after the call |
//!
//! An extension may set its own limits in its manifest, under
//! `extension.wasm.limits` (`memory_mib`, `fuel`): each limit given replaces
//! the host's for every instance of that extension
//! ([`WasmSettings::with_limits`]). A module whose initial memory is above
//! the limit cannot be instantiated. A nested handler call has fuel of its
//! own, and the outer call's remainder is restored after it. Instantiating
//! the module has one call's fuel, and each of the entries `define` and
//! `packages` one call's fuel of its own. wasmi runs with its `deterministic`
//! feature: a module answers the same on every machine (NaNs are canonical).
//!
//! # Strict mode
//!
//! With [`WasmSettings::strict`], every outermost handler call runs on a
//! fresh instance, dropped after the call: nothing a module keeps in its
//! memory survives a call.
//!
//! # The rehearsal
//!
//! The objects a feature's `register` hands to FLM (a stage, a hook, a rule's
//! body, …) are values of the instance that made them, as in the JavaScript
//! binding. When an instance that has not run a feature's `register` with the
//! configuration of the registration at hand must run one of the feature's
//! handlers, the binding first calls its entry `rehearse(settings)`, in which
//! the module runs `register` again with a builder that dispatches nothing,
//! then each finalizer that `register` recorded, likewise; it then holds the
//! objects, found again by name, by element and kind, or by position. A
//! template module's `describe` runs inside the `register` that contributes
//! it, without a rehearsal. The contract is the JavaScript binding's: a
//! feature's `register` declares the same things under the same names, in
//! the same order, every time it runs with one configuration.
//!
//! # Failures
//!
//! | Failure | What the handler's caller gets |
//! |---|---|
//! | a handler's entry answers `{"err": …}` | the [`ExtError`](crate::ExtError) its data describe, as the guest returned it |
//! | `define` or `packages` answers `{"err": …}` | "the entry `define` of the module `P` of the extension `S` must answer without an error: DETAIL" on every thread that creates an instance (likewise "the entry `packages` …" at load), DETAIL the text of the error its data describe |
//! | the fuel runs out | "WHAT must finish within its budget of N fuel units", the data `{ "limit": "fuel", "fuel": N }` |
//! | a trap after a refused memory growth | "WHAT must stay within the memory limit of N bytes", the data `{ "limit": "memory", "limit_bytes": N }` |
//! | any other trap (`unreachable`, an access out of bounds, the call depth, a host function given a wrong length) | "WHAT must run without a trap: DETAIL", with "; the guest panicked: MESSAGE" when `flm_last_panic` answers one; the data `{ "trap": DETAIL, "panic": MESSAGE or null }` |
//! | the reply is not JSON, nests too deep, holds neither `ok` nor `err`, or its `err` is not an error's data | the text saying so |
//! | the instance cannot be created (on a thread other than the loading one) | "the engine of the extension `S` could not be created on this thread: DETAIL" |
//! | the extension does not load (a module that does not compile, an import not offered, an export missing or of another type, an initial memory above the limit, `define` or `packages` failing, a package object that is not one) | every problem reported where the `use` entry stands |
//!
//! The fuel limit, the memory limit and a trap poison the instance, which is
//! dropped when the outermost call leaves it. What the library does with a
//! failing handler is the same as for a JavaScript handler (the module
//! `flm_ext::quickjs`, cargo feature `quickjs`).
//!
//! # The digest
//!
//! What the registry digest receives for every feature of a WebAssembly
//! extension, after the manifest: the bytes `flm-ext wasm ` and
//! [`ENGINE_VERSION`], a NUL byte, the module's bytes, a NUL byte. The host's
//! settings do not enter.

mod abi;
mod engine;
mod host;
mod instance;

pub use engine::WasmEngine;
pub use host::{WasmHost, WasmSettings};

/// The version of wasmi this binding runs, which enters the digest. Kept in
/// step with the `wasmi` line of `flm-ext/Cargo.toml`.
pub const ENGINE_VERSION: &str = "wasmi 2.0.0";

#[cfg(test)]
mod tests {
    use wasmi::{
        Caller, Config, Engine, Error, Extern, Linker, Module, Store, StoreLimits,
        StoreLimitsBuilder, TrapCode,
    };

    /// A module of a few instructions, in the shape of the binding's
    /// interface: the import `flm.call`, the export `flm_entry` that calls it,
    /// the exported memory; and three exports for the limits and the traps.
    const MODULE: &str = r#"
        (module
          (import "flm" "call" (func $call (param i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "flm_entry") (param $ptr i32) (param $len i32) (result i64)
            (i64.extend_i32_u (call $call (local.get $ptr) (local.get $len))))
          (func (export "grow") (param $pages i32) (result i32)
            (memory.grow (local.get $pages)))
          (func (export "spin") (loop $again (br $again)))
          (func (export "fail") unreachable))
    "#;

    /// The store's data: the requests the host function saw, in order, and
    /// the memory limit.
    struct Probe {
        requests: Vec<u8>,
        limits: StoreLimits,
    }

    /// The host function `flm.call`: reads a one-byte request (a depth) at
    /// `ptr` in the module's memory; at a depth above zero, writes the next
    /// request after it and calls the export `flm_entry` of the same instance
    /// through the caller, while this host function runs. Answers the number
    /// of nested calls.
    fn call(mut caller: Caller<'_, Probe>, ptr: i32, len: i32) -> Result<i32, Error> {
        assert_eq!(len, 1);
        let memory = caller
            .get_export("memory")
            .and_then(Extern::into_memory)
            .ok_or_else(|| Error::new("the module exports no memory"))?;
        let at = ptr as usize;
        let depth = memory.data(&caller)[at];
        caller.data_mut().requests.push(depth);
        if depth == 0 {
            return Ok(0);
        }
        memory.data_mut(&mut caller)[at + 1] = depth - 1;
        let entry = caller
            .get_export("flm_entry")
            .and_then(Extern::into_func)
            .ok_or_else(|| Error::new("the module exports no flm_entry"))?
            .typed::<(i32, i32), i64>(&caller)?;
        let nested = entry.call(&mut caller, (ptr + 1, 1))?;
        Ok(nested as i32 + 1)
    }

    #[test]
    fn engine_fuel_memory_limit_host_function_and_reentry() {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let bytes = wat::parse_str(MODULE).expect("the text module parses");
        let module = Module::new(&engine, &bytes).expect("the module compiles");
        let limits = StoreLimitsBuilder::new().memory_size(2 * 65536).build();
        let mut store = Store::new(&engine, Probe { requests: Vec::new(), limits });
        store.limiter(|probe| &mut probe.limits);
        store.set_fuel(1_000_000).expect("fuel metering is on");
        let mut linker = Linker::<Probe>::new(&engine);
        linker.func_wrap("flm", "call", call).expect("one definition");
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("the module instantiates");

        // The host function, called from the module, calls the module's export
        // back three times over, each nested in the one before.
        let memory = instance.get_memory(&store, "memory").expect("exported");
        memory.data_mut(&mut store)[0] = 3;
        let entry = instance
            .get_typed_func::<(i32, i32), i64>(&store, "flm_entry")
            .expect("exported");
        assert_eq!(entry.call(&mut store, (0, 1)).expect("no trap"), 3);
        assert_eq!(store.data().requests, [3, 2, 1, 0]);
        assert_eq!(memory.data(&store)[..4], [3, 2, 1, 0]);
        let left = store.get_fuel().expect("fuel metering is on");
        assert!(left < 1_000_000, "the calls consumed fuel: {left} left");

        // The memory limit: a second page fits, a third does not, and the
        // module's `memory.grow` answers -1 without a trap.
        let grow = instance.get_typed_func::<i32, i32>(&store, "grow").expect("exported");
        assert_eq!(grow.call(&mut store, 1).expect("no trap"), 1);
        assert_eq!(grow.call(&mut store, 1).expect("no trap"), -1);

        // The traps: fuel running out, and `unreachable`, with wasmi's texts.
        store.set_fuel(10_000).expect("fuel metering is on");
        let spin = instance.get_typed_func::<(), ()>(&store, "spin").expect("exported");
        let error = spin.call(&mut store, ()).expect_err("the loop runs out of fuel");
        assert_eq!(error.as_trap_code(), Some(TrapCode::OutOfFuel));
        assert_eq!(error.to_string(), "all fuel consumed by WebAssembly");
        store.set_fuel(10_000).expect("fuel metering is on");
        let fail = instance.get_typed_func::<(), ()>(&store, "fail").expect("exported");
        let error = fail.call(&mut store, ()).expect_err("unreachable traps");
        assert_eq!(error.as_trap_code(), Some(TrapCode::UnreachableCodeReached));
        assert_eq!(error.to_string(), "wasm `unreachable` instruction executed");
    }
}
