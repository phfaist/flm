//! The stylesheet of the [`toc`](super) feature on the `html` backend,
//! `flm.toc.style` ([`stylesheet`]), and the markup it selects on.
//!
//! On the HTML page the table of contents is written with the same
//! presentation elements as on every other output ([`rules`](super::rules)),
//! and the HTML backend writes them as its HTML contract states (see
//! [`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)): the block of
//! role `flm.toc` is a `<nav>` carrying the role `doc-toc`, each list a `<ul>`,
//! each entry an `<li>` holding the link to its heading, the number in a span
//! of role `flm.toc.entry.number`. For `\section{One}\subsection{Two}` under
//! numbering:
//!
//! ```html
//! <nav class="flm-pres-block flm-toc" role="doc-toc" data-flm-kind="toc">
//!   <ul class="flm-pres-list" data-flm-kind="unnumbered toc">
//!     <li><a class="flm-pres-link" href="#sec--One" data-flm-kind="toc"><span class="flm-pres-span flm-toc-entry-number">1</span> One</a>
//!       <ul class="flm-pres-list" data-flm-kind="unnumbered toc">
//!         <li><a class="flm-pres-link" href="#sec--Two" data-flm-kind="toc"><span class="flm-pres-span flm-toc-entry-number">1.1</span> Two</a></li>
//!       </ul>
//!     </li>
//!   </ul>
//! </nav>
//! ```
//!
//! The general rule of the root ([`RootRule`](super::rules::RootRule))
//! requires the feature's style identifier [`STYLE`] (`flm.toc.style`), which
//! on this backend names the stylesheet: one cascade layer that removes the
//! list markers and indents each nested list, the indentation a custom
//! property it declares (`--flm-toc-indent`). It sets structure only; a
//! page that wants another presentation adds a stylesheet of its own, or
//! replaces this one through the render option
//! [`definitions`](crate::stdcore::backends::html::HtmlRenderOptions::definitions).

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

use super::STYLE;

/// The feature's style definition, `flm.toc.style` ([`STYLE`]): the lists
/// without a marker, each nested list indented, in one cascade layer, with
/// the indentation a custom property it declares (`--flm-toc-indent`). The
/// space after an entry's number is text the entry's rule writes, not a rule
/// of the stylesheet. See the [module documentation](self).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/toc.css")))],
        docs: String::from(
            "the lists of the table of contents without a marker, each nested list indented, and a \
             space after an entry's number",
        ),
    }
}

/// Registers the stylesheet ([`stylesheet`]).
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
        assert_eq!(css.lines().next(), Some("/* flm.toc.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
