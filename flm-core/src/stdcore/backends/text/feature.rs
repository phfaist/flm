//! The `text` feature: it registers the text backend, its native element
//! set, its stages and tables, its render options, the `text` output and the
//! `text` workflow.
//!
//! A feature ([`FlmFeature`]) contributes a related bundle of things to a
//! registry. The `text` feature ([`TextBackend`], obtained as a shared value
//! from [`feature()`]) registers next to the `lowering` feature
//! ([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature)),
//! which registers the presentation vocabulary and the shared lowering rules,
//! and next to the `pieces` and `check` features; [`feature()`] lists what each
//! of them provides to this one.
//!
//! # The elements the backend writes natively
//!
//! The backend's native set ([`native_elements`]) is the whole [presentation
//! vocabulary](crate::stdcore::backends::pres) plus the five [structural
//! elements](crate::lang::structural::StructuralElements) of [`FLOW_THROUGH`],
//! whose meaning plain text does not need to translate: `flm.text`,
//! `flm.group`, `flm.comment`, `flm.container` and `flm.parbreak`. The
//! paragraph-break element `flm.parbreak` is in that list because the
//! block-structure pass carries an [opaque
//! region](crate::guide::concepts_overview#opaque-regions), such as
//! mathematics, over unchanged and leaves the paragraph breaks inside it in
//! place. The emitter treats it as a
//! separation between blocks in a run of blocks (including the top level of a
//! tree that no block-structure pass has gone over), and as a space inside
//! inline content.
//!
//! The native set holds no element of another feature. Each feature registers
//! lowering rules for its own elements, and the backend writes only the
//! presentation elements those rules produce, so the backend never decides
//! from outside a feature how that feature's content is printed. The
//! `math-passthrough` feature, for instance, lowers its own fallback element
//! into a `flm.pres.math-passthrough` node holding the original source.
//!
//! # What the `text` workflow demands
//!
//! The `text` [workflow](crate::core::registry::WorkflowDef) lists the `text`
//! output and demands one capability beside it:
//! [`flm.validated`](crate::stdcore::VALIDATED), a capability whose demand
//! schedules every check the registry holds. A render of the `text` workflow
//! therefore validates the document: a refused label type, a duplicate label
//! or an error in mathematics content is reported exactly as by the [`check`
//! workflow](crate::stdcore::CHECK_WORKFLOW), so a document gets the same
//! diagnostics regardless of the workflow run over it. The capability
//! `flm.validated` is declared by the `check` feature
//! ([`check_feature`](crate::stdcore::check_feature)), so a registry offering
//! this backend must hold that feature too; a registry without it fails to
//! build, with an error that gives this workflow and the undeclared
//! capability.
//!
//! Everything else the run needs follows from what the selected stages and
//! lowering rules require, not from a list of demands: for instance, the
//! lowering rule for paragraphs requires the capability
//! `flm.blocks.structured`, which puts the block-structure rewrite into the
//! plan.

use std::sync::Arc;

use crate::core::region::RegionsOutput;
use crate::core::settings::SettingsDecl;
use crate::stdcore::backends::lowering::REGIONS_SLOT;
use crate::model::names::ElementName;
use crate::model::options::Bind;
use crate::core::registry::{
    BackendDef, BuildError, FeatureBuilder, FeatureInfo, FlmFeature, OutputDef, WorkflowDef,
};
use crate::lang::structural;
use crate::stdcore::VALIDATED;
use crate::stdcore::backends::lowering::{LoweredTrees, lowered_table_name};
use crate::stdcore::backends::pieces::{LoweredPieces, PiecesStage, lowered_pieces_table_name};
use crate::stdcore::backends::pres;

use super::emit::TEXT_OUTPUT;
use super::stages::{
    BACKEND, EXTENSION, MEDIA_TYPE, TEXT_DOCUMENT, TEXT_REGIONS, TextRegionsStage,
    TextRenderOptions, WORKFLOW, stages,
};

/// The feature's registered name, `text`. It is also the middle part of the
/// identifiers of the conditions this backend reports (`flm.text.*`).
pub const FEATURE_NAME: &str = "text";

/// The declaration of the feature's settings: the fields of
/// [`TextRenderOptions`] are its settings keys, bound to the render request
/// ([`Bind::Render`]). See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE_NAME)
        .options::<TextRenderOptions>(
            Bind::Render,
            "the line width in columns (null: never wrap), default 80; the width of a tab in \
            columns, default 4; the stop written after a run-in heading's title, default `.`, \
            the empty text for none; whether a link to an address outside the document prints \
            the address after its content, default on; whether a link whose content already \
            shows its address prints the address too, default off; and the heading level \
            printed as a first-level heading, default 1. Set per render request.",
        )
        .markup("run_in_stop")
}

/// The five [structural elements](crate::lang::structural::StructuralElements)
/// the text backend writes natively, beside the presentation vocabulary:
/// `flm.text`, `flm.group`, `flm.comment`, `flm.container` and `flm.parbreak`
/// (see the module documentation).
pub const FLOW_THROUGH: [&str; 5] = [
    structural::TEXT,
    structural::GROUP,
    structural::COMMENT,
    structural::CONTAINER,
    structural::PARBREAK,
];

/// Returns the backend's native element set, the elements its emitter writes
/// without a lowering rule: every element of the presentation vocabulary
/// ([`pres::ALL`]), then the elements of [`FLOW_THROUGH`].
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pres;
/// use flm_core::stdcore::backends::text::{FLOW_THROUGH, native_elements};
///
/// let native = native_elements();
/// assert_eq!(native.len(), pres::ALL.len() + FLOW_THROUGH.len());
/// assert!(native.iter().any(|element| element.as_str() == pres::PAR));
/// ```
pub fn native_elements() -> Vec<ElementName> {
    pres::ALL.iter().chain(FLOW_THROUGH.iter()).map(|name| ElementName::from(*name)).collect()
}

/// The `text` feature, which registers the text backend (see the module
/// documentation).
///
/// [`feature()`] returns it as a shared value that a package entry can list.
/// It registers: the lowered-trees table `flm.lowered.text` that the lowering
/// stage writes; the pieces stage ([`PiecesStage`] for `text`) and the
/// lowered-pieces table `flm.lowered.text.pieces` it writes; the regions stage
/// ([`TextRegionsStage`]) and its table `flm.output.text.regions`, together
/// with the [`RegionsOutput`] that describes the `text` output's regions to a
/// host (contributed to the service slot `flm.output.regions`); the output
/// tables `flm.output.text` and `flm.output.text.document`; the settings of
/// [`TextRenderOptions`]; the backend with its lowering, emit and final
/// stages; the `text` output; and the `text` workflow.
#[derive(Debug, Default)]
pub struct TextBackend;

impl FlmFeature for TextBackend {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE_NAME, "Text backend")
            .docs("plain-text output over the layout engine: the `text` output and workflow")
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table_named::<LoweredTrees>(
            lowered_table_name(&BACKEND.into()),
            "the text backend's lowered trees, one per document tree",
        )?;
        b.table_named::<LoweredPieces>(
            lowered_pieces_table_name(&BACKEND.into()),
            "the pieces that the render request places, lowered for the text backend",
        )?;
        b.stage(Arc::new(PiecesStage::new(BACKEND)))?;
        b.table(TEXT_REGIONS, "the output's one region: the printed text")?;
        b.stage(Arc::new(TextRegionsStage))?;
        b.contribute_to::<RegionsOutput>(
            REGIONS_SLOT,
            Arc::new(RegionsOutput {
                output: BACKEND.into(),
                backend: BACKEND.into(),
                document: TEXT_DOCUMENT.name().into(),
                capability: TEXT_DOCUMENT.name().into(),
                regions: TEXT_REGIONS.handle(),
                artifacts: Vec::new(),
            }),
        )?;
        b.table(
            TEXT_OUTPUT,
            "the printed text and its position marks, which record where each presentation node \
             was printed",
        )?;
        b.table(TEXT_DOCUMENT, "the printed text as a one-artifact set")?;
        let (lowering, emit, final_stage) = stages();
        b.backend(
            BackendDef::new(BACKEND, native_elements(), lowering, emit, final_stage)
                .docs("plain text over the layout engine"),
        )?;
        b.output(
            OutputDef::new(
                BACKEND,
                TEXT_DOCUMENT.name(),
                TEXT_DOCUMENT.name(),
                MEDIA_TYPE,
                EXTENSION,
            )
            .docs("the document as plain text"),
        )?;
        b.workflow(
            WorkflowDef::new(WORKFLOW)
                .output(BACKEND)
                .demand(VALIDATED)
                .docs("render the document as plain text, running every check"),
        )?;
        Ok(())
    }
}

/// Returns the `text` feature ([`TextBackend`]) as a shared value, ready to be
/// listed in a package entry.
///
/// A registry holding it must also hold three other features:
///
/// - the `lowering` feature
///   ([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature)),
///   which registers the presentation vocabulary this backend writes
///   natively; without it, the registry fails to build, because the
///   backend's native elements are not declared;
/// - the `pieces` feature
///   ([`pieces::feature`](crate::stdcore::backends::pieces::feature())), which
///   declares the render options the pieces stage reads; without it, every
///   plan of this backend fails;
/// - the `check` feature ([`check_feature`](crate::stdcore::check_feature)),
///   which declares the capability
///   [`flm.validated`](crate::stdcore::VALIDATED) that the `text` workflow
///   demands; without it, the registry fails
///   to build.
///
/// The standard package ([`stdcore::package`](crate::stdcore::package)) lists
/// all four.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::text;
///
/// assert_eq!(text::feature().info().name.as_str(), text::FEATURE_NAME);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(TextBackend)
}
