//! The guest's own types of `flm:lang/mint`: [`ArgDef`] (an argument
//! declared by its code, or a helper's, by its handle), [`SyntaxDef`],
//! [`ElementDef`] (an element declaration as data, or a helper's, by its
//! handle), [`FallbackDef`] and [`ScopedPackageDef`].

use std::fmt;

use serde_json::{json, Map, Value};

use super::{arg, get, not_shape, object, single};
use crate::adapter::handles::{Handle, HandleKind};
use crate::adapter::values::{handle_to_value, JsonForm, HANDLE_KEY};
use crate::api::lang::mint::{ArgLayout, BodyDef, FallbackKind, Mode};
use crate::guest::abi;
use crate::guest::model::element::ElementSpec;
use crate::runtime::ExtError;

/// How an argument crosses.
#[derive(Debug, Clone, PartialEq)]
enum ArgForm {
    /// Declared by its code, with what an argument declares.
    Declared {
        /// The argument's code (`{`, `[`, …); `None` for an argument the
        /// host built from a parser, which does not read back.
        code: Option<String>,
        /// Its name.
        name: Option<String>,
        /// Its layout.
        layout: ArgLayout,
        /// Its documentation.
        docs: String,
        /// Its mode, when it is not that of its surroundings.
        mode: Option<Mode>,
        /// The scoped packages whose commands are legal in it.
        scoped: Vec<String>,
        /// Its role: the registered name of what it holds.
        role: Option<String>,
    },
    /// An argument a helper of the host made, by the handle it lent, with
    /// the fields the setters were given since.
    Lent {
        /// The handle the helper lent.
        handle: Handle,
        /// Each field a setter was given, under its name in the crossing
        /// form, written beside the handle.
        changes: Map<String, Value>,
    },
}

/// Mirrors `flm-core`'s `ArgDef`: an argument of a declaration. The guest's
/// own: an argument is declared by its code, or is one a helper of the host
/// made (`refs::label_fields_arg`), which crosses as its handle. The host
/// has no method that changes a helper's argument: a setter called on one
/// keeps its field beside the handle, and the argument is written with that
/// field next to the handle, a form the host's reader refuses.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgDef {
    /// How it crosses.
    form: ArgForm,
}

impl ArgDef {
    /// Mirrors `ArgDef::new`: the argument of the code `code` (`{` a
    /// mandatory argument, `[` an optional one, …).
    pub fn new(code: impl Into<String>) -> ArgDef {
        ArgDef {
            form: ArgForm::Declared {
                code: Some(code.into()),
                name: None,
                layout: ArgLayout::default(),
                docs: String::new(),
                mode: None,
                scoped: Vec::new(),
                role: None,
            },
        }
    }

    /// Mirrors `ArgDef::named`: the argument of the code `code`, named `name`.
    pub fn named(name: impl Into<String>, code: impl Into<String>) -> ArgDef {
        let mut arg = ArgDef::new(code);
        if let ArgForm::Declared { name: slot, .. } = &mut arg.form {
            *slot = Some(name.into());
        }
        arg
    }

    /// Mirrors `ArgDef::mandatory`: a mandatory argument (`{`) named `name`.
    pub fn mandatory(name: impl Into<String>) -> ArgDef {
        ArgDef::named(name, "{")
    }

    /// Mirrors `ArgDef::optional`: an optional argument (`[`) named `name`.
    pub fn optional(name: impl Into<String>) -> ArgDef {
        ArgDef::named(name, "[")
    }

    /// Mirrors `ArgDef::mode`.
    pub fn mode(mut self, mode: Mode) -> ArgDef {
        match &mut self.form {
            ArgForm::Declared { mode: slot, .. } => *slot = Some(mode),
            ArgForm::Lent { changes, .. } => {
                changes.insert("mode".to_string(), mode.to_value());
            }
        }
        self
    }

    /// Mirrors `ArgDef::layout`.
    pub fn layout(mut self, layout: ArgLayout) -> ArgDef {
        match &mut self.form {
            ArgForm::Declared { layout: slot, .. } => *slot = layout,
            ArgForm::Lent { changes, .. } => {
                changes.insert("layout".to_string(), layout.to_value());
            }
        }
        self
    }

    /// Mirrors `ArgDef::blocks`: the argument laid out as blocks.
    pub fn blocks(self) -> ArgDef {
        self.layout(ArgLayout::Blocks)
    }

    /// Mirrors `ArgDef::scoped`: the commands of the scoped package `package`
    /// are legal in the argument.
    pub fn scoped(mut self, package: impl Into<String>) -> ArgDef {
        match &mut self.form {
            ArgForm::Declared { scoped, .. } => scoped.push(package.into()),
            ArgForm::Lent { changes, .. } => {
                let list = changes.entry("scoped").or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(items) = list {
                    items.push(Value::String(package.into()));
                }
            }
        }
        self
    }

    /// The argument's documentation (the field `docs` of `flm-core`'s).
    pub fn docs(mut self, text: impl Into<String>) -> ArgDef {
        match &mut self.form {
            ArgForm::Declared { docs, .. } => *docs = text.into(),
            ArgForm::Lent { changes, .. } => {
                changes.insert("docs".to_string(), Value::String(text.into()));
            }
        }
        self
    }

    /// Mirrors `ArgDef::role`: the argument's role, the registered name of
    /// what it holds (`flm.refs.target`), declared by a feature with
    /// `FeatureBuilder::argument_role`.
    pub fn role(mut self, name: impl Into<String>) -> ArgDef {
        match &mut self.form {
            ArgForm::Declared { role, .. } => *role = Some(name.into()),
            ArgForm::Lent { changes, .. } => {
                changes.insert("role".to_string(), Value::String(name.into()));
            }
        }
        self
    }

    /// Mirrors `ArgDef::code`: the code the argument was declared by; `None`
    /// for a helper's argument or one the host built from a parser.
    pub fn code(&self) -> Option<&str> {
        match &self.form {
            ArgForm::Declared { code, .. } => code.as_deref(),
            ArgForm::Lent { .. } => None,
        }
    }
}

/// `{"code", "name", "layout", "docs", "mode", "scoped", "role"}`, or a helper's
/// handle (of the kind `arg_def`); the host's `arg_def_to_value` and
/// `arg_defs_from_value`. A helper's argument given fields by its setters is
/// its handle's map holding those fields too, which the host's reader
/// refuses and the guest reads back as itself.
impl JsonForm for ArgDef {
    fn to_value(&self) -> Value {
        match &self.form {
            ArgForm::Declared { code, name, layout, docs, mode, scoped, role } => json!({
                "code": code,
                "name": name,
                "layout": layout.to_value(),
                "docs": docs,
                "mode": mode.as_ref().map_or(Value::Null, JsonForm::to_value),
                "scoped": scoped,
                "role": role,
            }),
            ArgForm::Lent { handle, changes } => {
                let mut value = handle_to_value(*handle);
                if let Value::Object(map) = &mut value {
                    map.extend(changes.clone());
                }
                value
            }
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an argument";
        let map = object(WHAT, value)?;
        if let Some(lent) = map.get(HANDLE_KEY) {
            let mut changes = map.clone();
            changes.remove(HANDLE_KEY);
            let handle = abi::handle_of(&json!({ HANDLE_KEY: lent }), HandleKind::ArgDef)?;
            return Ok(ArgDef { form: ArgForm::Lent { handle, changes } });
        }
        Ok(ArgDef {
            form: ArgForm::Declared {
                code: get(WHAT, map, "code")?,
                name: get(WHAT, map, "name")?,
                layout: get::<Option<ArgLayout>>(WHAT, map, "layout")?.unwrap_or_default(),
                docs: get::<Option<String>>(WHAT, map, "docs")?.unwrap_or_default(),
                mode: get(WHAT, map, "mode")?,
                scoped: get::<Option<Vec<String>>>(WHAT, map, "scoped")?.unwrap_or_default(),
                role: get(WHAT, map, "role")?,
            },
        })
    }
}

/// Mirrors `flm-core`'s `SyntaxDef`: the syntax of a declaration, in the
/// forms that cross (an inclusion's syntax is `flm-core`'s alone). The
/// guest's own: its arguments are the guest's [`ArgDef`]s.
#[derive(Debug, Clone)]
pub enum SyntaxDef {
    /// A macro and its arguments.
    Macro {
        /// The arguments.
        args: Vec<ArgDef>,
    },
    /// An environment, its arguments and its body.
    Environment {
        /// The arguments.
        args: Vec<ArgDef>,
        /// The body.
        body: BodyDef,
    },
    /// A specials' trigger and its arguments.
    Specials {
        /// The trigger.
        trigger: String,
        /// The arguments.
        args: Vec<ArgDef>,
    },
    /// An element made by the library, with no syntax.
    Synthesized,
}

/// `{"macro": {"args"}}`, `{"environment": {"args", "body"}}`,
/// `{"specials": {"trigger", "args"}}` or `"synthesized"`, the arguments a
/// list; read by the host's `syntax_def_from_value`.
impl JsonForm for SyntaxDef {
    fn to_value(&self) -> Value {
        let list = |args: &[ArgDef]| args.iter().map(JsonForm::to_value).collect::<Vec<_>>();
        match self {
            SyntaxDef::Macro { args } => json!({ "macro": { "args": list(args) } }),
            SyntaxDef::Environment { args, body } => json!({ "environment": { "args": list(args), "body": body.to_value() } }),
            SyntaxDef::Specials { trigger, args } => json!({ "specials": { "trigger": trigger, "args": list(args) } }),
            SyntaxDef::Synthesized => Value::String("synthesized".to_string()),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a syntax";
        if value.as_str() == Some("synthesized") {
            return Ok(SyntaxDef::Synthesized);
        }
        let (key, inner) = single(value, &["macro", "environment", "specials"])
            .ok_or_else(|| not_shape(WHAT, "`\"synthesized\"` or a map of one of the keys `macro`, `environment`, `specials`", value))?;
        let map = object(WHAT, inner)?;
        let args = get::<Option<Vec<ArgDef>>>(WHAT, map, "args")?.unwrap_or_default();
        Ok(match key {
            "macro" => SyntaxDef::Macro { args },
            "environment" => SyntaxDef::Environment { args, body: get(WHAT, map, "body")? },
            _ => SyntaxDef::Specials { trigger: get(WHAT, map, "trigger")?, args },
        })
    }
}

/// An element declaration as data.
#[derive(Clone)]
struct ElementDefData {
    /// The owner's name.
    owner: String,
    /// The names it declares.
    names: Vec<String>,
    /// Its syntax.
    syntax: SyntaxDef,
    /// Its element.
    element: ElementSpec,
    /// Its documentation.
    docs: String,
    /// Its modes; `None` keeps the default, text mode.
    modes: Option<Vec<Mode>>,
}

/// How an element declaration crosses.
#[derive(Clone)]
enum ElementDefForm {
    /// The declaration as data.
    Data(Box<ElementDefData>),
    /// A declaration a helper of the host made, by the handle it lent.
    Lent(Handle),
}

/// Mirrors [`ElementDef`](crate::api::lang::mint::ElementDef): the
/// declaration of an element and the names it is reached by. The guest's
/// own: a declaration is data, or one a helper of the host made
/// (`UserDefinition::element_def`), which crosses as its handle.
#[derive(Clone)]
pub struct ElementDef {
    /// How it crosses.
    form: ElementDefForm,
}

impl ElementDef {
    /// Mirrors [`ElementDef::new`](crate::api::lang::mint::ElementDef::new):
    /// the declaration of `element` under `names`, with the syntax `syntax`,
    /// owned by `owner`.
    pub fn new(owner: impl Into<String>, names: impl IntoIterator<Item = impl Into<String>>, syntax: SyntaxDef, element: ElementSpec) -> ElementDef {
        ElementDef {
            form: ElementDefForm::Data(Box::new(ElementDefData {
                owner: owner.into(),
                names: names.into_iter().map(Into::into).collect(),
                syntax,
                element,
                docs: String::new(),
                modes: None,
            })),
        }
    }

    /// Mirrors [`ElementDef::docs`](crate::api::lang::mint::ElementDef::docs);
    /// a helper's declaration asks the host.
    ///
    /// # Errors
    ///
    /// What the host answers for a helper's declaration.
    pub fn docs(mut self, text: impl Into<String>) -> Result<ElementDef, ExtError> {
        let text = text.into();
        match &mut self.form {
            ElementDefForm::Data(data) => {
                data.docs = text;
                Ok(self)
            }
            ElementDefForm::Lent(handle) => {
                let reply = abi::call(*handle, "docs", vec![arg(text.as_str(), "docs")?])?;
                ElementDef::from_value(&reply)
            }
        }
    }

    /// Mirrors [`ElementDef::modes`](crate::api::lang::mint::ElementDef::modes);
    /// a helper's declaration asks the host.
    ///
    /// # Errors
    ///
    /// What the host answers for a helper's declaration.
    pub fn modes(mut self, list: impl IntoIterator<Item = Mode>) -> Result<ElementDef, ExtError> {
        let list: Vec<Mode> = list.into_iter().collect();
        match &mut self.form {
            ElementDefForm::Data(data) => {
                data.modes = Some(list);
                Ok(self)
            }
            ElementDefForm::Lent(handle) => {
                let reply = abi::call(*handle, "modes", vec![arg(&list, "modes")?])?;
                ElementDef::from_value(&reply)
            }
        }
    }

    /// Mirrors [`ElementDef::owner`](crate::api::lang::mint::ElementDef::owner):
    /// `None` for a helper's declaration.
    pub fn owner(&self) -> Option<&str> {
        match &self.form {
            ElementDefForm::Data(data) => Some(&data.owner),
            ElementDefForm::Lent(_) => None,
        }
    }

    /// Mirrors [`ElementDef::names`](crate::api::lang::mint::ElementDef::names).
    pub fn names(&self) -> Option<&[String]> {
        match &self.form {
            ElementDefForm::Data(data) => Some(&data.names),
            ElementDefForm::Lent(_) => None,
        }
    }

    /// Mirrors [`ElementDef::syntax`](crate::api::lang::mint::ElementDef::syntax).
    pub fn syntax(&self) -> Option<&SyntaxDef> {
        match &self.form {
            ElementDefForm::Data(data) => Some(&data.syntax),
            ElementDefForm::Lent(_) => None,
        }
    }

    /// Mirrors [`ElementDef::element`](crate::api::lang::mint::ElementDef::element).
    pub fn element(&self) -> Option<&ElementSpec> {
        match &self.form {
            ElementDefForm::Data(data) => Some(&data.element),
            ElementDefForm::Lent(_) => None,
        }
    }
}

/// As the api writes it: the data form's fields; a helper's declaration by
/// its handle.
impl fmt::Debug for ElementDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.form {
            ElementDefForm::Data(data) => f
                .debug_struct("ElementDef")
                .field("owner", &data.owner)
                .field("names", &data.names)
                .field("syntax", &data.syntax)
                .field("element", &data.element)
                .field("docs", &data.docs)
                .field("modes", &data.modes)
                .finish(),
            ElementDefForm::Lent(handle) => f.debug_struct("ElementDef").field("handle", handle).finish(),
        }
    }
}

/// `{"owner", "names", "syntax", "element", "docs", "modes"}`, or a helper's
/// handle (of the kind `element_def`); read by the host's
/// `element_def_from_table`.
impl JsonForm for ElementDef {
    fn to_value(&self) -> Value {
        match &self.form {
            ElementDefForm::Data(data) => json!({
                "owner": data.owner,
                "names": data.names,
                "syntax": data.syntax.to_value(),
                "element": data.element.to_value(),
                "docs": data.docs,
                "modes": data.modes.as_ref().map_or(Value::Null, JsonForm::to_value),
            }),
            ElementDefForm::Lent(handle) => handle_to_value(*handle),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an element declaration";
        if single(value, &[crate::adapter::values::HANDLE_KEY]).is_some() {
            return Ok(ElementDef { form: ElementDefForm::Lent(abi::handle_of(value, HandleKind::ElementDef)?) });
        }
        let map = object(WHAT, value)?;
        Ok(ElementDef {
            form: ElementDefForm::Data(Box::new(ElementDefData {
                owner: get::<Option<String>>(WHAT, map, "owner")?.unwrap_or_default(),
                names: get::<Option<Vec<String>>>(WHAT, map, "names")?.unwrap_or_default(),
                syntax: get(WHAT, map, "syntax")?,
                element: get(WHAT, map, "element")?,
                docs: get::<Option<String>>(WHAT, map, "docs")?.unwrap_or_default(),
                modes: get(WHAT, map, "modes")?,
            })),
        })
    }
}

/// Mirrors [`FallbackDef`](crate::api::lang::mint::FallbackDef): the
/// element unknown macros or environments of some modes become.
#[derive(Debug, Clone)]
pub struct FallbackDef {
    /// The modes it applies in.
    pub modes: Vec<Mode>,
    /// Whether it catches macros or environments.
    pub kind: FallbackKind,
    /// The element.
    pub element: ElementSpec,
}

impl FallbackDef {
    /// Mirrors [`FallbackDef::new`](crate::api::lang::mint::FallbackDef::new).
    pub fn new(modes: impl IntoIterator<Item = Mode>, kind: FallbackKind, element: ElementSpec) -> FallbackDef {
        FallbackDef { modes: modes.into_iter().collect(), kind, element }
    }
}

/// `{"modes", "kind", "element"}`; read by the host's
/// `fallback_def_from_table`.
impl JsonForm for FallbackDef {
    fn to_value(&self) -> Value {
        json!({ "modes": self.modes.to_value(), "kind": self.kind.to_value(), "element": self.element.to_value() })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a fallback";
        let map = object(WHAT, value)?;
        Ok(FallbackDef { modes: get(WHAT, map, "modes")?, kind: get(WHAT, map, "kind")?, element: get(WHAT, map, "element")? })
    }
}

/// Mirrors [`ScopedPackageDef`](crate::api::lang::mint::ScopedPackageDef): a
/// package of elements legal only where a declaration scopes it in.
#[derive(Debug, Clone)]
pub struct ScopedPackageDef {
    /// The package's name.
    pub name: String,
    /// Its elements.
    pub elements: Vec<ElementDef>,
    /// Its documentation.
    pub docs: String,
}

impl ScopedPackageDef {
    /// Mirrors [`ScopedPackageDef::new`](crate::api::lang::mint::ScopedPackageDef::new).
    pub fn new(name: impl Into<String>, elements: Vec<ElementDef>, docs: impl Into<String>) -> ScopedPackageDef {
        ScopedPackageDef { name: name.into(), elements, docs: docs.into() }
    }
}

/// `{"name", "elements", "docs"}`; read by the host's
/// `scoped_package_def_from_table`.
impl JsonForm for ScopedPackageDef {
    fn to_value(&self) -> Value {
        json!({ "name": self.name, "elements": self.elements.to_value(), "docs": self.docs })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a scoped package";
        let map = object(WHAT, value)?;
        Ok(ScopedPackageDef {
            name: get(WHAT, map, "name")?,
            elements: get::<Option<Vec<ElementDef>>>(WHAT, map, "elements")?.unwrap_or_default(),
            docs: get::<Option<String>>(WHAT, map, "docs")?.unwrap_or_default(),
        })
    }
}
