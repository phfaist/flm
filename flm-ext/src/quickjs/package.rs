//! The binding's side of a loaded extension: what every handler of one
//! extension shares ([`JsExtension`], the [`Binding`]), the engine instance
//! entered for one handler call ([`JsInstance`], the [`Instance`]), and the
//! engine's part of the digest ([`digest`]). The handler objects,
//! `run_handler` and the natives' bodies are the shared ones
//! ([`crate::binding`]).
//!
//! # One handler call
//!
//! [`Binding::enter`] enters this thread's engine instance of the extension
//! ([`engine::with_engine`], which creates it first if needed) and starts the
//! call's time budget; `run_handler` pushes the frame inside it and calls the
//! entry points through [`Instance::call_entry`]: the prelude's entry point
//! with the arguments as strings (a name as itself, a value as JSON text),
//! its failure rendered by [`Engine::fail`], then the queued promise jobs when
//! the call is the outermost entry into the instance ([`Engine::finish`]; a
//! refused `import()` among them fails the call), then the reply read
//! ([`parse_reply`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use rquickjs::{Ctx, Function, Object};
use serde_json::Value;

use super::engine::{self, Engine};
use super::host::{JsHost, JsSettings};
use super::ENGINE_VERSION;
use crate::binding::natives::parse_reply;
use crate::binding::{Binding, EntryArg, Instance, Registered};
use crate::engine::ExtensionIdentity;
use crate::runtime::ExtError;

/// One extension the js engine loaded: what every engine instance of it on
/// every thread shares. Held by `Arc` from the runtime and from every stage
/// and hook object.
pub(crate) struct JsExtension {
    /// From a process-wide counter, one per load
    /// ([`next_extension_id`](crate::binding::next_extension_id)):
    /// with the host's, it keys
    /// the extension's engine instances. (Not the identity: a second load of
    /// one identity, after an edit, must not reach the instances of the first,
    /// whose modules may differ.)
    pub id: u64,
    /// The host the extension was loaded with.
    pub host: JsHost,
    /// The settings every engine instance of the extension is created with:
    /// the host's, with the limits of the extension's manifest
    /// ([`JsSettings::with_limits`]).
    pub settings: JsSettings,
    /// The identity and the stem, for the messages.
    pub extension: ExtensionIdentity,
    /// The main module's normalized path (`main.js`, `src/main.js`).
    pub main: String,
    /// The recorded modules: normalized path → text.
    pub modules: BTreeMap<String, Arc<str>>,
    /// Whether an engine instance calls the default export when it is
    /// created: `true` for a loaded extension, `false` for a test package.
    pub define: bool,
}

/// The engine's part of the digest: the bytes `flm-ext quickjs `, the
/// engine's version, then for each recorded module in path order, and then
/// for each module of the host in name order: a NUL byte, the path or name, a
/// NUL byte, the text; then a final NUL byte. The main module's path does not
/// enter (the manifest names it), nor do the host's settings (they change
/// whether a handler fails, not what a succeeding handler answers).
pub(crate) fn digest(host: &JsHost, modules: &BTreeMap<String, Arc<str>>) -> Vec<u8> {
    let mut bytes = b"flm-ext quickjs ".to_vec();
    bytes.extend_from_slice(ENGINE_VERSION.as_bytes());
    let mut add = |name: &str, source: &str| {
        bytes.push(0);
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(source.as_bytes());
    };
    for (name, source) in modules {
        add(name, source);
    }
    for (name, source) in host.module_sources() {
        add(name, source);
    }
    bytes.push(0);
    bytes
}

impl Binding for JsExtension {
    fn identity(&self) -> &ExtensionIdentity {
        &self.extension
    }

    fn id(&self) -> u64 {
        self.id
    }

    /// [`engine::with_engine`], with the call's time budget started before
    /// `run` and ended after it.
    fn enter(
        self: Arc<Self>,
        run: &mut dyn FnMut(&mut dyn Instance) -> Result<Value, ExtError>,
    ) -> Result<Value, ExtError> {
        engine::with_engine(&self.host, &self, |ctx, engine| {
            let budget = engine.start_budget();
            let result = run(&mut JsInstance { ctx, engine });
            engine.end_budget(budget);
            result
        })?
    }
}

/// An engine instance entered for one handler call: the live context and the
/// instance.
pub(crate) struct JsInstance<'a, 'js> {
    /// The live context.
    ctx: &'a Ctx<'js>,
    /// The instance.
    engine: &'a Engine,
}

impl Instance for JsInstance<'_, '_> {
    fn registered(&self) -> &Registered {
        &self.engine.registered
    }

    /// The entry point of `flm:_entry`'s namespace called with each argument
    /// as a string ([`call_with`]), its failure rendered by [`Engine::fail`];
    /// then [`Engine::finish`] (the queued jobs, at the outermost entry); then
    /// the reply read by [`parse_reply`].
    fn call_entry(&mut self, entry: &str, args: &[EntryArg], what: &str) -> Result<Value, ExtError> {
        let (ctx, engine) = (self.ctx, self.engine);
        let result = engine.prelude(ctx).and_then(|namespace| {
            let function = entry_function(ctx, engine, &namespace, entry, what)?;
            let texts = args
                .iter()
                .map(|arg| match arg {
                    EntryArg::Text(text) => text.clone(),
                    EntryArg::Json(value) => value.to_string(),
                })
                .collect();
            call_with(&function, texts).map_err(|error| engine.fail(ctx, error, what))
        });
        let reply = engine.finish(ctx, what, result)?;
        parse_reply(reply.as_deref(), what)
    }
}

/// The entry point `name` of the namespace of `flm:_entry`.
fn entry_function<'js>(
    ctx: &Ctx<'js>,
    engine: &Engine,
    namespace: &Object<'js>,
    name: &str,
    what: &str,
) -> Result<Function<'js>, ExtError> {
    namespace.get::<_, Function>(name).map_err(|error| engine.fail(ctx, error, what))
}

/// Calls `function` with the string arguments `args`, at most six (`expand`,
/// `lower`: the package, the feature, the name and three handles); the entry
/// points ignore the arguments past their own.
fn call_with<'js>(function: &Function<'js>, args: Vec<String>) -> rquickjs::Result<Option<String>> {
    let mut args = args.into_iter();
    let mut next = || args.next().unwrap_or_default();
    let (a, b, c, d, e, f) = (next(), next(), next(), next(), next(), next());
    function.call((a, b, c, d, e, f))
}
