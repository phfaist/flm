//! The argument codes: the short texts that describe the form of an argument in
//! settings given as data, and the only reader of those texts.
//!
//! [Settings given as data](crate::core::settings) cannot hold a parser object,
//! so when they declare a macro or an environment they write each of its
//! arguments as a short text, its **argument code**: `{` for a mandatory group,
//! `[` for an optional one, `r()` for a group in parentheses. This module reads
//! those codes, and no other part of the library does: [`arg_def_from_code`]
//! turns one code into an argument declaration ([`ArgDef`]), and
//! [`ArgListData`] reads a whole argument list written in any of the three
//! shapes that settings may use. Every feature whose settings declare macros
//! or environments reads its argument lists through these two, so that one
//! code means one form of argument in every settings file; the `userdefs`
//! feature is one of them
//! ([`deserialize_arg_list`] reads its `args` settings key).
//!
//! # The codes
//!
//! The codes are those of techy's table of argument codes
//! ([`argument_specs`](techy::latexlike::argument_specs)), plus four of FLM's
//! own for a terminated argument (`u`, `U`, `f` and `F`, described below the
//! table); techy is the parser toolkit FLM is built on. Each code corresponds
//! to one standard argument kind ([`ArgStdKind`]). Every code of techy's table
//! means what it means there, except the code `v` alone: it selects FLM's own verbatim reader, the one
//! FLM's verbatim commands use, which closes `{` with `}`, `[` with `]`, `(`
//! with `)`, `<` with `>` and any other character with itself, whereas techy's
//! `v` closes on the same character that opened. Each example shows a use of a
//! macro `\m` declared with that code as its only argument.
//!
//! | code | the argument | example | standard kind |
//! |---|---|---|---|
//! | `{` or `m` | a mandatory `{…}` group; a single token also counts | `\m{arg}`, `\m1` | [`Mandatory`](ArgStdKind::Mandatory) |
//! | `[` or `o` | an optional `[…]` group | `\m[arg]` | [`Optional`](ArgStdKind::Optional) |
//! | `*` or `s` | an optional `*` marker | `\m*` | [`Star`](ArgStdKind::Star) |
//! | `t<c>` | an optional marker made of the one character `<c>` (`t*` is the same as `*`) | `t!` reads `\m!` | [`Marker`](ArgStdKind::Marker) |
//! | `r<c1><c2>` | a mandatory group between `<c1>` and `<c2>` | `r()` reads `\m(arg)` | [`DelimitedGroup`](ArgStdKind::DelimitedGroup) |
//! | `d<c1><c2>` | an optional group between `<c1>` and `<c2>` | `d<>` reads `\m<arg>` | [`OptionalDelimitedGroup`](ArgStdKind::OptionalDelimitedGroup) |
//! | `v` | verbatim text between two delimiters, matched to the opening one | `\m+raw %text+` | [`Verbatim`](ArgStdKind::Verbatim) |
//! | `v<c1><c2>` | verbatim text between `<c1>` and `<c2>` | `v+-` reads `\m+raw %text-` | [`Verbatim`](ArgStdKind::Verbatim) |
//! | `e{<chars>}` | embellishments: each character between the braces is a marker that is followed by one expression; the markers may be written in any order, each at most once | `e{^_}` reads `\m^{a}_{b}` | [`Embellishments`](ArgStdKind::Embellishments) |
//! | `AnyDelimited` | a mandatory group in `{…}`, `[…]`, `(…)` or `<…>` | `\m<arg>` | [`AnyDelimitedGroup`](ArgStdKind::AnyDelimitedGroup) |
//! | `AnyDelimitedOptional` | the same pairs, optional | `\m(arg)` | [`OptionalAnyDelimitedGroup`](ArgStdKind::OptionalAnyDelimitedGroup) |
//! | `BracedOnly` | a mandatory `{…}` group, never a single token | `\m{arg}` | [`BracedOnly`](ArgStdKind::BracedOnly) |
//! | `u<terminator>` | document content up to `<terminator>`, one or more characters | `u\endm` reads `\m some \emph{text}\endm` | [`Terminated`](ArgStdKind::Terminated) |
//! | `U<terminator>` | raw text up to `<terminator>` | `U\endm` reads `\m raw %text\endm` | [`Terminated`](ArgStdKind::Terminated) with `raw` |
//! | `f<c><terminator>` | document content after a run of the character `<c>`, up to the same run followed by `<terminator>`, which may be empty | `f+` reads `\m++some text++` | [`Terminated`](ArgStdKind::Terminated) with `repeat_char` |
//! | `F<c><terminator>` | raw text after a run of the character `<c>`, up to the same run followed by `<terminator>` | `F+` reads `\m+++raw %text+++` | [`Terminated`](ArgStdKind::Terminated) with `repeat_char` and `raw` |
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
//! declaration that needs them is written in Rust.
//!
//! Three standard kinds have no code: a group read as plain characters
//! ([`CharsGroup`](ArgStdKind::CharsGroup)), a group of `key=value` pairs
//! ([`KeyVals`](ArgStdKind::KeyVals)), and the fields a command absorbs after
//! its other arguments ([`TackOn`](ArgStdKind::TackOn)). A feature declares
//! them in Rust only.
//!
//! # The three shapes of an argument list
//!
//! An argument list ([`ArgListData`]) is written in one of three shapes:
//!
//! - one text in which each character is one code (`"[{{"`: an optional group,
//!   then two mandatory ones); only the codes of one character can be written
//!   this way;
//! - a list of codes (`["[", "r()"]`);
//! - a list whose entries are each a code, or a map giving the code with the
//!   argument's name, parsing mode, layout, scoped packages, documentation and
//!   role (`[{code: "[", name: "size"}, "{"]`, `{code: "{", mode: "math"}`,
//!   `{code: "{", name: "target", role: "flm.refs.target"}`).
//
// Why the alphabet is the parser toolkit's and has one reader here: see
// decision rationale at [§flm-dd-dr:lang-arg-codes].

use std::fmt;

use serde::Deserialize;

use techy::latexlike::Mode;

use crate::lang::mint::decl::{ArgDef, ArgLayout};
use crate::lang::mint::parsers::ArgStdKind;
use crate::model::names::Name;

// --- an argument and a list of arguments, as data ------------------------------------------------

/// One argument of a macro or environment declared in settings given as data:
/// either its argument code alone (`"{"`), or a map giving the code with the
/// argument's name, parsing mode, layout, scoped packages, documentation and
/// role.
///
/// It converts into an argument declaration ([`ArgDef`]) through `TryFrom`,
/// which reads its code with [`arg_def_from_code`]; the codes are listed in
/// the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
pub enum ArgData {
    /// The argument code alone: the argument has no name and its content is
    /// laid out inline.
    Code(#[schemars(schema_with = "arg_code_schema")] String),
    /// The argument code with the argument's name, parsing mode, layout,
    /// scoped packages, documentation and role.
    Declared(ArgDataFields),
}

/// An argument written as a map in settings given as data: its code, name,
/// parsing mode, layout, scoped packages, documentation and role (see
/// [`ArgData`]).
//
// It is also written to data (`Serialize`), with every field present, and
// reads back as the same value: see the registry's "Serialized forms"
// (`crate::core::registry`). This is a plain comment, not documentation,
// because the documentation of this type is the description of a settings
// key in the settings' JSON Schema.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArgDataFields {
    /// The argument code, which says what form the argument takes (the codes
    /// are those [`arg_def_from_code`] reads).
    #[schemars(schema_with = "arg_code_schema")]
    pub code: String,
    /// The argument's name, by which a user definition's template refers to
    /// it. No name by default.
    #[serde(default)]
    pub name: Option<String>,
    /// How the argument's content is laid out: `inline` (running text with
    /// no paragraph, the default), `blocks` (paragraphs and other blocks), or
    /// `inline-or-blocks` (decided by the content); see [`ArgLayout`].
    #[serde(default)]
    pub layout: ArgLayout,
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

impl TryFrom<ArgData> for ArgDef {
    type Error = ArgCodeError;

    fn try_from(data: ArgData) -> Result<ArgDef, ArgCodeError> {
        match data {
            ArgData::Code(code) => arg_def_from_code(&code),
            ArgData::Declared(fields) => {
                let mut arg = arg_def_from_code(&fields.code)?.layout(fields.layout).docs(fields.docs);
                arg.name = fields.name;
                arg.mode = fields.mode;
                arg.scoped = fields.scoped;
                arg.role = fields.role;
                Ok(arg)
            }
        }
    }
}

/// The argument list of a macro or environment declared in settings given as
/// data, written either as one text in which each character is one argument
/// code (`"[{"`), or as a list whose entries are each a code or a map
/// ([`ArgData`]).
///
/// [`arg_defs`](ArgListData::arg_defs) converts it into argument declarations;
/// [`deserialize_arg_list`] reads a settings field of type `Vec<ArgDef>` in any
/// of the shapes. The [module documentation](self) lists the codes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
pub enum ArgListData {
    /// One text in which each character is one argument code: `"[{{"`.
    Codes(#[schemars(schema_with = "arg_codes_text_schema")] String),
    /// A list of arguments, each an argument code or a map.
    List(Vec<ArgData>),
}

impl ArgListData {
    /// Converts the list into argument declarations, in the order the
    /// arguments are written.
    ///
    /// # Errors
    ///
    /// The [`ArgCodeError`] of the first entry that is not an argument code,
    /// or that lacks the characters its code letter requires.
    pub fn arg_defs(self) -> Result<Vec<ArgDef>, ArgCodeError> {
        match self {
            ArgListData::Codes(codes) => {
                codes.chars().map(|code| arg_def_from_code(&String::from(code))).collect()
            }
            ArgListData::List(args) => args.into_iter().map(ArgDef::try_from).collect(),
        }
    }
}

/// Deserializes an argument list written in any of the shapes of
/// [`ArgListData`] into argument declarations.
///
/// Use it as the deserializer of a settings field of type `Vec<ArgDef>`, and
/// give the field [`ArgListData`]'s schema, as in the example.
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
/// ```
///
/// # Errors
///
/// The deserializer's error when the data has none of the shapes of
/// [`ArgListData`], or when an entry is not an argument code; the error then
/// carries the [`ArgCodeError`]'s message.
pub fn deserialize_arg_list<'de, D>(deserializer: D) -> Result<Vec<ArgDef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ArgListData::deserialize(deserializer)?.arg_defs().map_err(serde::de::Error::custom)
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
/// by [`arg_def_from_code`] and [`ArgListData::arg_defs`].
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
    fn the_data_form_serializes_every_field_and_reads_back_the_same() {
        let fields: ArgDataFields = serde_json::from_value(serde_json::json!({
            "code": "{", "name": "path", "mode": "text", "scoped": ["my.package"], "role": "my.path",
        }))
        .unwrap();
        let written = serde_json::to_value(&fields).unwrap();
        assert_eq!(
            written,
            serde_json::json!({
                "code": "{", "name": "path", "layout": "inline", "docs": "", "mode": "text",
                "scoped": ["my.package"], "role": "my.path",
            })
        );
        let read: ArgDataFields = serde_json::from_value(written).unwrap();
        assert_eq!(read, fields);
    }
}
