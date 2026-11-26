//! The guest's own type of `flm:techy/node`: [`Nodes`], several nodes, as
//! the host answers them (a sequence) or as the extension joins them.

#[cfg(feature = "native")]
use crate::adapter::values::push_nodes;
#[cfg(feature = "native")]
use crate::api;
use crate::guest::hand::joined::sequence_type;
use crate::guest::techy::node::Node;

sequence_type! {
    /// Several nodes, in order: what a method answers in place of a list of
    /// nodes (a node's children, an argument's nodes, a body, …), and what a
    /// function that takes several nodes is given.
    ///
    /// A sequence that the host answered is one handle and its length, and
    /// costs nothing until it is read: [`get`](Nodes::get) and
    /// [`iter`](Nodes::iter) fetch its nodes from the host, a crossing for an
    /// index and one for each group of nodes iterated. An extension builds its
    /// own sequence empty ([`Default`]) or from a `Vec` of nodes, and joins
    /// nodes and other sequences to it with [`push`](Nodes::push) and
    /// [`push_all`](Nodes::push_all), with no crossing: a sequence the host
    /// answered is passed back as its handle, whatever its length.
    /// [`to_vec`](Nodes::to_vec) fetches every node.
    Nodes of Node, "node", api Box<dyn api::techy::node::Node<'c> + 'c>, entry Nodes, push push_nodes
}
