//! The payload table: for each presentation element whose payload an
//! extension may give as data, the reader that rebuilds the payload from its
//! projection.
//!
//! The table holds every element of the standard presentation vocabulary
//! ([`pres::names::ALL`](flm_core::stdcore::backends::pres::names::ALL)), each
//! read through its payload type's `from_projection`, and the HTML construct
//! [`flm.pres.html.tag`](flm_core::stdcore::backends::html::construct::TAG)
//! with children ([`HtmlTagPayload::from_projection`]), and the four
//! construct elements of the LaTeX family
//! ([`ALL`](flm_core::stdcore::backends::latex::construct::ALL)), each read
//! through its payload type's `from_projection`; a tag whose payload holds
//! `markup` (the construct's markup body) is refused, since that body is not
//! offered to extensions.
//!
//! Beside the table, [`DataPayload`]: the one payload type of every
//! presentation element a scripted feature declares
//! ([`FeatureBuilder::presentation_element`](crate::api::core::registry::FeatureBuilder::presentation_element)),
//! whose payload is the data value the rule staged. A rule staging an element
//! a package declared this way, its own or a peer's, gives the payload as any
//! data value; the rule must list the element among those it produces.

use std::sync::Arc;

use flm_core::dump::json::from_json;
use flm_core::lang::PresPayload;
use flm_core::stdcore::backends::html::construct::{HtmlTagPayload, TagBody, TAG};
use flm_core::stdcore::backends::latex::construct::{DEFINITION, ENVIRONMENT, MACRO, RAW};
use flm_core::stdcore::backends::latex::{
    LatexDefinitionPayload, LatexEnvironmentPayload, LatexMacroPayload, LatexRawPayload,
};
use flm_core::stdcore::backends::pres::names;
use flm_core::stdcore::backends::pres::payload::{
    BlockPayload, CellsPayload, DecoMarkPayload, DecorationPayload, DroppedPayload, HeadingPayload, ImagePayload,
    LinePayload, LinesPayload, LinkPayload, ListPayload, MathPassthroughPayload, MathTextPayload, ParPayload,
    SpanPayload, SubtreePayload, UnknownPayload, UnsupportedPayload, VerbatimPayload,
};
use flm_core::techy::serialize::{DeserializeError, SerialValue};
use serde_json::Value;

use crate::runtime::ExtError;

/// The payload of a presentation element an extension declared
/// ([`FeatureBuilder::presentation_element`](crate::api::core::registry::FeatureBuilder::presentation_element)):
/// the data value the rule staged, checked when it crossed (no
/// floating-point number, no reserved key).
///
/// A Rust rule or emitter reads it with
/// [`payload_of::<DataPayload>`](flm_core::stdcore::backends::pres::payload_of);
/// a scripted one with [`Node::payload`](crate::api::techy::node::Node::payload).
///
/// No reader of the saved form is registered for it
/// ([`PayloadReaders`](flm_core::dump::session::PayloadReaders)), so a saved
/// tree holding such an element cannot be read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPayload(pub Value);

impl PresPayload for DataPayload {
    /// The value, converted to techy's value model through its JSON text; a
    /// value that does not convert (which the check on crossing excludes)
    /// projects as `null`.
    fn project(&self) -> SerialValue {
        from_json(&self.0.to_string()).unwrap_or(SerialValue::Null)
    }
}

/// A reader: the payload of `element`, rebuilt from its projection.
type Reader = fn(&str, &SerialValue) -> Result<Arc<dyn PresPayload>, DeserializeError>;

/// Boxes a payload for the table.
fn boxed<P: PresPayload>(payload: P) -> Arc<dyn PresPayload> {
    Arc::new(payload)
}

/// The table, in the order of [`names::ALL`], then the HTML construct, then
/// the LaTeX family's constructs in their registration order.
const READERS: [(&str, Reader); 27] = [
    (names::PAR, |_, value| ParPayload::from_projection(value).map(boxed)),
    (names::BLOCK, |_, value| BlockPayload::from_projection(value).map(boxed)),
    (names::BLOCK_HEADING, |element, value| HeadingPayload::from_projection(element, value).map(boxed)),
    (names::RUNIN_HEADING, |element, value| HeadingPayload::from_projection(element, value).map(boxed)),
    (names::SPAN, |_, value| SpanPayload::from_projection(value).map(boxed)),
    (names::LIST, |_, value| ListPayload::from_projection(value).map(boxed)),
    (names::CELLS, |_, value| CellsPayload::from_projection(value).map(boxed)),
    (names::LINES, |_, value| LinesPayload::from_projection(value).map(boxed)),
    (names::LINE, |_, value| LinePayload::from_projection(value).map(boxed)),
    (names::VERBATIM, |_, value| VerbatimPayload::from_projection(value).map(boxed)),
    (names::MATH_PASSTHROUGH, |_, value| MathPassthroughPayload::from_projection(value).map(boxed)),
    (names::MATH_TEXT, |_, value| MathTextPayload::from_projection(value).map(boxed)),
    (names::LINK, |_, value| LinkPayload::from_projection(value).map(boxed)),
    (names::IMAGE, |_, value| ImagePayload::from_projection(value).map(boxed)),
    (names::UNSUPPORTED, |_, value| UnsupportedPayload::from_projection(value).map(boxed)),
    (names::DROPPED, |_, value| DroppedPayload::from_projection(value).map(boxed)),
    (names::UNKNOWN, |_, value| UnknownPayload::from_projection(value).map(boxed)),
    (names::DECO_RUNIN, |element, value| DecorationPayload::from_projection(element, value).map(boxed)),
    (names::DECO_FINAL, |element, value| DecorationPayload::from_projection(element, value).map(boxed)),
    (names::DECO_MARGIN, |element, value| DecorationPayload::from_projection(element, value).map(boxed)),
    (names::DECO_MARK, |_, value| DecoMarkPayload::from_projection(value).map(boxed)),
    (names::SUBTREE, |_, value| SubtreePayload::from_projection(value).map(boxed)),
    (TAG, |_, value| HtmlTagPayload::from_projection(value).map(boxed)),
    (MACRO, |_, value| LatexMacroPayload::from_projection(value).map(boxed)),
    (ENVIRONMENT, |_, value| LatexEnvironmentPayload::from_projection(value).map(boxed)),
    (RAW, |_, value| LatexRawPayload::from_projection(value).map(boxed)),
    (DEFINITION, |_, value| LatexDefinitionPayload::from_projection(value).map(boxed)),
];

/// The names of the elements the table holds, in its order.
pub fn elements() -> impl Iterator<Item = &'static str> {
    READERS.iter().map(|(element, _)| *element)
}

/// The payload of a node of `element`, read from `payload`, the projection of
/// the element's payload type in its JSON form.
///
/// # Errors
///
/// [`ExtError::Argument`] naming `method`: `element` is not in the table (the
/// message lists the table's names, and names the elements declared with a
/// data payload, which the lowerer stages before reaching this function); the payload is not of the element's
/// shape; a tag's payload holds `markup`.
pub fn payload_from_value(method: &str, element: &str, payload: &Value) -> Result<Arc<dyn PresPayload>, ExtError> {
    let refuse = |detail: String| ExtError::Argument { method: method.to_string(), detail };
    let Some((_, reader)) = READERS.iter().find(|(name, _)| *name == element) else {
        let names: Vec<String> = elements().map(|name| format!("`{name}`")).collect();
        return Err(refuse(format!(
            "the element `{element}` must be one of the presentation elements whose payload can be given as data: \
             {}, or a presentation element a package declared with a data payload that this rule declares it \
             produces",
            names.join(", ")
        )));
    };
    let projection = from_json(&payload.to_string()).map_err(|error| refuse(error.to_string()))?;
    let built = reader(element, &projection).map_err(|error| refuse(error.to_string()))?;
    if element == TAG {
        // A tag's payload is read again to see its body: `PresPayload` gives no
        // access to the concrete type.
        if let Ok(HtmlTagPayload { body: TagBody::Markup(_), .. }) = HtmlTagPayload::from_projection(&projection) {
            return Err(refuse(format!("the payload of `{TAG}` must not hold `markup`")));
        }
    }
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_payload_projects_as_its_value() {
        let payload = DataPayload(serde_json::json!({ "kind": "note", "level": 2, "tags": ["a"] }));
        assert_eq!(
            flm_core::dump::json::to_json(&payload.project()).ok(),
            Some(r#"{"kind":"note","level":2,"tags":["a"]}"#.to_string())
        );
    }

    #[test]
    fn the_table_holds_the_vocabulary_the_tag_and_the_latex_constructs() {
        let mut expected: Vec<&str> = names::ALL.to_vec();
        expected.push(TAG);
        expected.extend(flm_core::stdcore::backends::latex::construct::ALL);
        assert_eq!(elements().collect::<Vec<_>>(), expected);
    }
}
