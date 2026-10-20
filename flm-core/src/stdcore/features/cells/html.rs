//! The `cells` feature's part of an HTML page: its style definition
//! [`STYLE`] (`flm.cells.style`, [`stylesheet`]), the stylesheet that the
//! HTML backend includes wherever a page holds a table of cells.
//!
//! A style definition ([`StyleDef`]) is a stylesheet that a feature's output
//! depends on, recorded in the registry under an identifier; a lowering rule
//! or an emitter requires it by identifier, and the backend that writes the
//! output decides how it is delivered (see
//! [style and script definitions](crate::guide::concepts_overview#style-and-script-definitions)).
//!
//! # The markup
//!
//! The HTML backend writes the `flm.pres.cells` node that the general rule
//! stages as the [HTML contract](crate::guide::rendering#the-html-contract)
//! states for it ([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)):
//! a `<table>` holding a `<tr>` per row of the grid and, in it, one element
//! per position that no span covers. That element is a `<td>`, or a `<th>`
//! carrying `scope` for a header cell, with `colspan` and `rowspan` where the
//! cell spans several positions, and the cell's facts as its attributes; an
//! empty `<td>` is written where no cell is placed:
//!
//! ```html
//! <table class="flm-pres-cells">
//!   <tr>
//!     <th scope="col">
//!       <p class="flm-pres-par">Name</p>
//!     </th>
//!     <th scope="col">
//!       <p class="flm-pres-par">City</p>
//!     </th>
//!   </tr>
//!   <tr>
//!     <td data-flm-align="right" data-flm-rules="left" data-flm-style-green="">
//!       <p class="flm-pres-par">John</p>
//!     </td>
//!     <td></td>
//!   </tr>
//! </table>
//! ```
//!
//! The feature adds no tag and no class of its own: a stylesheet selects a
//! table by the element's class and a cell by its attributes.
//!
//! # The stylesheet
//!
//! The style definition [`STYLE`] ([`stylesheet`]) holds one cascade layer
//! with these declarations:
//!
//! - the table's outline, a line above it and a line below it, drawn on the
//!   `<table>` element;
//! - a line under a column header;
//! - a line at each edge listed in a cell's `data-flm-rules` (the cell's
//!   rules: lines drawn along its edges), independent of the other cells'
//!   lines;
//! - a cell's alignment from `data-flm-align`, to the left where the cell
//!   has none;
//! - the padding of a cell;
//! - the backgrounds of the four custom styles the feature ships
//!   (`[data-flm-style-green]` and the three others);
//! - a table that is a float's content is centered in the float.
//!
//! Every value the stylesheet uses is a custom property declared in it
//! (`--flm-cells-spacing`, `--flm-cells-padding`, `--flm-cells-align`,
//! `--flm-cells-outline`, `--flm-cells-header-rule`, `--flm-cells-rule`,
//! `--flm-cells-green` and the other three colors), which a page can set to
//! change these values without writing a selector of its own.
//!
//! The style definition reaches a page through the HTML contract: the
//! contract's row for `flm.pres.cells` refers to it, and the backend
//! requires it wherever it writes a table, so a page without a table does
//! not include it. A custom cell style that another feature declares is
//! styled by that feature's own style definition, which the declaration
//! requires ([`CellStyle::requires`](super::CellStyle::requires)) and which
//! the general rule requires in turn wherever a cell carries the style.

use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::model::names::Name;

use super::STYLE;

/// The feature's style definition [`STYLE`] (`flm.cells.style`): a table's
/// outline and edge lines, a cell's padding and alignment, and the backgrounds of
/// the four custom styles the feature ships, in one cascade layer, every
/// value a custom property it declares. The definition holds the
/// stylesheet's text itself ([`DefContent::Inline`]). See the
/// [module documentation](self).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/cells.css")))],
        docs: String::from(
            "a table's outline and edge lines, a cell's padding and alignment, and the backgrounds \
             of the custom styles `green`, `blue`, `yellow` and `red`",
        ),
    }
}

/// Registers the style definition ([`stylesheet`]).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made in
/// a registration round that does not accept it (see
/// [`FeatureBuilder`](crate::core::registry::FeatureBuilder)).
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
        assert_eq!(css.lines().next(), Some("/* flm.cells.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
