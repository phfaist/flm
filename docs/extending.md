# Extending FLM: features and packages

This chapter is for the author of a feature or of a backend: a Rust programmer who wants
FLM to read a new construct, compute new data about a document, or write a new output
format.

Everything FLM can do comes from **features**. A
[registry](crate::core::registry) is the frozen record of everything the language and
the pipeline consult for a set of features: the elements, stages, tables, capabilities,
lowering rules, backends, outputs, workflows, option types and services the features
declared, built once and shared by every document, plan and run. A feature
([`FlmFeature`](crate::core::registry::FlmFeature)) is a Rust value that contributes a
related bundle of things to a registry (elements, stages, tables, lowering rules,
backends, services, settings) through the feature builder
([`FeatureBuilder`](crate::core::registry::FeatureBuilder)). A program decides what FLM can
do by choosing the features its registry is built from, usually grouped in
[packages](#bundling-features-as-a-package).

The standard features in [`stdcore`](crate::stdcore) register through the same interface
as a feature of yours. The library has no built-in set of elements that features add to,
and a standard feature uses no method of the feature builder that your feature cannot
call. For that reason this chapter uses the standard features as its examples: each
extension point is used by at least one of them, in code you can read.

A feature is written in Rust and compiled into the host that uses it (a host is any
program that embeds the library), or it comes from an *extension*: a set of files that a
host finds by name while it processes a document, written as data (settings only), in
JavaScript, or in Rust, compiled to WebAssembly or into the program (features with
behavior), and read through
the crate `flm-ext` (see
[Extensions](#extensions)). The library offers no way today to load a compiled feature
into a host that was not built with it.

The chapter covers, in order: a complete feature in one example; writing a feature;
declaring elements and their syntax; aspects; tables and stages; capabilities,
collections, ordering and workflows; lowering rules and output formats; options, settings
and services; the two build hooks; bundling features as a package; choosing packages at
run time; extensions, written as data, in JavaScript, or in Rust compiled to WebAssembly or into the program; a table of the standard features to read for each extension point, with
recipes that extend the numbering, headings, xnotes and cells features; and an extension
crate of this repository, end to end.

## A feature, end to end

The following example is a whole feature: a `\shout{…}` command that renders as bold
text. The example writes the feature, registers it beside the standard package, and
renders a document that uses it:

```rust
use std::sync::Arc;

use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use flm_core::model::element::ElementSpec;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::model::names::ElementName;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder, RegistryInputs,
};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::stdcore::backends::pres::{self, SpanPayload, SpanStyle};
use flm_core::techy::core::node::NodeRef;

/// The element `\shout` produces, and the rule that lowers it. Both names
/// begin with the feature's own name, so that they cannot collide with the
/// names of another feature.
const SHOUT: &str = "shout.shout";
const SHOUT_RULE: &str = "shout.shout.lower.default";

/// The lowering rule: a `\shout{…}` node becomes a bold `flm.pres.span` around
/// its argument, which is lowered by the rules of the elements it holds.
struct ShoutRule;

impl Lower for ShoutRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let body = lw.lower_arg_named(cx, &node, "text")?;
        let payload = SpanPayload { style: Some(SpanStyle::Bold), ..SpanPayload::default() };
        let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?;
        Ok(Lowering::Emit(vec![span]))
    }
}

/// The feature itself: one element and one lowering rule.
struct Shout;

impl FlmFeature for Shout {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("shout", "Shouting").docs("The `\\shout` command.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();

        // The element value: what a `\shout` node *means*. Its block role says
        // it is ordinary inline content.
        let element = Arc::new(
            ElementSpec::new(SHOUT)
                .docs("`\\shout{text}`: loud inline content.")
                .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
        );

        // The syntax: a macro `\shout` with one mandatory argument named `text`.
        b.element(
            ElementDef::new(
                owner,
                ["shout"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                element,
            )
            .docs("`\\shout{text}`"),
        )?;

        // How it renders: the default rule for this element, on every backend.
        b.lowering_rule(
            LoweringRule::new(
                SHOUT_RULE,
                SHOUT,
                LoweringDecl::new().produces(pres::SPAN),
                Arc::new(ShoutRule),
                DefaultMark::Everywhere,
            )
            .docs("`\\shout` lowers to a bold span"),
        )?;

        Ok(())
    }
}

// Register it beside the standard package.
let mut builder = RegistryBuilder::new();
builder.use_package(flm_core::stdcore::package(flm_core::stdcore::StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(Shout));
let registry = builder.build(&RegistryInputs::new())?;

# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{FlmOrigin, ParseOptions, parse_fragment};
# use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
# use flm_core::techy::source::Source;
// Render a document that uses it through the `text` workflow.
let source =
    Arc::new(Source::new("Please \\shout{listen} now.").with_origin(FlmOrigin::none()));
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));

let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// Plain text cannot show boldness, so the emitter prints the content alone.
assert_eq!(run.table(TEXT_OUTPUT)?.expect("written").text, "Please listen now.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

The example shows three separations that the rest of the chapter relies on:

- The **element** ([`ElementSpec`](crate::model::element::ElementSpec)) is the value
  that says what a node means: a declaration with a stable dotted type name, identified
  by the element value itself (its allocation) and not by its name.
- The **syntax** ([`ElementDef`](crate::lang::mint::ElementDef)) says how the construct
  is written, separately from the element.
- The **lowering rule** ([`LoweringRule`](crate::core::lowering::LoweringRule)) says how
  the construct renders, separately from both.

The builder records every declaration as the declaration of the feature that is
registering. A declaration that holds its owner's name, such as `ElementDef::new` above,
takes it from [`FeatureBuilder::feature`](crate::core::registry::FeatureBuilder::feature),
which returns the name under which the feature was registered, so the feature does not
repeat its name there.

## Writing a feature

A feature implements [`FlmFeature`](crate::core::registry::FlmFeature), which has two
required methods:

- [`info`](crate::core::registry::FlmFeature::info) returns a
  [`FeatureInfo`](crate::core::registry::FeatureInfo): the feature's registered name, a
  title, documentation and an optional version. The name is unique in a registry. By
  convention it is the middle segment of the names of everything the feature declares:
  `refs` in the stage name `flm.refs.resolve`.
- [`register`](crate::core::registry::FlmFeature::register) declares everything the
  feature contributes, by calling methods on the
  [`FeatureBuilder`](crate::core::registry::FeatureBuilder) it is handed.

A third method, [`settings`](crate::core::registry::FlmFeature::settings), is optional:
a feature whose settings can be written as data returns their declaration from it (see
[Settings given as data](#settings-given-as-data)).

Implement the trait on a value that holds the feature's own configuration (for example
`Blocks::new(BlocksConfig { .. })`), so that the compiler checks the configuration
rather than a schema. A feature's configuration is the plain Rust value the feature is
constructed from; it is fixed when the registry is built and cannot vary per document or
per render ([settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)). The
registry build consumes the feature value; the finished registry keeps only its
`FeatureInfo` ([`FlmRegistry::features`](crate::core::registry::FlmRegistry::features)).

The **registry digest** ([`digest`](crate::model::digest)) is a number computed from
everything about a registry that can change what processing produces; a cache of results
keyed on it is reused only for an identical registry. The build computes the digest from
the declarations it can enumerate. A configuration that changes what processing produces
in a way the declarations do not show, such as the text of an author-written template,
must be passed to
[`FeatureBuilder::digest_input`](crate::core::registry::FeatureBuilder::digest_input), so
that the digest changes when that configuration changes. A feature's version string, if
it sets one, is also an input of the digest, so two registries whose features differ only
in version have different digests.

### The builder only records

Calling a method of the feature builder records one declaration. Nothing is resolved,
looked up or checked while a feature registers. **Registration order therefore never
limits what a feature may declare**: a feature may refer to a capability, a table or a
stage that another feature declares, whether or not that feature has registered yet, and
features never have to be listed in dependency order.

All the checking happens afterwards, in
[`RegistryBuilder::build`](crate::core::registry::RegistryBuilder::build), which works
through five phases: collect, finalize, validate, language and late initialization
([`BuildPhase`](crate::core::registry::BuildPhase); the [build
module](crate::core::registry::build) describes each). Each phase collects every problem
it finds. The first phase that finds a problem stops the build and returns all the
problems of that phase together, as one
[`RegistryBuildError`](crate::core::registry::RegistryBuildError); the phases after it
do not run, because they would report the consequences of the earlier problems as
problems of their own. A successful build returns a frozen
[`FlmRegistry`](crate::core::registry::FlmRegistry), which every plan and run built
against it shares through an `Arc` and which never changes again.

Features register in two rounds: `register` is the first, and the second is described
under [Build hooks](#build-hooks).

### Names

Every entity a feature registers, other than an element, is identified by a
[`Name`](crate::model::names::Name): a string, usually dotted and by convention
lowercase, whose first segment says who owns it: `flm` for the standard features, a
prefix of your own for anything you write. An element also has a name, its type name,
but it is identified by its element value (see [Element values](#element-values)). Each
kind of name has a Rust type of its own
([`ElementName`](crate::model::names::ElementName),
[`CapName`](crate::model::names::CapName),
[`StageName`](crate::model::names::StageName),
[`RuleName`](crate::model::names::RuleName), and the others of
[`names`](crate::model::names)), so that a function that asks for a capability name
cannot be given a stage name by mistake ([names and
identity](crate::guide::concepts_overview#names-and-identity)).

Capabilities, tables and collections share one set of names: declaring a table declares
a capability of the same name, and a collection may not take the name of either.
Features, stages, lowering rules, backends, outputs and workflows each have a set of
names of their own.

The usual forms are `<owner>.<thing>` for an element and
`<owner>.<element>.lower.<variant>` for a lowering rule. The standard features write a
stage's name `flm.<feature>.<verb>`, as in `flm.refs.resolve`.

Two kinds of name are also **a public interface for a stylesheet**: the name of a
presentation element, and the **role** that a lowering rule puts in a presentation
node's payload, a dotted name that says what a container is (`flm.heading.number` for
the number of a heading; [`pres::payload`](crate::stdcore::backends::pres::payload);
[decorations, roles and kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)).
The HTML backend
derives a CSS class from each of them, with every hyphen doubled and every dot turned
into one hyphen ([`class_of_role`](crate::stdcore::backends::html::names::class_of_role)):
`flm.heading.number` becomes `flm-heading-number`, and a page's stylesheet selects on
that class. Renaming a role therefore breaks every stylesheet written against it, as
renaming a public function breaks its callers: choose the name once and keep it. The
rendering guide describes the classes and attributes a stylesheet may rely on ([the HTML
contract](crate::guide::rendering#the-html-contract)).

The class reads back to exactly one name only when the name is **well formed**: each of
its segments (the parts between the dots) is non-empty and neither begins nor ends with
a hyphen. A name written in the usual forms above is well formed. Nothing checks the
condition, and a name that breaks it can share its class with another name
(`mypkg.a.-b` and `mypkg.a-.b` both give `mypkg-a---b`, and `mypkg.a..b` gives
`mypkg-a--b`, the class of the well-formed name `mypkg.a-b`).

## Declaring elements

An element is the value that says what a node means
([`ElementSpec`](crate::model::element::ElementSpec)); the syntax is how the construct is
written in the source. One declaration gives both: an element declaration
([`ElementDef`](crate::lang::mint::ElementDef)) is the plain data a feature gives for an
element, the syntax it is written in (macro, environment or special character sequence),
its arguments (each with a parser, a layout, a parsing mode and the scoped packages
available inside it) and its body. It lists the names the construct is written under, a
[`SyntaxDef`](crate::lang::mint::SyntaxDef) saying how those names are parsed, and the
element value its nodes carry. A feature registers it with
[`FeatureBuilder::element`](crate::core::registry::FeatureBuilder::element).

A feature gives the parser plain data only. From these declarations the library builds
the packages and specifications of techy, the parser toolkit FLM is built on, that a parse
resolves against. This step is called *minting* ([`lang::mint`](crate::lang::mint)).
Every parsed node therefore carries its element directly, and no lookup is needed to find
what a node means.

### Syntax

[`SyntaxDef`](crate::lang::mint::SyntaxDef) has five forms:

- `Macro { args }`: `\name` followed by its list of arguments;
- `Environment { args, body }`: `\begin{name}…\end{name}`, with its arguments after
  `\begin{name}` and its body;
- `Inclusion { args }`: a macro that brings the content of another file into the
  document, such as `\input{reference}`; the language's own inclusion parser reads it
  ([`lang::input`](crate::lang::input));
- `Specials { trigger, args }`: a special character sequence, such as `~` or `--`,
  followed by its arguments;
- `Synthesized`: no syntax at all. The element is never parsed: only a rewrite stage, a
  lowering rule, or a declaration elsewhere that carries its own syntax creates nodes of
  it.

The field element ([`FIELD`](crate::lang::mint::FIELD)) is declared as `Synthesized`, by
the `lowering` feature
([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature)). A field is a
macro that a construct absorbs as data rather than as content, such as `\label` after a
heading ([fields: data a construct
reads](crate::guide::concepts_overview#fields-data-a-construct-reads)). The `lowering`
feature's declaration creates only the element type name `flm.field` and the default
lowering rule of field nodes. Each field gets its own element value from its own
declaration ([`field_element`](crate::lang::mint::field_element)), and the construct that
accepts the field gives it the macro syntax by which it is parsed there (see [Fields
absorbed after a construct](#fields-absorbed-after-a-construct)).

A declaration is available in text mode unless it lists its parsing modes with
[`ElementDef::modes`](crate::lang::mint::ElementDef::modes): a command of mathematics
is declared with `[Mode::Math]`, and a command that applies in text and in mathematics
alike with `[Mode::Text, Mode::Math]`. So a command a feature declares without thinking
about mathematics cannot be written inside a formula: there it is an unknown command.
Two features that define one name in parsing modes that do not overlap do not clash.

### Arguments

Each argument is declared by an [`ArgDef`](crate::lang::mint::ArgDef): the parser that
reads it (one of the standard kinds of [`ArgStdKind`](crate::lang::mint::ArgStdKind), or
a parser of the feature's own,
[`FlmArgumentParser`](crate::lang::mint::FlmArgumentParser)), an optional name, the
parsing mode it is read in, its layout, whether its content is kept as typed under an
opaque wrapper, its scoped packages (the last three described below), and
documentation. Give an argument a name whenever anything reads it: a reader that
finds an argument by name keeps working when the argument list changes later, and one
that finds it by position does not.

| standard kind ([`ArgStdKind`](crate::lang::mint::ArgStdKind)) | what it reads |
|---|---|
| [`Mandatory`](crate::lang::mint::ArgStdKind::Mandatory) | a `{…}` group; a single token also counts, as in TeX |
| [`Optional`](crate::lang::mint::ArgStdKind::Optional) | an optional `[…]` group |
| [`Star`](crate::lang::mint::ArgStdKind::Star) | an optional `*` marker |
| [`Marker`](crate::lang::mint::ArgStdKind::Marker) | an optional marker of one other character, as the `!` of `\name!` |
| [`DelimitedGroup`](crate::lang::mint::ArgStdKind::DelimitedGroup) | a group between a prescribed pair of characters, as the `(…)` of `\name(arg)`; a single token does not count |
| [`OptionalDelimitedGroup`](crate::lang::mint::ArgStdKind::OptionalDelimitedGroup) | the same, optional |
| [`AnyDelimitedGroup`](crate::lang::mint::ArgStdKind::AnyDelimitedGroup) | a group in `{…}`, `[…]`, `(…)` or `<…>`, whichever the use site opens |
| [`OptionalAnyDelimitedGroup`](crate::lang::mint::ArgStdKind::OptionalAnyDelimitedGroup) | the same, optional |
| [`BracedOnly`](crate::lang::mint::ArgStdKind::BracedOnly) | a `{…}` group and nothing else: a single token does not count |
| [`Embellishments`](crate::lang::mint::ArgStdKind::Embellishments) | marker characters each followed by one expression, in any order, each at most once, as the `_{a}^{b}` of `\name_{a}^{b}` |
| [`KeyVals`](crate::lang::mint::ArgStdKind::KeyVals) | an optional `[key=value,…]` group |
| [`CharsGroup`](crate::lang::mint::ArgStdKind::CharsGroup) | a `{…}` group read as plain characters, as in `\label{sec:intro}` |
| [`Verbatim`](crate::lang::mint::ArgStdKind::Verbatim) | raw text between a pair of delimiters, as in `\verbcode|…|`; the closing delimiter is matched to the opening character unless the declaration prescribes the pair |
| [`TackOn`](crate::lang::mint::ArgStdKind::TackOn) | the fields ([`TackOnField`](crate::lang::mint::TackOnField)) that the construct absorbs after its own arguments, as `\section{…}` absorbs `\label{…}` |
| [`Terminated`](crate::lang::mint::ArgStdKind::Terminated) | everything up to a fixed *terminator*, as FLM markup or as raw text: `\phf … \endphf`, or the code span `` `…` `` that ends at the first place where as many backticks as opened it follow one another |

A terminated argument is written in three parts: a *prefix*, which may be a run of copies
of one character (as many as the author writes) followed by a fixed opener; the content;
and the terminator, which repeats the run of the prefix. At the argument's own level, the
terminator is matched character for character before any other markup, and it is never
read as a command: in `\phf … \endphf`, `\endphf` ends the content even where a command
`\endphf` exists. Nothing is trimmed from the content. What the content may hold depends
on the declaration:

- With `raw: true`, the content is raw text, like a verbatim argument's: the terminator is
  the only thing recognized in it, so the content needs no escaping.
- With `raw: false`, the content is parsed as FLM markup and follows the document's
  rules: commands are read as usual, groups must pair, and `%` starts a comment. Inside a
  nested `{…}` group the terminator is not recognized: `\phf a{b\endphf}c\endphf` ends at
  the second `\endphf`.

The variant's documentation gives the complete rules (such as where a longer run of the
repeated character ends the argument) and the errors reported for a missing terminator or
an incomplete prefix.

### An argument's syntax in settings

Settings written as data cannot hold a parser, so they give the syntax of each argument as
data, in one of two forms:

- The **structured syntax** is a standard kind written as data: a map whose key `syntax`
  holds the kind's name, in lowercase with hyphens, and whose other keys hold the kind's
  parameters, as in `{syntax: delimited-group, delimiters: ["(", ")"]}` for a group in
  parentheses or `{syntax: chars-group}` for a group read as plain characters. Every
  standard kind above can be written this way except `TackOn`: its fields are
  declarations that a feature makes in Rust, each with an element of its own, so settings
  cannot declare one. The [documentation of
  `ArgStdKind`](crate::lang::mint::ArgStdKind#the-kind-as-data) lists each kind's name and
  parameters, and what reading a kind refuses.
- An **argument code** is a short text that stands for one structured syntax: `{` for a
  mandatory group, `[` for an optional one, `r()` for a group in parentheses, `e{_^}` for
  a subscript and a superscript, ``` F`` ``` for raw text after a run of backticks up to
  the same run followed by one more backtick (a code span, whose first backtick the
  command itself reads). A terminated argument has four codes: `u<terminator>` (content
  up to the terminator), `U<terminator>` (raw text up to it), `f<c><terminator>` (content
  after a run of the character `<c>`, up to the same run followed by the terminator, which
  may be empty) and `F<c><terminator>` (the same, raw text); these four are FLM's own. The
  [table of codes](crate::lang::mint::codes#the-codes) gives each code with the
  structured syntax it stands for. Three forms have no code and are written with the
  structured syntax: a group read as plain characters (`chars-group`), a group of
  `key=value` pairs (`key-vals`), and a terminated argument with an opener or with fixed
  text before the repeated run of its terminator (`terminated` with `opener` or `head`).

An argument list ([`ArgListData`](crate::lang::mint::ArgListData)) is written in one of
three shapes: one text of one-character codes, such as `"[{"`; a list of codes; or a list
of entries, each a code or an argument's map. An argument's map
([`ArgDataFields`](crate::lang::mint::ArgDataFields)) gives the syntax in exactly one of
the two forms, the key `syntax` with the kind's parameters or the key `code`, beside the
argument's own keys: its name, layout, opaque wrapper (`opaque_wrapper`: `true` or
`false`), documentation, parsing mode (`mode`: `text` or `math`), scoped packages
(`scoped`: a list of names) and role. So
`{syntax: delimited-group, delimiters: ["(", ")"], name: size}` and
`{code: "r()", name: size}` declare the same argument. Two functions read these into
declarations:
[`arg_def_from_code`](crate::lang::mint::arg_def_from_code) reads one code, and
[`ArgListData::arg_defs`](crate::lang::mint::ArgListData::arg_defs) reads an argument
list. A declaration read from a code keeps that code
([`ArgDef::code`](crate::lang::mint::ArgDef::code)), and one read from a structured syntax
holds its standard kind, so that each can be written back in the form it was read from.

A feature whose own settings declare commands reads their argument lists through these
functions, as the `userdefs` feature does: it declares the settings field as
`Vec<ArgDef>` read with
[`deserialize_arg_list`](crate::lang::mint::deserialize_arg_list). It never reads an
argument's syntax itself, so that a syntax means the same form of argument in every
settings file.

An extension in JavaScript, or in Rust written against the guest client `flm_ext::guest`
(see [A Rust extension compiled to
WebAssembly](#a-rust-extension-compiled-to-webassembly)), declares its arguments with the
same data, since its declarations cross to FLM as data: `new ArgDef(syntax)` and
`ArgDef.named(name, syntax)` in JavaScript, and `ArgDef::new(syntax)` and
`ArgDef::named(name, syntax)` in Rust, take a code (`"r()"`) or a structured syntax
(`{ syntax: "chars-group" }` in JavaScript, `ArgStdKind::CharsGroup` in Rust). When FLM
gives a declaration to an extension, it writes each argument in the form it was read
from: with its code, with its structured syntax, or, for a tack-on argument or a parser
of a feature's own, with neither.

### What an argument holds: its role

An argument may state its **role**: a registered, dotted name that says what kind of thing
it holds, independently of how it is written. The argument of `\ref` and the argument of
`\eqref` are written differently, but both hold a label that points at a construct, so
both have the role `flm.refs.target`. The command `flm info` shows each argument's role on
the page of its command. The registry lists every argument that has a given role
([`Declarations::role_uses`](crate::core::registry::Declarations::role_uses), reached
through [`FlmRegistry::declarations`](crate::core::registry::FlmRegistry::declarations)),
which a program that describes the language or an editor can use, for example to complete
a label or a file path. Parsing and processing never read the role, and it is not an input
of the [registry digest](crate::model::digest).

Roles are names in a namespace of their own, the *argument roles*. A feature declares a role
with [`FeatureBuilder::argument_role`](crate::core::registry::FeatureBuilder::argument_role),
giving its name and a documentation text, and sets it on an argument with
[`ArgDef::role`](crate::lang::mint::ArgDef::role()):

```rust
# use std::sync::Arc;
# use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry};
# use flm_core::lang::mint::{ArgDef, ArgStdKind};
struct Glossary;
impl FlmFeature for Glossary {
    fn info(&self) -> FeatureInfo { FeatureInfo::new("glossary", "Glossary") }
    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.argument_role("acme.glossary.entry", "The key of a glossary entry.")?;
        let key = ArgDef::named("key", ArgStdKind::CharsGroup).role("acme.glossary.entry");
        # let _ = key;
        Ok(())
    }
}
# let registry = build_registry([Arc::new(Glossary) as Arc<dyn FlmFeature>])?;
# assert!(registry.argument_role_named("acme.glossary.entry").is_some());
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

The registry lists the roles with their documentation
([`FlmRegistry::argument_roles`](crate::core::registry::FlmRegistry::argument_roles),
[`argument_role_named`](crate::core::registry::FlmRegistry::argument_role_named)). In
settings written as data, an argument entry gives its role under the key `role`
(`{syntax: chars-group, name: target, role: flm.refs.target}`); an extension in
JavaScript or in Rust declares a role with `b.argument_role(name, docs)` and sets it with
`.role(name)` on its `ArgDef`. The same map gives the argument's name, layout,
documentation, mode, scoped packages and opaque wrapper (`opaque_wrapper: true` keeps the
argument's text as typed).

A feature may set a role that another feature declares. When that other feature may be
absent from the registry, the feature setting the role declares it as a peer with
[`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role),
as for the other [peer declarations](#peer-declarations): if no feature of the registry
declares the role, the registry still builds, with one note. A role that no feature declares
and that the feature setting it did not declare as a peer stops the build
([`BuildError::UndeclaredArgumentRole`](crate::core::registry::BuildError::UndeclaredArgumentRole)),
which catches a misspelled name.

The standard features declare eight roles:

| role | declared by | set on |
|---|---|---|
| `flm.refs.target` | `refs` | the label of `\ref` and `\hyperref`; `\eqref` of `math` |
| `flm.refs.defined-label` | `refs` | the label of the field `\label`, inside the scoped package `flm.refs.label` too; the `\label` of an equation line of `math` |
| `flm.citations.key` | `citations` | the keys of `\cite`, and of the field `\cite` written after `\cite` or `\R` |
| `flm.defterm.defined-term` | `defterm` | the term of the environment `defterm` and of the field `\aliasterm` |
| `flm.defterm.target` | `defterm` | the term that `\term` points at |
| `flm.graphics.path` | `graphics` | the path of `\includegraphics` |
| `flm.input.path` | `input` | the file of `\input` |
| `flm.href.address` | `href` | the address of `\href`, `\url` and `\email` |

The names are constants of their features (for example
[`refs::TARGET_ARG_ROLE`](crate::stdcore::features::refs::TARGET_ARG_ROLE)), and a feature
of your own that sets one of these roles uses the constant. A role of your own begins with
a prefix of your own, as every name does (see [Names](#names)), and follows the form of the
standard ones, `<owner>.<feature>.<thing>` (`acme.glossary.entry`).

### An argument's layout

An argument's content is laid out by the block-structure pass, the rewrite that lays out
content in paragraphs and blocks, or as inline content
([`blocks`](crate::lang::rewrites::blocks)), unless it is an [opaque
region](crate::guide::concepts_overview#opaque-regions), which the pass carries over as
written: content read in mathematics mode, verbatim text, and content under an opaque
wrapper (an argument declared with
[`ArgDef::with_opaque_wrapper`](crate::lang::mint::ArgDef::with_opaque_wrapper), and every
characters-only group). An argument declared with an opaque wrapper
takes no layout: a layout other than inline is refused when the language is built.
The argument's declaration chooses its layout
([`ArgDef::layout`](crate::lang::mint::ArgDef::layout)), one of the three values of
[`ArgLayout`](crate::lang::mint::ArgLayout):

- **inline content**, the default: no paragraph is built inside the argument. A blank
  line in it is reported and read as one space; a block-level element in it is reported
  and kept where it is written.
- **a region of blocks** ([`ArgDef::blocks`](crate::lang::mint::ArgDef::blocks)):
  paragraphs are built inside the argument and block-level elements stay on their own,
  as in an environment body declared
  [`BodyLayout::Blocks`](crate::lang::mint::BodyLayout::Blocks). One line of text is one
  paragraph.
- **inferred from the content**
  ([`InlineOrBlocks`](crate::lang::mint::ArgLayout::InlineOrBlocks)): a region of blocks
  when the content holds a blank line or a block-level element, inline content
  otherwise. A fragment laid out
  [`InlineOrBlocks`](crate::model::doc::FragmentLayout::InlineOrBlocks) applies the same
  test to its top level.

An argument's layout does not depend on where the command is written: a command written
in the middle of a paragraph, or in a heading's title, still holds paragraphs in an
argument declared as blocks.

Two declarations can never take effect, and minting reports both: a layout other than
inline on an argument declared with an opaque wrapper
([`ArgDef::with_opaque_wrapper`](crate::lang::mint::ArgDef::with_opaque_wrapper);
[`MintError::LayoutOnOpaqueWrappedArgument`](crate::lang::mint::MintError::LayoutOnOpaqueWrappedArgument)),
and the blocks layout on an argument read in mathematics mode
([`ArgDef::mode`](crate::lang::mint::ArgDef::mode);
[`MintError::BlocksOnMathArgument`](crate::lang::mint::MintError::BlocksOnMathArgument)).
The block-structure pass carries the content of both over untouched ([opaque
regions](crate::guide::concepts_overview#opaque-regions)).
The check of the tree contract
([`check_block_contract`](crate::lang::rewrites::blocks::check_block_contract)) reports
an argument whose content does not match its declared layout, as it does for an
environment body.

An element that must know whether one of its arguments holds blocks (to choose its own
block role, which the block-structure pass reads, and again to choose how it is rendered
after that pass) calls
[`argument_holds_blocks`](crate::lang::rewrites::blocks::argument_holds_blocks) instead
of inspecting the content. The function decides from the declared layout and, for an
inferred layout, by the same test the block-structure pass applies, and it returns the
same result on the tree before the pass and after it. The decision is therefore made in
one place, and no element works it out again.

The worked case is `\footnote`, which the
[`xnotes`](crate::stdcore::features::xnotes) feature declares. Its content argument is
inferred by default, since a note is usually one line of text and sometimes several
paragraphs, and the feature's configuration may fix the layout for a kind of note. Its
optional argument accepts the `\label{…}` of the note, and nothing else, through a
scoped package (see [Scoped packages](#scoped-packages) below):

```text
[
    ArgDef::optional("labels").scoped("flm.refs.label"),
    ArgDef::mandatory("xnote").layout(kind.layout),
]
```

With this declaration, `\footnote[\label{fn:a}]{One line.}` and a `\footnote{…}` holding
two paragraphs are both read as the author means them, and `\label` written in the
running text or inside the note's content is an unknown command. The scoped package
`flm.refs.label` belongs to the `refs` feature, which declares `\label`. The `xnotes`
feature makes a peer declaration of it (see [Scoped packages](#scoped-packages)), so a
registry without the `refs` feature still builds, and the optional argument then
accepts nothing.

### Fields absorbed after a construct

A field is a macro that a construct absorbs as data rather than as content, and a
construct takes its fields through its **tack-on argument**, a trailing argument
([`ArgStdKind::TackOn`](crate::lang::mint::ArgStdKind::TackOn)) that absorbs the field
macros written after the construct's other arguments ([fields: data a construct
reads](crate::guide::concepts_overview#fields-data-a-construct-reads)).

The fields a tack-on argument absorbed are **data that the construct which declared the
argument reads**, and nothing else reads them. Every field node carries the one element
type `flm.field` ([`FIELD`](crate::lang::mint::FIELD)), whose value holds the field's
name as written ([`field_name`](crate::lang::mint::field_name) returns it). A field's
name means what its declaring construct says it means, and nothing across the document is
keyed on it: your `\label` field is your construct's own, even though the `refs` feature
declares a field of that name too.

Where a field is written in the content makes no difference, since it represents no
content there. For that reason, a field node that reaches a backend is an error
(`flm.lowering.field-not-read`): it means that the construct that absorbed it did not
read its data. Read your fields in the aspect or the lowering rule that needs them, and
write what they mean from there.

A user definition (a macro, environment or special character sequence that a document's
author or the host defines by substitution;
[`Definition`](crate::lang::rewrites::userdefs::Definition)) may declare a tack-on
argument, but its template may not refer to it: the macros a call absorbed are whole
commands and have no meaning as the value of a parameter. A template that refers to one
is an error, reported when the definition is compiled, on every backend. What such a call
absorbs is written nowhere; no placeholder exists today that reads the absorbed fields.

### Environment bodies

An environment's body is declared by a [`BodyDef`](crate::lang::mint::BodyDef), which
says:

- how the block-structure pass lays the body out
  ([`BodyLayout`](crate::lang::mint::BodyLayout): inline, blocks, or according to the
  element's own block role);
- what reads the body when it is not read the ordinary way: raw text
  ([`BodyDef::verbatim`](crate::lang::mint::BodyDef::verbatim)), or a body parser of the
  feature's own ([`BodyDef::parser`](crate::lang::mint::BodyDef::parser));
- which parsing mode the body is read in;
- whether the body's content is kept as typed under an opaque wrapper
  ([`BodyDef::with_opaque_wrapper`](crate::lang::mint::BodyDef::with_opaque_wrapper);
  [opaque regions](crate::guide::concepts_overview#opaque-regions)), in which case a
  layout other than the default, `FromRole`, is refused when the language is built;
- which scoped packages are available inside it.

### Scoped packages

A scoped package ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) is a set of
element declarations that can be written only inside the environment bodies and
arguments whose declarations refer to the package by name, for instance `\item` inside a
list's body or `\caption` inside a float's body. A body or an argument refers to a
scoped package with [`BodyDef::scoped`](crate::lang::mint::BodyDef::scoped) or
[`ArgDef::scoped`](crate::lang::mint::ArgDef::scoped), and a feature declares one with
[`FeatureBuilder::scoped_package`](crate::core::registry::FeatureBuilder::scoped_package)
([scoped packages](crate::guide::concepts_overview#scoped-packages)).

A scoped package gives an environment or a command constructs of its own, which mean
nothing outside it, without adding them to the language everywhere. The parser adds the
package to the parsing state for the length of the body or the argument, so its
declarations take precedence over any other declaration of the same name there. Any
body or argument may refer to a scoped package, whatever reads it: inside a reading
that recognizes no command, such as a characters-only group or a verbatim argument,
the package's declarations cannot be reached, and nothing is reported. The
`math` feature declares one for the commands of an equation line, which can be written
only inside the bodies of its display-math environments
([`math::lines`](crate::stdcore::features::math::lines)); `\footnote`'s optional
argument, above, is the case of an argument. A scoped package may add as little as one
special character: the data argument of the `cells` feature's `\celldata` refers to a
package that holds `&` alone
([`cells::DATA_PACKAGE`](crate::stdcore::features::cells::DATA_PACKAGE)), so that `&`
ends an entry there and is an ordinary character everywhere else. Two scoped packages
that declare one name (two features each giving `\\` a meaning inside its own
environment) never clash: each is available only where it is referred to, and where one
is added inside the other, the inner one takes precedence.

Any feature may declare a scoped package, whether it registers before or after the
features that refer to it. A feature that refers to a package which another feature
owns makes a **peer declaration** of it with
[`peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package). A
peer declaration ([`PeerName`](crate::core::registry::PeerName)) is a feature's forward
declaration of something another feature owns (see [Peer declarations](#peer-declarations)).
When no feature of the registry declares the package, the build succeeds with one Note,
the bodies and arguments that refer to the package get nothing from it, and every
command the package would define is an unknown command there.

The registry build fails, with a [`MintError`](crate::lang::mint::MintError), in two
cases:

- a body or an argument refers to a package that no feature declares and that the
  referring feature did not peer-declare; this catches a misspelled name
  ([`MintError::UnknownScopedPackage`](crate::lang::mint::MintError::UnknownScopedPackage));
- packages refer to each other in a circle, through bodies or arguments
  ([`MintError::ScopedPackageCycle`](crate::lang::mint::MintError::ScopedPackageCycle)).

### Element values

One element *type name* may have several element *values*. `\section` and `\subsection`
are two values of the single element type `flm.heading`, which differ in the data each
value carries ([`ElementSpec::with_value`](crate::model::element::ElementSpec::with_value));
the four commands of the `textstyles` feature are four values of `flm.textstyle`. A
family of related commands is declared as one element type with one element value per
command: the type name says what kind of thing a node is, and the data of each element
value tells the commands apart ([elements and
aspects](crate::guide::concepts_overview#elements-and-aspects)). One lowering rule, written
for the element type, then serves every command of the family.

An element value is shared as `Arc<ElementSpec>`, and its **identity is the allocation**,
not the name: two handles designate the same element when
[`ElementSpec::same_value`](crate::model::element::ElementSpec::same_value) returns
`true`. Several names may share one element value: an
[`ElementDef`](crate::lang::mint::ElementDef) lists every name its element is written
under, and every command of one heading kind is, in this way, a name of that kind's one
element value.

### Fallbacks, and replacing another feature's definition

Two more declarations concern the names of commands rather than elements.

A **fallback** ([`FallbackDef`](crate::lang::mint::FallbackDef), registered with
[`FeatureBuilder::fallback`](crate::core::registry::FeatureBuilder::fallback)) says what
a command name means in a parsing mode where no declaration defines it. The
`math-passthrough` feature declares two fallbacks, one for macros and one for
environments, both restricted to mathematics mode. In mathematics, a command that no
feature declares therefore becomes a node of the element `flm.math-passthrough.opaque`
([`OPAQUE`](crate::math_passthrough::OPAQUE)), which keeps its source
character for character and is not reported, instead of an unknown command reported as
an error.

Where several features define one command name in a parsing mode, the definition of the
feature placed last in the order of features applies (each mode is decided on its own:
a definition for text mode never meets one for mathematics mode). This is allowed when
the chains of the feature order
([`FeatureChain`](crate::core::registry::FeatureChain); a package's `feature_order`)
place that feature after every other feature that defines the name in that mode, or
when that feature declares its definition as an override with
[`override_callable`](crate::core::registry::FeatureBuilder::override_callable), giving
the command's callable kind (macro, environment or special character sequence;
[`CallableKind`](crate::lang::mint::CallableKind)) and its name. Otherwise the registry
build fails, with a message naming the features and the chain that would choose the
last one's definition. Only the override of the feature placed last is consulted; the
others' are never read.

## Aspects

An aspect is a piece of behavior attached to an element when the feature registers it
and read back through a trait object; it answers per occurrence of the element and reads
no options ([elements and aspects](crate::guide::concepts_overview#elements-and-aspects)).
A stage or a backend finds out what an element does by asking for one of its aspects
("is this a block?", "which reference points does this node carry?"), never by matching
the element's name against a list. A stage written today therefore works on an element
that a feature written later declares. A feature of an extension written in JavaScript
answers the aspects `Referenceable` and `Sectioning` through objects of its script (see
[What a feature in JavaScript adds to the
output](#what-a-feature-in-javascript-adds-to-the-output)).

Any object-safe trait whose implementations can be shared between threads (a trait with
`Send + Sync` as supertraits) can serve as an aspect: an aspect trait needs no
registration, and a feature, or a test, may define its own. An element carries at most one
implementation of each aspect. Attach an aspect when you build the element value, with
[`ElementSpec::with`](crate::model::element::ElementSpec::with), and read it back with
[`ElementSpec::aspect`](crate::model::element::ElementSpec::aspect):

```rust
use std::sync::Arc;
use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use flm_core::model::element::ElementSpec;

// Any object-safe trait with `Send + Sync` as supertraits can be an aspect; the trait
// is registered nowhere.
trait Glossary: Send + Sync {
    fn term(&self) -> &str;
}
struct Term(&'static str);
impl Glossary for Term {
    fn term(&self) -> &str { self.0 }
}

let element = ElementSpec::new("thm.term")
    .docs("a glossary term")
    .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
    .with::<dyn Glossary>(Arc::new(Term("stabilizer")));

assert!(element.has_aspect::<dyn Glossary>());
assert_eq!(element.aspect::<dyn Glossary>().map(|g| g.term()), Some("stabilizer"));
```

Two rules hold for every aspect:

- **An aspect is evaluated per occurrence.** The node is a parameter of every method, so
  the same heading element declares a different reference point at each place it
  occurs.
- **An aspect reads no options.** Aspects are consulted during the rewrite stages and
  while a plan is built, before option values are bound. A fact that varies from
  document to document is an option of the stage that uses the aspect's result, not of
  the aspect. A fact fixed by the feature's configuration is held by the element's data
  or by the aspect's implementation.

### The standard aspects

The aspects that many features implement, and the aspects that state what an element is,
are in [`model::aspects`](crate::model::aspects):

- [`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect): whether an occurrence is a
  block, inline content, or a decorator attached to its block;
- [`Referenceable`](crate::model::aspects::Referenceable): the reference points an
  occurrence declares;
- [`Referencing`](crate::model::aspects::Referencing): the reference sites an occurrence
  declares, the places that point at a reference point;
- [`Sectioning`](crate::model::aspects::Sectioning): a heading's level, whether it is
  listed in the table of contents, and what the table of contents shows for it
  ([`toc_title`](crate::model::aspects::Sectioning::toc_title)); the
  [`toc`](crate::stdcore::features::toc) feature lists every element that carries this
  aspect, a heading of an extension's own element included;
- [`ApartFromText`](crate::model::aspects::ApartFromText): the content of an occurrence
  that is not part of the text around it, such as the words of a footnote written in a
  heading's title;
- [`InCopy`](crate::model::aspects::InCopy): what replaces an occurrence in a copy of
  content shown a second time, away from where it is written;
- [`Whitespace`](crate::model::aspects::Whitespace): that each occurrence is whitespace,
  as `~` and `\ ` are. The block-structure pass removes such a node where it removes
  whitespace: beside an element whose block role removes the whitespace on that side
  ([`InlineSpacing`](crate::model::aspects::InlineSpacing), as `\cite` does before it), at
  the edges of a paragraph, and between blocks. A paragraph that a lowering rule writes
  itself, over content it cuts out of a longer run (a quotation's passage, attribution and
  lines, a `\celldata` entry), is trimmed by
  [`stage_trimmed`](crate::stdcore::backends::lowering::stage_trimmed), which removes the
  same whitespace at its edges as the pass does, whitespace nodes included, and the
  standard features' tests for blank content count such a node as whitespace (a footnote
  holding only `~` is empty), except the tests of text read as typed or with its markup,
  such as a citation key or a term, of which the node is part (a term holding only `~` is
  not empty). Everywhere else the node is kept, as a node of its own,
  which its element's lowering rule lowers. The aspect has no method: attach its
  implementation
  [`IsWhitespace`](crate::model::aspects::IsWhitespace), and read it with
  [`is_whitespace_of`](crate::model::aspects::is_whitespace_of).

Three aspects are read when a document is rendered and are declared with the elements of
the presentation vocabulary (the presentation elements the standard backends write;
[`pres`](crate::stdcore::backends::pres)): [`Decoration`](crate::stdcore::backends::pres::Decoration),
[`OpaqueContent`](crate::stdcore::backends::pres::OpaqueContent) and
[`ContentInOpaque`](crate::stdcore::backends::pres::ContentInOpaque).

Three further aspects are declared beside the rewrite pass that reads them, because they
mean nothing apart from it:

- [`Expand`](crate::lang::rewrites::userdefs::Expand), through which an element rewrites
  its own use sites while user definitions are expanded;
- [`BlockStructure`](crate::lang::rewrites::blocks::BlockStructure), through which an
  element lays out its own content while paragraphs and blocks are built;
- [`BlockTransparent`](crate::lang::rewrites::blocks::BlockTransparent), through which a
  [block-transparent](crate::guide::concepts_overview#block-transparent-elements) element
  is cut into parts before any paragraph is built.

The rewrite pass calls each of these through an interface of its own (an
[`Expander`](crate::lang::rewrites::userdefs::Expander), a
[`BlockStructurer`](crate::lang::rewrites::blocks::BlockStructurer), a
[`BlockCutter`](crate::lang::rewrites::blocks::BlockCutter)), in the same way as a
lowering rule works through a [`Lowerer`](crate::core::lowering::Lowerer): the
element's implementation never accesses the rewrite pass itself.

### A block role that depends on the position

An element whose block role depends on where it is written returns its role from its
aspect, which receives the node. The `cells` feature's `\cell` is a block where it is
a direct child of the body of a `cells` environment, and inline content anywhere else
([`InBody`](crate::stdcore::features::cells::InBody)); the `graphics` feature's image is
a block where it is a direct child of a float's body. Keep the test in one function
([`cells::stands_in_body`](crate::stdcore::features::cells::stands_in_body)), and make
the element's lowering rule, or any other reader, either call that function or read
exactly the nodes the function accepts (the `cells` reader reads the direct children of
the body, which are those nodes and no others), so that the role and the reading cannot
disagree. Make the function test the element of the environment whose body holds the
node (the node's owner), not any other node that this environment holds.

### Reference points and labels

A reference point is one place in a document that can be referred to, numbered, or
given an anchor (a heading, an equation line, a theorem, a footnote); an element
declares its reference points per occurrence through the
[`Referenceable`](crate::model::aspects::Referenceable) aspect, and the `refpoints`
feature records them per tree ([`RefPoint`](crate::model::aspects::RefPoint)). A label is
the text an author writes as `\label{type:label}` after a construct, which the construct
reads as a field and attaches to its reference point ([reference points, labels and
anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).

**A construct with labels takes three steps.**

1. Declare [`label_fields_arg`](crate::stdcore::features::refs::label_fields_arg) as
   the element's last argument, so that `\thing{…}\label{x:y}` is parsed.
2. Read the fields with [`label_uses`](crate::stdcore::features::refs::label_uses), and
   split them with [`point_labels`](crate::stdcore::features::refs::point_labels) into
   the labels written `type:label` and the texts that are not, which the `refs` feature
   then reports.
3. Put both lists on the [`RefPoint`](crate::model::aspects::RefPoint) that your
   `Referenceable` implementation returns.

Nothing else reads those fields, and nothing else has to: a `\label` is data your
construct absorbed. If your construct has a lowering rule of its own on the recomposed
LaTeX, write its labels there with
[`stage_label`](crate::stdcore::features::refs::stage_label) and
[`stage_label_group`](crate::stdcore::features::refs::stage_label_group); the rule must
declare what those functions read and produce, which their documentation lists (among
others, the table of safe labels
[`flm.labels.safe`](crate::stdcore::features::refs::SAFE_LABELS)).

A reference point states **the two things a reference to it may show**:

- a [number source](crate::model::aspects::RefPoint::number), where the construct can be
  numbered at all. A number source
  ([`NumberSource`](crate::model::aspects::NumberSource)) is what a reference point says
  it is numbered by: the counter, the counter style, the scoped instances it counts
  under, and optionally a number template of its own;
- [content](crate::model::aspects::RefPoint::content), where the construct has content
  of its own: an argument of the node, its body, one node of its tree, or, for content
  that no node holds by itself (a label text that the feature's configuration supplies, a
  part of a run of characters), a small tree that the construct builds
  ([`ContentRef::Synthesized`](crate::model::aspects::ContentRef::Synthesized)).

A reference shows the first of them that exists: the number, where the document numbered
the point; else the content; else the label the reference was written with. A construct
states both, one, or neither. A point that states a number source is numbered by the
[numbering](crate::stdcore::features::numbering) feature, which has no setting that
turns numbering off for a point; whether a point states one is therefore the construct's
decision, taken from its node and from its feature's configuration. The headings feature
states a number source for a heading whose kind its configuration numbers (no kind, by
default), so a reference to a heading shows the heading's number in a registry that
numbers that kind, and its title in one that does not. The mathematics feature states one
for every line that has neither `\nonumber` nor `\tag`, unless its configuration numbers
no equations; a tagged line states its tag as content instead. A
construct of your own is numbered in the same way (see [Number a construct of your
own](#number-a-construct-of-your-own)).

A reference point also declares **which label types may be written for it**
([`RefPoint::accepts`](crate::model::aspects::RefPoint::accepts)). A label is written
`type:label`, and the label type says what sort of thing the label marks.
[`AcceptedRefTypes::these(["thm", "lem"])`](crate::model::aspects::AcceptedRefTypes)
accepts those two types and refuses every other, and
[`AcceptedRefTypes::Any`](crate::model::aspects::AcceptedRefTypes) accepts every type,
which suits a construct whose label types are not fixed in advance. A refused label
defines nothing: no reference resolves through it, a point whose every label was refused
gets no anchor from a label, and the label check reports
`flm.refs.label-type-not-accepted`.

### Templates with values the feature computes

A feature may write its macros as templates, like user definitions, and also place in
those templates values that the feature computes at each use, such as a delimiter drawn
at the size the use asked for. Such a feature declares its macros as
[`Definition`](crate::lang::rewrites::userdefs::Definition)s and declares the extra
placeholders of each with
[`Definition::extra_placeholder`](crate::lang::rewrites::userdefs::Definition::extra_placeholder)
(`#{name}`, or `#[…]{name}` when the placeholder takes a substitution argument). A
placeholder is a reference in a template to an argument, the body, or a name whose
content an expansion hook computes ([the template
language](crate::lang::rewrites::userdefs::template)).

The templates compile as every user definition's do, arguments, default values and
parsing modes included, through
[`UserDefinition::compile_with`](crate::lang::rewrites::userdefs::UserDefinition::compile_with),
called from a [`LateInit`](crate::core::registry::LateInit) hook (see [Build
hooks](#build-hooks)). The feature then attaches its own `Expand` aspect to the element,
in place of the definition's, and instantiates the definition's template with
placeholder bindings of its own: they give the content of the extra names and pass every
other reference to
[`DefinitionBindings`](crate::lang::rewrites::userdefs::DefinitionBindings), the
bindings a user definition's own expansion uses. `#1`, a named argument and a default
value therefore mean the same in the feature's templates as in an author's.

A value the feature computes is given as a finished tree
([`PlaceholderValue::Tree`](crate::lang::rewrites::userdefs::PlaceholderValue::Tree)),
which the feature can prepare once, when the registry is built: it compiles the value's
text with
[`compile_template_with`](crate::lang::rewrites::userdefs::compile_template_with)
through [`compile_in_modes`](crate::lang::rewrites::userdefs::compile_in_modes), which
applies the rule a user definition's text follows: the text is kept in each parsing mode
where it compiles, and its failures are reported only when it compiles in none. The
feature's values and the definitions' templates then exist in the same parsing modes.

A feature may also compile a template that refers to no argument of a use site, only to
values the feature computes (for example a heading's title line `#{number}. `, compiled
from the feature's configuration), and instantiate it outside the expand pass, from a
lowering rule. [`instantiate`](fn@crate::lang::rewrites::userdefs::instantiate)
instantiates a compiled template under any placeholder bindings, as a new tree whose
root's children the rule splices into its output
([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)).
[`ValueBindings`](crate::lang::rewrites::userdefs::ValueBindings) are the bindings for
such a template: `ValueBindings::new().with("number", tree)` gives the extra placeholder
`#{number}` the finished tree `tree`; an extra placeholder they hold no tree for has no
value (`\IfNoValueTF{#{name}}` takes its first branch), and an argument or the body,
which such a template does not have, makes `instantiate` fail. The instantiation
descends the template recursively, so it runs on a worker thread of the library, as a
lowering rule and an expansion hook do ([the worker-thread
requirement](crate::lang::guard#the-worker-thread-requirement)).

Three more functions let such a feature register its definitions without repeating any
rule of the user definitions:

- [`UserDefinition::element_def_with`](crate::lang::rewrites::userdefs::UserDefinition::element_def_with)
  builds the element and its declaration as for a user definition, but with the
  feature's `Expand` aspect: the element still carries the definition as its data, its
  block role and its parsing modes.
- [`UserDefinition::variant_for_use`](crate::lang::rewrites::userdefs::UserDefinition::variant_for_use)
  returns the compiled templates for a use site, or the condition to report where the
  definition cannot expand that use (the definition was refused when the registry was
  built, or its text did not parse in the use site's parsing mode).
- [`element_name_under`](crate::stdcore::features::userdefs::element_name_under)
  derives the element's type name from the feature's own prefix (for example
  `mypack.macro.ket`), and
  [`expand_in_tree_rule_for`](crate::stdcore::features::userdefs::expand_in_tree_rule_for)
  builds the element's lowering rule for every backend. That rule requires the trees
  table that the `userdefs` feature's rewrite stage for the definition's expansion policy
  writes ([`ALWAYS_EXPANDED`](crate::stdcore::features::userdefs::ALWAYS_EXPANDED) for an
  `always` definition, [`EXPANDED`](crate::stdcore::features::userdefs::EXPANDED) for a
  `per-plan` one), so the plan schedules that stage. Only the `userdefs` feature declares that table, so a registry that holds such a rule without the `userdefs`
  feature does not build.

The crate `flm-phfpack` uses these functions (see [An extension crate, end to
end](#an-extension-crate-end-to-end)).

### User definitions in the recomposed LaTeX

On the `latex-recomposed` backend, a user definition is not only expanded: the backend
writes it back as a LaTeX definition of its own in the style file (the `.sty` file
written beside the recomposed LaTeX document), and writes each use site as a call of it.
Everything a definition can declare is written back in this way: arguments of every form
listed above, in any position and in any number, default values, the fourteen
conditionals of the template language, a name declared twice (once per parsing mode),
and a definition that uses itself.

Three kinds of definition are written expanded at every use site instead:

- a definition of a **special character sequence**, whose trigger is not a LaTeX command;
- a definition marked **always expanded**, which asked for it;
- a definition the registry **refused**, whose refusal was reported when it happened.

A definition that uses one of these three is written expanded with it. So is a definition
whose template holds a placeholder that no use site gives (one that an expansion hook of
a feature computes, or one written with a substitution argument); no definition written
in a document's settings can hold one. The rendering guide shows the form a call and a
definition take, and lists what is written expanded and why ([user definitions in the
recomposed LaTeX](crate::guide::rendering#user-definitions-in-the-recomposed-latex)).

### Attaching an aspect to someone else's element

[`FeatureBuilder::attach`](crate::core::registry::FeatureBuilder::attach) gives an
aspect to every element value whose element type has a given name, including other
features' elements and the language's structural elements:

```rust
use std::sync::Arc;

use flm_core::model::element::ElementSpec;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry,
};
use flm_core::lang::mint::{ElementDef, SyntaxDef};

// The aspect one feature wants every "box" to have.
trait Fillable: Send + Sync {
    fn fill(&self) -> &str;
}
struct Solid;
impl Fillable for Solid {
    fn fill(&self) -> &str { "solid" }
}

// The feature that owns the element, and does not depend on `Fillable`.
struct Shapes;
impl FlmFeature for Shapes {
    fn info(&self) -> FeatureInfo { FeatureInfo::new("shapes", "Shapes") }
    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.element(ElementDef::new(
            "shapes",
            ["shapebox"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(ElementSpec::new("shapes.box")),
        ))
    }
}

// The feature that gives the element behavior, and does not own it.
struct Filling;
impl FlmFeature for Filling {
    fn info(&self) -> FeatureInfo { FeatureInfo::new("filling", "Filling") }
    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.attach::<dyn Fillable>("shapes.box", Arc::new(Solid))
    }
}

let registry = build_registry([
    Arc::new(Shapes) as Arc<dyn FlmFeature>,
    Arc::new(Filling) as Arc<dyn FlmFeature>,
])?;

let element = registry.element_named("shapes.box").expect("declared above");
assert_eq!(element.aspect::<dyn Fillable>().map(|f| f.fill()), Some("solid"));
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

A feature gives behavior in this way to elements it does not own, and to the language's
structural elements, which no feature declares (`flm.text`, `flm.group`;
[structural elements](crate::lang::structural::StructuralElements)). The
`lowering` feature uses it to mark elements of the presentation vocabulary: it attaches
[`Decoration`](crate::stdcore::backends::pres::Decoration) to the three decoration
elements, [`OpaqueContent`](crate::stdcore::backends::pres::OpaqueContent) to the two
elements whose content is source text, and
[`ContentInOpaque`](crate::stdcore::backends::pres::ContentInOpaque) to
`flm.pres.math-text`; the lowering driver (the shared code of
[`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage) that applies the
rules) recognizes those elements by their aspects,
not by their names.

Give the aspect as its **trait-object type** (`attach::<dyn Fillable>`): an aspect is
looked up by the trait it is requested as, and a concrete type fails to compile. Two
mistakes are registry build errors rather than silent successes: attaching to an element
type name that no feature declares, and attaching an aspect that an element already
carries.

## Tables and stages

A table is typed data that one stage writes beside the frozen trees and other stages
read; its identity is its registered dotted name, never its Rust type, and that name is
also a capability ([`Table`](crate::core::data::table::Table)). A frozen tree is a
document tree after the rewrite stages have run: it is never changed again, and
everything a later stage computes about it is stored in tables beside it, keyed by node
address ([frozen trees and computed
data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).

To define a table, implement [`Table`](crate::core::data::table::Table) on its value
type. The only required method is [`absorb`](crate::core::data::table::Table::absorb),
which adds to the accumulated value a table piece (the part of a table that one stage
invocation produced) of the same table; the runner passes the pieces of a per-tree stage
in the run's order of the trees. Two methods have defaults:
[`project`](crate::core::data::table::Table::project) writes the value for dumps and
tests, and [`piece_class`](crate::core::data::table::Table::piece_class) says whether a
piece states a fact about the document as a whole, which [value
catch-up](crate::guide::concepts_overview#value-catch-up) needs to know.

A stage is one unit of work of the pipeline: it declares the capabilities it provides
and requires, the tables it reads and writes, its effect (compute, check, or
input/output) and its scope (once per tree, or once per document), and the plan decides
whether and when it runs ([`Stage`](crate::core::stage::Stage)). There are two kinds:

- A [`RewriteStage`](crate::core::stage::RewriteStage) runs *before* the trees are
  frozen and turns one tree into another. It reads no computed data, and it publishes
  the trees it produced in a table named after the capability it provides
  ([`TreesTable`](crate::core::stage::TreesTable)). The standard package has three:
  the expansion of user definitions (the `userdefs` feature), the insertion of included
  files (the `input` feature), and the layout of content in paragraphs and blocks (the
  `blocks` feature).
- A [`Stage`](crate::core::stage::Stage) runs *after* the trees are frozen. It reads the
  frozen trees and the tables it declared, and writes tables of its own.

A stage describes itself in two parts, at two different moments:

- Its **declaration** ([`StageDecl`](crate::core::stage::StageDecl)), returned by
  [`Stage::decl`](crate::core::stage::Stage::decl), is fixed when the feature registers
  the stage: its name; its [`Effect`](crate::core::stage::Effect) (it computes data,
  only reports diagnostics, or has an effect outside the computed data, such as handing
  an output's files to the program that writes them); its
  [`Scope`](crate::core::stage::Scope) (once per tree, or once for the whole document);
  the capabilities it provides; and the tables it writes.
- Its **needs** ([`Needs`](crate::core::stage::Needs)) are computed once per plan by
  [`Stage::needs`](crate::core::stage::Stage::needs), from the plan request: the
  capabilities it requires, the tables it reads (required or optional), the option types
  it reads, and its ordering edges. Most stages return a constant; a stage whose inputs
  depend on the plan request computes them there, as a backend's lowering stage does.

Declaring a table as written provides that table's capability, and declaring a table as
read requires it, so a stage's author lists the reads and writes and nothing else.

The `index` feature ([`index`](crate::stdcore::features::index)) is one table and one
stage: it declares one table and registers one `Compute` stage of `PerTree` scope that
requires nothing; the stage walks each frozen tree once and records which nodes carry
which element. The following example declares a table and a stage the same way:

```rust
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::core::data::table::{Table, TableName};

// A table is any type that can absorb another piece of itself.
#[derive(Debug, Default)]
struct WordCount(usize);

impl Table for WordCount {
    fn absorb(&mut self, piece: WordCount) {
        self.0 += piece.0;
    }
}

const WORDS: TableName<WordCount> = TableName::new("example.words");

let decl = StageDecl::new("example.words.count", Effect::Compute, Scope::PerTree)
    .writes(WORDS)
    .docs("counts the words of each tree");
let needs = Needs::new().requires("flm.blocks.structured");

// Writing a table provides its capability, so nothing else has to be declared.
assert!(decl.provides.is_empty());
assert_eq!(needs.requires.len(), 1);
```

In the feature's `register` method, declare the table with
[`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table) (or with
[`table_named`](crate::core::registry::FeatureBuilder::table_named) when the name is
computed, as `flm.lowered.<backend>` is), and register the stage with
[`FeatureBuilder::stage`](crate::core::registry::FeatureBuilder::stage) or
[`FeatureBuilder::rewrite_stage`](crate::core::registry::FeatureBuilder::rewrite_stage).

A feature written in JavaScript ([Extensions](#extensions)) registers a rewrite stage
with `b.rewrite_stage(stage)`, where `stage` is an object with three methods: `decl()`
answers the stage's name and the capability it publishes (`new RewriteDecl(name,
publishes)`), `needs()` its ordering edges, and `restage(node, cx, rs)` decides what
becomes of one node. The script does not build the new tree itself: the library walks
the tree and calls `restage` once for each node it reaches. The handler answers
`Restage.Descend` to keep the node, whose children the library then visits in turn, or
`Restage.Emit([...])` with the nodes it created in the node's place through `rs`, the
*restager*: characters with `rs.stage_chars(text, like)`, a copy of a part of the tree
with `rs.restage_subtree(node)` (whose nodes go through `restage` again), a copy of a
macro call with new content in an argument with `rs.restage_argument_with_content` and
`rs.restage_invocation`. A handler that fails leaves its node as it was, with the Error
`flm.ext.handler-failed` at the node, and the rest of the tree is rewritten. An
operation after which the new tree cannot be built (for example the tree's root replaced
by two nodes) abandons the rewrite of that tree, with an Error at its root, and the tree
is absent from the output. The test extension `smartdash` (the folder
`flm-ext/tests/js/smartdash/` of the FLM repository) replaces `--` and `---` by dashes
outside mathematics and verbatim content, and sets its macro `\sdquote{…}` between
quotation marks; the documentation of the module `flm_ext::quickjs` describes every method
of `rs`.

An external table is a table that no stage of the run writes: the caller installs its
value when the run is constructed ([`Externals`](crate::core::run::Externals); declared
with [`external_table`](crate::core::registry::FeatureBuilder::external_table)). Data from
outside the processing of one document takes part in a plan in this way: the `refs`
feature declares one for the labels of the *other* documents of a
[corpus](crate::guide::concepts_overview#documents-fragments-and-the-corpus).

## Capabilities, collections, ordering and workflows

A capability is a registered dotted name for a state of processing that a stage brings
about, such as `flm.blocks.structured`: a stage declares the capabilities it provides and
the capabilities it requires, and a caller demands the capabilities it wants
([`CapabilityDef`](crate::core::registry::CapabilityDef)). To demand a capability is to
ask the plan for a stage that provides it ([`PlanRequest`](crate::core::plan::PlanRequest)).
Every table already declares its own capability, so a feature declares a capability with
[`FeatureBuilder::capability`](crate::core::registry::FeatureBuilder::capability) only
for a state that no table represents: a marker, the result of a check, or the parent of
refinements.

A capability may **refine** another, which means that it is a stronger form of it
([refinement](crate::guide::pipeline#refinement)):
`flm.blocks.structured` and `flm.blocks.raw` are two refinements of `flm.blocks`, so a
stage providing either satisfies a demand for `flm.blocks`, and a plan that demands both
is refused. At most one refinement of a capability may be marked
[`default`](crate::core::registry::CapabilityDef::default); a demand for the parent
alone selects it when nothing else decides.

A capability can carry what a plan error says when a plan needs it and no registered stage
provides it ([`unprovided`](crate::core::registry::CapabilityDef::unprovided)): the
condition in the words of the feature that declares the capability, with its fix when the
fix is not obvious. An extension gives the text as the third argument of `b.capability(name, docs,
unprovided)`, or `null` (`None` in Rust) for the generic text.

A **collection** ([`CollectionDef`](crate::core::registry::CollectionDef)) is a name for a
set of capabilities that any feature may add to; demanding the collection demands every
member. `flm.validated` is one. A feature that provides a check declares its own check
capability and adds it to the collection with
[`collection_member`](crate::core::registry::FeatureBuilder::collection_member). Every
workflow of the standard package then runs that check too: the `check` workflow demands
the collection and nothing else, and each rendering workflow demands it beside its
output. A collection is only demanded; no stage provides one. A member added to a
collection that no feature of the registry declares is dropped without a diagnostic, so
a feature that provides a check can be registered on its own.

An **ordering edge** says that a stage runs after, or before, the stage that provides
some capability. A stage ordinarily gets its place in the order from what it requires and
reads; edges exist for the case where one feature must place its stage within another
feature's order without needing that work's results. **An edge concerns order only**: it
never causes a stage to be scheduled, and it has no effect when the stage that provides
the capability is not scheduled.

There are two ways to declare an edge. A stage declares its own edges in its needs, with
[`Needs::after`](crate::core::stage::Needs::after) and
[`Needs::before`](crate::core::stage::Needs::before); the block-structure rewrite does
so, adding an edge after the expansion of user definitions only when the registry
declares that capability. An [`OrderingDef`](crate::core::registry::OrderingDef),
registered with [`FeatureBuilder::ordering`](crate::core::registry::FeatureBuilder::ordering),
declares an edge from outside: any feature may contribute edges on any registered stage.
A feature orders a stage it does not own in this way.

A **workflow** ([`WorkflowDef`](crate::core::registry::WorkflowDef)) is a named set of
demands together with the choices that go with them: which outputs to produce, which
other capabilities to demand, which stages and lowering rules to prefer, which elements
to accept as unrenderable, and which render option values to set. A workflow is data held
in the registry, so a caller chooses one by name instead of writing a plan request by
hand. A workflow may extend another, and inherits the fields it does not declare.

The `flm` program and its host library `flm-main` choose a workflow through a
*profile*, a name given with the option `-p` or the front matter's `default_profile`:
the profile `NAME` runs the workflow `NAME` (`flm render -p text` runs the workflow
`text`). A workflow that an extension declares is therefore run by its name as the
profile, once the document uses the extension. An extension named `profile-NAME`
needs no such listing: the profile `NAME` alone loads it, after the document's other
packages, and an extension of that name must declare the workflow `NAME`. A workflow can
so come with the packages it needs, listed under `use` in the extension's manifest, and
with its own settings, in the workflow's `settings`, which apply only when the workflow
runs:

```yaml
# _flmextensions/profile-draft.yaml
flm:
  use: [mystyle]
  workflows:
    draft:
      extends: html
      settings:
        html: {heading_tags: {section: h3}}
```

`flm render -p draft paper.flm` then uses `mystyle` and runs `draft`; a command with
another profile uses neither the extension `profile-draft` nor `mystyle`. A feature of a
scripted extension `profile-NAME` may name what it declares after the profile as well as
after the extension's stem ([Extensions](#extensions)): its output `NAME`, its tables,
stages and diagnostics `NAME.SOMETHING`, so that the output, like the workflow, carries
the profile's name. The guide of the `flm` program describes profiles in full
([Profiles and workflows](crate::guide::command_line#profiles-and-workflows)).

## Lowering rules

Lowering rewrites each frozen tree into a presentation tree for one backend, by applying
the lowering rule selected for each element, in rounds, until only elements the backend
writes natively remain; the result is computed data in the table `flm.lowered.<backend>`
([backends, lowering and
outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)). A lowering rule
is a named registry entity written for one element: it declares what it requires, reads
and produces, and implements `Lower`, which stages the element's presentation for one
backend ([`LoweringRule`](crate::core::lowering::LoweringRule)). To stage a node is to
build it into the tree that a lowering rule or a rewrite hook is producing, through the
context the rule or hook is handed; the staged nodes exist in that tree only
([`Lowerer`](crate::core::lowering::Lowerer)).

A feature registers a rule with
[`FeatureBuilder::lowering_rule`](crate::core::registry::FeatureBuilder::lowering_rule).
A [`LoweringRule`](crate::core::lowering::LoweringRule) holds:

- a name;
- the element it lowers;
- a [`LoweringDecl`](crate::core::lowering::LoweringDecl), which says what the rule
  needs and which presentation elements it may build;
- the [`Lower`](crate::core::lowering::Lower) implementation;
- a [`DefaultMark`](crate::core::lowering::DefaultMark), which says on which backends the
  rule is chosen when nothing else decides.

A rule is written for an element, not for a backend, so it is written once and used by
every backend whose plan selects it. Where an output format needs something more
specific, a second rule for the same element is registered with
[`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends) for that
backend, and the plan selects it there. A rule that a caller must ask for by name is
registered with [`DefaultMark::Never`](crate::core::lowering::DefaultMark::Never): the
`userdefs` feature's `instantiate-at-lowering` rule is one, an alternative to two default
rules, which a plan request selects with
[`LoweringSelection::prefer`](crate::core::plan::LoweringSelection::prefer)
([`userdefs::kept`](crate::stdcore::features::userdefs::kept)). Lowering rule selection
chooses one rule per element and backend in a fixed order: the rule the request prefers
by name, else the backend's native emission, else a rule marked default on that backend,
else a rule marked default everywhere, else the sole candidate
([`SelectedRules`](crate::core::plan::SelectedRules)).

The declaration lets a rule take part in a plan without the rule being called: plan
construction reads it. The capabilities the rule requires, the tables it reads and the
option types it reads are added to the needs of its backend's lowering stage, so a rule
that reads the table of resolved references causes the stage that writes that table to
be scheduled. The presentation elements the rule declares it produces are checked
against what it stages: building an element it did not declare is an error
([`LowerError::UndeclaredProduces`](crate::core::lowering::LowerError::UndeclaredProduces)).

A table read with [`reads_opt`](crate::core::lowering::LoweringDecl::reads_opt) is read
only if the run holds it, and the rule then works without the feature that owns the
table. The backend's lowering stage turns such a read into a required read whenever the
registry declares the table and some stage writes it, so that the stage writing it is
scheduled. The read stays optional for a table that no stage writes (an external table,
which the caller installs), and it is left out of the stage's needs for a table that no
feature of the registry declares (a peer declaration that resolved to nothing, described
below). In both of these cases the rule reads `None` when the run does not hold the
table.

### Peer declarations

A rule registered for one backend often refers to things that its feature does not
register: the backend it is the default on; presentation elements that belong to that
backend's family (the `headings` feature's rule for the `latex-recomposed` backend stages
the LaTeX family's macro construct); and sometimes a table another feature computes (the
same rule reads the table of safe labels
[`flm.labels.safe`](crate::stdcore::features::refs::SAFE_LABELS) to write the safe label
of each of the heading's labels). The registry build refuses a reference to anything that
no feature declares, and the feature must not depend on the backend's feature, or a host
that renders plain text only
would have to register the LaTeX features too.

The feature therefore makes a **peer declaration** of each. A peer declaration is a
feature's forward declaration of an element, element type, table, service slot, scoped
package or backend that another feature owns, with the element's payload type or the
table's value type as the signature; the registry build resolves it after every feature
has registered, and a lowering rule that depends on a name nobody declares is set aside
(excluded from rule selection, with one build Note)
([`PeerName`](crate::core::registry::PeerName)). A peer declaration registers nothing,
and it counts only for the feature that makes it. There are six forms:

- [`peer_backend`](crate::core::registry::FeatureBuilder::peer_backend) declares a
  backend by name;
- [`peer_element`](crate::core::registry::FeatureBuilder::peer_element) declares a
  presentation element by name, with its payload type as the signature;
- [`peer_table`](crate::core::registry::FeatureBuilder::peer_table) declares a table by
  name, with its value type as the signature. A peer table is another feature's table,
  computed by that feature's stage; it differs from an external table, which the caller
  installs;
- [`peer_slot`](crate::core::registry::FeatureBuilder::peer_slot) contributes a value to
  a service slot that another feature owns (see [Services and service
  slots](#services-and-service-slots));
- [`peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package)
  declares a scoped package that this feature's bodies and arguments refer to (see
  [Scoped packages](#scoped-packages));
- [`peer_element_value`](crate::core::registry::FeatureBuilder::peer_element_value)
  declares one more value, with the commands that produce it, of an element type that
  another feature declares: for example a float type of your own, of the element type
  that the `floats` feature declares and lowers. The declaration is kept where some
  feature declares an element of that type name in any way (with
  [`element`](crate::core::registry::FeatureBuilder::element), inside a scoped package,
  as a fallback, a presentation element or a field). Where none does, the declaration is
  dropped with one Note: its commands are unknown commands, so no node of that element
  exists, and no plan is refused because no rule lowers it.

When the registry is built, every peer declaration is resolved against what the features
registered. A name that some feature declares resolves: an element's payload type and a
table's value type are checked, and a mismatch is
[`BuildError::PeerElementMismatch`](crate::core::registry::BuildError::PeerElementMismatch)
or [`BuildError::PeerTableMismatch`](crate::core::registry::BuildError::PeerTableMismatch);
the rule then takes part like any other. A name that no feature declares leaves the
declaration unresolved: the build succeeds, and one Note
([`UnresolvedPeer`](crate::core::registry::UnresolvedPeer)) joins the registry's build
diagnostics. A rule that *depends* on the unresolved name is then **set aside**: it is
excluded from rule selection and from the check that every element can be lowered, in
that registry; a registry built with the missing feature includes the rule. A rule depends on the backend it is
the default on, on every element it produces, and on every table it reads with
[`reads`](crate::core::lowering::LoweringDecl::reads). A table read with
[`reads_opt`](crate::core::lowering::LoweringDecl::reads_opt) is the exception: the rule
runs and reads the table as absent. A rule uses such a read to write what it can without
the peer feature: the heading rule reads the anchors table in this way, so in a registry
without the `refs` feature, headings are printed without anchors. With a required read,
the rule would be set aside, and headings would have no rule on any backend.

A stage of the feature may also *write* a peer table: the peer feature declares the
table, and this feature's stage writes it. The `graphics` feature's publish stage writes
the copies of images into an output's artifact table, which the backend declares. A stage
that writes an unresolved peer table is not registered at all, since it would have no
table to write and the feature that reads the table is absent; the Note gives the stage's
name.

The frozen registry lists the peer declarations with their outcome
([`peer_backends`](crate::core::registry::FlmRegistry::peer_backends),
[`peer_elements`](crate::core::registry::FlmRegistry::peer_elements),
[`peer_tables`](crate::core::registry::FlmRegistry::peer_tables),
[`peer_slots`](crate::core::registry::FlmRegistry::peer_slots),
[`peer_scoped_packages`](crate::core::registry::FlmRegistry::peer_scoped_packages),
[`peer_element_values`](crate::core::registry::FlmRegistry::peer_element_values)) and the
rules set aside ([`set_aside_rules`](crate::core::registry::FlmRegistry::set_aside_rules)),
so a misspelled name can be found on any registry that holds the feature. The `headings`
feature's [`latex::register`](crate::stdcore::features::headings::latex::register) shows
the usual set of peer declarations for a rule on the recomposed LaTeX.

### Presentation elements of your own

A **presentation element** is an element of the trees that lowering produces, which a
backend writes natively or lowers further; the shared [presentation
vocabulary](crate::stdcore::backends::pres) is the set the standard backends write. A
feature declares a presentation element of its own with
[`presentation_element`](crate::core::registry::FeatureBuilder::presentation_element),
which takes the element's name and its payload type and returns the element value your
rules build nodes of. A feature needs one only when no element of the shared vocabulary
can carry what its element means, and a backend must then write the new element natively
for anything to be emitted; most features need only a lowering rule.

A feature written in JavaScript ([Extensions](#extensions)) declares a presentation
element with `b.presentation_element(name, docs)`. The payload of every such element is
data: a rule gives any data value as the payload when it stages the node,
`lw.pres(cx, element, { kind: "warning" }, children)` (the rule lists the element among
the elements it produces), and a stage reading the lowered trees gets the value back with
`node.payload()`. Only a backend that lists the element in its native set receives it, so
the feature registers two rules for the element of the document that the new element
presents: one marked as the default on the backend that writes the new element
(`DefaultMark.OnBackends([backend])`), staging it; and one marked as the default
everywhere, lowering the element into the shared vocabulary for every other backend.

### Format-native output: the construct elements

A construct element is a presentation element that mirrors one fundamental construct of
an output format (an HTML tag; a LaTeX macro call, environment, raw text or definition),
so that format-specific output is structured data in the lowered tree rather than a
string ([backends, lowering and
outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)). A rule stages
one where it must produce something that an output format has and the shared vocabulary
does not. Only the backends of the construct's own format write it natively, and it is
not part of the shared vocabulary, so a rule that stages one is a rule for that format
and is registered as the default on that format's backend.

The LaTeX family ([`latex::construct`](crate::stdcore::backends::latex::construct),
registered by the `latex` feature) has four construct elements: a macro call, an
environment, raw text and a definition. The HTML backend
([`html::construct`](crate::stdcore::backends::html::construct), registered by the `html`
feature) has one, `flm.pres.html.tag`, whose payload
([`HtmlTagPayload`](crate::stdcore::backends::html::HtmlTagPayload)) holds one tag: its
name; its attributes in the order they are written; its content, which is either the
node's own content slot or a string of already-formed HTML
([`TagBody`](crate::stdcore::backends::html::TagBody)); and whether the tag is
block-level, which decides the line breaks the emitter writes around it.

A tag construct is outside the HTML contract (the classes and attributes the emitter
derives from the presentation vocabulary; [the HTML
contract](crate::guide::rendering#the-html-contract)), and outside the policy that
decides which addresses a link or an image may carry: the rule that stages the tag writes
its own classes, attributes and addresses, and is responsible for them. Where the tag's
content is already-formed HTML, the rule's author creates it with
[`Markup::trusted`](crate::stdcore::backends::html::Markup::trusted) and accepts the
requirement stated there: the bytes are HTML that is safe to place in a page, and
anything that came from an input the author does not control is sanitized first. Outside
the library, bytes reach a page without being escaped in two ways only: through that
constructor, and through
[`HtmlSink::raw_text_element`](crate::stdcore::backends::html::HtmlSink::raw_text_element),
which writes the content of a `<style>` or a `<script>`.

A feature that stages a construct element registers neither the element nor the backend
it writes for: it makes a peer declaration of both (see [Peer
declarations](#peer-declarations)), so that it builds in a registry that does not hold
that backend, with its rule set aside.

### Adding an output format

A backend is the registration bundle for one output format: the elements its emitter
writes natively, a lowering stage, an emit stage and a final stage
([`BackendDef`](crate::core::registry::BackendDef)). A feature registers the four
together with [`FeatureBuilder::backend`](crate::core::registry::FeatureBuilder::backend),
so that a plan recognizes the three stages as one backend's.

The lowering stage is not written from scratch: create an instance of the shared
[`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage) for the backend's
name, and declare the table `flm.lowered.<backend>` that it writes with
[`table_named`](crate::core::registry::FeatureBuilder::table_named) under the name
[`lowered_table_name`](crate::stdcore::backends::lowering::lowered_table_name) returns.
What is left to write is the emitter and the final stage. An emitter is a backend's emit
stage: it walks the lowered trees and writes the output's bytes into a table, from which
the backend's final stage makes the artifact set.

The native set must satisfy every **native-set implication** the registry holds: a
declared requirement on what a backend's native set must contain, checked against every
backend when the registry is built
([`NativeImplication`](crate::core::registry::NativeImplication)). The lowering driver
(the shared code of [`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage)
that applies the rules) stages two elements by itself, without any rule declaring them, so no chain of
rules leads to them and plan construction cannot see that a backend needs them. The
`lowering` feature therefore declares two implications
([`native_implications`](crate::stdcore::backends::lowering::native_implications)):

- every backend must write `flm.pres.unsupported` natively, the stand-in for an element
  the backend cannot render or for a rule that failed;
- a backend that writes natively any element carrying the
  [`Decoration`](crate::stdcore::backends::pres::Decoration) aspect must write
  `flm.pres.deco-mark` natively, the mark left where a decoration was moved out of a
  content run ([decorations, roles and
  kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)).

A native set that misses one fails the build with
[`BuildError::NativeSetIncomplete`](crate::core::registry::BuildError::NativeSetIncomplete),
which gives the backend, the element it must write natively and the native element that
caused the requirement. A feature whose own code stages an element without declaring it
declares an implication of its own with
[`native_implication`](crate::core::registry::FeatureBuilder::native_implication).

An [`OutputDef`](crate::core::registry::OutputDef), registered with
[`FeatureBuilder::output`](crate::core::registry::FeatureBuilder::output), then gives the
format a name that a caller can ask for. It holds the capability a plan must reach, the
table that holds the output's artifact set, and the media type and file extension of the
entry artifact. An artifact is one file's worth of output held as a value (a folder
artifact holds the files of a folder), identified by its producer and a name and never by
a path; an artifact set is the artifacts of one
output, at most one of which is the entry artifact, the one that stands for the output as
a whole ([`ArtifactSet`](crate::core::artifact::ArtifactSet)). A
[`WorkflowDef`](crate::core::registry::WorkflowDef) that lists the output makes it
available by name. The `text` backend
([`TextBackend`](crate::stdcore::backends::text::TextBackend)) does all of this in one
`register` method.

A backend's feature registers the rules that are its own (the `latex-recomposed` backend
registers two, for paragraphs and paragraph breaks) and no rule for another feature's
elements. A feature that wants its element written in the new format ships the rule
itself, with peer declarations of the backend and of the format's presentation elements
(see [Peer declarations](#peer-declarations)). The backend therefore depends on no content
feature, and an absent content feature contributes no rule. An element that no rule
lowers makes plan construction fail, unless the plan request accepts it as unrenderable:
an accepted unrenderable element is an element the plan request lists, per backend, as
one the backend need not render; the plan builds anyway, and each occurrence lowers to a
childless `flm.pres.unsupported` node with one Error diagnostic
([`PlanRequest`](crate::core::plan::PlanRequest)).

**An output made of regions.** An output region is one named part of an output's entry
artifact (the document's content; each piece, a document-level value a backend places
beside the content ([`Piece`](crate::core::piece::Piece)); and on HTML the stylesheets, the scripts, the title and the language), which the backend writes once into
the table `flm.output.<backend>.regions` ([`Regions`](crate::core::region::Regions)). A
backend whose entry artifact a host may want to assemble in another way, through a
template, splits its final work in two, as the four standard backends do:

- a stage `flm.<backend>.regions` writes the output's regions into the table
  `flm.output.<backend>.regions`;
- the backend's final stage assembles the entry artifact from those regions, in a layout
  of the backend's own ([output regions and the final
  stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).

The backend then contributes a [`RegionsOutput`](crate::core::region::RegionsOutput) to
the service slot `flm.output.regions`
([`REGIONS_SLOT`](crate::stdcore::backends::lowering::REGIONS_SLOT)): the output's name,
its tables, and the capability its final stage provides. A host's own assembler reads
that service slot and serves the output without code of its own for it. The files written
beside the entry artifact are in the tables that the `RegionsOutput` lists: a table of
the backend's own for its own files (the recomposed LaTeX's style file), and the output's
artifact table `flm.output.<backend>.artifacts`, which only the `graphics` feature's
publish stage writes. The final stage adds them to the artifact set with
[`ArtifactSet::insert_entry_with`](crate::core::artifact::ArtifactSet::insert_entry_with),
which reports a malformed set.

**An output made from the files of another output.** Some outputs are made by processing
the files of another output instead of the document: an output of PDF files, for
instance, is made by running LaTeX over the files of the `latex-rendered` output. Such an
output needs no backend. Its feature registers a table of type
[`ArtifactSet`](crate::core::artifact::ArtifactSet), the output, and a stage that reads
the other output's table (it declares the read among its
[`Needs`](crate::core::stage::Needs), which makes the plan schedule the stages that write
that table) and writes the output's artifact set. The output's declaration lists the other
output with [`OutputDef::made_from`](crate::core::registry::OutputDef::made_from). A host
reads that list before any plan is built, to prepare those outputs as well: the `flm`
program sets up the compose template of each output in the list, so the compose template
chosen for `latex-rendered` assembles the LaTeX that the PDF is made from. The registry
build refuses a name in the list that no feature declares as an output.

The stage writes the files of the other output where it needs them (into a folder where
it runs a program, for instance) at the paths the host would write them at, so that the
links between the files hold: a LaTeX document loads its style file and includes its
images by those paths. [`path_of`](crate::core::artifact::path_of) gives the path of
each artifact: its [fixed path](crate::core::artifact::Artifact::fixed_path) when it has
one, else the path that the host's naming of files gives it (the
[`ArtifactNamer`](crate::core::artifact::ArtifactNamer) that the host contributes to the
service slot `flm.artifacts.namer`,
[`NAMER_SLOT`](crate::stdcore::backends::lowering::NAMER_SLOT), read for a scope built with
[`NamingScope::of`](crate::core::artifact::NamingScope::of) over the registry and the
plan's demands). The output's declaration, passed to `path_of`, gives an entry artifact
the extension of the output's entry file.

**A backend written in JavaScript.** A feature written in JavaScript
([Extensions](#extensions)) registers a backend and an output through the same
builder methods, `b.backend(new BackendDef(name, native, lowering, emit, final_stage))`
and `b.output(new OutputDef(name, capability, table, media_type, extension))`. The name
of such a backend, and of its output, is the extension's stem itself or begins with the
stem and a dot, since a reader types it on the command line (`-f typst`); an extension
whose stem is `profile-NAME` may also use `NAME` in place of its stem. The three stages:

- the lowering stage is the library's: `new LoweringStage(backend)`, whose table the
  feature declares with `b.table_named(lowered_table_name(backend),
  StandardTable.LoweredTrees, docs)`;
- the emit stage is a stage object, as `b.stage` takes one: it reads the lowered trees
  by the table's name (`cx.get(name).lowered`, whose `tree(id)` answers a tree's root),
  walks their nodes (`element_name()`, `payload()`, `body()`, `children()`,
  `slot_count()`, `slot_content_nodes(index)`) and pushes the text it writes into a table
  of the extension's own;
- the final stage is a stage object that reads that text and pushes the artifact set
  into a table declared with `b.table_named(name, StandardTable.ArtifactSet, docs)`:
  each artifact is written as data, `Artifact.bytes(new ArtifactId(owner, name),
  media_type, content).as_entry()` for the entry artifact or `Artifact.folder(id, [[path,
  content], …])` for a folder artifact.

**The content of a file, as data.** An extension gives the bytes of a file, the
`content` above, in one of three forms: a text or `{ text: TEXT }`, for bytes that are
UTF-8 text (the bytes of the text), or `{ base64: TEXT }`, for bytes of any kind, written
in base64 with the standard alphabet and padding (an image, a PDF file). When an
extension reads a file, its content is `{ text }` when the bytes are UTF-8 and `{ base64
}` otherwise. The host service `programs` (see [Host services](#host-services)) takes the
files it writes for a program, and gives back the files it collects, in the same two
forms, so an extension passes a file between an artifact and a program without
converting it.

**Reading the files of an output from an extension.** A stage of an extension that
declares a read of a table of type `ArtifactSet` (the table of another output, such as
`flm.output.latex-rendered.document`) gets the output's files as data:
`cx.get(name).projection` is `{ artifacts, violations }`. `artifacts` holds every artifact
of the set, ordered by identity, each as a map of seven fields: `id`, `media_type`,
`entry`, `data` (each content `{ text }` or `{ base64 }`), `fixed_path`, `refs` and
`extension` (the class `Artifact` writes five of them, without `fixed_path` and `refs`).
Each map has one more field, `path`: the path at which the host writes the file, relative
to the output's folder,
computed as [`path_of`](crate::core::artifact::path_of) computes it (a folder artifact's
files go under it, each at `path/FILE`); `null` when the file has no fixed path and the
host contributes no naming of files. Writing each file at its `path` keeps the links
between the files intact. `violations` holds the text of each problem recorded in the set
(two files of one identity, two main files). An artifact read can be pushed into another
table of files as it is: a push ignores `path`. The output made from these files lists
the output it reads with `new OutputDef(…).made_from(name)` (see the paragraph on outputs
made from another output's files above).

The native set lists the elements the emit stage writes: the presentation vocabulary
(`ALL` of `flm:stdcore/backends/pres`), the structural elements the lowered trees keep
(six of the names of `flm:lang/structural`, `TEXT`, `GROUP`, `OPAQUE_WRAPPER`, `COMMENT`,
`CONTAINER` and `PARBREAK`: a run of characters, a group, a group holding content kept as
typed, a comment, a list of nodes, a paragraph break),
and the extension's own presentation elements. The backend
covers, by its native set or by its rules, every element the documents it serves can
hold. The standard package registers some of its rules for its own backends only (the
`graphics` feature's rule for `flm.graphics.image`), so a backend written in JavaScript
registers a rule of its own for such an element, which may stage the stand-in
`flm.pres.unsupported`; an element no rule brings into the native set makes plan
construction fail. A workflow written in the package's `flm` block that lists the output
(`flm: { workflows: { typst: { outputs: ["typst"], demands: ["flm.validated"] } } }`)
makes it available by name, so that the `flm` program renders a document with `-p typst`. The emit stage must escape every character that the output's syntax
reserves, in link addresses and verbatim content as well as in text.

The test extension `typst` (the folder `flm-ext/tests/js/typst/` of the FLM repository)
writes a document as Typst markup this way: the library's lowering stage, an emit stage
in its own module `emit.js`, a final stage writing one entry artifact with the media type
`application/x-typst`, the output `typst` with the extension `typ`, the workflow `typst`,
a presentation element of its own for its macro `\tcallout[kind]{text}`, and a rule that
lowers an image to the stand-in. Its registration, abridged:

```js
const lowered = lowered_table_name("typst");                       // "flm.lowered.typst"
b.table_named(lowered, StandardTable.LoweredTrees, "the lowered trees");
b.table("typst.output", "the document as Typst markup");
b.table_named("typst.document", StandardTable.ArtifactSet, "the Typst file");
const emitStage = {
  decl: () => new StageDecl("typst.emit", Effect.Compute, Scope.Document).writes("typst.output"),
  needs: () => new Needs().reads(lowered),
  run(cx, out) { out.table("typst.output").push(emit(cx, cx.get(lowered).lowered)); },
};
const finalStage = {
  decl: () => new StageDecl("typst.final", Effect.Compute, Scope.Document).writes("typst.document"),
  needs: () => new Needs().reads("typst.output"),
  run(cx, out) {
    const [text] = cx.get("typst.output")?.ext.items() ?? [];
    if (text !== undefined)
      out.table("typst.document").push(
        Artifact.bytes(new ArtifactId("typst", "document"), "application/x-typst", text).as_entry());
  },
};
b.backend(new BackendDef("typst", [...ALL, TEXT, GROUP, OPAQUE_WRAPPER, COMMENT, CONTAINER, PARBREAK, "typst.pres.callout"],
                         new LoweringStage("typst"), emitStage, finalStage));
b.output(new OutputDef("typst", "typst.document", "typst.document", "application/x-typst", "typ"));
```

Such an output is made of no regions and contributes no
[`RegionsOutput`](crate::core::region::RegionsOutput), so a compose template cannot
assemble it; and the library's pieces stage does not run for it, so the text of a
document's footnotes, which the standard backends place as pieces beside the content, is
not written into it (the footnote marks are).

### Templates and template engines

The library's feature [`compose`](crate::stdcore::features::compose) assembles each
output made of regions through a **compose template**: a text with named places for an
output's regions, chosen by name (the `flm` program's option `-t`). The feature is not
part of the package `stdcore`: a host that wants it registers it, as the `flm` program's
host library `flm-main` does in its package `stdmain`. It declares two service slots, and a
feature of any package contributes to them as the package `stdmain` does:

- `flm.compose.templates` ([`TEMPLATES_SLOT`](crate::stdcore::features::compose::TEMPLATES_SLOT))
  takes the templates ([`Template`](crate::stdcore::features::compose::Template)). A template holds its name, the output it
  serves, its engine, its text, the pieces it accepts (every piece, or a list of names:
  the pieces the render then builds for that output), the pieces it requires (a plan in
  which no stage provides one fails), its own settings with their defaults, the files
  it ships beside the output's main file
  ([`TemplateArtifact`](crate::stdcore::features::compose::TemplateArtifact)), and the
  handle through which it reads the files it includes
  ([`Template::files`](crate::stdcore::features::compose::Template::files); see below).
- `flm.compose.engines` ([`ENGINES_SLOT`](crate::stdcore::features::compose::ENGINES_SLOT))
  takes the template engines, each a value that implements
  [`TemplateEngine`](crate::stdcore::features::compose::TemplateEngine). The feature
  contributes `content-only`, which places the content and the pieces with nothing
  around them; `stdmain` contributes `minijinja`.

A template reads the regions by name, the pieces by name with their words and their
place, the document's metadata, its own settings, and the path of each file it ships
(`artifacts`, a map from the file's name to the path the host's naming service gives,
relative to the folder of the main file)
([`TemplateContext`](crate::stdcore::features::compose::TemplateContext) lists them). A
feature contributes to the two slots as a peer
([`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)), so
that its registry still builds where `compose` is switched off. An extension ships
compose templates as files (see [Extensions](#extensions)).

**Shipped files and folders.** A template ships two kinds of
[`TemplateArtifact`](crate::stdcore::features::compose::TemplateArtifact):

- [`File`](crate::stdcore::features::compose::TemplateArtifact::File): one file, with its
  name, its extension, its media type, whether it is itself applied as a template (filled
  in with the same values as the main file, through
  [`TemplateEngine::render_text`](crate::stdcore::features::compose::TemplateEngine::render_text)),
  and its bytes;
- [`Folder`](crate::stdcore::features::compose::TemplateArtifact::Folder): a whole folder,
  with its name and its files, each by its path relative to the folder and its bytes. It
  is never applied as a template.

The composer turns each into one artifact `compose:NAME`. A folder becomes a *folder
artifact*: an [`Artifact`](crate::core::artifact::Artifact) whose data is
[`ArtifactData::Files`](crate::core::artifact::ArtifactData::Files), named by the host's
naming service as a folder
([`ArtifactRef::folder`](crate::core::artifact::ArtifactRef::folder)); the template reads
the folder's path, with no `/` at its end, in `artifacts`. A host that writes the files
writes each file of the folder under that path (see [Outputs and
artifacts](crate::guide::rendering#outputs-and-artifacts) in the rendering guide).
[`Template::from_data`](crate::stdcore::features::compose::Template::from_data) makes a
template from its description written as data. It reads the template's files through
three functions its caller gives, so that this library reads no file itself: one for the
file of the template's text (the description's key `main`), one for a file the
description lists under `artifacts` (`file`), and one for a folder listed there (`dir`).
Each function receives the path as the description writes it, relative to the folder of
the description.

**Files chosen when a template is applied.** The composer takes the list of the files a
template ships from the template's engine:
[`TemplateEngine::artifacts`](crate::stdcore::features::compose::TemplateEngine::artifacts)
is called once each time the template is applied, before the composer names the files,
with the context's values (`artifacts` still empty, since no file is named yet). Its
default answers the template's own list, which is what `content-only` and the `flm`
program's `minijinja` do. An engine whose templates choose their files when they are
applied answers them there, for example a stylesheet computed from the template's
settings: the composer names and ships the files in the order given, and `render`
receives their paths in `artifacts`. An `Err` fails the template as a failure of
`render` does (`flm.compose.template-failed`), and the output is not made.

**Files a template includes.** A template's text may be built from other files when it is
applied: the engine `minijinja` of the `flm` program offers the statement `include`, which
applies another file as a template with the same values, and the function `include_raw`,
which writes another file's text as it is. An engine reads such files through the
template's handle [`Template::files`](crate::stdcore::features::compose::Template::files),
a value implementing
[`TemplateFiles`](crate::stdcore::features::compose::TemplateFiles): `read_text(path)`
returns the text of the file at `path`, relative to the folder of the template's
description, `Ok(None)` when there is no such file, and `Err` with a complete text for a
person (the file leaves the extension's folder, cannot be read, or is not UTF-8 text),
which the engine reports as it is. The handle is given to `Template::from_data` by the
code that reads the template (an extension's reader attaches one over the extension's
files, and the `flm` program reads its own compose templates with the same reader); a
template made without a handle includes nothing. The
files are read when the template is applied, not when it is made, so an include's name
may be computed; a host that records the files a build reads (the `flm` program's live
preview does, to build again when one changes) records them through the handle's reads.
A shipped file applied as a template keeps the handle and may include files too.

### Style and script definitions

A style definition (`StyleDef`) or a script definition (`ScriptDef`) is a stylesheet or a
script that a feature's output depends on, recorded in the registry under an identifier;
a lowering rule or an emitter *requires* it by identifier, and the backend that writes
the output decides its *delivery*: written into the output, loaded from an address, or
left to the host ([`StyleDef`](crate::core::registry::StyleDef); [style and script
definitions](crate::guide::concepts_overview#style-and-script-definitions)).

A rule requires a definition in one of two ways:

- [`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)
  for a definition the rule needs whenever the plan selects it;
- [`Lowerer::require`](crate::core::lowering::Lowerer::require) for a definition it
  needs because of the node it is lowering.

Both record an identifier, and both are collected into the requirements of the lowered
tree. A feature registers the definitions those identifiers designate with
[`style_def`](crate::core::registry::FeatureBuilder::style_def) and
[`script_def`](crate::core::registry::FeatureBuilder::script_def), and they can be read
back from the finished registry. A definition's content is an **ordered list of parts**
([`DefContent`](crate::core::registry::DefContent)), each part the content itself, the
name of a file the host reads, or an address that the output loads and that this library
never fetches. Several parts let one identifier cover one concern that needs several
contents: for example, a script definition whose first part is the script's
configuration and whose second part is the address the script is loaded from. The parts
are written to the output in that order.

The backend that writes the output turns the identifiers into something a reader
receives. The HTML backend adds to each tree's requirements the style definition that
every element it writes needs, resolves all the requirements against the registry, lets
the render request say how each definition reaches the page, and writes the result into
the table `flm.html.definitions`
([`DEFINITIONS_TABLE`](crate::stdcore::backends::html::DEFINITIONS_TABLE)), which its
page assembly reads. A host that assembles its own pages reads that table too. Plain text
has no stylesheet, so the text backend records requirements and reads none. A rule
selected on several backends therefore requires an HTML definition only where
[`Lowerer::backend`](crate::core::lowering::Lowerer::backend) returns the HTML backend.

A script that **transforms elements of the page** in the browser (it replaces an
element's content by something it computes, as a script that typesets mathematical
formulas does)
must follow two rules, because a host may bring one output region of the page up to date
without loading the page again (the `flm` program's live preview does):

- just before transforming an element, the script records the HTML inside it in the
  attribute `data-flm-original-html`; an update that brings the same HTML then leaves the
  transformed element as it is;
- the script listens for the event `flm:region-updated`, which bubbles up to `document`,
  and transforms the elements of the updated region that carry no record.

The rendering guide states both rules ([a page updated in
place](crate::guide::rendering#a-page-updated-in-place)); the configuration script that
the HTML backend ships for its formula typesetting follows them.

The [rendering guide](crate::guide::rendering) describes these mechanisms in detail: how
a rule is called, what it may read, how one is chosen per backend, what the presentation
vocabulary offers, how a definition reaches a page ([style and script
definitions](crate::guide::rendering#style-and-script-definitions)), and what the
stylesheets of the HTML backend hold ([the stylesheets this backend
ships](crate::guide::rendering#the-stylesheets-this-backend-ships)). A worked example of
a feature that ships a stylesheet of its own is in [the AI guide on
extending](crate::guide::ai_guide_extending).

### Definition pieces for the rendered LaTeX

The rendered LaTeX ([the `latex-rendered`
backend](crate::guide::rendering#the-rendered-latex-backend)) writes a feature's content
through the shared presentation elements: a block, a span or a link carrying the
feature's **role** (a dotted name, `callout.box`) and **kind tokens**. Each element is one
`\flmx…` command or environment of a style file, which looks up LaTeX code registered
under the element's role or kind tokens and otherwise uses its default. A feature gives
its roles a look on this backend by shipping a **definition piece**
([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)), LaTeX text
that registers that code with the registration commands of the contract
(`\flmxDefineBlock`, `\flmxDefineSpan`, `\flmxDefineLink`, …; [Registering a
look](crate::guide::rendering#registering-a-look)).

The piece is brought into a document's style file by the feature's **style identifier**,
the identifier of its HTML stylesheet. The feature's general lowering rule requires that
identifier wherever it writes the feature's content
([`Lowerer::require`](crate::core::lowering::Lowerer::require)); the HTML backend resolves
it to the stylesheet, and this backend to every definition piece that declares, with
[`delivering`](crate::stdcore::backends::latex::LatexPreambleDef::delivering), that it
delivers it. A feature with nothing to style on the page registers a style definition
with no part under the identifier, so that the HTML backend knows it and writes nothing.
The piece requires the core of the contract, which defines the registration commands, and
is contributed to the LaTeX family's preamble slot through a peer declaration, so that the
feature builds in a registry without the LaTeX backends:

```rust
use std::sync::Arc;
use flm_core::core::registry::{
    BuildError, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder,
    RegistryInputs, StyleDef,
};
use flm_core::model::names::Name;
use flm_core::stdcore::backends::latex::{LatexPreambleDef, preamble};
use flm_core::stdcore::backends::latex_rendered::contract::CORE_CHUNK;
use flm_core::stdcore::{self, StdcoreConfig};

/// The style identifier the feature's general rule requires at every callout.
const STYLE: &str = "callout.style";

struct Callout;

impl FlmFeature for Callout {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("callout", "Callout boxes")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // The page's stylesheet for the role `callout.box`.
        b.style_def(StyleDef {
            id: Name::from(STYLE),
            content: vec![DefContent::Inline(String::from(".callout-box { margin: 1em 0; }\n"))],
            docs: String::from("the callout box"),
        })?;
        // The rendered LaTeX's look for the same role: the block set in LaTeX's
        // `quote` environment, the target of its anchor first.
        let piece = LatexPreambleDef::new(
            "callout",
            "\\flmxDefineBlock{callout.box}\n  {\\begin{quote}\\flmxIfKey{anchor}{\\flmxTarget{\\flmxKey{anchor}}}{}}\n  {\\end{quote}}\n",
            "the callout box",
        )
        .requires([CORE_CHUNK])
        .delivering([STYLE]);
        b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece))?;
        // The element and its general rule, which stages a block of role
        // `callout.box` and calls `lw.require(&Name::from(STYLE))`, are declared
        // as in the rest of this guide.
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.feature(Arc::new(Callout));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

Register each role and kind token in one piece only: the shipped pieces never register a
name twice, and a name your feature writes belongs to your feature's piece. A **template**
changes the look again by registering the same name after the style file is loaded
(`\flmxDefineBlock{callout.box}{…}{…}` after `\usepackage{…}`); the later registration
wins. A definition piece can be contributed only by a feature compiled into the program:
an extension written in JavaScript or as data has no counterpart yet, and its style
identifiers write nothing on this backend.

## Options and services

A feature takes its values in three forms that differ by when they are fixed: its
configuration, a plain Rust value fixed when the registry is built; its option values, set
on a document or on a render request according to the option type's binding; and its
settings, the same values written as data ([settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)). This
section describes option types, settings, and services, through which features supply
values to each other.

### Option types

An option type is a plain Rust struct a feature declares once; its values are set on a
document or on a render request (the type's binding says which), and a stage reads the
value for an option type it declared a read of: the render value if one is set, otherwise
the document value, otherwise the type's default ([`options`](crate::model::options)). A
feature declares an option type with
[`FeatureBuilder::options`](crate::core::registry::FeatureBuilder::options), or with
[`options_with_docs`](crate::core::registry::FeatureBuilder::options_with_docs), which
also records a description of the type for a generated reference. The option type's Rust
type is its identity: option types have no names of their own. Every option type
implements `Default`, `Hash` and `Debug`, and can be shared between threads
([`Options`](crate::model::options::Options)); the hash reduces a set of option values to
one number, which is part of the identity of a plan and of the record of how a tree was
produced.

The declaration's binding ([`Bind`](crate::model::options::Bind)) says where a value may
be set:

- [`Document`](crate::model::options::Bind::Document): on the document, with
  [`Doc::with_options`](crate::model::doc::Doc::with_options);
- [`Render`](crate::model::options::Bind::Render): on the render request, in
  [`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options());
- [`Either`](crate::model::options::Bind::Either): on the document, on the render
  request, or on both, the render request's value replacing the document's;
- [`Registry`](crate::model::options::Bind::Registry): nowhere, because the value is
  part of the configuration the feature was constructed with, and the feature holds it.

A stage, a rewrite stage or a lowering rule declares the option types it reads (a stage
and a rewrite stage with [`Needs::option`](crate::core::stage::Needs::option), a lowering
rule with [`LoweringDecl::option`](crate::core::lowering::LoweringDecl::option)) and
reads the value from its context
([`StageCx::option`](crate::core::run::StageCx::option), or `option` on the
[`Lowerer`](crate::core::lowering::Lowerer)). The lookup order is the same everywhere
(the render value, else the document value, else the type's `Default`), so no feature
defines a precedence of its own. Reading an option type that was not declared is an
error, and so is setting a value in a place the binding does not admit. A stage may not
declare a read of a [`Bind::Registry`](crate::model::options::Bind::Registry) option type:
the read could only return the default, since the feature holds the value itself, so
building the plan fails.

### Settings given as data

Settings are a feature's configuration and option values written as data, one map per
feature from key to value, which the library reads into the feature's own Rust types; a
settings key is one field of those types ([`settings`](crate::core::settings)). Values
given outside Rust code, such as on a command line or in a document's front matter, take
this form.

A feature that takes settings declares them once, in a
[`SettingsDecl`](crate::core::settings::SettingsDecl), which it returns from
[`FlmFeature::settings`](crate::core::registry::FlmFeature::settings):

- [`config`](crate::core::settings::SettingsDecl::config) declares the type the feature
  is constructed from;
- [`options`](crate::core::settings::SettingsDecl::options) declares each option type,
  with its binding.

Every field of those types is one settings key, read by the type's `serde::Deserialize`
implementation. Each type therefore derives `serde::Deserialize`, with
`#[serde(default, deny_unknown_fields)]` so that an empty map reads as the default and a
misspelled key is refused, and `schemars::JsonSchema`, from which the schema of the
settings is derived, with the fields' documentation as descriptions. A crate outside the
library lists `serde` and `schemars` among its dependencies and derives both; no cargo
feature of the library is involved.

The registry build declares the option types listed in the settings declaration, with
their bindings, before it calls the feature's `register` method, so `register` does not
declare them again. A package factory lists each feature with its settings declaration
(see [Bundling features as a package](#bundling-features-as-a-package)). Each key, each
binding and the feature's name are therefore stated once, by the feature.

Three settings marks (the marks a feature sets on a settings key;
[`SettingsDecl::marks`](crate::core::settings::SettingsDecl::marks)) tell a host how to
read a key:

- A collection of named things is read from a map with
  [`named::deserialize`](crate::core::settings::named::deserialize) and marked with
  [`merge_entries`](crate::core::settings::SettingsDecl::merge_entries). The key must be
  one of the feature's own keys, never a field below one. `merge_entries` also takes a
  function that writes the collection's default entries as data (usually
  [`named::to_data`](crate::core::settings::named::to_data) over the field's default). A
  host that combines several sources of settings, such as `flm`, then combines the
  entries one at a time, with the default entries as the lowest-ranked source, so that a
  source adds, replaces or removes (with `false`) one entry and leaves the others as they
  are.
- Text in FLM markup is marked with
  [`markup`](crate::core::settings::SettingsDecl::markup).
- A path to a file is marked with [`path`](crate::core::settings::SettingsDecl::path).

The schema that [`json_schema`](crate::core::settings::SettingsDecl::json_schema)
returns describes every key with its documentation as plain text, without code blocks and
without anything from the first heading on (`# Examples`). Write the first paragraphs of
a field's documentation for a person who edits a settings file.

```rust
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::settings::SettingsDecl;
use flm_core::model::options::Bind;

/// How loudly the shouting feature shouts, set per render.
#[derive(Debug, Clone, Default, Hash, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct ShoutOptions {
    /// How many exclamation marks follow each shout; 0 by default.
    marks: u8,
}

struct Shout;
impl FlmFeature for Shout {
    fn info(&self) -> FeatureInfo { FeatureInfo::new("shout", "Shouting") }
    fn settings(&self) -> Option<SettingsDecl> {
        Some(SettingsDecl::new("shout").options::<ShoutOptions>(Bind::Render, "How loudly to shout."))
    }
    fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        Ok(())
    }
}

let data = serde_json::json!({ "marks": 3 });
let read = Shout.settings().unwrap().read(data.as_object().unwrap()).unwrap();
assert_eq!(read.render_options().get::<ShoutOptions>().unwrap().marks, 3);
```

### Services and service slots

A service is a value that can be shared across threads (behavior behind a trait object,
or plain data) which one feature supplies for another feature, or for the host, to
use. A service slot is the declaration, by the feature that owns an
interaction, of a dotted name, the type of the values the slot takes and its arity
(at most one value, or any number); any feature, and the host through the registry
builder, may contribute a value to it, and the registry build assembles all
contributions once ([`services`](crate::core::services); [services and service
slots](crate::guide::concepts_overview#services-and-service-slots)).

The owning feature declares the slot with
[`service_slot`](crate::core::registry::FeatureBuilder::service_slot), giving a name, a
value type and an [`Arity`](crate::core::services::Arity). A feature contributes a value
with [`contribute_to`](crate::core::registry::FeatureBuilder::contribute_to) (or
[`contribute`](crate::core::registry::FeatureBuilder::contribute), which finds the slot
by the value's type), and the host with
[`RegistryBuilder::contribute`](crate::core::registry::RegistryBuilder::contribute). A
feature that cannot work in any plan without a service declares it with
[`require_service`](crate::core::registry::FeatureBuilder::require_service): an unfilled
required slot then fails the registry build, instead of a render.

A contribution to a service slot that no feature declares is a registry build error,
which catches a misspelled name. A feature that contributes to a slot that *another*
feature owns, and that may be absent, makes a **peer contribution** with
[`peer_slot`](crate::core::registry::FeatureBuilder::peer_slot), the service-slot form of
a peer declaration. If some feature declares the slot, the value fills it exactly as
`contribute_to` would, with its type checked and after the contributions already made. If
no feature declares it, the value is dropped, the build still succeeds, and one Note
([`UnresolvedPeer`](crate::core::registry::UnresolvedPeer)) joins the build diagnostics. A
feature can therefore offer a value, such as a counter declaration or a cell style, to a
feature that the host may not have registered, without depending on it.
[`peer_slots`](crate::core::registry::FlmRegistry::peer_slots) lists every peer
contribution, resolved or not.

The `graphics` feature's three service slots divide one job among services by what each
of them can do:

- the **graphics resolver**
  ([`GraphicsResolver`](crate::stdcore::features::graphics::GraphicsResolver),
  `flm.graphics.resolver`, one value) finds and reads an image file;
- the **inspector**
  ([`GraphicsInspector`](crate::stdcore::features::graphics::GraphicsInspector),
  `flm.graphics.inspector`, one value) reports an image's kind and size. It receives the
  file's content when a graphics resolver has read it; otherwise it receives the path and
  finds the file itself;
- the **converters**
  ([`GraphicsConverter`](crate::stdcore::features::graphics::GraphicsConverter),
  `flm.graphics.converters`, any number, asked in contribution order) turn content of
  one format into another; they receive content and open no file.

A feature of your own contributes a converter with `peer_slot`, as a host does with
`contribute_to`; an output's conversion rules
([`ConversionRule`](crate::stdcore::features::graphics::ConversionRule), one of the
graphics feature's settings) can then select it by name in their `via` field, where they
want that converter only:

```rust
use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::stdcore::features::graphics::{
    CONVERTERS_SLOT, ConversionRequest, GraphicsConvertError, GraphicsConverter, ImageFormat,
};

/// Turns a WebP image into a PNG (a stand-in: it keeps the bytes).
struct WebpToPng;

impl GraphicsConverter for WebpToPng {
    fn name(&self) -> &str {
        "webp-to-png"
    }

    fn offers(&self, from: ImageFormat, to: ImageFormat) -> bool {
        (from, to) == (ImageFormat::Webp, ImageFormat::Png)
    }

    fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
        Ok(request.content.to_vec())
    }
}

/// A feature contributing the converter, whether or not `graphics` is registered.
struct Webp;

impl FlmFeature for Webp {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("webp", "WebP images")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.peer_slot::<dyn GraphicsConverter>(CONVERTERS_SLOT, Arc::new(WebpToPng))
    }
}
# let _ = Webp;
```

A stage reads services from its context: [`StageCx::service`](crate::core::run::StageCx::service)
returns one service of a given type, and
[`StageCx::services`](crate::core::run::StageCx::services) every service of that type. An
empty slot never fails a render: `service` returns `None` and `services` returns nothing,
and the stage reports the absence as a diagnostic of its own.

The registry build assembles the service slots once, between the two registration rounds,
and they never change afterwards. A value that exists only once some work has been done,
such as an index over a collection of documents, is therefore not a service but data
installed when a run is constructed (an [external
table](crate::core::run::Externals)): a value that is empty at one moment and filled at
another would make a run's result depend on when a stage read it.

**In JavaScript.** A feature of an extension written in JavaScript (see
[Extensions](#extensions)) declares and fills service slots through the same builder
methods, with data as the values:

- `b.service_slot(name, arity, docs)` declares a slot whose values are data; `arity` is
  `Arity.Single` or `Arity.OrderedMany` (from `flm:core/services`), and the name begins
  with the extension's stem and a dot (or with `NAME` and a dot, for an extension whose
  stem is `profile-NAME`), as every name the feature declares;
- `b.contribute_to(slot, value)` adds a value after those contributed so far,
  `b.contribute_first_to(slot, value)` before them, and `b.peer_slot(slot, value)` adds
  it when some feature declares the slot (otherwise the value is dropped with one Note, as
  above); a feature of any package of any extension may contribute;
- a stage reads the slot by its name: `cx.services(name)` answers every value in the
  assembled order, the values contributed first, then the others, each group in the
  order contributed (an empty list when there is none);
  `cx.services_by_precedence(name)` every value in the order of precedence, the values
  contributed first, then the others, and within each group the values of the feature
  placed last in the feature order first, each feature's in the order it contributed
  them; and `cx.service(name)` the first value in the order of precedence (`null` when
  the slot is empty or no feature declares it), so that between features the value of
  the feature placed last wins.

The values are data in both directions: a value with functions in it is refused, and a
slot of a compiled feature whose values are not data, such as the graphics converters,
cannot be read from a script. A data value contributed to such a slot fails the registry
build, with the message of the library naming the slot's type. Two slots of the standard
features take objects with methods instead of data: the citation providers and the
compose templates ([A citation provider from a
script](#a-citation-provider-from-a-script), [A compose template written as a
module](#a-compose-template-written-as-a-module)).

The extension `bibfile` of FLM's repository (`flm-ext/tests/js/bibfile/`) declares a slot
in its default package and fills it from its package `extra`; its stage reports each value
as a Note:

```javascript
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { Arity } from "flm:core/services";

const NOTES = "bibfile.notes";

// In the feature `bibfile` of the package `_`:
register(b, settings) {
  b.service_slot(NOTES, Arity.OrderedMany, "notes other packages add, each `{ note }`");
  b.table(NOTES, "the notes, one item per contribution");
  b.stage({
    decl: () => new StageDecl("bibfile.notes", Effect.Compute, Scope.Document).writes(NOTES),
    run(cx, out) {
      for (const { note } of cx.services(NOTES)) {
        out.table(NOTES).push({ note });
        out.diags({ id: "bibfile.note", severity: "note", message: `a package notes: ${note}` });
      }
    },
  });
},

// In the feature `extra` of the package `extra`, which uses `.#_`:
register(b) {
  b.contribute_to(NOTES, { note: "the extra package was used" });
},
```

## Build hooks

A build hook is a callback that the registry build calls on a feature at a fixed moment.
The two build hooks let a feature declare things it cannot know by itself:

- A [`Finalizer`](crate::core::registry::Finalizer), registered with
  [`FeatureBuilder::finalizer`](crate::core::registry::FeatureBuilder::finalizer), is the
  build's **second registration round**. It runs after every feature's first round, once
  the service slots are assembled, so it can read the services
  ([`FeatureBuilder::services`](crate::core::registry::FeatureBuilder::services)) and
  declare further entities (an element, a stage, a lowering rule) derived from what other
  features contributed. Because the service slots are closed by then, a finalizer may not
  declare a service slot, contribute to one, require one, or register another finalizer;
  those methods return
  [`BuildError::ContributionAfterAssembly`](crate::core::registry::BuildError::ContributionAfterAssembly).
  There are exactly two rounds, so the build is finite and its order is easy to follow.
- A [`LateInit`](crate::core::registry::LateInit) hook, registered with
  [`FeatureBuilder::late_init`](crate::core::registry::FeatureBuilder::late_init) in
  either round, runs once the **language is built**. The usual use is compiling
  templates: a feature configured with author-written definitions cannot parse them while
  the language is still being built, since the definitions may use commands that other
  features declare. A problem the hook finds in the configuration it prepares, such as a
  template that does not parse, does not fail the build: it is written to the registry's
  build diagnostics
  ([`FlmRegistry::build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics)),
  for the host to report once per build (the `userdefs` feature also reports a refused
  definition again at each use). The hook returns an error only when it could not do its
  own work, and that fails the build.

A late-initialization hook is also the place for a fact about the *whole* of a feature's
configuration, one that needs every entry at once (for example, whether another
definition has the same name as this one). The hook works it out once and records it on
the element values it initializes, so that no later stage needs the configuration again.

A feature of an extension written in JavaScript registers both hooks as objects of its
script: `b.late_init({ late_init(lang, diags) { … } })`, whose `diags.push(diagnostic)`
writes to the registry's build diagnostics, and `b.finalizer({ finalize(b) { … } })`; an
exception thrown by either fails the build (see [What a feature in JavaScript adds to the
output](#what-a-feature-in-javascript-adds-to-the-output)).

## Bundling features as a package

A package is a named bundle of features together with the packages it builds on
([`PackageEntry`](crate::core::registry::PackageEntry)): its name, the packages it
`uses`, and its own features in the order they register. A program passes it to
[`RegistryBuilder::use_package`](crate::core::registry::RegistryBuilder::use_package),
which adds the features of the packages it uses first, then its own.

```rust
use std::sync::Arc;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder,
    RegistryInputs,
};
use flm_core::stdcore::{self, StdcoreConfig};

# struct Shout;
# impl FlmFeature for Shout {
#     fn info(&self) -> FeatureInfo { FeatureInfo::new("shout", "Shouting") }
#     fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> { Ok(()) }
# }
// A package of one feature, built on the standard package.
let mine = PackageEntry::new("mine")
    .uses(stdcore::package(StdcoreConfig::new()))
    .feature(Arc::new(Shout));

let mut builder = RegistryBuilder::new();
builder.use_package(mine);
let registry = builder.build(&RegistryInputs::new())?;

assert!(registry.has_feature("shout"));
assert!(registry.has_feature("headings")); // came in with `stdcore`
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

The registry builder records each package by its name, and skips a package whose name it
has already added. A package that two other packages both use therefore registers its
features once.

A package that a document can select by name, or whose settings are given as data, also
has a [`PackageFactory`](crate::core::registry::PackageFactory): its name, the names of
the packages it uses, and the function that builds its entry from the final settings
([`PackageSettings`](crate::core::settings::PackageSettings)), leaving out a feature that
the settings switch off. The factory finds the package's features, each with its settings
declaration, by building the package once from empty settings and asking each feature it
holds, so nothing about a feature is written a second time for the factory.
[`stdcore::factory`](crate::stdcore::factory) returns the standard package's factory.

Registration **order** matters in a few places only, because the checks run once every
feature has registered. The registry numbers capabilities, tables, stages and lowering
rules in the order they were declared, and the same order breaks ties when a plan has
nothing else to decide between two stages; two builds from the same features in the same
order therefore produce the same numbering and the same plans. Contributions fill a
service slot in the order they were made (a read for precedence takes the feature placed
last first), and the fields that several features register for one workflow apply in
registration order. Where several features define one command, the feature placed last
supplies the definition (see [Fallbacks, and replacing another feature's
definition](#fallbacks-and-replacing-another-features-definition)). The registration
order is the order the packages give, changed by the chains of the feature order: a
package's ([`PackageEntry::feature_order`](crate::core::registry::PackageEntry::feature_order);
`feature_order` in an extension's `flm` block, [An extension of the data
engine](#an-extension-of-the-data-engine)) and the host's
([`RegistryBuilder::feature_order`](crate::core::registry::RegistryBuilder::feature_order);
the `flm` program's are the front matter's). The standard package
fixes the order of its features for these reasons.

## Features and packages chosen at run time

A document may list, in its `use` list, packages that are not compiled into the host: the
host finds them by name when it processes the document. This library declares the
interface and reads no file. A
[`PackageCatalog`](crate::core::registry::PackageCatalog) finds a package's
[`PackageFactory`](crate::core::registry::PackageFactory) by name: first among the
factories compiled into the host, then through the
[`PackageResolver`](crate::core::registry::PackageResolver) implementations the host
added, which may read files. [`resolve_all`](crate::core::registry::PackageCatalog::resolve_all)
walks the packages that a list of names uses and returns them in the order in which they
register: each package after the packages it uses, and each once; a package already found,
or one whose walk is under way, is skipped, so two packages that use each other cause no
problem. A resolver's factory is identified by its name, so a resolver that finds one file
under two spellings gives one factory for both, and the package is used once.

The crate `flm-ext` provides a resolver: its loader of extensions finds a package of an
extension, written as data, in JavaScript, compiled to WebAssembly or compiled into the
program, through the sources
the host gives it (see
[Extensions](#extensions)). The library itself ships no resolver.

A package of an extension may write fields for a workflow that a compiled package defines
under the same name. The host combines the fields that the packages write for the
workflow, by the rule between packages of the settings cascade
([`workflow_def`](crate::core::settings::cascade::workflow_def)), and registers the result
with
[`RegistryBuilder::workflow_fields`](crate::core::registry::RegistryBuilder::workflow_fields):
those fields replace the fields of the workflow of that name, or define the workflow when
no package does, so that the two make one workflow. A compiled feature registers fields
for a workflow in the same way, with
[`FeatureBuilder::workflow_fields`](crate::core::registry::FeatureBuilder::workflow_fields).

## Documentation texts

Every declaration a feature makes carries a documentation text: the feature itself
([`FeatureInfo::docs`](crate::core::registry::FeatureInfo::docs)), a command
([`ElementDef::docs`](crate::lang::mint::ElementDef::docs)), an argument
([`ArgDef::docs`](crate::lang::mint::ArgDef::docs)), a field written after a command, a
scoped package, a stage, a table, a lowering rule, an argument role, an option type, a
heading kind, a counter, and the other entities the builder records. The registry keeps
these texts with the declarations, and the command `flm info` shows them: a feature's page
shows its text, a command's page the texts of the command and of each argument. This
section states what such a text holds and how it is shown.

### What a documentation text holds

A documentation text is written for the person who reads the pages of `flm info`, and it
follows these rules:

- **It is FLM.** The text is read as an FLM document that uses the extension `flmdoc`
  (below): plain text and the commands of the standard features (`\emph{…}`), with code
  written between backticks, as in Markdown: `` `\section{title}` ``. The backtick is the
  only Markdown-like sequence; `*…*` and `**…**` are shown as written. Outside a code span
  a backslash starts a command, braces must pair up, and a lone `%` is an error, so write
  a command's name, a LaTeX fragment or a file name between backticks.
- **Its first paragraph is its summary.** The text up to its first blank line is shown
  wherever the item is listed (the list of a feature's commands, for example); the whole
  text is shown on the item's own page. A text is usually one sentence; write more
  paragraphs only when the thing needs them.
- **It says what the thing does.** A command's text begins with what the command does, not
  with how it is written: the page shows the command's signature and each argument
  separately.

The descriptions of a feature's settings come from another place: the Rust documentation
of the settings type, which the schema-derivation library turns into the settings' JSON
Schema ([`SettingsDecl::json_schema`](crate::core::settings::SettingsDecl::json_schema)).
Each description is first turned into plain text (a code block and everything from the
first heading on are dropped, a link keeps its words alone), and then read by the same
rules, as FLM with code between backticks. Write a settings field's documentation
accordingly: `` `true` ``, not `true` in quotes, and `` `\label` `` between backticks.

The program's own texts are tested to be valid FLM. A text that an extension wrote may not
be; `flm info` then shows it as written, as plain text, with a warning, and does not fail
(see [Documentation texts that are not valid FLM](crate::guide::command_line#documentation-texts-that-are-not-valid-flm)).

### Writing documentation pages: the extension `flmdoc`

The extension `flmdoc` provides the markup in which the pages of `flm info` are written,
and any document may use it. It is written in Rust and compiled into the `flm` program, so
nothing needs to be installed; a document lists it in its front matter's `use` list, or
the command line adds it with `--use flmdoc`:

```text
---
flm:
  use: [flmdoc]
---
The command `\section` starts a section; see \docref{features/headings}{the feature}.
```

It offers three things:

- **Code spans.** A code span begins with a run of backticks and ends at the next place
  where as many backticks follow one another (unlike in CommonMark, a longer run also ends
  it, at its first backticks, and the backticks left over follow the span); its content is
  shown as inline code, exactly as written (a backslash, a brace or a percent sign is an
  ordinary character there). To show a backtick, open the span with a longer
  run: ``` ``a`b`` ``` shows ``a`b``. As in CommonMark, each line ending in a span becomes a
  space, and when the content both begins and ends with a space (and is not all spaces),
  one space is removed from each end. Code spans are written in text; inside mathematics a
  backtick is an ordinary character. In a document that uses
  `flmdoc`, every backtick in text opens a code span, so the quotation marks that FLM
  otherwise writes with backticks (``` ``…'' ``` and `` `…' ``) are not available: write “…”
  and ‘…’ directly. A backtick that no later run of as many backticks closes is an error.
- **The documentation of an item**, laid out by a few constructs:

  ```text
  \begin{docitem}[macro-section]{macro}{\section}
  \docsignature{\section*[short]{title}}
  \docsummary{Starts a section of the document.}
  \begin{docparams}
  \docparam{*}{star}{}{The section is not numbered.}
  \docparam{{title}}{title}{flm.headings.title}{The title.}
  \end{docparams}
  \docfield{Declared by}{the feature \docref{features/headings}{headings}}
  \end{docitem}
  ```

  `\begin{docitem}[anchor]{kind}{name} … \end{docitem}` documents one item of a kind
  (`macro`, `feature`, any word), with an optional anchor that a link points at;
  `\docsignature{…}` shows how the item is written, as a block of code;
  `\docsummary{…}` states its summary; `\begin{docparams}[title] … \end{docparams}`
  lists its parameters under a title (`Arguments` by default), each given by
  `\docparam{form}{name}{role}{text}`, available only there; `\docfield{key}{value}`
  states one labeled fact, `key: value`. The kind and name of `docitem`, the signature,
  and the form, name and role of `\docparam` are raw text; one that holds a brace alone
  is written between another pair of delimiters (`\docsignature|\{|`).
- **Links to other pages.** `\docref{path}{text}` links to the documentation page of a
  path (raw text). The address depends on the render setting `flmdoc.link_style`: with
  `command`, the default, it is the command that shows the page,
  `flm info features/headings`, for reading the pages at a terminal; with `path`, it is
  the page's path followed by `.html`, `features/headings.html`, counted from the root of
  the page tree (not relative to the page holding the link), for a folder of HTML pages.
  The `text` profile prints a link as its text followed by its address. Any other value
  is an error, reported at each `\docref`.

Each construct becomes ordinary presentation nodes (paragraphs, blocks, spans, a list,
inline and block code, a link) that carry a role named after the construct
(`flmdoc.item`, `flmdoc.signature`, `flmdoc.param.role`, …), which the HTML output writes
as a class (`flmdoc-item`); the extension ships no stylesheet. The constructs are written
for the profiles `text` and `html`; the profile `latex-rendered` writes the nodes they
become as it writes every such node, and the profile `latex-recomposed` cannot render a
document that uses them.

### Dropping the documentation texts: the cargo feature `docs`

The cargo feature `docs` of `flm-core`, on by default, keeps the documentation texts. A
program that never shows them (a build for WebAssembly that renders documents, for
example) can turn it off to carry none of them: every builder method, builder function
and constructor that takes a documentation text then stores an empty one, the texts read
from settings data (a user definition, a heading kind, a workflow) included. Turning the
feature off changes neither a rendered document nor the [registry
digest](crate::model::digest), since no documentation text is an input of either. What
does change, beside the texts: the top-level description of each feature's settings JSON
Schema, which is made of those texts (the descriptions of the individual settings keys stay,
being part of the schema), and the message of
[`RunError::MissingExternal`](crate::core::run::RunError::MissingExternal), which quotes a
table's text. A text that a program assigns directly to a public `docs` field is kept.

A library that shows the texts, such as `flm-info`, depends on `flm-core` without its
default features, so that the program that embeds it decides: that program keeps `docs`
on through its own dependency on `flm-core`.

## Extensions

### What an extension is

An *extension* is a set of files that a host finds by name while it processes a
document, without having been compiled with it, and that provides one or more packages.
A document lists a package of an extension under `use`, as it lists a compiled package.
The words this chapter uses:

- the *extension* is the unit the host finds and reads: a manifest, and for an extension
  written as a folder, the files beside it;
- a *package* is the unit a `use` entry names; one extension provides one or more;
- the *engine* of an extension turns its files into packages. The engine `data` reads an
  extension written as data (settings, workflows, compose templates: no behavior); the
  engine `js` runs an extension written in JavaScript, whose features declare elements,
  tables, stages, rewrite stages, lowering rules, presentation elements, backends,
  outputs and settings through the same feature builder as a feature written in Rust;
  the engine `wasm` runs an extension compiled to one WebAssembly module, typically
  from Rust, whose features declare the same things through the same builder; the engine
  `native` runs an extension written in Rust and compiled into the program that loads it
  (see [A Rust extension compiled into a program](#a-rust-extension-compiled-into-a-program));
- the *manifest* is the extension's YAML file: `NAME.yaml` for an extension of one file,
  `NAME/flmextension.yaml` for an extension written as a folder.

The crate `flm-ext` holds the extension layer: the manifest's reader, the grammar of a
`use` entry, the engines, and the loader `flm_ext::ExtensionLoader`, a
[`PackageResolver`](crate::core::registry::PackageResolver) that a host adds to its
[`PackageCatalog`](crate::core::registry::PackageCatalog). The host gives the loader its
*sources*, which find an extension by name and read its files; the host library of the
`flm` program gives it the folders of its search path and, last, the extensions shipped
with FLM, compiled into the program. `flm-ext` reaches the library
only through this crate's public interface, as any other extension crate does.

### The manifest

The manifest's root holds two keys, each optional. `extension` says what the extension
is: `description`, `author`, `flm_version` (the versions of FLM it works with, as a
version requirement such as `>=0.5`), `engine` (`data` when the key is absent, `js`,
`wasm` or `native`), `js` (for the engine `js`: `{main: PATH}`, the main module's path inside the
extension's folder) and `wasm` (for the engine `wasm`: `{module: PATH}`, the module's path
inside the extension's folder). `flm` holds the package of an extension of the engine `data`, and is
refused for another engine. The command-line guide lists every key, the two layouts, the
grammar of a `use` entry (`LOCATOR` or `LOCATOR#PACKAGE`, `_` being the default package)
and the folders searched ([Extensions](crate::guide::command_line#extensions) and [Where
extensions are looked for](crate::guide::command_line#where-extensions-are-looked-for)).

### An extension of the data engine

An extension of the engine `data` provides one package, named `_`, so a document lists it
by its locator alone (`use: [mystyle]`). The package's contents are the manifest's `flm`
block, read by the same rule as the `flm` block of a document's front matter: `use` (the
packages it uses), `features` (settings of features, or `false` to switch one off),
`parsing`, `workflows` and `feature_order` (chains of the order of features, below). Any
other key is refused, `layout` and `default_profile` included, which a document's front
matter alone writes. When the extension is a folder that holds compose templates
under `templates/OUTPUT/` (each either the folder `NAME/` holding the description
`flmtemplate.yaml` and the template's own files, or the file `NAME.yaml`, a description
alone; the template's name is `NAME` in both forms, in the second the file's name without
the suffix `.yaml`, so that a file `flmtemplate.yaml` directly in `templates/OUTPUT/` is
the template `flmtemplate`; the description's key `main` gives the file of the template's text, and its
key `artifacts` the files and folders the compose template ships, each by its path
relative to the description's folder, read from the extension's files; the template's
includes are read from the extension's files too, when it is applied; see [the guide to
the command-line tool](crate::guide::command_line#extensions)),
the package also has one feature, which contributes them to the
[`compose`](crate::stdcore::features::compose) feature's service slot of templates, under
the extension's `description` as its title. An extension written as data never adds
behavior.

**The order of features.** A package states the order it needs between features with
`feature_order`: one chain, or a list of chains, each two or more features' names (not
packages' names) joined by `<`:

```yaml
# mypreamble/flmextension.yaml
flm:
  use: [phfpack, math-mathjax]
  feature_order: [math-mathjax < phfqit]
```

The package's chains reach the registry build together with the chains of every other
package in use and of the document's front matter; a feature that is not in use is
left out of a chain, and the rest of the chain still applies. The order decides which
definition applies where two features define one command, and which feature's
contribution wins where two compete
([Features and packages](crate::guide::concepts_overview#features-and-packages)).
Declare a chain only when the extension needs that order: the chain then states that
the extension does not work with the reverse order, and a document or another package
that places the features the other way round stops the build with an error naming both
chains. An extension that works in either order says nothing. A chain orders two whole
features: every collision between them is settled one way. An extension that needs a
finer order splits a feature into several (`phfqit-delimiters`, `phfqit-stdmacros`), each
ordered on its own.

The settings an extension's package writes are a source of settings of their own, below
the document's front matter: a package wins over the packages it uses, and two packages
of which neither uses the other must agree (see [the ranks of the
settings](crate::guide::command_line#ranks-of-the-settings)). A host combines the
sources with the library's settings cascade
([`core::settings::cascade`](crate::core::settings::cascade)).

### An extension in JavaScript

An extension in JavaScript is a folder whose manifest names the engine `js` and its main
module:

```yaml
# wordcount/flmextension.yaml
extension:
  description: Counts words
  engine: js
  js:
    main: main.js
```

**The default export.** The main module's default export is a function. The engine calls
it once with the extension builder `ext`, and each call `ext.package(name, object)`
declares one package of the extension: `_` is the default package, which a `use` entry
without `#` names, and any other name (letters, digits, `-` and `_`) is reached as
`wordcount#name`.

**The package object** has these keys, each optional:

| key | what it holds |
|---|---|
| `uses` | the packages the package uses, as `use` entries: `stdcore`, another extension, or `.#core` for the package `core` of this extension |
| `flm` | a block of settings, as the `flm` block of an extension of the engine `data` writes it (`features`, `parsing`, `workflows`, `feature_order`: `{ feature_order: ["math-mathjax < phfqit"] }`) |
| `templates` | the compose templates the package ships: a list of the paths of their descriptions inside the extension, each `templates/OUTPUT/NAME/flmtemplate.yaml` (a compose template written as a folder) or `templates/OUTPUT/NAME.yaml` (a description alone), the compose template's name being `NAME`; a path of neither shape is refused |
| `host` | the host services the package's handlers call: a list of names ([Host services](#host-services)) |
| `features` | the package's features: a map from a feature's name to its feature object, in the order written |

**The feature object** has these keys:

| key | what it holds |
|---|---|
| `title`, `docs`, `version` | texts, optional: the feature's information, as a compiled feature's [`FeatureInfo`](crate::core::registry::FeatureInfo) |
| `settings` | optional: the feature's settings, declared as data (below) |
| `register(b, settings)` | a function: it declares what the feature contributes through the feature builder `b`, a JavaScript object with the methods of [`FeatureBuilder`](crate::core::registry::FeatureBuilder); `settings` is the feature's configuration as written in the document's settings (`{}` when nothing is written) |

**Settings.** A feature in JavaScript declares its settings as data, since it has no Rust
type to derive them from. `settings.config` is the feature's configuration, fixed when
the registry is built: `{ docs, schema }`, where `schema` is a JSON Schema of an object
whose properties are the configuration's settings keys. `settings.options` declares
option types by name: each `{ bind, docs, schema }`, where `bind` is `document`, `render`
or `either` (see [options](crate::guide::concepts_overview#settings-options-and-configuration)).
The library registers them as it registers a compiled feature's settings
([`SettingsDecl::config_data`](crate::core::settings::SettingsDecl::config_data),
[`SettingsDecl::options_data`](crate::core::settings::SettingsDecl::options_data)), so
a document writes the keys of both under `flm.features.NAME`, `flm schema` describes
them, and the preview's settings tool edits them. `register` receives the
configuration; a stage reads an option type by its name, after declaring the read in
its needs: `new Needs().option(name)` in `needs()`, then `cx.option(name)` in `run`,
which answers the option's settings as a map.

**Imports.** A module imports:

- the modules of the extension interface, under the prefix `flm:` (`flm:core/stage`,
  `flm:lang/rewrites/userdefs`, …), which mirror this crate's items, with the same names;
- the files of the extension, by a path relative to the importing module, with the file
  name's extension written (`./helpers.js`, not `./helpers`); a path may not climb above
  the extension's folder;
- the modules the host program offers, by a bare name.

A dynamic `import()` is refused: every module is imported by an `import` declaration at
the top of a module, and the extension's files are read once, when it is loaded.

**Names.** Every element, table, stage, capability, option type, scoped package, lowering
rule, style and script definition and diagnostic identifier that a feature of an extension declares, and the name of
every value it contributes as data (a counter, a definition piece), must begin with the extension's
*stem* and a dot: the stem is the last segment of the locator through which the extension
was first reached (`wordcount` for `wordcount`, `style` for `@acme/style`). An extension
whose stem is `profile-NAME`, which the profile `NAME` uses ([Capabilities, collections,
ordering and workflows](#capabilities-collections-ordering-and-workflows)), may also begin
these names with `NAME` and a dot: wherever this chapter speaks of the stem, `NAME` may
take its place. A name that
another feature owns, such as a table a stage reads, is not checked.

A feature's own name carries no stem, so that a front matter configures it under a short
key (`features: { wordcount: … }`). Two extensions that declare features of one name
cannot be used together: the build fails with an error. Give a feature of
your own a name that no other extension is likely to use, for example one that begins
with your organization's or your extension's name.

**What a script can reach.** A script runs in an engine instance that reaches nothing
outside itself: no files, network, timers or processes, and a clock and a random-number
generator that give the same answers on every run. What the program offers as *host
services*, and the package declares, is the one way out ([Host
services](#host-services)). A handler call has a time budget and
an engine instance a memory limit. When an engine instance other than the one that
loaded the extension must run a stage or a hook, it first runs the feature's `register`
again with the same configuration, so a feature's `register` must declare the same
things under the same names every time it runs. The documentation of the module
`flm_ext::quickjs` states the sandbox, the limits and this rule in full, together with how
each item of the extension interface appears in JavaScript.

**Editor support.** The file `flm-ext/flm.d.ts` in FLM's repository declares every module
under `flm:` in TypeScript: its classes with their methods, the objects an extension writes
(a stage, an expansion hook, a rule's body) as interfaces, its enums and constants with
their values. A declaration that mirrors a documented Rust item carries the first sentence
of its documentation. An editor that reads TypeScript declarations completes the names a
script imports from `flm:` modules, shows each item's sentence, and reports a misspelled
method or a wrong number of arguments, once a file `jsconfig.json` beside the extension's
manifest points at the declarations (replace `PATH` with the path of `flm.d.ts`;
`"strict": false` keeps the checker from reporting every function parameter that has no
declared type):

```json
{ "compilerOptions": { "checkJs": true, "strict": false, "module": "es2022",
                       "moduleResolution": "bundler" },
  "files": ["PATH/flm.d.ts"], "include": ["**/*.js"] }
```

The declarations describe the extension interface of the `flm-ext` version written in the
file's first line. FLM itself reads neither file: the checks are the editor's. The
documentation of the module `flm_ext::quickjs` describes the declarations under "Editor
support".

### What a feature in JavaScript adds to the output

A feature in JavaScript registers lowering rules, style and script definitions and the
tables they read the way a compiled feature does, through the builder `b` and the modules
under `flm:`. The `math-mathjax` extension, which renders mathematics with MathJax, uses
each of the items below (its files are in the folder `extensions/js/math-mathjax/` of the
FLM repository; [mathematics processed by
MathJax](crate::guide::command_line#mathematics-processed-by-mathjax) describes it for an
author).

- **Lowering rules.** `new LoweringRule(name, element, decl, body, mark)` (from
  `flm:core/lowering`), registered with `b.lowering_rule(rule)`, takes its arguments in the
  order of [`LoweringRule`](crate::core::lowering::LoweringRule)'s, with the rule's
  body an object `{ lower(node, cx, lw) { … } }` that answers a `Lowering`. The body runs
  on the thread that lowers, under the extension's limits; an exception in it is the Error
  `flm.ext.handler-failed` at the node, and the node is replaced by an
  `flm.pres.unsupported` stand-in. The extension's rule `math-mathjax.formula.lower.html`
  lowers `flm.math` on the `html` backend.
- **The lowerer.** The body's `lw` offers the methods of
  [`Lowerer`](crate::core::lowering::Lowerer), each taking `cx` first where it stages:
  `lw.lower_arg_named(cx, node, name)` lowers an argument, `lw.lower_nodes(cx, nodes)` a run
  of sibling nodes (the extension lowers the argument of `\text{…}` with it),
  `lw.get(name)` reads a table, and `lw.get_at(name, node)` reads only the items of the
  extension's own table that belong to one node.
- **Presentation as data.** `lw.pres_tree(cx, tree)` stages a whole tree of presentation
  nodes in one call. Each node is `{ element, payload, children }`, with the payload
  written as the element's payload type serializes; `{ text }` is a run of text, and
  `{ build }` places a node staged before. The extension builds the MathML of a formula
  this way, as HTML construct nodes (`flm.pres.html.tag`) with the lowered text of each
  `\text{…}` placed inside its `mtext` element.
- **Style and script definitions.** `b.style_def(new StyleDef(id, parts, docs))` and
  `b.script_def(new ScriptDef(id, parts, docs))` (from `flm:core/registry`) take the
  parts `DefContent.Inline(text)`, `DefContent.Url(address)` and `DefContent.Table(name)`,
  the last one a [part supplied by a
  table](crate::guide::rendering#style-and-script-definitions). Such a table is declared
  with `b.definition_text_table(name, docs)` and holds one text, which a stage writes with
  `out.table(name).push(text)`. The extension's `load-in-browser` mode lists the TeX packages the
  document uses in its MathJax configuration that way.
- **Peer declarations.** `b.peer_backend(name, docs)`, `b.peer_table(name, docs)` and
  `b.peer_element(name, docs)` declare a backend, a table or a presentation element that
  another feature owns ([peer declarations](#peer-declarations)). The extension declares
  the four standard backends, the anchors and counter tables, and the LaTeX family's
  construct elements.
- **The registry digest.** `b.digest_input(text)` gives the UTF-8 bytes of the string
  `text` to the [registry digest](crate::model::digest), as
  [`FeatureBuilder::digest_input`](crate::core::registry::FeatureBuilder::digest_input)
  does for a compiled feature. The extension gives its configuration, written as JSON,
  since the `preamble` and its other settings change the output in ways its declarations
  do not show.
- **A table that refines a capability.** `b.table_refining(name, parent, docs)` declares a
  table whose capability refines `parent`, so that writing it satisfies a demand for the
  parent ([refinement](crate::guide::pipeline#refinement)). The extension's table
  `math-mathjax.rendered`, the MathML of each formula, refines `flm.math.rendered`.
- **The library of mathematics.** The module `flm:stdcore/features/math` offers the `math`
  feature's names (`MATH`, `ENVIRONMENT`, `CONTENT`) and the plain functions of its library
  ([`math`](crate::stdcore::features::math)): `equation_lines`, `line_frames` (each line's
  anchor and number), `stage_tag_content`, the LaTeX parts `group_opening`,
  `environment_opening`, `line_closing` and the others, the line writer `stage_lines`,
  which writes an environment's lines in the recomposed LaTeX, and the newline rule
  `normalize_line_newlines`, which a writer of an environment's lines as text applies to
  each line so that no blank line is left where a `\label` stood on a line of its own;
  `flm:stdcore/features/userdefs` offers `write_math_run`, which writes the content of a
  formula as LaTeX.
- **Engine limits.** The manifest's `extension.js.limits` sets the limits every engine
  instance of the extension is created with, each in place of the host's: `memory_mib`,
  `stack_mib` (below the stack of the threads that run handlers) and `time_s`, per
  handler call. MathJax needs a deeper stack and more memory than the defaults, and one
  call converts every formula of a document, so the extension's manifest writes
  `limits: { stack_mib: 16, memory_mib: 64, time_s: 120 }`: a 120 s time budget per call.

A feature in JavaScript can also add an output format of its own and rewrite the trees
before they are frozen. Two test extensions of the FLM repository, in the folder
`flm-ext/tests/js/`, show each item below: `typst` writes a document as Typst markup,
and `smartdash` replaces `--` and `---` by dashes.

- **A backend and an output.** `b.backend(new BackendDef(name, native, lowering, emit,
  final_stage))` and `b.output(new OutputDef(name, capability, table, media_type,
  extension))` (from `flm:core/registry`), with the library's lowering stage
  `new LoweringStage(backend)` (from `flm:stdcore/backends/lowering`) and an emit stage
  and a final stage written as stage objects ([Adding an output
  format](#adding-an-output-format)). An output made from the files of another output
  lists that output with `.made_from(name)` on its `OutputDef`. The name of a backend or
  an output is the extension's stem itself or begins with the stem and a dot. `typst` registers the
  backend and the output `typst`, which the `flm` program renders with the workflow `typst` (`-p typst`).
- **The standard tables by name.** `b.table_named(name, type, docs)` declares a table of
  one of the library's types, `StandardTable.LoweredTrees`, `LoweredPieces`, `Regions`
  or `ArtifactSet`; the name `flm.lowered.<backend>` that the library's lowering stage
  writes is accepted for a backend of the extension. A stage reads a table of lowered
  trees as an object (`cx.get(name).lowered`: `trees()`, `tree(id)`, `requires(id)`,
  `rounds(id)`), pushes artifacts into an artifact set as data (`Artifact` and
  `ArtifactId`, from `flm:core/artifact`, each file's content a text, `{ text }` or `{
  base64 }`; `media_type_of(extension)` answers the media type the library knows for a
  file extension), and reads an artifact set as data, each file with the path the host
  writes it at (`cx.get(name).projection`; see [Adding an output
  format](#adding-an-output-format)).
- **The nodes of a lowered tree.** Besides the methods of any node, `node.payload()`
  answers a presentation node's payload as data, and `node.slot_count()`,
  `node.slot_name(index)` and `node.slot_content_nodes(index)` read its slots by
  position (a list's labels and items). `flm:lang/structural` names the structural
  elements (`TEXT`, `GROUP`, `PARBREAK`, …).
- **Presentation elements of your own.** `b.presentation_element(name, docs)` declares
  one whose payload is any data value ([Presentation elements of your
  own](#presentation-elements-of-your-own)). `typst` declares `typst.pres.callout`, which
  its emit stage writes from the payload's field `kind`.
- **Rewrite stages.** `b.rewrite_stage({ decl, needs, restage(node, cx, rs) })`, with
  `RewriteDecl` (from `flm:core/stage`) and `Restage` (from `flm:techy/transform`): the
  library calls `restage` once per node and the handler keeps the node or replaces it
  ([Tables and stages](#tables-and-stages)). `smartdash` registers the rewrite stage
  `smartdash.dash`, which publishes `smartdash.dashed`, and the workflow `smartdash`
  demanding it.

A feature in JavaScript cannot yet give its output regions, so a compose template cannot
assemble that output, nor have the library's pieces stage run for its backend; and it
cannot declare a native-set implication, nor an ordering edge between two stages of other
features (its own stages and rewrite stages order themselves through their needs, `after`
and `before`). It joins a collection, such as the `flm.validated` the standard workflows
demand, with `b.collection_member` (below).

A feature in JavaScript also declares numbered and referenceable constructs, the way the
standard features declare theorems and headings. The two test extensions `exercises` (a
numbered environment `\begin{exercise}…\end{exercise}` with references and a command
`\hint` legal inside it alone) and `myhead` (a numbered heading listed in the table of
contents, whose number is written through a template) use the items below; their files
are in the folders `flm-ext/tests/js/exercises/` and `flm-ext/tests/js/myhead/` of the
FLM repository, and [Number a construct of your
own](#number-a-construct-of-your-own) gives the steps of `exercises` in Rust and in
JavaScript.

- **Aspects answered by handlers.** The aspects
  [`Referenceable`](crate::model::aspects::Referenceable) (the reference points of an
  occurrence) and [`Sectioning`](crate::model::aspects::Sectioning) (its level, whether
  it is listed in the table of contents, and its title there) are objects of the script:
  `Aspect.Referenceable({ points(node) { … } })` and `Aspect.Sectioning({ level(node) {
  … }, in_toc(node) { … }, toc_title(node) { … } })` (from `flm:model/element`), given in
  the element's specification with `new ElementSpec(name).with(aspect)`, or attached to
  an element of another feature with `b.attach(element, aspect)`. `points` answers a list
  of `RefPoint`s, built with the classes of `flm:model/aspects` (`RefPoint`,
  `NumberSource`, `AnchorPolicy`, `ContentRef`, `AcceptedRefTypes`). The stage that asks
  for the aspect calls the handler, on the thread that runs it, with the occurrence's
  node alone. A handler that fails is reported nowhere: the occurrence then has no
  reference point, or the level 0, is not listed, and has no title. The package `intoc`
  of `exercises` attaches a `Sectioning` aspect to the exercises of the package `_`, so
  that the titled exercises are listed in the table of contents.
- **Labels and references.** The module `flm:stdcore/features/refs` offers the
  [`refs`](crate::stdcore::features::refs) feature's names (`ANCHORS`, `SAFE_LABELS`,
  `LABEL_PACKAGE`, …) and its helpers: `label_fields_arg()`, the argument that holds the
  `\label` fields written after a construct; `label_uses(node)` and
  `point_labels(uses)`, which read those fields into a point's labels; `stage_label` and
  `stage_label_group`, which write the labels into the recomposed LaTeX. A rule reads the
  anchors table with `lw.get(ANCHORS)`, which answers `{ anchors }`, whose `id(point)`
  answers the anchor of a reference point (a point is written `{ addr, index }`, with
  `lw.addr()` the address of the node being lowered).
- **Numbering.** The module `flm:stdcore/features/numbering` offers the
  [`numbering`](crate::stdcore::features::numbering) feature's names (`COUNTERS`,
  `COUNTERS_SLOT`, `STYLES_SLOT`, …) and the classes of its declarations (`CounterDecl`,
  `CounterStyleDecl`, `Within`, `PrefixWords`, `FormatOptions`, …). A rule reads the
  counter table with `lw.get(COUNTERS)`, which answers `{ counters }`, whose
  `format(point, options)` answers the segments of the point's number for
  `lw.splice(cx, segment.content)`, and whose `record(point)` answers the point's record,
  or `null` for a point the document did not number.
- **Values contributed as data.** `b.contribute_to(slot, value)`,
  `b.contribute_first_to(slot, value)` and `b.peer_slot(slot, value)` contribute to
  another feature's service slot (see [Services and service
  slots](#services-and-service-slots)) a value written as data. Five standard slots read
  that data as their own value types: a counter and a counter style
  (`flm.numbering.counters`, `flm.numbering.styles`), a parent set
  (`flm.numbering.parent-sets`), a definition piece and a package definition of the
  recomposed LaTeX (`flm.latex.preamble`, `flm.latex.packages`). The value's own name
  (its `name`, or the `id` of a package definition) must begin with the extension's stem
  and a dot. A value written as data for any other standard slot whose values are not
  data, such as the slot of number writers, whose values carry behavior, fails the
  registry build, which names the slot and the two types.
- **The LaTeX constructs.** The module `flm:stdcore/backends/latex` offers the LaTeX
  family's construct elements (`MACRO`, `ENVIRONMENT`, `RAW`, …), their payload classes
  (`LatexMacroPayload`, `LatexEnvironmentPayload`, `LatexRawPayload`, …), the slot names
  `ARG_SLOT`, `BEGIN_SLOT`, `END_SLOT` and the backend's name `RECOMPOSED_BACKEND`; its
  namespaces `latex.preamble` and `latex.packages` hold the definition pieces and
  packages of the style file. A rule writes a LaTeX construct with
  `lw.pres_slots(cx, element, payload, [[slot, staged], …])`. A feature contributes its
  own piece (`new latex.LatexPreambleDef(name, content, docs)` to `latex.preamble.SLOT`),
  and the rule that writes a construct the piece defines requires it, while it lowers
  with `lw.require(id)` ([`Lowerer::require`](crate::core::lowering::Lowerer::require)),
  or when the rule is selected, with
  [`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)
  (the identifier is `latex.preamble.piece_id(name)`); the recomposed LaTeX backend then
  writes the piece into the style file. The `exercises` extension contributes the piece
  `\newtheorem{exercise}{Exercise}` this way.
- **Scoped packages.** `b.scoped_package(new ScopedPackageDef(name, elements, docs))`
  (from `flm:lang/mint`) declares commands that are available only inside the bodies or
  arguments that name the package (`new BodyDef().scoped(name)`,
  `scoped(name)` on an `ArgDef`); see [Scoped packages](#scoped-packages). The command `\hint` of
  `exercises` is declared this way.
- **Collections.** `b.collection_member(collection, capability)` adds a capability the
  feature declares to a collection another feature declares, such as the collection
  `flm.validated` that the standard workflows demand (see [Capabilities, collections,
  ordering and workflows](#capabilities-collections-ordering-and-workflows)).
- **Build hooks.** `b.late_init({ late_init(lang, diags) { … } })` runs once the
  language is built, with the built language and the registry's build diagnostics
  (`diags.push(diagnostic)`); `b.finalizer({ finalize(b) { … } })` runs in the build's
  second round (see [Build hooks](#build-hooks)). The `myhead` extension checks its
  configured title template in a late-initialization hook.
- **Templates and durable objects.** A feature compiles a template against the built
  language and fills it with values it computes: `compile_template_with(spec, lang, new
  TemplateLanguage(lang))` (from `flm:lang/rewrites/userdefs`) compiles a
  `TemplateSpec`, with the language that `lw.lang()`, `xp.lang()` or a
  late-initialization hook gives; `instantiate(node, template, new
  ValueBindings().with(name, root))` instantiates it, giving each extra placeholder the
  tree whose root is `root` (see [Templates with values the feature
  computes](#templates-with-values-the-feature-computes)). The template language and
  the compiled template are *durable objects*: FLM keeps them for the feature as long as
  the registry is in use, and a script may use them on any thread and in any later call.
  Compiling a second time with an equal specification answers the same object without
  compiling the text again, so a rule may compile its template at every use. The
  `myhead` extension writes its heading's number this way.

### Host services

An engine instance reaches nothing outside itself. What an extension needs from outside
(a file beside the document, a program installed on the machine) it reaches through a
*host service*: a named service the program offers to extensions, which a package
declares and a handler calls. The `flm` program offers two, `files` and `programs`; a
program embedding FLM offers the ones it registers (see [Host services for
extensions](crate::guide::embedding#host-services-for-extensions) in the guide to
embedding FLM). The service `files` reads within two folders only and reaches neither the
rest of the machine nor the network; a program run through `programs` may do anything the
user running `flm` may, the network included. No service offers a clock or random
numbers.

**The declaration.** A package lists the services its handlers call under the key `host`
of its package object:

```javascript
ext.package("_", {
  uses: ["stdcore"],
  host: ["files"],
  features: { /* … */ },
});
```

A package that declares a service the program does not offer does not load: the error
names the package, the extension and the service, and lists the services the program
offers. The list enters the registry digest, so adding a service to it is a change of the
extension. A handler reaches a declared service only: a name the package did not
declare is refused, even when the program offers it.

**The call.** A stage's context, a citation provider's context and a template module's
context answer `cx.host(name)`: the service, as an object valid for the handler call.
The service `files` is an object with the method `read_text(scope, path)`; any other
service is an object with the method `call(method, args)`, the arguments and the answer
being data. A failure of the service is thrown as an `FlmError` of the kind `host`, whose
text begins with the service and the method (`` `files.read_text`: … ``).

**The service `files`** reads the text of a file in one of two folders, its *scope*:

- `cx.host("files").read_text("extension", path)` reads a file of the extension's own
  folder (an extension of one file has none);
- `cx.host("files").read_text("document", path)` reads a file of the document's folder:
  the folder of the input file, or the current folder for a document given as text.

`FilesScope.Extension` and `FilesScope.Document` (from `flm:host`) name the two scopes.
The path is relative to the folder and written with `/`; a path that is absolute, holds
`\`, or leaves the folder once `.` and `..` are removed is refused, and so is a file of
the document's folder that a symbolic link leads outside it. The answer is the file's
text, or `null` when there is nothing at the path; a file that is not UTF-8 text is
refused. A file is read when the handler calls, during the build: the program records the
document's files read this way among the files of the build, so the live preview (`flm
watch`) builds the document again when one of them changes. A script receives text only;
the files a template module ships are read by FLM itself, as bytes ([A compose template
written as a module](#a-compose-template-written-as-a-module)).

**The service `programs`** finds a program installed on the machine and runs it in a
fresh temporary folder. The `flm` program offers it to every document; a program embedding
FLM offers it only when it asks for it. Its two methods:

- `cx.host("programs").call("find", { names, folders })` answers the path of a program, or
  `null`. The names are tried in order (`["gs", "gswin64c"]`); for each name, the folders
  of the environment variable `PATH` are searched (on Windows, with each extension of
  `PATHEXT` added to a name that has none), then the folders of `folders`, optional, in
  their order. In a folder of `folders`, `*` stands for any run of characters inside one
  folder name (`/opt/gs*/bin`, `C:/Program Files/MyUtil*/bin`); where several folders
  match, the one with the highest version comes first, runs of digits being compared as
  numbers (`gs10.02.1` before `gs9.56.1`). A name that holds a folder separator is taken
  as a path.
- `cx.host("programs").call("run", { program, args, files, stdin, collect, timeout_ms })`
  makes a fresh temporary folder, writes `files` into it (a map from a relative path to
  `{ text }` or `{ base64 }`, the forms of a file's content in an artifact, base64 being
  written with the standard alphabet and padding), starts `program` (a path, as `find` answers it, or a name
  looked up in `PATH`) with the list of texts `args` in that folder, writes `stdin` to its
  standard input when given, and waits for it at most `timeout_ms` milliseconds (60000 when
  not given; past it, the program is stopped and the call fails). It answers `{ status,
  stdout, stderr, files }`: the exit status (`null` when the program was ended by a
  signal), the text the program wrote on its two outputs, and, for each path of `collect`
  at which there is a file after the run, `{ text }` when the file is UTF-8 text and
  `{ base64 }` otherwise. The folder is then removed. A status other than 0 is an answer
  like any other; a program that cannot be found or started, or that runs past its time,
  is an error naming the extension and the program. A path of `files` or `collect` that is
  absolute, leaves the folder or names the folder itself is refused.

A run is not cached and not watched: the program runs at every build, and a change to a
file it reads outside its folder does not make the preview build again. The program is not
confined: it runs with the rights of the user running FLM, and the temporary folder only
gives it a place to work. On Linux, macOS and other Unix systems, a program past its time
is stopped together with the programs it started (LuaLaTeX, started by `latexmk`, for
instance), and so it is when Ctrl+C, `Ctrl+\` (the quit signal), the closing of the
terminal or the signal `SIGTERM` ends the `flm` program; on Windows, only the program that
the service started is stopped at the end of its time.

**Install extensions from trusted sources only.** An extension runs code, and through the
service `programs` it can run any program on the machine with your rights. The folder
`_flmextensions` beside a document comes with the document: building a document received
from someone else runs the extensions in its folder.

### A citation provider from a script

The `citations` feature asks its citation providers for the entry of each cited key (see
[the citation providers](crate::guide::embedding#the-citation-providers) in the guide to
embedding FLM). A feature in JavaScript contributes one to the feature's slot
`PROVIDERS_SLOT` (`flm.citations.providers`, from `flm:stdcore/features/citations`) as an
object with methods:

- `name`, a text beginning with the extension's stem and a dot, unique among the
  providers one `register` contributes;
- `answer_all(requests, cx)`, which answers every request of one call, or
  `answer(request, cx)`, which answers one; when the object has `answer` alone, it is
  called once per request.

The providers are asked in turn, each about the keys no earlier provider claimed: those
of the feature placed last in the feature order first, each feature's in the order it
contributed them. A package's provider thus comes before the `flm` program's own (that of
the feature `citations-standard`, which leaves the keys none of its record sources takes)
when the package's feature is placed after `citations-standard`, as a package the
document uses is by default. A request is `{ key }`, the key as the author wrote it. The answer
holds one item per request, in order: `null` when the key is not this provider's, `{
error }` when it is and its entry cannot be given (the text, which the citations feature
reports at the key's citations), or `{ entry, identity, warnings }`. The entry is made by
the context: `cx.build_entry(parts)` builds it from parts (`{ text }`, `{ source }` for
FLM text, `{ emphasized: [parts] }`, `{ strong: [parts] }`, `{ link: { address, content:
[parts] } }`), and `cx.parse_entry(source, origin)` parses FLM text, `origin` naming
where the text comes from. `identity`, optional, says which work the key names: two keys
with one identity are one cited work, with one number. `warnings`, optional, is a list of
texts, each reported as a Warning at the key's first citation.

The extension `bibfile` of FLM's repository (`flm-ext/tests/js/bibfile/`) answers the keys
from a JSON file beside the document, read through the host service `files`:

```javascript
import { PROVIDERS_SLOT } from "flm:stdcore/features/citations";

function provider(file) {
  return {
    name: "bibfile.json",
    answer_all(requests, cx) {
      const text = cx.host("files").read_text("document", file);
      if (text === null)
        return requests.map(() => ({ error: `the file \`${file}\` beside the document must hold the bibliography entries` }));
      const records = JSON.parse(text);
      return requests.map(({ key }) => {
        const record = records[key.toLowerCase()];
        if (record === undefined) return null;
        const parts = [{ text: `${record.author}, ` }, { emphasized: [{ text: record.title }] }, { text: ` (${record.year})` }];
        return { entry: cx.build_entry(parts), identity: key.toLowerCase() };
      });
    },
  };
}

// The package `_` declares `host: ["files"]`; its feature's `register`:
register(b, settings) {
  b.peer_slot(PROVIDERS_SLOT, provider(settings.file ?? "bib.json"));
},
```

Since the file is read when the provider is asked, during the build, a change to it makes
the live preview build the document again, with the same extensions.

When the provider's call fails (an exception, a limit of the engine, an answer of another
shape), every key of the call is answered with an error whose text holds "the citation
provider `NAME` of the feature `F` of the package `P` of the extension `S` failed: …",
which the citations feature reports at the key's citations; no later provider is asked
about those keys.

### A compose template written as a module

A compose template may be a JavaScript module rather than a text for a template engine:
its default export is an object that describes the template, chooses the files it ships
when it is applied, and renders the page.

- `name`: the template's name, which a document chooses (`-t NAME`, or the setting
  `compose: {template: NAME}`). It is not under the stem rule, and it must be unique among
  the compose templates of the packages in use, as every template's name.
- `describe(settings)`: answers `{ output, accepts, requires, config }`, the template's
  description: the output it serves, the pieces it accepts (`"all"` or a list of piece
  names), the pieces it requires (optional) and its own settings with their defaults
  (`config`, optional). It runs inside the `register` of the feature that contributes
  the module, given that feature's configuration, so a description may depend on it.
- `files(data, cx)`, optional: answers the files the template ships, chosen when it is
  applied, each `{ name, extension, media_type, text }` (a text the module computes) or
  `{ name, file, scope, media_type }` (a file of the extension's folder or of the
  document's folder, `scope` being `"extension"` or `"document"`, read by FLM as bytes;
  in this form `media_type` may be left out when the file's extension is one the program
  knows). The program names the files and writes them beside the page.
- `render(data, cx)`: answers the page, a text. `data` is the template's context: `{ html,
  regions: [[name, text], …], pieces: [{ name, place, text, words }, …], meta, config,
  artifacts: [[name, path], …] }`, `config` holding the template's settings and
  `artifacts` the path of each shipped file, relative to the page's folder (empty when
  `files` runs). A number of `meta` or `config` must be a whole number.

A feature contributes the module to the slot `TEMPLATES_SLOT` (`flm.compose.templates`,
from `flm:stdcore/features/compose`) with `b.peer_slot(TEMPLATES_SLOT, module)`. FLM then
contributes the template the description gives, with a template engine of its own whose
name is made of the package's name and the template's, and which calls `files` and
`render`. A document sets the template's settings as any template's, under the
output's name and then the template's: `compose: {template_config: {OUTPUT: {NAME: …}}}`.
Reading the files a module ships, and reading
its own files with `cx.host("files")`, needs `host: ["files"]` in the package object.

The extension `tinytpl` of FLM's repository (`flm-ext/tests/js/tinytpl/`) ships the module
`tiny`: a heading from its feature's configuration, a stylesheet it computes, a partial it
reads from its own folder, and a banner image from the document's folder when the
template's setting `banner` names one:

```javascript
// templates/html/tiny.js
export default {
  name: "tiny",
  describe(settings) {
    return {
      output: "html",
      accepts: "all",
      requires: [],
      config: { heading: settings.heading ?? "Tiny", color: "teal", banner: null },
    };
  },
  files(data, cx) {
    const files = [{ name: "style", extension: "css", media_type: "text/css", text: `h1 { color: ${data.config.color}; }\n` }];
    if (data.config.banner !== null) files.push({ name: "banner", file: data.config.banner, scope: "document" });
    return files;
  },
  render(data, cx) {
    const regions = Object.fromEntries(data.regions);
    const artifacts = Object.fromEntries(data.artifacts);
    const head = cx.host("files").read_text("extension", "templates/html/head.html") ?? "";
    const banner = artifacts.banner === undefined ? "" : `<img src="${artifacts.banner}" alt="">\n`;
    const after = data.pieces.filter((piece) => piece.place === "after-content").map((piece) => piece.text).join("\n");
    return `<!doctype html>\n<html>\n<head>\n${head}<title>${regions.title ?? ""}</title>\n` +
      `<link rel="stylesheet" href="${artifacts.style}">\n${regions.styles ?? ""}\n</head>\n<body>\n` +
      `<h1>${data.config.heading}</h1>\n${banner}${regions.content}\n${after}\n${regions.scripts ?? ""}\n</body>\n</html>\n`;
  },
};
```

```javascript
// main.js
import { TEMPLATES_SLOT } from "flm:stdcore/features/compose";
import tiny from "./templates/html/tiny.js";

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    host: ["files"],
    features: {
      tinytpl: {
        title: "Tiny page",
        settings: { config: { docs: "The page's heading.", schema: { type: "object", properties: { heading: { type: "string" } } } } },
        register(b, settings) { b.peer_slot(TEMPLATES_SLOT, tiny); },
      },
    },
  });
}
```

A document `notes.flm` then writes `use: [tinytpl]`, and `flm render -f html -t tiny -o
notes.html notes.flm` writes the page `notes.html` with `notes-style.css` and, when the
setting `banner` is given, `notes-banner.png` beside it: each file the module ships is
named after the page (the stem that `-o` gives), followed by the name the module gives it.
The partial and the banner are read during the build, so the live preview
builds again when one of them changes.

A failure of `describe` fails the feature's `register`, hence the registry build. A
failure of `files` or `render` (an exception, a limit, an answer of another shape, a
shipped file that is not there) fails the template: the Error
`flm.compose.template-failed`, whose text holds "the template module `NAME` of the feature
`F` of the package `P` of the extension `S` failed: …", and the output is not made.

### Two examples

The first example is an extension with one package and one feature that has three
settings, one per binding: `shout_suffix`, in the configuration, which `register` reads;
`min_length`, in the option type `wordcount.counting`, set per render; `report_counts`,
in the option type `wordcount.reporting`, set on the document. It declares the macro
`\shout`, whose replacement an expansion hook computes, and a stage that counts each
tree's words into a table (abridged; `helpers.js` exports `count_words`):

```js
// wordcount/main.js
import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
import { ArgDef } from "flm:lang/mint";
import { chars_text } from "flm:lang/text";
import { Definition, UserDefinition, Expansion } from "flm:lang/rewrites/userdefs";
import { element_name_under, expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";
import { count_words } from "./helpers.js";

const object = (key, type) => ({ type: "object", properties: { [key]: { type } } });
const SETTINGS = {
  config: { docs: "What the macros write.", schema: object("shout_suffix", "string") },
  options: {
    "wordcount.counting": { bind: "render", docs: "How words are counted.", schema: object("min_length", "integer") },
    "wordcount.reporting": { bind: "document", docs: "What the count reports.", schema: object("report_counts", "boolean") },
  },
};

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: { wordcount: { title: "Word count", settings: SETTINGS, register } },
  });
}

function register(b, settings) {
  const suffix = settings.shout_suffix ?? "";
  const shout = Definition.macro_("shout", [ArgDef.mandatory("text")]);
  const element = element_name_under("wordcount", shout);
  b.element(new UserDefinition(shout).element_def_with(b.feature(), element, {
    expand(node, cx, xp) {
      const text = chars_text(node.argument_content_nodes_named("text")) ?? "";
      return Expansion.Emit([cx.stage_chars(text.toUpperCase() + suffix)]);
    },
  }));
  b.lowering_rule(expand_in_tree_rule_for(element, shout));
  b.table("wordcount.words", "the number of words of each tree");
  b.capability("wordcount.counted", "the words of each tree counted", null);
  b.collection_member("flm.validated", "wordcount.counted");
  b.stage({
    decl: () => new StageDecl("wordcount.count", Effect.Compute, Scope.PerTree)
      .writes("wordcount.words").provides("wordcount.counted"),
    needs: () => new Needs().option("wordcount.counting").option("wordcount.reporting"),
    run(cx, out) {
      const min = cx.option("wordcount.counting").min_length ?? 1;
      let words = 0;
      for (const node of cx.my_tree().descendants()) if (node.is_chars()) words += count_words(node.chars(), min);
      out.table("wordcount.words").push({ tree: cx.tree_id(), words });
      if (cx.option("wordcount.reporting").report_counts === true)
        out.diags({ id: "wordcount.count", severity: "note", message: `the tree \`${cx.tree_id()}\` holds ${words} words` });
    },
  });
}
```

A document uses it and sets its three settings in its front matter:

```yaml
flm:
  use: [wordcount]
  features:
    wordcount: {shout_suffix: '!', min_length: 3, report_counts: true}
```

A stage runs when something demands what it writes, as every stage does. The table
`wordcount.words` is demanded by no output, so the stage `wordcount.count` also provides
the capability `wordcount.counted`, and the feature adds that capability to the
collection `flm.validated` with `b.collection_member` (see [Capabilities, collections,
ordering and workflows](#capabilities-collections-ordering-and-workflows)). Every
standard workflow demands `flm.validated`, so every render of a document that uses
`wordcount` runs the count and reports its Notes, as it runs the checks of the compiled
features that joined the collection. A feature written in JavaScript joins a collection
as a compiled feature does: the capability is the feature's own, and must begin with the
extension's stem and a dot; the collection is another feature's.

A stage that writes a table which no collection covers runs when a workflow demands its
table. An extension of the engine `data` can define such a workflow:

```yaml
# counting.yaml
extension:
  description: The HTML page with the word count.
flm:
  use: [wordcount]
  workflows:
    counted:
      extends: html
      demands: [flm.validated, wordcount.words]
```

A document that uses `[wordcount, counting]` and is rendered with `-p counted` runs
every stage that the workflow `html` runs, and the stages that write `wordcount.words`.

The second example is an extension of two packages that share one module. The package
`_` defines `\tpmark`; the package `proofs` uses it (`.#_`) and adds `\tpqed`. A
document that lists `twopack#proofs` gets both packages. The two packages share the
extension's engine instance, so the module `./shared.js` is evaluated once in each
instance, for both (abridged):

```js
// twopack/main.js
import { Definition, UserDefinition } from "flm:lang/rewrites/userdefs";
import { element_name_under, expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";
import { mark } from "./shared.js";

function declare(b, definition) {
  const element = element_name_under("twopack", definition);
  b.element(new UserDefinition(definition).element_def(b.feature(), element));
  b.lowering_rule(expand_in_tree_rule_for(element, definition));
}

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: { twopack: { title: "Marks", register(b) { declare(b, Definition.macro_("tpmark", []).content(mark("mark"))); } } },
  });
  ext.package("proofs", {
    uses: [".#_"],
    features: { proofs: { title: "Proofs", register(b) { declare(b, Definition.macro_("tpqed", []).content(mark("qed"))); } } },
  });
}
```

### An extension in WebAssembly

An extension in WebAssembly is a folder whose manifest names the engine `wasm` and its
module, one WebAssembly file inside the extension's folder:

```yaml
# wordcount/flmextension.yaml
extension:
  description: Counts words, collects to-do markers, shouts
  engine: wasm
  wasm:
    module: ./wordcount.wasm
```

`extension.wasm.module` is the module's path inside the extension's folder, and
`extension.wasm.limits`, optional, sets the limits of the extension's own instances (below).
The module is the extension's one file of code: it is read once, when the extension is
loaded, and its package objects, features and handlers are what its entry points answer,
as a JavaScript extension's are what its main module declares.

**The interface between the host and the module.** Every request of the module goes to
the host through one imported function, `flm.call`, which names the request: a call of a
method of the extension API, the lending of an object the host will call back (a stage, an
expansion hook, a lowering rule's body), a line written to `console`. It exports one entry
function, `flm_entry`, which the host calls with the name of an entry point (`define`,
`packages`, `register`, `run_stage`, `expand`, `lower`, …) and its arguments, and which
routes the call by that name. Both directions are JSON text in the module's memory, in
the shapes the JavaScript binding's values have (an object of the host as its handle, a
number, a text, a list, a map). A second import, `flm.take`, and the exports `flm_alloc`
and `flm_free` move that text in and out of the module's memory. A Rust extension never
writes this interface itself: the guest client of the next section writes it. The
module documentation of `flm_ext::wasm` states it in full, for a module written in
another language.

**What a module reaches.** Nothing but the host's imports: no file, network, clock,
random source or program. A module that imports anything else is refused when it is
loaded. What it needs from outside it asks of the host services its package declares
under `host` ([Host services](#host-services)), through `flm.call`, as any call of the
extension API.

**Limits.** Every handler call has a budget of *fuel*, about one unit per instruction
the module runs, and every instance a memory limit. The host sets both (by default
1,000,000,000 units and 64 MiB); the manifest's `extension.wasm.limits` sets either for
the extension's own instances, in place of the host's: `memory_mib` (MiB) and `fuel`
(units). A call that runs out of fuel stops, and the handler fails with "… must finish
within its budget of N fuel units". A module that asks for memory past the limit is
refused it; when it then stops, as a Rust module's allocator does, the handler fails
with "… must stay within the memory limit of N bytes". After either, and after any other
trap of the module, the instance is replaced once the call returns. A module whose
initial memory is above its limit does not load. The depth of nested calls inside the
module is bounded by the interpreter and cannot be set.

**Instances.** Each thread that runs a handler of the extension runs its own instance
of the module, made from the module read at load, and an instance that has not run a
feature's `register` with the configuration at hand runs it again first, as a
JavaScript instance does: a feature's `register` declares the same things under the
same names, in the same order, every time it runs with one configuration. Module state
is therefore no channel between handler calls.

### A Rust extension compiled to WebAssembly

An extension written in Rust that a program loads without having been compiled with it
is a WebAssembly module written against the *guest client*: the module `flm_ext::guest`,
under `flm-ext`'s cargo feature `guest`. The guest client holds one module per module of
the extension API, `flm_ext::guest::core::stage` for `flm_ext::api::core::stage` and so
on, with the same items under the same names. A façade (the feature builder, a stage's
context, a node) is a struct whose methods forward each call to the host and answer a
`Result<_, ExtError>`; the handler traits (`Stage`, `Expand`, `Lower`, …) are the
guest's own, with the API's methods and defaults; plain data (`StageDecl`, `Needs`) is
the API's own type. Everything the sections above show for JavaScript — lowering rules,
backends, rewrite stages, citation providers, template modules, host services — a
module reaches under the same names. The `flm` program runs such extensions: its cargo
feature `wasm` is on by default.

**The macro `extension!`.** The crate's root invokes `flm_ext::guest::extension!` once,
with the function that declares the packages; the macro writes the module's exports and
routes the entry `define` to that function. The function declares each package with
`ext.package(name, Package)`, a `Package` holding its `uses`, its `flm` block, its
`templates`, its `host` services and its features, a `Feature` its `title`, `docs`,
`version`, `settings` and `register`. The word count of the example above, written in
Rust, abridged to `register` and one stage (`flm-ext` is not a dependency of this
guide's crate, so the example is not compiled):

```ignore
// wordcount/src/lib.rs
use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Value, json};

mod helpers;

const WORDS: &str = "wordcount.words";
const COUNTED: &str = "wordcount.counted";

flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().uses(["stdcore"]).feature(
            "wordcount",
            Feature::new().title("Word count").settings(settings()).register(register),
        ),
    );
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    b.table(WORDS, "the number of words of each tree")?;
    b.capability(COUNTED, "the words of each tree counted", None)?;
    b.collection_member("flm.validated", COUNTED)?;
    b.stage(Arc::new(Count))?;
    Ok(())
}

struct Count;

impl Stage for Count {
    fn decl(&self) -> StageDecl {
        StageDecl::new("wordcount.count", Effect::Compute, Scope::PerTree)
            .writes(WORDS)
            .provides(COUNTED)
    }

    fn needs(&self) -> Needs {
        Needs::new().option("wordcount.counting")
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let min = cx.option("wordcount.counting")?["min_length"].as_u64().unwrap_or(1) as usize;
        let tree = cx.tree_id()?.unwrap_or_default();
        let mut words = 0;
        if let Some(root) = cx.my_tree()? {
            for node in root.descendants()? {
                if node.is_chars()? {
                    words += helpers::count_words(&node.chars()?.unwrap_or_default(), min);
                }
            }
        }
        out.table(WORDS)?.push(json!({ "tree": tree, "words": words }))?;
        Ok(())
    }
}

// `settings()` answers the feature's settings declared as data, the JavaScript
// example's `SETTINGS`; `helpers.rs` holds `count_words`.
```

**Building the module.** The crate is of the type `cdylib` (`[lib] crate-type =
["cdylib"]`), depends on `flm-ext` without its default features and with the feature
`guest` alone, and its release profile has `panic = "abort"` (a small module also asks
for `opt-level = "s"`, `lto = true` and `codegen-units = 1`). With the target installed
(`rustup target add wasm32-unknown-unknown`), it is built with

```text
$ cargo build --target wasm32-unknown-unknown --release
```

which writes `target/wasm32-unknown-unknown/release/wordcount.wasm`; that file, copied
beside the manifest, is the extension's module. A handler that returns `Err` fails with
that error, as a compiled feature's handler does; a panic stops the module, and the
handler fails with the panic's message.

**From a crate written against `flm_ext::api`.** A crate written against the extension
API moves to the guest client mechanically: the module paths `flm_ext::api::…` become
`flm_ext::guest::…`, a façade type in a signature becomes the struct of the same name
(`&dyn StageCx` is `&StageCx`, `&dyn Node<'_>` is `&Node`), and a call that crosses to
the host answers a `Result` where the API's answers its value (a `?` more). The module
documentation of `flm_ext::guest` states how each item of the API appears in the guest,
and names the few places where the move is not mechanical.

**What shipping a Rust extension this way means:**

- it is one `.wasm` file beside a manifest `engine: wasm`, found on the search path like
  any extension;
- it runs sandboxed: no file, no network, no clock, nothing but the host's imports and
  the host services its package declares;
- it is one file for every platform and every Rust toolchain, with no interface to keep
  stable but the JSON shapes of the extension API;
- every call of the API crosses as JSON text, so a loop over many nodes runs slower than
  the same loop compiled into the program;
- its author builds it with `cargo build --target wasm32-unknown-unknown --release` and a
  release profile with `panic = "abort"`;
- a program never loads a dynamic library as an extension: a Rust extension that is not
  compiled into the program is a WebAssembly module, and the same crate can also be
  compiled into a program (next section).

### A Rust extension compiled into a program

An extension written in Rust against the guest client can also be compiled into a
program, as a Rust library the program links, instead of being loaded as a WebAssembly
module. The program then runs the extension with the engine `native`: the module
`flm_ext::native`, under `flm-ext`'s cargo feature `native`. Nothing else changes for the
extension: it declares its packages, features and handlers as the section above shows,
and a document lists its packages under `use` as it lists any extension's.

**One crate for both forms.** The extension's source is the same in both forms: it is
written against `flm_ext::guest`, its crate depends on `flm-ext` with the feature `guest`
alone, and its root invokes the macro `extension!` once. How the guest client reaches the
host depends on whether `flm-ext`'s cargo feature `native` is on in the build:

- without `native`, the crate compiled for `wasm32-unknown-unknown` is a WebAssembly
  module, and every call of the API crosses as JSON text through the module's import (the
  section above);
- with `native`, which a program that carries the engine `native` turns on, the crate is a
  Rust library of that program, built for any target, `wasm32-unknown-unknown` included (a
  program that runs the library in a web browser, for instance); every call of the API is
  answered in the same process, by the same code of the host, passed as a JSON value
  (`serde_json::Value`) with no text.

A crate meant for both forms keeps the default type of its library, which a program
depends on, and builds its WebAssembly form with `cargo rustc --target
wasm32-unknown-unknown --release --crate-type cdylib` (the release profile with
`panic = "abort"` as above).

**Building the WebAssembly form apart from a program.** Within one build, cargo compiles
`flm-ext` once, with one set of features, for every crate of the build. An extension's
WebAssembly module is therefore built in a cargo run where no crate turns on `native`: with
the extension's package selected alone (`cargo rustc -p NAME …`), not in a build of a whole
workspace that also holds a program carrying the engine. A module built in a run where some
crate turns on `native` lacks the functions a module of the engine `wasm` must export
(`flm_entry`, `flm_alloc`, `flm_free`), and the engine refuses it when it loads, with one
message for each function the module lacks, saying which.

**The macro.** On every target, `extension!` writes one public static at the crate's
root, `FLM_EXTENSION` (a `flm_ext::guest::CompiledExtension`), holding the function that
declares the packages and the crate's name and version; for `wasm32`, in a build without
the feature `native`, it also writes the module's exports. A program reaches the extension
through the static. Every extension crate writes a static of the same name, and none
exports a symbol in a build with `native`, so several extension crates link into one
program.

**How a program carries an extension.** A program that compiles an extension in depends
on the extension's crate and on `flm-ext` with the feature `native`. It gives the engine
`native` the list of the extensions it holds, each as a `flm_ext::native::NativeExtension`:
the stem a document's `use` entry gives (`wordcount`), the text of the extension's
manifest, and the crate's static:

```ignore
use std::sync::Arc;

use flm_ext::native::{NativeEngine, NativeExtension};

let engine = NativeEngine::new([NativeExtension {
    stem: "wordcount",
    manifest: include_str!("../wordcount/flmextension.yaml"),
    compiled: &wordcount::FLM_EXTENSION,
}]);
let loader = flm_ext::ExtensionLoader::new(sources).with_engine(Arc::new(engine));
```

The program's source answers the extension's stem with its manifest, `extension.engine`
being `native` (the manifest specifies no module); the loader then gives the extension to
the engine `native`, which finds it by its stem. A manifest that specifies the engine
`native` for a stem the engine does not hold is refused, and the message lists the stems
the engine holds. The module documentation of `flm_ext::native` shows the whole set-up
in a compiled example. The `flm` program carries the extension `profile-pdf-latex` this way (see
[PDF through LaTeX](crate::guide::command_line#pdf-through-latex)): its crate is in the
repository's folder `extensions/rust/`, and the same crate builds as a WebAssembly module
with the script `extensions/rust/build-wasm.sh`.

**What differs from a WebAssembly module:**

- the extension is the program's own code, with the program's rights: there is no
  sandbox. An extension that reaches the outside through the host services its package
  declares behaves the same in both forms;
- there is no limit: no fuel, no memory limit, so a handler that never returns keeps the
  run from finishing. Nor is the depth of a value bounded: the engines `js` and `wasm` refuse
  a value nested deeper than 127 levels when they read JSON text, and the engine
  `native` reads no text;
- every call of the API is a call in the same process, so a loop over many nodes runs
  without the cost of JSON text;
- a panic in a handler, or in FLM while it answers one of the handler's calls, fails the
  handler with the panic's message ("… must run without a panic: MESSAGE"), and the
  thread's instance of the extension is replaced, as a module's instance is after a trap.
  A program built with `panic = "abort"` (the strategy of the target
  `wasm32-unknown-unknown`) stops at the panic instead;
- the objects a feature's `register` hands to FLM (stages, rule bodies, hooks) are Rust
  values, dropped when the engine discards an instance (after a panic, or once the
  extension is no longer in use) and when the thread ends; a panic in their `Drop` fails
  no handler, and only Rust's panic hook reports it (by default on standard error);
- a call of the API from a thread the extension started has no host to answer it, and
  fails with `ExtError::Unsupported`;
- the registry's digest takes the extension crate's name and version, not its code,
  where a module's digest takes the module's bytes. A change to the extension that keeps
  the crate's version leaves the digest unchanged, as a change to a feature written in
  Rust does when the feature keeps its version string, unless the program's build
  identity
  ([`RegistryInputs::build_identity`](crate::core::registry::RegistryInputs::build_identity),
  such as a hash of the program's binary) changes with it.

Each thread runs its own instance of the extension, as for a module: the guest runtime's
state (what `define` declared, what each `register` recorded) belongs to the instance, two
extensions compiled into one program never see each other's, and an instance that has not
run a feature's `register` with the configuration at hand runs it again first.

## The standard features as worked examples

Every extension point of this chapter is used by the standard package, so the shortest
way to see one in use is to read the feature that uses it:

| feature | what to read it for |
|---|---|
| [`textstyles`](crate::stdcore::features::textstyles) | the smallest complete feature: one element type with four values, one lowering rule, and a set of style names of its own that the rule maps onto the presentation vocabulary |
| [`literals`](crate::stdcore::features::literals) | a family of commands declared from one list, a macro and a special character sequence in the same element type, a rule that emits a text node, and a rule that emits nothing |
| [`headings`](crate::stdcore::features::headings) | element data built from the feature's configuration (one element value per declared heading kind, written under the names of its commands), three aspects on one element, counters and counter styles contributed as peer contributions, a rule that reads the counter and anchor tables optionally, and the peer declarations of a rule on the recomposed LaTeX |
| [`index`](crate::stdcore::features::index) | one table and one stage, and nothing else |
| [`refpoints`](crate::stdcore::features::refpoints) | one table and one stage, whose table two other features require: the stage asks every node of a document for its reference points (the `Referenceable` aspect), and the other stages read the points from its table |
| [`refs`](crate::stdcore::features::refs) | several stages and tables, an external table, a check added to a collection, two elements that refer to reference points (`\ref` and `\hyperref`) with a general rule and a rule for the recomposed LaTeX each, and content built into a synthesized value that a rule splices |
| [`blocks`](crate::stdcore::features::blocks) | a rewrite stage, a capability with two refinements, an ordering edge declared only when the registry holds the capability it refers to, and a configuration fixed when the registry is built |
| [`math`](crate::stdcore::features::math) | declarations restricted to mathematics mode, a capability declared for another feature to provide, and a library of plain functions that other features' rules call |
| [`math_passthrough`](crate::math_passthrough) | a fallback for names that nothing defines, opaque content, a check stage that provides another feature's capability, and rules on every backend, with rules of their own for the recomposed LaTeX, built with another feature's library (see [A feature that processes mathematics](#a-feature-that-processes-mathematics)) |
| [`verbatim`](crate::stdcore::features::verbatim) | verbatim arguments and verbatim environment bodies |
| [`userdefs`](crate::stdcore::features::userdefs) | author-written definitions, a late-initialization hook that compiles them, and the lowering rules created for each definition |
| [`xnotes`](crate::stdcore::features::xnotes) | an argument that refers to another feature's scoped package, a block role read from an argument's layout, reference points built by one public function that other features use too, a service slot of xnote kinds filled by one public function, public functions that a contributing feature's rule calls to stage its marks, and a piece per xnote kind built by a stage of its own, which another stage replaces when a request prefers it (see [An xnote kind of your own](#an-xnote-kind-of-your-own) and [Building a kind's list your own way](#building-a-kinds-list-your-own-way)) |
| [`cells`](crate::stdcore::features::cells) | an environment in a scoped package of its own and, under a setting, available everywhere too; a block role decided by position; an argument parser of its own; a scoped package that adds one special character; a service slot of declarations that hold no code (see [A cell style of your own](#a-cell-style-of-your-own)); one reader of a presentation payload; a float content handler contributed as a peer contribution |
| [`lowering`](crate::stdcore::backends::lowering::LoweringFeature) | the registration of the presentation vocabulary and of the structural elements' default rules |
| [`text`](crate::stdcore::backends::text::TextBackend) | a backend: its native element set, its stages and tables, its render options, its output and workflow, the shared pieces stage created for it, and its output regions |
| [`CheckWorkflow`](crate::stdcore::CheckWorkflow) | a feature that declares only a collection and a workflow over it |

[`stdcore::package`](crate::stdcore::package) bundles them into the standard package, and
[`StdcoreConfig`](crate::stdcore::StdcoreConfig) is the configuration of those that take
one.

The sections below are recipes for the extension points that four standard features
offer to other features: numbering a construct, declaring heading kinds, contributing an
xnote kind, and declaring cell styles.

### A feature that processes mathematics

The [`math`](crate::stdcore::features::math) feature introduces mathematics into the
language (the math groups `\(…\)` and `\[…\]`, the display-math environments, their
equation lines and numbers, the text commands) and processes none of it. A *processing
feature* does that, and needs nothing else of `math`. It does four things:

1. **It says what a command of mathematics is**: a fallback for macros and environments in
   mathematics mode ([`FallbackDef`](crate::lang::mint::FallbackDef)), or elements declared
   with `[Mode::Math]`, or both.
2. **It provides the capability**
   [`flm.math.content`](crate::stdcore::features::math::CONTENT) with a check stage of its
   own, and adds its check to the collection `flm.validated`, which every standard
   workflow demands
   ([`collection_member`](crate::core::registry::FeatureBuilder::collection_member)), so
   that every workflow runs it. The `math` feature declares `flm.math.content` and adds
   it to no collection. A feature written in Rust adds `flm.math.content` itself, as
   `math-passthrough` does; an extension adds to a collection only a capability named
   under its own stem, so its check provides a capability of its own beside
   `flm.math.content` and the extension adds that one (`math-mathjax.check`).
   A feature that renders mathematics inside the pipeline also provides a refinement of
   [`flm.math.rendered`](crate::stdcore::features::math::RENDERED), named in its own
   namespace.
3. **It registers lowering rules** for `flm.math` (the element of a math group,
   [`MATH`](crate::lang::structural::MATH)) and for `flm.math.environment`
   ([`ENVIRONMENT`](crate::stdcore::features::math::ENVIRONMENT)) on each backend it
   serves. The rules build what the `math` feature knows of a formula with the functions of
   its library: [`frame::line_frames`](crate::stdcore::features::math::frame::line_frames)
   gives each equation line with its anchor and its number or tag, and
   [`math::latex`](crate::stdcore::features::math::latex) the LaTeX that opens and closes a
   formula and each line. On a backend where it registers no rule, every math group is
   the Error `flm.math.unprocessed` of the `lowering` feature's rule of last resort
   ([`MathGroupRule`](crate::stdcore::backends::lowering::MathGroupRule)), and every
   display-math environment the same Error of the `math` feature's rule of last resort
   ([`unprocessed`](crate::stdcore::features::math::unprocessed)).
4. **Its tables and stages are its own**: nothing in the registry is reserved for
   mathematics.

A processing feature is the feature of a *math processor*, a package of its own beside
`stdcore`, which processes no mathematics. A registry holds one math processor: two would
each provide `flm.math.content` and each register a default rule for `flm.math`, which is
an error. The host chooses a default processor for its documents, and a document chooses
its own with the key `math` of its front matter: a processor's name, which stands for
the package `math-NAME`, or `false` for none
([the packages a document uses](crate::guide::command_line#the-packages-a-document-uses)).
A processor whose package's name has another form is listed under `use`, with
`math: false`.

In Rust, the library's math processor [`math_passthrough`](crate::math_passthrough) is the
worked example: a package of one feature, which uses `stdcore`; two fallbacks, a check
stage, three rules marked for every backend (a math group, an environment, an unknown
command of mathematics) and two for the `latex-recomposed` backend.

In JavaScript, the `math-mathjax` extension is the worked example (see [What a feature in
JavaScript adds to the output](#what-a-feature-in-javascript-adds-to-the-output)). Its
`register` follows the same four steps; abridged:

```js
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { CONTENT, ENVIRONMENT, MATH, equation_lines } from "flm:stdcore/features/math";

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      "math-mathjax": {
        register(b, settings) {
          // 1. One element per MathJax command, declared with modes([Mode.Math]).
          declare_commands(b, read_commands(/* … */));
          // 2. The check, which provides flm.math.content and a capability of the
          //    extension's own, which joins flm.validated.
          b.capability("math-mathjax.check", "the mathematics checked by math-mathjax", null);
          b.collection_member("flm.validated", "math-mathjax.check");
          b.stage({
            decl() {
              return new StageDecl("math-mathjax.check", Effect.Check, Scope.PerTree)
                .provides(CONTENT).provides("math-mathjax.check");
            },
            run(cx, out) { /* report every equation line that holds no mathematics */ },
          });
          // 3. The rules of MATH and ENVIRONMENT on html, text and latex-recomposed.
          register_rules(b, settings);
        },
      },
    },
  });
}
```

The package says nothing of `math-passthrough`: the two are independent math processors,
and a document uses one or the other. The `flm` program uses `math-mathjax` by default
([the packages a document
uses](crate::guide::command_line#the-packages-a-document-uses)).

### Number a construct of your own

A feature gives its construct a number (shown beside the construct and in every
reference to it) through the [`numbering`](crate::stdcore::features::numbering) feature,
which numbers every reference point that states a number source. A counter
([`CounterDecl`](crate::stdcore::features::numbering::CounterDecl)) is a dotted name under which things are counted in document order, declared by a feature together
with the counter it is numbered within; a counter style
([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl)) says how a
counter value is shown
([counters and counter
styles](crate::guide::concepts_overview#counters-and-counter-styles)). The numbering
feature declares no command and no lowering rule: the feature that owns the construct
declares the counter, states the number source and shows the number. It takes four steps:

1. **Declare a counter style and a counter.** Build a
   [`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl) and a
   [`CounterDecl`](crate::stdcore::features::numbering::CounterDecl), whose names are
   dotted and begin with your own prefix, and contribute each with
   [`peer_slot`](crate::core::registry::FeatureBuilder::peer_slot) to the numbering
   feature's service slots `flm.numbering.styles`
   ([`STYLES_SLOT`](crate::stdcore::features::numbering::STYLES_SLOT)) and
   `flm.numbering.counters`
   ([`COUNTERS_SLOT`](crate::stdcore::features::numbering::COUNTERS_SLOT)). The counter
   declares its default counter style, and may be numbered within another counter
   ([`CounterDecl::within`](crate::stdcore::features::numbering::CounterDecl::within),
   with a [`Within`](crate::stdcore::features::numbering::Within)). Because these are
   peer contributions, a registry without the numbering feature drops them, with one Note
   per service slot, and your feature still registers.
2. **State a number source on the reference point.** Your element's
   [`Referenceable`](crate::model::aspects::Referenceable) aspect returns a
   [`RefPoint`](crate::model::aspects::RefPoint) whose
   [`number`](crate::model::aspects::RefPoint::number) is a
   [`NumberSource`](crate::model::aspects::NumberSource): the counter, the counter style,
   and the levels of the counter the point advances (`[0]` for a plain number). Leave the
   number source out for an occurrence that is not to be numbered (a starred form, or a
   setting of your feature's configuration); the numbering feature has no setting of its
   own that turns numbering off.
3. **Show the number in the lowering rule.** The numbering feature writes its counter
   table `flm.counters` ([`COUNTERS`](crate::stdcore::features::numbering::COUNTERS), a
   [`Counters`](crate::stdcore::features::numbering::Counters)). Declare it as a peer
   table ([`peer_table`](crate::core::registry::FeatureBuilder::peer_table)), and declare
   that the rule reads it optionally
   ([`LoweringDecl::reads_opt`](crate::core::lowering::LoweringDecl::reads_opt)). In the
   rule, [`Counters::format`](crate::stdcore::features::numbering::Counters::format)
   writes the number of the point ([`PointId`](crate::model::aspects::PointId): the
   node's address and the point's index among its points), and returns `None` for a
   point the document did not number. Stage each segment
   ([`Segment`](crate::stdcore::features::numbering::Segment)) it returns with
   [`Lowerer::splice`](crate::core::lowering::Lowerer::splice). Where the numbering
   feature is registered, the optional read becomes a required one, and the plan
   schedules the numbering stage. Where it is not, the rule reads no table and shows the
   construct without a number.
4. **Include the `refpoints` feature in the registry.** The numbering stage reads the
   reference points from the table `flm.refpoints` of the
   [`refpoints`](crate::stdcore::features::refpoints) feature. The standard package holds
   that feature; a registry assembled from single features must list it, or every plan
   that schedules the numbering stage fails.

A reference to the point (`\ref`) then shows the number without anything more from your
feature: the `refs` feature reads the counter table for the target's number. A document
changes the numbering through its numbering options
([`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions)): which
counter a counter is numbered within, and the value it has before it is first advanced.
In the following example, a `\step{…}` command is numbered `(1)`, `(2)`, … by default;
the document gives the counter the value 10 before its first advance, so the two steps
are numbered `(11)` and `(12)`:

```rust
use std::sync::Arc;

use flm_core::techy::core::node::{BuildId, NodeRef};

use flm_core::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, NumberSource, PointId, RefPoint, Referenceable,
};
use flm_core::model::element::ElementSpec;
use flm_core::model::names::RefKind;
use flm_core::model::options::OptionValues;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder, RegistryInputs,
};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, Counters, Delimiters, FormatOptions, NumberingOptions,
    COUNTERS, COUNTERS_SLOT, STYLES_SLOT,
};

/// The data and the `Referenceable` aspect of the element a `\step` node carries. Its
/// only reference point states the number source.
#[derive(Debug)]
struct Step;

impl Referenceable for Step {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        vec![RefPoint {
            kind: RefKind::from("step"),
            labels: Vec::new(),
            malformed: Vec::new(),
            computed: 0,
            anchor: AnchorPolicy::Auto { prefix: "step".to_string() },
            // The construct counts in `example.step`, is shown in
            // `example.style.step`, and advances the number itself (level 0)
            // at every occurrence.
            number: Some(NumberSource {
                counter: "example.step".into(),
                instances: Vec::new(),
                style: "example.style.step".into(),
                advance: vec![0],
                number_format: None,
            }),
            content: None,
            accepts: AcceptedRefTypes::Any,
        }]
    }
}

/// The rule: it writes the point's number, then the text the author wrote.
#[derive(Debug)]
struct StepRule;

impl Lower for StepRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let point = PointId { addr: lw.addr(), index: 0 };
        // The number of this point, in the counter style its record carries.
        // The segments are taken out of the table before they are spliced,
        // because splicing needs the `Lowerer` mutably.
        let written = lw
            .get(COUNTERS)?
            .and_then(|counters: &Counters| counters.format(point, &FormatOptions::new()))
            .transpose()
            .map_err(|error| LowerError::structure(error.to_string()))?
            .unwrap_or_default();
        let mut out: Vec<BuildId> = Vec::new();
        for segment in &written {
            out.extend(lw.splice(cx, segment.content())?);
        }
        if !out.is_empty() {
            out.push(lw.text(cx, " ")?);
        }
        out.extend(lw.lower_children(cx, &node)?);
        Ok(Lowering::Emit(out))
    }
}

struct Steps;

impl FlmFeature for Steps {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("steps", "Numbered steps")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();

        // Step 1. The counter style, `(3)`, and the counter, numbered within
        // nothing.
        let mut style = CounterStyleDecl::new("example.style.step");
        style.delimiters = Delimiters::new("(", ")");
        b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style))?;
        b.peer_slot::<CounterDecl>(
            COUNTERS_SLOT,
            Arc::new(CounterDecl::new("example.step", "example.style.step").docs("The steps.")),
        )?;

        // Step 2. The element, whose `Referenceable` aspect states the number
        // source.
        let value = Arc::new(Step);
        let element = Arc::new(
            ElementSpec::new("example.step")
                .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                .with_value(value),
        );
        b.element(
            ElementDef::new(
                owner,
                ["step"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                element,
            )
            .docs("`\\step{…}`: one numbered step."),
        )?;

        // Step 3. The counter table belongs to the numbering feature, which
        // may be absent: a peer table, read optionally.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "The counter values, read to show a step's number.",
        )?;
        b.lowering_rule(
            LoweringRule::new(
                "example.step.lower",
                "example.step",
                LoweringDecl::new().reads_opt(COUNTERS),
                Arc::new(StepRule),
                DefaultMark::Everywhere,
            )
            .docs("a step lowers to its number, a space, and the text the author wrote"),
        )?;
        Ok(())
    }
}

// Step 4. The standard package holds the `refpoints` feature, which the
// numbering stage requires.
let mut builder = RegistryBuilder::new();
builder.use_package(flm_core::stdcore::package(flm_core::stdcore::StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(Steps));
let registry = builder.build(&RegistryInputs::new())?;

# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
# use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
let source = Arc::new(
    FlmSource::new("\\step{Mix.} \\step{Bake.}").with_origin(FlmOrigin::none()),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));

// The document gives the counter the value 10 before its first advance.
let options = OptionValues::new().with(NumberingOptions::new().initial("example.step", 10));
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(Arc::new(Doc::with_options(content, options)), plan, Externals::none())?;
run.execute(&RunOptions::new())?;

assert_eq!(run.table(TEXT_OUTPUT)?.expect("written").text, "(11) Mix. (12) Bake.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

**The same steps in JavaScript.** A feature of an [extension written in
JavaScript](#an-extension-in-javascript) takes the same four steps through the modules
under `flm:`. The test extension `exercises` (the folder `flm-ext/tests/js/exercises/` of
the FLM repository) declares a numbered environment `\begin{exercise}[title]…\end{exercise}`
with `\label` fields, and writes a heading line "Exercise 3 (title)" before the body
(abridged: the extension also declares the command `\hint`, legal inside an exercise
alone, and a LaTeX rule; the files hold the whole):

```js
// exercises/main.js
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, BodyDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole, RefPoint, NumberSource, AnchorPolicy, ContentRef, AcceptedRefTypes } from "flm:model/aspects";
import * as refs from "flm:stdcore/features/refs";
import { COUNTERS, COUNTERS_SLOT, STYLES_SLOT, CounterDecl, CounterStyleDecl, FormatOptions, PrefixWords }
  from "flm:stdcore/features/numbering";
import { BLOCK, SPAN } from "flm:stdcore/backends/pres";

const EXERCISE = "exercises.exercise";          // the element
const COUNTER = "exercises.exercise";           // the counter
const STYLE = "exercises.style.exercise";       // its counter style

// Step 2. The reference point of an exercise: its labels, an anchor, and the number
// source naming the counter and the style.
function points(node) {
  const [labels, malformed] = refs.point_labels(refs.label_uses(node));
  return [new RefPoint({
    kind: "exercise",
    labels, malformed, computed: 0,
    anchor: labels.length ? AnchorPolicy.FromFirstLabel : AnchorPolicy.Auto({ prefix: "exercise" }),
    number: new NumberSource({ counter: COUNTER, style: STYLE, advance: [0] }),
    content: null,
    accepts: AcceptedRefTypes.these(["ex"]),
  })];
}

// Step 3. The heading line: the number the counter table writes, then the title.
function heading(node, cx, lw) {
  const point = { addr: lw.addr(), index: 0 };
  const children = [lw.text(cx, "Exercise ")];
  const counters = lw.get(COUNTERS)?.counters ?? null;
  const segments = counters?.format(point, new FormatOptions().without_word().without_delimiters()) ?? [];
  for (const segment of segments) children.push(...lw.splice(cx, segment.content));
  if (node.argument_is_provided("title"))
    children.push(lw.text(cx, " ("), ...lw.lower_arg_named(cx, node, "title"), lw.text(cx, ")"));
  return lw.pres(cx, SPAN, { role: "exercises.heading" }, children);
}

export default function (ext) {
  ext.package("_", {
    // Step 4. The standard package holds the `refpoints` feature.
    uses: ["stdcore"],
    features: {
      exercises: {
        title: "Exercises",
        register(b) {
          // Step 1. The counter style, whose words `\ref` writes before the number,
          // and the counter, contributed as data.
          const style = new CounterStyleDecl(STYLE);
          style.prefix_words = new PrefixWords("Exercise~", "Exercises~");
          b.peer_slot(STYLES_SLOT, style);
          b.peer_slot(COUNTERS_SLOT, new CounterDecl(COUNTER, STYLE).docs("The exercises."));
          // Step 2. The element, whose `Referenceable` aspect is an object of the script.
          b.element(new ElementDef(b.feature(), ["exercise"],
            SyntaxDef.Environment([ArgDef.optional("title"), refs.label_fields_arg()], new BodyDef().blocks()),
            new ElementSpec(EXERCISE)
              .with(Aspect.BlockRoleAspect(BlockRole.Block))
              .with(Aspect.Referenceable({ points })),
          ).docs("`\\begin{exercise}[title]\\label{ex:…}…\\end{exercise}`: a numbered exercise."));
          // Step 3. The counter table and the anchors table, read optionally.
          b.peer_table(COUNTERS, "The counter values, read for an exercise's number.");
          b.peer_table(refs.ANCHORS, "The anchors, read for an exercise's anchor.");
          b.lowering_rule(new LoweringRule("exercises.exercise.lower.default", EXERCISE,
            new LoweringDecl().reads_opt(COUNTERS).reads_opt(refs.ANCHORS).produces(BLOCK).produces(SPAN),
            { lower(node, cx, lw) {
                const anchor = lw.get(refs.ANCHORS)?.anchors.id({ addr: lw.addr(), index: 0 }) ?? null;
                const content = [heading(node, cx, lw), ...lw.lower_body(cx, node)];
                return Lowering.Emit([lw.pres(cx, BLOCK, { role: EXERCISE, anchor }, content)]);
            } },
            DefaultMark.OnBackends(["html", "text"]),
          ).docs("An exercise lowers to a block: its heading line, then its body."));
        },
      },
    },
  });
  // A second package lists the titled exercises in the table of contents, at level 3,
  // under their titles: a `Sectioning` aspect attached to the first package's element.
  ext.package("intoc", {
    uses: [".#_"],
    features: {
      intoc: {
        register(b) {
          b.attach(EXERCISE, Aspect.Sectioning({
            level(node) { return 3; },
            in_toc(node) { return node.argument_is_provided("title"); },
            toc_title(node) { return ContentRef.ArgumentNamed("title"); },
          }));
        },
      },
    },
  });
}
```

A reference to an exercise then writes the counter style's words before the number:
`\ref{ex:first}` writes "Exercise 1" in the text output, and `\ref[noprefix]{ex:second}`
writes "2". The extension's LaTeX rule writes an exercise into the recomposed LaTeX as
the environment `exercise` that its definition piece `\newtheorem{exercise}{Exercise}`
declares, with the labels and references written by the commands of the recomposed
LaTeX's style file:

```latex
\begin{exercise}[A title]\flmLabels{\label{ref1}}
Solve it. Hint: Think of the first case.
\end{exercise}

See \flmRef{ref1} and \flmRef[noprefix]{ref2}.
```

The [numbering guide](crate::guide::numbering) describes each part in full: what a
counter declaration holds and the checks the registry build makes on it ([declaring a
counter](crate::guide::numbering#declaring-a-counter)), what a counter style is made of,
how a counter is numbered within another
([numbering within another counter](crate::guide::numbering#numbering-within-another-counter)),
the levels a point advances and the errors of a point that cannot be numbered ([numbering
a construct: the number
source](crate::guide::numbering#numbering-a-construct-the-number-source)), the numbering
options of a document, a construct numbered afresh inside each occurrence of an enclosing
construct ([scoped instances](crate::guide::numbering#scoped-instances)), and the other
ways a rule writes a number ([showing a number from a lowering
rule](crate::guide::numbering#showing-a-number-from-a-lowering-rule)). A feature that
needs a form the declarations cannot express contributes a within-joiner of its own
([numbering within another
counter](crate::guide::numbering#numbering-within-another-counter)) or a number writer of
its own ([the number writer](crate::guide::numbering#the-number-writer)). The [`headings`](crate::stdcore::features::headings) feature is a
complete example inside the library: it declares a counter and a counter style per
numbered heading kind as peer contributions, and its rule reads the counter table
optionally.

### Heading kinds and commands of your own

The `headings` feature builds its heading kinds and heading commands from its
configuration, so a host adds its own without writing a feature. A **heading kind**
([`HeadingDecl`](crate::stdcore::features::headings::HeadingDecl)) is a declared heading
(section, subsection, …) with a name, a level and its numbering. A **heading command**
([`HeadingCommand`](crate::stdcore::features::headings::HeadingCommand)) is a macro that
writes a heading of one kind, with default key-values of its own. Both go in a
[`HeadingsConfig`](crate::stdcore::features::headings::HeadingsConfig), which a host
passes to the standard package; the configuration is fixed when the registry is built and
is an input of the registry digest.

```rust
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::features::headings::latex::HeadingsLatexOptions;
use flm_core::stdcore::features::headings::{HeadingCommand, HeadingDecl, HeadingsConfig};
use flm_core::stdcore::features::numbering::NumberTemplate;
use flm_core::stdcore::{self, StdcoreConfig};

// The five shipped kinds; an `appendix` kind, whose number (where the
// configuration numbers headings of its level) is written A, B, C in a counter
// of its own; and `\quietsection`, which writes a section left out of the
// contents unless an occurrence says `toc=true`.
let mut kinds = HeadingsConfig::new().headings;
kinds.push(HeadingDecl::new("appendix", 1).number_format(NumberTemplate::formats("#{Alph}")));
let headings = HeadingsConfig::new()
    .headings(kinds)
    .command(HeadingCommand::new("quietsection", "section").default_value("toc", "false"));

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new().headings(headings)));
let registry = builder.build(&RegistryInputs::new())?;
// Six kinds: six values of the element `flm.heading`.
assert_eq!(registry.elements_named(flm_core::stdcore::features::headings::HEADING).len(), 6);

// In LaTeX, `\appendix` is the switch into the appendices, not a sectioning
// command, so the recomposed LaTeX is asked to write an `appendix` as `\section`.
let render_options =
    OptionValues::new().with(HeadingsLatexOptions::new().command("appendix", "section"));
# let _ = render_options;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

[`HeadingsConfig::headings`](crate::stdcore::features::headings::HeadingsConfig::headings)
gives each kind a command of its own name, and
[`command`](crate::stdcore::features::headings::HeadingsConfig::command) adds one more.
A kind may also be written by no command at all, when the program creates its headings
itself (in a stage, for instance): its value of `flm.heading` is then registered with no
name, and no document can write it. Which kinds are numbered is the configuration's numbering depth
([`HeadingNumbering`](crate::stdcore::features::headings::HeadingNumbering)); by default,
no kind is numbered.

Every kind declared this way is listed in the table of contents that the
[`toc`](crate::stdcore::features::toc) feature builds, at its level, like the shipped
kinds: unless the kind is declared
[`skip_toc`](crate::stdcore::features::headings::HeadingDecl::skip_toc), an occurrence
says `toc=false`, or the level is deeper than the feature's setting `depth`. An entry
shows the occurrence's alternative title, written with the key `alt=`, and its title
otherwise ([the star and the
key-values](crate::guide::language_syntax#the-star-and-the-key-values)). A heading kind
that a feature declares with an element of its own, rather than through
[`HeadingDecl`](crate::stdcore::features::headings::HeadingDecl), is listed when its
element carries the [`Sectioning`](crate::model::aspects::Sectioning) aspect, which
answers the level, whether the occurrence is listed, and what its entry shows.

The LaTeX command a kind is recomposed as, and the HTML tag it is written with, are
render options: the headings feature's
[`HeadingsLatexOptions`](crate::stdcore::features::headings::latex::HeadingsLatexOptions)
and the HTML backend's
[`HtmlRenderOptions::heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags).
They default to the kind's name and to the tag of its level. A kind whose name is not a
LaTeX sectioning command, such as `appendix`, must be mapped to one, since the recomposed
LaTeX runs the command it is given with the title. The recomposed LaTeX leaves what is
written before a heading's title (the number and its joining text) to the LaTeX document
class.

What is written before the title of a numbered heading, on the other backends, is the
kind's **heading title template**,
[`HeadingDecl::title_number_format`](crate::stdcore::features::headings::HeadingDecl::title_number_format):
a text in the template language ([the template language shared by
features](crate::guide::concepts_overview#the-template-language-shared-by-features)) that
states how the number joins the title, `#{number}. ` by default. In it:

- `#{number}` is the number as the kind's counter style writes it (`1.2`);
- `#[bare]{number}` is the kind's own number without the numbers of its parent counters
  (`2`);
- `#[Roman]{number}` is the kind's own number in another number format, with the parent
  counters' numbers written as usual (`1.II`);
- everything else is content: a joining text or a word.

```rust
use flm_core::stdcore::features::headings::HeadingDecl;

// `Chapter 1: Introduction`, with a no-break space after the word.
let chapter = HeadingDecl::new("chapter", 1).title_number_format("Chapter~#{number}: ");
# let _ = chapter;
```

The template states structure, not styling. How the number looks is decided by a
stylesheet, which selects the role `flm.heading.number` that the number carries, or by a
lowering rule of the host that replaces the rule of the element `flm.heading.number`,
which holds the number. A template that does not parse, that uses a placeholder other
than `number`, or that gives `number` a key it does not take, fails the registry build
with an error that gives the kind.

### An xnote kind of your own

An xnote is a piece of text set apart from the running text, marked where it belongs and
shown elsewhere or in place (a footnote, an endnote, a side note), and an xnote kind is
one way of being an xnote ([`xnotes`](crate::stdcore::features::xnotes)). The xnote kinds
that an author writes with a command of their own are the `xnotes` feature's
configuration ([`XnotesConfig`](crate::stdcore::features::xnotes::XnotesConfig)). A
feature that creates xnotes itself (for example, a feature of addenda that makes one
xnote per addendum) contributes an xnote kind with no command, and goes through the same
four steps as the `xnotes` feature's own `\footnote`:

1. **The service slot of xnote kinds.** In its `register` method, the feature calls
   [`contribute_kind`](crate::stdcore::features::xnotes::contribute_kind) with its
   [`XnoteKind`](crate::stdcore::features::xnotes::XnoteKind). That one function makes
   every declaration an xnote kind needs: the kind, into the service slot
   `flm.xnotes.kinds`; a numbered kind's counter and its two counter styles; and the
   kind's list, that is, its element, its piece table, the piece's declaration, the
   builder that writes it ([`ListBuilder`](crate::stdcore::features::xnotes::ListBuilder))
   and the rule that lowers it ([`ListRule`](crate::stdcore::features::xnotes::ListRule)).
   It also makes the peer declarations these need, so the feature declares none of them
   itself (declaring one again does no harm: the registry merges a repeated identical
   peer declaration into the first). The feature therefore builds and renders in a
   registry without the `xnotes` feature, where its xnotes are not collected and no list
   is placed.
2. **The aspect.** The feature's own element implements
   [`XnoteSource`](crate::stdcore::features::xnotes::XnoteSource), which returns, for a
   node, its xnotes: the xnote kind of each, and its content. The content is always a
   node region, one of the node's arguments or content slots, given by position
   ([`NodeRegion`](crate::core::lowering::NodeRegion);
   [`NodeRegion::argument_named`](crate::core::lowering::NodeRegion::argument_named)
   finds an argument's position from its name). The kind's list shows that region by
   lowering it where the list entry is placed, never a copy of it, so the content is
   lowered once, in the list, as the document's own content. The region always belongs
   to a node of the document: an xnote whose content the feature builds itself cannot be
   declared today.
3. **The reference points.** The element's
   [`Referenceable`](crate::model::aspects::Referenceable) aspect returns the points that
   [`ref_points`](crate::stdcore::features::xnotes::ref_points) gives, after its own
   points if it has any: the xnote and its mark, numbered, labeled and given anchors as a
   `\footnote`'s are. The `xnotes` feature's collect stage (the stage that writes the table
   [`flm.xnotes`](crate::stdcore::features::xnotes::XNOTES)) finds xnotes by these points'
   reference kinds, never by element, so every node of the element becomes a record of the table
   [`flm.xnotes`](crate::stdcore::features::xnotes::XNOTES), like every `\footnote`.
4. **The content apart from the text.** The element implements
   [`ApartFromText`](crate::model::aspects::ApartFromText) and returns the regions its
   xnotes are made from, so that their words are not read as part of the text around the
   node: a heading whose title holds the node is identified by the title's own words
   ([`plain_text_of`](crate::lang::text::plain_text_of)), as for a `\footnote`. It takes
   the regions from its own `XnoteSource` implementation through
   [`regions_apart_from_text`](crate::stdcore::features::xnotes::regions_apart_from_text),
   so that the regions are stated once. The arguments of an element that does not
   implement the aspect are read as words of that text.

```rust
use std::sync::Arc;

use flm_core::core::lowering::NodeRegion;
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::aspects::{ApartFromText, ContentRef, RefPoint, Referenceable};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::features::numbering::PrefixWords;
use flm_core::stdcore::features::xnotes::{
    self, contribute_kind, ref_points, XnoteKind, XnoteOf, XnoteSource,
};
use flm_core::stdcore::{package, StdcoreConfig};
use flm_core::techy::core::node::NodeRef;

/// `\myaddendum{text}`: one xnote of the kind `addendum`, whose content is the
/// argument `text`.
struct Addendum(Arc<XnoteKind>);

impl XnoteSource for Addendum {
    fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        NodeRegion::argument_named(node, "text")
            .map(|content| XnoteOf { kind: self.0.name.clone(), content })
            .into_iter()
            .collect()
    }
}

impl Referenceable for Addendum {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        ref_points(&self.0, &[], None)
    }
}

impl ApartFromText for Addendum {
    fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        xnotes::regions_apart_from_text(node)
    }
}

struct Addenda;

impl FlmFeature for Addenda {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("example-addenda", "Addenda")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let kind = Arc::new(
            XnoteKind::numbered("addendum", PrefixWords::new("Add.~", "Adds.~")).label_type("add"),
        );
        contribute_kind(b, Arc::clone(&kind))?;
        let value = Arc::new(Addendum(kind));
        b.element(ElementDef::new(
            b.feature().clone(),
            ["myaddendum"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
            Arc::new(
                ElementSpec::new("example.addendum")
                    .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                    .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                    .with::<dyn ApartFromText>(value as Arc<dyn ApartFromText>),
            ),
        ))
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(package(StdcoreConfig::new()));
builder.feature(Arc::new(Addenda));
let registry = builder.build(&RegistryInputs::new())?;
assert_eq!(
    registry.services().services_named::<XnoteKind>(xnotes::KINDS_SLOT).count(),
    2, // `footnote`, then `addendum`
);
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

A kind with no command is written by no author: `contribute_kind` declares no command,
and the `xnotes` feature declares commands only for the kinds of its own configuration.
Two features that contribute kinds of one name make the registry build fail.

The `xnotes` feature's own lowering rule writes the mark of a `\footnote` through public
functions, which a contributing feature's rule calls to write the xnotes of its own
element in the same way:

- [`stage_mark`](crate::stdcore::features::xnotes::stage_mark), for a listed xnote kind,
  is given the node, the kind and the xnote's position among the node's xnotes of that
  kind (`0` for an element carrying one), and writes the mark alone;
- [`stage_in_place`](crate::stdcore::features::xnotes::stage_in_place), for an in-place
  xnote kind, is also given how the brackets around an in-place xnote are written
  ([`Brackets`](crate::stdcore::features::xnotes::Brackets)).

A rule that calls `stage_mark` declares that it reads `flm.xnotes` and produces
`flm.pres.span` and `flm.pres.link`; a rule that calls `stage_in_place` declares that it
reads `flm.xnotes` and produces `flm.pres.span`, `flm.pres.block` and `flm.pres.par`.
Either may read `flm.counters` and `flm.anchors` optionally: without the first, the mark
has no number; without the second, it has no anchor and no link. Each function's
documentation lists these requirements. The feature writes nothing for its list: the kind's
builder collects the kind's xnotes from `flm.xnotes` together with the footnotes', and
writes one list entry per xnote, holding a content pointer to the region the aspect gives
(never a copy of the content). A content pointer is the payload of a synthesized node
that shows, where the node is placed, content written elsewhere in the tree without
copying it ([`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer)). The
kind's list rule writes the list after the content, under the kind's title, in a list of
its own after the lists of the kinds declared before it, and lowers each region where its
entry is placed.

In content shown a second time (a heading's title that a reference shows), an element
whose [`InCopy`](crate::model::aspects::InCopy) aspect returns
[`copied_as`](crate::stdcore::features::xnotes::copied_as) with the kinds of its xnotes
is shown as its marks, as a `\footnote` is. An element that does not is copied as
written; its node in the copy is not a node of the document, and is reported as holding
no recorded xnote.

### Several xnotes on one node

An element may carry more than one xnote on one node: a command that writes two footnotes
at one place, `\twonotes{first}{second}`, or an element that carries an xnote of each of
two kinds. It goes through the same four steps as an element carrying one, once per
xnote:

- its [`XnoteSource`](crate::stdcore::features::xnotes::XnoteSource) aspect returns one
  [`XnoteOf`](crate::stdcore::features::xnotes::XnoteOf) per xnote, and its
  [`Referenceable`](crate::model::aspects::Referenceable) aspect calls
  [`ref_points`](crate::stdcore::features::xnotes::ref_points) once per xnote, in the
  same order. The two results are paired by order, kind by kind: the node's second pair
  of points of a kind is the xnote whose content is the second `XnoteOf` of that kind. A
  node whose two results hold different numbers of xnotes of a kind is reported as the
  Error [`flm.xnotes.unpaired`](crate::stdcore::features::xnotes::Unpaired);
- the element decides which labels belong to which xnote: each `ref_points` call takes
  the labels of its own xnote (read, for example, from an argument of that xnote), as the
  one xnote of `\footnote[\label{fn:x}]{…}` takes the labels of its optional argument;
- its lowering rule calls
  [`stage_mark`](crate::stdcore::features::xnotes::stage_mark) (or
  [`stage_in_place`](crate::stdcore::features::xnotes::stage_in_place)) once per xnote,
  with the xnote's position among the node's xnotes of its kind (`0`, then `1`), and
  places what each call returns where it wants: the marks side by side, or apart. Its
  rule on the recomposed LaTeX calls
  [`stage_latex`](crate::stdcore::features::xnotes::stage_latex) once per xnote, with the
  xnote's labels, and each xnote is written as a command of its own;
- its [`InCopy`](crate::model::aspects::InCopy) aspect returns
  [`copied_as`](crate::stdcore::features::xnotes::copied_as) with the kinds of its
  xnotes: in content shown a second time, such as a heading's title that a reference
  shows, the node is written as the marks of all its xnotes, side by side, in order.

Each xnote is numbered, listed in its kind's list with its own content, and linked to and
from its own mark, as if each had been written on a node of its own.

```rust
use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    NodeRegion,
};
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::lang::copy::ValueContent;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::aspects::{
    ApartFromText, BlockRole, BlockRoleAspect, ContentRef, CopiedAs, InCopy, InlineSpacing,
    RefPoint, Referenceable,
};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::backends::pres;
use flm_core::stdcore::features::numbering::COUNTERS;
use flm_core::stdcore::features::refs::ANCHORS;
use flm_core::stdcore::features::xnotes::{self, XnoteKind, XnoteOf, XnoteSource, XNOTES};
use flm_core::stdcore::{package, StdcoreConfig};
use flm_core::techy::core::node::NodeRef;

/// `\twonotes{first}{second}`: two footnotes on one node, whose contents are
/// the two arguments.
struct TwoNotes(Arc<XnoteKind>);

const ARGS: [&str; 2] = ["first", "second"];

impl XnoteSource for TwoNotes {
    fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        ARGS.iter()
            .filter_map(|arg| NodeRegion::argument_named(node, arg))
            .map(|content| XnoteOf { kind: self.0.name.clone(), content })
            .collect()
    }
}

impl Referenceable for TwoNotes {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        // One call per xnote, in the order the `XnoteSource` aspect returns
        // them; no labels.
        ARGS.iter().flat_map(|_| xnotes::ref_points(&self.0, &[], None)).collect()
    }
}

impl ApartFromText for TwoNotes {
    fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        xnotes::regions_apart_from_text(node)
    }
}

impl InCopy for TwoNotes {
    fn in_copy(&self, node: &NodeRef<'_, Flm>, purpose: ValueContent) -> CopiedAs {
        xnotes::copied_as(node, purpose, vec![Arc::clone(&self.0)])
    }
}

/// The rule: the two marks, side by side.
struct TwoMarks(Arc<XnoteKind>);

impl Lower for TwoMarks {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut marks = xnotes::stage_mark(cx, lw, &node, &self.0, 0)?;
        marks.extend(xnotes::stage_mark(cx, lw, &node, &self.0, 1)?);
        Ok(Lowering::Emit(marks))
    }
}

struct TwoNotesFeature;

impl FlmFeature for TwoNotesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("example-two-notes", "Two notes")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // The kind the `xnotes` feature configures by default.
        let kind = Arc::new(XnoteKind::footnote());
        let value = Arc::new(TwoNotes(Arc::clone(&kind)));
        b.element(ElementDef::new(
            b.feature().clone(),
            ["twonotes"],
            SyntaxDef::Macro { args: ARGS.iter().map(|arg| ArgDef::mandatory(*arg)).collect() },
            Arc::new(
                ElementSpec::new("example.two-notes")
                    .with::<dyn BlockRoleAspect>(Arc::new(
                        flm_core::model::aspects::FixedBlockRole(BlockRole::Inline(InlineSpacing {
                            absorb_before: true,
                            absorb_after: false,
                        })),
                    ))
                    .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                    .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                    .with::<dyn ApartFromText>(Arc::clone(&value) as Arc<dyn ApartFromText>)
                    .with::<dyn InCopy>(value as Arc<dyn InCopy>),
            ),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "example.two-notes.lower",
            "example.two-notes",
            LoweringDecl::new()
                .reads(XNOTES)
                .reads_opt(COUNTERS)
                .reads_opt(ANCHORS)
                .produces(pres::SPAN)
                .produces(pres::LINK),
            Arc::new(TwoMarks(kind)),
            DefaultMark::Everywhere,
        ))
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(TwoNotesFeature));
let registry = builder.build(&RegistryInputs::new())?;

# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{FlmOrigin, ParseOptions, parse_fragment};
# use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
# use flm_core::techy::source::Source;
let source = Arc::new(
    Source::new("A claim.\\twonotes{First.}{Second.} More.\\footnote{Third.}")
        .with_origin(FlmOrigin::none()),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none())?;
run.execute(&RunOptions::new())?;
assert_eq!(
    run.table(TEXT_OUTPUT)?.expect("written").text,
    "A claim.\u{b9}\u{b2} More.\u{b3}\n\nFootnotes\n---------\n\n\
     \u{b9} First.\n\n\u{b2} Second.\n\n\u{b3} Third.\n",
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Building a kind's list your own way

The list of an xnote kind is what its piece table holds
([`piece_table`](crate::stdcore::features::xnotes::piece_table),
`flm.xnotes.list.<kind>`), and the kind's standard builder is one stage that writes it.
The list is a piece ([`Piece`](crate::core::piece::Piece)), built once by its builder
stage into that table and rendered by each backend with its own rules.

To build one kind's list differently (a list with only the labeled footnotes, a list of
the lists feature's elements, a sentence), a feature registers a stage of `Document`
scope that writes that table from what it declares that it reads (the table of xnotes
[`flm.xnotes`](crate::stdcore::features::xnotes::XNOTES), the frozen trees, or more), and
a request prefers that stage:
[`Overrides::prefer`](crate::core::plan::Overrides::prefer), or
[`WorkflowDef::prefer`](crate::core::registry::WorkflowDef::prefer) in a workflow of the
feature's own. Without a preference, the plan refuses to choose between the two stages
([`PlanError::Contested`](crate::core::plan::PlanError::Contested), which lists both): a
replacement is used only when it is asked for. The lists of the other kinds are not
affected. The replacement's piece may hold any elements that the plan's rules lower; the
kind's list element and entry element keep their rules, which find each entry's xnote
among its kind's records by the node's address and the node region its content pointer
designates.

To show the xnotes' content, a replacement points at it, as the standard builder does,
and never copies it: a copy is a synthesized value, in which a labeled point is refused
and a point has no number and no anchor. Each entry of the kind's list element
([`ENTRY`](crate::stdcore::features::xnotes::ENTRY)) holds one node of the content
pointer element [`CONTENT_POINTER`](crate::stdcore::features::xnotes::CONTENT_POINTER),
whose payload [`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer)
designates the xnote's node (the address of its record's point in
[`flm.xnotes`](crate::stdcore::features::xnotes::XNOTES)) and the region its
[`XnoteSource`](crate::stdcore::features::xnotes::XnoteSource) aspect gives. A
[`SynthesizedPiece`](crate::lang::copy::SynthesizedPiece) carrying that payload, built
into the value by
[`copy_pieces_into_value`](crate::lang::copy::copy_pieces_into_value), is such a node.
The list rule lowers the region where the entry is placed, through
[`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region), as the
document's own content, and gives the entry the xnote's mark as its label and the xnote's
identifier. **Point at each xnote once**: the region is lowered wherever a pointer
designates it, and nothing checks that it is lowered only once. An entry that holds no
pointer has no mark and no identifier.

```rust
use std::sync::Arc;

use flm_core::core::data::store::StageOut;
use flm_core::core::piece::Piece;
use flm_core::core::plan::{Overrides, Plan, PlanCx, PlanError};
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::StageCx;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::FragmentLayout;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::xnotes::{self, XNOTES};
use flm_core::stdcore::{package, StdcoreConfig};

/// Writes the footnotes' list as one sentence counting them.
struct CountFootnotes;

impl Stage for CountFootnotes {
    fn decl(&self) -> StageDecl {
        StageDecl::new("mypkg.xnotes.build", Effect::Compute, Scope::Document)
            .writes(xnotes::piece_table("footnote"))
            .docs("the footnotes' list, as a count")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(XNOTES)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let count = match cx.get(XNOTES) {
            Ok(Some(table)) => table.of_kind("footnote").len(),
            _ => return,
        };
        let source = Arc::new(
            FlmSource::new(format!("{count} footnotes.")).with_origin(FlmOrigin::none()),
        );
        let options = ParseOptions::default();
        let Ok(value) = parse_fragment(cx.registry().lang(), source, FragmentLayout::default(), &options)
        else { return };
        if let Ok(table) = out.table(&xnotes::piece_table("footnote")) {
            *table = Piece::new(Arc::clone(value.tree()));
        }
    }
}

struct MyNotes;

impl FlmFeature for MyNotes {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("mypkg", "My notes")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.stage(Arc::new(CountFootnotes))
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(MyNotes));
let registry = Arc::new(builder.build(&RegistryInputs::new())?);

// Asked for: the override builds the list, the standard builder is not
// scheduled.
let prefer = Overrides { prefer: vec!["mypkg.xnotes.build".into()], ..Overrides::none() };
let plan = Plan::for_workflow(&registry, text::WORKFLOW, &prefer).expect("a plan");
assert!(plan.stage_names().iter().any(|stage| stage.as_str() == "mypkg.xnotes.build"));
assert!(!plan.stage_names().iter().any(|stage| stage.as_str() == "flm.xnotes.build-list.footnote"));

// Not asked for: the plan refuses to choose between the two stages that write the list.
let contested = Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none());
assert!(matches!(contested, Err(PlanError::Contested { .. })));
# Ok::<(), Box<dyn std::error::Error>>(())
```

To change how the list *looks* rather than what it holds, register another lowering rule
for the kind's list element `flm.xnotes.<kind>.list`, marked
[`DefaultMark::Never`](crate::core::lowering::DefaultMark::Never), and select it in the
backend's rule selection, as for any element; the two kinds of replacement can be
combined. To leave a kind's list out of one render, exclude its piece through
[`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions).

For the HTML backend, `contribute_kind` registers, per kind, the rule of the kind's list
element on that backend, which writes the list with a link back from each entry to its
mark. For a listed kind, the contributing feature's own element needs no rule of its own
on HTML: its general rule's mark serves the page too. For an in-place kind, a rule of its
own marked
[`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends)`(["html"])`
calls `stage_in_place` with `Brackets::Drawn` and requires the stylesheet
[`flm.xnotes.style`](crate::stdcore::features::xnotes::STYLE), which draws the brackets
on the page, as the `xnotes` feature's own rule does.

On the recomposed LaTeX, the contributing feature's rule for that backend calls
[`stage_latex`](crate::stdcore::features::xnotes::stage_latex), given the node, the kind,
the xnote's position and its labels. The function writes
`\flmXN<kind>[<labels>]{<content>}`, a LaTeX hook (a macro the recomposed LaTeX writes
and the style file defines with a plain default, which a user redefines in the document's
preamble; [`preamble`](crate::stdcore::backends::latex::preamble)), and requires the
kind's definition in the style file, which `contribute_kind` contributes. The rule
declares that it produces the LaTeX family's macro and raw construct elements, that it
reads the table of safe labels optionally, and that it reads the backend's render
options. `contribute_kind` also registers, per kind, the rule of the kind's list element
on that backend, which writes nothing, and makes peer declarations of the backend, the
two construct elements and the table of safe labels on behalf of the contributing
feature. That feature, including its own rule that calls `stage_latex`, therefore builds
in a registry without the LaTeX features, with its LaTeX rules set aside, without
declaring anything for the LaTeX backend itself.

### A cell style of your own

A cell of a table of cells carries style tokens, as in `\cell<total>{…}`, and every
token refers by name to a cell style
([`CellStyle`](crate::stdcore::features::cells::CellStyle)), a declaration contributed to
the `cells` feature's service slot `flm.cells.styles`
([`STYLES_SLOT`](crate::stdcore::features::cells::STYLES_SLOT)). The `cells` feature
contributes its own cell styles (`align{…}`, `head{…}`, `rule{…}`; their short forms
`l`, `c`, `r`, `H`, `rH`, `lvert`, `rvert`; and the backgrounds `green`, `blue`,
`yellow`, `red`), and any feature may contribute more. A token that no cell style
declares is an error in the document, which catches a misspelled style, and also a
document that uses a host's style in a registry without the host's feature.

A cell style says what its token states about the cell, as one of four **effects**
([`Effect`](crate::stdcore::features::cells::Effect)), each a fact that holds regardless
of the output format:

- the cell's alignment;
- that the cell heads its column or its row;
- a rule (a line) at one of the cell's edges;
- a **custom style**, which the cell carries by its name.

A cell style of yours is usually a custom style; it may also state one of the other
three effects, as a short form of a shipped token does. Each backend writes a custom
style in a form of its own: the HTML backend writes the attribute
`data-flm-style-<name>` on the cell's element, the recomposed LaTeX writes the table
command `\flmCellsSet{<name>}` at the start of the cell's text, and plain text shows
nothing. Your definitions for each backend give the name its meaning (a stylesheet rule
that selects the attribute; the keys that a LaTeX definition piece registers under the
name), and the cell style's requirements, per backend
([`CellStyle::requires`](crate::stdcore::features::cells::CellStyle::requires)), make the
page or the style file hold those definitions exactly where a cell uses the style:

```rust
use std::sync::Arc;
use flm_core::core::registry::{
    BuildError, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder,
    RegistryInputs, StyleDef,
};
use flm_core::model::names::Name;
use flm_core::stdcore::backends::latex::{LatexPreambleDef, preamble};
use flm_core::stdcore::backends::{html, latex_recomposed};
use flm_core::stdcore::features::cells::{CellStyle, LATEX_PIECE, STYLES_SLOT};
use flm_core::stdcore::{self, StdcoreConfig};

struct HostStyles;

impl FlmFeature for HostStyles {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("host", "The host's cell styles")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // The host's definitions: a style definition for HTML, and a
        // definition piece for the recomposed LaTeX registering each style's
        // `tabularray` cell keys, written after the `cells` feature's piece,
        // which defines `\flmCellsStyleKeys`.
        b.style_def(StyleDef {
            id: Name::from("host.cells.style"),
            // The host's stylesheet rules, which select `.flm-pres-cells
            // [data-flm-style-total]` and the attributes of the other styles.
            content: vec![DefContent::Inline(String::from("/* host.cells.style */\n"))],
            docs: String::from("the host's cell styles"),
        })?;
        let piece = LatexPreambleDef::new(
            "host-cells",
            "\\flmCellsStyleKeys{total}{cmd=\\bfseries}\n\\flmCellsStyleKeys{pending}{bg=lightgray}\n",
            "the `tabularray` keys of the host's cell styles",
        )
        .requires([LATEX_PIECE]);
        // A peer contribution, as the styles below: the feature builds
        // without the LaTeX backend too.
        b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece))?;
        let host_piece = preamble::piece_id("host-cells");
        let styles = [
            // Given its meaning by the host's style definition on HTML and by
            // the keys the host's definition piece registers for `total` on the
            // recomposed LaTeX.
            CellStyle::custom("total", "a cell that holds a total")
                .requires(html::BACKEND, ["host.cells.style"])
                .requires(latex_recomposed::BACKEND, [host_piece.clone()]),
            // The same two requirements, for the keys registered for `pending`.
            CellStyle::custom("pending", "a value not yet confirmed")
                .requires(html::BACKEND, ["host.cells.style"])
                .requires(latex_recomposed::BACKEND, [host_piece]),
            // A style a stylesheet alone reads: nothing to require elsewhere.
            CellStyle::custom("warn", "a cell to notice").requires(html::BACKEND, ["host.cells.style"]),
        ];
        for style in styles {
            // A peer contribution: the feature builds without `cells` too.
            b.peer_slot::<CellStyle>(STYLES_SLOT, Arc::new(style))?;
        }
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.feature(Arc::new(HostStyles));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

The identifiers a cell style requires designate definitions that your feature registers
for that backend: the style definition `host.cells.style` (see [Style and script
definitions](#style-and-script-definitions)) and the definition piece
`flm.latex.def.host-cells`. A definition piece is a unit of LaTeX definitions that a
feature registers under an identifier `flm.latex.def.<name>`, together with the
identifiers it needs in turn
([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)). On the
recomposed LaTeX, a cell carrying `total` and `pending` begins
`\flmCellsSet{total}\flmCellsSet{pending}`, and each table command applies the keys
registered under its name; of two styles that set `cmd=`, the later one applies.

On the rendered LaTeX, a cell's styles are written by name in the key `styles` of its
spec, and the keys of a style are registered with `\flmxDefineCellStyle{<name>}{<tblr
cell keys>}` (`\flmxDefineCellStyle{total}{font=\bfseries}`), in a definition piece of
yours that requires the core of the contract
([`CORE_CHUNK`](crate::stdcore::backends::latex_rendered::contract::CORE_CHUNK)); the cell
style requires that piece for the backend `latex-rendered`
(`.requires(latex_rendered::BACKEND, [piece_id])`), as above for the recomposed LaTeX
([Definition pieces for the rendered LaTeX](#definition-pieces-for-the-rendered-latex)).

A cell style carries no code, so it cannot change a cell's content or the table's model
(an alignment on a decimal point, for instance, cannot be expressed). It cannot redeclare
a shipped token either: that is a registry build error. To give `green` another color,
set the custom property `--flm-cells-green` (defined by the `cells` feature's style
definition) in a stylesheet of your own on HTML, or, in LaTeX, define `\flmCellsKeysgreen` (the keys registered for `green`) or the color
`flmCellsColorgreen` before the style file is loaded. A token takes a value in braces only
where its cell style is of the structural form, as the `cells` feature's own three
(`align`, `head`, `rule`) are; a value on a custom style is an error.

## An extension crate, end to end

The crate `flm-phfpack` of this repository is a crate of extensions written as a third
party would write it: it depends on no crate of this repository other than `flm-core`
(besides `serde`, `schemars` and `serde_json`, which declaring settings asks of any
crate), reaches the parser toolkit techy
only through the library's re-export (`flm_core::techy`), and holds one package,
`phfpack`, with one feature, `phfqit`. The feature declares notation macros for quantum
information, whose delimiters are drawn at the size a use asks for, and quantity macros
such as entropies. The crate shows the whole path from declaration to program:

- **The macros are data.** Each macro is one row of a table: its name, its arguments, its
  template. A macro that a document declares in the feature's settings becomes the same
  kind of value as a row. Each becomes a
  [`Definition`](crate::lang::rewrites::userdefs::Definition) whose arguments are read by
  the library's readers of an argument's syntax (a row's codes by
  [`arg_def_from_code`](crate::lang::mint::arg_def_from_code); the arguments of a macro
  declared in the settings by [`ArgData`](crate::lang::mint::ArgData), each a structured
  syntax or a code) and whose templates may use the feature's computed placeholders,
  declared on the definition with
  [`Definition::extra_placeholder`](crate::lang::rewrites::userdefs::Definition::extra_placeholder)
  (see [Templates with values the feature
  computes](#templates-with-values-the-feature-computes)).
- **One argument parser of its own.** A size written as one token (a size command or a
  star) after a backtick is read by an
  [`FlmArgumentParser`](crate::lang::mint::FlmArgumentParser) of the crate, since no
  standard argument kind reads a command in that position. Every other argument is of a
  standard kind.
- **Elements and rules.** Every macro registers the element declaration that
  [`UserDefinition::element_def_with`](crate::lang::rewrites::userdefs::UserDefinition::element_def_with)
  builds, with the feature's own [`Expand`](crate::lang::rewrites::userdefs::Expand)
  aspect in place of the definition's, under a name that
  [`element_name_under`](crate::stdcore::features::userdefs::element_name_under) gives
  with the feature's prefix. It registers the lowering rule that
  [`expand_in_tree_rule_for`](crate::stdcore::features::userdefs::expand_in_tree_rule_for)
  builds for it, which requires
  [`ALWAYS_EXPANDED`](crate::stdcore::features::userdefs::ALWAYS_EXPANDED): the
  `userdefs` feature's rewrite stage replaces the macros in the tree before any output is
  written, in every workflow. The `userdefs` feature is therefore necessary, since a
  registry without it does not declare what the rule requires. A macro is available in
  the parsing modes where its text parses, as a user definition declared by one text is,
  and a declared macro's `content` is read by
  [`DefinitionContent::definitions`](crate::stdcore::features::userdefs::DefinitionContent::definitions).
- **Templates compiled once.** A [`LateInit`](crate::core::registry::LateInit) hook
  compiles every definition's templates, then each value that a computed placeholder
  stands for (a delimiter at each size, a named value) into a small tree. Each value is
  kept in the parsing modes where its text parses, through the library's
  [`compile_in_modes`](crate::lang::rewrites::userdefs::compile_in_modes) (a delimiter
  with a size command parses in formulas only). The hook's placeholder bindings return
  that tree at each use, and pass every other reference to
  [`DefinitionBindings`](crate::lang::rewrites::userdefs::DefinitionBindings).
- **Settings.** The feature declares its settings through
  [`FlmFeature::settings`](crate::core::registry::FlmFeature::settings): a data type that
  is read and converted to the feature's configuration, with the keys that hold named
  things merged entry by entry. It passes its configuration to the registry digest with
  [`digest_input`](crate::core::registry::FeatureBuilder::digest_input).
- **The program.** A program carries the crate's
  [`PackageFactory`](crate::core::registry::PackageFactory): the `flm` program passes it
  to its library entry point behind a cargo feature, and a program of your own passes its
  own list of factories in the same way (see [building a program that carries other
  packages](crate::guide::command_line#building-a-program-that-carries-other-packages)).

## See also

- [Stages, capabilities, and plans](crate::guide::pipeline): how the plan that runs your
  stage is derived.
- [Rendering: backends and outputs](crate::guide::rendering): lowering rules, the
  presentation vocabulary, and the standard backends.
- [The document model](crate::guide::document_model): elements, nodes and the frozen
  trees your stage reads.
- [Numbering](crate::guide::numbering): counters, counter styles and the numbering
  feature, in full.
- [Embedding flm-core](crate::guide::embedding): building a registry and running it from
  your own program.
- [Features and packages](crate::guide::concepts_overview#features-and-packages): the
  concept entry, with the API items in one list.
- [The AI guide on extending FLM](crate::guide::ai_guide_extending): the same material in
  a form written for an AI agent, with further recipes.
