//! The contract a language binding implements: [`ExtRuntime`] and
//! [`ExtError`]. What an extension says about itself is its engine's report
//! ([`Engine::load`](crate::engine::Engine::load)).
//!
//! A binding implements [`ExtRuntime`] once for its language, and implements
//! the two traits a scripted feature registers —
//! [`Stage`](crate::api::core::stage::Stage) and
//! [`Expand`](crate::api::lang::rewrites::userdefs::Expand) — with small objects
//! that know which script object to call. Everything a handler may call is a
//! method of a façade the adapter lends for the duration of the call; a binding
//! whose values cross as JSON reaches those methods by name through
//! [`dispatch::call`](crate::adapter::dispatch::call).

use std::fmt;

use serde_json::{json, Value};

use crate::adapter::durable::DurableKind;
use crate::adapter::handles::Handle;
use crate::adapter::names;
use crate::api::core::registry::FeatureBuilder;

/// The extension side of the boundary: what FLM calls in an extension. A binding
/// implements it once for its language; the Rust test double implements it in
/// Rust. A runtime is shared by every feature of every package of one
/// extension and by every thread
/// that runs a handler, which is why it is `Send + Sync`; a binding whose engine
/// is bound to a thread keeps the engine in a thread-local of its own.
pub trait ExtRuntime: Send + Sync {
    /// `FlmFeature::register` of the feature `feature` of the package
    /// `package` (its name within the extension)
    /// ([`FlmFeature::register`](flm_core::core::registry::FlmFeature::register)):
    /// the handler declares everything the feature contributes through `b`.
    /// `settings` is the feature's final configuration (`{}` when it declares
    /// none or none is written).
    ///
    /// # Errors
    ///
    /// The handler's failure; the registry build reports it as
    /// [`BuildError::Feature`](flm_core::core::registry::BuildError::Feature).
    fn register(&self, package: &str, feature: &str, settings: &Value, b: &mut dyn FeatureBuilder) -> Result<(), ExtError>;

    /// The bytes that identify the extension's code — every source text, the
    /// engine's version — fed into the registry digest by every feature's
    /// registration
    /// ([`FeatureBuilder::digest_input`](flm_core::core::registry::FeatureBuilder::digest_input)),
    /// so that a cached result never survives a change to a script.
    fn digest_input(&self) -> Vec<u8>;
}

/// A failure at the boundary. Every variant carries text a reader understands
/// ([`Display`](fmt::Display)), and [`data`](ExtError::data) gives the variant
/// and its fields as a JSON value, which is what a diagnostic reporting the
/// failure carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtError {
    /// The handler's own failure: an exception, a limit exceeded, a script that
    /// does not load.
    Handler {
        /// The binding's rendering of the failure.
        detail: String,
        /// Whatever the binding can add (a stack, a line), or [`Value::Null`].
        data: Value,
    },
    /// A handle used outside the call that lent it, or in a call that is not
    /// the innermost running one.
    StaleHandle {
        /// The handle that was used.
        handle: Handle,
    },
    /// A method called on a handle of another kind, an unknown method name, an
    /// argument that is not what the method takes (a missing field, a float, a
    /// handle of the wrong kind), or a handle whose index the running call
    /// never lent.
    Argument {
        /// The method (or function) that was called; empty when the failure
        /// is not tied to one (a handle the running call never lent).
        method: String,
        /// What is wrong with the call.
        detail: String,
    },
    /// A name declared outside the extension's namespaces: its stem, and
    /// `X` as well for a stem `profile-X`
    /// ([`namespaces`](crate::adapter::names::namespaces)).
    NameOutsideExtension {
        /// What the name names, as the text says it: such as `element`,
        /// `table`, `stage`, `capability`, `diagnostic`, `option`, `rule`,
        /// `style`, `script`, `scoped package`, `argument role`, `service
        /// slot`, `citation provider`, `counter`, `counter style`, `parent
        /// set`, `package definition`, `preamble piece`, `backend`, `output`
        /// (the last two may also be a namespace itself, and their text says
        /// so).
        what: String,
        /// The name declared.
        name: String,
        /// The package that declared it (its name within the extension).
        package: String,
        /// The extension's stem.
        extension: String,
    },
    /// A value that cannot cross: a floating-point number, a map key starting
    /// with `$` (techy's value model reserves it), a value not of the expected
    /// shape.
    Value {
        /// What is wrong with the value.
        detail: String,
    },
    /// An `flm-core` error met inside a call, rendered: an `AccessError`, a
    /// `NamedAccessError`, an `ExtractError`, a `LowerError`, a `BadDefinition`.
    Core {
        /// The kind of the error, a short name (`access`, `expand`, …), or the
        /// identifier of the `flm-core` condition
        /// (`flm.userdefs.bad-definition`).
        kind: String,
        /// The error's rendering.
        detail: String,
        /// What the `flm-core` condition carries as its data
        /// ([`serializable_data`](flm_core::techy::error::DiagnosticInfo::serializable_data))
        /// where a condition is at hand (`variant_for_use`'s `BadDefinition`),
        /// or the place of the failure (the node a `NotCharacters` names);
        /// [`Value::Null`] otherwise.
        data: Value,
    },
    /// A host service's own failure
    /// ([`HostServiceError`](crate::host::HostServiceError)), met by a
    /// handler calling it through a handle.
    Host {
        /// The service's name (`files`).
        service: String,
        /// The method called (`read_text`).
        method: String,
        /// The service's text, complete.
        detail: String,
    },
    /// Something the extension interface does not support yet. Two cases
    /// answer it: reading a service slot whose values are not data; and a
    /// request of the guest client (the module `flm_ext::guest`) that no host
    /// answers, sent neither from an extension's own WebAssembly module nor
    /// during a call of the engine `native` into the extension: on a thread the
    /// extension started, while a thread ends, and in the guest client's own
    /// tests.
    Unsupported {
        /// What is not supported.
        what: String,
    },
    /// A failure followed by what the engine met after it, before the call
    /// returned: the texts of the refused `import()` calls and of the promises
    /// left rejected with no handler, which the queued jobs run at the end of
    /// the call revealed. It reads as `first` does: [`kind`](ExtError::kind)
    /// is `first`'s, [`data`](ExtError::data) is `first`'s map with the key
    /// `followed_by` holding `then`, and its text is `first`'s followed by
    /// each of `then`, joined by `"; "`.
    Followed {
        /// The failure itself.
        first: Box<ExtError>,
        /// The texts met after it, in their order; never empty.
        then: Vec<String>,
    },
    /// A durable token resolved against a table that does not hold it
    /// ([`DurableTable::get`](crate::adapter::durable::DurableTable::get)): a
    /// token of another feature's table or of another registry's, an index
    /// the table does not hold, or an entry of another kind.
    StaleDurable {
        /// The kind the token names.
        kind: DurableKind,
        /// The number of the table the token names.
        table: u64,
        /// The index the token names.
        index: u32,
        /// The feature whose table the token was resolved against.
        feature: String,
        /// That feature's package (its name within the extension).
        package: String,
        /// The extension's stem.
        extension: String,
    },
}

impl ExtError {
    /// The variant's name, in `snake_case`: `handler`, `stale_handle`,
    /// `argument`, `name_outside_extension`, `value`, `core`, `host`, `unsupported`,
    /// `stale_durable`. It is
    /// the `kind` field of [`data`](ExtError::data). A [`Followed`](ExtError::Followed)
    /// failure answers the kind of its first failure.
    pub fn kind(&self) -> &'static str {
        match self {
            ExtError::Followed { first, .. } => first.kind(),
            ExtError::Handler { .. } => "handler",
            ExtError::StaleHandle { .. } => "stale_handle",
            ExtError::Argument { .. } => "argument",
            ExtError::NameOutsideExtension { .. } => "name_outside_extension",
            ExtError::Value { .. } => "value",
            ExtError::Core { .. } => "core",
            ExtError::Host { .. } => "host",
            ExtError::Unsupported { .. } => "unsupported",
            ExtError::StaleDurable { .. } => "stale_durable",
        }
    }

    /// The variant and its fields as a JSON map, `{"kind": "handler", "detail":
    /// "…", "data": …}` and likewise for the other variants (an `flm-core`
    /// error's own kind under `core_kind`, its data under `data`); a handle is written
    /// as its `{"kind", "call", "index"}` map, a durable token as its `{"kind",
    /// "table", "index"}` map. A diagnostic reporting the failure
    /// carries this value as its data. A [`Followed`](ExtError::Followed)
    /// failure answers its first failure's map with the key `followed_by`
    /// holding the texts met after it.
    pub fn data(&self) -> Value {
        let kind = self.kind();
        match self {
            ExtError::Followed { first, then } => {
                let mut data = first.data();
                if let Value::Object(map) = &mut data {
                    map.insert("followed_by".to_string(), json!(then));
                }
                data
            }
            ExtError::Handler { detail, data } => json!({ "kind": kind, "detail": detail, "data": data }),
            ExtError::StaleHandle { handle } => json!({
                "kind": kind,
                "handle": { "kind": handle.kind.name(), "call": handle.call, "index": handle.index },
            }),
            ExtError::Argument { method, detail } => json!({ "kind": kind, "method": method, "detail": detail }),
            ExtError::NameOutsideExtension { what, name, package, extension } => {
                json!({ "kind": kind, "what": what, "name": name, "package": package, "extension": extension })
            }
            ExtError::Value { detail } => json!({ "kind": kind, "detail": detail }),
            ExtError::Core { kind: core_kind, detail, data } => {
                json!({ "kind": kind, "core_kind": core_kind, "detail": detail, "data": data })
            }
            ExtError::Host { service, method, detail } => {
                json!({ "kind": kind, "service": service, "method": method, "detail": detail })
            }
            ExtError::Unsupported { what } => json!({ "kind": kind, "what": what }),
            ExtError::StaleDurable { kind: durable, table, index, feature, package, extension } => json!({
                "kind": kind,
                "durable": { "kind": durable.name(), "table": table, "index": index },
                "feature": feature, "package": package, "extension": extension,
            }),
        }
    }
}

impl ExtError {
    /// The error whose [`data`](ExtError::data) is `data`: the inverse of
    /// [`data`](ExtError::data), for every variant (a map with `followed_by`
    /// is a [`Followed`](ExtError::Followed) failure whose first failure is
    /// the rest of the map). Keys other than the variant's are ignored. The
    /// WebAssembly binding reads a guest's `err` reply with it, and the guest
    /// client an `err` envelope of the host.
    ///
    /// # Errors
    ///
    /// What is wrong with `data`, as text: it is not a map, its `kind` is not
    /// one of the variants' names, or a field the variant needs is missing or
    /// not of its type.
    pub fn from_data(data: &Value) -> Result<ExtError, String> {
        let Value::Object(map) = data else {
            return Err(format!("it must be a map, not `{data}`"));
        };
        if let Some(then) = map.get("followed_by") {
            let then: Vec<String> = serde_json::from_value(then.clone())
                .map_err(|_| "`followed_by` must be a list of texts".to_string())?;
            let mut first = map.clone();
            first.remove("followed_by");
            let first = ExtError::from_data(&Value::Object(first))?;
            return Ok(ExtError::Followed { first: Box::new(first), then });
        }
        let text = |key: &str| -> Result<String, String> {
            map.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("`{key}` must be a text"))
        };
        let value = |key: &str| -> Result<Value, String> { map.get(key).cloned().ok_or_else(|| format!("`{key}` must be given")) };
        let kind = text("kind")?;
        Ok(match kind.as_str() {
            "handler" => ExtError::Handler { detail: text("detail")?, data: value("data")? },
            "stale_handle" => ExtError::StaleHandle {
                handle: serde_json::from_value(value("handle")?)
                    .map_err(|error| format!("`handle` must be a map of `kind`, `call` and `index`: {error}"))?,
            },
            "argument" => ExtError::Argument { method: text("method")?, detail: text("detail")? },
            "name_outside_extension" => ExtError::NameOutsideExtension {
                what: text("what")?,
                name: text("name")?,
                package: text("package")?,
                extension: text("extension")?,
            },
            "value" => ExtError::Value { detail: text("detail")? },
            "core" => ExtError::Core { kind: text("core_kind")?, detail: text("detail")?, data: value("data")? },
            "host" => ExtError::Host { service: text("service")?, method: text("method")?, detail: text("detail")? },
            "unsupported" => ExtError::Unsupported { what: text("what")? },
            "stale_durable" => {
                #[derive(serde::Deserialize)]
                struct Token {
                    kind: DurableKind,
                    table: u64,
                    index: u32,
                }
                let token: Token = serde_json::from_value(value("durable")?)
                    .map_err(|error| format!("`durable` must be a map of `kind`, `table` and `index`: {error}"))?;
                ExtError::StaleDurable {
                    kind: token.kind,
                    table: token.table,
                    index: token.index,
                    feature: text("feature")?,
                    package: text("package")?,
                    extension: text("extension")?,
                }
            }
            other => {
                return Err(format!(
                    "`kind` must be `handler`, `stale_handle`, `argument`, `name_outside_extension`, `value`, `core`, \
                     `host`, `unsupported` or `stale_durable`, not `{other}`"
                ))
            }
        })
    }
}

impl fmt::Display for ExtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtError::Handler { detail, .. } => f.write_str(detail),
            ExtError::StaleHandle { handle } => write!(
                f,
                "the `{}` object (call {}, index {}) belongs to a call that is not the one running",
                handle.kind.name(),
                handle.call,
                handle.index
            ),
            ExtError::Argument { method, detail } if method.is_empty() => f.write_str(detail),
            ExtError::Argument { method, detail } => write!(f, "`{method}`: {detail}"),
            ExtError::NameOutsideExtension { what, name, package, extension } => write!(
                f,
                "the {what} `{name}` declared by the package `{package}` of the extension `{extension}` must {}",
                names::requirement(what, extension)
            ),
            ExtError::Value { detail } => f.write_str(detail),
            ExtError::Core { detail, .. } => f.write_str(detail),
            ExtError::Host { service, method, detail } => write!(f, "`{service}.{method}`: {detail}"),
            ExtError::Unsupported { what } => write!(f, "{what} is not supported yet"),
            ExtError::Followed { first, then } => {
                write!(f, "{first}")?;
                for text in then {
                    write!(f, "; {text}")?;
                }
                Ok(())
            }
            ExtError::StaleDurable { kind, index, feature, package, extension, .. } => write!(
                f,
                "the `{}` object {index} must be one the feature `{feature}` of the package `{package}` of the \
                 extension `{extension}` made under the registry in use",
                kind.name()
            ),
        }
    }
}

impl std::error::Error for ExtError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::handles::HandleKind;

    /// Checks `error`'s data and text, and that [`ExtError::from_data`] reads
    /// the data back as `error`.
    fn check(error: ExtError, data: Value, text: &str) {
        assert_eq!(error.data(), data);
        assert_eq!(error.to_string(), text);
        assert_eq!(ExtError::from_data(&data), Ok(error));
    }

    #[test]
    fn a_handler_failure_carries_the_binding_s_rendering() {
        check(
            ExtError::Handler { detail: "TypeError: x is undefined".to_string(), data: json!({ "line": 3 }) },
            json!({ "kind": "handler", "detail": "TypeError: x is undefined", "data": { "line": 3 } }),
            "TypeError: x is undefined",
        );
    }

    #[test]
    fn a_stale_handle_writes_the_handle_as_its_map() {
        let handle = Handle { kind: HandleKind::Node, call: 4, index: 2 };
        check(
            ExtError::StaleHandle { handle },
            json!({ "kind": "stale_handle", "handle": { "kind": "node", "call": 4, "index": 2 } }),
            "the `node` object (call 4, index 2) belongs to a call that is not the one running",
        );
    }

    #[test]
    fn a_bad_argument_names_its_method_when_it_has_one() {
        check(
            ExtError::Argument { method: "Node::child".to_string(), detail: "the index must be a number".to_string() },
            json!({ "kind": "argument", "method": "Node::child", "detail": "the index must be a number" }),
            "`Node::child`: the index must be a number",
        );
        check(
            ExtError::Argument { method: String::new(), detail: "the running call lent no `node` object at index 9".to_string() },
            json!({ "kind": "argument", "method": "", "detail": "the running call lent no `node` object at index 9" }),
            "the running call lent no `node` object at index 9",
        );
    }

    #[test]
    fn a_name_outside_the_extension_names_the_stem_and_the_package() {
        check(
            ExtError::NameOutsideExtension {
                what: "element".to_string(),
                name: "shout".to_string(),
                package: "_".to_string(),
                extension: "wordcount".to_string(),
            },
            json!({ "kind": "name_outside_extension", "what": "element", "name": "shout", "package": "_", "extension": "wordcount" }),
            "the element `shout` declared by the package `_` of the extension `wordcount` must begin with `wordcount.`",
        );
    }

    #[test]
    fn a_value_failure_is_its_detail() {
        check(
            ExtError::Value { detail: "a number must be an integer".to_string() },
            json!({ "kind": "value", "detail": "a number must be an integer" }),
            "a number must be an integer",
        );
    }

    #[test]
    fn a_core_error_keeps_its_own_kind_apart() {
        check(
            ExtError::Core {
                kind: "access".to_string(),
                detail: "no argument is named `text`".to_string(),
                data: Value::Null,
            },
            json!({ "kind": "core", "core_kind": "access", "detail": "no argument is named `text`", "data": null }),
            "no argument is named `text`",
        );
        check(
            ExtError::Core {
                kind: "flm.userdefs.bad-definition".to_string(),
                detail: "definition ‘\\ket’ is not in force here".to_string(),
                data: json!({ "mode": "text" }),
            },
            json!({
                "kind": "core", "core_kind": "flm.userdefs.bad-definition",
                "detail": "definition ‘\\ket’ is not in force here", "data": { "mode": "text" },
            }),
            "definition ‘\\ket’ is not in force here",
        );
    }

    #[test]
    fn a_host_service_s_failure_names_the_service_and_the_method() {
        check(
            ExtError::Host {
                service: "files".to_string(),
                method: "read_text".to_string(),
                detail: "the path `../x` must stay inside the document's folder `doc`".to_string(),
            },
            json!({
                "kind": "host", "service": "files", "method": "read_text",
                "detail": "the path `../x` must stay inside the document's folder `doc`",
            }),
            "`files.read_text`: the path `../x` must stay inside the document's folder `doc`",
        );
    }

    #[test]
    fn a_stale_durable_token_names_the_feature_whose_table_was_asked() {
        check(
            ExtError::StaleDurable {
                kind: DurableKind::CompiledTemplate,
                table: 7,
                index: 2,
                feature: "titles".to_string(),
                package: "_".to_string(),
                extension: "myhead".to_string(),
            },
            json!({
                "kind": "stale_durable", "durable": { "kind": "compiled_template", "table": 7, "index": 2 },
                "feature": "titles", "package": "_", "extension": "myhead",
            }),
            "the `compiled_template` object 2 must be one the feature `titles` of the package `_` of the extension \
             `myhead` made under the registry in use",
        );
    }

    #[test]
    fn an_unsupported_request_names_what_is_not_supported() {
        check(
            ExtError::Unsupported { what: "module workers".to_string() },
            json!({ "kind": "unsupported", "what": "module workers" }),
            "module workers is not supported yet",
        );
    }

    #[test]
    fn a_followed_failure_reads_as_its_first_with_the_texts_after_it() {
        let first = ExtError::NameOutsideExtension {
            what: "table".to_string(),
            name: "seen".to_string(),
            package: "_".to_string(),
            extension: "wc".to_string(),
        };
        let error = ExtError::Followed {
            first: Box::new(first),
            then: vec!["the handler must leave no promise rejected without a handler: lost".to_string()],
        };
        assert_eq!(error.kind(), "name_outside_extension");
        check(
            error,
            json!({
                "kind": "name_outside_extension", "what": "table", "name": "seen", "package": "_", "extension": "wc",
                "followed_by": ["the handler must leave no promise rejected without a handler: lost"],
            }),
            "the table `seen` declared by the package `_` of the extension `wc` must begin with `wc.`; the handler must \
             leave no promise rejected without a handler: lost",
        );
    }

    #[test]
    fn from_data_refuses_what_is_not_an_error_s_data() {
        assert_eq!(ExtError::from_data(&json!(3)), Err("it must be a map, not `3`".to_string()));
        assert_eq!(ExtError::from_data(&json!({ "detail": "x" })), Err("`kind` must be a text".to_string()));
        assert_eq!(
            ExtError::from_data(&json!({ "kind": "oops" })),
            Err("`kind` must be `handler`, `stale_handle`, `argument`, `name_outside_extension`, `value`, `core`, `host`, \
                 `unsupported` or `stale_durable`, not `oops`"
                .to_string())
        );
        assert_eq!(ExtError::from_data(&json!({ "kind": "value" })), Err("`detail` must be a text".to_string()));
        assert_eq!(ExtError::from_data(&json!({ "kind": "handler", "detail": "x" })), Err("`data` must be given".to_string()));
        assert_eq!(
            ExtError::from_data(&json!({ "kind": "value", "detail": "x", "followed_by": [1] })),
            Err("`followed_by` must be a list of texts".to_string())
        );
        // Keys other than the variant's are ignored.
        assert_eq!(
            ExtError::from_data(&json!({ "kind": "value", "detail": "x", "line": 3 })),
            Ok(ExtError::Value { detail: "x".to_string() })
        );
    }
}
