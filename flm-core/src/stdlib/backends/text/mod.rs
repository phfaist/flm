//! The text backend: it prints a document as plain text, wrapped to a line
//! width, through the [layout engine](crate::layout).
//!
//! A backend is the registration bundle for one output format: the elements
//! its emitter writes natively, a lowering stage, an emit stage and a final
//! stage ([`BackendDef`](crate::core::registry::BackendDef); see [backends,
//! lowering, and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! The text backend is registered by the `text` feature ([`TextBackend`],
//! [`feature()`]), which also registers the `text` output and the `text`
//! workflow ([`WORKFLOW`]). The standard features
//! ([`stdlib::add_features`](crate::stdlib::add_features)) include that feature
//! together with the features it needs (see [`feature()`]).
//!
//! # Using the backend
//!
//! A caller builds a plan for the `text` workflow
//! ([`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) with
//! [`WORKFLOW`]), runs it over a document, and reads one of two tables:
//!
//! - [`TEXT_OUTPUT`] (`flm.output.text`), a [`TextOutput`]: the printed text,
//!   its position marks, which give for each printed part the presentation
//!   node it was printed from ([`TextMark`]), and its style marks, which give
//!   the printed ranges whose content carries a property plain text cannot
//!   show, such as bold weight or a link's address ([`TextStyleMark`]);
//! - [`TEXT_DOCUMENT`] (`flm.output.text.document`), the `text` output's
//!   artifact set: one entry artifact `text:document` of media type
//!   `text/plain`.
//!
//! The render options ([`TextRenderOptions`]) set the line width, the width
//! of a tab, the stop after a run-in heading's title, whether links print
//! their address, the heading level printed as a first-level heading, how a
//! table is drawn ([`TableFrame`]) and the characters of the rules under the
//! headings ([`HeadingRules`]).
//! A program that writes the text to a medium able to show what plain text
//! cannot reads the style marks; the extension `profile-tty`, which is
//! compiled into the `flm` program, paints the printed text from them for a
//! terminal, under the same render options.
//! [The plain-text backend](crate::guide::rendering#the-plain-text-backend), in
//! the rendering guide, shows a complete run.
//!
//! # The stages
//!
//! The text backend writes natively every element of the [presentation
//! vocabulary](crate::stdlib::backends::pres) and the six [structural
//! elements](crate::lang::structural::StructuralElements) of [`FLOW_THROUGH`]
//! (see [`native_elements`]). A plan of the `text`
//! workflow runs, in order:
//!
//! 1. the lowering stage `flm.text.lower`: the shared
//!    [`LoweringStage`](crate::stdlib::backends::lowering::LoweringStage) for
//!    this backend, which writes the lowered trees into `flm.lowered.text`;
//! 2. the pieces stage `flm.text.pieces`: the shared
//!    [`PiecesStage`](crate::stdlib::backends::pieces::PiecesStage) for this
//!    backend, which lowers the [pieces](crate::core::piece) the render places
//!    (document-level values such as a list of notes) into
//!    `flm.lowered.text.pieces`;
//! 3. the emit stage `flm.text.emit` ([`TextEmitStage`]), which prints the
//!    pieces placed before the content, the lowered trees in document order
//!    and the pieces placed after the content into one [`TextOutput`];
//! 4. the regions stage `flm.text.regions` ([`TextRegionsStage`]), which
//!    writes that text as the output's one [output
//!    region](crate::core::region::Regions), its content;
//! 5. the backend's final stage `flm.text.final` ([`TextFinalStage`]), which
//!    writes the content as the artifact set of [`TEXT_DOCUMENT`].
//!
//! # Items of this module
//!
//! - [`mod@emit`]: the emitter [`emit()`], the output table [`TextOutput`]
//!   with its position marks ([`TextMark`]) and its style marks
//!   ([`TextStyleMark`], [`TextStyle`]), the depth limit [`DEPTH_LIMIT`], and the
//!   conditions `flm.text.missing-subtree` ([`MissingSubtree`]),
//!   `flm.text.subtree-cycle` ([`SubtreeCycle`]), `flm.text.piece-placement`
//!   ([`PiecePlacement`]), `flm.text.nesting-too-deep` ([`NestingTooDeep`])
//!   and `flm.text.worker-thread-unavailable` ([`NoWorkerThread`]).
//! - [`mod@stages`]: the names of the backend, its stages and tables,
//!   [`TextEmitStage`], [`TextRegionsStage`], [`TextFinalStage`],
//!   [`TextRenderOptions`], and the condition `flm.text.stage-failed`
//!   ([`StageFailed`]).
//! - [`mod@feature`]: [`TextBackend`], the feature that registers the backend,
//!   and its native element set.
//! - [`mod@superscript`]: the Unicode superscript and subscript forms with
//!   which a superscript or subscript span is printed.

pub mod emit;
pub mod feature;
pub mod stages;
pub mod superscript;

pub use emit::{
    DEPTH_LIMIT, Emitted, MissingSubtree, NestingTooDeep, NoWorkerThread, PiecePlacement,
    SubtreeCycle, TEXT_OUTPUT, TextMark, TextOutput, TextStyle, TextStyleMark, emit,
};
pub use feature::{FEATURE_NAME, FLOW_THROUGH, TextBackend, feature, native_elements};
pub use stages::{
    ARTIFACT_NAME, ARTIFACT_OWNER, BACKEND, EMIT_STAGE, EXTENSION, FINAL_STAGE, HeadingRules,
    MEDIA_TYPE, REGIONS_STAGE, StageFailed, TEXT_DOCUMENT, TEXT_REGIONS, TableFrame,
    TextEmitStage, TextFinalStage, TextRegionsStage, TextRenderOptions, WORKFLOW, stages,
};
