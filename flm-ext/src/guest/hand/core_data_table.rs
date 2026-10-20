//! The guest's own type of `flm:core/data/table`: [`TableValue`], a table as
//! the host answers it, each kind by its handle.

use serde_json::{json, Value};

use super::{not_shape, single};
use crate::adapter::values::JsonForm;
use crate::guest::core::data::table::ExtTableRead;
use crate::guest::core::stage::TreesTable;
use crate::guest::stdcore::backends::lowering::LoweredTrees;
use crate::guest::stdcore::features::numbering::Counters;
use crate::guest::stdcore::features::refs::Anchors;
use crate::runtime::ExtError;

/// Mirrors [`TableValue`](crate::api::core::data::table::TableValue): a table
/// a stage or a rule reads, by its kind.
#[derive(Debug, Clone, PartialEq)]
pub enum TableValue {
    /// An extension's own table.
    Ext(ExtTableRead),
    /// A table of trees.
    Trees(TreesTable),
    /// The lowered trees of a backend.
    Lowered(LoweredTrees),
    /// A projection, as data.
    Projection(Value),
    /// The counters of the numbering library.
    Counters(Counters),
    /// The anchors of the references library.
    Anchors(Anchors),
}

/// The keys of the shape, in the order of the variants.
const KEYS: &[&str] = &["ext", "trees", "lowered", "projection", "counters", "anchors"];

/// `{"ext": handle}`, `{"trees": handle}`, `{"lowered": handle}`,
/// `{"projection": data}`, `{"counters": handle}` or `{"anchors": handle}`;
/// the host's `table_value_to_value`.
impl JsonForm for TableValue {
    fn to_value(&self) -> Value {
        match self {
            TableValue::Ext(table) => json!({ "ext": table.to_value() }),
            TableValue::Trees(table) => json!({ "trees": table.to_value() }),
            TableValue::Lowered(table) => json!({ "lowered": table.to_value() }),
            TableValue::Projection(data) => json!({ "projection": data }),
            TableValue::Counters(table) => json!({ "counters": table.to_value() }),
            TableValue::Anchors(table) => json!({ "anchors": table.to_value() }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let shape = || not_shape("a table", "a map of one of the keys `ext`, `trees`, `lowered`, `projection`, `counters`, `anchors`", value);
        let (key, inner) = single(value, KEYS).ok_or_else(shape)?;
        Ok(match key {
            "ext" => TableValue::Ext(JsonForm::from_value(inner)?),
            "trees" => TableValue::Trees(JsonForm::from_value(inner)?),
            "lowered" => TableValue::Lowered(JsonForm::from_value(inner)?),
            "projection" => TableValue::Projection(inner.clone()),
            "counters" => TableValue::Counters(JsonForm::from_value(inner)?),
            _ => TableValue::Anchors(JsonForm::from_value(inner)?),
        })
    }
}
