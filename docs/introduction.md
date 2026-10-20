# Introduction

FLM ("flexible latexlike markup") is a markup language for documents. It is written
the way LaTeX is written, with backslash commands, `\begin{…}…\end{…}` environments
and blank lines between paragraphs, but each construct says what a part of the
document *means* rather than how it should look:

```text
\section{Encodings}\label{sec:enc}

A code is \emph{degenerate} when distinct errors act identically on the
code space; see \ref{sec:enc} and the identity \(\alpha^2 = 1\).
```

`\section` marks a heading, `\emph` marks emphasis, `\label` attaches a label to the
heading, and `\ref` refers to the place that carries that label. Nothing in the
source says how a heading is set or whether emphasis is shown in italics: the backend
that writes each output format, and the stylesheet or style file applied to its output,
decide that, so one source can be rendered in several formats.

`flm-core`, the crate whose documentation you are reading, is the Rust library that
implements the language: it parses FLM source, processes it, and renders it. It is
built on [techy](crate::techy), a parser toolkit for LaTeX-like languages, which
supplies the tokenizer, the parser and the node tree over which FLM's own language is
defined. The command-line program `flm` (crate `flm-cli`) renders, checks and
previews documents with this library; it relies on the library `flm-main`, which
finds and reads a document's files and settings and writes the outputs, and on the
library `flm-info`, which describes the commands, settings and workflows a document may
use. FLM is used
both for stand-alone documents and as the markup language of the [Error Correction
Zoo](https://errorcorrectionzoo.org/).

## What the library does today

The standard package, [`crate::stdcore`], is the set of
[features](crate::core::registry::FlmFeature) that makes the crate a working FLM
processor. It gives an author headings, inline formatting commands, mathematics,
verbatim content, labels and references, lists, theorems, footnotes, citations, and user
definitions ([`Definition`](crate::lang::rewrites::userdefs::Definition)): macros and
environments that the author or the embedding program defines by substitution; it lays
content out in paragraphs and blocks. It also registers five
[workflows](crate::core::registry::WorkflowDef). A workflow is a named set of demands
together with the choices that go with them, a complete plan request held in the
registry, which a caller chooses by name:

- `check` ([`CHECK_WORKFLOW`](crate::stdcore::CHECK_WORKFLOW)) runs every check the
  registry holds and produces no output;
- `text` ([`WORKFLOW`](crate::stdcore::backends::text::WORKFLOW)) renders the
  document as plain text, wrapped to a chosen width;
- `latex-recomposed` ([`WORKFLOW`](crate::stdcore::backends::latex_recomposed::WORKFLOW))
  writes the document back as LaTeX source, close to what an author would have typed,
  with a style file holding the definitions that output needs, and leaves numbering
  and cross-references to LaTeX;
- `latex-rendered` ([`WORKFLOW`](crate::stdcore::backends::latex_rendered::WORKFLOW))
  writes the document as fully resolved LaTeX for LuaLaTeX, every element as a command
  of the `\flmx…` contract whose look a style file defines and a template can change;
- `html` ([`WORKFLOW`](crate::stdcore::backends::html::WORKFLOW)) renders the
  document as an HTML page for a browser; the CSS classes and attributes a stylesheet
  relies on are derived from the names of the [presentation
  elements](crate::stdcore::backends::pres) the document is lowered into, of the roles
  those elements carry and of their payload fields
  ([`conventions`](crate::stdcore::backends::html::conventions)).

All five workflows run the same checks: each of the four rendering workflows asks
for the collection of checks [`flm.validated`](crate::stdcore::VALIDATED) beside its
output, and the `check` workflow asks for that collection alone. The checks read the
document's trees. In the recomposed LaTeX, a user definition written back as a LaTeX
definition is checked by LaTeX when the output is compiled, not by these checks.

**Plain text, recomposed LaTeX and HTML are the output formats the library ships.**
The parts that produce them are general: the [presentation
elements](crate::stdcore::backends::pres) that every backend shares, the
[lowering](crate::guide::concepts_overview#backends-lowering-and-outputs) that
rewrites a document into them, and the [backend](crate::core::registry::BackendDef)
registration for one output format. A program may therefore register a backend of
its own beside the four backends of the crate,
[`text`](crate::stdcore::backends::text),
[`latex-recomposed`](crate::stdcore::backends::latex_recomposed),
[`latex-rendered`](crate::stdcore::backends::latex_rendered) and
[`html`](crate::stdcore::backends::html).

Nothing in the processing pipeline reads or writes files. A caller supplies source
text and receives [artifacts](crate::core::artifact::Artifact) — each one file's
worth of output, held as a value and identified by its producer and a name — and
decides where they go. When a document includes other files, the caller supplies an
[input resolver](crate::lang::InputResolver) that finds and reads them.

## Three levels of use

### Running the `flm` program

The `flm` program has six commands:

- `flm watch INPUT` (or `flm INPUT`) shows the document in the browser and builds it
  again whenever a file it read changes;
- `flm render INPUT` renders the document and writes the output: HTML unless the front
  matter or the command line chooses another;
- `flm check INPUT` reports the problems of the document and writes nothing;
- `flm schema` prints the schema of the settings files, which a text editor can check
  them against;
- `flm info [PATH…]` describes the environment in which a document is processed (its
  packages, the commands their features declare, the settings, the workflows) as pages
  of documentation, printed as text or as JSON (`flm info '\section'` shows how
  `\section` is written and what each argument holds);
- `flm dump FILE` processes one file and prints one view of it as JSON — a tree at a
  chosen point of the processing, a computed table, or an output's files — for
  inspecting what the library does.

```text
$ flm render paper.flm > paper.html
$ flm render -p text paper.flm
$ flm render -p latex-recomposed -o out/paper.tex paper.flm
$ flm check paper.flm
```

`render`, `check` and `watch` combine the settings of a document from five sources, a
later source overriding an earlier one: the program's defaults; the packages of the
extensions the front matter lists (sets of files, found by name, that hold settings
shared by several documents or features written in JavaScript); the front matter
itself (a block of YAML at the top of the file); the settings a package of an extension
gives the workflow that runs; and the command line. [The command-line
chapter](crate::guide::command_line) describes every command and option.

### Embedding the library

A program that renders FLM itself does four things: it builds a
[registry](crate::core::registry), the record of everything the language
and the processing consult; it [parses](crate::lang::parse_fragment) source into a
[fragment](crate::model::doc::Fragment) and places the fragment in a
[document](crate::model::doc::Doc); it builds a [plan](crate::core::plan::Plan), the
list of processing stages that produce what it asks for; and it executes a
[run](crate::core::run::Run) of that plan over the document. The results are
[tables](crate::core::data::table::Table) that the run computed, read back through
typed keys.

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
use flm_core::stdcore::{StdcoreConfig, package};

// 1. What this program can do with a document: the standard package.
let mut builder = RegistryBuilder::new();
builder.use_package(package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = Arc::new(builder.build(&RegistryInputs::new())?);

// 2. Parse one piece of source, and place it in a document.
let source = Arc::new(FlmSource::new("Some \\emph{emphasized} text."));
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// 3. Ask for the `text` workflow; the library derives the stages it needs.
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// 4. Read the result out of the table the text backend wrote it to.
let output = run.table(TEXT_OUTPUT).expect("declared").expect("written");
assert_eq!(output.text, "Some emphasized text.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Step 3 does not list the stages to run. A caller states which
[capabilities](crate::core::registry::CapabilityDef) it wants — here, by choosing a
workflow — and the library works out which registered stages provide them and in
which order they run. [The embedding chapter](crate::guide::embedding) describes this
work in full, including the options a program sets and the services it supplies.

### Writing features and backends

Every element of the language and every stage of the processing is contributed by
[features](crate::core::registry::FlmFeature): Rust values that contribute a related
bundle of things to a registry (elements, tables, stages, lowering rules, backends,
workflows, options) through the feature builder.
The standard package registers its features exactly the way an added feature
registers. A program can therefore add commands of its own, replace how a standard
element is rendered, add a check, or add a whole output format, without any special
case in the library. [Extending FLM](crate::guide::extending) describes that work.

## How this guide is organized

The guide and the API reference form one documentation bundle and link each other
throughout. The [guide's landing page](crate::guide) lists every chapter. The
chapters are worth reading in this order:

- **To write and render documents:** [FLM language
  syntax](crate::guide::language_syntax), then [the flm command-line
  tool](crate::guide::command_line).
- **To understand the library:** [concepts overview](crate::guide::concepts_overview)
  first; it defines the library's main concepts and is the page the rest of the
  documentation links into. Then [the document model](crate::guide::document_model),
  [stages, capabilities and plans](crate::guide::pipeline) and
  [rendering](crate::guide::rendering), in the order a document passes through them.
- **To build on the library:** [extending FLM](crate::guide::extending) to add
  features, elements and backends; [embedding flm-core](crate::guide::embedding) to
  drive the library from a program of your own. [Migrating from Python
  FLM](crate::guide::python_flm_migration) maps the concepts of the earlier Python
  implementation onto this one, and [the list of panicking
  items](crate::guide::panics) states where the public API can panic and under which
  condition.
- **For an AI assistant:** the [AI guide](crate::guide::ai_guide) and its
  sub-chapters are a condensed orientation to the whole library, written to be loaded
  into an assistant's context.

## Where to go next

- To write documents: [FLM language syntax](crate::guide::language_syntax).
- To render them: [the flm command-line tool](crate::guide::command_line).
- To embed or extend the library: the [concepts
  overview](crate::guide::concepts_overview), then the crate root's [list of
  modules](crate) for where each part of the API is defined.
