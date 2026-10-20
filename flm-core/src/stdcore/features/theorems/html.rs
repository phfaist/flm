//! The stylesheet of the [`theorems`](super) feature on the `html` backend,
//! `flm.theorems.style` ([`stylesheet`]), and the markup it selects on.
//!
//! A theorem is lowered on every backend by the general rule
//! ([`TheoremRule`](super::TheoremRule)), which writes presentation
//! elements; the `html` backend writes them as HTML following [the HTML
//! contract](crate::guide::rendering#the-html-contract) (the CSS classes and
//! attributes a stylesheet may rely on). The general rule also requires
//! (records a requirement for) the feature's style identifier [`STYLE`]
//! (`flm.theorems.style`) at every theorem; on the `html` backend the
//! identifier names this stylesheet, so that a page holding a theorem carries
//! it. The rendering guide describes this output under [Theorems in
//! HTML](crate::guide::rendering#theorems-in-html).
//!
//! # The markup
//!
//! A theorem is a block of role `flm.theorems.theorem`: a `<div>` whose
//! second CSS class is the role's (`flm-theorems-theorem`), with the anchor of
//! the theorem's reference point as its `id` and the theorem class's name then
//! the theorem kind's as its kind tokens (`data-flm-kind`). Its heading is the
//! run-in decoration at the start of its first paragraph: it holds the kind's
//! word and number in a span of role `flm.theorems.name` and the title in a span of role `flm.theorems.title`,
//! joined as the theorem class's heading template says. The backend writes its
//! run-in stop (the render option `run_in_stop`, a period by default) after
//! the heading:
//!
//! ```html
//! <div class="flm-pres-block flm-theorems-theorem" id="theorem-1" data-flm-kind="theoremlike theorem">
//!   <p class="flm-pres-par"><span class="flm-pres-deco-runin"><span class="flm-pres-span flm-theorems-name">Theorem&#xA0;1</span> (<span class="flm-pres-span flm-theorems-title">My theorem</span>).</span>The square root of two is irrational.</p>
//! </div>
//! ```
//!
//! A proof's shown back-reference is a link of kind `ref` to the statement's
//! anchor inside the title span, written as any reference is written, and the
//! closing mark is the final decoration at the end of the last paragraph:
//!
//! ```html
//! <div class="flm-pres-block flm-theorems-theorem" data-flm-kind="prooflike proof">
//!   <p class="flm-pres-par"><span class="flm-pres-deco-runin"><span class="flm-pres-span flm-theorems-name">Proof</span> (<span class="flm-pres-span flm-theorems-title"><a class="flm-pres-link" href="#theorem-1" data-flm-kind="ref">Theorem&#xA0;1</a></span>).</span>We argue by contradiction.<span class="flm-pres-deco-final">□</span></p>
//! </div>
//! ```
//!
//! A theorem with no anchor (a proof, which accepts no label, or any theorem
//! in a registry without the references feature) has no `id`. Where the body
//! opens with another block, such as a list or a displayed formula, the
//! heading is placed in a paragraph of its own before it; an empty body is
//! one paragraph holding the heading and the mark. The feature adds no tag
//! and no CSS class of its own: a stylesheet selects a theorem by the role's
//! CSS class, the two spans' CSS classes and the kind tokens, which carry a
//! host's own theorem kinds and theorem classes as they carry the shipped
//! ones.
//!
//! # The stylesheet
//!
//! [`stylesheet`] (`flm.theorems.style`) is one cascade layer. It sets the
//! kind's word and number in bold and the rest of the heading in the weight of
//! the text, and, for a kind of the `prooflike` class, the word in italics in
//! the weight of the text. Every value it uses is a custom property declared
//! in it (`--flm-theorems-heading-weight`, `--flm-theorems-name-weight`,
//! `--flm-theorems-prooflike-name-weight`,
//! `--flm-theorems-prooflike-name-style`), which a page sets to change the
//! look without writing a CSS rule. The general rule requires the stylesheet
//! at every theorem, so a page that holds no theorem does not carry it.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

/// The identifier of the feature's stylesheet, `flm.theorems.style`
/// ([`stylesheet`]), a style definition ([`StyleDef`]) that the general rule
/// [`TheoremRule`](super::TheoremRule) requires at every theorem.
pub const STYLE: &str = "flm.theorems.style";

/// The feature's stylesheet, `flm.theorems.style`, as a style definition
/// whose text the library holds (the file `css/theorems.css`, included at
/// compile time); the backend decides how the page receives it. It sets a
/// theorem's word and number in bold and a proof's in italics, in one
/// cascade layer, every value a custom property it declares. See the [module
/// documentation](self#the-stylesheet).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/theorems.css")))],
        docs: String::from("the heading of a theorem: its name set apart from its title"),
    }
}

/// Registers what the feature contributes to the `html` backend: the
/// stylesheet ([`stylesheet`]).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the registration round that accepts it.
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
        assert_eq!(css.lines().next(), Some("/* flm.theorems.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
