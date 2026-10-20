//! The stylesheet of the [`defterm`](super) feature on the `html` backend,
//! `flm.defterm.style` ([`stylesheet`]), and the markup it selects on.
//!
//! On the `html` backend a term definition is lowered by the general rule
//! ([`DefinitionRule`](super::rules::DefinitionRule)) and a term link by the
//! general rule [`TermRule`](super::rules::TermRule); the backend writes the
//! presentation elements as HTML following [the HTML
//! contract](crate::guide::rendering#the-html-contract) (the CSS classes and
//! attributes a stylesheet may rely on). The rendering guide describes this
//! output under [Defined terms in
//! HTML](crate::guide::rendering#defined-terms-in-html).
//!
//! - A **term definition** is a block of role `flm.defterm` carrying the term
//!   definition's anchor as its `id`; its lead-in is a run-in decoration at
//!   the start of its first paragraph, holding a span of role
//!   `flm.defterm.leadin`:
//!
//!   ```html
//!   <div class="flm-pres-block flm-defterm" id="defterm-Entropy">
//!     <p class="flm-pres-par"><span class="flm-pres-deco-runin"><span class="flm-pres-span flm-defterm-leadin">Entropy: </span></span>A measure of disorder.</p>
//!   </div>
//!   ```
//!
//! - A **term link** is a link whose `data-flm-kind` is `term`, to the term
//!   definition's anchor:
//!   `<a class="flm-pres-link" href="#defterm-Entropy" data-flm-kind="term">entropy</a>`.
//! - A term link **inside the definition of its own term** is a span of role
//!   `flm.defterm.term`, and no link:
//!   `<span class="flm-pres-span flm-defterm-term">Entropy</span>`.
//!
//! A term link inside the text of another link (an `\href` whose text holds a
//! `\term`) is written by the backend as every link inside a link is: a
//! `<span>` with the link's CSS classes and kind and no address.
//!
//! # The stylesheet
//!
//! [`stylesheet`] (`flm.defterm.style`) is one cascade layer holding one
//! CSS rule: the term being defined, where its own term definition uses it, is
//! set in italics, as the recomposed LaTeX sets it with `\emph` by default.
//! The value is the custom property `--flm-defterm-term-style`, which a page
//! sets to change the look without writing a CSS rule. The term link's
//! general rule requires the feature's style identifier [`STYLE`], which on
//! the `html` backend names this stylesheet, only where it writes that span,
//! so a page that holds no such span does not carry it.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

/// The identifier of the feature's stylesheet, `flm.defterm.style`
/// ([`stylesheet`]), a style definition ([`StyleDef`]) that the term link's
/// general rule [`TermRule`](super::rules::TermRule) requires where the term
/// link is inside the definition of its own term.
pub const STYLE: &str = "flm.defterm.style";

/// The feature's stylesheet, `flm.defterm.style`, as a style definition
/// whose text the library holds (the file `css/defterm.css`, included at
/// compile time); the backend decides how the page receives it. It sets the
/// term being defined, where its own term definition uses it, in italics, in
/// one cascade layer, the value a custom property it declares. See the
/// [module documentation](self#the-stylesheet).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/defterm.css")))],
        docs: String::from("the term being defined, where its own term definition uses it"),
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
        assert_eq!(css.lines().next(), Some("/* flm.defterm.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
