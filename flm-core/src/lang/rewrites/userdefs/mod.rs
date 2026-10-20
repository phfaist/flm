//! User definitions: macros, environments and special character sequences that
//! are defined by substitution, and the rewrite pass that expands them.
//!
//! A **user definition** ([`Definition`]) is a macro, environment or special
//! character sequence that a document's author or the embedding program defines
//! by substitution: it has a name, an argument syntax, a template, and the
//! parsing modes it is defined for ([`DefinitionModes`]: text mode, mathematics
//! mode, or both). Its **template** is FLM text with placeholders: `#1` or
//! `#{name}` refers to an argument, and `#{body}` to an environment's body. A
//! **use site** of a definition is a node of a document tree where the document
//! uses it, such as `\hi{World}` for a macro `\hi`.
//!
//! A user definition enters the [registry](crate::core::registry) as an
//! ordinary element declaration whose element carries the [`Expand`] aspect (an
//! [aspect](crate::guide::concepts_overview#elements-and-aspects) is a piece of
//! behavior attached to an element and read back through a trait object). The
//! [`userdefs` feature](crate::stdcore::features::userdefs) declares the
//! definitions of its configuration this way and compiles their templates when
//! the registry is built. The **expand pass** ([`expand`]) then replaces every
//! use site in a tree by the definition's template, instantiated with the
//! arguments written at the use site. The pass is a plain function over a tree,
//! so it can be called on any tree; the `userdefs` feature runs it as two
//! [rewrite stages](crate::core::stage::RewriteStage). A document's body cannot
//! define a macro (FLM has no `\newcommand`): every user definition comes from
//! the configuration the registry is built with, which an author can write in
//! the document's front matter.
//!
//! The guide describes the same language for authors:
//! [author definitions](crate::guide::language_syntax#author-definitions) and
//! [the template language](crate::guide::language_syntax#the-template-language).
//! Other features write their own templates in this template language (number
//! templates, heading title templates, and others): see [the template language
//! shared by features](crate::guide::concepts_overview#the-template-language-shared-by-features).
//!
//! # Items of this module
//!
//! - [`definition`]: [`Definition`], the plain data a caller declares;
//!   [`UserDefinition`], the data of a definition's element
//!   ([`ElementSpec::value`](crate::model::element::ElementSpec::value)), which
//!   implements [`Expand`] for that definition and which the `userdefs` feature
//!   attaches to the definition's element; [`compile`], which compiles a definition's templates into a
//!   [`CompiledDefinition`] against a built language; and [`compile_with`], the
//!   same over one [`TemplateLanguage`] built once for all the definitions of a
//!   registry. A registry build therefore proceeds in two steps: the language
//!   first, then the templates written in it.
//! - [`template`]: the template language (the placeholders `#1`, `#{name}`,
//!   `#{body}`, `#[…]{…}` and `##`, and the conditionals), the parsing state a
//!   template is parsed under, and the compiler of one template into a
//!   [`CompiledTemplate`] whose placeholders are checked.
//! - [`hook`]: the [`Expand`] aspect, the [`Expander`] interface through which
//!   an `Expand` implementation asks the pass to instantiate a template or to
//!   report a problem, and [`PlaceholderBindings`], which says what each
//!   placeholder is replaced by at one use site.
//! - [`instantiate`](mod@instantiate): what happens at one use site. The
//!   substitution pass replaces each placeholder by the use site's already
//!   parsed nodes and evaluates the conditionals; the re-sourcing pass then
//!   copies every node of the template onto a new source created for this use
//!   site. The function [`instantiate`](fn@instantiate) runs the two passes, and
//!   a feature outside the library calls it to instantiate a template it
//!   compiled, with the values it computed held by [`ValueBindings`].
//! - [`pass`]: [`expand`], the pass over a whole tree, its options
//!   ([`ExpandOptions`]) and its failure ([`ExpandFailure`]).
//! - [`diagnostics`]: the conditions the pass and the compiler report.
//!
//! # How a use site is expanded
//!
//! - The nodes of the instantiated template are inserted in place of the use
//!   site, without a wrapper node. A paragraph break in a template therefore
//!   splits the enclosing paragraph when the
//!   [block-structure pass](super::blocks) runs later.
//! - The arguments are the use site's already parsed nodes, copied with their
//!   positions in the document and never parsed again, so a definition cannot
//!   change how its arguments were read.
//! - Substitution comes first, then expansion: the instantiated template goes
//!   through the pass again, one level deeper, so that definitions used in the
//!   template or in an argument are expanded in turn.
//! - Every use site gets a source of its own, created for it, so that each
//!   expanded node can be traced back to its position in the template and to
//!   the use site.
//! - The whitespace after a macro invocation is part of the invocation and
//!   disappears with it.
//! - The pass limits how many expansions may be nested inside one another, and
//!   the limit applies to every [`Expand`] implementation alike
//!   ([`ExpandOptions::recursion_limit`]).
//! - No placeholder node remains in the expanded tree, including in content
//!   that an expansion hook computed.
//! - A problem at one use site (a definition that was refused, a use in a
//!   parsing mode where its template did not parse, a placeholder without a
//!   value, the expansion depth limit) is reported as an Error diagnostic, and
//!   the use site is replaced by its literal source text. Content nested deeper
//!   than the [recursion guard](crate::lang::guard) allows, and a failure of the
//!   pass itself, leave no expanded tree at all ([`ExpandFailure`]).
//!
//! # The three tests a template can write
//!
//! A template can test its arguments with conditional commands. Each takes one
//! placeholder as its first argument. The three tests are easy to confuse:
//!
//! - **provided**: `\IfProvidedT`, `\IfProvidedF`, `\IfProvidedTF`, and the
//!   synonyms `\IfBooleanT`, `\IfBooleanF`, `\IfBooleanTF`. The test holds when
//!   the use site *wrote* something at this argument: a `*` marker, an optional
//!   argument in brackets, a mandatory group. A declared default value does not
//!   make it hold.
//! - **value**: `\IfValueT`, `\IfValueF`, `\IfValueTF`, and the negated
//!   `\IfNoValueT`, `\IfNoValueF`, `\IfNoValueTF`. The test holds when the
//!   placeholder has something to substitute. An argument the use site left out
//!   has a value when a default is declared for it, because a default value is
//!   bound before the conditionals are evaluated.
//! - **blank**: `\ifblank`, and the negated `\notblank`. The test holds when
//!   what the placeholder would substitute is empty or holds nothing but
//!   comments. Whitespace is content, so `\m{ }` is not blank. The test reads
//!   through a declared default value, as the *value* test does.
//!
//! # Rules of the template language
//!
//! - **A template is compiled once, when the registry is built**, against the
//!   language the registry declares. It is parsed in the language's initial
//!   parsing state, so it can use every element available at the top level of
//!   a document, other user definitions included, and what a template means
//!   does not depend on where it is used.
//! - **A bracketed part `#[…]` on a placeholder of an argument (numbered or by
//!   its declared name) or of the body is refused when the registry is built.** That bracketed part, the
//!   *substitution argument*, is data for a placeholder whose content an
//!   expansion hook of an extension computes; on any other placeholder it would
//!   have no effect, so the definition is refused once rather than the part
//!   being ignored at every use.
//! - **A declared default value is bound before the conditionals are
//!   evaluated.** This is why the *provided* test exists beside the *value*
//!   test: an argument that the use site left out and a default fills has a
//!   value and is not blank, and only *provided* tells whether the use site
//!   wrote anything.
//! - **The test argument of `\ifblank` may be any content**, not only a
//!   placeholder: the test holds when the argument holds nothing but comments
//!   and placeholders that substitute blank content; any other node (text,
//!   whitespace included, a group or a command) makes it not blank.
//! - **A placeholder is replaced by the argument's content nodes and nothing
//!   else**, regardless of the kind of argument declared: a verbatim argument
//!   substitutes the characters between its delimiters, not its group (see
//!   [`definition`]).
//! - **A definition is available in the parsing modes it is defined for, and
//!   nowhere else.** Its element is declared available in those modes alone, so
//!   a use of a definition for text mode inside a formula is an unknown command
//!   like any other name the registry does not define; there is no separate
//!   condition for it. One name may be declared twice, once per mode, when the
//!   two modes need different templates: each declaration applies in its own
//!   mode, and the two never apply at the same place.
//! - **The argument number offset is declared on the definition**
//!   ([`Definition::argument_number_offset`]) and is never written in a
//!   template. The numbered keys of the default values follow the same
//!   numbering as the placeholders, so `#1` and the default value keyed `"1"`
//!   always refer to the same argument (see [`definition`]).
//! - **`\IfProvidedT/F/TF` and `\IfBooleanT/F/TF` are one test** under two
//!   spellings; both are accepted.
//!
//! # Command names shared with the Python implementation
//!
//! The command names of the conditionals (`\IfValueTF`, `\IfNoValueTF`,
//! `\IfBooleanTF`, `\ifblank`, `\notblank`) are those of the Python
//! implementation of FLM, whose `substmacros` feature this module replaces.
//! Where the behavior under one of those names differs, the migration guide
//! lists the difference:
//! [user definitions and the template language](crate::guide::python_flm_migration#user-definitions-and-the-template-language).

pub mod definition;
pub mod diagnostics;
pub mod hook;
pub mod instantiate;
pub mod pass;
pub mod template;

pub use definition::{
    compile, compile_with, CompiledDefinition, Definition, DefinitionBindings, DefinitionKind,
    DefinitionModes,
    EffectiveSignature, ModeVariant, NotRecomposable, RegistryBuildFacts, UserDefinition,
};

pub use diagnostics::{
    BadDefinition, BadPlaceholder, DefinitionProblem, ExpansionFailed, PlaceholderProblem,
    RecursionLimit,
};
pub use hook::{
    Expand, ExpandCx, ExpandError, ExpandPolicy, ExpandSet, Expander, Expansion,
    PlaceholderBindings, PlaceholderValue,
};
pub use instantiate::{instantiate, ValueBindings};
pub use pass::{expand, ExpandFailure, ExpandOptions, DEFAULT_RECURSION_LIMIT};
pub use template::{
    compile_in_modes, compile_template, compile_template_with, CompiledTemplate, PlaceholderRef,
    PlaceholderTarget, PlaceholderVocabulary, TemplateLanguage, TemplateSpec,
};
