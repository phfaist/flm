# Guide

FLM ("flexible latexlike markup") is a markup language for documents, written with
a syntax borrowed from LaTeX, in which each construct says what a part of the
document means rather than how it looks. `flm-core` is the Rust library that parses
FLM documents and renders them to several output formats; it is built on
[techy](crate::techy), a parser toolkit for LaTeX-like languages. The command-line
program `flm` (crate `flm-cli`) renders, checks and previews documents, and describes
the commands and settings a document may use; it uses the library through `flm-main`,
the library that finds and reads a document's files and settings and writes the outputs,
and describes them through `flm-info`. FLM is used both for stand-alone documents and as
the markup language of the [Error Correction Zoo](https://errorcorrectionzoo.org/).

This guide is the narrative documentation; the crate's modules are the API
reference, and the two link each other throughout. Each chapter below is a page of
this module, written from a Markdown file under `docs/` in the repository. The
chapters are grouped into a **User Guide** (writing FLM documents and running the
`flm` program), a **Developer Guide** (how the library works, extending it with
features and backends of your own, and embedding it in a program), and an **AI
Guide** (condensed chapters written to be loaded into an AI assistant's context, also
useful to a reader who already knows FLM).

**New to FLM? Read the [Introduction](crate::guide::introduction) first:** it
explains what the library does and which chapter serves which purpose.

The chapters describe the library as it is built today. Where a chapter also
describes something that is designed but not yet built, it says so at that place.

## User Guide

- [Introduction](crate::guide::introduction) — what FLM is, what the library does
  today, and the three levels of use: running the `flm` program, embedding the
  library, writing features and backends.
- [FLM language syntax](crate::guide::language_syntax) — the markup as a document
  author writes it: commands, environments, paragraphs, mathematics, and the standard
  set of elements.
- [The flm command-line tool](crate::guide::command_line) — rendering, checking and
  previewing documents with the `flm` program, and describing the commands and settings
  available to a document (`flm info`): its commands, their options, and the settings a
  document and its configuration files may hold.

## Developer Guide

- [Concepts overview](crate::guide::concepts_overview) — the definitions of the
  library's main concepts, one section each; the rest of the documentation links
  into it.
- [The document model](crate::guide::document_model) — the parsed node tree as the
  only representation of a document, the elements carried by its nodes, how trees
  and nodes are addressed, the frozen tree, and the computed data kept beside it.
- [Stages, capabilities, and plans](crate::guide::pipeline) — how processing is
  organized: stages that provide and require capabilities, plans derived from what a
  caller asks for, and runs that execute a plan over a document.
- [Rendering: backends and outputs](crate::guide::rendering) — how a frozen document
  becomes output: lowering into presentation trees for each backend, the emitters,
  the text layout engine, and the four backends the library ships.
- [Numbering: counters and counter styles](crate::guide::numbering) — how a counter
  value becomes the number a reader sees (counter styles, number formats, the words
  before a number), and how the numbering feature counts the numbered places of a
  document.
- [Extending FLM: features and packages](crate::guide::extending) — defining your own
  elements, stages, tables and backends, bundling them as a package, and writing
  extensions, found by name at run time, as data, in JavaScript, or in Rust compiled to
  WebAssembly.
- [Embedding flm-core](crate::guide::embedding) — using the library from your own
  program: the entry points, the options, the services a program supplies, and the
  WebAssembly build.
- [List of panicking items](crate::guide::panics) — the complete list of the items of
  the public API that can panic.
- [Migrating from Python FLM](crate::guide::python_flm_migration) — how the concepts
  of the earlier Python implementation of FLM correspond to this library's.

## AI Guide

- [AI guide](crate::guide::ai_guide) — the entry point of the AI guides, written to be
  loaded into an AI assistant's context: what FLM is, the vocabulary every AI guide
  uses, an author's reference of the language and of the `flm` program, a map of the
  library and of its standard package, and which page to load next.
- [AI guide: document model](crate::guide::ai_guide_document_model) — condensed
  reference for the document tree, elements and aspects, names and addresses, sources
  and spans, fragments and documents, tree states, and the tables and synthesized
  values kept beside the trees.
- [AI guide: pipeline](crate::guide::ai_guide_pipeline) — condensed reference for
  stages, capabilities, plan requests and workflows, plan construction, the runner,
  external tables and value catch-up.
- [AI guide: extending FLM](crate::guide::ai_guide_extending) — condensed reference
  for adding functionality to FLM, with the exact items, signatures and refusals: a
  feature with its elements, aspects, tables, stages, lowering rules, options and
  services; a backend and its output; an extension of a standard feature.
- [AI guide: embedding](crate::guide::ai_guide_embedding) — condensed reference for a
  program that embeds FLM, through `flm-core` directly (the six steps, options, the
  services a host provides, external tables, the WebAssembly build) or through the
  host library `flm-main`.
