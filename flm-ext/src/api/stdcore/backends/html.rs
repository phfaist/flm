//! Mirrors [`flm_core::stdcore::backends::html::construct`]: the HTML
//! backend's construct, which a rule's body on the `html` backend stages as
//! any presentation element, and declares with
//! [`LoweringDecl::produces`](crate::api::core::lowering::LoweringDecl::produces).
//! The module holds names only. What differs: the construct's payload and
//! body types are not mirrored; the payload crosses as data.
//!
//! The construct's payload: `{ tag, attributes?: [[name, value], …], block? }`:
//! the tag name without its angle brackets (`div`, `mrow`, `tex-html`), the
//! attributes written in this order (each value escaped where it is written),
//! and whether the element is block-level (`false` when left out). Its
//! children are the node's content, written between the opening and the
//! closing tag; a construct's markup body (HTML already formed) cannot be given
//! as data. The emitter checks the tag and attribute names; a node whose name
//! fails is reported and written as the unsupported stand-in.
//!
//! ```text
//! lw.pres_tree(cx, { element: TAG, payload: { tag: "mrow", attributes: [["class", "x"]] },
//!                    children: [{ text: "x" }] })
//! ```
//!
//! writes `<mrow class="x">x</mrow>`.

/// Mirrors [`TAG`](flm_core::stdcore::backends::html::construct::TAG):
/// `flm.pres.html.tag`, the HTML construct, an element with a tag name,
/// attributes and children.
pub const TAG: &str = flm_core::stdcore::backends::html::construct::TAG;
