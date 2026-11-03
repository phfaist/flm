//! The guest's own type of `flm:core/data/table`: [`TableValue`], a table as
//! the host answers it, each kind by its handle.

use serde_json::{json, Value};

use super::{not_shape, single};
#[cfg(feature = "native")]
use crate::adapter::dispatch::extra::LowererTable;
#[cfg(feature = "native")]
use crate::adapter::handles::{CallTable, Entry, TableEntry};
#[cfg(feature = "native")]
use crate::adapter::typed::FromApi;
use crate::adapter::values::JsonForm;
#[cfg(feature = "native")]
use crate::api;
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

/// The api's `TableValue`, its table lent (the counters and the anchors as
/// lent façades); the host's `table_value_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::core::data::table::TableValue<'c>> for TableValue {
    fn from_api(api: api::core::data::table::TableValue<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        use api::core::data::table::TableValue as Api;
        Ok(match api {
            Api::Ext(read) => TableValue::Ext(FromApi::from_api(read, table)?),
            Api::Trees(trees) => TableValue::Trees(FromApi::from_api(trees, table)?),
            Api::Lowered(lowered) => TableValue::Lowered(FromApi::from_api(lowered, table)?),
            Api::Projection(projection) => TableValue::Projection(projection),
            Api::Counters(counters) => TableValue::Counters(Counters { handle: table.lend(Entry::Counters(TableEntry::Lent(counters))) }),
            Api::Anchors(anchors) => TableValue::Anchors(Anchors { handle: table.lend(Entry::Anchors(TableEntry::Lent(anchors))) }),
        })
    }
}

/// A table as a rule's lowerer answers it, the counters and the anchors
/// recorded under their handles, any other table lent as `TableValue`'s
/// conversion lends it; the host's reply of `Lowerer.get`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, LowererTable<'c>> for TableValue {
    fn from_api(api: LowererTable<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(match api {
            LowererTable::Projection(projection) => TableValue::Projection(projection),
            LowererTable::Counters(handle) => TableValue::Counters(Counters { handle }),
            LowererTable::Anchors(handle) => TableValue::Anchors(Anchors { handle }),
            LowererTable::Owned(owned) => FromApi::from_api(owned, table)?,
        })
    }
}
