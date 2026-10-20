//! The text backend's stages (emit, regions and final), its render options,
//! the names of its stages and tables, and the condition its stages report.
//!
//! A plan of the `text` workflow runs the backend's stages in this order; the
//! first two are shared stages created for this backend by other modules, and
//! the last three are defined here:
//!
//! 1. The **lowering stage** `flm.text.lower`
//!    ([`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage) for
//!    `text`) writes one lowered tree per document tree into
//!    `flm.lowered.text`.
//! 2. The **pieces stage** `flm.text.pieces`
//!    ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage) for
//!    `text`) lowers the [pieces](crate::core::piece) the render places
//!    (document-level values such as a list of notes) into
//!    `flm.lowered.text.pieces`.
//! 3. The **emit stage** `flm.text.emit` ([`TextEmitStage`]) prints the pieces
//!    placed before the content, every tree of the run in document order and
//!    the pieces placed after the content, one blank line between any two of
//!    them, and writes one [`TextOutput`] into `flm.output.text`. A tree placed
//!    inside another tree (at a `flm.pres.subtree` node) is printed at that
//!    node, so a wrapped line continues across the boundary between the two
//!    trees. The stage has `Document` scope ([`Scope::Document`]): the runner
//!    calls it once for the whole run rather than once per tree, because the
//!    printed text is one document.
//! 4. The **regions stage** `flm.text.regions` ([`TextRegionsStage`]) writes
//!    that text as the output's one [output region](crate::core::region::Regions),
//!    the content, into `flm.output.text.regions` ([`TEXT_REGIONS`]). The
//!    pieces are printed in the same text as the trees, since a wrapped line
//!    may continue across them, so the text backend has no region per piece.
//! 5. The **final stage** `flm.text.final` ([`TextFinalStage`]) is the
//!    backend's [final
//!    stage](crate::guide::concepts_overview#output-regions-and-the-final-stage):
//!    it writes the content region as an artifact set into
//!    `flm.output.text.document` ([`TEXT_DOCUMENT`]), with the single entry
//!    artifact `text:document` of media type `text/plain`. That table is the
//!    `text` output's, and the `text` workflow lists that output. A host that
//!    assembles the region through a template of its own provides the same
//!    table from the same region; the plan runs one of the two stages, the one
//!    the plan request prefers
//!    ([`Overrides::prefer`](crate::core::plan::Overrides::prefer)). Without a
//!    preference, the plan cannot be built
//!    ([`PlanError::Contested`](crate::core::plan::PlanError::Contested)).
//!
//! [`stages()`] returns the three stages a backend declaration takes
//! (lowering, emit, final); the `text` feature
//! ([`TextBackend`](super::TextBackend)) registers the pieces and regions
//! stages separately.

use std::sync::Arc;

use techy::error::Diagnostic;

use crate::model::addr::TreeId;
use crate::core::artifact::{Artifact, ArtifactId, ArtifactSet};
use crate::core::region::{CONTENT, Region, Regions};
use crate::model::names::BackendName;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::core::data::store::StageOut;
use crate::core::data::table::TableName;
use crate::lang::{nowhere, FlmOrigin};
use crate::stdcore::backends::lowering::lowered_table;
use crate::stdcore::backends::pieces::{LoweredPieces, lowered_pieces_table};
use crate::stdcore::backends::pres::{self, I48F16, Length, LengthUnit};

use super::emit::{NOWHERE_LABEL, TEXT_OUTPUT, TextOutput, emit};

/// The backend's name, `text`.
pub const BACKEND: &str = "text";
/// The `text` workflow's name, `text`: the name to give
/// [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) to render a
/// document as plain text. It is written like the backend's name, but it is
/// the name of a workflow.
pub const WORKFLOW: &str = "text";
/// The emit stage's name ([`TextEmitStage`]).
pub const EMIT_STAGE: &str = "flm.text.emit";
/// The regions stage's name ([`TextRegionsStage`]).
pub const REGIONS_STAGE: &str = "flm.text.regions";
/// The final stage's name ([`TextFinalStage`]).
pub const FINAL_STAGE: &str = "flm.text.final";
/// The owner part of the entry artifact's identifier, `text:document`.
pub const ARTIFACT_OWNER: &str = "text";
/// The name part of the entry artifact's identifier, `text:document`.
pub const ARTIFACT_NAME: &str = "document";
/// The entry artifact's media type.
pub const MEDIA_TYPE: &str = "text/plain";
/// The file extension, `txt`, that a host writing the entry artifact to a
/// file should use.
pub const EXTENSION: &str = "txt";

/// The typed name of the text backend's artifact-set table,
/// `flm.output.text.document`, which [`TextFinalStage`] writes. It is the
/// `text` output's table, and its name is the capability a plan demands for
/// that output.
pub const TEXT_DOCUMENT: TableName<ArtifactSet> = TableName::new("flm.output.text.document");

/// The typed name of the text backend's regions table,
/// `flm.output.text.regions`, which [`TextRegionsStage`] writes: one [output
/// region](crate::core::region::Regions), the content.
pub const TEXT_REGIONS: TableName<Regions> = TableName::new("flm.output.text.regions");

/// The text backend's render options: the line width, the width of a tab, the
/// stop after a run-in heading's title, whether links print their address,
/// and the heading level printed as a first-level heading.
///
/// The `text` feature declares them with
/// [`Bind::Render`](crate::model::options::Bind::Render), so their values come
/// from the render request rather than from the document (a caller sets them
/// in the render options of [`Overrides`](crate::core::plan::Overrides)), and
/// they are part of what identifies a plan ([`PlanKey`](crate::core::plan::PlanKey)):
/// two runs asking for different widths use two plans. As settings, the
/// fields are the settings keys of the `text` feature.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::text::TextRenderOptions;
///
/// assert_eq!(TextRenderOptions::default().width, Some(80));
/// assert_eq!(TextRenderOptions::default().tab_width, 4);
/// assert_eq!(TextRenderOptions::default().run_in_stop, ".");
/// assert!(TextRenderOptions::default().link_addresses);
/// assert!(!TextRenderOptions::default().link_addresses_when_shown);
/// assert_eq!(TextRenderOptions::default().root_heading_level, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TextRenderOptions {
    /// The line width in columns; `None` never wraps. The default is 80.
    pub width: Option<u32>,
    /// The width of a tab in columns: how far a line indented by one
    /// [tab](crate::stdcore::backends::pres::LengthUnit::Tabs) (one `\indent`
    /// in a lines section of a quotation) is indented. The default is 4. Every
    /// length, in any unit, is converted to columns by
    /// [`columns`](Self::columns).
    pub tab_width: u32,
    /// The stop written after the title of a run-in heading (a heading set at
    /// the start of the paragraph it introduces), so that `\paragraph{Note} Some
    /// content.` is printed as `Note. Some content.` The default is
    /// [`DEFAULT_RUN_IN_STOP`](crate::stdcore::backends::pres::DEFAULT_RUN_IN_STOP),
    /// a period; the empty text writes no stop.
    ///
    /// The stop is written only when the title does not already end in
    /// sentence punctuation
    /// ([`ends_in_sentence_punctuation`](crate::stdcore::backends::pres::ends_in_sentence_punctuation)).
    /// That test looks only at the last characters of the title's text, so a
    /// title ending in a formula, or in a symbol drawn by an author's own
    /// command, gets the stop after it. A program that needs another treatment
    /// of headings registers a lowering rule or a stage of its own ([the
    /// extension guide](crate::guide::extending#lowering-rules)).
    pub run_in_stop: String,
    /// Whether a link to an address outside the document prints that address
    /// after its content, between angle brackets: `\href{https://example.com}{Click
    /// here}` prints `Click here <https://example.com>`. The default is `true`.
    /// Off, every link prints its content alone, which keeps a text holding
    /// many links readable. A link to a place inside the document prints its
    /// content alone either way.
    pub link_addresses: bool,
    /// Whether a link whose content already shows its address also prints the
    /// address after it. A `\url` or an `\email` is such a link: its content
    /// is derived from the address by the lowering rule that produced the
    /// link, which sets
    /// [`LinkPayload::shows_target`](crate::stdcore::backends::pres::LinkPayload::shows_target).
    /// With this option off (the default), `\url{https://example.com}` prints
    /// `example.com`; on, it prints `example.com <https://example.com>`. It
    /// has no effect when [`link_addresses`](Self::link_addresses) is off.
    pub link_addresses_when_shown: bool,
    /// The heading level the text output writes as its first-level heading.
    /// A heading's level is a number counted from zero, smaller for an outer
    /// heading
    /// ([`HeadingPayload::level`](crate::stdcore::backends::pres::HeadingPayload::level)).
    /// A heading at this level is underlined with `=`, a heading one level
    /// deeper with `-`, and every deeper heading is preceded by `#`
    /// characters, one
    /// more than its depth below this level as
    /// [`heading_depth`](crate::stdcore::backends::pres::heading_depth) counts
    /// it from 0: three for a heading two levels below. The default is
    /// [`DEFAULT_ROOT_HEADING_LEVEL`](crate::stdcore::backends::pres::DEFAULT_ROOT_HEADING_LEVEL),
    /// 1, the level of a section, the same as for the HTML backend's option of
    /// the same name
    /// ([`HtmlRenderOptions::root_heading_level`](crate::stdcore::backends::html::HtmlRenderOptions::root_heading_level)).
    ///
    /// A heading whose level is less than this one is reported as
    /// [`flm.pres.heading-above-root`](crate::stdcore::backends::pres::HeadingAboveRoot)
    /// and written as a first-level heading. The option affects only the
    /// underline or the `#` characters of a block heading: the numbering and the
    /// run-in headings use a heading's own level.
    pub root_heading_level: u8,
}

impl Default for TextRenderOptions {
    fn default() -> TextRenderOptions {
        TextRenderOptions {
            width: Some(80),
            tab_width: 4,
            run_in_stop: String::from(pres::DEFAULT_RUN_IN_STOP),
            link_addresses: true,
            link_addresses_when_shown: false,
            root_heading_level: pres::DEFAULT_ROOT_HEADING_LEVEL,
        }
    }
}

impl TextRenderOptions {
    /// Returns the number of columns the length `length` spans in the text
    /// output. The emitter converts every [`Length`] to columns with this
    /// function, and with no other.
    ///
    /// A tab spans [`tab_width`](Self::tab_width) columns and an em two. The
    /// absolute units are converted at twelve points to the em, so a point
    /// spans a sixth of a column, an inch (72 points) twelve columns, and a
    /// millimeter and a centimeter what 72/25.4 and 720/25.4 points span. The
    /// result is rounded to the nearest whole column, a half rounded up. A
    /// length of zero or less spans no column, since plain text cannot be
    /// indented leftward, and a length too large for a `u32` spans
    /// [`u32::MAX`] columns.
    ///
    /// ```
    /// use flm_core::stdcore::backends::pres::Length;
    /// use flm_core::stdcore::backends::text::TextRenderOptions;
    ///
    /// let options = TextRenderOptions::default();
    /// assert_eq!(options.columns(&Length::tabs(2)), 8);
    /// assert_eq!(options.columns(&Length::parse("1.5em")?), 3);
    /// assert_eq!(options.columns(&Length::parse("1in")?), 12);
    /// # Ok::<(), flm_core::stdcore::backends::pres::LengthError>(())
    /// ```
    pub fn columns(&self, length: &Length) -> u32 {
        // Columns per unit, as a fraction: two per em, twelve points per em.
        let (numerator, denominator): (i128, i128) = match length.unit {
            LengthUnit::Tabs => (i128::from(self.tab_width), 1),
            LengthUnit::Ems => (2, 1),
            LengthUnit::Points => (1, 6),
            LengthUnit::Inches => (12, 1),
            LengthUnit::Millimeters => (120, 254),
            LengthUnit::Centimeters => (1200, 254),
        };
        // The amount is a count of scaled points, 1/65536 of the unit.
        let scale = denominator << I48F16::FRAC_NBITS;
        let scaled = i128::from(length.amount.to_bits()) * numerator;
        if scaled <= 0 {
            return 0;
        }
        let rounded = (2 * scaled + scale) / (2 * scale);
        u32::try_from(rounded).unwrap_or(u32::MAX)
    }
}

/// Condition `flm.text.stage-failed`: a stage of the text backend could not
/// complete. A [condition](crate::guide::concepts_overview#diagnostics) is the
/// typed body of a diagnostic.
///
/// A stage reports it, at error severity, when it cannot read or write a
/// table or an option it declared (the plan it runs in does not match its
/// declarations), and the final stage reports it when its entry artifact
/// cannot be added to the artifact set
/// ([`ArtifactSet::insert_entry_with`](crate::core::artifact::ArtifactSet::insert_entry_with)
/// returns an error); the other problems of the set are reported as
/// conditions of their own.
/// Recovery: the stage writes no output.
#[derive(Debug, Clone, PartialEq, Eq, techy::error::DiagnosticInfo)]
#[diagnostic(
    id = "flm.text.stage-failed",
    message = "the stage `{stage}` failed: {detail}"
)]
pub struct StageFailed {
    /// The stage's name.
    pub stage: String,
    /// The error, as text.
    pub detail: String,
}

fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: String::from(stage), detail: detail.to_string() },
        nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
    ));
}

/// The emit stage, `flm.text.emit`: it prints the backend's lowered pieces and
/// trees into a [`TextOutput`] with [`emit`](super::emit()) (see the module
/// documentation).
///
/// It reads the lowered-trees table `flm.lowered.text`, the lowered-pieces
/// table `flm.lowered.text.pieces` (a plan in which no stage wrote it prints
/// no piece) and the render options [`TextRenderOptions`], and writes
/// `flm.output.text` ([`TEXT_OUTPUT`]). When the lowered trees are absent,
/// the stage writes nothing. When the text cannot be printed at all, the
/// table is left absent and the reason is reported as a diagnostic.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextEmitStage;

impl Stage for TextEmitStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(EMIT_STAGE, Effect::Compute, Scope::Document)
            .writes(TEXT_OUTPUT)
            .docs(
                "prints the text backend's lowered trees as one text, with a position mark for \
                 each printed presentation node, into `flm.output.text`",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        let backend = BackendName::from(BACKEND);
        Needs::new()
            .reads(lowered_table(&backend))
            .reads(lowered_pieces_table(&backend))
            .option::<TextRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let trees = match cx.get(lowered_table(&BackendName::from(BACKEND))) {
            Ok(Some(trees)) => trees,
            Ok(None) => return,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        // A plan in which the pieces stage wrote nothing is the same output
        // with no piece placed.
        let none = LoweredPieces::default();
        let pieces = match cx.get(lowered_pieces_table(&BackendName::from(BACKEND))) {
            Ok(Some(pieces)) => pieces,
            Ok(None) => &none,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let options = match cx.option::<TextRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let order: Vec<TreeId> = cx.trees().map(|(id, _)| id).collect();
        let emitted = emit(trees, &order, pieces, options);
        for diagnostic in emitted.diags {
            out.diags.push(diagnostic);
        }
        // No emission, no table: the output is absent beside its diagnostic.
        let Some(output) = emitted.output else { return };
        match out.table(TEXT_OUTPUT) {
            Ok(table) => *table = output,
            Err(error) => stage_failed(out, EMIT_STAGE, error),
        }
    }
}

/// The regions stage, `flm.text.regions`: it writes the printed text of
/// `flm.output.text` as the output's one [output
/// region](crate::core::region::Regions), [`CONTENT`], into [`TEXT_REGIONS`],
/// with the entry artifact's identity `text:document` and media type
/// `text/plain`. It writes nothing when `flm.output.text` is absent.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextRegionsStage;

impl Stage for TextRegionsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(REGIONS_STAGE, Effect::Compute, Scope::Document)
            .writes(TEXT_REGIONS)
            .docs("writes the printed text as the output's one region into `flm.output.text.regions`")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(TEXT_OUTPUT)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let output: &TextOutput = match cx.get(TEXT_OUTPUT) {
            Ok(Some(output)) => output,
            Ok(None) => return,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let mut regions =
            Regions::new(ArtifactId::new(ARTIFACT_OWNER, ARTIFACT_NAME), MEDIA_TYPE);
        regions.regions.push(Region::new(CONTENT, vec![output.text.clone()]));
        match out.table(TEXT_REGIONS) {
            Ok(table) => *table = regions,
            Err(error) => stage_failed(out, REGIONS_STAGE, error),
        }
    }
}

/// The final stage, `flm.text.final`: the backend's [final
/// stage](crate::guide::concepts_overview#output-regions-and-the-final-stage),
/// which writes the content region of [`TEXT_REGIONS`] as an artifact set
/// with one entry artifact into [`TEXT_DOCUMENT`].
///
/// The text output is one file, but it is published as an artifact set
/// ([`ArtifactSet`]) like every backend's output, so that a host reads the
/// result of every backend the same way, whether it is one file or several.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextFinalStage;

impl Stage for TextFinalStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(FINAL_STAGE, Effect::Compute, Scope::Document)
            .writes(TEXT_DOCUMENT)
            .docs("writes the content region as the one-artifact set `flm.output.text.document`")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(TEXT_REGIONS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let regions = match cx.get(TEXT_REGIONS) {
            Ok(Some(regions)) => regions,
            Ok(None) => return,
            Err(error) => return stage_failed(out, FINAL_STAGE, error),
        };
        let text = regions.get(CONTENT).map(|region| region.text()).unwrap_or_default();
        let mut entry =
            Artifact::bytes(regions.entry.clone(), regions.media_type.clone(), text.into_bytes())
                .as_entry();
        entry.refs.extend(regions.refs.iter().cloned());
        match out.table(TEXT_DOCUMENT) {
            Ok(set) => match set.insert_entry_with(entry, &[]) {
                Ok(reports) => reports.into_iter().for_each(|report| out.diags.push(report)),
                Err(violation) => stage_failed(out, FINAL_STAGE, violation),
            },
            Err(error) => stage_failed(out, FINAL_STAGE, error),
        }
    }
}

/// Returns the backend's three stages as shared values, in the order a backend
/// declaration ([`BackendDef::new`](crate::core::registry::BackendDef::new))
/// takes them: the lowering stage
/// ([`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage) for
/// `text`), the emit stage ([`TextEmitStage`]) and the final stage
/// ([`TextFinalStage`]).
pub fn stages() -> (Arc<dyn Stage>, Arc<dyn Stage>, Arc<dyn Stage>) {
    (
        Arc::new(crate::stdcore::backends::lowering::LoweringStage::new(BACKEND)),
        Arc::new(TextEmitStage),
        Arc::new(TextFinalStage),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(options: &TextRenderOptions, text: &str) -> u32 {
        options.columns(&Length::parse(text).expect("a length"))
    }

    #[test]
    fn every_unit_converts_to_whole_columns() {
        let options = TextRenderOptions::default();
        // A tab at the option's width, an em as two columns.
        assert_eq!(columns(&options, "1tab"), 4);
        assert_eq!(columns(&options, "2.5tab"), 10);
        assert_eq!(columns(&options, "1em"), 2);
        assert_eq!(columns(&options, "1.25em"), 3, "2.5 columns, a half rounded up");
        // Twelve points to the em: six points to a column.
        assert_eq!(columns(&options, "12pt"), 2);
        assert_eq!(columns(&options, "10pt"), 2, "1.67 columns");
        assert_eq!(columns(&options, "2pt"), 0, "0.33 of a column");
        assert_eq!(columns(&options, "3pt"), 1, "half a column, rounded up");
        // An inch is 72 points; a millimeter 72/25.4 of a point.
        assert_eq!(columns(&options, "1in"), 12);
        assert_eq!(columns(&options, "0.25in"), 3);
        assert_eq!(columns(&options, "25.4mm"), 12);
        assert_eq!(columns(&options, "3mm"), 1, "1.42 columns");
        assert_eq!(columns(&options, "2.54cm"), 12);
        assert_eq!(columns(&options, "1cm"), 5, "4.72 columns");
    }

    #[test]
    fn the_tab_width_sets_the_tab_alone() {
        let options = TextRenderOptions { tab_width: 2, ..TextRenderOptions::default() };
        assert_eq!(columns(&options, "1tab"), 2);
        assert_eq!(columns(&options, "3tab"), 6);
        assert_eq!(columns(&options, "1em"), 2);
        assert_eq!(columns(&TextRenderOptions { tab_width: 0, ..options }, "5tab"), 0);
    }

    #[test]
    fn a_zero_or_negative_length_spans_no_column_and_a_huge_one_saturates() {
        let options = TextRenderOptions::default();
        assert_eq!(columns(&options, "0tab"), 0);
        assert_eq!(columns(&options, "-3em"), 0);
        let huge = Length::new(I48F16::MAX, LengthUnit::Tabs);
        let wide = TextRenderOptions { tab_width: u32::MAX, ..TextRenderOptions::default() };
        assert_eq!(wide.columns(&huge), u32::MAX);
    }
}
