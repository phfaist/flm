//! The `textstyles` feature: the inline formatting commands `\emph`, `\textbf`,
//! `\textit` and `\texttt`.
//!
//! The guide to the language describes these commands for authors
//! ([inline formatting](crate::guide::language_syntax#inline-formatting)).
//!
//! # What an author writes
//!
//! Each command takes one mandatory argument, its content, and marks that content
//! with one inline style: `\emph{…}` is semantic emphasis, `\textbf{…}` bold
//! weight, `\textit{…}` italic shape and `\texttt{…}` a monospaced face. The
//! content is ordinary document content (text, references, mathematics, other
//! formatting commands). A command is inline: it is part of the paragraph it is
//! written in and keeps the whitespace around it. The commands are declared in
//! text mode only; inside mathematics, the [`math`](crate::stdcore::features::math)
//! feature declares its own `\textbf`, `\textit` and `\texttt` (see
//! [`math::text`](crate::stdcore::features::math::text)).
//!
//! # What the feature registers
//!
//! The `textstyles` feature ([`TextStyles`], a [`FlmFeature`]) registers:
//!
//! - the element type [`TEXT_STYLE`] (`flm.textstyle`): the four commands form
//!   one family, declared as one element type with one element value per command
//!   ([elements and
//!   aspects](crate::guide::concepts_overview#elements-and-aspects)): each
//!   element value carries a [`TextStyleElement`], whose [`TextStyle`] says which
//!   style the command applies;
//! - the lowering rule [`RULE`] (`flm.textstyle.lower.default`), the default on
//!   every backend, and the recomposition rule of [`latex`] for the
//!   `latex-recomposed` backend.
//!
//! [`TextStyle`] is the feature's own list of styles. The presentation elements
//! use a separate list, [`SpanStyle`]; the lowering rule converts one into the
//! other ([`TextStyle::span_style`]), so that the element never refers to a
//! presentation type.
//!
//! # What a formatting command becomes on each backend
//!
//! - **Default rule** ([`RULE`], used by the `text` and `html` backends): the
//!   rule stages (builds into the lowered tree; see
//!   [`Lowerer`]) a
//!   [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) node whose payload
//!   carries the [`SpanStyle`], over the argument lowered through the rules.
//! - **`text` backend**: it prints the span's content; the style has no form in
//!   plain text and is left out.
//! - **`html` backend**: it writes the span as the [HTML
//!   contract](crate::guide::rendering#the-html-contract) states for
//!   `flm.pres.span` ([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)):
//!   an `<em>`, `<strong>`, `<i>` or `<code>` element by the style.
//! - **`latex-recomposed` backend**: the rule of [`latex`] writes the command
//!   back as the LaTeX macro of the same name.
//!
//! A backend that has no way to show a style writes the content without it.
//!
//! # Map of the module
//!
//! - [`TextStyles`] — the feature; [`feature`] returns it for a package entry.
//! - [`TEXT_STYLE`], [`TextStyle`], [`TextStyleElement`] — the element type and
//!   the data of each element value.
//! - [`RULE`], [`TextStyleRule`] — the default lowering rule and its body.
//! - [`latex`] — the recomposition rule for the `latex-recomposed` backend.
//! - [`literals`](crate::stdcore::features::literals) — the sibling feature that
//!   declares the commands standing for one character.

// The command written for a value is what tells the four apart in a dump, since
// the element type is one; see decision rationale at
// [§flm-dd-dr:std-element-families] and [§flm-dd-dr:std-own-vocabulary].

pub mod latex;

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::model::names::ElementName;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::Flm;
use crate::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use crate::stdcore::backends::lowering::lower_arg_or_nothing;
use crate::stdcore::backends::pres::{self, SpanPayload, SpanStyle};

/// The feature's registered name, `textstyles`.
pub const FEATURE: &str = "textstyles";

/// The name of the element type of the four formatting commands,
/// `flm.textstyle`. The data of each element value is a
/// [`TextStyleElement`].
pub const TEXT_STYLE: &str = "flm.textstyle";

/// The name of the default lowering rule of [`TEXT_STYLE`] on every backend,
/// `flm.textstyle.lower.default`, whose body is [`TextStyleRule`].
pub const RULE: &str = "flm.textstyle.lower.default";

/// The inline style a formatting command applies: one value per command of the
/// `textstyles` feature.
///
/// [`TextStyle::command`] gives the command's name and
/// [`TextStyle::span_style`] the [`SpanStyle`] the lowering rule writes into the
/// span it stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextStyle {
    /// Semantic emphasis (`\emph`).
    Emphasis,
    /// Bold weight (`\textbf`).
    Bold,
    /// Italic shape (`\textit`).
    Italic,
    /// A monospaced face (`\texttt`).
    Monospace,
}

impl TextStyle {
    /// Every style, in the order the feature registers the commands in.
    pub const ALL: [TextStyle; 4] =
        [TextStyle::Emphasis, TextStyle::Bold, TextStyle::Italic, TextStyle::Monospace];

    /// The command name written for this style, without the leading backslash
    /// (`"emph"`, `"textbf"`, `"textit"`, `"texttt"`).
    pub fn command(self) -> &'static str {
        match self {
            TextStyle::Emphasis => "emph",
            TextStyle::Bold => "textbf",
            TextStyle::Italic => "textit",
            TextStyle::Monospace => "texttt",
        }
    }

    /// The [`SpanStyle`] of the presentation elements that corresponds to this
    /// style: what [`TextStyleRule`] writes into the payload of the span it
    /// stages.
    pub fn span_style(self) -> SpanStyle {
        match self {
            TextStyle::Emphasis => SpanStyle::Emphasis,
            TextStyle::Bold => SpanStyle::Bold,
            TextStyle::Italic => SpanStyle::Italic,
            TextStyle::Monospace => SpanStyle::Monospace,
        }
    }
}

/// The data of one element value of `flm.textstyle`: the style that the
/// command applies.
///
/// A lowering rule or a stage reads it from a node with [`element_value`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyleElement {
    /// The style the command applies.
    pub style: TextStyle,
}

/// The element value of one formatting command: a `flm.textstyle` element
/// carrying `style` and the default inline block role.
fn element(style: TextStyle) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(TEXT_STYLE)
            .docs(match style {
                TextStyle::Emphasis => "Semantic emphasis of its content.",
                TextStyle::Bold => "Its content in bold weight.",
                TextStyle::Italic => "Its content in italic shape.",
                TextStyle::Monospace => "Its content in a monospaced face.",
            })
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with_value(Arc::new(TextStyleElement { style })),
    )
}

/// The body of [`RULE`]: it stages a
/// [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) node whose payload
/// carries the command's [`SpanStyle`] ([`TextStyle::span_style`]), over the
/// command's argument lowered through the rules.
///
/// When the argument is missing (a parse recovery left it absent and the
/// parse reported it), the span is empty and nothing more is reported.
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`TextStyleElement`], or when the argument or the span could not be
/// lowered or staged.
#[derive(Debug, Default)]
pub struct TextStyleRule;

impl Lower for TextStyleRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<TextStyleElement>(&node)
            .ok_or_else(|| LowerError::structure("a text-style node without its value"))?;
        // An argument a parse recovery left absent lowers to nothing: an empty
        // span, and no second report of what the parse already reported.
        let body = lower_arg_or_nothing(cx, lw, &node, 0)?;
        let payload =
            SpanPayload { style: Some(value.style.span_style()), ..SpanPayload::default() };
        let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?;
        Ok(Lowering::Emit(vec![span]))
    }
}

/// The `textstyles` feature: it registers the four formatting commands, their
/// default lowering rule and the recomposition rule of [`latex`]. See the
/// module documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextStyles;

impl FlmFeature for TextStyles {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Text styles").docs(
            "The inline formatting commands `\\emph`, `\\textbf`, `\\textit` and `\\texttt`: \
             four values of the element type `flm.textstyle`, each lowered to a \
             `flm.pres.span` node that carries its style.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        for style in TextStyle::ALL {
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [style.command()],
                    SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
                    element(style),
                )
                .docs(format!(
                    "`\\{}{{…}}`: {:?} formatting of its content.",
                    style.command(),
                    style
                )),
            )?;
        }
        b.lowering_rule(
            LoweringRule::new(
                RULE,
                TEXT_STYLE,
                LoweringDecl::new().produces(pres::SPAN),
                Arc::new(TextStyleRule),
                DefaultMark::Everywhere,
            )
            .docs("a formatting command lowers to `flm.pres.span` carrying its style"),
        )?;
        latex::register(b)?;
        Ok(())
    }
}

/// The [`TextStyles`] feature as a shared value, ready to be listed in a
/// package entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(TextStyles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_names_its_command_and_span_style() {
        assert_eq!(TextStyle::Emphasis.command(), "emph");
        assert_eq!(TextStyle::Monospace.span_style(), SpanStyle::Monospace);
        assert_eq!(TextStyle::ALL.len(), 4);
    }

    #[test]
    fn an_element_value_carries_its_style_and_the_inline_role() {
        let element = element(TextStyle::Bold);
        assert_eq!(element.name, TEXT_STYLE);
        assert_eq!(element.value_as::<TextStyleElement>().map(|v| v.style), Some(TextStyle::Bold));
        assert!(element.has_aspect::<dyn BlockRoleAspect>());
    }
}
