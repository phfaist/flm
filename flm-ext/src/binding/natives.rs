//! The natives every binding shares: the requests an extension makes of the
//! host ([`Native`]), the body of each lending, the envelope a native answers,
//! the refusal of a native called while no handler runs, and the reading of
//! the JSON a script or a guest passes (its depth bounded).
//!
//! | Native | What it does |
//! |---|---|
//! | `call(handle, method, args)` | [`dispatch::call`] on the running call's table |
//! | `module_handle(path)` | the handle of the module `path` ([`module_handle`]) |
//! | `lend_stage(decl, needs)` | lends a stage of the extension's own ([`lend_stage`], a [`BindingStage`]) |
//! | `lend_expand(element, policy)` | lends an expansion hook of the extension's own ([`lend_expand`], a [`BindingExpand`]) |
//! | `lend_lower(rule)` | lends a lowering rule's body of the extension's own ([`lend_lower`], a [`BindingLower`]) |
//! | `lend_rewrite(decl, needs)` | lends a rewrite stage of the extension's own ([`lend_rewrite`], a [`BindingRewrite`]) |
//! | `lend_provider(name)` | lends a citation provider of the extension's own ([`lend_provider`], a [`BindingProvider`]) |
//! | `lend_template(name)` | lends a template module of the extension's own ([`lend_template`], a [`BindingTemplate`]) |
//! | `lend_aspect(kind, element, methods)` | lends an element's handler aspect of the extension's own ([`lend_referenceable`], a [`BindingReferenceable`]; [`lend_sectioning`], a [`BindingSectioning`]) |
//! | `lend_late_init(index)` | lends a late-initialization hook of the extension's own ([`lend_late_init`], a [`BindingLateInit`]) |
//! | `lend_finalizer(index)` | lends a finalizer of the extension's own ([`lend_finalizer`], a [`BindingFinalizer`]) |
//! | `console(level, text)` | the host's console sink |
//!
//! # One body, two front doors
//!
//! The body of a lending takes the running call's frame and the lending's
//! arguments, typed, and builds the shared handler object over the frame's
//! registration; it reads no value and cannot fail. `lend_aspect` has one
//! body per kind of aspect, since the kind decides the object's type. Two
//! front doors reach the bodies:
//!
//! - **The text bindings** (QuickJS, the engine `wasm`): [`answer`] takes the
//!   request ([`Native`], its arguments JSON values), looks the frame up,
//!   reads the arguments into their types, calls the body, lends the object
//!   into the call's table and answers its handle, as a value, in the
//!   envelope.
//! - **The guest client compiled into a program** (the engine `native`): each
//!   record-and-lend method of `flm_ext::guest` calls the body inside the
//!   closure of the frame of the call it runs in (`guest::abi::in_frame`), with
//!   the arguments it read from the extension's object, and hands the object
//!   to the API method that takes it (`FeatureBuilder::stage`, …) in the same
//!   closure, with no handle; `ElementSpec::with`, whose aspect crosses later
//!   inside an element's declaration, lends the object into the table and
//!   keeps the handle, as a guest of a text binding keeps the handle its
//!   lending answered.
//!
//! The other natives (`call`, `module_handle`, `console`) serve the text
//! bindings alone. Compiled into a program, the guest client calls the
//! extension API directly where a text binding's guest sends `call` or
//! `module_handle` (`guest::abi::typed`), and writes a line of console
//! output to the sink its instance's state holds, which the engine `native`
//! gave it, where a text binding's guest sends `console`. The refusal
//! [`outside`] names what the extension must do from a handler ([`Act`]), in
//! one text for both front doors.
//!
//! What a native needs of the running handler, [`answer`] reads from the
//! thread's frames ([`frames`]): the frame is looked up before the values are
//! read, so a native that needs one, called while no handler of its extension
//! runs, answers the refusal [`outside`] whatever its arguments. The innermost
//! frame may be another extension's: an instance created on a thread while
//! that extension's handler call runs there runs its module's top level (or
//! `define`) inside that call, and natively the objects of an instance the
//! engine `native` discards there are dropped inside it; a native sent from
//! either, or natively a request or a lending of the typed route, is refused
//! likewise. A binding reads its own wire format first
//! (QuickJS each text with [`parse`]); only a JSON text that does not parse
//! or nests too deep is refused before the frame lookup.
//!
//! The envelope is the JSON value `{"ok": <value>}`, or `{"err": <the
//! error's data>, "message": <its text>}` for a failure ([`ExtError::data`]
//! and its `Display`). A native never fails into the engine: the prelude (or
//! the guest client) turns an `err` envelope into the extension's own error.
//!
//! The JSON texts a script passes are parsed with `serde_json` under its
//! default recursion limit: a value may nest lists and maps at most
//! [`MAX_JSON_DEPTH`] (127) levels deep, which is about 63 levels of a
//! presentation tree given to `pres_tree` (each level a map and its list of
//! children), and likewise for a table item. A deeper value is refused as an
//! argument error naming the bound, a thrown `FlmError` the script may catch;
//! uncaught, the handler fails (`flm.ext.handler-failed`). A handler's reply
//! (a rule body's `Lowering`, a hook's `Expansion`, …) is held to the same
//! bound when the binding reads it ([`parse_reply`]): a deeper reply fails the
//! handler, with a message naming the bound.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use serde_json::json;

use super::frames::FrameView;
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use super::frames;
use super::handlers::{
    BindingExpand, BindingFinalizer, BindingLateInit, BindingLower, BindingProvider, BindingReferenceable, BindingRewrite,
    BindingSectioning, BindingStage, BindingTemplate,
};
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::adapter::dispatch;
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::adapter::handles::{module_handle, CallTable, Entry, Handle};
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::adapter::values::{handle_from_value, handle_to_value, is_durable, JsonForm};
use crate::api::core::stage::{Needs, RewriteDecl, StageDecl};
use crate::api::lang::rewrites::userdefs::ExpandPolicy;
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::engine::ExtensionIdentity;
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::host::{ConsoleLevel, ConsoleSink};
use crate::runtime::ExtError;

/// One request to the host: the WebAssembly ABI carries it as JSON
/// (`{"native": "lend_stage", "decl": …, "needs": …}`); the QuickJS glue builds it from
/// its natives' arguments.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "native", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Native {
    /// Calls `method` on the object `handle` stands for in the running handler call,
    /// with the list `args`; answers the dispatcher's answer.
    Call {
        /// The handle, or the durable token of a durable object
        /// ([`receiver`]).
        handle: Value,
        /// The method's name.
        method: String,
        /// The arguments, a list.
        args: Value,
    },
    /// The handle of the API module `path` (`"lang/text"`, …), valid in every call.
    ModuleHandle {
        /// The module's path.
        path: String,
    },
    /// Lends a [`BindingStage`] of the running feature; answers its handle.
    LendStage {
        /// The declaration, in its JSON shape.
        decl: Value,
        /// The needs, in their JSON shape.
        needs: Value,
    },
    /// Lends a [`BindingExpand`] of the running feature; answers its handle.
    LendExpand {
        /// The element the hook expands.
        element: String,
        /// The policy's name (`"per-plan"`, `"always"`).
        policy: String,
    },
    /// Lends a [`BindingLower`] of the running feature; answers its handle.
    LendLower {
        /// The rule's name.
        rule: String,
    },
    /// Lends a [`BindingRewrite`] of the running feature; answers its handle.
    LendRewrite {
        /// The declaration, in its JSON shape.
        decl: Value,
        /// The needs, in their JSON shape.
        needs: Value,
    },
    /// Lends a [`BindingProvider`] of the running feature; answers its handle.
    LendProvider {
        /// The provider's name.
        name: String,
    },
    /// Lends a [`BindingTemplate`] of the running feature; answers its handle.
    LendTemplate {
        /// The template module's name.
        name: String,
    },
    /// Lends the handler aspect of kind `kind` of the running feature for the element
    /// `element`; answers its handle.
    LendAspect {
        /// `"Referenceable"` or `"Sectioning"`.
        kind: String,
        /// The element.
        element: String,
        /// The names of the methods the extension's object has, a list of texts.
        methods: Value,
    },
    /// Lends a [`BindingLateInit`] of the running feature; answers its handle.
    LendLateInit {
        /// The hook's position among the feature's late-initialization hooks.
        index: u32,
    },
    /// Lends a [`BindingFinalizer`] of the running feature; answers its handle.
    LendFinalizer {
        /// The finalizer's position among the feature's finalizers.
        index: u32,
    },
    /// One line of `console` output; answers `null`.
    Console {
        /// The `console` method's name (`log`, `info`, …).
        level: String,
        /// The joined, stringified arguments.
        text: String,
    },
}

/// What an extension must do from a handler, which the refusal [`outside`]
/// names: the act of a native that needs the running call, in one text for
/// both front doors.
#[derive(Clone, Copy)]
pub(crate) enum Act<'a> {
    /// A call of the method or function of the extension API of this name:
    /// the native `call`, and a request of the guest client's typed route.
    Call(&'a str),
    /// The lending of a stage (`lend_stage`).
    Stage,
    /// The lending of the expansion hook of the element of this name
    /// (`lend_expand`).
    Expand(&'a str),
    /// The lending of the body of the lowering rule of this name
    /// (`lend_lower`).
    Lower(&'a str),
    /// The lending of a rewrite stage (`lend_rewrite`).
    Rewrite,
    /// The lending of the citation provider of this name (`lend_provider`).
    Provider(&'a str),
    /// The lending of the template module of this name (`lend_template`).
    Template(&'a str),
    /// The lending of a handler aspect (`lend_aspect`).
    Aspect {
        /// The aspect's kind: `Referenceable` or `Sectioning`.
        kind: &'a str,
        /// The element.
        element: &'a str,
    },
    /// The lending of a late-initialization hook (`lend_late_init`).
    LateInit,
    /// The lending of a finalizer (`lend_finalizer`).
    Finalizer,
}

/// "call `chars` of the extension API", "declare a stage", ….
impl fmt::Display for Act<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Act::Call(method) => write!(f, "call `{method}` of the extension API"),
            Act::Stage => f.write_str("declare a stage"),
            Act::Expand(element) => write!(f, "attach the expansion hook of the element `{element}`"),
            Act::Lower(rule) => write!(f, "build the lowering rule `{rule}`"),
            Act::Rewrite => f.write_str("declare a rewrite stage"),
            Act::Provider(name) => write!(f, "contribute the citation provider `{name}`"),
            Act::Template(name) => write!(f, "contribute the template module `{name}`"),
            Act::Aspect { kind, element } => write!(f, "attach the aspect `{kind}` of the element `{element}`"),
            Act::LateInit => f.write_str("register a late-initialization hook"),
            Act::Finalizer => f.write_str("register a finalizer"),
        }
    }
}

/// The body of `lend_stage`: the stage of the running feature of the
/// declaration `decl` and the needs `needs`, over the frame's registration.
pub(crate) fn lend_stage(view: &FrameView<'_, '_>, decl: StageDecl, needs: Needs) -> Arc<BindingStage> {
    Arc::new(BindingStage { registration: view.registration.clone(), decl, needs })
}

/// The body of `lend_rewrite`: the rewrite stage of the running feature of
/// the declaration `decl` and the needs `needs`, over the frame's
/// registration.
pub(crate) fn lend_rewrite(view: &FrameView<'_, '_>, decl: RewriteDecl, needs: Needs) -> Arc<BindingRewrite> {
    Arc::new(BindingRewrite { registration: view.registration.clone(), decl, needs })
}

/// The body of `lend_expand`: the expansion hook of the running feature for
/// the element `element`, of the policy `policy`, over the frame's
/// registration.
pub(crate) fn lend_expand(view: &FrameView<'_, '_>, element: String, policy: ExpandPolicy) -> Arc<BindingExpand> {
    Arc::new(BindingExpand { registration: view.registration.clone(), element, policy })
}

/// The body of `lend_lower`: the body of the lowering rule `rule` of the
/// running feature, over the frame's registration.
pub(crate) fn lend_lower(view: &FrameView<'_, '_>, rule: String) -> Arc<BindingLower> {
    Arc::new(BindingLower { registration: view.registration.clone(), rule })
}

/// The body of `lend_provider`: the citation provider `name` of the running
/// feature, over the frame's registration.
pub(crate) fn lend_provider(view: &FrameView<'_, '_>, name: String) -> Arc<BindingProvider> {
    Arc::new(BindingProvider { registration: view.registration.clone(), name })
}

/// The body of `lend_template`: the template module `name` of the running
/// feature, over the frame's registration.
pub(crate) fn lend_template(view: &FrameView<'_, '_>, name: String) -> Arc<BindingTemplate> {
    Arc::new(BindingTemplate { registration: view.registration.clone(), name })
}

/// The body of `lend_aspect` for the kind `Referenceable`: the aspect of the
/// running feature for the element `element`, over the frame's
/// registration.
pub(crate) fn lend_referenceable(view: &FrameView<'_, '_>, element: String) -> Arc<BindingReferenceable> {
    Arc::new(BindingReferenceable { registration: view.registration.clone(), element })
}

/// The body of `lend_aspect` for the kind `Sectioning`: the aspect of the
/// running feature for the element `element`, whose object has the methods
/// `methods`, over the frame's registration.
pub(crate) fn lend_sectioning(view: &FrameView<'_, '_>, element: String, methods: Vec<String>) -> Arc<BindingSectioning> {
    Arc::new(BindingSectioning { registration: view.registration.clone(), element, methods })
}

/// The body of `lend_late_init`: the late-initialization hook of the running
/// feature at the position `index`, over the frame's registration.
pub(crate) fn lend_late_init(view: &FrameView<'_, '_>, index: usize) -> Arc<BindingLateInit> {
    Arc::new(BindingLateInit { registration: view.registration.clone(), index })
}

/// The body of `lend_finalizer`: the finalizer of the running feature at the
/// position `index`, over the frame's registration.
pub(crate) fn lend_finalizer(view: &FrameView<'_, '_>, index: usize) -> Arc<BindingFinalizer> {
    Arc::new(BindingFinalizer { registration: view.registration.clone(), index })
}

/// Runs `native` for the extension `extension`, the loaded extension of number `load`
/// ([`Binding::id`](super::Binding::id)), on the running call's frame; answers its
/// envelope: the front door of the text bindings. The frame is looked up first: a
/// native that needs one, called while no handler runs or while the innermost frame's
/// registration is another loaded extension's, answers the `outside` refusal whatever
/// its arguments, and the frame's table is not borrowed ([`frames::with_current`]).
/// A lending then reads its arguments, calls its body, lends the object into the
/// call's table and answers its handle.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn answer(extension: &ExtensionIdentity, load: u64, console: &ConsoleSink, native: Native) -> Value {
    envelope(match native {
        Native::Call { handle, method, args } => in_frame(extension, load, Act::Call(&method), |view| {
            let target = receiver(view.table, &handle).map_err(|error| for_method(&method, error))?;
            dispatch::call(view.table, target, &method, &args)
        }),
        Native::ModuleHandle { path } => module_handle(&path).map(handle_to_value).ok_or_else(|| ExtError::Argument {
            method: "module_handle".to_string(),
            detail: format!("`flm:{path}` must be a module of the extension API that has functions"),
        }),
        Native::LendStage { decl, needs } => in_frame(extension, load, Act::Stage, |view| {
            let stage = lend_stage(view, StageDecl::from_value(&decl)?, Needs::from_value(&needs)?);
            Ok(lent(view.table, Entry::Stage(stage)))
        }),
        Native::LendExpand { element, policy } => in_frame(extension, load, Act::Expand(&element), |view| {
            let hook = lend_expand(view, element.clone(), ExpandPolicy::from_value(&Value::String(policy))?);
            Ok(lent(view.table, Entry::Expand(hook)))
        }),
        Native::LendLower { rule } => in_frame(extension, load, Act::Lower(&rule), |view| {
            let body = lend_lower(view, rule.clone());
            Ok(lent(view.table, Entry::Lower(body)))
        }),
        Native::LendRewrite { decl, needs } => in_frame(extension, load, Act::Rewrite, |view| {
            let stage = lend_rewrite(view, RewriteDecl::from_value(&decl)?, Needs::from_value(&needs)?);
            Ok(lent(view.table, Entry::RewriteStage(stage)))
        }),
        Native::LendProvider { name } => in_frame(extension, load, Act::Provider(&name), |view| {
            let provider = lend_provider(view, name.clone());
            Ok(lent(view.table, Entry::CitationProvider(provider)))
        }),
        Native::LendTemplate { name } => in_frame(extension, load, Act::Template(&name), |view| {
            let template = lend_template(view, name.clone());
            Ok(lent(view.table, Entry::TemplateModule(template)))
        }),
        Native::LendAspect { kind, element, methods } => {
            in_frame(extension, load, Act::Aspect { kind: &kind, element: &element }, |view| {
                let methods: Vec<String> = serde_json::from_value(methods).map_err(|_| ExtError::Argument {
                    method: "lend_aspect".to_string(),
                    detail: "the aspect's methods must be a list of texts".to_string(),
                })?;
                let entry = match kind.as_str() {
                    "Referenceable" => Entry::Referenceable(lend_referenceable(view, element.clone())),
                    "Sectioning" => Entry::Sectioning(lend_sectioning(view, element.clone(), methods)),
                    other => {
                        return Err(ExtError::Argument {
                            method: "lend_aspect".to_string(),
                            detail: format!(
                                "the kind of a handler aspect must be `Referenceable` or `Sectioning`, not `{other}`"
                            ),
                        })
                    }
                };
                Ok(lent(view.table, entry))
            })
        }
        Native::LendLateInit { index } => in_frame(extension, load, Act::LateInit, |view| {
            let hook = lend_late_init(view, index as usize);
            Ok(lent(view.table, Entry::LateInit(hook)))
        }),
        Native::LendFinalizer { index } => in_frame(extension, load, Act::Finalizer, |view| {
            let hook = lend_finalizer(view, index as usize);
            Ok(lent(view.table, Entry::Finalizer(hook)))
        }),
        Native::Console { level, text } => {
            console(ConsoleLevel::from_name(&level), &text);
            Ok(Value::Null)
        }
    })
}

/// The handle of `entry`, lent into `table`, as a value.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
fn lent<'c>(table: &mut CallTable<'c>, entry: Entry<'c>) -> Value {
    handle_to_value(table.lend(entry))
}

/// What `body` answers on the frame of the running call of the loaded
/// extension `load`, or the refusal of `act` by the extension `extension`
/// when no frame of that loaded extension is the innermost one.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
fn in_frame(
    extension: &ExtensionIdentity,
    load: u64,
    act: Act<'_>,
    body: impl for<'v, 'c> FnOnce(&mut FrameView<'v, 'c>) -> Result<Value, ExtError>,
) -> Result<Value, ExtError> {
    frames::with_current(load, body).unwrap_or_else(|| Err(outside(&extension.stem, act)))
}

/// The receiver of a `call` written as `value`: a handle, or a durable token,
/// whose object is resolved against the call's durable table and lent into
/// the call ([`dispatch::lend_durable`]), so that a method of a durable
/// object (a compiled template's `placeholders`) is called on it as on a
/// per-call one.
///
/// # Errors
///
/// `value` is not a handle or a durable token, or the token names no object
/// of the call's durable table.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
fn receiver(table: &mut CallTable<'_>, value: &Value) -> Result<Handle, ExtError> {
    if is_durable(value) {
        dispatch::lend_durable(table, value)
    } else {
        handle_from_value(value)
    }
}

/// `error` with the method `method` named, when it is an argument error that names
/// none (a value of `call` that does not read: "`chars`: the arguments must be JSON
/// text: …").
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn for_method(method: &str, error: ExtError) -> ExtError {
    match error {
        ExtError::Argument { method: found, detail } if found.is_empty() => {
            ExtError::Argument { method: method.to_string(), detail }
        }
        other => other,
    }
}

/// The envelope of `result`.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn envelope(result: Result<Value, ExtError>) -> Value {
    match result {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "err": error.data(), "message": error.to_string() }),
    }
}

/// The error of a native called while no handler call of its extension runs on
/// this thread (at a module's top level, in the default export, in `define`):
/// the extension of stem `stem` must `act` ("call `chars` of the extension
/// API") from a handler.
pub(crate) fn outside(stem: &str, act: Act<'_>) -> ExtError {
    ExtError::Handler {
        detail: format!(
            "the extension `{stem}` must {act} from a feature's `register` or from a handler that `register` declared"
        ),
        data: Value::Null,
    }
}

/// The deepest nesting of lists and maps a value crossing from a script may
/// have: `serde_json`'s default recursion limit (128) less one, the bound
/// under which its parser answers. One level of a presentation tree
/// (`pres_tree`'s `{ element, payload, children: [...] }`) costs two (the map
/// and its list of children), so a tree crosses with about 63 levels, and so
/// does a table item holding a tree written the same way. The bound stays: an
/// unbounded parse would grow the stack of the worker thread with the depth
/// of the data.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) const MAX_JSON_DEPTH: usize = 127;

/// The deepest nesting of lists and maps in the JSON text `text` (strings
/// skipped), counting the outermost value as 1.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn json_depth(text: &str) -> usize {
    let (mut depth, mut deepest, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for byte in text.bytes() {
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

/// `text` parsed as JSON; `what` names it in the error (the method is
/// added in front of it, "`chars`: the arguments must be JSON text: …").
/// Lists and maps nested deeper than [`MAX_JSON_DEPTH`] are refused with a
/// message naming the bound.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn parse(what: &str, text: &str) -> Result<Value, ExtError> {
    if json_depth(text) > MAX_JSON_DEPTH {
        return Err(ExtError::Argument {
            method: String::new(),
            detail: format!(
                "{what} must nest lists and maps at most {MAX_JSON_DEPTH} levels deep (a presentation tree about 63 levels deep)"
            ),
        });
    }
    serde_json::from_str(text).map_err(|error| ExtError::Argument {
        method: String::new(),
        detail: format!("{what} must be JSON text: {error}"),
    })
}

/// The reply `text` of an entry point of the handler call `what` (as the
/// messages name it): `null` when it answers nothing; a reply nesting lists
/// and maps deeper than [`MAX_JSON_DEPTH`] is refused, as a value a script
/// passes to a native is.
///
/// # Errors
///
/// [`ExtError::Value`]: the reply nests too deep, or is not JSON.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn parse_reply(text: Option<&str>, what: &str) -> Result<Value, ExtError> {
    match text {
        None => Ok(Value::Null),
        Some(text) if json_depth(text) > MAX_JSON_DEPTH => Err(ExtError::Value {
            detail: format!("the answer of {what} must nest lists and maps at most {MAX_JSON_DEPTH} levels deep"),
        }),
        Some(text) => serde_json::from_str(text)
            .map_err(|error| ExtError::Value { detail: format!("the answer of {what} must be JSON: {error}") }),
    }
}

#[cfg(all(test, any(feature = "quickjs", feature = "wasm")))]
mod tests {
    use super::*;
    use crate::binding::next_extension_id;

    /// The request `text` read as a [`Native`], as the WebAssembly ABI reads it.
    fn read(text: &str) -> Result<Native, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Asserts that the request `text` reads as `native`.
    fn reads(text: &str, native: Native) {
        assert_eq!(read(text).map_err(|error| error.to_string()), Ok(native), "{text}");
    }

    #[test]
    fn call_reads_from_its_request() {
        let (handle, method, args) = (json!({"@handle": 3}), "chars".to_string(), json!([]));
        reads(r#"{"native": "call", "handle": {"@handle": 3}, "method": "chars", "args": []}"#, Native::Call { handle, method, args });
    }

    #[test]
    fn module_handle_reads_from_its_request() {
        reads(r#"{"native": "module_handle", "path": "lang/text"}"#, Native::ModuleHandle { path: "lang/text".to_string() });
    }

    #[test]
    fn lend_stage_reads_from_its_request() {
        let (decl, needs) = (json!({"name": "wc.count"}), json!({}));
        reads(r#"{"native": "lend_stage", "decl": {"name": "wc.count"}, "needs": {}}"#, Native::LendStage { decl, needs });
    }

    #[test]
    fn lend_expand_reads_from_its_request() {
        let (element, policy) = ("wc".to_string(), "always".to_string());
        reads(r#"{"native": "lend_expand", "element": "wc", "policy": "always"}"#, Native::LendExpand { element, policy });
    }

    #[test]
    fn lend_lower_reads_from_its_request() {
        reads(r#"{"native": "lend_lower", "rule": "wc.rule"}"#, Native::LendLower { rule: "wc.rule".to_string() });
    }

    #[test]
    fn lend_rewrite_reads_from_its_request() {
        let (decl, needs) = (json!({"name": "wc.dash"}), json!({}));
        reads(r#"{"native": "lend_rewrite", "decl": {"name": "wc.dash"}, "needs": {}}"#, Native::LendRewrite { decl, needs });
    }

    #[test]
    fn lend_provider_reads_from_its_request() {
        reads(r#"{"native": "lend_provider", "name": "local"}"#, Native::LendProvider { name: "local".to_string() });
    }

    #[test]
    fn lend_template_reads_from_its_request() {
        reads(r#"{"native": "lend_template", "name": "page"}"#, Native::LendTemplate { name: "page".to_string() });
    }

    #[test]
    fn lend_aspect_reads_from_its_request() {
        let (kind, element, methods) = ("Sectioning".to_string(), "section".to_string(), json!(["level"]));
        reads(
            r#"{"native": "lend_aspect", "kind": "Sectioning", "element": "section", "methods": ["level"]}"#,
            Native::LendAspect { kind, element, methods },
        );
    }

    #[test]
    fn lend_late_init_reads_from_its_request() {
        reads(r#"{"native": "lend_late_init", "index": 0}"#, Native::LendLateInit { index: 0 });
    }

    #[test]
    fn lend_finalizer_reads_from_its_request() {
        reads(r#"{"native": "lend_finalizer", "index": 2}"#, Native::LendFinalizer { index: 2 });
    }

    #[test]
    fn console_reads_from_its_request() {
        let (level, text) = ("warn".to_string(), "careful".to_string());
        reads(r#"{"native": "console", "level": "warn", "text": "careful"}"#, Native::Console { level, text });
    }

    #[test]
    fn an_unknown_native_or_field_is_refused() {
        let unknown = read(r#"{"native": "lend_everything"}"#).map(|_| ()).unwrap_err().to_string();
        assert!(unknown.starts_with("unknown variant `lend_everything`"), "{unknown}");
        let extra = read(r#"{"native": "lend_lower", "rule": "r", "mark": true}"#).map(|_| ()).unwrap_err().to_string();
        assert!(extra.starts_with("unknown field `mark`"), "{extra}");
        let missing = read(r#"{"native": "lend_lower"}"#).map(|_| ()).unwrap_err().to_string();
        assert!(missing.starts_with("missing field `rule`"), "{missing}");
    }

    /// A loaded extension of identity and stem `name` and of number `id`, which no
    /// handler call enters.
    struct Named(ExtensionIdentity, u64);

    impl super::super::Binding for Named {
        fn identity(&self) -> &ExtensionIdentity {
            &self.0
        }

        fn id(&self) -> u64 {
            self.1
        }

        fn enter(
            self: Arc<Self>,
            _run: &mut dyn FnMut(&mut dyn super::super::Instance) -> Result<super::super::Reply, ExtError>,
        ) -> Result<super::super::Reply, ExtError> {
            unreachable!("no handler call enters the binding")
        }
    }

    #[test]
    fn a_native_is_refused_while_another_extension_s_handler_is_the_running_call() {
        let identity = |name: &str| ExtensionIdentity { identity: name.to_string(), stem: name.to_string() };
        let console: ConsoleSink = Arc::new(|_, _| {});
        let lend = || Native::LendLower { rule: "r".to_string() };
        let refused = |stem: &str| envelope(Err(outside(stem, Act::Lower("r"))));
        let (a, b, a_again) = (next_extension_id(), next_extension_id(), next_extension_id());
        assert_eq!(answer(&identity("b"), b, &console, lend()), refused("b"));
        let mut table = crate::adapter::handles::CallTable::new(frames::next_call());
        let registration = super::super::Registration {
            binding: Arc::new(Named(identity("a"), a)),
            package: "_".to_string(),
            feature: "words".to_string(),
            settings: Arc::new(Value::Null),
        };
        let frame = frames::Frame { call: table.call(), registration, table: &mut table };
        // The innermost frame is a call of the loaded extension `a`: `b`'s
        // request (an instance of `b` created inside it, say) is refused, and so
        // is the request of a second load of `a`'s identity; `a`'s own is
        // answered.
        let (theirs, again, own) = frames::enter(frame, || {
            (
                answer(&identity("b"), b, &console, lend()),
                answer(&identity("a"), a_again, &console, lend()),
                answer(&identity("a"), a, &console, lend()),
            )
        });
        assert_eq!(theirs, refused("b"));
        assert_eq!(again, refused("a"));
        assert_eq!(own["ok"]["@handle"]["kind"], "lower", "{own}");
    }

    #[test]
    fn json_depth_counts_lists_and_maps_and_skips_strings() {
        assert_eq!(json_depth("1"), 0);
        assert_eq!(json_depth("[]"), 1);
        assert_eq!(json_depth(r#"{"a": [1, {"b": []}]}"#), 4);
        assert_eq!(json_depth(r#"["[[[{", "\"[["]"#), 1);
        assert_eq!(json_depth(r#"[[]] [[[]]]"#), 3);
    }

    #[test]
    fn a_text_or_a_reply_nesting_past_the_bound_is_refused() {
        let deep = format!("{}{}", "[".repeat(MAX_JSON_DEPTH + 1), "]".repeat(MAX_JSON_DEPTH + 1));
        let bound = format!("{}{}", "[".repeat(MAX_JSON_DEPTH), "]".repeat(MAX_JSON_DEPTH));
        assert!(parse("the arguments", &bound).is_ok());
        assert!(matches!(parse("the arguments", &deep), Err(ExtError::Argument { .. })));
        assert!(parse_reply(Some(&bound), "the handler").is_ok());
        assert!(matches!(parse_reply(Some(&deep), "the handler"), Err(ExtError::Value { .. })));
        assert_eq!(parse_reply(None, "the handler").ok(), Some(Value::Null));
    }
}
