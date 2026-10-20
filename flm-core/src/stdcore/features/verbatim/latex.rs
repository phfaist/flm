//! The recomposition rules of the [`verbatim`](super) feature for the
//! `latex-recomposed` backend: the five verbatim commands written back as LaTeX.
//!
//! Verbatim content must reach LaTeX unchanged, including inside a *moving
//! argument*, such as a section title or a caption, which LaTeX writes to an
//! auxiliary file and reads back, and where LaTeX's own `\verb` cannot be used.
//! Both rules therefore write what the `\EscVerb` command of the `fvextra`
//! package reads: an ordinary braced argument in which every printable ASCII
//! character other than a letter, a digit or `@` is preceded by a backslash
//! (`fvextra` rejects `\@`; see
//! [`escverb_encode`](crate::stdcore::backends::latex::sink::escverb_encode)). The
//! rules do not encode the content themselves: they stage (build into the
//! lowered tree; see [`Lowerer`]) the content
//! unchanged in an argument of the shape [`EscVerb`](LatexArgShape::EscVerb), and
//! the backend's emitter writes it in that encoding.
//!
//! | written | rule | LaTeX |
//! |---|---|---|
//! | `\verbtext{…}`, `\verbcode[lang]{…}`, `\verba[lang]{…}` | [`MACRO_LATEX_RULE`] | `\EscVerb[formatcom=\flmFmtVRB{<kind>}]{<content>}` |
//! | `{verbatimtext}` (inline) | [`ENVIRONMENT_LATEX_RULE`] | the same, with `<kind>` = `verbtext` |
//! | `{verbatimcode}[lang]` (a block) | [`ENVIRONMENT_LATEX_RULE`] | `\begin{flmVerbatimCode}[<lang>]`, the content on lines of its own, `\end{flmVerbatimCode}` |
//!
//! `<kind>` is `verbtext`, `verbcode` or `verba`, after the macro of the verbatim
//! kind. `\flmFmtVRB{<kind>}` runs that kind's formatting hook,
//! `\flmFmtVRB<kind>`, which a user redefines to change how that kind is set (see
//! [`verbatim`](super), *What a verbatim command becomes on each backend*). The
//! language of a block is written as the optional argument of `flmVerbatimCode`,
//! which hands it to the LaTeX hook `\flmVerbatimCodeSetup{<lang>}`. The language
//! of `\verbcode[lang]` and `\verba[lang]` is not written.
//!
//! The formatting hooks, the environment `flmVerbatimCode` and
//! `\flmVerbatimCodeSetup` are defined by the definition piece
//! [`VERBATIM`](crate::stdcore::backends::latex::preamble::VERBATIM)
//! (`flm.latex.def.verbatim`; a definition piece is a unit of LaTeX definitions
//! written to the style file when a rule requires it, see
//! [`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)). Both
//! rules require that piece and request the LaTeX package `fvextra`
//! ([`FVEXTRA`](packages::FVEXTRA); a package request, see [`packages`]).
//!
//! The option `formatcom=\flmFmtVRB{<kind>}` is staged as a raw node, a construct
//! element whose text the emitter writes byte for byte
//! ([`flm.pres.latex.raw`](crate::stdcore::backends::latex::construct::RAW)),
//! because it is a list of keys and values that LaTeX must read without a brace
//! group around it. Both rules are the default on the `latex-recomposed` backend
//! only.
//!
//! The feature does not own the backend or the three construct elements the
//! rules stage. [`register`] declares them as peer declarations
//! ([`PeerName`](crate::core::registry::PeerName)), so that a registry without
//! the LaTeX features still builds; the two rules are then set aside (excluded
//! from rule selection; see [`PeerName`](crate::core::registry::PeerName)).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::{Flm, FlmGroupType};
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{
    ARG_SLOT, ENVIRONMENT as LATEX_ENVIRONMENT, MACRO, RAW,
};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexEnvironmentPayload, LatexMacroPayload, LatexRawPayload, packages,
    preamble,
};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::{copy_subtree_verbatim, stage_verbatim_content};
use crate::stdcore::backends::pres::{BODY_SLOT, VerbatimKind};

use super::{CONTENT_ARG, ENVIRONMENT, LANG_ARG, MACRO as VERBATIM_MACRO, VerbatimCallable};

/// The name of the recomposition rule of `flm.verbatim.macro`,
/// `flm.verbatim.macro.lower.latex-recomposed`, whose body is
/// [`MacroLatexRule`].
pub const MACRO_LATEX_RULE: &str = "flm.verbatim.macro.lower.latex-recomposed";

/// The name of the recomposition rule of `flm.verbatim.environment`,
/// `flm.verbatim.environment.lower.latex-recomposed`, whose body is
/// [`EnvironmentLatexRule`].
pub const ENVIRONMENT_LATEX_RULE: &str = "flm.verbatim.environment.lower.latex-recomposed";

/// Records the two requirements of every verbatim command: the package
/// request `fvextra` and the definition piece `flm.latex.def.verbatim`.
fn require_verbatim(lw: &mut dyn Lowerer) {
    lw.require(&Name::from(packages::FVEXTRA));
    lw.require(&Name::from(preamble::VERBATIM));
}

/// Stages `\EscVerb[formatcom=\flmFmtVRB{<kind>}]{<content>}` over
/// `content`, for the verbatim kind `kind`.
fn escverb(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    kind: VerbatimKind,
    content: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let options = lw.pres_with(
        cx,
        &ElementName::from(RAW),
        LatexRawPayload::new(preamble::escverb_options(kind)),
        Vec::new(),
    )?;
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(preamble::ESCVERB, [LatexArgShape::Optional, LatexArgShape::EscVerb]),
        vec![(String::from(ARG_SLOT), vec![options]), (String::from(ARG_SLOT), content)],
    )
}

/// The body of [`MACRO_LATEX_RULE`]: `\EscVerb` over the characters of the
/// macro's content argument.
///
/// When the content argument is missing (a parse recovery left it absent and
/// the parse reported it), `\EscVerb` is written with an empty argument and
/// nothing more is reported. The rule returns a [`LowerError`] when the node
/// carries no [`VerbatimCallable`] or when a node could not be staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct MacroLatexRule;

impl Lower for MacroLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<VerbatimCallable>(&node)
            .ok_or_else(|| LowerError::structure("a verbatim node without its element's data"))?;
        let content = match node.argument_content_nodes_named(CONTENT_ARG) {
            Ok(Some(content)) => stage_verbatim_content(&node, content, cx, lw)?,
            _ => Vec::new(),
        };
        require_verbatim(lw);
        Ok(Lowering::Emit(vec![escverb(cx, lw, value.kind, content)?]))
    }
}

/// The body of [`ENVIRONMENT_LATEX_RULE`]: `\EscVerb` over the body of an
/// inline environment (`{verbatimtext}`), the environment `flmVerbatimCode`
/// over the body of a block (`{verbatimcode}`).
///
/// The body of a verbatim environment holds the group node of the verbatim
/// class that the parse puts around every verbatim run; the rule copies that
/// group's children, not the group itself. The rule returns a [`LowerError`]
/// when the node carries no [`VerbatimCallable`] or when a node could not be
/// staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvironmentLatexRule;

impl Lower for EnvironmentLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<VerbatimCallable>(&node)
            .ok_or_else(|| LowerError::structure("a verbatim node without its element's data"))?;
        let mut content = Vec::new();
        if let Some(body) = node.body() {
            for child in body.iter() {
                if child.group_type() == Some(FlmGroupType::Verbatim) {
                    content.extend(stage_verbatim_content(&node, child.children(), cx, lw)?);
                } else {
                    content.push(copy_subtree_verbatim(cx, child)?);
                }
            }
        }
        require_verbatim(lw);
        if !value.block {
            return Ok(Lowering::Emit(vec![escverb(cx, lw, value.kind, content)?]));
        }
        // The optional argument is always written, `[<lang>]` or `[]`.
        // LaTeX looks for an absent optional argument by reading ahead past
        // spaces, and the line end after `\begin{flmVerbatimCode}` is one:
        // without the brackets that lookahead swallows it, and `fancyvrb`,
        // which needs the verbatim content to begin on the next line, stops
        // with "Extraneous input ... between \begin{flmVerbatimCode}[<key=value>]
        // and line end". One shape for every block also makes a user's own
        // `flmVerbatimCode`, defined the natural way with an optional
        // argument, work.
        let shapes = vec![LatexArgShape::Optional];
        let mut slots = Vec::new();
        let lang = language_slot(&node, cx, lw)?;
        slots.push((String::from(ARG_SLOT), lang));
        slots.push((String::from(BODY_SLOT), content));
        let block = lw.pres_slots_with(
            cx,
            &ElementName::from(LATEX_ENVIRONMENT),
            LatexEnvironmentPayload::new(preamble::VERBATIM_CODE_ENVIRONMENT, shapes).verbatim(),
            slots,
        )?;
        Ok(Lowering::Emit(vec![block]))
    }
}

/// The content of the optional argument of `flmVerbatimCode`: the language
/// the document wrote, trimmed, or the content a user definition's template
/// wrote there.
///
/// A language the document writes is plain text. It is written as
/// [`language_of`](super::language_of) reads it, trimmed of the whitespace
/// around it, a blank language being no language at all; every other backend
/// reads it through the same function, so one document gives one language on
/// every backend.
///
/// A language argument holding anything other than text comes from the
/// template of a user definition kept as a LaTeX definition, which may place
/// one of the definition's arguments there (`\begin{verbatimcode}[#{lang}]`).
/// Such an argument is lowered as content, so that the placeholder is written
/// as the definition's parameter and the language reaches the environment
/// where the definition is called; read as text, it would be written as
/// nothing and the language would be lost. The rest of what the template
/// wrote in the argument is written as the template wrote it.
///
/// An absent argument gives an empty argument. The caller writes the brackets
/// in every case.
fn language_slot(
    node: &NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Vec<BuildId>, LowerError> {
    let Ok(Some(content)) = node.argument_content_nodes_named(LANG_ARG) else {
        return Ok(Vec::new());
    };
    if content.iter().all(|child| child.chars().is_some()) {
        return match super::language_of(node) {
            Some(lang) => Ok(vec![lw.text(cx, &lang)?]),
            None => Ok(Vec::new()),
        };
    }
    lw.lower_nodes(cx, content)
}

/// The two recomposition rules, ready to be registered: each produces the
/// macro, environment and raw constructs and is the default on the
/// `latex-recomposed` backend only.
pub fn latex_rules() -> Vec<LoweringRule> {
    let decl = || LoweringDecl::new().produces(MACRO).produces(LATEX_ENVIRONMENT).produces(RAW);
    let on_this_backend = || DefaultMark::OnBackends(vec![BACKEND.into()]);
    vec![
        LoweringRule::new(
            MACRO_LATEX_RULE,
            VERBATIM_MACRO,
            decl(),
            Arc::new(MacroLatexRule),
            on_this_backend(),
        )
        .docs(
            "a verbatim macro lowers to `\\EscVerb` over its content, in the encoding \
             `\\EscVerb` reads",
        ),
        LoweringRule::new(
            ENVIRONMENT_LATEX_RULE,
            ENVIRONMENT,
            decl(),
            Arc::new(EnvironmentLatexRule),
            on_this_backend(),
        )
        .docs(
            "an inline verbatim environment lowers to `\\EscVerb` over its content; a block \
             lowers to the `flmVerbatimCode` environment over its content",
        ),
    ]
}

/// Registers what the `verbatim` feature contributes to the
/// `latex-recomposed` backend: a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the backend and of the
/// macro, environment and raw constructs, each with its payload type, then
/// [`latex_rules`].
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
        "the latex-recomposed backend, on which the recomposition rules of the verbatim \
         commands are the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro construct, which the recomposition rules of the verbatim \
         commands stage",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        LATEX_ENVIRONMENT,
        "the LaTeX family's environment construct, which the recomposition rules of the \
         verbatim commands stage",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, which the recomposition rules of the verbatim \
         commands stage",
    )?;
    for rule in latex_rules() {
        b.lowering_rule(rule)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_rules_are_the_default_on_the_recomposed_backend_only() {
        let rules = latex_rules();
        assert_eq!(rules[0].name, MACRO_LATEX_RULE);
        assert_eq!(rules[0].element, VERBATIM_MACRO);
        assert_eq!(rules[1].name, ENVIRONMENT_LATEX_RULE);
        assert_eq!(rules[1].element, ENVIRONMENT);
        for rule in &rules {
            assert!(rule.default.is_specific_to(&BACKEND.into()));
            assert_eq!(rule.decl.produces.len(), 3);
        }
    }
}
