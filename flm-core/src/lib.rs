//! `flm-core` is the Rust library that implements FLM ("flexible latexlike
//! markup"), a markup language for documents. FLM is written with a syntax
//! borrowed from LaTeX (backslash commands, `\begin{…}…\end{…}` environments,
//! blank lines between paragraphs), but each construct says what a part of the
//! document *means* rather than how it looks, so that one source can be rendered
//! in several output formats.
//!
//! The library parses FLM source, processes the parsed document, and renders it.
//! It ships four output formats: plain text, HTML, LaTeX for LuaLaTeX with
//! everything the library resolves already resolved (the *rendered LaTeX*), and
//! LaTeX source written back close to what an author would type (the *recomposed
//! LaTeX*). It is built on
//! [techy], a parser toolkit for LaTeX-like languages, which supplies the
//! tokenizer, the parser and the node tree. The command-line program `flm` (crate
//! `flm-cli`) and the host library `flm-main`, which finds and reads a document's
//! files and settings and writes the outputs, are built on this crate.
//!
//! Nothing in the processing reads or writes files (the only file access in the
//! crate is the test tool [`dump::golden`]). A caller supplies the source text,
//! receives the output as values ([artifacts](core::artifact::Artifact), each
//! one file's worth of output), and decides where they go. When a document
//! includes other files, the caller supplies an [input
//! resolver](lang::InputResolver) that finds and reads them.
//!
//! # Using the library
//!
//! A program that renders an FLM document takes five steps. The
//! [Introduction](guide::introduction#embedding-the-library) shows them in one
//! complete program, and [Embedding flm-core](guide::embedding) describes each
//! of them in full.
//!
//! 1. **Build a registry.** A [registry](core::registry) is the frozen record
//!    of everything the language and the pipeline consult for a set of
//!    features: the elements an author can write, the stages, the tables, the
//!    backends, the workflows and the other things the features declared. It
//!    is built once with a
//!    [`RegistryBuilder`](core::registry::RegistryBuilder) from
//!    [features](core::registry::FlmFeature), Rust values that each contribute a
//!    related bundle of these things, and the finished
//!    [`FlmRegistry`](core::registry::FlmRegistry) is shared by every document.
//!    The standard package [`stdcore`] holds the features of the standard language
//!    and the four backends; [`stdcore::package`] prepares it for registration.
//!    A registry that renders formulas also needs a feature that processes
//!    mathematics, such as the one of the package [`math_passthrough`].
//! 2. **Parse the source and assemble a document.**
//!    [`parse_fragment`](lang::parse_fragment) parses one source text into a
//!    [fragment](model::doc::Fragment), which is then placed in a
//!    [document](model::doc::Doc). [`assemble`](model::assemble::assemble) parses
//!    a primary file together with the files it includes.
//! 3. **Ask for a plan.** A [plan](core::plan::Plan) is the list of
//!    [stages](core::stage::Stage) (the units of processing work) that
//!    satisfies a [plan request](core::plan::PlanRequest), in dependency order.
//!    The caller does not list the stages: in the plan request it states the
//!    [capabilities](core::registry::CapabilityDef) it wants (named states of
//!    processing, such as a rendered output), usually by choosing a
//!    [workflow](core::registry::WorkflowDef) (a named set of demands together
//!    with the choices that go with them, held in the registry) with
//!    [`Plan::for_workflow`](core::plan::Plan::for_workflow), and the library
//!    selects the stages that provide them and orders them.
//! 4. **Execute a run.** A [run](core::run::Run) is one execution of one plan
//!    over one document ([`Run::execute`](core::run::Run::execute)).
//! 5. **Read the results.** What a run computed is held in typed
//!    [tables](core::data::table::Table), read with
//!    [`Run::table`](core::run::Run::table). Each output (a rendered format such
//!    as `text` or `html`) produces an [artifact
//!    set](core::artifact::ArtifactSet), which the run holds in that output's
//!    table.
//!    The problems found in the document are
//!    [diagnostics](guide::concepts_overview#diagnostics), read with
//!    [`Run::diagnostics`](core::run::Run::diagnostics).
//!
//! A program that adds commands, checks or output formats of its own writes
//! features and registers them beside the standard package, exactly as the
//! standard package registers its own; [Extending FLM](guide::extending)
//! describes that work.
//!
//! # How a document is processed
//!
//! Three design decisions shape the whole library. The [Concepts
//! overview](guide::concepts_overview) defines each concept they rely on.
//!
//! 1. **The parsed node tree is the document model.** Every node of techy's
//!    node tree carries the [element](model::element::ElementSpec) that says
//!    what the node means, and the library never converts the parsed tree into a
//!    second representation. See [the document
//!    tree](guide::concepts_overview#the-document-tree).
//! 2. **The trees are frozen early, and computed data is stored beside them.**
//!    Only [rewrite stages](core::stage::RewriteStage) change the trees, and
//!    they all run first; in the standard package they insert the content of
//!    included files, expand the definitions an author wrote, and lay content
//!    out in paragraphs and blocks. After them the trees never change. Every
//!    later stage stores what it computes in typed *tables* beside the trees
//!    (numbers, references, the [presentation
//!    tree](guide::concepts_overview#backends-lowering-and-outputs) of each
//!    backend, the output
//!    text), or checks the document and reports diagnostics. See [frozen trees
//!    and computed data](guide::concepts_overview#frozen-trees-and-computed-data).
//! 3. **The library derives the processing from what the caller asks for.** A
//!    stage declares the capabilities it provides and the capabilities it
//!    requires; a caller *demands* capabilities (asks the plan for stages that
//!    provide them; see [`PlanRequest`](core::plan::PlanRequest)), and the library builds the plan that satisfies those
//!    demands. See [stages, capabilities, and
//!    plans](guide::concepts_overview#stages-capabilities-and-plans).
//!
//! # Modules
//!
//! - [`model`] — the document model: dotted names, tree and node addresses,
//!   elements and their aspects, fragments and documents, and the option values
//!   bound on a document.
//! - [`lang`] — FLM's language over techy: the language type, the parse entry
//!   points, the building of techy packages from the features' element
//!   declarations ([`lang::mint`]), the recursion guard and worker threads
//!   ([`lang::guard`]), and the three rewrite passes: splicing of included
//!   files, expansion of user definitions, and block structure
//!   ([`lang::rewrites`]).
//! - [`core`] — the processing framework: stages, the store of computed tables,
//!   services, lowering rules, artifacts, the registry, plan construction, and
//!   runs.
//! - [`stdcore`] — the standard package: the features of the standard language,
//!   the presentation elements and lowering shared by every backend, and the
//!   `text`, `latex-recomposed` and `html` backends.
//! - [`math_passthrough`] — the package `math-passthrough`, which uses `stdcore`:
//!   mathematics carried to the output as written, one of the features that
//!   process mathematics.
//! - [`layout`] — the text layout engine used by the plain-text backend: a
//!   description of the intended text and a printer that wraps and indents it.
//! - [`dump`] — inspection and test tooling: the projection of trees, tables and
//!   diagnostics that tests compare, golden files, assertions on diagnostics, and
//!   the serialization of parses used by the `flm dump` command.
//! - [`guide`] — the guide chapters (see below).
//!
//! # Cargo features
//!
//! Both features are on by default.
//!
//! - `parallel`: a [run](core::run::Run) applies a stage that works tree by
//!   tree to the document's trees at the same time, on the threads of a pool
//!   (through the `rayon` crate). The output is the same as with the feature off, when
//!   all the work runs on one thread;
//!   [`RunOptions::sequential`](core::run::RunOptions::sequential) asks for
//!   one thread at run time.
//! - `docs`: the documentation texts of the declarations a
//!   [registry](core::registry) is built from (of a command, an argument, a
//!   feature, a stage, a table, a setting, a heading kind, a counter, a cell
//!   style, and the other things a registry holds) are kept, so that a program
//!   can show them. A program that never shows these texts, such as one
//!   compiled to WebAssembly, turns the feature off and then does not carry
//!   them. With the feature off:
//!   - every builder method, builder function and constructor that takes a
//!     documentation text stores an empty text instead, such as
//!     [`ElementDef::docs`](lang::mint::ElementDef::docs),
//!     [`FeatureBuilder::argument_role`](core::registry::FeatureBuilder::argument_role),
//!     [`HeadingDecl::docs`](stdcore::features::headings::HeadingDecl::docs),
//!     [`CounterDecl::docs`](stdcore::features::numbering::CounterDecl::docs)
//!     and [`CellStyle::fixed`](stdcore::features::cells::CellStyle::fixed);
//!     so does the registration of a scoped package, a stylesheet or a script
//!     ([`FeatureBuilder::scoped_package`](core::registry::FeatureBuilder::scoped_package),
//!     [`style_def`](core::registry::FeatureBuilder::style_def),
//!     [`script_def`](core::registry::FeatureBuilder::script_def));
//!   - the library reads the texts written in settings data (of a user
//!     definition, a heading kind or a workflow) through these same builders,
//!     so they are dropped too;
//!   - a text that a program assigns directly to a public `docs` field is
//!     kept, and so is the text of a LaTeX package request given among a
//!     backend's render options, which is an option and not a declaration.
//!
//!   Turning the feature off changes neither the rendered output of a document
//!   nor the [registry digest](model::digest), which does not include the
//!   texts. Besides the texts themselves, two things change: the JSON Schema
//!   of a feature's settings
//!   ([`SettingsDecl::json_schema`](core::settings::SettingsDecl::json_schema))
//!   loses its top-level description, which is made of the documentation
//!   texts the feature gives its settings declaration (the descriptions of the
//!   individual settings stay: they are part of the schema itself, compiled
//!   from the Rust documentation comments of the settings types or written in
//!   the schema a feature gives as data); and the error
//!   [`RunError::MissingExternal`](core::run::RunError::MissingExternal) no
//!   longer quotes the description of the missing table.
//!
//! # Module layering
//!
//! Each top-level module uses only the modules on levels below its own:
//!
//! ```text
//! {model, lang} < core < dump < stdcore < math_passthrough        layout standalone
//! ```
//!
//! [`model`] and [`lang`] form one level, not two: a node carries its element,
//! so the language needs the model, and the model's fragments are typed by the
//! language, so the model needs the language. Together they use no other module
//! of this crate. [`core`] uses both; [`dump`] uses those three; [`stdcore`]
//! uses all of them and the [`layout`] engine; [`math_passthrough`] uses
//! [`stdcore`] and the levels below it. [`layout`] uses no other module of
//! this crate. Within [`stdcore`], only the backends
//! ([`stdcore::backends`]) use the layout engine and the Unicode-to-LaTeX
//! encoder [`untechxt`]. Because each module uses only the levels below it, the
//! dependencies between the modules have no cycles.
//!
//! # Guide
//!
//! The [`guide`] module holds the narrative documentation; the other modules
//! are the API reference, and the two link each other throughout. Start with
//! the [Introduction](guide::introduction). The [Concepts
//! overview](guide::concepts_overview) defines the library's main concepts, one
//! section each. The [guide's landing page](guide) lists every chapter: the
//! language syntax and the `flm` program for authors and users; the document
//! model, the pipeline, rendering, numbering, extending and embedding for
//! programmers; the [list of panicking items](guide::panics); the [migration
//! guide from the Python implementation](guide::python_flm_migration); and the [AI
//! guide](guide::ai_guide), a condensed orientation written to be loaded into an
//! AI assistant's context.
//!
//! [techy]: https://github.com/phfaist/techy

/// The parser toolkit FLM is built on, re-exported at exactly the version this
/// crate is compiled against.
///
/// A program, an extension crate or a backend that uses FLM needs techy's own
/// types: sources, node trees, diagnostics, serialization sessions. Use them
/// through `flm_core::techy` rather than through a techy dependency of your own.
/// Two versions of techy in one build are two unrelated sets of types, and a
/// value of one is not accepted by the functions of the other; the re-export
/// guarantees that the program uses the version this crate uses.
pub use techy;

/// The Unicode-to-LaTeX encoder (the crate `untechxt`), re-exported at exactly
/// the version this crate is compiled against.
///
/// `untechxt` holds a table that writes each character as LaTeX source, records
/// what each such LaTeX form needs in a document's preamble (a package, a font
/// encoding), and protects a form from merging with the text written after it.
/// It does not depend on FLM or on any document model. The LaTeX family's
/// escaping ([`stdcore::backends::latex::escape`]) writes every non-ASCII
/// character of a document's text through it, and the preamble needs it records
/// ([`PreambleNeeds`](untechxt::preamble::PreambleNeeds),
/// [`Profile`](untechxt::preamble::Profile)) appear in this crate's own
/// interface. Use the crate through `flm_core::untechxt` rather than through a
/// dependency of your own, for the reason given at [`techy`].
pub use untechxt;

// See decision rationale at [§flm-dd-dr:docs-feature].
/// Returns the documentation text `docs` as a builder method stores it: the
/// text itself when the cargo feature `docs` is on, and an empty text when it
/// is off, so that a build without the feature keeps no documentation text.
///
/// Every builder method, builder function and constructor that stores a
/// documentation text passes it through this function, in its own body, and
/// so does a library function that builds such a value without one (the LaTeX
/// definitions derived from the Unicode-to-LaTeX table); a reader of data
/// sets the text through a builder. The code that calls a builder never tests
/// the feature.
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
// updated whenever a top-level module is added or renamed. See the architecture
// notes at [§flm-dd-arch:crates].
pub mod core;
pub mod dump;
pub mod lang;
pub mod layout;
pub mod math_passthrough;
pub mod model;
pub mod stdcore;

// Narrative documentation: markdown pages in the workspace-level `docs/` rendered as
// doc-only modules. `cfg(doc)` keeps them out of compiled code; rustdoc (including
// doctest collection) builds with `--cfg doc`, so code blocks in these pages still run
// as doctests. The docs sidebar pins these pages in a "Guide" section
// (docs/rustdoc-header.html, wired up in .cargo/config.toml); new chapters must also be
// added to GUIDE_PAGES there and to the chapter index in docs/guide.md. The full
// procedure is in Documentation_Structure.md ("Guides included in rustdoc").
#[cfg(doc)]
#[doc = include_str!("../../docs/guide.md")]
pub mod guide {
    // User Guide.
    #[doc = include_str!("../../docs/introduction.md")]
    pub mod introduction {}

    #[doc = include_str!("../../docs/language-syntax.md")]
    pub mod language_syntax {}

    #[doc = include_str!("../../docs/command-line.md")]
    pub mod command_line {}

    // Developer Guide.
    #[doc = include_str!("../../docs/concepts-overview.md")]
    pub mod concepts_overview {}

    #[doc = include_str!("../../docs/document-model.md")]
    pub mod document_model {}

    #[doc = include_str!("../../docs/pipeline.md")]
    pub mod pipeline {}

    #[doc = include_str!("../../docs/rendering.md")]
    pub mod rendering {}

    #[doc = include_str!("../../docs/numbering.md")]
    pub mod numbering {}

    #[doc = include_str!("../../docs/extending.md")]
    pub mod extending {}

    #[doc = include_str!("../../docs/embedding.md")]
    pub mod embedding {}

    #[doc = include_str!("../../docs/panics.md")]
    pub mod panics {}

    #[doc = include_str!("../../docs/python-flm-migration.md")]
    pub mod python_flm_migration {}

    // AI Guide.
    #[doc = include_str!("../../docs/ai-guide.md")]
    pub mod ai_guide {}

    #[doc = include_str!("../../docs/ai-guide-document-model.md")]
    pub mod ai_guide_document_model {}

    #[doc = include_str!("../../docs/ai-guide-pipeline.md")]
    pub mod ai_guide_pipeline {}

    #[doc = include_str!("../../docs/ai-guide-extending.md")]
    pub mod ai_guide_extending {}

    #[doc = include_str!("../../docs/ai-guide-embedding.md")]
    pub mod ai_guide_embedding {}
}
