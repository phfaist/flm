# Introduction

FLM ("flexible latexlike markup") is a markup language for documents. Its syntax is
borrowed from LaTeX: commands begin with a backslash, environments are written as
`\begin{…}…\end{…}`, and a blank line separates two paragraphs. For instance:

```text
\section{Absolute convergence}

\begin{definition}\label{def:absconv}
  A series \(\sum_k a_k\) is \emph{absolutely convergent} if the series
  \(\sum_k |a_k|\) converges.
\end{definition}

By \ref{def:absconv}, the series \(\sum_k (-1)^k/k^2\) is absolutely convergent.
```

Each construct of FLM expresses the meaning of a part of the document. In this example,
`\section` starts a section with the heading "Absolute convergence". The `definition`
environment contains a numbered definition, and `\label` attaches the label
`def:absconv` to this definition. `\emph` marks emphasized text, and `\(…\)` encloses
inline mathematics. `\ref{def:absconv}` refers to the definition with this label. A
label has the form `type:name`, and `def` is one of the label types accepted for a
definition.

The source does not specify how a heading, a definition or emphasized text looks. The
appearance is determined when the document is rendered, by the output format and by the
template or the stylesheet used for this format. Therefore, one source can be rendered
in several output formats, such as an HTML page and a LaTeX document.

`flm-core` is the Rust library that implements FLM, and this guide is part of its
documentation. The library parses FLM source, processes the document (for instance,
it numbers the sections and resolves the references), and renders the document in an
output format. It is built on [techy](crate::techy), a toolkit for parsing LaTeX-like
languages, which provides the tokenizer, the parser and the node tree. The
command-line program `flm`, built from the crate of the same name, renders, checks
and previews documents. It is built on the host library `flm-host`, which reads the
files and the settings of a document, uses `flm-core` to process the document, and
writes the outputs to files.

## What the standard features provide

The constructs of the language, and the processing that they need, are defined in
*features*. Each feature contains a group of related definitions: for instance, the
environments `itemize`, `enumerate` and `description` and the command `\item` are
defined in the feature `lists`. Features are grouped in *packages*. The *standard
features*, in [`stdlib`](crate::stdlib), are the features that most documents need. A
program adds them to a package that it declares: the `flm` program holds them in its
package `std`. They provide commands and environments for headings, text formatting,
mathematics, labels and cross-references, lists, footnotes, theorems and proofs,
quotations, hyperlinks, images, figures, tables, citations, verbatim content and included
files. They also let the author define macros and environments, and they provide the
table of contents and the document's title, authors and date.

The standard features contain a *backend* for each of four output formats. A backend
writes a document in one output format:

- `text` writes plain text.
- `html` writes HTML.
- `latex-rendered` writes LaTeX for LuaLaTeX. This output contains the numbers, the
  cross-references and the table of contents as computed by FLM.
- `latex-recomposed` writes LaTeX source that is close to what an author would type. In
  this output, LaTeX computes the numbers and the cross-references when the output is
  compiled.

For each of these output formats, the standard features also contain a *workflow* of
the same name. A workflow is a processing request stored under a name: it specifies
which outputs to produce and which other results, such as checks, to compute. A program
selects a workflow by its name. A fifth workflow, `check`, runs the checks of the
document and produces no output. The four rendering workflows run the same checks beside
their output. Therefore, the problems of a document are reported whether the document is
rendered or only checked.

The standard features do not process mathematics. A document that contains
mathematics also needs a *math processor*, a package that processes the formulas. For
instance, with the package [`math-passthrough`](crate::math_passthrough), the source of
each formula is written into the output, where a LaTeX compiler or the MathJax script in
a web browser typesets it. Without a math processor, each formula is reported as an
error when the document is rendered. The `flm` program uses a math processor by default.

## Three ways to use FLM

You can use FLM in three ways: run the `flm` program on your documents, embed the
library in a program of your own, or extend the library with commands, processing
stages and output formats of your own.

### Running the `flm` program

The `flm` program has seven commands:

- `flm render INPUT` renders the document in the file `INPUT` and writes the output.
- `flm check INPUT` checks the document and reports its problems. It writes no output.
- `flm watch INPUT`, or `flm INPUT`, opens a live preview of the document in a web
  browser, and renders the document again whenever one of its files changes.
- `flm schema` prints the JSON Schema of the settings files. A text editor can check a
  settings file against this schema.
- `flm info` describes the packages, the commands, the settings and the workflows that
  are available to a document. For instance, `flm info '\section'` describes the syntax
  and the arguments of `\section`.
- `flm dump FILE` processes the file `FILE` and prints one view of the processing as
  JSON, for instance a tree or a computed table. Use it to inspect what the library
  does.
- `flm cache list` and `flm cache clean` list and remove the data of the program's
  cache, the folder in which `flm render`, `flm check` and `flm watch` keep the results
  of costly work from one run to the next ([The
  cache](crate::guide::command_line#the-cache)).

```text
$ flm render paper.flm > paper.html
$ flm render -p text paper.flm
$ flm render -p latex-recomposed -o out/paper.tex paper.flm
$ flm check paper.flm
```

By default, `flm render` writes an HTML page to standard output. To produce another kind
of output, select a *profile* with the option `-p`, for instance `-p text` or `-p
latex-recomposed`. To write the output to a file or a folder, give its path with the
option `-o`.

A document can begin with a *front matter*: a block of YAML between a line `---` and the
next line `---`. The front matter contains the metadata of the document, such as its
title and its authors. Under the key `flm`, the front matter contains the settings of
the document, for instance the settings of individual features:

```yaml
---
title: Convergence of series
author: [A. Author, B. Author]
flm:
  features:
    text: {width: 60}
---
```

In this example, the line width of the plain-text output is set to 60 columns with the
setting `width` of the feature `text`. A setting can also be given on the command line
with the option `-O`, as in `-O text.width=60`. Settings that several documents share
can be written in an *extension*, a set of files that the program finds by name. The
chapter [The flm command-line tool](crate::guide::command_line) describes every command,
every option, and the [settings of a
document](crate::guide::command_line#configuration-files-and-document-front-matter).

**Process only documents that you trust.** A document can make the `flm` program read any
file that you can read: a file that it includes with `\input`, an image, or a file given
in its settings, such as a bibliography file. A document can also use extensions,
including the extensions in the folder `_flmextensions` beside it ([where extensions are
looked for](crate::guide::command_line#where-extensions-are-looked-for)). An extension
written in JavaScript or compiled to WebAssembly runs code, and the `flm` program lets
such an extension start any program on the machine, with your rights.

### Embedding the library

A Rust program can use `flm-core` to render FLM documents. The library reads and writes
no files. The program provides the source text, reads the output from the results of the
processing, and writes the output wherever it wants. When a document includes another
file with `\input`, the program provides an [input
resolver](crate::lang::InputResolver), which finds and reads that file.

A program renders a document in five steps:

1. Build a [registry](crate::core::registry). The registry contains the definitions
   used to parse and process documents. These definitions are made in features, which
   the program adds to the registry, usually grouped in packages, such as a package that
   holds the standard features.
2. Parse the source text into a [fragment](crate::model::doc::Fragment), and place the
   fragment in a [document](crate::model::doc::Doc).
3. Build a [plan](crate::core::plan::Plan). A plan is the list of the processing stages
   that compute the requested results, in the order of their dependencies.
4. Execute a [run](crate::core::run::Run). A run is one execution of a plan over a
   document.
5. Read the results. A run stores what it computes, including the output, in typed
   [tables](crate::core::data::table::Table).

The following program takes these five steps and renders a document as plain text:

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmSource, ParseOptions, parse_fragment};
use flm_core::stdlib::backends::text::{self, TEXT_OUTPUT};
use flm_core::stdlib::{StdlibConfig, add_features};

// 1. Build a registry with a package `std` of the standard features, and a math processor.
let mut builder = RegistryBuilder::new();
builder.use_package(add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// 2. Parse the source text, and place the fragment in a document.
let source = Arc::new(FlmSource::new("Some \\emph{emphasized} text."));
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// 3. Build the plan of the `text` workflow.
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);

// 4. Execute a run of the plan over the document.
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// 5. Read the plain text from the table of the text output.
let output = run.table(TEXT_OUTPUT).expect("declared").expect("written");
assert_eq!(output.text, "Some emphasized text.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

In step 3, the program does not list the stages to run. Each stage is declared with the
[capabilities](crate::core::registry::CapabilityDef) that it provides and the
capabilities that it requires. A capability is a named state of the processing, such as
a rendered output. The program requests capabilities, here by selecting the `text`
workflow, and the library selects the stages that provide them and orders these stages.
Therefore, a stage can be added in a feature without any change to the other stages: the
stage runs in every plan that needs a capability that it provides.

The registry and the plan do not depend on a document. A program therefore builds them
once and uses them for every document that it processes. The chapter [Embedding
flm-core](crate::guide::embedding) describes each step in full. It also describes the
[host library
`flm-host`](crate::guide::embedding#the-host-library-for-a-program-that-reads-files),
which the `flm` program uses to read the files and the settings of a document.

### Extending the library

Every command, environment, processing stage and output format of FLM is defined in a
feature. You write a feature as a Rust value that implements the trait
[`FlmFeature`](crate::core::registry::FlmFeature). In the method `register` of this
trait, you declare the definitions of the feature with a
[`FeatureBuilder`](crate::core::registry::FeatureBuilder).

The standard features are written in the same way, with the same feature builder.
Therefore, you can add commands and environments of your own, change how a standard
construct is rendered, add a check, or add an output format, without any special
support in the library. Your program adds its features to the registry beside the
standard features, either one by one or bundled in a package.

An author can also use the packages of an extension in a document, by listing them in
the setting `use` of the front matter. An extension is written as data, in JavaScript,
or in Rust. The chapter [Extending FLM](crate::guide::extending) describes how to write
features, packages and extensions.

## Suggested reading order

The [guide's landing page](crate::guide) lists every chapter with a short description.
Read the chapters in the following order, depending on what you want to do:

- **To write and render documents:** [FLM language
  syntax](crate::guide::language_syntax), then [std flm](crate::guide::std_flm), the
  baseline of the language that any reader of a document can expect, then [The flm
  command-line tool](crate::guide::command_line).
- **To embed or extend the library:** first the [Concepts
  overview](crate::guide::concepts_overview), which defines the main concepts of the
  library. The other chapters and the API reference link to these definitions. Then [The
  document model](crate::guide::document_model), [Stages, capabilities, and
  plans](crate::guide::pipeline) and [Rendering](crate::guide::rendering), in the order
  in which a document is processed. [Numbering](crate::guide::numbering) describes how
  the sections, equations, theorems and other constructs are numbered. Then [Embedding
  flm-core](crate::guide::embedding) or [Extending
  FLM](crate::guide::extending), depending on your task. The crate root lists the
  [modules](crate#modules) of the API.
- **If you know the Python implementation of FLM:** [Migrating from Python
  FLM](crate::guide::python_flm_migration) maps its concepts to the items of this
  library.
- **For an AI assistant:** the [AI guide](crate::guide::ai_guide) and its sub-chapters
  are a condensed reference of the whole library, written to be loaded into the context
  of an assistant.
