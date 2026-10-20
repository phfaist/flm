//! The stylesheet of the [`annotations`](super) feature on the `html`
//! backend, `flm.annotations.style` ([`stylesheet`]), and the markup it
//! selects on.
//!
//! On the `html` backend an annotation is lowered by the general rule
//! [`AnnotationRule`](super::AnnotationRule), into the shared presentation
//! vocabulary; the rule also requires (records a
//! requirement for) the feature's style identifier [`STYLE`]
//! (`flm.annotations.style`), which on the `html` backend names this
//! stylesheet, a style definition ([`StyleDef`]) that the backend delivers
//! with the page
//! ([style and script definitions](crate::guide::concepts_overview#style-and-script-definitions)).
//! The HTML itself comes from the backend's rules for the presentation
//! elements, the same for every feature
//! ([the HTML contract](crate::guide::rendering#the-html-contract)):
//!
//! - a **highlight** is a span of role `flm.annotations.highlight` in the flow
//!   of the text, whose kind tokens are the annotator's command and number;
//!   the annotator's initials are at its start, in a margin decoration
//!   written where its decoration mark is:
//!
//!   ```html
//!   <span class="flm-pres-span flm-annotations-highlight" data-flm-kind="phf annotator-0"><span class="flm-pres-deco-margin"><span class="flm-pres-span flm-annotations-initials" data-flm-kind="phf annotator-0">PhF</span></span>a passage</span>
//!   ```
//!
//! - a **comment** is the same under the role `flm.annotations.comment`;
//! - a **cut part around a block** (a list inside a highlighted passage) is a
//!   `<div>` of the role, with the initials, when it is the first cut part, at
//!   the start of the `<div>`;
//! - a **cut part of a cut annotation** also carries the kind token
//!   `mark-continues` when a cut part follows it and `mark-continued` when one
//!   precedes it ([`MARK_CONTINUES_KIND`](super::MARK_CONTINUES_KIND),
//!   [`MARK_CONTINUED_KIND`](super::MARK_CONTINUED_KIND)); an annotation that
//!   was not cut carries neither:
//!
//!   ```html
//!   <p class="flm-pres-par">Some text <span class="flm-pres-span flm-annotations-comment" data-flm-kind="phf annotator-0 mark-continues"><span class="flm-pres-deco-margin"><span class="flm-pres-span flm-annotations-initials" data-flm-kind="phf annotator-0">PhF</span></span>A comment.</span></p>
//!   <p class="flm-pres-par"><span class="flm-pres-span flm-annotations-comment" data-flm-kind="phf annotator-0 mark-continued">With a second paragraph.</span> More text.</p>
//!   ```
//!
//! The rendering guide describes the same markup
//! ([Annotations in HTML](crate::guide::rendering#annotations-in-html)).
//!
//! # The stylesheet
//!
//! The stylesheet [`stylesheet`] (`flm.annotations.style`) is one cascade
//! layer. It holds the **palette**, one custom property per annotator number,
//! `--flm-annotation-color-0` to `--flm-annotation-color-7`: eight colors,
//! those that the `phfcc` LaTeX package presets, in its order, so that an
//! annotator has the same color on the page as in the recomposed LaTeX
//! ([`latex`](super::latex)). An annotation and its initials select their
//! color by the kind token `annotator-<n>` (`[data-flm-kind~="annotator-0"]`).
//! Every annotation first takes the fallback color,
//! `--flm-annotation-color-fallback`, and a number within the palette then
//! takes its own color, so that an annotator numbered eight or more takes the
//! fallback: red, the color `phfcc` gives an annotator once its presets run
//! out ([`PALETTE_SIZE`]).
//!
//! A **highlight**'s text is in the color; its background is tinted with the
//! color only where a page turns the tint on
//! (`--flm-annotation-highlight-tint`, `0%` by default). A **comment** is in
//! the color, in the annotations' font family (`--flm-annotation-family`,
//! sans-serif by default) and in a heavier weight
//! (`--flm-annotation-comment-weight`), between brackets that the stylesheet
//! draws: the opening bracket on a cut part not marked `mark-continued`, the
//! closing bracket on a cut part not marked `mark-continues`, so that a cut
//! comment is bracketed once. A cut part around a block carries its bracket
//! on a line of its own, above or below the block. The **initials** are
//! smaller than the note they are shown in, upright, of normal weight, in the
//! annotations' font family, in a thin box of the color; they are shown in
//! the margin decoration, which the HTML backend's core stylesheet
//! ([`STYLE_CORE`](crate::stdcore::backends::html::conventions::STYLE_CORE))
//! places beside the text. Every color and length is a custom property that the
//! stylesheet declares, which a page sets to change the look without writing
//! a rule; every selector begins with a class of the HTML contract.
//!
//! The general rule requires the stylesheet wherever it writes an
//! annotation, so that a page holding none carries none of it; when the
//! render option [`hidden`](super::AnnotationsRenderOptions::hidden) is true,
//! it requires nothing, since nothing of an annotation's own reaches the page.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

/// The identifier of the feature's stylesheet, `flm.annotations.style`
/// ([`stylesheet`]), which the general rule
/// [`AnnotationRule`](super::AnnotationRule) requires at every annotation it
/// writes.
pub const STYLE: &str = "flm.annotations.style";

/// How many annotator numbers the stylesheet's palette gives a color of their
/// own: eight, `--flm-annotation-color-0` to `--flm-annotation-color-7`; an
/// annotator with a higher number takes the fallback color,
/// `--flm-annotation-color-fallback`.
pub const PALETTE_SIZE: usize = 8;

/// Returns the feature's stylesheet, `flm.annotations.style`, as a style
/// definition: the palette and the look of a highlight, a comment and the
/// initials, in one cascade layer, every color and length a custom property
/// it declares. See the [module documentation](self#the-stylesheet).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/annotations.css")))],
        docs: String::from(
            "a reviewer's highlights and comments in the annotator's color, and the initials beside the line",
        ),
    }
}

/// Registers what the feature contributes to the `html` backend: the
/// stylesheet ([`stylesheet`]).
///
/// # Errors
///
/// Never fails: the declaration it makes ([`FeatureBuilder::style_def`]) is
/// checked when the registry is built.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.style_def(stylesheet())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stylesheet_names_itself_and_is_one_layer() {
        let style = stylesheet();
        assert_eq!(style.id.as_str(), STYLE);
        let [DefContent::Inline(css)] = style.content.as_slice() else { panic!("one inline part") };
        assert_eq!(css.lines().next(), Some("/* flm.annotations.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }

    #[test]
    fn the_palette_holds_one_color_per_number_and_the_fallback_is_phfcc_s() {
        let style = stylesheet();
        let [DefContent::Inline(css)] = style.content.as_slice() else { panic!("one inline part") };
        for number in 0..PALETTE_SIZE {
            assert!(css.contains(&format!("--flm-annotation-color-{number}: ")), "{number}");
            assert!(css.contains(&format!("[data-flm-kind~=\"annotator-{number}\"]")), "{number}");
        }
        assert!(!css.contains(&format!("--flm-annotation-color-{PALETTE_SIZE}:")));
        assert!(css.contains("--flm-annotation-color: var(--flm-annotation-color-fallback);"));
        // `phfcc` falls back to xcolor's `red`.
        assert!(css.contains("--flm-annotation-color-fallback: rgb(255 0 0);"));
    }
}
