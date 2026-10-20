//! The recomposition rules of the [`math-passthrough`](super) feature for the
//! `latex-recomposed` backend: inline and displayed mathematics, and
//! display-math environments rebuilt line by line, each formula written as it
//! was written.
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
//! # Mathematics is written from its nodes
//!
//! Every formula of the recomposed LaTeX (`\(…\)`, `\[…\]`, and each equation line
//! of a display-math environment) is written by the mathematics writer of the
//! `userdefs` feature
//! ([`write_math_run`](crate::stdcore::features::userdefs::kept::write_math_run)):
//! the source byte for byte, except where a user definition is used. A use of a
//! user definition that can be written back as a LaTeX definition is written as a
//! call, and its definition is written to the style file, as for a use in text; a
//! use of any other user definition is written as its expansion. Nothing inside a
//! formula is lowered
//! through the rules except a text command, which is lowered like document text
//! ([`TEXT_LATEX_RULE`](crate::stdcore::features::math::latex::TEXT_LATEX_RULE)),
//! and a source comment, which is staged as a comment node
//! for the emitter.
//!
//! # Inline and displayed mathematics
//!
//! [`MATH_LATEX_RULE`] (`flm.math.lower.passthrough-latex-recomposed`) is the rule of
//! `flm.math` on this backend: the group's content written by the mathematics
//! writer, between the opening and closing parts of the `math` feature's LaTeX
//! library ([`group_opening`], [`group_closing`]): `\[`…`\]` for displayed
//! mathematics and `\(`…`\)` for inline mathematics, whichever delimiters the
//! author used (the formula of an `\ensuremath{…}` written in text:
//! `\ensuremath{`…`}`).
//!
//! # Display-math environments
//!
//! A display-math environment is rebuilt line by line, because its body holds
//! data that the recomposed LaTeX writes differently from the source: the labels
//! of each line, which the author may write anywhere on the line and which are
//! written together at the line's end (see *What is written for a line's labels*
//! below), and the tag of a line, whose content is document text that the rules
//! lower and the emitter escapes; the rule writes every tag as `\tag*{…}` and
//! adds the parentheses of an unstarred `\tag` itself.
//!
//! [`ENVIRONMENT_LATEX_RULE`] (`flm.math.environment.lower.passthrough-latex-recomposed`)
//! reads the body as equation lines ([`line_frames`]) and stages, between the
//! opening and closing parts of the `math` feature's LaTeX library
//! ([`environment_opening`], [`environment_closing`]), the lines as the
//! library's line writer ([`stage_lines`]) writes them: for each line in order
//! its content (item 1 below), the library's closing part of a line
//! ([`line_closing`](crate::stdcore::features::math::latex::line_closing):
//! items 2 to 5), and the run of whitespace of item 6:
//!
//! 1. The line's content, written by the mathematics writer: raw nodes under the
//!    library's newline rule (below), the constructs of the user definitions
//!    used on the line, and every source comment as its comment node, which the
//!    emitter writes as it writes a comment in text (ending the comment's line
//!    itself), or leaves out under the backend's render option
//!    [`drop_comments`](LatexRecomposedRenderOptions::drop_comments).
//! 2. `\tag*{…}` when the line has a tag: a macro construct whose argument holds
//!    the tag's content lowered through the ordinary rules, between a `(` and a
//!    `)` when the tag is not starred. The content is document text, so the
//!    emitter escapes it.
//! 3. The line's labels, in order, as one group in `\flmLabels`, a [LaTeX
//!    hook](crate::stdcore::backends::latex::preamble) (a macro that the style
//!    file defines with a plain default and that a user redefines in the
//!    document's preamble; see *What is written for a line's labels*).
//! 4. `\nonumber` when the line is marked with it, as a macro construct with no
//!    argument.
//! 5. The line's `\\`, with its spacing argument as written, as a raw node, when
//!    another line follows. A final `\\` followed only by whitespace and comments
//!    is not written back: `amsmath` would set an empty line for it.
//! 6. The run of whitespace that ended the line's content (a newline and the
//!    indentation after it), when the newline rule took it off, as a raw node.
//!
//! **The newline rule** is the `math` library's
//! ([`normalize_line_newlines`](crate::stdcore::features::math::latex::normalize_line_newlines);
//! the library's module documentation, *The newline rule*, states it in full).
//! Inside a line's content, every run of spaces, tabs and newlines that holds a
//! newline is written as one newline followed by the indentation after the run's
//! last newline, so that a label taken off a line of its own, or a dropped
//! comment, leaves no blank line; the run that ends the content is written after
//! the line's closing part (item 6), and a run that comes right after a line's
//! end (that run of the previous line, or a comment) is dropped. When the content
//! of a line ends with a comment, the line's tag, labels, `\nonumber` and `\\` are
//! written after the comment's newline, on the next line: a comment never hides
//! the commands of its line.
//!
//! Every environment except `equation` is defined by the LaTeX package `amsmath`,
//! which the rule then requests (records a package request for; see
//! [`packages`](crate::stdcore::backends::latex::packages)). A body reported as malformed (an empty line, reported by the
//! feature's check stage, or a label or tag on a line without a number,
//! reported by the `math` feature when the body is parsed) is written back as
//! the author wrote it; those Errors are the only report.
//!
//! Two options of the `math` feature ([`MathRecomposeOptions`], set per render
//! request) change the result:
//!
//! - [`surround_display_math`](MathRecomposeOptions::surround_display_math) is
//!   a pair of LaTeX texts written, byte for byte, before and after every
//!   displayed formula, environments and `\[…\]` groups alike (both empty by
//!   default).
//! - [`emit_flm_math_environment_macro`](MathRecomposeOptions::emit_flm_math_environment_macro)
//!   writes the environment as `\flmMathEnvironment{<name>}{<arguments>}{<body>}`
//!   instead, a LaTeX hook defined over `\begin`…`\end` by the definition piece
//!   [`MATH_ENVIRONMENT`](crate::stdcore::backends::latex::preamble::MATH_ENVIRONMENT),
//!   which the rule then requires. The hook is a LaTeX hook like `\flmLabels`
//!   (item 3 above); a definition piece is the unit of LaTeX definitions that
//!   defines it
//!   ([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)).
//!   The six environments take no arguments, so `<arguments>` is empty.
//!
//! # What is written for a line's labels
//!
//! A label written on an equation line is data of that line, and
//! [`ENVIRONMENT_LATEX_RULE`] writes it. Every label of the recomposed LaTeX is
//! written inside the LaTeX hook `\flmLabels{…}`, in mathematics as after a
//! heading: each label through [`stage_label`](crate::stdcore::features::refs::stage_label), and the line's labels together
//! through [`stage_label_group`](crate::stdcore::features::refs::stage_label_group), as in `\flmLabels{\label{ref1}\label{ref2}}`.
//! The group makes every label after the first on a line refer to that line's
//! number, which `amsmath` alone does not do. The render option
//! [`label_hooks`](LatexRecomposedRenderOptions::label_hooks) turns the group off
//! here as it does everywhere.
//!
//! What is written inside each `\label` depends on the safe-labels table of the
//! references feature ([`SAFE_LABELS`]), which the rule reads optionally, under a
//! peer declaration ([`PeerName`](crate::core::registry::PeerName)):
//!
//! - **With the references feature**, the table holds a safe label for the label
//!   (the key under which the recomposed LaTeX writes it,
//!   [`SafeLabels`]), and [`stage_label`](crate::stdcore::features::refs::stage_label) writes it.
//! - **Without it**, the table is absent, and [`stage_label`](crate::stdcore::features::refs::stage_label) writes the label as
//!   the document wrote it, byte for byte and **unescaped**; what LaTeX makes of
//!   a character it treats specially is the caller's concern. The choice between
//!   the two cases is made by
//!   [`label_spelling`](crate::stdcore::features::refs::label_spelling).
//!
//! When the table is absent, the render option
//! [`drop_math_labels_without_refs`](LatexRecomposedRenderOptions::drop_math_labels_without_refs)
//! is also read: when it is set, nothing is written for the line's labels, no
//! `\label` and no group. With the references feature registered, the option is
//! not read and changes nothing. A `\tag` and a `\nonumber` are not labels and are
//! written in both cases.
//!
//! No rule here numbers a line: the recomposed LaTeX leaves the numbering to
//! LaTeX, so the numbers of the compiled document follow its document class and
//! not FLM's counters.
//!
//! # Peer declarations
//!
//! The feature owns neither the backend nor the construct elements nor the
//! safe-labels table, and reads the options of the `math` feature. [`register`] declares them as peer declarations, so that a
//! registry without the LaTeX features still builds; the rules are then set aside
//! (excluded from rule selection; see
//! [`PeerName`](crate::core::registry::PeerName)). The safe-labels table is read
//! optionally, so that a registry with the backend and without the references
//! feature still renders.


use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::lang::structural::MATH;
use crate::stdcore::backends::latex::construct::{DEFINITION, MACRO, RAW};
use crate::stdcore::backends::latex::{LatexDefinitionPayload, LatexMacroPayload, LatexRawPayload};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::features::math::frame::line_frames;
use crate::stdcore::features::math::latex::{
    environment_closing, environment_opening, group_closing, group_opening, stage_lines,
};
use crate::stdcore::features::math::{ENVIRONMENT, MathRecomposeOptions};
use crate::stdcore::features::refs::{SAFE_LABELS, SafeLabels};
use crate::stdcore::features::userdefs::kept::stage_math_run;

/// The name of the recomposition rule of the display-math environments
/// (`flm.math.environment`) of passthrough mathematics,
/// `flm.math.environment.lower.passthrough-latex-recomposed`, whose body is
/// [`EnvironmentLatexRule`]. The default on the `latex-recomposed` backend
/// only.
pub const ENVIRONMENT_LATEX_RULE: &str =
    "flm.math.environment.lower.passthrough-latex-recomposed";

/// The name of the recomposition rule of inline and displayed mathematics
/// (`flm.math`) of passthrough mathematics,
/// `flm.math.lower.passthrough-latex-recomposed`, whose body is
/// [`MathLatexRule`]. The default on the `latex-recomposed` backend only.
pub const MATH_LATEX_RULE: &str = "flm.math.lower.passthrough-latex-recomposed";

/// The body of [`MATH_LATEX_RULE`]: the group's opening part
/// ([`group_opening`]), the group's content written by the mathematics writer
/// ([`stage_math_run`]), and the group's closing part ([`group_closing`]). See
/// the module documentation, *Inline and displayed mathematics*.
#[derive(Debug, Default, Clone, Copy)]
pub struct MathLatexRule;

impl Lower for MathLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let content: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
        let mut nodes = group_opening(cx, lw, &node)?;
        nodes.extend(stage_math_run(cx, lw, &content)?);
        nodes.extend(group_closing(cx, lw, &node)?);
        Ok(Lowering::Emit(nodes))
    }
}

/// The rule [`MATH_LATEX_RULE`], ready to be registered: it targets
/// `flm.math`, produces the raw construct and the definition construct (for a
/// user definition used inside mathematics), and is the default on the
/// `latex-recomposed` backend only.
pub fn math_rule() -> LoweringRule {
    LoweringRule::new(
        MATH_LATEX_RULE,
        MATH,
        LoweringDecl::new()
            .option::<MathRecomposeOptions>()
            .option::<LatexRecomposedRenderOptions>()
            .produces(RAW)
            .produces(DEFINITION),
        Arc::new(MathLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "inline or displayed mathematics lowers to its delimiters around its content written \
         from its nodes: byte for byte, except that a use of a user definition is written as \
         a call, with the definition for the style file, or as its expansion",
    )
}

/// The body of [`ENVIRONMENT_LATEX_RULE`]: the environment rebuilt line by
/// line (see the module documentation, *Display-math environments*), between
/// the environment's opening part ([`environment_opening`]) and its closing
/// part ([`environment_closing`]).
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`MathEnvironment`](crate::stdcore::features::math::MathEnvironment), when a
/// table or an option cannot be read, or when a node could not be staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvironmentLatexRule;

impl Lower for EnvironmentLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut nodes = environment_opening(cx, lw, &node)?;
        // LaTeX numbers the lines and finds their anchors itself: the frames
        // are read without the two tables.
        let frames = line_frames(None, None, lw.addr(), &node)
            .map_err(|error| LowerError::structure(error.to_string()))?;
        nodes.extend(stage_lines(cx, lw, &frames)?);
        nodes.extend(environment_closing(cx, lw, &node)?);
        Ok(Lowering::Emit(nodes))
    }
}

/// The rule [`ENVIRONMENT_LATEX_RULE`], ready to be registered: it targets
/// `flm.math.environment`, reads the safe-labels table optionally and two
/// option types (its own [`MathRecomposeOptions`] and the backend's
/// [`LatexRecomposedRenderOptions`]), produces the macro, raw and
/// definition constructs (the last for a user definition used on a line), and
/// is the default on the `latex-recomposed` backend only.
///
/// A line's labels are written from the line's own data; the module
/// documentation, *What is written for a line's labels*, says what the table
/// and the backend's options change.
pub fn environment_rule() -> LoweringRule {
    LoweringRule::new(
        ENVIRONMENT_LATEX_RULE,
        ENVIRONMENT,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .option::<MathRecomposeOptions>()
            .option::<LatexRecomposedRenderOptions>()
            .produces(MACRO)
            .produces(RAW)
            .produces(DEFINITION),
        Arc::new(EnvironmentLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a display-math environment lowers to the LaTeX environment of the same name over \
         its equation lines rebuilt one by one, each line's labels written as one group in \
         `\\flmLabels` at the end of the line: as their safe labels when the safe-labels \
         table is present, as the document wrote them when it is absent, and not at all \
         when it is absent and the render options ask for none",
    )
}


/// Registers what the `math-passthrough` feature contributes to the
/// `latex-recomposed` backend: a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the backend, of the three
/// construct elements the rules stage (each with its payload type) and of the
/// safe-labels table (with its value type), then [`math_rule`] and
/// [`environment_rule`].
///
/// In a registry without the LaTeX features the peer declarations resolve to
/// nothing and the rules are set aside (excluded from rule selection); the
/// registry still builds. The safe-labels table is read optionally, so that a
/// registry with the backend and without the references feature still
/// renders: a line's labels are then written as the document wrote them,
/// inside `\flmLabels` as always (see the module documentation, *What is
/// written for a line's labels*).
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rules of passthrough \
         mathematics are the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro construct, which the recomposition rules of mathematics \
         stage",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, which the recomposition rules of mathematics \
         stage",
    )?;
    b.peer_element::<LatexDefinitionPayload>(
        DEFINITION,
        "the LaTeX family's definition construct, which the recomposition rules of \
         mathematics stage for a user definition used inside a formula",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the recomposition rule of the \
         display-math environments reads for the safe label of each label of an equation \
         line",
    )?;
    b.lowering_rule(math_rule())?;
    b.lowering_rule(environment_rule())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::names::ElementName;

    #[test]
    fn the_rule_is_the_default_on_the_recomposed_backend_only() {
        let rule = environment_rule();
        assert_eq!(rule.name, ENVIRONMENT_LATEX_RULE);
        assert_eq!(rule.element, ENVIRONMENT);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
        assert_eq!(rule.decl.options.len(), 2);
        assert!(rule.decl.reads.is_empty());
        assert_eq!(
            rule.decl.reads_opt.len(),
            1,
            "a line's labels are written as their safe keys where the table is there"
        );
        assert_eq!(rule.decl.produces.len(), 3);
        let math = math_rule();
        assert_eq!(math.name, MATH_LATEX_RULE);
        assert_eq!(math.element, MATH);
        assert!(math.default.is_specific_to(&BACKEND.into()));
        assert_eq!(math.decl.produces, [ElementName::from(RAW), ElementName::from(DEFINITION)]);
        assert!(
            math.decl.options.contains(&crate::model::options::OptionRef::of::<MathRecomposeOptions>()),
            "a displayed group is surrounded as an environment is, so the rule reads the math options"
        );
    }

}
