//! The render stage `math-techxt.render` and its table
//! `math-techxt.rendered`.
//!
//! The stage ([`Render`]) walks the tree of its call (a document's tree, or a
//! synthesized value) and renders every formula it holds, nested ones
//! included, with the feature's converter: every math group, and every
//! display-math environment one equation line at a time. It writes one row per
//! math group or environment into the table [`RENDERED_TABLE`] ([`Row`]),
//! keyed by the node's address; an environment's row holds one formula per
//! equation line.
//!
//! The stage is in a plan only where a selected rule reads its table, which
//! the rules of the text and HTML outputs do; it provides no capability. Its
//! diagnostics, techxt's among them, are therefore reported on those runs only.
//! The check that runs on every workflow is the stage
//! [`CHECK`](crate::math_techxt::check::CHECK), which reports the empty equation lines.
//!
//! # The TeX techxt reads
//!
//! The TeX of a math group, or of an equation line, is written by the `math`
//! feature's writer (`group_tex`, `line_tex`), each text command a *hole*.
//! Hole `i` is written as `\text{<marker>}`, whatever text command the author
//! wrote, around the marker `U+E000 + i`, one of the private-use characters of
//! the Basic Multilingual Plane (`U+E000` to `U+F8FF`); the text command's own
//! content is lowered by the rules, never by techxt. The TeX is given to the
//! converter as a formula of its own: `\(…\)` for an inline math group, `\[…\]`
//! for a displayed one and for each equation line. A formula reads no
//! definition from another: under the converter's parse language a defining
//! command defines nothing.
//!
//! # The rendered lines
//!
//! techxt writes an inline formula as one line and a displayed one as lines
//! indented by four columns (a matrix or an alignment drawn over several
//! lines); its text ends with one newline. The final newline and the common
//! indentation of a displayed formula are removed, and each line is split at
//! the markers into [`Segment`]s, a text or the index of a hole.
//!
//! # The diagnostics
//!
//! - [`CONVERSION`]: a condition techxt reports while it renders a formula, at
//!   the formula (for an equation line, at the first node of mathematics of
//!   the line's content), with techxt's severity; techxt's message is in the message, its
//!   identifier in the data (`condition`).
//! - [`CONVERSION_FAILED`] (an Error): techxt cannot read the formula at all;
//!   the formula gets no row.
//! - [`PRIVATE_USE_CHARACTER`] (an Error): the formula's own TeX holds a
//!   private-use character of the markers' range, which no marker could be
//!   told from; the formula gets no row.
//! - [`TOO_MANY_TEXT_COMMANDS`] (an Error): a formula, or an equation line,
//!   holds more text commands than there are markers; the formula gets no row.
//!
//! The diagnostics of an equation line other than techxt's are placed as
//! techxt's are; a line that gets no rendering leaves the whole environment
//! without a row. An equation line that holds no mathematics is rendered as it
//! is (the check reports it).

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::lowering::NodeAddr;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::stdlib::features::math::{
    ENVIRONMENT, MATH, RENDERED, TexPart, equation_lines, formula_display, group_tex, line_tex,
};
use flm_ext::guest::techy::error::{Diagnostic, Severity};
use flm_ext::guest::techy::node::Node;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use techxt::convert::{Latexlike, MathMode};
use techxt::def::DefinitionSet;
use techxt::{Converter, ConverterBuilder};

use crate::math_techxt::check::is_mathematics;
use crate::math_techxt::rules::{Display, Segment};

/// The name of the stage, `math-techxt.render`.
pub const RENDER_STAGE: &str = "math-techxt.render";

/// The table of the rendered formulas, `math-techxt.rendered`: one [`Row`] per
/// math group or display-math environment, keyed by the node's address; an
/// environment's row holds one formula per equation line. Its capability
/// refines `flm.math.rendered`.
pub const RENDERED_TABLE: &str = "math-techxt.rendered";

/// The diagnostic `math-techxt.conversion`: a condition techxt reports while it
/// renders a formula, at techxt's severity.
pub const CONVERSION: &str = "math-techxt.conversion";

/// The diagnostic `math-techxt.conversion-failed` (an Error): techxt cannot
/// read a formula.
pub const CONVERSION_FAILED: &str = "math-techxt.conversion-failed";

/// The diagnostic `math-techxt.private-use-character` (an Error): a formula's
/// own TeX holds a private-use character of the markers' range.
pub const PRIVATE_USE_CHARACTER: &str = "math-techxt.private-use-character";

/// The diagnostic `math-techxt.too-many-text-commands` (an Error): a formula,
/// or an equation line, holds more text commands than there are markers.
pub const TOO_MANY_TEXT_COMMANDS: &str = "math-techxt.too-many-text-commands";

/// The capabilities of the expansion of user definitions, after which the
/// stage runs when the plan holds that expansion, so that techxt reads the
/// expanded formulas.
const EXPANDED: &str = "flm.userdefs.expanded";

/// See [`EXPANDED`].
const ALWAYS_EXPANDED: &str = "flm.userdefs.always-expanded";

/// The first private-use character, `U+E000`: the marker of hole 0.
pub const FIRST_MARKER: char = '\u{E000}';

/// The last private-use character of the Basic Multilingual Plane, `U+F8FF`:
/// the marker of the last hole a formula can hold.
pub const LAST_MARKER: char = '\u{F8FF}';

/// The number of markers, the most text commands a formula, or an equation
/// line, can hold: 6400.
const MARKERS: usize = (LAST_MARKER as usize) - (FIRST_MARKER as usize) + 1;

/// The indentation techxt writes before every line of a displayed formula.
const DISPLAY_INDENT: &str = "    ";

/// Builds the feature's converter over `set`: techxt's `Fancy` mode (Unicode
/// symbols, scripts and alphabets with TeX's spacing), no wrap width, every
/// other option at its default.
///
/// # Errors
///
/// techxt refuses the definitions ([`ExtError::Handler`]).
pub fn converter(set: DefinitionSet<Latexlike>) -> Result<Converter<Latexlike>, ExtError> {
    ConverterBuilder::<Latexlike>::with_definitions(set).math_mode(MathMode::Fancy).wrap_width(None).build().map_err(
        |error| ExtError::Handler { detail: format!("Failed to build techxt's converter: {error}"), data: Value::Null },
    )
}

/// Declares the stage and its table.
///
/// # Errors
///
/// The host's refusal.
pub fn register_render(b: &mut FeatureBuilder, converter: &Converter<Latexlike>) -> Result<(), ExtError> {
    b.table_refining(
        RENDERED_TABLE,
        RENDERED,
        "The formulas rendered as Unicode text by techxt, one row per math group or display-math environment, \
         keyed by its address: a math group's formula, or one formula per equation line of an environment, each \
         formula's lines split at the places of its text commands.",
    )?;
    b.stage(Arc::new(Render { converter: converter.clone() }))
}

/// A row of the table [`RENDERED_TABLE`]: one formula node, rendered, keyed
/// by its address. What the row holds beside the address is a
/// [`RenderedNode`], a math group's or a display-math environment's, written
/// in JSON with its kind under `kind` (`group` or `environment`) beside the
/// address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// The formula node's address, by which the rules find the row.
    pub addr: NodeAddr,
    /// The formula node, rendered.
    #[serde(flatten)]
    pub rendered: RenderedNode,
}

/// A formula node rendered: a math group or a display-math environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RenderedNode {
    /// A math group: whether it is inline or displayed, and its formula.
    Group {
        /// Whether the formula is inline or displayed.
        display: Display,
        /// The formula, rendered.
        formula: RenderedFormula,
    },
    /// A display-math environment, displayed: its name and one formula per
    /// equation line, in order.
    Environment {
        /// The environment's name.
        environment: String,
        /// The equation lines, each rendered as a formula.
        lines: Vec<RenderedFormula>,
    },
}

/// A math group or an equation line, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedFormula {
    /// The name of each hole's text command (`text`, `mbox`, …), in order: the
    /// hole of index `i` is the `i`-th text command the writer answers for the
    /// formula.
    pub holes: Vec<String>,
    /// The rendered lines, each a list of segments.
    pub lines: Vec<Vec<Segment>>,
}

/// Why the TeX of a formula cannot be written for techxt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The formula's own TeX holds this private-use character of the markers'
    /// range.
    PrivateUse(char),
    /// The formula holds more text commands than there are markers.
    TooManyHoles,
}

impl Refusal {
    /// The diagnostic of the refusal, an Error.
    fn diagnostic(self) -> Diagnostic {
        match self {
            Refusal::PrivateUse(c) => Diagnostic::error(
                PRIVATE_USE_CHARACTER,
                format!(
                    "Invalid character U+{:04X} in a formula (private-use characters U+{:04X} to U+{:04X} may not \
                     be used in mathematics)",
                    u32::from(c),
                    u32::from(FIRST_MARKER),
                    u32::from(LAST_MARKER)
                ),
            )
            .with_data(json!({ "character": c.to_string() })),
            Refusal::TooManyHoles => Diagnostic::error(
                TOO_MANY_TEXT_COMMANDS,
                format!("Too many text commands such as ‘\\text’ in a formula (at most {MARKERS})"),
            ),
        }
    }
}

/// A piece of a formula's TeX: TeX text, or a hole written as the command
/// named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece<'a> {
    /// TeX text, written as it is.
    Tex(&'a str),
    /// A hole for the text command of this name.
    Hole(&'a str),
}

/// The pieces of the writer's parts `parts`.
fn pieces(parts: &[TexPart]) -> impl Iterator<Item = Piece<'_>> {
    parts.iter().map(|part| match part {
        TexPart::Tex(text) => Piece::Tex(text),
        TexPart::Hole(hole) => Piece::Hole(&hole.command),
    })
}

/// The TeX of a formula given to techxt, with its holes' commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaTex {
    /// The formula, delimiters included.
    pub tex: String,
    /// The name of each hole's text command, in order.
    pub holes: Vec<String>,
}

/// The TeX of the formula made of `pieces`, displayed or not, as techxt reads
/// it: between `\(…\)` or `\[…\]`, hole `i` written as `\text{<U+E000 + i>}`.
/// (An inline formula is not written between `$…$`: an empty one, `$$`, would
/// open a displayed formula.)
///
/// # Errors
///
/// A TeX piece holds a private-use character of the markers' range, or there
/// are more holes than markers.
pub fn formula_tex<'a>(pieces: impl IntoIterator<Item = Piece<'a>>, display: bool) -> Result<FormulaTex, Refusal> {
    let mut tex = String::from(if display { r"\[" } else { r"\(" });
    let mut holes = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Tex(text) => {
                if let Some(c) = text.chars().find(|c| (FIRST_MARKER..=LAST_MARKER).contains(c)) {
                    return Err(Refusal::PrivateUse(c));
                }
                tex.push_str(text);
            }
            Piece::Hole(command) => {
                let marker = marker(holes.len()).ok_or(Refusal::TooManyHoles)?;
                tex.push_str(r"\text{");
                tex.push(marker);
                tex.push('}');
                holes.push(command.to_string());
            }
        }
    }
    tex.push_str(if display { r"\]" } else { r"\)" });
    Ok(FormulaTex { tex, holes })
}

/// The marker of hole `index`, the private-use character `U+E000 + index`, if
/// there is one.
pub fn marker(index: usize) -> Option<char> {
    let code = u32::try_from(index).ok()?.checked_add(u32::from(FIRST_MARKER))?;
    char::from_u32(code).filter(|c| *c <= LAST_MARKER)
}

/// The lines of `text`, a formula rendered by techxt: the final newline
/// removed and, for a displayed formula, the four columns of indentation that
/// every line holds; each line split at the markers of the formula's `holes`
/// holes into segments. An empty text has no line.
pub fn rendered_lines(text: &str, display: bool, holes: usize) -> Vec<Vec<Segment>> {
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    let indented = display && text.split('\n').all(|line| line.is_empty() || line.starts_with(DISPLAY_INDENT));
    let hole_of = |c: char| {
        let index = usize::try_from(u32::from(c).checked_sub(u32::from(FIRST_MARKER))?).ok()?;
        (index < holes).then_some(index)
    };
    text.split('\n')
        .map(|line| {
            let line = if indented { line.strip_prefix(DISPLAY_INDENT).unwrap_or(line) } else { line };
            let mut segments = Vec::new();
            let mut run = String::new();
            for c in line.chars() {
                match hole_of(c) {
                    Some(index) => {
                        if !run.is_empty() {
                            segments.push(Segment::Text(std::mem::take(&mut run)));
                        }
                        segments.push(Segment::Hole(index));
                    }
                    None => run.push(c),
                }
            }
            if !run.is_empty() {
                segments.push(Segment::Text(run));
            }
            segments
        })
        .collect()
}

/// A formula rendered by techxt, with what techxt reported.
#[derive(Debug)]
pub struct Rendering {
    /// The formula, rendered, or `None` when techxt cannot read it.
    pub formula: Option<RenderedFormula>,
    /// techxt's diagnostics, as the extension reports them (not yet placed):
    /// [`CONVERSION_FAILED`] alone when techxt cannot read the formula, else
    /// one [`CONVERSION`] per condition techxt reported.
    pub diagnostics: Vec<Diagnostic>,
}

/// Renders `tex` with `converter`, displayed or not.
pub fn render(converter: &Converter<Latexlike>, tex: FormulaTex, display: bool) -> Rendering {
    let conversion = match converter.latex_to_text(&tex.tex) {
        Ok(conversion) => conversion,
        Err(error) => {
            let diagnostic = Diagnostic::error(
                CONVERSION_FAILED,
                format!("Failed to render the formula as text: {}", error.message()),
            )
            .with_data(json!({ "condition": error.identifier() }));
            return Rendering { formula: None, diagnostics: vec![diagnostic] };
        }
    };
    let diagnostics = conversion
        .diagnostics
        .iter()
        .map(|diagnostic| {
            Diagnostic::new(
                severity(diagnostic.severity()),
                CONVERSION,
                format!("Problem rendering the formula as text: {}", diagnostic.message()),
            )
            .with_data(json!({ "condition": diagnostic.identifier() }))
        })
        .collect();
    let lines = rendered_lines(&conversion.text, display, tex.holes.len());
    Rendering { formula: Some(RenderedFormula { holes: tex.holes, lines }), diagnostics }
}

/// The extension's severity of techxt's severity `severity`.
fn severity(severity: techxt::techy::error::Severity) -> Severity {
    match severity {
        techxt::techy::error::Severity::Note => Severity::Note,
        techxt::techy::error::Severity::Warning => Severity::Warning,
        techxt::techy::error::Severity::Error => Severity::Error,
    }
}

/// The stage [`RENDER_STAGE`]. See the module documentation.
struct Render {
    /// The feature's converter.
    converter: Converter<Latexlike>,
}

impl Stage for Render {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RENDER_STAGE, Effect::Compute, Scope::PerTree).writes(RENDERED_TABLE).docs(
            "Renders every formula of the tree as Unicode text with techxt, writes it into \
             `math-techxt.rendered`, and reports what techxt reports.",
        )
    }

    fn needs(&self) -> Needs {
        Needs::new().after(ALWAYS_EXPANDED).after(EXPANDED)
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let Some(tree) = cx.my_tree()? else { return Ok(()) };
        for node in tree.root()?.descendants()?.elements([MATH, ENVIRONMENT])? {
            let node = node?;
            let rendered = if node.element_name()? == MATH { self.group(&node, out)? } else { self.environment(&node, out)? };
            let (Some(rendered), Some(addr)) = (rendered, cx.addr_of(&node)?) else { continue };
            let row = Row { addr, rendered };
            let item = serde_json::to_value(&row)
                .map_err(|error| ExtError::Value { detail: format!("Failed to serialize a row of the table ‘{RENDERED_TABLE}’: {error}") })?;
            out.table(RENDERED_TABLE)?.push(item)?;
        }
        Ok(())
    }
}

impl Render {
    /// The math group `node` rendered, or `None` when it gets no row; its
    /// diagnostics reported into `out`.
    fn group(&self, node: &Node, out: &mut StageOut) -> Result<Option<RenderedNode>, ExtError> {
        let display = formula_display(node)?;
        let parts = group_tex(node)?;
        let Some(formula) = self.formula(pieces(&parts), display, node, out)? else { return Ok(None) };
        let display = if display { Display::Display } else { Display::Inline };
        Ok(Some(RenderedNode::Group { display, formula }))
    }

    /// The display-math environment `node` rendered, or `None` when it gets no
    /// row; its diagnostics reported into `out`.
    fn environment(&self, node: &Node, out: &mut StageOut) -> Result<Option<RenderedNode>, ExtError> {
        let name = node.name()?.unwrap_or_default();
        let mut lines = Vec::new();
        let mut complete = true;
        let mut after_line_end = false;
        for line in &equation_lines(node)? {
            let mut mathematics = None;
            let content_nodes = line.content.to_vec()?;
            for content in &content_nodes {
                if is_mathematics(content)? {
                    mathematics = Some(content);
                    break;
                }
            }
            let at = mathematics.or(content_nodes.first()).unwrap_or(node);
            let tex = line_tex(&line.content, after_line_end)?;
            after_line_end = tex.ends_line();
            match self.formula(pieces(&tex.parts), true, at, out)? {
                Some(formula) => lines.push(formula),
                None => complete = false,
            }
        }
        Ok(complete.then_some(RenderedNode::Environment { environment: name, lines }))
    }

    /// The formula made of `pieces`, displayed or not, rendered, or `None` when
    /// its TeX cannot be written or techxt cannot read it; its diagnostics
    /// reported into `out` at `at`.
    fn formula<'a>(
        &self,
        pieces: impl IntoIterator<Item = Piece<'a>>,
        display: bool,
        at: &Node,
        out: &mut StageOut,
    ) -> Result<Option<RenderedFormula>, ExtError> {
        let tex = match formula_tex(pieces, display) {
            Ok(tex) => tex,
            Err(refusal) => {
                out.diags(refusal.diagnostic().at(at))?;
                return Ok(None);
            }
        };
        let rendering = render(&self.converter, tex, display);
        for diagnostic in rendering.diagnostics {
            out.diags(diagnostic.at(at))?;
        }
        Ok(rendering.formula)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math_techxt::vocabulary::definitions;

    fn converter() -> Converter<Latexlike> {
        super::converter(definitions()).expect("the converter builds")
    }

    /// The lines of the formula made of `pieces`, rendered, and the
    /// identifiers of techxt's diagnostics.
    fn lines_of(pieces: &[Piece<'_>], display: bool) -> (Vec<Vec<Segment>>, Vec<String>) {
        let tex = formula_tex(pieces.iter().copied(), display).expect("the TeX is written");
        let rendering = render(&converter(), tex, display);
        let ids = rendering.diagnostics.iter().map(|d| d.data["condition"].as_str().unwrap_or_default().to_string());
        (rendering.formula.expect("techxt reads the formula").lines, ids.collect())
    }

    fn text(text: &str) -> Segment {
        Segment::Text(text.to_string())
    }

    #[test]
    fn the_tex_writes_each_hole_as_a_marker_inside_text() {
        let tex = formula_tex([Piece::Tex("a + "), Piece::Hole("mbox"), Piece::Tex(" = "), Piece::Hole("textbf")], false)
            .expect("the TeX is written");
        assert_eq!(tex.tex, "\\(a + \\text{\u{E000}} = \\text{\u{E001}}\\)");
        assert_eq!(tex.holes, ["mbox", "textbf"]);
        let tex = formula_tex([], false).expect("the TeX is written");
        assert_eq!(tex.tex, "\\(\\)");
        let tex = formula_tex([Piece::Tex("x")], true).expect("the TeX is written");
        assert_eq!(tex.tex, "\\[x\\]");
    }

    #[test]
    fn an_empty_inline_formula_is_read_as_inline() {
        let (lines, ids) = lines_of(&[], false);
        assert_eq!(lines, Vec::<Vec<Segment>>::new());
        assert_eq!(ids, Vec::<String>::new());
    }

    #[test]
    fn a_marker_is_carried_through_an_inline_formula() {
        let (lines, ids) = lines_of(&[Piece::Tex(r"\alpha + "), Piece::Hole("text"), Piece::Tex(" = b")], false);
        assert_eq!(lines, vec![vec![text("α + "), Segment::Hole(0), text(" = 𝑏")]]);
        assert_eq!(ids, Vec::<String>::new());
    }

    #[test]
    fn a_marker_is_carried_through_a_displayed_formula_whose_indentation_is_removed() {
        let (lines, ids) = lines_of(&[Piece::Tex(r"x^2 "), Piece::Hole("text"), Piece::Tex(r" \le 1")], true);
        assert_eq!(lines, vec![vec![text("𝑥² "), Segment::Hole(0), text(" ≤ 1")]]);
        assert_eq!(ids, Vec::<String>::new());
    }

    #[test]
    fn a_matrix_is_drawn_over_several_lines() {
        let (lines, ids) = lines_of(&[Piece::Tex(r"\begin{pmatrix} 1 & 2 \\ 30 & 4 \end{pmatrix}")], true);
        assert_eq!(lines, vec![vec![text("⎛  1  2 ⎞")], vec![text("⎝ 30  4 ⎠")]]);
        assert_eq!(ids, Vec::<String>::new());
    }

    #[test]
    fn an_align_line_alone_is_one_line() {
        let (lines, ids) = lines_of(&[Piece::Tex("a &= b + c")], true);
        assert_eq!(lines, vec![vec![text("𝑎 = 𝑏 + 𝑐")]]);
        assert_eq!(ids, Vec::<String>::new());
    }

    #[test]
    fn a_diagnostic_of_techxt_keeps_its_severity_and_names_its_condition() {
        let tex = formula_tex([Piece::Tex(r"\frac{a}")], false).expect("the TeX is written");
        let rendering = render(&converter(), tex, false);
        assert!(rendering.formula.is_some(), "techxt reads the formula");
        let [diagnostic] = rendering.diagnostics.as_slice() else { panic!("one diagnostic: {:?}", rendering.diagnostics) };
        assert_eq!((diagnostic.id.as_str(), diagnostic.severity), (CONVERSION, Severity::Error));
        assert!(diagnostic.data["condition"].as_str().is_some_and(|id| !id.is_empty()), "{diagnostic:?}");
    }

    #[test]
    fn a_private_use_character_of_the_source_is_refused() {
        let refusal = formula_tex([Piece::Tex("a + \u{E000}")], false);
        assert_eq!(refusal, Err(Refusal::PrivateUse('\u{E000}')));
        let refusal = formula_tex([Piece::Tex("\u{F8FF}"), Piece::Hole("text")], true);
        assert_eq!(refusal, Err(Refusal::PrivateUse('\u{F8FF}')));
        // A private-use character beyond the markers' range is not refused.
        assert!(formula_tex([Piece::Tex("\u{F0000}")], false).is_ok());
    }

    #[test]
    fn a_private_use_character_beyond_the_holes_is_text() {
        let line = format!("𝑎 + {FIRST_MARKER} = 𝑏\n");
        assert_eq!(rendered_lines(&line, false, 1), vec![vec![text("𝑎 + "), Segment::Hole(0), text(" = 𝑏")]]);
        assert_eq!(rendered_lines(&line, false, 0), vec![vec![text(&format!("𝑎 + {FIRST_MARKER} = 𝑏"))]]);
    }

    #[test]
    fn the_indentation_is_removed_only_where_every_line_holds_it() {
        let lines = rendered_lines("    ⎛ 1 ⎞\n    ⎝ 2 ⎠\n", true, 0);
        assert_eq!(lines, vec![vec![text("⎛ 1 ⎞")], vec![text("⎝ 2 ⎠")]]);
        let lines = rendered_lines("    a\n  b\n", true, 0);
        assert_eq!(lines, vec![vec![text("    a")], vec![text("  b")]]);
        // An inline formula keeps its leading spaces.
        assert_eq!(rendered_lines("    a\n", false, 0), vec![vec![text("    a")]]);
        assert_eq!(rendered_lines("", true, 0), Vec::<Vec<Segment>>::new());
    }

    #[test]
    fn the_markers_are_the_private_use_characters_of_the_first_plane() {
        assert_eq!(marker(0), Some('\u{E000}'));
        assert_eq!(marker(MARKERS - 1), Some('\u{F8FF}'));
        assert_eq!(marker(MARKERS), None);
        let pieces = std::iter::repeat_n(Piece::Hole("text"), MARKERS + 1);
        assert_eq!(formula_tex(pieces, false), Err(Refusal::TooManyHoles));
    }
}
