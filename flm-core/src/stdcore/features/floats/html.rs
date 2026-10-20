//! The stylesheet of the [`floats`](super) feature on the `html` backend,
//! `flm.floats.style` ([`stylesheet`]), and the markup it selects on.
//!
//! On the `html` backend a float is lowered by the general rule
//! ([`FloatRule`](super::FloatRule)), which requires the feature's style
//! identifier [`STYLE`] (`flm.floats.style`) at every float; on this backend
//! the identifier names this stylesheet, so that every page that holds a
//! float carries the stylesheet and a page that holds none does not.
//!
//! # The markup
//!
//! The HTML backend writes the blocks the general rule stages from its tag
//! table ([`BLOCK_TAGS`](crate::stdcore::backends::html::conventions::BLOCK_TAGS)).
//! The float is a block of role `flm.floats.float`, written as a `<figure>`,
//! carrying the float type's name and `numbered` or `unnumbered` as its kind
//! tokens and the anchor of its reference point as its `id`. Its content is a
//! `<div>` of role `flm.floats.float.content`. The line under the content,
//! where the float has one, is a block of role `flm.floats.float.caption`,
//! written as a `<figcaption>` and placed last in the `<figure>` regardless of
//! the order in the source; its one paragraph holds the number in a span of
//! role `flm.floats.float.number`:
//!
//! ```html
//! <figure class="flm-pres-block flm-floats-float" id="figure-1" data-flm-kind="figure numbered">
//!   <div class="flm-pres-block flm-floats-float-content"><img class="flm-pres-block flm-pres-image" src="plot.png"></div>
//!   <figcaption class="flm-pres-block flm-floats-float-caption" data-flm-kind="figure numbered">
//!     <p class="flm-pres-par"><span class="flm-pres-span flm-floats-float-number">Figure&#xA0;1</span>: A photo</p>
//!   </figcaption>
//! </figure>
//! ```
//!
//! The classes and attributes follow the HTML contract, the classes and
//! attributes a stylesheet may rely on
//! ([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS); [the HTML
//! contract](crate::guide::rendering#the-html-contract)). The HTML backend
//! writes a float with the tags and classes it writes for every block,
//! derived from the block's role and kind tokens. A stylesheet selects every
//! float by the role's class `flm-floats-float`, one float type by its kind
//! token in `data-flm-kind`, and an unnumbered float's line by the kind tokens
//! of the caption block, which repeat the float type's name and
//! `unnumbered`. The rendering
//! guide shows the result ([floats in
//! HTML](crate::guide::rendering#floats-in-html)).
//!
//! # The stylesheet
//!
//! [`stylesheet`] returns the style definition `flm.floats.style`. A style
//! definition ([`StyleDef`]) is a stylesheet that a feature's output depends
//! on, recorded in the registry under an identifier; a lowering rule or an
//! emitter requires it by identifier, and the backend that writes the output
//! decides its delivery: written into the output, loaded from an address, or
//! left to the host. The
//! stylesheet sets a float apart from the text around it: space above and
//! below it, the text of its content and of the line under it aligned by
//! `--flm-floats-align` (centered by default), and that line slightly apart
//! from the content and slightly smaller than the text. An image that is the
//! float's content is a block, which text alignment does not move, so the
//! stylesheet centers it with its side margins regardless of the alignment;
//! the HTML backend's core stylesheet keeps the image within the width of the
//! text and keeps its proportions
//! ([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)). Every value the stylesheet uses is a
//! custom property it declares (`--flm-floats-spacing`, `--flm-floats-align`,
//! `--flm-floats-caption-spacing`, `--flm-floats-caption-size`), which a page
//! sets to change the result without writing a rule of its own. The whole
//! stylesheet is one cascade layer, `flm`.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

/// The identifier of the feature's stylesheet, `flm.floats.style`, the
/// style definition [`stylesheet`] returns.
pub const STYLE: &str = "flm.floats.style";

// See decision rationale at [§flm-dd-dr:std-floats-stylesheet].
/// Returns the feature's stylesheet, the style definition `flm.floats.style`,
/// whose content is the text of the file `css/floats.css`: a float set apart from
/// the text around it and centered, in one cascade layer, every value a
/// custom property it declares. See the [module
/// documentation](self#the-stylesheet).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/floats.css")))],
        docs: String::from(
            "a float set apart from the text around it, with its content and the line under its \
             content centered",
        ),
    }
}

/// Registers what the feature contributes to the `html` backend: the
/// stylesheet ([`stylesheet`]).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the registration round that accepts it (see
/// [`Finalizer`](crate::core::registry::Finalizer)).
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
        assert_eq!(css.lines().next(), Some("/* flm.floats.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
