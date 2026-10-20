//! The natives every binding shares: the requests an extension makes of the
//! host ([`Native`]) and their bodies ([`answer`]), the envelope a native
//! answers, the refusal of a native called while no handler runs, and the
//! reading of the JSON a script or a guest passes (its depth bounded).
//!
//! | Native | What it does |
//! |---|---|
//! | `call(handle, method, args)` | [`dispatch::call`] on the running call's table |
//! | `module_handle(path)` | the handle of the module `path` ([`module_handle`]) |
//! | `lend_stage(decl, needs)` | lends a stage of the extension's own ([`BindingStage`]) |
//! | `lend_expand(element, policy)` | lends an expansion hook of the extension's own ([`BindingExpand`]) |
//! | `lend_lower(rule)` | lends a lowering rule's body of the extension's own ([`BindingLower`]) |
//! | `lend_rewrite(decl, needs)` | lends a rewrite stage of the extension's own ([`BindingRewrite`]) |
//! | `lend_provider(name)` | lends a citation provider of the extension's own ([`BindingProvider`]) |
//! | `lend_template(name)` | lends a template module of the extension's own ([`BindingTemplate`]) |
//! | `lend_aspect(kind, element, methods)` | lends an element's handler aspect of the extension's own ([`BindingReferenceable`], [`BindingSectioning`]) |
//! | `lend_late_init(index)` | lends a late-initialization hook of the extension's own ([`BindingLateInit`]) |
//! | `lend_finalizer(index)` | lends a finalizer of the extension's own ([`BindingFinalizer`]) |
//! | `console(level, text)` | the host's console sink |
//!
//! What a native needs of the running handler, [`answer`] reads from the
//! thread's frames ([`frames`]): the frame is looked up before the values are
//! read, so a native that needs one, called while no handler of its extension
//! runs, answers the refusal [`outside`] whatever its arguments. The innermost
//! frame may be another extension's: an instance created on a thread while
//! that extension's handler call runs there runs its module's top level (or
//! `define`) inside that call, and natively the objects of an instance the
//! engine `native` discards there are dropped inside it; a native sent from
//! either is refused likewise. A binding reads its own wire format first
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

use std::sync::Arc;

use serde_json::{json, Value};

use super::frames::{self, FrameView};
use super::handlers::{
    BindingExpand, BindingFinalizer, BindingLateInit, BindingLower, BindingProvider, BindingReferenceable, BindingRewrite,
    BindingSectioning, BindingStage, BindingTemplate,
};
use crate::adapter::dispatch;
use crate::adapter::handles::{module_handle, Entry};
use crate::adapter::values::{handle_from_value, handle_to_value, JsonForm};
use crate::api::core::stage::{Needs, RewriteDecl, StageDecl};
use crate::api::lang::rewrites::userdefs::ExpandPolicy;
use crate::engine::ExtensionIdentity;
use crate::host::{ConsoleLevel, ConsoleSink};
use crate::runtime::ExtError;

/// One request to the host: the WebAssembly ABI carries it as JSON
/// (`{"native": "lend_stage", "decl": …, "needs": …}`); the QuickJS glue builds it from
/// its natives' arguments.
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "native", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Native {
    /// Calls `method` on the object `handle` stands for in the running handler call,
    /// with the list `args`; answers the dispatcher's answer.
    Call {
        /// The handle.
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

impl Native {
    /// What the extension must do from a handler, for the [`outside`] refusal:
    /// "call `chars` of the extension API", "declare a stage", ….
    fn act(&self) -> String {
        match self {
            Native::Call { method, .. } => format!("call `{method}` of the extension API"),
            Native::LendStage { .. } => "declare a stage".to_string(),
            Native::LendExpand { element, .. } => format!("attach the expansion hook of the element `{element}`"),
            Native::LendLower { rule } => format!("build the lowering rule `{rule}`"),
            Native::LendRewrite { .. } => "declare a rewrite stage".to_string(),
            Native::LendProvider { name } => format!("contribute the citation provider `{name}`"),
            Native::LendTemplate { name } => format!("contribute the template module `{name}`"),
            Native::LendAspect { kind, element, .. } => format!("attach the aspect `{kind}` of the element `{element}`"),
            Native::LendLateInit { .. } => "register a late-initialization hook".to_string(),
            Native::LendFinalizer { .. } => "register a finalizer".to_string(),
            Native::ModuleHandle { .. } | Native::Console { .. } => String::new(),
        }
    }

    /// Runs a native that needs the running call, on its frame `view`. A lending clones
    /// the frame's registration into the handler object it builds; `call` needs none.
    fn in_frame(&self, view: &mut FrameView<'_, '_>) -> Result<Value, ExtError> {
        let entry = match self {
            Native::Call { handle, method, args } => {
                let target = handle_from_value(handle).map_err(|error| for_method(method, error))?;
                return dispatch::call(view.table, target, method, args);
            }
            Native::LendStage { decl, needs } => {
                let decl = StageDecl::from_value(decl)?;
                let needs = Needs::from_value(needs)?;
                let registration = view.registration.clone();
                Entry::Stage(Arc::new(BindingStage { registration, decl, needs }))
            }
            Native::LendExpand { element, policy } => {
                let policy = ExpandPolicy::from_value(&Value::String(policy.clone()))?;
                let registration = view.registration.clone();
                Entry::Expand(Arc::new(BindingExpand { registration, element: element.clone(), policy }))
            }
            Native::LendLower { rule } => {
                let registration = view.registration.clone();
                Entry::Lower(Arc::new(BindingLower { registration, rule: rule.clone() }))
            }
            Native::LendRewrite { decl, needs } => {
                let decl = RewriteDecl::from_value(decl)?;
                let needs = Needs::from_value(needs)?;
                let registration = view.registration.clone();
                Entry::RewriteStage(Arc::new(BindingRewrite { registration, decl, needs }))
            }
            Native::LendProvider { name } => {
                let registration = view.registration.clone();
                Entry::CitationProvider(Arc::new(BindingProvider { registration, name: name.clone() }))
            }
            Native::LendTemplate { name } => {
                let registration = view.registration.clone();
                Entry::TemplateModule(Arc::new(BindingTemplate { registration, name: name.clone() }))
            }
            Native::LendAspect { kind, element, methods } => {
                let methods: Vec<String> = serde_json::from_value(methods.clone()).map_err(|_| ExtError::Argument {
                    method: "lend_aspect".to_string(),
                    detail: "the aspect's methods must be a list of texts".to_string(),
                })?;
                match kind.as_str() {
                    "Referenceable" => {
                        let registration = view.registration.clone();
                        Entry::Referenceable(Arc::new(BindingReferenceable { registration, element: element.clone() }))
                    }
                    "Sectioning" => {
                        let registration = view.registration.clone();
                        Entry::Sectioning(Arc::new(BindingSectioning { registration, element: element.clone(), methods }))
                    }
                    other => {
                        return Err(ExtError::Argument {
                            method: "lend_aspect".to_string(),
                            detail: format!(
                                "the kind of a handler aspect must be `Referenceable` or `Sectioning`, not `{other}`"
                            ),
                        })
                    }
                }
            }
            Native::LendLateInit { index } => {
                let registration = view.registration.clone();
                Entry::LateInit(Arc::new(BindingLateInit { registration, index: *index as usize }))
            }
            Native::LendFinalizer { index } => {
                let registration = view.registration.clone();
                Entry::Finalizer(Arc::new(BindingFinalizer { registration, index: *index as usize }))
            }
            Native::ModuleHandle { .. } | Native::Console { .. } => return Ok(Value::Null),
        };
        Ok(handle_to_value(view.table.lend(entry)))
    }
}

/// Runs `native` for the extension `extension`, the loaded extension of number `load`
/// ([`Binding::id`](super::Binding::id)), on the running call's frame; answers its
/// envelope. The frame is looked up first: a native that needs one, called while no
/// handler runs or while the innermost frame's registration is another loaded
/// extension's, answers the `outside` refusal whatever its arguments, and the frame's
/// table is not borrowed ([`frames::with_current`]).
pub(crate) fn answer(extension: &ExtensionIdentity, load: u64, console: &ConsoleSink, native: Native) -> Value {
    envelope(match &native {
        Native::ModuleHandle { path } => module_handle(path).map(handle_to_value).ok_or_else(|| ExtError::Argument {
            method: "module_handle".to_string(),
            detail: format!("`flm:{path}` must be a module of the extension API that has functions"),
        }),
        Native::Console { level, text } => {
            console(ConsoleLevel::from_name(level), text);
            Ok(Value::Null)
        }
        native => frames::with_current(load, |view| native.in_frame(view))
            .unwrap_or_else(|| Err(outside(&extension.stem, &native.act()))),
    })
}

/// `error` with the method `method` named, when it is an argument error that names
/// none (a value of `call` that does not read: "`chars`: the arguments must be JSON
/// text: …").
pub(crate) fn for_method(method: &str, error: ExtError) -> ExtError {
    match error {
        ExtError::Argument { method: found, detail } if found.is_empty() => {
            ExtError::Argument { method: method.to_string(), detail }
        }
        other => other,
    }
}

/// The envelope of `result`.
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
pub(crate) fn outside(stem: &str, act: &str) -> ExtError {
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

#[cfg(test)]
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
            _run: &mut dyn FnMut(&mut dyn super::super::Instance) -> Result<Value, ExtError>,
        ) -> Result<Value, ExtError> {
            Ok(Value::Null)
        }
    }

    #[test]
    fn a_native_is_refused_while_another_extension_s_handler_is_the_running_call() {
        let identity = |name: &str| ExtensionIdentity { identity: name.to_string(), stem: name.to_string() };
        let console: ConsoleSink = Arc::new(|_, _| {});
        let lend = || Native::LendLower { rule: "r".to_string() };
        let refused = |stem: &str| envelope(Err(outside(stem, "build the lowering rule `r`")));
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
    #[cfg(any(feature = "quickjs", feature = "wasm"))]
    fn json_depth_counts_lists_and_maps_and_skips_strings() {
        assert_eq!(json_depth("1"), 0);
        assert_eq!(json_depth("[]"), 1);
        assert_eq!(json_depth(r#"{"a": [1, {"b": []}]}"#), 4);
        assert_eq!(json_depth(r#"["[[[{", "\"[["]"#), 1);
        assert_eq!(json_depth(r#"[[]] [[[]]]"#), 3);
    }

    #[test]
    #[cfg(any(feature = "quickjs", feature = "wasm"))]
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
