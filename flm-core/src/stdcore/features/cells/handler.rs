//! The content handler that the `cells` feature contributes to the
//! [`floats`](crate::stdcore::features::floats) feature: [`CellsContent`],
//! named `cells`, which admits exactly one table of cells as a float's
//! content.
//!
//! A float type declares the content handlers it admits
//! ([`FloatType::handlers`](crate::stdcore::features::floats::FloatType::handlers)).
//! The default `table` float type lists `cells` first, and lists the
//! feature's scoped package
//! [`ENVIRONMENT_PACKAGE`](super::ENVIRONMENT_PACKAGE) among the scoped
//! packages of its body, so that `\begin{cells}` can be written there. The
//! handler is contributed to the floats feature's service slot
//! [`CONTENT_SLOT`](crate::stdcore::features::floats::CONTENT_SLOT) as a peer
//! contribution, regardless of the switch
//! [`CellsConfig::global`](super::CellsConfig::global): a registry without
//! the `floats` feature drops the contribution with one Note, and still
//! builds.

use techy::core::node::NodeRef;

use crate::lang::Flm;
use crate::model::element::element_of;
use crate::stdcore::features::floats::FloatContentHandler;

use super::ENVIRONMENT;

/// The name of the feature's content handler, `cells`: a float type lists
/// this name among its content handlers to admit one table of cells.
pub const HANDLER: &str = "cells";

/// The content handler [`HANDLER`] (`cells`): it admits exactly one table of
/// cells, a node of [`ENVIRONMENT`] (`flm.cells.cells`), and refuses any
/// other content: two tables, text, a table beside a paragraph, or nothing
/// at all. Its refusal reads "exactly one table of cells", which the float's
/// rule reports as the end of the sentence "a ‘table’ must hold …". See the
/// [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct CellsContent;

impl FloatContentHandler for CellsContent {
    fn name(&self) -> &str {
        HANDLER
    }

    fn admit(&self, content: &[NodeRef<'_, Flm>]) -> Result<(), String> {
        match content {
            [one] if element_of(one).name.as_str() == ENVIRONMENT => Ok(()),
            _ => Err(String::from("exactly one table of cells")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::floats::CELLS_HANDLER;

    #[test]
    fn the_handler_is_the_one_the_default_table_type_names() {
        assert_eq!(CellsContent.name(), HANDLER);
        assert_eq!(HANDLER, CELLS_HANDLER);
    }

    #[test]
    fn no_content_is_refused() {
        assert_eq!(CellsContent.admit(&[]), Err(String::from("exactly one table of cells")));
    }
}
