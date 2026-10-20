//! What the [`math`](super) feature contributes to the `latex-recomposed`
//! backend: the recomposition rule of the text commands, the names a
//! processing feature writes a display-math environment with, the options
//! that change how an environment is written, the parts and the line writer a
//! processing feature writes a formula with, and the newline rule that every
//! LaTeX writer of an environment's lines applies.
//!
//! The `latex-recomposed` backend writes the document back as LaTeX source. Its
//! rules stage (build into the lowered tree; see
//! [`Lowerer`]) construct elements, presentation
//! elements that mirror one construct of LaTeX each ([backends, lowering and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)): a
//! macro call, an environment, a raw node (`flm.pres.latex.raw`, text written
//! byte for byte), or a definition for the style file. The rendering guide
//! describes the result under [mathematics in the recomposed
//! LaTeX](crate::guide::rendering#mathematics-in-the-recomposed-latex).
//!
//! The formulas themselves (`\(…\)`, `\[…\]` and the display-math
//! environments) are written by the rules of a feature that processes
//! mathematics, for example
//! [`math-passthrough`](crate::math_passthrough), whose
//! [`latex`](crate::math_passthrough::latex) module writes
//! them as they were written. Those rules read [`MathRecomposeOptions`], and
//! write an environment as [`FLM_MATH_ENVIRONMENT`] when the options ask for it
//! and request `amsmath` for every environment but [`KERNEL_ENVIRONMENT`].
//!
//! # The LaTeX frame of a formula
//!
//! Five plain functions write what surrounds a formula's content in the
//! recomposed LaTeX, for the rules a processing feature registers on this
//! backend. They are parts called in order, never a callback: each stages raw
//! nodes (and, for a line, the constructs of its tag, labels and `\nonumber`)
//! and returns the build identifiers to emit, and the rule stages its own
//! content between them (the source as written, lowered content, or a
//! renderer's output). A sixth function, [`stage_lines`], writes the lines of
//! an environment as they were written.
//!
//! - A math group: [`group_opening`], the content, [`group_closing`]. A
//!   displayed group (`\[…\]`) is surrounded by the texts of
//!   [`surround_display_math`](MathRecomposeOptions::surround_display_math)
//!   as an environment is. An inline group is written `\(…\)`, but the
//!   formula of `\ensuremath{…}` written in text, which is written
//!   `\ensuremath{…}`.
//! - A display-math environment: [`environment_opening`]; for each equation
//!   line ([`line_frames`](super::frame::line_frames)) the line's content, then
//!   [`line_closing`]; then [`environment_closing`]. [`stage_lines`] writes
//!   every line this way, the content as written by the mathematics writer of
//!   the `userdefs` feature ([`write_math_run`]) under the newline rule (see
//!   *The newline rule* below); a rule that writes a line's content otherwise
//!   calls [`line_closing`] itself.
//!
//! The parts read [`MathRecomposeOptions`], the backend's
//! [`LatexRecomposedRenderOptions`] and the safe-labels table ([`SAFE_LABELS`])
//! through the [`Lowerer`]: the calling rule declares those reads (the two
//! option types, and the table as an optional read under a peer declaration
//! of its own feature), and declares that it produces the raw and macro
//! constructs (and, for [`stage_lines`], the definition construct, which the
//! mathematics writer stages for a user definition used on a line).
//!
//! **An environment** is written as `\begin{<name>}` … `\end{<name>}`, or,
//! when [`emit_flm_math_environment_macro`](MathRecomposeOptions::emit_flm_math_environment_macro)
//! is set, as `\flmMathEnvironment{<name>}{}{` … `}`, through the LaTeX hook
//! [`FLM_MATH_ENVIRONMENT`] defined by the definition piece
//! [`MATH_ENVIRONMENT`](crate::stdcore::backends::latex::preamble::MATH_ENVIRONMENT),
//! which the opening then requires; the six environments take no arguments, so
//! the second group is empty. The texts of
//! [`surround_display_math`](MathRecomposeOptions::surround_display_math) are
//! written byte for byte before the opening and after the closing. Every
//! environment but [`KERNEL_ENVIRONMENT`] needs the LaTeX package `amsmath`,
//! which the opening requests.
//!
//! **A line's closing** is, in order: `\tag*{…}` when the line has a tag (the
//! tag's content lowered through the rules, between `(` and `)` when the author
//! wrote `\tag` without the star, so that LaTeX adds nothing of its own); the
//! line's labels, in order, as one group in the LaTeX hook `\flmLabels`; the
//! line's `\nonumber`; and the line's `\\` with its spacing as written, when
//! another line follows. Each label is written as its safe label when the
//! safe-labels table is present, and as the document wrote it when it is
//! absent; when the table is absent and the backend's render option
//! [`drop_math_labels_without_refs`](LatexRecomposedRenderOptions::drop_math_labels_without_refs)
//! is set, nothing is written for the labels
//! ([`stage_label`], [`stage_label_group`]). No part numbers a line: the
//! recomposed LaTeX leaves the numbering to LaTeX.
//!
//! **A math group** is written between `\(` and `\)` when it is inline and
//! between `\[` and `\]` when it is displayed
//! ([`formula_display`]), whichever delimiters
//! the author used; the formula of an `\ensuremath{…}` written in text is
//! written `\ensuremath{`…`}`.
//!
//! # The newline rule
//!
//! The `\label`, `\tag` and `\nonumber` of an equation line are taken out of
//! the line's content, and a final `\\` followed only by whitespace is not
//! written ([`equation_lines`](super::lines::equation_lines)); the whitespace
//! around them stays in the content. Where such a command stood on a line of
//! its own, the whitespace before it and the whitespace after it meet, and the
//! content written as it was written would hold a blank line, which LaTeX
//! refuses inside mathematics. Every LaTeX writer of an environment's lines
//! (the recomposed LaTeX through [`stage_lines`], and the rendered LaTeX)
//! therefore writes the text of each line's content under the newline rule
//! ([`normalize_line_newlines`]):
//!
//! - Every run of spaces, tabs and newlines that holds a newline is written as
//!   one newline followed by the indentation after the run's last newline. A
//!   run that holds no newline is kept as it is.
//! - A run that ends the line's content is taken off it and written after the
//!   line's closing part (its tag, labels, `\nonumber` and `\\`), so that the
//!   closing part ends the line as written and the next line begins where the
//!   author's newline put it.
//! - A run that begins a text part of the content is dropped when what was
//!   written just before it ends a line: the run that the previous line's
//!   content ended with, written after that line's `\\`, or a comment, which
//!   ends its own line. Two newlines therefore never follow each other.
//! - When the content is written in several parts (text around a staged
//!   construct or a comment), only the last part can end the content, and
//!   anything written between two text parts other than a comment is not a
//!   line's end.
//!
//! When the content of a line ends with a comment, the line's closing part is
//! written after the comment's newline, on the next line: a comment never hides
//! the commands of its line.
//!
//! # Text commands
//!
//! [`TEXT_LATEX_RULE`] (`flm.math.text.lower.latex-recomposed`) writes a text
//! command as the LaTeX macro of the same name, over its argument lowered through
//! the ordinary rules: the emitter escapes the text, a user definition used in it
//! is written as a call, and mathematics inside it is written by the rule of the
//! feature that processes mathematics. `\text` is the only text command that the
//! LaTeX kernel does not define; the rule requests `amsmath` where it writes one.
//!
//! # Peer declarations
//!
//! The feature owns neither the backend nor the construct elements nor the
//! safe-labels table that the recomposition rules of its text commands and of
//! `\eqref` ([`eqref::LATEX_RULE`](super::eqref::LATEX_RULE)) use. [`register`]
//! declares them as peer declarations ([`PeerName`](crate::core::registry::PeerName)),
//! so that a registry without the LaTeX features still builds; the rules are
//! then set aside (excluded from rule selection). The safe-labels table is read
//! optionally, so that a registry with the backend and without the references
//! feature still renders.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::{element_of, element_value};
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexMacroPayload, LatexRawPayload, packages, preamble,
};
use crate::stdcore::backends::lowering::{lower_arg_or_nothing, lower_run};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::features::refs::{SAFE_LABELS, SafeLabels, stage_label, stage_label_group};

use crate::stdcore::features::userdefs::kept::{MathPiece, write_math_run};

use super::ensuremath::ENSUREMATH;
use super::frame::{LineFrame, formula_display};
use super::lines::{LINEBREAK_COMMAND, NONUMBER_COMMAND, TAG_COMMAND};
use super::text::{MathTextCommand, TEXT};
use super::MathEnvironment;

/// The name of the recomposition rule of the text commands of math mode
/// (`flm.math.text`), `flm.math.text.lower.latex-recomposed`, whose body is
/// [`TextLatexRule`].
pub const TEXT_LATEX_RULE: &str = "flm.math.text.lower.latex-recomposed";

/// The LaTeX hook that a display-math environment is written through when
/// [`emit_flm_math_environment_macro`](MathRecomposeOptions::emit_flm_math_environment_macro)
/// is set, `flmMathEnvironment`.
pub const FLM_MATH_ENVIRONMENT: &str = "flmMathEnvironment";

/// The only display-math environment that the LaTeX kernel defines,
/// `equation`; the other five need the LaTeX package `amsmath`.
pub const KERNEL_ENVIRONMENT: &str = "equation";

/// The only text command of math mode that the LaTeX kernel does not define,
/// `text`; the `amsmath` package defines it, and the LaTeX kernel defines
/// every other command of [`COMMANDS`](super::text::COMMANDS).
pub const AMSMATH_COMMAND: &str = "text";

/// The options of the `math` feature that say how the recomposed LaTeX writes a
/// displayed formula: an option type ([`crate::model::options`]) whose
/// values are set on a render request
/// ([`Bind::Render`](crate::model::options::Bind::Render)). The rules of a
/// feature that processes mathematics read them on the `latex-recomposed`
/// backend, through the parts of this module (see the module documentation).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::math::MathRecomposeOptions;
///
/// let options = MathRecomposeOptions::default();
/// assert_eq!(options.surround_display_math, (String::new(), String::new()));
/// assert!(!options.emit_flm_math_environment_macro);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MathRecomposeOptions {
    /// The LaTeX written, byte for byte, before and after every displayed
    /// formula: every display-math environment and every displayed math group
    /// (`\[…\]`). Both empty by default.
    pub surround_display_math: (String, String),
    /// Whether an environment is written as
    /// `\flmMathEnvironment{<name>}{<arguments>}{<body>}` rather than as
    /// `\begin{<name>}…\end{<name>}`. Off by default. The style file then
    /// holds the definition of `\flmMathEnvironment`, which a user may
    /// redefine.
    pub emit_flm_math_environment_macro: bool,
}

/// The body of [`TEXT_LATEX_RULE`]: a text command of math mode written as a
/// macro construct of the same name, over its argument lowered through the
/// ordinary rules.
///
/// The argument is document text, so it is written as text is written
/// anywhere else: the emitter escapes it, a user definition used in it is
/// written as a call with its definition for the style file, and mathematics
/// inside it is written by the rule of the feature that processes mathematics.
/// Copying the command's source
/// instead would leave out all three.
///
/// `\text` is the only command of the list that the LaTeX kernel does not
/// define, so the rule requests the package `amsmath` where it writes one
/// ([`AMSMATH_COMMAND`]).
///
/// When the argument is missing (a parse recovery left it absent and the parse
/// reported it), the macro is written with an empty argument and nothing more
/// is reported. The rule returns a [`LowerError`] when the node carries no
/// [`MathTextCommand`] or when a node could not be lowered or staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextLatexRule;

impl Lower for TextLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<MathTextCommand>(&node)
            .ok_or_else(|| LowerError::structure("a math text command without its element's data"))?;
        if value.command == AMSMATH_COMMAND {
            lw.require(&Name::from(packages::AMSMATH));
        }
        let argument = lower_arg_or_nothing(cx, lw, &node, 0)?;
        let call = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(value.command.as_str(), [LatexArgShape::Mandatory]),
            vec![(String::from(ARG_SLOT), argument)],
        )?;
        Ok(Lowering::Emit(vec![call]))
    }
}

/// The rule [`TEXT_LATEX_RULE`], ready to be registered: it targets
/// `flm.math.text`, produces the macro construct, requests the package
/// `amsmath` where it writes `\text`, and is the default on the
/// `latex-recomposed` backend only.
pub fn text_rule() -> LoweringRule {
    LoweringRule::new(
        TEXT_LATEX_RULE,
        TEXT,
        LoweringDecl::new().produces(MACRO),
        Arc::new(TextLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a text command of math mode lowers to the LaTeX macro of the same name, over its \
         argument lowered as document text",
    )
}


// --- the LaTeX frame of a formula -------------------------------------------------------

/// Stages a raw node holding `text`, written byte for byte.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// The name of the display-math environment `node`, from its element value.
fn environment_name(node: &NodeRef<'_, Flm>) -> Result<String, LowerError> {
    element_value::<MathEnvironment>(node)
        .map(|value| value.name.to_string())
        .ok_or_else(|| LowerError::structure("a math environment without its element's data"))
}

/// The opening part of a display-math environment in the recomposed LaTeX:
/// the text before every displayed formula
/// ([`surround_display_math`](MathRecomposeOptions::surround_display_math)),
/// then `\begin{<name>}`, or `\flmMathEnvironment{<name>}{}{` when the options
/// ask for the hook (see the module documentation, *The LaTeX frame of a
/// formula*). It requests `amsmath` for every environment but
/// [`KERNEL_ENVIRONMENT`], and requires the definition piece of the hook when
/// it writes it.
///
/// A processing feature's rule of `flm.math.environment` on the
/// `latex-recomposed` backend calls it first, then writes each line's content
/// followed by [`line_closing`], then calls [`environment_closing`]. Under the
/// hook, what it writes between the parts is inside a LaTeX argument, while
/// the lowered tree holds it as siblings of raw nodes and not inside a macro's
/// argument, so the emitter does not know it is there: such a rule must not
/// stage block verbatim content there directly (verbatim content inside a text
/// command's argument is written as such an argument requires).
///
/// # Errors
///
/// Returns [`LowerError`] when `node` carries no [`MathEnvironment`], when the
/// calling rule does not declare that it reads [`MathRecomposeOptions`], or
/// when a node could not be staged.
pub fn environment_opening(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
) -> Result<Vec<BuildId>, LowerError> {
    let name = environment_name(node)?;
    let options = lw.option::<MathRecomposeOptions>()?.clone();
    if name != KERNEL_ENVIRONMENT {
        lw.require(&Name::from(packages::AMSMATH));
    }
    let mut nodes = Vec::with_capacity(2);
    let (before, _) = &options.surround_display_math;
    if !before.is_empty() {
        nodes.push(raw(cx, lw, before)?);
    }
    let opening = if options.emit_flm_math_environment_macro {
        lw.require(&Name::from(preamble::MATH_ENVIRONMENT));
        format!("\\{FLM_MATH_ENVIRONMENT}{{{name}}}{{}}{{")
    } else {
        format!("\\begin{{{name}}}")
    };
    nodes.push(raw(cx, lw, &opening)?);
    Ok(nodes)
}

/// The closing part of an equation line in the recomposed LaTeX, what follows
/// the line's content: `\tag*{…}` when the line has a tag, the line's labels in
/// `\flmLabels`, `\nonumber`, and the line's `\\` when another line follows
/// (see the module documentation, *The LaTeX frame of a formula*).
///
/// A processing feature's rule of `flm.math.environment` on the
/// `latex-recomposed` backend calls it after each line's content, with the
/// line's frame ([`line_frames`](super::frame::line_frames)); only the line
/// ([`LineFrame::line`]) is read, since LaTeX numbers the lines itself.
///
/// # Errors
///
/// Returns [`LowerError`] when the calling rule does not declare that it reads
/// the safe-labels table (optionally) and [`LatexRecomposedRenderOptions`], or
/// when a node could not be lowered or staged.
pub fn line_closing(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    frame: &LineFrame<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let line = &frame.line;
    let mut pieces = Vec::new();
    if let Some(tag) = &line.tag {
        // `\tag*{…}` always, with FLM's parentheses written here where the
        // author's `\tag` asks for them, so that LaTeX adds none of its own.
        // The tag's content is inline text of the document and goes through
        // the ordinary lowering: the emitter escapes it, and a text command
        // inside it comes out as its own LaTeX.
        let mut argument = Vec::new();
        if tag.parenthesized() {
            argument.push(lw.text(cx, "(")?);
        }
        argument.extend(lower_run(cx, lw, &tag.content)?);
        if tag.parenthesized() {
            argument.push(lw.text(cx, ")")?);
        }
        pieces.push(lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(TAG_COMMAND, [LatexArgShape::Star, LatexArgShape::Mandatory]),
            vec![(String::from(ARG_SLOT), argument)],
        )?);
    }
    // The line's labels, at the line's end whatever place in the line they
    // were written at, wrapped as one group in the labels hook: on a line of
    // a display-math environment that is what makes every label past the
    // first resolve to the line's number, which `amsmath` alone does not.
    // The labels are left out only where the safe-labels table is absent and
    // the render options ask for that.
    let written = lw.get(SAFE_LABELS)?.is_some()
        || !lw.option::<LatexRecomposedRenderOptions>()?.drop_math_labels_without_refs;
    if written {
        let mut group = Vec::new();
        for label in &line.labels {
            group.push(stage_label(cx, lw, &label.text, label.label.as_ref())?);
        }
        pieces.extend(stage_label_group(cx, lw, group, false)?);
    }
    if line.nonumber {
        pieces.push(lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(NONUMBER_COMMAND, []),
            Vec::new(),
        )?);
    }
    if let Some(brk) = &line.linebreak {
        let mut text = format!("\\{LINEBREAK_COMMAND}");
        if let Some(spacing) = &brk.spacing {
            text.push('[');
            text.push_str(spacing);
            text.push(']');
        }
        pieces.push(raw(cx, lw, &text)?);
    }
    Ok(pieces)
}

/// The closing part of a display-math environment in the recomposed LaTeX:
/// `\end{<name>}`, or `}` when the options ask for the hook
/// `\flmMathEnvironment`, then the text after every displayed formula
/// ([`surround_display_math`](MathRecomposeOptions::surround_display_math)).
///
/// A processing feature's rule of `flm.math.environment` on the
/// `latex-recomposed` backend calls it last, after the last line's
/// [`line_closing`] and whatever the rule writes after it.
///
/// # Errors
///
/// Returns [`LowerError`] when `node` carries no [`MathEnvironment`], when the
/// calling rule does not declare that it reads [`MathRecomposeOptions`], or
/// when a node could not be staged.
pub fn environment_closing(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
) -> Result<Vec<BuildId>, LowerError> {
    let name = environment_name(node)?;
    let options = lw.option::<MathRecomposeOptions>()?.clone();
    let closing = if options.emit_flm_math_environment_macro {
        String::from("}")
    } else {
        format!("\\end{{{name}}}")
    };
    let mut nodes = vec![raw(cx, lw, &closing)?];
    let (_, after) = &options.surround_display_math;
    if !after.is_empty() {
        nodes.push(raw(cx, lw, after)?);
    }
    Ok(nodes)
}

// --- the newline rule ------------------------------------------------------------------------

/// One part of a line's content after the newline rule (see the module
/// documentation, *The newline rule*): what [`normalize_line_newlines`]
/// returns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NormalizedLine {
    /// The text to write: every run of whitespace that holds a newline
    /// reduced to one newline and the indentation after it, the run at the
    /// beginning dropped where the rule drops it, and the run at the end
    /// taken off where the rule takes it off.
    pub text: String,
    /// The run that ended the content, reduced in the same way, when the rule
    /// took it off, to be written after the line's closing part (its tag,
    /// labels, `\nonumber` and `\\`); empty otherwise.
    pub trailing: String,
}

/// Whether `ch` is a space, a tab or a newline: the characters of a run of
/// whitespace for the newline rule.
fn is_run_char(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n')
}

/// Applies the newline rule to one text part of a line's content (see the
/// module documentation, *The newline rule*).
///
/// - Every run of spaces, tabs and newlines that holds a newline becomes one
///   newline followed by the spaces and tabs after the run's last newline.
/// - A run that begins `content` is dropped when `after_line_end` is `true`:
///   what was written just before this part ends a line (the run the
///   previous line's content ended with, written after that line's `\\`, or
///   a comment).
/// - A run that ends `content` is taken off and returned as
///   [`trailing`](NormalizedLine::trailing), to be written after the line's
///   closing part, unless `inner` is `true` (this part is not the last of the
///   line's content, so it does not end the content).
/// - A run that both begins and ends `content` (a part made only of
///   whitespace) is treated as a run that begins it.
/// - A run that holds no newline is kept as it is.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::math::latex::{NormalizedLine, normalize_line_newlines};
///
/// let line = normalize_line_newlines("\n  a &= 1 \n   \n  ", false, false);
/// assert_eq!(line, NormalizedLine { text: String::from("\n  a &= 1"), trailing: String::from("\n  ") });
/// // After a line end, the run that begins the part is dropped.
/// assert_eq!(normalize_line_newlines("\n  b &= 2", true, false).text, "b &= 2");
/// // A part that is not the last of the line's content keeps its run in place.
/// let line = normalize_line_newlines("c\n  ", false, true);
/// assert_eq!(line, NormalizedLine { text: String::from("c\n  "), trailing: String::new() });
/// ```
pub fn normalize_line_newlines(content: &str, after_line_end: bool, inner: bool) -> NormalizedLine {
    let mut line = NormalizedLine::default();
    let mut rest = content;
    let mut at = 0;
    while !rest.is_empty() {
        let run_end = rest.find(|ch: char| !is_run_char(ch)).unwrap_or(rest.len());
        let run = &rest[..run_end];
        match run.rfind('\n') {
            None => line.text.push_str(run),
            Some(last) => {
                let normalized = &run[last..];
                if at == 0 {
                    if !after_line_end {
                        line.text.push_str(normalized);
                    }
                } else if run_end == rest.len() && !inner {
                    line.trailing.push_str(normalized);
                } else {
                    line.text.push_str(normalized);
                }
            }
        }
        rest = &rest[run_end..];
        at += run_end;
        let text_end = rest.find(is_run_char).unwrap_or(rest.len());
        line.text.push_str(&rest[..text_end]);
        rest = &rest[text_end..];
        at += text_end;
    }
    line
}

/// Whether the output ends a line once `text` is written, written as it is
/// after output that ended a line when `after_line_end`: only spaces and tabs
/// follow the last newline of `text`, or `text` holds no newline and only
/// spaces and tabs while the output before it ended a line.
fn ends_line(text: &str, after_line_end: bool) -> bool {
    let only_blanks = |text: &str| text.chars().all(|ch| matches!(ch, ' ' | '\t'));
    match text.rsplit_once('\n') {
        Some((_, last)) => only_blanks(last),
        None => after_line_end && only_blanks(text),
    }
}

/// The newline rule applied across the lines of one environment, piece by
/// piece, in the order a writer writes them: it keeps whether the output
/// written so far ends a line (see the module documentation, *The newline
/// rule*). The LaTeX writers of `flm-core` use it: [`stage_lines`], and the
/// rule of the `math-passthrough` feature that writes the lines as source
/// ([`EnvironmentRule`](crate::math_passthrough::EnvironmentRule)).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NewlineRule {
    /// Whether the output written so far ends a line.
    after_line_end: bool,
}

impl NewlineRule {
    /// A text part of a line's content, `last` when no part of the content
    /// follows it: the text to write and, for the last part, the run to write
    /// after the line's closing part ([`normalize_line_newlines`]).
    pub(crate) fn text(&mut self, text: &str, last: bool) -> NormalizedLine {
        let line = normalize_line_newlines(text, self.after_line_end, !last);
        self.after_line_end = ends_line(&line.text, self.after_line_end);
        line
    }

    /// A piece of a line's content written as it is, as the text `written`
    /// (a comment written as its source).
    pub(crate) fn written(&mut self, written: &str) {
        self.after_line_end = ends_line(written, self.after_line_end);
    }

    /// A piece of a line's content that ends a line (`true`: a comment that
    /// the emitter writes) or does not (`false`: a construct).
    pub(crate) fn piece(&mut self, ends_line: bool) {
        self.after_line_end = ends_line;
    }

    /// The end of a line: `closed` when the line's closing part wrote
    /// something (a tag, a label, `\nonumber` or `\\`), `trailing` the run
    /// the rule took off the content's end, written after it.
    pub(crate) fn line_end(&mut self, closed: bool, trailing: &str) {
        if closed {
            self.after_line_end = false;
        }
        if !trailing.is_empty() {
            self.after_line_end = true;
        }
    }
}

/// Stages the equation lines of a display-math environment in the recomposed
/// LaTeX, as they were written (see the module documentation, *The LaTeX frame
/// of a formula* and *The newline rule*): for each line of `frames`, in order,
/// the line's content written by the mathematics writer of the `userdefs`
/// feature ([`write_math_run`]), its text under the newline rule; the line's
/// closing part ([`line_closing`]); and the run of whitespace the newline
/// rule took off the content's end. Returns the nodes to emit.
///
/// A processing feature's rule of `flm.math.environment` on the
/// `latex-recomposed` backend calls it between [`environment_opening`] and
/// [`environment_closing`], with the frames of the environment's lines
/// ([`line_frames`](super::frame::line_frames), read without the two tables,
/// since LaTeX numbers the lines itself). A comment on a line is staged as its
/// comment node, which the emitter writes ending its line; under the render
/// option
/// [`drop_comments`](LatexRecomposedRenderOptions::drop_comments) it writes
/// nothing, and the rule then counts it as nothing written.
///
/// # Errors
///
/// Returns [`LowerError`] when the calling rule does not declare that it reads
/// the safe-labels table (optionally) and [`LatexRecomposedRenderOptions`], or
/// that it produces the raw, macro and definition constructs, or when a node
/// could not be lowered, written or staged.
pub fn stage_lines(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    frames: &[LineFrame<'_>],
) -> Result<Vec<BuildId>, LowerError> {
    let drop_comments = lw.option::<LatexRecomposedRenderOptions>()?.drop_comments;
    let mut rule = NewlineRule::default();
    let mut nodes = Vec::new();
    for frame in frames {
        let pieces = write_math_run(cx, lw, &frame.line.content)?;
        let count = pieces.len();
        let mut trailing = String::new();
        for (index, piece) in pieces.into_iter().enumerate() {
            match piece {
                MathPiece::Text(text) => {
                    let line = rule.text(&text, index + 1 == count);
                    if !line.text.is_empty() {
                        nodes.push(raw(cx, lw, &line.text)?);
                    }
                    trailing = line.trailing;
                }
                MathPiece::Comment(id) => {
                    nodes.push(id);
                    if !drop_comments {
                        rule.piece(true);
                    }
                }
                MathPiece::Node(id) => {
                    nodes.push(id);
                    rule.piece(false);
                }
            }
        }
        let closing = line_closing(cx, lw, frame)?;
        rule.line_end(!closing.is_empty(), &trailing);
        nodes.extend(closing);
        if !trailing.is_empty() {
            nodes.push(raw(cx, lw, &trailing)?);
        }
    }
    Ok(nodes)
}

// --- math groups ------------------------------------------------------------------------------

/// Whether the math group `node` is the formula of `\ensuremath{…}` written
/// in text: the group is the command's argument, so its parent is an
/// [`ENSUREMATH`] node.
fn ensuremath_formula(node: &NodeRef<'_, Flm>) -> bool {
    node.parent().is_some_and(|parent| element_of(&parent).name == ENSUREMATH)
}

/// The opening part of a math group in the recomposed LaTeX: for a displayed
/// group, the text before every displayed formula
/// ([`surround_display_math`](MathRecomposeOptions::surround_display_math))
/// then `\[`; for an inline one, `\(` ([`formula_display`]), whichever
/// delimiters the author used, except for the formula of `\ensuremath{…}`
/// written in text (the math group its braces delimit; see
/// [`ensuremath`](super::ensuremath)), whose opening is `\ensuremath{`. The
/// command is kept as written because such a formula may end up inside another
/// formula in LaTeX (written in a kept user definition's template, or in an
/// argument the template places in mathematics), where LaTeX refuses `\(`.
///
/// A processing feature's rule of `flm.math` on the `latex-recomposed`
/// backend calls it before the group's content, and [`group_closing`] after.
///
/// # Errors
///
/// Returns [`LowerError`] when the calling rule does not declare that it reads
/// [`MathRecomposeOptions`], or when the raw node could not be staged, for
/// instance when the calling rule does not declare that it produces the raw
/// construct.
pub fn group_opening(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
) -> Result<Vec<BuildId>, LowerError> {
    let options = lw.option::<MathRecomposeOptions>()?.clone();
    if !formula_display(node) {
        let opening = if ensuremath_formula(node) { "\\ensuremath{" } else { "\\(" };
        return Ok(vec![raw(cx, lw, opening)?]);
    }
    let mut nodes = Vec::with_capacity(2);
    let (before, _) = &options.surround_display_math;
    if !before.is_empty() {
        nodes.push(raw(cx, lw, before)?);
    }
    nodes.push(raw(cx, lw, "\\[")?);
    Ok(nodes)
}

/// The closing part of a math group in the recomposed LaTeX: for a displayed
/// group, `\]` then the text after every displayed formula
/// ([`surround_display_math`](MathRecomposeOptions::surround_display_math));
/// for an inline one, `\)`, or `}` for the formula of `\ensuremath{…}`
/// written in text; see [`group_opening`].
///
/// # Errors
///
/// Returns [`LowerError`] when the calling rule does not declare that it reads
/// [`MathRecomposeOptions`], or when the raw node could not be staged.
pub fn group_closing(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
) -> Result<Vec<BuildId>, LowerError> {
    let options = lw.option::<MathRecomposeOptions>()?.clone();
    if !formula_display(node) {
        let closing = if ensuremath_formula(node) { "}" } else { "\\)" };
        return Ok(vec![raw(cx, lw, closing)?]);
    }
    let mut nodes = vec![raw(cx, lw, "\\]")?];
    let (_, after) = &options.surround_display_math;
    if !after.is_empty() {
        nodes.push(raw(cx, lw, after)?);
    }
    Ok(nodes)
}

/// Registers what the `math` feature contributes to the `latex-recomposed`
/// backend: a peer declaration ([`PeerName`](crate::core::registry::PeerName))
/// of the backend, of the two construct elements the rules of the text
/// commands and of `\eqref` stage (each with its payload type) and of the
/// safe-labels table (with its value type), then [`text_rule`]. The options
/// [`MathRecomposeOptions`] are declared with the feature's settings.
///
/// In a registry without the LaTeX features the peer declarations resolve to
/// nothing and the rules are set aside (excluded from rule selection); the
/// registry still builds.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rules of the text commands \
         and of `\\eqref` are the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro construct, which the recomposition rules of the text \
         commands and of `\\eqref` stage",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, which the recomposition rule of `\\eqref` stages",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the recomposition rule of \
         `\\eqref` reads for the safe label of the label it refers to",
    )?;
    b.lowering_rule(text_rule())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalized(content: &str, after_line_end: bool, inner: bool) -> (String, String) {
        let line = normalize_line_newlines(content, after_line_end, inner);
        (line.text, line.trailing)
    }

    #[test]
    fn every_newline_run_becomes_one_newline_and_the_last_indentation() {
        assert_eq!(normalized("\n   \n   x &= 0.\n", false, false), ("\n   x &= 0.".into(), "\n".into()));
        assert_eq!(normalized("a\n\n\n\tb c\n", false, false), ("a\n\tb c".into(), "\n".into()));
        assert_eq!(normalized("x = 1 \n", false, false), ("x = 1".into(), "\n".into()));
        assert_eq!(normalized("x = 1 ", false, false), ("x = 1 ".into(), String::new()));
        assert_eq!(normalized("a \t b", false, false), ("a \t b".into(), String::new()));
        assert_eq!(normalized("", false, false), (String::new(), String::new()));
        assert_eq!(normalized("  ", false, false), ("  ".into(), String::new()));
    }

    #[test]
    fn the_run_that_begins_a_part_is_dropped_only_after_a_line_end() {
        assert_eq!(normalized("\n  b &= 2 \n", false, false), ("\n  b &= 2".into(), "\n".into()));
        assert_eq!(normalized("\n  b &= 2 \n", true, false), ("b &= 2".into(), "\n".into()));
        // A content of whitespace only follows the rule for the run that begins it.
        assert_eq!(normalized("\n  ", false, false), ("\n  ".into(), String::new()));
        assert_eq!(normalized("\n  ", true, false), (String::new(), String::new()));
    }

    #[test]
    fn a_part_that_is_not_the_last_keeps_its_run_in_place() {
        assert_eq!(normalized("a %% c\n   ", false, true), ("a %% c\n   ".into(), String::new()));
        assert_eq!(normalized("a %% c\n \n  ", false, true), ("a %% c\n  ".into(), String::new()));
    }

    #[test]
    fn whether_written_text_ends_a_line() {
        assert!(ends_line("a\n  ", false));
        assert!(ends_line("a\n", false));
        assert!(!ends_line("a\n  b", false));
        assert!(!ends_line("a", true));
        assert!(ends_line("  ", true));
        assert!(!ends_line("  ", false));
        assert!(!ends_line("", false));
        assert!(ends_line("", true));
    }

    /// The parts of one line written under the rule, as `(text, trailing)`:
    /// each part a text or, as `None`, a comment that ends its line.
    fn written_line(rule: &mut NewlineRule, parts: &[Option<&str>], closed: bool) -> (String, String) {
        let mut text = String::new();
        let mut trailing = String::new();
        for (index, part) in parts.iter().enumerate() {
            match part {
                Some(part) => {
                    let line = rule.text(part, index + 1 == parts.len());
                    text.push_str(&line.text);
                    trailing = line.trailing;
                }
                None => {
                    text.push_str("%c\n  ");
                    rule.piece(true);
                }
            }
        }
        rule.line_end(closed, &trailing);
        (text, trailing)
    }

    #[test]
    fn the_rule_follows_the_line_ends_across_lines_and_comments() {
        // `a &= b` / `\label{x}` / `\\` / `c &= d` / `\\` (final): the run
        // after the label goes after the first line's `\\`, so the second
        // line's first run goes; the run before the final `\\` and the one
        // after it go after the last line.
        let mut rule = NewlineRule::default();
        assert_eq!(written_line(&mut rule, &[Some("\n  a &= b\n  \n  ")], true), ("\n  a &= b".into(), "\n  ".into()));
        assert_eq!(written_line(&mut rule, &[Some("\n  c &= d\n  \n")], false), ("c &= d".into(), "\n".into()));
        // `a &= b \\ %c` / `\label{x}` / `c &= d`: the comment ends its line,
        // so the run after the label, which begins the next part, goes.
        let mut rule = NewlineRule::default();
        assert_eq!(written_line(&mut rule, &[Some("\n  a &= b ")], true), ("\n  a &= b ".into(), String::new()));
        assert_eq!(
            written_line(&mut rule, &[Some(" "), None, Some("\n  c &= d\n")], false),
            (" %c\n  c &= d".into(), "\n".into())
        );
        // A construct between two parts is not a line's end.
        let mut rule = NewlineRule::default();
        rule.piece(true);
        rule.piece(false);
        assert_eq!(rule.text("\n  x", false).text, "\n  x");
        // A comment written as source ends a line only when its text does.
        let mut rule = NewlineRule::default();
        rule.written("%c");
        assert_eq!(rule.text("\n\n  x", true).text, "\n  x");
        rule.written("%c\n  ");
        assert_eq!(rule.text("\n  y", true).text, "y");
    }

    #[test]
    fn the_text_rule_is_the_default_on_the_recomposed_backend_only() {
        let rule = text_rule();
        assert_eq!(rule.name, TEXT_LATEX_RULE);
        assert_eq!(rule.element, TEXT);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
        assert_eq!(rule.decl.produces, [ElementName::from(MACRO)]);
    }
}
