//! The ABI between the host and a guest: the two host functions the module
//! imports (`flm.call`, `flm.take`), the guest's memory read and written, an
//! entry's request and reply, and the answer to a guest's request.
//!
//! JSON text crosses both ways in the guest's linear memory. The host writes
//! an entry's request `{"entry": NAME, "args": [ … ]}` into a buffer it takes
//! with `flm_alloc` and calls `flm_entry(ptr, len)`; the guest answers `(ptr
//! << 32) | len` of its reply, a buffer of its own that the host frees with
//! `flm_free` after reading it. While an entry runs, the guest sends its
//! requests with `flm.call(ptr, len)`, which answers the length of the reply;
//! `flm.take(ptr, len)` copies the reply into the guest's buffer.
//!
//! `flm.call` does not answer the request itself: it keeps the request's bytes
//! in the store's data and fails with the binding's own error, which suspends
//! the running call (wasmi's resumable calls). The function driving the call
//! ([`WasmInstance::call_entry`](super::instance::WasmInstance::call_entry))
//! then runs the request with the store free, so that a nested handler call
//! may enter the same instance, and resumes the guest with the reply's length.

use serde_json::Value;
use wasmi::{Caller, Error, Extern, Linker, Memory};

use super::instance::WasmExtension;
use crate::binding::natives::{self, Native};
use crate::binding::EntryArg;
use crate::runtime::ExtError;

/// The store's data of an instance: the memory limit, the request `flm.call`
/// kept, the reply `flm.take` copies.
pub(crate) struct Guest {
    /// The memory limit, which records a refused growth.
    pub limiter: Limiter,
    /// The bytes of the request of the guest's last `flm.call`, kept until
    /// the driving function takes them.
    pub request: Option<Vec<u8>>,
    /// The bytes of the reply to the guest's last request, kept until its
    /// `flm.take`.
    pub reply: Option<Vec<u8>>,
}

/// The binding's own resource limiter: refuses a memory growth past `limit`
/// and records the refusal, so that the trap that follows is read as the
/// memory limit. Tables and instances are not limited beyond wasmi's
/// defaults.
pub(crate) struct Limiter {
    /// The memory limit in bytes; `None` is unlimited.
    pub limit: Option<usize>,
    /// Whether a growth was refused since the mark was last cleared (at the
    /// outermost entry into the instance).
    pub refused: bool,
}

impl wasmi::ResourceLimiter for Limiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmi_core::LimiterError> {
        if self.limit.is_some_and(|limit| desired > limit) {
            self.refused = true;
            return Ok(false);
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmi_core::LimiterError> {
        Ok(true)
    }

    // The counts of wasmi's own `StoreLimits` by default.
    fn instances(&self) -> usize {
        10_000
    }

    fn tables(&self) -> usize {
        10_000
    }

    fn memories(&self) -> usize {
        10_000
    }
}

/// The text of the error `flm.call` suspends the call with; read by a person
/// only when a guest calls `flm.call` outside `flm_entry` (from `flm_alloc`,
/// say), where nothing resumes it and the call traps with it.
const SUSPENDED: &str = "`flm.call` must be called while `flm_entry` runs";

/// The linker of the two host functions, `flm.call` and `flm.take`.
///
/// # Errors
///
/// wasmi's text when a function cannot be defined (never, for two distinct
/// names).
pub(crate) fn linker(engine: &wasmi::Engine) -> Result<Linker<Guest>, String> {
    let mut linker = Linker::<Guest>::new(engine);
    linker.func_wrap("flm", "call", call).map_err(|error| error.to_string())?;
    linker.func_wrap("flm", "take", take).map_err(|error| error.to_string())?;
    Ok(linker)
}

/// The memory the guest exports, through the caller.
fn memory_of(caller: &Caller<'_, Guest>) -> Result<Memory, Error> {
    caller.get_export("memory").and_then(Extern::into_memory).ok_or_else(|| Error::new("the module must export its memory as `memory`"))
}

/// `flm.call(ptr, len)`: keeps the request at `[ptr, ptr + len)` and suspends
/// the call; the driving function resumes it with the reply's length.
fn call(mut caller: Caller<'_, Guest>, ptr: i32, len: i32) -> Result<i32, Error> {
    let memory = memory_of(&caller)?;
    let (start, length) = (ptr as u32 as usize, len as u32 as usize);
    let bytes = memory
        .data(&caller)
        .get(start..start.saturating_add(length))
        .ok_or_else(|| Error::new("the request given to `flm.call` must lie within the module's memory"))?
        .to_vec();
    caller.data_mut().request = Some(bytes);
    Err(Error::new(SUSPENDED))
}

/// `flm.take(ptr, len)`: copies the reply to the guest's last request to
/// `ptr`; `len` must be its length.
fn take(mut caller: Caller<'_, Guest>, ptr: i32, len: i32) -> Result<(), Error> {
    let memory = memory_of(&caller)?;
    let reply = caller.data_mut().reply.take().unwrap_or_default();
    if len as u32 as usize != reply.len() {
        return Err(Error::new(format!(
            "`flm.take` must be given the length of the reply, {} bytes, not {}",
            reply.len(),
            len as u32
        )));
    }
    memory
        .write(&mut caller, ptr as u32 as usize, &reply)
        .map_err(|_| Error::new("the buffer given to `flm.take` must lie within the module's memory"))
}

/// The JSON text of the request of the entry `entry` with `args`: `{"entry":
/// NAME, "args": [ … ]}`, each argument a JSON value (a name as a string). The
/// entry's name comes first, so that a guest may read it at a fixed place.
pub(crate) fn entry_request(entry: &str, args: &[EntryArg]) -> String {
    let args: Vec<Value> = args
        .iter()
        .map(|arg| match arg {
            EntryArg::Text(text) => Value::String(text.clone()),
            EntryArg::Json(value) => value.clone(),
        })
        .collect();
    format!("{{\"entry\":{},\"args\":{}}}", Value::String(entry.to_string()), Value::Array(args))
}

/// What the guest answered in the reply `reply` of the entry call `what`:
/// the `ok` of `{"ok": value}`, or the guest's own error of `{"err": data,
/// "message": text}`, read by [`ExtError::from_data`].
///
/// # Errors
///
/// A reply the binding cannot read (the guest's own error is the inner
/// `Err`): [`ExtError::Value`], the reply is not JSON or nests too deep;
/// [`ExtError::Handler`], it holds neither `ok` nor `err`, or its `err` is
/// not an error's data.
pub(crate) fn read_reply(reply: &[u8], what: &str) -> Result<Result<Value, ExtError>, ExtError> {
    let text = std::str::from_utf8(reply)
        .map_err(|error| ExtError::Value { detail: format!("the answer of {what} must be JSON: {error}") })?;
    let mut value = natives::parse_reply(Some(text), what)?;
    if let Some(ok) = value.get_mut("ok") {
        return Ok(Ok(ok.take()));
    }
    match value.get("err") {
        Some(data) => match ExtError::from_data(data) {
            Ok(error) => Ok(Err(error)),
            Err(detail) => Err(ExtError::Handler {
                detail: format!("the error {what} answered must be the data of an `ExtError`: {detail}"),
                data: Value::Null,
            }),
        },
        None => Err(ExtError::Handler { detail: format!("the answer of {what} must hold `ok` or `err`"), data: Value::Null }),
    }
}

/// The envelope answering the request `bytes` of a guest of `extension`, as
/// JSON text: the request read (JSON text, its depth bounded, one of the
/// natives' shapes), then run by [`natives::answer`] on the running call's
/// frame. A request that cannot be read answers an `err` envelope.
pub(crate) fn answer_request(extension: &WasmExtension, bytes: &[u8]) -> Vec<u8> {
    let stem = &extension.extension.stem;
    let what = format!("a request of the extension `{stem}`");
    let native = std::str::from_utf8(bytes)
        .map_err(|error| ExtError::Argument { method: String::new(), detail: format!("{what} must be JSON text: {error}") })
        .and_then(|text| natives::parse(&what, text))
        .and_then(|value| {
            serde_json::from_value::<Native>(value).map_err(|error| ExtError::Argument {
                method: String::new(),
                detail: format!("{what} must be one of the requests the host answers: {error}"),
            })
        });
    let envelope = match native {
        Ok(native) => natives::answer(&extension.extension, extension.id, extension.host.console(), native),
        Err(error) => natives::envelope(Err(error)),
    };
    envelope.to_string().into_bytes()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::adapter::handles::CallTable;
    use crate::binding::{frames, next_extension_id, Registration};
    use crate::engine::ExtensionIdentity;
    use crate::wasm::{WasmHost, WasmSettings};
    use serde_json::json;

    /// A loaded extension of stem `stem` over an empty module.
    fn extension(stem: &str) -> Arc<WasmExtension> {
        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, wat::parse_str("(module)").expect("the text parses")).expect("it compiles");
        Arc::new(WasmExtension {
            id: next_extension_id(),
            host: WasmHost::new(WasmSettings::default()).with_console(Arc::new(|_, _| {})),
            settings: WasmSettings::default(),
            extension: ExtensionIdentity { identity: stem.to_string(), stem: stem.to_string() },
            path: format!("{stem}.wasm"),
            engine,
            module,
            initial_memory: 0,
        })
    }

    #[test]
    fn a_request_sent_while_another_extension_s_handler_runs_is_refused() {
        let (a, b) = (extension("a"), extension("b"));
        let request = br#"{"native":"lend_lower","rule":"r"}"#;
        let answered = |extension: &WasmExtension| -> Value {
            serde_json::from_slice(&answer_request(extension, request)).expect("an envelope is JSON")
        };
        let mut table = CallTable::new(frames::next_call());
        let registration = Registration {
            binding: Arc::clone(&a) as Arc<dyn crate::binding::Binding>,
            package: "_".to_string(),
            feature: "words".to_string(),
            settings: Arc::new(Value::Null),
        };
        let frame = frames::Frame { call: table.call(), registration, table: &mut table };
        // An instance of `b` created while `a`'s handler call runs sends its
        // request inside that call.
        let (theirs, own) = frames::enter(frame, || (answered(&b), answered(&a)));
        assert_eq!(
            theirs["message"],
            "the extension `b` must build the lowering rule `r` from a feature's `register` or from a handler that \
             `register` declared"
        );
        assert_eq!(own["ok"]["@handle"]["kind"], "lower", "{own}");
    }

    #[test]
    fn an_entry_request_writes_the_name_first_and_the_arguments_as_values() {
        let args = [EntryArg::Text("_".to_string()), EntryArg::Json(json!({ "level": 2 }))];
        assert_eq!(entry_request("register", &args), r#"{"entry":"register","args":["_",{"level":2}]}"#);
    }

    #[test]
    fn a_reply_answers_its_ok_or_the_guest_s_error() {
        assert_eq!(read_reply(br#"{"ok":[1,2]}"#, "the entry"), Ok(Ok(json!([1, 2]))));
        let error = ExtError::Value { detail: "bad".to_string() };
        let reply = natives::envelope(Err(error.clone())).to_string();
        assert_eq!(read_reply(reply.as_bytes(), "the entry"), Ok(Err(error)));
        assert_eq!(
            read_reply(br#"{"result":1}"#, "the entry `x`"),
            Err(ExtError::Handler { detail: "the answer of the entry `x` must hold `ok` or `err`".to_string(), data: Value::Null })
        );
        assert_eq!(
            read_reply(br#"{"err":{"kind":"nope"}}"#, "the entry `x`"),
            Err(ExtError::Handler {
                detail: "the error the entry `x` answered must be the data of an `ExtError`: `kind` must be `handler`, \
                         `stale_handle`, `argument`, `name_outside_extension`, `value`, `core`, `host`, `unsupported` or \
                         `stale_durable`, not `nope`"
                    .to_string(),
                data: Value::Null
            })
        );
        let not_json = read_reply(b"{", "the entry `x`").expect_err("refused");
        assert!(not_json.to_string().starts_with("the answer of the entry `x` must be JSON: "), "{not_json}");
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert_eq!(
            read_reply(deep.as_bytes(), "the entry `x`").expect_err("refused").to_string(),
            "the answer of the entry `x` must nest lists and maps at most 127 levels deep"
        );
    }
}
