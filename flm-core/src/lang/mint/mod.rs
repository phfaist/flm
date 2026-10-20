//! Minting: building, from the features' element declarations, the techy
//! packages and specifications that a parse resolves commands against.
//!
//! FLM parses documents with techy, the parser toolkit it is built on. When
//! techy's parser reads a command such as `\section` or `\begin{itemize}`, it
//! looks the name up and follows a **specification** (a value implementing
//! techy's `CallableSpec`): the arguments the command takes and how to parse
//! them. Specifications are held in **techy packages**, named tables of
//! specifications, and a parse consults a stack of them, the **scope stack**,
//! innermost first. This module builds all of these from what the features
//! declare; that work is called *minting*. [`mint_packages`] does it; the
//! language build ([`build_language`](crate::lang::build_language)) calls it
//! once per registry and installs the result.
//!
//! # What a feature declares
//!
//! A feature gives **declarations**, plain data that say what exists, to its
//! feature builder ([`FeatureBuilder`](crate::core::registry::FeatureBuilder));
//! the registry passes them here when it builds the language, and minting
//! builds every specification from them:
//!
//! - an element declaration ([`ElementDef`]): an element together with the
//!   syntax it is written in (macro, environment or special character sequence),
//!   its arguments, its body, and the parsing modes it is available in (text
//!   mode unless it lists its modes, [`ElementDef::modes`]);
//! - a fallback ([`FallbackDef`]): what a macro or an environment that no
//!   declaration defines means, in the parsing modes the fallback lists;
//! - a scoped package ([`ScopedPackageDef`]): a set of element declarations
//!   that can be written only inside the environment bodies and arguments
//!   whose declarations refer to the package by name;
//! - an override ([`OverrideDef`]): a feature's statement that its definition
//!   of a name replaces another feature's.
//!
//! An argument or a body whose syntax no standard argument kind
//! ([`ArgStdKind`]) can express is still declared this way: the argument
//! declaration holds a parser object of the feature's own
//! ([`FlmArgumentParser`], in [`ArgDef::kind`]), or the body declaration does
//! ([`FlmBodyParser`], in [`BodyDef::parser`]). The name, layout, parsing mode,
//! scoped packages and documentation are declared beside the parser, and this
//! module builds the specification around it: the mode and the scoped packages
//! become the parsing state the parser reads under, and the specification
//! carries the layout for the [block-structure
//! pass](crate::lang::rewrites::blocks). The library reads a parser object
//! only through the methods of these traits and never tests which parser
//! type it is. The methods return, for instance, whether the argument holds
//! document content, and the bytes that identify the parser in the [registry
//! digest](crate::model::digest), the hash of everything about a registry that
//! can change what processing produces.
//!
//! # Every specification carries its element
//!
//! Every specification a parse can resolve is one of a **closed list** of
//! FLM-owned types ([`specs`]), and each of them carries the element its nodes
//! receive. [`resolve_element`] therefore gives a node its element by testing
//! the type of its specification, with no name to look up. A specification
//! registered by a feature directly would produce a node with no element
//! ([`UnmintedSpec`]), which is why features declare and this module builds
//! the specifications.
//!
//! # Declaration errors and where they are reported
//!
//! Minting never stops on a faulty declaration: it builds everything that is
//! sound and returns the problems as [`MintError`] values, such as two features
//! defining one name in the same parsing mode, or a reference to a scoped
//! package no feature declares. The language build reports each one as a
//! [`LangBuildError::Mint`](crate::lang::LangBuildError::Mint), and the
//! registry build refuses a language with any such error
//! ([`BuildPhase::Language`](crate::core::registry::BuildPhase::Language)).
//!
//! # Submodules
//!
//! - [`decl`] — the declaration types a feature fills in.
//! - [`parsers`] — the parser objects an argument or a body declaration holds:
//!   the traits [`FlmArgumentParser`] and [`FlmBodyParser`], and the standard
//!   argument kinds [`ArgStdKind`].
//! - [`codes`] — the argument codes: the text form of an argument (`{`, `[`,
//!   `r()`, ...) that settings given as data use, and its only reader.
//! - [`field`] — the element `flm.field` that every field node carries; a field
//!   is a macro that a construct absorbs as data, such as the `\label` written
//!   after a heading.
//! - [`specs`] — the FLM specification types, the language's own specifications
//!   for unknown macros and environments, and [`resolve_element`].
//! - [`packages`] — [`mint_packages`] and its result [`MintedProviders`]: the
//!   features' techy packages, the scoped packages, the detection of two
//!   features defining one name, and the fallback provider
//!   ([`FlmFallbackProvider`]), which resolves a command that no declaration
//!   defines.
//!
//! See [features and packages](crate::guide::concepts_overview#features-and-packages)
//! for how features reach a registry, and the guide section [declaring
//! elements](crate::guide::extending#declaring-elements) for a walk through the
//! declaration types.

pub mod codes;
pub mod decl;
pub mod field;
pub mod packages;
pub mod parsers;
pub mod specs;

pub use codes::{
    arg_def_from_code, deserialize_arg_list, ArgCodeError, ArgCodeProblem, ArgData,
    ArgDataFields, ArgListData,
};
pub use decl::{
    ArgDef, ArgLayout, BodyDef,
    BodyLayout, CallableKind, ElementDef, FallbackDef, FallbackKind, OverrideDef,
    ScopedPackageDef, SyntaxDef, TackOnField,
};
pub use parsers::{
    ArgStdKind, DefinitionShape, FlmArgumentParser, FlmBodyParser, IntoFlmArgumentParser,
    IntoFlmBodyParser,
};
pub use field::{FIELD, Field, field_element, field_name, field_type_element, is_field_named};
pub use packages::{
    mint_packages, ClashKind, FlmFallbackProvider, MintError, MintedProviders, Symbol,
    FALLBACK_PACKAGE_NAME, FALLBACK_PROVIDER_NAME, FIELDS_PACKAGE_NAME, LANGUAGE_OWNER,
};
pub use specs::{
    arg_layout_of, body_def_of, is_math_text_command, reads_text_in_math, resolve_element, FlmCallable,
    FlmEnvironment,
    FlmPresentationSpec,
    FlmSynthesizedSpec, FlmUnknownEnvironmentBehavior, FlmUnknownMacroSpec, UnmintedSpec,
};
