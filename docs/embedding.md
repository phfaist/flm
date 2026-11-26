# Embedding flm-core

This chapter shows how a Rust program uses the library `flm-core`: it turns FLM
source text into a document, asks for a plan, runs the plan, and reads the results.
It assumes that you know Rust and have seen LaTeX, and nothing about this library.

A program uses the library in six steps, always in this order:

1. Build a **registry**. A registry is the frozen record of everything the
   language and the pipeline consult for a set of features
   ([`core::registry`](crate::core::registry)): the commands an author can write,
   the stages, the tables, the lowering rules, the backends and the workflows that
   the registered features declared.
2. **Parse** each source text into a **fragment**. A fragment is one piece of
   source as the parser delivered it: its tree, its source and its parse
   diagnostics ([`Fragment`](crate::model::doc::Fragment)).
3. Place the fragments in a **document** ([`Doc`](crate::model::doc::Doc)).
4. Build a **plan**. A plan is the list of stages that satisfies a plan request
   (what the caller asks for; [`PlanRequest`](crate::core::plan::PlanRequest)),
   in dependency order and grouped in batches ([`Plan`](crate::core::plan::Plan));
   the library derives it from the registry and from what you ask for. A stage is
   one unit of work of the pipeline ([`Stage`](crate::core::stage::Stage)).
5. **Run** the plan over the document. A run is one execution of one plan over
   one document ([`Run`](crate::core::run::Run)).
6. Read the results out of the run: the computed **tables**, the output's
   artifacts and the diagnostics. A table is typed data that one stage writes and
   other stages read ([`Table`](crate::core::data::table::Table)).

Steps 1 and 4 depend only on the registry and on what you ask for, never on a
document. A program therefore builds the registry and the plan once and uses them
for every document it processes.

## What the library does, and what it leaves to you

`flm-core` performs no input or output. It never opens a file, never connects to
the network and never starts a process. Your program reads the source text and
hands it to the library as an [`FlmSource`](crate::lang::FlmSource); it takes the
output's bytes out of the run and writes them where it wants them. The only part
of the library that reads or writes files is the golden-file helper
[`crate::dump::golden`], which tests use to compare output with a file; no stage
of the processing touches the file system.

This documentation calls a program that embeds the library a *host*. The library
gives a host two ways to supply what only the host can do:

- A feature that needs behavior from outside the library declares a **service
  slot**, and the host contributes a value to it when it builds the registry. A
  service is a value that can be shared across threads which one feature supplies
  for another feature, or for the embedding program, to use
  ([`crate::core::services`]); see [Services the host
  provides](#services-the-host-provides).
- A document that brings in another file with `\input` asks the host to read that
  file through an **input resolver**, an object that implements
  [`InputResolver`](crate::lang::InputResolver); see [Documents of several
  files](#documents-of-several-files). Without an input resolver, every document
  is still processed, and each `\input` is reported as an error and includes
  nothing.

The library is built on techy, a toolkit for parsing LaTeX-like languages, and
re-exports it as [`flm_core::techy`](crate::techy). Use techy's types (sources,
node trees, diagnostics, serialization sessions) through that re-export rather
than through a `techy` dependency of your own: when one build holds two versions
of techy, their types are unrelated, and a value of one cannot be passed to a
function of the other.

## The shortest complete program

The following program takes every step (registry, parse, document, plan, run,
read) for one document rendered as plain text:

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::stdlib::backends::text::TEXT_DOCUMENT;
use flm_core::stdlib::{self, StdlibConfig};

// 1. The registry: a package of the standard features, and a math processor.
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// 2. The parse. The source carries its origin, `hello.flm`, which diagnostics
//    report and which identifies the source wherever something about it is recorded.
let source = Arc::new(
    FlmSource::new("Hello \\textbf{world}.").with_origin(FlmOrigin::new("hello.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

// 3. The document: one placed fragment.
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// 4. The plan of the `text` workflow, with nothing changed about it.
let plan = Arc::new(Plan::for_workflow(&registry, "text", &Overrides::none())?);

// 5. The run.
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// 6. The result: the output's artifact set, and the bytes of its entry artifact.
let artifacts = run.table(TEXT_DOCUMENT)?.expect("the text workflow writes it");
let bytes = artifacts.entry().and_then(|a| a.as_bytes()).expect("one entry artifact");
assert_eq!(String::from_utf8_lossy(bytes).trim_end(), "Hello world.");
# Ok::<(), Box<dyn std::error::Error>>(())
```

The sections that follow explain each step.

## Building a registry

A registry is the frozen record of everything the language and the pipeline
consult for a set of features: the elements, stages, tables, capabilities,
lowering rules, backends, outputs, workflows, option types and services the
features declared, built once and shared by every document, plan and run
([`core::registry`](crate::core::registry)). The finished registry is an
[`Arc<FlmRegistry>`](crate::core::registry::FlmRegistry), and it never changes
after it is built.

A registry is built from **features**. A feature is a Rust value that contributes
a related bundle of things to a registry (elements, stages, tables, lowering
rules, backends, services, settings) through the feature builder
([`FlmFeature`](crate::core::registry::FlmFeature)). A **package** is a named
bundle of features together with the packages it builds on
([`PackageEntry`](crate::core::registry::PackageEntry)). A
[`RegistryBuilder`](crate::core::registry::RegistryBuilder) collects both:
[`use_package`](crate::core::registry::RegistryBuilder::use_package) adds the
features of a package, and
[`feature`](crate::core::registry::RegistryBuilder::feature) adds one feature.
[`build`](crate::core::registry::RegistryBuilder::build) then checks everything
and returns the registry.

An **element** is the value that says what a node of a document tree means
([`ElementSpec`](crate::model::element::ElementSpec)). A **backend** is the
registration bundle for one output format
([`BackendDef`](crate::core::registry::BackendDef)): it writes the output from a
presentation tree that **lowering** builds, by applying to each element the
lowering rule selected for it
([`LoweringRule`](crate::core::lowering::LoweringRule); [backends, lowering and
outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).

`flm-core` ships the *standard features* in its module [`stdlib`](crate::stdlib). A
program declares a package entry, under a name of its choice, and adds the standard
features to it with [`stdlib::add_features`](crate::stdlib::add_features). The standard
features provide the commands and environments an author writes (inline formatting,
headings, mathematics, references, lists, footnotes, theorems, images, citations,
included files, user definitions and more), the stages that structure and check a
document, four output formats (plain text, HTML, rendered LaTeX and recomposed LaTeX)
and the `check` workflow. The [`stdlib`](crate::stdlib) module documentation lists the
thirty-three features. They read where a formula begins and ends, but process no
mathematics: that is the work of a *math processor*, a package whose feature decides
which commands a formula may hold, checks the formulas and writes them into the outputs.
The registry of a program that renders documents adds one, in a few lines. `flm-core`
ships the math processor [`math-passthrough`](crate::math_passthrough)
([`math_passthrough::package`](crate::math_passthrough::package)), which needs no
JavaScript and nothing outside `flm-core`: it carries each formula's source to the
output, for LaTeX or for a formula renderer in the reader's browser. Another processor,
such as the extension `math-mathjax`, which runs MathJax, takes its place in the same
way. With no math processor, a document without formulas renders as any other, and a
render reports each formula as the Error `flm.math.unprocessed` and writes its content
as ordinary text; two processors in one registry are an error.

```rust
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
let registry = builder.build(&RegistryInputs::new())?;

// The registry can be queried by name.
assert!(registry.workflow("text").is_some());
assert!(registry.workflow(stdlib::CHECK_WORKFLOW).is_some());
assert!(registry.table_named("flm.labels").is_some());
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

Two values decide what the registry becomes:

- [`StdlibConfig`](crate::stdlib::StdlibConfig) holds the configuration of
  each of the sixteen standard features that take one; the other seventeen take
  none. A feature's configuration is the plain Rust value the feature is
  constructed from; it is fixed when the registry is built and cannot vary per
  document or per render ([settings, options and
  configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
  The configurations decide which commands the language has, how a document's
  content is structured and which things are numbered, so they must be known
  before anything is parsed. [Configuring the standard
  features](#configuring-the-standard-features) below describes them.
- [`RegistryInputs`](crate::core::registry::RegistryInputs) holds what the
  library itself takes:
  - the parsing configuration ([`FlmParsing`](crate::lang::FlmParsing)): whether
    comments are recognized and which character starts one, and whether `$`
    delimits mathematics;
  - the structural elements, the elements of the node shapes that no feature
    declares, such as text and groups
    ([`StructuralElements`](crate::lang::structural::StructuralElements));
  - the policy of the recursion guard
    ([`GuardPolicy`](crate::lang::guard::GuardPolicy); see [The worker-thread
    requirement](#the-worker-thread-requirement));
  - the number of diagnostics one parse keeps;
  - the input resolver that every parse under this registry uses (see
    [Documents of several files](#documents-of-several-files));
  - `build_identity`, a string that identifies your program, such as its
    version.

Each standard feature's configuration enters the **registry digest**
([`crate::model::digest`]), a fingerprint of everything about the registry that
can change what processing produces; a host that caches results stores them
under it. Two registries built from different configurations therefore never
share a cached result. Of the inputs above, the parsing configuration and the
structural elements enter the digest too, and so does `build_identity`, so that
a cache written by one program is never read by another; the guard policy, the
number of diagnostics a parse keeps and the input resolver do not. A feature of your own whose declarations depend on its configuration
adds that configuration to the digest with
[`FeatureBuilder::digest_input`](crate::core::registry::FeatureBuilder::digest_input).

Besides features, a host can give the registry builder two things of its own:
values for service slots
([`contribute`](crate::core::registry::RegistryBuilder::contribute) and
[`contribute_to`](crate::core::registry::RegistryBuilder::contribute_to); see
[Services the host provides](#services-the-host-provides)), and the fields of a
workflow, which replace those a feature registered
([`workflow_fields`](crate::core::registry::RegistryBuilder::workflow_fields)).

### Configuring the standard features

Each configurable standard feature is constructed from one field
of `StdlibConfig`, which the builder method of the same name sets, for example
[`StdlibConfig::theorems`](crate::stdlib::StdlibConfig::theorems). This section
describes the configurations a host most often changes; each feature's module
documentation describes its configuration in full. The registry build checks
every configuration, and a configuration that is refused makes the build fail
with a message that identifies the faulty part (see [When a registry build
fails](#when-a-registry-build-fails)).

**Heading levels.** A heading's **level** is a number counted from zero, smaller
for an outer heading. The standard heading kinds (a heading kind is one declared
heading, such as `section`, with the command that writes it;
[`HeadingsConfig`](crate::stdlib::features::headings::HeadingsConfig)) have the
levels 1 (`\section`) to 5 (`\subparagraph`), and a host may declare a kind at
level 0 above them, such as a chapter. By default no heading kind is a run-in
heading. A run-in heading is a heading set at the start of the paragraph it
introduces, on the paragraph's first line, rather than on a line of its own
([`headings`](crate::stdlib::features::headings)). Three settings take a level,
and the registry build accepts any value for each of them:

- the level of a heading kind
  ([`HeadingDecl::level`](crate::stdlib::features::headings::HeadingDecl::level));
- the level of the title above a list of xnotes
  ([`XnoteKind::title_level`](crate::stdlib::features::xnotes::XnoteKind::title_level),
  2 by default);
- the level of the title above the list of cited works
  ([`CitationsConfig::heading_level`](crate::stdlib::features::citations::CitationsConfig::heading_level),
  2 by default).

The level that a render writes as its first-level heading is a render option (a
value set per render request, that is, in the plan request's `render_options`;
see [Options](#options)), `root_heading_level`, of the HTML backend
([`HtmlRenderOptions::root_heading_level`](crate::stdlib::backends::html::HtmlRenderOptions::root_heading_level))
and of the text backend
([`TextRenderOptions::root_heading_level`](crate::stdlib::backends::text::TextRenderOptions::root_heading_level)).
It is 1 by default, so a section is a first-level heading. A heading whose level
is less than the root heading level is reported as an Error,
`flm.pres.heading-above-root`, and is written as a first-level heading.

**Xnotes.** An xnote is a piece of text set apart from the running text, marked
where it belongs and shown elsewhere or in place (a footnote, an endnote, a side
note); a kind of xnote is one way of being an xnote, declared by the feature's
configuration ([`xnotes`](crate::stdlib::features::xnotes)). The xnote kinds are
configured by [`XnotesConfig`](crate::stdlib::features::xnotes::XnotesConfig),
set with [`StdlibConfig::xnotes`](crate::stdlib::StdlibConfig::xnotes). For
each kind, the configuration gives its command, its words, its label type,
whether it is numbered, its xnote mark, whether its xnotes are listed after the
content or shown in place, and how its content is laid out. By default there is
one kind, `footnote`, written `\footnote`. The configuration is fixed when the
registry is built because each kind's command is part of the language, each
numbered kind declares a counter and two counter styles, and whether an in-place
xnote is a block is decided before the document's trees are frozen.

**Defined terms.** A term definition (the `defterm` environment) opens with a
**lead-in**: a text shown before the words of its first paragraph, usually the
term being defined, as in `Entropy: A measure of disorder.` The
[`defterm`](crate::stdlib::features::defterm) feature writes it from the lead-in
template
[`DeftermConfig::lead_in`](crate::stdlib::features::defterm::DeftermConfig::lead_in),
written in the [template
language](crate::guide::concepts_overview#the-template-language-shared-by-features)
(described for authors under [the template
language](crate::guide::language_syntax#the-template-language)). The template has
one placeholder: `#{term}` is the term as the term definition writes it, markup
included. The default is `#{term}: `; `None` sets no lead-in, and a term
definition is then its body alone. A program sets the lead-in with
[`StdlibConfig::defterm`](crate::stdlib::StdlibConfig::defterm):

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::defterm::DeftermConfig;

let lead_in = r"\textbf{Definition} (\emph{#{term}}): ".to_string();
let config = StdlibConfig::new().defterm(DeftermConfig::new().lead_in(Some(lead_in)));
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), config));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

The template is compiled when the registry is built. A template that does not
compile, that uses `#{term}` more than once or that is blank makes the build
fail.

**Theorems.** The theorem environments (`theorem`, `lemma`, `proof` and the
others) belong to the [`theorems`](crate::stdlib::features::theorems) feature,
configured by
[`TheoremsConfig`](crate::stdlib::features::theorems::TheoremsConfig). The
configuration declares **theorem kinds** and **theorem classes**:

- A theorem kind is one environment an author writes, under the kind's name. It
  declares its class, its words (the word written before its number, such as
  `Lemma`, its plural, and named variants such as `abbreviated`, which a
  reference selects by name: `\ref[abbreviated]{lem:x}`), and whether each
  statement of the kind expects a proof.
- A theorem class
  ([`TheoremClass`](crate::stdlib::features::theorems::TheoremClass)) is what a
  family of theorem kinds shares: whether they are numbered, and if so whether in
  the shared counter `flm.theorems.theorem` or each in a counter of its own,
  `flm.theorems.counter.<kind>`; the theorem heading template, which joins the
  kind's word and number with the theorem's title; the closing mark written at
  the end of each theorem; and whether a proof's title may refer back to the
  statement it proves (`[*lem:x]`).

A proof refers to the statement it proves through one of the statement's labels.
A kind that expects a proof must therefore belong to a numbered class, whose
theorems accept labels; otherwise the registry build fails. The default
configuration,
[`TheoremsConfig::new`](crate::stdlib::features::theorems::TheoremsConfig::new),
holds eight kinds in the classes `theoremlike`, `definitionlike` and `prooflike`;
[`simpleset`](crate::stdlib::features::theorems::TheoremsConfig::simpleset)
holds six kinds and
[`richset`](crate::stdlib::features::theorems::TheoremsConfig::richset)
thirteen. A kind of your own needs no code, only its declaration:

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::numbering::{PluralWords, PrefixWords};
use flm_core::stdlib::features::theorems::{TheoremClass, TheoremKind, TheoremsConfig};

let exercise = TheoremKind::new(
    "exerciselike",
    PrefixWords::new("Exercise", "Exercises")
        .with_variant("short", PluralWords::new("Ex.", "Exs.")),
);
let theorems = TheoremsConfig::new()
    // Numbered in a counter of their own; the title follows the word "on".
    .class(
        "exerciselike",
        TheoremClass::numbered()
            .shared_counter(false)
            .heading(r"#{name}\IfValueT{#{title}}{ on #{title}}"),
    )
    .kind("exercise", exercise);
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new().theorems(theorems)));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

With this configuration, `\begin{exercise}[limits]` opens with the heading
`Exercise 1 on limits`, and `\ref[short]{x:a}` shows `Ex. 1`. In the theorem
heading template, `#{name}` is the kind's word and the theorem's number (the word
alone where the theorem has no number), and `#{title}` is the theorem's title.
`#{title}` is bound only where the theorem has a title, which
`\IfValueT{#{title}}{…}` tests. The template states how the word, the number and
the title are joined, not how they look: a lowering rule or a stylesheet decides
that, selecting the two parts the template produces by their roles
`flm.theorems.name` and `flm.theorems.title` (a role is a dotted name, owned by a
feature, that a presentation node carries to say what the node is;
[`payload`](crate::stdlib::backends::pres::payload)). The templates, the closing marks
and the words are compiled when the registry is built; one that does not compile
makes the build fail, and so does a kind whose class the configuration does not
declare. A kind's number is written with the configuration's number template,
`#{arabic}` by default
([`number_format`](crate::stdlib::features::theorems::TheoremsConfig#structfield.number_format)).
To write one kind's numbers differently, replace that kind's counter style,
`flm.theorems.style.<kind>`, through
[`NumberingConfig`](crate::stdlib::features::numbering::NumberingConfig).

On the recomposed LaTeX output, a proof's back-reference (`[*lem:a]`) is written
as the author wrote it, for the LaTeX package `phfthm`, which reads that syntax.
A host whose documents are compiled where `phfthm` is not installed sets the
render option
[`resolved_proof_refs`](crate::stdlib::backends::latex_recomposed::LatexRecomposedRenderOptions::resolved_proof_refs).
A LaTeX hook is a macro, or an environment, that the recomposed LaTeX writes and
the style file (the `.sty` file written beside the recomposed LaTeX;
[`style`](crate::stdlib::backends::latex::style)) defines with a plain default,
which a user redefines in the document's preamble
([`preamble`](crate::stdlib::backends::latex::preamble)). With
`resolved_proof_refs` set, the back-reference is written inside the LaTeX hook
`\flmProofTitle`, which sets "Proof (theorem 1)" by default, and the recomposed
LaTeX needs only the package `amsthm` ([theorems in the recomposed
LaTeX](crate::guide::rendering#theorems-in-the-recomposed-latex)). Whether or not
`resolved_proof_refs` is set, the theorems of a numbered kind are set in the
LaTeX theorem style named after the kind's class. A class of your own, such as
`exerciselike` above, is set by default with the parameters of the `plain` style
of `amsthm`, and the shipped class `definitionlike` with those of its
`definition` style; a document's preamble may declare a theorem style of the
class's name before the style file is loaded.

**Quotations.** The quotation environments (`quote`, `blockquote`, `address`)
belong to the [`quote`](crate::stdlib::features::quote) feature, configured by
[`QuoteConfig`](crate::stdlib::features::quote::QuoteConfig), which maps each
quote environment's name to a
[`QuoteEnvironment`](crate::stdlib::features::quote::QuoteEnvironment). A quote
environment declares two things:

- the set of quotation-section commands its body accepts, among `\text`,
  `\lines`, `\attributed` and `\block`
  ([`QuoteSection`](crate::stdlib::features::quote::QuoteSection)), which an
  author writes in any order;
- the kind of quotation section that the content outside those commands is read
  as, if any. Where the environment gives none, content outside the commands is
  an error.

By default, `quote` accepts the four commands and reads no content outside them,
`blockquote` reads its whole content as one block section, and `address` reads
its whole content as one lines section. An environment of your own needs only
its declaration:

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::quote::{QuoteConfig, QuoteEnvironment, QuoteSection};

// A passage and its attribution, nothing else.
let epigraph = QuoteEnvironment::new([QuoteSection::Text, QuoteSection::Attributed]);
let quote = QuoteConfig::new().environment("epigraph", epigraph);
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new().quote(quote)));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

`\begin{epigraph}\text{…}\attributed{…}\end{epigraph}` is then a quotation like
the others: its presentation node carries the kind token `epigraph` (a name,
owned by a feature, that classifies the node;
[`payload`](crate::stdlib::backends::pres::payload)), by which a stylesheet can
select it. The body is laid out as paragraphs when the content
outside the quotation-section commands is read as a block section or is an
error, and as one run of inline content when that content is read as a passage
(`\text`), as lines or as an attribution. The configuration refers to no
backend, HTML tag or stylesheet. The registry build fails, identifying the
environment, for an environment that accepts no command and reads no content
outside them; for a name that is empty or holds whitespace, a brace, a backslash
or `%`; for a name that another feature also declares; and for a name that is
one of the four kind tokens a list reserves (`numbered`, `unnumbered`, `terms`,
`labeled`).

**Annotations.** A reviewer's marks (a highlighted passage, a remark written in
the text) belong to the [`annotations`](crate::stdlib::features::annotations)
feature, configured by
[`AnnotationsConfig`](crate::stdlib::features::annotations::AnnotationsConfig):
the annotators, in order, each an
[`Annotator`](crate::stdlib::features::annotations::Annotator) with the name of
the command the annotator writes and, optionally, the initials shown at the start
of the annotator's annotations. An annotator's number is its position in the
list, counted from zero; an output that shows colors chooses one per number. By
default there is no annotator, and the feature then declares no command:

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::annotations::{AnnotationsConfig, Annotator};

let annotations = AnnotationsConfig::new()
    .annotator(Annotator::new("phf").initials("PhF"))
    .annotator(Annotator::new("abc"));
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new().annotations(annotations)));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

An author then writes `\phf{…}` or `\phf … \endphf` to highlight a passage and
`\phf[…]` to write a remark ([language
syntax](crate::guide::language_syntax#annotations)). The registry build fails,
identifying the annotator, for a command that is not made of ASCII letters or is
`begin` or `end`, for a command that two annotators share or that another feature
declares, and for initials that are empty or made only of whitespace. Whether the
annotations are shown is decided per render, not by the configuration
([`AnnotationsRenderOptions`](crate::stdlib::features::annotations::AnnotationsRenderOptions);
see [Options](#options)).

**Images.** An image, `\includegraphics[options]{path}`, belongs to the
[`graphics`](crate::stdlib::features::graphics) feature, configured by
[`GraphicsConfig`](crate::stdlib::features::graphics::GraphicsConfig). Its one
setting, `global`, decides where an author may write the command: by default
only inside the body of a float (a `figure` or a `table`), and with `global` set
to `true` also anywhere text can be written. An image is inline content, except
as a direct child of a float's body, where it is a block ([language
syntax](crate::guide::language_syntax#graphics)). What the library knows about an
image file comes from the services your program provides (see [Services the host
provides](#services-the-host-provides)).

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::graphics::GraphicsConfig;

let config = StdlibConfig::new().graphics(GraphicsConfig::new().global(true));
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), config));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

**Tables of cells.** A table of cells, `\begin{cells}…\end{cells}`, belongs to
the [`cells`](crate::stdlib::features::cells) feature, configured by
[`CellsConfig`](crate::stdlib::features::cells::CellsConfig). Its one setting,
`global`, decides where an author may write the environment. By default the
environment can be written only inside the body of a float whose float type
refers to the feature's scoped package `flm.cells.environment`, as the `table`
float type does
([`FloatType::table`](crate::stdlib::features::floats::FloatType::table)). A
scoped package is a set of element declarations that can be written only inside
the environment bodies and arguments whose declarations refer to the package by
name ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)). With `global`
set to `true`, the environment can also be written wherever blocks can: between
paragraphs, in a list item, in a footnote ([language
syntax](crate::guide::language_syntax#tables-of-cells)). A registry that holds
the `cells` feature without the `floats` feature holds, under the default, an
environment that no document can write; the registry build does not report it.
The cell styles a cell may carry are not a setting: a feature declares them by
contributing to the service slot `flm.cells.styles`
([`STYLES_SLOT`](crate::stdlib::features::cells::STYLES_SLOT)).

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::cells::CellsConfig;

let config = StdlibConfig::new().cells(CellsConfig::new().global(true));
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), config));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

**Floats.** The floats (`figure`, `table`) belong to the
[`floats`](crate::stdlib::features::floats) feature, configured by
[`FloatsConfig`](crate::stdlib::features::floats::FloatsConfig): the float
types, in order, each a
[`FloatType`](crate::stdlib::features::floats::FloatType). A float type is one
environment an author writes. It has a name; a caption name, written before the
number in the line under the float's content (`Figure`); a numbering policy
([`FloatNumbering`](crate::stdlib::features::floats::FloatNumbering): numbered
unless the float carries `\nonumber`, never numbered, or numbered when the float
carries a label); a number template (how the number is written, `#{arabic}` by
default); the words a reference writes before the
number; the content handlers it accepts; the scoped packages available in its
body; and the label types it accepts. A content handler is a value that accepts
a float's content or refuses it, contributed by the feature that owns that kind
of content and listed by name by the float types that accept it
([`FloatContentHandler`](crate::stdlib::features::floats::FloatContentHandler)).
[`FloatType::new`](crate::stdlib::features::floats::FloatType::new) derives the
defaults from the name and takes the names of the content handlers:

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::floats::{FloatNumbering, FloatType, FloatsConfig};

// A diagram holding anything, every one numbered.
let diagram = FloatType::new("diagram", ["any"]).numbering(FloatNumbering::Numbered);
let floats = FloatsConfig::new().float_type(diagram);
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new().floats(floats)));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

`\begin{diagram}…\caption{…}\label{diagram:x}\end{diagram}` is then a float like
the others: the line under its content reads `Diagram 1: …`, and
`\ref{diagram:x}` shows `Diagram 1`. By default, `figure` accepts exactly one
image and `table` exactly one table of cells or one image, through the content
handlers that the `graphics` and `cells` features contribute; the content handler
`any`, which the `floats` feature contributes, accepts any content. A content
handler that a float type lists and that no registered feature contributes
accepts nothing; a run reports it as a Note (`flm.floats.missing-handler`), once
per float type and content handler, at the first float of that type.
The registry build fails, identifying the float type, for a name that is not made
of ASCII letters, a name that is one of the kind tokens `numbered` and
`unnumbered`, a float type declared twice, and a float type that lists no content
handler.

**Citations.** Citations (`\cite[note]{keys}` and `\R\cite{keys}`) belong to the
[`citations`](crate::stdlib::features::citations) feature, configured by
[`CitationsConfig`](crate::stdlib::features::citations::CitationsConfig):

- the **placement** of the citation entries
  ([`CitationPlacement`](crate::stdlib::features::citations::CitationPlacement)):
  listed after the content under a heading, the citation marks in the text
  showing numbers (the default); or in place, each citation mark showing its
  citation entries, with no list;
- the **form** of a citation mark under the listed placement
  ([`MarkForm`](crate::stdlib::features::citations::MarkForm)): bracketed, the
  numbers inline between the delimiters of the citation counter style (the
  default); or superscript, the numbers raised with no delimiters;
- the **citation counter style**
  ([citations](crate::stdlib::features::citations#the-configuration)), a counter
  style ([`CounterStyleDecl`](crate::stdlib::features::numbering::CounterStyleDecl),
  which says how a counter value is shown) that gives the number template, the
  delimiters (`[` and `]`), the texts joining several numbers, whether
  consecutive numbers make a range, and the words `\R` writes (`ref.~`, `refs.~`,
  and the variant `capital`: `Ref.~` and `Refs.~`);
- the **heading** above the list of cited works: its title, `References` by
  default
  ([`without_heading`](crate::stdlib::features::citations::CitationsConfig::without_heading)
  places the list with no heading), and its level, 2 by default;
- the **note separator**, the text written before a note, `; ` by default.

The citation counter style belongs to the `citations` feature and is compiled
when the registry is built. The counter styles a host replaces through
[`NumberingConfig`](crate::stdlib::features::numbering::NumberingConfig) do not
replace it, and the `citations` feature does not need the `numbering` feature.

```
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::citations::{CitationsConfig, MarkForm};
use flm_core::stdlib::features::numbering::Delimiters;

// Numbers between parentheses, each written alone: `(1,2,3)`, not `(1–3)`.
let mut citations = CitationsConfig::new();
citations.style.delimiters = Delimiters::new("(", ")");
citations.style.ranges = false;
assert_eq!(citations.mark, MarkForm::Bracketed);
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new().citations(citations)));
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

A text of the citation counter style, the title or the note separator that is
not valid FLM makes the registry build fail, identifying the text. The citation
entries themselves come from the citation providers your program contributes
(see [Services the host provides](#services-the-host-provides)). The list of
cited works is a piece. A piece is a document-level synthesized value (a small
tree that a stage builds while a run is under way; [synthesized
values](crate::guide::concepts_overview#synthesized-values)) that a backend places
before or after the document's content ([`Piece`](crate::core::piece::Piece)).
The `citations` feature declares the list, placed after the content, in the
service slot `flm.pieces` of the `pieces` feature, and each backend places it. In
a registry without the `pieces` feature, the list is built and no backend places
it.

### When a registry build fails

A build that fails returns a
[`RegistryBuildError`](crate::core::registry::RegistryBuildError): the
[`BuildPhase`](crate::core::registry::BuildPhase) that found problems, and every
problem that phase found. The phases after it do not run, so the problems they
would have found are not listed.
[`render`](crate::core::registry::RegistryBuildError::render) formats the
problems as text, one per line.

A build that succeeds may still report problems.
[`FlmRegistry::build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics)
holds them: what the features reported about the configuration they compiled when
the registry was built (for example, the warnings of a counter style, and the
errors and warnings of each user definition given in the `userdefs` feature's
configuration, each identifying its user definition; a user definition is a
macro, environment or special character sequence defined by substitution,
[`Definition`](crate::lang::rewrites::userdefs::Definition)), and one Note for
each peer declaration that no feature of the registry resolves. A peer declaration is
a feature's forward declaration of something another feature owns, such as a
backend or a service slot ([`PeerName`](crate::core::registry::PeerName)). The
build succeeds even when these diagnostics hold errors: a refused user definition
stays registered, and every use of it is reported again. A host should therefore
report this collection once per build.

Writing features of your own is described in [Extending
FLM](crate::guide::extending).

### Settings given as data, and packages by name

Settings are a feature's configuration and option values written as data, one map
per feature from key to value, which the library reads into the feature's own
Rust types; a settings key is one field of those types
([`core::settings`](crate::core::settings)). A program that takes its settings
from a person, through a command line or through the front matter of a document,
gives them in this form.

Each feature that takes settings declares them once, as a
[`SettingsDecl`](crate::core::settings::SettingsDecl): the Rust types its
settings are read into, each with its binding (where a value may be set; see
[Options](#options)). A package factory whose build function adds the standard
features with
[`stdlib::add_features_from_settings`](crate::stdlib::add_features_from_settings)
lists them, each with its declaration. Reading a feature's data gives its
configuration, from which the factory builds the package, and its option values,
which you set on the document and on the plan request.

```rust
use flm_core::core::registry::{PackageEntry, PackageFactory, RegistryBuilder, RegistryInputs};
use flm_core::core::settings::PackageSettings;
use flm_core::stdlib;

let factory = PackageFactory::new("std", |settings: &PackageSettings| {
    Ok(stdlib::add_features_from_settings(PackageEntry::new("std"), settings))
})?;
let data = serde_json::json!({
    "math": { "numbered_equations": false },
    "text": { "width": 60 },
});

let mut settings = PackageSettings::new();
let mut render = flm_core::model::options::OptionValues::new();
for (feature, values) in data.as_object().unwrap() {
    let decl = factory.settings(feature).expect("a feature that takes settings");
    let read = decl.read(values.as_object().unwrap()).expect("settings that read");
    render.merge_from(read.render_options());
    settings.insert(read);
}
settings.switch_off("annotations");

let mut builder = RegistryBuilder::new();
builder.use_package(factory.build(&settings)?);
let registry = builder.build(&RegistryInputs::new())?;
assert!(!registry.has_feature("annotations"));
// `render` goes into the plan request's `render_options`.
assert_eq!(render.len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

[`SettingsDecl::read`](crate::core::settings::SettingsDecl::read) refuses a key
the feature does not declare and a value that does not have the shape its key
requires; each problem identifies the key by its path from the feature's name
(`text.width`). The reader receives the data only, not the file it was written
in, so the host adds the file and the line when it reports the problem.

A collection of named things (the xnote kinds, the float types) is written as a
map from each name to its settings entry, and is read in the order written. For
that, `flm-core` turns on the cargo feature `preserve_order` of `serde_json`.
Because cargo turns a feature on for a whole build, every JSON map of a program
that embeds the library then keeps the order in which its entries were inserted,
instead of sorting its keys.

Each declaration also describes the feature's settings as a JSON Schema
([`SettingsDecl::json_schema`](crate::core::settings::SettingsDecl::json_schema)),
with descriptions taken from the Rust documentation of the fields. A feature
that has no Rust type for its settings, such as a feature of an extension written
in JavaScript, declares them from a JSON Schema instead
([`SettingsDecl::config_data`](crate::core::settings::SettingsDecl::config_data),
[`SettingsDecl::options_data`](crate::core::settings::SettingsDecl::options_data));
its values are read as [`DataValue`](crate::model::options::DataValue)s, and a stage
reads such an option type by its name
([`StageCx::option_named`](crate::core::run::StageCx::option_named)).

Combining several sources of settings into one map per feature, before reading,
is the host's task: the host decides which sources exist and how they rank. The
library's settings cascade
([`core::settings::cascade`](crate::core::settings::cascade)) applies the rule
between them: the host records every value with an origin of its own type, which
gives the source's rank and, for a package, the package's name; the cascade gives
each key the value of the highest-ranked source, lets a package win over the
packages it uses
([`PackageCatalog::reach`](crate::core::registry::PackageCatalog::reach)), reports
two peer packages that disagree, combines a value key by key where the marks of the
feature's declaration say so ([`Mark`](crate::core::settings::Mark), written on the
schema's nodes under `x-flm-marks`), and reads the result through the declaration of
every feature in use that declares settings, a feature nothing is written for from
the empty map (a problem found there has no origin). Whatever a source writes for a
feature that no package in use has, `false` or settings, is accepted and read by
nothing: the result lists one warning for each source that writes it, at the first
origin of that source
([`ReadSettings::warnings`](crate::core::settings::cascade::ReadSettings::warnings)),
which your program reports, since the name may be misspelled. When the settings have
errors, the error lists the warnings beside them
([`ReadFailure`](crate::core::settings::cascade::ReadFailure)), so that your program
reports them with the errors.

A [`PackageCatalog`](crate::core::registry::PackageCatalog) finds a package's
factory by name: first among the factories compiled into your program, then
through the [`PackageResolver`](crate::core::registry::PackageResolver)
implementations you add, asked in order, only when no compiled factory has the
name. The library ships no package resolver.
[`resolve_all`](crate::core::registry::PackageCatalog::resolve_all) takes the
list of package names a document uses, walks the packages each of them uses, and
returns their factories in registration order: each after the packages it uses,
each once. A package that uses itself, directly or through others, is accepted.
When a name is unknown or a resolver fails, `resolve_all` returns every such
problem, each identifying the package and the position, in the list, of the name
whose walk reached it ([`UseError`](crate::core::registry::UseError)).

The crate `flm-ext` provides a package resolver for *extensions*: sets of files,
found by name, that provide packages written as data, in JavaScript or compiled to
WebAssembly (see
[Extensions](crate::guide::extending#extensions)). Its loader,
`flm_ext::ExtensionLoader`, asks the *sources* your program gives it, in order, for
the extension a `use` entry names; a source implements `flm_ext::ExtensionSource`,
and `flm_ext::MemorySource` holds extensions in memory. The loader reads each
extension once, through the engine its manifest names (the engine `data` always;
the engine `js` with `ExtensionLoader::with_js`, under `flm-ext`'s cargo feature
`quickjs`; the engine `wasm` with `ExtensionLoader::with_wasm`, under the cargo feature
`wasm`), and answers a factory for each package. Each of the two engines takes a host
value (`flm_ext::quickjs::JsHost`, `flm_ext::wasm::WasmHost`) that holds the limits of
its instances, whether every call runs on a fresh instance, and the log sink of
`flm-core` (`with_log_sink`) that receives the extensions' `console` output, which
otherwise goes nowhere; your program chooses them. The services your program offers to
extensions' handlers, such as reading a file beside the document, are added with
`ExtensionLoader::with_host_service` (see [Host services for
extensions](#host-services-for-extensions)). After the catalog has resolved
the document's packages, `loader.blocks()` gives the settings each package of an
extension writes, which your program adds to its settings cascade as one source per
package. Since `flm-ext` is not a dependency of this guide's crate, the example is
not compiled:

```ignore
use std::sync::Arc;
use flm_core::core::registry::{PackageCatalog, PackageEntry, PackageFactory};
use flm_core::core::settings::PackageSettings;
use flm_core::stdlib;
use flm_ext::{ExtensionLoader, ExtensionSource, MemoryFiles, MemorySource};

// An extension written as data, held in memory under the locator `house`.
let source = MemorySource::new().with(
    "house",
    "memory/house",
    serde_json::json!({ "flm": { "features": { "text": { "width": 72 } } } }),
    MemoryFiles::new(),
);
let sources: Vec<Arc<dyn ExtensionSource>> = vec![Arc::new(source)];
let loader = Arc::new(ExtensionLoader::new(sources));

let mut catalog = PackageCatalog::new();
catalog.add_compiled(PackageFactory::new("std", |settings: &PackageSettings| {
    Ok(stdlib::add_features_from_settings(PackageEntry::new("std"), settings))
})?);
catalog.add_resolver(loader.clone());
let factories = catalog.resolve_all(&["std", "house"])?;

// The settings the package `memory/house#_` writes: one source of the cascade.
for loaded in loader.blocks() {
    for (feature, block) in &loaded.block.features {
        // `cascade.switch_off(feature, origin)` when `block.enabled` is false,
        // `cascade.switch_on` otherwise, then
        // `cascade.set(feature, key, value, origin)` for each key of `block.settings`.
    }
}
```

## Parsing sources into fragments

[`parse_fragment`](crate::lang::parse_fragment) parses one source text with a
registry's language and returns one [`Fragment`](crate::model::doc::Fragment). A
fragment is one piece of source as the parser delivered it: its tree, its source,
its parse diagnostics, and how its top level is laid out
([`FragmentLayout`](crate::model::doc::FragmentLayout): as blocks, as inline
content, or as blocks only when it holds a block, the default).

```rust
use std::sync::Arc;
use flm_core::model::doc::FragmentLayout;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(
    FlmSource::new("A \\emph{first} paragraph.").with_origin(FlmOrigin::new("intro.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

assert!(fragment.diags().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

The call behaves as follows.

**The language comes from the registry.**
[`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang) returns the
language that the registry built from the features' element declarations. It is
the only way to obtain the language of a registry, and one language serves any
number of parses on any number of threads.

**The caller builds the source.** The call takes an `Arc<FlmSource>` rather than
a string, because a source carries its [origin](crate::lang::FlmOrigin). The
origin's canonical path identifies the source wherever something about it is
recorded: in the check that no file includes itself, in the tables a host keeps
beside the document, and in the file name a diagnostic reports. A source created
inside the parse would carry no such identity.

**A parse is tolerant.** A problem in the source does not stop the parse: the
problem becomes a diagnostic recorded in the fragment, and the tree still covers
the whole input. An unknown command, a stray `$` and an unbalanced group are all
diagnostics of a fragment that has a tree.

**A parse fails only for reasons that are not the document's syntax.**
[`FragmentError`](crate::lang::FragmentError) lists the three ways a parse
produces no tree: the content nests more deeply than the recursion guard allows;
the parse was aborted because an extension's parser broke a requirement of
techy's parser or a method of the language failed, which is an error in the
program, not in the document; or no
worker thread could be created (see [The worker-thread
requirement](#the-worker-thread-requirement)).
[`FragmentError::diagnostic`](crate::lang::FragmentError::diagnostic) returns the
single diagnostic to record in place of the missing tree.

[`ParseOptions`](crate::lang::ParseOptions) changes two settings of the
registry's language for one parse: `max_diagnostics`, how many diagnostics the
parse keeps (the diagnostics past that number are counted and the count is
reported), and `input_resolver`, the input resolver to use instead of the
registry's (see [Documents of several files](#documents-of-several-files)).

[`parse_fragment_with_state`](crate::lang::parse_fragment_with_state) is the same
parse, started from a parsing state that you derive from the language's initial
state with [`BuiltLang::derive_state`](crate::lang::BuiltLang::derive_state). A
derived state keeps what the registry build installed (the features' packages,
the structural elements) and changes something else; the library uses it to
parse the template of a user definition, with the placeholders added.

### The worker-thread requirement

Parsing, walking and rewriting a tree are recursive operations, so deeply nested
input uses stack in proportion to its depth. The library's **recursion guard**
([`crate::lang::guard`]) measures how much stack a recursive operation has used
and refuses to descend one level further once the budget
[`STACK_BUDGET`](crate::lang::guard::STACK_BUDGET) (48 MiB) is spent. The
library then reports a nesting-too-deep condition instead of overflowing the
stack, which would end the process: `flm.blocks.nesting-too-deep` for a parse or
a rewrite, while the lowering and each backend's emit stage report their own.

The budget limits how much stack an operation may use; it does not check how
much stack the thread has. A guarded operation must therefore run on a **worker
thread**. A worker thread is a thread created with at least `WORKER_STACK_SIZE`
bytes of stack; every guarded run (a parse, a rewrite, an emit) must execute on
one, and the library's entry points create one when the calling thread is not
marked as a worker thread
([`on_worker_thread`](crate::lang::guard::on_worker_thread); [the recursion guard
and worker
threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).
[`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) is 64 MiB. A
thread-local marker records whether the current thread is a worker thread, so a
program that calls the library's entry points needs to do nothing about this
requirement.

Three functions of [`crate::lang::guard`] let a program arrange the worker
threads itself:

- [`on_worker_thread`](crate::lang::guard::on_worker_thread) runs a closure on
  the current thread if that thread is marked as a worker thread, and otherwise
  on a new worker thread. Calling it once around a batch of work saves each entry
  point inside from creating a thread of its own; the `flm` command-line program
  does so around the whole of a `flm dump` command.
- [`run_with_worker_stack`](crate::lang::guard::run_with_worker_stack) always
  creates a new worker thread and runs the closure there.
- [`mark_worker_thread`](crate::lang::guard::mark_worker_thread) marks the
  current thread as a worker thread. Call it *only* on a thread that you know has
  at least `WORKER_STACK_SIZE` bytes of stack: marking a thread with less stack
  allows the stack overflow that the requirement prevents.

A traversal of a tree that your program writes itself, outside the library's
entry points, has no recursion guard, and only the stack of its thread limits it.
Running such a traversal inside `on_worker_thread` gives it the same amount of
stack as the library's own operations.

## Assembling a document

A **document** ([`Doc`](crate::model::doc::Doc)) is its content
([`DocContent`](crate::model::doc::DocContent)) and the option values bound on
it, and nothing else. The content consists of three things: an arena that numbers
every tree with a [`TreeId`](crate::model::addr::TreeId) (a tree is one placement
of a fragment's node tree in the document), the order in which the trees are
walked, and a map that records which node places which other tree.

- [`DocContent::place`](crate::model::doc::DocContent::place) adds a fragment to
  the arena *and* to the walk order. This is the usual case.
- [`DocContent::add_tree`](crate::model::doc::DocContent::add_tree) adds a
  fragment to the arena only, for a tree that the walk reaches through a node that
  places it.
- [`DocContent::resolve`](crate::model::doc::DocContent::resolve) records that a
  node places another tree, so that the walk visits that tree at the node. This
  is how one part of a document includes another.

Placing one shared `Arc<Fragment>` twice makes two trees, each with computed data
of its own. A document has no other structure: no front matter, no chapters, no
named parts. Its sections come from the headings in its content, which a stage
computes.

[`Doc::new`](crate::model::doc::Doc::new) creates a document with no option
value; [`Doc::with_options`](crate::model::doc::Doc::with_options) also sets
option values on the document (see [Options](#options)).

### Documents of several files

A document may bring in other files with `\input{file}`, the command of the
standard [`input`](crate::stdlib::features::input) feature. The
library reads no file, so for each `\input` it asks the host's input resolver, an
object implementing [`InputResolver`](crate::lang::InputResolver). The library
passes it three things: the reference as the document wrote it, the name of the
file that holds the command, and the parsing settings that apply where the
command is written. The resolver finds the file, reads it, splits off its front
matter, and returns an [`IncludedFile`](crate::lang::IncludedFile):

- the name under which the file is known, which messages print and which tells
  two files apart;
- the file's content, without its front matter, and the line of the file on
  which that content begins;
- the parsing settings the content is read under, when the file sets some of its
  own (the settings passed to the resolver, with the file's own on top; the `flm`
  program reads them from the keys `comments`, `comment_prefix` and
  `dollar_math` under `flm.parsing` in the included file's front matter);
- the problems the resolver found in the file's front matter.

When no file is found or it cannot be read, the resolver returns an
[`InputError`](crate::lang::InputError). The resolver alone decides how a
reference becomes a path and what a front matter holds.

The library does the rest. It parses the content where the command is written,
under the parsing state that applies there. It records the parsing settings each
included file is read under, so the resolver needs no memory of its earlier
calls. It checks that no file includes itself and that a chain of inclusions is
no deeper than [`DEPTH_LIMIT`](crate::lang::input::DEPTH_LIMIT). Before any other
rewrite, the rewrite stage `flm.input.splice` replaces the command's node by the
included content, so every later stage sees one continuous text.

An included file can instead be a **heading input**: a file whose front matter
declares the heading it opens with
([`IncludedFile::heading`](crate::lang::IncludedFile::heading), an
[`InputHeading`](crate::lang::InputHeading): the heading command, the title as
FLM text and where it is written, the label, and whether the heading is
numbered). The resolver reads these facts from the file's front matter, and the
library makes the file a tree of its own, opening with that heading, placed at
the command. Only [`assemble`](crate::model::assemble::assemble) can place a
heading input; a plain parse reports it as an error.

[`assemble`](crate::model::assemble::assemble) parses a primary file (the file a
host is asked to process) and the files it includes into a document's content.
The input resolver is given either for one assembly, in
[`ParseOptions::input_resolver`](crate::lang::ParseOptions::input_resolver), which
suits a host whose resolver records the files one build read, or for every parse
of a registry, in
[`RegistryInputs::input_resolver`](crate::core::registry::RegistryInputs#structfield.input_resolver):

```rust
use std::sync::Arc;
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::lang::{
    FlmOrigin, FlmParsing, FlmSource, IncludedFile, InputError, InputResolver, ParseOptions,
};
use flm_core::model::assemble::assemble;
use flm_core::model::doc::{Doc, FragmentLayout};
use flm_core::stdlib::{self, StdlibConfig};

/// A host whose files are held in memory.
struct Files;

impl InputResolver for Files {
    fn resolve(&self, reference: &str, _including: Option<&str>, _parsing: &FlmParsing) -> Result<IncludedFile, InputError> {
        match reference {
            "middle.flm" => Ok(IncludedFile {
                name: String::from("middle.flm"),
                text: String::from("the middle"),
                first_line: 1,
                ..IncludedFile::default()
            }),
            _ => Err(InputError::new(format!("there is no file `{reference}`"))),
        }
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
let registry = builder.build(&RegistryInputs::new())?;
let source = Arc::new(
    FlmSource::new("Begin \\input{middle.flm} end.").with_origin(FlmOrigin::new("main.flm")),
);
let options = ParseOptions { input_resolver: Some(Arc::new(Files)), ..ParseOptions::default() };
let content = assemble(registry.lang(), source, FragmentLayout::default(), &options)?;
let doc = Arc::new(Doc::new(content));
# let _ = doc;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Asking for a plan

A plan says which stages run, and in which order. The library derives it from
**capabilities**. A capability is a registered dotted name for a state of
processing that a stage brings about, such as `flm.blocks.structured`: a stage
declares the capabilities it provides and the capabilities it requires, and a
caller demands the capabilities it wants
([`CapabilityDef`](crate::core::registry::CapabilityDef)). To demand a
capability is to ask the plan for a stage that provides it. A
[`PlanRequest`](crate::core::plan::PlanRequest) holds the demands and the choices
that go with them, and [`Plan::build`](crate::core::plan::Plan::build) derives
from it the [`Plan`](crate::core::plan::Plan): the stages that run, in dependency
order and grouped in batches. The [pipeline guide](crate::guide::pipeline)
describes how the plan is derived.

Most callers never write a plan request. They choose a registered **workflow**, a
named set of demands together with the choices that go with them
([`WorkflowDef`](crate::core::registry::WorkflowDef)), and
[`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) turns it into the
plan request. The standard features register five workflows:

- `text` ([`text::WORKFLOW`](crate::stdlib::backends::text::WORKFLOW)): render
  the document as plain text;
- `latex-recomposed`
  ([`latex_recomposed::WORKFLOW`](crate::stdlib::backends::latex_recomposed::WORKFLOW)):
  recompose the document into LaTeX;
- `latex-rendered`
  ([`latex_rendered::WORKFLOW`](crate::stdlib::backends::latex_rendered::WORKFLOW)):
  render the document as fully resolved LaTeX for LuaLaTeX;
- `html` ([`html::WORKFLOW`](crate::stdlib::backends::html::WORKFLOW)): render
  the document as one HTML page;
- `check` ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW)): validate the
  document and write no output.

All five demand the collection [`flm.validated`](crate::stdlib::VALIDATED), a
name for the set of capabilities that every check of the standard features adds
itself to. The four rendering workflows demand it beside their output, so a
render reports the problems that `check` reports, with one difference: an
unresolved reference is an Error by default in a render and always a Warning
under `check`. The `check` feature declares
that collection, so a registry that holds a standard backend must hold the
`check` feature too ([`check_feature`](crate::stdlib::check_feature)); the
standard features include it.

[`Overrides`](crate::core::plan::Overrides) changes a workflow's plan request
without restating it. It has five fields:

- capabilities demanded in addition to the workflow's;
- stages preferred ahead of the workflow's preferences;
- lowering rule selections that replace the workflow's for the backends they
  concern;
- elements accepted as unrenderable, in addition to those the workflow accepts
  (an accepted unrenderable element is an element that a backend need not
  render: the plan builds anyway, and each occurrence lowers to an empty
  `flm.pres.unsupported` node with one Error diagnostic;
  [`PlanRequest`](crate::core::plan::PlanRequest#structfield.unsupported));
- option values of the render request, the most common change:

```rust
use flm_core::model::options::OptionValues;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::stdlib::backends::text::TextRenderOptions;
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;

let overrides = Overrides {
    render_options: OptionValues::new()
        .with(TextRenderOptions { width: Some(40), ..TextRenderOptions::default() }),
    ..Overrides::none()
};
let plan = Plan::for_workflow(&registry, "text", &overrides)?;

// A plan lists its stages in the order a run runs them.
assert!(plan.stage_names().iter().any(|name| name.as_str() == "flm.text.emit"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The text backend's render options are
[`TextRenderOptions`](crate::stdlib::backends::text::TextRenderOptions):

- `width`, the line width in columns (80 by default; `None` never wraps);
- `tab_width`, the width of a tab in columns (4 by default): how far a line of a
  quotation's lines section indented by one `\indent` is indented
  ([`TextRenderOptions::columns`](crate::stdlib::backends::text::TextRenderOptions::columns)
  converts every length to columns);
- `run_in_stop`, the text written after the title of a run-in heading;
- `root_heading_level`, the heading level printed as a first-level heading (1 by
  default);
- `link_addresses` and `link_addresses_when_shown`, whether a link prints its
  address (see [Options](#options));
- `table_frame`, how a table is drawn: `none`, the default, sets the columns two
  spaces apart with a rule of `-` under the header rows; `box` draws a frame of
  box-drawing characters around every cell;
- `heading_rules`, the characters of the rule under a heading: `ascii`, the default,
  draws `=` and `-`; `unicode` draws `═` and `─`.

A plan is a pure value: it depends on the registry and the plan request alone, and
holds nothing of any document. One plan therefore renders every document in the
same way. [`Plan::key`](crate::core::plan::Plan::key) is a number computed over
everything the plan was built from ([`PlanKey`](crate::core::plan::PlanKey)), so
a program can cache a built plan under its key.

A plan states what a run of it will do:

- [`stage_names`](crate::core::plan::Plan::stage_names) and
  [`batch_names`](crate::core::plan::Plan::batch_names): the stages in the order
  they run, and their grouping in batches;
- [`active_backends`](crate::core::plan::Plan::active_backends): the backends the
  plan lowers for;
- [`externals`](crate::core::plan::Plan::externals): the external tables your
  program must install (see [External tables and two-run
  composition](#external-tables-and-two-run-composition));
- [`warnings`](crate::core::plan::Plan::warnings): the preferences the plan could
  not honor and the output of its lowering rules that nothing checks
  ([`PlanWarning`](crate::core::plan::PlanWarning)), which the runner reports as
  Warnings at the start of every run (`flm.run.unchecked-output` for output nothing
  checks); a plan of `html`, `latex-rendered` or `latex-recomposed` with
  `math-passthrough` holds one, for the formulas of that feature, whose commands nothing
  checks, and so does a plan of `latex-rendered` or `latex-recomposed` with
  `math-techxt`;
- [`Plan::render`](crate::core::plan::Plan::render): the plan as five lines of
  text (its rewrite stages, the order of its stages, the order in which they were
  chosen, the stages ruled out, and the external tables), for a report or for a
  test that compares two plans.

A plan that cannot be built is a [`PlanError`](crate::core::plan::PlanError).
Every variant holds the names of the stages, capabilities, elements or other
entities involved, and a variant about a name that is not declared also lists
the names that are, except `UnknownElement`.

## Running

A **run** ([`Run`](crate::core::run::Run)) is one execution of one plan over one
document; it owns the frozen trees, the store and the diagnostics. A frozen tree
is a document tree after the rewrite stages have run, which nothing changes again
([frozen trees and computed
data](crate::guide::concepts_overview#frozen-trees-and-computed-data)); the store
holds the tables the stages compute
([`Computed`](crate::core::data::store::Computed)). Because a run owns everything
it computes, one document can be run several times, under different plans, and
no run sees another's results.

[`Run::new`](crate::core::run::Run::new) creates a run and executes nothing: it
checks the inputs and resolves the option values of every scheduled stage, so
that every run that exists has sound inputs. Then either
[`Run::execute`](crate::core::run::Run::execute) runs it to the end, or
[`Run::step`](crate::core::run::Run::step) runs one step at a time (the rewrite
stages first, then one batch per call) and returns
[`Step::More`](crate::core::run::Step) while steps remain and `Step::Done` once
the run is over, which lets a program do other work between two steps. Stepping
changes only when the work happens, never what is computed. Once the run is over,
`execute` and [`Run::outcome`](crate::core::run::Run::outcome) give its
[`RunOutcome`](crate::core::run::RunOutcome): `Finished`, `HaltedAtGate` (see [The
error gate](#the-error-gate)) or `Cancelled` (see [Following a
build](#following-a-build)).

To show the progress of a run, or to be able to cancel it, give the run a scope
of a progress sheet with
[`Run::with_progress`](crate::core::run::Run::with_progress) before its first
step ([Following a build](#following-a-build)). A run given none reports nowhere
and cannot be cancelled; it computes the same either way.

[`RunOptions`](crate::core::run::RunOptions) has three settings:

- `sequential`: run all the work on one thread, even when the cargo feature
  `parallel` is compiled in. The result is the same either way.
- `continue_with_errors`: run the stages that have effects outside the store even
  when the run has recorded errors (see [The error gate](#the-error-gate)).
- `max_diagnostics`: how many of the diagnostics the run produces it keeps. The
  diagnostics past that number are counted but not kept;
  [`DiagnosticsView::any_recorded`](crate::core::run::DiagnosticsView::any_recorded),
  [`DiagnosticsView::has_errors`](crate::core::run::DiagnosticsView::has_errors)
  and
  [`DiagnosticsView::error_count`](crate::core::run::DiagnosticsView::error_count)
  count them. The run reads this setting from the options of its first step
  only.

Apart from the error gate, these settings do not change what is computed.

### The error gate

Each stage declares an [`Effect`](crate::core::stage::Effect):
[`Compute`](crate::core::stage::Effect::Compute) (it produces computed data),
[`Check`](crate::core::stage::Effect::Check) (it produces diagnostics only) or
[`Io`](crate::core::stage::Effect::Io) (it has effects outside the store, such as
handing an output's files to the program that writes them). The plan orders every
`Io` stage after every `Compute` and `Check` stage. A run that has recorded an
Error diagnostic stops before the first batch that holds an `Io` stage: this is
the **error gate**. A document with errors therefore produces no effect outside
the store, unless you set `continue_with_errors`.
The outcome of such a run, which [`Run::execute`](crate::core::run::Run::execute) and
[`Run::outcome`](crate::core::run::Run::outcome) return, is
[`RunOutcome::HaltedAtGate`](crate::core::run::RunOutcome::HaltedAtGate).

The standard features register no `Io` stage: the text backend, for example,
writes its bytes into a table like any other computed data, and writing them to a
file is the host's task. A plan built from the standard features alone therefore
always runs to the end.

### What a run refuses

A [`RunError`](crate::core::run::RunError) means that the run was asked for
something it cannot do; a problem with the document's content is a diagnostic,
never a `RunError`. [`Run::new`](crate::core::run::Run::new) refuses an option
value set on the document for an option type that may not be set there, an
external table the plan needs and that has no installed value, an installed value
for a table that the registry does not declare or does not declare as external,
an installed value of the wrong type, and an installed value whose stamp does not
match the run's (see [External tables and two-run
composition](#external-tables-and-two-run-composition)).
[`Run::step`](crate::core::run::Run::step) and
[`Run::execute`](crate::core::run::Run::execute) return a `RunError` when no
worker thread can be created and when an invariant of the store fails.

A stage that panics propagates the panic out of `step` and `execute`. A panic is
a defect in the stage, never a way of reporting something about a document. The
whole batch is collected before anything is added to the store, so the store is
unchanged when a panic propagates. [The panics chapter](crate::guide::panics)
lists every item of the public interface that can panic.

## Reading results

Everything a run computes is held in typed **tables**. A table is typed data that
one stage writes beside the frozen trees and other stages read; its identity is
its registered dotted name, never its Rust type, and that name is also a
capability ([`Table`](crate::core::data::table::Table)).
[`Run::table`](crate::core::run::Run::table) reads one table through its typed name
([`TableKey`](crate::core::data::table::TableKey), such as a
[`TableName`](crate::core::data::table::TableName) constant). Unlike a stage's read, it is not restricted to declared reads: a
caller may read any table the registry declares. `Ok(None)` means that the table
holds no value yet: no stage of the plan writes it, or the batch that writes it
has not run.

The text backend writes two tables a program usually reads:

- [`TEXT_DOCUMENT`](crate::stdlib::backends::text::TEXT_DOCUMENT)
  (`flm.output.text.document`) holds an
  [`ArtifactSet`](crate::core::artifact::ArtifactSet). An artifact is one file's
  worth of output held as a value, identified by its producer and a name and never
  by a path; an artifact set is the artifacts of one output, at most one of which
  is the entry artifact, the one that stands for the output as a whole. Nothing in
  the library writes files, so a host that writes them asks its own
  [`ArtifactNamer`](crate::core::artifact::ArtifactNamer) where each artifact
  goes.
- [`TEXT_OUTPUT`](crate::stdlib::backends::text::TEXT_OUTPUT)
  (`flm.output.text`) holds the
  [`TextOutput`](crate::stdlib::backends::text::TextOutput) the entry artifact
  was made from: the printed `text`, the `marks` that give the byte range of
  the text that each element produced, and the `styles`, the style marks that give
  the ranges whose content plain text cannot show (a heading's title, a bold or an
  emphasized span, a link's content). Read it when you need positions as well as
  text, or to show the styles in a medium that can, as the `flm` program's profile
  `tty` does for a terminal.

The HTML backend writes four tables that a program reads, depending on whether
it uses the backend's page or writes a page of its own:

- [`DOCUMENT`](crate::stdlib::backends::html::DOCUMENT)
  (`flm.output.html.document`) holds the artifact set whose entry artifact is a
  complete page.
- [`OUTPUT_TABLE`](crate::stdlib::backends::html::OUTPUT_TABLE)
  (`flm.output.html`) holds each document tree's HTML on its own, for a program
  that places the content in a page it writes itself.
- [`REGIONS`](crate::stdlib::backends::html::REGIONS)
  (`flm.output.html.regions`) holds the page's output regions
  ([`Regions`](crate::core::region::Regions)). An output region is one named part
  of an output's entry artifact (the document's content, each piece placed beside
  it, and on HTML the stylesheets, the scripts, the title and the language); a
  host that assembles the page through a template of its own reads them ([output
  regions and the final
  stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
- [`DEFINITIONS_TABLE`](crate::stdlib::backends::html::DEFINITIONS_TABLE)
  (`flm.html.definitions`) holds every stylesheet and script the document
  requires, once each. A style or script definition is a stylesheet or a script
  that a feature's output depends on, recorded in the registry under an
  identifier ([`StyleDef`](crate::core::registry::StyleDef)); the [rendering
  guide](crate::guide::rendering) describes them in full. The table lists the
  stylesheets first, then the scripts, each in the order in which the features
  registered them. Each row says whether the definition is a stylesheet or a
  script, the feature that declared it, and its delivery: written into the page,
  loaded from an address, or supplied by your own page or bundle. Write the
  stylesheets in the page's head and the scripts at the end of its body, in the
  table's order. One row,
  [`flm.html.style.page`](crate::stdlib::backends::html::STYLE_PAGE), styles the
  page that the backend assembles rather than the content; leave it out of a page
  of your own. A stylesheet or script of your own that every page must carry,
  such as a script that your program adds to every page it shows, is required
  through the render option
  [`required_definitions`](crate::stdlib::backends::html::HtmlRenderOptions::required_definitions)
  and appears in the table like the content's.

```rust
use flm_core::stdlib::backends::text::{TEXT_DOCUMENT, TEXT_OUTPUT};
# use std::sync::Arc;
# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;
# let source = Arc::new(
#     FlmSource::new("\\section{Intro}\n\nSome text.").with_origin(FlmOrigin::new("d.flm")),
# );
# let fragment = parse_fragment(
#     registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
# )?;
# let mut content = DocContent::new();
# content.place(Arc::new(fragment));
# let doc = Arc::new(Doc::new(content));
# let plan = Arc::new(Plan::for_workflow(&registry, "text", &Overrides::none())?);
# let mut run = Run::new(doc, plan, Externals::none())?;
# run.execute(&RunOptions::new())?;

// The printed text, with the marks that point into it.
let output = run.table(TEXT_OUTPUT)?.expect("the text workflow writes it");
assert!(output.text.contains("Intro"));
assert!(!output.marks.is_empty());

// The same text as the output's one artifact.
let artifacts = run.table(TEXT_DOCUMENT)?.expect("the text workflow writes it");
let entry = artifacts.entry().expect("a one-artifact set");
assert_eq!(entry.media_type, "text/plain");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Besides the tables, a run gives access to the trees and the store:

- [`Run::frozen_tree`](crate::core::run::Run::frozen_tree) and
  [`frozen_trees`](crate::core::run::Run::frozen_trees) return each tree as the
  last rewrite stage left it. Every stage after the rewrite stages reads this form,
  and no stage changes it.
- [`Run::tree_order`](crate::core::run::Run::tree_order) returns the order in
  which the run walks the trees.
- [`Run::computed`](crate::core::run::Run::computed) returns the whole store, for
  walking tables whose types you do not know.

### Diagnostics

A diagnostic reports a problem with a document: a severity (Error, Warning or
Note), a position, and a typed condition with its data; its message is written
for a person and is not a stable interface
([diagnostics](crate::guide::concepts_overview#diagnostics)). A condition is
identified by a dotted name, such as `flm.refs.unresolved`. The identifier and
the condition's data are a stable interface; the message may be reworded at any
time, so a program selects diagnostics by identifier, never by message text.

The diagnostics of a parse belong to the fragment
([`Fragment::diags`](crate::model::doc::Fragment::diags)); the diagnostics of the
stages belong to the run. [`Run::diagnostics`](crate::core::run::Run::diagnostics)
returns a [`DiagnosticsView`](crate::core::run::DiagnosticsView) of both: each
fragment's parse diagnostics, in the order in which the run walks the fragments
and once per fragment however many times it is placed, then the diagnostics the
stages produced.
[`Run::pipeline_diagnostics`](crate::core::run::Run::pipeline_diagnostics)
returns the stages' diagnostics alone.

```rust
use flm_core::stdlib::CHECK_WORKFLOW;
# use std::sync::Arc;
# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;

// A reference to a label the document never defines.
let source = Arc::new(
    FlmSource::new("See \\ref{sec:nowhere}.").with_origin(FlmOrigin::new("d.flm")),
);
let fragment = parse_fragment(
    registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
)?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

let plan = Arc::new(Plan::for_workflow(&registry, CHECK_WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// Select by identifier, never by message text.
let unresolved: Vec<_> = run
    .diagnostics()
    .iter()
    .filter(|d| d.identifier() == "flm.refs.unresolved")
    .collect();
assert_eq!(unresolved.len(), 1);

// `has_errors` counts what was reported, kept or not. The `check` workflow
// reports an unresolved reference as a Warning, so this document has no error.
assert!(!run.diagnostics().has_errors());
# Ok::<(), Box<dyn std::error::Error>>(())
```

[`DiagnosticsView::render_all`](crate::core::run::DiagnosticsView::render_all)
formats every kept diagnostic as text, one per line, as a command-line program
prints them. In tests, [`crate::dump::diag`] selects diagnostics by identifier and
narrows the selection by the text they cover, their position, their severity and
their data.

## Following a build

A build can take a while: a document with many images to convert, citations to
fetch over the network, or a program such as `latexmk` to run. A program that
shows how far a build has gone, or that lets a person stop it, uses the module
[`flm_core::progress`](crate::progress).

The module provides a **progress sheet** ([`Sheet`](crate::progress::Sheet)):
the record of one build, as a tree of **rows**. A row stands for one piece of
work a person may wait for (a stage, a conversion, a fetch, a program run). Each
row has a **kind**, a dotted name for the sort of work, the same for every row of
that sort (e.g., `flm.graphics.convert`), and optionally the kind's **title**, a
text for a person ("Converting graphics"), and a **label** that tells one row of
the kind from another (a file, a citation key). A row has a state
([`RowState`](crate::progress::RowState): `Pending`, `Running`, `Finished`,
`Failed`, or `NotRun` for a row declared and never started) and optionally a
count of units done out of a total fixed when the row starts. The work writes the
rows; your program reads them when it chooses, and the work is never told that it
was read. The one thing the work reads from the sheet is whether the build was
asked to stop.

### Creating a sheet

[`Sheet::start`](crate::progress::Sheet::start) creates a sheet and returns its
two halves:

- the [`Sheet`](crate::progress::Sheet), your program's half: it takes snapshots
  and cancels the build. It is cheap to clone, and may be read from any thread.
- the **root row**, as a [`Task`](crate::progress::Task): its
  [`scope`](crate::progress::Task::scope) is what you give to the work, and you
  end it with [`finish`](crate::progress::Task::finish) when the build is over.

A [`Scope`](crate::progress::Scope) is the handle through which the work declares
rows under a running row, asks whether the build was cancelled, and writes log
lines. Give the root's scope to a run with
[`Run::with_progress`](crate::core::run::Run::with_progress): the run declares
its rows under it ([the rows of a
run](crate::guide::pipeline#progress-and-cancellation-of-a-run)), and each stage
declares the rows of its own slow work under the row of the stage.
[`Scope::none`](crate::progress::Scope::none) is the scope of a build that
reports nowhere.

`Sheet::start` also takes the two things your program gives the sheet, both
optional: an **observer** and a **log sink**. Nothing else of your program's is
ever called by the sheet.

### The observer

An [`Observer`](crate::progress::Observer) holds three optional functions, each a
closure that is `Send` and `Sync`:

- `row_started` is called when a row starts, and `row_ended` when a row that had
  started ends (as `Finished` or `Failed`). Each receives the row as a snapshot
  gives it, and returns the time to store on the row, if any: the time elapsed
  since an origin your program chooses, usually the start of the build. The sheet
  reads no clock of its own, so a row carries times only when these functions
  return them.
- `checkpoint` is called each time the work is about to ask whether the build was
  cancelled ([`Scope::cancelled`](crate::progress::Scope::cancelled)). The work
  asks once per unit of its work: a tree, an image, a formula.

The functions are called on the thread doing the work, possibly from several
threads at once, and the work waits until they return. They are never called when
a count advances. A function you do not set costs one test and no call. Inside
any of them, your program may take a snapshot and may cancel the build. The
documentation of [`Observer`](crate::progress::Observer) states the full
contract.

A program that has a thread of its own to read the sheet with, such as the `flm`
program, sets the two row functions to return its clock, and sets no
checkpoint.

### Reading the sheet: the snapshot and the estimate

[`Sheet::snapshot`](crate::progress::Sheet::snapshot) returns a
[`Snapshot`](crate::progress::Snapshot): whether the build was cancelled, and the
rows in the order they were declared, as plain data that serializes with serde.
Rows are never removed, a state only moves forward, and a count only grows, so a
program that reads the sheet from time to time misses nothing that matters. A
snapshot is for display: the counts are read one after the other, not at one
instant.

The sheet holds facts, and no percentage. The fraction of a build done is an
estimate that your program computes with two functions beside the sheet, which
keep no memory and read no clock:

- [`estimate`](crate::progress::estimate)`(&snapshot, &expected, now)` returns
  the fraction done of the root row, from 0 to 1, at the time `now` (counted from
  the same origin as the rows' times). It weighs each row by its expected
  duration, taken from a table [`Expected`](crate::progress::Expected), which
  gives durations by the name of a kind.
- [`measure`](crate::progress::measure)`(&snapshot)` reads such a table from the
  last snapshot of a build whose rows carry their times.

A program that builds the same document again lays the table measured on the
last build over a table of defaults
([`Expected::over`](crate::progress::Expected::over)), so that the next estimate
is closer to that document's real proportions. The estimate can decrease from one
call to the next, as a row reveals its total or a row is declared late: a display
keeps the highest fraction it has shown for the build, and shows the whole only
when the root row has finished.

A row that the work may need is declared before the work knows whether it will
be needed, and dropped unstarted (it then ends as `NotRun`) when it is not. A row
declared in advance and not run moves the estimate forward; a row declared late
moves it back. This is why possible work is declared before it starts.

### The depth limit

A program chooses how much detail its sheet holds with a depth limit:
[`Scope::with_depth_limit`](crate::progress::Scope::with_depth_limit) and
[`Pending::with_depth_limit`](crate::progress::Pending::with_depth_limit) give
the number of levels of rows that may be declared below a row, 0 for none. Below
the limit, a handle has no row: the work calls it all the same and nothing is
declared, but `cancelled` still answers and a log line still reaches the log sink,
attributed to the nearest row that exists. A site generator that counts its
documents, for example, gives each document's row the limit 0, so that the sheet
holds one row per document.

### The log sink

A [`LogSink`](crate::progress::LogSink) receives the **log lines** of the work:
text for a person about what the build does, such as each request sent to fetch
citation data. A problem of the document is never a log line; it is a diagnostic.
Each line is a [`LogRecord`](crate::progress::LogRecord) with a level
([`LogLevel`](crate::progress::LogLevel): `Error`, `Warn` or `Info`), the text,
and its source, a short name of who writes it: for a line written through a scope,
the name of the kind of that scope's row. The sink's method `enabled(level)` lets
the work skip building a text that nobody wants. The work writes through
[`Scope::log`](crate::progress::Scope::log). The sink is called on the thread
that writes the line, possibly from several threads at once.

The trait does not depend on a sheet: the same sink may receive lines from code
that runs outside any build. The engines of `flm-ext` that run extensions, for
example, take a log sink for the extensions' `console` output, which an extension
also writes while it is loaded.

### Cancelling a build

[`Sheet::cancel`](crate::progress::Sheet::cancel) asks the build to stop, from
any thread or from inside a function of the observer. The work sees it the next
time it asks: a run discards the step under way and ends with the outcome
[`RunOutcome::Cancelled`](crate::core::run::RunOutcome::Cancelled), and the tables
and diagnostics of the steps completed before stay readable. Every row still
running ends as `Failed`. A cancellation produces no diagnostic, since it is not a
problem of the document. A run that is not cancelled produces exactly what it
would produce without a sheet.

The example below follows a run on a sheet whose rows carry the time from the
start of the build, then runs the same document again on a sheet cancelled before
the run starts:

```rust
# use std::sync::Arc;
# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;
# let source = Arc::new(
#     FlmSource::new("\\section{Intro}\n\nSome text.").with_origin(FlmOrigin::new("d.flm")),
# );
# let fragment = parse_fragment(
#     registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
# )?;
# let mut content = DocContent::new();
# content.place(Arc::new(fragment));
# let doc = Arc::new(Doc::new(content));
# let plan = Arc::new(Plan::for_workflow(&registry, "text", &Overrides::none())?);
use std::time::Instant;
use flm_core::core::run::{Externals, Run, RunOptions, RunOutcome};
use flm_core::progress::{
    estimate, measure, Expected, Kind, LogLevel, LogRecord, LogSink, Observer, RowState, Sheet,
};

// The kind of the root row: a constant of the program.
const BUILD: Kind = Kind::new("myprogram.build");

// A log sink that prints the warnings and the errors.
struct PrintWarnings;

impl LogSink for PrintWarnings {
    fn enabled(&self, level: LogLevel) -> bool {
        level <= LogLevel::Warn
    }

    fn log(&self, record: &LogRecord<'_>) {
        if self.enabled(record.level) {
            eprintln!("{}: {}", record.source, record.text);
        }
    }
}

// The observer stamps each row with the time since the start of the build.
let origin = Instant::now();
let observer = Observer {
    row_started: Some(Box::new(move |_row| Some(origin.elapsed()))),
    row_ended: Some(Box::new(move |_row| Some(origin.elapsed()))),
    ..Observer::default()
};
let (sheet, root) = Sheet::start(&BUILD, Some("d.flm"), observer, Some(Arc::new(PrintWarnings)));

let mut run = Run::new(Arc::clone(&doc), Arc::clone(&plan), Externals::none())?
    .with_progress(root.scope());
// Another thread may read `sheet` at any moment while the run executes.
let outcome = run.execute(&RunOptions::new())?;
root.finish();
assert_eq!(outcome, RunOutcome::Finished);

let snapshot = sheet.snapshot();
assert!(snapshot.rows.iter().all(|row| row.state == RowState::Finished));
assert_eq!(estimate(&snapshot, &Expected::new(), origin.elapsed()), 1.0);
// The durations of this build, to estimate the next build of the document with.
let _measured: Expected = measure(&snapshot);

// A build cancelled before its run starts: the run executes nothing.
let (sheet, root) = Sheet::start(&BUILD, Some("d.flm"), Observer::default(), None);
sheet.cancel();
let mut run = Run::new(doc, plan, Externals::none())?.with_progress(root.scope());
assert_eq!(run.execute(&RunOptions::new())?, RunOutcome::Cancelled);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### A host with one thread, such as WebAssembly

A program that runs the library compiled to WebAssembly usually has one thread:
while a call into the library runs, the program can act only inside a function
that the library calls. Such a program does its display inside the observer. In
`checkpoint`, it reads its own clock and, when enough time has passed since its
last display, takes a snapshot, estimates the fraction done and shows it. It may
also decide there to cancel the build, which the work sees at the question that
follows. `row_ended` is the place to finish what the display of a row needs, such
as a progress bar.

In a browser, a page repaints only between two of its own calls, so a long call
into the library shows nothing on the page while it runs. Run the library in a
web worker, and post each text from the observer to the page, which shows it.

On WebAssembly, a panic stops the whole module: no row ends as `Failed`, and the
sheet cannot be read after it. The [panics chapter](crate::guide::panics) lists
every item of the public interface that can panic.

### The layout of a build in the host library

The host library `flm-host` ([The host library, for a program that reads
files](#the-host-library-for-a-program-that-reads-files)) reports on the same
sheet. Its two calls `prepare` and `build` (module `flm_host::session`) each
receive the scope under which they report, so that one preparation can serve many
builds, each with a sheet of its own. `prepare` reports in a row
`flm.host.prepare` ("Preparing"), with the parts `flm.host.packages` ("Loading
packages"), `flm.host.registry` and `flm.host.plan`; `build` reports in a row
`flm.host.parse` ("Reading document"), then a row `flm.host.run` ("Processing
document") under which the run reports. A call whose build was cancelled returns a
`PrepareError` of the kind `PrepareErrorKind::Cancelled`, with no problem, the
diagnostics found so far, and the paths read.

The `flm` program and the live preview lay out the sheet of one build this way:

1. the root row, of the program's own kind;
2. under it, a row of the kind `flm_host::session::BUILD_KIND` (`flm.host.build`),
   declared before `prepare` is called and started once `prepare` has returned;
3. `prepare` reports under the root's scope, and `build` under the scope of the
   `flm.host.build` row, which ends when `build` returns.

The `flm.host.build` row is declared before `prepare` runs because `build`
declares its rows only when it starts. Without the row declared in advance, the
sheet would hold only the rows of `prepare` while `prepare` runs: the estimate
would reach the whole build when `prepare` ends, and the display would stay at 99%
while the document is read and processed. With the row declared in advance, the
percentage covers the whole build from the start:

```ignore
let (sheet, root) = Sheet::start(&ROOT, None, observer, log_sink);
let progress = root.scope();
let build_row = progress.child(&flm_host::session::BUILD_KIND, None);
let prepared = prepare(request, files, &[], &progress)?;
let build_task = build_row.start();
let built = build(&prepared, &build_task.scope())?;
build_task.finish();
root.finish();
```

The host library also provides what these two programs show, in its module
`flm_host::report`: `expected_durations()` returns the table of expected
durations shipped with the host library, measured on sample documents, for the
rows its builds declare on those documents (a kind not in the table gets the
median weight of the rows beside it); `ShownFraction` keeps the highest fraction estimated
for one build, so that a display never shows a decrease; `progress_line` returns
the text of the progress line (`42% Converting graphics figure-3.pdf`); and
`Record::progress` returns the same state as a record that serializes to JSON
([the progress line and the `progress`
records](crate::guide::command_line#progress-of-a-build)).

## Options

An option type is a plain Rust struct a feature declares once; its values are set
on a document or on a render request (the type's binding says which), and a stage
reads the value for an option type it declared a read of: the render value if one
is set, otherwise the document value, otherwise the type's default
([`model::options`](crate::model::options)). The Rust type is the option's
identity, so options need no names of their own.

A program sets option values in an
[`OptionValues`](crate::model::options::OptionValues) collection, in one of two
places: on the document, with
[`Doc::with_options`](crate::model::doc::Doc::with_options), or on the render
request, in the plan request's
[`render_options`](crate::core::plan::PlanRequest#structfield.render_options)
(for a workflow, in the `render_options` of its [`Overrides`](crate::core::plan::Overrides)).
The option type's binding ([`Bind`](crate::model::options::Bind)) says which
places admit a value: the document
([`Bind::Document`](crate::model::options::Bind::Document)), the render request
([`Bind::Render`](crate::model::options::Bind::Render)), or either
([`Bind::Either`](crate::model::options::Bind::Either)), in which case a value
set on the render request replaces the document's for that render. A value set
in a place the binding does not admit is refused, never ignored: when the plan is
built for a render value, and when the run is created for a document value
([`OptionError`](crate::model::options::OptionError)).

A fourth binding, [`Bind::Registry`](crate::model::options::Bind::Registry),
marks a feature's configuration: no value may be set on the document or on the
render request, because the value is fixed when the registry is built. The
sixteen configurations of [`StdlibConfig`](crate::stdlib::StdlibConfig) have
this binding.
[`FlmRegistry::options`](crate::core::registry::FlmRegistry::options) lists every
option type of a registry with its binding.

Most option types of the standard features are set per render request. Three of
them are:

- [`TextRenderOptions`](crate::stdlib::backends::text::TextRenderOptions): the
  text backend's line width, the stop after a run-in heading's title, the heading
  level printed as a first-level heading, and whether a link prints its address;
- [`RefsRenderOptions`](crate::stdlib::features::refs::RefsRenderOptions):
  whether a reference that points at no label is an Error (the default) or a
  Warning, in a plan that renders; under the `check` workflow it is always a
  Warning;
- [`AnnotationsRenderOptions`](crate::stdlib::features::annotations::AnnotationsRenderOptions):
  `hidden` (off by default) leaves every annotation out of every output, so that a
  highlight shows its passage alone and a remark shows nothing. A host uses it to
  produce a clean copy of an annotated document.

Two fields of `TextRenderOptions` decide what plain text shows of a link to an
address outside the document. `link_addresses` (on by default) prints the
address after the link's text. `link_addresses_when_shown` (off by default) also
prints it after a link whose text already shows the address, such as a `\url` or
an `\email`; it has no effect while `link_addresses` is off. For
`\href{https://example.com}{Click here}`, `\url{https://example.com}` and
`\email{a@b.c}`, the text output prints:

| `link_addresses` | `link_addresses_when_shown` | printed |
|---|---|---|
| on (default) | off (default) | `Click here <https://example.com>`, `example.com`, `a@b.c` |
| on | on | `Click here <https://example.com>`, `example.com <https://example.com>`, `a@b.c <mailto:a@b.c>` |
| off | off or on | `Click here`, `example.com`, `a@b.c` |

```rust
use flm_core::model::options::OptionValues;
use flm_core::stdlib::backends::text::TextRenderOptions;

// Links print their text alone.
let options = OptionValues::new()
    .with(TextRenderOptions { link_addresses: false, ..TextRenderOptions::default() });
# let _ = options;
```

[`NumberingOptions`](crate::stdlib::features::numbering::NumberingOptions) is
the only option type of the standard features that either place may set. A counter is
a dotted name under which things are counted in document order, declared by a
feature together with the counter it is numbered within
([`CounterDecl`](crate::stdlib::features::numbering::CounterDecl)). For each
counter it mentions, `NumberingOptions` says which counter it is numbered within
and the value it has before it is first advanced. A counter is written with its
full dotted name, prefixed by the feature that declares it:
`flm.headings.section` to `flm.headings.subparagraph` for the heading kinds,
`flm.math.equation` for the lines of displayed equations:

```rust
use flm_core::model::options::OptionValues;
use flm_core::stdlib::features::numbering::{NumberingOptions, Within};

let options = OptionValues::new().with(
    NumberingOptions::new()
        .within("flm.math.equation", Within::parent("flm.headings.section"))
        .initial("flm.headings.section", 0),
);
# let _ = options;
```

Set on the document, the numbering options say how that document is numbered
wherever it is rendered; set again on a render request, they replace the
document's for that render. A counter that the options do not mention keeps what
its feature declared. An entry of the numbering options for a counter that no
feature declares changes nothing and is reported as a Warning; so is an entry
that numbers a counter
within an undeclared counter, and the counter then keeps the place it was
declared with. A reference point whose number needs a counter that has no value
yet (a subsection before any section) is not numbered and is reported as an
Error, once per counter and run. `initial` gives such a counter a value, and
[`NumberingOptions::within_nothing`](crate::stdlib::features::numbering::NumberingOptions::within_nothing)
numbers a counter within no other counter.

Whether something is numbered at all is not one of these options. It is the
configuration of the feature that owns the numbered thing:
[`HeadingsConfig::numbering`](crate::stdlib::features::headings::HeadingsConfig::numbering)
for headings, none numbered by default;
[`MathConfig`](crate::stdlib::features::math::MathConfig) for equations,
numbered by default; [`ListsConfig`](crate::stdlib::features::lists::ListsConfig)
for the items of each list kind, those of `enumerate` numbered by default. Each
list numbers its items from one: every list environment is a scoped instance of
its list kind's counter ([scoped
instances](crate::model::aspects::NumberSource#scoped-instances)). A document's
numbering options may number the outermost lists within a section, as they
number any counter (`flm.lists.enumerate`). How a number looks is not one of these options
either: it is the counter styles that features declare, which a host replaces
with [`NumberingConfig`](crate::stdlib::features::numbering::NumberingConfig)
when the registry is built ([counters and counter
styles](crate::guide::concepts_overview#counters-and-counter-styles); [the
numbering guide](crate::guide::numbering)).

## Services the host provides

A service is a value that can be shared across threads (behavior behind a trait
object, or plain data) which one feature supplies for another feature, or for the
embedding program, to use ([`core::services`](crate::core::services)). A service
slot is the declaration, by the feature that owns an interaction, of a dotted
name, the type of the values the slot takes and its arity: at most one value, or
any number ([`Arity`](crate::core::services::Arity)). Any feature may contribute
a value to a service slot, and so may the embedding program, through the registry
builder; the registry build assembles all contributions once ([services and
service slots](crate::guide::concepts_overview#services-and-service-slots)).

A host contributes on its own behalf rather than a feature's:
[`RegistryBuilder::contribute`](crate::core::registry::RegistryBuilder::contribute)
finds the service slot by the value's type, and
[`contribute_to`](crate::core::registry::RegistryBuilder::contribute_to) takes
the service slot's name. A host's contribution comes after every contribution of
a feature. This is how a host supplies an implementation that a feature left to
the embedding program.

Services are fixed when the registry is built: every run shares them, and no
service slot receives a value during a run. Data that exists only once some work
has been done, such as an index built by scanning a collection of documents, is
not a service but an external table (see [External tables and two-run
composition](#external-tables-and-two-run-composition)).

The standard features declare several service slots, and fill most of them
themselves; the LaTeX backends, for example, fill the service slots of the LaTeX
package definitions and definition pieces
([`crate::stdlib::backends::latex::packages`],
[`crate::stdlib::backends::latex::preamble`]). Your program fills three groups of
service slots: the naming scheme of the output's files, `flm.artifacts.namer`
([`NAMER_SLOT`](crate::stdlib::backends::lowering::NAMER_SLOT), one
[`ArtifactNamer`](crate::core::artifact::ArtifactNamer) at most, which a host
that writes files contributes); the three services of images; and the citation
providers.

A program built on the `flm` program's host library, `flm-host`, gives two more things
that are not service slots, as fields of its request (`flm_host::session::Request`):
what downloads a file from the network, `Request::downloads`, and the citations' access
to the machine, `Request::citations`. Both are properties of the program, never settings
of a document (see [Host services for extensions](#host-services-for-extensions) for the
downloads).

### Image services: resolver, inspector and converters

The `graphics` feature reads no image file. Three services do the work that
needs files or programs, and a host contributes them; the feature requires none
of them ([`graphics`](crate::stdlib::features::graphics)).

**The graphics resolver**, in the service slot `flm.graphics.resolver`
([`RESOLVER_SLOT`](crate::stdlib::features::graphics::RESOLVER_SLOT), one value
at most), finds the file an image's path refers to and reads it. Your
[`GraphicsResolver`](crate::stdlib::features::graphics::GraphicsResolver)
receives the path as the author wrote it and the origin of the source in which
the image is written, from which a relative path is found. It returns an
[`ImageFile`](crate::stdlib::features::graphics::ImageFile): the file's identity
in your own form (two images that refer to one file have one identity), its
format as a media type, and the address an output writes to reach the file where
it is. It then reads the file's content whenever a stage asks for it. You give it
the rest of what it needs, such as the folders to search, when you construct it.
A file it cannot find or read is reported as an Error that gives the path. A path
that begins with a scheme (`https://…`, `data:…`) is never handed to it, and is
written as the author wrote it.

**The inspector**, in the service slot `flm.graphics.inspector`
([`INSPECTOR_SLOT`](crate::stdlib::features::graphics::INSPECTOR_SLOT), one value
at most), reports what an image file states: whether it is a vector or a raster
image, its size in pixels, its resolution and its physical size. Your
[`GraphicsInspector`](crate::stdlib::features::graphics::GraphicsInspector)
receives one request per image
([`GraphicsInspectRequest`](crate::stdlib::features::graphics::GraphicsInspectRequest)):
the path as written, the origin of the source in which the image is written, and,
where a graphics resolver is registered, the file the resolver found and its
content. The inspector measures that content. It returns what the file states,
or an error in its own words, which the run reports with the path.

The inspector is asked again wherever content holding an image is shown a second
time, such as a heading's title that a reference shows. What it reports there is
reported again, with the place where the copy is shown, as everything reported
over such a copy is ([reports over content shown
again](crate::guide::rendering#reports-over-content-shown-again)). An inspector
that is slow to run may therefore keep the results it has already returned. Where
a raster image states no resolution, 96 dots per inch is assumed, with one
Warning per image; where it states no physical size, the physical size is
computed in points from its size in pixels and its resolution. The table `flm.graphics.info`
([`INFO`](crate::stdlib::features::graphics::INFO)) holds the record of each
image, keyed by the image's node.

```
use std::sync::Arc;

use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::backends::pres::{Length, LengthUnit};
use flm_core::stdlib::features::graphics::{
    Dimensions, GraphicsInspectError, GraphicsInspectRequest, GraphicsInspectedInformation,
    GraphicsInspector, INSPECTOR_SLOT,
};

/// An inspector that knows one picture: `logo.svg`, two inches by one.
struct Logo;

impl GraphicsInspector for Logo {
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
        if request.path != "logo.svg" {
            return Err(GraphicsInspectError::new("no such file"));
        }
        let inches = |amount: i32| Length::new(amount.into(), LengthUnit::Inches);
        Ok(GraphicsInspectedInformation::Vector {
            physical_dimensions: Dimensions { width: inches(2), height: inches(1) },
        })
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.contribute_to(INSPECTOR_SLOT, Arc::new(Logo) as Arc<dyn GraphicsInspector>);
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

Without an inspector, the library knows no image's size: each output gives an
image the size its options give and, where they give none, writes no size, so
that the image is shown at its own size, and reports the Warning
`flm.graphics.unknown-size` at the image. Without any of the three services, the
library knows only each image's path, and every output writes the path as the
author wrote it.

**The converters**, in the service slot `flm.graphics.converters`
([`CONVERTERS_SLOT`](crate::stdlib::features::graphics::CONVERTERS_SLOT), any
number, asked in order), turn the content of an image of one format into another
format. A
[`GraphicsConverter`](crate::stdlib::features::graphics::GraphicsConverter) has a
name, says which pairs of formats it can convert, and converts content. Its method
`convert` also receives the scope of the conversion's row on the build's progress
sheet (`flm.graphics.convert`, "Converting graphics"), under which a converter with
slow parts reports them and checks whether the build was cancelled ([reporting
progress from a stage or a
service](crate::guide::extending#reporting-progress-from-a-stage-or-a-service)).

With a graphics resolver, each output **publishes** its images according to its
strategy, a render option of the `graphics` feature
([`GraphicsRenderOptions`](crate::stdlib::features::graphics::GraphicsRenderOptions))
given per output:

- `reference`, the default: the output writes the address the resolver gave;
- `copy`: a copy of each file becomes an artifact of the output, and your
  [`ArtifactNamer`](crate::core::artifact::ArtifactNamer) says where it goes;
- `data-uri`: the file's content is written into the address;
- `content`: the output writes the address the resolver gave, and the file's
  content is kept in a table for an output that renders images itself.

Under the last three strategies, the conversion rules that the same render option
gives per output (which format to turn into which) turn an image of one format
into another, such as a PDF figure into a PNG image for the page, through
the converters you contribute. [Images and how an output reaches
them](crate::guide::rendering#images-and-how-an-output-reaches-them) describes
what each output then writes. The same mechanism serves any feature of your own
that leaves something to its host ([Extending FLM](crate::guide::extending)).

### The citation providers

The citation providers, in the service slot `flm.citations.providers`
([`PROVIDERS_SLOT`](crate::stdlib::features::citations::PROVIDERS_SLOT), any
number), give the library the citation entry of each cited work: a document
writes citation keys only. The providers are asked in turn: first those that features
contribute, the feature placed last in the feature order first, then yours, in the
order in which you contributed them:

- The first provider receives, in one call, every distinct citation key the
  document cites, in order of first appearance
  ([`CitationProvider::answer_all`](crate::stdlib::features::citations::CitationProvider::answer_all)),
  each key as the author wrote it
  ([`CitationRequest`](crate::stdlib::features::citations::CitationRequest)),
  with a context that holds the registry's language and the scope of the
  retrieve stage's row on the build's progress sheet
  ([`ProviderCx`](crate::stdlib::features::citations::ProviderCx);
  [`ProviderCx::progress`](crate::stdlib::features::citations::ProviderCx::progress)),
  under which a provider that fetches over the network counts the keys it fetches
  and checks whether the build was cancelled.
- Each later provider receives, in one call, the keys that every provider before
  it declined.

A provider that handles one key at a time implements
[`answer`](crate::stdlib::features::citations::CitationProvider::answer) alone;
the default `answer_all` then calls it key by key. A provider that fetches many
keys at once, such as with one request to a server for a hundred keys, implements
`answer_all` too. `answer_all` returns one result per request, in the order of
the requests; when the number differs, every key of the call is reported as
failed ([`ProviderFailed`](crate::stdlib::features::citations::ProviderFailed))
and no later provider is asked about them.

For each key, a provider returns a
[`CitationAnswer`](crate::stdlib::features::citations::CitationAnswer), "not
mine" (`Ok(None)`), or an error in its own words, which ends the lookup of that key
and is reported. The answer holds:

- the work's **citation entry**, inline content, which
  [`ProviderCx::parse_entry`](crate::stdlib::features::citations::ProviderCx::parse_entry)
  parses from FLM source and
  [`ProviderCx::build_entry`](crate::stdlib::features::citations::ProviderCx::build_entry)
  builds from parts (characters, FLM source, emphasis, strong importance, a link:
  [`EntryPiece`](crate::stdlib::features::citations::EntryPiece));
- optionally, the work's **identity**: one spelling of the citation key that
  every key referring to the same work shares, such as `arxiv:1211.1037` for both
  `arXiv:1211.1037` and `arxiv:1211.1037`. Keys given one identity are one
  cited work, with one number and one citation entry in the list. An answer with
  no identity has the key as written as its identity. Only a provider can know
  that two keys refer to one work: the library reads no meaning into a key.
- optionally, **warnings**
  ([`CitationAnswer::with_warning`](crate::stdlib::features::citations::CitationAnswer::with_warning)):
  sentences that tell the author something about the key, for example that two
  keys cited as two works refer to the same work. Each is reported as a Warning
  at the key's first citation, and the citation entry is shown as returned.

The library performs no input or output: a provider that reads a database or asks
a server does so as your service. Every run asks the providers again (the
`check` workflow too, because every workflow checks that each cited key has a
citation entry), so a provider that is slow to run should keep the results it has
already returned. Without a provider, a run of a document that cites works
reports an Error (`flm.citations.no-provider`) at the first citation, and no
cited key has a citation entry. The table `flm.citations.entries`
([`CitationEntries`](crate::stdlib::features::citations::CitationEntries)) holds
what the providers returned.

```
use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::lang::{FlmOrigin, FlmSource};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::stdlib::features::citations::{
    CitationAnswer, CitationProvider, CitationRequest, CitationResult, EntryPiece, ProviderCx,
    PROVIDERS_SLOT,
};

/// The citation entries of a small bibliography, as FLM source.
struct FromMap(BTreeMap<String, String>);

impl CitationProvider for FromMap {
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
        let Some(text) = self.0.get(&request.key) else { return Ok(None) };
        let source = FlmSource::new(text.as_str()).with_origin(FlmOrigin::new("refs.flm"));
        cx.parse_entry(Arc::new(source)).map(|entry| Some(CitationAnswer::new(entry)))
    }
}

/// Every key `doi:…`, handled in one call; one work regardless of the letter
/// case of its DOI; its citation entry built from parts.
struct Dois;

impl CitationProvider for Dois {
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
        self.answer_all(std::slice::from_ref(request), cx).remove(0)
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &ProviderCx<'_>) -> Vec<CitationResult> {
        // A real provider would send one request to a server for all of `requests` here.
        requests
            .iter()
            .map(|request| {
                let Some(doi) = request.key.strip_prefix("doi:") else { return Ok(None) };
                let entry = cx.build_entry(&[
                    EntryPiece::text("An article, "),
                    EntryPiece::Link {
                        address: format!("https://doi.org/{doi}"),
                        content: vec![EntryPiece::text("DOI")],
                    },
                ])?;
                Ok(Some(CitationAnswer::new(entry).with_identity(format!("doi:{}", doi.to_lowercase()))))
            })
            .collect()
    }
}

let entries = BTreeMap::from([
    ("knuth84".to_string(), "Knuth, \\emph{The Art of Computer Programming}, 1984".to_string()),
    ("lamport94".to_string(), "Lamport, 1994".to_string()),
]);
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.contribute_to(PROVIDERS_SLOT, Arc::new(FromMap(entries)) as Arc<dyn CitationProvider>);
builder.contribute_to(PROVIDERS_SLOT, Arc::new(Dois) as Arc<dyn CitationProvider>);
let registry = builder.build(&RegistryInputs::new())?;
# let _ = registry;
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

### Host services for extensions

An extension written in JavaScript runs in a sandbox that reaches no file, no network and
no program (see [Extensions](crate::guide::extending#extensions) in the guide to extending
FLM). What its handlers need from outside, your program offers as *host services*: named
objects added to `flm-ext`'s loader, which a package of an extension declares under the
key `host` of its package object and its handlers call through `cx.host(name)`. A host
service implements the trait `flm_ext::host::HostService`: a name, and
`call(caller, method, args, progress)`, which receives the calling extension and package
(`flm_ext::host::HostCaller`, with the bytes the extension's digest is computed from, for
a service that decides by the extension's contents), a method's name, its arguments as
JSON and the scope of the handler call on the build's progress sheet (`Scope::none()`
from a template module), and answers data (`HostAnswer::Data`), bytes for a Rust caller
(`HostAnswer::Bytes`), or a refusal with a complete text for a person
(`HostServiceError`), which the handler receives as an error. A service is shared by
every thread that runs a stage, so it is `Send + Sync`.

`ExtensionLoader::with_host_service(Arc<dyn HostService>)` offers a service under its
name, and `host_services()` lists the names. A package that declares a service the loader
does not offer fails its load with an error that lists the services offered; the
declaration enters the registry digest; and a handler reaches only the services its
package declared.

`flm-ext` defines the protocols of two services: `download` (`flm_ext::host::download`),
the file at an `https` address, through which a template module ships such a file; and
`files` (`flm_ext::host::files`): the
methods `read_text` and `read_bytes`, each with the arguments `{ "path" }`, a path with a
root written `ROOT:PATH` (`flm_ext::paths`), answering the file's text or bytes, or `null`
when nothing is at the path. `flm-ext` serves two roots itself, and neither reaches your
object: `self:`, the extension's own files, from the files your source gave the loader,
and `ext+NAME:`, the files of the extension that the use name `NAME` found, from the
extensions the loader loaded. It also refuses a path with no root and an `https:`
address. Your object serves the roots `std:` (your program's own files), `doc:` (the
document's folder; an absolute path arrives as `doc:` followed by itself) and `cwd:` (the
folder in which the program runs). Join a path of `doc:` to the document's folder as a path
written in the document is (an absolute path, `~` and `..` included), and read through the
file reader your program records a build's files with, so that a live preview watches the
files an extension read. A program that also reads `ext+NAME:` itself (a template engine)
gives the loader its own table with `ExtensionLoader::with_extensions_by_name`. In memory,
with a map from a path to a text standing for the document's files (`flm-ext` is not a
dependency of this guide's crate, so the example is not compiled):

```ignore
use std::collections::BTreeMap;
use std::sync::Arc;
use flm_core::progress::Scope;
use flm_ext::host::{files, HostAnswer, HostCaller, HostService, HostServiceError};
use flm_ext::ExtensionLoader;
use serde_json::Value;

/// The document's files as a map from a path, relative to the document's folder, to a text.
struct MemoryDocumentFiles(BTreeMap<String, String>);

impl HostService for MemoryDocumentFiles {
    fn name(&self) -> &str {
        files::NAME
    }

    fn call(&self, _caller: &HostCaller, method: &str, args: &Value, _progress: &Scope) -> Result<HostAnswer, HostServiceError> {
        let written = args["path"].as_str().unwrap_or_default();
        let Some(path) = written.strip_prefix("doc:") else {
            return Err(HostServiceError { detail: format!("Unsupported path ‘{written}’: this program reads the root ‘doc’ alone") });
        };
        Ok(match (method, self.0.get(path)) {
            (_, None) => HostAnswer::Data(Value::Null),
            (files::READ_TEXT, Some(text)) => HostAnswer::Data(Value::String(text.clone())),
            (files::READ_BYTES, Some(text)) => HostAnswer::Bytes(text.clone().into_bytes()),
            (other, _) => return Err(HostServiceError { detail: format!("the method of the host service ‘files’ must be ‘read_text’ or ‘read_bytes’, not ‘{other}’") }),
        })
    }
}

let document = BTreeMap::from([("bib.json".to_string(), "{}".to_string())]);
let loader = ExtensionLoader::new(sources)
    .with_host_service(Arc::new(MemoryDocumentFiles(document)));
```

The `flm` program's host library, `flm-host`, offers the services `files` and `cache` on
the loader of every preparation (`flm_host::packages::loader`, which receives the
preparation's reader of qualified paths and its handle of the caches, `Roots` and
`ServiceCaches`), the service `download` when the request gives a downloader (see below),
and the service `programs` when the request asks for it (cargo feature `fs`). `files` (`flm_host::host_files::FilesService`) serves the roots `std`, `doc` and
`cwd`; it reads the document's folder, the folder of the input file, every path joined to
it as a path written in the document is, through the reader of the build under way, so
that each
file read is among the files of the build and the live preview builds again when it
changes. `programs` (`flm_host::host_programs::ProgramsService`) finds a program
installed on the machine (`find`, by a list of names, in the folders of `PATH` and in
further folders that may hold `*`) and runs it in a fresh temporary folder (`run`, with
its arguments, the files written into the folder first, its standard input and a time
limit, answering its exit status, its two outputs and the files read back from the
folder, and links into that folder the cache folders that the caller's package declares,
the argument `cache` of `run`); the module's documentation gives both shapes. `cache`
(`flm_host::host_cache::CacheService`) reads and stores the values of the caches that a
package declares under the key `caches` of its package object, with the methods `get`,
`put` and `remove`, in the caches of the request's manager (see [Caches in the host
library](#caches-in-the-host-library)); its module's documentation gives the arguments
and the answers. A program embedding `flm-host`
offers `programs` only when it sets `Request::offer_programs`, which the `flm` program
always does: an extension's handler may then run any program with the rights of the user
running your program, which matters most for the extensions that come with a document,
in its folder `_flmextensions`. Such a program must ignore the signal `SIGPIPE`, as Rust
programs do, or a program that exits without reading the standard input it is given
ends it. On Unix, the service starts each program in a process group of its own, so that
a program past its time is stopped with the programs it started. The signals sent to your
program's process group, such as the interrupt of Ctrl+C at the terminal, the quit signal
of `Ctrl+\`, the hang-up of a closed terminal or `SIGTERM`, do not reach such a group, so a
program that ends on such a signal calls `flm_host::host_programs::stop_running_programs`
first, as the `flm` program does, or the programs that the service is running go on after
it.

**The roots that a host gives.** `flm-host` reads every path with a root of one
preparation through one reader, `flm_host::roots::Roots`, which the service `files`, the
template engine `minijinja` and the citations share. The reader is made when the
preparation starts, from four things the host gives: the table of the extensions loaded
by their use names (`flm_ext::paths::ExtensionsByName`, filled by the loader), for
`ext+NAME`; the program's own files, for `std`; the file reader of the build under way,
through which `doc` and `cwd` are read, so that each read is recorded among the build's
files; and the document's folder, the empty path for a text given as the input. With
`Roots::with_downloads`, it reads `https` through the program's downloads. `self` never
reaches the reader: the caller replaces it with the root of the place of the text that
writes it (`flm_ext::paths::WrittenPath::at`).

**The download that a program gives.** `flm-host` downloads a file from an `https` address
only through the downloader that your program gives in `Request::downloads`, a
`flm_host::download::Downloads`. By default it gives none: every download is then refused
(a compose template's `https` path stops the render), and the loader does not offer the
service `download`, so a package that declares that service does not load. To give one:

- under the cargo feature `download` of `flm-host`,
  `Downloads::new(flm_host::download::network_downloader())` downloads over the internet,
  with a limit of 60 seconds per request, 32 MiB and five redirects, each to an `https`
  address, and with the proxy given by the environment variables `ALL_PROXY`,
  `HTTPS_PROXY` and `HTTP_PROXY`. The `flm` program gives this downloader in every request;
- otherwise, `Downloads::new(Arc::new(MyDownloader))` takes your own implementation of
  the trait `flm_host::download::Downloader`, whose one method `get(url)` requests the
  address once and returns the bytes and the media type, or the text of the failure. A
  test gives a stand-in that answers from memory, so that it opens no connection.

`Downloads::get` keeps each file in the request's cache for seven days
(`flm_host::download::CACHE`, shared by documents), answers a kept file without a request,
and answers the kept copy when a later download fails, with a warning once the copy is
past its expiry. The clones of one
`Downloads` share the record of the addresses whose download failed, which are not
requested again for five minutes (`RETRY_AFTER`). With a downloader given, the loader of
every preparation offers the service `download` (`flm_host::host_download::DownloadService`),
whose protocol is `flm_ext::host::download`. The citations fetch their records with their
own fetcher, given in `Request::citations`, and never through `Request::downloads`.

A program built on `flm-ext` without `flm-host` offers the services it registers, and no
other. An extension whose package declares a service the program does not offer does not
load, in any mode of the extension. The extension `math-mathjax`, shipped with FLM,
declares the service `cache`: such a program offers a service named `cache` to load it,
at least one that answers `null` to every call, which `math-mathjax` reads as a value
not found in the cache.

## External tables and two-run composition

An external table is a table that no stage of the run writes: the caller installs
its value when the run is constructed
([`Externals`](crate::core::run::Externals)). External tables bring data from
outside one document to the stages of a run.

The standard use is a **corpus**: several documents processed separately, whose
references to each other must resolve ([documents, fragments, and the
corpus](crate::guide::concepts_overview#documents-fragments-and-the-corpus)). The
`refs` feature declares the external table
[`EXTERNAL_INDEX`](crate::stdlib::features::refs::EXTERNAL_INDEX)
(`flm.refs.external-index`), an
[`ExternalIndex`](crate::stdlib::features::refs::ExternalIndex) that maps a
label of *another* document to an
[`ExternalTarget`](crate::stdlib::features::refs::ExternalTarget): the link
target as you want it written, and the content a reference to it shows. The
`refs` feature's resolve stage reads the table when it has a value; a run without
it resolves no reference to another document.

A corpus is therefore processed in two runs per document: run each document once
and read the labels it defines and their anchors from the tables
[`LABELS`](crate::stdlib::features::refs::LABELS) (`flm.labels`) and
[`ANCHORS`](crate::stdlib::features::refs::ANCHORS) (`flm.anchors`); build an
`ExternalIndex` from them
([`ExternalIndex::with`](crate::stdlib::features::refs::ExternalIndex::with));
then run each document again with the index installed by
`Externals::none().install_unstamped(EXTERNAL_INDEX, index)`. The resolve stage
of the `refs` feature looks a label up in the index only when no reference point
of the document carries it ([`RefPoint`](crate::model::aspects::RefPoint)). The
example below installs a table of your own in the same way.

```rust
use flm_core::core::run::Externals;
use flm_core::core::data::table::{Table, TableName};

#[derive(Debug, Default)]
struct KnownLabels(Vec<String>);

impl Table for KnownLabels {
    fn absorb(&mut self, piece: KnownLabels) {
        self.0.extend(piece.0);
    }
}

const KNOWN: TableName<KnownLabels> = TableName::new("example.known-labels");

// Data that did not come from a run of this library: nothing checks it
// against the document, and the caller is responsible for it.
let externals = Externals::none()
    .install_unstamped(KNOWN, KnownLabels(vec!["thm:main".to_string()]));

assert_eq!(externals.len(), 1);
```

[`Externals::install_unstamped`](crate::core::run::Externals::install_unstamped)
installs data that did not come from a run of this library.
[`Externals::install`](crate::core::run::Externals::install) installs a value
that an earlier run of this library produced, together with that run's
[`Stamp`](crate::core::run::Stamp): the registry digest, the rewrite stages that
ran, and a digest of the option values those rewrite stages read.
[`Run::new`](crate::core::run::Run::new) compares the stamp with the stamp of the
run being constructed and refuses a mismatch, so data computed under one reading
of a document is never used under another without notice. Read a run's stamp
with [`Run::stamp`](crate::core::run::Run::stamp).

[`Run::new`](crate::core::run::Run::new) checks the installed values as [What a
run refuses](#what-a-run-refuses) lists;
[`Plan::externals`](crate::core::plan::Plan::externals) lists the external tables
a plan needs, so a program can check them before constructing the run.

## Inspecting, debugging and testing

[`crate::dump`] renders what the library holds in memory (parsed and processed
trees, computed tables, diagnostics) for inspection and for tests. It is ordinary
public interface, not test-only code: no cargo feature switches it on, so a
program that embeds the library writes its test fixtures against the same
functions the library's own tests use. It renders in two forms, which it keeps
apart.

**The projection** ([`crate::dump::project`]) is a one-way rendering of a tree, a
node, a table or a diagnostic as a serialized value, in a vocabulary this library
owns and keeps stable: `elem`, `chars`, `args`, `children`, `body` and `payload`
among its keys, and spans as `line:col` on request. It never shows a parsing state
or a node identifier, and it cannot be read back. Tests compare it, and the `flm`
program's `flm dump --view compact` prints it.

```rust
use flm_core::dump::project::{project_tree, ProjectOptions};
use flm_core::dump::to_json_pretty;
# use std::sync::Arc;
# use flm_core::model::doc::FragmentLayout;
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(FlmSource::new("A \\emph{word}.").with_origin(FlmOrigin::none()));
let fragment = parse_fragment(
    registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
)?;

let value = project_tree(fragment.tree(), &ProjectOptions::new());
let json = to_json_pretty(&value)?;
assert!(json.contains("flm.textstyle"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use a [`Projector`](crate::dump::project::Projector) directly to project more
than one thing: one projector serves any number of trees, nodes, tables and
diagnostics, and keeps the line positions it computes.
[`Projector::table_projection`](crate::dump::project::Projector::table_projection)
gives the context through which a table projects itself. A
[`Run`](crate::core::run::Run) is a
[`TreeSource`](crate::dump::project::TreeSource), so the node addresses a table
holds are resolved through the run's frozen trees.

**The wire form** ([`crate::dump::session`]) is techy's serialization of a parse:
complete, verbose, and readable back into objects in memory with their sharing
and identity intact. [`writer`](crate::dump::session::writer) and
[`reader`](crate::dump::session::reader) create the serialization sessions;
[`write_fragment`](crate::dump::session::write_fragment),
[`read_fragment`](crate::dump::session::read_fragment),
[`write_tree`](crate::dump::session::write_tree) and
[`read_tree`](crate::dump::session::read_tree) write and read objects through
them; [`segment_line`](crate::dump::json::segment_line) writes one segment of a
session's output as one line of JSON. Writing a fragment and reading it back keeps
the tree's structure, its spans and sources, the parsing states, the
specifications and the elements (by identity), the diagnostics with their
identifiers, data and counts, and the fragment's layout. It does not keep the
node identifiers and tree tags, which are new on reading, nor the Rust types of
the diagnostics' conditions: a condition is read back as techy's generic
condition, with the same identifier and data.

Two more modules serve tests: [`crate::dump::golden`] compares output with a file
on disk, and rewrites the file instead when the environment variable
`UPDATE_GOLDEN` is `1`; [`crate::dump::diag`] checks diagnostics by identifier and
data.

## Describing an environment: the crate `flm-info`

The crate `flm-info` answers questions about an *environment*: a registry, the plan of a
workflow, and the package factories (each package's features with the declarations of
their settings). It is what the `flm` program's command `flm info` is built on, and a
program that embeds FLM uses it to list the commands a document may write, to show how a
command is written and what its arguments hold, to list the settings a feature takes, or to
show what a workflow runs. The crate documentation of `flm_info`, in the same
documentation as this guide, describes each module; this section states what the crate
offers.

The crate is built in layers, each using the one before it:

- **The queries** (the module `env`). An `Environment` is made from a registry, an
  optional plan and the package factories (`Environment::new`). Its methods answer typed
  values, the *views*: `features()`, `command(kind, name)` (every declaration of a macro,
  an environment or a special character sequence, with its signature, its arguments with
  their roles and documentation, and the fields written after it), `settings(feature)`,
  `workflow(name)` (its outputs and, when the plan was built from it, the plan's stages in
  order), `namespaces()` and the entities of each, and others. These views are the
  primary interface of the crate.
- **The description** (the module `describe`). Every view serializes to JSON and has a
  JSON Schema; `describe::schema()` returns the schema of the whole description. The module
  states the rules of the JSON form: one object per view, a key with nothing to show left
  out, names as their dotted strings, enumerated values as the lowercase words of the
  projection of `flm dump`, and lists in the order of the registry. A program that writes
  the description puts a header first (`describe::Meta`: the kind of document, the
  program's version and the number of the form).
- **The pages** (the module `pages`). A `pages::Tree` arranges the description in pages of
  documentation, each with a path (`features/headings/macros/section`) and the list of the
  pages below it; `Tree::page(path)` gives one page, `Tree::lookup(name)` the pages of a
  name, `Page::to_json` writes a page as JSON, and `pages::source(page, options)` writes it
  as an FLM document in the markup of the extension `flmdoc`, for a program to render (see
  [Writing documentation pages](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc)).
  The crate does not render pages itself and does not depend on that extension.
- **The engine of `flm dump`** (the module `dump`), which processes one file with the
  library alone and returns one view of what the run holds.

Without cargo features, the crate depends on `flm-core` alone (without its default
features) and builds for WebAssembly. Its cargo feature `flm-host` adds the layer over the
host library: `Environment::of_prepared` makes the environment of a document that
`flm_host::session::prepare` prepared, and `Environment::of_request` that of a request
without a document. The views then also show what only the host knows (the module `host`):
the profile in view and the math processor, where each package comes from (with the
description, author and engine of an extension's manifest), the settings in effect with
where each value was written, the front matter's keys, and the extension search path.

The documentation texts the views show are kept by `flm-core`'s cargo feature `docs` (see
[Dropping the documentation texts](crate::guide::extending#dropping-the-documentation-texts-the-cargo-feature-docs)):
a program that shows them keeps that feature on through its own dependency on
`flm-core`.

```rust,ignore
// Cargo.toml: flm-core, flm-info and serde_json. This example is the crate
// documentation's example of flm_info, where it is tested.
use std::sync::Arc;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{PackageEntry, PackageFactory, RegistryBuilder, RegistryInputs};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::mint::CallableKind;
use flm_core::stdlib::{self, StdlibConfig};
use flm_info::Environment;
use flm_info::pages::{SourceOptions, Tree, source};

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;
let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;
let std = PackageFactory::new("std", |settings: &PackageSettings| {
    Ok(stdlib::add_features_from_settings(PackageEntry::new("std"), settings))
})?;
let factories = vec![std, flm_core::math_passthrough::factory()?];
let env = Environment::new(registry, Some(Arc::new(plan)), factories);

// A query: the declarations of `\section`, and the feature that makes each.
let section = env.command(CallableKind::Macro, "section");
assert_eq!(section.last().map(|view| view.feature.as_str()), Some("headings"));
assert!(env.workflow("text").and_then(|view| view.plan).is_some());
// Every view serializes to JSON.
let json = serde_json::to_value(&section)?;
assert!(json.is_array());

// A page, as JSON and as FLM source in the markup of the extension `flmdoc`.
let tree = Tree::new(&env, None);
let page = tree.page("features/headings/macros/section").expect("the page of `\\section`");
assert_eq!(page.to_json(false)["path"], "features/headings/macros/section");
let flm = source(&page, &SourceOptions::default());
assert!(flm.contains("\\docsignature"));
```

## Threads, the `parallel` feature, and WebAssembly

`flm-core` has two cargo features, both on by default: `parallel`, described here, and
`docs`, which keeps the documentation texts of the declarations (see
[Dropping the documentation texts](crate::guide::extending#dropping-the-documentation-texts-the-cargo-feature-docs)).
With `parallel`, the
work a run does for each tree (the rewrite stages over each tree, and each stage
whose scope is one tree, [`PerTree`](crate::core::stage::Scope::PerTree)) runs in
parallel on a thread pool. The pool's threads are created with
[`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack and
marked as worker threads as they start, so the guarded work on that path never
creates a thread of its own.

The parallel and the sequential paths produce **identical** results: the results
of a batch run in parallel are always collected in the same order. The library's
tests run under both paths, and again with the parallel path restricted to one
thread. Choose the sequential path for one run with
[`RunOptions::sequential`](crate::core::run::RunOptions::sequential), or for the
whole program by building without the feature.

The sequential path is always compiled, and it is the only path when the crate
is built with `--no-default-features`. The WebAssembly build uses that
configuration: the library's tests check that `flm-core` without its default
features compiles for `wasm32-unknown-unknown`. Nothing else changes there,
because the library performs no input or output in its processing: there is no
file, network or process access to arrange. A program that runs the library on
one thread shows the progress of a build from inside the observer of its progress
sheet ([A host with one thread, such as
WebAssembly](#a-host-with-one-thread-such-as-webassembly)).

On any platform where creating threads may fail, remember that the parse entry
points create a worker thread when the calling thread is not marked as one. If the
operating system refuses, the parse is not attempted and reports
[`WorkerThreadUnavailable`](crate::lang::parse::WorkerThreadUnavailable)
(`flm.lang.worker-thread-unavailable`), rather than running on a stack that the
recursion guard's budget could exceed. A host whose own thread has at least
`WORKER_STACK_SIZE` bytes of stack can mark it with
[`mark_worker_thread`](crate::lang::guard::mark_worker_thread), and no thread is
then created.

## The document's metadata

A document's **metadata** (its title, its authors, its date, and any other key
its author writes about it) is one option value bound on the document,
[`DocMeta`](crate::stdlib::features::metadata::DocMeta), which the `metadata` feature
declares. The title, the date and each author's name, affiliations and notes are
FLM text ([`MetaText`](crate::stdlib::features::metadata::MetaText)), each with the
place where it was written when it was read from a file, so that a problem in it is
reported there. An author
([`MetaAuthor`](crate::stdlib::features::metadata::MetaAuthor)) may also have an
email address, a plain text that is not parsed, and other keys, kept as data.

A host sets the value on the document it builds; the `flm` program reads it from
the document's front matter. A host that holds the metadata as data, a map from
each key to its value, reads it with
[`DocMeta::from_data`](crate::stdlib::features::metadata::DocMeta::from_data). Every
host uses the same form: `title` and `date` are one text each; `author` is one
author or a list of authors, an author being a text (the name) or a map holding
`name` and, where given, `affiliation` (one text or a list), `email` and `note`
(one text or a list); a number or a truth value is read as the text it is written
as; and every other key, of the metadata or of an author's map, is kept as data. A
map without `name` is an error. The host gives the place where each value is
written, when it knows it, by the value's path
([`MetaPath`](crate::stdlib::features::metadata::MetaPath), `author.0.affiliation`),
and receives each problem with the path of the value at fault.

```rust
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdlib::backends::text::TEXT_DOCUMENT;
use flm_core::stdlib::features::metadata::{DocMeta, MetaText};
use flm_core::stdlib::{self, StdlibConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(FlmSource::new("The content.").with_origin(FlmOrigin::none()));
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let data = serde_json::json!({
    "title": r"On \emph{flexible} markup",
    "author": [{"name": "Ada", "affiliation": "University of X"}, "Grace"],
});
let meta = DocMeta::from_data(data.as_object().expect("a map"), |_| None).expect("texts");
assert_eq!(meta.title, Some(MetaText::new(r"On \emph{flexible} markup")));
let doc = Arc::new(Doc::with_options(content, OptionValues::new().with(meta)));

let plan = Arc::new(Plan::for_workflow(&registry, "text", &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;
let set = run.table(TEXT_DOCUMENT)?.expect("the text output");
let text = String::from_utf8(set.entry().expect("an entry").as_bytes().expect("bytes").to_vec())?;
assert!(text.starts_with("On flexible markup\n\nAda\nUniversity of X\n\nGrace\n"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The title, the authors and the date are each a piece placed before the content
(`flm.metadata.title`, `flm.metadata.author`, `flm.metadata.date`). They are placed by default,
and a render request leaves any of them out as it leaves out any piece
([`PiecesRenderOptions`](crate::stdlib::backends::pieces::PiecesRenderOptions)).
A document with no metadata places none of them. On the text, HTML and
rendered-LaTeX outputs, each author is a group of lines: the name, each
affiliation, the email address as a link to `mailto:` and the address, and each
note, each on a line of its own and in a span of its own role
(`flm.metadata.author.name`, `flm.metadata.author.affiliation`,
`flm.metadata.author.email`, `flm.metadata.author.note`), which a stylesheet or a
compose template selects. On the recomposed LaTeX, each author is what a LaTeX
author writes in `\author`: the name, `\thanks{…}` for each note, then each
affiliation and the email address after `\\`, the authors joined by `\and`. The
library numbers the distinct affiliations of the document
([`AffiliationPayload`](crate::stdlib::features::metadata::AffiliationPayload)) for
a lowering rule that marks each author with numbers; its own rules write no
number. The library does not show the other keys
([`DocMeta::other`](crate::stdlib::features::metadata::DocMeta::other),
[`MetaAuthor::other`](crate::stdlib::features::metadata::MetaAuthor::other)).

## The host library, for a program that reads files

Everything above hands the library text and takes values back: the library reads
no file and writes none. A program that processes documents stored as files can
use the **host library** `flm-host` instead of writing that part itself. In
flm-core, a host is any program that embeds the library; the host library
`flm-host` is the one shipped with the project: it finds and reads a document,
reads its settings, chooses its packages and workflow, prepares the registry and
the plan, runs the library and writes the outputs. The `flm` command-line program
([the command-line guide](crate::guide::command_line)) is built on it. The crate
documentation of `flm_host`, in the same documentation as this guide, describes
each of its modules; this section states what the host library adds to
`flm-core`.

- **Reading files.** Every file is read through one service, which reads from the
  disk or from files held in memory, and which records the list of the files one
  build read (the module `files`).
- **The front matter.** The front matter is the block of YAML between a first line
  `---` and the next `---` line of a file: its root keys are the author's metadata
  (title, authors, date), and the reserved key `flm` holds the document's
  settings: `use`, `initial_use`, `math`, `features`, `parsing`, `feature_order`, `layout`
  and `default_profile` (the module `seed`, named after `Seed`, the value a front matter is
  read into). The host
  library reads the metadata from the root keys and sets it on
  the document (see [The document's metadata](#the-documents-metadata)).
- **The profile.** The request's field `profile` (else the front matter's
  `default_profile`, else `html`) gives the *profile*, the name that chooses the workflow
  that runs: the profile `NAME` runs the workflow `NAME`, and uses the extension
  `profile-NAME`, after the document's other packages, when one exists (the module
  `session`, "Which profile runs, and its workflow").
- **Extensions.** An extension is a YAML file `NAME.yaml`, or a folder
  `NAME/` holding `flmextension.yaml` and other files, found by name on the host's
  search path, that provides packages a document lists under `use`. The module
  `extensions` finds an extension in the folders of the search path and reads its
  files; the module `packages` makes the package catalog of one preparation, whose
  resolver is `flm-ext`'s loader over those folders. The loader also finds the
  extensions that the program offers itself, after the folders: those of the collections
  it loaded, and those compiled into it (the module `bundled`; see [The extensions a
  program offers](#the-extensions-a-program-offers)). Under the cargo
  features `js` and `wasm` (off by default in `flm-host`, on in the `flm` program), the
  loader has the engines `js` and `wasm`, configured by the request's `js_host` and
  `wasm_host`: the limits of the extensions' instances (for `wasm`, the memory and the
  fuel of a handler call). The request's `log_sink` receives the `console` output of the
  extensions of every engine, each line with the extension's name as its source; without
  it, the JavaScript and WebAssembly engines keep the sink their hosts were given (the
  default hosts have none), and the other engines' output goes nowhere. An extension's
  manifest may set the limits of its own instances (`extension.js.limits`,
  `extension.wasm.limits`).
- **Settings sources.** The host combines a document's settings from ranked
  sources (the program's defaults, the packages of the extensions the document
  uses, the front matter, the workflow, the command line), each value keeping a
  record of where it was written, through the library's settings cascade (the
  module `settings`).
- **The package `std`, the initial list and the math processor.**
  `std` holds the standard features, added by
  [`add_features_from_settings`](crate::stdlib::add_features_from_settings), then the
  host's own features (`compose`, `compose-standard`, `write`, `preview`,
  `graphics-files` and `citations-standard`); it uses no other package and processes no
  mathematics. A document takes the whole package `std` or leaves it out of its initial
  list, and leaves out one of its features by switching that feature off. A document
  uses the packages of an *initial list*, then the package of its *math processor*, then
  those its front matter lists under `use`, then those the request adds, each package
  after the packages it uses, and each once.
  - The host gives its initial list in the request (`Request::default_initial_use`, a
    `GivenList`: the packages, and the words that say where the list comes from, which
    a message about one of its entries opens with); the host library's own default is
    `[std]`. The front matter's key `initial_use` replaces the host's list, and the
    request's `initial_use` (the `flm` program's `--initial-use`) replaces both. Each list
    is used exactly as given.
  - The math processor is chosen by its name, which stands for the package `math-NAME`
    (`MathChoice`, in the module `seed`: `MathChoice::Processor(NAME)` or
    `MathChoice::NoProcessor`). The host gives its default in the request
    (`Request::default_math`); the host library's own default is no processor, so a host
    that renders documents sets one (the `flm` program sets `mathjax`, or `passthrough`
    when it is built without JavaScript extensions or without the extensions shipped with
    FLM).
    The front matter's key `math`
    takes precedence over the host's choice, and the request's `math` (a `GivenMath`, the
    choice with the words that say where it comes from; the `flm` program's `--math`)
    takes precedence over both.
    The processor's package follows the initial list whichever list applies, and is found
    as an entry of `use` is.
  - The request's `uses` (a `GivenList`; the `flm` program's `--use`, one package per
    occurrence) adds packages after those of `use`, as if the front matter listed them
    last there, each found as an entry of `use` is.

  The feature `write` of `std` hands the files of every output of a command to a *sink* (the
  disk, or memory) in one delivery, named by a naming scheme that the package
  contributes to the library's service slot `flm.artifacts.namer`; the scheme names
  every file of a command after one stem (`paper.html`, `paper.txt`,
  `paper-flmrecomposed.1a2b3c4d.sty`). The feature's setting `write.output_names`,
  set on the render request, gives the main file of one output a path of its own,
  taken exactly as written (`{bibtex: refs.bib}`); the other files of that output keep
  the stem, because their producers ask for their names before the output they belong
  to is known. A name for an output that the registry does not have is an error, and
  nothing is written. It registers the library's feature
  [`compose`](crate::stdlib::features::compose), which assembles the output regions
  of an output through a compose template, a text with named places for an output's
  regions, instead of the backend's final stage; its own feature `compose-standard`
  contributes the template engine `minijinja` and the program's own compose templates
  (`flm_host::stdpkg::compose_standard::templates::std_templates` returns them, each
  with its output and its name). A host that contributes compose templates of its own
  makes each from its description with
  [`Template::from_data`](crate::stdlib::features::compose::Template::from_data), whose
  keys `name` and `output` give the compose template's name and output, and which also
  reads, through three functions the host gives, the file of the template's text
  (the description's key `main`) and the files and the folders the description lists
  under `artifacts`: what the compose template ships beside the output's main file (a
  stylesheet, a logo, a folder of icons). A fourth function gives the place of each
  file read, its qualified path
  ([`Template::source`](crate::stdlib::features::compose::Template::source)), beside
  which the engine reads, when the template is applied, a file that the template
  includes or ships by a path without a root. The host also sets the `flm`
  program's own defaults for several standard features, ranked below everything
  a document writes (for example, footnotes marked a, b, c and figures numbered
  I, II, III), so a document rendered through the host library can differ from
  the same document rendered with the library's defaults; the module `stdpkg`
  lists them.
- **Included files.** An input resolver that finds each file relative to the file
  that holds the `\input` and reads it through the build's reading of files (the
  module `inclusion`). It reads a heading input's heading from the included
  file's front matter: the key `flm.input_as` holds `heading` (the heading
  command), `label` and `numbered`, and the root key `title` holds the title.
- **Two operations, *prepare* and *build*** (the module `session`). *Prepare*
  reads the front matter, finds the packages, combines the settings, and builds
  the registry and the plan. *Build* reads the document's file, parses its
  content, makes the document and executes one run. It returns every diagnostic,
  the files read, and the run, whose tables hold each output's artifact set; the
  output's named files go to the sink. A program that wants the files without a
  disk gives a sink that keeps them in memory.
- **What a prepared document exposes.** Beside the registry and the plan, the value
  *prepare* returns (`Prepared`) gives the profile and its workflow, the packages and their
  factories, the settings in effect with where each value was written
  (`settings_in_effect`), the document's option values and layout, the text of its front
  matter, the extension search path, the manifest of each extension loaded, and the
  packages and extensions a `use` entry may name (`suggestions`). The crate `flm-info`
  (see [Describing an environment](#describing-an-environment-the-crate-flm-info)) reads
  them.
- **The packages of a document whose front matter has problems.** `document_packages`
  finds the packages a document uses from the four keys that decide them (`use`,
  `initial_use`, `math`, `default_profile`); a problem elsewhere in the front matter does
  not stop it, and *prepare* reports that problem.
- **Problems as text or as records** (the module `report`). `report::diagnostic` writes a
  diagnostic as the text the `flm` program prints; `report::Record` is the same diagnostic,
  a problem outside the diagnostics (one that stopped the work, or an error or a warning of
  the program itself), a count of problems or a line of the program's progress as a record
  that serializes to one JSON object, the form of the program's option
  `--json-messages` ([Messages as JSON records](crate::guide::command_line#messages-as-json-records---json-messages)).
  A place in a record is a file, a line from 1 and a column from 1 in bytes, and, in a
  file's parsed content, the byte offset from the top of the file.

With its default cargo feature `fs` off, the host library performs no input or
output and builds for WebAssembly.

### The extensions a program offers

A document lists the extensions it uses by name, under `use` in its front matter. The host
library looks for each name in three places, in this order, and the first place that holds
an extension of that name provides it:

1. the folders of the search path, in order (`Request::extension_folders`);
2. the *collections* that the program loaded, in the order given
   (`Request::collections`, cargo feature `wasm`): a collection is one WebAssembly file
   that holds several extensions (below);
3. the extensions compiled into the program (`Request::bundled`, a `bundled::Bundled`).

The places 2 and 3 are empty unless the program fills them, so a program built on the host
library offers no extension of its own until its request says so. The `flm` program fills
the third with the extensions shipped with FLM, and loads no collection.

The manifest, the files and the code of an extension always come from the same source: its
folder, a collection, or the tables built into the program. Files from two sources are
never combined. The source is chosen once per extension, never per file. Therefore a copy
of an extension placed in a folder of the search path replaces the program's own copy
entirely.

**The extensions compiled into the program.** A program describes them with two tables,
whose row types are defined in `flm-ext`:

- the *table of files* (`flm_ext::source::BundledExtension` rows, re-exported by
  `flm_host::bundled`): for each extension, its folder (`data`, `js` or `rust`), its stem
  (the name a document uses) and its files, the manifest among them;
- the *table of code* (`flm_ext::guest::NamedExtension` rows): for each extension written
  in Rust, its stem and its compiled code.

The program puts both tables in the request with `Bundled::new(files)` and
`Bundled::with_code(code)`. The table of code and `with_code` exist under the cargo feature
`native` of `flm-host`, which also gives the loader the engine `native`
(`flm_ext::native::NativeEngine`). This engine runs an extension written in Rust in the
program's own process, with no sandbox and no limits. The host gives such an extension
its manifest with the engine `native` in place of `wasm`, and it reads its other files
from its row of the table of files. Without the feature `native`, the manifest of an
extension written in Rust names its WebAssembly module, which the table of files does not
hold, and the extension does not load; a program that offers extensions written in Rust
therefore turns `native` on. An extension compiled into the program is identified as
`bundled:FOLDER/STEM` (`bundled:rust/phfpack`).

The crate `flm-extensions` provides the two tables for the extensions shipped with FLM, as
the statics `flm_extensions::EXTENSIONS` (the table of files) and
`flm_extensions::COMPILED_EXTENSIONS` (the table of code). Each shipped extension has a
cargo feature of its own, named after it (`lecturenotes`, `math-mathjax`, `phfpack`, …);
the feature `all` turns on every one, and no feature is on by default. The tables hold
exactly the extensions whose features are on, so a program that offers `phfpack` and
`syntax-highlight` alone depends on `flm-extensions` with the features `phfpack` and
`syntax-highlight`. The table of code exists when the feature of at least one extension
written in Rust is on. The `flm` program offers them all (its cargo feature
`bundled-extensions`, on by default, turns on `all` and the feature `native` of
`flm-host`):

```ignore
use flm_host::bundled::Bundled;

request.bundled = Bundled::new(flm_extensions::EXTENSIONS).with_code(flm_extensions::COMPILED_EXTENSIONS);
```

**Collections.** A *collection* is one WebAssembly file that holds several extensions
whole: the compiled code of those written in Rust, and the files of every one, extensions
of the engines `data` and `js` included. Its name ends in `.flmextensions.wasm` by
convention. The script `extensions/rust/build-wasm.sh --collection FILE NAME …` of the
repository builds a collection of shipped extensions. A program loads a collection
itself, once, with `flm_ext::wasm::Collection::read(label, bytes)` (cargo feature `wasm`
of `flm-ext`), and offers its extensions by adding it to `Request::collections`:

```ignore
use flm_ext::wasm::Collection;

let bytes = std::fs::read("notes.flmextensions.wasm")?;
request.collections.push(Collection::read("notes", bytes)?);
```

`Collection::read` compiles the module, checks that it has the functions that a module of
the engine `wasm` must have, and reads the extensions' files; it reports an error when
one of these fails. The program chooses the label; an extension of the collection is
identified as `LABEL:FOLDER/STEM` (`notes:rust/phfpack`). The search path never provides
a collection: looking for an extension by name in a folder does not open the WebAssembly
files there, and a module found that way is refused when it holds the extensions of a
collection.

On each thread, every extension of a collection runs in one instance of the collection's
module, so one memory serves them all. The memory limit of that instance is the sum of
the memory limits of the collection's extensions that have code, each its manifest's
(`extension.wasm.limits`) or else the host's (`Request::wasm_host`). The fuel limit stays
the limit of the extension whose handler runs. A program that wants an instance per
extension builds one collection per extension. The module page of `flm_ext::wasm`
describes the instances and the limits in full.

### Images and citations in the host library

Under its cargo feature `graphics`, the package `std` holds the feature
`graphics-files`, which supplies the three services of images (see [The services
services](#image-services-resolver-inspector-and-converters)):

- a graphics resolver that looks for a file in the folders of a search path and
  reads it through the build's reading of files;
- an inspector that measures PNG, JPEG, GIF, WebP, SVG and PDF files;
- two converters: `raster`, which turns a PNG, JPEG, GIF or WebP image into a PNG
  image in the program's own code, and `magick`, which converts between any two
  formats by running ImageMagick's program `magick`.

The program that `magick` runs is given in the session request (the value that
the operations of the module `session` take, with the document's path and the
output's folder), never in a setting, so that a document cannot choose a program
that the host starts. The `flm` program takes it from the environment variable
`MAGICK`. The converter `magick` keeps each converted image in the cache
`flm.graphics-files.magick.converted` of the request's manager
(`flm_host::graphics::MagickConverter::CACHE`; see [Caches in the host
library](#caches-in-the-host-library)), under a key that holds the program's version, both
formats, the options and the image's content.

Under its cargo feature `citations`, the package `std` holds the feature
`citations-standard`, a citation provider (see [The citation
providers](#the-citation-providers)). It fetches the details of the cited works
through the crate `autocitefetch`, from the arXiv and from doi.org, many keys in
one request, or reads them from the author's bibliography files through the
build's reading of files, or takes the citation entry written in the citation
key itself (a `manual` key, such as `\cite{manual:{A. Author, A title}}`). What it needs of the machine is given in the session
request (`CitationsHost`), never in a setting:

- how addresses on the internet are fetched (none, for a program given no
  network);
- the folder of the cache file, in which the fetched details are kept between two
  runs (none keeps no cache file; the `flm` program uses the document's folder).

Beside the cache file, the provider keeps the details fetched for all documents in the
cache `flm.citations-standard.autocitefetch.items` of the request's manager
(`flm_host::citations::StandardProvider::CACHE`, the *central cache*). A document whose
cache file lacks a record, or holds a copy that expires earlier, takes the central copy
with no request, and the provider writes it into the cache file. The central cache never
receives a record read from a cache file, since a document may come from someone else.
When a record's server cannot be reached, the provider uses the kept record however old
it is, and gives a warning for a record past its lifetime.

The provider reports the fetching on the build's progress sheet: for each pass
over the keys that has keys to fetch, a row `flm.citations.fetch` ("Fetching
citation data", `flm_host::citations::FETCH_KIND`) counted in the keys to
fetch, the keys found in the cache file not counted; and, for each request and
each wait, a log line at the level `Info` that gives its address or its
expected duration (the arXiv asks for three seconds between two requests).

Unless a work's record carries its citation entry already written (a `manual`
key, or a bibliography record with a ready text in FLM), the provider makes the
work's details into its citation entry in a **citation style**, a file of the
Citation Style Language (CSL) that says how the authors, the title, the journal
and the year are laid out: by default the style "American
Physical Society - et al." that the host library ships, or the style that the
feature's setting `style` gives. The feature also registers two outputs, `bibtex`
and `csl-json`, each with a workflow of the same name, which write the details of
the cited works as a BibTeX file or a CSL-JSON file.

### Caches in the host library

The host library keeps the results of costly work that FLM does outside the library's
own stages in *caches*: the images that ImageMagick converts, the citation records fetched
from servers, and the values and folders that extensions declare. The crate `cachette`,
which `flm-host` re-exports as `flm_host::cache::cachette`, keeps the caches. Its
`Manager` keeps them under one folder, the *root*, bounds their total size and removes old
data. The library `flm-core` has no part in caching, and a program that gives no manager
pays nothing for it.

Your program gives the manager in the request's field `cache`, a
`flm_host::cache::CacheHost` with two fields:

- `manager`, a `cachette::Manager` in the form your program chooses: a folder on disk
  (`Manager::open_path`, which the `flm` program uses), a file system that your program
  supplies (`Manager::new`, for a program that holds its files elsewhere, such as a
  browser), memory (`Manager::in_memory`, which keeps results for the life of the
  process), or a manager that keeps nothing (`Manager::none`, the default).
- `document`, an optional text that identifies the document among all the documents
  that use the root, such as the canonical absolute path of its file (the `flm` program's
  choice). A cache kept per document, such as the folder of LaTeX's auxiliary files of
  the extension `profile-pdf-latex`, has one instance for each such text, and keeps
  nothing for a request without one.

The manager computes the expiry times of the values it stores with the clock it is given,
and the crate that fetches citation records compares these times with the system's
clock. Give the manager the system's clock (`cachette::SystemClock`), or a clock that
reads the same time.

`prepare` makes, from the request's `CacheHost`, the handle through which the features
of the package `std` and the host services open their sessions
(`flm_host::cache::ServiceCaches`, given to the package `std` in `StdParts::cache` and
to `packages::loader`). Each `build` attaches to the handle while it runs: a session that
a feature or a service opens during the build serves every later call of the same build,
and is closed when the build ends. Since the manager is part of the request, every
preparation and every build made from it share the caches, and so do the builds of the
live preview.

The host library keeps these caches:

| cache | what it holds | declared in |
|---|---|---|
| `flm.graphics-files.magick.converted` | the images that the converter `magick` converted, shared by all documents | `flm_host::graphics::MagickConverter::CACHE` (cargo feature `graphics`) |
| `flm.citations-standard.autocitefetch.items` | the citation records fetched for all documents | `flm_host::citations::StandardProvider::CACHE` (cargo feature `citations`) |
| `flm.ext.EXTENSION.NAME` | the caches that the package of an extension declares under the key `caches`, which its handlers reach through the host services `cache` and `programs` | the extension's package (see [Host services](crate::guide::extending#host-services) in the guide to extending FLM) |

**The cargo feature `sqlite`.** A cache whose owner declares the SQLite engine, such as
the central cache of the citation records, is kept in an SQLite database when the
manager offers that engine: the manager is made by `Manager::open_path`, and `flm-host`
is built with its cargo feature `sqlite` (which turns on `cachette`'s feature `sqlite`;
the `flm` program turns it on). On any other manager, the handle opens such a cache with
the engine `Jsonl` when its values have no blobs, and `Files` when they have, so every
cache works on every form of the manager. A program built without the cargo feature
`sqlite` that shares a root with the `flm` program, however, gets sessions without a
cache for the caches that the SQLite engine holds in that root: `cachette` refuses to
open a cache with another engine than the one that wrote it, and reports this to the
manager's reporter. The build then does the work as on a first run.

A failure of a cache is never a failure of a build. An operation of a session never
fails: a read that fails answers "not found", and a write that fails stores nothing. The
manager sends each such event to its *reporter* (`cachette::Reporter`), which your
program gives when it makes the manager; no cache event is a diagnostic of the document.
The host library never removes old data itself: your program calls `Manager::maintain`
when it suits it (the `flm` program calls it after each `render` and `check`, and once
when `flm watch` starts), and `Manager::remove_instances` removes data on request (the
command `flm cache clean`).

### The live preview

A program that shows a document while its author writes it, such as a preview in
a browser or an editor's side panel, can use the host library's module `watch`.

Its loop, `Watch` (cargo feature `watch`), builds the document, then builds it
again whenever a file the last build read changes on the disk (the document, an
included file, an extension's manifest or file, an image, a bibliography file), and whenever your
program asks, one build at a time. It watches the document's file from the start,
so that a change saved while the first build runs causes one more build, and then
every path the last build read, found or not, also when that build stopped early.
Each build continues through the document's errors, writes each element's
position in its source into the page, and writes no file. When the front matter
or an extension changes, the loop prepares again; otherwise it builds over the
same registry. Every preparation and every build of the loop uses the caches of the
request's manager (see [Caches in the host library](#caches-in-the-host-library)), so a
result kept in a cache, such as a converted image, serves the next build also after a
new preparation.

Each build of the loop reports on a progress sheet of its own, laid out as
[The layout of a build in the host library](#the-layout-of-a-build-in-the-host-library)
describes. A
signal that makes the build under way useless cancels it: a file that changed, a
call of `Handle::build_again`, `Handle::set_profile` or `Handle::stop`. The loop
then starts the next build with every change collected, or ends. A cancelled build
hands your program no `Built` event and leaves the current output as it is; the
paths it read are added to the files the loop watches. An export is never
cancelled, and a request for an export does not cancel the build under way.

While a build runs, `Handle::progress()` returns its progress, a
`watch::BuildProgress`: the build's `Sheet`, the table of expected durations to
estimate it with (the durations measured on the last build handed over, laid over
the table shipped with the host library), and the `Instant` from which the times
of its rows are counted. It returns `None` between two builds. To show the build's
percentage, keep one `flm_host::report::ShownFraction` for each build: its method
`estimate` returns the highest fraction estimated since the build started, so
that the percentage shown never decreases.

The loop hands your program each build as a value: the files that changed, the
diagnostics, the output now current, and an **update**. An update compares the
new output with the previous one: whether the page must be loaded again, the
output regions of the page whose HTML changed with their new HTML, the files
beside the page that changed, and the problems of the build, each with its
severity, its text and its place. The update is also available as the JSON
message that a script in the page applies. The comparison (the module
`watch::update`) performs no input or output and is compiled without the cargo
feature, for a program that builds in its own way, such as in a browser from files
held in memory.

The module's server, `watch::server::Server` (cargo feature `watch`), serves the
preview to a browser. It listens on this machine's loopback address only, at port
18910 or the next free one, or at the port your program gives. It serves the
output the loop holds, from memory: each file at its path, and the entry artifact
also at `/`. At `/.flm-preview/`, it serves the *tools page*, a page of its own
that shows the document's page in a frame beside the preview's tools: the problems,
the outline, the help pages, the forms that change the front matter, the export,
and the viewer settings. Under the same prefix, it serves a stream of server-sent
events that tells every open page which build the server holds, when each build
starts and ends, and the text of the progress line of the build under way, and it
answers the requests of the tools page: build again,
change the profile, open a place of the document's files in an editor
(`watch::editor`), read the front matter's settings and write changes into the
document's file (`watch::settings`, `watch::front_matter`), read the constructs
written at a place of a source (`watch::snapshot`), read the help pages, export
the document as `flm render -o` writes it (`watch::export`), and change the
viewer settings (`watch::viewer`). The server opens only a file that the last
build read. It carries out a request that changes something only when the request
comes from a page the server itself served, or from no page.

`Server::start` takes, besides the port and the loop's `Handle`, the editors found
on the machine (`watch::editor::Editors::find`, with `Editors::with_command` for an
editor command of the user's own), a function that receives the outcome of each request to open a
place, the document's settings (`watch::settings::DocumentSettings::on_disk`, over
the JSON Schema of a front matter), and your program's **services**, a value of the
trait `watch::services::Services`. The services give the server what only your
program knows, each with a default that gives nothing:

- `help` returns the help pages of the document's environment (the trait
  `watch::services::Help`: an index of the pages, one page as HTML, and the page of
  a construct written in the document). Without it, the tools page has no help.
- `export_command` returns the command line that writes an export from a terminal,
  which the tools page shows. Without it, the tools page shows none.
- `open_file` opens a file with the system's default application, which the tools
  page asks for an export saved. Without it, the request is refused.
- `home_folder` returns the user's home folder, in which the server reads and writes
  the viewer settings, the file `.flm/watch.json`. Without it, the viewer settings
  are kept in memory.

A program that gives none of them passes `watch::services::NoServices`. The server
calls the services on the threads that answer requests, never on the loop's thread.

Every build of the loop adds the preview's page script to the page, through the
HTML backend's render option `required_definitions` and the feature `preview` of
the package `std` (`watch::build_settings` lists the settings the loop adds). The
script brings each changed output region up to date in place, following the HTML
backend's requirements for a page updated in place ([a page updated in
place](crate::guide::rendering#a-page-updated-in-place)), marks the elements in which
the build's problems lie, and opens the place of an element clicked with Ctrl or Cmd
and Shift in the editor. It adds no visible element to the page. A program that
updates pages in its own way follows the same requirements, so that the page's
formulas stay typeset. The page script and the tools page are the *watch client*,
whose sources are in the folder `npm/flm-watchclient/` of FLM's repository; its file
`PROTOCOL.md` specifies the messages and the requests that the watch client exchanges
with the server, so that a program can serve the pages with a server of its own. The
command-line guide describes the preview as the `flm` program offers it ([watching a
document](crate::guide::command_line#watching-a-document-the-live-preview)).

## Where to look next

- [Stages, capabilities, and plans](crate::guide::pipeline): what the plan and
  the run do, in more detail.
- [The document model](crate::guide::document_model): the tree, the elements it
  carries, and the computed data beside it.
- [Extending FLM: features and packages](crate::guide::extending): writing the
  features you register.
- [Concepts overview](crate::guide::concepts_overview): the library's concepts
  that no single item of the interface represents, each with links to the items
  that do.
- [The flm command-line tool](crate::guide::command_line): the same library
  used from a command line.
- [Panics](crate::guide::panics): every item of the public interface that can
  panic.
