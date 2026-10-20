//! The HTML construct element: one HTML tag, as a node in the lowered tree.
//!
//! A construct element is a presentation element that mirrors one fundamental
//! construct of an output format, so that format-specific output is structured
//! data in the lowered tree rather than a string (see [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! HTML's fundamental construct is the tag, and this module holds the only
//! element that mirrors it, [`TAG`] (`flm.pres.html.tag`), with its payload
//! [`HtmlTagPayload`]: a tag name, attributes, and either children or one
//! string of HTML that is already formed ([`Markup`]).
//!
//! | element | payload | slots |
//! |---|---|---|
//! | [`TAG`] (`flm.pres.html.tag`) | [`HtmlTagPayload`]: the tag name, the attributes in order, the body, and whether the tag is block-level | one [`BODY_SLOT`](crate::stdcore::backends::pres::BODY_SLOT) when the body is [`TagBody::Children`]; none when it is [`TagBody::Markup`] |
//!
//! The `html` feature ([`super::HtmlBackend`]), which registers the HTML
//! backend, registers this element through [`register`], and the HTML backend
//! writes it natively. It is **not** part of the shared presentation
//! vocabulary ([`ALL`](crate::stdcore::backends::pres::ALL)): a backend that
//! is not an HTML backend cannot write it, and a lowering rule that stages it
//! (builds a node of it into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) is a rule for HTML. A feature
//! that stages it without registering the `html` feature itself declares it
//! through a peer declaration ([`PeerName`](crate::core::registry::PeerName),
//! [`FeatureBuilder::peer_element`](crate::core::registry::FeatureBuilder::peer_element)).
//!
//! # When a rule uses it
//!
//! Rarely. A lowering rule normally builds with the shared presentation
//! vocabulary (a paragraph, a block with a role, a span with a style, a list,
//! a link, an image), which every backend can write, so the rule's output is
//! also written as plain text and as LaTeX. A rule stages a tag when it must
//! put HTML in the page that no shared element carries: a formula an external
//! tool typeset as `<svg>`, a widget a feature of its own defines, a tag the
//! browser must see for a script to find it.
//!
//! The rule's author is then responsible for everything about that tag. The
//! [HTML contract](super::CONVENTIONS) (the classes and attributes derived
//! from the presentation vocabulary that a stylesheet may rely on) does not
//! apply to a construct node: the emitter writes the tag, the attributes and
//! the body as the payload states them and adds nothing of its own. The
//! [address policy](super::check_address), which refuses an address a browser would
//! execute, does not apply either. So the rule's author writes the classes its
//! own stylesheet needs, checks the addresses it writes, and, where the body
//! is markup, accepts the requirement stated at
//! [`Markup::trusted`](super::Markup::trusted): the bytes are HTML that is
//! safe to place in a page, and anything that came from a document or from
//! another input the author does not control is sanitized first. The emitter
//! still checks the tag name and the attribute names, and writes a node whose
//! name fails as the unsupported stand-in
//! ([`flm.html.invalid-name`](super::emit::InvalidName)).

use std::sync::Arc;

use techy::serialize::{DeserializeContext, DeserializeError, SerialValue};

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::dump::session::PayloadReaders;
use crate::lang::{Flm, PresPayload};
use crate::stdcore::backends::pres::payload::{bad, entries, field, map, string, text};

use super::markup::Markup;

/// `flm.pres.html.tag` — one HTML tag: a name, attributes, and either children
/// or one string of HTML that is already formed ([`Markup`]).
pub const TAG: &str = "flm.pres.html.tag";

/// The content between the opening and the closing tag of a [`TAG`] node.
///
/// A tag either holds content the lowering rule built out of presentation
/// nodes, which the emitter writes as it writes any other content, or one
/// [`Markup`] value: HTML that is already formed and is written through
/// unchanged. It
/// cannot hold both: markup the emitter could not look inside and nodes it
/// would have to place around it have no defined order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TagBody {
    /// The node's content slot ([`BODY_SLOT`](crate::stdcore::backends::pres::BODY_SLOT))
    /// holds the body, as lowered nodes. This is the default: a tag with no
    /// content at all is a `Children` body whose slot is empty.
    #[default]
    Children,
    /// The body is this HTML, written through unchanged, and the node is
    /// childless. See [`Markup`] for the requirement its creator accepts.
    Markup(Markup),
}

/// The payload of `flm.pres.html.tag`: one HTML tag as structured data.
///
/// A lowering rule stages a node of this element instead of assembling a
/// string of HTML; the emitter writes the tag from the payload, escaping every
/// attribute value, and writes the body as [`TagBody`] says (see [the module
/// documentation](self) for when a rule uses it and what its author is then
/// responsible for). Build one with [`new`](Self::new) and the chained
/// setters.
///
/// # A tag inside a formula
///
/// Mathematics is handed to a formula renderer in the reader's browser as
/// LaTeX, and content inside the formula is written as HTML inside that LaTeX.
/// The renderer finds the end of a formula by searching all its characters,
/// the HTML written inside it included, for the closing delimiter and for its
/// `<tex-html>` tag. So a tag staged inside a formula is written under two
/// extra rules:
///
/// - its **attribute values** are checked by the emitter, as every attribute
///   written there is, and one holding the closing delimiter of the
///   surrounding formula (`\)`, `\]` or `\end{<env>}`), `<tex-html` or
///   `</tex-html` is left out and reported
///   ([`flm.html.refused-in-math`](super::emit::RefusedInMath)), the tag and
///   its content kept. The emitter does this because a rule cannot: only the
///   emitter knows which delimiter it wrote around the formula.
/// - a **[`TagBody::Markup`] body is written unchanged**, there as anywhere
///   else. Inside a formula, the author of those bytes must therefore ensure
///   one thing more than the requirement of [`Markup::trusted`]: the markup
///   must not hold the closing delimiter of the surrounding formula, nor a
///   `<tex-html>` or `</tex-html>` tag. Markup that holds one ends the formula
///   early, and the reader gets an error in place of the formula.
///
/// A tag named `tex-html` is the renderer's element itself: a rule that writes
/// a formula as text stages one to put HTML inside it. The emitter repairs
/// the text written among its children as it repairs the text inside its own
/// `<tex-html>` elements, breaking `\)`, `\]`, `\end{`, `<tex-html` and
/// `</tex-html` with an empty element; a [`TagBody::Markup`] body is still
/// written unchanged.
///
/// [`Markup::trusted`]: super::Markup::trusted
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HtmlTagPayload {
    /// The tag name, without its angle brackets (`div`, `svg`, `tex-html`).
    ///
    /// The emitter checks it
    /// ([`is_valid_tag_name`](super::names::is_valid_tag_name)); a node whose
    /// name fails is reported and written as the unsupported stand-in.
    pub tag: String,
    /// The attributes, each a name and a value, written in this order.
    ///
    /// The names are checked as the tag name is
    /// ([`is_valid_attribute_name`](super::names::is_valid_attribute_name));
    /// the values are escaped where they are written and may hold anything.
    pub attributes: Vec<(String, String)>,
    /// The content inside the tag.
    pub body: TagBody,
    /// Whether the tag is **block-level**: whether the emitter may write a
    /// line break and indentation around it.
    ///
    /// The rule that stages the tag knows whether it is block-level (a `<div>`
    /// is, a `<span>` is not), and the sink takes this field rather than a
    /// list of tag names, so that a tag the rule invented is laid out as its
    /// author meant. Inside inline content, and inside a preformatted region,
    /// no whitespace is added regardless of this field.
    pub block: bool,
}

impl HtmlTagPayload {
    /// Returns a tag with the given name, no attributes, a body held by its
    /// node's content slot, and inline layout.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::{HtmlTagPayload, TagBody};
    ///
    /// let payload = HtmlTagPayload::new("span");
    /// assert_eq!(payload.tag, "span");
    /// assert_eq!(payload.body, TagBody::Children);
    /// assert!(!payload.block);
    /// ```
    pub fn new(tag: impl Into<String>) -> HtmlTagPayload {
        HtmlTagPayload {
            tag: tag.into(),
            attributes: Vec::new(),
            body: TagBody::Children,
            block: false,
        }
    }

    /// Returns the same tag with `attributes`, written in the order given.
    pub fn attributes<N, V>(mut self, attributes: impl IntoIterator<Item = (N, V)>) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        self.attributes =
            attributes.into_iter().map(|(name, value)| (name.into(), value.into())).collect();
        self
    }

    /// Returns the same tag with `markup` as its body, which makes the node
    /// childless.
    pub fn markup(mut self, markup: Markup) -> Self {
        self.body = TagBody::Markup(markup);
        self
    }

    /// Returns the same tag, marked as block-level (the `block` field).
    pub fn block(mut self) -> Self {
        self.block = true;
        self
    }

    /// Rebuilds the payload from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] that identifies `flm.pres.html.tag` when the
    /// projection is not a map, when `tag` is missing or is not a string, when
    /// `attributes` is present and is not a list of two-element lists of
    /// strings, when `markup` is present and is not a string, or when `block`
    /// is present and is not a boolean.
    pub fn from_projection(value: &SerialValue) -> Result<HtmlTagPayload, DeserializeError> {
        let e = entries(TAG, value)?;
        let tag = string(TAG, e, "tag")?;
        let attributes = match field(e, "attributes") {
            None | Some(SerialValue::Null) => Vec::new(),
            Some(SerialValue::List(items)) => items
                .iter()
                .map(|item| match item {
                    SerialValue::List(pair) => match pair.as_slice() {
                        [SerialValue::Str(name), SerialValue::Str(value)] => {
                            Ok((name.clone(), value.clone()))
                        }
                        other => Err(bad(
                            TAG,
                            format!("`attributes` holds {other:?}, not a name and a value"),
                        )),
                    },
                    other => {
                        Err(bad(TAG, format!("`attributes` holds a non-list {other:?}")))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(other) => {
                return Err(bad(TAG, format!("`attributes` must be a list, found {other:?}")));
            }
        };
        let body = match field(e, "markup") {
            None | Some(SerialValue::Null) => TagBody::Children,
            Some(SerialValue::Str(html)) => TagBody::Markup(Markup::from_library(html.clone())),
            Some(other) => {
                return Err(bad(TAG, format!("`markup` must be a string, found {other:?}")));
            }
        };
        let block = match field(e, "block") {
            None | Some(SerialValue::Null) => false,
            Some(SerialValue::Bool(block)) => *block,
            Some(other) => {
                return Err(bad(TAG, format!("`block` must be a boolean, found {other:?}")));
            }
        };
        Ok(HtmlTagPayload { tag, attributes, body, block })
    }
}

impl PresPayload for HtmlTagPayload {
    fn project(&self) -> SerialValue {
        let mut fields = vec![("tag", text(&self.tag))];
        if !self.attributes.is_empty() {
            fields.push((
                "attributes",
                SerialValue::List(
                    self.attributes
                        .iter()
                        .map(|(name, value)| {
                            SerialValue::List(vec![text(name.as_str()), text(value.as_str())])
                        })
                        .collect(),
                ),
            ));
        }
        if let TagBody::Markup(markup) = &self.body {
            fields.push(("markup", text(markup.as_str())));
        }
        if self.block {
            fields.push(("block", SerialValue::Bool(true)));
        }
        map(fields)
    }
}

/// Returns the payload reader of the construct element: what a reading session
/// ([`reader_with`](crate::dump::session::reader_with)) needs, beside the
/// shared vocabulary's own
/// ([`payload_readers`](crate::stdcore::backends::pres::payload_readers)), to
/// read a stored lowered tree of the HTML backend back into typed payloads.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::construct::{TAG, payload_readers};
///
/// let readers = payload_readers();
/// assert_eq!(readers.len(), 1);
/// assert!(readers.elements().any(|element| element.as_str() == TAG));
/// ```
pub fn payload_readers() -> PayloadReaders {
    let mut readers = PayloadReaders::new();
    readers.register(TAG, |value: &SerialValue, _cx: &mut DeserializeContext<'_, Flm>| {
        HtmlTagPayload::from_projection(value)
            .map(|payload| Arc::new(payload) as Arc<dyn PresPayload>)
    });
    readers
}

/// Registers the construct element `flm.pres.html.tag` with its payload type.
/// The `html` feature ([`super::HtmlBackend`]) calls this from its own
/// registration; a second registration in one registry declares the element
/// twice and fails the build.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises for a presentation
/// element already declared in this registry.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.presentation_element::<HtmlTagPayload>(
        TAG,
        "one HTML tag: its name, its attributes, and either children or one piece of markup",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_projection_holds_the_keys_that_have_a_value() {
        let plain = HtmlTagPayload::new("span");
        assert_eq!(plain.project(), map(vec![("tag", text("span"))]));

        let full = HtmlTagPayload::new("div")
            .attributes([("class", "mine"), ("data-x", "1")])
            .markup(Markup::trusted("<svg></svg>"))
            .block();
        assert_eq!(
            full.project(),
            map(vec![
                ("tag", text("div")),
                (
                    "attributes",
                    SerialValue::List(vec![
                        SerialValue::List(vec![text("class"), text("mine")]),
                        SerialValue::List(vec![text("data-x"), text("1")]),
                    ])
                ),
                ("markup", text("<svg></svg>")),
                ("block", SerialValue::Bool(true)),
            ])
        );
    }

    #[test]
    fn a_projection_reads_back_as_the_payload_it_came_from() {
        for payload in [
            HtmlTagPayload::new("span"),
            HtmlTagPayload::new("div").block(),
            HtmlTagPayload::new("tex-html")
                .attributes([("id", "eq-1")])
                .markup(Markup::trusted("<span></span>")),
        ] {
            let read = HtmlTagPayload::from_projection(&payload.project()).expect("reads back");
            assert_eq!(read, payload);
        }
    }

    #[test]
    fn a_projection_that_is_not_one_names_the_element_and_the_key() {
        let cases = [
            (map(vec![]), "`tag` is missing"),
            (map(vec![("tag", SerialValue::Int(3))]), "`tag` must be a string"),
            (map(vec![("tag", text("p")), ("block", text("yes"))]), "`block` must be a boolean"),
            (
                map(vec![("tag", text("p")), ("markup", SerialValue::Bool(true))]),
                "`markup` must be a string",
            ),
            (
                map(vec![("tag", text("p")), ("attributes", text("class"))]),
                "`attributes` must be a list",
            ),
        ];
        for (projection, expected) in &cases {
            let error = HtmlTagPayload::from_projection(projection).expect_err("refused");
            let message = error.to_string();
            assert!(message.contains(TAG), "{message}");
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn an_attributes_list_of_the_wrong_shape_is_refused() {
        let projection = map(vec![
            ("tag", text("p")),
            (
                "attributes",
                SerialValue::List(vec![SerialValue::List(vec![text("only-a-name")])]),
            ),
        ]);
        let error = HtmlTagPayload::from_projection(&projection).expect_err("refused");
        assert!(error.to_string().contains("not a name and a value"), "{error}");
    }
}
