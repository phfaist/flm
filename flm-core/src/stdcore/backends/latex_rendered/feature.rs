//! The `latex-rendered` feature: the backend's names, its native element set,
//! its render options, the definition pieces of its contract (the chunks), and
//! the registration of its tables, stages, output and workflow.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::core::region::RegionsOutput;
use crate::core::registry::{
    BackendDef, BuildError, FeatureBuilder, FeatureInfo, FlmFeature, OutputDef, WorkflowDef,
};
use crate::core::settings::SettingsDecl;
use crate::lang::structural;
use crate::model::names::ElementName;
use crate::model::options::Bind;
use crate::stdcore::VALIDATED;
use crate::stdcore::backends::latex::packages::{self, AMSMATH, GRAPHICX, TABULARRAY, XCOLOR};
use crate::stdcore::backends::latex::preamble;
use crate::stdcore::backends::latex::{ExtraPreamble, LatexPackageDef, LatexPreambleDef, construct};
use crate::stdcore::backends::lowering::{LoweredTrees, REGIONS_SLOT, lowered_table_name};
use crate::stdcore::backends::pieces::{LoweredPieces, PiecesStage, lowered_pieces_table_name};
use crate::stdcore::backends::pres;

use super::contract::{
    CELLS_CHUNK, CORE_CHUNK, HEADINGS_CHUNK, IMAGES_CHUNK, LINES_CHUNK, LINKS_CHUNK, LISTS_CHUNK,
    MARKS_CHUNK, VERBATIM_CHUNK,
};
use super::stages::{
    ARTIFACTS, DOCUMENT, EXTENSION, LatexRenderedEmitPiecesStage, LatexRenderedRegionsStage,
    MEDIA_TYPE, OUTPUT_TABLE, PIECES_TABLE, REGIONS, STYLE_FILE, stages,
};

/// The registered name of the [`LatexRendered`] feature, `latex-rendered`. It
/// is also the middle part of the identifiers of the conditions this backend
/// reports, `flm.latex-rendered.*`.
pub const FEATURE_NAME: &str = "latex-rendered";

/// The backend's registered name, `latex-rendered`. A lowering rule marked as
/// the default on this backend refers to it, and the names of the backend's
/// lowering stage and lowered-trees table (`flm.lowered.latex-rendered`) are
/// derived from it.
pub const BACKEND: &str = "latex-rendered";

/// The `latex-rendered` output's name: spelled like the backend's name, but
/// the name of an output, which a workflow lists and a host looks up.
pub const OUTPUT: &str = "latex-rendered";

/// The `latex-rendered` workflow's name: it lists the `latex-rendered`
/// output and demands the collection [`flm.validated`](VALIDATED).
pub const WORKFLOW: &str = "latex-rendered";

/// The default stem of the style file's name, `flmx`; see
/// [`LatexRenderedRenderOptions::style_stem`].
pub const DEFAULT_STYLE_STEM: &str = "flmx";

/// The package `fontspec`, `flm.latex.pkg.fontspec`, which the core chunk
/// requires: under LuaLaTeX it gives the document its fonts.
pub const FONTSPEC: &str = "flm.latex.pkg.fontspec";

/// The structural elements that this backend's emitter writes natively,
/// without a lowering rule: `flm.text`, `flm.group`, `flm.opaque-wrapper`,
/// `flm.comment`, `flm.container` and `flm.parbreak`.
pub const FLOW_THROUGH: [&str; 6] = [
    structural::TEXT,
    structural::GROUP,
    structural::OPAQUE_WRAPPER,
    structural::COMMENT,
    structural::CONTAINER,
    structural::PARBREAK,
];

/// The backend's native element set, the elements its emitter writes without
/// a lowering rule: the twenty-two presentation elements ([`pres::ALL`]), the
/// four LaTeX construct elements ([`construct::ALL`]) and the six structural
/// elements of [`FLOW_THROUGH`], thirty-two in all.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::native_elements;
///
/// let native = native_elements();
/// assert_eq!(native.len(), 32);
/// assert!(native.iter().any(|element| element.as_str() == "flm.pres.par"));
/// assert!(native.iter().any(|element| element.as_str() == "flm.pres.latex.macro"));
/// ```
pub fn native_elements() -> Vec<ElementName> {
    pres::ALL
        .iter()
        .chain(construct::ALL.iter())
        .chain(FLOW_THROUGH.iter())
        .map(|name| ElementName::from(*name))
        .collect()
}

// --- the render options -------------------------------------------------------------------

/// The render options of the `latex-rendered` backend: the style file's name,
/// the stop after a run-in title, and the author's own packages and LaTeX
/// text in the style file.
///
/// The option type is declared with [`Bind::Render`]: its value is set on the
/// render request, never on the document. The same fields are the keys of the
/// feature's settings (see [`crate::core::settings`]).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::LatexRenderedRenderOptions;
///
/// let options = LatexRenderedRenderOptions::default();
/// assert_eq!(options.style_stem, "flmx");
/// assert_eq!(options.run_in_stop, ".");
/// assert!(options.skip_packages.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct LatexRenderedRenderOptions {
    /// The stem of the style file's name, `flmx` by default. The file is
    /// named `<stem>.<hash>.sty`, `<hash>` being eight hexadecimal digits of
    /// its content's digest, to which the program's naming of files may add a
    /// prefix (`paper-flmx.1a2b3c4d.sty`).
    pub style_stem: String,
    /// The stop written after a run-in heading's title and a run-in
    /// decoration's content, as the key `stop`, `.` by default; empty for
    /// none. It is not written where the title already ends in sentence
    /// punctuation.
    pub run_in_stop: String,
    /// LaTeX packages this backend never requests, each given by the name
    /// `\usepackage` takes, for a template that loads them itself. Empty by
    /// default. A package listed here is left out of the style file's
    /// package block, whoever requested it; listing `hyperref` leaves out
    /// the line that loads it at the end of the preamble. The author's own
    /// requests ([`extra_packages`](Self::extra_packages)) are not left out.
    pub skip_packages: BTreeSet<String>,
    /// LaTeX packages the style file always requests, with their options,
    /// merged after the requests the document's content causes into one
    /// `\RequirePackage` line per package. Empty by default.
    pub extra_packages: Vec<LatexPackageDef>,
    /// Three parts of LaTeX text written byte for byte into the style file:
    /// after the engine check, after the line loading `hyperref`, and at the
    /// end. Empty by default.
    pub extra_preamble: ExtraPreamble,
}

impl Default for LatexRenderedRenderOptions {
    fn default() -> LatexRenderedRenderOptions {
        LatexRenderedRenderOptions {
            style_stem: String::from(DEFAULT_STYLE_STEM),
            run_in_stop: String::from(pres::DEFAULT_RUN_IN_STOP),
            skip_packages: BTreeSet::new(),
            extra_packages: Vec::new(),
            extra_preamble: ExtraPreamble::default(),
        }
    }
}

/// The declaration of the feature's settings: one option type,
/// [`LatexRenderedRenderOptions`], bound to the render request.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE_NAME)
        .options::<LatexRenderedRenderOptions>(
            Bind::Render,
            "the stem of the style file's name (default `flmx`), to which the program's naming \
             of files may add a prefix; the stop written after a run-in heading's title; the \
             LaTeX packages this backend never requests, for a template that loads them \
             itself; the packages the style file always requests, with their options; and \
             three parts of LaTeX text written byte for byte into the style file, after the \
             engine check, after the packages and at its end. Set per render request.",
        )
        .markup("run_in_stop")
}

// --- the chunks ---------------------------------------------------------------------------

/// The definition pieces of the contract, one per concern, in the order the
/// style file writes them: the core chunk, then the chunks of headings,
/// links, lists, lines, verbatim text, tables of cells, images and the three
/// marks (see [`super::contract`]). Each piece's text is a fragment of
/// `latex_rendered/sty/`, and each piece requires the core chunk and the
/// packages it calls into; the core chunk requires `fontspec`, `amsmath` and
/// `xcolor`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::{chunks, contract};
///
/// let chunks = chunks();
/// assert_eq!(chunks.len(), 9);
/// assert_eq!(chunks[0].id.as_str(), contract::CORE_CHUNK);
/// assert!(chunks[0].content.contains("\\NewDocumentCommand \\flmxSpan"));
/// ```
pub fn chunks() -> Vec<LatexPreambleDef> {
    let chunk = |id: &str, content: &str, docs: &str| {
        let name = id.strip_prefix(preamble::ID_PREFIX).unwrap_or(id);
        LatexPreambleDef::new(name, content, docs)
    };
    vec![
        chunk(
            CORE_CHUNK,
            include_str!("sty/flmx-core.sty"),
            "the core of the latex-rendered contract: the keys, the registrations, the target \
             of an anchor, blocks, spans and decorations",
        )
        .requires([FONTSPEC, AMSMATH, XCOLOR]),
        chunk(HEADINGS_CHUNK, include_str!("sty/flmx-headings.sty"), "headings")
            .requires([CORE_CHUNK]),
        chunk(LINKS_CHUNK, include_str!("sty/flmx-links.sty"), "links").requires([CORE_CHUNK]),
        chunk(LISTS_CHUNK, include_str!("sty/flmx-lists.sty"), "lists").requires([CORE_CHUNK]),
        chunk(LINES_CHUNK, include_str!("sty/flmx-lines.sty"), "lines").requires([CORE_CHUNK]),
        chunk(VERBATIM_CHUNK, include_str!("sty/flmx-verbatim.sty"), "verbatim text")
            .requires([CORE_CHUNK]),
        chunk(CELLS_CHUNK, include_str!("sty/flmx-cells.sty"), "tables of cells")
            .requires([CORE_CHUNK, TABULARRAY]),
        chunk(IMAGES_CHUNK, include_str!("sty/flmx-images.sty"), "images")
            .requires([CORE_CHUNK, GRAPHICX]),
        chunk(
            MARKS_CHUNK,
            include_str!("sty/flmx-marks.sty"),
            "the marks of what could not be written",
        )
        .requires([CORE_CHUNK]),
    ]
}

/// The request for `fontspec` ([`FONTSPEC`]), which this feature contributes
/// to the LaTeX family's package slot.
fn fontspec() -> LatexPackageDef {
    LatexPackageDef::new(
        "fontspec",
        "the fonts of a document compiled with LuaLaTeX; the latex-rendered backend always \
         requests it",
    )
}

// --- the feature --------------------------------------------------------------------------

/// The feature that registers the `latex-rendered` backend (see the [module
/// documentation](super)).
///
/// Use [`feature()`] to obtain it as a value that a package entry can list.
/// When registered, it declares the tables of its stages, the pieces stage
/// ([`PiecesStage`], every piece placed by default), the stage writing the
/// pieces' LaTeX and the regions stage, its regions output for a host's
/// templates, the backend with its native set ([`native_elements`]) and its
/// lowering, emit and final stages, the `latex-rendered` output and workflow,
/// the chunks of its contract ([`chunks`]) and the request for `fontspec`,
/// and its settings ([`LatexRenderedRenderOptions`]).
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRendered;

impl FlmFeature for LatexRendered {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE_NAME, "LaTeX (rendered) backend").docs(
            "the document as fully resolved LaTeX for LuaLaTeX, written through one command or \
             environment per presentation element: the `latex-rendered` output and workflow, a \
             document and a style file",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table_named::<LoweredTrees>(
            lowered_table_name(&BACKEND.into()),
            "the rendered-LaTeX backend's lowered trees, one per document tree",
        )?;
        b.table(
            OUTPUT_TABLE,
            "per tree: the rendered LaTeX with the trees placed in it, the requirement \
             identifiers and the definitions it staged",
        )?;
        b.table_named::<LoweredPieces>(
            lowered_pieces_table_name(&BACKEND.into()),
            "the pieces that the render request places, lowered for the rendered-LaTeX backend",
        )?;
        b.stage(Arc::new(PiecesStage::new(BACKEND)))?;
        b.table(
            PIECES_TABLE,
            "the rendered LaTeX of each piece the render request places, with its words and \
             requirements",
        )?;
        b.stage(Arc::new(LatexRenderedEmitPiecesStage))?;
        b.table(REGIONS, "the content, the pieces and the style file's package name")?;
        b.table(STYLE_FILE, "the style file beside the rendered LaTeX")?;
        b.table(DOCUMENT, "the rendered LaTeX document, its style file and the files beside it")?;
        b.table(
            ARTIFACTS,
            "files a stage produces beside the rendered LaTeX for it to refer to (copies of \
             images), added to the document's set after the style file",
        )?;
        b.stage(Arc::new(LatexRenderedRegionsStage))?;
        b.contribute_to::<RegionsOutput>(
            REGIONS_SLOT,
            Arc::new(RegionsOutput {
                output: OUTPUT.into(),
                backend: BACKEND.into(),
                document: DOCUMENT.name().into(),
                capability: DOCUMENT.name().into(),
                regions: REGIONS.handle(),
                artifacts: vec![STYLE_FILE.handle(), ARTIFACTS.handle()],
            }),
        )?;
        let (lowering, emit, final_stage) = stages();
        b.backend(
            BackendDef::new(BACKEND, native_elements(), lowering, emit, final_stage)
                .docs("fully resolved LaTeX for LuaLaTeX, with a style file"),
        )?;
        b.output(
            OutputDef::new(OUTPUT, DOCUMENT.name(), DOCUMENT.name(), MEDIA_TYPE, EXTENSION)
                .docs("the document as fully resolved LaTeX for LuaLaTeX, with its style file"),
        )?;
        b.workflow(
            WorkflowDef::new(WORKFLOW).output(OUTPUT).demand(VALIDATED).docs(
                "render the document as LaTeX for LuaLaTeX, running every check and leaving \
                 only the typesetting to LaTeX",
            ),
        )?;
        b.peer_slot::<LatexPackageDef>(packages::SLOT, Arc::new(fontspec()))?;
        for chunk in chunks() {
            b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(chunk))?;
        }
        Ok(())
    }
}

/// Returns the [`LatexRendered`] feature as a shared value, ready to be listed
/// in a package entry.
///
/// A package that lists it must also list the `lowering` feature (the
/// presentation elements), the `latex` feature
/// ([`latex::feature`](crate::stdcore::backends::latex::feature())), which
/// registers the construct elements and the service slots its regions stage
/// reads, the `pieces` feature
/// ([`pieces::feature`](crate::stdcore::backends::pieces::feature())), and the
/// `check` feature ([`check_feature`](crate::stdcore::check_feature)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered;
///
/// assert_eq!(latex_rendered::feature().info().name.as_str(), latex_rendered::FEATURE_NAME);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(LatexRendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::packages::package_id;

    #[test]
    fn every_chunk_of_the_contract_is_a_piece_and_each_requires_the_core() {
        let chunks = chunks();
        let ids: Vec<&str> = chunks.iter().map(|chunk| chunk.id.as_str()).collect();
        assert_eq!(ids, super::super::contract::CHUNKS);
        for chunk in &chunks[1..] {
            assert!(chunk.requires.iter().any(|id| id.as_str() == CORE_CHUNK), "{}", chunk.id);
            assert!(chunk.content.starts_with(&format!("% {} -- ", chunk.id)), "{}", chunk.id);
        }
        assert_eq!(package_id("fontspec").as_str(), FONTSPEC);
    }
}
