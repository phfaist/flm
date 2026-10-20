//! The standard package of flm-core, `stdcore`: the features and backends that make
//! the library a working FLM processor, registered together in one call.
//!
//! A *package* is a named bundle of features together with the packages it builds on
//! ([`PackageEntry`]), and a *feature* ([`FlmFeature`]) is a Rust value that
//! contributes a related bundle of things to a registry (elements, stages, tables,
//! lowering rules, backends, services, settings) through the feature builder
//! ([`FeatureBuilder`]). The `stdcore` package is the
//! only package this crate ships. It provides:
//!
//! - the commands and environments an author writes: inline formatting, literal
//!   characters, headings, mathematics, labels and references, verbatim content, lists,
//!   footnotes and the other xnotes, hyperlinks, defined terms, theorems, quotations,
//!   annotations, images, floats, tables of cells, citations, included files and user
//!   definitions;
//! - the stages that structure and check a document: block structure, the element
//!   index, the table of reference points, numbering, and the checks every workflow runs
//!   (see [Every workflow validates](#every-workflow-validates-the-collection-flmvalidated)
//!   below);
//! - the document's metadata (title, authors, date), shown before the content, and the
//!   table of contents;
//! - four output formats: plain text (`text`), HTML (`html`), rendered LaTeX
//!   (`latex-rendered`) and recomposed LaTeX (`latex-recomposed`), the last two built
//!   on `latex`, which every LaTeX backend shares, with the
//!   lowering stage, the presentation elements and the pieces (document-level values
//!   such as the title, placed before or after the content;
//!   [`Piece`](crate::core::piece::Piece)) they share ([`backends`]);
//! - the `check` workflow, which validates a document and writes no output.
//!
//! The package processes no mathematics: the `math` feature introduces mathematics into
//! the language, and a feature of another package processes it, for example the package
//! [`math-passthrough`](crate::math_passthrough), which carries every formula to the
//! output as written, or an extension that typesets formulas. A registry that holds
//! `stdcore` and no such feature builds and plans every standard workflow, and a render
//! reports each formula as an Error (the [`math`](features::math) module documentation,
//! *Processing mathematics*).
//!
//! The standard features have no special status: each registers through the same
//! feature builder a feature of another crate uses, so a program can add commands,
//! change how a standard element is rendered or add an output format without an
//! extension point made for the purpose (see [features and
//! packages](crate::guide::concepts_overview#features-and-packages), and [the extension
//! guide](crate::guide::extending) for writing a feature or a package).
//!
//! # Using the package
//!
//! A program written in Rust builds the package entry with [`package`] from a
//! [`StdcoreConfig`] and adds it to a registry builder with
//! [`RegistryBuilder::use_package`](crate::core::registry::RegistryBuilder::use_package):
//!
//! ```
//! use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
//! use flm_core::stdcore::{self, StdcoreConfig};
//!
//! let mut builder = RegistryBuilder::new();
//! builder.use_package(stdcore::package(StdcoreConfig::new()));
//! let registry = builder.build(&RegistryInputs::new())?;
//!
//! // The package ships the `check` workflow: validation, no output.
//! assert!(registry.workflow(stdcore::CHECK_WORKFLOW).is_some());
//! # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
//! ```
//!
//! [`StdcoreConfig`] holds the configuration of each of the sixteen standard features
//! that take one; the other seventeen take none. A feature's configuration is the plain
//! Rust value the feature is constructed from; it is fixed when the registry is built
//! and cannot vary per document or per render (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!
//! A program that reads its settings as data, for example from a document's front
//! matter, uses [`factory`] instead: the [`PackageFactory`] it returns declares the
//! settings of the features that take some, and builds the package entry from the
//! settings it is given, leaving out the features those settings switch off.
//!
//! The pipeline guide ([`crate::guide::pipeline`]) explains what a run does with the
//! stages the features declare, and the embedding guide ([`crate::guide::embedding`])
//! shows a complete program.
//!
//! # The features of the package and where each is documented
//!
//! [`package`] lists the thirty-three features in this order, each once, with the module
//! that documents it:
//!
//! 1. `textstyles` ([`features::textstyles`]): `\emph`, `\textbf`, `\textit`, `\texttt`.
//! 2. `literals` ([`features::literals`]): the commands and characters that stand for
//!    one character of text, such as `\%` and `~`.
//! 3. `blocks` ([`features::blocks`]): paragraphs and blocks, typographic
//!    substitutions, and the check of the tree contract.
//! 4. `index` ([`features::index`]): the table of the nodes whose elements carry each
//!    aspect.
//! 5. `refpoints` ([`features::refpoints`]): the table of every reference point.
//! 6. `numbering` ([`features::numbering`]): counters, counter styles, and the number
//!    of every numbered reference point.
//! 7. `math` ([`features::math`]): inline and display mathematics: the display-math
//!    environments, their equation lines and numbers, the text commands of mathematics.
//! 8. `refs` ([`features::refs`]): `\label`, `\ref`, `\hyperref`, anchors, and the
//!    label check.
//! 9. `headings` ([`features::headings`]): `\section` to `\subparagraph`, and heading
//!    kinds a program declares.
//! 10. `verbatim` ([`features::verbatim`]): content taken exactly as written.
//! 11. `lists` ([`features::lists`]): `itemize`, `enumerate`, `description` and their
//!     `\item`s.
//! 12. `xnotes` ([`features::xnotes`]): `\footnote` and the other kinds of xnote.
//! 13. `href` ([`features::href`]): `\href`, `\url` and `\email`.
//! 14. `defterm` ([`features::defterm`]): the `defterm` environment and `\term`.
//! 15. `theorems` ([`features::theorems`]): `theorem`, `lemma`, `proof` and the other
//!     theorem kinds.
//! 16. `quote` ([`features::quote`]): `quote`, `blockquote` and `address`.
//! 17. `annotations` ([`features::annotations`]): a reviewer's highlights and
//!     comments, one command per configured reviewer.
//! 18. `graphics` ([`features::graphics`]): `\includegraphics`.
//! 19. `floats` ([`features::floats`]): `figure`, `table` and other float types, with
//!     their captions.
//! 20. `cells` ([`features::cells`]): tables of cells, the `cells` environment.
//! 21. `citations` ([`features::citations`]): `\cite` and the list of cited works.
//! 22. `input` ([`features::input`]): `\input`, the content of another file.
//! 23. `userdefs` ([`features::userdefs`]): macros and environments defined by
//!     substitution.
//! 24. `lowering` ([`backends::lowering`]): the lowering stage, and the lowering rules
//!     of the structural elements.
//! 25. `meta` ([`features::meta`]): the document's title, authors and date.
//! 26. `toc` ([`features::toc`]): the table of contents, a table of the listed
//!     headings and a piece placed before the content.
//! 27. `pieces` ([`backends::pieces`]): what a backend places before or after the
//!     document's content.
//! 28. `text` ([`backends::text`]): the plain-text backend and the `text` workflow.
//! 29. `latex` ([`backends::latex`]): what every LaTeX backend shares.
//! 30. `latex-recomposed` ([`backends::latex_recomposed`]): the recomposed-LaTeX
//!     backend and the `latex-recomposed` workflow.
//! 31. `latex-rendered` ([`backends::latex_rendered`]): the rendered-LaTeX
//!     backend and the `latex-rendered` workflow.
//! 32. `html` ([`backends::html`]): the HTML backend and the `html` workflow.
//! 33. `check` ([`CheckWorkflow`]): the collection `flm.validated` and the `check`
//!     workflow.
//!
//! The order is the registry's registration order. It breaks the tie between two
//! stages a plan could run in either order, it decides which feature is recorded as
//! the owner of an element value two features both declare (the one placed last), and, where two
//! features define one command name in the same parsing mode, it decides whose
//! definition applies: the later one, and the clash is reported unless the feature
//! order places that feature explicitly after the other, or that feature declares an
//! override (see [`crate::lang::mint::packages`]).
//!
//! # Every workflow validates: the collection `flm.validated`
//!
//! A *collection* is a name for a set of capabilities that any feature may add to:
//! demanding the collection demands every member, and a collection can be demanded but
//! never provided ([`CollectionDef`]).
//!
//! The `check` feature ([`CheckWorkflow`]) declares the collection [`VALIDATED`]
//! (`flm.validated`) and the workflow [`CHECK_WORKFLOW`] (`check`), which demands the
//! collection and nothing else: a run of `check` validates the document and writes no
//! output. The workflows of the four standard backends, `text`, `html`,
//! `latex-rendered` and `latex-recomposed`, demand the collection beside their output. A render therefore
//! reports the problems `check` reports: a label written twice or a cited key with no
//! entry is reported alike by a render and by `check`.
//!
//! The members the standard features add are:
//!
//! - `flm.blocks.contract-checked` ([`blocks`](features::blocks)): every tree
//!   satisfies the [tree contract](crate::lang::rewrites::blocks::check), or its
//!   violations are reported.
//! - `flm.refs.check-labels` ([`refs`]): no label is written twice,
//!   every label's type is one its construct accepts, and every reference that
//!   resolves to nothing is reported.
//! - `flm.theorems.check-proofs` ([`theorems`](features::theorems)): every statement
//!   that expects a proof and has none is reported, and so is every malformed
//!   back-reference of a proof.
//! - `flm.floats.check-handlers` ([`floats`](features::floats)): every content handler
//!   that a float type used in the document requires and no feature registers is
//!   reported.
//! - `flm.citations.check` ([`citations`](features::citations)): every cited key with
//!   no entry is reported.
//! - `flm.meta.read` ([`meta`]): the document's metadata is parsed, and
//!   the problems the parse finds are reported.
//! - `flm.input.spliced` ([`input`]): the content of every included
//!   file is in place in the document's trees.
//!
//! The feature that processes mathematics adds one more member, the check of the
//! formulas. Its check stage provides `flm.math.content`, which the
//! [`math`](features::math) feature declares (a stage has checked the content of every
//! formula), and which no feature of this package provides or adds. The feature of the
//! package [`math-passthrough`](crate::math_passthrough) adds `flm.math.content`
//! itself; its stage `flm.math-passthrough.check` accepts every formula and reports
//! every equation line that holds no mathematics. An extension adds to a collection
//! only a capability named under its own stem, so the `math-mathjax` extension adds
//! `math-mathjax.check`, which its check provides beside `flm.math.content`. Where two
//! such features are registered, the plan request prefers one stage, because a
//! capability with two providers and nothing to choose between them makes the plan
//! build fail ([`PlanError::Contested`](crate::core::plan::PlanError::Contested)).
//!
//! A feature adds its member from its own registration
//! ([`FeatureBuilder::collection_member`]), in any order of registration. In a
//! registry that does not hold the `check` feature, the build drops the member without
//! a diagnostic, so a feature that adds a check still registers on its own. A registry
//! that holds a standard backend feature but not the `check` feature fails to build,
//! because the backend's workflow demands the collection `flm.validated`, which no
//! feature then declares; a
//! program that wants the checks without the rest of the package registers
//! [`check_feature`] beside its own features.
//!
//! The checks read the document's trees as the plan's rewrites leave them. Where a
//! render of the recomposed LaTeX keeps the use sites of a user definition (to write
//! them back as calls), the checks see the call and not the definition's expansion;
//! LaTeX checks that content when it compiles the output.
//!
//! An Error a check reports does not stop a render. The stages of the standard
//! backends have no effect outside the run's store, so the run's error gate (see [the
//! error gate](crate::guide::embedding#the-error-gate)) does not stop them: the output
//! is computed, and the diagnostics are reported beside it.
//!
//! **An unresolved reference is the only problem whose severity depends on the
//! workflow.** The label check of the [`refs`] feature reads how the
//! references were resolved, so every plan that demands the collection runs the
//! resolution of references and reports the references that resolve to nothing. A plan
//! that renders nothing cannot know whether a label would resolve in a render, because
//! a render may install the external index of labels defined in other documents
//! ([`EXTERNAL_INDEX`](features::refs::EXTERNAL_INDEX)). An unresolved
//! reference is therefore a Warning under `check`, regardless of
//! [`RefsRenderOptions`](features::refs::RefsRenderOptions), and an Error under a
//! rendering workflow unless those options allow it. A label written twice and a label
//! of a type its construct does not accept are Errors under every workflow, because
//! each is a fact of the document alone.

pub mod backends;
pub mod features;

use std::sync::Arc;

use crate::core::registry::{
    BuildError, CollectionDef, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry,
    PackageError, PackageFactory, WorkflowDef,
};
use crate::core::settings::PackageSettings;
use crate::stdcore::backends::{html, latex, latex_recomposed, latex_rendered, lowering, pieces, text};
use crate::stdcore::features::annotations::{AnnotationsConfig, AnnotationsFeature};
use crate::stdcore::features::blocks::{Blocks, BlocksConfig};
use crate::stdcore::features::defterm::{Defterm, DeftermConfig};
use crate::stdcore::features::cells::{Cells, CellsConfig};
use crate::stdcore::features::citations::{Citations, CitationsConfig};
use crate::stdcore::features::floats::{Floats, FloatsConfig};
use crate::stdcore::features::graphics::{Graphics, GraphicsConfig};
use crate::stdcore::features::headings::{Headings, HeadingsConfig};
use crate::stdcore::features::lists::{Lists, ListsConfig};
use crate::stdcore::features::math::{Math, MathConfig};
use crate::stdcore::features::numbering::{Numbering, NumberingConfig};
use crate::stdcore::features::quote::{Quote, QuoteConfig};
use crate::stdcore::features::refs::RefsConfig;
use crate::stdcore::features::theorems::{Theorems, TheoremsConfig};
use crate::stdcore::features::userdefs::{Userdefs, UserdefsConfig};
use crate::stdcore::features::xnotes::{XnotesConfig, XnotesFeature};
use crate::stdcore::features::{
    href, index, input, literals, meta, refpoints, refs, textstyles, toc, verbatim,
};

/// The name of the `stdcore` package, `stdcore`.
///
/// A registry builder adds a package's features once per package name, so a
/// package that several packages use is registered once
/// ([`RegistryBuilder::use_package`](crate::core::registry::RegistryBuilder::use_package)).
pub const PACKAGE: &str = "stdcore";

/// The registered name of the [`CheckWorkflow`] feature, `check`. The feature
/// and the workflow it declares ([`CHECK_WORKFLOW`]) have the same name.
pub const CHECK_FEATURE: &str = "check";

/// The name of the collection `flm.validated`, which every check of the standard
/// features joins: demanding it demands every member, so a plan that demands it
/// runs every check the registry holds.
///
/// The [`CheckWorkflow`] feature declares the collection. Every workflow this
/// package registers demands it: [`CHECK_WORKFLOW`] demands it alone, and the
/// workflows of the `text`, `html` and `latex-recomposed` backends demand it beside
/// their output, so a document gets the same diagnostics under every workflow
/// (with one exception, an unresolved reference). The [module
/// documentation](self#every-workflow-validates-the-collection-flmvalidated) lists
/// the members.
pub const VALIDATED: &str = "flm.validated";

/// The name of the `check` workflow, `check`: it demands [`VALIDATED`] and
/// nothing else, so a run of it validates the document and writes no output.
///
/// A rendering workflow runs the same checks, because it demands [`VALIDATED`]
/// beside its output. The only diagnostic whose severity differs is an
/// unresolved reference: a Warning under `check`, an Error under a rendering
/// workflow unless the render options of the `refs` feature allow it (see the
/// [module documentation](self#every-workflow-validates-the-collection-flmvalidated)).
pub const CHECK_WORKFLOW: &str = "check";

/// The configuration of the `stdcore` package: one field for each of the sixteen
/// standard features that take a configuration. The other fifteen features take
/// none.
///
/// [`package`] constructs each of these features from its field. Each field's
/// documentation says what it holds and what its default is; [`StdcoreConfig::new`]
/// is the default of every field. A feature's configuration is the plain Rust
/// value the feature is constructed from; it is fixed when the registry is built
/// and cannot vary per document or per render (see [settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// The configurations are fixed then because the commands the language knows, the
/// trees a run freezes and the reference points that are numbered depend on them.
///
/// A program that reads the configurations as data uses [`factory`], which reads
/// each field from the settings of its feature.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::StdcoreConfig;
/// use flm_core::stdcore::features::blocks::BlocksConfig;
/// use flm_core::stdcore::features::headings::HeadingsConfig;
///
/// let config = StdcoreConfig::new()
///     .blocks(BlocksConfig::no_typography())
///     .headings(HeadingsConfig::new().run_in_levels([4, 5]));
/// assert!(!config.blocks.auto_unicode_quotes);
/// assert!(config.headings.heading("paragraph").is_some_and(|kind| kind.run_in));
/// ```
#[derive(Debug, Clone, Default)]
pub struct StdcoreConfig {
    /// The `blocks` feature's text options.
    pub blocks: BlocksConfig,
    /// The `userdefs` feature's user definitions and expansion depth limit. A
    /// program that gathers definitions from several sources merges them into
    /// this one configuration.
    pub userdefs: UserdefsConfig,
    /// The `headings` feature's heading kinds and heading commands, and which
    /// kinds are numbered. By default, the five standard kinds `section` to
    /// `subparagraph`, none set as a run-in heading and none numbered.
    pub headings: HeadingsConfig,
    /// The `numbering` feature's style replacements: counter styles that each
    /// replace, by name, the counter style a feature declared. No replacement by
    /// default.
    pub numbering: NumberingConfig,
    /// The `math` feature's configuration: whether equations are numbered. They
    /// are by default.
    pub math: MathConfig,
    /// The `refs` feature's configuration: the texts that join the items of a
    /// reference to several targets. By default, `A and B` and `A, B, and C`.
    pub refs: RefsConfig,
    /// The `lists` feature's list environments: whether each numbers its
    /// items, and the labels it writes beside them at each depth. By default,
    /// `itemize`, `enumerate` and `description`.
    pub lists: ListsConfig,
    /// The `xnotes` feature's xnote kinds: each kind's commands, words,
    /// numbering, placement and list. By default, the one kind `footnote`,
    /// written `\footnote{…}`, numbered and listed after the content.
    pub xnotes: XnotesConfig,
    /// The `defterm` feature's configuration: the lead-in a term definition
    /// opens with, a template in which `#{term}` is the term. By default,
    /// `#{term}: `.
    pub defterm: DeftermConfig,
    /// The `theorems` feature's theorem classes and theorem kinds: the
    /// theorem-like environments, their words, numbering, heading, closing mark
    /// and back-reference. By default, the eight standard kinds, `theorem` to
    /// `proof`.
    pub theorems: TheoremsConfig,
    /// The `quote` feature's quote environments: the quotation section commands
    /// each accepts, and what content written outside any quotation section is
    /// read as. By default, `quote`, `blockquote` and `address`.
    pub quote: QuoteConfig,
    /// The `annotations` feature's annotators: each one's command and
    /// initials, in order. By default, no annotator, and therefore no command.
    pub annotations: AnnotationsConfig,
    /// The `graphics` feature's configuration: whether `\includegraphics` can be
    /// written anywhere in the document's text, and not only in a float's body.
    /// Not by default.
    pub graphics: GraphicsConfig,
    /// The `floats` feature's float types: each one's numbering policy,
    /// words, content handlers and label types. By default, `figure` and
    /// `table`, both numbered.
    pub floats: FloatsConfig,
    /// The `cells` feature's configuration: whether the `cells` environment can
    /// be written anywhere a block can, and not only in the body of a float of
    /// the `table` float type. Not by default.
    pub cells: CellsConfig,
    /// The `citations` feature's configuration: the form of a citation mark, the
    /// citation counter style and the text before a note. By default, bracketed marks:
    /// arabic numbers between `[` and `]`, the words `ref.~` and `refs.~`, and
    /// `; ` before a note.
    pub citations: CitationsConfig,
}

impl StdcoreConfig {
    /// The default configuration: every family of typographic substitutions
    /// on, no user definitions, the five standard heading kinds and their
    /// commands with none numbered, no counter style replaced, equations
    /// numbered, the default texts that join a reference to several targets,
    /// the three standard list environments, the one xnote kind `footnote`, the
    /// lead-in `#{term}: `, the eight standard theorem kinds, the three standard
    /// quote environments, no annotator, `\includegraphics` only in a float's
    /// body, the float types `figure` and `table`, the `cells` environment only
    /// in the body of a `table` float, and bracketed citation marks in the
    /// default citation counter style.
    pub fn new() -> StdcoreConfig {
        StdcoreConfig::default()
    }

    /// Builder: sets the `blocks` feature's text options.
    pub fn blocks(mut self, blocks: BlocksConfig) -> StdcoreConfig {
        self.blocks = blocks;
        self
    }

    /// Builder: sets the `userdefs` feature's user definitions and expansion
    /// depth limit.
    pub fn userdefs(mut self, userdefs: UserdefsConfig) -> StdcoreConfig {
        self.userdefs = userdefs;
        self
    }

    /// Builder: sets the `headings` feature's heading kinds, heading commands, and
    /// which kinds are numbered.
    pub fn headings(mut self, headings: HeadingsConfig) -> StdcoreConfig {
        self.headings = headings;
        self
    }

    /// Builder: sets the `numbering` feature's counter style replacements.
    pub fn numbering(mut self, numbering: NumberingConfig) -> StdcoreConfig {
        self.numbering = numbering;
        self
    }

    /// Builder: sets the `math` feature's configuration.
    pub fn math(mut self, math: MathConfig) -> StdcoreConfig {
        self.math = math;
        self
    }

    /// Builder: sets the `refs` feature's configuration.
    pub fn refs(mut self, refs: RefsConfig) -> StdcoreConfig {
        self.refs = refs;
        self
    }

    /// Builder: sets the `lists` feature's list environments.
    pub fn lists(mut self, lists: ListsConfig) -> StdcoreConfig {
        self.lists = lists;
        self
    }

    /// Builder: sets the `xnotes` feature's xnote kinds.
    pub fn xnotes(mut self, xnotes: XnotesConfig) -> StdcoreConfig {
        self.xnotes = xnotes;
        self
    }

    /// Builder: sets the `defterm` feature's configuration.
    pub fn defterm(mut self, defterm: DeftermConfig) -> StdcoreConfig {
        self.defterm = defterm;
        self
    }

    /// Builder: sets the `theorems` feature's theorem classes and theorem kinds.
    pub fn theorems(mut self, theorems: TheoremsConfig) -> StdcoreConfig {
        self.theorems = theorems;
        self
    }

    /// Builder: sets the `quote` feature's quote environments.
    pub fn quote(mut self, quote: QuoteConfig) -> StdcoreConfig {
        self.quote = quote;
        self
    }

    /// Builder: sets the `annotations` feature's annotators.
    pub fn annotations(mut self, annotations: AnnotationsConfig) -> StdcoreConfig {
        self.annotations = annotations;
        self
    }

    /// Builder: sets the `graphics` feature's configuration.
    pub fn graphics(mut self, graphics: GraphicsConfig) -> StdcoreConfig {
        self.graphics = graphics;
        self
    }

    /// Builder: sets the `floats` feature's float types.
    pub fn floats(mut self, floats: FloatsConfig) -> StdcoreConfig {
        self.floats = floats;
        self
    }

    /// Builder: sets the `cells` feature's configuration.
    pub fn cells(mut self, cells: CellsConfig) -> StdcoreConfig {
        self.cells = cells;
        self
    }

    /// Builder: sets the `citations` feature's configuration.
    pub fn citations(mut self, citations: CitationsConfig) -> StdcoreConfig {
        self.citations = citations;
        self
    }
}

/// Returns the package entry of the `stdcore` package: the thirty-three standard
/// features, each once, in the order the [module
/// documentation](self#the-features-of-the-package-and-where-each-is-documented)
/// lists them, with the sixteen configurable features constructed from the
/// fields of `config`.
///
/// Add the entry to a registry builder with
/// [`RegistryBuilder::use_package`](crate::core::registry::RegistryBuilder::use_package);
/// the module documentation has an example. A program that reads its settings
/// as data uses [`factory`] instead.
pub fn package(config: StdcoreConfig) -> PackageEntry {
    PackageEntry::new(PACKAGE)
        .feature(textstyles::feature())
        .feature(literals::feature())
        .feature(Arc::new(Blocks::new(config.blocks)))
        .feature(index::feature())
        .feature(refpoints::feature())
        // The owner of the two counter slots stands before every feature that
        // fills them, although the order decides nothing here.
        // See decision rationale at [§flm-dd-dr:std-numbering-place].
        .feature(Arc::new(Numbering::new(config.numbering.clone())))
        .feature(Arc::new(Math::new(config.math)))
        .feature(Arc::new(refs::Refs::new(config.refs.clone())))
        .feature(Arc::new(Headings::new(config.headings.clone())))
        .feature(verbatim::feature())
        .feature(Arc::new(Lists::new(config.lists)))
        .feature(Arc::new(XnotesFeature::new(config.xnotes)))
        .feature(href::feature())
        .feature(Arc::new(Defterm::new(config.defterm)))
        .feature(Arc::new(Theorems::new(config.theorems)))
        .feature(Arc::new(Quote::new(config.quote)))
        .feature(Arc::new(AnnotationsFeature::new(config.annotations)))
        .feature(Arc::new(Graphics::new(config.graphics)))
        .feature(Arc::new(Floats::new(config.floats)))
        .feature(Arc::new(Cells::new(config.cells)))
        .feature(Arc::new(Citations::new(config.citations)))
        .feature(input::feature())
        .feature(Arc::new(Userdefs::new(config.userdefs)))
        .feature(lowering::feature())
        .feature(meta::feature())
        .feature(toc::feature())
        // The owner of the pieces slot and option stands before the backends
        // that read them, although the order decides nothing here.
        // See decision rationale at [§flm-dd-dr:std-numbering-place].
        .feature(pieces::feature())
        .feature(text::feature())
        .feature(latex::feature())
        .feature(latex_recomposed::feature())
        .feature(latex_rendered::feature())
        .feature(html::feature())
        .feature(check_feature())
}

/// Returns the package factory of the `stdcore` package: a description of the
/// package that a program reads its settings against, and the operation that
/// builds the package entry from those settings.
///
/// A [`PackageFactory`] describes a package before its package entry is built.
/// This one has the name
/// `stdcore`, uses no other package, and lists the features in the order
/// [`package`] registers them, each with the declaration of its settings when
/// it takes some ([`FlmFeature::settings`]). [`PackageFactory::build`] then
/// reads each feature's configuration from the settings (the default where the
/// settings give none), hands the configurations to [`package`], and leaves out
/// every feature the settings switch off.
///
/// The settings also hold option values (values of a feature's option types).
/// The package entry does not take them: the program sets them on the document
/// and on the render request (see [`crate::core::settings::FeatureSettings`]).
/// A program written in Rust that builds its configuration in code calls
/// [`package`] directly.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::core::settings::PackageSettings;
/// use flm_core::stdcore;
///
/// let factory = stdcore::factory()?;
/// let math = factory.settings("math").expect("math takes settings");
/// let data = serde_json::json!({ "numbered_equations": false });
///
/// let mut settings = PackageSettings::new();
/// settings.insert(math.read(data.as_object().unwrap()).unwrap());
/// settings.switch_off("annotations");
///
/// let entry = factory.build(&settings)?;
/// assert!(entry.features.iter().all(|feature| feature.info().name != "annotations"));
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(entry);
/// assert!(builder.build(&RegistryInputs::new()).is_ok());
/// # Ok::<(), flm_core::core::registry::PackageError>(())
/// ```
///
/// # Errors
///
/// Returns the [`PackageError`] that [`PackageFactory::new`] reports when the
/// build operation fails for empty settings. The build of the `stdcore`
/// package never fails (every configuration has a default), so this function
/// does not fail.
pub fn factory() -> Result<PackageFactory, PackageError> {
    PackageFactory::new(PACKAGE, build_from_settings)
}

/// The build operation of [`factory`]: the package entry made by [`package`]
/// from the configurations the settings hold, without the features the settings
/// switch off.
fn build_from_settings(settings: &PackageSettings) -> Result<PackageEntry, PackageError> {
    use crate::stdcore::features::{
        annotations, blocks, cells, citations, defterm, floats, graphics, headings, lists, math,
        numbering, quote, theorems, userdefs, xnotes,
    };
    fn take<C: Clone + 'static>(settings: &PackageSettings, feature: &str, into: &mut C) {
        if let Some(config) = settings.config::<C>(feature) {
            *into = config.clone();
        }
    }
    let mut config = StdcoreConfig::new();
    take(settings, blocks::FEATURE, &mut config.blocks);
    take(settings, userdefs::FEATURE, &mut config.userdefs);
    take(settings, headings::FEATURE, &mut config.headings);
    take(settings, numbering::FEATURE, &mut config.numbering);
    take(settings, math::FEATURE, &mut config.math);
    take(settings, refs::FEATURE, &mut config.refs);
    take(settings, lists::FEATURE, &mut config.lists);
    take(settings, xnotes::FEATURE, &mut config.xnotes);
    take(settings, defterm::FEATURE, &mut config.defterm);
    take(settings, theorems::FEATURE, &mut config.theorems);
    take(settings, quote::FEATURE, &mut config.quote);
    take(settings, annotations::FEATURE, &mut config.annotations);
    take(settings, graphics::FEATURE, &mut config.graphics);
    take(settings, floats::FEATURE, &mut config.floats);
    take(settings, cells::FEATURE, &mut config.cells);
    take(settings, citations::FEATURE, &mut config.citations);
    let mut entry = package(config);
    entry.features.retain(|feature| !settings.is_off(&feature.info().name));
    Ok(entry)
}

/// The `check` feature: it declares the collection [`VALIDATED`]
/// (`flm.validated`) and the workflow [`CHECK_WORKFLOW`] (`check`), which
/// demands that collection, and nothing else.
///
/// The feature declares no element and no stage. A program that registers its
/// own features without the rest of the `stdcore` package can register this one
/// beside them ([`check_feature`]) to get the `check` workflow over the checks its
/// features add to the collection.
///
/// The workflow of every standard backend demands [`VALIDATED`] too, so a
/// registry that holds a standard backend feature must hold this feature: without
/// it, the registry build fails with an error that gives the backend's workflow
/// and the missing name `flm.validated`. See the [module
/// documentation](self#every-workflow-validates-the-collection-flmvalidated).
#[derive(Debug, Default, Clone, Copy)]
pub struct CheckWorkflow;

impl FlmFeature for CheckWorkflow {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(CHECK_FEATURE, "The check workflow").docs(
            "The collection `flm.validated`, which every check joins, and the `check` \
             workflow, which demands it: the document is validated and no output is written.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.collection(CollectionDef::new(VALIDATED).docs(
            "Every check the registry's features add has run. The standard features add \
             `flm.blocks.contract-checked`, `flm.refs.check-labels`, \
             `flm.theorems.check-proofs`, `flm.floats.check-handlers`, `flm.citations.check`, \
             `flm.meta.read` and `flm.input.spliced`; the feature that processes mathematics \
             adds its check (`math-passthrough` adds `flm.math.content`). Demanding the \
             collection demands every member.",
        ))?;
        b.workflow(
            WorkflowDef::new(CHECK_WORKFLOW)
                .demand(VALIDATED)
                .docs("validate the document: run every check the features add, and write no output"),
        )?;
        Ok(())
    }
}

/// Returns the [`CheckWorkflow`] feature, ready to be listed in a package entry or
/// registered beside the features whose checks the collection `flm.validated`
/// gathers.
pub fn check_feature() -> Arc<dyn FlmFeature> {
    Arc::new(CheckWorkflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_check_feature_declares_the_collection_and_the_workflow() {
        let info = CheckWorkflow.info();
        assert_eq!(info.name, CHECK_FEATURE);
        assert_eq!(VALIDATED, "flm.validated");
        assert_eq!(CHECK_WORKFLOW, "check");
    }

    #[test]
    fn the_package_lists_its_features_once_each_in_order() {
        let entry = package(StdcoreConfig::new());
        assert_eq!(entry.name, PACKAGE);
        assert!(entry.uses.is_empty());
        let names: Vec<String> =
            entry.features.iter().map(|feature| feature.info().name.to_string()).collect();
        assert_eq!(
            names,
            [
                "textstyles", "literals", "blocks", "index", "refpoints", "numbering", "math",
                "refs", "headings", "verbatim", "lists", "xnotes", "href", "defterm", "theorems",
                "quote",
                "annotations",
                "graphics",
                "floats",
                "cells",
                "citations",
                "input",
                "userdefs", "lowering", "meta", "toc", "pieces", "text", "latex", "latex-recomposed", "latex-rendered", "html",
                "check"
            ]
        );
        assert_eq!(names.len(), 33);
    }

    #[test]
    fn the_configuration_builders_set_their_fields() {
        use crate::stdcore::features::numbering::CounterStyleDecl;
        let config = StdcoreConfig::new()
            .blocks(BlocksConfig::no_typography())
            .userdefs(UserdefsConfig::new().recursion_limit(3))
            .headings(HeadingsConfig::new().run_in_levels([4, 5]))
            .numbering(NumberingConfig::new().style(CounterStyleDecl::new("a.style")));
        assert!(!config.blocks.auto_unicode_quotes);
        assert_eq!(config.userdefs.recursion_limit, 3);
        assert!(config.headings.heading("subparagraph").is_some_and(|kind| kind.run_in));
        assert_eq!(config.numbering.styles.len(), 1);
    }
}
