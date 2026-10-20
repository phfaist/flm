//! The `html` feature: registers the HTML backend, its render options, the
//! `html` output and the `html` workflow, and declares the four names the rest
//! of the library uses to refer to them.
//!
//! The `html` feature ([`FlmFeature`]),
//! implemented by [`HtmlBackend`] and obtained with [`feature()`], registers:
//!
//! - the construct element [`flm.pres.html.tag`](super::construct::TAG), one
//!   HTML tag as a node of the lowered tree;
//! - the seven stylesheets and the formula renderer's script that the backend
//!   ships, the definitions table `flm.html.definitions` and the stage that
//!   writes it ([`super::defs`]);
//! - the tables the backend's stages write: the lowered trees
//!   (`flm.lowered.html`), the HTML of each tree (`flm.output.html`), the page's
//!   artifact set (`flm.output.html.document`) with its capability
//!   [`flm.output.html.singlefile`](super::stages::SINGLEFILE), the artifact
//!   table `flm.output.html.artifacts` that the final stage reads, the lowered
//!   pieces (the document-level values a backend places beside the content,
//!   such as the title; [`Piece`](crate::core::piece::Piece))
//!   (`flm.lowered.html.pieces`), the HTML of each piece
//!   (`flm.output.html.pieces`) and the regions (`flm.output.html.regions`);
//! - the stage that builds the lowered pieces ([`PiecesStage`] for the `html`
//!   backend), the stage `flm.html.emit-pieces` and the regions stage
//!   `flm.html.regions`;
//! - the backend's regions output
//!   ([`RegionsOutput`]), contributed to
//!   the service slot `flm.output.regions`, so that a host's own page assembler
//!   can find the regions;
//! - the backend itself, with its native element set and its three stages
//!   ([`super::stages::stages`]);
//! - the `html` output and the `html` workflow.
//!
//! The render options are [`HtmlRenderOptions`], declared with the feature's
//! settings under the feature name `html`. The backend's name is the base of
//! the names of its stages (`flm.html.*`) and of its lowered-trees table
//! (`flm.lowered.html`). The identifiers of the conditions it reports
//! (`flm.html.*`) take the feature's name, [`FEATURE_NAME`], which is also
//! `html`.
//!
//! A future HTML output, such as a multi-file one, is meant to add a second
//! final stage to this backend rather than a backend of its own. A feature of
//! another crate whose lowering rule stages an HTML tag node (builds it into
//! the lowered tree; see [`Lowerer`](crate::core::lowering::Lowerer)) declares
//! the construct element through a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName),
//! [`FeatureBuilder::peer_element`](crate::core::registry::FeatureBuilder::peer_element))
//! instead of depending on this feature.
//!
//! # The elements the backend writes natively
//!
//! The native set ([`native_elements`]) holds 28 elements: every element of the
//! [presentation vocabulary](crate::stdcore::backends::pres), the construct
//! element [`flm.pres.html.tag`](super::construct::TAG), and the five
//! [structural elements](crate::guide::concepts_overview#structural-elements)
//! of [`FLOW_THROUGH`] that the backend emits natively, without a lowering rule,
//! because they carry nothing an HTML page has to translate: `flm.text`,
//! `flm.group`, `flm.comment`, `flm.container` and `flm.parbreak`.
//!
//! The native set holds **no element another feature owns**, and in
//! particular not the reference element `flm.ref`. The `refs` feature's own
//! lowering rule for `flm.ref` is therefore selected, and because that rule
//! reads the resolved references, the plan schedules the stage that resolves
//! them and, before it, the stage that computes the anchors. A heading then
//! reaches the emitter with the anchor that becomes its `id`. In a registry
//! without the `refs` feature, a heading carries no anchor and therefore no
//! `id`, as on every other backend.
//!
//! This backend needs the same [tree
//! state](crate::guide::concepts_overview#frozen-trees-and-computed-data) as the
//! text backend (blocks
//! structured, user definitions expanded), so one plan serves both, and a
//! request for the `text` and the `html` outputs together is one run. A
//! request for `html` and `latex-recomposed` together is refused, as one for
//! `text` and `latex-recomposed` is, because those backends need different
//! tree states.
//!
//! # What the `html` workflow demands
//!
//! The `html` [workflow](crate::core::registry::WorkflowDef) lists the `html`
//! output and also demands (asks the plan for a stage that provides;
//! [`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)) the
//! [capability](crate::core::registry::CapabilityDef)
//! [`flm.validated`](crate::stdcore::VALIDATED), which the document's checks
//! provide together, so that every workflow runs the same checks over a
//! document. The `check` feature
//! ([`check_feature`](crate::stdcore::check_feature)) declares that
//! capability, so a registry that holds this backend must hold the `check`
//! feature too.
//!
//! See the [module documentation](super) for the backend as a whole.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::artifact::ArtifactSet;
use crate::core::registry::{
    BackendDef, BuildError, CapabilityDef, DefContent, FeatureBuilder, FeatureInfo, FlmFeature,
    OutputDef, WorkflowDef,
};
use crate::lang::structural;
use crate::model::names::{ElementName, Name};
use crate::model::options::Bind;
use crate::stdcore::VALIDATED;
use crate::stdcore::backends::lowering::{LoweredTrees, lowered_table_name};
use crate::stdcore::backends::pieces::{LoweredPieces, PiecesStage, lowered_pieces_table_name};
use crate::stdcore::backends::pres;

use crate::core::region::RegionsOutput;
use crate::stdcore::backends::lowering::REGIONS_SLOT;

use super::construct;
use super::defs;
use super::stages::{
    HtmlEmitPiecesStage, HtmlRegionsStage, PIECES_TABLE, REGIONS,
    ARTIFACTS, DOCUMENT, EXTENSION, MEDIA_TYPE, OUTPUT_TABLE, SINGLEFILE, stages,
};

/// The feature's registered name, `html`. It is also the middle part of the
/// identifiers of the conditions this backend reports, `flm.html.*`, and the
/// name under which the feature's settings are declared.
pub const FEATURE_NAME: &str = "html";

/// The declaration of the feature's settings: the fields of
/// [`HtmlRenderOptions`] are its settings keys, bound to the render request,
/// and the key `run_in_stop` is marked as text in FLM markup
/// ([`ValueKind::Markup`](crate::core::settings::ValueKind::Markup)). See
/// [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE_NAME)
        .options::<HtmlRenderOptions>(
            Bind::Render,
            "the page's title and language tag; whether the elements written for a node carry \
            their position in the source; the stop written after a run-in heading's title; \
            what the host delivers in place of the declared content of a stylesheet or script; \
            the stylesheets and scripts the host requires for the page in addition to those its \
            content requires; the tag each heading kind is written with; and the heading level \
            written as `<h1>`. Set per render request.",
        )
        .markup("run_in_stop")
}

/// The backend's name, `html`.
///
/// A lowering rule marked as the default on this backend refers to it by this
/// name, and the names of the backend's lowering stage (`flm.html.lower`) and
/// lowered-trees table (`flm.lowered.html`) are derived from it.
pub const BACKEND: &str = "html";

/// The `html` output's name.
///
/// It is spelled like the backend's name but is the name of an output: a
/// workflow lists it
/// ([`WorkflowDef::output`](crate::core::registry::WorkflowDef::output)), and a
/// host looks the output up by it
/// ([`FlmRegistry::output`](crate::core::registry::FlmRegistry::output)).
pub const OUTPUT: &str = "html";

/// The `html` workflow's name.
///
/// It is spelled like the backend's name but is the name of a workflow, the
/// argument [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow)
/// takes.
pub const WORKFLOW: &str = "html";

/// The five [structural elements](crate::guide::concepts_overview#structural-elements)
/// the HTML backend emits natively, without a lowering rule, beside the
/// presentation vocabulary and the construct element: text, groups, source
/// comments, containers and paragraph breaks (see the [module
/// documentation](self)).
pub const FLOW_THROUGH: [&str; 5] = [
    structural::TEXT,
    structural::GROUP,
    structural::COMMENT,
    structural::CONTAINER,
    structural::PARBREAK,
];

/// Returns the backend's native element set, the elements its emitter writes
/// without a lowering rule: every element of the presentation vocabulary
/// ([`pres::ALL`]), the construct element
/// [`flm.pres.html.tag`](super::construct::TAG), then the elements of
/// [`FLOW_THROUGH`], in that order (28 elements).
///
/// The set holds no element another feature owns; the [module
/// documentation](self#the-elements-the-backend-writes-natively) says why.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::{FLOW_THROUGH, TAG, native_elements};
/// use flm_core::stdcore::backends::pres;
///
/// let native = native_elements();
/// assert_eq!(native.len(), pres::ALL.len() + 1 + FLOW_THROUGH.len());
/// assert_eq!(native.len(), 28);
/// assert!(native.iter().any(|element| element.as_str() == TAG));
/// // No element another feature owns: a reference is lowered by the `refs`
/// // feature's own rule, so the plan schedules the stage computing the anchors.
/// assert!(!native.iter().any(|element| element.as_str() == "flm.ref"));
/// ```
pub fn native_elements() -> Vec<ElementName> {
    pres::ALL
        .iter()
        .chain(std::iter::once(&construct::TAG))
        .chain(FLOW_THROUGH.iter())
        .map(|name| ElementName::from(*name))
        .collect()
}

// --- the render options -------------------------------------------------------------------

/// The delivery of a style or script definition: how the definition reaches
/// the page.
///
/// A style or script definition ([`StyleDef`](crate::core::registry::StyleDef),
/// [`ScriptDef`](crate::core::registry::ScriptDef)) is a stylesheet or a script
/// that a feature's output depends on, recorded in the registry under an
/// identifier; the backend that writes the output decides its *delivery*:
/// written into the output, loaded from an address, or left to the host (see
/// [style and script
/// definitions](crate::guide::concepts_overview#style-and-script-definitions)).
///
/// This type has two uses:
///
/// - a host gives one per definition identifier in the render option
///   [`HtmlRenderOptions::definitions`], in place of the definition's declared
///   content;
/// - each row of the definitions table records the delivery the definition
///   ended up with
///   ([`ResolvedDefinition::delivery`](super::defs::ResolvedDefinition::delivery)).
///
/// An entry of the option replaces the declared content and never changes
/// *what* the document requires: the definition is still listed in the
/// definitions table, so a host or a bundler reading that table learns that the
/// document needs it.
///
/// A delivery is an ordered list of parts, as a definition's declared content
/// is, and an entry of the option replaces the whole list. The formula
/// renderer's definition, for example, holds a configuration and then the
/// address of the program: a host that wants its own address writes both
/// parts, because a page without the configuration renders mathematics
/// wrongly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DefinitionOverride {
    /// The host's own page or bundle supplies the definition, and the page
    /// writes nothing for it.
    ///
    /// An empty list of parts also writes nothing, but only this variant tells
    /// a reader of the definitions table that the host supplies the
    /// definition.
    ProvidedByHost,
    /// The page writes these parts, in this order. Each part is text written
    /// into the page, an address the page loads, or a file, which this library
    /// does not read and reports
    /// ([`DefinitionNotInlinable`](super::defs::DefinitionNotInlinable)).
    Parts(Vec<DefContent>),
}

/// The render options of the HTML backend: the page's title and language, the
/// source-position attribute, the stop after a run-in heading, how stylesheets
/// and scripts are delivered, the stylesheets and scripts the host adds to the
/// page, how headings are tagged, and the heading level written as `<h1>`.
///
/// The option type is declared with [`Bind::Render`], so its values come from
/// the render request rather than from the document (see [settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// The values are set on the plan request
/// ([`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options())),
/// so two runs asking for different titles use two plans. The emit stages
/// (`flm.html.emit`, `flm.html.emit-pieces`), the resolve stage
/// `flm.html.resolve-definitions` and the regions stage `flm.html.regions` read
/// them; the final stage reads only the regions.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::{DEFAULT_TITLE, HtmlRenderOptions};
///
/// let options = HtmlRenderOptions::default();
/// assert_eq!(options.title, DEFAULT_TITLE);
/// assert_eq!(options.lang, None);
/// assert!(!options.source_spans);
/// assert_eq!(options.run_in_stop, ".");
/// assert!(options.definitions.is_empty());
/// assert!(options.required_definitions.is_empty());
/// assert!(options.heading_tags.is_empty());
/// assert_eq!(options.root_heading_level, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HtmlRenderOptions {
    /// The page's title when the document states none: the words written into
    /// `<title>` (the output region [`title`](crate::core::region::TITLE)) when
    /// the render places no title piece
    /// ([`flm.meta.title`](crate::stdcore::features::meta::TITLE)) or the title
    /// piece holds no words. The default is [`DEFAULT_TITLE`].
    pub title: String,
    /// The page's language as a language tag (`en`, `de-CH`), written as the
    /// `lang` attribute of `<html>` (the output region
    /// [`lang`](crate::core::region::LANG)). `None`, the default, writes no
    /// attribute, because a page that claims a language it is not written in is
    /// worse for a reader using a screen reader than one that claims none.
    pub lang: Option<String>,
    /// Whether the emitter writes the attribute `data-flm-source-span`, which
    /// records where in the source a node was written, on the element it
    /// writes for each node.
    ///
    /// The attribute is written on every element written for a node, inline
    /// or block-level, once per node, when the node's position can be traced
    /// back to a source with a name. Its value is a JSON array of the source's
    /// name, the first byte of the range and the byte just past its end
    /// (`["book/ch1.flm",0,8]`), so a script reads it back with `JSON.parse`.
    /// It is written between single quotation marks, which HTML allows, so the
    /// raw page shows the JSON as JSON. Off by default: the attribute serves
    /// tools that map a place in the page back to the document, and it adds
    /// bytes to every page.
    pub source_spans: bool,
    /// The stop written after the title of a run-in heading, so that
    /// `\paragraph{Note} Some content.` reads as `Note. Some content.` A
    /// run-in heading is a heading set at the start of the paragraph it
    /// introduces, on the paragraph's first line
    /// ([`headings`](crate::stdcore::features::headings)). The default is
    /// [`DEFAULT_RUN_IN_STOP`](crate::stdcore::backends::pres::DEFAULT_RUN_IN_STOP),
    /// a period; the empty text writes none.
    ///
    /// The stop is written as escaped text inside the run-in decoration's
    /// `<span>`, after the title, so a reader copies it with the rest of the
    /// page and a host that replaces the stylesheet keeps it. It is written
    /// only when the title holds text and does not already end in sentence
    /// punctuation
    /// ([`ends_in_sentence_punctuation`](crate::stdcore::backends::pres::ends_in_sentence_punctuation)).
    /// That test looks only at the characters the title ends with, so a title
    /// that ends in a formula, or in a box an author's own command draws, gets
    /// the stop after it. A host that needs headings written another way
    /// registers a lowering rule or a stage of its own ([the extension
    /// guide](crate::guide::extending#lowering-rules)).
    pub run_in_stop: String,
    /// The deliveries the host gives in place of the declared content of
    /// style and script definitions, by definition identifier
    /// ([`DefinitionOverride`]). Empty by default: every definition is then
    /// delivered as its declaration says.
    ///
    /// An identifier that no feature declares is ignored and reported as the
    /// Warning [`flm.html.unknown-definition`](super::defs::UnknownDefinition).
    pub definitions: BTreeMap<Name, DefinitionOverride>,
    /// The style and script definitions the host requires for the page as a
    /// whole, by identifier, in addition to those the document's content
    /// requires. Empty by default: a page then carries exactly what its content
    /// needs.
    ///
    /// A host that adds something of its own to every page it serves (for
    /// example a script that keeps an open page in step with the document, and
    /// its stylesheet) registers it as a definition of one of its features and
    /// lists its identifier here, rather than editing the page after it was
    /// written. A definition listed here appears once in the [definitions
    /// table](super::defs::DEFINITIONS_TABLE), like any other, whether or not
    /// the content requires it too, and reaches the page the same way;
    /// [`definitions`](Self::definitions) still decides its delivery. An
    /// identifier that no feature declares is reported as the Error
    /// [`flm.html.unknown-requirement`](super::defs::UnknownRequirement), as a
    /// requirement of the content would be.
    pub required_definitions: BTreeSet<Name>,
    /// The tag a block heading of each heading kind is written with, keyed by
    /// the heading kind's name (`section`), the tag written without angle
    /// brackets (`h2`). Empty by default. A heading kind the map does not
    /// list is written `<h1>` to `<h6>` by its level
    /// ([`root_heading_level`](Self::root_heading_level)).
    ///
    /// The title above every list of xnotes (notes set apart from the running
    /// text, such as footnotes; see
    /// [`xnotes`](crate::stdcore::features::xnotes)) is a heading of the
    /// heading kind `xnotes`
    /// ([`TITLE_HEADING`](crate::stdcore::features::xnotes::TITLE_HEADING)),
    /// for every xnote kind: mapping `xnotes` to `h3` writes every such title
    /// `<h3>`.
    ///
    /// The tag is not part of the [HTML contract](super::CONVENTIONS): the
    /// attributes a block heading carries (`data-flm-heading`,
    /// `data-flm-level`, `data-flm-toc`) are the same regardless of its tag. A
    /// heading of a run-in kind is written as a `<span>` carrying the same
    /// attributes inside its run-in decoration, never with this tag. A tag
    /// that is not a writable HTML tag name
    /// ([`is_valid_tag_name`](super::names::is_valid_tag_name)) is reported as
    /// [`flm.html.invalid-name`](super::emit::InvalidName), and the heading is
    /// written with the tag of its level.
    pub heading_tags: BTreeMap<Name, String>,
    /// The heading level the page writes as its first-level heading, `<h1>`.
    ///
    /// A heading's level is a number counted from zero, the smaller the outer
    /// ([`HeadingPayload::level`](crate::stdcore::backends::pres::HeadingPayload::level));
    /// a heading at this level is `<h1>`, one level deeper `<h2>`, and so on
    /// to `<h6>`, which every deeper level shares. The default is
    /// [`DEFAULT_ROOT_HEADING_LEVEL`](crate::stdcore::backends::pres::DEFAULT_ROOT_HEADING_LEVEL),
    /// 1, the level of a section: a section is `<h1>`. A root of 0 writes a
    /// kind declared at level 0, a chapter, `<h1>` and a section `<h2>`; a
    /// root of 2 writes the content of one section with its subsections as
    /// the first-level headings.
    ///
    /// A heading whose level is less than the root is reported as
    /// [`flm.pres.heading-above-root`](crate::stdcore::backends::pres::HeadingAboveRoot)
    /// and written `<h1>`, or with its tag where
    /// [`heading_tags`](Self::heading_tags) maps its kind: the mapping decides
    /// the tag and nothing else. The attribute `data-flm-level` holds a
    /// heading's own level regardless of the root. The option concerns the
    /// page's tags only: the numbering, the run-in headings and the
    /// identifiers read a heading's own level, as every other output does. The
    /// text output has the same option
    /// ([`TextRenderOptions::root_heading_level`](crate::stdcore::backends::text::TextRenderOptions::root_heading_level)).
    pub root_heading_level: u8,
}

/// The title a page carries when neither the document nor the render request
/// gives one: `FLM document` ([`HtmlRenderOptions::title`]).
pub const DEFAULT_TITLE: &str = "FLM document";

impl Default for HtmlRenderOptions {
    fn default() -> HtmlRenderOptions {
        HtmlRenderOptions {
            title: String::from(DEFAULT_TITLE),
            lang: None,
            source_spans: false,
            run_in_stop: String::from(pres::DEFAULT_RUN_IN_STOP),
            definitions: BTreeMap::new(),
            required_definitions: BTreeSet::new(),
            heading_tags: BTreeMap::new(),
            root_heading_level: pres::DEFAULT_ROOT_HEADING_LEVEL,
        }
    }
}

// --- the feature --------------------------------------------------------------------------

/// The `html` feature, which registers the HTML backend: its construct
/// element, its definitions, its tables, its render options, its stages, the
/// `html` output and the `html` workflow (the [module documentation](self)
/// lists them).
///
/// Use [`feature()`] to obtain it as a shared value that a package entry can
/// list; [`feature()`] also says which features must be registered beside it.
#[derive(Debug, Default, Clone, Copy)]
pub struct HtmlBackend;

impl FlmFeature for HtmlBackend {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE_NAME, "HTML backend").docs(
            "HTML output for a browser: the construct element `flm.pres.html.tag`, the `html` \
             backend, the `html` output and the `html` workflow",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        construct::register(b)?;
        defs::register(b)?;
        b.table_named::<LoweredTrees>(
            lowered_table_name(&BACKEND.into()),
            "the HTML backend's lowered trees, one per document tree",
        )?;
        b.table(
            OUTPUT_TABLE,
            "the HTML of each document tree, and the stylesheets and scripts each tree requires",
        )?;
        b.table::<ArtifactSet>(DOCUMENT, "the page as the entry artifact, and the files beside it")?;
        b.table::<ArtifactSet>(
            ARTIFACTS,
            "files a stage produces for the page to link to, added to the page's set",
        )?;
        b.capability(
            CapabilityDef::new(SINGLEFILE)
                .refines(DOCUMENT.name())
                .default()
                .docs(
                    "the page written as one HTML file",
                ),
        )?;
        b.table_named::<LoweredPieces>(
            lowered_pieces_table_name(&BACKEND.into()),
            "the pieces that the render request places, lowered for the HTML backend",
        )?;
        b.stage(Arc::new(PiecesStage::new(BACKEND)))?;
        b.table(
            PIECES_TABLE,
            "the HTML of each piece that the render request places, and the stylesheets and \
             scripts each piece requires",
        )?;
        b.stage(Arc::new(HtmlEmitPiecesStage))?;
        b.table(REGIONS, "the parts the page is made of: the content, the pieces, the stylesheets \
                          and the scripts")?;
        b.stage(Arc::new(HtmlRegionsStage))?;
        b.contribute_to::<RegionsOutput>(
            REGIONS_SLOT,
            Arc::new(RegionsOutput {
                output: OUTPUT.into(),
                backend: BACKEND.into(),
                document: DOCUMENT.name().into(),
                capability: SINGLEFILE.into(),
                regions: REGIONS.handle(),
                artifacts: vec![ARTIFACTS.handle()],
            }),
        )?;
        let (lowering, emit, final_stage) = stages();
        b.backend(
            BackendDef::new(BACKEND, native_elements(), lowering, emit, final_stage)
                .docs("HTML for a browser"),
        )?;
        b.output(
            OutputDef::new(OUTPUT, SINGLEFILE, DOCUMENT.name(), MEDIA_TYPE, EXTENSION)
                .docs("the document as one HTML page"),
        )?;
        b.workflow(
            WorkflowDef::new(WORKFLOW)
                .output(OUTPUT)
                .demand(VALIDATED)
                .docs("render the document as an HTML page, running every check"),
        )?;
        Ok(())
    }
}

/// Returns the `html` feature ([`HtmlBackend`]) as a shared value, ready to be
/// listed in a package entry.
///
/// A package that lists it must also list three other features:
///
/// - the `lowering` feature, which registers the presentation vocabulary this
///   backend writes natively;
/// - the `pieces` feature
///   ([`pieces::feature`](crate::stdcore::backends::pieces::feature())), which
///   declares the render option the pieces stage reads; without it, every plan
///   of a registry holding this backend fails;
/// - the `check` feature ([`check_feature`](crate::stdcore::check_feature)),
///   which declares the capability [`flm.validated`](crate::stdcore::VALIDATED)
///   that the `html` workflow demands beside its output.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html;
///
/// assert_eq!(html::feature().info().name.as_str(), html::FEATURE_NAME);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(HtmlBackend)
}
