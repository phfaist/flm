//! The `latex-recomposed` backend: it writes an FLM document back as LaTeX
//! source that stays as close as possible to what the author typed, and leaves
//! numbering, cross-references and every other resolution to LaTeX and to the
//! author's own document class.
//!
//! A backend ([`BackendDef`](crate::core::registry::BackendDef)) is the
//! registration bundle for one output format: the elements its emitter writes
//! natively, a lowering stage, an emit stage and a final stage (see [backends,
//! lowering, and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! This module holds the backend for recomposed LaTeX, registered by the
//! [`LatexRecomposed`] feature.
//!
//! # What the backend produces
//!
//! The output is an artifact set
//! ([`ArtifactSet`](crate::core::artifact::ArtifactSet)) of up to two LaTeX
//! files and any files other features add beside them:
//!
//! - the recomposed LaTeX of the document (the entry artifact), and
//! - a **style file**, a `.sty` file written beside it that holds exactly the
//!   definitions the recomposed LaTeX needs to compile: LaTeX packages,
//!   declarations of non-ASCII characters,
//!   [definition pieces](crate::stdcore::backends::latex::LatexPreambleDef)
//!   (units of LaTeX definitions that features register), and the document's
//!   own [user definitions](crate::lang::rewrites::userdefs::Definition)
//!   written as LaTeX definitions (see [`style`](crate::stdcore::backends::latex::style)). The
//!   definitions in it are guarded, so that a user can define any of them
//!   before the style file is loaded, or redefine it after, in the document's
//!   preamble. The exceptions are the definitions that the render option
//!   [`bare_names`](LatexRecomposedRenderOptions::bare_names) puts under the
//!   document's own names, which are written unconditionally, and the
//!   author's own text from
//!   [`extra_preamble`](LatexRecomposedRenderOptions::extra_preamble), which
//!   is written as given. The file's name is the render option
//!   [`style_stem`](LatexRecomposedRenderOptions::style_stem) and a hash of
//!   the file's content, to which the host's naming service may add a prefix
//!   (`paper-flmrecomposed.1a2b3c4d.sty` for a document written as
//!   `paper.tex`); the document loads the file by that name (see
//!   [`mod@stages`], "The style file's path").
//!
//! The recomposed LaTeX is written over the *raw* tree: the frozen tree in
//! which no paragraphs were built, no typographic substitution was made, and
//! the author's whitespace and blank lines are kept. Some constructs are
//! written in a fixed form rather than as the author typed them. A use site of
//! a user definition that the backend keeps is written as a call of the
//! definition that the style file holds, so that LaTeX never reads again
//! anything FLM has already read
//! ([`kept`](crate::stdcore::features::userdefs::kept)). With the default
//! render options, headings, labels and references are written through LaTeX
//! hooks ([`preamble`](crate::stdcore::backends::latex::preamble)), with each
//! label replaced by its safe label
//! ([`SafeLabels`](crate::stdcore::features::refs::SafeLabels)); see the
//! render options [`heading_hook`](LatexRecomposedRenderOptions::heading_hook)
//! and [`label_hooks`](LatexRecomposedRenderOptions::label_hooks). The syntax
//! of every call is normalized (see [`mod@emit`]).
//!
//! # Who registers what
//!
//! The [`LatexRecomposed`] feature registers the backend, its two structural
//! [lowering rules](crate::core::lowering::LoweringRule), its stages and
//! tables, the `latex-recomposed` output and the `latex-recomposed` workflow.
//! It relies on three other features: the `latex`
//! feature ([`LatexFamily`](crate::stdcore::backends::latex::LatexFamily)),
//! which registers the LaTeX construct elements
//! ([`construct`](crate::stdcore::backends::latex::construct)) and the service
//! slots through which package requests and definition pieces are resolved,
//! and the `lowering` feature
//! ([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature)),
//! which registers the shared presentation elements, and the `check` feature
//! ([`check_feature`](crate::stdcore::check_feature)), which declares the
//! collection `flm.validated` that the `latex-recomposed` workflow demands.
//!
//! Each content feature (headings, references, mathematics, and so on)
//! registers, in its own `latex` module, the lowering rule that writes its
//! elements back as LaTeX construct elements: its *recomposition rule* (see
//! [`mod@rules`] for how such a rule depends on this backend without the
//! feature depending on it). The emitter writes those construct elements
//! through the LaTeX family's typed sink
//! ([`LatexSink`](crate::stdcore::backends::latex::LatexSink)).
//!
//! # The modules of the backend
//!
//! - [`mod@feature`]: the [`LatexRecomposed`] feature, the native element set
//!   ([`native_elements`]) and the render options
//!   ([`LatexRecomposedRenderOptions`]).
//! - [`mod@rules`]: the two structural lowering rules (the paragraph break and
//!   the paragraph), which require the raw tree.
//! - [`mod@emit`]: the emitter ([`emit_tree`]), what each lowered node writes,
//!   and the conditions (the kinds of diagnostic it reports; see
//!   [diagnostics](crate::guide::concepts_overview#diagnostics))
//!   `flm.latex-recomposed.unknown-character`,
//!   `flm.latex-recomposed.nesting-too-deep` and
//!   `flm.latex-recomposed.worker-thread-unavailable`.
//! - [`mod@stages`]: the emit stage, the regions stage (which writes the
//!   placed pieces, assembles the style file and writes the output regions),
//!   the final stage, their tables, the artifact set, and the
//!   conditions `flm.latex-recomposed.stage-failed`,
//!   `flm.latex-recomposed.unknown-requirement`,
//!   `flm.latex-recomposed.package-options-conflict` and
//!   `flm.latex-recomposed.definition-name-refused`.
//!
//! [The recomposed LaTeX
//! backend](crate::guide::rendering#the-recomposed-latex-backend) section of the
//! rendering guide describes the backend in full: what each construct writes,
//! escaping and the Unicode modes, references and safe labels, mathematics,
//! verbatim content, and the plan a `latex-recomposed` workflow builds.

pub mod emit;
pub mod feature;
pub mod rules;
pub mod stages;

pub use emit::{
    DEPTH_LIMIT, Emitted, LatexFragment, NestingTooDeep, NoWorkerThread,
    UnknownCharacter, emit_tree,
};
pub use feature::{
    BACKEND, DEFAULT_STYLE_STEM, FEATURE_NAME, FLOW_THROUGH, LatexRecomposed,
    LatexRecomposedRenderOptions, SHARED_NATIVE, StyleFile, feature,
    native_elements,
};
pub use rules::{PARAGRAPH_RULE, PARBREAK_RULE, ParagraphRule, ParbreakRule, rules};
pub use stages::{
    ARTIFACT_OWNER, ARTIFACTS, DefinitionNameRefusal, DefinitionNameRefused, DOCUMENT, DOCUMENT_ARTIFACT,
    DefinitionNameOption, EMIT_STAGE,
    EXTENSION, FINAL_STAGE,
    LatexFragments, LatexPackages, LatexPreamble, LatexRecomposedEmitStage,
    LatexRecomposedFinalStage, LatexRecomposedRegionsStage, MEDIA_TYPE, REGIONS, REGIONS_STAGE, OUTPUT_TABLE, PACKAGES_TABLE, PREAMBLE_TABLE,
    STYLE_EXTENSION, STYLE_FILE, StageFailed, StylePathRefused, TREE_END, UnknownRequirement, WORKFLOW,
    ConflictingPackageOptions, stages, style_artifact,
};
