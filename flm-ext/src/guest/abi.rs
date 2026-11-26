//! The guest's side of the ABI between a WebAssembly module and the engine
//! `wasm` (the host): the two imports of the module `flm`, the guest's
//! buffers, the requests a guest sends (the natives), and the conversion of
//! an argument to JSON ([`ToArg`]); and, beside it, the typed route that
//! takes the ABI's place when the guest client is compiled into a program
//! (the cargo feature `native`): the typed route of a request and of a
//! lending (`in_frame`, `typed`), and the log sink [`console`] writes
//! to. One extension's source uses either; the feature `native` alone
//! chooses which.
//!
//! # The ABI
//!
//! JSON text in the guest's linear memory both ways, in the shapes of the
//! dispatcher.
//!
//! - **Imports** (module `flm`): `call(ptr, len) -> len`, a request at
//!   `[ptr, ptr + len)`, answering the length of its reply, which the host
//!   keeps; `take(ptr, len)`, which copies the kept reply to `ptr` (`len` must
//!   be its length).
//! - **Exports** (written by the guest's entry macro, each a call of a
//!   function here): `memory`; `flm_alloc(len) -> ptr` ([`alloc`]);
//!   `flm_free(ptr, len)` ([`free`]); `flm_entry(ptr, len) -> i64`
//!   ([`entry`]), which reads its request from the buffers ([`read`]) and
//!   answers its reply's buffer ([`keep`]); `flm_last_panic() -> i64`
//!   ([`last_panic`]), the message of the last panic, which the panic hook
//!   [`entry`] installs keeps.
//! - **A request** is `{"native": NAME, …}` (the function `native`); its reply is the
//!   envelope `{"ok": value}` or `{"err": data, "message": text}`, `data`
//!   being an [`ExtError`]'s data ([`ExtError::data`]), read back by
//!   [`ExtError::from_data`].
//!
//! # The buffers
//!
//! A buffer the host asks for (`flm_alloc`) and a reply the guest answers
//! are `Vec<u8>`s kept in a map by their address: `flm_alloc` inserts,
//! `flm_free` removes, `flm_entry` reads a copy of its request by address.
//! Every buffer stays kept until the host frees it, the request too, so no
//! address the host still holds is given to another buffer meanwhile (the
//! host frees the request before it reads the reply). The guest never reads
//! memory through a raw pointer: a buffer's address is the address of a
//! `Vec` it owns, exposed to the host (`expose_provenance`), and the host
//! reads and writes the buffer through its own access to the guest's
//! memory, only within the buffer's length. The addresses are the ABI's 32
//! bits, a WebAssembly module's: [`alloc`], [`free`], [`read`] and [`keep`]
//! serve the module's exports alone.
//!
//! # Re-entry
//!
//! A request suspends the guest inside its call of `flm.call`, and the host
//! may enter the same instance meanwhile to run a nested handler call
//! (`flm_alloc`, `flm_entry`, `flm_free`). So no borrow of the guest's own
//! state (the buffers, the module handles) is held across a call of
//! `flm.call`: each is taken, released, and only then is the request sent.
//! The module handles are kept for a module alone: compiled into a program,
//! the guest client calls a module's functions with no handle.
//!
//! # Where the imports are compiled
//!
//! The import block is compiled when the crate is built for `wasm32` and
//! `flm-ext`'s cargo feature `native` is off: the extension's crate is then a
//! WebAssembly module of its own, which the engine `wasm` runs. The feature
//! `native`, not the target, is the sign of a program that carries the engine
//! `native`: with it, on every target, `wasm32` included, the import block is
//! not compiled and the extension's requests are answered in the same process.
//!
//! # `unsafe`
//!
//! The import block is the module's one `unsafe`: `unsafe extern "C"`, its
//! two functions declared `safe`. They take integers; the host checks every
//! address and length against the guest's memory (a buffer outside it is a
//! trap, never a write elsewhere), and the guest passes only the addresses of
//! buffers it owns for the duration of the call, with their lengths.
//!
//! # Without the imports
//!
//! Where the import block is not compiled (built for another target, or with
//! the feature `native`), the guest client runs inside the program that
//! carries it. With the feature `native`, an extension's requests are
//! answered in the same process while one of its native instances has its
//! state current on the thread: during one of the instance's entries, and
//! while the engine `native` drops the objects the instance recorded. Every
//! other request answers [`ExtError::Unsupported`]: in the client's own
//! tests and the tests of its types, on a thread the extension started, and
//! while a thread's native instances are dropped at its end. Built for
//! another target without the feature `native`, every request answers
//! [`ExtError::Unsupported`]; the module's unit tests install a hook that
//! answers in its place. The buffer functions serve a module's exports
//! alone: in a program built for another target an address need not fit the
//! ABI's 32 bits.
//!
//! # The typed route
//!
//! With the feature `native`, every crossing between the guest and the host
//! is a direct call, and the function `native` is not compiled. No JSON value
//! is built or read but the data that is JSON by nature: a feature's
//! settings, which its `register` and its template modules' `describe`
//! receive as a `serde_json::Value`, and the reports of `packages`, the
//! declarations of the extension's packages. The crossings:
//!
//! - **A request.** A method of a façade and a function of a module reach
//!   the extension API by a direct call: their body (generated, or written
//!   by hand for the guest's own types) runs inside `typed`, which hands it
//!   the table of the handler call the request was sent from, the innermost
//!   frame of the current state's loaded extension
//!   (`binding::frames::with_current`). The body is the dispatcher's arm for
//!   the same method with the conversions swapped: the guest's values
//!   converted into the API's and the answer back (the crate's module
//!   `adapter::typed`), a handle resolved and an answered object lent in the
//!   same table.
//! - **A lending.** A record-and-lend method (`FeatureBuilder::stage`,
//!   `ElementSpec::with`, …) records its object, reads from it what its
//!   lending takes, then runs inside `in_frame`, which hands it the frame:
//!   the natives' body (`binding::natives`) builds the host's handler object
//!   over the frame's registration, and the method hands it to the API
//!   method that takes it, in the same closure (one access to the frame per
//!   call); `ElementSpec::with` lends it into the table and keeps its handle.
//! - **A line of `console` output** goes to the log sink that the engine
//!   `native` gave the current state's loaded extension when it created the
//!   instance, with the extension's stem as its source.
//! - **An entry** comes the other way: the engine `native` calls the guest's
//!   routing directly (the crate's module `guest::entry`), the entry's
//!   arguments and reply the guest's values, where a module's export
//!   `flm_entry` reads the same request from JSON text ([`entry`]) and
//!   writes the reply as JSON text.
//!
//! Outside a handler call of the extension, `in_frame` refuses in the words
//! the host refuses the first native the same item sends as JSON: a request
//! as the native `call`, a lending as its lending native. With no native
//! instance's state current, it answers [`ExtError::Unsupported`].

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Once;

use flm_core::progress::LogLevel;
use serde_json::Value;
#[cfg(not(feature = "native"))]
use serde_json::json;

#[cfg(feature = "native")]
use crate::adapter::dispatch;
#[cfg(not(feature = "native"))]
use crate::adapter::durable::DurableToken;
#[cfg(feature = "native")]
use crate::adapter::handles::CallTable;
use crate::adapter::handles::{Handle, HandleKind};
use crate::adapter::values::{bytes_to_value, handle_from_value, mismatch, JsonForm, HANDLE_KEY};
#[cfg(not(feature = "native"))]
use crate::adapter::values::{durable_to_value, handle_to_value};
#[cfg(feature = "native")]
use crate::binding::frames::{self, FrameView};
#[cfg(feature = "native")]
use crate::binding::natives::{self, Act};
use crate::guest::NamedExtension;
use crate::runtime::ExtError;

// Not compiled with the feature `native`: a program built for `wasm32` that carries
// extensions compiled in answers their requests itself, and its module must import nothing
// that its host does not provide.
#[cfg(all(target_arch = "wasm32", not(feature = "native")))]
#[link(wasm_import_module = "flm")]
unsafe extern "C" {
    /// `flm.call`: sends the request at `[ptr, ptr + len)`; answers the
    /// length of its reply, which the host keeps until [`flm_take`].
    #[link_name = "call"]
    safe fn flm_call(ptr: i32, len: i32) -> i32;

    /// `flm.take`: copies the kept reply to `[ptr, ptr + len)`; `len` must be
    /// its length.
    #[link_name = "take"]
    safe fn flm_take(ptr: i32, len: i32);
}

thread_local! {
    /// The buffers the host asked for and the replies the guest answered, by
    /// address.
    static BUFFERS: RefCell<BTreeMap<usize, Vec<u8>>> = const { RefCell::new(BTreeMap::new()) };

    /// The message of the last panic, kept by the panic hook for
    /// `flm_last_panic`.
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[cfg(not(feature = "native"))]
thread_local! {
    /// The handle of each API module asked for, by the module's path: asked
    /// once, then kept (a module handle is valid in every call). Compiled
    /// into a program, the guest client calls a module's functions directly,
    /// with no handle.
    static MODULES: RefCell<BTreeMap<String, Handle>> = const { RefCell::new(BTreeMap::new()) };
}

/// Installs the panic hook once.
static PANIC_HOOK: Once = Once::new();

/// The address of `buffer`, exposed so that the host may read and write it.
fn address(buffer: &mut Vec<u8>) -> usize {
    buffer.as_mut_ptr().expose_provenance()
}

/// An address or a length as the ABI's `i32` (its 32 bits, read as unsigned
/// by the host; a guest's addresses and lengths are below 2³²).
fn to_abi(number: usize) -> i32 {
    number as u32 as i32
}

/// An `i32` of the ABI as an address or a length (read as unsigned).
fn from_abi(number: i32) -> usize {
    number as u32 as usize
}

/// Keeps `buffer` in the buffers map; answers its address. A buffer of no
/// bytes still holds one byte of capacity, so that its address is its own.
fn insert(mut buffer: Vec<u8>) -> usize {
    if buffer.capacity() == 0 {
        buffer.reserve_exact(1);
    }
    let address = address(&mut buffer);
    BUFFERS.with(|buffers| buffers.borrow_mut().insert(address, buffer));
    address
}

/// The buffer at `address`, taken out of the buffers map.
fn remove(address: usize) -> Option<Vec<u8>> {
    BUFFERS.with(|buffers| buffers.borrow_mut().remove(&address))
}

/// A copy of the buffer at `address`, which stays kept.
fn read_at(address: usize) -> Option<Vec<u8>> {
    BUFFERS.with(|buffers| buffers.borrow().get(&address).cloned())
}

/// `flm_alloc(len)`: a new buffer of `len` bytes for the host to write,
/// kept by its address, which is answered. The address is a WebAssembly
/// module's (the ABI's 32 bits): this serves the export alone.
pub fn alloc(len: i32) -> i32 {
    to_abi(insert(vec![0; from_abi(len)]))
}

/// `flm_free(ptr, len)`: drops the buffer at `ptr` (a buffer [`alloc`] made,
/// a request among them, or a reply [`keep`] answered); nothing when no
/// buffer is kept there. The address is a WebAssembly module's (the ABI's
/// 32 bits): this serves the export alone.
pub fn free(ptr: i32, _len: i32) {
    remove(from_abi(ptr));
}

/// A copy of the buffer at `ptr` (`flm_entry` reads its request so, `len`
/// being the length it was asked for with); `None` when no buffer is kept
/// there. The buffer stays kept until the host's `flm_free`, so the reply
/// is never given the address of a request the host has yet to free. The
/// address is a WebAssembly module's (the ABI's 32 bits): this serves the
/// export `flm_entry` alone.
pub fn read(ptr: i32, _len: i32) -> Option<Vec<u8>> {
    read_at(from_abi(ptr))
}

/// `bytes` kept in the buffers map as a reply for the host to read and free:
/// answers `(address << 32) | length`, `flm_entry`'s answer. The address is
/// a WebAssembly module's (the ABI's 32 bits): this serves the export
/// `flm_entry` alone.
pub fn keep(bytes: Vec<u8>) -> i64 {
    let len = bytes.len();
    let address = insert(bytes);
    (((address as u32 as u64) << 32) | (len as u32 as u64)) as i64
}

/// `flm_entry(ptr, len)`: runs the entry whose request the host wrote into
/// the buffer at `ptr` and answers its reply's buffer as [`keep`] packs it,
/// `extensions` being the table of code the entry macro names (the entry
/// `define` runs the declaring function of the row of its stem with the
/// extension builder). The first call installs the panic hook
/// that keeps a panic's message for [`last_panic`] (after the hook installed
/// before it, which still runs). The address is a WebAssembly module's (the
/// ABI's 32 bits): this serves the export alone.
pub fn entry(ptr: i32, len: i32, extensions: &[NamedExtension]) -> i64 {
    PANIC_HOOK.call_once(keep_panics);
    let request = read(ptr, len);
    keep(crate::guest::entry::answer(request.as_deref(), extensions))
}

/// `flm_last_panic()`: the message of the last panic, kept in a reply's
/// buffer as [`keep`] packs it; `0` when none was kept.
pub fn last_panic() -> i64 {
    match LAST_PANIC.with(|slot| slot.borrow().clone()) {
        Some(message) => keep(message.into_bytes()),
        None => 0,
    }
}

/// Installs the panic hook that keeps each panic's message (its text and
/// where it was raised) for [`last_panic`], before running the hook
/// installed until then.
fn keep_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let text = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("a panic without a message");
        let message = match info.location() {
            Some(at) => format!("{text}, at {}:{}:{}", at.file(), at.line(), at.column()),
            None => text.to_string(),
        };
        LAST_PANIC.with(|slot| *slot.borrow_mut() = Some(message));
        previous(info);
    }));
}

/// Sends `request` (JSON text) to the host through the module's import and
/// answers the reply's text.
#[cfg(all(target_arch = "wasm32", not(feature = "native")))]
fn send(mut request: Vec<u8>) -> Result<Vec<u8>, ExtError> {
    let len = flm_call(to_abi(address(&mut request)), to_abi(request.len()));
    let mut reply = vec![0; from_abi(len)];
    flm_take(to_abi(address(&mut reply)), len);
    Ok(reply)
}

/// Without the module's import (a crate built for another target): no host to send to
/// ([`UNANSWERED`]).
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
fn send(request: Vec<u8>) -> Result<Vec<u8>, ExtError> {
    #[cfg(test)]
    if let Some(reply) = hook::answer(&request) {
        return Ok(reply);
    }
    drop(request);
    Err(ExtError::Unsupported { what: UNANSWERED.to_string() })
}

/// What [`ExtError::Unsupported`] says of a request that no host answers: one sent without
/// the module's import while no native instance's state is current, which is the case in
/// the guest client's own tests, on a thread the extension started, and while a thread's
/// native instances are dropped at its end.
#[cfg(any(not(target_arch = "wasm32"), feature = "native"))]
pub(crate) const UNANSWERED: &str = "a request of the guest client sent neither from an extension's own WebAssembly module \
                              nor during a call of the engine `native` (from a thread the extension started, or while \
                              a thread ends)";

/// The error of a reply that is not an envelope.
#[cfg(not(feature = "native"))]
fn not_an_envelope(reply: &str) -> ExtError {
    ExtError::Handler {
        detail: format!("the host's reply must be `{{\"ok\": value}}` or `{{\"err\": data, \"message\": text}}`, not `{reply}`"),
        data: Value::Null,
    }
}

/// The value of the envelope `reply`, JSON text: its `ok`, or its `err` read
/// back as the [`ExtError`] whose data it is.
#[cfg(not(feature = "native"))]
fn open(reply: &[u8]) -> Result<Value, ExtError> {
    let text = String::from_utf8_lossy(reply);
    let envelope = serde_json::from_str::<Value>(&text).map_err(|_| not_an_envelope(&text))?;
    let Value::Object(mut fields) = envelope else { return Err(not_an_envelope(&envelope.to_string())) };
    if let Some(value) = fields.remove("ok") {
        return Ok(value);
    }
    let Some(data) = fields.get("err") else { return Err(not_an_envelope(&Value::Object(fields).to_string())) };
    Err(ExtError::from_data(data).unwrap_or_else(|detail| ExtError::Handler {
        detail: format!("the error of the host's reply must be the data of an error: {detail}"),
        data: Value::Null,
    }))
}

/// Sends the request `request` (`{"native": NAME, …}`, the shape the host's
/// natives take) through the module's import and answers the value of its
/// reply: the route of every request of an extension's own WebAssembly
/// module. Compiled into a program (the cargo feature `native`), the
/// requests take the typed route instead, and this function is not compiled.
///
/// # Errors
///
/// The host's refusal or the dispatcher's error, read back from the reply's
/// `err` ([`ExtError::from_data`]); without the module's import (a crate
/// built for another target), [`ExtError::Unsupported`].
#[cfg(not(feature = "native"))]
pub fn native(request: &Value) -> Result<Value, ExtError> {
    open(&send(request.to_string().into_bytes())?)
}

/// Runs `body`, the typed route of a request or of a lending (the cargo
/// feature `native`), on the frame of the handler call it was sent from: the
/// innermost frame of the current state's loaded extension
/// ([`frames::with_current`]). `body`
/// resolves the handles in the frame's table and lends the answered objects
/// into it, as the dispatcher's arm does for a request sent as JSON; a
/// lending builds its handler object over the frame's registration with the
/// native's body (`binding::natives`), as the native does for a lending sent
/// as JSON. `act` is what the refusal outside a handler call names: the
/// first native the same item sends as JSON, a lending's or the call of the
/// method.
///
/// # Errors
///
/// What `body` answers, an argument error naming `method` as the
/// dispatcher names it; the refusal of `act` when no handler call of the
/// extension is the innermost one on this thread (sent from `define`, from a
/// `Drop` while the engine drops the instance, or while another extension's
/// instance is created): "the extension `S` must declare a stage from a
/// feature's `register` or from a handler that `register` declared";
/// [`ExtError::Unsupported`] when no native instance's state is current (on a
/// thread the extension started, while a thread ends).
#[cfg(feature = "native")]
pub(crate) fn in_frame<T>(
    act: Act<'_>,
    method: &str,
    body: impl for<'v, 'c> FnOnce(&mut FrameView<'v, 'c>) -> Result<T, ExtError>,
) -> Result<T, ExtError> {
    let state = crate::guest::state::current();
    let Some(loaded) = state.loaded.as_ref() else {
        return Err(ExtError::Unsupported { what: UNANSWERED.to_string() });
    };
    // A typed request runs inside the frame's closure, as `dispatch::call` runs for one sent
    // as JSON, so the invariants of `binding::frames` hold for both routes.
    frames::with_current(loaded.load, |view| body(view).map_err(|error| dispatch::named(method, error)))
        .unwrap_or_else(|| Err(natives::outside(&loaded.extension.stem, act)))
}

/// [`in_frame`] for a request of the method or function `method`, whose
/// `body` takes the frame's table alone: a request needs nothing else of the
/// frame. Outside a handler call of the extension, the request is refused in
/// the words the native `call` refuses one sent as JSON: "the extension `S`
/// must call `M` of the extension API from a feature's `register` or from a
/// handler that `register` declared".
///
/// # Errors
///
/// As [`in_frame`].
#[cfg(feature = "native")]
pub(crate) fn typed<T>(method: &str, body: impl for<'c> FnOnce(&mut CallTable<'c>) -> Result<T, ExtError>) -> Result<T, ExtError> {
    in_frame(Act::Call(method), method, |view| body(view.table))
}

/// Calls `method` on the object `handle` stands for, with `args`; answers
/// the dispatcher's answer.
///
/// # Errors
///
/// As [`native`].
#[cfg(not(feature = "native"))]
pub fn call(handle: Handle, method: &str, args: Vec<Value>) -> Result<Value, ExtError> {
    native(&json!({ "native": "call", "handle": handle_to_value(handle), "method": method, "args": args }))
}

/// Calls `method` on the durable object `token` names (a compiled template),
/// with `args`: the host lends the object into the running call and calls
/// the method on it, as on a handle's object; answers the dispatcher's
/// answer.
///
/// # Errors
///
/// As [`native`].
#[cfg(not(feature = "native"))]
pub fn call_durable(token: DurableToken, method: &str, args: Vec<Value>) -> Result<Value, ExtError> {
    native(&json!({ "native": "call", "handle": durable_to_value(token), "method": method, "args": args }))
}

/// Calls the function `function` of the API module `path` (`"lang/text"`,
/// …) with `args`, on the module's handle, which is asked for once and kept.
///
/// # Errors
///
/// As [`native`].
#[cfg(not(feature = "native"))]
pub fn call_module(path: &str, function: &str, args: Vec<Value>) -> Result<Value, ExtError> {
    call(module_handle(path)?, function, args)
}

/// The handle of the API module `path`: the one kept, or asked for (the
/// map's borrow released before the request is sent) and kept.
#[cfg(not(feature = "native"))]
fn module_handle(path: &str) -> Result<Handle, ExtError> {
    if let Some(handle) = MODULES.with(|modules| modules.borrow().get(path).copied()) {
        return Ok(handle);
    }
    let handle = handle_of(&native(&json!({ "native": "module_handle", "path": path }))?, HandleKind::Module)?;
    MODULES.with(|modules| modules.borrow_mut().insert(path.to_string(), handle));
    Ok(handle)
}

/// The handle `value` writes (`{"@handle": {…}}`), which must be of the kind
/// `kind`.
///
/// # Errors
///
/// `value` is not a handle, or a handle of another kind
/// ([`ExtError::Argument`]).
pub fn handle_of(value: &Value, kind: HandleKind) -> Result<Handle, ExtError> {
    let handle = handle_from_value(value)?;
    if handle.kind == kind {
        Ok(handle)
    } else {
        Err(mismatch(kind, handle.kind))
    }
}

/// The handle of kind `kind` that `value` writes, a sequence's, and the
/// length the host wrote beside it
/// ([`sequence_to_value`](crate::adapter::values::sequence_to_value)).
///
/// # Errors
///
/// `value` is not a handle, or one of another kind, or has no length
/// ([`ExtError::Argument`]).
pub fn sequence_of(value: &Value, kind: HandleKind) -> Result<(Handle, usize), ExtError> {
    let handle = handle_of(value, kind)?;
    let len = value.get(HANDLE_KEY).and_then(|token| token.get("len")).and_then(Value::as_u64);
    match len.and_then(|len| usize::try_from(len).ok()) {
        Some(len) => Ok((handle, len)),
        None => Err(ExtError::Argument {
            method: String::new(),
            detail: format!("the handle of a sequence must hold its length `len`, not `{value}`"),
        }),
    }
}

/// The number of items a traversal asks the host for in one pull (`Walk`,
/// `Descendants`, `Trees`): a pull is one crossing, so a traversal of `n`
/// items crosses about `n / PULL_GROUP` times.
pub(crate) const PULL_GROUP: usize = 256;

/// The items of a traversal that a pull answered and the extension has not
/// taken yet: what the struct of a traversal holds beside its handle. A pull
/// asks for [`PULL_GROUP`] items; a group of fewer is the traversal's last.
/// The host keeps a copy of the items of the last group it answered: a
/// method of the traversal (a restriction), which consumes the struct,
/// sends last the number of them not handed over ([`held`](Pulled::held)),
/// and the host restricts those with the items not pulled yet.
#[derive(Debug)]
pub(crate) struct Pulled<T> {
    /// The items of the last pull not handed over yet, in order.
    group: VecDeque<T>,
    /// Whether the host has no item left.
    ended: bool,
}

impl<T> Default for Pulled<T> {
    fn default() -> Pulled<T> {
        Pulled { group: VecDeque::new(), ended: false }
    }
}

impl<T> Pulled<T> {
    /// The next item of the traversal: the first of the group, which `pull`
    /// fills when it is spent and the host has items left. `pull` sends the
    /// request for a group of at most the number it is given. A pull that
    /// fails answers its error once, and ends the traversal.
    pub(crate) fn next(&mut self, pull: impl FnOnce(usize) -> Result<Vec<T>, ExtError>) -> Option<Result<T, ExtError>> {
        if self.group.is_empty() && !self.ended {
            match pull(PULL_GROUP) {
                Ok(group) => {
                    self.ended = group.len() < PULL_GROUP;
                    self.group = group.into();
                }
                Err(error) => {
                    self.ended = true;
                    return Some(Err(error));
                }
            }
        }
        self.group.pop_front().map(Ok)
    }

    /// The number of items of the last pull not handed over yet.
    pub(crate) fn held(&self) -> usize {
        self.group.len()
    }
}

/// The most bytes of JSON text a pull of a sequence of data (`Rows`) asks
/// the host for, beside at most [`PULL_GROUP`] items: a row larger than this
/// crosses alone.
pub(crate) const PULL_BYTES: usize = 64 << 10;

/// The iteration of a sequence of `len` items: the items in order, which
/// `pull` fetches in groups from the index it is given (a crossing, at most
/// [`PULL_GROUP`] items and [`PULL_BYTES`] bytes). A pull that fails answers
/// its error once, and ends the iteration; so does a group with no item
/// before the end.
pub(crate) struct Paged<T, F> {
    /// The index of the first item not pulled yet.
    from: usize,
    /// The number of items of the sequence.
    len: usize,
    /// The items of the last pull not handed over yet, in order.
    group: VecDeque<T>,
    /// What fetches the group at an index.
    pull: F,
}

impl<T, F: FnMut(usize) -> Result<Vec<T>, ExtError>> Paged<T, F> {
    /// The iteration of the `len` items that `pull` fetches.
    pub(crate) fn new(len: usize, pull: F) -> Paged<T, F> {
        Paged { from: 0, len, group: VecDeque::new(), pull }
    }
}

impl<T, F: FnMut(usize) -> Result<Vec<T>, ExtError>> Iterator for Paged<T, F> {
    type Item = Result<T, ExtError>;

    fn next(&mut self) -> Option<Result<T, ExtError>> {
        if self.group.is_empty() && self.from < self.len {
            match (self.pull)(self.from) {
                Ok(group) if !group.is_empty() => {
                    self.from += group.len();
                    self.group = group.into();
                }
                Ok(_) => self.from = self.len,
                Err(error) => {
                    self.from = self.len;
                    return Some(Err(error));
                }
            }
        }
        self.group.pop_front().map(Ok)
    }
}

/// One line of `console` output, at `level`: in a WebAssembly module, the
/// native `console` with the level's name (`error`, `warn` or `info`);
/// compiled into a program (the cargo feature `native`), written to the log
/// sink of the current state's loaded extension, with the extension's stem
/// as its source.
///
/// # Errors
///
/// In a WebAssembly module, as the function `native`; compiled into a program,
/// [`ExtError::Unsupported`] when no native instance's state is current (on
/// a thread the extension started, while a thread ends).
pub fn console(level: LogLevel, text: &str) -> Result<(), ExtError> {
    #[cfg(not(feature = "native"))]
    {
        native(&json!({ "native": "console", "level": level, "text": text })).map(|_| ())
    }
    #[cfg(feature = "native")]
    {
        let state = crate::guest::state::current();
        let Some(loaded) = state.loaded.as_ref() else {
            return Err(ExtError::Unsupported { what: UNANSWERED.to_string() });
        };
        loaded.console.write(&loaded.extension.stem, level, text);
        Ok(())
    }
}

/// How a value is passed as an argument of a forwarded call: its JSON.
pub trait ToArg {
    /// The value as JSON.
    fn arg(&self) -> Value;
}

/// A value of a type with a JSON form: that form.
impl<T: JsonForm> ToArg for T {
    fn arg(&self) -> Value {
        self.to_value()
    }
}

/// A text: a string.
impl ToArg for str {
    fn arg(&self) -> Value {
        Value::String(self.to_string())
    }
}

/// A slice: a list, each item in its JSON form.
impl<T: JsonForm> ToArg for [T] {
    fn arg(&self) -> Value {
        Value::Array(self.iter().map(JsonForm::to_value).collect())
    }
}

/// Bytes: a string when they are a UTF-8 text, `{"base64": text}` otherwise,
/// forms of an artifact's content, which the host reads as bytes.
impl ToArg for [u8] {
    fn arg(&self) -> Value {
        bytes_to_value(self)
    }
}

/// The test hook: answers the requests in place of the host, where the module's import is not
/// compiled and the requests are sent as JSON (without the cargo feature `native`).
#[cfg(all(test, not(feature = "native")))]
pub(crate) mod hook {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::Value;

    /// A hook: the reply's envelope for a request.
    type Hook = Rc<dyn Fn(&Value) -> Value>;

    thread_local! {
        /// The hook installed on this thread.
        static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    /// Installs `hook` on this thread, in place of any other.
    pub(crate) fn install(hook: impl Fn(&Value) -> Value + 'static) {
        HOOK.with(|slot| *slot.borrow_mut() = Some(Rc::new(hook)));
    }

    /// Removes this thread's hook.
    pub(crate) fn remove() {
        HOOK.with(|slot| *slot.borrow_mut() = None);
    }

    /// The reply the hook gives `request`, as text; `None` when no hook is
    /// installed. The hook is cloned out of its slot before it runs.
    pub(super) fn answer(request: &[u8]) -> Option<Vec<u8>> {
        let hook = HOOK.with(|slot| slot.borrow().clone())?;
        let request: Value = serde_json::from_slice(request).ok()?;
        Some(hook(&request).to_string().into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::adapter::values::handle_to_value;
    use crate::api::lang::mint::Mode;

    /// Runs `body` with `hook` installed, then removes it.
    #[cfg(not(feature = "native"))]
    fn with_hook(hook: impl Fn(&Value) -> Value + 'static, body: impl FnOnce()) {
        hook::install(hook);
        body();
        hook::remove();
    }

    #[test]
    #[cfg(not(feature = "native"))]
    fn without_a_host_a_request_is_unsupported() {
        hook::remove();
        let error = native(&json!({ "native": "console", "level": "log", "text": "x" })).expect_err("no host");
        assert_eq!(error, ExtError::Unsupported { what: UNANSWERED.to_string() });
    }

    #[test]
    #[cfg(not(feature = "native"))]
    fn an_ok_reply_answers_its_value() {
        with_hook(
            |request| {
                assert_eq!(request["native"], "call");
                assert_eq!(request["method"], "option");
                json!({ "ok": { "answer": [request["args"][0].clone()] } })
            },
            || {
                let handle = Handle { kind: HandleKind::Node, call: 3, index: 1 };
                assert_eq!(call(handle, "option", vec![json!("x")]), Ok(json!({ "answer": ["x"] })));
            },
        );
    }

    #[test]
    #[cfg(not(feature = "native"))]
    fn an_err_reply_reads_back_as_the_error_whose_data_it_is() {
        let error = ExtError::Argument { method: "option".to_string(), detail: "argument 1 must be a string, not `3`".to_string() };
        let data = error.data();
        let message = error.to_string();
        with_hook(
            move |_| json!({ "err": data.clone(), "message": message.clone() }),
            || assert_eq!(native(&json!({ "native": "console", "level": "log", "text": "" })), Err(error)),
        );
    }

    #[test]
    #[cfg(not(feature = "native"))]
    fn a_reply_that_is_not_an_envelope_is_refused() {
        with_hook(
            |_| json!({ "neither": 1 }),
            || {
                let error = native(&json!({ "native": "module_handle", "path": "lang/text" })).expect_err("refused");
                assert_eq!(error, not_an_envelope("{\"neither\":1}"));
            },
        );
    }

    #[test]
    #[cfg(not(feature = "native"))]
    fn a_module_handle_is_asked_once() {
        let asked = std::rc::Rc::new(std::cell::Cell::new(0));
        let counted = std::rc::Rc::clone(&asked);
        with_hook(
            move |request| match request["native"].as_str() {
                Some("module_handle") => {
                    counted.set(counted.get() + 1);
                    json!({ "ok": handle_to_value(Handle { kind: HandleKind::Module, call: 0, index: 4 }) })
                }
                _ => json!({ "ok": request["handle"].clone() }),
            },
            || {
                let module = handle_to_value(Handle { kind: HandleKind::Module, call: 0, index: 4 });
                assert_eq!(call_module("lang/text", "chars_text", vec![]), Ok(module.clone()));
                assert_eq!(call_module("lang/text", "chars_text", vec![]), Ok(module));
            },
        );
        assert_eq!(asked.get(), 1);
    }

    #[test]
    fn a_handle_of_another_kind_is_refused() {
        let value = handle_to_value(Handle { kind: HandleKind::Node, call: 1, index: 0 });
        assert_eq!(handle_of(&value, HandleKind::Node), Ok(Handle { kind: HandleKind::Node, call: 1, index: 0 }));
        let error = handle_of(&value, HandleKind::Module).expect_err("another kind");
        assert_eq!(error, mismatch(HandleKind::Module, HandleKind::Node));
    }

    #[test]
    fn the_buffers_are_kept_by_address() {
        let first = insert(vec![1, 2, 3]);
        let empty = insert(Vec::new());
        assert_ne!(first, empty);
        assert_eq!(remove(first), Some(vec![1, 2, 3]));
        assert_eq!(remove(first), None);
        assert_eq!(remove(empty), Some(Vec::new()));
        // A freed buffer is gone; freeing it again does nothing.
        let other = insert(vec![9]);
        BUFFERS.with(|buffers| assert!(buffers.borrow().contains_key(&other)));
        remove(other);
        BUFFERS.with(|buffers| assert!(!buffers.borrow().contains_key(&other)));
    }

    #[test]
    fn a_request_stays_kept_until_the_host_frees_it() {
        // `flm_entry` reads its request and drops its copy before it keeps
        // its reply; the host then frees the request, and only then reads
        // and frees the reply.
        let request = insert(br#"{"entry":"define","extension":"x","args":["x","m"]}"#.to_vec());
        let copy = read_at(request).expect("the request is kept");
        assert_eq!(copy, br#"{"entry":"define","extension":"x","args":["x","m"]}"#.to_vec());
        drop(copy);
        let reply = insert(br#"{"ok":null}"#.to_vec());
        assert_ne!(reply, request, "the reply is given the address of a request still kept");
        assert_eq!(remove(request), Some(br#"{"entry":"define","extension":"x","args":["x","m"]}"#.to_vec()));
        BUFFERS.with(|buffers| assert_eq!(buffers.borrow().get(&reply), Some(&br#"{"ok":null}"#.to_vec())));
        assert_eq!(remove(reply), Some(br#"{"ok":null}"#.to_vec()));
        assert_eq!(read_at(request), None);
    }

    #[test]
    #[cfg(not(feature = "native"))]
    fn a_console_line_is_a_request() {
        with_hook(
            |request| {
                assert_eq!(request, &json!({ "native": "console", "level": "warn", "text": "careful" }));
                json!({ "ok": null })
            },
            || assert_eq!(console(LogLevel::Warn, "careful"), Ok(())),
        );
    }

    #[test]
    fn a_panic_s_message_is_kept_for_the_host() {
        assert_eq!(last_panic(), 0, "no panic was kept on this thread");
        PANIC_HOOK.call_once(keep_panics);
        let caught = std::panic::catch_unwind(|| panic!("the count overflowed"));
        assert!(caught.is_err());
        let packed = last_panic();
        let address = (packed as u64 >> 32) as usize;
        let len = (packed as u64 & 0xffff_ffff) as usize;
        let kept = BUFFERS.with(|buffers| buffers.borrow().iter().find(|(at, _)| (**at as u32 as usize) == address).map(|(_, bytes)| bytes.clone()));
        let kept = String::from_utf8(kept.expect("the message is kept in a buffer")).expect("a text");
        assert_eq!(kept.len(), len);
        assert!(kept.starts_with("the count overflowed"), "{kept}");
        assert!(kept.contains("abi.rs"), "{kept}");
    }

    #[test]
    fn the_arguments_are_their_json() {
        assert_eq!("a text".arg(), json!("a text"));
        assert_eq!(String::from("b").arg(), json!("b"));
        assert_eq!([Mode::Text, Mode::Math][..].arg(), json!(["text", "math"]));
        assert_eq!(vec![1usize, 2].arg(), json!([1, 2]));
        assert_eq!(b"bytes"[..].arg(), json!("bytes"));
    }
}
