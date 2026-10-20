//! The stylesheet of the [`quote`](super) feature on the `html` backend,
//! `flm.quote.style` ([`stylesheet`]), and the markup it selects on.
//!
//! On the `html` backend a quotation is lowered by the general rule
//! [`QuoteRule`](super::QuoteRule), into the shared presentation vocabulary;
//! the rule also requires (records a requirement for) the feature's style
//! identifier [`STYLE`] (`flm.quote.style`), which on the `html` backend
//! names this stylesheet, a style definition ([`StyleDef`]) that the backend
//! delivers with the page
//! ([style and script definitions](crate::guide::concepts_overview#style-and-script-definitions)).
//! The HTML itself comes from the backend's rules for the presentation
//! elements, the same for every feature
//! ([the HTML contract](crate::guide::rendering#the-html-contract)).
//!
//! # The markup
//!
//! A quotation is a block of role `flm.quote`. The HTML backend's table of
//! block tags ([`BLOCK_TAGS`](crate::stdcore::backends::html::conventions::BLOCK_TAGS))
//! writes it as a `<blockquote>` for every quote environment, with the
//! environment's name as its kind token in `data-flm-kind`. Each quotation
//! section is a `<div>` whose second class is its role's class:
//! `flm-quote-text`, `flm-quote-lines`, `flm-quote-attributed` or
//! `flm-quote-block`:
//!
//! ```html
//! <blockquote class="flm-pres-block flm-quote" data-flm-kind="quote">
//!   <div class="flm-pres-block flm-quote-text">
//!     <p class="flm-pres-par">To be or not to be.</p>
//!   </div>
//!   <div class="flm-pres-block flm-quote-attributed">
//!     <p class="flm-pres-par">Shakespeare</p>
//!   </div>
//! </blockquote>
//! ```
//!
//! A lines section holds a run of lines. A line with a left indentation
//! carries it as a custom property in its `style` attribute; a line without
//! one has no `style` attribute:
//!
//! ```html
//! <blockquote class="flm-pres-block flm-quote" data-flm-kind="address">
//!   <div class="flm-pres-block flm-quote-lines">
//!     <div class="flm-pres-lines">
//!       <div class="flm-pres-line" data-flm-align="left">123 Main St</div>
//!       <div class="flm-pres-line" style="--flm-element-indent-left: calc(1 * var(--flm-tab))" data-flm-align="left">City, ST 12345</div>
//!     </div>
//!   </div>
//! </blockquote>
//! ```
//!
//! The backend writes no tag or class specific to this feature: the classes
//! come from the role and the kind tokens, as for every feature, so a
//! stylesheet selects every quotation by the role's class (`.flm-quote`), a
//! quotation section by its role's class, and one environment by its kind token
//! (`.flm-quote[data-flm-kind~="address"]`).
//!
//! # The stylesheet
//!
//! The stylesheet [`stylesheet`] (`flm.quote.style`) is one cascade layer
//! that sets a quotation apart from the text around it. Every quotation is
//! indented on both sides and spaced above and below. A `quote`, and every
//! environment a host configures, has a faint background and inner space, and
//! its passages and lines are in italics; a `blockquote` is only indented, and
//! an `address` is indented with its lines slightly smaller. A block section
//! has a thin rule on its left, and an attribution is set on a line of its
//! own, aligned to the right, after a dash. Each rule selects an environment
//! by its kind token, never by the tag, and selects a quotation section only
//! as a direct child of its quotation, so that a quotation nested in another
//! keeps its own look. Every value it uses is a custom property declared in
//! the stylesheet (`--flm-quote-indent`, `--flm-quote-background`,
//! `--flm-quote-attributed-lead` and the others), which a page sets to change
//! the look without writing a rule. A quotation also sets
//! `--flm-margin-note-inset` to how far it is indented from the right of the
//! text, so that a note shown in the margin and written inside the quotation
//! still reaches the page's column of notes (the margin rule of the HTML
//! backend's core stylesheet
//! ([`STYLE_CORE`](crate::stdcore::backends::html::conventions::STYLE_CORE))
//! reads it).
//!
//! The general rule requires the stylesheet at every quotation, so that a
//! page holding no quotation carries none of it.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

/// The identifier of the feature's stylesheet, `flm.quote.style`
/// ([`stylesheet`]), which the general rule [`QuoteRule`](super::QuoteRule)
/// requires at every quotation.
pub const STYLE: &str = "flm.quote.style";

/// Returns the feature's stylesheet, `flm.quote.style`, as a style
/// definition: one cascade layer that sets a quotation apart from the text
/// around it, every value a custom property it declares. See the [module
/// documentation](self#the-stylesheet).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/quote.css")))],
        docs: String::from("a quotation set apart from the text around it, and its attribution from its passage"),
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
        assert_eq!(css.lines().next(), Some("/* flm.quote.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
