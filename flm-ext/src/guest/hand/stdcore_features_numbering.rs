//! The guest's own types of `flm:stdcore/features/numbering`:
//! [`CounterRecord`] and [`NumberSegment`].

use serde_json::{json, Value};

use super::{get, not_shape, object};
#[cfg(feature = "native")]
use crate::adapter::handles::CallTable;
#[cfg(feature = "native")]
use crate::adapter::typed::FromApi;
use crate::adapter::values::JsonForm;
#[cfg(feature = "native")]
use crate::api;
use crate::guest::core::lowering::NodeAddr;
use crate::guest::model::aspects::PointId;
use crate::guest::techy::node::Tree;
use crate::runtime::ExtError;

/// Mirrors [`CounterRecord`](crate::api::stdcore::features::numbering::CounterRecord):
/// the value a counter has at a point. The guest's own: it holds a node's
/// address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterRecord {
    /// The counter's name.
    pub counter: String,
    /// Its value, by level.
    pub value: Vec<i64>,
    /// Its style.
    pub style: String,
    /// The address of the instance that set it, if any.
    pub instance: Option<NodeAddr>,
}

/// `{"counter", "value", "style", "instance"}`; the host's
/// `counter_record_to_value`.
impl JsonForm for CounterRecord {
    fn to_value(&self) -> Value {
        json!({
            "counter": self.counter,
            "value": self.value,
            "style": self.style,
            "instance": self.instance.as_ref().map_or(Value::Null, JsonForm::to_value),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a counter's record";
        let map = object(WHAT, value)?;
        let levels = map.get("value").unwrap_or(&Value::Null);
        let numbers = levels
            .as_array()
            .and_then(|items| items.iter().map(Value::as_i64).collect::<Option<Vec<i64>>>())
            .ok_or_else(|| not_shape(&format!("the field `value` of {WHAT}"), "a list of whole numbers", levels))?;
        Ok(CounterRecord { counter: get(WHAT, map, "counter")?, value: numbers, style: get(WHAT, map, "style")?, instance: get(WHAT, map, "instance")? })
    }
}

/// The api's `CounterRecord`, its instance's address written as the guest's;
/// the host's `counter_record_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::stdcore::features::numbering::CounterRecord> for CounterRecord {
    fn from_api(api: api::stdcore::features::numbering::CounterRecord, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(CounterRecord { counter: api.counter, value: api.value, style: api.style, instance: FromApi::from_api(api.instance, table)? })
    }
}

/// Mirrors [`NumberSegment`](crate::api::stdcore::features::numbering::NumberSegment):
/// a segment of a formatted number, and the point it links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberSegment {
    /// The segment's content.
    pub content: Tree,
    /// The point it links to, if any.
    pub link: Option<PointId>,
}

/// `{"content": handle, "link"}`; the host's reply for a number segment.
impl JsonForm for NumberSegment {
    fn to_value(&self) -> Value {
        json!({ "content": self.content.to_value(), "link": self.link.as_ref().map_or(Value::Null, JsonForm::to_value) })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a number segment";
        let map = object(WHAT, value)?;
        Ok(NumberSegment { content: get(WHAT, map, "content")?, link: get(WHAT, map, "link")? })
    }
}

/// The api's `NumberSegment`, its content lent as a tree, its point written
/// as the guest's; the host's reply for a number segment.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::stdcore::features::numbering::NumberSegment> for NumberSegment {
    fn from_api(api: api::stdcore::features::numbering::NumberSegment, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(NumberSegment { content: FromApi::from_api(api.content, table)?, link: FromApi::from_api(api.link, table)? })
    }
}
