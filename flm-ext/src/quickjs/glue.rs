//! The native functions every engine holds as properties of the global object
//! `__flm`: the only way JavaScript reaches Rust.
//!
//! | Native | Arguments → answer | What it does |
//! |---|---|---|
//! | `__flm.call(handle, method, args)` | three JSON texts → envelope | [`Native::Call`] |
//! | `__flm.module_handle(path)` | a module path → envelope | [`Native::ModuleHandle`] |
//! | `__flm.lend_stage(decl, needs)` | two JSON texts → envelope | [`Native::LendStage`] |
//! | `__flm.lend_expand(element, policy)` | two strings → envelope | [`Native::LendExpand`] |
//! | `__flm.lend_lower(rule)` | a string → envelope | [`Native::LendLower`] |
//! | `__flm.lend_rewrite(decl, needs)` | two JSON texts → envelope | [`Native::LendRewrite`] |
//! | `__flm.lend_provider(name)` | a string → envelope | [`Native::LendProvider`] |
//! | `__flm.lend_template(name)` | a string → envelope | [`Native::LendTemplate`] |
//! | `__flm.lend_aspect(kind, element, methods)` | two strings and a JSON list → envelope | [`Native::LendAspect`] |
//! | `__flm.lend_late_init(index)` | a number → envelope | [`Native::LendLateInit`] |
//! | `__flm.lend_finalizer(index)` | a number → envelope | [`Native::LendFinalizer`] |
//! | `__flm.console(level, text)` | two strings → nothing | the host's console sink |
//!
//! Each native but `console` parses its JSON texts ([`parse`]), builds the
//! request ([`Native`]) and answers the shared body's envelope ([`answer`]) as
//! JSON text: `{"ok": <value>}`, or `{"err": <the error's data>, "message":
//! <its text>}` for a failure. A text that does not parse, or nests deeper
//! than [`MAX_JSON_DEPTH`](crate::binding::natives::MAX_JSON_DEPTH), is
//! refused before the request is answered. Every native answers a string (or
//! nothing) and never throws into the engine: the prelude turns an `err`
//! envelope into a thrown `FlmError`. The closures capture only `'static`
//! data (the extension's identity, the host's console sink); what a call
//! needs of the running handler, the shared body reads from the thread's
//! frames ([`frames`](crate::binding::frames)).

use std::sync::Arc;

use rquickjs::{Ctx, Function, Object};

use super::host::JsHost;
use crate::binding::natives::{answer, envelope, for_method, parse, Native};
use crate::engine::ExtensionIdentity;
use crate::host::{ConsoleLevel, ConsoleSink};
use crate::runtime::ExtError;

/// What the closures of the natives share: the extension's identity (the
/// messages name its stem), the load's number, and the host's console sink.
struct Natives {
    /// The extension.
    extension: ExtensionIdentity,
    /// The load's number ([`Binding::id`](crate::binding::Binding::id)).
    load: u64,
    /// The console sink.
    console: ConsoleSink,
}

impl Natives {
    /// The envelope of `request` as JSON text: the shared body's answer, or the
    /// refusal of an argument that does not read.
    fn answer(&self, request: Result<Native, ExtError>) -> String {
        match request {
            Ok(native) => answer(&self.extension, self.load, &self.console, native),
            Err(error) => envelope(Err(error)),
        }
        .to_string()
    }
}

/// Installs `__flm` with its natives in the engine of `ctx`, an instance of
/// the extension `extension` (whose stem the messages name), the loaded
/// extension of number `load`.
///
/// # Errors
///
/// rquickjs's own, when the engine cannot allocate the objects.
pub(crate) fn install(ctx: &Ctx<'_>, host: &JsHost, extension: &ExtensionIdentity, load: u64) -> rquickjs::Result<()> {
    let flm = Object::new(ctx.clone())?;
    let sink = Arc::clone(host.console());
    flm.set(
        "console",
        Function::new(ctx.clone(), move |level: String, text: String| {
            sink(ConsoleLevel::from_name(&level), &text);
        })?,
    )?;
    let natives = Arc::new(Natives { extension: extension.clone(), load, console: Arc::clone(host.console()) });
    flm.set("call", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |handle: String, method: String, args: String| {
            let request = parse("the object", &handle)
                .and_then(|handle| Ok(Native::Call { handle, method: method.clone(), args: parse("the arguments", &args)? }))
                .map_err(|error| for_method(&method, error));
            natives.answer(request)
        })?
    })?;
    flm.set("module_handle", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |path: String| natives.answer(Ok(Native::ModuleHandle { path })))?
    })?;
    flm.set("lend_stage", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |decl: String, needs: String| {
            let decl = parse("the stage's declaration", &decl);
            let needs = parse("the stage's needs", &needs);
            natives.answer(decl.and_then(|decl| Ok(Native::LendStage { decl, needs: needs? })))
        })?
    })?;
    flm.set("lend_expand", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |element: String, policy: String| {
            natives.answer(Ok(Native::LendExpand { element, policy }))
        })?
    })?;
    flm.set("lend_lower", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |rule: String| natives.answer(Ok(Native::LendLower { rule })))?
    })?;
    flm.set("lend_rewrite", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |decl: String, needs: String| {
            let decl = parse("the rewrite stage's declaration", &decl);
            let needs = parse("the rewrite stage's needs", &needs);
            natives.answer(decl.and_then(|decl| Ok(Native::LendRewrite { decl, needs: needs? })))
        })?
    })?;
    flm.set("lend_provider", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |name: String| natives.answer(Ok(Native::LendProvider { name })))?
    })?;
    flm.set("lend_template", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |name: String| natives.answer(Ok(Native::LendTemplate { name })))?
    })?;
    flm.set("lend_aspect", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |kind: String, element: String, methods: String| {
            let methods = parse("the aspect's methods", &methods);
            natives.answer(methods.map(|methods| Native::LendAspect { kind, element, methods }))
        })?
    })?;
    flm.set("lend_late_init", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |index: u32| natives.answer(Ok(Native::LendLateInit { index })))?
    })?;
    flm.set("lend_finalizer", {
        let natives = Arc::clone(&natives);
        Function::new(ctx.clone(), move |index: u32| natives.answer(Ok(Native::LendFinalizer { index })))?
    })?;
    ctx.globals().set("__flm", flm)?;
    Ok(())
}
