//! The declaration types a feature fills in to say what can be written in a
//! document: elements and their syntax, fallbacks, scoped packages and
//! overrides.
//!
//! A declaration is plain data. [`mint_packages`](super::mint_packages) builds
//! the techy packages and specifications that a parse uses from the
//! declarations (see the [module documentation](super) for this work, called
//! minting). A feature constructs its declarations itself and hands them to
//! its feature builder, which records the feature as their owner:
//!
//! - [`ElementDef`], an element and the syntax it is written in
//!   ([`SyntaxDef`]), passed to
//!   [`FeatureBuilder::element`](crate::core::registry::FeatureBuilder::element);
//!   a declaration is available in text mode unless it lists its parsing
//!   modes ([`ElementDef::modes`]);
//! - [`FallbackDef`], what a macro or an environment that no declaration
//!   defines means in given parsing modes, passed to
//!   [`FeatureBuilder::fallback`](crate::core::registry::FeatureBuilder::fallback);
//! - [`ScopedPackageDef`], a set of element declarations available only inside
//!   certain environment bodies and arguments, passed to
//!   [`FeatureBuilder::scoped_package`](crate::core::registry::FeatureBuilder::scoped_package);
//! - [`OverrideDef`], a feature's statement that its definition of a name
//!   replaces another feature's, made with
//!   [`FeatureBuilder::override_callable`](crate::core::registry::FeatureBuilder::override_callable).
//!
//! The two types to read first are [`SyntaxDef`], which says how an element is
//! written, and [`ArgDef`], which declares one argument: the parser that reads
//! it ([`FlmArgumentParser`], most often one of the standard kinds of
//! [`ArgStdKind`]) and what the declaration says around the parser (name,
//! parsing mode, layout, scoped packages, documentation). [`BodyDef`] does the
//! same for an environment's body. Settings given as data write an argument
//! declaration as a short text; see [`super::codes`].

use std::fmt;
use std::sync::Arc;

use techy::latexlike::Mode;

use crate::model::element::ElementSpec;
use crate::model::names::{FeatureName, Name};
use crate::lang::flm::FlmCallableType;
use crate::lang::mint::codes::ArgData;
use crate::lang::mint::parsers::{
    ArgStdKind, FlmArgumentParser, FlmBodyParser, IntoFlmArgumentParser, IntoFlmBodyParser,
};
use crate::lang::verbatim::FlmVerbatimBehavior;

/// A callable kind: one of the three forms in which an element can be written
/// in a document, a macro, an environment or a special character sequence.
///
/// [`SyntaxDef::kind`] returns it for a syntax; [`OverrideDef`] and
/// [`Symbol`](super::Symbol) use it to say which form a name belongs to. Its
/// `Display` text (`macro`, `environment`, `specials`) appears in error
/// messages and in the [registry digest](crate::model::digest) (the hash of
/// everything about a registry that can change what processing produces).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CallableKind {
    /// A macro: `\name{…}`.
    Macro,
    /// An environment: `\begin{name}…\end{name}`.
    Environment,
    /// A special character sequence, recognized wherever it appears rather than
    /// after an escape character (`~`, `---`).
    Specials,
}

impl CallableKind {
    /// Returns the callable type of the FLM language ([`FlmCallableType`])
    /// under which a name of this kind is registered in a techy package.
    pub fn callable_type(self) -> FlmCallableType {
        match self {
            CallableKind::Macro => FlmCallableType::Macro,
            CallableKind::Environment => FlmCallableType::Environment,
            CallableKind::Specials => FlmCallableType::Specials,
        }
    }
}

impl fmt::Display for CallableKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CallableKind::Macro => "macro",
            CallableKind::Environment => "environment",
            CallableKind::Specials => "specials",
        })
    }
}

/// The declaration of one **tack-on field**: a macro that a construct absorbs
/// as data after its own arguments, as `\section{…}` absorbs the `\label{…}`
/// written after it.
///
/// A construct accepts its tack-on fields through its tack-on argument, the
/// trailing argument declared as [`ArgStdKind::TackOn`] with the list of
/// fields. A field is written like a macro, but its name is not defined as a
/// command anywhere: only the construct that declares the field accepts it,
/// and only directly after that construct's other arguments.
///
/// A field is **data that the declaring construct reads**, not content: its
/// node carries the field element ([`FIELD`](super::FIELD)), the same element
/// name for every field, with the field's name as its value, and the construct that
/// declared the field is its only reader. The declaration therefore chooses no
/// element. See the [`field`](super::field) module and
/// [fields](crate::guide::concepts_overview#fields-data-a-construct-reads).
#[derive(Clone)]
pub struct TackOnField {
    /// The field's name, without the escape character (`label`).
    pub name: String,
    /// The field's own arguments, in the order they are written.
    pub args: Vec<ArgDef>,
    /// Whether the field may be written more than once after one construct.
    pub repeatable: bool,
    /// Documentation of the field. The library does not act on it; the
    /// registry keeps it with the declaration
    /// ([`FlmRegistry::declarations`](crate::core::registry::FlmRegistry::declarations))
    /// for a program that describes what an author may write.
    pub docs: String,
    /// The element the field's node carries: the field element, created from
    /// the name when the field is declared.
    element: Arc<ElementSpec>,
}

impl TackOnField {
    /// Declares a field written `\name`, taking the given arguments, which may
    /// be written at most once after the construct;
    /// [`TackOnField::repeatable`] allows it more than once. The field's
    /// element is created here with [`field_element`](super::field_element).
    pub fn new(name: impl Into<String>, args: Vec<ArgDef>) -> TackOnField {
        let name = name.into();
        let element = super::field::field_element(
            Name::from(name.as_str()),
            format!("The field `\\{name}`, which the construct it is written after reads as data."),
        );
        TackOnField { name, args, repeatable: false, docs: String::new(), element }
    }

    /// Builder: the field may be written more than once after one construct.
    pub fn repeatable(mut self) -> TackOnField {
        self.repeatable = true;
        self
    }

    /// Builder: sets the field's documentation.
    pub fn docs(mut self, docs: impl Into<String>) -> TackOnField {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Returns the element this field's nodes carry: the field element
    /// ([`FIELD`](super::FIELD)) whose value is this field's name.
    pub fn element(&self) -> &Arc<ElementSpec> {
        &self.element
    }

    /// Gives mutable access to the element, for the registry build, which
    /// replaces it with the element value that carries the aspects that
    /// features attached to the field element. Nothing else may change which element a
    /// field carries.
    pub(crate) fn element_mut(&mut self) -> &mut Arc<ElementSpec> {
        &mut self.element
    }
}

impl fmt::Debug for TackOnField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TackOnField")
            .field("name", &self.name)
            .field("args", &self.args)
            .field("repeatable", &self.repeatable)
            .finish()
    }
}

/// How the [block-structure pass](crate::lang::rewrites::blocks) lays out an
/// argument's content: as inline
/// content, as a region of blocks, or as whichever of the two the content
/// calls for.
///
/// Inline content is running text (words, formulas, commands such as
/// `\emph{…}`) with no paragraph inside it. A region of blocks holds
/// paragraphs and block-level constructs such as lists. The layout belongs to
/// the command's declaration ([`ArgDef::layout`]); an author cannot choose it
/// where the command is written.
///
/// Only an argument that holds document content
/// ([`FlmArgumentParser::holds_content`]; among the standard kinds, every
/// group of content, mandatory or optional, and embellishments) may take a
/// layout other than [`Inline`](ArgLayout::Inline). An argument read in
/// mathematics mode is an [opaque
/// region](crate::guide::concepts_overview#opaque-regions), which the
/// block-structure pass carries over untouched, so it may not be declared
/// [`Blocks`](ArgLayout::Blocks). Minting reports a declaration that breaks
/// either rule as
/// [`MintError::LayoutOnNonContentArgument`](super::MintError::LayoutOnNonContentArgument).
///
/// [`arg_layout_of`](super::arg_layout_of) returns the declared layout of an
/// argument of a parsed node;
/// [`argument_holds_blocks`](crate::lang::rewrites::blocks::argument_holds_blocks)
/// returns whether an argument holds blocks, before and after the
/// block-structure pass alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ArgLayout {
    /// Inline content: no paragraph is built inside it, and a blank line in it
    /// is an error, reported and read as one space. The default.
    #[default]
    Inline,
    /// A region of blocks: paragraphs are built inside it, exactly as in an
    /// environment body declared [`BodyLayout::Blocks`]. Content written on
    /// one line becomes one paragraph.
    Blocks,
    /// Inferred from the content: a region of blocks when the content holds a
    /// blank line (a paragraph break) or a block-level construct such as a
    /// list, inline content otherwise. This is the rule a fragment laid out
    /// [`InlineOrBlocks`](crate::model::doc::FragmentLayout::InlineOrBlocks)
    /// applies to its top level.
    InlineOrBlocks,
}

/// The declaration of one argument of a macro, an environment or a special
/// character sequence: the parser that reads it, the name it can be read by,
/// the parsing mode it is read in, how its content is laid out, the scoped
/// packages available inside it, documentation, and what it holds (its
/// role).
///
/// The parser is one field among the others ([`ArgDef::kind`]): one of the
/// standard argument kinds ([`ArgStdKind`]) for the forms FLM provides, or a
/// parser of a feature's own ([`FlmArgumentParser`]) for a syntax they cannot
/// express. Either way, minting applies the name, mode, layout and scoped
/// packages declared here around the parser.
///
/// Give an argument a name whenever anything reads it: reading by name keeps
/// working when the argument list changes later, reading by position does not.
///
/// An argument may also state its **role**: what the text written for it
/// holds (a label, a citation key, the path of a file), as a dotted name
/// that a feature registers in the registry's namespace of argument roles.
/// The role describes the argument for a reader of the registry; parsing and
/// processing do not read it. See [`ArgDef::role`].
///
/// [`ArgDef::new`], [`ArgDef::named`], [`ArgDef::mandatory`] and
/// [`ArgDef::optional`] create a declaration; the builder methods
/// [`mode`](ArgDef::mode), [`layout`](ArgDef::layout),
/// [`blocks`](ArgDef::blocks), [`scoped`](ArgDef::scoped),
/// [`docs`](ArgDef::docs) and [`role`](ArgDef::role) each change one thing.
/// Settings given as data write an argument as its argument code, or as a map
/// of the code with the argument's name, parsing mode, layout, scoped
/// packages, documentation and role ([`ArgData`], which converts into an
/// `ArgDef` through `TryFrom` and whose code
/// [`arg_def_from_code`](super::arg_def_from_code) reads).
#[derive(Debug, Clone, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(try_from = "ArgData")]
#[schemars(with = "ArgData")]
pub struct ArgDef {
    /// The name the argument can be read by, when it has one.
    pub name: Option<String>,
    /// The parser that reads the argument and tells FLM what it needs to know
    /// about the argument beyond reading it ([`FlmArgumentParser`]). It is a
    /// shared object, so cloning a declaration shares its parser.
    /// [`ArgDef::new`] and [`ArgDef::named`] build it from a standard kind or
    /// from a parser of one's own.
    pub kind: Arc<dyn FlmArgumentParser>,
    /// The parsing mode the argument is read in; `None` reads it in the mode
    /// the command itself is written in. See [`ArgDef::mode`].
    pub mode: Option<Mode>,
    /// How the [block-structure pass](crate::lang::rewrites::blocks) lays out
    /// the argument's content; inline by default. See [`ArgDef::layout`].
    pub layout: ArgLayout,
    /// The names of the [scoped packages](ScopedPackageDef) whose
    /// declarations are available inside this argument. See
    /// [`ArgDef::scoped`].
    pub scoped: Vec<Name>,
    /// Documentation of the argument. The library does not act on it; the
    /// registry keeps it with the declaration
    /// ([`FlmRegistry::declarations`](crate::core::registry::FlmRegistry::declarations))
    /// for a program that describes what an author may write.
    pub docs: String,
    /// The **argument role**: what the argument holds, beyond how it is
    /// written, as a registered dotted name such as `flm.refs.target` (the
    /// label a reference points to). `None`, the default, says nothing about
    /// what the argument holds. See [`ArgDef::role`].
    pub role: Option<Name>,
    /// The argument code the declaration was read from; see [`ArgDef::code`].
    pub(super) code: Option<String>,
}

impl ArgDef {
    /// Creates the declaration of an argument read by `kind`, with no name, so
    /// that it can be read only by position. Its content is inline
    /// ([`ArgLayout::Inline`]), it is read in the mode the command is written
    /// in, and no scoped package is available inside it.
    ///
    /// `kind` is a standard kind (`ArgDef::new(ArgStdKind::Star)`), a parser
    /// of one's own passed by value, or a parser already shared as
    /// `Arc<dyn FlmArgumentParser>`; [`IntoFlmArgumentParser`] converts all
    /// three.
    pub fn new(kind: impl IntoFlmArgumentParser) -> ArgDef {
        ArgDef {
            name: None,
            kind: kind.into_flm_argument_parser(),
            mode: None,
            layout: ArgLayout::Inline,
            scoped: Vec::new(),
            docs: String::new(),
            role: None,
            code: None,
        }
    }

    /// The argument code this declaration was read from
    /// ([`arg_def_from_code`](super::arg_def_from_code), and every reader of
    /// settings given as data, which reads its codes through it): `"["`,
    /// `"r()"`. `None` for a declaration built from a standard kind or a
    /// parser ([`ArgDef::new`] and the constructors built on it).
    ///
    /// A program that writes a declaration back as data writes this code,
    /// so that the declaration reads back as it was written: the parser
    /// object alone does not say which code chose it (`{` and `m` choose the
    /// same parser). The builder methods leave the code as it is; a caller
    /// that replaces [`kind`](ArgDef::kind) after reading a code makes the
    /// code describe a parser the declaration no longer holds.
    ///
    /// ```
    /// use flm_core::lang::mint::{arg_def_from_code, ArgDef, ArgStdKind};
    ///
    /// assert_eq!(arg_def_from_code("m").unwrap().code(), Some("m"));
    /// assert_eq!(ArgDef::new(ArgStdKind::Mandatory).code(), None);
    /// ```
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Creates the declaration of an argument read by `kind`, which can be
    /// read by `name`. Everything else is as for [`ArgDef::new`].
    pub fn named(name: impl Into<String>, kind: impl IntoFlmArgumentParser) -> ArgDef {
        ArgDef { name: Some(name.into()), ..ArgDef::new(kind) }
    }

    /// Creates the declaration of a mandatory `{…}` argument named `name`;
    /// shorthand for `ArgDef::named(name, ArgStdKind::Mandatory)`.
    pub fn mandatory(name: impl Into<String>) -> ArgDef {
        ArgDef::named(name, ArgStdKind::Mandatory)
    }

    /// Creates the declaration of an optional `[…]` argument named `name`;
    /// shorthand for `ArgDef::named(name, ArgStdKind::Optional)`.
    pub fn optional(name: impl Into<String>) -> ArgDef {
        ArgDef::named(name, ArgStdKind::Optional)
    }

    /// Builder: sets the parsing mode the argument is read in. Without it,
    /// the argument is read in the mode the command is written in.
    ///
    /// `Mode::Math` reads the argument as mathematics wherever the command is
    /// written. `Mode::Text` restores the innermost non-mathematics context
    /// that encloses the command, with its mode and its token rules; this is
    /// how `\text{…}` inside mathematics holds ordinary document content.
    /// Outside mathematics, `Mode::Text` changes nothing. The two values mean
    /// the same on an environment's body ([`BodyDef::mode`]).
    pub fn mode(mut self, mode: Mode) -> ArgDef {
        self.mode = Some(mode);
        self
    }

    /// Builder: sets how the [block-structure
    /// pass](crate::lang::rewrites::blocks) lays out the argument's content: inline
    /// content (the default), a region of blocks, or whichever of the two the content
    /// calls for. See [`ArgLayout`].
    ///
    /// Only an argument that holds document content
    /// ([`FlmArgumentParser::holds_content`]) may take a layout other than
    /// [`ArgLayout::Inline`], and an argument read in mathematics mode may not
    /// take [`ArgLayout::Blocks`]. Minting reports a declaration that breaks
    /// either rule as
    /// [`MintError::LayoutOnNonContentArgument`](super::MintError::LayoutOnNonContentArgument)
    /// and lays the argument out inline.
    ///
    /// ```
    /// use flm_core::lang::mint::{ArgDef, ArgLayout};
    ///
    /// // A note whose text may be one line of running text or several
    /// // paragraphs, as the author writes it.
    /// let text = ArgDef::mandatory("text").layout(ArgLayout::InlineOrBlocks);
    /// assert_eq!(text.layout, ArgLayout::InlineOrBlocks);
    /// ```
    pub fn layout(mut self, layout: ArgLayout) -> ArgDef {
        self.layout = layout;
        self
    }

    /// Builder: the argument's content is a region of blocks; shorthand for
    /// `.layout(ArgLayout::Blocks)`.
    pub fn blocks(self) -> ArgDef {
        self.layout(ArgLayout::Blocks)
    }

    /// Builder: makes the declarations of one more [scoped
    /// package](ScopedPackageDef), given by name, available inside this
    /// argument, as [`BodyDef::scoped`] does for an environment's body.
    ///
    /// This is how a command gives one of its arguments commands of its own:
    /// a note whose optional argument accepts `\label{…}`, while `\label` is
    /// not defined in the running text. While the argument is parsed, the
    /// scoped package is the innermost techy package of the parsing state's
    /// scope stack, so its declarations shadow any other definition of the
    /// same names inside the argument.
    ///
    /// Some feature must declare a scoped package of that name; the order in
    /// which the features register does not matter. A name that no feature
    /// declares is reported as
    /// [`MintError::UnknownScopedPackage`](super::MintError::UnknownScopedPackage),
    /// unless the feature that refers to it declared the name as a peer
    /// ([`FeatureBuilder::peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package)).
    /// A package that another feature owns may be absent from a registry; the
    /// argument then makes no command available through it, and a command
    /// the package would define is an unknown command there. Scoped packages
    /// that refer to each other in a cycle are reported as
    /// [`MintError::ScopedPackageCycle`](super::MintError::ScopedPackageCycle).
    /// Only an argument that holds document content
    /// ([`FlmArgumentParser::holds_content`]) may use a scoped package; on
    /// any other argument, minting reports the declaration as
    /// [`MintError::ScopeOnNonContentArgument`](super::MintError::ScopeOnNonContentArgument)
    /// and does not use the package.
    ///
    /// ```
    /// use flm_core::lang::mint::ArgDef;
    ///
    /// let labels = ArgDef::optional("labels").scoped("my.labels");
    /// assert_eq!(labels.scoped, ["my.labels"]);
    /// ```
    pub fn scoped(mut self, package: impl Into<Name>) -> ArgDef {
        self.scoped.push(package.into());
        self
    }

    /// Builder: sets the argument's documentation.
    pub fn docs(mut self, docs: impl Into<String>) -> ArgDef {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Builder: sets the argument's **role**, the registered name of what the
    /// argument holds, replacing any role set before.
    ///
    /// The syntax of an argument says how it is written (a group of
    /// characters, raw text up to a closing brace); its role says what the
    /// written text is: a label, a citation key, the path of an image file. A
    /// program that describes the language reads the role, for example to
    /// show that the argument of `\ref` and the argument of `\eqref` hold the
    /// same kind of thing. Parsing and processing do not read it, and the
    /// role is not an input of the [registry digest](crate::model::digest).
    ///
    /// A role is an entity of the registry, in a namespace of its own, the
    /// *argument roles*. A feature declares a role with
    /// [`FeatureBuilder::argument_role`](crate::core::registry::FeatureBuilder::argument_role),
    /// giving its name and a description. A feature may set a role that
    /// another feature declares; when that feature may be absent from a
    /// registry, the feature using the role declares it as a peer
    /// ([`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role)).
    /// When the registry is built, a role that no feature declares, and that
    /// the feature setting it did not declare as a peer, is refused as a
    /// [`BuildError::UndeclaredArgumentRole`](crate::core::registry::BuildError::UndeclaredArgumentRole).
    ///
    /// ```
    /// use flm_core::lang::mint::{ArgDef, ArgStdKind};
    ///
    /// let target = ArgDef::named("target", ArgStdKind::CharsGroup).role("flm.refs.target");
    /// assert_eq!(target.role.as_ref().map(|name| name.as_str()), Some("flm.refs.target"));
    /// assert!(ArgDef::mandatory("text").role.is_none());
    /// ```
    // See decision rationale at [§flm-dd-dr:arg-role].
    pub fn role(mut self, name: impl Into<Name>) -> ArgDef {
        self.role = Some(name.into());
        self
    }

    /// Replaces the element of each [tack-on field](TackOnField) this
    /// argument declares by the element `remap` returns for it, and keeps a
    /// field's element when `remap` returns `None`. The registry build uses it
    /// to put onto the declaration the element value that carries the
    /// aspects that features attached to the field element. An argument that
    /// declares no field is left unchanged.
    ///
    /// The parser object is shared, so it is not changed in place: when a
    /// field's element changes, the argument gets a new tack-on parser holding
    /// the changed fields.
    pub(crate) fn remap_field_elements(
        &mut self,
        remap: impl Fn(&Arc<ElementSpec>) -> Option<Arc<ElementSpec>>,
    ) {
        let Some(fields) = self.kind.tack_on_fields() else { return };
        if fields.iter().all(|field| remap(field.element()).is_none()) {
            return;
        }
        let mut fields = fields.to_vec();
        for field in &mut fields {
            if let Some(element) = remap(field.element()) {
                *field.element_mut() = element;
            }
        }
        self.kind = ArgStdKind::TackOn { fields }.into_flm_argument_parser();
    }
}

/// How the [block-structure pass](crate::lang::rewrites::blocks) lays out an
/// environment's body: as inline content, as a region of blocks, or according
/// to the element's block role.
///
/// [`Inline`](BodyLayout::Inline) and [`Blocks`](BodyLayout::Blocks) take
/// precedence over the element's block role; [`FromRole`](BodyLayout::FromRole)
/// follows it. A
/// body that does not hold document content ([`BodyDef::holds_content`]) or
/// that is read in mathematics mode is an [opaque
/// region](crate::guide::concepts_overview#opaque-regions): the
/// block-structure pass carries it over untouched, regardless of its layout.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BodyLayout {
    /// The body is inline content: no paragraph is built inside it, and a blank
    /// line in it is an error.
    Inline,
    /// The body is a region of blocks: paragraphs are built inside it.
    Blocks,
    /// The body follows the element's block role: a region of blocks for an
    /// element whose role is [`BlockRole::Block`](crate::model::aspects::BlockRole::Block),
    /// inline content otherwise. This is the default, so that a body's layout
    /// follows from what the element is.
    #[default]
    FromRole,
}

/// The declaration of an environment's body: how it is laid out, which parser
/// reads it, which parsing mode it is read in, and which scoped packages are
/// available inside it.
///
/// [`BodyDef::new`] (the same as `BodyDef::default()`) declares a body laid
/// out by the element's block role, read the ordinary way, read in the mode
/// the environment is written in, and with no scoped package; each builder
/// method changes one of these.
///
/// The body parser is one field among the others ([`BodyDef::parser`]): a body
/// that is not read the ordinary way (raw text, or content a feature checks as
/// it reads it) holds the object that reads it, and minting applies the
/// layout, mode and scoped packages declared here around that object.
#[derive(Debug, Clone, Default)]
pub struct BodyDef {
    /// How the [block-structure pass](crate::lang::rewrites::blocks) lays out
    /// the body.
    pub layout: BodyLayout,
    /// The parser that reads the body, when the body is not read the ordinary
    /// way (content up to and including `\end{name}`). See
    /// [`BodyDef::parser`].
    pub parser: Option<Arc<dyn FlmBodyParser>>,
    /// The parsing mode the body is read in: `Some(Mode::Math)` for an
    /// equation-like environment, `Some(Mode::Text)` to restore the
    /// non-mathematics context that encloses the mathematics, `None` to read
    /// the body in the mode the environment is written in.
    pub mode: Option<Mode>,
    /// The names of the [scoped packages](ScopedPackageDef) whose
    /// declarations are available inside this body, such as the package that
    /// defines `\item` inside a list environment. See [`BodyDef::scoped`].
    pub scoped: Vec<Name>,
}

impl BodyDef {
    /// Creates the declaration of a body read the ordinary way, laid out by
    /// the element's block role ([`BodyLayout::FromRole`]), read in the mode
    /// the environment is written in, with no scoped package. The same as
    /// `BodyDef::default()`.
    pub fn new() -> BodyDef {
        BodyDef::default()
    }

    /// Builder: sets how the body is laid out.
    pub fn layout(mut self, layout: BodyLayout) -> BodyDef {
        self.layout = layout;
        self
    }

    /// Builder: the body is a region of blocks; shorthand for
    /// `.layout(BodyLayout::Blocks)`.
    pub fn blocks(self) -> BodyDef {
        self.layout(BodyLayout::Blocks)
    }

    /// Builder: the body is raw text, read by FLM's verbatim reader
    /// ([`crate::lang::verbatim`]), which places the content under a group of
    /// the verbatim class; shorthand for
    /// `.parser(FlmVerbatimBehavior::new())`.
    pub fn verbatim(self) -> BodyDef {
        self.parser(FlmVerbatimBehavior::new())
    }

    /// Builder: sets the parser that reads the body ([`FlmBodyParser`]), for
    /// a body not read the ordinary way: FLM's verbatim reader
    /// ([`BodyDef::verbatim`]), or a feature's own parser, such as one that
    /// reads the ordinary content and then checks it.
    ///
    /// Only the parser's reading of the body is used: the environment's
    /// arguments are those of the declaration, and the body is read under the
    /// mode and the scoped packages declared here. A body whose parser does
    /// not hold document content ([`FlmBodyParser::holds_content`]) may
    /// declare neither a layout other than [`BodyLayout::FromRole`] nor a
    /// scoped package; minting reports either declaration
    /// ([`MintError::LayoutOnNonContentBody`](super::MintError::LayoutOnNonContentBody),
    /// [`MintError::ScopeOnNonContentBody`](super::MintError::ScopeOnNonContentBody))
    /// and uses no scoped package.
    ///
    /// `parser` is a parser passed by value, or one already shared as
    /// `Arc<dyn FlmBodyParser>` by several declarations; [`IntoFlmBodyParser`]
    /// converts both.
    pub fn parser(mut self, parser: impl IntoFlmBodyParser) -> BodyDef {
        self.parser = Some(parser.into_flm_body_parser());
        self
    }

    /// Returns whether the body holds document content: what its parser's
    /// [`FlmBodyParser::holds_content`] returns, and `true` for a body read
    /// the ordinary way. The block-structure pass carries a body that does not
    /// hold content (raw text) over untouched, as an opaque region.
    pub fn holds_content(&self) -> bool {
        self.parser.as_ref().is_none_or(|parser| parser.holds_content())
    }

    /// Builder: sets the parsing mode the body is read in. `Mode::Math` and
    /// `Mode::Text` mean what they mean for an argument ([`ArgDef::mode`]).
    pub fn mode(mut self, mode: Mode) -> BodyDef {
        self.mode = Some(mode);
        self
    }

    /// Builder: makes the declarations of one more scoped package, given by
    /// name, available inside the body. Some feature must declare a
    /// [`ScopedPackageDef`] of that name; the rules are those of
    /// [`ArgDef::scoped`].
    pub fn scoped(mut self, package: impl Into<Name>) -> BodyDef {
        self.scoped.push(package.into());
        self
    }
}

/// How an element is written in the source: as a macro, an environment, a
/// file-inclusion macro, a special character sequence, or not at all.
///
/// Each form holds its list of argument declarations ([`ArgDef`]), and an
/// environment also holds a body declaration ([`BodyDef`]). A syntax the
/// standard argument kinds cannot express is still one of these forms: its
/// argument, or its body, holds a parser of the feature's own
/// ([`FlmArgumentParser`], [`FlmBodyParser`]), with its name, layout,
/// documentation, mode and scoped packages declared like any other. Only these
/// declarations reach techy: minting builds every specification from them.
#[derive(Debug, Clone)]
pub enum SyntaxDef {
    /// A macro taking the given arguments, registered under each of the
    /// declaration's names.
    Macro {
        /// The arguments, in the order they are written.
        args: Vec<ArgDef>,
    },
    /// An environment taking the given arguments, written after `\begin{name}`,
    /// and the given body; registered under each of the declaration's names.
    Environment {
        /// The arguments, in the order they are written.
        args: Vec<ArgDef>,
        /// What the body is.
        body: BodyDef,
    },
    /// A macro that brings the content of another file into the document,
    /// written `\input{reference}`. One of the given arguments must be named
    /// [`REFERENCE_ARGUMENT`](crate::lang::input::REFERENCE_ARGUMENT); it holds
    /// the reference as plain characters, and minting reports a declaration
    /// without it as
    /// [`MintError::NoReferenceArgument`](super::MintError::NoReferenceArgument).
    /// The macro is registered under each of the declaration's names and
    /// parsed by the language's own inclusion parser ([`crate::lang::input`]),
    /// which asks the [input resolver](crate::lang::InputResolver) on the
    /// [parse driver](crate::lang::FlmDriver) for the file and parses the
    /// file's content where the command is written.
    Inclusion {
        /// The arguments, in the order they are written.
        args: Vec<ArgDef>,
    },
    /// A special character sequence with the given trigger and arguments. The
    /// declaration lists no name: the trigger is how the sequence is written.
    Specials {
        /// The character sequence that triggers it.
        trigger: String,
        /// The arguments, in the order they are written.
        args: Vec<ArgDef>,
    },
    /// No syntax: the element is never parsed, and only a rewrite or a
    /// lowering stage creates nodes of it, such as a heading's number or a
    /// pointer to another node's argument. The declaration lists no name.
    Synthesized,
}

impl SyntaxDef {
    /// Returns the callable kind this syntax is registered under (a
    /// file-inclusion macro is a macro); `None` for
    /// [`SyntaxDef::Synthesized`], which is not registered.
    pub fn kind(&self) -> Option<CallableKind> {
        match self {
            SyntaxDef::Macro { .. } | SyntaxDef::Inclusion { .. } => Some(CallableKind::Macro),
            SyntaxDef::Environment { .. } => Some(CallableKind::Environment),
            SyntaxDef::Specials { .. } => Some(CallableKind::Specials),
            SyntaxDef::Synthesized => None,
        }
    }

    /// Returns the argument declarations of this syntax, in the order they are
    /// written; empty for [`SyntaxDef::Synthesized`], which takes none.
    ///
    /// ```
    /// use flm_core::lang::mint::{ArgDef, SyntaxDef};
    ///
    /// let syntax = SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] };
    /// assert_eq!(syntax.args().len(), 1);
    /// assert!(SyntaxDef::Synthesized.args().is_empty());
    /// ```
    pub fn args(&self) -> &[ArgDef] {
        match self {
            SyntaxDef::Macro { args }
            | SyntaxDef::Environment { args, .. }
            | SyntaxDef::Inclusion { args }
            | SyntaxDef::Specials { args, .. } => args,
            SyntaxDef::Synthesized => &[],
        }
    }
}

/// An element declaration: the plain data a feature gives for an element, the
/// syntax it is written in (macro, environment or special character sequence),
/// its arguments (each with a parser, a layout, a parsing mode and the scoped
/// packages available inside it) and its body.
///
/// Besides the syntax ([`SyntaxDef`]), the declaration holds the feature that
/// declares it, the names the element is written under, the element value its
/// nodes carry, the parsing modes it is available in, and documentation. A
/// feature passes it to
/// [`FeatureBuilder::element`](crate::core::registry::FeatureBuilder::element),
/// or places it in a [`ScopedPackageDef`].
///
/// A declaration is available in text mode unless it lists its parsing modes
/// with [`ElementDef::modes`]: a feature that defines a command of mathematics
/// lists `[Mode::Math]`, and one that defines a command for both modes lists
/// both. Two declarations of one name whose modes do not overlap never clash,
/// whether they come from one feature or from two: a text-mode `\textbf` and
/// a mathematics-mode `\textbf` are two different commands. A declaration
/// with no syntax ([`SyntaxDef::Synthesized`]) carries the field and ignores
/// it.
#[derive(Clone)]
pub struct ElementDef {
    /// The feature that declares the element. Error messages identify the
    /// declaration by it, and the symbol table
    /// ([`MintedProviders::symbols`](super::MintedProviders::symbols)) records
    /// it. The feature builder sets it to the feature being built, so a
    /// feature need not set it; a caller of
    /// [`build_language`](crate::lang::build_language) that builds the
    /// declarations by hand sets it here.
    pub owner: FeatureName,
    /// The names this element is written under, without the escape
    /// character. Several names may share one element value: the headings
    /// feature, for instance, declares all the commands of one heading kind
    /// under one element value. Empty for a special
    /// character sequence and for an element with no syntax
    /// ([`SyntaxDef::Synthesized`]).
    pub names: Vec<Name>,
    /// How the element is written.
    pub syntax: SyntaxDef,
    /// The element value every node of this declaration carries.
    pub element: Arc<ElementSpec>,
    /// Documentation of the declaration. The library does not act on it; the
    /// registry keeps it with the declaration
    /// ([`FlmRegistry::declarations`](crate::core::registry::FlmRegistry::declarations))
    /// for a program that describes what an author may write.
    pub docs: String,
    /// The parsing modes the declaration is available in: `[Mode::Text]`
    /// unless the feature lists others with [`ElementDef::modes`]. Ignored for
    /// [`SyntaxDef::Synthesized`], which no document writes.
    pub modes: Vec<Mode>,
}

impl ElementDef {
    /// Creates the declaration, by `owner`, that `element` is written as
    /// `syntax` under `names`, with no documentation and available in text
    /// mode only.
    pub fn new(
        owner: impl Into<FeatureName>,
        names: impl IntoIterator<Item = impl Into<Name>>,
        syntax: SyntaxDef,
        element: Arc<ElementSpec>,
    ) -> ElementDef {
        ElementDef {
            owner: owner.into(),
            names: names.into_iter().map(Into::into).collect(),
            syntax,
            element,
            docs: String::new(),
            modes: vec![Mode::Text],
        }
    }

    /// Builder: sets the declaration's documentation.
    pub fn docs(mut self, docs: impl Into<String>) -> ElementDef {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Builder: sets the parsing modes the declaration is available in,
    /// replacing the default, text mode. A feature that defines a command of
    /// mathematics passes `[Mode::Math]`; one that defines a command for both
    /// modes passes `[Mode::Text, Mode::Math]`.
    pub fn modes(mut self, modes: impl IntoIterator<Item = Mode>) -> ElementDef {
        self.modes = modes.into_iter().collect();
        self
    }
}

impl fmt::Debug for ElementDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementDef")
            .field("owner", &self.owner)
            .field("names", &self.names)
            .field("syntax", &self.syntax)
            .field("element", &self.element.name)
            .field("modes", &self.modes)
            .finish()
    }
}

/// The callable kinds a fallback can be declared for: macros and
/// environments.
///
/// There is no fallback for special character sequences: a sequence of
/// characters that no declaration defines is not a special character
/// sequence, and is read as ordinary text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FallbackKind {
    /// Macros that no declaration defines.
    Macro,
    /// Environments that no declaration defines.
    Environment,
}

impl FallbackKind {
    /// Returns the callable kind ([`CallableKind`]) this fallback kind
    /// applies to.
    pub fn callable_kind(self) -> CallableKind {
        match self {
            FallbackKind::Macro => CallableKind::Macro,
            FallbackKind::Environment => CallableKind::Environment,
        }
    }

    /// Returns the callable type of the FLM language ([`FlmCallableType`])
    /// this fallback kind applies to.
    pub fn callable_type(self) -> FlmCallableType {
        self.callable_kind().callable_type()
    }
}

impl fmt::Display for FallbackKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FallbackKind::Macro => "macro",
            FallbackKind::Environment => "environment",
        })
    }
}

/// A fallback: a feature's declaration of what a macro or an environment that
/// no declaration defines means, in the parsing modes the fallback lists.
///
/// In those modes, such a command of the fallback's kind
/// ([`kind`](Self::kind)) resolves to a specification that takes no argument
/// and carries `element`, and the parse reports nothing. A mathematics feature
/// uses a fallback to accept any command name inside `\(…\)` as content for
/// the mathematics renderer, while text mode still reports an unknown command.
///
/// Only one fallback may apply to one kind in one parsing mode across a
/// registry: a later fallback that overlaps an earlier one is reported as a
/// [`MintError::Clash`](super::MintError::Clash) and not installed. The
/// language has its own two fallbacks, consulted after every fallback a
/// feature declares, which apply in every mode and report the unknown command;
/// their nodes carry [`UNKNOWN_MACRO`](crate::lang::structural::UNKNOWN_MACRO) or
/// [`UNKNOWN_ENVIRONMENT`](crate::lang::structural::UNKNOWN_ENVIRONMENT). A feature
/// declares a fallback with
/// [`FeatureBuilder::fallback`](crate::core::registry::FeatureBuilder::fallback);
/// [`FlmFallbackProvider`](super::FlmFallbackProvider) applies them.
#[derive(Clone)]
pub struct FallbackDef {
    /// The feature that declares the fallback; the feature builder sets it.
    pub owner: FeatureName,
    /// The parsing modes in which the fallback applies.
    pub modes: Vec<Mode>,
    /// The callable kind the fallback applies to.
    pub kind: FallbackKind,
    /// The element that the node of such a command carries.
    pub element: Arc<ElementSpec>,
}

impl fmt::Debug for FallbackDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FallbackDef")
            .field("owner", &self.owner)
            .field("modes", &self.modes)
            .field("kind", &self.kind)
            .field("element", &self.element.name)
            .finish()
    }
}

/// A scoped package: a set of element declarations that can be written only
/// inside the environment bodies and arguments whose declarations refer to the
/// package by name, for instance `\item` inside a list's body or `\caption`
/// inside a float's body.
///
/// A body refers to a scoped package with [`BodyDef::scoped`], an argument
/// with [`ArgDef::scoped`]. While such a body or argument is parsed, the
/// package's declarations are available there and shadow any other definition
/// of the same names; outside, they do not exist.
///
/// Minting builds one techy package per scoped package. That package is not
/// on the document's scope stack: it is added as the innermost techy package
/// only while a body or an argument that refers to it is parsed. Minting never
/// checks the declarations of a scoped package for clashes with those of
/// another scoped package or of a feature; only two scoped packages of one
/// name clash ([`ClashKind::ScopedPackage`](super::ClashKind::ScopedPackage)).
///
/// A feature declares a scoped package with
/// [`FeatureBuilder::scoped_package`](crate::core::registry::FeatureBuilder::scoped_package).
/// A body or an argument may refer to a scoped package that another feature
/// declares. The referring feature then declares the name as a peer
/// ([`FeatureBuilder::peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package)),
/// so that a registry without the owning feature still builds: the referring
/// bodies and arguments then make no command available through the package.
/// See also [scoped packages](crate::guide::concepts_overview#scoped-packages).
#[derive(Clone)]
pub struct ScopedPackageDef {
    /// The feature that declares the package; the feature builder sets it,
    /// and sets it on each of the package's element declarations too.
    pub owner: FeatureName,
    /// The name by which a body or an argument refers to the package. It must
    /// be unique across the registry: minting reports a second package of the
    /// same name as a [`MintError::Clash`](super::MintError::Clash) and builds
    /// only the first.
    pub name: Name,
    /// The element declarations the package holds.
    pub elements: Vec<ElementDef>,
    /// Documentation of the package. The library does not act on it; the
    /// registry keeps it with the declaration
    /// ([`FlmRegistry::declarations`](crate::core::registry::FlmRegistry::declarations))
    /// for a program that describes what an author may write.
    pub docs: String,
}

impl fmt::Debug for ScopedPackageDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedPackageDef")
            .field("owner", &self.owner)
            .field("name", &self.name)
            .field("elements", &self.elements)
            .finish()
    }
}

/// An override: the statement, by the feature `owner`, that its definition of
/// `name` replaces the definitions other features give that name.
///
/// Registration order decides which of several definitions of one name
/// applies, because a later feature's techy package is innermost on the scope
/// stack. Two features that define one name in one parsing mode are reported
/// as a [`MintError::Clash`](super::MintError::Clash), unless the feature
/// order places the feature registered last explicitly after the others
/// ([`crate::core::registry::order`]), or that feature declares an override.
/// An override thus prevents that report in each mode where `owner` is the
/// feature registered last among those that define the name; elsewhere it is
/// not consulted and has no effect. A feature declares an override with
/// [`FeatureBuilder::override_callable`](crate::core::registry::FeatureBuilder::override_callable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideDef {
    /// The feature whose definition replaces the others.
    pub owner: FeatureName,
    /// The callable kind of the redefined name.
    pub kind: CallableKind,
    /// The redefined name, without the escape character, or the trigger of a
    /// special character sequence.
    pub name: Name,
}
