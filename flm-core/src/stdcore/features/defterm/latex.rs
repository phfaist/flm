//! The lowering rules of the [`defterm`](super) feature for the
//! `latex-recomposed` backend, and the definition piece they require. A term
//! definition is written back as the environment `defterm` the author wrote,
//! its labels made to refer to it with the term as the text a reference
//! prints; the style file defines the environment from the configured
//! lead-in; and a term link is written as a LaTeX hook. The rendering guide
//! describes this output for users under [Defined terms in the recomposed
//! LaTeX](crate::guide::rendering#defined-terms-in-the-recomposed-latex).
//!
//! | written | LaTeX |
//! |---|---|
//! | `\begin{defterm}{Entropy}\label{topic:s}\aliasterm{Entropies} A measure. \end{defterm}` | `\begin{defterm}{Entropy}\phantomsection\flmLabelsWithDisplayText{Entropy}{\flmLabels{\label{ref1}\label{ref2}\label{ref3}}}\ignorespaces A measure.\end{defterm}` |
//! | the lead-in `#{term}: ` (the default) | `\ProvideDocumentEnvironment{defterm}{+m}{#1: }{}` in the style file |
//! | the lead-in `\textbf{Definition} (\emph{#{term}}): ` | `\ProvideDocumentEnvironment{defterm}{+m}{\textbf{Definition} (\emph{#1}): }{}` |
//! | no lead-in | `\ProvideDocumentEnvironment{defterm}{+m}{}{}` |
//! | `\term{entropy}`, `\term[entropy]{the entropy}` | `\flmTerm{ref1}{entropy}`, `\flmTerm{ref1}{the entropy}` |
//! | `\term{Entropy}` inside the definition of `Entropy` | `\flmTermInDefterm{Entropy}` |
//!
//! The rules use the LaTeX family's vocabulary: a **LaTeX hook** is a macro
//! that the recomposed LaTeX writes and the style file defines with a plain
//! default, which a user redefines in the document's preamble
//! ([`preamble`]; the rendering guide lists the hooks under [the LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks)); a **definition piece**
//! is a unit of LaTeX definitions that a rule requires by identifier and the
//! style file holds ([`LatexPreambleDef`]); a **package request** is a LaTeX
//! package that a rule or a definition piece requires by identifier ([`packages`]); and
//! a **safe label** is the key the recomposed LaTeX writes for an FLM label
//! (`ref1`, `ref2`, …; [`SafeLabels`]).
//!
//! # A term definition
//!
//! [`DEFINITION_LATEX_RULE`] writes a term definition as the environment
//! `defterm` ([`ENVIRONMENT`]), with the term, lowered as the content it is,
//! as its one argument. Its body opens with three things before the term
//! definition's own paragraphs:
//!
//! - `\phantomsection`, the `hyperref` command that places an anchor where no
//!   sectioning command is, so that a link to the term definition leads to it
//!   rather than to the place anchored last;
//! - `\flmLabelsWithDisplayText{<term>}{<labels>}`, the LaTeX family's
//!   single command for labels that refer to a place with no number and
//!   print a text of their own
//!   ([`label_display_text_piece`](crate::stdcore::backends::latex::preamble::label_display_text_piece)):
//!   here the term, so that `\ref` or `\zcref` to a label of the term
//!   definition prints the term. The labels are every label of the term
//!   definition's reference point, in its order (the label computed from the
//!   term, one computed from each alias, then the ones the author wrote),
//!   each under the safe label the references feature allocated for it,
//!   inside the labels hook `\flmLabels`, as every group of labels is written
//!   ([`stage_label`](crate::stdcore::features::refs::stage_label),
//!   [`stage_label_group`](crate::stdcore::features::refs::stage_label_group));
//! - `\ignorespaces`, which drops the line break after the labels, so that
//!   the term definition's text follows the lead-in with only the lead-in's
//!   own space.
//!
//! The term is written twice, once as the environment's argument and once as
//! the text a reference prints, so that the lead-in and a reference each have
//! it. With the render option
//! [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! off, the labels are written bare after `\phantomsection`, with no display
//! text, as every label is then: a reference to the term definition prints
//! what LaTeX recorded last, and giving it a text is left to the author.
//!
//! The term definition's fields, `\label{…}` and `\aliasterm{…}`, label it
//! and are not written anywhere else. A term definition whose term is empty is
//! reported as on every other backend ([`EmptyTerm`]).
//!
//! # The LaTeX definition of the environment
//!
//! The style file defines the environment `defterm` from the configured
//! lead-in ([`DeftermConfig::lead_in`](super::DeftermConfig::lead_in)). The
//! begin code is the lead-in template, instantiated once as on every other
//! output and lowered through the ordinary rules, with `#{term}` written as
//! the environment's parameter `#1`; the end code is empty; with no lead-in,
//! the begin code is empty too. The lead-in a compiled document shows
//! therefore follows the configuration, and a LaTeX author changes it by
//! defining `defterm` before loading the style file, or with
//! `\RenewDocumentEnvironment` after.
//!
//! The rule stages (builds into the lowered tree; see [`Lowerer`]) that LaTeX
//! definition at every term definition, as the
//! LaTeX family's definition construct written by this feature
//! ([`LatexDefinitionPayload::feature`]), which the style file holds once,
//! guarded, before the document's own user definitions. It is not part of a
//! definition piece, because a definition piece is fixed text and the begin
//! code is a template lowered through the rules.
//!
//! The feature's definition piece ([`definition_piece`],
//! `flm.latex.def.defterm`) requires what the recomposed term definition
//! uses: the `hyperref` package, which defines `\phantomsection`; the labels
//! hook; the display-text hook; and the `zref-clever` reference type that
//! makes a reference to such a label print its text alone. A recomposed
//! document that holds a term definition therefore loads `hyperref`, as one
//! that holds a hyperlink does; a document that loads it itself, with options
//! of its own, lists it in the render option
//! [`skip_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::skip_packages).
//!
//! # A term link
//!
//! [`TERM_LATEX_RULE`] writes a term link as one of two LaTeX hooks, which
//! the feature's definition piece defines and a LaTeX author may redefine.
//! The text written for the link is lowered as the content it is, as it is
//! for `\flmHyperref`:
//!
//! - `\flmTerm{<label>}{<shown>}` ([`TERM_COMMAND`]) anywhere but inside the
//!   definition of its own term, `<label>` being the safe label of the label
//!   the link targets; by default `\flmHyperref{#1}{#2}`, a link where the
//!   document loads `hyperref`;
//! - `\flmTermInDefterm{<shown>}` ([`TERM_IN_DEFTERM_COMMAND`]) inside the
//!   definition of its own term: the term being defined, set apart and linked
//!   to nothing; by default `\emph{#1}`.
//!
//! Which of the two is written is decided at lowering, from the structure of
//! the tree, by the function every output calls,
//! [`in_own_definition`](super::in_own_definition); LaTeX keeps no state of
//! its own for it. A term link to a term that no term definition carries is
//! given a safe label as every reference to nothing is, so the term's own
//! characters, markup included, never reach a label name, and LaTeX reports
//! an undefined reference. A term link whose term is empty is reported as on
//! every other output ([`EmptyTerm`]) and written with an empty label.
//!
//! A user definition whose template holds a term link with a placeholder in
//! its term (`\term[#1]{…}`, `\term{#1}`, `\term[\emph{#1}]{…}`) is not kept
//! as a LaTeX definition: the label such a link targets is computed from the
//! term, which a call supplies. The rule returns
//! [`LowerError::PlaceholderCharacters`] through the reading of source text at
//! lowering ([`spelling_at_lowering`]), and each use site is expanded instead,
//! its link written under the safe label of its own term. A term link with a
//! fixed term in the template of a kept definition (a user definition whose
//! use sites a plan leaves in the tree; see [kept
//! definitions](crate::stdcore::features::userdefs::kept)) is written as
//! anywhere else, under its safe label.
//!
//! # The term pointer
//!
//! A term pointer occurs only in the lead-in's instance, which this backend
//! lowers as the begin code of the environment's LaTeX definition. Its rule
//! ([`PointerLatexRule`]) writes the environment's parameter there, `#1`,
//! which LaTeX replaces by the term at each term definition.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    spelling_at_lowering,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{
    ARG_SLOT, BEGIN_SLOT, DEFINITION, END_SLOT, ENVIRONMENT as ENVIRONMENT_CONSTRUCT, MACRO, RAW,
};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload,
    LatexMacroPayload, LatexPreambleDef, LatexRawPayload, packages, preamble,
};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::lower_arg_named_or_nothing;
use crate::stdcore::backends::pres::{BODY_SLOT, spelling_identity};
use crate::stdcore::features::refs::{
    self, SAFE_LABELS, SafeLabels, VALUE_SAFE_LABELS, ValueSafeLabels, stage_spelling,
    target_spelling,
};

use super::FEATURE;
use super::diagnostics::EmptyTerm;
use super::elements::{
    DEFTERM, SHOWN_ARG, TERM, TERM_ARG, computed_labels, decide_term, written_term,
};
use super::lead_in::{LeadIn, TERM_POINTER, lead_in_instance};

/// The name of the term definition's rule on the `latex-recomposed` backend,
/// `flm.defterm.lower.latex-recomposed` ([`DefinitionLatexRule`]).
pub const DEFINITION_LATEX_RULE: &str = "flm.defterm.lower.latex-recomposed";

/// The name of the term link's rule on the `latex-recomposed` backend,
/// `flm.term.lower.latex-recomposed` ([`TermLatexRule`]).
pub const TERM_LATEX_RULE: &str = "flm.term.lower.latex-recomposed";

/// The name of the term pointer's rule on the `latex-recomposed` backend,
/// `flm.defterm.term-pointer.lower.latex-recomposed` ([`PointerLatexRule`]).
pub const POINTER_LATEX_RULE: &str = "flm.defterm.term-pointer.lower.latex-recomposed";

/// The LaTeX environment that a term definition is written as, `defterm`:
/// the name the author wrote, which the style file defines
/// ([`DefinitionLatexRule`]).
pub const ENVIRONMENT: &str = "defterm";

/// The command that ends the opening of a term definition's body,
/// `ignorespaces`: the spaces after it (the line break after the labels) are
/// dropped, so that the term definition's text follows the lead-in with only
/// the lead-in's own space.
const IGNORE_SPACES_COMMAND: &str = "ignorespaces";

/// The parameter that the term pointer is written as in the environment's
/// LaTeX definition, `#1`: the environment's one argument, the term.
pub const TERM_PARAMETER: &str = "#1";

/// The LaTeX hook that a term link is written as outside the definition of
/// its own term, `flmTerm`: `\flmTerm{<label>}{<shown>}`, `<label>` being a
/// safe label.
pub const TERM_COMMAND: &str = "flmTerm";

/// The LaTeX hook that a term link is written as inside the definition of
/// its own term, `flmTermInDefterm`: `\flmTermInDefterm{<shown>}`.
pub const TERM_IN_DEFTERM_COMMAND: &str = "flmTermInDefterm";

/// The name of the feature's definition piece, `defterm`; its identifier is
/// `flm.latex.def.defterm` ([`PIECE`]).
pub const PIECE_NAME: &str = "defterm";

/// The identifier of the feature's definition piece, `flm.latex.def.defterm`
/// ([`definition_piece`]).
pub const PIECE: &str = "flm.latex.def.defterm";

/// The feature's definition piece, `flm.latex.def.defterm` ([`PIECE`]). It
/// defines the two LaTeX hooks of a term link, `\flmTerm{<label>}{<shown>}`
/// (by default `\flmHyperref{#1}{#2}`) and `\flmTermInDefterm{<shown>}` (by
/// default `\emph{#1}`), each with `\ProvideDocumentCommand`, so that a
/// definition the user made first is kept; and what each hook contributes to
/// a PDF bookmark: its text alone. It requires what the hooks and a term
/// definition use: the `\flmHyperref` definition piece ([`preamble::HYPERREF`]); the
/// `hyperref` package ([`packages::HYPERREF`]), for `\phantomsection`; the
/// labels hook ([`preamble::LABELS`]); the display-text hook
/// ([`preamble::LABEL_DISPLAY_TEXT`]); and the `zref-clever` reference type
/// that makes a reference to such a label print its text alone
/// ([`preamble::ZREF_CLEVER_TYPE`]). The environment `defterm` itself is not
/// in the definition piece: the style file defines it from the configured lead-in (see
/// [the LaTeX definition of the
/// environment](self#the-latex-definition-of-the-environment)).
///
/// Redefine a hook, before loading the style file or after it, to write term
/// links another way.
pub fn definition_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% Defined terms. A term definition is written as the environment defterm,
% whose one argument is the term. This file defines the environment further
% below, in the part headed \"definitions written from the configuration of
% FLM features\"; the environment's opening text is the lead-in that the
% document was recomposed with. The body of a term definition opens with
% \\phantomsection, so that a link to the term definition leads to it, and
% with \\flmLabelsWithDisplayText, so that a reference to one of its labels
% prints the term.
% \\flmTerm{<label>}{<shown>}: <shown>, a link to the definition of a term,
%   <label> being a label of that definition, as \\flmHyperref takes it.
% \\flmTermInDefterm{<shown>}: <shown> written inside the definition of its
%   own term: the term being defined, set apart, with no link.
% Redefine either command to set term links another way.
\\ProvideDocumentCommand\\flmTerm{m m}{\\flmHyperref{#1}{#2}}
\\ProvideDocumentCommand\\flmTermInDefterm{m}{\\emph{#1}}
% What a term link contributes to a PDF bookmark: its text alone. The code
% below runs when the document begins (\\AtBeginDocument) and does nothing
% where hyperref is not loaded.
\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands
  \\pdfstringdefDisableCommands{%
    \\def\\flmTerm#1#2{#2}%
    \\def\\flmTermInDefterm#1{#1}%
  }\\fi}
",
        "`\\flmTerm{label}{shown}` and `\\flmTermInDefterm{shown}`: the LaTeX hooks of a term \
         link; and the package and definition pieces that a term definition requires",
    )
    .requires([
        preamble::HYPERREF,
        packages::HYPERREF,
        preamble::LABELS,
        preamble::LABEL_DISPLAY_TEXT,
        preamble::ZREF_CLEVER_TYPE,
    ])
}

/// The lowering rule [`DEFINITION_LATEX_RULE`], the default on the
/// `latex-recomposed` backend: the environment's LaTeX definition from the
/// lead-in, then the environment around `\phantomsection`, the labels with
/// the term as their display text, `\ignorespaces` and the body. See the
/// [module documentation](self#a-term-definition).
#[derive(Debug)]
pub struct DefinitionLatexRule {
    lead_in: Arc<LeadIn>,
}

impl Lower for DefinitionLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let term = node.argument_content_nodes_named(TERM_ARG).ok().flatten();
        if term.is_none_or(|content| spelling_identity(content.iter()).is_empty()) {
            lw.error(EmptyTerm { what: String::from("term definition") }, node.span().clone());
        }
        lw.require(&Name::from(PIECE));
        let definition = environment_definition(&self.lead_in, &node, cx, lw)?;
        // Every label of the point, in its order: the computed ones, then the
        // written ones, each under its safe key.
        let mut labels = Vec::new();
        for label in computed_labels(&node) {
            labels.push(refs::stage_label(cx, lw, &label.to_string(), Some(&label))?);
        }
        for written in refs::label_uses(&node) {
            labels.push(refs::stage_label(cx, lw, &written.text, written.label.as_ref())?);
        }
        let hooks = lw.option::<LatexRecomposedRenderOptions>()?.label_hooks;
        let labels = refs::stage_label_group(cx, lw, labels, true)?;
        let phantom = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(preamble::PHANTOMSECTION_COMMAND, []),
            Vec::new(),
        )?;
        let mut body = vec![phantom];
        if hooks {
            let shown = lower_arg_named_or_nothing(cx, lw, &node, TERM_ARG)?;
            body.push(refs::stage_labels_with_display_text(cx, lw, shown, labels)?);
        } else {
            body.extend(labels);
        }
        // The begin code ends with the lead-in, whose default ends in a space,
        // and the anchor, the labels and the line break after them would add
        // a second space before the definition's text: `\ignorespaces` drops
        // the spaces that follow it, so one space stands after the lead-in.
        body.push(lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(IGNORE_SPACES_COMMAND, []),
            Vec::new(),
        )?);
        body.extend(lw.lower_body(cx, &node)?);
        let term = lower_arg_named_or_nothing(cx, lw, &node, TERM_ARG)?;
        let environment = lw.pres_slots_with(
            cx,
            &ElementName::from(ENVIRONMENT_CONSTRUCT),
            LatexEnvironmentPayload::new(ENVIRONMENT, [LatexArgShape::Mandatory]),
            vec![(String::from(ARG_SLOT), term), (String::from(BODY_SLOT), body)],
        )?;
        Ok(Lowering::Emit(vec![definition, environment]))
    }
}

/// Stages the LaTeX definition of the environment [`ENVIRONMENT`] for the
/// style file: one argument; its begin code the lead-in's instance lowered
/// through the rules ([`lead_in_instance`]), with the pointer written as
/// [`TERM_PARAMETER`], or nothing where no lead-in is configured; its end
/// code empty. As a definition written by a feature, it is guarded.
fn environment_definition(
    lead_in: &LeadIn,
    node: &NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<BuildId, LowerError> {
    let begin = match lead_in_instance(lead_in, node, lw)? {
        Some(instance) => lw.splice(cx, &instance)?,
        None => Vec::new(),
    };
    let payload = LatexDefinitionPayload::new(LatexDefinitionKind::Environment, ENVIRONMENT, 1)
        .of_feature(FEATURE);
    lw.pres_slots_with(
        cx,
        &ElementName::from(DEFINITION),
        payload,
        vec![(String::from(BEGIN_SLOT), begin), (String::from(END_SLOT), Vec::new())],
    )
}

/// The term definition's rule on the `latex-recomposed` backend,
/// [`DEFINITION_LATEX_RULE`], writing the environment's begin code from
/// `lead_in`.
fn definition_latex_rule(lead_in: Arc<LeadIn>) -> LoweringRule {
    LoweringRule::new(
        DEFINITION_LATEX_RULE,
        DEFTERM,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .option::<LatexRecomposedRenderOptions>()
            .produces(DEFINITION)
            .produces(ENVIRONMENT_CONSTRUCT)
            .produces(MACRO)
            .produces(RAW),
        Arc::new(DefinitionLatexRule { lead_in }),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a term definition lowers to the environment `defterm` around its term, its body \
         opening with `\\phantomsection` and `\\flmLabelsWithDisplayText` over its labels; the \
         style file defines the environment from the lead-in template",
    )
}

/// The lowering rule [`TERM_LATEX_RULE`], the default on the
/// `latex-recomposed` backend: `\flmTerm` around the safe label of the label
/// the term link targets and the text written for it, or
/// `\flmTermInDefterm` around that text inside the definition of its own
/// term. See the [module documentation](self#a-term-link).
#[derive(Debug, Default, Clone, Copy)]
pub struct TermLatexRule;

impl Lower for TermLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let shown = lower_arg_named_or_nothing(cx, lw, &node, SHOWN_ARG)?;
        let Some(decision) = decide_term(&node) else {
            // The target is computed from the term's spelling, not its
            // characters: see [`spelling_at_lowering`].
            if let Some(written) = written_term(&node) {
                spelling_at_lowering(written.iter())?;
            }
            // An empty term names nothing: reported, and written as a
            // reference naming nothing is, with an empty label.
            lw.error(EmptyTerm { what: String::from("term link") }, node.span().clone());
            return Ok(Lowering::Emit(vec![term_hook(cx, lw, Vec::new(), shown)?]));
        };
        if decision.in_own_definition {
            lw.require(&Name::from(PIECE));
            let hook = lw.pres_slots_with(
                cx,
                &ElementName::from(MACRO),
                LatexMacroPayload::new(TERM_IN_DEFTERM_COMMAND, [LatexArgShape::Mandatory]),
                vec![(String::from(ARG_SLOT), shown)],
            )?;
            return Ok(Lowering::Emit(vec![hook]));
        }
        let target = decision.target;
        let spelling = target_spelling(lw, &target.to_string(), Some(&target))?;
        let key = stage_spelling(cx, lw, &spelling)?;
        Ok(Lowering::Emit(vec![term_hook(cx, lw, key, shown)?]))
    }
}

/// Stages `\flmTerm{<key>}{<shown>}`, `key` being the safe label, and
/// requires the definition piece that defines the hook.
fn term_hook(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    key: Vec<BuildId>,
    shown: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    lw.require(&Name::from(PIECE));
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(TERM_COMMAND, [LatexArgShape::Mandatory, LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), key), (String::from(ARG_SLOT), shown)],
    )
}

/// The term link's rule on the `latex-recomposed` backend,
/// [`TERM_LATEX_RULE`].
fn term_latex_rule() -> LoweringRule {
    LoweringRule::new(
        TERM_LATEX_RULE,
        TERM,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .reads_opt(VALUE_SAFE_LABELS)
            .produces(MACRO)
            .produces(RAW),
        Arc::new(TermLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a term link lowers to `\\flmTerm` around its target's safe label and its text, or, \
         inside the definition of its own term, to `\\flmTermInDefterm` around its text",
    )
}

/// The lowering rule [`POINTER_LATEX_RULE`], the default on the
/// `latex-recomposed` backend: the environment's parameter
/// [`TERM_PARAMETER`], as a raw node (`flm.pres.latex.raw`). A term pointer
/// occurs only in the lead-in's instance, which this backend lowers as the
/// begin code of the environment's LaTeX definition, where the term is that
/// parameter.
#[derive(Debug, Default, Clone, Copy)]
pub struct PointerLatexRule;

impl Lower for PointerLatexRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let parameter = lw.pres_with(
            cx,
            &ElementName::from(RAW),
            LatexRawPayload::new(TERM_PARAMETER),
            Vec::new(),
        )?;
        Ok(Lowering::Emit(vec![parameter]))
    }
}

/// Registers the three rules, each the default on the `latex-recomposed`
/// backend only, and contributes the definition piece to the service slot of
/// definition pieces ([`preamble::SLOT`]). The names they use that other
/// features own (the backend, the LaTeX family's construct elements, the
/// safe-labels tables, the service slot of definition pieces) are declared through
/// peer declarations ([`PeerName`](crate::core::registry::PeerName)), so that
/// a registry without them still builds, with these rules set aside
/// (excluded from rule selection).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder.
pub(super) fn register(b: &mut FeatureBuilder<'_>, lead_in: Arc<LeadIn>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the feature's LaTeX rules are the default",
    )?;
    b.peer_element::<LatexDefinitionPayload>(
        DEFINITION,
        "the LaTeX family's definition construct, which the term definition's rule stages for \
         the environment's LaTeX definition",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        ENVIRONMENT_CONSTRUCT,
        "the LaTeX family's environment construct, which the term definition's rule stages",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the rules stage for the hooks and labels",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, in which the term pointer writes the \
         environment's parameter, and a term link a label whose label type the references \
         feature marks as raw (written byte for byte)",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the term definition's and the term \
         link's rules read for the safe label written for a label",
    )?;
    b.peer_table::<ValueSafeLabels>(
        VALUE_SAFE_LABELS.name(),
        "the references feature's value names, which the term link's rule reads for the safe \
         label of the target of a term link inside a synthesized value",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    let on_latex = || DefaultMark::OnBackends(vec![BACKEND.into()]);
    b.lowering_rule(definition_latex_rule(lead_in))?;
    b.lowering_rule(term_latex_rule())?;
    b.lowering_rule(
        LoweringRule::new(
            POINTER_LATEX_RULE,
            TERM_POINTER,
            LoweringDecl::new().produces(RAW),
            Arc::new(PointerLatexRule),
            on_latex(),
        )
        .docs(
            "a term pointer lowers to the environment's parameter `#1`: it occurs only in the \
             lead-in written as the begin code of the environment's LaTeX definition",
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_is_named_as_its_identifier_says_and_requests_what_a_definition_uses() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.contains(&Name::from(packages::HYPERREF)));
        assert!(piece.requires.contains(&Name::from(preamble::LABEL_DISPLAY_TEXT)));
        assert!(piece.requires.contains(&Name::from(preamble::ZREF_CLEVER_TYPE)));
        assert!(piece.requires.contains(&Name::from(preamble::HYPERREF)));
        for hook in [TERM_COMMAND, TERM_IN_DEFTERM_COMMAND] {
            assert!(piece.content.contains(&format!("\\ProvideDocumentCommand\\{hook}")));
        }
    }
}
