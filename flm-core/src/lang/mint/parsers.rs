//! The parser objects that argument and body declarations hold:
//! [`FlmArgumentParser`], which reads one argument, with the standard argument
//! kinds ([`ArgStdKind`]) as its standard implementations, and
//! [`FlmBodyParser`], which reads an environment's body.
//!
//! A declaration is data ([`super::decl`]), and its parser is one of its
//! fields. An argument declaration ([`ArgDef`]) holds the parser that reads
//! the argument beside everything else it declares: its name, its parsing
//! mode, its layout, the scoped packages available inside it, and its
//! documentation. Minting applies all of those around the parser, so a parser
//! of a feature's own receives them exactly as a standard kind does. A body
//! declaration ([`BodyDef`](super::BodyDef)) may likewise hold the parser that
//! reads the body, with its layout, mode and scoped packages declared beside
//! it.
//!
//! techy, the parser toolkit FLM is built on, defines the traits that read the
//! source ([`ArgumentParser`], [`EnvironmentBehavior`]). The two traits here
//! extend them with what FLM needs to know about an argument or a body beyond
//! reading it: what identifies the parser in the [registry
//! digest](crate::model::digest) (the hash of everything about a registry that
//! can change what processing produces), and a short description. For an argument, they also return which tack-on fields it
//! declares, and how the [recomposed LaTeX
//! backend](crate::stdcore::backends::latex_recomposed) takes the argument
//! when it keeps a [user definition](crate::lang::rewrites::userdefs::Definition)
//! as a LaTeX command instead of expanding its uses. The library reads a
//! parser only through these methods and never tests which parser type it
//! is.

use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    ArgumentParser, ConstructParserResult, EmbellishmentsArgumentParser, GroupArgumentParser,
    MarkerArgumentParser, OptionalGroupArgumentParser, ParseContext, ParsedArgumentNodes,
    TackOnFieldsArgumentParser, TerminatedArgumentParser,
};
use techy::core::specs::ArgumentSpec;
use techy::core::token::GroupRule;
use techy::latexlike::{argument_specs, EnvironmentBehavior};

use crate::lang::chars_group::FlmCharsGroupArgumentParser;
use crate::lang::flm::{Flm, FlmCallableType, FlmGroupType};
use crate::lang::mint::decl::{ArgDef, TackOnField};
use crate::lang::mint::opaque::OpaqueWrapperReader;
use crate::lang::verbatim::{
    FlmRawTerminatedArgumentParser, FlmVerbatimArgumentParser, FlmVerbatimBehavior,
};

// --- the argument parser ---------------------------------------------------------------------

/// The parser of one argument of a macro, an environment or a special
/// character sequence, together with what FLM needs to know about the
/// argument beyond reading it.
///
/// techy's [`ArgumentParser`] reads the argument from the source: it stages
/// (builds into the parsed tree) the argument's nodes and says which of them
/// are its content, or reports the argument absent. This trait adds the
/// questions that minting, the registry build, the JSON schema of the
/// settings and the `userdefs` feature ask of an argument, one method each:
/// its identity, its tack-on fields, the shape of a definition written back,
/// and its description. The standard kinds ([`ArgStdKind`]) implement them; a
/// feature whose syntax the standard kinds cannot express writes a parser of
/// its own and implements them itself.
///
/// An argument declaration ([`ArgDef`]) holds the parser as a shared object,
/// `Arc<dyn FlmArgumentParser>`. Minting applies what the declaration holds
/// beside it, the parsing mode ([`ArgDef::mode`]) and the scoped packages
/// ([`ArgDef::scoped`]), as a change of parsing state over the whole argument,
/// before [`parse_argument`](ArgumentParser::parse_argument) is called: the
/// parser finds them already in `cx.state` and does nothing to obtain them.
/// Whether the content is kept as written is the declaration's choice too
/// ([`ArgDef::opaque_wrapper`]): where the declaration asks for it, minting
/// places what the parser read under an opaque wrapper, whatever the parser.
///
/// techy's own trait has one more method that matters here:
/// [`can_match_empty`](ArgumentParser::can_match_empty) returns whether a use
/// site may leave the argument out (`true` for an optional group or a `*`
/// marker, `false` for a mandatory group). When the recomposed LaTeX backend
/// keeps a user definition, it calls this method to decide whether a call
/// must tell the definition whether the argument was written. techy's default returns
/// `true`, so a parser that always reads something, and never reports the
/// argument absent, overrides it to return `false`.
///
/// # Implementing FlmArgumentParser
///
/// One parser object serves every use of the command: it takes `&self`,
/// holds no state of one parse, and receives what one use needs through its
/// arguments (techy's requirement for [`ArgumentParser`]).
/// It must never panic or loop on any input: an argument that reads nothing,
/// and the end of the input reached before the argument, are ordinary inputs.
/// Report a problem through `cx.recover` and return what was read.
///
/// ```
/// use flm_core::lang::Flm;
/// use flm_core::lang::mint::{DefinitionShape, FlmArgumentParser};
/// use flm_core::techy::core::constructs::{
///     ArgumentParser, ConstructParserResult, OptionalGroupArgumentParser, ParseContext,
///     ParsedArgumentNodes,
/// };
/// use flm_core::techy::core::specs::ArgumentSpec;
/// use flm_core::techy::core::token::GroupRule;
/// use flm_core::lang::FlmGroupType;
/// use std::sync::Arc;
///
/// /// An optional `<…>` group.
/// #[derive(Debug)]
/// struct Angles(OptionalGroupArgumentParser<Flm>);
///
/// impl Angles {
///     fn new() -> Angles {
///         let rule = Arc::new(GroupRule {
///             group_type: FlmGroupType::Content,
///             open: String::from("<"),
///             close: String::from(">"),
///         });
///         Angles(OptionalGroupArgumentParser::new(rule))
///     }
/// }
///
/// impl ArgumentParser<Flm> for Angles {
///     fn parse_argument(
///         &self,
///         cx: &mut ParseContext<'_, '_, Flm>,
///         spec: &ArgumentSpec<Flm>,
///     ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
///         self.0.parse_argument(cx, spec)
///     }
/// }
///
/// impl FlmArgumentParser for Angles {
///     fn identity(&self) -> Vec<u8> {
///         b"example.angles".to_vec()
///     }
///     fn definition_shape(&self) -> DefinitionShape {
///         DefinitionShape::NoShape { token: "angles" }
///     }
///     fn description(&self) -> String {
///         String::from("an optional `<…>` group")
///     }
/// }
///
/// let arg = flm_core::lang::mint::ArgDef::named("extra", Angles::new());
/// assert_eq!(arg.kind.description(), "an optional `<…>` group");
/// ```
pub trait FlmArgumentParser: ArgumentParser<Flm> + Send + Sync {
    /// Returns the bytes that identify this parser in the [registry
    /// digest](crate::model::digest) (the hash of everything about a registry
    /// that can change what processing produces) and in the digest that the
    /// `userdefs` feature computes over its configuration
    /// ([`UserdefsConfig::digest_bytes`](crate::stdcore::features::userdefs::UserdefsConfig::digest_bytes)).
    ///
    /// Two parsers that read differently must return different bytes (a
    /// standard kind returns bytes per kind and per delimiter pair), and one
    /// parser must return the same bytes in every run and every process. Build
    /// them from data the parser holds, never from an address or a randomized
    /// hash. The digests take the bytes as one string, preceded by its length,
    /// so the bytes need no terminator. A parser of a feature's own begins its
    /// bytes with a name no other parser uses, such as the feature's name.
    fn identity(&self) -> Vec<u8>;

    /// Returns the [tack-on fields](TackOnField) the argument declares, when
    /// it is a **tack-on argument**: the trailing argument that absorbs the
    /// field macros written after a command, such as the `\label{…}` after a
    /// heading. The default returns `None`, for every other argument.
    ///
    /// A parser of a feature's own keeps the default; declare a tack-on
    /// argument with [`ArgStdKind::TackOn`]. For any argument whose parser
    /// returns fields, minting builds the argument's reader itself, and that
    /// reader replaces the object's
    /// [`parse_argument`](ArgumentParser::parse_argument).
    ///
    /// The reason is that each tack-on field is a declaration of its own, with
    /// its own element and arguments. Minting builds the field's specification
    /// into the techy package of fields
    /// ([`FIELDS_PACKAGE_NAME`](super::FIELDS_PACKAGE_NAME)), which records
    /// where the specification is held so that a serialized tree can find it
    /// again (the [provenance stamp](super::packages)). That package is
    /// created during the language build, after every parser object has been
    /// created, so a parser object cannot hold those specifications.
    fn tack_on_fields(&self) -> Option<&[TackOnField]> {
        None
    }

    /// Returns how a user definition that declares this argument is written
    /// back as a LaTeX definition, when the [recomposed LaTeX
    /// backend](crate::stdcore::backends::latex_recomposed) keeps the
    /// definition rather than expanding it. See [`DefinitionShape`].
    fn definition_shape(&self) -> DefinitionShape;

    /// Returns a description of the argument's syntax in a short phrase, such
    /// as "an optional `[…]` group".
    ///
    /// The JSON schema of the settings shows it for each argument code that a
    /// settings file may write
    /// ([`arg_def_from_code`](crate::lang::mint::arg_def_from_code)). Nothing
    /// else in the library reads it.
    fn description(&self) -> String;

    /// Returns the [standard kind](ArgStdKind) this parser reads, for a
    /// program that describes the registry's declarations, such as a
    /// generator of documentation pages; `None`, the default, for a parser of
    /// a feature's own.
    fn std_kind(&self) -> Option<&ArgStdKind> {
        None
    }
}

/// How the [recomposed LaTeX backend](crate::stdcore::backends::latex_recomposed)
/// takes one declared argument when it keeps a [user
/// definition](crate::lang::rewrites::userdefs::Definition) as a LaTeX command
/// instead of expanding its uses, as returned by
/// [`FlmArgumentParser::definition_shape`].
///
/// When the recomposed LaTeX backend keeps a user definition, it writes the
/// definition once, as a LaTeX command, and each use of the definition as a
/// call of that command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefinitionShape {
    /// The definition takes the argument as a value: each call writes, in one
    /// brace group, what the use site wrote for the argument, so the
    /// argument's delimiters in the source never reach LaTeX. A tack-on
    /// argument, which no template may refer to, contributes no group. Every
    /// standard kind returns this, except
    /// [`ArgStdKind::Terminated`].
    Value,
    /// No shape of a kept definition expresses the argument. A user
    /// definition that declares it is not kept: the recomposed LaTeX backend
    /// expands its uses instead, and gives `token` as the reason the
    /// definition was not kept
    /// ([`NotRecomposable::ArgumentShape`](crate::lang::rewrites::userdefs::NotRecomposable::ArgumentShape)).
    NoShape {
        /// The shape's name in one stable word, given as the reason the
        /// definition was not kept.
        token: &'static str,
    },
}

/// Conversion into an argument's shared parser object: the parameter type of
/// [`ArgDef::new`] and [`ArgDef::named`], so that a standard kind and a parser
/// of one's own are passed the same way.
///
/// Three types convert: an [`ArgStdKind`], wrapped into the object that reads
/// it; any [`FlmArgumentParser`], passed by value and shared from then on; and
/// an already shared `Arc<dyn FlmArgumentParser>`, passed through unchanged,
/// so that one parser reused by several declarations stays one value.
pub trait IntoFlmArgumentParser {
    /// Returns the shared parser object an [`ArgDef`] holds.
    fn into_flm_argument_parser(self) -> Arc<dyn FlmArgumentParser>;
}

impl IntoFlmArgumentParser for ArgStdKind {
    fn into_flm_argument_parser(self) -> Arc<dyn FlmArgumentParser> {
        Arc::new(StdArgumentParser::new(self))
    }
}

impl<P: FlmArgumentParser + 'static> IntoFlmArgumentParser for P {
    fn into_flm_argument_parser(self) -> Arc<dyn FlmArgumentParser> {
        Arc::new(self)
    }
}

impl IntoFlmArgumentParser for Arc<dyn FlmArgumentParser> {
    fn into_flm_argument_parser(self) -> Arc<dyn FlmArgumentParser> {
        self
    }
}

// --- the standard kinds ----------------------------------------------------------------------

/// The standard argument kinds: the forms of argument that FLM provides,
/// each describing, as data, what one argument looks like in the source.
///
/// An argument declaration takes a kind as its parser
/// (`ArgDef::named("title", ArgStdKind::Mandatory)`, see [`ArgDef::named`]);
/// each kind is read by a parser of techy's or of FLM's own and implements
/// [`FlmArgumentParser`]. An argument that no standard kind expresses holds a
/// parser of its own instead.
///
/// # The kind as data
///
/// A kind is written to data and read from it (`serde::Serialize`,
/// `serde::Deserialize`) as a map: the key `syntax` holds the kind's name, in
/// lowercase with hyphens, and the kind's parameters stand beside it, as in
/// `{"syntax": "delimited-group", "delimiters": ["(", ")"]}`. This map is the
/// **structured syntax** of an argument in settings given as data, which the
/// [`codes`](crate::lang::mint::codes) module reads together with the
/// argument codes, the shorthands that stand for the same kinds. The names
/// and the parameters:
///
/// | kind | `syntax` | parameters |
/// |---|---|---|
/// | [`Mandatory`](ArgStdKind::Mandatory) | `mandatory` | none |
/// | [`Optional`](ArgStdKind::Optional) | `optional` | none |
/// | [`Star`](ArgStdKind::Star) | `star` | none |
/// | [`Marker`](ArgStdKind::Marker) | `marker` | `marker`: the marker's character (`*` reads as `star`) |
/// | [`DelimitedGroup`](ArgStdKind::DelimitedGroup) | `delimited-group` | `delimiters`: the opening and the closing character, as a list of two (`["(", ")"]`) |
/// | [`OptionalDelimitedGroup`](ArgStdKind::OptionalDelimitedGroup) | `optional-delimited-group` | `delimiters`, as for `delimited-group` |
/// | [`AnyDelimitedGroup`](ArgStdKind::AnyDelimitedGroup) | `any-delimited-group` | none |
/// | [`OptionalAnyDelimitedGroup`](ArgStdKind::OptionalAnyDelimitedGroup) | `optional-any-delimited-group` | none |
/// | [`BracedOnly`](ArgStdKind::BracedOnly) | `braced-only` | none |
/// | [`Embellishments`](ArgStdKind::Embellishments) | `embellishments` | `markers`: the marker characters, a list of at least one (`["^", "_"]`) |
/// | [`KeyVals`](ArgStdKind::KeyVals) | `key-vals` | none |
/// | [`CharsGroup`](ArgStdKind::CharsGroup) | `chars-group` | none |
/// | [`Verbatim`](ArgStdKind::Verbatim) | `verbatim` | `delimiters`: the prescribed pair, as for `delimited-group`; absent for a closing delimiter matched to whichever opening one is found |
/// | [`TackOn`](ArgStdKind::TackOn) | `tack-on` | `fields`: a list of maps `{"name", "args", "repeatable", "docs"}`, one per field, each field's `args` a list of maps of its arguments ([`ArgDataFields`](crate::lang::mint::ArgDataFields)) |
/// | [`Terminated`](ArgStdKind::Terminated) | `terminated` | `terminator`: text; `repeat_char`: one character, absent for none; `opener`: text, empty by default; `head`: text, empty by default; `raw`: `true` or `false`, `false` by default |
///
/// Each character is written as a text of one character, which must not be
/// whitespace, as in the argument codes. A parameter that has no default must
/// be written. Reading a kind refuses, with a message that states the
/// condition broken:
///
/// - a key that is not one of the kind's parameters, and a name that is no
///   kind's;
/// - a character written as a text of another length, or as whitespace;
/// - `delimiters` that are not two, and an empty list of `markers`;
/// - the two configurations of `terminated` that can never work (see
///   [`Terminated`](ArgStdKind::Terminated)): a terminator that is always
///   empty, and an `opener` that begins with `repeat_char`;
/// - every `tack-on` kind: its fields are declarations a feature makes in
///   Rust, each with an element of its own ([`TackOnField`]), so a tack-on
///   argument is declared by a feature, never in settings.
///
/// Three kinds have no argument code ([`KeyVals`](ArgStdKind::KeyVals),
/// [`CharsGroup`](ArgStdKind::CharsGroup), [`TackOn`](ArgStdKind::TackOn)),
/// and the codes of [`Terminated`](ArgStdKind::Terminated) set neither its
/// opener nor its terminator head: settings write those with the structured
/// syntax, a tack-on argument excepted.
///
/// Two kinds are equal (`PartialEq`) when they are the same kind with equal
/// parameters; the fields of two tack-on kinds are compared as
/// [`TackOnField`]'s `PartialEq` states.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(
    into = "crate::lang::mint::codes::ArgStdKindData",
    try_from = "crate::lang::mint::codes::ArgStdKindData"
)]
pub enum ArgStdKind {
    /// A mandatory `{…}` group. A single token also counts, as in TeX:
    /// `\frac12` is `\frac{1}{2}`.
    Mandatory,
    /// An optional `[…]` group. When the whole content is one `{…}` group,
    /// the argument's content is that group's content, so it may hold the
    /// closing character: `[{a]b}]` reads `a]b`.
    Optional,
    /// An optional `*` marker: whether the command was written in its starred
    /// form.
    Star,
    /// An optional marker made of one character other than `*`, as the `!` of
    /// `\name!`: whether the command was written with it. The `*` marker is
    /// [`ArgStdKind::Star`].
    Marker {
        /// The marker's character.
        marker: char,
    },
    /// A mandatory group between a prescribed pair of characters, as the
    /// `(…)` of `\name(arg)`. Nothing else is accepted: no `{…}` group and no
    /// single token. The two characters delimit a group only at this
    /// argument's position; everywhere else they are ordinary characters.
    DelimitedGroup {
        /// The opening character.
        open: char,
        /// The closing character.
        close: char,
    },
    /// An optional group between a prescribed pair of characters, as the
    /// `<…>` of `\name<arg>`. As for [`ArgStdKind::Optional`], when the
    /// whole content is one `{…}` group, the argument's content is that
    /// group's content: `<{a>b}>` reads `a>b`.
    OptionalDelimitedGroup {
        /// The opening character.
        open: char,
        /// The closing character.
        close: char,
    },
    /// A mandatory group between any of the pairs `{…}`, `[…]`, `(…)` and
    /// `<…>`, whichever the use site opens; the parsed group records which
    /// pair delimits it. No single token counts.
    AnyDelimitedGroup,
    /// An optional group between any of the pairs of
    /// [`ArgStdKind::AnyDelimitedGroup`].
    OptionalAnyDelimitedGroup,
    /// A mandatory `{…}` group and nothing else: unlike
    /// [`ArgStdKind::Mandatory`], a single token does not count, so a use
    /// site that writes no group is told that the argument is missing.
    BracedOnly,
    /// **Embellishments**: a run of pairs, each a marker character followed by
    /// one expression (a group, a command with its arguments, or one
    /// character), in any order, each marker at most once, as the subscript
    /// and the superscript of `\name_{a}^{b}` or `\name^{b}_{a}`. The argument
    /// is absent when no marker follows the command. techy's
    /// [`split_embellishments`](techy::extract::split_embellishments) reads
    /// the pairs back by marker.
    Embellishments {
        /// The marker characters, at least one.
        markers: Vec<char>,
    },
    /// An optional `[key=value,…]` group.
    ///
    /// Parsed like [`ArgStdKind::Optional`]; the element that declares the
    /// argument reads the keys and values out of it. Declaring this kind
    /// rather than [`ArgStdKind::Optional`] records what the argument is for.
    /// The values are FLM markup. A group whose values the document shows (an
    /// item's tag, a heading's `alt=`) is laid out by the [block-structure
    /// pass](crate::lang::rewrites::blocks) as any content. An element that
    /// reads the values as data declares the argument with
    /// [`ArgDef::with_opaque_wrapper`], so that they are kept as written.
    KeyVals,
    /// A mandatory `{…}` group read as plain characters, as in
    /// `\label{sec:intro}`. An escape character and a special character
    /// sequence are ordinary characters inside it; comments and nested groups
    /// are still recognized, and when a user definition's template is parsed,
    /// its placeholders (`#1`, `#{name}`) are still recognized inside it
    /// ([`crate::lang::chars_group`]). The reader places the group under an
    /// opaque wrapper ([`FlmGroupType::OpaqueWrapper`]), so the
    /// [block-structure pass](crate::lang::rewrites::blocks) carries the
    /// characters over as written.
    CharsGroup,
    /// A verbatim argument delimited by a pair of characters
    /// (`\verbcode|…|`, `\verbcode{…}`): raw text up to the closing delimiter,
    /// wrapped in a group of the verbatim class like every other verbatim run
    /// ([`crate::lang::verbatim`], which also states how this shape differs
    /// from [`ArgStdKind::CharsGroup`]). The [block-structure
    /// pass](crate::lang::rewrites::blocks) carries the text over as written.
    Verbatim {
        /// `None` matches the closing delimiter to whichever opening character
        /// is found (`{}`, `[]`, `<>`, `()`, and otherwise the same character
        /// again); `Some((open, close))` prescribes the pair.
        delimiters: Option<(char, char)>,
    },
    /// The [tack-on fields](TackOnField) a command accepts after its other
    /// arguments, absorbed as its last argument. Each field written there
    /// becomes a node carrying the field's element.
    TackOn {
        /// The fields this command accepts, in no particular order: they may be
        /// written in any order after the command.
        fields: Vec<TackOnField>,
    },
    // See decision rationale at [§flm-dd-dr:terminated-argument].
    /// A mandatory argument that runs up to a fixed character sequence, its
    /// **terminator**, as in `\phf … \endphf` or a code span `` `…` ``.
    ///
    /// The argument is written as three consecutive parts:
    ///
    /// 1. the **prefix**: a run of copies of `repeat_char`, as many as the use
    ///    site writes, possibly none (only when `repeat_char` is set),
    ///    followed by `opener` (empty when not needed);
    /// 2. the **content**;
    /// 3. the terminator: `head`, then as many copies of `repeat_char` as the
    ///    prefix had, then `terminator`.
    ///
    /// | syntax | declaration |
    /// |---|---|
    /// | `\phf … \endphf`, content parsed | a macro `\phf` with `Terminated { terminator: "\\endphf", repeat_char: None, opener: "", head: "", raw: false }` |
    /// | `` `x` `` and ``` ``a`b`` ```, raw text | a special character sequence `` ` `` with `` Terminated { terminator: "`", repeat_char: Some('`'), opener: "", head: "", raw: true } `` |
    /// | `\fenced+++…+++`, raw text | a macro `\fenced` with `Terminated { terminator: "", repeat_char: Some('+'), opener: "", head: "", raw: true }` |
    ///
    /// In the code span row, the special character sequence has already read
    /// the first backtick, and the argument counts the backticks that follow
    /// it: the terminator is one backtick for each counted one, then the
    /// final backtick, as many backticks as opened the span.
    ///
    /// Nothing is skipped before the prefix: a space at the start is content,
    /// as in the code span `` ` x` ``, except the spaces that the source
    /// reader counts as part of the preceding command name (`\phf a\endphf`
    /// has the content `a`). The terminator is matched character for
    /// character wherever it appears, before any other markup: inside parsed
    /// content, `\endphf` is never read as a command, and a run of three
    /// backticks ends a code span opened with two at its first two, the third
    /// following the argument. Nothing is trimmed from the content. The
    /// toolkit's [`TerminatedArgumentParser`]
    /// reads the argument; its documentation gives the complete rules.
    ///
    /// With `raw: false`, the content is parsed as document content: commands,
    /// groups and comments inside it are read as usual, and the terminator is
    /// not recognized inside a nested `{…}` group. With `raw: true`, the
    /// content is raw text, read under the same parsing state as a verbatim
    /// argument's interior and held by a group of the verbatim class, like
    /// every verbatim run ([`crate::lang::verbatim`]); the [block-structure
    /// pass](crate::lang::rewrites::blocks) carries it over as written.
    ///
    /// When the terminator never appears, the argument keeps the content read
    /// up to the end of the input (or up to a closing delimiter that nothing
    /// in it opened) and the toolkit's
    /// [`MissingArgumentTerminator`](techy::core::constructs::MissingArgumentTerminator)
    /// is reported. When the prefix is incomplete (the opener does not follow
    /// the run, or the run is the whole terminator and the use site writes
    /// none of it), the toolkit's
    /// [`ExpectedArgumentOpener`](techy::core::constructs::ExpectedArgumentOpener)
    /// is reported and the argument is absent.
    ///
    /// Two configurations can never work, and the toolkit stops every parse
    /// that reaches such an argument with an implementation error: a
    /// terminator that is always empty (`repeat_char`, `head` and
    /// `terminator` all empty), and an `opener` that begins with
    /// `repeat_char`. Reading the kind from data refuses both.
    ///
    /// Settings given as data write this kind with the structured syntax
    /// `terminated`, which sets each of the five fields, or with the codes
    /// `u`, `U`, `f` and `F`, which set neither `opener` nor `head`
    /// ([`crate::lang::mint::codes`]). The recomposed LaTeX backend cannot
    /// keep a user definition that declares such an argument
    /// ([`DefinitionShape::NoShape`]).
    Terminated {
        /// The end of the terminator, after the repeated characters: the
        /// whole terminator unless `repeat_char` or `head` is set. It may be
        /// empty only when `repeat_char` is set.
        terminator: String,
        /// The character whose run starts the prefix and is repeated in the
        /// terminator, or `None`.
        repeat_char: Option<char>,
        /// The fixed text that follows the run of `repeat_char` in the
        /// prefix; empty for none.
        opener: String,
        /// The fixed text that begins the terminator, before the repeated
        /// characters; empty for none.
        head: String,
        /// Whether the content is raw text (`true`) or parsed document
        /// content (`false`).
        raw: bool,
    },
}

impl ArgStdKind {
    /// Returns the parser that reads this kind: one of techy's argument
    /// presets (`m`, `o`, `s`); one of techy's group, marker or embellishments
    /// parsers, built as techy's table of argument codes builds it for the
    /// same code; FLM's characters-only group or verbatim argument; techy's
    /// terminated parser, run under the raw-text state for raw content
    /// ([`FlmRawTerminatedArgumentParser`]); or, for a tack-on argument,
    /// techy's tack-on reader with no field, which absorbs nothing. The specifications of the fields exist only once minting has
    /// built them, and minting then builds the reader of a tack-on argument
    /// itself ([`FlmArgumentParser::tack_on_fields`]). The reader of a
    /// characters-only group places what it reads under an opaque wrapper
    /// ([`OpaqueWrapperReader`]).
    fn reader(&self) -> Arc<dyn ArgumentParser<Flm>> {
        match self {
            ArgStdKind::Mandatory => preset("m"),
            ArgStdKind::Optional | ArgStdKind::KeyVals => preset("o"),
            ArgStdKind::Star => preset("s"),
            ArgStdKind::Marker { marker } => Arc::new(MarkerArgumentParser::new(String::from(*marker))),
            ArgStdKind::DelimitedGroup { open, close } => {
                Arc::new(GroupArgumentParser::with_rule(group_rule(*open, *close)))
            }
            ArgStdKind::OptionalDelimitedGroup { open, close } => Arc::new(
                OptionalGroupArgumentParser::new(group_rule(*open, *close))
                    .with_unwrap_lone_group(FlmGroupType::Content),
            ),
            ArgStdKind::AnyDelimitedGroup => Arc::new(GroupArgumentParser::any_of(any_delimited_rules())),
            ArgStdKind::OptionalAnyDelimitedGroup => Arc::new(
                OptionalGroupArgumentParser::any_of(any_delimited_rules())
                    .with_unwrap_lone_group(FlmGroupType::Content),
            ),
            ArgStdKind::BracedOnly => Arc::new(
                GroupArgumentParser::<Flm>::new(FlmGroupType::Content).with_expression_fallback(false),
            ),
            ArgStdKind::Embellishments { markers } => {
                Arc::new(EmbellishmentsArgumentParser::new(markers.iter().map(|marker| String::from(*marker))))
            }
            ArgStdKind::CharsGroup => {
                Arc::new(OpaqueWrapperReader::new(Arc::new(FlmCharsGroupArgumentParser::new())))
            }
            ArgStdKind::Verbatim { delimiters: Some((open, close)) } => {
                Arc::new(FlmVerbatimArgumentParser::new().with_delimiters(*open, *close))
            }
            ArgStdKind::Verbatim { delimiters: None } => Arc::new(FlmVerbatimArgumentParser::new()),
            ArgStdKind::TackOn { .. } => {
                Arc::new(TackOnFieldsArgumentParser::<Flm>::new(FlmCallableType::Macro))
            }
            ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw } => {
                let group_type = if *raw { FlmGroupType::Verbatim } else { FlmGroupType::Content };
                let mut parser = TerminatedArgumentParser::<Flm>::new(group_type, terminator.as_str())
                    .with_opener(opener.as_str())
                    .with_terminator_head(head.as_str());
                if let Some(repeat_char) = repeat_char {
                    parser = parser.with_repeat_char(*repeat_char);
                }
                if *raw {
                    Arc::new(FlmRawTerminatedArgumentParser::new(parser))
                } else {
                    Arc::new(parser)
                }
            }
        }
    }

    /// Returns the identity bytes of [`FlmArgumentParser::identity`]: the
    /// kind's word, then the characters the kind holds (a marker, a pair, the
    /// markers of embellishments), a verbatim argument's prescribed pair, a
    /// tack-on argument's fields (each field's name, its arguments' names and
    /// identities, and whether it may repeat), or a terminated argument's
    /// configuration (its terminator, its repeat character or none, its
    /// opener, its terminator head, and whether it is raw).
    fn identity(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            ArgStdKind::Mandatory => put(&mut out, "mandatory"),
            ArgStdKind::Optional => put(&mut out, "optional"),
            ArgStdKind::Star => put(&mut out, "star"),
            ArgStdKind::Marker { marker } => {
                put(&mut out, "marker");
                put(&mut out, &String::from(*marker));
            }
            ArgStdKind::DelimitedGroup { open, close } => {
                put(&mut out, "delimited");
                put(&mut out, &format!("{open}{close}"));
            }
            ArgStdKind::OptionalDelimitedGroup { open, close } => {
                put(&mut out, "optional-delimited");
                put(&mut out, &format!("{open}{close}"));
            }
            ArgStdKind::AnyDelimitedGroup => put(&mut out, "any-delimited"),
            ArgStdKind::OptionalAnyDelimitedGroup => put(&mut out, "optional-any-delimited"),
            ArgStdKind::BracedOnly => put(&mut out, "braced-only"),
            ArgStdKind::Embellishments { markers } => {
                put(&mut out, "embellishments");
                put(&mut out, &markers.iter().collect::<String>());
            }
            ArgStdKind::KeyVals => put(&mut out, "keyvals"),
            ArgStdKind::CharsGroup => put(&mut out, "chars"),
            ArgStdKind::Verbatim { delimiters } => {
                put(&mut out, "verbatim");
                match delimiters {
                    None => put(&mut out, "-"),
                    Some((open, close)) => put(&mut out, &format!("+{open}{close}")),
                }
            }
            ArgStdKind::TackOn { fields } => {
                put(&mut out, "tack-on");
                put_count(&mut out, fields.len());
                for field in fields {
                    put(&mut out, &field.name);
                    put_count(&mut out, field.args.len());
                    for arg in &field.args {
                        put_arg_identity(&mut out, arg);
                    }
                    put_count(&mut out, usize::from(field.repeatable));
                }
            }
            ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw } => {
                put(&mut out, "terminated");
                put(&mut out, terminator);
                match repeat_char {
                    None => put(&mut out, "-"),
                    Some(repeat_char) => put(&mut out, &format!("+{repeat_char}")),
                }
                put(&mut out, opener);
                put(&mut out, head);
                put_count(&mut out, usize::from(*raw));
            }
        }
        out
    }

    /// Returns the phrase of [`FlmArgumentParser::description`].
    fn description(&self) -> String {
        match self {
            ArgStdKind::Mandatory => String::from("a mandatory `{…}` group"),
            ArgStdKind::Optional => String::from("an optional `[…]` group"),
            ArgStdKind::Star => String::from("an optional `*` marker"),
            ArgStdKind::Marker { marker } => format!("an optional `{marker}` marker"),
            ArgStdKind::DelimitedGroup { open, close } => format!("a mandatory `{open}…{close}` group"),
            ArgStdKind::OptionalDelimitedGroup { open, close } => {
                format!("an optional `{open}…{close}` group")
            }
            ArgStdKind::AnyDelimitedGroup => {
                String::from("a mandatory group in `{…}`, `[…]`, `(…)` or `<…>`")
            }
            ArgStdKind::OptionalAnyDelimitedGroup => {
                String::from("an optional group in `{…}`, `[…]`, `(…)` or `<…>`")
            }
            ArgStdKind::BracedOnly => String::from("a mandatory `{…}` group, never a single token"),
            ArgStdKind::Embellishments { markers } => {
                let names: Vec<String> = markers.iter().map(|marker| format!("`{marker}`")).collect();
                format!(
                    "optional embellishments {}, each followed by one expression, in any order",
                    names.join(", ")
                )
            }
            ArgStdKind::KeyVals => String::from("an optional `[key=value,…]` group"),
            ArgStdKind::CharsGroup => {
                String::from("a `{…}` group read as plain characters, kept as written")
            }
            ArgStdKind::Verbatim { delimiters: None } => {
                String::from("verbatim text between a pair of delimiters")
            }
            ArgStdKind::Verbatim { delimiters: Some((open, close)) } => {
                format!("verbatim text between `{open}` and `{close}`")
            }
            ArgStdKind::TackOn { fields } => {
                let names: Vec<String> =
                    fields.iter().map(|field| format!("`\\{}`", field.name)).collect();
                format!("the fields {} written after the command", names.join(", "))
            }
            ArgStdKind::Terminated { terminator, repeat_char, opener, head, raw } => {
                let what = if *raw { "raw text" } else { "content" };
                let mut prefix = Vec::new();
                if let Some(repeat_char) = repeat_char {
                    prefix.push(format!("a run of {}", code_span(&String::from(*repeat_char))));
                }
                if !opener.is_empty() {
                    prefix.push(code_span(opener));
                }
                let after = if prefix.is_empty() {
                    String::new()
                } else {
                    format!(" after {},", prefix.join(" then "))
                };
                let until = match repeat_char {
                    None => code_span(&format!("{head}{terminator}")),
                    Some(_) => {
                        let mut parts = Vec::new();
                        if !head.is_empty() {
                            parts.push(code_span(head));
                        }
                        parts.push(String::from("the same run"));
                        if !terminator.is_empty() {
                            parts.push(code_span(terminator));
                        }
                        parts.join(" then ")
                    }
                };
                format!("{what}{after} up to {until}")
            }
        }
    }
}

/// Writes `text` as a code span of the description of a standard kind:
/// between single backticks, or, when `text` holds a backtick, between runs
/// of backticks longer than any run in `text`, with a space inside each end.
fn code_span(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    if longest == 0 {
        return format!("`{text}`");
    }
    let fence = "`".repeat(longest + 1);
    format!("{fence} {text} {fence}")
}

/// The parser object of a standard kind: the kind, and the parser that reads
/// it.
///
/// The reading parser is built **once**, when the kind is wrapped into its
/// shared object ([`IntoFlmArgumentParser`]), and shared by every invocation.
/// Building it at each parse instead would let the kind be the object by
/// itself, but it would repeat, for every argument of every invocation, the
/// allocations some of the parsers make when they are built (the group rule of
/// an optional group, the text of a marker). A declaration is wrapped once per
/// registry, so building once adds no work to a parse.
struct StdArgumentParser {
    kind: ArgStdKind,
    reader: Arc<dyn ArgumentParser<Flm>>,
}

impl StdArgumentParser {
    fn new(kind: ArgStdKind) -> StdArgumentParser {
        let reader = kind.reader();
        StdArgumentParser { kind, reader }
    }
}

impl fmt::Debug for StdArgumentParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.kind, f)
    }
}

impl ArgumentParser<Flm> for StdArgumentParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        self.reader.parse_argument(cx, spec)
    }

    fn can_match_empty(&self) -> bool {
        self.reader.can_match_empty()
    }
}

impl FlmArgumentParser for StdArgumentParser {
    fn identity(&self) -> Vec<u8> {
        self.kind.identity()
    }

    fn tack_on_fields(&self) -> Option<&[TackOnField]> {
        match &self.kind {
            ArgStdKind::TackOn { fields } => Some(fields),
            _ => None,
        }
    }

    /// Every standard kind but one is taken as a value: a key-value argument
    /// is a value like any other (the recomposed definition does not read its
    /// keys separately), and a verbatim argument is the characters the use
    /// site wrote between its delimiters. A terminated argument has no shape:
    /// a LaTeX command defined by the recomposed definition cannot read an
    /// argument up to a terminator.
    fn definition_shape(&self) -> DefinitionShape {
        match self.kind {
            ArgStdKind::Terminated { .. } => DefinitionShape::NoShape { token: "terminated" },
            _ => DefinitionShape::Value,
        }
    }

    fn description(&self) -> String {
        self.kind.description()
    }

    fn std_kind(&self) -> Option<&ArgStdKind> {
        Some(&self.kind)
    }
}

/// Returns the parser of one of techy's argument codes (`m`, `o`, `s`). The
/// codes are constants of this module, so the conversion cannot fail; the
/// `expect` records that.
fn preset(code: &str) -> Arc<dyn ArgumentParser<Flm>> {
    argument_specs::<Flm, _>([code])
        .expect("the preset argument codes listed here are valid")
        .into_iter()
        .next()
        .expect("one code yields one argument specification")
        .parser
        .clone()
}

/// Returns the rule of a group delimited by `open` and `close` at one
/// argument's position: a group of the content class, as techy's table of
/// argument codes builds it for the codes `r` and `d`.
fn group_rule(open: char, close: char) -> Arc<GroupRule<Flm>> {
    Arc::new(GroupRule {
        group_type: FlmGroupType::Content,
        open: String::from(open),
        close: String::from(close),
    })
}

/// The pairs a group of [`ArgStdKind::AnyDelimitedGroup`] may be written in,
/// in the order techy's table of argument codes lists them for the code
/// `AnyDelimited`.
const ANY_DELIMITED_PAIRS: [(char, char); 4] = [('{', '}'), ('[', ']'), ('(', ')'), ('<', '>')];

/// The rules of [`ANY_DELIMITED_PAIRS`].
fn any_delimited_rules() -> Vec<Arc<GroupRule<Flm>>> {
    ANY_DELIMITED_PAIRS.into_iter().map(|(open, close)| group_rule(open, close)).collect()
}

// --- identity bytes --------------------------------------------------------------------------

/// Writes a string, its length first.
fn put(out: &mut Vec<u8>, text: &str) {
    put_count(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes a count or a flag as eight little-endian bytes.
fn put_count(out: &mut Vec<u8>, count: usize) {
    out.extend_from_slice(&(count as u64).to_le_bytes());
}

/// Writes one argument declaration's name (`-` when it has none, `+` and the
/// name otherwise) and its parser's identity, its length first.
fn put_arg_identity(out: &mut Vec<u8>, arg: &ArgDef) {
    match &arg.name {
        None => put(out, "-"),
        Some(name) => put(out, &format!("+{name}")),
    }
    let identity = arg.kind.identity();
    put_count(out, identity.len());
    out.extend_from_slice(&identity);
}

// --- the body parser -------------------------------------------------------------------------

/// The parser of an environment's body that is not read the ordinary way,
/// together with what FLM needs to know about the body beyond reading it.
///
/// techy's [`EnvironmentBehavior`] reads the body through
/// [`make_body_parser`](EnvironmentBehavior::make_body_parser): FLM's verbatim
/// reader reads raw text up to `\end{name}`, and a feature's parser may read
/// the ordinary content and then check it. A body declaration
/// ([`BodyDef::parser`](super::BodyDef::parser)) holds such an object;
/// [`BodyDef::verbatim`](super::BodyDef::verbatim) installs the verbatim
/// reader. A body that declares none is read the ordinary way: content up to
/// and including `\end{name}`.
///
/// Only `make_body_parser` is called. The environment's arguments are those
/// of the declaration ([`SyntaxDef::Environment`](super::SyntaxDef::Environment)),
/// and the body is read under the parsing state the body declaration asks
/// for, its parsing mode ([`BodyDef::mode`](super::BodyDef::mode)) and its
/// scoped packages ([`BodyDef::scoped`](super::BodyDef::scoped)), which
/// minting applies around the body parse. The object's own
/// [`arguments`](EnvironmentBehavior::arguments) and
/// [`body_state_delta`](EnvironmentBehavior::body_state_delta) are not
/// called.
///
/// Like an argument parser, a body parser is shared by every invocation and
/// must never panic or loop: the end of the input reached before `\end{name}`
/// is an ordinary input.
pub trait FlmBodyParser: EnvironmentBehavior<Flm> + Send + Sync {
    /// Returns the bytes that identify this body parser in the [registry
    /// digest](crate::model::digest), under the same rules as
    /// [`FlmArgumentParser::identity`].
    fn identity(&self) -> Vec<u8>;

    /// Returns a description of how the body is read, in a short phrase. The
    /// library does not act on it; a program that describes the registry's
    /// declarations
    /// ([`FlmRegistry::declarations`](crate::core::registry::FlmRegistry::declarations))
    /// shows it.
    fn description(&self) -> String;
}

/// Conversion into a body's shared parser object: the parameter type of
/// [`BodyDef::parser`](super::BodyDef::parser), as [`IntoFlmArgumentParser`]
/// is for an argument.
///
/// Two types convert: any [`FlmBodyParser`], passed by value and shared from
/// then on, and an already shared `Arc<dyn FlmBodyParser>`, passed through
/// unchanged, so that one parser reused by several environments stays one
/// value. The body declaration always holds the parser as
/// `Option<Arc<dyn FlmBodyParser>>` (the field `BodyDef::parser`).
pub trait IntoFlmBodyParser {
    /// Returns the shared parser object a [`BodyDef`](super::BodyDef) holds.
    fn into_flm_body_parser(self) -> Arc<dyn FlmBodyParser>;
}

impl<P: FlmBodyParser + 'static> IntoFlmBodyParser for P {
    fn into_flm_body_parser(self) -> Arc<dyn FlmBodyParser> {
        Arc::new(self)
    }
}

impl IntoFlmBodyParser for Arc<dyn FlmBodyParser> {
    fn into_flm_body_parser(self) -> Arc<dyn FlmBodyParser> {
        self
    }
}

impl FlmBodyParser for FlmVerbatimBehavior {
    fn identity(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put(&mut out, "verbatim");
        out
    }

    fn description(&self) -> String {
        String::from("raw text up to `\\end{name}`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(kind: ArgStdKind) -> Arc<dyn FlmArgumentParser> {
        kind.into_flm_argument_parser()
    }

    fn terminated(terminator: &str, repeat_char: Option<char>, raw: bool) -> ArgStdKind {
        ArgStdKind::Terminated {
            terminator: String::from(terminator),
            repeat_char,
            opener: String::new(),
            head: String::new(),
            raw,
        }
    }

    #[test]
    fn every_standard_kind_answers_a_distinct_identity() {
        let kinds = [
            ArgStdKind::Mandatory,
            ArgStdKind::Optional,
            ArgStdKind::Star,
            ArgStdKind::KeyVals,
            ArgStdKind::CharsGroup,
            ArgStdKind::Verbatim { delimiters: None },
            ArgStdKind::Verbatim { delimiters: Some(('{', '}')) },
            ArgStdKind::Verbatim { delimiters: Some(('|', '|')) },
            ArgStdKind::TackOn { fields: Vec::new() },
            ArgStdKind::TackOn {
                fields: vec![TackOnField::new("label", vec![ArgDef::new(ArgStdKind::CharsGroup)])],
            },
            ArgStdKind::Marker { marker: '!' },
            ArgStdKind::Marker { marker: '+' },
            ArgStdKind::DelimitedGroup { open: '(', close: ')' },
            ArgStdKind::DelimitedGroup { open: '<', close: '>' },
            ArgStdKind::OptionalDelimitedGroup { open: '(', close: ')' },
            ArgStdKind::AnyDelimitedGroup,
            ArgStdKind::OptionalAnyDelimitedGroup,
            ArgStdKind::BracedOnly,
            ArgStdKind::Embellishments { markers: vec!['^'] },
            ArgStdKind::Embellishments { markers: vec!['^', '_'] },
            ArgStdKind::Embellishments { markers: vec!['_', '^'] },
            terminated("\\endphf", None, false),
            terminated("\\endphf", None, true),
            terminated("\\endph", None, false),
            terminated("`", Some('`'), true),
            terminated("`", Some('+'), true),
            terminated("", Some('`'), true),
            ArgStdKind::Terminated {
                terminator: String::from("]"),
                repeat_char: Some('='),
                opener: String::from("["),
                head: String::new(),
                raw: true,
            },
            ArgStdKind::Terminated {
                terminator: String::from("]"),
                repeat_char: Some('='),
                opener: String::new(),
                head: String::from("["),
                raw: true,
            },
        ];
        let identities: Vec<Vec<u8>> = kinds.into_iter().map(|kind| object(kind).identity()).collect();
        for (i, a) in identities.iter().enumerate() {
            for b in &identities[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn a_kind_with_a_code_reads_as_the_toolkit_s_table_of_codes_builds_it() {
        // The two verbatim codes, `v` and `v<c1><c2>`, are left out: they name
        // the library's own verbatim reader, so their parser is not the
        // toolkit's.
        let kinds = [
            ("m", ArgStdKind::Mandatory),
            ("o", ArgStdKind::Optional),
            ("s", ArgStdKind::Star),
            ("t!", ArgStdKind::Marker { marker: '!' }),
            ("r()", ArgStdKind::DelimitedGroup { open: '(', close: ')' }),
            ("d<>", ArgStdKind::OptionalDelimitedGroup { open: '<', close: '>' }),
            ("AnyDelimited", ArgStdKind::AnyDelimitedGroup),
            ("AnyDelimitedOptional", ArgStdKind::OptionalAnyDelimitedGroup),
            ("BracedOnly", ArgStdKind::BracedOnly),
            ("e{^_}", ArgStdKind::Embellishments { markers: vec!['^', '_'] }),
        ];
        for (code, kind) in kinds {
            let toolkit = argument_specs::<Flm, _>([code]).unwrap();
            assert_eq!(format!("{:?}", kind.reader()), format!("{:?}", toolkit[0].parser), "{code}");
        }
    }

    #[test]
    fn a_use_site_may_leave_out_exactly_the_optional_shapes() {
        assert!(object(ArgStdKind::Optional).can_match_empty());
        assert!(object(ArgStdKind::KeyVals).can_match_empty());
        assert!(object(ArgStdKind::Star).can_match_empty());
        assert!(!object(ArgStdKind::Mandatory).can_match_empty());
        assert!(!object(ArgStdKind::CharsGroup).can_match_empty());
        assert!(!object(ArgStdKind::Verbatim { delimiters: None }).can_match_empty());
        assert!(object(ArgStdKind::Marker { marker: '!' }).can_match_empty());
        assert!(object(ArgStdKind::OptionalDelimitedGroup { open: '<', close: '>' }).can_match_empty());
        assert!(object(ArgStdKind::OptionalAnyDelimitedGroup).can_match_empty());
        assert!(object(ArgStdKind::Embellishments { markers: vec!['^'] }).can_match_empty());
        assert!(!object(ArgStdKind::DelimitedGroup { open: '(', close: ')' }).can_match_empty());
        assert!(!object(ArgStdKind::AnyDelimitedGroup).can_match_empty());
        assert!(!object(ArgStdKind::BracedOnly).can_match_empty());
        assert!(!object(terminated("\\endphf", None, false)).can_match_empty());
        assert!(!object(terminated("`", Some('`'), true)).can_match_empty());
    }

    #[test]
    fn only_a_tack_on_argument_declares_fields() {
        let field = TackOnField::new("label", Vec::new());
        let tack_on = object(ArgStdKind::TackOn { fields: vec![field] });
        assert_eq!(tack_on.tack_on_fields().map(<[TackOnField]>::len), Some(1));
        assert!(object(ArgStdKind::Mandatory).tack_on_fields().is_none());
    }

    #[test]
    fn a_standard_kind_describes_itself() {
        assert_eq!(object(ArgStdKind::Optional).description(), "an optional `[…]` group");
        let field = TackOnField::new("label", Vec::new());
        assert_eq!(
            object(ArgStdKind::TackOn { fields: vec![field] }).description(),
            "the fields `\\label` written after the command"
        );
        assert_eq!(object(ArgStdKind::Star).definition_shape(), DefinitionShape::Value);
    }

    /// An optional `!` marker: a parser of the test's own, standing for the
    /// parser a feature defines for itself.
    #[derive(Debug)]
    struct Bang(MarkerArgumentParser);

    impl ArgumentParser<Flm> for Bang {
        fn parse_argument(
            &self,
            cx: &mut ParseContext<'_, '_, Flm>,
            spec: &ArgumentSpec<Flm>,
        ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
            self.0.parse_argument(cx, spec)
        }
        fn can_match_empty(&self) -> bool {
            true
        }
    }

    impl FlmArgumentParser for Bang {
        fn identity(&self) -> Vec<u8> {
            b"test.bang".to_vec()
        }
        fn definition_shape(&self) -> DefinitionShape {
            DefinitionShape::NoShape { token: "bang" }
        }
        fn description(&self) -> String {
            String::from("an optional `!` marker")
        }
    }

    #[test]
    fn only_a_standard_parser_tells_its_kind() {
        let kind = object(ArgStdKind::Marker { marker: '!' });
        assert!(matches!(kind.std_kind(), Some(ArgStdKind::Marker { marker: '!' })));
        let own: Arc<dyn FlmArgumentParser> =
            Arc::new(Bang(MarkerArgumentParser::new(String::from("!"))));
        assert!(own.std_kind().is_none());
    }

    #[test]
    fn a_terminated_kind_describes_itself_and_has_no_definition_shape() {
        assert_eq!(object(terminated("\\endphf", None, false)).description(), "content up to `\\endphf`");
        assert_eq!(
            object(terminated("`", Some('`'), true)).description(),
            "raw text after a run of `` ` ``, up to the same run then `` ` ``"
        );
        assert_eq!(
            object(terminated("", Some('+'), true)).description(),
            "raw text after a run of `+`, up to the same run"
        );
        let raw_string = ArgStdKind::Terminated {
            terminator: String::new(),
            repeat_char: Some('#'),
            opener: String::from("\""),
            head: String::from("\""),
            raw: true,
        };
        assert_eq!(
            object(raw_string.clone()).description(),
            "raw text after a run of `#` then `\"`, up to `\"` then the same run"
        );
        assert_eq!(
            object(raw_string).definition_shape(),
            DefinitionShape::NoShape { token: "terminated" }
        );
        assert_eq!(
            object(terminated("\\endphf", None, false)).definition_shape(),
            DefinitionShape::NoShape { token: "terminated" }
        );
    }

    #[test]
    fn a_code_span_holds_the_backticks_of_its_text() {
        assert_eq!(code_span("ab"), "`ab`");
        assert_eq!(code_span("`"), "`` ` ``");
        assert_eq!(code_span("a``b`"), "``` a``b` ```");
    }

    #[test]
    fn the_verbatim_body_has_an_identity_of_its_own() {
        let body = FlmVerbatimBehavior::new();
        assert_ne!(body.identity(), object(ArgStdKind::Verbatim { delimiters: None }).identity());
    }
}
