//! The recomposition rule of the [`annotations`](super) feature on the
//! `latex-recomposed` backend: an annotation is written back as the author
//! wrote it, over the LaTeX package `phfcc`, which defines a reviewer's
//! commands.
//!
//! | written | LaTeX |
//! |---|---|
//! | `\phf{a passage}` | `\phf{a passage}` |
//! | `\phf[a remark]` | `\phf[a remark]` |
//! | `\phf a passage\endphf` | `\phf a passage\endphf` |
//! | `\phf[one⏎⏎two]` | `\phf[one⏎⏎two]`, one invocation |
//!
//! The rendering guide describes the same output for readers of the
//! recomposed LaTeX
//! ([Annotations in the recomposed LaTeX](crate::guide::rendering#annotations-in-the-recomposed-latex)).
//!
//! # The invocation as written
//!
//! The rule [`LATEX_RULE`] ([`AnnotationLatexRule`]) writes the command as it
//! was written: the backslash and the name, and the whitespace that ended
//! the name; then the whitespace and source comments written between the
//! command and its opening `{` or `[`, the opening delimiter, the argument's
//! content lowered through the ordinary rules, and the closing delimiter:
//! `}`, `]`, or `\endphf` with the whitespace it read. A passage that was not
//! closed is written with no closing command, as it was written. The
//! delimiters are the ones the argument's group records (the annotation's
//! [form](super::form_of) is read from the same record), so that the three
//! forms come back as the author typed them. The command, the whitespace
//! after it and the delimiters are written as raw nodes
//! (`flm.pres.latex.raw`, LaTeX text written byte for byte).
//!
//! **An annotation across a paragraph break is one invocation.** The
//! `latex-recomposed` backend lowers the tree before the block-structure
//! pass, the pass that cuts an annotation into cut parts at its paragraph
//! breaks and blocks. An annotation written across a paragraph
//! break, or around a list, therefore reaches this rule as one node, with its
//! paragraph breaks and blocks inside its argument, and is written back as
//! typed: `\phf[A comment.⏎⏎With more.]` byte for byte, and `phfcc` colors
//! the whole of it.
//!
//! # The commands, made by `phfcc`
//!
//! The rule requires the package request `flm.latex.pkg.phfcc`
//! ([`packages::PHFCC`], with no option; a LaTeX package that a rule
//! requires by identifier, [`packages`]). It also writes into the [style
//! file](crate::stdcore::backends::latex::style),
//! from the configuration, one declaration per configured annotator, in
//! configuration order:
//!
//! ```latex
//! \ifcsname phf\endcsname\else\phfMakeCommentingCommand[initials={PhF}]{phf}\fi
//! \ifcsname abc\endcsname\else\phfMakeCommentingCommand[formatinitials=hide]{abc}\fi
//! ```
//!
//! The package assigns its colors in the order in which the commands are
//! made, so the declarations follow the annotators' numbers, the numbers by
//! which the HTML stylesheet chooses its colors, and every configured
//! annotator is declared, whether the document uses it or not. An annotator
//! with initials passes them with the key `initials`, written as text; an
//! annotator with none is made with `formatinitials=hide`, since the package
//! would otherwise show the command's name. Each declaration is guarded on
//! the command it makes ([`LatexDefinitionKind::Declaration`]): where the
//! author's preamble already made a command of that name, that command is
//! kept and the declaration does nothing. No LaTeX hook of the LaTeX family
//! is involved: the package defines the commands.
//!
//! # Output when every annotation is hidden
//!
//! When [`AnnotationsRenderOptions::hidden`] is true, the rule writes no command, no
//! package request and no declaration: a highlight lowers to its passage, and
//! a comment to nothing but the paragraph breaks it held, so that the text
//! around a comment written across a paragraph break remains two paragraphs.
//! Hiding does not use the package's own switch,
//! `\phfDisableCommentingCommands`: that switch makes every use of a
//! commenting command an error, for a final version that must contain none,
//! whereas hiding leaves the text clean.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::latexlike::InvocationSyntaxData;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::{structural, Flm};
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{DEFINITION, RAW};
use crate::stdcore::backends::latex::{
    packages, LatexDefinitionKind, LatexDefinitionPayload, LatexRawPayload,
};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::backends::pres::BODY_SLOT;

use super::config::{AnnotationsRenderOptions, Annotator};
use super::parser::{delimiters_of, form_of, Form};
use super::{AnnotationElement, ANNOTATION, FEATURE, TEXT_ARG};

/// The name of the lowering rule of an annotation on the `latex-recomposed`
/// backend, `flm.annotations.annotation.lower.latex-recomposed`
/// ([`AnnotationLatexRule`]).
pub const LATEX_RULE: &str = "flm.annotations.annotation.lower.latex-recomposed";

/// The name of the command of the `phfcc` package that makes a reviewer's
/// commenting command, `phfMakeCommentingCommand`.
pub const MAKE_COMMAND: &str = "phfMakeCommentingCommand";

/// The implementation of the lowering rule [`LATEX_RULE`], the default on the
/// `latex-recomposed` backend: it writes an annotation back as the author
/// wrote it, and stages the declarations of every configured annotator's
/// command for the style file. See the [module documentation](self).
///
/// # Errors
///
/// [`LowerError`] when the node carries no [`AnnotationElement`] or no
/// argument group, and
/// any error from reading the render option or lowering the content.
#[derive(Debug, Clone)]
pub struct AnnotationLatexRule {
    annotators: Arc<[Annotator]>,
}

impl AnnotationLatexRule {
    /// Returns the rule for the configured `annotators`, given in
    /// configuration order.
    pub fn new(annotators: impl Into<Arc<[Annotator]>>) -> AnnotationLatexRule {
        AnnotationLatexRule { annotators: annotators.into() }
    }

    /// Stages the declarations of every configured annotator's command, in
    /// configuration order, each guarded on the command's name.
    fn declarations(&self, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Vec<BuildId>, LowerError> {
        let mut staged = Vec::with_capacity(self.annotators.len());
        for annotator in self.annotators.iter() {
            let command = annotator.command.as_str();
            let body = match annotator.initials.as_deref() {
                Some(initials) => vec![
                    raw(cx, lw, &format!("\\{MAKE_COMMAND}[initials={{"))?,
                    lw.text(cx, initials)?,
                    raw(cx, lw, &format!("}}]{{{command}}}"))?,
                ],
                None => vec![raw(cx, lw, &format!("\\{MAKE_COMMAND}[formatinitials=hide]{{{command}}}"))?],
            };
            let payload = LatexDefinitionPayload::new(LatexDefinitionKind::Declaration, command, 0)
                .of_feature(FEATURE);
            staged.push(lw.pres_slots_with(
                cx,
                &ElementName::from(DEFINITION),
                payload,
                vec![(String::from(BODY_SLOT), body)],
            )?);
        }
        Ok(staged)
    }
}

impl Lower for AnnotationLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        if element_value::<AnnotationElement>(&node).is_none() {
            return Err(LowerError::structure("an annotation without its element's data"));
        }
        let form = form_of(&node).ok_or_else(|| LowerError::structure("an annotation without its argument"))?;
        if lw.option::<AnnotationsRenderOptions>()?.hidden {
            return Ok(Lowering::Emit(match form {
                Form::Highlight => lw.lower_arg_named(cx, &node, TEXT_ARG)?,
                Form::Comment => paragraph_breaks(&node, cx, lw)?,
            }));
        }
        let (open, close) =
            delimiters_of(&node).ok_or_else(|| LowerError::structure("an annotation without its argument"))?;
        lw.require(&Name::from(packages::PHFCC));
        // Every annotation stages the declarations of every configured
        // annotator; the emitter records each declaration once, so the style
        // file holds them in configuration order whatever the document uses
        // first.
        let mut staged = self.declarations(cx, lw)?;
        staged.push(raw(cx, lw, &command_as_written(&node))?);
        staged.extend(before_group(&node, cx, lw)?);
        if !open.is_empty() {
            staged.push(raw(cx, lw, open)?);
        }
        staged.extend(lw.lower_arg_named(cx, &node, TEXT_ARG)?);
        if !close.is_empty() {
            staged.push(raw(cx, lw, close)?);
        }
        Ok(Lowering::Emit(staged))
    }
}

/// Stages a raw node (`flm.pres.latex.raw`): LaTeX text written byte for
/// byte.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Returns the command of the annotation `node` as it was written: the escape
/// character and the name, followed by the whitespace that ended the name,
/// which belongs to the command (it separates the name from a passage that
/// begins with a letter, `\phf passage`).
fn command_as_written(node: &NodeRef<'_, Flm>) -> String {
    let name = node.name().unwrap_or_default();
    match node.invocation_syntax() {
        Some(InvocationSyntaxData::Macro { escape_char, post_space }) => {
            format!("{escape_char}{name}{}", post_space.resolve(node.span().source()))
        }
        _ => format!("\\{name}"),
    }
}

/// Lowers what is written in the argument before its group, the whitespace
/// and source comments between the command and an opening `{` or `[`, as the
/// recomposed LaTeX writes whitespace and source comments anywhere.
fn before_group(node: &NodeRef<'_, Flm>, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Vec<BuildId>, LowerError> {
    let Some(region) = node.arguments().and_then(|arguments| arguments.get_named(TEXT_ARG)?.region.as_ref())
    else {
        return Ok(Vec::new());
    };
    let group = region.content_parent();
    let noise: Vec<NodeRef<'_, Flm>> = node
        .argument_nodes_named(TEXT_ARG)
        .ok()
        .flatten()
        .map(|nodes| nodes.iter().take_while(|written| written.id() != group).collect())
        .unwrap_or_default();
    lower_run(cx, lw, &noise)
}

/// Lowers the paragraph breaks written directly in the argument of the
/// comment `node`: what a hidden comment leaves of itself, so that the text
/// around a comment written across a paragraph break remains two
/// paragraphs.
fn paragraph_breaks(node: &NodeRef<'_, Flm>, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Vec<BuildId>, LowerError> {
    let Some(content) = node.argument_content_nodes_named(TEXT_ARG).ok().flatten() else {
        return Ok(Vec::new());
    };
    let mut staged = Vec::new();
    for written in content.iter() {
        if written.ext().element.name.as_str() == structural::PARBREAK {
            staged.extend(lower_run(cx, lw, std::slice::from_ref(&written))?);
        }
    }
    Ok(staged)
}

/// Returns the lowering rule [`LATEX_RULE`], the default on the
/// `latex-recomposed` backend only, which declares the commands of
/// `annotators` in the style file.
pub fn latex_rule(annotators: impl Into<Arc<[Annotator]>>) -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        ANNOTATION,
        LoweringDecl::new()
            .option::<AnnotationsRenderOptions>()
            .produces(DEFINITION)
            .produces(RAW),
        Arc::new(AnnotationLatexRule::new(annotators)),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "an annotation lowers to its invocation as written (the command, the opening delimiter, \
         the content and the closing delimiter), one invocation even across a paragraph break, \
         since the backend lowers the tree before the block-structure pass; it requires the \
         `phfcc` package, and the style file makes every configured annotator's command with \
         `\\phfMakeCommentingCommand`, guarded on its name; when annotations are hidden \
         (`hidden`), a highlight lowers to its passage alone and a comment to its paragraph \
         breaks",
    )
}

/// Registers the lowering rule [`latex_rule`], the default on the
/// `latex-recomposed` backend only, after a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the backend and of each
/// construct element the rule stages, so that a registry without the LaTeX
/// features still builds, with the rule set aside (excluded from rule
/// selection; see [`PeerName`](crate::core::registry::PeerName)).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder.
pub(super) fn register(b: &mut FeatureBuilder<'_>, annotators: &[Annotator]) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the annotation's rule \
         `flm.annotations.annotation.lower.latex-recomposed` is the default",
    )?;
    b.peer_element::<LatexDefinitionPayload>(
        DEFINITION,
        "the LaTeX family's definition construct, which the annotation's rule stages for the \
         declaration of each annotator's command",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, in which the annotation's rule writes the command and \
         the delimiters as written",
    )?;
    b.lowering_rule(latex_rule(annotators.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_is_the_default_on_the_latex_recomposed_backend_only() {
        let rule = latex_rule(vec![Annotator::new("phf")]);
        assert_eq!(rule.element, ANNOTATION);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
    }
}
