//! Mirrors [`flm_core::model::aspects`]: the block role an element plays, and
//! the aspects an extension's handler answers ([`Referenceable`],
//! [`Sectioning`]) with the data they answer.

pub use flm_core::model::aspects::{AcceptedRefTypes, BlockRole, FixedBlockRole, InlineSpacing, Label, Placement, PointId};

use flm_core::model::names::{Name, RefKind};

use crate::api::techy::node::{Node, Span};
use crate::runtime::ExtError;

/// Mirrors [`Referenceable`](flm_core::model::aspects::Referenceable): the
/// aspect of an element whose occurrences are reference points, answered by a
/// handler of the extension.
///
/// What differs: the node is the extension API's façade, the points are the
/// mirror's [`RefPoint`], which may name nodes of the node's tree for the
/// call's lifetime, and the answer is a `Result`. A handler that fails has no
/// diagnostics channel (the stage that asks for the points passes none to the
/// aspect): the aspect then answers as if the element carried none, so the
/// occurrence has no reference point (a `\ref` to it is unresolved, it gets
/// no number and no anchor), and the failure is reported nowhere.
pub trait Referenceable: Send + Sync {
    /// Mirrors [`Referenceable::points`](flm_core::model::aspects::Referenceable::points):
    /// the reference points of the occurrence `node`. What differs: the
    /// node's tree and the borrow share the call's lifetime `'c`, which the
    /// points' nodes carry.
    ///
    /// # Errors
    ///
    /// The handler's failure: the occurrence then has no reference point.
    fn points<'c>(&self, node: &'c dyn Node<'c>) -> Result<Vec<RefPoint<'c>>, ExtError>;
}

/// Mirrors [`Sectioning`](flm_core::model::aspects::Sectioning): the aspect of
/// an element that opens a section, answered by a handler of the extension.
///
/// What differs: the node is the extension API's façade, the title is the
/// mirror's [`ContentRef`], and every answer is a `Result`. A handler that
/// fails has no diagnostics channel (the stage that asks passes none to the
/// aspect): the method then answers as if the element carried no aspect,
/// `level` 0, `in_toc` `false`, `toc_title` `None`, and the failure is
/// reported nowhere.
pub trait Sectioning: Send + Sync {
    /// Mirrors [`Sectioning::level`](flm_core::model::aspects::Sectioning::level):
    /// the section's level. What differs: the answer is a `Result`.
    ///
    /// # Errors
    ///
    /// The handler's failure: the level is then 0.
    fn level<'c>(&self, node: &'c dyn Node<'c>) -> Result<u32, ExtError>;

    /// Mirrors [`Sectioning::in_toc`](flm_core::model::aspects::Sectioning::in_toc):
    /// whether the section is listed in the table of contents; `true` unless
    /// the handler says otherwise, as in `flm-core`. What differs: the answer
    /// is a `Result`.
    ///
    /// # Errors
    ///
    /// The handler's failure: the section is then not listed.
    fn in_toc<'c>(&self, node: &'c dyn Node<'c>) -> Result<bool, ExtError> {
        let _ = node;
        Ok(true)
    }

    /// Mirrors [`Sectioning::toc_title`](flm_core::model::aspects::Sectioning::toc_title):
    /// the title the table of contents shows; `None` unless the handler says
    /// otherwise, as in `flm-core`. What differs: the content is the mirror's
    /// [`ContentRef`], and the answer is a `Result`.
    ///
    /// # Errors
    ///
    /// The handler's failure: the title is then `None`.
    fn toc_title<'c>(&self, node: &'c dyn Node<'c>) -> Result<Option<ContentRef<'c>>, ExtError> {
        let _ = node;
        Ok(None)
    }
}

/// Mirrors [`RefPoint`](flm_core::model::aspects::RefPoint): one reference
/// point of an occurrence, with the fields of `flm-core` by their names.
/// What differs: the anchor, the number source and the content are the
/// mirror's, which name nodes as façades for the lifetime `'t` of the
/// occurrence's tree; a malformed label's span is the mirror's [`Span`].
pub struct RefPoint<'t> {
    /// The kind of reference point (`section`, `equation`, `theorem`).
    pub kind: RefKind,
    /// The labels of the point: first the computed ones, then those written
    /// for the point in the document, in the order they were written.
    pub labels: Vec<Label>,
    /// The number of the point's computed labels: the first `computed`
    /// labels were computed by the element, not written by the author.
    pub computed: usize,
    /// The texts written as labels for the point that are not of the form
    /// `type:label`, in the order they were written.
    pub malformed: Vec<MalformedLabel>,
    /// How the point's anchor is formed.
    pub anchor: AnchorPolicy<'t>,
    /// Where the point's number comes from, if it is numbered.
    pub number: Option<NumberSource<'t>>,
    /// The content a reference to the point shows when it has no number.
    pub content: Option<ContentRef<'t>>,
    /// The label types the point accepts.
    pub accepts: AcceptedRefTypes,
}

/// Mirrors [`MalformedLabel`](flm_core::model::aspects::MalformedLabel): a
/// label written on a point that does not parse. What differs: the span is
/// the mirror's [`Span`], on the source of the point's node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedLabel {
    /// The text as written, trimmed.
    pub text: String,
    /// The span of the field it was written in, on the source of the point's
    /// node.
    pub span: Span,
    /// The position of this text among the point's labels and these texts
    /// together, counted from zero.
    pub position: usize,
}

/// Mirrors [`NumberSource`](flm_core::model::aspects::NumberSource): where a
/// reference point's number comes from, with the fields of `flm-core` by
/// their names. What differs: `instances` are nodes of the point's tree, as
/// façades for the lifetime `'t` of that tree, in place of their identifiers.
pub struct NumberSource<'t> {
    /// The name of the counter the point counts in.
    pub counter: Name,
    /// The scoped instances of the counter that the point counts under,
    /// outermost first: nodes of the tree of the point's node that contain
    /// it. Empty for a point that counts in the counter's single sequence.
    pub instances: Vec<Box<dyn Node<'t> + 't>>,
    /// The name of the counter style the point's number is shown in.
    pub style: Name,
    /// Which levels of the counter the point advances, in the order it
    /// advances them.
    pub advance: Vec<u32>,
    /// A number template of the point's own, or `None` to use the number
    /// format of the counter style.
    pub number_format: Option<String>,
}

/// Mirrors [`AnchorPolicy`](flm_core::model::aspects::AnchorPolicy): how the
/// anchor of a reference point is formed, with the variants of `flm-core` by
/// their names. What differs: a slug's content is the mirror's
/// [`ContentRef`].
pub enum AnchorPolicy<'t> {
    /// This identifier, used as given.
    Explicit(String),
    /// Encoded from the point's first remaining label.
    FromFirstLabel,
    /// Encoded from the given content after a prefix and a double hyphen.
    Slug {
        /// The text written before the double hyphen.
        prefix: String,
        /// The content the identifier is encoded from.
        content: ContentRef<'t>,
    },
    /// The prefix, a hyphen, and the number the document gave the point.
    FromValue {
        /// The text written before the number.
        prefix: String,
    },
    /// The point's first remaining label, else the prefix and the point's
    /// position.
    Auto {
        /// The text written before the position.
        prefix: String,
    },
}

/// Mirrors [`ContentRef`](flm_core::model::aspects::ContentRef): content a
/// reference point names, with the variants of `flm-core` by their names.
/// What differs: a node of the point's tree is its façade, in place of its
/// identifier; a synthesized tree is its root node's façade (the root node of
/// a tree the handler holds), in place of the shared tree.
pub enum ContentRef<'t> {
    /// The content of the argument at this position, counted from zero.
    Argument(usize),
    /// The content of the argument with this name.
    ArgumentNamed(String),
    /// The node's body.
    Body,
    /// One node of the point's tree.
    Node(Box<dyn Node<'t> + 't>),
    /// The root node of a tree of its own, which the handler built.
    Synthesized(Box<dyn Node<'t> + 't>),
}

fn _object_safe(_: &dyn Referenceable, _: &dyn Sectioning) {}
