//! The content pointer of a list of xnotes: the element
//! [`CONTENT_POINTER`](super::CONTENT_POINTER), whose payload is the lowering
//! feature's [`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer),
//! and its general lowering rule [`PointerRule`].
//!
//! A content pointer is the payload of a synthesized node that shows, where
//! the node is placed, content written elsewhere in the tree without copying
//! it (see [content
//! pointers](crate::guide::concepts_overview#content-pointers)). A list of
//! xnotes shows each xnote's content in this way. Each entry of the list holds
//! one node of the pointer element, whose payload refers to the node carrying
//! the xnote and to the region of that node holding the content. Lowering the
//! pointer lowers that region where the pointer is placed, through
//! [`lower_content_pointer`], as content of the tree that holds the xnote.
//! Everything inside the content is therefore lowered as the document's own
//! content (a numbered list inside a footnote is numbered as the document
//! numbered it, a labeled equation is an ordinary labeled reference point),
//! and lowered once, in the list: the xnote's own rule writes its mark where
//! the xnote is written, and never its content.
//!
//! The pointer is checked where it is lowered: a pointer that refers to a
//! region that is not the content of an xnote of a kind the table of xnotes
//! records (where the table is absent, an xnote of any kind is accepted) is
//! refused ([`PointerRefused`]), since what it would show is not
//! an xnote's content, and the reader would see that content twice.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    NodeRegion, RegionShape,
};
use crate::lang::Flm;
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::stdcore::backends::lowering::{lower_content_pointer, lower_region_content};

use super::diagnostics::PointerRefused;
use super::stages::XNOTES;
use super::{XnoteSource, CONTENT_POINTER, POINTER_RULE};

/// The element value of a content pointer: a block of its own in its entry,
/// so that the block-structure pass places no paragraph around it.
pub(super) fn pointer_element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(CONTENT_POINTER)
            .docs("The content of an xnote, shown where this node is placed.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
    )
}

/// The general rule of the element [`CONTENT_POINTER`], [`POINTER_RULE`],
/// marked default on every backend; the recomposed LaTeX, which places no
/// list, uses a rule of its own that writes nothing
/// ([`PointerLatexRule`](super::latex::PointerLatexRule)).
pub(super) fn pointer_rule() -> LoweringRule {
    LoweringRule::new(
        POINTER_RULE,
        CONTENT_POINTER,
        LoweringDecl::new().reads_opt(XNOTES),
        Arc::new(PointerRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "a content pointer lowers to the content of the xnote it refers to, lowered where the \
         pointer is placed; a pointer that refers to no xnote's content is refused and writes \
         nothing",
    )
}

/// The general lowering rule of the element
/// [`CONTENT_POINTER`](super::CONTENT_POINTER),
/// [`POINTER_RULE`](super::POINTER_RULE): the content that the pointer refers
/// to ([`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer)),
/// lowered where the pointer is placed, as it is lowered inside the xnote,
/// through [`lower_content_pointer`].
///
/// A pointer that refers to a region that is not the content of an xnote of a
/// kind the table of xnotes records (where the table is absent, an xnote of
/// any kind is accepted) is refused ([`PointerRefused`]) and writes nothing. A pointer that refers to a node the lowering cannot reach, or to
/// a region the node does not have, is reported at the pointer as the
/// lowering stage's `flm.lowering.region-not-found` and writes nothing.
///
/// The list of an xnote kind lowers the pointer of each entry itself, through
/// the same check, to place the link back to the mark at the end of the
/// content's last paragraph ([`ListRule`](super::ListRule)); this rule lowers
/// a pointer placed anywhere else.
#[derive(Debug, Default, Clone, Copy)]
pub struct PointerRule;

impl Lower for PointerRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let staged = lower_pointed(cx, lw, &node, &mut lower_region_content)?;
        Ok(Lowering::Emit(staged))
    }
}

/// Lowers the content that the content pointer `pointer` refers to, shaped by
/// `shape` ([`RegionShape`]), through [`lower_content_pointer`], once the
/// check has found that the node it refers to carries an xnote whose content
/// is that region. A pointer that the check refuses is reported
/// ([`PointerRefused`]) and writes nothing. A pointer that refers to a region
/// that cannot be found is reported at the pointer as the lowering stage's
/// `flm.lowering.region-not-found`, and writes nothing either.
///
/// The calling rule must declare an optional read of [`XNOTES`], from which
/// the check takes the xnote kinds the table records.
///
/// # Errors
///
/// [`LowerError`] when `pointer` carries no
/// [`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer),
/// when the calling rule did not declare its read of [`XNOTES`], and any
/// error that `shape` returns.
pub(super) fn lower_pointed(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    pointer: &NodeRef<'_, Flm>,
    shape: &mut RegionShape<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let span = pointer.span().clone();
    let mut check = |lw: &mut dyn Lowerer, node: &NodeRef<'_, Flm>, region: NodeRegion| {
        if is_xnote_content(lw, node, region)? {
            return Ok(true);
        }
        let element = node.ext().element.name.to_string();
        lw.error(PointerRefused { element, region: region.to_string() }, span.clone());
        Ok(false)
    };
    lower_content_pointer(cx, lw, pointer, &mut check, shape)
}

/// Whether `node` carries an xnote whose content is `region`, of an xnote
/// kind for which the table of xnotes has an entry (any kind, where the table
/// is absent).
fn is_xnote_content(
    lw: &dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    region: NodeRegion,
) -> Result<bool, LowerError> {
    let Some(source) = node.ext().element.aspect::<dyn XnoteSource>() else { return Ok(false) };
    let xnotes = lw.get(XNOTES)?;
    Ok(source.xnotes(node).iter().any(|xnote| {
        xnote.content == region && xnotes.is_none_or(|table| table.by_kind.contains_key(&xnote.kind))
    }))
}
