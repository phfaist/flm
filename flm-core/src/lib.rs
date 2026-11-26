//! `flm-core` is the Rust library that implements FLM ("flexible latexlike
//! markup"), a markup language for documents. FLM borrows its syntax from
//! LaTeX: commands begin with a backslash, environments are written as
//! `\begin{…}…\end{…}`, and a blank line separates two paragraphs. Each
//! construct of FLM expresses the meaning of a part of the document, not its
//! appearance, so that one source can be rendered in several output formats.
//!
//! The library parses FLM source, processes the document, and renders it in
//! four output formats: plain text, HTML, *rendered LaTeX* and *recomposed
//! LaTeX*. Rendered LaTeX is LaTeX for LuaLaTeX in which FLM has already
//! resolved the numbering, the cross-references and the other computed
//! content. Recomposed LaTeX is LaTeX source close to what an author would
//! type; it leaves the numbering and the cross-references to LaTeX.
//!
//! The library is built on [techy], a parser toolkit for LaTeX-like
//! languages, which provides the tokenizer, the parser and the node tree. The
//! library `flm-host`, which finds and reads the files and the settings of a
//! document and writes its outputs, uses this library, and so does the
//! command-line program `flm` (crate `flm`).
//!
//! The processing reads and writes no files. A program that uses the library
//! provides the source text and receives the output as
//! [artifacts](core::artifact::Artifact): each artifact is a value that
//! contains the content of one output file. The program decides where to
//! write the artifacts. When a document includes other files with `\input`,
//! the program provides an [input resolver](lang::InputResolver), which finds
//! and reads these files.
//!
//! # Using the library
//!
//! A program that renders an FLM document takes the following five steps. The
//! [Introduction](guide::introduction#embedding-the-library) shows them in a
//! complete program, and the chapter [Embedding flm-core](guide::embedding)
//! describes each of them in full.
//!
//! 1. **Build a registry.** A [registry](core::registry) contains the
//!    definitions used to parse and process documents: the commands and
//!    environments that an author can write, the processing stages, the tables
//!    of computed data, the backends, the workflows, and so on. These
//!    definitions are made in *features*: a
//!    [feature](core::registry::FlmFeature) is a Rust value that contributes a
//!    group of related definitions to a registry. Build the registry with a
//!    [`RegistryBuilder`](core::registry::RegistryBuilder); most programs add
//!    to it a package that holds the standard features of [`stdlib`]
//!    ([`stdlib::add_features`]). The
//!    result, an [`FlmRegistry`](core::registry::FlmRegistry), cannot be
//!    changed after it is built. For documents that contain mathematics, the
//!    registry also needs a feature that processes mathematics, such as the
//!    feature of the package [`math_passthrough`]. Without such a feature,
//!    each formula is reported as an error when the document is rendered.
//! 2. **Parse the source and place it in a document.**
//!    [`parse_fragment`](lang::parse_fragment) parses one source text into a
//!    [fragment](model::doc::Fragment), and the program places the fragment in
//!    a [document](model::doc::Doc). To parse a file together with the files
//!    that it includes, use [`assemble`](model::assemble::assemble).
//! 3. **Build a plan.** A [plan](core::plan::Plan) is the list of
//!    [stages](core::stage::Stage) that a run executes, in the order of their
//!    dependencies. A stage is one unit of processing work. The program states
//!    in a [plan request](core::plan::PlanRequest) which *capabilities* it
//!    wants. A [capability](core::registry::CapabilityDef) is a named state of
//!    processing, such as a rendered output. The library then selects the
//!    stages that provide these capabilities and orders them. Usually, the
//!    program uses a [workflow](core::registry::WorkflowDef), a complete plan
//!    request stored in the registry under a name such as `html`, and builds
//!    the plan with [`Plan::for_workflow`](core::plan::Plan::for_workflow).
//! 4. **Execute a run.** A [run](core::run::Run) is one execution of a plan
//!    over a document. Create the run with [`Run::new`](core::run::Run::new)
//!    and execute it with [`Run::execute`](core::run::Run::execute).
//! 5. **Read the results.** A run stores what it computes in typed
//!    [tables](core::data::table::Table), which
//!    [`Run::table`](core::run::Run::table) returns. The files of an output,
//!    such as the HTML page, form an [artifact
//!    set](core::artifact::ArtifactSet), which is stored in the table
//!    declared for that output ([`OutputDef`](core::registry::OutputDef)).
//!    The problems found in the document are
//!    [diagnostics](guide::concepts_overview#diagnostics), which
//!    [`Run::diagnostics`](core::run::Run::diagnostics) returns.
//!
//! The registry and the plan do not depend on a document. A program therefore
//! builds them once and uses them for every document that it processes.
//!
//! A program that adds commands, checks or output formats of its own defines
//! them in features, and adds these features to the registry beside the
//! standard features. The standard features register in the same way. The
//! chapter [Extending FLM](guide::extending) describes this work.
//!
//! # How a document is processed
//!
//! Three design decisions shape the library. The [Concepts
//! overview](guide::concepts_overview) explains each of them.
//!
//! 1. **The parsed node trees are the only representation of a document.** A
//!    document consists of the node trees that techy's parser produces. Each
//!    node stores an [element](model::element::ElementSpec), a value that
//!    describes what the node means, such as a heading or emphasis. The
//!    library does not convert the trees into another data structure. See [the
//!    document tree](guide::concepts_overview#the-document-tree).
//! 2. **The trees are frozen early, and computed data is stored in tables.**
//!    The *rewrite stages* run first, and they are the only stages that change
//!    the trees. Among the standard features, they insert the content of included
//!    files, expand user-defined macros and environments, and group the
//!    content into paragraphs and blocks. After the rewrite stages, the trees
//!    are *frozen*, and no later stage changes them. Each later stage stores
//!    what it computes in tables, such as the numbers of the headings or the
//!    output text, or it checks the document and reports diagnostics. [Frozen
//!    trees and computed
//!    data](guide::concepts_overview#frozen-trees-and-computed-data) explains
//!    the reasons for this design.
//! 3. **The library derives the processing from what a program requests.**
//!    Each stage is declared with the capabilities that it provides and the
//!    capabilities that it requires. A program *demands* capabilities in its
//!    plan request, and the library builds the plan that provides them. A
//!    feature can therefore add a stage without any change to the other
//!    stages: the stage runs in every plan that needs a capability that it
//!    provides. See [stages, capabilities, and
//!    plans](guide::concepts_overview#stages-capabilities-and-plans).
//!
//! # Modules
//!
//! - [`model`]: the document model. It defines documents and fragments, the
//!   elements that the nodes store and their aspects, the addresses of trees
//!   and nodes, dotted names, and the option values set on a document.
//! - [`lang`]: the FLM language on top of techy. It contains the parse
//!   functions, the construction of techy's command specifications from the
//!   elements defined in the features ([`lang::mint`]), the recursion
//!   guard and the worker threads ([`lang::guard`]), and the rewrite passes
//!   that insert included files, expand user definitions and group content
//!   into blocks ([`lang::rewrites`]).
//! - [`core`]: the processing framework. It contains the registry, stages and
//!   the tables of computed data, plans and runs, lowering rules, artifacts,
//!   services and settings.
//! - [`stdlib`]: the standard features. They provide the commands and
//!   environments of the language, the stages that structure and check a
//!   document, and the four backends `text`, `html`, `latex-rendered` and
//!   `latex-recomposed`. [`stdlib::add_features`] adds them to a package that
//!   a program declares.
//! - [`math_passthrough`]: the package `math-passthrough`. Its feature
//!   processes mathematics by copying the source of each formula to the output
//!   as the author wrote it.
//! - [`layout`]: the text layout engine, which wraps and indents text to a
//!   given line width. The plain-text backend uses it.
//! - [`progress`]: the progress sheet of a build: the rows of the work under
//!   way and how far each is, the request to stop a build, and the sink of the
//!   log lines that the work writes.
//! - [`dump`]: tools to inspect and test what the library holds in memory:
//!   projections of trees, tables and diagnostics that tests compare, golden
//!   files, assertions on diagnostics, and the serialization of parses that
//!   the command `flm dump` prints.
//! - [`guide`]: the chapters of the guide (see [Guide](#guide) below).
//!
//! # Cargo features
//!
//! The crate has two Cargo features, and both are enabled by default.
//!
//! - `parallel`: A [run](core::run::Run) executes independent work in
//!   parallel, on a thread pool of the `rayon` crate. For instance, a stage
//!   that runs once per tree runs on all the trees of a document at the same
//!   time. The output is the same as without the feature. To run all the work
//!   on one thread in a build with this feature, pass
//!   [`RunOptions::sequential`](core::run::RunOptions::sequential) to
//!   [`Run::execute`](core::run::Run::execute).
//! - `docs`: The library stores the documentation texts given with the
//!   definitions of a registry: the texts of the commands, of their
//!   arguments, of the features, of the stages, of the tables, of the
//!   settings, and so on. The command `flm info` shows these texts. A program
//!   that never shows them, such as a WebAssembly build, can disable the
//!   feature; the library then stores no documentation text.
//!
//!   When the feature is disabled, every builder method and constructor that
//!   takes a documentation text, such as
//!   [`ElementDef::docs`](lang::mint::ElementDef::docs), stores an empty text.
//!   This also applies to the texts read from settings data, such as the
//!   description of a user definition or of a workflow, because they pass
//!   through the same builder methods. A text that a program assigns directly
//!   to a public `docs` field is kept.
//!
//!   Disabling the feature changes neither the rendered output of a document
//!   nor the [registry digest](core::registry::FlmRegistry::digest). The JSON
//!   Schema of a feature's settings
//!   ([`SettingsDecl::json_schema`](core::settings::SettingsDecl::json_schema))
//!   then has no top-level description, but it keeps the descriptions of the
//!   individual settings.
//!
//! # Module layering
//!
//! The top-level modules form layers, and each module uses only the modules of
//! its own layer and of the layers below it:
//!
//! ```text
//! {model, lang} < core < dump < stdlib < math_passthrough        layout, progress standalone
//! ```
//!
//! The modules [`model`] and [`lang`] form one layer because each uses the
//! other. The parser defined in [`lang`] stores an element in each node, and
//! elements are defined in [`model`]. A fragment, defined in [`model`],
//! contains a tree of the FLM language defined in [`lang`]. The [`layout`]
//! module uses no other module of this crate, and [`stdlib`] uses it in
//! addition to the modules of the lower layers. The [`progress`] module uses
//! no other module of this crate either, and [`core`] and [`stdlib`] use it.
//!
//! # Guide
//!
//! The [`guide`] module contains the narrative documentation of FLM, and the
//! other modules form the API reference. Start with the
//! [Introduction](guide::introduction). The [Concepts
//! overview](guide::concepts_overview) defines the main concepts of the
//! library, one per section. The [guide's landing page](guide) lists all
//! chapters: the chapters on the language and on the `flm` program for
//! authors, the chapters on the library for programmers, and condensed
//! chapters for AI assistants.
//!
//! [techy]: https://github.com/phfaist/techy

/// Re-exports the parser toolkit techy, on which FLM is built. The re-exported
/// crate is the version of techy that `flm-core` is compiled against.
///
/// A program or an extension that uses FLM works with techy's types, such as
/// node trees, diagnostics and serialization sessions. Use these types through
/// `flm_core::techy` rather than through a dependency on techy in your own
/// crate. If a build contains two versions of techy, the types of one version
/// are distinct from the types of the other, and the functions of one version
/// do not accept the values of the other. The re-export ensures that your code
/// uses the same version of techy as `flm-core`.
pub use techy;

/// Re-exports the Unicode-to-LaTeX encoder `untechxt`. The re-exported crate is
/// the version of `untechxt` that `flm-core` is compiled against.
///
/// `untechxt` converts text into LaTeX source. It replaces each character that
/// LaTeX does not accept, or treats specially, with LaTeX code that prints the
/// character, and it records what this code needs in the preamble of a LaTeX
/// document, such as a package or a font encoding. The LaTeX backends of
/// `flm-core` take the LaTeX code for each non-ASCII character of a document
/// from `untechxt` ([`stdlib::backends::latex::escape`]), and the interface of
/// that module contains types of `untechxt`, such as
/// [`PreambleNeeds`](untechxt::preamble::PreambleNeeds). Use `untechxt` through
/// `flm_core::untechxt` rather than through a dependency in your own crate, for
/// the reason given in the documentation of [`techy`].
pub use untechxt;

// The cargo feature `docs` is tested in this one function, which each builder
// calls in its own body, never where a builder is called: those calls number
// in the hundreds, and every one added would have to remember the test.
/// Returns the documentation text `docs` when the Cargo feature `docs` is
/// enabled, and an empty string when it is disabled.
///
/// Every builder method and constructor that stores a documentation text
/// passes the text through this function, so that a build without the feature
/// stores no documentation text. The code that calls these builder methods,
/// including the code that reads definitions from settings data, therefore
/// does not test the feature itself. Library code that sets a documentation
/// text without a builder method calls this function directly. For instance,
/// the shared LaTeX module ([`stdlib::backends::latex`]) calls it for the
/// LaTeX definitions that it derives from the Unicode-to-LaTeX table of
/// [`untechxt`].
#[inline]
pub(crate) fn kept_docs(docs: impl Into<String>) -> String {
    #[cfg(feature = "docs")]
    {
        docs.into()
    }
    #[cfg(not(feature = "docs"))]
    {
        let _ = docs;
        String::new()
    }
}

// The module layering stated in the crate documentation above is not something
// the compiler enforces; `tests/module_layering.rs` reads the source tree and
// fails on any `crate::` path pointing upward, and its ALLOWED table has to be
// updated whenever a top-level module is added or renamed.
pub mod core;
pub mod dump;
pub mod lang;
pub mod layout;
pub mod math_passthrough;
pub mod model;
pub mod progress;
pub mod stdlib;

// Narrative documentation: markdown pages in the workspace-level `docs/` rendered as
// doc-only modules. They are read through the symbolic link `flm-core/docs` to `../docs`,
// so that the crate's package holds them. `cfg(doc)` keeps them out of compiled code;
// rustdoc (including doctest collection) builds with `--cfg doc`, so code blocks in these
// pages still run as doctests. The docs sidebar pins these pages in a "Guide" section
// (docs/rustdoc-header.html, wired up in .cargo/config.toml). A new chapter takes four
// steps: its file `docs/NAME.md`, its module in the block below, its entry in
// GUIDE_PAGES of docs/rustdoc-header.html, and its line in the chapter index of
// docs/guide.md.
#[cfg(doc)]
#[doc = include_str!("../docs/guide.md")]
pub mod guide {
    // User Guide.
    #[doc = include_str!("../docs/introduction.md")]
    pub mod introduction {}

    #[doc = include_str!("../docs/language-syntax.md")]
    pub mod language_syntax {}

    #[doc = include_str!("../docs/std-flm.md")]
    pub mod std_flm {}

    #[doc = include_str!("../docs/command-line.md")]
    pub mod command_line {}

    // Developer Guide.
    #[doc = include_str!("../docs/concepts-overview.md")]
    pub mod concepts_overview {}

    #[doc = include_str!("../docs/document-model.md")]
    pub mod document_model {}

    #[doc = include_str!("../docs/pipeline.md")]
    pub mod pipeline {}

    #[doc = include_str!("../docs/rendering.md")]
    pub mod rendering {}

    #[doc = include_str!("../docs/numbering.md")]
    pub mod numbering {}

    #[doc = include_str!("../docs/extending.md")]
    pub mod extending {}

    #[doc = include_str!("../docs/embedding.md")]
    pub mod embedding {}

    #[doc = include_str!("../docs/panics.md")]
    pub mod panics {}

    #[doc = include_str!("../docs/python-flm-migration.md")]
    pub mod python_flm_migration {}

    // AI Guide.
    #[doc = include_str!("../docs/ai-guide.md")]
    pub mod ai_guide {}

    #[doc = include_str!("../docs/ai-guide-document-model.md")]
    pub mod ai_guide_document_model {}

    #[doc = include_str!("../docs/ai-guide-pipeline.md")]
    pub mod ai_guide_pipeline {}

    #[doc = include_str!("../docs/ai-guide-extending.md")]
    pub mod ai_guide_extending {}

    #[doc = include_str!("../docs/ai-guide-embedding.md")]
    pub mod ai_guide_embedding {}
}
