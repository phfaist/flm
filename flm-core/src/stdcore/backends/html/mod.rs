//! The `html` backend: writes a document as an HTML page for a browser.
//!
//! A backend is the registration bundle for one output format: the elements its
//! emitter writes natively, a lowering stage, an emit stage and a final stage
//! ([`BackendDef`](crate::core::registry::BackendDef); see [backends, lowering,
//! and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! This module is the HTML backend. Its feature, [`HtmlBackend`], registers the
//! backend, the `html` output and the `html` workflow; the feature, the backend,
//! the output and the workflow are all named `html`. A package that lists the
//! feature ([`feature()`]) must also list the `lowering`, `pieces`
//! and `check` features.
//!
//! A run of the `html` workflow produces two things a caller can use:
//!
//! - the **page**: the entry artifact of the table `flm.output.html.document`
//!   ([`DOCUMENT`]), one HTML file that carries, or loads from an address, the
//!   stylesheets and the scripts the document requires;
//! - the **HTML of each document tree**, without a page around it, in the table
//!   `flm.output.html` ([`HtmlFragments`]), with the identifiers of the style
//!   and script definitions each tree requires (the stylesheets and scripts its
//!   output depends on; see [style and script
//!   definitions](crate::guide::concepts_overview#style-and-script-definitions)).
//!   A host that embeds the content in a page of its own reads this table, the
//!   table `flm.output.html.pieces` ([`HtmlPieces`]) of the pieces (document-level
//!   values a backend places beside the content, such as the title;
//!   [`Piece`](crate::core::piece::Piece)), and the definitions table
//!   `flm.html.definitions` ([`DEFINITIONS_TABLE`]).
//!
//! The page is assembled from output regions ([`Regions`](crate::core::region::Regions)):
//! named parts of the page (the content, each piece, the stylesheets, the
//! scripts, the title and the language) that the stage `flm.html.regions` writes
//! once. The backend's final stage assembles the page from them in a layout of
//! its own; a host may assemble the same regions through a template of its own
//! instead (see [output regions and the final
//! stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
//!
//! The [HTML backend chapter](crate::guide::rendering#the-html-backend) of the
//! rendering guide describes the output element by element.
//!
//! # Writing HTML: escaping and the trust guarantee
//!
//! Every byte of HTML this library writes goes through the modules below, and
//! they guarantee one thing: **text that comes from a document reaches the
//! page escaped, and by no other way**. Bytes are written unescaped only as a
//! [`Markup`] value, or as the content of a `<style>` or a `<script>` through
//! [`HtmlSink::raw_text_element`]; the documentation of each states what the
//! caller must ensure.
//!
//! The guarantee covers every byte outside mathematics. Inside a formula that
//! the feature of the package
//! [`math-passthrough`](crate::math_passthrough) writes,
//! nothing is inspected and the author is trusted: what an author writes there
//! reaches the reader's formula renderer as typed.
//! [`mod@mathjax`] says what that means for a host and what such a host
//! configures for itself.
//!
//! - [`escape`] — the escaping functions: one for text between tags, two for an
//!   attribute value.
//! - [`markup`] — [`Markup`], the value that carries HTML written without
//!   escaping.
//! - [`sink`] — [`HtmlSink`], the only interface through which HTML bytes are
//!   written: separate entry points for document text, for a tag and its
//!   attributes, for a markup value, and for the content of a `<style>` or a
//!   `<script>`; it also writes the line breaks and the indentation between
//!   block-level tags, and offers no way to read back what was written.
//! - [`names`] — how the names of presentation elements, roles and payload
//!   fields become the class and attribute names of the [HTML
//!   contract](CONVENTIONS) (the classes and attributes a stylesheet or a script
//!   may rely on in the output), and the checks a tag name, an attribute name,
//!   a role or a kind passes before it is written.
//! - [`address`] — which addresses a link and an image may carry, and what
//!   happens to the others.
//! - [`construct`] — the construct element [`construct::TAG`]
//!   (`flm.pres.html.tag`): one HTML tag as a node of the lowered tree, for a
//!   lowering rule that must put HTML in the page that no shared presentation
//!   element carries.
//!
//! # The backend's stages, tables and conventions
//!
//! - [`conventions`] — the HTML contract as data: one row per presentation
//!   element, saying which tag, classes and attributes the output carries for
//!   it, which payload fields it cannot express, and which style definition
//!   gives it its default look.
//! - [`mod@emit`] — the emitter: [`emit_tree`] writes the HTML of one lowered
//!   tree; the conditions `flm.html.invalid-name`, `flm.html.refused-address`,
//!   `flm.html.refused-in-math`, `flm.html.nesting-too-deep` and
//!   `flm.html.worker-thread-unavailable`.
//! - [`mod@stages`] — the emit stages [`HtmlEmitStage`] and
//!   [`HtmlEmitPiecesStage`], the regions stage [`HtmlRegionsStage`], the final
//!   stage [`HtmlFinalStage`], the tables they write and read, and the
//!   conditions `flm.html.stage-failed`, `flm.html.missing-subtree`,
//!   `flm.html.subtree-cycle` and `flm.html.piece-placement`.
//! - [`mod@defs`] — the stylesheets this backend ships, the stage
//!   [`ResolveDefinitionsStage`] that decides how each definition a document
//!   requires reaches the page, the table `flm.html.definitions` it writes, and
//!   the conditions `flm.html.unknown-requirement`,
//!   `flm.html.unknown-definition` and `flm.html.definition-not-inlinable`.
//! - [`mod@mathjax`] — the formula renderer's script definition
//!   `flm.html.mathjax`, what its shipped configuration does, and the
//!   configuration a host writes for itself when it renders mathematics written
//!   by authors it does not trust.
//! - [`mod@page`] — how the regions are made from the tables
//!   ([`page::regions`]) and how the final stage assembles the page from them
//!   ([`page::assemble`]).
//! - [`mod@feature`] — [`HtmlBackend`], the native element set
//!   ([`native_elements`]), the render options [`HtmlRenderOptions`], and the
//!   four names [`FEATURE_NAME`], [`BACKEND`], [`OUTPUT`] and [`WORKFLOW`].
//!
//! **Not yet implemented:** a multi-file output, which would be a second final
//! stage of this same backend.

pub mod address;
pub mod construct;
pub mod conventions;
pub mod defs;
pub mod emit;
pub mod escape;
pub mod feature;
pub mod markup;
pub mod mathjax;
pub mod names;
pub mod page;
pub mod sink;
pub mod stages;

pub use address::{AddressUse, RefusedAddress, check_address};
pub use construct::{HtmlTagPayload, TAG, TagBody};
pub use defs::{
    DEFINITIONS_TABLE, DefinitionKind, DefinitionNotInlinable, DefinitionPartMissing,
    NotInlinablePart, RESOLVE_STAGE,
    ResolveDefinitionsStage, ResolvedDefinition, ResolvedDefinitions, STYLE_PAGE,
    UnknownDefinition, UnknownRequirement,
};
pub use conventions::{
    AUXILIARY_CLASSES, CONTENT_CLASS, CONVENTIONS, Convention, LIST_LABEL_CLASS, TagRule,
    UNKNOWN_MARKER_CLASS, markdown_table,
};
pub use emit::{
    AddressRefused, DEPTH_LIMIT, EmittedTree, InvalidName, NestingTooDeep,
    NoWorkerThread, RefusedInMath, emit_tree,
};
pub use escape::{escape_attr_into, escape_attr_single_quoted_into, escape_text_into};
pub use feature::{
    BACKEND, DEFAULT_TITLE, DefinitionOverride, FEATURE_NAME, FLOW_THROUGH, HtmlBackend,
    HtmlRenderOptions, OUTPUT, WORKFLOW, feature, native_elements,
};
pub use markup::Markup;
pub use mathjax::{MATHJAX_LOADER, mathjax_script};
pub use names::{
    class_of_element, class_of_role, data_attribute, is_valid_attribute_name, is_valid_tag_name,
    is_valid_token, style_property,
};
pub use page::HtmlRegions;
pub use sink::{HtmlSink, RawTextRefused, TagBuilder};
pub use stages::{
    ARTIFACT_NAME, ARTIFACT_OWNER, ARTIFACTS, DOCUMENT, EMIT_PIECES_STAGE, EMIT_STAGE, EXTENSION,
    FINAL_STAGE, HtmlEmitPiecesStage, HtmlEmitStage, HtmlFinalStage, HtmlFragment, HtmlFragments,
    HtmlPiece, HtmlPieces, HtmlRegionsStage, HtmlSegment, MEDIA_TYPE, MissingSubtree, OUTPUT_TABLE,
    PIECES_TABLE, PiecePlacement, REGIONS, REGIONS_STAGE, SINGLEFILE, StageFailed, SubtreeCycle, stages,
};
