# AI guide

This page is the entry point of the AI guides: pages written for an AI agent that is
about to write FLM documents, embed the FLM library in a program, or extend it. It
states what FLM is, defines the vocabulary every AI guide uses, gives an author's
reference of the language and of the `flm` program, maps the library and its standard
package, and points at the page to load next. Each statement links the API item or the
guide section that documents it in full.

How to use the AI guides: load this page first; it is enough on its own for writing documents
and for the everyday embedding flow. For deeper work, load at most one more page from
[Where to look next](#where-to-look-next) beside it.

## What FLM is

- **FLM** ("flexible latexlike markup") is a markup language for documents. Its syntax is
  borrowed from LaTeX: `\emph{…}`, `\begin{env}…\end{env}`, blank lines between
  paragraphs. Each construct says what a part of the document *means*; a backend decides
  what it becomes in one output format (plain text, HTML, rendered LaTeX, recomposed LaTeX). FLM takes
  LaTeX's syntax but not its processing model: parsing never expands a macro, an author
  never steps a counter, and there is no page model.
- **`flm-core`** ([`crate`]) is the Rust library that implements FLM: it parses source,
  processes the parsed document through a pipeline it derives from what the caller asks
  for, and renders it. It is built on [techy](crate::techy), a parser toolkit for
  LaTeX-like languages, and on [untechxt](crate::untechxt), a Unicode-to-LaTeX table and
  encoder; it re-exports both, and a dependent writes `flm_core::techy` and
  `flm_core::untechxt` rather than depending on those crates itself.
- **`flm-main`** is the host library: it finds and reads a document's files, reads the
  front matter and the extensions it uses, combines settings, prepares the registry and
  the plan, runs the library and writes the outputs. Its default cargo feature `fs` reads
  files and writes outputs on disk; without it the crate performs no input or output and
  compiles for WebAssembly. Under further cargo features it reads image files
  (`graphics`), fetches and formats citation entries (`citations`) and serves the live
  preview (`watch`).
- **`flm-cli`** is the `flm` program over the host library: `flm render`, `flm check`,
  `flm watch` (also `flm INPUT`: the live preview), `flm schema`, `flm info` and
  `flm dump`. See [The flm command-line tool](crate::guide::command_line).
- **`flm-info`** describes an *environment* (a registry, a plan, the package factories):
  the query API `Environment` and its typed views (features, commands with their
  arguments and roles, settings, workflows and plans, namespaces), the description as
  JSON with its JSON Schema, a tree of documentation pages written as FLM source, and the
  engine of `flm dump`. Its core needs `flm-core` alone and builds for WebAssembly; its
  cargo feature `flm-main` describes a document prepared by the host library. `flm info`
  is built on it ([Describing an environment](crate::guide::embedding#describing-an-environment-the-crate-flm-info)).
- **`flm-ext`** is the extension layer: it finds and loads *extensions*, sets of files
  found by name that provide packages, written as data (the engine `data`), in
  JavaScript (the engine `js`, run by QuickJS under the cargo feature `quickjs`) or
  compiled to a WebAssembly module (the engine `wasm`, run by wasmi under the cargo
  feature `wasm`; a Rust extension is written against the guest client `flm_ext::guest`,
  the cargo feature `guest`) or, from the same Rust source, compiled into the program
  (the engine `native`, under the cargo feature `native`), and
  lets their features reach `flm-core` through an interface that mirrors `flm-core`'s
  own items. See [Extensions](crate::guide::extending#extensions).
- **`flm-phfpack`** is an extension crate, the package `phfpack` (macros for the notation
  and quantities of quantum information), written over `flm-core`'s public interface
  alone. The `flm` program carries it under its default cargo feature `phfpack`; a
  document uses it by listing it under `use` in its front matter.
- **`flmdoc`** is a Rust extension compiled into the program (engine `native`): the markup
  of documentation pages, a Markdown-style code span between backticks, the constructs
  `docitem`, `\docsignature`, `\docsummary`, `docparams`/`\docparam`, `\docfield`, and the
  link `\docref{path}{text}` with the render setting `flmdoc.link_style` (`command`, `path`).
  `flm info` writes its pages in it; a document uses it with `use: [flmdoc]`
  ([the extension flmdoc](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc)).

Two facts constrain everything a caller does:

- **`flm-core` performs no input or output**: no file, no network, no process. A caller hands in text and takes out values
  ([`Artifact`](crate::core::artifact::Artifact)s); when a document includes other
  files, the caller supplies an [`InputResolver`](crate::lang::InputResolver) that finds
  and reads them. The only file access in `flm-core` is the test tool
  [`dump::golden`](crate::dump::golden).
- **No document content can make the library panic**: every problem a document can
  cause is a diagnostic or a typed error. [`crate::guide::panics`] lists every public item
  that can panic, each under a condition a document cannot bring about.

## Vocabulary

The AI guides use these terms with exactly these meanings. Each definition is the
canonical one; the link leads to where the term is documented in full.

### The document

| term | definition | canonical location |
|---|---|---|
| element | the value that says what a node means: a declaration with a stable dotted type name, identified by the element value itself (its allocation) and not by its name | [`ElementSpec`](crate::model::element::ElementSpec) |
| element value | one allocation of an element; a family of related commands is one element type with one element value per command (`\section` and `\subsection` are two values of `flm.heading`) | [elements and aspects](crate::guide::concepts_overview#elements-and-aspects) |
| aspect | a piece of behavior attached to an element when the feature registers it and read back through a trait object; it is evaluated per occurrence of the element and reads no options | [elements and aspects](crate::guide::concepts_overview#elements-and-aspects) |
| fragment, tree, document | a fragment is one piece of source as the parser delivered it (its tree, its source, its parse diagnostics); a tree is one placement of a fragment's node tree in a document, numbered by the document's arena; a document is its content (the tree arena, the traversal list and the resolution map) and the option values bound on it, and nothing else | [`Fragment`](crate::model::doc::Fragment), [`TreeId`](crate::model::addr::TreeId), [`Doc`](crate::model::doc::Doc) |
| raw tree, structured tree | a raw tree is a tree the block-structure rewrite has not laid out: it stays as parsed, without paragraphs and with its text unresolved (the capability `flm.blocks.raw`); a structured tree is a tree that rewrite has laid out in paragraphs and blocks, with its text resolved (`flm.blocks.structured`) | [`RAW`](crate::stdcore::features::blocks::RAW), [`STRUCTURED`](crate::stdcore::features::blocks::STRUCTURED) |
| frozen tree | a document tree after the rewrite stages have run: it is never changed again, and everything a later stage computes about it is stored in tables beside it, keyed by node address | [frozen trees and computed data](crate::guide::concepts_overview#frozen-trees-and-computed-data) |
| structural elements | the elements of the node shapes no feature declares: text, comments, containers, groups of each class, paragraph breaks, template placeholders, and the macros and environments a parse could not attribute to any feature (`flm.unknown.macro`, `flm.unknown.environment`) | [`StructuralElements`](crate::lang::structural::StructuralElements) |
| content region | a run of sibling nodes laid out together: a fragment's top level, a body, an argument, a group's children; the block-structure pass lays each out as blocks, inline, or opaque (its region layout) | [`crate::lang::rewrites::blocks`] |
| opaque region, verbatim region, opaque wrapper | an opaque region is a part of a tree that the rewrite passes carry over untouched (mathematics and verbatim content, any run parsed in math mode, the content of an opaque wrapper); the lowering copies mathematics and verbatim content unchanged and writes an opaque wrapper through its children; a verbatim region is a run of raw text in which no markup character means anything, always inside a group of the verbatim class; an opaque wrapper is a group with no delimiters (`flm.opaque-wrapper`) that the parser places around the content of an argument or a body declared `with_opaque_wrapper()`, of every characters-only group, and of a proof title that begins with a star, so that the element reads the text as typed (`\cite`'s keys, `\label`, `\ref`, `\hyperref`'s target, a proof's back-reference) | [opaque regions](crate::guide::concepts_overview#opaque-regions) |
| field, tack-on argument | a field is a macro that a construct absorbs as data rather than as content (`\label` after a heading; `\\`, `\label`, `\tag` and `\nonumber` in a display-math body); a tack-on argument is the trailing argument through which a construct absorbs the field macros written after its other arguments | [fields](crate::guide::concepts_overview#fields-data-a-construct-reads) |
| scoped package | a set of element declarations that can be written only inside the environment bodies and arguments whose declarations refer to the package by name, for instance `\item` inside a list's body or `\caption` inside a float's body | [`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef) |
| synthesized value | a small tree that a stage builds while a run is under way, held outside the document's trees and numbered per stage invocation; a consumer brings it up to date through value catch-up before reading it as document content | [synthesized values](crate::guide::concepts_overview#synthesized-values) |
| diagnostic, condition | a diagnostic reports a problem with a document: a severity (Error, Warning or Note), a position, and a typed condition with its data; its message is written for a person and is not a stable interface. A condition is the typed body of a diagnostic, identified as `flm.<feature>.<condition>` | [diagnostics](crate::guide::concepts_overview#diagnostics) |

### The registry and the pipeline

| term | definition | canonical location |
|---|---|---|
| registry | the frozen record of everything the language and the pipeline consult for a set of features: the elements, stages, tables, capabilities, lowering rules, backends, outputs, workflows, option types and services the features declared, built once and shared by every document, plan and run | [`crate::core::registry`] |
| feature | a Rust value that contributes a related bundle of things to a registry (elements, stages, tables, lowering rules, backends, services, settings) through the feature builder | [`FlmFeature`](crate::core::registry::FlmFeature) |
| package | a named bundle of features together with the packages it builds on | [`PackageEntry`](crate::core::registry::PackageEntry) |
| stage | one unit of work of the pipeline: it declares the capabilities it provides and requires, the tables it reads and writes, its effect (compute, check, or input/output) and its scope (once per tree, or once per document), and the plan decides whether and when it runs | [`Stage`](crate::core::stage::Stage) |
| rewrite stage | a stage that runs before the document's trees are frozen, turns one tree into another, reads no computed data, and publishes the trees it produced in a table named after the capability it provides | [`RewriteStage`](crate::core::stage::RewriteStage) |
| capability | a registered dotted name for a state of processing that a stage brings about, such as `flm.blocks.structured` | [`CapabilityDef`](crate::core::registry::CapabilityDef) |
| demand, workflow | to demand a capability is to ask the plan for a stage that provides it; a workflow is a named set of demands together with the choices that go with them | [`PlanRequest`](crate::core::plan::PlanRequest), [`WorkflowDef`](crate::core::registry::WorkflowDef) |
| plan | the list of stages that satisfies a plan request, in dependency order and grouped in batches; a pure value of the registry and the request, with nothing of any document in it | [`Plan`](crate::core::plan::Plan) |
| run | one execution of one plan over one document; it owns the frozen trees, the store and the diagnostics | [`Run`](crate::core::run::Run) |
| table | typed data that one stage writes beside the frozen trees and other stages read; its identity is its registered dotted name, never its Rust type, and that name is also a capability | [`Table`](crate::core::data::table::Table) |
| store | the run's collection of computed tables: one writer per table, a declared write always installs a value, and every read and write is checked against the stage's declarations at the access site | [`Computed`](crate::core::data::store::Computed) |
| external table | a table that no stage of the run writes: the caller installs its value when the run is constructed | [`Externals`](crate::core::run::Externals) |
| value catch-up | the work done on demand when something comes to render a synthesized value: the plan's rewrites and the per-tree stages the consumer depends on run over that one tree, and what they compute goes into a catch-up store that a view reads on top of the run's store | [value catch-up](crate::guide::concepts_overview#value-catch-up) |
| service, service slot | a service is a value that can be shared across threads which one feature supplies for another feature, or for the embedding program, to use; a service slot is the declaration, by the feature that owns an interaction, of a dotted name, the type of the values the slot takes and its arity | [`crate::core::services`] |
| peer declaration | a feature's forward declaration of an element, element type, table, service slot, scoped package or backend that another feature owns; the registry build resolves it after every feature has registered | [`PeerName`](crate::core::registry::PeerName) |
| set aside (a rule) | a lowering rule is set aside when a peer declaration it depends on resolves to nothing: it is excluded from rule selection and from the completeness check, and the registry build records one Note and succeeds | [`PeerName`](crate::core::registry::PeerName) |
| configuration, option type, settings | a feature's configuration is the plain Rust value the feature is constructed from, fixed when the registry is built; an option type is a plain Rust struct a feature declares, whose values are set on a document or on a render request (its binding says which); settings are configuration and option values written as data, one map per feature | [settings, options and configuration](crate::guide::concepts_overview#settings-options-and-configuration), [`crate::model::options`], [`crate::core::settings`] |
| collection, refinement | a collection is a name for a set of capabilities that any feature may add to: demanding it demands every member, and it can be demanded but never provided; a refinement is a capability that refines a parent capability: providing it satisfies a demand for the parent, and at most one refinement is the parent's default | [`CollectionDef`](crate::core::registry::CollectionDef), [refinement](crate::guide::pipeline#refinement) |
| output | a name a caller asks for to have something produced (`text`, `html`, `latex-rendered`, `latex-recomposed`), which ties together the capability the plan must reach, the table that holds the resulting artifact set, and the entry artifact's media type and file extension | [`OutputDef`](crate::core::registry::OutputDef) |

### Rendering

| term | definition | canonical location |
|---|---|---|
| backend | the registration bundle for one output format: the elements its emitter writes natively, a lowering stage, an emit stage and a final stage | [`BackendDef`](crate::core::registry::BackendDef) |
| lowering | rewrites each frozen tree into a presentation tree for one backend, by applying the lowering rule selected for each element, in rounds, until only elements the backend writes natively remain; the result is computed data in the table `flm.lowered.<backend>` | [backends, lowering, and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs) |
| lowering rule | a named registry entity written for one element: it declares what it requires, reads and produces, and implements `Lower`, which stages the element's presentation for one backend | [`LoweringRule`](crate::core::lowering::LoweringRule) |
| to stage a node | to build it into the tree that a lowering rule or a rewrite hook is producing, through the context the rule or hook is handed; the staged nodes exist in that tree only | [`Lowerer`](crate::core::lowering::Lowerer) |
| emitter | a backend's emit stage: it walks the lowered trees and writes the output's bytes into a table, from which the backend's final stage makes the artifact set | [backends, lowering, and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs) |
| artifact, artifact set, entry artifact | an artifact is one file's worth of output held as a value (a folder artifact holds the files of a folder), identified by its producer and a name and never by a path; an artifact set is the artifacts of one output, at most one of which is the entry artifact, the one that represents the output as a whole | [`Artifact`](crate::core::artifact::Artifact), [`ArtifactSet`](crate::core::artifact::ArtifactSet) |
| output region, final stage | an output region is one named part of an output's entry artifact (the content, each piece placed beside it, and on HTML the stylesheets, the scripts, the title and the language); a backend's final stage assembles the entry artifact from the regions in a layout of its own, and a host may assemble the same regions through a template of its own instead | [`Regions`](crate::core::region::Regions), [output regions and the final stage](crate::guide::concepts_overview#output-regions-and-the-final-stage) |
| presentation element, presentation tree | a presentation element is an element that never occurs in a source document: lowering rules stage nodes of presentation elements and a backend's emitter writes them; a presentation tree is a lowered tree, built from presentation elements | [`FeatureBuilder::presentation_element`](crate::core::registry::FeatureBuilder::presentation_element), [`crate::stdcore::backends::pres`] |
| piece | a document-level synthesized value that a backend places before or after the document's content (the title, the table of contents, the list of footnotes, the bibliography), built once by its builder stage into a table named for the piece | [`Piece`](crate::core::piece::Piece) |
| accepted unrenderable element | an element the plan request lists, per backend, as one the backend need not render: the plan builds anyway, and each occurrence lowers to a childless `flm.pres.unsupported` node with one Error diagnostic | [`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported) |
| style and script definition | a stylesheet or a script that a feature's output depends on, recorded in the registry under an identifier; a lowering rule or an emitter requires it by identifier, and the backend decides its delivery | [`StyleDef`](crate::core::registry::StyleDef) |
| recomposed LaTeX, style file | the recomposed LaTeX is the output of the `latex-recomposed` backend: the document written back as LaTeX source that stays as close as possible to what the author wrote; a style file is the `.sty` file written beside the recomposed LaTeX document, holding exactly the definitions that document needs | [`latex_recomposed`](crate::stdcore::backends::latex_recomposed), [`style`](crate::stdcore::backends::latex::style) |
| rendered LaTeX, contract | the rendered LaTeX is the output of the `latex-rendered` backend: resolved content (numbers, references, table of contents, notes computed by FLM) for LuaLaTeX alone, one `\flmx…` command or environment per presentation element, every payload field a key of its key-value argument; the contract is that set of commands, keys, roles and kind tokens, stated as data, which a LaTeX template redefines by registering code under a role or kind (`\flmxDefineBlock{<role>}{…}{…}`); a style file `<stem>.<hash>.sty` (stem `flmx`) is always written. A product independent of the recomposed LaTeX | [`latex_rendered`](crate::stdcore::backends::latex_rendered), [`contract`](crate::stdcore::backends::latex_rendered::contract) |
| package request, definition piece, LaTeX hook | on the LaTeX outputs, a lowering rule requires LaTeX packages (`flm.latex.pkg.<name>`) and units of LaTeX definitions (`flm.latex.def.<name>`) by identifier; the style file holds what the document required; a LaTeX hook is a macro or environment the recomposed LaTeX writes and the style file defines with a plain default, which a user redefines in the preamble | [`packages`](crate::stdcore::backends::latex::packages), [`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef), [`preamble`](crate::stdcore::backends::latex::preamble), [LaTeX package requests, definition pieces and hooks](crate::guide::concepts_overview#latex-package-requests-definition-pieces-and-hooks) |

### References and numbering

| term | definition | canonical location |
|---|---|---|
| reference point | one place in a document that can be referred to, numbered, or given an anchor (a heading, an equation line, a theorem, a footnote) | [`RefPoint`](crate::model::aspects::RefPoint) |
| label | the text an author writes as `\label{type:label}` after a construct, which the construct reads as a field and attaches to its reference point; the label type (the part before the colon) must be one the point accepts, and a label may be written only once in a document | [reference points, labels and anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors) |
| anchor | the identifier a link points at for a reference point, computed once by the refs feature and written by the backends as the link target | [`ANCHORS`](crate::stdcore::features::refs::ANCHORS) |
| reference site | one place that points at a reference point (a `\ref`, a `\hyperref`, a proof's back-reference, a term link) | [`RefSite`](crate::model::aspects::RefSite) |
| counter, counter style | a counter is a dotted name under which things are counted in document order; a counter style says how a counter value is shown (the number template, the prefix words, the delimiters, the join texts) | [`CounterDecl`](crate::stdcore::features::numbering::CounterDecl), [`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl), [counters and counter styles](crate::guide::concepts_overview#counters-and-counter-styles) |
| xnote, xnote kind | an xnote is a piece of text set apart from the running text, marked where it belongs and shown elsewhere or in place (a footnote, an endnote, a side note); an xnote kind is one way of being an xnote, declared by the feature's configuration | [`xnotes`](crate::stdcore::features::xnotes) |
| user definition | a macro, environment or special character sequence that a document's author or the host defines by substitution (a name, an argument syntax, a template and the parsing modes it is defined for) | [`Definition`](crate::lang::rewrites::userdefs::Definition) |
| template language | FLM text with placeholders (`#1`, `#{name}`, `#{body}`, `#[…]{ref}`), compiled once when the registry is built; user definitions, number templates, within templates, list label templates, heading title templates, theorem heading templates and the defterm lead-in are written in it | [the template language shared by features](crate::guide::concepts_overview#the-template-language-shared-by-features) |

### The host and the `flm` program

| term | definition | canonical location |
|---|---|---|
| host | any program that embeds the library; the host library `flm-main` is the one shipped with the project | the crate root of `flm-main` |
| front matter | the block of YAML between a first line `---` and the next `---` line of a file: its root keys are the author's metadata (title, authors, date), and the reserved key `flm` holds the document's settings | [the front matter](crate::guide::command_line#the-front-matter) |
| extension | a set of files found by name on the host's search path (`NAME.yaml`, or a folder `NAME/` holding `flmextension.yaml`) that provides one or more packages; its manifest's engine reads it: `data` (settings, workflows, compose templates) or `js` (features written in JavaScript) | [extensions](crate::guide::command_line#extensions), [writing one](crate::guide::extending#extensions) |
| `use` entry | `LOCATOR` or `LOCATOR#PACKAGE`: the locator names a compiled package or an extension (never a path; `@` allowed at its start, `:` reserved), `#PACKAGE` one of the extension's packages, `_` when absent | [extensions](crate::guide::command_line#extensions) |
| compose template | the host's text with named places for the parts of an output's entry artifact (its output regions) | [compose templates](crate::guide::command_line#compose-templates) |

The host's terms are canonical in the `flm_main` modules `seed`, `extensions` and
`packages`, and in the crate `flm_ext`, which this crate's documentation cannot link
(the compose template is [`Template`](crate::stdcore::features::compose::Template)); the links of this table
lead to the command-line guide.

**Words the AI guides do not use**, with what replaces them (an agent reading older
text or code comments meets them): *accepted hole* → accepted unrenderable element;
*fixed skeleton*, *skeleton* → the backend's final stage (for a composed page: the rest
of the page, outside its regions); *façade* → the [`Lowerer`](crate::core::lowering::Lowerer)
(or the [`BlockCutter`](crate::lang::rewrites::blocks::BlockCutter), the
[`BlockStructurer`](crate::lang::rewrites::blocks::BlockStructurer), the
[`Expander`](crate::lang::rewrites::userdefs::Expander)); *forced deduction* → provider
selection; *the ladder* → the rule selection order; *scratch store* → catch-up store;
*mode twins* → per-mode definitions; *data package*, *package written as data* → extension (of the engine `data`); *seed* →
front matter; *a fact of the machine* → a value that depends on the machine the program
runs on; *citation style* for the citations feature's `style` → citation counter style
(*citation style* is the host's CSL file only); *safe key* → safe label; *minted*,
*mint* (outside the `mint` module) → assigned, created or derived; *stamped point* → a
[`LatexPoint`](crate::stdcore::backends::latex::LatexPoint) node; *roster* → the
collected declarations, the registered names; *fallback gate* → fallback provider;
*worker-thread contract* → worker-thread requirement; *raw piece* → raw node
(`flm.pres.latex.raw`).

## Writing FLM documents

This section is the author's reference in short; [FLM language
syntax](crate::guide::language_syntax) is the full one, and [The flm command-line
tool](crate::guide::command_line) describes the program. Everything below describes the
standard package as the `flm` program configures it, which is the library's default
configuration except for the few values listed under [the program's
defaults](crate::guide::command_line#the-programs-defaults).

### A document file

An FLM document is a UTF-8 text file, by convention named `*.flm`. It may begin with a
front matter; the rest is the content:

```text
---
title: A short paper
author: [A. Author, B. Author]
date: 2026
flm:
  parsing: {dollar_math: false}
  features:
    headings: {numbering: all-numbered}
---
\section{Introduction}\label{sec:intro}

FLM marks \emph{what} each part is. See \ref{sec:results}.\footnote{A note.}

\section{Results}\label{sec:results}

\begin{equation}
  E = mc^2 \label{eq:energy}
\end{equation}
By \eqref{eq:energy}, \(E\) grows with \(m\).
```

- The title, the authors and the date are FLM text (`title: The \emph{Art} of \(x^2\)`),
  shown before the content. In a YAML text quoted with `"`, a backslash is doubled.
- Under `flm`: `use` (packages, compiled or of extensions, by name), `features` (each feature's settings, or
  `false` to switch it off), `parsing` (`comments`, `comment_prefix`, `dollar_math`),
  `feature_order` (one chain `NAME < NAME < … < NAME` of features' names, or a list of
  chains), `layout` (`blocks`, `inline` or `auto`), `default_profile` (the profile when `-p` gives none). In the primary file, any
  other key under `flm` stops the program with status 2 ([the front
  matter](crate::guide::command_line#the-front-matter)). `flm schema` prints the JSON
  Schema of every settings key; it suggests, without restricting, the compiled packages
  and the extensions installed on the search path for `use` and `initial_use`, and the
  processors among them for `math`.
- An included file (`\input`) may have a front matter too: its metadata and, under `flm`,
  only `parsing` (how its own content is read) and `input_as` (the file opens with a
  heading; see the row *included files* below). Any other key under `flm` in an included
  file is reported, and the file's content is still included.
- Headings are **not numbered** unless the settings say so: the key
  `headings.numbering`, a [`HeadingNumbering`](crate::stdcore::features::headings::HeadingNumbering)
  written `nothing-numbered` (the default), `all-numbered` or `{up-to-depth: 2}`.
  Equations are numbered by default.

### Characters read specially

| written | meaning |
|---|---|
| `\name` (ASCII letters), `\c` (one non-letter) | a command; whitespace after a letter name is part of the command (`\emph x` takes `x` as its argument; `\nope and` prints `\nopeand`) |
| `{…}` | a group (inline content) or a mandatory argument |
| `%%` to the end of the line | a comment; the line break belongs to it, so `word%% c` joins the line to the next |
| a lone `%` | **refused** (`flm.lang.percent-forbidden`), kept as a character; write `\%` for a percent sign |
| `$` | **refused** (`flm.lang.dollar-math-disabled`) unless the front matter sets `flm.parsing.dollar_math: true`; write `\(…\)`, `\[…\]`, or `\$` for a dollar sign |
| a blank line | a paragraph break |
| `\(…\)`, `\[…\]` | inline and display mathematics |
| `~` | a non-breaking space; whitespace to the paragraph layout (below) |
| `\%` `\#` `\&` `\$` `\{` `\}` `\textbackslash` `\ ` | the character itself (`\ ` is a space never merged with the spaces around it; whitespace to the paragraph layout, as `~` is) |
| `\@` | nothing: a hint that a period ends an abbreviation (`cf.\@ the`) or a sentence (`NASA\@.`) |
| `---`, `--`, `...`, two backticks and `''`, `"`, `'` | em dash, en dash, ellipsis, curly quotes (typographic substitutions; not inside mathematics, verbatim content or text kept as typed: `\cite` keys, labels) |

A run of spaces and line breaks inside a paragraph is one space. `~` and `\ ` are
whitespace to the paragraph layout: removed at the edges of a paragraph and of a quotation's
line, between blocks and before `\cite` or `\footnote` (`word~\cite{a}` is `word\cite{a}`);
ignored where a command ignores whitespace (`\footnote{~}` is empty, `\item[~]` is
`\item[]`), except in text a command reads as typed or with its markup (a `\cite` key, a
label, a proof's back-reference, a term), where they are part of the text (`\cite{~a}`
cites `~a`, `[~*lem:a]` is a malformed back-reference); kept everywhere else, each a space
of its own, never merged with the spaces around it (`a~~b` keeps both). A command written in
text that no feature declares is an Error
([`UnresolvableCommand`](crate::techy::core::constructs::UnresolvableCommand), matched by
its `IDENTIFIER`) and is kept as written; an unknown environment is an Error and its body is still read. Inside
mathematics, under the math processor `math-passthrough`, an unknown command is accepted without a report; a
command of another feature (`\href`, `\section`) is declared for text only, so inside a formula it is
such an unknown command. Under the `math-mathjax` extension, a command that MathJax's packages do not
define is an Error inside a formula too.

### The commands of the standard package

| construct | written | notes |
|---|---|---|
| inline styles | `\emph{…}` `\textbf{…}` `\textit{…}` `\texttt{…}` | [`textstyles`](crate::stdcore::features::textstyles) |
| headings | `\section` `\subsection` `\subsubsection` `\paragraph` `\subparagraph`, each `\cmd*[numbered=…, toc=…, alt={…}]{Title}\label{sec:x}` | levels 1 to 5; the star is `numbered=false` and nothing else; the bracket holds key-values, not a short title (`\section[Short]{…}` is the Error `flm.heading.unknown-key`); `alt={Short}` is the alternative title the table of contents shows (a reference still shows the full title; `alt` without a value is the Error `flm.heading.missing-value`); `pdfbookmark` is reserved (Warning); label types `sec`, `topic` ([`headings`](crate::stdcore::features::headings)) |
| labels | `\label{type:label}` after a construct that declares it | a field, not a command: after `\emph{x}` it is an unknown macro ([labels](crate::guide::language_syntax#writing-a-label)) |
| references | `\ref[options]{type:label}`, `\ref{eq:a,eq:b}`, `\hyperref[type:label]{text}` | shows the target's number, else its content, else the label; options `noprefix`, `S` (capitalized word), or a variant name (`lowercase`, `abbreviated`, `capital`) ([`refs`](crate::stdcore::features::refs)) |
| mathematics | `\(…\)`, `\[…\]`, environments `equation` `align` `gather` and their starred forms | inside the six environments only: `\\[spacing]`, `\label{eq:x}`, `\tag{A}`/`\tag*{A}`, `\nonumber` (`\notag` is not one of them); `\eqref{eq:x}` shows `(3)`; text in a formula: `\text{…}` and `\textnormal` `\textup` `\textrm` `\textmd` `\textbf` `\textit` `\textsl` `\textsc` `\textsf` `\texttt` `\mbox`, whose argument is ordinary FLM content ([`math`](crate::stdcore::features::math)) |
| verbatim content | `\verbtext+…+`, `\verbcode[lang]+…+`, `\verba[lang]+…+` (any delimiter; `{…}` `[…]` `<…>` `(…)` nest), `\begin{verbatimtext}…\end{verbatimtext}`, `\begin{verbatimcode}[lang]…\end{verbatimcode}` (a block) | no character is special inside ([`verbatim`](crate::stdcore::features::verbatim)) |
| lists | `itemize`, `enumerate`, `description`; `\item[tag]\label{item:x}` | optional `[label=(#{alph}), ref=#{alph}]` or a short form `[(i)]`, `[1.]`, `[-]`; a `description` item needs a tag; nothing but whitespace and comments before the first `\item` ([`lists`](crate::stdcore::features::lists)) |
| footnotes | `\footnote[\label{fn:x}]{content}` | the label goes **inside the optional argument**, never after the footnote; the space before `\footnote` is removed, `~` included ([`xnotes`](crate::stdcore::features::xnotes)) |
| hyperlinks | `\href{address}{text}`, `\url{address}`, `\email{address}` | the address is raw text up to the balancing `}` (`%` and `#` are characters there) ([`href`](crate::stdcore::features::href)) |
| citations | `\cite[note]{key1,key2}`, `\R[S]\cite{key}`, `\R[I]\cite{key}` | adjacent `\cite` calls are one mark; `\cite` removes the space before it (`~` included) and writes its own; `\R` writes "ref."/"refs." before the mark (`S`: capitalized, `I`: no word) ([`citations`](crate::stdcore::features::citations)). The `flm` program finds entries by key prefix: `arxiv:`, `doi:`, `manual:{…}`, `bib:` ([citations](crate::guide::command_line#citations)) |
| defined terms | `\begin{defterm}{term}\label{topic:x}\aliasterm{terms} body \end{defterm}`, `\term[ref]{shown}` | a term link matches the term exactly (case and markup count) ([`defterm`](crate::stdcore::features::defterm)) |
| theorems | `theorem` `proposition` `lemma` `corollary` `conjecture` `definition` `remark` (one shared count), `proof`; `\begin{lemma}[Title]\label{lem:x}\noproofref …` | label types `thm` `prop` `cor` `lem` `rem` `def` `dfn` `x` `topic`; a proof's back-reference `\begin{proof}[*lem:x]` (shown) or `[**lem:x]` (recorded only); a `theorem`/`proposition`/`lemma`/`corollary` that no proof refers back to and that has no `\noproofref` is the Warning `flm.theorems.missing-proof` ([`theorems`](crate::stdcore::features::theorems)) |
| quotations | `quote` (with `\text{…}`, `\lines{…}`, `\attributed{…}`, `\block{…}` sections), `blockquote`, `address` | text written directly in `quote` outside a section is an Error; in `\lines` and `address`, `\\` ends a line and `\indent` at a line's start indents it (`~` does not: it is removed there) ([`quote`](crate::stdcore::features::quote)) |
| floats | `figure`, `table` with `\caption[alt=…]{…}`, `\label{figure:x}`, `\nonumber` in the body | `figure` holds exactly one `\includegraphics`; `table` one `cells` environment or one image; the label type is the float type's name ([`floats`](crate::stdcore::features::floats)) |
| images | `\includegraphics[alt=…, width=5cm, height=…]{path}` | by default only inside a float's body; lengths in `pt`, `mm`, `cm`, `in` ([`graphics`](crate::stdcore::features::graphics)) |
| tables of cells | `\begin{cells} \celldata<H>{A & B} \celldata{1 & 2 \\ 3 & 4} \end{cells}`, `\cell<styles>[row;col]{…}` | by default only inside a `table` float ([`cells`](crate::stdcore::features::cells)) |
| annotations | `\phf{passage}`, `\phf[remark]`, `\phf … \endphf` | exist only for an annotator the settings configure (`annotations.annotators`); none by default ([`annotations`](crate::stdcore::features::annotations)) |
| included files | `\input{file.flm}` | read relative to the including file's folder; a file whose front matter says `flm: {input_as: {heading: section, label: 'sec:x'}}` becomes a heading of its own ([`input`](crate::stdcore::features::input)) |
| user definitions | `\name…` as the settings define it | a document cannot define a command: no `\newcommand`; definitions are settings of the `userdefs` feature (below) ([`userdefs`](crate::stdcore::features::userdefs)) |

**Label types by construct**: heading `sec`, `topic`; equation line `eq` (only a line
that can be numbered); list item `item`; footnote `fn`; theorem `thm`, `prop`, `cor`,
`lem`, `rem`, `def`, `dfn`, `x`, `topic` (not on `proof`); term definition `term`,
`topic`; float the float type's name (`figure`, `table`). A label of a type the
construct does not accept is the Error `flm.refs.label-type-not-accepted` and labels
nothing.

**User definitions in the front matter** (the settings of the `userdefs` feature, read as
[`UserdefsData`](crate::stdcore::features::userdefs::UserdefsData); each entry a
[`DefinitionData`](crate::stdcore::features::userdefs::DefinitionData)):

```yaml
flm:
  features:
    userdefs:
      macros:
        hi: {content: "Hello, #1!", args: "{"}
        vec: {content: {text: "arrow", math: "\\vec{#1}"}, args: "{"}
        unit: {content: "#{n} #{u}", args: [{syntax: optional, name: u}, {code: "{", name: n}], defaults: {u: m}}
        pair: {content: "(#1, #2)", args: [{syntax: delimited-group, delimiters: ["<", ">"]}, "{"]}
      environments:
        note: {content: "[Note: #{body}]"}
```

`args` gives each argument's syntax as a structured syntax or as an argument code. The
structured syntax is a map whose key `syntax` names a standard kind (`mandatory`,
`optional`, `star`, `marker`, `delimited-group`, `optional-delimited-group`,
`any-delimited-group`, `optional-any-delimited-group`, `braced-only`, `embellishments`,
`key-vals`, `chars-group`, `verbatim`, `terminated`) with the kind's parameters beside it
(`delimiters: ["<", ">"]`, `markers`, `marker`, `terminator`, …: [the kind as
data](crate::lang::mint::ArgStdKind#the-kind-as-data)). An argument code is its shorthand
(`{` or `m` mandatory, `[` or `o` optional, `*` or `s` star, `v` verbatim, `r<c1><c2>` a
`delimited-group`, and others: [the table of argument
codes](crate::lang::mint::codes#the-codes)), written alone or under the key `code` of a
map; `chars-group`, `key-vals` and a `terminated` argument with an `opener` or a `head`
have no code. A map holds exactly one of `syntax` and `code`, beside the argument's
`name`, `layout`, `opaque_wrapper`, `docs`, `mode`, `scoped` and `role`. A placeholder is
replaced by the argument's content nodes and nothing else, whatever the argument's kind and
its `opaque_wrapper`: the text is kept as typed only where the template places the
placeholder in an argument that its command reads as typed (`\cite{#1}`, `\ref{#1}`).
Other keys: `defaults`, `argument_number_offset`, `block_role`, `expand` (`per-plan`, the
default, or `always`), `overrides`, `docs`. The template language and its conditionals
(`\IfValueT`, `\IfProvidedTF`, `\ifblank`, …) are described under [the template
language](crate::guide::language_syntax#the-template-language).

### Checking and rendering with the `flm` program

```text
$ flm check paper.flm                         # every check, no output; problems on stderr
$ flm check -p html paper.flm                 # also the problems found while making the HTML
$ flm render paper.flm > paper.html           # the default profile `html`: a complete HTML page
$ flm render -p text paper.flm                # plain text
$ flm render -p latex-recomposed -o out/paper.tex paper.flm   # paper.tex, paper-flmrecomposed.HASH.sty
$ flm render -p latex-rendered -o out/paper.tex paper.flm     # paper.tex, paper-flmx.HASH.sty; lualatex twice
$ flm render -p pdf-latex -o out/paper.pdf paper.flm          # extension profile-pdf-latex; latexmk -lualatex in a temp folder; paper.pdf alone
$ flm render -c 'Some \emph{text}.' -p text   # a document given on the command line
$ flm render -O text.width=60 -p text paper.flm               # one setting, YAML one-line form
$ flm render --use syntax-highlight paper.flm                 # one package more, after the front matter's `use`
$ flm info                                    # the environment at a glance: packages, features, workflows
$ flm info '\section'                         # a command's page, looked up by name
$ flm info -d paper.flm settings/text         # the settings in effect for paper.flm, with where each was written
$ flm info --json -r features/refs            # a feature and every page below it, as JSON
$ flm info --write-flm refs.flm features/refs # one page as an FLM document (uses flmdoc), for flm render
$ flm check --json-diagnostics paper.flm      # problems as JSON records on stderr, one per line
$ flm dump --help                             # inspect trees, tables and outputs as JSON
```

- Each problem is printed as `error:` (or `warning:`, `note:`), the message, an `at
  FILE:LINE:COL` line and `inside …` traceback lines; a last line counts errors and
  warnings ([problems reported by render and
  check](crate::guide::command_line#problems-reported-by-render-and-check)).
- Exit status of `flm render` and `flm check`: 0 done (warnings allowed); 1 the document has errors or an output could
  not be written; 2 the command could not start (arguments, front matter, settings,
  package, profile, workflow); 3 a failure of the program itself. `flm dump` has statuses of its
  own ([exit status of flm dump](crate::guide::command_line#exit-status-of-flm-dump)).
- `flm info [PATH…]` describes the environment (never the content): pages with paths
  `packages/NAME`, `features/NAME`, `features/NAME/macros/MACRO` (also `environments/`,
  `specials/`, `scopedpackages/PKG/…`), `settings/FEATURE`, `settings/parsing`,
  `settings/front-matter`, `workflows/NAME`, `namespaces/NS/NAME`, `program/`,
  `program/paths`, `program/extensions`. A name that is no path is looked up (`section`,
  `text.width`, `flm.refs.resolve`). `-d DOC` describes that document's environment; the
  build options (`-p`, `--use`, `--initial-use`, `--math`, `--extensions-dir`, `-O`) shape
  the environment described, never how the pages print. Text output at the terminal's
  width (`COLUMNS`), else 80 columns; `--json` prints `{meta, pages: […]}` (`meta`:
  `kind: info`, `flm` version, `format: 1`), each page `{path, kind, title, summary, body,
  children}`; `-r` writes children as full pages. Status 0 shown (warnings allowed), 2
  arguments, document, environment, or a path and name with no page, 3 a failure of the
  program ([flm info](crate::guide::command_line#describing-the-environment-flm-info)).
- `--json-diagnostics` (any command but `flm dump`, before or after its name): standard
  error carries one JSON object per line, a header `{"kind":"diagnostics","flm":…,"format":1}`
  then records `diagnostic` (`severity`, `id`, `data`, `message`, `place`, `frames`,
  `made_from`), `problem` (`severity` `error` or `warning`, `message`, `place`: a problem
  that stopped the command, or the program's own error or warning, such as `flm info`'s
  warning about a documentation text shown as plain text), `summary` (`errors`,
  `warnings`), `build` (the preview), `log`.
  A place: `file`, `line` from 1, `column` from 1 in bytes, and in a file's parsed content
  `offset` (bytes from the top of the file) and `end`; none in a front matter or manifest
  ([records](crate::guide::command_line#problems-as-json-records---json-diagnostics)).
- A document with errors writes no output unless `--continue-with-errors` is given.
- A plain `flm check` does not report what is found only while an output is made: a
  missing image file, a failed conversion, a list's or a float's or a table of cells'
  mistakes (reported at lowering). Use `flm check -p html` to see them.
- `-p NAME` gives the profile: it uses the extension `profile-NAME` when one exists (on
  the search path or compiled in), after the document's packages, and runs the workflow
  `NAME`; without `-p`, the front matter's `default_profile`, else `html` (`check` for
  `flm check`). No workflow `NAME` is status 2 ([profiles and
  workflows](crate::guide::command_line#profiles-and-workflows)).
- A command produces every output of its workflow, or the one `-f` names. Standard output
  receives one file in total: more (the recomposed LaTeX with its style file; the
  rendered LaTeX, always with its style file; a workflow of two outputs) is status 1; give `-o FILE` or `-o FOLDER/`. `-o NAME` gives every file
  one stem: `NAME` less the extension of any output the command produces (`paper.html`,
  `paper.txt`, `paper` all give `paper`), `index` in a folder; entry `STEM.EXT`, other
  files `STEM-NAME.EXT`. A workflow listing `latex-recomposed` with `html`, `text` or
  `latex-rendered` cannot be planned (status 2); `latex-rendered` with `html` or `text`
  can. `flm render --help` lists the program's compose templates for each output (`-t`).
  A compose template's settings are given under the output's name, then the template's
  name: `-O 'compose.template_config={latex-rendered: {simple: {documentclass: report}}}'`.
  The `latex-rendered` templates are written with the delimiters `<<* *>>`, `<<< >>>`,
  `<<!-- -->>`.

### Mistakes an author makes most often

1. A `%` comment or a `$` formula written as in LaTeX: use `%%` and `\(…\)`.
2. `\label` written after `\footnote{…}`, or inside a float's body nested in a group:
   the footnote's label goes in its optional argument; a float's fields must be written
   directly in its body.
3. A label type the construct does not accept (`\section{…}\label{eq:x}`).
4. `\section[Short title]{…}`: the bracket holds key-values only.
5. `\includegraphics` outside a float, `\begin{cells}` outside a `table` float, unless
   the settings make them global (`graphics.global`, `cells.global`).
6. A command defined in the document's body (`\newcommand`): define it in the front
   matter under `flm.features.userdefs`.
7. `\notag` in a display-math body: write `\nonumber`.
8. Expecting a misspelled command inside a formula (`\(\alpah\)`) to be reported: inside
   mathematics an unknown command is kept without a report under `math-passthrough` (the
   `math-mathjax` extension reports it); outside mathematics an unknown command
   (`\alpha` in running text) is an Error.

## The modules of flm-core and their layering

`flm-core`'s modules:

| module | what it holds |
|---|---|
| [`crate::model`] | the document model: [names](crate::model::names), [addresses](crate::model::addr), [elements](crate::model::element) and [aspects](crate::model::aspects), [fragments and documents](crate::model::doc), [assembly of a document from its files](crate::model::assemble), [option types](crate::model::options), the [digest](crate::model::digest) |
| [`crate::lang`] | FLM's language over techy: [building the language](crate::lang::build), [parsing](crate::lang::parse), [building techy packages from element declarations](crate::lang::mint) (minting), the [recursion guard](crate::lang::guard), the [three rewrite passes](crate::lang::rewrites) (included files spliced, user definitions expanded, block structure), [copies into synthesized values](crate::lang::copy) |
| [`crate::core`] | the processing framework: [computed data](crate::core::data) ([tables](crate::core::data::table), [the store](crate::core::data::store)), the [registry](crate::core::registry), [plan construction](crate::core::plan), the [runner](crate::core::run), [stages](crate::core::stage), [services](crate::core::services), [lowering rules](crate::core::lowering), [artifacts](crate::core::artifact), [output regions](crate::core::region), [pieces](crate::core::piece), [settings](crate::core::settings) |
| [`crate::stdcore`] | the standard package: the [features](crate::stdcore::features) of the standard language and the [presentation elements, the lowering and the backends](crate::stdcore::backends) |
| [`crate::math_passthrough`] | the package `math-passthrough`, the library's math processor, built from `stdcore`'s public items |
| [`crate::layout`] | the text layout engine ([`DocBuf`](crate::layout::DocBuf), [`print`](crate::layout::print)) used by the text backend |
| [`crate::dump`] | inspection and test tools: [projection](crate::dump::project) of trees, tables and diagnostics, the [serialized parse](crate::dump::session) of `flm dump`, [golden files](crate::dump::golden), [diagnostic assertions](crate::dump::diag); ordinary public interface behind no cargo feature |
| [`crate::techy`], [`crate::untechxt`] | the re-exported parser toolkit and Unicode-to-LaTeX encoder ([`DEFAULT_TABLE`](crate::untechxt::builtin::DEFAULT_TABLE), [`Encoder`](crate::untechxt::Encoder), [`PreambleNeeds`](crate::untechxt::preamble::PreambleNeeds); see the recipe [Spell one character as LaTeX](#spell-one-character-as-latex-and-learn-what-its-spelling-needs)) |
| [`crate::guide`] | the guide pages, this one included (documentation only) |

Layering: each module uses only the modules on levels below its own, which the test
`module_layering` of the crate checks:

```text
{model, lang} < core < dump < stdcore < math_passthrough        layout standalone
```

[`crate::model`] and [`crate::lang`] form one level: a node carries its element (the
language needs the model) and the model's fragments are typed by the language (the model
needs the language). [`crate::layout`] uses no other module of the crate. Within
[`crate::stdcore`], only the backends ([`crate::stdcore::backends`]) use the layout
engine and the Unicode-to-LaTeX encoder.

## Embedding: the everyday flow

```text
&str → Arc<FlmSource> ──parse_fragment──→ Fragment ──DocContent::place──→ Doc
registry + demands ──Plan::build / Plan::for_workflow──→ Plan   (no document in it)
Doc + Plan + Externals ──Run::new──→ Run ──execute / step──→ tables, frozen trees, diagnostics
```

Inside a run:

```text
rewrite stages (trees change) → trees frozen → compute and check stages
      → lowering stage (per backend) → emit stage → final stage (artifacts)
```

The registry and the plan hold nothing about any document: build them once and reuse
them. [`Plan::key`](crate::core::plan::Plan::key) is a cache key for a plan. [AI guide:
embedding](crate::guide::ai_guide_embedding) covers each step in depth.

### Registry and language

| item | notes |
|---|---|
| [`RegistryBuilder`](crate::core::registry::RegistryBuilder) | [`use_package`](crate::core::registry::RegistryBuilder::use_package), [`feature`](crate::core::registry::RegistryBuilder::feature), [`contribute`](crate::core::registry::RegistryBuilder::contribute) (a service value from the embedding program) |
| [`RegistryBuilder::build`](crate::core::registry::RegistryBuilder::build)`(&`[`RegistryInputs`](crate::core::registry::RegistryInputs)`)` | → `Arc<`[`FlmRegistry`](crate::core::registry::FlmRegistry)`>`, or a [`RegistryBuildError`](crate::core::registry::RegistryBuildError) |
| [`stdcore::package`](crate::stdcore::package)`(`[`StdcoreConfig`](crate::stdcore::StdcoreConfig)`)` | the standard package, configured in Rust; [`stdcore::factory`](crate::stdcore::factory) builds it from settings written as data |
| [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang) | the built language ([`BuiltLang`](crate::lang::BuiltLang)) to parse with; one per registry, shared across threads |
| [`FlmRegistry::build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics) | what the build reported about the configuration (a user definition whose template does not parse); report these once per registry: a build succeeds even when they hold Errors |
| [`workflow`](crate::core::registry::FlmRegistry::workflow), [`output`](crate::core::registry::FlmRegistry::output), [`table_named`](crate::core::registry::FlmRegistry::table_named), [`has_feature`](crate::core::registry::FlmRegistry::has_feature), [`set_aside_rules`](crate::core::registry::FlmRegistry::set_aside_rules) | inspecting a finished registry by name |

[`RegistryInputs`](crate::core::registry::RegistryInputs) carries the parsing settings
(`parsing`, a [`FlmParsing`](crate::lang::FlmParsing): `comments`, `comment_prefix`
(`%%` by default), `dollar_math` (off by default)), the structural element
specifications (`structural`), the recursion guard policy (`guard`, a
[`GuardPolicy`](crate::lang::GuardPolicy)), the parse diagnostics limit
(`diagnostics_limit`), the input resolver (`input_resolver`) and `build_identity`, a
string identifying the host's own version. The parsing settings alone decide the
forbidden characters: `$` is forbidden exactly when `dollar_math` is off, and a lone `%`
unless `comments` is on and `comment_prefix` is exactly `"%"`
([`FlmParsing::percent_forbidden`](crate::lang::FlmParsing::percent_forbidden)); they are
reported as [`PercentForbidden`](crate::lang::PercentForbidden) and
[`DollarMathDisabled`](crate::lang::DollarMathDisabled).

### Parsing

- [`parse_fragment`](crate::lang::parse_fragment)`(&`[`BuiltLang`](crate::lang::BuiltLang)`, Arc<`[`FlmSource`](crate::lang::FlmSource)`>, `[`FragmentLayout`](crate::model::doc::FragmentLayout)`, &`[`ParseOptions`](crate::lang::ParseOptions)`)` → `Result<`[`Fragment`](crate::model::doc::Fragment)`, `[`FragmentError`](crate::lang::FragmentError)`>`.
- The caller builds the source. An [`FlmOrigin`](crate::lang::FlmOrigin)'s canonical path
  is the key of everything keyed by source (diagnostic labels, detection of a file that
  includes itself, a host's input-resolver tables); use
  [`FlmOrigin::none`](crate::lang::FlmOrigin::none) for an in-memory snippet.
- A parse is tolerant: a syntax problem is a diagnostic held in the fragment, and the tree
  covers the whole input. [`FragmentError`](crate::lang::FragmentError) lists the only
  ways a parse yields no tree: `NestingTooDeep` (nesting past the recursion guard),
  `Aborted` (an extension's parser broke a requirement of techy's parser: an error in the
  program), `WorkerThread` (the operating system refused a worker thread);
  [`FragmentError::diagnostic`](crate::lang::FragmentError::diagnostic) turns any of them
  into the one diagnostic to record.
- [`parse_fragment_with_state`](crate::lang::parse_fragment_with_state) starts from a
  parsing state derived with [`BuiltLang::derive_state`](crate::lang::BuiltLang::derive_state),
  for content with parsing settings of its own.
- [`assemble`](crate::model::assemble::assemble) parses a primary file together with the
  files it includes through `\input`.

**The worker-thread requirement.** A worker thread is a thread created with at least
[`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack; every guarded
run (a parse, a rewrite, an emit) must execute on one, and the library's entry points
create one when the calling thread is not marked as a worker thread
([`on_worker_thread`](crate::lang::guard::on_worker_thread); [the recursion guard and
worker threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).
The recursion guard limits stack use by a budget rather than measuring the stack, so the
stack must really be that large. A host need arrange nothing. Wrap a batch of work in
`on_worker_thread` so that the inner entry points do not each create a thread of their
own. A recursive walk of a tree of your own is limited by the recursion guard only if you
build it with [`guard::walker`](crate::lang::guard::walker) (or
[`restager`](crate::lang::guard::restager), [`recomposer`](crate::lang::guard::recomposer)).
Run such a walk inside `on_worker_thread`, so that the stack is as large as the guard's
budget assumes. Call [`mark_worker_thread`](crate::lang::guard::mark_worker_thread) only
on a thread that has that much stack.

### Document

| item | notes |
|---|---|
| [`Fragment`](crate::model::doc::Fragment) | one parse: [`tree`](crate::model::doc::Fragment::tree), [`source`](crate::model::doc::Fragment::source), [`diags`](crate::model::doc::Fragment::diags), [`layout`](crate::model::doc::Fragment::layout); shared as `Arc`, its identity is the pointer |
| [`DocContent::place`](crate::model::doc::DocContent::place) | adds a tree to the arena *and* to the traversal list → [`TreeId`](crate::model::addr::TreeId) |
| [`DocContent::add_tree`](crate::model::doc::DocContent::add_tree) | arena only, for a tree reached through a placing node |
| [`DocContent::resolve`](crate::model::doc::DocContent::resolve) | makes a node place another tree (a heading input uses it) |
| [`DocContent::walk`](crate::model::doc::DocContent::walk) | document order, the only definition of it ([`walk_nodes`](crate::model::doc::DocContent::walk_nodes), [`walk_trees`](crate::model::doc::DocContent::walk_trees)) |
| [`Doc::new`](crate::model::doc::Doc::new), [`Doc::with_options`](crate::model::doc::Doc::with_options) | the content, and the option values bound on the document (its metadata, [`DocMeta`](crate::stdcore::features::meta::DocMeta), and its [`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions)) |

### Plan

| item | notes |
|---|---|
| [`PlanRequest`](crate::core::plan::PlanRequest) | five fields, nothing else decides a plan: `demands`, `prefer`, `lowering`, `unsupported`, `render_options`; a sixth, `workflow`, names the workflow it was made from, for the messages only (not in the plan key) |
| [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow)`(&registry, name, &`[`Overrides`](crate::core::plan::Overrides)`)` | the usual call: a registered workflow, adjusted |
| [`Plan::build`](crate::core::plan::Plan::build)`(&registry, &request)` | → a plan, or a [`PlanError`](crate::core::plan::PlanError) |
| [`workflow_request`](crate::core::plan::workflow_request) | the plan request a workflow makes, to inspect or adjust |
| [`stage_names`](crate::core::plan::Plan::stage_names), [`rewrite_names`](crate::core::plan::Plan::rewrite_names), [`batch_names`](crate::core::plan::Plan::batch_names), [`active_backends`](crate::core::plan::Plan::active_backends), [`externals`](crate::core::plan::Plan::externals), [`warnings`](crate::core::plan::Plan::warnings), [`excluded`](crate::core::plan::Plan::excluded), [`render`](crate::core::plan::Plan::render) | reading a plan back ([AI guide: pipeline](crate::guide::ai_guide_pipeline)) |

### Run

| item | notes |
|---|---|
| [`Run::new`](crate::core::run::Run::new)`(Arc<Doc>, Arc<Plan>, `[`Externals`](crate::core::run::Externals)`)` | checks the inputs and resolves option values; executes nothing. Its failures are [`RunError`](crate::core::run::RunError)s, never statements about the document |
| [`Run::execute`](crate::core::run::Run::execute), [`Run::step`](crate::core::run::Run::step) | to the end, or one batch ([`Step`](crate::core::run::Step)); stepping changes when work happens, never what is computed |
| [`RunOptions`](crate::core::run::RunOptions) | `sequential`, `continue_with_errors`, `max_diagnostics` |
| [`Run::table`](crate::core::run::Run::table) | typed read of any table of the registry; `Ok(None)` means no scheduled stage writes it |
| [`Run::frozen_tree`](crate::core::run::Run::frozen_tree), [`frozen_trees`](crate::core::run::Run::frozen_trees), [`tree_order`](crate::core::run::Run::tree_order), [`computed`](crate::core::run::Run::computed) | the trees the stages read, and the whole store |
| [`Run::diagnostics`](crate::core::run::Run::diagnostics) | a [`DiagnosticsView`](crate::core::run::DiagnosticsView): each fragment's parse diagnostics (once per fragment), then the stages'; [`pipeline_diagnostics`](crate::core::run::Run::pipeline_diagnostics) holds the stages' alone |
| [`Run::stamp`](crate::core::run::Run::stamp), [`Externals::install`](crate::core::run::Externals::install), [`Externals::install_unstamped`](crate::core::run::Externals::install_unstamped) | external tables, and the [`Stamp`](crate::core::run::Stamp) (the registry's identity, the rewrite stages that ran and a digest of the option values they read) with which `Run::new` refuses data computed under different rewrites ([AI guide: pipeline](crate::guide::ai_guide_pipeline#external-tables)) |

### Reading the outputs and the diagnostics

- **Text**: [`TEXT_OUTPUT`](crate::stdcore::backends::text::TEXT_OUTPUT)
  (`flm.output.text`) holds a [`TextOutput`](crate::stdcore::backends::text::TextOutput):
  the printed `text` and its position marks
  ([`marks`](crate::stdcore::backends::text::TextOutput::marks),
  [`TextMark`](crate::stdcore::backends::text::TextMark): which byte and line/column
  range each lowered node produced; [`slice`](crate::stdcore::backends::text::TextOutput::slice),
  [`marks_at`](crate::stdcore::backends::text::TextOutput::marks_at)). Read it for text
  with positions.
  [`TEXT_DOCUMENT`](crate::stdcore::backends::text::TEXT_DOCUMENT)
  (`flm.output.text.document`) holds the artifact set, whose entry artifact is
  `text:document`, media type `text/plain`.
- **HTML**: [`DOCUMENT`](crate::stdcore::backends::html::DOCUMENT)
  (`flm.output.html.document`) holds the page, `html:document`, `text/html`;
  [`OUTPUT_TABLE`](crate::stdcore::backends::html::OUTPUT_TABLE) (`flm.output.html`)
  holds each tree's HTML and the style and script definitions it requires, for a host
  that assembles its own page.
- **Rendered LaTeX**: [`DOCUMENT`](crate::stdcore::backends::latex_rendered::DOCUMENT)
  (`flm.output.latex-rendered.document`) holds the document (`latex-rendered:document`, a
  complete `article` document under the backend's own final stage) and always its style
  file (`latex-rendered:<stem>.<hash>`, extension `sty`, media type `application/x-latex`;
  [`style_artifact`](crate::stdcore::backends::latex_rendered::style_artifact) finds it),
  then the image copies of
  [`ARTIFACTS`](crate::stdcore::backends::latex_rendered::ARTIFACTS); the style file is
  also alone in [`STYLE_FILE`](crate::stdcore::backends::latex_rendered::STYLE_FILE), and
  each tree's LaTeX with its requirements in
  [`OUTPUT_TABLE`](crate::stdcore::backends::latex_rendered::OUTPUT_TABLE)
  (`flm.output.latex-rendered`).
- **Recomposed LaTeX**: [`DOCUMENT`](crate::stdcore::backends::latex_recomposed::DOCUMENT)
  (`flm.output.latex-recomposed.document`) holds the LaTeX document
  (`latex-recomposed:document`) and, where needed, its style file
  (`latex-recomposed:<stem>.<hash>`, extension `sty`, media type `application/x-latex`;
  [`style_artifact`](crate::stdcore::backends::latex_recomposed::style_artifact) finds
  it). Its fixed path is what the host's naming service gives
  (`paper-flmrecomposed.1a2b3c4d.sty` under the `flm` program's `-o paper.tex`), else
  `<stem>.<hash>.sty`; the package name is that path without `.sty`.
- **Artifacts** are identified by [`ArtifactId`](crate::core::artifact::ArtifactId)
  (producer and name), never by a path; a host maps identities to paths with an
  [`ArtifactNamer`](crate::core::artifact::ArtifactNamer), which `flm-core` declares and
  does not implement. `url_of(&ArtifactRef, &NamingScope)`: the artifact (identity, entry
  role, media type, extension, output) and the command's outputs with their extensions;
  an artifact may carry its extension (`Artifact::extension`).
- **Diagnostics**: a [`FlmDiagnostic`](crate::lang::FlmDiagnostic) has a
  [`Severity`](crate::techy::error::Severity), a span and a typed condition with a dotted
  identifier (`flm.refs.unresolved`). **The identifier and the condition's data are
  stable; the message text is not**: match identifiers, never text.
  [`crate::dump::diag`] selects diagnostics by identifier in tests.

### The dump projection

[`crate::dump::project`] renders a tree, a node, a table or a diagnostic as a serialized
value in a vocabulary this library owns (`elem`, `chars`, `args`, `children`, `body`,
`payload`, spans as `line:col` on request). It is one-way: it shows neither parsing
states nor node identifiers and cannot be read back.
[`project_tree`](crate::dump::project::project_tree),
[`Projector`](crate::dump::project::Projector) (reuse one for several projections),
[`Table::project`](crate::core::data::table::Table::project),
[`to_json_pretty`](crate::dump::to_json_pretty). The lossless counterpart is
[`crate::dump::session`], techy's own serialization, which can be read back.

## The standard package

[`crate::stdcore`] registers thirty-three features through
[`stdcore::package`](crate::stdcore::package), configured by
[`StdcoreConfig`](crate::stdcore::StdcoreConfig) (one field per feature that takes a
configuration; fixed when the registry is built). Each module's documentation states
what the feature declares, what it becomes on each backend, and its conditions.

### The features

| # | feature | what it provides | configuration |
|---|---|---|---|
| 1 | `textstyles` | `\emph` `\textbf` `\textit` `\texttt`: element type `flm.textstyle`, one value per command ([`textstyles`](crate::stdcore::features::textstyles)) | — |
| 2 | `literals` | the nine literal characters (`flm.literal`) and `\@` (`flm.sentence-spacing`) ([`literals`](crate::stdcore::features::literals)) | — |
| 3 | `blocks` | the block-structure rewrite `flm.blocks.structure` (table and capability `flm.blocks.structured`, the default refinement of `flm.blocks`), the marker `flm.blocks.raw`, the tree-contract check `flm.blocks.check-contract` ([`blocks`](crate::stdcore::features::blocks)) | [`BlocksConfig`](crate::lang::rewrites::blocks::BlocksConfig): the typographic substitutions |
| 4 | `index` | the element index `flm.element-index`, written by `flm.index.build` ([`index`](crate::stdcore::features::index)) | — |
| 5 | `refpoints` | the table of every reference point, `flm.refpoints`, written by `flm.refpoints.scan` ([`refpoints`](crate::stdcore::features::refpoints)) | — |
| 6 | `numbering` | five service slots (counters, counter styles, within-joiners, number writers, parent sets), the counting stage `flm.numbering.number` and the table `flm.counters`, the document option [`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions) ([`numbering`](crate::stdcore::features::numbering); [the numbering guide](crate::guide::numbering)) | [`NumberingConfig`](crate::stdcore::features::numbering::NumberingConfig): counter styles that replace declared ones by name |
| 7 | `math` | `\(…\)`, `\[…\]`, the six display environments (`flm.math.environment`), their scoped package `flm.math.lines`, the text commands of math mode (`flm.math.text`), `\ensuremath` in both modes (`flm.math.ensuremath`: in text an inline formula, inside a formula its argument), `\eqref`, the counter `flm.math.equation`, the capability `flm.math.content` (declared; a processing feature provides it and adds its check to `flm.validated`), the capability `flm.math.rendered` (declared, no provider), the library of plain functions processing features build their rules with; one rule on a formula, the rule of last resort of `flm.math.environment` (`flm.math.environment.lower.default`, never a default: the Error `flm.math.unprocessed` where no processing feature lowers the environment) ([`math`](crate::stdcore::features::math)) | [`MathConfig`](crate::stdcore::features::math::MathConfig): whether equations are numbered (yes) |
| 8 | `refs` | `\label` (scoped package `flm.refs.label`), `\ref` (`flm.ref`), `\hyperref` (`flm.hyperref`); stages `flm.refs.scan`, `flm.refs.anchors`, `flm.refs.resolve`, `flm.refs.check-labels`, the safe labels of the recomposed LaTeX; tables `flm.labels`, `flm.anchors`, `flm.refs.targets`, `flm.refs.resolutions`, the external table `flm.refs.external-index` ([`refs`](crate::stdcore::features::refs)) | [`RefsConfig`](crate::stdcore::features::refs::RefsConfig): the texts joining several targets |
| 9 | `headings` | the heading kinds and commands, element `flm.heading`, counters `flm.headings.<kind>` in the parent set `flm.headings.levels` ([`headings`](crate::stdcore::features::headings)) | [`HeadingsConfig`](crate::stdcore::features::headings::HeadingsConfig): kinds, commands, numbering depth (none), run-in levels (none) |
| 10 | `verbatim` | the three verbatim macros (`flm.verbatim.macro`) and two environments (`flm.verbatim.environment`) ([`verbatim`](crate::stdcore::features::verbatim)) | — |
| 11 | `lists` | the list environments (`flm.lists.list`), `\item` in the scoped package `flm.lists.items`, counters `flm.lists.<kind>` ([`lists`](crate::stdcore::features::lists)) | [`ListsConfig`](crate::stdcore::features::lists::ListsConfig): `itemize`, `enumerate`, `description` |
| 12 | `xnotes` | the xnote kinds (`flm.xnotes.xnote`, one value per kind), the service slot `flm.xnotes.kinds`, the stage `flm.xnotes.collect` and table `flm.xnotes`, per listed kind the piece `flm.xnotes.list.<kind>` ([`xnotes`](crate::stdcore::features::xnotes)) | [`XnotesConfig`](crate::stdcore::features::xnotes::XnotesConfig): one kind, `footnote` |
| 13 | `href` | `\href`, `\url`, `\email`: element `flm.href.link` ([`href`](crate::stdcore::features::href)) | — |
| 14 | `defterm` | the `defterm` environment (`flm.defterm`), `\term` (`flm.term`), `\aliasterm` ([`defterm`](crate::stdcore::features::defterm)) | [`DeftermConfig`](crate::stdcore::features::defterm::DeftermConfig): the lead-in template, `#{term}: ` |
| 15 | `theorems` | the theorem environments (`flm.theorems.theorem`, one value per kind), the check `flm.theorems.check-proofs` ([`theorems`](crate::stdcore::features::theorems)) | [`TheoremsConfig`](crate::stdcore::features::theorems::TheoremsConfig): classes and kinds (eight by default; `simpleset` six, `richset` thirteen) |
| 16 | `quote` | the quote environments (`flm.quote`) and their quotation-section commands (`flm.quote.section`) ([`quote`](crate::stdcore::features::quote)) | [`QuoteConfig`](crate::stdcore::features::quote::QuoteConfig): `quote`, `blockquote`, `address` |
| 17 | `annotations` | one command per annotator (`flm.annotations.annotation`), block-transparent ([`annotations`](crate::stdcore::features::annotations)) | [`AnnotationsConfig`](crate::stdcore::features::annotations::AnnotationsConfig): no annotator |
| 18 | `graphics` | `\includegraphics` (`flm.graphics.image`, in the scoped package `flm.graphics.includegraphics`), the service slots `flm.graphics.resolver`, `flm.graphics.inspector`, `flm.graphics.converters`, the stage `flm.graphics.inspect` (table `flm.graphics.info`), one publish stage per backend ([`graphics`](crate::stdcore::features::graphics)) | [`GraphicsConfig`](crate::stdcore::features::graphics::GraphicsConfig): `global` (off) |
| 19 | `floats` | the float types (`flm.floats.float`, one value per type), the fields package `flm.floats.fields`, the service slots `flm.floats.types` and `flm.floats.content` ([`floats`](crate::stdcore::features::floats)) | [`FloatsConfig`](crate::stdcore::features::floats::FloatsConfig): `figure`, `table` |
| 20 | `cells` | the `cells` environment (`flm.cells.cells`) and its body commands, the service slot of cell styles `flm.cells.styles` ([`cells`](crate::stdcore::features::cells)) | [`CellsConfig`](crate::stdcore::features::cells::CellsConfig): `global` (off) |
| 21 | `citations` | `\cite` (`flm.citations.cite`), `\R` (`flm.citations.refcite`), the service slot of citation providers `flm.citations.providers`, stages `flm.citations.collect`, `flm.citations.retrieve`, `flm.citations.check`, the piece `flm.citations.bibliography` ([`citations`](crate::stdcore::features::citations)) | [`CitationsConfig`](crate::stdcore::features::citations::CitationsConfig): placement (listed), mark form (bracketed), the citation counter style, the heading above the list |
| 22 | `input` | `\input` (`flm.input`) and the rewrite `flm.input.splice` ([`input`](crate::stdcore::features::input)) | — |
| 23 | `userdefs` | every user definition as an element, the rewrites `flm.userdefs.expand-always` and `flm.userdefs.expand`, the capability `flm.userdefs` refined by `flm.userdefs.expanded` (default) and `flm.userdefs.kept`, the rules for kept use sites ([`userdefs`](crate::stdcore::features::userdefs)) | [`UserdefsConfig`](crate::stdcore::features::userdefs::UserdefsConfig): the definitions, the expansion depth limit |
| 24 | `lowering` | the lowering stage of every backend, the rules of the structural elements, the service slots `flm.artifacts.namer` and `flm.output.regions` ([`lowering`](crate::stdcore::backends::lowering)) | — |
| 25 | `meta` | the document option [`DocMeta`](crate::stdcore::features::meta::DocMeta) and the pieces `flm.meta.title`, `flm.meta.author`, `flm.meta.date`, the HTML stylesheet `flm.meta.style` ([`meta`](crate::stdcore::features::meta)) | — |
| 26 | `toc` | the table of contents: the table `flm.toc.entries` and the piece `flm.toc` (off by default; placed before the content), written by the stage `flm.toc.build`; the three synthesized elements `flm.toc`, `flm.toc.list`, `flm.toc.entry` with their rules on every backend (on `latex-recomposed` they write nothing), the HTML stylesheet `flm.toc.style`, the option [`TocOptions`](crate::stdcore::features::toc::TocOptions) ([`toc`](crate::stdcore::features::toc)) | — |
| 27 | `pieces` | the service slot `flm.pieces` and the render option [`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions); each backend's pieces stage ([`pieces`](crate::stdcore::backends::pieces)) | — |
| 28 | `text` | the text backend, the `text` output and workflow ([`text`](crate::stdcore::backends::text)) | — |
| 29 | `latex` | the LaTeX family: construct elements (`flm.pres.latex.macro`, `.environment`, `.raw`, `.definition`), escaping, the sink through which an emitter writes LaTeX ([`LatexSink`](crate::stdcore::backends::latex::LatexSink)), the service slots of package requests `flm.latex.packages` and definition pieces `flm.latex.preamble`, the style file; no backend ([`latex`](crate::stdcore::backends::latex)) | — |
| 30 | `latex-recomposed` | the backend that writes the document back as LaTeX source, over the raw tree, with its output and workflow ([`latex_recomposed`](crate::stdcore::backends::latex_recomposed)) | — |
| 31 | `latex-rendered` | the backend that writes resolved LaTeX for LuaLaTeX, over the structured tree, through the contract of `\flmx…` commands ([`contract`](crate::stdcore::backends::latex_rendered::contract)); its nine definition pieces `flm.latex.def.flmx.*`, its output and workflow ([`latex_rendered`](crate::stdcore::backends::latex_rendered)) | — |
| 32 | `html` | the HTML backend, the construct element `flm.pres.html.tag`, the HTML contract ([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)), the `html` output and workflow ([`html`](crate::stdcore::backends::html)) | — |
| 33 | `check` | the collection `flm.validated` and the `check` workflow ([`check_feature`](crate::stdcore::check_feature)) | — |

`stdcore` processes no mathematics. A registry that renders formulas holds one *math
processor*, a package beside it whose feature processes the formulas and adds its check
to `flm.validated`; with none, a document without formulas renders without error and a
render reports each formula as the Error `flm.math.unprocessed` (the rules of last resort
of `flm.math` and `flm.math.environment`), and two are an error. The library's math
processor is the package `math-passthrough`
([`math_passthrough::package`](crate::math_passthrough::package)), one feature that uses
`stdcore`'s public items: the fallbacks of math mode and their element
`flm.math-passthrough.opaque` (an unknown command kept as written, no report), the check
`flm.math-passthrough.check` (provides `flm.math.content`, which the feature adds to
`flm.validated`; reports `flm.math-passthrough.empty-line`), the rules of `flm.math` and
`flm.math.environment` on
every backend, with rules of their own for the recomposed LaTeX, staging
`flm.pres.math-passthrough` (the source for MathJax in the browser on `html`)
([`math_passthrough`](crate::math_passthrough)). The extension `math-mathjax` is another
math processor, the `flm` program's default (not guaranteed to stay the default). A
document chooses its processor with `flm.math` in its front matter: a name `NAME` for the
package `math-NAME` (`mathjax`, `passthrough`), or `false` for none; the host's default is
`Request::default_math`, which `flm-main` leaves at no processor.

Facts that cross features:

- **Mistakes found at lowering are not reported by `check`**: the lists, floats, cells,
  citations' mark rules and graphics options are read when a backend lowers them.
- **A feature that is not registered does not make the others fail**: a feature's rules
  for another feature's backend or elements are declared through peer declarations and
  set aside when that feature is not registered; a feature that reads another's table
  optionally renders without it (a term link without `refs` is its text; numbered lists
  without `numbering` report `flm.lists.no-numbering`).
- **The recomposed LaTeX works on the raw tree** (`flm.blocks.raw`) and writes back what
  the author wrote, leaving numbering and cross-references to LaTeX through LaTeX hooks;
  a `per-plan` user definition is kept and written as a call, with its definition in the
  style file ([`userdefs::kept`](crate::stdcore::features::userdefs::kept)).

### Workflows

A workflow ([`WorkflowDef`](crate::core::registry::WorkflowDef)) is a named set of
demands together with the choices that go with them.

| name | demands |
|---|---|
| `text` ([`text::WORKFLOW`](crate::stdcore::backends::text::WORKFLOW)) | the `text` output and the collection [`VALIDATED`](crate::stdcore::VALIDATED) |
| `latex-recomposed` ([`latex_recomposed::WORKFLOW`](crate::stdcore::backends::latex_recomposed::WORKFLOW)) | the `latex-recomposed` output and `VALIDATED`; its rules need raw trees, so it never shares a plan with `text`, `html` or `latex-rendered`, whose rules need structured trees |
| `latex-rendered` ([`latex_rendered::WORKFLOW`](crate::stdcore::backends::latex_rendered::WORKFLOW)) | the `latex-rendered` output and `VALIDATED`; it shares a plan with `text` and `html` |
| `html` ([`html::WORKFLOW`](crate::stdcore::backends::html::WORKFLOW)) | the `html` output and `VALIDATED` |
| `check` ([`CHECK_WORKFLOW`](crate::stdcore::CHECK_WORKFLOW)) | `VALIDATED` (`flm.validated`) alone: every check of the registry, no output |

Every workflow demands `VALIDATED`, so a render runs the checks that `check` runs, over
the document's trees. One severity differs: an unresolved reference is a Warning under
`check` and an Error under a render (unless
[`RefsRenderOptions`](crate::stdcore::features::refs::RefsRenderOptions) allows it). A
registry that holds a standard backend feature must also hold
[`check_feature`](crate::stdcore::check_feature), which declares the collection; otherwise
the registry build fails with an error that gives the backend's workflow and the missing
`flm.validated`. The
`flm` program adds the workflows `bibtex` and `csl-json` (exports of the cited works'
records).

### Outputs

| output | table | artifacts |
|---|---|---|
| `text` | `flm.output.text.document` ([`TEXT_DOCUMENT`](crate::stdcore::backends::text::TEXT_DOCUMENT)) | `text:document`, `text/plain` |
| `html` | `flm.output.html.document` ([`DOCUMENT`](crate::stdcore::backends::html::DOCUMENT)) | `html:document`, `text/html` (a complete page), then every artifact of [`flm.output.html.artifacts`](crate::stdcore::backends::html::ARTIFACTS) (copies of images, written by the `graphics` feature's publish stage) |
| `latex-rendered` | `flm.output.latex-rendered.document` ([`DOCUMENT`](crate::stdcore::backends::latex_rendered::DOCUMENT)) | `latex-rendered:document` (a complete `article` document), the style file `latex-rendered:<stem>.<hash>` (always; path from the naming service, else `<stem>.<hash>.sty`), copies of images from `flm.output.latex-rendered.artifacts` |
| `latex-recomposed` | `flm.output.latex-recomposed.document` ([`DOCUMENT`](crate::stdcore::backends::latex_recomposed::DOCUMENT)) | `latex-recomposed:document`, the style file `latex-recomposed:<stem>.<hash>` (path from the naming service, else `<stem>.<hash>.sty`; also in [`flm.output.latex-recomposed.style`](crate::stdcore::backends::latex_recomposed::STYLE_FILE)), copies of images from `flm.output.latex-recomposed.artifacts` |

Each backend writes its output regions into `flm.output.<backend>.regions` and
contributes a [`RegionsOutput`](crate::core::region::RegionsOutput) to the service slot
`flm.output.regions`, so that a host can assemble the entry artifact itself (the `flm`
program does, through a compose template).
[`ArtifactSet::insert_entry_with`](crate::core::artifact::ArtifactSet::insert_entry_with)
merges the artifact tables of a regions output beside the entry artifact, drops an
artifact equal to one already held, and reports each violation as an Error
(`flm.artifacts.duplicate`, `flm.artifacts.second-entry`).

### Rendering in brief

Lowering, emitting and assembling are three ordinary stages per backend ([backends,
lowering, and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs);
[Rendering](crate::guide::rendering)): the [lowering
stage](crate::stdcore::backends::lowering::LoweringStage) rewrites each frozen tree into a
presentation tree built from the elements every backend shares
([`crate::stdcore::backends::pres`]) and stores it in `flm.lowered.<backend>`; the emit
stage (text: [`TextEmitStage`](crate::stdcore::backends::text::TextEmitStage)) walks
those trees and writes the output; the final stage (text:
[`TextFinalStage`](crate::stdcore::backends::text::TextFinalStage)) publishes the artifact
set. A lowering rule ([`LoweringRule`](crate::core::lowering::LoweringRule)) is written
for an element, not for a backend; one rule may serve every backend. A piece is lowered by
the backend's pieces stage ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage),
through [`lower_value`](crate::stdcore::backends::lowering::lower_value)) with the same
rules; a piece that cannot be lowered (for example one holding a labeled reference
point) is left out and reported (`flm.pieces.lowering-failed`). The text backend's emitter
builds a [layout document](crate::layout::DocBuf) and [`print`](crate::layout::print)s it
at the requested width.

### Tables of the standard package

| table | holds |
|---|---|
| `flm.input.spliced`, `flm.userdefs.always-expanded`, `flm.userdefs.expanded`, `flm.blocks.structured` | the trees after each rewrite |
| `flm.element-index` ([`ELEMENT_INDEX`](crate::stdcore::features::index::ELEMENT_INDEX)) | per tree, the nodes and their elements |
| [`flm.refpoints`](crate::stdcore::features::refpoints::REFPOINTS) | every reference point, read in document order with [`in_document_order`](crate::stdcore::features::refpoints::RefPoints::in_document_order) |
| [`flm.counters`](crate::stdcore::features::numbering::COUNTERS) | per reference point, its counter value, or nothing for a point the document does not number |
| [`flm.labels`](crate::stdcore::features::refs::LABELS), [`flm.anchors`](crate::stdcore::features::refs::ANCHORS), [`flm.refs.targets`](crate::stdcore::features::refs::REF_TARGETS), [`flm.refs.resolutions`](crate::stdcore::features::refs::REF_RESOLUTIONS) | the label index (with duplicates and refused labels), the anchors, the reference targets (document scope), the resolved references |
| [`flm.refs.external-index`](crate::stdcore::features::refs::EXTERNAL_INDEX) | external: the labels of other documents, installed by the caller |
| [`flm.labels.safe`](crate::stdcore::features::refs::SAFE_LABELS), [`flm.labels.safe.value`](crate::stdcore::features::refs::VALUE_SAFE_LABELS) | the safe labels (`ref1`, `ref2`, … in order of first mention) and the value names (`ref-v1`, …) of the recomposed LaTeX |
| [`flm.xnotes`](crate::stdcore::features::xnotes::XNOTES), `flm.xnotes.list.<kind>` | per xnote kind, each xnote's reference point, mark point and content; each listed kind's piece |
| [`flm.toc.entries`](crate::stdcore::features::toc::ENTRIES), `flm.toc` | the listed headings in document order, each with its level, its first reference point and the content its entry shows (no number, no anchor); the table-of-contents piece |
| [`flm.citations.keys`](crate::stdcore::features::citations::KEYS), [`flm.citations`](crate::stdcore::features::citations::CITATIONS), [`flm.citations.entries`](crate::stdcore::features::citations::ENTRIES), `flm.citations.bibliography` | the cited keys in order of first citation, the numbered works, the citation entries, the bibliography piece |
| [`flm.graphics.info`](crate::stdcore::features::graphics::INFO), `flm.graphics.published.<backend>` | what is known about each image; how each backend refers to it |
| `flm.lowered.<backend>` ([`lowered_table`](crate::stdcore::backends::lowering::lowered_table)), `flm.lowered.<backend>.pieces` ([`lowered_pieces_table`](crate::stdcore::backends::pieces::lowered_pieces_table)) | the presentation trees, and the placed pieces lowered |
| `flm.output.text`, `flm.output.text.regions`, `flm.output.text.document` | the printed text with its position marks, the text's output regions, the artifact set |
| [`flm.output.html`](crate::stdcore::backends::html::OUTPUT_TABLE), [`flm.output.html.pieces`](crate::stdcore::backends::html::PIECES_TABLE), [`flm.html.definitions`](crate::stdcore::backends::html::DEFINITIONS_TABLE), `flm.output.html.regions`, `flm.output.html.document`, `flm.output.html.artifacts` | each tree's HTML and the definitions it requires; each placed piece's HTML; every style and script definition the page requires, once each, with its delivery; the regions; the artifact set; the copies of images |
| [`flm.output.latex-rendered`](crate::stdcore::backends::latex_rendered::OUTPUT_TABLE), [`flm.output.latex-rendered.pieces`](crate::stdcore::backends::latex_rendered::PIECES_TABLE), [`flm.output.latex-rendered.regions`](crate::stdcore::backends::latex_rendered::REGIONS), [`flm.output.latex-rendered.style`](crate::stdcore::backends::latex_rendered::STYLE_FILE), `flm.output.latex-rendered.document`, `flm.output.latex-rendered.artifacts` | the rendered LaTeX: each tree's LaTeX with its requirements and staged definitions, each piece's LaTeX and words, the regions (`content`, one per piece, `style-package`), the style file, the artifact set, the image copies |
| [`flm.output.latex-recomposed`](crate::stdcore::backends::latex_recomposed::OUTPUT_TABLE), [`flm.latex-recomposed.packages`](crate::stdcore::backends::latex_recomposed::PACKAGES_TABLE), [`flm.latex-recomposed.preamble`](crate::stdcore::backends::latex_recomposed::PREAMBLE_TABLE), `flm.output.latex-recomposed.regions`, `.style`, `.document`, `.artifacts` | each tree's recomposed LaTeX and requirements; the merged package requests; the definition pieces; the regions; the style file; the artifact set; the copies of images |

### Capabilities worth knowing

[`flm.blocks`](crate::stdcore::features::blocks::BLOCKS), refined by
`flm.blocks.structured` (the default) and [`flm.blocks.raw`](crate::stdcore::features::blocks::RAW);
[`flm.userdefs`](crate::stdcore::features::userdefs::USERDEFS), refined by
[`flm.userdefs.expanded`](crate::stdcore::features::userdefs::EXPANDED) (the default) and
[`flm.userdefs.kept`](crate::stdcore::features::userdefs::KEPT), which a kept
definition's `emit-call` rule (the default on `latex-recomposed`) and its
`instantiate-at-lowering` rule require;
[`flm.validated`](crate::stdcore::VALIDATED), the collection of checks, whose members in the standard package
are `flm.blocks.contract-checked`, `flm.refs.check-labels`,
`flm.theorems.check-proofs`, `flm.floats.check-handlers`, `flm.citations.check`,
`flm.input.spliced` and `flm.meta.read`, and to which a math processor adds its check
(`math-passthrough` adds `flm.math.content`);
[`flm.math.rendered`](crate::stdcore::features::math::RENDERED), declared with no
provider, for an extension that renders mathematics.

### Option types of the standard package

Each option type's binding ([`Bind`](crate::model::options::Bind)) says where its value
is set; the value a stage reads is the render value if one is set, otherwise the document
value, otherwise the type's default.

| option type | binding | fields |
|---|---|---|
| [`DocMeta`](crate::stdcore::features::meta::DocMeta) | document | `title`, `authors`, `date` (FLM text), `other` (kept, not shown) |
| [`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions) | either: set on the document, and a render request may set it again, its value then replacing the document's for that render | `counters`: per counter, named in full (`flm.math.equation`), what it is numbered within and its value before it is first advanced ([the numbering guide](crate::guide::numbering)) |
| [`TocOptions`](crate::stdcore::features::toc::TocOptions) | either | `depth` (3): the deepest heading level listed in the table of contents |
| [`TextRenderOptions`](crate::stdcore::backends::text::TextRenderOptions) | render | `width` (80; `None` never wraps), `tab_width` (4), `run_in_stop` (`.`), `link_addresses` (on), `link_addresses_when_shown` (off), `root_heading_level` (1) |
| [`HtmlRenderOptions`](crate::stdcore::backends::html::HtmlRenderOptions) | render | `title` (`FLM document` where the document states none), `lang`, `source_spans` (off), `run_in_stop`, `definitions` (deliveries a host gives in place of declared content), `required_definitions`, `heading_tags` (a tag per heading kind), `root_heading_level` |
| [`LatexRecomposedRenderOptions`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions) | render | `unicode` (`Declare` or `Replace`), `unicode_overrides`, `drop_comments`, `skip_packages`, `extra_packages`, `extra_preamble`, `bare_names`, `robust_names`, `style_stem` (`flmrecomposed`; the naming service may prefix it), `style_file` (`WhenNeeded` or `Always`), `heading_hook` (on), `label_hooks` (on), `drop_math_labels_without_refs` (off), `resolved_proof_refs` (off) |
| [`LatexRenderedRenderOptions`](crate::stdcore::backends::latex_rendered::LatexRenderedRenderOptions) | render | `style_stem` (`flmx`), `run_in_stop` (`.`), `skip_packages` (`hyperref` there drops its loading line), `extra_packages`, `extra_preamble`; set from the command line as `-O latex-rendered.KEY=VALUE` |
| [`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions) | render | `include`, `exclude` (pieces by name; an unknown name is the Warning `flm.pieces.unknown-piece`), `backends` (set by a host from what its compose template places; not a setting) |
| [`RefsRenderOptions`](crate::stdcore::features::refs::RefsRenderOptions) | render | `unresolved`: an unresolved reference is an Error (default) or a Warning |
| [`RefsRecomposeOptions`](crate::stdcore::features::refs::RefsRecomposeOptions) | render | `safe_label_types`, `protect` |
| [`MathRecomposeOptions`](crate::stdcore::features::math::MathRecomposeOptions) | render | `surround_display_math`, `emit_flm_math_environment_macro` |
| [`HeadingsLatexOptions`](crate::stdcore::features::headings::latex::HeadingsLatexOptions) | render | `latex_commands`: the LaTeX sectioning command per heading kind |
| [`AnnotationsRenderOptions`](crate::stdcore::features::annotations::AnnotationsRenderOptions) | render | `hidden` (off): a clean copy without annotations |
| [`GraphicsRenderOptions`](crate::stdcore::features::graphics::GraphicsRenderOptions) | render | `strategy`, `convert` and `magnification` (`raster`, `vector`; 1), each a map by output |

An option value set at the wrong site is refused, not ignored: a render-bound type set on
the document is an [`OptionError`](crate::model::options::OptionError) at
[`Run::new`](crate::core::run::Run::new), and an unknown or misplaced render option value
is [`PlanError::RenderOption`](crate::core::plan::PlanError::RenderOption).

## Recipes

### Render a string to plain text

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT, TextRenderOptions};
use flm_core::stdcore::{self, StdcoreConfig};

// 1. Registry: the standard package. Build once, reuse for every document.
let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// 2. Parse. The caller owns the source and its origin.
let source = Arc::new(
    FlmSource::new("\\section{Title}\n\nSome \\emph{emphasized} text.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

// 3. Document: one placed fragment.
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// 4. Plan: the `text` workflow, wrapping at 40 columns. Holds no document.
let overrides = Overrides {
    render_options: OptionValues::new()
        .with(TextRenderOptions { width: Some(40), ..TextRenderOptions::default() }),
    ..Overrides::none()
};
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &overrides)?);

// 5. Run, and read the printed text out of the table the emit stage wrote.
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

let output = run.table(TEXT_OUTPUT)?.expect("the `text` workflow writes it");
assert_eq!(output.text, "Title\n=====\n\nSome emphasized text.\n");
assert!(!output.marks.is_empty()); // the position marks of the lowered nodes
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Recompose a document to LaTeX

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::latex_recomposed::{self, DOCUMENT};
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;
let source = Arc::new(
    FlmSource::new("\\section{Title}\\label{sec:t}\n\nSee \\ref{sec:t}: caf\u{e9} \\emph{x}.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// The `latex-recomposed` workflow: raw tree, safe labels, lowering, emit, final.
let plan = Arc::new(Plan::for_workflow(&registry, latex_recomposed::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// Two artifacts: the recomposed LaTeX (the entry) and the style file it loads.
let set = run.table(DOCUMENT)?.expect("the workflow writes it");
let recomposed = std::str::from_utf8(set.entry().unwrap().as_bytes().unwrap()).unwrap();
assert!(recomposed.ends_with(
    "\\flmHeading[toc=true]\n\\section{Title}{\\flmLabels{\\label{ref1}}}\n\nSee \\flmRef{ref1}: caf\u{e9} \\emph{x}.%%\n"
));
let style = latex_recomposed::style_artifact(&set).unwrap();
let style_text = std::str::from_utf8(style.as_bytes().unwrap()).unwrap();
assert!(style_text.contains("\\RequirePackage{zref-clever}"));
assert!(style_text.contains("\\newunicodechar{\u{e9}}{\\'e}")); // `Declare`, the default mode
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Render a document as LaTeX for LuaLaTeX

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::latex_rendered::{self, DOCUMENT};
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;
let source = Arc::new(
    FlmSource::new("\\section{Title}\\label{sec:t}\n\nSee \\ref{sec:t}: \\emph{x}.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// The `latex-rendered` workflow: structured tree, numbering, references, lowering, emit,
// regions (the style file), final (a complete `article` document).
let plan = Arc::new(Plan::for_workflow(&registry, latex_rendered::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// Two artifacts: the document (the entry) and the style file it loads.
let set = run.table(DOCUMENT)?.expect("the workflow writes it");
let document = std::str::from_utf8(set.entry().unwrap().as_bytes().unwrap()).unwrap();
assert!(document.starts_with("% flm: latex-rendered; style file flmx."));
assert!(document.contains("\\flmxHeading[kind=section, level=1, toc=true, anchor=sec-t, plain={Title}]{Title}"));
assert!(document.contains("\\flmxLink[kind=ref, href={\\#sec-t}, target=sec-t]"));
let style = latex_rendered::style_artifact(&set).unwrap();
let style_text = std::str::from_utf8(style.as_bytes().unwrap()).unwrap();
assert!(style_text.contains("\\RequireLuaTeX"));
assert!(style_text.contains("% flm.latex.def.flmx.headings\n"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Render a document to HTML

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::html::{self, DOCUMENT, OUTPUT_TABLE};
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;
let source = Arc::new(
    FlmSource::new("\\section{Title}\\label{sec:t}\n\nSee \\ref{sec:t}: \\emph{x} & y.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// The `html` workflow: structured trees, the anchors, lowering, emit, final.
let plan = Arc::new(Plan::for_workflow(&registry, html::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// One artifact: the page.
let set = run.table(DOCUMENT)?.expect("the workflow writes it");
let page = std::str::from_utf8(set.entry().unwrap().as_bytes().unwrap()).unwrap();
assert!(page.starts_with("<!DOCTYPE html>"));
assert!(page.contains("<title>FLM document</title>"));
// Document text is escaped where it is written.
assert!(page.contains("&amp; y"));

// The content without the page around it, and what each tree needs.
let fragments = run.table(OUTPUT_TABLE)?.expect("the emit stage writes it");
let tree = fragments.trees.values().next().unwrap();
assert!(tree.html().contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-t\" data-flm-heading=\"section\" data-flm-level=\"1\" data-flm-toc=\"true\">Title</h1>"));
let requires: Vec<&str> = tree.requires.iter().map(|name| name.as_str()).collect();
assert_eq!(requires, ["flm.headings.style", "flm.html.style.core", "flm.html.style.headings"]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Spell one character as LaTeX, and learn what its spelling needs

```rust
use flm_core::untechxt::builtin::DEFAULT_TABLE;
use flm_core::untechxt::lookuptable::LookupTable;
use flm_core::untechxt::preamble::{Engine, PreambleNeeds};
use flm_core::untechxt::protection::{ProtectInput, ReplacementProtection, StandardProtection};
use flm_core::untechxt::report::NoReport;
use flm_core::untechxt::{Encoder, default_rules};

// One character: the table's entry holds the LaTeX, a hint saying which mode
// it is valid in and how it ends, and what a document must load to print it.
let one = DEFAULT_TABLE.lookup('\u{1D7D9}').unwrap(); // 𝟙
assert_eq!(one.encoded, r"\mathds{1}");
let mut needs = PreambleNeeds::new();
needs.include(one.needs.unwrap());
let mut preamble = String::new();
needs.write_preamble_for(Engine::PdfLatex, &mut preamble).unwrap();
assert_eq!(preamble, "\\usepackage{dsfont}\n");

// Written into text, the value is wrapped for its mode by the protection
// strategy, as the LaTeX family's escaping does, character by character.
let mut out = String::new();
StandardProtection::text_mode()
    .write_protected(&mut out, &mut NoReport, ProtectInput::new(one.encoded, one.hint))
    .unwrap();
assert_eq!(out, r"\ensuremath{\mathds{1}}");

// A whole string: the encoder writes the LaTeX and reports the same needs for
// everything it wrote, plus the characters no entry spells (kept as they are
// under the default policy).
let encoder = Encoder::new(default_rules());
let (text, report) = encoder.encode_with_report("caf\u{e9} \u{1D7D9} \u{e0a}").unwrap();
assert_eq!(text, "caf\\'e \\ensuremath{\\mathds{1}} \u{e0a}");
assert_eq!(report.needs.chunks().map(|c| c.id()).collect::<Vec<_>>(), ["dsfont"]);
assert_eq!(report.unknown_chars.len(), 1);
```

### Check a document and read its diagnostics

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::{self, CHECK_WORKFLOW, StdcoreConfig};
use flm_core::techy::core::constructs::UnresolvableCommand;
use flm_core::techy::error::DiagnosticInfo;

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// `\nope` is declared by no feature; `sec:nowhere` is defined nowhere.
let source = Arc::new(
    FlmSource::new("See \\ref{sec:nowhere} and \\nope.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// `check` runs every check the registry holds and produces no output.
let plan = Arc::new(Plan::for_workflow(&registry, CHECK_WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// Parse diagnostics and stage diagnostics arrive in one view. Select by
// identifier, never by message text, which is not a stable interface.
let ids: Vec<&str> = run.diagnostics().iter().map(|d| d.identifier()).collect();
assert!(ids.contains(&"flm.refs.unresolved")); // from the resolve stage
assert!(ids.contains(&UnresolvableCommand::IDENTIFIER)); // from the parse
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Read a computed table

```rust
use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::aspects::Label;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::features::refs::LABELS;
use flm_core::stdcore::{self, CHECK_WORKFLOW, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(
    FlmSource::new("\\section{Introduction}\\label{sec:intro}\n\nText.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;
let mut content = DocContent::new();
let tree = content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));
assert_eq!(tree, TreeId::tree(0));

let plan = Arc::new(Plan::for_workflow(&registry, CHECK_WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// A caller may read any table of the registry. `Ok(None)` would mean that no
// scheduled stage writes it; a declared write always installs, if only empty.
let labels = run.table(LABELS)?.expect("the label scan is scheduled");
let point = labels
    .by_label
    .get(&Label::parse("sec:intro").expect("a well-formed label"))
    .expect("the heading carries it");
assert_eq!(point.addr.tree, tree);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Describe the environment

From the command line: `flm info` prints pages; `--json` gives data; `--write-flm FILE`
writes one page's FLM source, which `flm render` renders. The pages describe
what a document may write and set, never its content.

```text
$ flm info --json -d paper.flm features/            # the features of paper.flm's environment
$ flm info --json -d paper.flm '\cite'              # how \cite is written, its arguments and roles
$ flm info --json settings/text                     # each key: type, default, in_effect {value, origin}
$ flm info --json -p latex-rendered workflows/latex-rendered   # the plan: stages in order
```

From Rust, with the crates `flm-core`, `flm-info` and `serde_json` (the crate
documentation of `flm_info` holds these lines in its tested example):

```rust,ignore
use std::sync::Arc;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::lang::mint::CallableKind;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_info::Environment;

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;
let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;
let factories = vec![stdcore::factory()?, flm_core::math_passthrough::factory()?];
let env = Environment::new(registry, Some(Arc::new(plan)), factories);

let section = env.command(CallableKind::Macro, "section");   // Vec<CommandView>
let json = serde_json::to_value(&section)?;                  // every view serializes
let pages = flm_info::pages::Tree::new(&env, None);          // the pages of `flm info`
```

With the cargo feature `flm-main` of `flm-info`, `Environment::of_prepared(&prepared)`
describes a document prepared by `flm_main::session::prepare`, with the settings in
effect and where each was written.

### Read diagnostics as JSON

From the command line, `--json-diagnostics` turns standard error into JSON lines; standard
output and the exit status are unchanged:

```text
$ flm check --json-diagnostics paper.flm 2> problems.jsonl; echo $?
1
$ head -2 problems.jsonl
{"kind":"diagnostics","flm":"0.5.0","format":1}
{"kind":"diagnostic","severity":"error","id":"core.specs.unresolvable-command","data":{"name":"nosuchmacro","escape_char":"\\","detail":null},"message":"cannot resolve command ‘\\nosuchmacro’","place":{"file":"paper.flm","offset":47,"line":4,"column":27,"end":{"offset":59,"line":4,"column":39}},"frames":[…]}
```

Match on `kind` and `id` (with `data`), never on `message`. From Rust, the host library
makes the same records: `flm_main::report::Record::of_diagnostic(&diagnostic, &offsets)`
for a diagnostic of `flm_main::session::build`, whose `Built::content_offsets` gives the
offsets; each record serializes with `serde_json::to_string`, and `Record::meta()` is the
header.

## Pitfalls for code that uses the library

1. **The recursion guard is a budget, not a measurement.** Guarded work must run on a
   worker thread (at least [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE)
   bytes of stack). The entry points arrange that themselves. A recursive walk of a tree
   of your own is guarded only if you build it with
   [`guard::walker`](crate::lang::guard::walker) (or `restager`, `recomposer`); run it
   inside [`on_worker_thread`](crate::lang::guard::on_worker_thread) so that it has the
   stack the budget assumes. Calling
   [`mark_worker_thread`](crate::lang::guard::mark_worker_thread) on a thread with a
   small stack brings back the stack overflow the requirement prevents. Where the
   operating system refuses a thread, a parse reports
   [`WorkerThreadUnavailable`](crate::lang::parse::WorkerThreadUnavailable) rather than
   running unguarded.
2. **An option value set at the wrong site is refused, not ignored.** Each option type
   carries a [`Bind`](crate::model::options::Bind):
   [`Document`](crate::model::options::Bind::Document) (set with
   [`Doc::with_options`](crate::model::doc::Doc::with_options)),
   [`Render`](crate::model::options::Bind::Render) (set in
   [`PlanRequest::render_options`](crate::core::plan::PlanRequest#structfield.render_options)
   or [`Overrides`](crate::core::plan::Overrides)),
   [`Either`](crate::model::options::Bind::Either), or
   [`Registry`](crate::model::options::Bind::Registry) (the feature holds the value; no
   stage may read one, and a plan whose stage does is refused with
   [`PlanError::RegistryBoundRead`](crate::core::plan::PlanError::RegistryBoundRead)).
   [`TextRenderOptions`](crate::stdcore::backends::text::TextRenderOptions) set on the
   document is an [`OptionError`](crate::model::options::OptionError) at
   [`Run::new`](crate::core::run::Run::new).
3. **A rule selection means nothing outside its own registry.** The rule indices in
   [`SelectedRules`](crate::core::plan::SelectedRules) belong to the registry the plan was
   built against; so does the whole plan. [`PlanKey`](crate::core::plan::PlanKey) covers
   the registry's identity, so it is a safe cache key. In a plan request, refer to rules
   by name ([`LoweringSelection`](crate::core::registry::LoweringSelection)).
4. **A stage reads and writes only what it declared.** An undeclared
   [`StageCx::get`](crate::core::run::StageCx::get) or
   [`StageOut::table`](crate::core::data::store::StageOut::table) is an
   [`AccessError`](crate::core::data::store::AccessError) identifying the stage and the
   table, never a silent `None`. Declare reads in [`Needs`](crate::core::stage::Needs) and
   writes in [`StageDecl`](crate::core::stage::StageDecl). A caller reading through
   [`Run::table`](crate::core::run::Run::table) is not restricted.
5. **The error gate stops only before `Io` stages, and the standard package registers
   none.** A run that recorded an Error stops before the first batch holding an
   [`Effect::Io`](crate::core::stage::Effect::Io) stage, unless
   [`RunOptions::continue_with_errors`](crate::core::run::RunOptions#structfield.continue_with_errors)
   is set. The backends write their output into tables, so
   [`Plan::first_io_batch`](crate::core::plan::Plan::first_io_batch) is `None` for a plan
   built from `stdcore` alone: a document full of errors still renders. Check
   [`DiagnosticsView::has_errors`](crate::core::run::DiagnosticsView::has_errors) where
   errors should stop something.
6. **Placing one fragment twice makes two trees**: two
   [`TreeId`](crate::model::addr::TreeId)s, two sets of computed data. The fragment's
   parse diagnostics are reported once, at the first place the walk reaches it.
7. **Iterating a map keyed by [`NodeAddr`](crate::model::addr::NodeAddr) is not document
   order** (trees in identifier order, nodes in storage order). Document order is
   [`DocContent::walk`](crate::model::doc::DocContent::walk), or in a stage
   [`StageCx::walk`](crate::core::run::StageCx::walk); reading
   [`StageCx::trees`](crate::core::run::StageCx::trees) one after the other is not,
   because a placed tree's nodes come in the middle of the tree that places it: sort by
   [`StageCx::position`](crate::core::run::StageCx::position).
8. **A synthesized value's node identifiers mean something only inside the stage call
   that built it.** Never key a table installed into the run's store by an address inside
   a value; the store refuses a table piece classified as a fact about the document when
   it comes from a [value catch-up](crate::guide::concepts_overview#value-catch-up)
   ([`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue)).
   A catch-up shadows a recomputed table **whole**: a stage run again over a value sees
   none of the document's entries in that table.
9. **Diagnostic messages are not an interface.** Match
   [`Diagnostic::identifier`](crate::techy::error::Diagnostic::identifier) and the
   condition's data. [`crate::dump::diag`] selects by identifier and narrows by covered
   text, position, severity and data.
10. **Depend on techy through [`crate::techy`]**, never on a `techy` crate of your own:
    two versions in one build are two unrelated sets of types.

## Not implemented today

Say so plainly rather than describing any of this as if it existed:

- **Output formats**: the library ships four backends,
  [`text`](crate::stdcore::backends::text),
  [`latex-recomposed`](crate::stdcore::backends::latex_recomposed),
  [`latex-rendered`](crate::stdcore::backends::latex_rendered) and
  [`html`](crate::stdcore::backends::html). The HTML backend writes one page; a
  multi-file HTML output (a second final stage of the same backend) is not implemented.
  The recomposed LaTeX of the theorem environments does not write a counter hierarchy
  (theorems numbered within sections) into the declarations: LaTeX numbers the theorem
  kinds in one sequence regardless of the document's numbering options.
- **Mathematics** is carried through as source text by the library's math processor
  `math-passthrough`; it does not parse it, except the argument of a text command, which
  is document content ([`math::text`](crate::stdcore::features::math::text)). No renderer
  of mathematics is part of the library ([`flm.math.rendered`](crate::stdcore::features::math::RENDERED)
  has no provider in it); the HTML page loads a formula renderer from an address. The
  JavaScript extension `math-mathjax` (in the repository, not in the library) checks the
  commands against MathJax's packages and can write MathML
  ([mathematics processed by MathJax](crate::guide::command_line#mathematics-processed-by-mathjax)).
- **Markup**: no numbering of anything but headings, equation lines, list items, xnotes,
  theorems, floats and cited works; no links but to a labeled reference point of the same
  document, to a label of another document that the caller installs in the external
  table [`flm.refs.external-index`](crate::stdcore::features::refs::EXTERNAL_INDEX), and
  to an address outside the document; no configuration block inside a document's body;
  no command definition in a document's body (no `\newcommand`); no title above the table
  of contents, and no `\tableofcontents` command (the output places the table of contents
  as a piece). The heading key `pdfbookmark` is reserved and reported as not implemented.
  An indent of a given
  amount in a lines section (`\indent[1.5em]`) is not available.
- **Configuration files**: `flm-core` reads no file and no front matter; parsing settings,
  typographic substitutions and user definitions are fixed when the registry is built.
  Settings written as data are read through each feature's
  [`SettingsDecl`](crate::core::settings::SettingsDecl), and a
  [`PackageCatalog`](crate::core::registry::PackageCatalog) looks packages up by name;
  `flm-core` ships no [`PackageResolver`](crate::core::registry::PackageResolver) and no
  [`ArtifactNamer`](crate::core::artifact::ArtifactNamer). The host library `flm-main`
  provides the namer, reads front matter, and writes the outputs; the crate `flm-ext`
  provides the resolver of extensions, which the host library uses.

## Where to look next

| to | load |
|---|---|
| work with the tree, elements, aspects, addresses, tables, synthesized values | [AI guide: document model](crate::guide::ai_guide_document_model) |
| understand stages, capabilities, plans, the runner, value catch-up | [AI guide: pipeline](crate::guide::ai_guide_pipeline) |
| write features, elements, stages, lowering rules, backends | [AI guide: extending FLM](crate::guide::ai_guide_extending) |
| write an extension, as data, in JavaScript, or in Rust compiled to WebAssembly or into the program | [Extensions](crate::guide::extending#extensions) |
| find where extensions are looked for | [Where extensions are looked for](crate::guide::command_line#where-extensions-are-looked-for) |
| embed the library: entry points, options, services, WebAssembly | [AI guide: embedding](crate::guide::ai_guide_embedding) |

The guide pages for people, by module path: [`crate::guide::introduction`],
[`crate::guide::language_syntax`] (the author's full reference),
[`crate::guide::command_line`] (the `flm` program), [`crate::guide::concepts_overview`]
(every concept, one section each), [`crate::guide::document_model`],
[`crate::guide::pipeline`], [`crate::guide::rendering`], [`crate::guide::numbering`],
[`crate::guide::extending`], [`crate::guide::embedding`], [`crate::guide::panics`],
[`crate::guide::python_flm_migration`].
