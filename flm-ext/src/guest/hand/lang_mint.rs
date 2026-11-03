//! The guest's own types of `flm:lang/mint`: [`ArgDef`] (an argument
//! declared by its standard kind, or read from its code), [`ArgListData`]
//! (an argument list written as data, which an extension's settings hold),
//! [`ArgStdKind`] and [`TackOnField`] (a standard kind, whose tack-on fields
//! hold the guest's arguments), [`SyntaxDef`],
//! [`ElementDef`] (an element declaration as data, or a helper's, by its
//! handle), [`FallbackDef`] and [`ScopedPackageDef`].

use std::borrow::Cow;
use std::fmt;

use flm_core::lang::mint::{
    ArgCode, ArgData, ArgDataError, ArgDef as CoreArgDef, ArgListData as CoreArgListData, ArgListError, ArgStdKind as CoreArgStdKind,
    TackOnField as CoreTackOnField,
};
#[cfg(feature = "native")]
use flm_core::model::names::Name;
use serde::de::{SeqAccess, Visitor};
use serde_json::{json, Map, Value};

use super::{get, not_shape, object, single};
#[cfg(feature = "native")]
use crate::adapter::dispatch::entry;
#[cfg(feature = "native")]
use crate::adapter::handles::{CallTable, Entry};
use crate::adapter::handles::{Handle, HandleKind};
#[cfg(feature = "native")]
use crate::adapter::typed::{FromApi, IntoApi};
#[cfg(feature = "native")]
use crate::adapter::values::mismatch;
#[cfg(feature = "native")]
use crate::api;
use crate::adapter::values::{arg_code_error, handle_to_value, read, JsonForm};
use crate::api::lang::mint::{ArgLayout, BodyDef, FallbackKind, Mode};
use crate::guest::abi;
#[cfg(not(feature = "native"))]
use crate::guest::abi::ToArg;
use crate::guest::model::element::ElementSpec;
use crate::runtime::ExtError;

/// Mirrors `flm-core`'s `ArgStdKind`: the standard argument kinds, each
/// describing, as data, what one argument looks like in the source. The
/// guest's own: the fields of a tack-on argument are the guest's
/// [`TackOnField`]s, whose arguments are the guest's [`ArgDef`]s. A kind
/// crosses as its structured syntax, the map the host reads and writes
/// (`{"syntax": "delimited-group", "delimiters": ["(", ")"]}`).
#[derive(Debug, Clone, PartialEq)]
pub enum ArgStdKind {
    /// Mirrors `ArgStdKind::Mandatory`: a mandatory `{…}` group, or a single
    /// token.
    Mandatory,
    /// Mirrors `ArgStdKind::Optional`: an optional `[…]` group.
    Optional,
    /// Mirrors `ArgStdKind::Star`: an optional `*` marker.
    Star,
    /// Mirrors `ArgStdKind::Marker`: an optional marker made of one or more
    /// characters, matched as they are written.
    Marker {
        /// The marker's characters, one or more.
        marker: String,
    },
    /// Mirrors `ArgStdKind::DelimitedGroup`: a mandatory group between a
    /// prescribed pair of characters.
    DelimitedGroup {
        /// The opening character.
        open: char,
        /// The closing character.
        close: char,
    },
    /// Mirrors `ArgStdKind::OptionalDelimitedGroup`: an optional group
    /// between a prescribed pair of characters.
    OptionalDelimitedGroup {
        /// The opening character.
        open: char,
        /// The closing character.
        close: char,
    },
    /// Mirrors `ArgStdKind::AnyDelimitedGroup`: a mandatory group between
    /// any of the pairs `{…}`, `[…]`, `(…)` and `<…>`.
    AnyDelimitedGroup,
    /// Mirrors `ArgStdKind::OptionalAnyDelimitedGroup`: an optional group
    /// between any of those pairs.
    OptionalAnyDelimitedGroup,
    /// Mirrors `ArgStdKind::BracedOnly`: a mandatory `{…}` group and nothing
    /// else.
    BracedOnly,
    /// Mirrors `ArgStdKind::Embellishments`: a run of marker–expression
    /// pairs, each marker at most once.
    Embellishments {
        /// The marker characters, at least one.
        markers: Vec<char>,
    },
    /// Mirrors `ArgStdKind::KeyVals`: an optional `[key=value,…]` group.
    KeyVals,
    /// Mirrors `ArgStdKind::CharsGroup`: a mandatory `{…}` group read as
    /// plain characters.
    CharsGroup,
    /// Mirrors `ArgStdKind::Verbatim`: a verbatim argument delimited by a
    /// pair of characters.
    Verbatim {
        /// `None` matches the closing delimiter to whichever opening
        /// character is found; `Some((open, close))` prescribes the pair.
        delimiters: Option<(char, char)>,
    },
    /// Mirrors `ArgStdKind::TackOn`: the fields a command accepts, absorbed
    /// as a run of field macros where the argument stands. What differs: the
    /// fields are the guest's [`TackOnField`]s.
    TackOn {
        /// The fields this command accepts, in any order after the command.
        fields: Vec<TackOnField>,
    },
    /// Mirrors `ArgStdKind::Terminated`: a mandatory argument that runs up to
    /// a fixed character sequence, its terminator.
    Terminated {
        /// The terminator's text.
        terminator: String,
        /// The character whose run the prefix and the terminator repeat;
        /// `None` for none.
        repeat_char: Option<char>,
        /// The text after the prefix's run, empty when not needed.
        opener: String,
        /// The text before the terminator's run, empty when not needed.
        head: String,
        /// Whether the content is raw text rather than parsed markup.
        raw: bool,
    },
}

/// A kind as `flm-core`'s, or the fields of a tack-on argument, which hold
/// the guest's arguments.
enum CoreForm<'k> {
    /// `flm-core`'s kind.
    Kind(CoreArgStdKind),
    /// A tack-on argument's fields.
    TackOn(&'k [TackOnField]),
}

impl ArgStdKind {
    /// The kind as `flm-core`'s, for every kind but a tack-on argument, whose
    /// fields are answered as they are.
    fn core_form(&self) -> CoreForm<'_> {
        CoreForm::Kind(match self {
            ArgStdKind::Mandatory => CoreArgStdKind::Mandatory,
            ArgStdKind::Optional => CoreArgStdKind::Optional,
            ArgStdKind::Star => CoreArgStdKind::Star,
            ArgStdKind::Marker { marker } => CoreArgStdKind::Marker { marker: marker.clone() },
            ArgStdKind::DelimitedGroup { open, close } => CoreArgStdKind::DelimitedGroup { open: *open, close: *close },
            ArgStdKind::OptionalDelimitedGroup { open, close } => {
                CoreArgStdKind::OptionalDelimitedGroup { open: *open, close: *close }
            }
            ArgStdKind::AnyDelimitedGroup => CoreArgStdKind::AnyDelimitedGroup,
            ArgStdKind::OptionalAnyDelimitedGroup => CoreArgStdKind::OptionalAnyDelimitedGroup,
            ArgStdKind::BracedOnly => CoreArgStdKind::BracedOnly,
            ArgStdKind::Embellishments { markers } => CoreArgStdKind::Embellishments { markers: markers.clone() },
            ArgStdKind::KeyVals => CoreArgStdKind::KeyVals,
            ArgStdKind::CharsGroup => CoreArgStdKind::CharsGroup,
            ArgStdKind::Verbatim { delimiters } => CoreArgStdKind::Verbatim { delimiters: *delimiters },
            ArgStdKind::TackOn { fields } => return CoreForm::TackOn(fields),
            ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw } => CoreArgStdKind::Terminated {
                terminator: terminator.clone(),
                repeat_char: *repeat_char,
                opener: opener.clone(),
                head: head.clone(),
                raw: *raw,
            },
        })
    }

    /// The kind of `flm-core`'s `kind`, its tack-on fields' arguments read as
    /// the guest's ([`ArgDef`]'s reading of `flm-core`'s argument).
    ///
    /// # Errors
    ///
    /// As that reading: an argument of a field has a parser of a feature's
    /// own ([`ExtError::Value`]).
    fn from_core(kind: CoreArgStdKind) -> Result<ArgStdKind, ExtError> {
        Ok(match kind {
            CoreArgStdKind::Mandatory => ArgStdKind::Mandatory,
            CoreArgStdKind::Optional => ArgStdKind::Optional,
            CoreArgStdKind::Star => ArgStdKind::Star,
            CoreArgStdKind::Marker { marker } => ArgStdKind::Marker { marker },
            CoreArgStdKind::DelimitedGroup { open, close } => ArgStdKind::DelimitedGroup { open, close },
            CoreArgStdKind::OptionalDelimitedGroup { open, close } => ArgStdKind::OptionalDelimitedGroup { open, close },
            CoreArgStdKind::AnyDelimitedGroup => ArgStdKind::AnyDelimitedGroup,
            CoreArgStdKind::OptionalAnyDelimitedGroup => ArgStdKind::OptionalAnyDelimitedGroup,
            CoreArgStdKind::BracedOnly => ArgStdKind::BracedOnly,
            CoreArgStdKind::Embellishments { markers } => ArgStdKind::Embellishments { markers },
            CoreArgStdKind::KeyVals => ArgStdKind::KeyVals,
            CoreArgStdKind::CharsGroup => ArgStdKind::CharsGroup,
            CoreArgStdKind::Verbatim { delimiters } => ArgStdKind::Verbatim { delimiters },
            CoreArgStdKind::TackOn { fields } => {
                ArgStdKind::TackOn { fields: fields.into_iter().map(TackOnField::from_core).collect::<Result<_, _>>()? }
            }
            CoreArgStdKind::Terminated { terminator, repeat_char, opener, head, raw } => {
                ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw }
            }
        })
    }
}

/// The structured syntax, `{"syntax", ...}` with the kind's parameters
/// beside the key `syntax`, as `flm-core` writes and reads its kind; a
/// tack-on argument's fields are `{"name", "args", "repeatable", "docs"}`,
/// their arguments each as [`ArgDef`] writes it.
impl JsonForm for ArgStdKind {
    fn to_value(&self) -> Value {
        match self.core_form() {
            CoreForm::Kind(kind) => kind.to_value(),
            CoreForm::TackOn(fields) => {
                let fields: Vec<Value> = fields
                    .iter()
                    .map(|field| {
                        let args: Vec<Value> = field.args.iter().map(JsonForm::to_value).collect();
                        json!({ "name": field.name, "args": args, "repeatable": field.repeatable, "docs": field.docs })
                    })
                    .collect();
                json!({ "syntax": "tack-on", "fields": fields })
            }
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        // Read as the host reads a kind, which states what it refuses.
        CoreArgStdKind::from_value(value).and_then(ArgStdKind::from_core)
    }
}

#[cfg(feature = "native")]
impl ArgStdKind {
    /// `flm-core`'s kind, a tack-on argument's fields with their arguments
    /// converted as [`args_to_core`] converts an argument list, as the host
    /// reads the kind's structured syntax.
    fn to_core(&self) -> CoreArgStdKind {
        match self.core_form() {
            CoreForm::Kind(kind) => kind,
            CoreForm::TackOn(fields) => CoreArgStdKind::TackOn {
                fields: fields
                    .iter()
                    .map(|field| CoreTackOnField {
                        name: field.name.clone(),
                        args: args_to_core(&field.args),
                        repeatable: field.repeatable,
                        docs: field.docs.clone(),
                    })
                    .collect(),
            },
        }
    }
}

/// Mirrors `flm-core`'s `TackOnField`: one field of a tack-on argument, a
/// macro written directly after the command, which the command reads as
/// data. The guest's own: its arguments are the guest's [`ArgDef`]s. It
/// crosses inside its tack-on argument's structured syntax ([`ArgStdKind`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TackOnField {
    /// The field's name, without the escape character (`label`), distinct
    /// from the names of the other fields of its tack-on argument.
    pub name: String,
    /// The field's own arguments, in the order they are written.
    pub args: Vec<ArgDef>,
    /// Whether the field may be written more than once after one construct.
    pub repeatable: bool,
    /// Documentation of the field.
    pub docs: String,
}

impl TackOnField {
    /// Mirrors `TackOnField::new`: the field written `\name`, taking the
    /// given arguments, which may be written at most once after the
    /// construct.
    pub fn new(name: impl Into<String>, args: Vec<ArgDef>) -> TackOnField {
        TackOnField { name: name.into(), args, repeatable: false, docs: String::new() }
    }

    /// Mirrors `TackOnField::repeatable`: the field may be written more than
    /// once after one construct.
    pub fn repeatable(mut self) -> TackOnField {
        self.repeatable = true;
        self
    }

    /// Mirrors `TackOnField::docs`: sets the field's documentation.
    pub fn docs(mut self, docs: impl Into<String>) -> TackOnField {
        self.docs = docs.into();
        self
    }

    /// The field of `flm-core`'s `field`, its arguments read as the guest's
    /// ([`ArgDef`]'s reading of `flm-core`'s argument).
    ///
    /// # Errors
    ///
    /// As that reading: an argument has a parser of a feature's own
    /// ([`ExtError::Value`]).
    fn from_core(field: CoreTackOnField) -> Result<TackOnField, ExtError> {
        Ok(TackOnField {
            name: field.name,
            args: field.args.iter().map(ArgDef::from_core).collect::<Result<_, _>>()?,
            repeatable: field.repeatable,
            docs: field.docs,
        })
    }
}

/// Mirrors `flm-core`'s `ArgDef`: an argument of a declaration. The guest's
/// own: an argument is declared by its standard kind ([`ArgDef::new`]),
/// which crosses as its structured syntax (`{"syntax": "chars-group", …}`),
/// or read from an argument code ([`ArgDef::from_code`]), which crosses as
/// that code, where the api's holds a parser. For an argument read from a
/// code, it keeps that code with the kind it stands for, as `flm-core`'s
/// does, so that the code is read once, where the argument is made. An
/// argument the host gives the guest, one a helper made included
/// (`refs::label_fields_arg`), is written with its code or its structured
/// syntax and reads back as an argument read from that code or declared by
/// that kind; the host writes an argument whose parser is a feature's own
/// with neither, and such an argument does not read back: reading it is an
/// error.
///
/// An argument is also read and written as data with `serde`, in the data
/// form of an argument that `flm-core`'s settings read (`ArgData`): an
/// argument code alone, or an argument's map, which gives the code or the
/// structured syntax beside the argument's name and its other keys. Its JSON
/// schema (`schemars`) is that data form's. A settings type of an extension
/// can therefore hold the guest's arguments and derive its schema, as a
/// settings type of `flm-core` holds `flm-core`'s arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgDef {
    /// How it is declared: by its standard kind, or by an argument code.
    syntax: Syntax,
    /// Its name.
    name: Option<String>,
    /// Its layout.
    layout: ArgLayout,
    /// Whether its content is placed under an opaque wrapper.
    opaque_wrapper: bool,
    /// Its documentation.
    docs: String,
    /// Its mode, when it is not that of its surroundings.
    mode: Option<Mode>,
    /// The scoped packages whose commands are legal in it.
    scoped: Vec<String>,
    /// Its role: the registered name of what it holds.
    role: Option<String>,
}

/// How an argument is declared: by its standard kind, or by an argument
/// code, read where the argument was made, with the kind it stands for.
#[derive(Debug, Clone, PartialEq)]
enum Syntax {
    /// Declared by its standard kind ([`ArgDef::new`]).
    Kind(ArgStdKind),
    /// Read from an argument code ([`ArgDef::from_code`]).
    Code(ArgCode),
}

impl ArgDef {
    /// Mirrors `ArgDef::new`: the argument declared by the standard kind
    /// `kind` (`ArgDef::new(ArgStdKind::Star)`), with no name. What differs:
    /// the kind is the guest's [`ArgStdKind`], where the api's takes a
    /// parser. An argument written as an argument code is read with
    /// [`ArgDef::from_code`].
    pub fn new(kind: ArgStdKind) -> ArgDef {
        ArgDef::declared(Syntax::Kind(kind))
    }

    /// The argument declared by `syntax`, with no name, inline layout, no
    /// opaque wrapper, no documentation, the mode of its surroundings, no
    /// scoped package and no role.
    fn declared(syntax: Syntax) -> ArgDef {
        ArgDef {
            syntax,
            name: None,
            layout: ArgLayout::default(),
            opaque_wrapper: false,
            docs: String::new(),
            mode: None,
            scoped: Vec::new(),
            role: None,
        }
    }

    /// Mirrors `ArgDef::named`: the argument declared by the standard kind
    /// `kind`, named `name`.
    pub fn named(name: impl Into<String>, kind: ArgStdKind) -> ArgDef {
        ArgDef { name: Some(name.into()), ..ArgDef::new(kind) }
    }

    /// Mirrors `arg_def_from_code`: the argument the argument code `code`
    /// stands for (`{`, `[`, `r()`, …), with no name, which keeps the code it
    /// was read from ([`ArgDef::code`]). What differs: a text that is not an
    /// argument code is [`ExtError::Core`]. The code is read here, where the
    /// argument is made, by `flm-core`'s reader (`ArgCode::read`), with no
    /// request to the host, so that `define` reads one too; the argument
    /// keeps it read, and crosses to the host with no second reading.
    ///
    /// # Errors
    ///
    /// `code` is not an argument code ([`ExtError::Core`] of kind `arg-code`,
    /// with `flm-core`'s text, which names the code).
    pub fn from_code(code: &str) -> Result<ArgDef, ExtError> {
        ArgCode::read(code).map(|code| ArgDef::declared(Syntax::Code(code))).map_err(arg_code_error)
    }

    /// No `flm-core` counterpart: the argument the argument code `code`
    /// stands for, read as [`ArgDef::from_code`] reads it, named `name`, which
    /// `flm-core` names by setting the field `name` of the argument
    /// `arg_def_from_code` answers.
    ///
    /// # Errors
    ///
    /// As [`ArgDef::from_code`].
    pub fn named_from_code(name: impl Into<String>, code: &str) -> Result<ArgDef, ExtError> {
        Ok(ArgDef { name: Some(name.into()), ..ArgDef::from_code(code)? })
    }

    /// Mirrors `ArgDef::mandatory`: a mandatory `{…}` argument named `name`,
    /// declared by the standard kind [`ArgStdKind::Mandatory`].
    pub fn mandatory(name: impl Into<String>) -> ArgDef {
        ArgDef::named(name, ArgStdKind::Mandatory)
    }

    /// Mirrors `ArgDef::optional`: an optional `[…]` argument named `name`,
    /// declared by the standard kind [`ArgStdKind::Optional`].
    pub fn optional(name: impl Into<String>) -> ArgDef {
        ArgDef::named(name, ArgStdKind::Optional)
    }

    /// Mirrors `ArgDef::mode`.
    pub fn mode(mut self, mode: Mode) -> ArgDef {
        self.mode = Some(mode);
        self
    }

    /// Mirrors `ArgDef::layout`.
    pub fn layout(mut self, layout: ArgLayout) -> ArgDef {
        self.layout = layout;
        self
    }

    /// Mirrors `ArgDef::blocks`: the argument laid out as blocks.
    pub fn blocks(self) -> ArgDef {
        self.layout(ArgLayout::Blocks)
    }

    /// Mirrors `ArgDef::with_opaque_wrapper`: the argument's content placed
    /// under an opaque wrapper, which keeps its text as typed.
    pub fn with_opaque_wrapper(mut self) -> ArgDef {
        self.opaque_wrapper = true;
        self
    }

    /// Mirrors `ArgDef::scoped`: the commands of the scoped package `package`
    /// are legal in the argument.
    pub fn scoped(mut self, package: impl Into<String>) -> ArgDef {
        self.scoped.push(package.into());
        self
    }

    /// The argument's documentation (the field `docs` of `flm-core`'s).
    pub fn docs(mut self, text: impl Into<String>) -> ArgDef {
        self.docs = text.into();
        self
    }

    /// Mirrors `ArgDef::role`: the argument's role, the registered name of
    /// what it holds (`flm.refs.target`), declared by a feature with
    /// `FeatureBuilder::argument_role`.
    pub fn role(mut self, name: impl Into<String>) -> ArgDef {
        self.role = Some(name.into());
        self
    }

    /// Mirrors `ArgDef::code`: the argument code the argument was read from
    /// ([`ArgDef::from_code`], or an argument the host wrote with its code);
    /// `None` for one declared by its standard kind.
    pub fn code(&self) -> Option<&str> {
        match &self.syntax {
            Syntax::Code(code) => Some(code.as_str()),
            Syntax::Kind(_) => None,
        }
    }

    /// The guest's argument of `flm-core`'s `arg`: read from its code where
    /// it was read from one, the code taken as `flm-core` keeps it read
    /// (`ArgDef::arg_code`), else declared by its parser's standard kind;
    /// with its name, layout, wrapper, documentation, mode, scoped packages
    /// and role. The JSON form, `serde` and the typed route each read
    /// `flm-core`'s argument, then this.
    ///
    /// # Errors
    ///
    /// `arg`'s parser is a feature's own, which has no data form
    /// ([`ExtError::Value`], with `flm-core`'s text for an argument's map
    /// that gives neither a code nor a structured syntax).
    fn from_core(arg: &CoreArgDef) -> Result<ArgDef, ExtError> {
        let syntax = match (arg.arg_code(), arg.kind.std_kind()) {
            (Some(code), _) => Syntax::Code(code.clone()),
            (None, Some(kind)) => Syntax::Kind(ArgStdKind::from_core(kind.clone())?),
            (None, None) => return Err(ExtError::Value { detail: ArgDataError::NeitherCodeNorSyntax.to_string() }),
        };
        Ok(ArgDef {
            syntax,
            name: arg.name.clone(),
            layout: arg.layout,
            opaque_wrapper: arg.opaque_wrapper,
            docs: arg.docs.clone(),
            mode: arg.mode,
            scoped: arg.scoped.iter().map(ToString::to_string).collect(),
            role: arg.role.as_ref().map(ToString::to_string),
        })
    }
}

/// `{"code", "name", "layout", "opaque_wrapper", "docs", "mode", "scoped",
/// "role"}`, with the keys of the structured syntax (`"syntax"` and its
/// parameters, [`ArgStdKind`]) in place of `"code"` for an argument declared
/// by one; the host's `arg_def_to_value` and `arg_defs_from_value`.
impl JsonForm for ArgDef {
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        match &self.syntax {
            Syntax::Code(code) => {
                map.insert("code".to_string(), Value::String(code.as_str().to_string()));
            }
            Syntax::Kind(kind) => {
                if let Value::Object(keys) = kind.to_value() {
                    map.extend(keys);
                }
            }
        }
        map.extend([
            ("name".to_string(), json!(self.name)),
            ("layout".to_string(), self.layout.to_value()),
            ("opaque_wrapper".to_string(), Value::Bool(self.opaque_wrapper)),
            ("docs".to_string(), Value::String(self.docs.clone())),
            ("mode".to_string(), self.mode.as_ref().map_or(Value::Null, JsonForm::to_value)),
            ("scoped".to_string(), json!(self.scoped)),
            ("role".to_string(), json!(self.role)),
        ]);
        Value::Object(map)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        // Read as the host reads an argument, which tells the keys of the
        // structured syntax from the argument's own and reads a code with
        // `flm-core`'s reader, then made the guest's.
        let arg: CoreArgDef = read("an argument", value)?;
        ArgDef::from_core(&arg)
    }
}

/// Mirrors the reading of `flm-core`'s argument from `ArgData`, the data form
/// of an argument in settings: an argument code alone (`"{"`), or an
/// argument's map, which gives the argument's syntax as a code (`code: "{"`)
/// or as a structured syntax (`syntax: delimited-group` with the kind's
/// parameters) beside the argument's own keys (`name`, `mode`, `layout`,
/// `opaque_wrapper`, `scoped`, `docs` and `role`). What differs: the argument
/// read is the guest's, read from that code as [`ArgDef::from_code`] reads it
/// or declared by that standard kind as [`ArgDef::new`] declares it, with
/// those keys as the setters set them.
///
/// The reading refuses what `flm-core`'s refuses, with its messages: a code
/// that is not a complete argument code, a map that gives both a code and a
/// structured syntax or neither, a key of the map that is neither one of the
/// argument's own nor a parameter of the kind, and a parameter that does not
/// read.
impl<'de> serde::Deserialize<'de> for ArgDef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ArgDef, D::Error> {
        // Read as `flm-core` reads its own argument (from `ArgData`, through
        // its conversion), then made the guest's as the host's arguments are.
        let arg = <CoreArgDef as serde::Deserialize>::deserialize(deserializer)?;
        ArgDef::from_core(&arg).map_err(serde::de::Error::custom)
    }
}

/// Writes the argument in the data form of `flm-core`'s `ArgData`, as an
/// argument's map: its code, or the structured syntax of its standard kind,
/// beside every one of its own keys (`name`, `layout`, `opaque_wrapper`,
/// `docs`, `mode`, `scoped` and `role`; `null` for a name, a mode or a role it
/// does not have). It is the map that `flm-core` writes for the same argument
/// in a declaration written as data, and it reads back as the same argument.
impl serde::Serialize for ArgDef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.to_value(), serializer)
    }
}

/// Gives the JSON schema of `flm-core`'s `ArgData`, under its name: an
/// argument code, one of the codes listed with what each declares, or an
/// argument's map, with one alternative per form of its syntax (a code, then
/// each standard kind with its parameters). A settings type that holds the
/// guest's arguments derives the schema that the same type holding `ArgData`
/// derives in `flm-core`.
impl schemars::JsonSchema for ArgDef {
    fn inline_schema() -> bool {
        <ArgData as schemars::JsonSchema>::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        <ArgData as schemars::JsonSchema>::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        <ArgData as schemars::JsonSchema>::schema_id()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <ArgData as schemars::JsonSchema>::json_schema(generator)
    }
}

#[cfg(feature = "native")]
impl ArgDef {
    /// `flm-core`'s argument, made from this one's code as read already
    /// (`flm-core`'s `From<ArgCode>`) or declared by its kind, with its name,
    /// layout, wrapper, documentation, mode, scoped packages and role, as the
    /// host reads the argument's map (`arg_defs_from_value`).
    fn to_core(&self) -> CoreArgDef {
        let arg = match &self.syntax {
            Syntax::Code(code) => CoreArgDef::from(code.clone()),
            Syntax::Kind(kind) => CoreArgDef::new(kind.to_core()),
        };
        let mut arg = arg.layout(self.layout).docs(self.docs.clone());
        arg.name = self.name.clone();
        arg.opaque_wrapper = self.opaque_wrapper;
        arg.mode = self.mode;
        arg.scoped = self.scoped.iter().map(|package| Name::from(package.as_str())).collect();
        arg.role = self.role.as_deref().map(Name::from);
        arg
    }
}

/// `flm-core`'s arguments of `args`, each as [`ArgDef::to_core`] converts it,
/// as the host reads an argument list (`arg_defs_from_value`).
#[cfg(feature = "native")]
pub(crate) fn args_to_core(args: &[ArgDef]) -> Vec<CoreArgDef> {
    args.iter().map(ArgDef::to_core).collect()
}

/// The guest's argument of `flm-core`'s `api`, as the guest reads the host's
/// `arg_def_to_value`: read from its code where it was read from one, else
/// declared by its parser's standard kind ([`ArgDef`]'s reading of
/// `flm-core`'s argument).
#[cfg(feature = "native")]
impl<'c> FromApi<'c, CoreArgDef> for ArgDef {
    fn from_api(api: CoreArgDef, _: &mut CallTable<'c>) -> Result<Self, ExtError> {
        ArgDef::from_core(&api)
    }
}

/// Mirrors `flm-core`'s `ArgListData`: the argument list of a declaration
/// written as data, either one text in which each character is one argument
/// code (`"[{"`), or a list of arguments, each written as an argument code or
/// as an argument's map. The guest's own, which no item of the api names: an
/// extension's settings type holds it (the arguments of a macro the settings
/// declare), and it never crosses the boundary by itself: the settings that
/// hold it cross as a value. What differs: a list holds the guest's
/// arguments ([`ArgDef`]), each read where the list is read.
///
/// It is read with `serde` in the shapes `flm-core`'s reads, and written in
/// the shape it holds (the text, or the list of its arguments in their own
/// data form), so that a settings type holding it writes back what it read.
/// Its JSON schema (`schemars`) is `flm-core`'s, so that a settings type
/// holding it derives the schema that the same type holding `flm-core`'s
/// derives.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum ArgListData {
    /// One text in which each character is one argument code: `"[{{"`.
    Codes(String),
    /// A list of arguments, each written as an argument code or a map.
    List(Vec<ArgDef>),
}

impl ArgListData {
    /// Mirrors `ArgListData::arg_defs`: the arguments of the list, in the
    /// order they are written. What differs: a list's arguments are the
    /// guest's already, read with the list, so only one text of codes can
    /// fail; and the error is the host's `arg-code` error
    /// ([`ExtError::Core`]), not `flm-core`'s `ArgListError`. Each character
    /// of one text of codes is read here, by `flm-core`'s reader and with no
    /// request to the host, as [`ArgDef::from_code`] reads a code.
    ///
    /// # Errors
    ///
    /// A character of one text of codes is not an argument code
    /// ([`ExtError::Core`] of kind `arg-code`, with `flm-core`'s text, which
    /// names the character's position from 1 and the code: `argument 2: …`).
    pub fn arg_defs(self) -> Result<Vec<ArgDef>, ExtError> {
        match self {
            ArgListData::Codes(codes) => codes
                .chars()
                .enumerate()
                .map(|(index, code)| {
                    ArgCode::read(&String::from(code))
                        .map(|code| ArgDef::declared(Syntax::Code(code)))
                        .map_err(|error| arg_code_error(ArgListError { index, error: error.into() }))
                })
                .collect(),
            ArgListData::List(args) => Ok(args),
        }
    }
}

/// Mirrors the reading of `flm-core`'s `ArgListData`: a text is one text of
/// codes ([`ArgListData::Codes`]), and a list is a list of arguments
/// ([`ArgListData::List`]), each entry read as [`ArgDef`] reads itself; any
/// other value is refused, with `flm-core`'s words for what an argument list
/// is. What differs: the error of an entry that cannot be read names the
/// entry's position from 1 before the entry's own message (`argument 2: …`),
/// as `flm-core`'s error of a code of one text of codes names the code's
/// position, so that a fault in an extension's settings, which the guest
/// reads from a value with no path, is located; `flm-core`'s reading of a
/// list locates an entry by its path when the settings are read.
impl<'de> serde::Deserialize<'de> for ArgListData {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ArgListData, D::Error> {
        struct List;

        impl<'de> Visitor<'de> for List {
            type Value = ArgListData;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a text of argument codes or a list of arguments")
            }

            fn visit_str<E: serde::de::Error>(self, codes: &str) -> Result<ArgListData, E> {
                Ok(ArgListData::Codes(codes.to_string()))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ArgListData, A::Error> {
                let mut args = Vec::new();
                while let Some(arg) = seq
                    .next_element::<ArgDef>()
                    .map_err(|error| serde::de::Error::custom(format!("argument {}: {error}", args.len() + 1)))?
                {
                    args.push(arg);
                }
                Ok(ArgListData::List(args))
            }
        }

        deserializer.deserialize_any(List)
    }
}

/// Gives the JSON schema of `flm-core`'s `ArgListData`, under its name: one
/// text of the one-character codes, or a list of arguments, each with the
/// schema of `flm-core`'s `ArgData` ([`ArgDef`]'s). A settings type that
/// holds the guest's argument list derives the schema that the same type
/// holding `ArgListData` derives in `flm-core`, with one definition of the
/// list's shape.
impl schemars::JsonSchema for ArgListData {
    fn inline_schema() -> bool {
        <CoreArgListData as schemars::JsonSchema>::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        <CoreArgListData as schemars::JsonSchema>::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        <CoreArgListData as schemars::JsonSchema>::schema_id()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <CoreArgListData as schemars::JsonSchema>::json_schema(generator)
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

/// The api's syntax, its arguments converted as the host reads an argument
/// list ([`args_to_core`]); the host's `syntax_def_from_value`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &SyntaxDef {
    type Api = api::lang::mint::SyntaxDef;

    fn into_api(self, _: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        use api::lang::mint::SyntaxDef as Api;
        Ok(match self {
            SyntaxDef::Macro { args } => Api::Macro { args: args_to_core(args) },
            SyntaxDef::Environment { args, body } => Api::Environment { args: args_to_core(args), body: body.clone() },
            SyntaxDef::Specials { trigger, args } => Api::Specials { trigger: trigger.clone(), args: args_to_core(args) },
            SyntaxDef::Synthesized => Api::Synthesized,
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
                #[cfg(not(feature = "native"))]
                {
                    let reply = abi::call(*handle, "docs", vec![ToArg::arg(text.as_str())])?;
                    ElementDef::from_value(&reply)
                }
                #[cfg(feature = "native")]
                {
                    let handle = *handle;
                    abi::typed("docs", |table| {
                        let this = entry!(table, handle, ElementDef, ElementDef).clone();
                        let answer = this.docs(text);
                        FromApi::from_api(answer, table)
                    })
                }
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
                #[cfg(not(feature = "native"))]
                {
                    let reply = abi::call(*handle, "modes", vec![ToArg::arg(&list)])?;
                    ElementDef::from_value(&reply)
                }
                #[cfg(feature = "native")]
                {
                    let handle = *handle;
                    abi::typed("modes", |table| {
                        let this = entry!(table, handle, ElementDef, ElementDef).clone();
                        let answer = this.modes(list);
                        FromApi::from_api(answer, table)
                    })
                }
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

/// The api's declaration: a helper's, the entry of its handle cloned; one
/// given as data, its syntax and its element converted; the host's
/// `element_def_from_table`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &ElementDef {
    type Api = api::lang::mint::ElementDef;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        let data = match &self.form {
            ElementDefForm::Lent(handle) => {
                return match table.entry(*handle)? {
                    Entry::ElementDef(def) => Ok(def.clone()),
                    other => Err(mismatch(HandleKind::ElementDef, other.kind())),
                };
            }
            ElementDefForm::Data(data) => data,
        };
        let syntax = data.syntax.into_api(table)?;
        let element = data.element.into_api(table)?;
        let mut def = api::lang::mint::ElementDef::new(data.owner.clone(), data.names.clone(), syntax, element).docs(data.docs.clone());
        if let Some(modes) = &data.modes {
            def = def.modes(modes.clone());
        }
        Ok(def)
    }
}

/// The api's declaration, lent as its entry and read as a helper's handle;
/// the host's reply of an `ElementDef`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::lang::mint::ElementDef> for ElementDef {
    fn from_api(api: api::lang::mint::ElementDef, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(ElementDef { form: ElementDefForm::Lent(table.lend(Entry::ElementDef(api))) })
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

/// The api's fallback, its element converted; the host's
/// `fallback_def_from_table`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &FallbackDef {
    type Api = api::lang::mint::FallbackDef;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        Ok(api::lang::mint::FallbackDef::new(self.modes.clone(), self.kind, self.element.into_api(table)?))
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

/// The api's scoped package, each element converted as [`ElementDef`]'s
/// conversion converts one; the host's `scoped_package_def_from_table`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &ScopedPackageDef {
    type Api = api::lang::mint::ScopedPackageDef;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        let elements = self.elements.iter().map(|element| element.into_api(table)).collect::<Result<_, _>>()?;
        Ok(api::lang::mint::ScopedPackageDef::new(self.name.clone(), elements, self.docs.clone()))
    }
}
