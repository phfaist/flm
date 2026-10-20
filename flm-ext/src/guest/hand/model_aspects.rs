//! The guest's own types of `flm:model/aspects` ([`RefPoint`],
//! [`NumberSource`], [`AnchorPolicy`], [`ContentRef`], which hold nodes, and
//! [`PointId`], which holds a node's address), and the handler traits
//! [`Referenceable`] and [`Sectioning`].

use serde_json::{json, Value};

use super::{get, not_shape, object, single};
use crate::adapter::values::JsonForm;
use crate::api::model::aspects::{AcceptedRefTypes, Label, MalformedLabel};
use crate::guest::core::lowering::NodeAddr;
use crate::guest::techy::node::Node;
use crate::runtime::ExtError;

/// Mirrors [`Referenceable`](crate::api::model::aspects::Referenceable):
/// what an element offers to references.
pub trait Referenceable: Send + Sync {
    /// Mirrors [`Referenceable::points`](crate::api::model::aspects::Referenceable::points):
    /// the reference points of `node`.
    ///
    /// # Errors
    ///
    /// What the aspect answers.
    fn points(&self, node: &Node) -> Result<Vec<RefPoint>, ExtError>;
}

/// Mirrors [`Sectioning`](crate::api::model::aspects::Sectioning): an
/// element as a heading. A guest lends all three methods, the defaults
/// running in the guest.
pub trait Sectioning: Send + Sync {
    /// Mirrors [`Sectioning::level`](crate::api::model::aspects::Sectioning::level).
    ///
    /// # Errors
    ///
    /// What the aspect answers.
    fn level(&self, node: &Node) -> Result<u32, ExtError>;

    /// Mirrors [`Sectioning::in_toc`](crate::api::model::aspects::Sectioning::in_toc):
    /// by default, `true`.
    ///
    /// # Errors
    ///
    /// What the aspect answers.
    fn in_toc(&self, node: &Node) -> Result<bool, ExtError> {
        let _ = node;
        Ok(true)
    }

    /// Mirrors [`Sectioning::toc_title`](crate::api::model::aspects::Sectioning::toc_title):
    /// by default, no title of its own.
    ///
    /// # Errors
    ///
    /// What the aspect answers.
    fn toc_title(&self, node: &Node) -> Result<Option<ContentRef>, ExtError> {
        let _ = node;
        Ok(None)
    }
}

/// Mirrors `flm-core`'s `PointId`: a reference point's identifier, its
/// node's address and its index there. The guest's own: the api's holds a
/// node identifier, which only a tree makes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointId {
    /// The address of the point's node.
    pub addr: NodeAddr,
    /// The point's index among its node's points.
    pub index: usize,
}

/// `{"addr": {"tree", "node"}, "index"}`; the host's `point_id_to_value`.
impl JsonForm for PointId {
    fn to_value(&self) -> Value {
        json!({ "addr": self.addr.to_value(), "index": self.index })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a point's identifier";
        let map = object(WHAT, value)?;
        Ok(PointId { addr: get(WHAT, map, "addr")?, index: get(WHAT, map, "index")? })
    }
}

/// Mirrors [`ContentRef`](crate::api::model::aspects::ContentRef): where the
/// content a reference shows comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentRef {
    /// The argument at this position.
    Argument(usize),
    /// The argument of this name.
    ArgumentNamed(String),
    /// The body.
    Body,
    /// A node of the point's tree.
    Node(Node),
    /// The root of a tree the aspect built.
    Synthesized(Node),
}

/// `{"argument": n}`, `{"argument_named": name}`, `"body"`, `{"node":
/// handle}` or `{"synthesized": handle}`; read by the host's
/// `content_ref_from_value`.
impl JsonForm for ContentRef {
    fn to_value(&self) -> Value {
        match self {
            ContentRef::Argument(position) => json!({ "argument": position }),
            ContentRef::ArgumentNamed(name) => json!({ "argument_named": name }),
            ContentRef::Body => Value::String("body".to_string()),
            ContentRef::Node(node) => json!({ "node": node.to_value() }),
            ContentRef::Synthesized(node) => json!({ "synthesized": node.to_value() }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a content reference";
        if value.as_str() == Some("body") {
            return Ok(ContentRef::Body);
        }
        let (key, inner) = single(value, &["argument", "argument_named", "node", "synthesized"]).ok_or_else(|| {
            not_shape(WHAT, "`\"body\"` or a map of one of the keys `argument`, `argument_named`, `node`, `synthesized`", value)
        })?;
        Ok(match key {
            "argument" => ContentRef::Argument(JsonForm::from_value(inner)?),
            "argument_named" => ContentRef::ArgumentNamed(JsonForm::from_value(inner)?),
            "node" => ContentRef::Node(JsonForm::from_value(inner)?),
            _ => ContentRef::Synthesized(JsonForm::from_value(inner)?),
        })
    }
}

/// Mirrors [`AnchorPolicy`](crate::api::model::aspects::AnchorPolicy): how a
/// point's anchor is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorPolicy {
    /// From the point's first label.
    FromFirstLabel,
    /// This anchor.
    Explicit(String),
    /// A slug of some content, after a prefix.
    Slug {
        /// The prefix.
        prefix: String,
        /// The content.
        content: ContentRef,
    },
    /// From the element's value, after a prefix.
    FromValue {
        /// The prefix.
        prefix: String,
    },
    /// Numbered automatically, after a prefix.
    Auto {
        /// The prefix.
        prefix: String,
    },
}

/// `"from_first_label"`, `{"explicit": id}`, `{"slug": {"prefix",
/// "content"}}`, `{"from_value": {"prefix"}}` or `{"auto": {"prefix"}}`; read
/// by the host's reader of reference points.
impl JsonForm for AnchorPolicy {
    fn to_value(&self) -> Value {
        match self {
            AnchorPolicy::FromFirstLabel => Value::String("from_first_label".to_string()),
            AnchorPolicy::Explicit(id) => json!({ "explicit": id }),
            AnchorPolicy::Slug { prefix, content } => json!({ "slug": { "prefix": prefix, "content": content.to_value() } }),
            AnchorPolicy::FromValue { prefix } => json!({ "from_value": { "prefix": prefix } }),
            AnchorPolicy::Auto { prefix } => json!({ "auto": { "prefix": prefix } }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an anchor policy";
        if value.as_str() == Some("from_first_label") {
            return Ok(AnchorPolicy::FromFirstLabel);
        }
        let (key, inner) = single(value, &["explicit", "slug", "from_value", "auto"]).ok_or_else(|| {
            not_shape(WHAT, "`\"from_first_label\"` or a map of one of the keys `explicit`, `slug`, `from_value`, `auto`", value)
        })?;
        if key == "explicit" {
            return Ok(AnchorPolicy::Explicit(JsonForm::from_value(inner)?));
        }
        let map = object(WHAT, inner)?;
        let prefix = get(WHAT, map, "prefix")?;
        Ok(match key {
            "slug" => AnchorPolicy::Slug { prefix, content: get(WHAT, map, "content")? },
            "from_value" => AnchorPolicy::FromValue { prefix },
            _ => AnchorPolicy::Auto { prefix },
        })
    }
}

/// Mirrors [`NumberSource`](crate::api::model::aspects::NumberSource): where
/// a point's number comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberSource {
    /// The counter's name.
    pub counter: String,
    /// The nodes that are instances of the counter.
    pub instances: Vec<Node>,
    /// The counter's style.
    pub style: String,
    /// How far the counter advances, by level.
    pub advance: Vec<u32>,
    /// The number's format, if not the style's.
    pub number_format: Option<String>,
}

/// `{"counter", "instances", "style", "advance", "number_format"}`; read by
/// the host's reader of reference points.
impl JsonForm for NumberSource {
    fn to_value(&self) -> Value {
        json!({
            "counter": self.counter,
            "instances": self.instances.to_value(),
            "style": self.style,
            "advance": self.advance,
            "number_format": self.number_format,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a number source";
        let map = object(WHAT, value)?;
        Ok(NumberSource {
            counter: get(WHAT, map, "counter")?,
            instances: get::<Option<Vec<Node>>>(WHAT, map, "instances")?.unwrap_or_default(),
            style: get(WHAT, map, "style")?,
            advance: get(WHAT, map, "advance")?,
            number_format: get(WHAT, map, "number_format")?,
        })
    }
}

/// Mirrors [`RefPoint`](crate::api::model::aspects::RefPoint): a point a
/// reference can target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefPoint {
    /// The point's kind.
    pub kind: String,
    /// Its labels.
    pub labels: Vec<Label>,
    /// How many labels were computed.
    pub computed: usize,
    /// The labels that did not read.
    pub malformed: Vec<MalformedLabel>,
    /// How its anchor is made.
    pub anchor: AnchorPolicy,
    /// Where its number comes from, if it has one.
    pub number: Option<NumberSource>,
    /// The content a reference to it shows, if any.
    pub content: Option<ContentRef>,
    /// The label types it accepts.
    pub accepts: AcceptedRefTypes,
}

/// `{"kind", "labels", "computed", "malformed", "anchor", "number",
/// "content", "accepts"}`; read by the host's `ref_point_from_value`.
impl JsonForm for RefPoint {
    fn to_value(&self) -> Value {
        json!({
            "kind": self.kind,
            "labels": self.labels.to_value(),
            "computed": self.computed,
            "malformed": self.malformed.to_value(),
            "anchor": self.anchor.to_value(),
            "number": self.number.as_ref().map_or(Value::Null, JsonForm::to_value),
            "content": self.content.as_ref().map_or(Value::Null, JsonForm::to_value),
            "accepts": self.accepts.to_value(),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a reference point";
        let map = object(WHAT, value)?;
        Ok(RefPoint {
            kind: get(WHAT, map, "kind")?,
            labels: get::<Option<Vec<Label>>>(WHAT, map, "labels")?.unwrap_or_default(),
            computed: get::<Option<usize>>(WHAT, map, "computed")?.unwrap_or(0),
            malformed: get::<Option<Vec<MalformedLabel>>>(WHAT, map, "malformed")?.unwrap_or_default(),
            anchor: get(WHAT, map, "anchor")?,
            number: get(WHAT, map, "number")?,
            content: get(WHAT, map, "content")?,
            accepts: get(WHAT, map, "accepts")?,
        })
    }
}
