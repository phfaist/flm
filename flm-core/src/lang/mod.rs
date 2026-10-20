//! The FLM language: the language type techy's parser runs with, the parse
//! entry points, and everything a parse needs.
//!
//! techy, the parser toolkit FLM is built on, supplies the tokenizer, the parser
//! and the node tree. It is generic over a *language*: a set of types that says
//! which kinds of groups and callables exist, which data is attached to parsing
//! states and nodes, and which behavior runs during a parse. This module defines
//! FLM's language, builds it from what the [features](crate::core::registry::FlmFeature)
//! declare, and parses sources with it.
//!
//! The intended entry points are two functions. [`build_language`] turns the
//! features' declarations ([`LangInputs`]) into a [`BuiltLang`]; the registry
//! calls it once when it is built, and a caller usually takes the result from
//! [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang).
//! [`parse_fragment`] parses one source under a [`BuiltLang`] and returns a
//! [`Fragment`](crate::model::doc::Fragment): the tree, its source and the
//! diagnostics of the parse. A document of several files is parsed through
//! [`crate::model::assemble`], which uses the same machinery.
//!
//! This module depends only on the lower layers of the crate ([`crate::model`]:
//! names, elements, aspects, the document); the registry, the pipeline and the
//! standard features are built on top of it.
//!
//! # The submodules and what each holds
//!
//! - [`flm`] — the language type [`Flm`], the group classes
//!   ([`FlmGroupType`]) and the callable forms ([`FlmCallableType`]); [`Flm`]
//!   gives every node its element.
//! - [`origin`] — [`FlmOrigin`], where a source comes from, and the aliases
//!   [`FlmSource`], [`FlmSpan`], [`FlmDiagnostic`] and [`FlmDiagnostics`].
//! - [`ext`] — the data FLM attaches to parsing states ([`FlmStateExt`]) and to
//!   nodes ([`FlmNodeExt`]), and the typed payload of a node ([`PresPayload`]).
//! - [`structural`] — the structural elements ([`StructuralElements`]): the
//!   elements of the node shapes that no feature declares.
//! - [`rules`] — [`FlmParsing`], the parsing settings a document may set, and
//!   their translation into techy's tokenizer rules.
//! - [`driver`] — [`FlmDriver`], the object holding the behavior and the
//!   settings of a running parse.
//! - [`mint`] — the element declarations a feature contributes, and minting:
//!   building from them the techy packages and specifications a parse resolves
//!   names against.
//! - [`chars_group`] and [`verbatim`] — the parsers of the two restricted
//!   argument shapes: arguments read as plain characters (`\label{…}`) and raw
//!   text (`\verbcode{…}`, verbatim environments).
//! - [`build`] — [`build_language`], which assembles all of the above into a
//!   [`BuiltLang`].
//! - [`parse`] — [`parse_fragment`] and the ways a parse can fail
//!   ([`FragmentError`]).
//! - [`input`] — the command `\input`: what the library asks of the embedding
//!   program to read another file ([`InputResolver`]), and the parser of the
//!   command.
//! - [`guard`] — the recursion guard and the worker threads every recursive
//!   operation runs on.
//! - [`copy`] — building a [synthesized
//!   value](crate::guide::concepts_overview#synthesized-values), a small tree
//!   held outside the document, from nodes of an existing tree.
//! - [`text`] — reading the characters of content: its plain text, or the
//!   characters of an argument that holds nothing else.
//! - [`rewrites`] — the three rewrite passes: splicing included files,
//!   expanding user definitions, and laying content out in paragraphs and
//!   blocks.
//! - [`serialize`] — storing an FLM parse in techy's serialized form and reading
//!   it back.
//!
//! # Guarantees that hold across the module
//!
//! - **Every node has an element, and finding it needs no name lookup.** A
//!   callable node's specification is always one of the wrapper types that
//!   [`mint`] builds, each carrying its element, and every other node shape
//!   takes its element from the [`StructuralElements`] the parsing state
//!   carries. See [structural
//!   elements](crate::guide::concepts_overview#structural-elements).
//! - **Every recursive operation is guarded and runs on a worker thread.**
//!   Deeply nested input is refused with a diagnostic instead of overflowing the
//!   stack; see [the recursion guard and worker
//!   threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads).
//! - **Raw text is always wrapped.** Every run of verbatim content is a child
//!   of a group node of the verbatim class, so whether content is verbatim is
//!   one question about a node anywhere in the library. See [verbatim
//!   regions](crate::guide::concepts_overview#verbatim-regions).
//! - **A parse is tolerant.** A problem in the source becomes a
//!   [diagnostic](crate::guide::concepts_overview#diagnostics) attached to the
//!   fragment, and the tree still covers the whole input; [`FragmentError`]
//!   lists the only cases with no tree.

pub mod build;
pub mod chars_group;
pub mod copy;
pub mod driver;
pub mod ext;
pub mod flm;
pub mod guard;
pub mod input;
pub mod mint;
pub mod origin;
pub mod parse;
pub mod rewrites;
pub mod rules;
pub mod serialize;
pub mod structural;
pub mod text;
pub mod verbatim;

pub use build::{build_language, BuiltLang, LangBuildError, LangInputs};
pub use driver::{DollarMathDisabled, FlmDriver, PercentForbidden};
pub use ext::{
    FlmNodeExt, FlmStateExt, PresPayload, TemplateCommandRule, TemplateInteriorRules,
    TextSourceMap, TextSourcePiece, RestrictedInterior,
};
pub use flm::{Flm, FlmCallableType, FlmGroupType, FlmNodeExts};
pub use guard::{GuardPolicy, NestingTooDeep};
pub use input::{IncludedFile, IncludedFileProblem, InputError, InputHeading, InputResolver};
pub use origin::{FlmDiagnostic, FlmDiagnostics, FlmOrigin, FlmSource, FlmSpan};
pub use parse::{parse_fragment, parse_fragment_with_state, FragmentError, ParseOptions};
pub use rules::FlmParsing;
pub use structural::StructuralElements;

// Not part of the public interface: the span a condition with no position in
// any document is reported at, spelled once for the whole crate.
pub(crate) use origin::nowhere;
