//! The headings on the `latex-recomposed` backend: each heading written back as the
//! LaTeX sectioning command its heading kind maps to, starred when the heading is not
//! numbered, inside the LaTeX hook `\flmHeading`, with the heading's labels after it.
//!
//! A *LaTeX hook* is a macro that the recomposed LaTeX writes and the style file (the
//! `.sty` file written beside the recomposed LaTeX;
//! [`style`](crate::stdcore::backends::latex::style)) defines with a plain default,
//! which a user redefines in the document's preamble to change what the recomposed
//! LaTeX does at that point
//! ([`latex::preamble`](crate::stdcore::backends::latex::preamble); see [the LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks)).
//!
//! # What the rule writes
//!
//! [`LATEX_RULE`] (`flm.heading.lower.latex-recomposed`), implemented by
//! [`HeadingLatexRule`], is the default lowering rule of `flm.heading` on the
//! `latex-recomposed` backend only. For a heading with two labels it writes:
//!
//! ```text
//! \flmHeading[toc=true]
//! \section*{Title}{\flmLabels{\label{ref1}\label{ref2}}}
//! ```
//!
//! that is, the hook with the key `toc`, a newline, the sectioning command with the
//! title as its mandatory argument, and one braced group that holds the heading's
//! labels inside the LaTeX hook `\flmLabels`. A heading written with an alternative
//! title (`\section[alt={Short title}]{…}`) adds the hook's key `alt`:
//! `\flmHeading[toc=true,alt={Short title}]`. Where the alternative title holds a
//! command, the whole optional argument is braced, as every optional argument holding
//! a command is written, so that a `]` inside it cannot end it:
//! `\flmHeading[{toc=true,alt={Short \emph{title}}}]`; LaTeX removes those braces
//! when it reads the argument.
//!
//! The sectioning command is the one the render option
//! [`HeadingsLatexOptions::latex_commands`] maps the heading kind's name to, or the
//! kind's name itself where the option gives no command for it: by default a
//! `section` is written `\section`, and under a mapping of `section` to `chapter` it
//! is written `\chapter`. It does not depend on the command the heading was written
//! with, so a heading written with a second command of the kind `section` is written
//! as every `section` is.
//!
//! The star is written when the heading is not numbered (written with a star or with
//! `numbered=false`, see [`heading_options`](super::heading_options)), because the
//! star is how LaTeX writes an unnumbered heading. LaTeX numbers the other headings
//! itself, so the rule writes no number. Whether the heading is listed in a table of
//! contents ([`Sectioning::in_toc`](crate::model::aspects::Sectioning::in_toc)) is
//! written as the hook's key, `toc=true` or `toc=false`, on every heading. The hook's
//! default definition adds a starred heading written with `toc=true` to LaTeX's table
//! of contents, which LaTeX does not do for a starred command, and emits a warning
//! that `toc=false` on an unstarred heading is not honored
//! ([`heading_piece`](crate::stdcore::backends::latex::preamble::heading_piece)). With
//! the render option `heading_hook` off, nothing carries the key and nothing is
//! written for it.
//!
//! An alternative title written with the key `alt` ([`heading_alt`])
//! is written as the hook's key `alt`, after `toc`, its value the alternative title's
//! pieces lowered as ordinary content inside braces. The hook hands it to the
//! sectioning command as its own optional argument on an unstarred heading, and adds it
//! as the starred heading's table-of-contents line where that heading is listed; LaTeX
//! builds its table of contents and its PDF bookmarks from it. With the render option
//! `heading_hook` off, nothing carries the key and nothing is written for it, as for
//! `toc`.
//!
//! The heading's bracket of options is never written back. The rule reports what that
//! bracket holds that it cannot read, as the default rule does (see the [feature's
//! documentation](super#what-an-occurrence-says-the-star-and-the-key-values)).
//!
//! The labels are the heading's own data: the rule reads its `\label` fields
//! ([`label_uses`](crate::stdcore::features::refs::label_uses)) and writes each as
//! `\label{<safe label>}` with [`stage_label`](crate::stdcore::features::refs::stage_label),
//! where a safe label is the key the recomposed LaTeX writes for an FLM label
//! ([`SafeLabels`]); [`stage_label_group`] wraps them in the LaTeX hook `\flmLabels`. A
//! `\label` field is never lowered on its own: a field that reaches a backend is an
//! error.
//!
//! The newline between the hook and the sectioning command is there for a person
//! reading the recomposed LaTeX: the sectioning command then starts a line of its own.
//! LaTeX reads the same tokens either way (a command name followed by a newline is the
//! same token sequence as the command name alone), so the sectioning command is still
//! the hook's first mandatory argument. The hook's optional argument, `[toc=…]`, is on
//! the hook's line, before the newline. Every heading gets the newline, including a
//! run-in heading in the middle of a paragraph (`text \flmHeading[toc=true]`, a
//! newline, `\paragraph{P}{…} and more`), where LaTeX reads the newline as ordinary
//! whitespace.
//!
//! # What the hook is for
//!
//! A LaTeX label refers to the last thing that was numbered. A heading the document
//! class did not number (`\section*`, or a level below the class's `secnumdepth`)
//! numbers nothing, so a reference to a label after it prints the number of the last
//! thing that was numbered before it, which is wrong and not reported. `\flmHeading`
//! corrects this without modifying any LaTeX command: it runs the sectioning command as
//! written, asks the LaTeX kernel whether a counter was advanced, and where none was,
//! makes the labels refer to the heading with its **title** as what a reference prints
//! ([`heading_piece`](crate::stdcore::backends::latex::preamble::heading_piece) says
//! how). A `\ref` to a starred heading therefore prints "Introduction".
//!
//! With the hook, the group that holds the labels is written for **every** heading,
//! as `{\flmLabels{}}` when the heading has no label, so that a user who redefines
//! `\flmHeading` can rely on its last argument, the labels group, always being present.
//!
//! # The hook's key-value argument
//!
//! `\flmHeading` takes an optional key-value argument with two keys
//! ([`heading_piece`](crate::stdcore::backends::latex::preamble::heading_piece) says
//! what each does):
//!
//! - `toc=true|false`, whether the heading is listed in a table of contents. This rule
//!   writes it on every heading, from the heading's
//!   [`Sectioning::in_toc`](crate::model::aspects::Sectioning::in_toc).
//! - `alt={Short title}`, an alternative title passed to the sectioning command as its
//!   own optional argument, for the table of contents and the PDF bookmark. This rule
//!   writes it where the heading's options give an alternative title with the key
//!   `alt` ([`heading_alt`]), and nowhere else.
//!
//! # The two render options of the recomposed LaTeX
//!
//! With [`heading_hook`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::heading_hook)
//! off, the hook is left out: the recomposed LaTeX is `\section*{Title}` followed by
//! the labels, and the labels group is written only where the heading has a label.
//! The hook's test then does not run and nothing replaces it: a reference to an
//! unnumbered heading prints what LaTeX recorded last, and giving it a display text
//! is the user's responsibility. Nothing carries the keys `toc` and `alt` either, so
//! nothing is written for them.
//!
//! With [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! off, `\flmLabels` is left out of the group, which then holds plain `\label`
//! commands; with the heading hook still on, `\flmHeading` receives them directly and
//! works unchanged.
//!
//! A title that a parse left absent after reporting it lowers to an empty argument.
//! Whether a kind is set as a run-in heading does not change the recomposed LaTeX:
//! LaTeX itself sets `\paragraph` and `\subparagraph` as run-in headings.
//!
//! # Registration without the LaTeX features
//!
//! The feature registers the rule with a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of each name the rule uses that the
//! LaTeX features own: the backend `latex-recomposed`, the
//! macro construct `flm.pres.latex.macro` and the raw construct `flm.pres.latex.raw`
//! (a raw node holds LaTeX text written as it is). A registry without the LaTeX
//! features therefore still builds, with the rule set aside (excluded from rule
//! selection; see [`PeerName`](crate::core::registry::PeerName)). See [`register`].

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::element_value;
use crate::model::names::ElementName;
use crate::model::names::Name;
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::preamble;
use crate::stdcore::backends::latex::{LatexArgShape, LatexMacroPayload, LatexRawPayload};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::{lower_arg_named_or_nothing, lower_written_pieces};
use crate::stdcore::features::refs::{self, SAFE_LABELS, SafeLabels, stage_label_group};

use super::title::NUMBER_ELEMENT;
use super::{HEADING, HeadingElement, TITLE_ARG, heading_alt, report_option_problems};

/// The name of the recomposition rule, `flm.heading.lower.latex-recomposed`.
pub const LATEX_RULE: &str = "flm.heading.lower.latex-recomposed";

/// The name of the lowering rule of a heading's number ([`NUMBER_ELEMENT`]) on
/// the `latex-recomposed` backend, `flm.heading.number.lower.latex-recomposed`
/// ([`number_latex_rule`]).
pub const NUMBER_LATEX_RULE: &str = "flm.heading.number.lower.latex-recomposed";

/// The render options of the headings on the recomposed LaTeX: which LaTeX
/// sectioning command each heading kind is written as.
///
/// The option type is declared with
/// [`Bind::Render`](crate::model::options::Bind::Render), so its value is set on
/// the render request and is part of the plan key
/// ([`PlanKey`](crate::core::plan::PlanKey)), the digest under which a built plan can
/// be cached. The feature declares it
/// whether or not the registry holds the `latex-recomposed` backend; only
/// [`LATEX_RULE`] reads it. In settings data, its key is `latex_commands`.
///
/// # Examples
///
/// ```
/// use flm_core::model::names::Name;
/// use flm_core::stdcore::features::headings::latex::HeadingsLatexOptions;
///
/// // A `section` is written `\chapter`, every other kind under its own name.
/// let options = HeadingsLatexOptions::new().command("section", "chapter");
/// assert_eq!(options.command_for(&Name::from("section")), "chapter");
/// assert_eq!(options.command_for(&Name::from("subsection")), "subsection");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HeadingsLatexOptions {
    /// The LaTeX sectioning command, without the backslash, that each heading
    /// kind is written as, keyed by the kind's name. A kind the map does not list
    /// is written as the command of its own name. Empty by default.
    pub latex_commands: BTreeMap<Name, String>,
}

impl HeadingsLatexOptions {
    /// Returns the default options: every kind written as the command of its
    /// own name.
    pub fn new() -> HeadingsLatexOptions {
        HeadingsLatexOptions::default()
    }

    /// Builder: the kind `heading` is written as the LaTeX command `command`.
    pub fn command(
        mut self,
        heading: impl Into<Name>,
        command: impl Into<String>,
    ) -> HeadingsLatexOptions {
        self.latex_commands.insert(heading.into(), command.into());
        self
    }

    /// Returns the LaTeX command, without the backslash, that the kind `heading`
    /// is written as: its entry in [`latex_commands`](Self::latex_commands), else
    /// its own name.
    pub fn command_for(&self, heading: &Name) -> String {
        self.latex_commands.get(heading).cloned().unwrap_or_else(|| heading.to_string())
    }
}

/// The implementation of the lowering rule [`LATEX_RULE`]: it stages the hook
/// `\flmHeading` with its key `toc`, and its key `alt` where the heading has an
/// alternative title, a newline, the sectioning command the
/// heading's kind maps to ([`HeadingsLatexOptions`]) as a LaTeX macro construct,
/// starred when the heading is not numbered, and a group holding the labels the
/// heading read from its `\label` fields, each written as its safe label. See the
/// [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct HeadingLatexRule;

impl Lower for HeadingLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<HeadingElement>(&node)
            .ok_or_else(|| LowerError::structure("a heading without its element's data"))?;
        report_option_problems(&node, value, lw)?;
        let hook = lw.option::<LatexRecomposedRenderOptions>()?.heading_hook;
        let command = lw.option::<HeadingsLatexOptions>()?.command_for(&value.decl.name);
        let occurrence = value.occurrence(&node);
        let mut args = Vec::new();
        if !occurrence.numbered {
            args.push(LatexArgShape::Star);
        }
        args.push(LatexArgShape::Mandatory);
        let title = lower_arg_named_or_nothing(cx, lw, &node, TITLE_ARG)?;
        let payload = LatexMacroPayload::new(command, args);
        let heading = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            payload,
            vec![(String::from(ARG_SLOT), title)],
        )?;
        // With the hook, every heading carries the group, empty or not; it
        // is the hook's fourth argument, and one shape means a user who
        // redefines `\flmHeading` can count on four arguments always being
        // there. Without the hook nothing takes the group, so it is written
        // only where the heading has a label — which is why the hook itself
        // is what decides whether an empty group is written.
        let write_empty_group = hook;
        let mut labels = Vec::new();
        for written in refs::label_uses(&node) {
            labels.push(refs::stage_label(cx, lw, &written.text, written.label.as_ref())?);
        }
        let labels = stage_label_group(cx, lw, labels, write_empty_group)?;
        if !hook {
            let mut nodes = vec![heading];
            nodes.extend(labels);
            return Ok(Lowering::Emit(nodes));
        }
        lw.require(&Name::from(preamble::HEADING));
        // Whether the heading is listed in a table of contents travels as the
        // hook's key `toc`, on every heading: the hook's argument is the one
        // place a heading's key-values are written, never the sectioning
        // command's.
        let listed = !value.decl.skip_toc && occurrence.toc;
        let mut keys = vec![raw(cx, lw, if listed { "toc=true" } else { "toc=false" })?];
        // The alternative title travels as the hook's key `alt`, its pieces
        // lowered as the author wrote them, inside braces.
        if let Some(alt) = heading_alt(&node) {
            keys.push(raw(cx, lw, ",alt={")?);
            keys.extend(lower_written_pieces(cx, lw, &alt)?);
            keys.push(raw(cx, lw, "}")?);
        }
        let token = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(preamble::HEADING_COMMAND, [LatexArgShape::Optional]),
            vec![(String::from(ARG_SLOT), keys)],
        )?;
        // The newline after the hook token puts the sectioning command on a
        // line of its own, for the reader; LaTeX reads the same tokens.
        let newline = raw(cx, lw, "\n")?;
        // The group's braces: the hook's fourth argument is a braced group
        // holding the labels, and a rule has no other way to write one.
        let open = raw(cx, lw, "{")?;
        let close = raw(cx, lw, "}")?;
        let mut nodes = vec![token, newline, heading, open];
        nodes.extend(labels);
        nodes.push(close);
        Ok(Lowering::Emit(nodes))
    }
}

/// Stages one raw node (`flm.pres.latex.raw`, LaTeX text written as it is)
/// holding `text`: the hook's keys, the braces around the value of its key
/// `alt`, the newline after the hook and the braces of the hook's label group.
fn raw(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    text: &str,
) -> Result<techy::core::node::BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Returns the lowering rule [`LATEX_RULE`], ready to be registered: it produces
/// `flm.pres.latex.macro` and `flm.pres.latex.raw`, reads the safe-labels table
/// optionally, and is the default on the `latex-recomposed` backend only.
pub fn latex_rule() -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        HEADING,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .option::<LatexRecomposedRenderOptions>()
            .option::<HeadingsLatexOptions>()
            .produces(MACRO)
            .produces(RAW),
        Arc::new(HeadingLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a heading lowers to the LaTeX hook `\\flmHeading[toc=…]` (with `alt={…}` where the \
         heading has an alternative title), a newline, the LaTeX \
         sectioning command its heading kind maps to, starred when not numbered, and a group \
         holding the labels it read from its `\\label` fields",
    )
}

/// Returns the lowering rule [`NUMBER_LATEX_RULE`] of a heading's number
/// ([`NUMBER_ELEMENT`]) on the `latex-recomposed` backend: the number's content,
/// lowered through the lowering rules, and nothing around it. It produces no
/// element of its own and is the default on that backend only.
///
/// The number node occurs only inside what the default heading rule writes
/// before a title. This backend writes a heading back as its sectioning command
/// through [`latex_rule`] and leaves the numbering to LaTeX, so the node never
/// reaches it. The rule exists because a plan checks that its backend can lower
/// every element a document may hold.
pub fn number_latex_rule() -> LoweringRule {
    LoweringRule::new(
        NUMBER_LATEX_RULE,
        NUMBER_ELEMENT,
        LoweringDecl::new(),
        Arc::new(NumberLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a heading's number lowers to its content; the heading itself is written back as its \
         sectioning command, so the number never reaches this backend",
    )
}

/// The implementation of the lowering rule [`number_latex_rule`].
#[derive(Debug, Default, Clone, Copy)]
struct NumberLatexRule;

impl Lower for NumberLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(lw.lower_body(cx, &node)?))
    }
}

/// Registers what the headings feature contributes to the `latex-recomposed`
/// backend: the peer declarations of the backend, of the constructs the rule
/// stages and of the table it reads, then [`latex_rule`] and
/// [`number_latex_rule`]. (The feature declares the render options
/// [`HeadingsLatexOptions`] through its settings.)
///
/// The backend, the constructs and the safe-labels table belong to other
/// features, so this function declares each as a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)): a forward declaration of
/// something another feature owns, with the construct's payload type, or the
/// table's value type, as its signature. A registry without the LaTeX features
/// therefore still builds, with the rules set aside (excluded from rule
/// selection). The safe-labels table is read optionally, so that a registry
/// holding the backend but not the `refs` feature still renders: the labels are
/// then written as the document wrote them.
///
/// # Errors
///
/// Never fails: the builder methods it calls report their problems when the
/// registry is built.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rule of a heading is the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct (LaTeX text written as it is), which the \
         recomposition rule stages for the newline after the LaTeX hook `\\flmHeading` and the \
         braces of the hook's label group",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the recomposition rule reads for \
         the safe label of each label the heading read from its `\\label` fields",
    )?;
    b.lowering_rule(latex_rule())?;
    b.lowering_rule(number_latex_rule())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_is_the_default_on_the_recomposed_backend_only() {
        let rule = latex_rule();
        assert_eq!(rule.name, LATEX_RULE);
        assert_eq!(rule.element, HEADING);
        assert_eq!(rule.decl.produces, [ElementName::from(MACRO), ElementName::from(RAW)]);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
        assert_eq!(rule.decl.options.len(), 2);
        // The labels are written from the heading's own fields, through the
        // safe-labels table where a feature of the registry declares it.
        assert!(rule.decl.reads.is_empty());
        assert_eq!(rule.decl.reads_opt.len(), 1);
    }
}
