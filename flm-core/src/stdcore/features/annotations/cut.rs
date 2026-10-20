//! How an annotation is cut into cut parts at paragraph breaks and blocks:
//! the element's implementation of the block-transparent aspect, and the
//! continuation record every cut part carries.
//!
//! An annotation is *block-transparent*: its passage or remark is laid out as
//! if it were written directly in the text around it
//! ([block-transparent elements](crate::guide::concepts_overview#block-transparent-elements)).
//! A blank line inside it is a paragraph break of the surrounding text, and
//! the annotation continues into the next paragraph; a list or a displayed
//! formula inside it is a block of the surrounding text, still part of the
//! annotation. Before any
//! paragraph is built, the block-structure pass calls the element's
//! [`BlockTransparent`] aspect, and the element replaces itself by **cut
//! parts**: one node of its own element around each stretch of inline
//! content between two paragraph breaks or blocks, and one around each
//! block. The paragraph breaks are placed between the cut parts, so that the
//! layout step that follows puts each stretch in its own paragraph. An
//! annotation is never a block holding paragraphs.
//!
//! Every cut part carries an [`AnnotationPiece`] record (which cut it came
//! from, its rank, how many cut parts the cut made). The lowering rules read
//! it to put the initials on the first cut part only, and, on the text
//! output, a comment's opening bracket on the first cut part and its closing
//! bracket on the last.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::serialize::{DeserializeContext, DeserializeError, SerialValue};

use crate::dump::session::PayloadReaders;
use crate::lang::rewrites::blocks::{
    argument_holds_blocks, run_holds_blocks, BlockCutter, BlockTransparent, CutItem, CutPart,
    Cutting, StructureCx, StructureError,
};
use crate::lang::{Flm, PresPayload};
use crate::model::aspects::{BlockRole, BlockRoleAspect, InlineSpacing};
use crate::stdcore::backends::pres::payload::{bad, entries, field, map};
use crate::stdcore::backends::pres::read::payload_of;

use super::{AnnotationElement, ANNOTATION, TEXT_ARG};

/// The continuation record a cut part of an annotation carries: which cut
/// made it, its rank among the cut parts of that cut, and how many cut parts
/// the cut made.
///
/// The block-structure pass cuts an annotation whose passage or remark
/// crosses a paragraph break or holds a block into cut parts: one node of the
/// annotation's element around each stretch of inline content, and one
/// around each block ([the feature's documentation](super)). Each cut part
/// carries this record as its payload
/// ([`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload)); read it with
/// [`payload_of`]`::<AnnotationPiece>(&node)`. An annotation that was not cut
/// carries no record, and the lowering rules treat it as both the first and
/// the last cut part.
///
/// The cut part with `index == 0` is the first: only it shows the initials.
/// The cut part with `index + 1 == count` is the last. The record is plain
/// data, serialized as `{cut, index, count}` and read back from that form by
/// the reader that [`payload_readers`] registers under the element's name,
/// so that a frozen tree holding cut parts can be read back from its
/// serialized form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnnotationPiece {
    /// The cut's number, unique within the tree, in the order in which the
    /// block-structure pass reaches the annotations: an annotation before
    /// those inside it.
    pub cut: usize,
    /// The cut part's rank among the cut parts of its cut, from zero.
    pub index: usize,
    /// How many cut parts the cut made.
    pub count: usize,
}

impl AnnotationPiece {
    /// Returns whether this is the first cut part of its cut.
    pub fn is_first(&self) -> bool {
        self.index == 0
    }

    /// Returns whether this is the last cut part of its cut.
    pub fn is_last(&self) -> bool {
        self.index + 1 == self.count
    }

    /// Rebuilds the record from its serialized form, `{cut, index, count}`.
    ///
    /// # Errors
    ///
    /// A [`DeserializeError`] that identifies the element when the value is
    /// not a map holding the three fields as non-negative integers.
    pub fn from_projection(value: &SerialValue) -> Result<AnnotationPiece, DeserializeError> {
        let e = entries(ANNOTATION, value)?;
        let number = |key: &str| match field(e, key) {
            Some(SerialValue::Int(n)) if *n >= 0 => Ok(*n as usize),
            Some(other) => Err(bad(ANNOTATION, format!("`{key}` must be a count, found {other:?}"))),
            None => Err(bad(ANNOTATION, format!("`{key}` is missing"))),
        };
        Ok(AnnotationPiece { cut: number("cut")?, index: number("index")?, count: number("count")? })
    }
}

impl PresPayload for AnnotationPiece {
    fn project(&self) -> SerialValue {
        let count = |n: usize| SerialValue::Int(n as i64);
        map(vec![("cut", count(self.cut)), ("index", count(self.index)), ("count", count(self.count))])
    }
}

/// Returns the payload readers of the feature: one, which reads back the
/// continuation records of cut parts ([`AnnotationPiece`]), registered under
/// the element's name [`ANNOTATION`].
///
/// A host that reads back a serialized frozen tree that may hold annotations
/// registers these readers in the reading session
/// ([`reader_with`](crate::dump::session::reader_with)), together with the
/// readers of the presentation vocabulary when the tree is a lowered one.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::annotations::{payload_readers, ANNOTATION};
///
/// let readers = payload_readers();
/// assert!(readers.get(ANNOTATION).is_some());
/// ```
pub fn payload_readers() -> PayloadReaders {
    let mut readers = PayloadReaders::new();
    readers.register(ANNOTATION, |value: &SerialValue, _cx: &mut DeserializeContext<'_, Flm>| {
        AnnotationPiece::from_projection(value).map(|record| Arc::new(record) as Arc<dyn PresPayload>)
    });
    readers
}

/// The position of the argument [`TEXT_ARG`] among an annotation's
/// arguments: the first and only one.
const TEXT_INDEX: usize = 0;

/// An annotation is inline content where its argument is laid out as inline
/// content, and a block where its argument is laid out as blocks: after the
/// cut, that is a cut part around one block. Before the cut, an annotation
/// holding a paragraph break is also a block, so that an annotation around
/// it treats it as a place to cut.
impl BlockRoleAspect for AnnotationElement {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        if argument_holds_blocks(node, TEXT_INDEX) {
            BlockRole::Block
        } else {
            BlockRole::Inline(InlineSpacing::CONTENT)
        }
    }
}

/// One cut part to build, or an item passed through unchanged.
enum Plan {
    /// A cut part around these staged nodes: a cut part around a block when
    /// the flag is set.
    Piece(Vec<BuildId>, bool),
    /// A paragraph-break marker, or a stretch of comments and whitespace
    /// alone, passed through.
    Through(CutItem),
}

/// The cut of an annotation. An annotation is left unchanged when its
/// argument holds no paragraph break and no block, or when it is already a
/// cut part. Otherwise it is cut into one cut part around each stretch of
/// inline content and one around each block, with the paragraph-break markers
/// passed through between them. A stretch of whitespace and source comments
/// alone makes no cut part, so an annotation holding nothing but a paragraph
/// break (`\phf{`, a blank line, `}`) is cut into no cut part and leaves
/// nothing of itself, while `\phf{}`, which is not cut, is an annotation of
/// empty content.
impl BlockTransparent for AnnotationElement {
    fn cut(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        cutter: &mut dyn BlockCutter,
    ) -> Result<Cutting, StructureError> {
        // A piece is asked again — when a structured tree is structured once
        // more, and a block piece's argument holds its block — and is left
        // as it is, its record with it.
        if payload_of::<AnnotationPiece>(&node).is_some() {
            return Ok(Cutting::Default);
        }
        let run: Vec<NodeRef<'_, Flm>> =
            node.argument_content_nodes(TEXT_INDEX).map(|content| content.iter().collect()).unwrap_or_default();
        if !run_holds_blocks(&run) {
            return Ok(Cutting::Default);
        }
        let cut = cutter.new_cut_number();
        let mut plans = Vec::new();
        let mut stretch = Vec::new();
        for item in cutter.restage_and_classify(cx, &run)? {
            match item {
                CutItem::Inline(_) | CutItem::Blank(_) => stretch.push(item),
                CutItem::Marker(_) => {
                    flush(&mut stretch, &mut plans);
                    plans.push(Plan::Through(item));
                }
                CutItem::Block(id) => {
                    flush(&mut stretch, &mut plans);
                    plans.push(Plan::Piece(vec![id], true));
                }
            }
        }
        flush(&mut stretch, &mut plans);
        let count = plans.iter().filter(|plan| matches!(plan, Plan::Piece(..))).count();
        let mut items = Vec::with_capacity(plans.len());
        let mut index = 0;
        for plan in plans {
            match plan {
                Plan::Through(item) => items.push(item),
                Plan::Piece(content, block) => {
                    let record = Arc::new(AnnotationPiece { cut, index, count });
                    let part = vec![(CutPart::ArgumentNamed(TEXT_ARG.to_string()), content)];
                    let id = cutter.rebuild(cx, &node, part, Some(record))?;
                    items.push(if block { CutItem::Block(id) } else { CutItem::Inline(id) });
                    index += 1;
                }
            }
        }
        Ok(Cutting::Emit(items))
    }
}

/// Ends the stretch of inline and blank items gathered so far: one cut part
/// around it when it holds some inline content, otherwise its items passed
/// through unchanged.
fn flush(stretch: &mut Vec<CutItem>, plans: &mut Vec<Plan>) {
    if stretch.iter().any(|item| matches!(item, CutItem::Inline(_))) {
        plans.push(Plan::Piece(stretch.drain(..).map(CutItem::id).collect(), false));
    } else {
        plans.extend(stretch.drain(..).map(Plan::Through));
    }
}
