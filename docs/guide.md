# Guide

This guide is the narrative documentation of FLM ("flexible latexlike markup"), a
markup language for documents with a syntax borrowed from LaTeX. The guide explains how
to write FLM documents, how to render them with the command-line program `flm` (crate
`flm`), and how to use and extend `flm-core`, the Rust library that parses,
processes and renders FLM documents. The other modules of `flm-core` form the API
reference, and the guide and the API reference link to each other.

The chapters of the guide are grouped in three parts:

- The **User Guide** is for the authors of FLM documents. It describes the language and
  the `flm` program.
- The **Developer Guide** is for programmers who embed the library in a program of their
  own, or who extend it with commands and output formats of their own. It describes how
  the library works and how to use it.
- The **AI Guide** contains condensed reference chapters, written to be loaded into the
  context of an AI assistant.

If you are new to FLM, start with the [Introduction](crate::guide::introduction).

## User Guide

- [Introduction](crate::guide::introduction) explains what FLM is and what the library
  provides. It shows the three ways to use FLM: running the `flm` program, embedding the
  library in a program, and extending the library with commands and output formats of
  your own. It also suggests an order in which to read the chapters.
- [FLM language syntax](crate::guide::language_syntax) is the reference of the language
  for authors. It describes how the source text is read, how the text is divided into
  paragraphs and blocks, and every command and environment of the standard features of
  FLM, such as headings, references, mathematics, lists, citations and theorems. It also
  describes how an author defines macros and environments of their own.
- [std flm](crate::guide::std_flm) describes the common baseline of the language: the
  constructs that a document may use, and what they mean, when its writer and its reader
  share nothing but the name "std flm". It also states what the baseline leaves to the
  program that processes the document, such as the content of formulas and how anything
  is rendered.
- [The flm command-line tool](crate::guide::command_line) describes the `flm` program
  and its commands. These commands render, check and preview a document, print the
  schema of the settings files, describe the commands and settings available to a
  document (`flm info`), and print the trees, tables and outputs of the processing as
  JSON for inspection (`flm dump`). The chapter also describes the settings of a
  document, which are written in the front matter of the document or in the extensions
  that the document uses.

## Developer Guide

- [Concepts overview](crate::guide::concepts_overview) defines the main concepts of the
  library, one concept per section. The other chapters and the API reference link to
  these definitions.
- [The document model](crate::guide::document_model) describes how the library
  represents a document. A document consists of the node trees that the parser
  produces, and each node stores an *element*, a value that describes what the node
  means. The chapter also describes the addresses of trees and nodes, and the *tables*
  in which the data computed about a document is stored after its trees are frozen.
- [Stages, capabilities, and plans](crate::guide::pipeline) describes how the
  processing of a document is organized. The processing consists of *stages*, units of
  work that provide and require *capabilities*. A program requests capabilities, and the
  library builds a *plan*, the list of stages needed to provide them, in the order of
  their dependencies. The chapter also describes how a *run* executes a plan over a
  document.
- [Rendering: backends and outputs](crate::guide::rendering) describes how a document is
  rendered in an output format. A *backend* produces one output format. Before a backend
  writes its output, *lowering rules* rewrite the trees of the document into trees of
  elements that the backend writes directly. The chapter describes this lowering, the
  outputs and their files, and the four backends of the library, which write plain
  text, HTML, and two forms of LaTeX.
- [Numbering: counters and counter styles](crate::guide::numbering) describes how the
  sections, equations, theorems and other constructs of a document are numbered. A
  *counter* counts one kind of construct in document order. A *counter style* specifies
  how the value of a counter is written, for instance in roman numerals, or preceded by
  a word such as "Eq.". The chapter also describes the counters of the standard features,
  and how to declare a counter for a construct of your own feature.
- [Extending FLM: features and packages](crate::guide::extending) describes how to add
  commands, environments, processing stages and output formats to FLM. You define them
  in *features*. A feature contributes a group of related definitions to the
  [registry](crate::core::registry), which contains the definitions used to parse and
  process documents. You can bundle several features in a *package*. The chapter also
  describes *extensions*: sets of files that a program finds by name when a document
  uses them. An extension provides packages, and it is written as data, in JavaScript,
  or in Rust.
- [Embedding flm-core](crate::guide::embedding) describes how to use the library in a
  program of your own: the steps from the source text to the output, the options that a
  program sets on a document or on a render, the services that a program provides to
  the features, and the build for WebAssembly. It also describes the host library
  `flm-host`, which reads the files and the settings of a document and writes its
  outputs. The `flm` program is built on `flm-host`.
- [List of panicking items](crate::guide::panics) lists every item of the public API
  that can panic, with the condition under which it panics. No content of a document can
  cause a panic: an item panics only when its caller does not meet a requirement that
  the item documents.
- [Migrating from Python FLM](crate::guide::python_flm_migration) is for readers who
  know the Python implementation of FLM. It maps the concepts of the Python
  implementation to the items of this library, and it lists the differences in behavior
  between the two implementations.

## AI Guide

- [AI guide](crate::guide::ai_guide) is the entry point of the AI guides. An AI
  assistant loads this page first, to write FLM documents, to embed the library in a
  program, or to extend the library. The page defines the terms that all AI guides use,
  contains a short reference of the language and of the `flm` program, gives an
  overview of the library and of its standard features, and states which AI guide to load
  next.
- [AI guide: document model](crate::guide::ai_guide_document_model) covers the subject
  of the chapter [The document model](crate::guide::document_model) in condensed form.
- [AI guide: pipeline](crate::guide::ai_guide_pipeline) covers the subject of the
  chapter [Stages, capabilities, and plans](crate::guide::pipeline) in condensed form.
- [AI guide: extending FLM](crate::guide::ai_guide_extending) covers the subject of the
  chapter [Extending FLM](crate::guide::extending) in condensed form, including the
  lowering rules and the backends that the chapter
  [Rendering](crate::guide::rendering) describes in full.
- [AI guide: embedding](crate::guide::ai_guide_embedding) covers the subject of the
  chapter [Embedding flm-core](crate::guide::embedding) in condensed form: a program
  that uses `flm-core` directly, or through the host library `flm-host`.
