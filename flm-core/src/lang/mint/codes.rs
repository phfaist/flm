//! The syntax of an argument in settings given as data, and the only reader
//! of it: the structured syntax, a standard argument kind written as a map,
//! and the argument codes, the short texts that stand for the kinds.
//!
//! [Settings given as data](crate::core::settings) cannot hold a parser object,
//! so when they declare a macro or an environment they write the syntax of
//! each of its arguments as data, in one of two forms:
//!
//! - the **structured syntax**: a map whose key `syntax` holds the name of a
//!   standard argument kind ([`ArgStdKind`]) and whose other keys hold the
//!   kind's parameters, as in `{syntax: delimited-group, delimiters: ["(",
//!   ")"]}` for a group in parentheses;
//! - an **argument code**, a short text that stands for one kind with its
//!   parameters: `{` for a mandatory group, `[` for an optional one, `r()`
//!   for a group in parentheses.
//!
//! This module reads both forms, and no other part of the library does:
//! [`ArgData`] reads one argument in either form, [`ArgListData`] reads a
//! whole argument list written in any of the three shapes that settings may
//! use, and [`arg_def_from_code`] turns one code into an argument declaration
//! ([`ArgDef`]). Every feature whose settings declare macros or environments
//! reads its argument lists through these, so that one syntax means one form
//! of argument in every settings file; the `userdefs` feature is one of them
//! ([`deserialize_arg_list`] reads its `args` settings key).
//!
//! # The structured syntax
//!
//! The structured syntax is a standard kind written as data: its map is the
//! kind's serialized form, so the kinds of the settings and of the Rust
//! interface are one vocabulary. The key `syntax` holds the kind's name in
//! lowercase with hyphens (`mandatory`, `delimited-group`, `chars-group`),
//! and the kind's parameters stand beside it (`delimiters`, `markers`,
//! `terminator`). The [documentation of `ArgStdKind`](ArgStdKind#the-kind-as-data)
//! lists each kind's name and parameters, and what reading a kind refuses.
//!
//! Every kind can be written this way except a tack-on argument
//! ([`TackOn`](ArgStdKind::TackOn)), which a feature declares in Rust. Three
//! of the forms that the structured syntax writes have no code: a group read
//! as plain characters (`chars-group`), a group of `key=value` pairs
//! (`key-vals`), and a terminated argument with an opener or a terminator
//! head (`terminated` with `opener` or `head`).
//!
//! # The codes
//!
//! The codes are those of techy's table of argument codes
//! ([`argument_specs`](techy::latexlike::argument_specs)), plus four of FLM's
//! own for a terminated argument (`u`, `U`, `f` and `F`, described below the
//! table); techy is the parser toolkit FLM is built on. Each code is a
//! shorthand for one structured syntax, a standard kind with its parameters,
//! which the last column of the table gives. Every code of techy's table
//! means what it means there, except the code `v` alone: it selects FLM's own verbatim reader, the one
//! FLM's verbatim commands use, which closes `{` with `}`, `[` with `]`, `(`
//! with `)`, `<` with `>` and any other character with itself, whereas techy's
//! `v` closes on the same character that opened. Each example shows a use of a
//! macro `\m` declared with that code as its only argument.
//!
//! | code | the argument | example | structured syntax |
//! |---|---|---|---|
//! | `{` or `m` | a mandatory `{…}` group; a single token also counts | `\m{arg}`, `\m1` | [`mandatory`](ArgStdKind::Mandatory) |
//! | `[` or `o` | an optional `[…]` group | `\m[arg]` | [`optional`](ArgStdKind::Optional) |
//! | `*` or `s` | an optional `*` marker | `\m*` | [`star`](ArgStdKind::Star) |
//! | `t<c>` | an optional marker made of the one character `<c>` (`t*` is the same as `*`) | `t!` reads `\m!` | [`marker`](ArgStdKind::Marker) with `marker: <c>` |
//! | `r<c1><c2>` | a mandatory group between `<c1>` and `<c2>` | `r()` reads `\m(arg)` | [`delimited-group`](ArgStdKind::DelimitedGroup) with `delimiters: [<c1>, <c2>]` |
//! | `d<c1><c2>` | an optional group between `<c1>` and `<c2>` | `d<>` reads `\m<arg>` | [`optional-delimited-group`](ArgStdKind::OptionalDelimitedGroup) with `delimiters: [<c1>, <c2>]` |
//! | `v` | verbatim text between two delimiters, matched to the opening one | `\m+raw %text+` | [`verbatim`](ArgStdKind::Verbatim) |
//! | `v<c1><c2>` | verbatim text between `<c1>` and `<c2>` | `v+-` reads `\m+raw %text-` | [`verbatim`](ArgStdKind::Verbatim) with `delimiters: [<c1>, <c2>]` |
//! | `e{<chars>}` | embellishments: each character between the braces is a marker that is followed by one expression; the markers may be written in any order, each at most once | `e{^_}` reads `\m^{a}_{b}` | [`embellishments`](ArgStdKind::Embellishments) with `markers`, one entry per character of `<chars>` |
//! | `AnyDelimited` | a mandatory group in `{…}`, `[…]`, `(…)` or `<…>` | `\m<arg>` | [`any-delimited-group`](ArgStdKind::AnyDelimitedGroup) |
//! | `AnyDelimitedOptional` | the same pairs, optional | `\m(arg)` | [`optional-any-delimited-group`](ArgStdKind::OptionalAnyDelimitedGroup) |
//! | `BracedOnly` | a mandatory `{…}` group, never a single token | `\m{arg}` | [`braced-only`](ArgStdKind::BracedOnly) |
//! | `u<terminator>` | document content up to `<terminator>`, one or more characters | `u\endm` reads `\m some \emph{text}\endm` | [`terminated`](ArgStdKind::Terminated) with `terminator: <terminator>` |
//! | `U<terminator>` | raw text up to `<terminator>` | `U\endm` reads `\m raw %text\endm` | [`terminated`](ArgStdKind::Terminated) with `terminator: <terminator>`, `raw: true` |
//! | `f<c><terminator>` | document content after a run of the character `<c>`, up to the same run followed by `<terminator>`, which may be empty | `f+` reads `\m++some text++` | [`terminated`](ArgStdKind::Terminated) with `repeat_char: <c>`, `terminator: <terminator>` |
//! | `F<c><terminator>` | raw text after a run of the character `<c>`, up to the same run followed by `<terminator>` | `F+` reads `\m+++raw %text+++` | [`terminated`](ArgStdKind::Terminated) with `repeat_char: <c>`, `terminator: <terminator>`, `raw: true` |
//!
//! A code is written exactly, with no space around it. The characters a code
//! takes (`<c>`, `<c1><c2>`, `<chars>`, `<terminator>`) follow its letter
//! immediately, and none of them may be a space. An optional argument the use
//! site does not write is absent; a mandatory one is reported missing.
//!
//! The four codes `u`, `U`, `f` and `F` are FLM's own: techy's table has no
//! code for a terminated argument. In their content, the terminator is
//! matched before any other markup, wherever its characters appear; with `f`
//! and `F`, the run of `<c>` that opens the argument may hold any number of
//! copies, and the terminator repeats the same number of copies before
//! `<terminator>`. A code span written with backticks, `` `x` `` or
//! ``` ``a`b`` ```, is read by a special character sequence `` ` `` declared
//! with one argument of the code ``` F`` ```: the sequence reads the first
//! backtick, the argument reads the run of backticks that follows, and the
//! terminator is that run followed by one more backtick. The [terminated
//! kind](ArgStdKind::Terminated) also has an opener (text after the run) and a
//! terminator head (text before the repeated copies), which no code sets: a
//! declaration that needs them writes the structured syntax.
//!
//! # The three shapes of an argument list
//!
//! An argument list ([`ArgListData`]) is written in one of three shapes:
//!
//! - one text in which each character is one code (`"[{{"`: an optional group,
//!   then two mandatory ones); only the codes of one character can be written
//!   this way;
//! - a list of codes (`["[", "r()"]`);
//! - a list whose entries are each a code or an argument's map (below):
//!   `[{syntax: optional, name: size}, "{"]`.
//!
//! # An argument's map
//!
//! An argument's map ([`ArgDataFields`]) gives the argument's syntax in
//! exactly one of the two forms: a structured syntax (`syntax` and the
//! kind's parameters) or a code (the key `code`). A map that gives both, and
//! one that gives neither, is refused. Beside the syntax, the map may give
//! the argument's own keys, each optional: `name`, `mode` (`text` or
//! `math`), `layout`, `opaque_wrapper`, `scoped` (scoped packages), `docs`
//! and `role`. For example, `{syntax: delimited-group, delimiters: ["(",
//! ")"], name: size}` and `{code: "r()", name: size}` declare the same
//! argument; `{code: "{", mode: "math"}`, `{syntax: chars-group, name:
//! "key"}` and `{code: "{", name: "target", role: "flm.refs.target"}` are
//! maps too.
//
// Why an argument's syntax in data is its standard kind, with the parser
// toolkit's codes as shorthands, and why both have one reader here: see
// decision rationale at [§flm-dd-dr:lang-arg-codes].

use std::fmt;

use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use techy::latexlike::Mode;

use crate::lang::mint::decl::{ArgDef, ArgLayout, TackOnField};
use crate::lang::mint::parsers::ArgStdKind;
use crate::model::names::Name;

// --- an argument and a list of arguments, as data ------------------------------------------------

/// One argument of a macro or environment declared in settings given as data:
/// either its argument code alone (`"{"`), or an argument's map, which gives
/// its syntax (a structured syntax or a code) with the argument's name,
/// parsing mode, layout, opaque wrapper, scoped packages, documentation and
/// role.
///
/// It converts into an argument declaration ([`ArgDef`]) through `TryFrom`,
/// which reads a code with [`arg_def_from_code`]; the [module
/// documentation](self) describes both forms of a syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(schemars::JsonSchema)]
#[schemars(untagged)]
pub enum ArgData {
    /// The argument code alone: the argument has no name and its content is
    /// laid out inline.
    Code(#[schemars(schema_with = "arg_code_schema")] String),
    /// The argument's syntax with the argument's name, parsing mode, layout,
    /// opaque wrapper, scoped packages, documentation and role.
    Declared(ArgDataFields),
}

// An argument is a text or a map, and is read as the one or the other, so
// that a problem inside a map is reported with its own message.
impl<'de> Deserialize<'de> for ArgData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ArgData, D::Error> {
        struct Entry;

        impl<'de> Visitor<'de> for Entry {
            type Value = ArgData;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an argument code or a map")
            }

            fn visit_str<E: serde::de::Error>(self, code: &str) -> Result<ArgData, E> {
                Ok(ArgData::Code(code.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ArgData, A::Error> {
                ArgDataFields::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(ArgData::Declared)
            }
        }

        deserializer.deserialize_any(Entry)
    }
}

/// An argument written as a map in settings given as data: its syntax, given
/// as a code or as a structured syntax, exactly one of the two, and its name,
/// parsing mode, layout, opaque wrapper, scoped packages, documentation and
/// role (see [`ArgData`]).
//
// It is also written to data (`Serialize`), with every field present except
// the form of the syntax it does not give, and reads back as the same value:
// see the registry's "Serialized forms" (`crate::core::registry`). The
// structured syntax is flattened into the map: `deserialize_syntax` reads
// the keys that are not the map's own, and `arg_map_schema` gives the schema
// one alternative per form of the syntax. This is a plain comment, not
// documentation, because the documentation of this type is the description
// of a settings key in the settings' JSON Schema.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[schemars(transform = arg_map_schema)]
pub struct ArgDataFields {
    /// The argument code, which says what form the argument takes in one
    /// short text (the codes are those [`arg_def_from_code`] reads). A map
    /// gives either a code or a structured syntax.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "arg_code_schema")]
    pub code: Option<String>,
    /// The structured syntax: the standard kind ([`ArgStdKind`]) written as
    /// data, its name under the key `syntax` and its parameters beside it, in
    /// the map itself. A map gives either a code or a structured syntax.
    #[serde(flatten, deserialize_with = "deserialize_syntax")]
    #[schemars(schema_with = "syntax_schema")]
    pub syntax: Option<ArgStdKind>,
    /// The argument's name, by which a user definition's template refers to
    /// it. No name by default.
    #[serde(default)]
    pub name: Option<String>,
    /// How the argument's content is laid out: `inline` (running text with
    /// no paragraph, the default), `blocks` (paragraphs and other blocks), or
    /// `inline-or-blocks` (decided by the content); see [`ArgLayout`].
    #[serde(default)]
    pub layout: ArgLayout,
    /// Whether the argument's content is placed under an opaque wrapper, so
    /// that it is kept as written: no whitespace is collapsed and no
    /// typographic substitution is made inside it. `false` by default; see
    /// [`ArgDef::with_opaque_wrapper`](ArgDef::with_opaque_wrapper()).
    #[serde(default)]
    pub opaque_wrapper: bool,
    /// Documentation of the argument. Empty by default.
    #[serde(default)]
    pub docs: String,
    /// The parsing mode the argument is read in: `text` or `math`. Without
    /// it, the argument is read in the mode the command is written in; see
    /// [`ArgDef::mode`](ArgDef::mode()).
    #[serde(default)]
    #[schemars(schema_with = "mode_schema")]
    pub mode: Option<Mode>,
    /// The names of the [scoped packages](super::ScopedPackageDef) whose
    /// declarations are available inside the argument. None by default; see
    /// [`ArgDef::scoped`](ArgDef::scoped()).
    #[serde(default)]
    pub scoped: Vec<Name>,
    /// The argument's role: the registered name of what the argument holds,
    /// such as `flm.refs.target` for a label that a reference points to. No
    /// role by default; see [`ArgDef::role`](ArgDef::role()).
    #[serde(default)]
    pub role: Option<Name>,
}

/// Writes a declaration as an argument's map, as [`ArgDef::code`] states a
/// declaration is written back: with the code it was read from, when it has
/// one; otherwise with the structured syntax of its parser's standard kind
/// ([`FlmArgumentParser::std_kind`](super::FlmArgumentParser::std_kind)),
/// unless the kind is a tack-on argument, which settings cannot declare;
/// otherwise with neither, for a parser of a feature's own. A map written
/// with neither does not read back. The declaration's other fields are
/// copied.
impl From<&ArgDef> for ArgDataFields {
    fn from(arg: &ArgDef) -> ArgDataFields {
        let code = arg.code().map(str::to_string);
        let syntax = match (&code, arg.kind.std_kind()) {
            (None, Some(kind)) if !matches!(kind, ArgStdKind::TackOn { .. }) => Some(kind.clone()),
            _ => None,
        };
        ArgDataFields {
            code,
            syntax,
            name: arg.name.clone(),
            layout: arg.layout,
            opaque_wrapper: arg.opaque_wrapper,
            docs: arg.docs.clone(),
            mode: arg.mode,
            scoped: arg.scoped.clone(),
            role: arg.role.clone(),
        }
    }
}

impl TryFrom<ArgData> for ArgDef {
    type Error = ArgDataError;

    fn try_from(data: ArgData) -> Result<ArgDef, ArgDataError> {
        match data {
            ArgData::Code(code) => Ok(arg_def_from_code(&code)?),
            ArgData::Declared(fields) => {
                let arg = match (fields.code, fields.syntax) {
                    (Some(code), None) => arg_def_from_code(&code)?,
                    (None, Some(kind)) => ArgDef::new(kind),
                    (Some(_), Some(_)) => return Err(ArgDataError::BothCodeAndSyntax),
                    (None, None) => return Err(ArgDataError::NeitherCodeNorSyntax),
                };
                let mut arg = arg.layout(fields.layout).docs(fields.docs);
                arg.name = fields.name;
                arg.opaque_wrapper = fields.opaque_wrapper;
                arg.mode = fields.mode;
                arg.scoped = fields.scoped;
                arg.role = fields.role;
                Ok(arg)
            }
        }
    }
}

/// What is wrong with an argument written as data: the error of converting
/// an [`ArgData`] into an argument declaration ([`ArgDef`], through
/// `TryFrom`), which an [`ArgListError`] holds with the argument's position.
///
/// Its `Display` text states the problem for the person who wrote the
/// settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgDataError {
    /// The argument's code is not a complete argument code.
    Code(ArgCodeError),
    /// The argument's map gives both a code (`code`) and a structured syntax
    /// (`syntax`).
    BothCodeAndSyntax,
    /// The argument's map gives neither a code nor a structured syntax.
    NeitherCodeNorSyntax,
}

impl From<ArgCodeError> for ArgDataError {
    fn from(error: ArgCodeError) -> ArgDataError {
        ArgDataError::Code(error)
    }
}

impl fmt::Display for ArgDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgDataError::Code(error) => fmt::Display::fmt(error, f),
            ArgDataError::BothCodeAndSyntax => f.write_str(
                "an argument written as a map must hold exactly one of the keys `code` and `syntax`, \
                 and this one holds both",
            ),
            ArgDataError::NeitherCodeNorSyntax => f.write_str(
                "an argument written as a map must hold the key `code` or the key `syntax`, which \
                 gives its syntax",
            ),
        }
    }
}

impl std::error::Error for ArgDataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArgDataError::Code(error) => Some(error),
            _ => None,
        }
    }
}

/// What is wrong with an argument list written as data: the error of
/// [`ArgListData::arg_defs`], the [`ArgDataError`] of one of its arguments
/// with the argument's position in the list. A reader that converts the
/// entries of a list one by one gives its errors the same form.
///
/// Its `Display` text states the problem for the person who wrote the
/// settings: the argument's position counted from 1, then the
/// [`ArgDataError`]'s text (`argument 2: ` followed by that text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgListError {
    /// The argument's index in the list, from 0: the index of its entry in a
    /// list, or of its character in one text of codes.
    pub index: usize,
    /// What is wrong with the argument.
    pub error: ArgDataError,
}

impl fmt::Display for ArgListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "argument {}: {}", self.index + 1, self.error)
    }
}

impl std::error::Error for ArgListError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The argument list of a macro or environment declared in settings given as
/// data, written either as one text in which each character is one argument
/// code (`"[{"`), or as a list whose entries are each a code or an argument's
/// map ([`ArgData`]).
///
/// [`arg_defs`](ArgListData::arg_defs) converts it into argument declarations;
/// [`deserialize_arg_list`] reads a settings field of type `Vec<ArgDef>` in any
/// of the shapes. The [module documentation](self) describes the shapes and
/// the forms of an argument's syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(schemars::JsonSchema)]
#[schemars(untagged)]
pub enum ArgListData {
    /// One text in which each character is one argument code: `"[{{"`.
    Codes(#[schemars(schema_with = "arg_codes_text_schema")] String),
    /// A list of arguments, each an argument code or a map.
    List(Vec<ArgData>),
}

// A list is a text or a list, and is read as the one or the other, so that a
// problem in one of its entries is reported with its own message, at the
// entry's position.
impl<'de> Deserialize<'de> for ArgListData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ArgListData, D::Error> {
        struct List;

        impl<'de> Visitor<'de> for List {
            type Value = ArgListData;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(LIST_EXPECTED)
            }

            fn visit_str<E: serde::de::Error>(self, codes: &str) -> Result<ArgListData, E> {
                Ok(ArgListData::Codes(codes.to_string()))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<ArgListData, A::Error> {
                entries(seq).map(ArgListData::List)
            }
        }

        deserializer.deserialize_any(List)
    }
}

impl ArgListData {
    /// Converts the list into argument declarations, in the order the
    /// arguments are written.
    ///
    /// # Errors
    ///
    /// An [`ArgListError`] for the first entry that is not an argument code,
    /// that lacks the characters its code letter requires, or that is a map
    /// giving both a code and a structured syntax, or neither: the entry's
    /// [`ArgDataError`] with its position.
    pub fn arg_defs(self) -> Result<Vec<ArgDef>, ArgListError> {
        match self {
            ArgListData::Codes(codes) => codes
                .chars()
                .enumerate()
                .map(|(index, code)| {
                    arg_def_from_code(&String::from(code)).map_err(|error| ArgListError { index, error: error.into() })
                })
                .collect(),
            ArgListData::List(args) => args
                .into_iter()
                .enumerate()
                .map(|(index, arg)| ArgDef::try_from(arg).map_err(|error| ArgListError { index, error }))
                .collect(),
        }
    }
}

/// What an argument list is, for the message of a value of another form.
const LIST_EXPECTED: &str = "a text of argument codes or a list of arguments";

/// Reads every entry of a list.
fn entries<'de, A: SeqAccess<'de>, T: Deserialize<'de>>(mut seq: A) -> Result<Vec<T>, A::Error> {
    let mut entries = Vec::new();
    while let Some(entry) = seq.next_element()? {
        entries.push(entry);
    }
    Ok(entries)
}

/// Deserializes an argument list written in any of the shapes of
/// [`ArgListData`] into argument declarations.
///
/// Use it as the deserializer of a settings field of type `Vec<ArgDef>`, and
/// give the field [`ArgListData`]'s schema, as in the example. Each entry of
/// a list is converted as it is read, so the error of an entry is reported at
/// the entry's position in the list.
///
/// ```
/// use flm_core::lang::mint::{deserialize_arg_list, ArgDef, ArgListData};
///
/// #[derive(serde::Deserialize, schemars::JsonSchema)]
/// struct MyMacro {
///     #[serde(deserialize_with = "deserialize_arg_list")]
///     #[schemars(with = "ArgListData")]
///     args: Vec<ArgDef>,
/// }
///
/// let declared: MyMacro = serde_json::from_value(serde_json::json!({ "args": "[{" })).unwrap();
/// assert_eq!(declared.args.len(), 2);
/// let declared: MyMacro = serde_json::from_value(serde_json::json!({
///     "args": [{ "syntax": "delimited-group", "delimiters": ["(", ")"], "name": "size" }],
/// }))
/// .unwrap();
/// assert_eq!(declared.args[0].kind.description(), "a mandatory `(…)` group");
/// ```
///
/// # Errors
///
/// The deserializer's error when the data has none of the shapes of
/// [`ArgListData`], or when an entry is not an argument; the error then
/// carries the [`ArgListError`]'s message for a code of one text of codes,
/// the [`ArgDataError`]'s message for an entry of a list, or the message of
/// the structured syntax that cannot be read.
pub fn deserialize_arg_list<'de, D>(deserializer: D) -> Result<Vec<ArgDef>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Args;

    impl<'de> Visitor<'de> for Args {
        type Value = Vec<ArgDef>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(LIST_EXPECTED)
        }

        fn visit_str<E: serde::de::Error>(self, codes: &str) -> Result<Vec<ArgDef>, E> {
            ArgListData::Codes(codes.to_string()).arg_defs().map_err(E::custom)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Vec<ArgDef>, A::Error> {
            entries(seq)
        }
    }

    deserializer.deserialize_any(Args)
}

// --- the structured syntax -----------------------------------------------------------------------

/// The keys of a standard kind written as data: the form in which
/// [`ArgStdKind`] is serialized and deserialized (its `into` and `try_from`
/// type). Each variant holds the parameters as data writes them, a character
/// as a text and a pair of delimiters as a list; the conversion into the kind
/// checks them. A kind without parameters is a variant with no field, so
/// that a key written beside its name is refused. A parameter's key must
/// differ from the keys of an argument's map ([`ArgDataFields`]), which the
/// map reads itself before the kind reads the keys left. The documentation of
/// the variants and of their fields is the description of each kind in the
/// settings' JSON Schema.
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "syntax", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum ArgStdKindData {
    /// A mandatory `{…}` group; a single token also counts.
    Mandatory {},
    /// An optional `[…]` group.
    Optional {},
    /// An optional `*` marker.
    Star {},
    /// An optional marker made of one character.
    Marker {
        /// The marker's character.
        #[schemars(pattern(ONE_CHARACTER))]
        marker: String,
    },
    /// A mandatory group between a pair of characters.
    DelimitedGroup {
        /// The opening and the closing character.
        #[schemars(length(equal = 2), inner(pattern(ONE_CHARACTER)))]
        delimiters: Vec<String>,
    },
    /// An optional group between a pair of characters.
    OptionalDelimitedGroup {
        /// The opening and the closing character.
        #[schemars(length(equal = 2), inner(pattern(ONE_CHARACTER)))]
        delimiters: Vec<String>,
    },
    /// A mandatory group in `{…}`, `[…]`, `(…)` or `<…>`.
    AnyDelimitedGroup {},
    /// An optional group in `{…}`, `[…]`, `(…)` or `<…>`.
    OptionalAnyDelimitedGroup {},
    /// A mandatory `{…}` group, never a single token.
    BracedOnly {},
    /// Embellishments: each marker character is followed by one expression;
    /// the markers may be written in any order, each at most once.
    Embellishments {
        /// The marker characters, at least one.
        #[schemars(length(min = 1), inner(pattern(ONE_CHARACTER)))]
        markers: Vec<String>,
    },
    /// An optional `[key=value,…]` group.
    KeyVals {},
    /// A `{…}` group read as plain characters, kept as written.
    CharsGroup {},
    /// Verbatim text between a pair of delimiters.
    Verbatim {
        /// The opening and the closing delimiter. Without it, the closing
        /// delimiter matches whichever opening one is found: `}` for `{`, `]`
        /// for `[`, `)` for `(`, `>` for `<`, and any other character for
        /// itself.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(equal = 2), inner(pattern(ONE_CHARACTER)))]
        delimiters: Option<Vec<String>>,
    },
    /// The fields a command accepts after its other arguments. A feature
    /// declares them; settings cannot.
    #[serde(deserialize_with = "refuse_tack_on")]
    #[schemars(skip_deserializing)]
    TackOn {
        /// The fields.
        fields: Vec<TackOnFieldData>,
    },
    /// An argument that runs up to a terminator: an optional run of a
    /// repeated character and an opener, the content, then the terminator,
    /// which is the head, as many copies of the repeated character as the
    /// run had, and the rest of the terminator.
    Terminated {
        /// The end of the terminator, after the repeated characters.
        terminator: String,
        /// The character whose run starts the argument and is repeated in
        /// the terminator. None by default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(pattern(ONE_CHARACTER))]
        repeat_char: Option<String>,
        /// The text that follows the run of the repeated character. Empty by
        /// default.
        #[serde(default)]
        opener: String,
        /// The text that begins the terminator, before the repeated
        /// characters. Empty by default.
        #[serde(default)]
        head: String,
        /// Whether the content is raw text (`true`) or parsed document
        /// content (`false`, the default).
        #[serde(default)]
        raw: bool,
    },
}

/// A tack-on field written as data, in the structured syntax `tack-on`: the
/// field's name, its arguments, whether it may repeat, and its
/// documentation.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub(crate) struct TackOnFieldData {
    /// The field's name, without the backslash.
    name: String,
    /// The field's arguments, each written as an argument's map.
    args: Vec<ArgDataFields>,
    /// Whether the field may be written more than once.
    repeatable: bool,
    /// The field's documentation.
    docs: String,
}

/// The pattern of a character in a structured syntax: one character, not
/// whitespace.
const ONE_CHARACTER: &str = r"^\S$";

/// The message that refuses the structured syntax `tack-on`.
const TACK_ON_REFUSED: &str = "an argument of the syntax `tack-on` is declared by a feature, not in settings";

/// Refuses the structured syntax `tack-on` when it is read, whatever its
/// fields: they are declarations a feature makes in Rust, each with an
/// element of its own ([`TackOnField`]).
fn refuse_tack_on<'de, D: Deserializer<'de>>(_deserializer: D) -> Result<Vec<TackOnFieldData>, D::Error> {
    Err(D::Error::custom(TACK_ON_REFUSED))
}

impl From<ArgStdKind> for ArgStdKindData {
    fn from(kind: ArgStdKind) -> ArgStdKindData {
        let pair = |open: char, close: char| vec![String::from(open), String::from(close)];
        match kind {
            ArgStdKind::Mandatory => ArgStdKindData::Mandatory {},
            ArgStdKind::Optional => ArgStdKindData::Optional {},
            ArgStdKind::Star => ArgStdKindData::Star {},
            ArgStdKind::Marker { marker } => ArgStdKindData::Marker { marker: String::from(marker) },
            ArgStdKind::DelimitedGroup { open, close } => {
                ArgStdKindData::DelimitedGroup { delimiters: pair(open, close) }
            }
            ArgStdKind::OptionalDelimitedGroup { open, close } => {
                ArgStdKindData::OptionalDelimitedGroup { delimiters: pair(open, close) }
            }
            ArgStdKind::AnyDelimitedGroup => ArgStdKindData::AnyDelimitedGroup {},
            ArgStdKind::OptionalAnyDelimitedGroup => ArgStdKindData::OptionalAnyDelimitedGroup {},
            ArgStdKind::BracedOnly => ArgStdKindData::BracedOnly {},
            ArgStdKind::Embellishments { markers } => {
                ArgStdKindData::Embellishments { markers: markers.into_iter().map(String::from).collect() }
            }
            ArgStdKind::KeyVals => ArgStdKindData::KeyVals {},
            ArgStdKind::CharsGroup => ArgStdKindData::CharsGroup {},
            ArgStdKind::Verbatim { delimiters } => ArgStdKindData::Verbatim {
                delimiters: delimiters.map(|(open, close)| pair(open, close)),
            },
            ArgStdKind::TackOn { fields } => ArgStdKindData::TackOn {
                fields: fields
                    .iter()
                    .map(|field: &TackOnField| TackOnFieldData {
                        name: field.name.clone(),
                        args: field.args.iter().map(ArgDataFields::from).collect(),
                        repeatable: field.repeatable,
                        docs: field.docs.clone(),
                    })
                    .collect(),
            },
            ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw } => ArgStdKindData::Terminated {
                terminator,
                repeat_char: repeat_char.map(String::from),
                opener,
                head,
                raw,
            },
        }
    }
}

impl TryFrom<ArgStdKindData> for ArgStdKind {
    type Error = String;

    fn try_from(data: ArgStdKindData) -> Result<ArgStdKind, String> {
        Ok(match data {
            ArgStdKindData::Mandatory {} => ArgStdKind::Mandatory,
            ArgStdKindData::Optional {} => ArgStdKind::Optional,
            ArgStdKindData::Star {} => ArgStdKind::Star,
            // As with the code `t*`, the `*` marker is the star kind, so that
            // one form has one kind.
            ArgStdKindData::Marker { marker } => match character("`marker`", &marker)? {
                '*' => ArgStdKind::Star,
                marker => ArgStdKind::Marker { marker },
            },
            ArgStdKindData::DelimitedGroup { delimiters } => {
                let (open, close) = delimiter_pair(&delimiters)?;
                ArgStdKind::DelimitedGroup { open, close }
            }
            ArgStdKindData::OptionalDelimitedGroup { delimiters } => {
                let (open, close) = delimiter_pair(&delimiters)?;
                ArgStdKind::OptionalDelimitedGroup { open, close }
            }
            ArgStdKindData::AnyDelimitedGroup {} => ArgStdKind::AnyDelimitedGroup,
            ArgStdKindData::OptionalAnyDelimitedGroup {} => ArgStdKind::OptionalAnyDelimitedGroup,
            ArgStdKindData::BracedOnly {} => ArgStdKind::BracedOnly,
            ArgStdKindData::Embellishments { markers } => {
                if markers.is_empty() {
                    return Err(String::from("`markers` must list at least one marker character"));
                }
                let markers = markers
                    .iter()
                    .map(|marker| character("each of the `markers`", marker))
                    .collect::<Result<Vec<char>, String>>()?;
                ArgStdKind::Embellishments { markers }
            }
            ArgStdKindData::KeyVals {} => ArgStdKind::KeyVals,
            ArgStdKindData::CharsGroup {} => ArgStdKind::CharsGroup,
            ArgStdKindData::Verbatim { delimiters } => ArgStdKind::Verbatim {
                delimiters: delimiters.as_deref().map(delimiter_pair).transpose()?,
            },
            ArgStdKindData::TackOn { .. } => return Err(String::from(TACK_ON_REFUSED)),
            ArgStdKindData::Terminated { terminator, repeat_char, opener, head, raw } => {
                let repeat_char = repeat_char.as_deref().map(|text| character("`repeat_char`", text)).transpose()?;
                match repeat_char {
                    None if terminator.is_empty() && head.is_empty() => {
                        return Err(String::from(
                            "a `terminated` argument must give a non-empty `terminator` or `head`, or a \
                             `repeat_char`, so that its terminator is never empty",
                        ));
                    }
                    Some(repeat_char) if opener.starts_with(repeat_char) => {
                        return Err(format!(
                            "the `opener` of a `terminated` argument must not begin with its \
                             `repeat_char` `{repeat_char}`"
                        ));
                    }
                    _ => {}
                }
                ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw }
            }
        })
    }
}

/// Reads a character of a structured syntax, written as `text`: one
/// character, not whitespace. `what` names the parameter in the message.
fn character(what: &str, text: &str) -> Result<char, String> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() => Ok(c),
        _ => Err(format!("{what} must be one character other than whitespace, not {text:?}")),
    }
}

/// Reads the `delimiters` of a structured syntax: an opening and a closing
/// character.
fn delimiter_pair(delimiters: &[String]) -> Result<(char, char), String> {
    let [open, close] = delimiters else {
        return Err(format!(
            "`delimiters` must list two characters, the opening one and the closing one, and it lists {}",
            delimiters.len()
        ));
    };
    Ok((character("each of the `delimiters`", open)?, character("each of the `delimiters`", close)?))
}

/// Reads the structured syntax of an argument's map ([`ArgDataFields`]): the
/// keys of the map that are not the map's own, which the map's other fields
/// read. With the key `syntax`, they are read as a standard kind; without it,
/// they must be none. When the kind cannot be read, the message states the
/// condition from the forms of the map's schema where they show it
/// ([`syntax_problem`]): a name that is no kind's, a key that is neither one
/// of the map's own nor a parameter of the kind, and a parameter's value of
/// another type.
fn deserialize_syntax<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<ArgStdKind>, D::Error> {
    let keys = serde_json::Map::<String, Value>::deserialize(deserializer)?;
    let Some(syntax) = keys.get("syntax") else {
        return match keys.keys().next() {
            Some(key) => Err(D::Error::custom(unknown_key(key, None, map_form(&map_forms(), None)))),
            None => Ok(None),
        };
    };
    ArgStdKind::deserialize(Value::Object(keys.clone())).map(Some).map_err(|error| {
        // The kind's own message for a name that is no kind's also lists
        // `tack-on`; for a key it does not take, it lists the kind's
        // parameters alone, not the keys of the map around them; for a
        // parameter's value of another type, it does not name the parameter.
        D::Error::custom(syntax_problem(syntax, &keys).unwrap_or_else(|| error.to_string()))
    })
}

/// States what keeps the structured syntax of an argument's map from being
/// read, when the forms of the map's schema show it ([`map_forms`]):
/// `syntax`, the value of the key `syntax`, that names no kind settings may
/// write ([`unknown_syntax`]); a key of `keys`, the map's keys that are not
/// its own, that is neither `syntax` nor a parameter of the kind
/// ([`unknown_key`]); or a parameter whose value has a type the kind does not
/// take ([`mistyped`]). `None` for any other condition, which the kind's own
/// message states.
fn syntax_problem(syntax: &Value, keys: &serde_json::Map<String, Value>) -> Option<String> {
    let forms = map_forms();
    let name = syntax.as_str();
    let Some(form) = name.and_then(|name| map_form(&forms, Some(name))) else {
        // The kind `tack-on` is refused by a message of its own.
        return (name != Some("tack-on")).then(|| unknown_syntax(syntax, &forms));
    };
    if let Some(key) = keys.keys().find(|key| !form.contains_key(*key)) {
        return Some(unknown_key(key, name, Some(form)));
    }
    keys.iter().find_map(|(key, value)| mistyped(key, form.get(key)?, value))
}

/// The message that refuses `syntax`, the value of the key `syntax` of an
/// argument's map, which names no kind that settings may write. It lists the
/// names of the kinds of `forms` ([`map_forms`]).
fn unknown_syntax(syntax: &Value, forms: &[MapForm]) -> String {
    let names: Vec<String> = forms.iter().filter_map(form_syntax).map(|name| format!("`{name}`")).collect();
    format!("{syntax} is not a syntax of an argument; the syntaxes are {}", names.join(", "))
}

/// The message that refuses the key `key` of an argument's map, a key that is
/// neither one of the map's own nor a parameter of the kind named `syntax`
/// (`None` for a map without the key `syntax`). It lists the keys of `form`,
/// the form of the map ([`map_form`]).
fn unknown_key(key: &str, syntax: Option<&str>, form: Option<&MapForm>) -> String {
    let listed = |code: bool| {
        let known = form.into_iter().flat_map(MapForm::keys).filter(|known| code || known.as_str() != "code");
        known.map(|known| format!("`{known}`")).collect::<Vec<_>>().join(", ")
    };
    match (syntax, form) {
        (_, None) => format!("`{key}` is not a key of an argument's map"),
        (Some(name), Some(_)) => format!(
            "`{key}` is not a key of an argument's map with the syntax `{name}`; its keys are {}",
            listed(true)
        ),
        (None, Some(_)) => format!(
            "`{key}` is not a key of an argument's map; its keys are {}, and `code` or `syntax` with the \
             syntax's parameters",
            listed(false)
        ),
    }
}

/// The message that refuses `value`, the value of the parameter `key` of a
/// structured syntax, when the parameter's schema `schema` does not admit its
/// JSON type, or, for a list, the JSON type of one of its entries; `None`
/// when it admits them, or when its type is not one a parameter takes
/// ([`expected`]).
fn mistyped(key: &str, schema: &Value, value: &Value) -> Option<String> {
    if !admits(schema, value) {
        return Some(format!("`{key}` must be {}, not {value}", expected(schema)?));
    }
    let (Some(items), Value::Array(entries)) = (schema.get("items"), value) else {
        return None;
    };
    let entry = entries.iter().find(|entry| !admits(items, entry))?;
    Some(format!("each of the `{key}` must be {}, not {entry}", expected(items)?))
}

/// Whether the JSON schema `schema` admits the JSON type of `value`: whether
/// its key `type`, one name or a list of names, names that type. A schema
/// without the key admits every type.
fn admits(schema: &Value, value: &Value) -> bool {
    let name = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    match schema.get("type") {
        Some(Value::Array(names)) => names.iter().any(|admitted| admitted == name),
        Some(admitted) => admitted == name,
        None => true,
    }
}

/// What a value that the schema `schema` of a parameter of a structured
/// syntax admits is, in a message: one character (a text of the pattern
/// [`ONE_CHARACTER`]), a text, a list, or `true` or `false`. `None` for a
/// schema of another type.
fn expected(schema: &Value) -> Option<&'static str> {
    let name = match schema.get("type")? {
        Value::Array(names) => names.iter().filter_map(Value::as_str).find(|name| *name != "null")?,
        name => name.as_str()?,
    };
    let one_character = schema.get("pattern").and_then(Value::as_str) == Some(ONE_CHARACTER);
    match name {
        "string" if one_character => Some("one character other than whitespace"),
        "string" => Some("a text"),
        "array" => Some("a list"),
        "boolean" => Some("`true` or `false`"),
        _ => None,
    }
}

/// One form of an argument's map in the map's JSON schema: the keys the map
/// may hold, each with the schema of its value.
type MapForm = serde_json::Map<String, Value>;

/// The forms of an argument's map ([`ArgDataFields`]), read from the map's
/// JSON schema, whose alternatives list each form's keys
/// ([`arg_map_schema`]): the form with the key `code`, then one per kind that
/// settings may write, with the key `syntax`, the kind's parameters, and the
/// map's own keys. The messages that refuse a map are read from them, so that
/// the messages and the schema name the same kinds, keys and types.
fn map_forms() -> Vec<MapForm> {
    let schema = schemars::SchemaGenerator::default().into_root_schema_for::<ArgDataFields>();
    let alternatives = schema.get("oneOf").and_then(Value::as_array).into_iter().flatten();
    alternatives.filter_map(|alternative| alternative.get("properties").and_then(Value::as_object)).cloned().collect()
}

/// The form among `forms` ([`map_forms`]) of an argument's map with the
/// structured syntax named `syntax`, or with a code for `None`. A name that
/// no kind settings may write has none.
fn map_form<'a>(forms: &'a [MapForm], syntax: Option<&str>) -> Option<&'a MapForm> {
    forms.iter().find(|form| form_syntax(form) == syntax)
}

/// The name of the kind of the form `form` of an argument's map
/// ([`map_forms`]), which its key `syntax` holds as its `const`; `None` for
/// the form with a code.
fn form_syntax(form: &MapForm) -> Option<&str> {
    form.get("syntax")?.get("const")?.as_str()
}

// --- the reader ----------------------------------------------------------------------------------

/// What is wrong with a text read as an argument code; part of
/// [`ArgCodeError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgCodeProblem {
    /// The text is not an argument code.
    Unknown,
    /// The text begins with a code letter that requires characters after it
    /// (`t`, `r`, `d`, `v`, `e`, `u`, `U`, `f`, `F`), and they are missing or
    /// include a space.
    Incomplete,
}

/// The error of reading a text that is not a complete argument code, returned
/// by [`arg_def_from_code`] and held by an [`ArgDataError`].
///
/// Its `Display` text states the problem for the person who wrote the
/// settings: for an unknown code it lists every code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgCodeError {
    /// The code as written.
    pub code: String,
    /// What is wrong with it.
    pub problem: ArgCodeProblem,
}

impl fmt::Display for ArgCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.problem {
            ArgCodeProblem::Unknown => {
                let codes: Vec<String> = FIXED_CODES
                    .iter()
                    .map(|(code, _)| *code)
                    .chain(CODE_FORMS.iter().map(|form| form.written))
                    .map(|code| format!("`{code}`"))
                    .collect();
                write!(f, "`{}` is not an argument code; the codes are {}", self.code, codes.join(", "))
            }
            ArgCodeProblem::Incomplete => {
                let letter = self.code.chars().next();
                match CODE_FORMS.iter().find(|form| Some(form.letter) == letter) {
                    Some(form) => write!(
                        f,
                        "in the argument code `{}`, `{}` must be followed by {}, as in `{}`",
                        self.code, form.letter, form.takes, form.example
                    ),
                    None => write!(f, "`{}` is not a complete argument code", self.code),
                }
            }
        }
    }
}

impl std::error::Error for ArgCodeError {}

/// Converts one argument code into an argument declaration ([`ArgDef`]) with
/// no name, inline layout and no scoped package, which keeps the code it was
/// read from ([`ArgDef::code`]).
///
/// This is the only reader of the codes, which are listed in the [module
/// documentation](self).
///
/// # Errors
///
/// An [`ArgCodeError`] for a text that is no code ([`ArgCodeProblem::Unknown`])
/// and for a code lacking the characters it takes
/// ([`ArgCodeProblem::Incomplete`]: `r(`, `t`, `e{}`).
///
/// # Examples
///
/// ```
/// use flm_core::lang::mint::{arg_def_from_code, ArgCodeProblem};
///
/// let arg = arg_def_from_code("r()").unwrap();
/// assert!(arg.name.is_none());
/// assert_eq!(arg.code(), Some("r()"));
/// assert_eq!(arg.kind.description(), "a mandatory `(…)` group");
/// assert_eq!(arg_def_from_code("q").unwrap_err().problem, ArgCodeProblem::Unknown);
/// assert_eq!(arg_def_from_code("r(").unwrap_err().problem, ArgCodeProblem::Incomplete);
/// ```
pub fn arg_def_from_code(code: &str) -> Result<ArgDef, ArgCodeError> {
    let mut arg = ArgDef::new(arg_kind_from_code(code)?);
    arg.code = Some(code.to_string());
    Ok(arg)
}

/// The standard kind an argument code corresponds to.
fn arg_kind_from_code(code: &str) -> Result<ArgStdKind, ArgCodeError> {
    if let Some((_, kind)) = FIXED_CODES.iter().find(|(known, _)| *known == code) {
        return Ok(kind.clone());
    }
    let error = |problem| ArgCodeError { code: code.to_string(), problem };
    let mut chars = code.chars();
    let Some(letter) = chars.next() else {
        return Err(error(ArgCodeProblem::Unknown));
    };
    let Some(form) = CODE_FORMS.iter().find(|form| form.letter == letter) else {
        return Err(error(ArgCodeProblem::Unknown));
    };
    let rest: Vec<char> = chars.collect();
    (form.read)(&rest).map_err(error)
}

/// The codes written as one fixed text, each with its standard kind.
const FIXED_CODES: [(&str, ArgStdKind); 10] = [
    ("{", ArgStdKind::Mandatory),
    ("m", ArgStdKind::Mandatory),
    ("[", ArgStdKind::Optional),
    ("o", ArgStdKind::Optional),
    ("*", ArgStdKind::Star),
    ("s", ArgStdKind::Star),
    ("v", ArgStdKind::Verbatim { delimiters: None }),
    ("AnyDelimited", ArgStdKind::AnyDelimitedGroup),
    ("AnyDelimitedOptional", ArgStdKind::OptionalAnyDelimitedGroup),
    ("BracedOnly", ArgStdKind::BracedOnly),
];

/// A code made of a letter and the characters it takes: how it is written,
/// matched and read.
struct CodeForm {
    /// The code's letter.
    letter: char,
    /// The code as the documentation and the messages show it (`r<c1><c2>`).
    written: &'static str,
    /// What the letter must be followed by, for the message of an incomplete
    /// code.
    takes: &'static str,
    /// A complete code of the form.
    example: &'static str,
    /// The regular expression a complete code of the form matches, for the
    /// schema of the settings.
    pattern: &'static str,
    /// What the argument is, for the schema of the settings.
    description: &'static str,
    /// Reads the characters after the letter.
    read: fn(&[char]) -> Result<ArgStdKind, ArgCodeProblem>,
}

/// The codes made of a letter and the characters it takes. The letter `v`
/// alone is the fixed code `v` ([`FIXED_CODES`]).
const CODE_FORMS: [CodeForm; 9] = [
    CodeForm {
        letter: 't',
        written: "t<c>",
        takes: "one marker character",
        example: "t!",
        pattern: r"^t\S$",
        description: "an optional marker made of the one character after `t`",
        read: |rest| {
            let [marker] = parameters::<1>(rest)?;
            Ok(if marker == '*' { ArgStdKind::Star } else { ArgStdKind::Marker { marker } })
        },
    },
    CodeForm {
        letter: 'r',
        written: "r<c1><c2>",
        takes: "an opening and a closing character",
        example: "r()",
        pattern: r"^r\S\S$",
        description: "a mandatory group between the two characters after `r`",
        read: |rest| {
            let [open, close] = parameters::<2>(rest)?;
            Ok(ArgStdKind::DelimitedGroup { open, close })
        },
    },
    CodeForm {
        letter: 'd',
        written: "d<c1><c2>",
        takes: "an opening and a closing character",
        example: "d<>",
        pattern: r"^d\S\S$",
        description: "an optional group between the two characters after `d`",
        read: |rest| {
            let [open, close] = parameters::<2>(rest)?;
            Ok(ArgStdKind::OptionalDelimitedGroup { open, close })
        },
    },
    CodeForm {
        letter: 'v',
        written: "v<c1><c2>",
        takes: "an opening and a closing character",
        example: "v||",
        pattern: r"^v\S\S$",
        description: "verbatim text between the two characters after `v`",
        read: |rest| {
            let [open, close] = parameters::<2>(rest)?;
            Ok(ArgStdKind::Verbatim { delimiters: Some((open, close)) })
        },
    },
    CodeForm {
        letter: 'e',
        written: "e{<chars>}",
        takes: "`{`, one or more marker characters and `}`",
        example: "e{_^}",
        pattern: r"^e\{[^\s}]+\}$",
        description: "embellishments: each character between the braces is a marker that is \
                      followed by one expression; the markers may be written in any order, \
                      each at most once",
        read: |rest| {
            let Some(('{', inner)) = rest.split_first().map(|(first, inner)| (*first, inner)) else {
                return Err(ArgCodeProblem::Incomplete);
            };
            let Some(end) = inner.iter().position(|c| *c == '}') else {
                return Err(ArgCodeProblem::Incomplete);
            };
            let markers = &inner[..end];
            if markers.is_empty() || markers.iter().any(|c| c.is_whitespace()) {
                return Err(ArgCodeProblem::Incomplete);
            }
            if end + 1 != inner.len() {
                return Err(ArgCodeProblem::Unknown);
            }
            Ok(ArgStdKind::Embellishments { markers: markers.to_vec() })
        },
    },
    CodeForm {
        letter: 'u',
        written: "u<terminator>",
        takes: "the terminator, one or more characters",
        example: "u\\endm",
        pattern: r"^u\S+$",
        description: "document content up to the characters after `u`, its terminator",
        read: |rest| terminated(rest, false, false),
    },
    CodeForm {
        letter: 'U',
        written: "U<terminator>",
        takes: "the terminator, one or more characters",
        example: "U\\endm",
        pattern: r"^U\S+$",
        description: "raw text up to the characters after `U`, its terminator",
        read: |rest| terminated(rest, false, true),
    },
    CodeForm {
        letter: 'f',
        written: "f<c><terminator>",
        takes: "a repeated character, then the rest of the terminator, if any",
        example: "f+",
        pattern: r"^f\S+$",
        description: "document content after a run of the character after `f`, up to the same run \
                      followed by the characters after that character",
        read: |rest| terminated(rest, true, false),
    },
    CodeForm {
        letter: 'F',
        written: "F<c><terminator>",
        takes: "a repeated character, then the rest of the terminator, if any",
        example: "F+",
        pattern: r"^F\S+$",
        description: "raw text after a run of the character after `F`, up to the same run \
                      followed by the characters after that character",
        read: |rest| terminated(rest, true, true),
    },
];

/// Reads the characters after the letter of a terminated code (`u`, `U`,
/// `f`, `F`): with `repeated`, the repeat character and then the rest of the
/// terminator, which may be empty; without it, the whole terminator, at
/// least one character. A space among them makes the code incomplete.
fn terminated(rest: &[char], repeated: bool, raw: bool) -> Result<ArgStdKind, ArgCodeProblem> {
    if rest.is_empty() || rest.iter().any(|c| c.is_whitespace()) {
        return Err(ArgCodeProblem::Incomplete);
    }
    let (repeat_char, terminator) = if repeated { (Some(rest[0]), &rest[1..]) } else { (None, rest) };
    Ok(ArgStdKind::Terminated {
        terminator: terminator.iter().collect(),
        repeat_char,
        opener: String::new(),
        head: String::new(),
        raw,
    })
}

/// The `N` characters a code takes after its letter: fewer, or one of them a
/// space, is an incomplete code; more is no code.
fn parameters<const N: usize>(rest: &[char]) -> Result<[char; N], ArgCodeProblem> {
    if rest.len() > N {
        return Err(ArgCodeProblem::Unknown);
    }
    match <[char; N]>::try_from(rest) {
        Ok(chars) if !chars.iter().any(|c| c.is_whitespace()) => Ok(chars),
        _ => Err(ArgCodeProblem::Incomplete),
    }
}

// --- the schemas ---------------------------------------------------------------------------------

/// The JSON schema of an argument code in settings given as data: one of the
/// fixed codes, each described by the phrase its parser returns
/// ([`FlmArgumentParser::description`](super::FlmArgumentParser::description)),
/// or a code of one of the forms that take characters, each matched by its
/// pattern.
fn arg_code_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let fixed = FIXED_CODES.iter().map(|(code, kind)| {
        let parser = ArgDef::new(kind.clone()).kind;
        serde_json::json!({ "const": code, "description": parser.description() })
    });
    let forms = CODE_FORMS.iter().map(|form| {
        serde_json::json!({
            "type": "string",
            "pattern": form.pattern,
            "description": format!("`{}`: {}", form.written, form.description),
        })
    });
    let codes: Vec<serde_json::Value> = fixed.chain(forms).collect();
    schemars::json_schema!({
        "description": "The argument code, which says what form the argument takes.",
        "anyOf": codes,
    })
}

/// The JSON schema of an argument's parsing mode in settings given as data:
/// `text` or `math` (techy's names of the two modes), or nothing.
fn mode_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "description": "The parsing mode the argument is read in: `text` or `math`. Without it, \
                        the argument is read in the mode the command is written in.",
        "enum": ["text", "math", null],
    })
}

/// The JSON schema of an argument list written as one text: each character is
/// one of the fixed codes of one character.
fn arg_codes_text_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let singles: String = FIXED_CODES
        .iter()
        .map(|(code, _)| *code)
        .filter(|code| code.chars().count() == 1)
        .map(|code| match code {
            "[" | "]" | "\\" | "^" | "-" => format!("\\{code}"),
            _ => code.to_string(),
        })
        .collect();
    schemars::json_schema!({
        "type": "string",
        "pattern": format!("^[{singles}]*$"),
        "description": "The arguments written as one text, in which each character is one \
                        argument code.",
    })
}

/// The JSON schema of the structured syntax that an argument's map holds
/// beside its own keys: one alternative per standard kind that settings may
/// write, each the kind's name under `syntax` with the kind's parameters, as
/// [`ArgStdKind`]'s schema lists them. [`arg_map_schema`] completes each
/// alternative into a whole map.
fn syntax_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let kinds = <ArgStdKind as schemars::JsonSchema>::json_schema(generator);
    let alternatives = kinds.get("oneOf").cloned().unwrap_or_else(|| Value::Array(Vec::new()));
    schemars::json_schema!({ "oneOf": alternatives })
}

/// Completes the JSON schema that `schemars` derives for an argument's map
/// ([`ArgDataFields`]), which lists the map's own keys under `properties`,
/// `code` among them, and the kinds of the structured syntax under `oneOf`
/// ([`syntax_schema`]).
///
/// The completed schema has one alternative per form of the syntax: the code
/// with the map's other keys, then each kind's name and parameters with the
/// same keys. Each alternative is a whole map that admits no other key, so a
/// map that gives both a code and a structured syntax, or neither, matches
/// none. A schema of another shape is left as it is.
fn arg_map_schema(schema: &mut schemars::Schema) {
    let Some(object) = schema.as_object_mut() else { return };
    let (Some(Value::Array(kinds)), Some(Value::Object(properties))) = (object.get("oneOf"), object.get("properties"))
    else {
        return;
    };
    let mut own = properties.clone();
    let Some(code) = own.shift_remove("code") else { return };
    let whole = |mut keys: serde_json::Map<String, Value>, required: Value, description: Option<&Value>| {
        keys.extend(own.clone());
        let mut map = serde_json::Map::new();
        map.insert(String::from("type"), Value::from("object"));
        if let Some(description) = description {
            map.insert(String::from("description"), description.clone());
        }
        map.insert(String::from("properties"), Value::Object(keys));
        map.insert(String::from("required"), required);
        map.insert(String::from("additionalProperties"), Value::Bool(false));
        Value::Object(map)
    };
    let mut alternatives = vec![whole(
        serde_json::Map::from_iter([(String::from("code"), code)]),
        serde_json::json!(["code"]),
        None,
    )];
    for kind in kinds {
        let parameters = kind.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
        // The kind's name first, then its parameters.
        let mut keys = serde_json::Map::new();
        if let Some(name) = parameters.get("syntax") {
            keys.insert(String::from("syntax"), name.clone());
        }
        keys.extend(parameters);
        let required = kind.get("required").cloned().unwrap_or_else(|| serde_json::json!(["syntax"]));
        alternatives.push(whole(keys, required, kind.get("description")));
    }
    for key in ["oneOf", "properties", "type", "required", "additionalProperties"] {
        object.shift_remove(key);
    }
    object.insert(String::from("oneOf"), Value::Array(alternatives));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(code: &str) -> Result<String, ArgCodeProblem> {
        arg_kind_from_code(code).map(|kind| format!("{kind:?}")).map_err(|error| error.problem)
    }

    #[test]
    fn each_form_reads_its_characters() {
        assert_eq!(kind("t!").unwrap(), "Marker { marker: '!' }");
        assert_eq!(kind("t*").unwrap(), "Star");
        assert_eq!(kind("r()").unwrap(), "DelimitedGroup { open: '(', close: ')' }");
        assert_eq!(kind("d<>").unwrap(), "OptionalDelimitedGroup { open: '<', close: '>' }");
        assert_eq!(kind("v").unwrap(), "Verbatim { delimiters: None }");
        assert_eq!(kind("v||").unwrap(), "Verbatim { delimiters: Some(('|', '|')) }");
        assert_eq!(kind("e{_^}").unwrap(), "Embellishments { markers: ['_', '^'] }");
        assert_eq!(kind("e{`}").unwrap(), "Embellishments { markers: ['`'] }");
        assert_eq!(
            kind("u\\endphf").unwrap(),
            r#"Terminated { terminator: "\\endphf", repeat_char: None, opener: "", head: "", raw: false }"#
        );
        assert_eq!(
            kind("U!!").unwrap(),
            r#"Terminated { terminator: "!!", repeat_char: None, opener: "", head: "", raw: true }"#
        );
        assert_eq!(
            kind("f+").unwrap(),
            r#"Terminated { terminator: "", repeat_char: Some('+'), opener: "", head: "", raw: false }"#
        );
        assert_eq!(
            kind("F``").unwrap(),
            r#"Terminated { terminator: "`", repeat_char: Some('`'), opener: "", head: "", raw: true }"#
        );
    }

    #[test]
    fn a_terminated_code_round_trips_through_its_declaration() {
        for code in ["u\\endphf", "U\\endverb", "f+", "F``", "F=]]"] {
            let arg = arg_def_from_code(code).unwrap();
            assert_eq!(arg.code(), Some(code));
            let again = arg_def_from_code(arg.code().unwrap()).unwrap();
            assert_eq!(again.kind.identity(), arg.kind.identity(), "{code}");
        }
        assert_ne!(
            arg_def_from_code("u\\endphf").unwrap().kind.identity(),
            arg_def_from_code("U\\endphf").unwrap().kind.identity()
        );
        assert_ne!(
            arg_def_from_code("f+").unwrap().kind.identity(),
            arg_def_from_code("u+").unwrap().kind.identity()
        );
    }

    #[test]
    fn a_code_lacking_characters_is_incomplete_and_one_with_more_is_unknown() {
        for code in [
            "t", "t ", "r", "r(", "r( ", "d<", "v|", "e", "e{", "e{}", "e{_", "e^", "e{ }", "u", "U", "f", "F",
            "u ", "ua b", "F` ",
        ] {
            assert_eq!(kind(code), Err(ArgCodeProblem::Incomplete), "{code:?}");
        }
        for code in ["", "q", " m", "m ", "mo", "t!!", "r()x", "e{^}x", "anydelimited"] {
            assert_eq!(kind(code), Err(ArgCodeProblem::Unknown), "{code:?}");
        }
    }

    #[test]
    fn the_messages_state_the_condition() {
        let unknown = arg_def_from_code("q").unwrap_err().to_string();
        assert!(unknown.starts_with("`q` is not an argument code; the codes are `{`, `m`"), "{unknown}");
        assert!(unknown.contains("`e{<chars>}`"), "{unknown}");
        let incomplete = arg_def_from_code("r(").unwrap_err().to_string();
        assert_eq!(
            incomplete,
            "in the argument code `r(`, `r` must be followed by an opening and a closing character, as in `r()`"
        );
        assert!(unknown.contains("`u<terminator>`, `U<terminator>`, `f<c><terminator>`, `F<c><terminator>`"), "{unknown}");
        assert_eq!(
            arg_def_from_code("u").unwrap_err().to_string(),
            "in the argument code `u`, `u` must be followed by the terminator, one or more characters, as in `u\\endm`"
        );
    }

    #[test]
    fn each_form_s_example_reads_and_matches_its_pattern() {
        for form in &CODE_FORMS {
            assert!(arg_kind_from_code(form.example).is_ok(), "{}", form.example);
            assert!(form.pattern.starts_with(&format!("^{}", form.letter)));
        }
    }

    #[test]
    fn the_data_form_reads_the_role_and_leaves_it_absent_by_default() {
        let list: ArgListData = serde_json::from_value(serde_json::json!([
            { "code": "{", "name": "target", "role": "flm.refs.target" },
            { "code": "[" },
            "m",
        ]))
        .unwrap();
        let args = list.arg_defs().unwrap();
        assert_eq!(args[0].role.as_ref().map(Name::as_str), Some("flm.refs.target"));
        assert_eq!(args[0].name.as_deref(), Some("target"));
        assert!(args[1].role.is_none());
        assert!(args[2].role.is_none());
    }

    #[test]
    fn a_structured_syntax_declares_what_its_code_does_and_a_map_gives_exactly_one() {
        let arg = |data| ArgDef::try_from(serde_json::from_value::<ArgData>(data).unwrap());
        let structured = arg(serde_json::json!({
            "syntax": "delimited-group", "delimiters": ["(", ")"], "name": "size",
        }))
        .unwrap();
        let mut coded = arg(serde_json::json!({ "code": "r()", "name": "size" })).unwrap();
        // The same declaration, but for the code a declaration read from one
        // keeps.
        assert_eq!(coded.code(), Some("r()"));
        coded.code = None;
        assert_eq!(structured, coded);
        let both = serde_json::json!({ "code": "r()", "syntax": "delimited-group", "delimiters": ["(", ")"] });
        assert_eq!(arg(both).unwrap_err(), ArgDataError::BothCodeAndSyntax);
        assert_eq!(arg(serde_json::json!({ "name": "size" })).unwrap_err(), ArgDataError::NeitherCodeNorSyntax);
    }

    #[test]
    fn the_data_form_serializes_every_field_and_reads_back_the_same() {
        let fields: ArgDataFields = serde_json::from_value(serde_json::json!({
            "code": "{", "name": "path", "mode": "text", "scoped": ["my.package"], "role": "my.path",
        }))
        .unwrap();
        let written = serde_json::to_value(&fields).unwrap();
        assert_eq!(
            written,
            serde_json::json!({
                "code": "{", "name": "path", "layout": "inline", "opaque_wrapper": false, "docs": "",
                "mode": "text", "scoped": ["my.package"], "role": "my.path",
            })
        );
        let read: ArgDataFields = serde_json::from_value(written).unwrap();
        assert_eq!(read, fields);
    }
}
