//! Content pointers: synthesized nodes that show content written elsewhere in
//! the tree, without copying it.
//!
//! A **content pointer** ([`ContentPointer`]) is the payload of a synthesized
//! node that shows, where the node is placed, content written elsewhere in
//! the tree (an xnote's content in the list of xnotes, a term in its
//! definition's lead-in, a theorem's title) without copying it: the address of
//! the node the content belongs to, and the node region of that node that
//! holds it (one of its arguments or slots; [`NodeRegion`]). See [content
//! pointers](crate::guide::concepts_overview#content-pointers).
//!
//! A feature that needs one builds a node of an element of its own carrying a
//! `ContentPointer`, and that element's lowering rule calls
//! [`lower_content_pointer`]. The function lowers the pointed region where the
//! pointer is placed, through [`Lowerer::lower_region`], as content of the
//! tree that holds it: a numbered list inside it is numbered as the document
//! numbered it, a labeled equation is an ordinary labeled reference point.
//! The content is lowered once, where the pointer is placed, provided the
//! feature lowers that region nowhere else.
//!
//! The feature that owns the pointer's element supplies a check
//! ([`PointerCheck`]) that the node a pointer refers to is one whose region
//! that element may show, because content shown both through a pointer and in
//! its own place would appear twice. Where the check fails, it reports a
//! condition of the feature's own.

use techy::core::node::{BuildId, NodeRef};
use techy::serialize::SerialValue;

use crate::core::lowering::{LowerCx, LowerError, Lowerer, NodeRegion, RegionShape};
use crate::lang::{Flm, PresPayload};
use crate::model::addr::NodeAddr;

use super::error::RegionNotFound;

/// The payload of a content pointer: the node some content belongs to, and
/// the node region of that node that holds the content, which the pointer
/// shows where it is placed. A feature's own synthesized element carries it
/// (see the [module documentation](self)).
///
/// The node is identified by its address, which is valid only within the
/// run that built the pointer: the address of a node of a document tree, or
/// of a node of a synthesized value the lowering holds (see
/// [`Lowerer::lower_region`]). A pointer stored in a table must refer to a
/// node of a document tree: a synthesized value's identifier is valid only
/// inside the stage call that brought the value up to date, and the runner
/// adds a table to the run's store after that call has ended.
///
/// Its projection is `{target: "<tree>/<node index>", region: "argument
/// <i>"}` (or `"slot <i>"`). No payload reader is registered for it, so a
/// tree holding a pointer can be printed but not read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPointer {
    /// The address of the node the content belongs to.
    pub target: NodeAddr,
    /// The node region of that node that holds the content.
    pub region: NodeRegion,
}

impl PresPayload for ContentPointer {
    fn project(&self) -> SerialValue {
        SerialValue::Map(vec![
            (
                String::from("target"),
                SerialValue::Str(format!("{}/{}", self.target.tree, self.target.node.index())),
            ),
            (String::from("region"), SerialValue::Str(self.region.to_string())),
        ])
    }
}

impl ContentPointer {
    /// The payload of the content pointer `node`, or `None` for a node that
    /// carries none.
    pub fn of<'n>(node: &'n NodeRef<'_, Flm>) -> Option<&'n ContentPointer> {
        node.ext().payload.as_deref()?.downcast_ref::<ContentPointer>()
    }
}

/// The check a feature supplies to [`lower_content_pointer`]. It receives the
/// [`Lowerer`], the node a pointer refers to and the node region the pointer
/// gives, and returns whether the pointer's element may show that region of
/// that node. When it returns `false`, it must already have reported the
/// refusal (a condition of the feature's own, at the pointer); the pointer
/// then writes nothing.
///
/// # Errors
///
/// A [`LowerError`] the check returns ends the pointer's lowering with that
/// error, for example a table read the calling rule did not declare.
pub type PointerCheck<'f> =
    dyn FnMut(&mut dyn Lowerer, &NodeRef<'_, Flm>, NodeRegion) -> Result<bool, LowerError> + 'f;

/// Lowers the content that the node `pointer` refers to through its
/// [`ContentPointer`] payload, where `pointer` is placed. Every lowering rule
/// of an element whose nodes carry a content pointer calls this function.
///
/// The function looks up, through [`Lowerer::lower_region`], the node and
/// the region the pointer refers to, and calls `check` with them; if `check`
/// refuses them, the pointer writes nothing. Otherwise the region's content
/// is lowered and arranged by `shape` ([`RegionShape`]; the plain shape is
/// [`lower_region_content`](super::lower_region_content)), and the staged
/// nodes are returned.
///
/// A pointer to a node the lowering cannot reach, or to a region that node
/// does not have, is reported at the pointer as
/// `flm.lowering.region-not-found` ([`RegionNotFound`], with the pointer's
/// element) and writes nothing either; the content around the pointer is
/// still written.
///
/// # Errors
///
/// [`LowerError`] when `pointer` carries no [`ContentPointer`], and any error
/// `check` or `shape` returns.
pub fn lower_content_pointer(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    pointer: &NodeRef<'_, Flm>,
    check: &mut PointerCheck<'_>,
    shape: &mut RegionShape<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let named = ContentPointer::of(pointer)
        .cloned()
        .ok_or_else(|| {
            LowerError::structure(
                "a content pointer node must carry its payload, and this one carries none",
            )
        })?;
    let lowered = lw.lower_region(cx, named.target, named.region, &mut |cx, lw, node, content| {
        if !check(lw, node, named.region)? {
            return Ok(Vec::new());
        }
        shape(cx, lw, node, content)
    });
    match lowered {
        Err(LowerError::RegionNotFound { detail }) => {
            let condition = RegionNotFound {
                backend: lw.backend().to_string(),
                rule: lw.rule().to_string(),
                element: pointer.ext().element.name.to_string(),
                detail,
            };
            lw.error(condition, pointer.span().clone());
            Ok(Vec::new())
        }
        other => other,
    }
}
