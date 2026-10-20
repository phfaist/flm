//! The identifiers of a tree and of a node: [`TreeId`] and [`NodeAddr`], the
//! keys under which computed data is stored per node.
//!
//! A **tree** is one placement of a [fragment](crate::model::doc::Fragment)'s
//! node tree in a document. The document's tree arena numbers its trees with
//! [`TreeId`]s in the order they are added, and never renumbers or reuses
//! them. Adding the same `Arc<Fragment>` twice to a document makes two trees,
//! each with its own computed data.
//!
//! A [`TreeId`] can also identify a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values): a small tree
//! that a stage builds while a run is under way, such as the content a `\ref`
//! displays or a formatted citation entry. A synthesized value is held outside
//! the document's trees: a consumer lowers it and renders it like document
//! content, but no stage that scans the document reaches it. Synthesized values
//! are numbered from zero by the stage invocation that builds them, so two
//! invocations assign the same identifiers, and the identifier of a synthesized
//! value means nothing outside the invocation that built it.
//!
//! [`TreeId`] holds both kinds of identifier in 4 bytes, told apart by a tag
//! bit. Code that stores computed data must respect two rules that follow from
//! the numbering of synthesized values:
//!
//! - no table stored in a run's shared store may use as a key a [`NodeAddr`]
//!   inside a synthesized value;
//! - a table shown for inspection writes a stored synthesized value out in
//!   full, never as its identifier.
//!
//! A [`NodeAddr`] pairs a [`TreeId`] with techy's [`NodeId`], 12 bytes in all.
//! It is `Copy` and ordered, so it can key an ordered map. A [`NodeId`] carries
//! a tag of the tree layout that assigned it, and that tag takes part in
//! equality, ordering and hashing. Two addresses in different trees never
//! compare equal, because their [`TreeId`]s differ. An ordered map keyed by addresses visits them tree by tree,
//! and within a tree in the tree's storage order. That order is deterministic,
//! but it is not document order: document order is defined by the walk,
//! [`DocContent::walk`].
//!
//! [`DocContent::walk`]: crate::model::doc::DocContent::walk

use std::fmt;

pub use techy::core::node::NodeId;

/// The identifier of one tree of a document's tree arena, or of one
/// synthesized value built inside a run.
///
/// The two kinds of identifier are numbered separately: a document tree and a
/// synthesized value with the same number are different identifiers.
/// [`TreeId::tree`] and [`TreeId::value`] create an identifier of each kind;
/// [`TreeId::is_document_tree`] and [`TreeId::is_value`] tell which kind an
/// identifier is, and [`TreeId::index`] returns its number. See the [module
/// documentation](self) for what each kind identifies.
///
/// Every document tree orders before every synthesized value; identifiers of
/// one kind order by their number. The `Display` form is `tree#3` for a
/// document tree and `value#3` for a synthesized value.
///
/// # Examples
///
/// ```
/// use flm_core::model::addr::TreeId;
///
/// let tree = TreeId::tree(3);
/// let value = TreeId::value(3);
/// assert_ne!(tree, value);
/// assert_eq!(tree.index(), value.index());
/// assert!(tree.is_document_tree());
/// assert!(value.is_value());
/// assert!(tree < value);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TreeId(u32);

impl TreeId {
    const VALUE_TAG: u32 = 1 << 31;
    const INDEX_MASK: u32 = !Self::VALUE_TAG;

    /// The identifier of the document tree at index `index` of a document's
    /// tree arena.
    ///
    /// A document assigns these identifiers itself as trees are added
    /// ([`DocContent::place`](crate::model::doc::DocContent::place),
    /// [`DocContent::add_tree`](crate::model::doc::DocContent::add_tree)); a
    /// caller creates one to refer to a tree it knows the index of. See
    /// [`TreeId::value`] for the identifier of a synthesized value.
    ///
    /// # Panics
    ///
    /// Panics if `index` is 2³¹ or greater: the highest bit is the tag that
    /// tells the two kinds of identifier apart.
    /// [`DocContent::add_tree`](crate::model::doc::DocContent::add_tree)
    /// checks the same bound before it creates an identifier.
    pub const fn tree(index: u32) -> TreeId {
        assert!(index & Self::VALUE_TAG == 0, "tree index exceeds the TreeId range");
        TreeId(index)
    }

    /// The identifier of the synthesized value numbered `index` within one
    /// stage invocation.
    ///
    /// Each stage invocation numbers its synthesized values from zero, so an
    /// identifier created here is meaningful only inside the invocation that
    /// built the synthesized value.
    ///
    /// # Panics
    ///
    /// Panics if `index` is 2³¹ or greater: the highest bit is the tag that
    /// tells the two kinds of identifier apart.
    pub const fn value(index: u32) -> TreeId {
        assert!(index & Self::VALUE_TAG == 0, "value index exceeds the TreeId range");
        TreeId(index | Self::VALUE_TAG)
    }

    /// The identifier's number: the arena index of a document tree, or the
    /// number a stage invocation gave a synthesized value.
    ///
    /// A document tree and a synthesized value may have the same number;
    /// [`TreeId::is_value`] tells them apart.
    pub const fn index(self) -> u32 {
        self.0 & Self::INDEX_MASK
    }

    /// Returns `true` when this identifies a synthesized value, `false` when it
    /// identifies a document tree.
    pub const fn is_value(self) -> bool {
        self.0 & Self::VALUE_TAG != 0
    }

    /// Returns `true` when this identifies a document tree, `false` when it
    /// identifies a synthesized value.
    pub const fn is_document_tree(self) -> bool {
        !self.is_value()
    }
}

impl fmt::Debug for TreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_value() {
            write!(f, "TreeId::value({})", self.index())
        } else {
            write!(f, "TreeId::tree({})", self.index())
        }
    }
}

impl fmt::Display for TreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_value() {
            write!(f, "value#{}", self.index())
        } else {
            write!(f, "tree#{}", self.index())
        }
    }
}

/// The address of one node in one tree: the tree's [`TreeId`] and the node's
/// [`NodeId`] in that tree.
///
/// Every computed table that stores data per node uses this as its key, and a
/// table entry uses it to refer to a node of another tree. It is `Copy` and
/// ordered; the [module documentation](self) explains that this order is not
/// document order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeAddr {
    /// The tree that contains the node.
    pub tree: TreeId,
    /// The node within that tree.
    pub node: NodeId,
}

impl NodeAddr {
    /// The address of the node `node` in the tree `tree`.
    ///
    /// No check is made here that the node belongs to that tree. A [`NodeId`]
    /// carries a tag of the tree layout that assigned it, so looking it up in
    /// any other tree (other than a copy with the same layout) with techy's
    /// `NodeTree::get` finds no node.
    pub const fn new(tree: TreeId, node: NodeId) -> NodeAddr {
        NodeAddr { tree, node }
    }
}

impl fmt::Debug for NodeAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeAddr({}, {:?})", self.tree, self.node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_and_value_ids_are_distinct_spaces() {
        let t = TreeId::tree(3);
        let v = TreeId::value(3);
        assert_ne!(t, v);
        assert_eq!(t.index(), 3);
        assert_eq!(v.index(), 3);
        assert!(!t.is_value());
        assert!(v.is_value());
        assert!(t.is_document_tree());
        assert_eq!(format!("{t}"), "tree#3");
        assert_eq!(format!("{v}"), "value#3");
        assert_eq!(format!("{t:?}"), "TreeId::tree(3)");
    }

    #[test]
    fn document_trees_order_before_values_and_by_index() {
        let mut ids = vec![TreeId::value(0), TreeId::tree(2), TreeId::tree(0), TreeId::value(1)];
        ids.sort();
        assert_eq!(ids, [TreeId::tree(0), TreeId::tree(2), TreeId::value(0), TreeId::value(1)]);
        assert_eq!(std::mem::size_of::<TreeId>(), 4);
    }
}
