//! The registration of the presentation vocabulary into a registry.
//!
//! One call, [`register`], declares every element of [the
//! vocabulary](super) with its payload type and attaches the three aspects the
//! lowering driver reads ([`super::aspects`]). The `lowering` feature
//! ([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature))
//! makes that call, so a registry holds the vocabulary once; a backend's own
//! feature is registered beside the `lowering` feature and does not declare
//! these elements again.
//!
//! A presentation element is declared through
//! [`FeatureBuilder::presentation_element`](crate::core::registry::FeatureBuilder::presentation_element),
//! which takes the element's name, its payload type and one line of
//! documentation. It has no callable name and no syntax definition: no source
//! document can invoke it, so there is no syntax to parse.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};

use super::aspects::{
    ContentInOpaque, Decoration, IsContentInOpaque, IsDecoration, IsOpaque, OpaqueContent,
};
use super::names;
use super::payload::{
    BlockPayload, CellsPayload, DecoMarkPayload, DecorationPayload, DroppedPayload,
    HeadingPayload, ImagePayload,
    LinePayload, LinesPayload, LinkPayload, ListPayload, MathPassthroughPayload, MathTextPayload,
    ParPayload,
    SpanPayload, SubtreePayload, UnknownPayload, UnsupportedPayload, VerbatimPayload,
};

/// Registers every presentation element of the vocabulary with its payload
/// type, and attaches the [`Decoration`] aspect to the three decoration
/// elements ([`DECORATIONS`](super::DECORATIONS)), the [`OpaqueContent`]
/// aspect to `flm.pres.verbatim` and `flm.pres.math-passthrough`, and the
/// [`ContentInOpaque`] aspect to `flm.pres.math-text`.
///
/// The `lowering` feature calls this from its own registration, so a package
/// that lists that feature provides the vocabulary. Calling it from a second
/// feature of the same registry declares the same elements twice and fails the
/// registry build.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises: an element of the
/// vocabulary already declared in this registry, or an aspect already attached
/// to one of them.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.presentation_element::<ParPayload>(names::PAR, "a laid-out paragraph (an inline run)")?;
    b.presentation_element::<BlockPayload>(
        names::BLOCK,
        "a block container with a role and kinds (a run of blocks)",
    )?;
    b.presentation_element::<HeadingPayload>(
        names::BLOCK_HEADING,
        "a heading placed on its own between blocks, with its heading kind's name, a level and \
         whether it is listed in a table of contents: a `body` content slot holding the title \
         (inline content) and an optional `alt` content slot holding the alternative title",
    )?;
    b.presentation_element::<HeadingPayload>(
        names::RUNIN_HEADING,
        "the title of a run-in heading, inside its run-in decoration, with the same payload and \
         content slots as a block heading: a `body` slot holding the title (inline content) and \
         an optional `alt` slot holding the alternative title",
    )?;
    b.presentation_element::<SpanPayload>(
        names::SPAN,
        "inline content with a display style, a role and kinds",
    )?;
    b.presentation_element::<ListPayload>(
        names::LIST,
        "a list: a `label` and an `item` content slot per entry",
    )?;
    b.presentation_element::<CellsPayload>(
        names::CELLS,
        "a table of cells: its grid in the payload, a `cell` content slot per entry",
    )?;
    b.presentation_element::<LinesPayload>(names::LINES, "a sequence of lines")?;
    b.presentation_element::<LinePayload>(
        names::LINE,
        "one line of a `lines` node, with its indentation and alignment",
    )?;
    b.presentation_element::<VerbatimPayload>(
        names::VERBATIM,
        "byte-exact content of a verbatim kind (text, code or identifier), inline or as a \
         block, with an optional language",
    )?;
    b.presentation_element::<MathPassthroughPayload>(
        names::MATH_PASSTHROUGH,
        "mathematics passed through as source text: the formula's content as written, which \
         the `math-passthrough` feature stages and nothing in FLM has read",
    )?;
    b.presentation_element::<MathTextPayload>(
        names::MATH_TEXT,
        "document content inside a formula: the argument of a text command",
    )?;
    b.presentation_element::<LinkPayload>(names::LINK, "a link around inline display content")?;
    b.presentation_element::<ImagePayload>(names::IMAGE, "an image")?;
    b.presentation_element::<UnsupportedPayload>(
        names::UNSUPPORTED,
        "the childless stand-in for a node the backend cannot render",
    )?;
    b.presentation_element::<DroppedPayload>(
        names::DROPPED,
        "the childless mark where a rule left out something it could not place",
    )?;
    b.presentation_element::<UnknownPayload>(
        names::UNKNOWN,
        "a macro or environment the parse could not attribute to any feature",
    )?;
    b.presentation_element::<DecorationPayload>(
        names::DECO_RUNIN,
        "a decoration at the start of its block (a theorem's heading)",
    )?;
    b.presentation_element::<DecorationPayload>(
        names::DECO_FINAL,
        "a decoration closing its block (a proof's closing mark)",
    )?;
    b.presentation_element::<DecorationPayload>(
        names::DECO_MARGIN,
        "a decoration set beside the text, level with its decoration mark",
    )?;
    b.presentation_element::<DecoMarkPayload>(
        names::DECO_MARK,
        "the childless mark left where the lowering driver moved a decoration out of a \
         content slot",
    )?;
    b.presentation_element::<SubtreePayload>(
        names::SUBTREE,
        "the position at which another document tree is placed",
    )?;
    for element in names::DECORATIONS {
        b.attach::<dyn Decoration>(element, Arc::new(IsDecoration))?;
    }
    for element in names::OPAQUE {
        b.attach::<dyn OpaqueContent>(element, Arc::new(IsOpaque))?;
    }
    for element in names::CONTENT_IN_OPAQUE {
        b.attach::<dyn ContentInOpaque>(element, Arc::new(IsContentInOpaque))?;
    }
    Ok(())
}
