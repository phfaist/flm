//! The stylesheet of the [`citations`](super) feature on the `html` backend,
//! `flm.citations.style` ([`stylesheet`]), and the markup it selects on.
//!
//! On the HTML page, a citation is written in the shared presentation
//! vocabulary, as on every other output: the general rules of `\cite` and
//! `\R` ([`CiteRule`](super::CiteRule), [`RefCiteRule`](super::RefCiteRule))
//! stage the citation mark, and the backend writes the HTML as its [HTML
//! contract](crate::guide::rendering#the-html-contract) says. The general
//! rules also require (record a requirement for; an identifier a rule
//! records to say that its output needs a definition, see [`StyleDef`]) the
//! feature's style identifier [`STYLE`] (`flm.citations.style`), which on
//! this backend names the stylesheet, so a page holding no citation does not
//! carry it. The list of cited works is lowered by its general rule
//! ([`BibliographyRule`](super::BibliographyRule)).
//!
//! # The markup
//!
//! `Codes of this family are well studied \cite{a,b}.` writes a no-break
//! space (U+00A0, written `&#xA0;`) in a span of role `flm.citations.space`,
//! then the citation mark: a span of role `flm.citations.mark` carrying the
//! kind token `attached`, holding the delimiters, the join texts and the
//! numbers. Each number is a link of kind `cite` to its work's entry in the
//! list of cited works, with the document role `doc-biblioref`:
//!
//! ```html
//! <p class="flm-pres-par">Codes of this family are well studied<span class="flm-pres-span flm-citations-space">&#xA0;</span><span class="flm-pres-span flm-citations-mark" data-flm-kind="attached">[<a class="flm-pres-link" href="#citation-1" role="doc-biblioref" data-flm-kind="cite">1</a>,<a class="flm-pres-link" href="#citation-2" role="doc-biblioref" data-flm-kind="cite">2</a>]</span>.</p>
//! ```
//!
//! The space is a character of the text, in a span of its own before the
//! citation mark's span: the text a reader copies from the page holds it, the
//! line does not break between the word and the citation mark, and a
//! stylesheet selects it by its class `flm-citations-space` (to hide it, or to
//! change its width).
//!
//! Under the superscript form ([`MarkForm::Superscript`](super::MarkForm)) of
//! the listed placement, the citation mark of a `\cite` is a `<sup>` element
//! (with `data-flm-style="superscript"`) holding no delimiters, and no space
//! is written before it. The citation mark of an `\R` is never raised and
//! shows its numbers with no delimiters; the word that `\R` writes is placed
//! before it, in a span of role `flm.citations.word` (`refs.&#xA0;1,2`).
//! Under the in-place placement, the citation mark carries the kind token
//! `in-place` after `attached` or `standalone`, and holds the citation
//! entries themselves, with no link.
//!
//! The list of cited works is placed after the document's content (after the
//! lists of xnotes in the standard package; an xnote is a note set apart from
//! the running text, such as a footnote, see
//! [`xnotes`](crate::stdcore::features::xnotes)): a block with the document
//! role `doc-bibliography`, holding the title as a heading and a labeled list
//! whose entries carry the works' identifiers. The entries carry no document
//! role:
//!
//! ```html
//! <div class="flm-pres-block flm-citations-bibliography" role="doc-bibliography" data-flm-kind="citations">
//!   <h2 class="flm-pres-block--heading" data-flm-heading="citations" data-flm-level="2" data-flm-toc="false">References</h2>
//!   <ol class="flm-pres-list" data-flm-kind="numbered citations labeled">
//!     <li id="citation-1"><span class="flm-pres-list-label"><span class="flm-pres-span flm-citations-entry-number">[1]</span></span>
//!       <p class="flm-pres-par">Knuth, The Art of Computer Programming, 1984</p>
//!     </li>
//!   </ol>
//! </div>
//! ```
//!
//! # The stylesheet
//!
//! The stylesheet [`STYLE`] (`flm.citations.style`, [`stylesheet`]) is one
//! cascade layer that keeps a raised citation mark from pushing its line apart
//! from the lines around it. It sets the line height of a raised citation mark
//! to the custom property `--flm-citations-raised-line-height`, declared in it
//! with the value `0`, which a page's own stylesheet sets to change that line
//! height without repeating the selector. A citation mark wraps where text
//! does. The raising itself, and the size of the raised numbers, come from the
//! backend's own stylesheet, for every span of the superscript style.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

/// The identifier of the feature's stylesheet, `flm.citations.style`
/// ([`stylesheet`]), which the general rules of `\cite` and `\R` require.
pub const STYLE: &str = "flm.citations.style";

/// Returns the style definition of the feature's stylesheet,
/// `flm.citations.style` ([`StyleDef`]): one cascade layer that keeps a raised
/// citation mark from pushing its line apart, with its one value held in a
/// custom property it declares. See the [module
/// documentation](self#the-stylesheet).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/citations.css")))],
        docs: String::from(
            "the line height of a raised citation mark, which keeps the mark from pushing its \
             line apart from the lines around it",
        ),
    }
}

/// Registers the feature's part of the HTML page: the stylesheet
/// ([`stylesheet`]).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the round that takes it.
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
        assert_eq!(css.lines().next(), Some("/* flm.citations.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
