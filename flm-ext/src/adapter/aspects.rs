//! `ExtReferenceable` and `ExtSectioning`, `flm-core`'s aspects over the
//! handler aspects of the extension API ([`Referenceable`], [`Sectioning`]),
//! and the conversions of what the handlers answer: a reference point into
//! `flm-core`'s ([`ref_point_to_core`]), and a reference point read from data
//! ([`ref_point_from_value`], the shape a JSON binding writes).
//!
//! # One call
//!
//! The `refpoints` stage asks an element's `Referenceable` for the points of
//! each occurrence, and the `toc` stage asks its `Sectioning` for the level,
//! whether it is listed and the title, each on the thread that runs the stage.
//! Each call lends the handler the occurrence's node
//! ([`NodeFacade::lent`]) on that thread, as
//! [`ExtExpand::expand`](crate::adapter::expand::ExtExpand) does, converts the
//! answer, and keeps nothing across calls.
//!
//! # A failing handler
//!
//! `flm-core`'s aspect traits take a node and answer a value: neither stage
//! passes the aspect a place to report to. A handler that fails (an `Err`, or
//! an answer that does not convert: a node of another tree, a synthesized
//! content that is not the root of its tree, a span outside the node's source)
//! therefore makes the aspect answer as if the element carried none: no
//! reference point (a `\ref` to the occurrence is then unresolved, and the
//! occurrence gets no number and no anchor), the level 0, not listed in the
//! table of contents, no title. The failure is reported nowhere.

use std::sync::Arc;

use serde_json::Value;

use flm_core::lang::{Flm, FlmSpan};
use flm_core::model::aspects::{
    AnchorPolicy as CoreAnchorPolicy, ContentRef as CoreContentRef, MalformedLabel as CoreMalformedLabel,
    NumberSource as CoreNumberSource, RefPoint as CoreRefPoint, Referenceable as CoreReferenceable,
    Sectioning as CoreSectioning,
};
use flm_core::model::names::{Name, RefKind};
use flm_core::techy::core::node::{NodeId, NodeRef};
use flm_core::techy::source::SourceSpan;

use crate::adapter::handles::CallTable;
use crate::adapter::names::Owner;
use crate::adapter::node::NodeFacade;
use crate::adapter::values::{field, fields, handle_from_value, label_from_value, node_from_table, required_text};
use crate::api::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ContentRef, MalformedLabel, NumberSource, RefPoint, Referenceable, Sectioning,
};
use crate::api::techy::node::{Node, Span};
use crate::runtime::ExtError;
use crate::sealed::Token;

/// `flm-core`'s [`Referenceable`](CoreReferenceable) over an extension's
/// [`Referenceable`]: what an element's aspect holds once the element is
/// registered.
///
/// A handler that fails, or whose points do not convert, gives the
/// occurrence no reference point (the module's "A failing handler").
pub struct ExtReferenceable {
    /// The extension's handler.
    pub(crate) aspect: Arc<dyn Referenceable>,
    /// The extension and the package that attached the aspect.
    #[allow(dead_code, reason = "no diagnostics channel reaches an aspect yet; the owner names the extension once one does")]
    pub(crate) owner: Owner,
}

impl CoreReferenceable for ExtReferenceable {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<CoreRefPoint> {
        let site = NodeFacade::lent(*node);
        let answer = self.aspect.points(&site);
        answer
            .and_then(|points| points.into_iter().map(|point| ref_point_to_core(point, node)).collect())
            .unwrap_or_default()
    }
}

/// `flm-core`'s [`Sectioning`](CoreSectioning) over an extension's
/// [`Sectioning`]: what an element's aspect holds once the element is
/// registered.
///
/// A method whose handler fails answers as if the element carried no
/// aspect: the level 0, not listed, no title (the module's "A failing
/// handler").
pub struct ExtSectioning {
    /// The extension's handler.
    pub(crate) aspect: Arc<dyn Sectioning>,
    /// The extension and the package that attached the aspect.
    #[allow(dead_code, reason = "no diagnostics channel reaches an aspect yet; the owner names the extension once one does")]
    pub(crate) owner: Owner,
}

impl CoreSectioning for ExtSectioning {
    fn level(&self, node: &NodeRef<'_, Flm>) -> u32 {
        self.aspect.level(&NodeFacade::lent(*node)).unwrap_or(0)
    }

    fn in_toc(&self, node: &NodeRef<'_, Flm>) -> bool {
        self.aspect.in_toc(&NodeFacade::lent(*node)).unwrap_or(false)
    }

    fn toc_title(&self, node: &NodeRef<'_, Flm>) -> Option<CoreContentRef> {
        let site = NodeFacade::lent(*node);
        match self.aspect.toc_title(&site) {
            Ok(Some(title)) => content_ref_to_core(title, node).ok(),
            Ok(None) | Err(_) => None,
        }
    }
}

// --- into flm-core -----------------------------------------------------------------------------

/// The failure of a handler's answer that does not convert.
fn not_converted(detail: &str) -> ExtError {
    ExtError::Argument { method: "Referenceable::points".to_string(), detail: detail.to_string() }
}

/// The id of `content`, which must be a node of the tree `node` is in.
fn node_id_in(content: &dyn Node<'_>, node: &NodeRef<'_, Flm>) -> Result<NodeId, ExtError> {
    let core = content.core_node_ref(Token::new());
    if std::ptr::eq(core.tree(), node.tree()) {
        Ok(core.id())
    } else {
        Err(not_converted("the content of a reference point must be a node of the tree the point's node is in"))
    }
}

/// `span`, an offset range into the source of `node`, as a span on that
/// source.
fn span_on(span: Span, node: &NodeRef<'_, Flm>) -> Result<FlmSpan, ExtError> {
    let source = node.span().source();
    let text = source.content();
    let fits = span.start <= span.end
        && span.end <= text.len()
        && text.is_char_boundary(span.start)
        && text.is_char_boundary(span.end);
    if !fits {
        return Err(not_converted(&format!(
            "the span {}..{} of a malformed label must lie within the source of the point's node, which has {} bytes",
            span.start,
            span.end,
            text.len()
        )));
    }
    Ok(SourceSpan::new(source, span.start..span.end))
}

/// `content` as `flm-core`'s: a node resolved to its id in the tree of
/// `node`, a synthesized root to the tree it is the root of.
///
/// # Errors
///
/// A node of another tree than `node`'s; a synthesized content that is not
/// the root node of a tree shared through an `Arc` ([`ExtError::Argument`]).
pub fn content_ref_to_core(content: ContentRef<'_>, node: &NodeRef<'_, Flm>) -> Result<CoreContentRef, ExtError> {
    Ok(match content {
        ContentRef::Argument(position) => CoreContentRef::Argument(position),
        ContentRef::ArgumentNamed(name) => CoreContentRef::ArgumentNamed(name),
        ContentRef::Body => CoreContentRef::Body,
        ContentRef::Node(content) => CoreContentRef::Node(node_id_in(&*content, node)?),
        ContentRef::Synthesized(root) => {
            let id = root.core_node_ref(Token::new()).id();
            let tree = root
                .shared_tree(Token::new())
                .filter(|tree| tree.root().id() == id)
                .ok_or_else(|| not_converted("the synthesized content of a reference point must be the root node of a tree"))?;
            CoreContentRef::Synthesized(tree)
        }
    })
}

/// `point`, answered for the occurrence `node`, as `flm-core`'s: every node
/// resolved to its id in the tree of `node`, every span placed on the source
/// of `node`, a synthesized root to its shared tree.
///
/// # Errors
///
/// As [`content_ref_to_core`]; a number source's instance of another tree,
/// or a malformed label's span outside the source of `node`
/// ([`ExtError::Argument`]).
pub fn ref_point_to_core(point: RefPoint<'_>, node: &NodeRef<'_, Flm>) -> Result<CoreRefPoint, ExtError> {
    let anchor = match point.anchor {
        AnchorPolicy::Explicit(id) => CoreAnchorPolicy::Explicit(id),
        AnchorPolicy::FromFirstLabel => CoreAnchorPolicy::FromFirstLabel,
        AnchorPolicy::Slug { prefix, content } => {
            CoreAnchorPolicy::Slug { prefix, content: content_ref_to_core(content, node)? }
        }
        AnchorPolicy::FromValue { prefix } => CoreAnchorPolicy::FromValue { prefix },
        AnchorPolicy::Auto { prefix } => CoreAnchorPolicy::Auto { prefix },
    };
    let number = match point.number {
        None => None,
        Some(source) => Some(CoreNumberSource {
            counter: source.counter,
            instances: source.instances.iter().map(|instance| node_id_in(&**instance, node)).collect::<Result<_, _>>()?,
            style: source.style,
            advance: source.advance,
            number_format: source.number_format,
        }),
    };
    let malformed = point
        .malformed
        .into_iter()
        .map(|label| Ok(CoreMalformedLabel { text: label.text, span: span_on(label.span, node)?, position: label.position }))
        .collect::<Result<_, ExtError>>()?;
    Ok(CoreRefPoint {
        kind: point.kind,
        labels: point.labels,
        computed: point.computed,
        malformed,
        anchor,
        number,
        content: point.content.map(|content| content_ref_to_core(content, node)).transpose()?,
        accepts: point.accepts,
    })
}

// --- from data ---------------------------------------------------------------------------------

/// The error of a field of `what` that is not `expected`.
fn bad_field(what: &str, key: &str, expected: &str, value: &Value) -> ExtError {
    ExtError::Value { detail: format!("the field `{key}` of {what} must be {expected}, not `{value}`") }
}

/// A list field, empty when absent or `null`.
fn list_field<'v>(what: &str, map: &std::collections::BTreeMap<String, &'v Value>, key: &str) -> Result<&'v [Value], ExtError> {
    match field(map, key) {
        Value::Null => Ok(&[]),
        Value::Array(items) => Ok(items),
        other => Err(bad_field(what, key, "a list", other)),
    }
}

/// A number field from 0 up, `default` when absent or `null`.
fn count_field(what: &str, map: &std::collections::BTreeMap<String, &Value>, key: &str, default: usize) -> Result<usize, ExtError> {
    match field(map, key) {
        Value::Null => Ok(default),
        other => other
            .as_u64()
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| bad_field(what, key, "a number from 0 up", other)),
    }
}

/// The map of one key `key` that `value` is, with the value under the key,
/// or `None` when `value` is not such a map.
fn single_key(value: &Value) -> Option<(&str, &Value)> {
    let Value::Object(map) = value else { return None };
    let mut entries = map.iter();
    match (entries.next(), entries.next()) {
        (Some((key, inner)), None) => Some((key.as_str(), inner)),
        _ => None,
    }
}

/// A content reference: `{"argument": i}`, `{"argument_named": name}`,
/// `"body"`, `{"node": node}` (a node handle of the call), or
/// `{"synthesized": node}` (the handle of the root node of a tree the call
/// holds).
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); a handle is stale or
/// stands for no node.
pub fn content_ref_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<ContentRef<'c>, ExtError> {
    let shape = || ExtError::Value {
        detail: format!(
            "a content reference must be `\"body\"` or a map of one of the keys `argument`, `argument_named`, `node`, \
             `synthesized`, not `{value}`"
        ),
    };
    if value.as_str() == Some("body") {
        return Ok(ContentRef::Body);
    }
    let (key, inner) = single_key(value).ok_or_else(shape)?;
    match key {
        "argument" => inner
            .as_u64()
            .and_then(|position| usize::try_from(position).ok())
            .map(ContentRef::Argument)
            .ok_or_else(|| bad_field("a content reference", key, "a number from 0 up", inner)),
        "argument_named" => inner
            .as_str()
            .map(|name| ContentRef::ArgumentNamed(name.to_string()))
            .ok_or_else(|| bad_field("a content reference", key, "a string", inner)),
        "node" => Ok(ContentRef::Node(node_from_table(table, handle_from_value(inner)?)?)),
        "synthesized" => Ok(ContentRef::Synthesized(node_from_table(table, handle_from_value(inner)?)?)),
        _ => Err(shape()),
    }
}

/// An anchor policy: `"from_first_label"`, `{"explicit": id}`, `{"slug":
/// {"prefix", "content"}}`, `{"from_value": {"prefix"}}` or `{"auto":
/// {"prefix"}}`.
///
/// # Errors
///
/// As [`content_ref_from_value`].
fn anchor_policy_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<AnchorPolicy<'c>, ExtError> {
    const WHAT: &str = "an anchor policy";
    let shape = || ExtError::Value {
        detail: format!(
            "{WHAT} must be `\"from_first_label\"` or a map of one of the keys `explicit`, `slug`, `from_value`, `auto`, \
             not `{value}`"
        ),
    };
    if value.as_str() == Some("from_first_label") {
        return Ok(AnchorPolicy::FromFirstLabel);
    }
    let (key, inner) = single_key(value).ok_or_else(shape)?;
    match key {
        "explicit" => {
            inner.as_str().map(|id| AnchorPolicy::Explicit(id.to_string())).ok_or_else(|| bad_field(WHAT, key, "a string", inner))
        }
        "slug" => {
            let map = fields("a slug anchor policy", inner, &["prefix", "content"])?;
            let content = match field(&map, "content") {
                Value::Null => return Err(ExtError::Value { detail: "a slug anchor policy must have the field `content`".to_string() }),
                content => content_ref_from_value(table, content)?,
            };
            Ok(AnchorPolicy::Slug { prefix: required_text("a slug anchor policy", &map, "prefix")?, content })
        }
        "from_value" => {
            let map = fields("a from-value anchor policy", inner, &["prefix"])?;
            Ok(AnchorPolicy::FromValue { prefix: required_text("a from-value anchor policy", &map, "prefix")? })
        }
        "auto" => {
            let map = fields("an auto anchor policy", inner, &["prefix"])?;
            Ok(AnchorPolicy::Auto { prefix: required_text("an auto anchor policy", &map, "prefix")? })
        }
        _ => Err(shape()),
    }
}

/// A number source: `{"counter", "instances": [node, …], "style", "advance":
/// [level, …], "number_format": text | null}`; `counter`, `style` and
/// `advance` required.
fn number_source_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<NumberSource<'c>, ExtError> {
    const WHAT: &str = "a number source";
    let map = fields(WHAT, value, &["counter", "instances", "style", "advance", "number_format"])?;
    let instances = list_field(WHAT, &map, "instances")?
        .iter()
        .map(|item| node_from_table(table, handle_from_value(item)?))
        .collect::<Result<_, _>>()?;
    let advance = match field(&map, "advance") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `advance`") }),
        Value::Array(levels) => levels
            .iter()
            .map(|level| level.as_u64().and_then(|level| u32::try_from(level).ok()))
            .collect::<Option<Vec<u32>>>()
            .ok_or_else(|| bad_field(WHAT, "advance", "a list of numbers from 0 up", field(&map, "advance")))?,
        other => return Err(bad_field(WHAT, "advance", "a list of numbers from 0 up", other)),
    };
    let number_format = match field(&map, "number_format") {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => return Err(bad_field(WHAT, "number_format", "a string or `null`", other)),
    };
    Ok(NumberSource {
        counter: Name::from(required_text(WHAT, &map, "counter")?),
        instances,
        style: Name::from(required_text(WHAT, &map, "style")?),
        advance,
        number_format,
    })
}

/// A malformed label `{"text", "span": {"start", "end"}, "position"}`.
pub(crate) fn malformed_label_from_value(value: &Value) -> Result<MalformedLabel, ExtError> {
    const WHAT: &str = "a malformed label";
    let map = fields(WHAT, value, &["text", "span", "position"])?;
    let span: Span = serde_json::from_value(field(&map, "span").clone())
        .map_err(|_| bad_field(WHAT, "span", "a map `{\"start\", \"end\"}` of two numbers from 0 up", field(&map, "span")))?;
    let position = match field(&map, "position") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `position`") }),
        _ => count_field(WHAT, &map, "position", 0)?,
    };
    Ok(MalformedLabel { text: required_text(WHAT, &map, "text")?, span, position })
}

/// The label types a point accepts: `"any"` or `{"these": [type, …]}`.
pub(crate) fn accepted_from_value(value: &Value) -> Result<AcceptedRefTypes, ExtError> {
    let shape = || ExtError::Value {
        detail: format!("the label types a point accepts must be `\"any\"` or `{{\"these\": [type, …]}}`, not `{value}`"),
    };
    match value {
        Value::Null => Ok(AcceptedRefTypes::Any),
        Value::String(text) if text == "any" => Ok(AcceptedRefTypes::Any),
        _ => match single_key(value) {
            Some(("these", Value::Array(types))) => types
                .iter()
                .map(|item| item.as_str().map(str::to_string))
                .collect::<Option<Vec<String>>>()
                .map(AcceptedRefTypes::These)
                .ok_or_else(shape),
            _ => Err(shape()),
        },
    }
}

/// A reference point in its data form, its nodes handles of the call
/// `table` stands for: `{"kind", "labels": [label, …], "computed",
/// "malformed": [malformed label, …], "anchor", "number": number source |
/// null, "content": content reference | null, "accepts"}`. `kind` and
/// `anchor` are required; `labels` and `malformed` default to empty lists,
/// `computed` to 0, `number` and `content` to `null`, `accepts` to `"any"`.
/// A label is `{"ref_type", "ref_label"}`; a malformed label `{"text",
/// "span": {"start", "end"}, "position"}`; the anchor, the number source and
/// the content as `flm-core`'s variants by their names (`"from_first_label"`,
/// `{"slug": {"prefix", "content"}}`, `{"argument_named": name}`, …).
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]: "the field `computed`
/// of a reference point must be a number from 0 up, not `-1`"); a handle is
/// stale or stands for no node.
pub fn ref_point_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<RefPoint<'c>, ExtError> {
    const WHAT: &str = "a reference point";
    let map = fields(WHAT, value, &["kind", "labels", "computed", "malformed", "anchor", "number", "content", "accepts"])?;
    let labels = list_field(WHAT, &map, "labels")?.iter().map(label_from_value).collect::<Result<_, _>>()?;
    let malformed = list_field(WHAT, &map, "malformed")?.iter().map(malformed_label_from_value).collect::<Result<_, _>>()?;
    let anchor = match field(&map, "anchor") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `anchor`") }),
        anchor => anchor_policy_from_value(table, anchor)?,
    };
    let number = match field(&map, "number") {
        Value::Null => None,
        number => Some(number_source_from_value(table, number)?),
    };
    let content = match field(&map, "content") {
        Value::Null => None,
        content => Some(content_ref_from_value(table, content)?),
    };
    Ok(RefPoint {
        kind: RefKind::from(required_text(WHAT, &map, "kind")?),
        labels,
        computed: count_field(WHAT, &map, "computed", 0)?,
        malformed,
        anchor,
        number,
        content,
        accepts: accepted_from_value(field(&map, "accepts"))?,
    })
}

/// A list of reference points in their data form, as
/// [`ref_point_from_value`] reads each: what a JSON binding's `points`
/// answers.
///
/// # Errors
///
/// The value is not a list, or a point does not read.
pub fn ref_points_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<Vec<RefPoint<'c>>, ExtError> {
    match value {
        Value::Array(items) => items.iter().map(|item| ref_point_from_value(table, item)).collect(),
        other => Err(ExtError::Value { detail: format!("the reference points must be a list, not `{other}`") }),
    }
}
