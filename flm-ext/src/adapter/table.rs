//! `ExtTable`, the type every table a scripted feature declares holds.

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::{ProjectCx, Table};
use flm_core::core::registry::DefinitionText;
use flm_core::model::addr::NodeAddr;
use flm_core::techy::serialize::SerialValue;
use serde_json::Value;

use crate::adapter::values::{artifact_from_value, check_data, node_addr_to_value};
use crate::api::core::data::store::TablePiece;
use crate::api::core::data::table::ExtTableRead;
use crate::runtime::ExtError;
use crate::sealed::Sealed;

/// An extension's own table: the data items its stage pushed, in traversal
/// order. Another scripted feature reads it as a list; a Rust feature reads it
/// as this type, under the table's name
/// ([`TableHandle::<ExtTable>::new`](flm_core::core::data::table::TableHandle::new)).
///
/// Every table a scripted feature declares holds this type; the registry's
/// own type check covers a Rust feature that names the table with another
/// type. A piece pushed while a value is caught up goes, as any piece keyed
/// by node does, to the catch-up's own store: the table keeps the library's
/// default [`piece_class`](Table::piece_class).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtTable {
    items: Vec<Value>,
}

impl ExtTable {
    /// Appends `item`, a value that can cross the boundary.
    ///
    /// # Errors
    ///
    /// `item` holds a floating-point number or a reserved map key
    /// ([`ExtError::Value`]); nothing is appended then.
    pub fn push(&mut self, item: Value) -> Result<(), ExtError> {
        check_data(&item)?;
        self.items.push(item);
        Ok(())
    }

    /// The items, in the order they were pushed and absorbed.
    pub fn items(&self) -> &[Value] {
        &self.items
    }

    /// The items at the node of address `addr`: the maps whose field `addr`
    /// equals the address's data form
    /// ([`node_addr_to_value`]),
    /// in order, copied.
    pub fn items_at(&self, addr: &NodeAddr) -> Vec<Value> {
        let key = node_addr_to_value(addr);
        self.items.iter().filter(|item| item.get("addr") == Some(&key)).cloned().collect()
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the table holds no item.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Table for ExtTable {
    fn absorb(&mut self, piece: ExtTable) {
        self.items.extend(piece.items);
    }

    /// The list of the items. An item converts to techy's value model since
    /// [`push`](ExtTable::push) checked it; one that did not would show as
    /// `null`.
    fn project(&self, _: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.items.iter().map(|item| serde_json::from_value(item.clone()).unwrap_or(SerialValue::Null)).collect(),
        )
    }
}

impl Sealed for ExtTable {}

impl TablePiece for ExtTable {
    fn push(&mut self, item: Value) -> Result<(), ExtError> {
        ExtTable::push(self, item)
    }
}

impl ExtTableRead for ExtTable {
    fn items(&self) -> Vec<Value> {
        self.items.clone()
    }

    fn items_at(&self, addr: &NodeAddr) -> Vec<Value> {
        ExtTable::items_at(self, addr)
    }
}

/// The piece of a table of a definition's text
/// ([`definition_text_table`](crate::api::core::registry::FeatureBuilder::definition_text_table)),
/// which a stage writes with one string.
impl Sealed for DefinitionText {}

impl TablePiece for DefinitionText {
    /// Takes `item`, which must be a string, as the part's text; an empty
    /// string leaves the text as it is, as [`DefinitionText`]'s pieces are
    /// absorbed.
    ///
    /// # Errors
    ///
    /// `item` is not a string ([`ExtError::Value`]).
    fn push(&mut self, item: Value) -> Result<(), ExtError> {
        match item {
            Value::String(text) => {
                if !text.is_empty() {
                    self.text = text;
                }
                Ok(())
            }
            other => Err(ExtError::Value {
                detail: format!("the text of a definition's part must be a string, not `{other}`"),
            }),
        }
    }
}

/// The piece of a table of an output's files
/// ([`StandardTable::ArtifactSet`](crate::api::core::registry::StandardTable::ArtifactSet)),
/// which a stage writes one artifact at a time.
impl Sealed for ArtifactSet {}

impl TablePiece for ArtifactSet {
    /// Reads the artifact from its data form
    /// ([`artifact_from_value`]) and adds it to the set
    /// ([`ArtifactSet::insert`]); pieces of several calls are combined as
    /// [`ArtifactSet`]'s pieces are.
    ///
    /// # Errors
    ///
    /// `item` is not an artifact ([`ExtError::Value`]); the set refuses it,
    /// an artifact of the same identity being there already or a second one
    /// being marked as the main file ([`ExtError::Core`] of kind `artifact`,
    /// with the set's own text). The set is left as it was.
    fn push(&mut self, item: Value) -> Result<(), ExtError> {
        let artifact = artifact_from_value(&item)?;
        self.insert(artifact).map_err(|violation| ExtError::Core {
            kind: "artifact".to_string(),
            detail: violation.to_string(),
            data: Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct NoRefs;
    impl ProjectCx for NoRefs {
        fn node_ref(&self, _: flm_core::model::addr::NodeAddr) -> SerialValue {
            SerialValue::Null
        }
        fn tree_ref(&self, _: flm_core::model::addr::TreeId) -> SerialValue {
            SerialValue::Null
        }
    }

    #[test]
    fn pieces_absorb_in_order_and_project_as_a_list() {
        let mut table = ExtTable::default();
        table.push(json!({ "tree": "a", "words": 3 })).expect("an item");
        let mut piece = ExtTable::default();
        TablePiece::push(&mut piece, json!("b")).expect("an item");
        table.absorb(piece);
        assert_eq!(table.len(), 2);
        assert_eq!(ExtTableRead::items(&table), vec![json!({ "tree": "a", "words": 3 }), json!("b")]);
        assert_eq!(
            flm_core::dump::json::to_json(&table.project(&NoRefs)).ok(),
            Some(r#"[{"tree":"a","words":3},"b"]"#.to_string())
        );
    }

    #[test]
    fn a_float_is_not_pushed() {
        let mut table = ExtTable::default();
        let error = table.push(json!([1, 2.5])).expect_err("a float");
        assert_eq!(error.to_string(), "a number must be an integer, not `2.5`");
        assert!(table.is_empty());
    }
}
