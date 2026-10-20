//! The host side of a script binding that does not depend on the engine,
//! shared by the bindings: the objects implementing the handler traits
//! ([`handlers`]), [`run_handler`], the one function every handler call goes
//! through, the bodies of the natives ([`natives`]), the thread's frames
//! ([`frames`]), and the reading of package objects ([`packages`]).
//!
//! # The boundary
//!
//! A binding implements two traits. [`Binding`] is a loaded extension as the
//! binding holds it: its identity, its load's number, and [`Binding::enter`],
//! which enters this thread's instance of the extension for one handler call,
//! under that call's limits where the binding has limits.
//! [`Instance`] is that instance, entered: the features whose `register` it
//! has run ([`Registered`]), and [`Instance::call_entry`], which calls one
//! entry point and answers its reply. Everything else is here: the handler
//! objects hold a [`Registration`] (the binding, the package, the feature,
//! the configuration), and the binding's [`ExtRuntime`] is [`BindingRuntime`].
//!
//! # One handler call
//!
//! [`run_handler`] creates the call's handle table, lends the objects the
//! adapter passed to the handler into it ([`HandlerKind::lend`], which also
//! answers the entry point and its arguments), and enters this thread's
//! instance of the extension ([`Binding::enter`], which creates it first if
//! needed). Inside, it pushes a frame ([`frames::enter`]) so that the natives
//! reach the table, rehearses the feature on an instance that has not run its
//! `register` with the configuration of the call's registration (the entry
//! point `rehearse`), and calls the handler's entry point with the package,
//! the feature and the handles. The reply is converted while the table is
//! alive, so that the handles it names still resolve; the table then drops,
//! which ends every handle of the call.
//!
//! The frame is pushed inside [`Binding::enter`], after the instance exists:
//! a native called while an instance is being created sees no frame of the
//! call. It sees the frame of the call it is created in, if any, which is
//! another loaded extension's, and is refused without that call's table
//! being borrowed (invariant 2 of [`frames`]).

pub(crate) mod frames;
pub(crate) mod handlers;
pub(crate) mod natives;
pub(crate) mod packages;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::adapter::handles::CallTable;
use crate::api::core::registry::FeatureBuilder;
use crate::engine::ExtensionIdentity;
use crate::runtime::{ExtError, ExtRuntime};
use frames::Frame;
use handlers::HandlerKind;

/// A loaded extension as a binding holds it: what the shared handler objects and
/// `run_handler` call back into.
pub(crate) trait Binding: Send + Sync {
    /// The extension's identity and stem (the messages name the stem).
    fn identity(&self) -> &ExtensionIdentity;

    /// The load's number ([`next_extension_id`]), one per load: it tells two loads of
    /// one identity apart (a second load after an edit, say), and a frame belongs to
    /// one load.
    fn id(&self) -> u64;

    /// Enters this thread's instance of the extension (created first when the thread
    /// has none, or a fresh one in strict mode) and runs `run` on it under one handler
    /// call's limits, where the binding has them (QuickJS's time budget, the WebAssembly
    /// fuel; the engine `native` has none); then leaves it: at the outermost level, a
    /// poisoned instance (by a limit, a trap or a panic), or any in strict mode, is
    /// discarded.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`] when the instance cannot be created on this thread;
    /// otherwise what `run` answers.
    fn enter(
        self: Arc<Self>,
        run: &mut dyn FnMut(&mut dyn Instance) -> Result<Value, ExtError>,
    ) -> Result<Value, ExtError>;
}

/// This thread's instance of a loaded extension, entered for one handler call.
pub(crate) trait Instance {
    /// The features whose `register` this instance has run, for real or rehearsed.
    fn registered(&self) -> &Registered;

    /// Calls the entry point `entry` with `args` (the package and the feature first) and
    /// answers its reply, `null` when it answers nothing: read from JSON text by
    /// `natives::parse_reply` in the two text bindings, the guest's value as it is in
    /// the engine `native`. QuickJS runs the queued jobs after the call when it is the
    /// outermost entry.
    ///
    /// # Errors
    ///
    /// [`ExtError::Handler`]: the entry point fails (it throws, traps or panics, exceeds
    /// a limit, or is missing); [`ExtError::Value`], in the text bindings: its reply is
    /// not JSON, or nests too deep.
    fn call_entry(&mut self, entry: &str, args: &[EntryArg], what: &str) -> Result<Value, ExtError>;
}

/// An argument of an entry point: a name (a stage's, an element's), which QuickJS passes
/// as the string itself, or a value (a configuration, a handle, the requests, an index),
/// which QuickJS passes as JSON text. The WebAssembly ABI writes both as JSON values; the
/// engine `native` passes both as values, with no text.
#[derive(Debug, Clone)]
pub(crate) enum EntryArg {
    /// A name.
    Text(String),
    /// A value.
    Json(Value),
}

/// The entry point a handler call ends in, its arguments after the package's and the
/// feature's names, and what the messages call the handler.
pub(crate) struct EntryCall {
    /// The entry point's name (in `flm:_entry`'s namespace, for QuickJS).
    pub entry: &'static str,
    /// Its arguments after the package and the feature: names and handles.
    pub args: Vec<EntryArg>,
    /// The handler, as the messages name it ("the stage `wc.count` of the feature `wc`
    /// of the package `_` of the extension `wc`").
    pub what: String,
}

/// The registration a handler object belongs to: the binding's extension, the package,
/// the feature, and the configuration `register` ran with.
#[derive(Clone)]
pub(crate) struct Registration {
    /// The extension.
    pub binding: Arc<dyn Binding>,
    /// The package whose feature registered the object (its name within the extension).
    pub package: String,
    /// The feature whose `register` registered the object.
    pub feature: String,
    /// The configuration of that registration.
    pub settings: Arc<Value>,
}

/// The features whose `register` an instance has run (for real or rehearsed), by
/// package and feature, with the configuration it ran with.
#[derive(Default)]
pub(crate) struct Registered(RefCell<BTreeMap<(String, String), Arc<Value>>>);

impl Registered {
    /// Whether the instance has run `register` of `feature` of `package` with the
    /// configuration `settings` (the last one it ran with, if it ran several).
    pub(crate) fn ran(&self, package: &str, feature: &str, settings: &Value) -> bool {
        self.0.borrow().get(&(package.to_string(), feature.to_string())).is_some_and(|seen| **seen == *settings)
    }

    /// Records that the instance ran `register` of `feature` of `package` with
    /// `settings`.
    pub(crate) fn record(&self, package: &str, feature: &str, settings: &Arc<Value>) {
        self.0.borrow_mut().insert((package.to_string(), feature.to_string()), Arc::clone(settings));
    }
}

/// Runs one handler of the feature of `registration` on this thread: lends the objects
/// of `kind` into a fresh table, enters the extension's instance, pushes the call's
/// frame, rehearses the feature if the instance has not run its `register` with the
/// registration's configuration (for any handler but `register` itself and a template
/// module's `describe`, which runs inside it), calls the entry point, and converts its
/// reply with `convert` while the table is alive (the reply's handles resolve against
/// it).
///
/// # Errors
///
/// [`ExtError::Handler`]: the instance cannot be created, the rehearsal fails, the
/// handler fails or exceeds a limit (or, QuickJS, calls `import()`); and whatever
/// `convert` answers.
pub(crate) fn run_handler<'c, R>(
    registration: &Registration,
    kind: HandlerKind<'c>,
    convert: impl FnOnce(&mut CallTable<'c>, Value) -> Result<R, ExtError>,
) -> Result<R, ExtError> {
    let mut table = CallTable::new(frames::next_call());
    let registering = matches!(kind, HandlerKind::Register { .. });
    // `describe` runs inside the `register` that contributes the module,
    // which is its rehearsal: no other `register` runs before it.
    let rehearses = !registering && !matches!(kind, HandlerKind::TemplateDescribe { .. });
    let Registration { package, feature, settings, .. } = registration;
    let owner = format!(
        "of the feature `{feature}` of the package `{package}` of the extension `{}`",
        registration.binding.identity().stem
    );
    let call = kind.lend(&mut table, &owner);
    let mut args = vec![EntryArg::Text(package.clone()), EntryArg::Text(feature.clone())];
    args.extend(call.args);
    let reply = Arc::clone(&registration.binding).enter(&mut |instance| {
        let frame = Frame { call: table.call(), registration: registration.clone(), table: &mut table };
        frames::enter(frame, || {
            if rehearses && !instance.registered().ran(package, feature, settings) {
                let rehearsal =
                    [EntryArg::Text(package.clone()), EntryArg::Text(feature.clone()), EntryArg::Json((**settings).clone())];
                instance.call_entry("rehearse", &rehearsal, &call.what)?;
                instance.registered().record(package, feature, settings);
            }
            let reply = instance.call_entry(call.entry, &args, &call.what)?;
            if registering {
                instance.registered().record(package, feature, settings);
            }
            Ok(reply)
        })
    })?;
    convert(&mut table, reply)
}

/// The binding's [`ExtRuntime`]: a loaded extension, which runs the features of every
/// package of it.
pub(crate) struct BindingRuntime {
    /// The extension.
    pub binding: Arc<dyn Binding>,
    /// The engine's part of the digest.
    pub digest: Vec<u8>,
}

impl ExtRuntime for BindingRuntime {
    fn register(&self, package: &str, feature: &str, settings: &Value, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        let registration = Registration {
            binding: Arc::clone(&self.binding),
            package: package.to_string(),
            feature: feature.to_string(),
            settings: Arc::new(settings.clone()),
        };
        run_handler(&registration, HandlerKind::Register { settings, b }, |_, _| Ok(()))
    }

    fn digest_input(&self) -> Vec<u8> {
        self.digest.clone()
    }
}

/// The process-wide counter of loaded extensions.
static NEXT_EXTENSION: AtomicU64 = AtomicU64::new(1);

/// A fresh number for a loaded extension: with the host's, it keys the extension's
/// instances on a thread.
pub(crate) fn next_extension_id() -> u64 {
    NEXT_EXTENSION.fetch_add(1, Ordering::Relaxed)
}

/// The data of an error that wraps `error` (an instance that could not be created, a
/// load entry's failure): an [`ExtError::Handler`]'s own data (so that a limit's `limit`
/// or a panic's `panic` stays at the top of the data), any other error's
/// [`ExtError::data`].
#[cfg(any(feature = "wasm", feature = "native"))]
pub(crate) fn inner_data(error: &ExtError) -> Value {
    match error {
        ExtError::Handler { data, .. } => data.clone(),
        other => other.data(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registered_tells_the_configuration_register_ran_with() {
        let registered = Registered::default();
        let settings = Arc::new(json!({ "level": 2 }));
        assert!(!registered.ran("_", "wc", &settings));
        registered.record("_", "wc", &settings);
        assert!(registered.ran("_", "wc", &json!({ "level": 2 })));
        assert!(!registered.ran("_", "wc", &json!({ "level": 3 })));
        assert!(!registered.ran("_", "other", &settings));
        assert!(!registered.ran("other", "wc", &settings));
    }
}
