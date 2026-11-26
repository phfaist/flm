//! Mirrors techy's node module, reached as [`flm_core::techy::core::node`]: the
//! node reader, and the trees `flm-core` shares.

use flm_core::lang::Flm;

use flm_core::techy::core::node::{NodeId, NodeRef};
use serde_json::Value;

use crate::api::lang::mint::Mode;
use crate::runtime::ExtError;
use crate::sealed::{NodeCore, Token, TreeCore};

/// Mirrors [`NodeRef`] over the FLM
/// language: a node of a tree, and its relatives.
///
/// What differs: a façade over a node of any tree the adapter holds (lent for
/// one call, or shared through an `Arc`); a slice of nodes is a list, which
/// crosses a binding as a sequence of nodes (see the
/// [mapping rules](crate::api#the-mapping-rules)); the iterator of the
/// descendants is a [`Descendants`]; owned text where `NodeRef` lends it; the
/// node's kind as the closed enum [`NodeKindName`]; three readings beyond
/// `NodeRef`'s own —
/// [`element_name`](Node::element_name), [`element_value`](Node::element_value)
/// and [`mode`](Node::mode) — and one composite,
/// [`argument_is_provided`](Node::argument_is_provided).
///
/// `'t` is the lifetime of the tree the node belongs to, as in `NodeRef<'t, _>`:
/// a node's relatives borrow the same tree, so every node the trait answers is a
/// `Box<dyn Node<'t> + 't>`.
///
/// The trait is sealed: the adapter alone implements it, so every `dyn Node` is
/// one of the adapter's nodes. Its seal carries two accessors only the crate can
/// call: the `flm-core` node behind the façade, borrowing the façade, and an
/// owned copy of the façade, through which the adapter keeps a node beyond one
/// borrow. Since `dyn Node<'t>` is invariant in `'t`, an
/// implementation over a tree of lifetime `'t` implements the trait for every
/// shorter lifetime as well (`impl<'t: 's, 's> Node<'s> for NodeFacade<'t>`), so
/// that a node of a longer-lived tree can be lent where a shorter one is
/// expected; the forwarding implementation for `&N` below keeps the lifetime of
/// `N`.
pub trait Node<'t>: NodeCore<'t> {
    /// Mirrors [`NodeRef::kind`](flm_core::techy::core::node::NodeRef::kind).
    /// What differs: the kind's name, not its data.
    fn kind(&self) -> NodeKindName;

    /// Mirrors [`NodeRef::is_chars`](flm_core::techy::core::node::NodeRef::is_chars).
    fn is_chars(&self) -> bool;

    /// Mirrors [`NodeRef::is_group`](flm_core::techy::core::node::NodeRef::is_group).
    fn is_group(&self) -> bool;

    /// Mirrors [`NodeRef::is_callable`](flm_core::techy::core::node::NodeRef::is_callable).
    fn is_callable(&self) -> bool;

    /// Mirrors [`NodeRef::is_comment`](flm_core::techy::core::node::NodeRef::is_comment).
    fn is_comment(&self) -> bool;

    /// Mirrors [`NodeRef::is_list`](flm_core::techy::core::node::NodeRef::is_list).
    fn is_list(&self) -> bool;

    /// Mirrors [`NodeRef::span`](flm_core::techy::core::node::NodeRef::span).
    /// What differs: byte offsets into the node's own source, without the
    /// source.
    fn span(&self) -> Span;

    /// Mirrors [`NodeRef::span_content`](flm_core::techy::core::node::NodeRef::span_content).
    /// What differs: owned text.
    fn span_content(&self) -> String;

    /// Mirrors [`NodeRef::parent`](flm_core::techy::core::node::NodeRef::parent).
    fn parent(&self) -> Option<Box<dyn Node<'t> + 't>>;

    /// Mirrors [`NodeRef::child_count`](flm_core::techy::core::node::NodeRef::child_count).
    fn child_count(&self) -> usize;

    /// Mirrors [`NodeRef::child`](flm_core::techy::core::node::NodeRef::child).
    fn child(&self, i: usize) -> Option<Box<dyn Node<'t> + 't>>;

    /// Mirrors [`NodeRef::children`](flm_core::techy::core::node::NodeRef::children).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    fn children(&self) -> Vec<Box<dyn Node<'t> + 't>>;

    /// Mirrors [`NodeRef::descendants`](flm_core::techy::core::node::NodeRef::descendants).
    /// What differs: a [`Descendants`], which owns what it reads, so that it
    /// does not borrow the node it was called on. Across a binding, the
    /// iterator is a traversal, which the extension can restrict to given
    /// elements ([`Descendants::elements`]) or node kinds
    /// ([`Descendants::kinds`]).
    fn descendants(&self) -> Descendants<'t>;

    /// Mirrors [`NodeRef::chars`](flm_core::techy::core::node::NodeRef::chars).
    /// What differs: owned text.
    fn chars(&self) -> Option<String>;

    /// Mirrors [`NodeRef::name`](flm_core::techy::core::node::NodeRef::name).
    /// What differs: owned text.
    fn name(&self) -> Option<String>;

    /// Mirrors [`NodeRef::group_delimiters`](flm_core::techy::core::node::NodeRef::group_delimiters).
    /// What differs: owned text.
    fn group_delimiters(&self) -> Option<(String, String)>;

    /// Mirrors [`NodeRef::argument_nodes`](flm_core::techy::core::node::NodeRef::argument_nodes).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    fn argument_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'t> + 't>>>;

    /// Mirrors [`NodeRef::argument_content_nodes`](flm_core::techy::core::node::NodeRef::argument_content_nodes).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    fn argument_content_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'t> + 't>>>;

    /// Mirrors [`NodeRef::argument_nodes_named`](flm_core::techy::core::node::NodeRef::argument_nodes_named).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    ///
    /// # Errors
    ///
    /// The node's callable has no argument of that name ([`ExtError::Core`]).
    fn argument_nodes_named(&self, name: &str) -> Result<Option<Vec<Box<dyn Node<'t> + 't>>>, ExtError>;

    /// Mirrors [`NodeRef::argument_content_nodes_named`](flm_core::techy::core::node::NodeRef::argument_content_nodes_named).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    ///
    /// # Errors
    ///
    /// As [`argument_nodes_named`](Node::argument_nodes_named).
    fn argument_content_nodes_named(&self, name: &str) -> Result<Option<Vec<Box<dyn Node<'t> + 't>>>, ExtError>;

    /// Mirrors the reading `node.arguments().get_named(name).is_provided()` of
    /// [`NodeRef::arguments`](flm_core::techy::core::node::NodeRef::arguments):
    /// whether the argument named `name` was written. What differs: one
    /// method, since the parsed arguments have no façade.
    ///
    /// # Errors
    ///
    /// The node's callable has no argument of that name ([`ExtError::Core`]).
    fn argument_is_provided(&self, name: &str) -> Result<bool, ExtError>;

    /// Mirrors [`NodeRef::body`](flm_core::techy::core::node::NodeRef::body).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    fn body(&self) -> Option<Vec<Box<dyn Node<'t> + 't>>>;

    /// Mirrors [`NodeRef::slot_content_nodes_named`](flm_core::techy::core::node::NodeRef::slot_content_nodes_named).
    /// What differs: a list, which crosses a binding as a sequence of nodes.
    /// As in techy, a slot that exists always has content nodes, possibly
    /// none: there is no "absent slot" answer.
    ///
    /// # Errors
    ///
    /// The node's callable has no slot of that name ([`ExtError::Core`]).
    fn slot_content_nodes_named(&self, name: &str) -> Result<Vec<Box<dyn Node<'t> + 't>>, ExtError>;

    /// Mirrors [`element_of`](flm_core::model::element::element_of)`(node).name`:
    /// the name of the node's element.
    fn element_name(&self) -> String;

    /// Mirrors [`element_value`](flm_core::model::element::element_value): the
    /// data of the node's element. What differs: not generic; the JSON value an
    /// extension's [`ElementSpec::with_value`](crate::api::model::element::ElementSpec::with_value)
    /// set, and `None` on any other element.
    fn element_value(&self) -> Option<Value>;

    /// Mirrors `node.parsing_state().mode()`: the parsing mode the node was
    /// read in.
    fn mode(&self) -> Mode;

    /// Mirrors the field [`FlmNodeExt::payload`](flm_core::lang::FlmNodeExt::payload)
    /// through [`project_payload`](flm_core::dump::project::project_payload):
    /// the payload of a presentation node, as its projection in JSON form
    /// (the shapes the [presentation elements' module](crate::api::stdlib::backends::pres)
    /// lists, or the value a rule staged for an element declared with a data
    /// payload). `None` for a node without a payload. What differs: data, not
    /// the payload's type.
    ///
    /// # Errors
    ///
    /// The projection cannot cross as data (a floating-point number, bytes,
    /// a reserved key): [`ExtError::Value`].
    fn payload(&self) -> Result<Option<Value>, ExtError>;

    /// Mirrors `node.slots().map(ParsedSlots::len)` of
    /// [`NodeRef::slots`](flm_core::techy::core::node::NodeRef::slots): the
    /// number of the node's slots (a list's `label` and `item` per entry, a
    /// presentation node's named slots), 0 for a node without slots. What
    /// differs: one method, since the parsed slots have no façade.
    fn slot_count(&self) -> usize;

    /// Mirrors `node.slots()?.get(i)?.name()` of
    /// [`NodeRef::slots`](flm_core::techy::core::node::NodeRef::slots): the
    /// name of the slot at index `i`, or `None` for an unnamed slot or an
    /// index past the last. What differs: one method, owned text.
    fn slot_name(&self, i: usize) -> Option<String>;

    /// Mirrors [`NodeRef::slot_content_nodes`](flm_core::techy::core::node::NodeRef::slot_content_nodes):
    /// the content of the slot at index `i`, or `None` for an index past the
    /// last. What differs: a list, which crosses a binding as a sequence of
    /// nodes.
    fn slot_content_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'t> + 't>>>;
}

/// Lends a reference to a node as a node: every method is forwarded, so that a
/// `&'c dyn Node<'t>` boxes as a `Box<dyn Node<'t> + 'c>`. The owned copy is
/// the referenced façade's, not the reference's.
impl<'t, N: Node<'t> + ?Sized> NodeCore<'t> for &N {
    fn core_node_ref(&self, token: Token) -> NodeRef<'_, Flm> {
        (**self).core_node_ref(token)
    }
    fn clone_box(&self, token: Token) -> Box<dyn Node<'t> + 't> {
        (**self).clone_box(token)
    }
    fn relative(&self, token: Token, id: NodeId) -> Box<dyn Node<'t> + 't> {
        (**self).relative(token, id)
    }
}

/// Lends a reference to a node as a node: every method is forwarded, so that a
/// `&'c dyn Node<'t>` boxes as a `Box<dyn Node<'t> + 'c>`.
impl<'t, N: Node<'t> + ?Sized> Node<'t> for &N {
    fn kind(&self) -> NodeKindName {
        (**self).kind()
    }
    fn is_chars(&self) -> bool {
        (**self).is_chars()
    }
    fn is_group(&self) -> bool {
        (**self).is_group()
    }
    fn is_callable(&self) -> bool {
        (**self).is_callable()
    }
    fn is_comment(&self) -> bool {
        (**self).is_comment()
    }
    fn is_list(&self) -> bool {
        (**self).is_list()
    }
    fn span(&self) -> Span {
        (**self).span()
    }
    fn span_content(&self) -> String {
        (**self).span_content()
    }
    fn parent(&self) -> Option<Box<dyn Node<'t> + 't>> {
        (**self).parent()
    }
    fn child_count(&self) -> usize {
        (**self).child_count()
    }
    fn child(&self, i: usize) -> Option<Box<dyn Node<'t> + 't>> {
        (**self).child(i)
    }
    fn children(&self) -> Vec<Box<dyn Node<'t> + 't>> {
        (**self).children()
    }
    fn descendants(&self) -> Descendants<'t> {
        (**self).descendants()
    }
    fn chars(&self) -> Option<String> {
        (**self).chars()
    }
    fn name(&self) -> Option<String> {
        (**self).name()
    }
    fn group_delimiters(&self) -> Option<(String, String)> {
        (**self).group_delimiters()
    }
    fn argument_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'t> + 't>>> {
        (**self).argument_nodes(i)
    }
    fn argument_content_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'t> + 't>>> {
        (**self).argument_content_nodes(i)
    }
    fn argument_nodes_named(&self, name: &str) -> Result<Option<Vec<Box<dyn Node<'t> + 't>>>, ExtError> {
        (**self).argument_nodes_named(name)
    }
    fn argument_content_nodes_named(&self, name: &str) -> Result<Option<Vec<Box<dyn Node<'t> + 't>>>, ExtError> {
        (**self).argument_content_nodes_named(name)
    }
    fn argument_is_provided(&self, name: &str) -> Result<bool, ExtError> {
        (**self).argument_is_provided(name)
    }
    fn body(&self) -> Option<Vec<Box<dyn Node<'t> + 't>>> {
        (**self).body()
    }
    fn slot_content_nodes_named(&self, name: &str) -> Result<Vec<Box<dyn Node<'t> + 't>>, ExtError> {
        (**self).slot_content_nodes_named(name)
    }
    fn element_name(&self) -> String {
        (**self).element_name()
    }
    fn element_value(&self) -> Option<Value> {
        (**self).element_value()
    }
    fn mode(&self) -> Mode {
        (**self).mode()
    }
    fn payload(&self) -> Result<Option<Value>, ExtError> {
        (**self).payload()
    }
    fn slot_count(&self) -> usize {
        (**self).slot_count()
    }
    fn slot_name(&self, i: usize) -> Option<String> {
        (**self).slot_name(i)
    }
    fn slot_content_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'t> + 't>>> {
        (**self).slot_content_nodes(i)
    }
}

/// Mirrors techy's [`Descendants`](flm_core::techy::core::node::Descendants):
/// an iterator over the descendants of a node, in document order, the node
/// itself excluded.
///
/// What differs: each item is a node façade; the iterator owns the façade of
/// its root node and the ids of the descendants, which
/// [`Node::descendants`] collects once with techy's iterator, so it borrows
/// neither the node it was called on nor the tree beyond `'t`. Across a
/// binding, the iterator is a [traversal](crate::api#the-mapping-rules): one
/// handle, whose nodes the binding pulls once, in order, a bounded group at
/// a time. A stage that looks for the nodes of a few elements or kinds
/// restricts the traversal with [`elements`](Descendants::elements) or
/// [`kinds`](Descendants::kinds) before it iterates: the other nodes then
/// cost the extension no crossing and no memory of its engine.
pub struct Descendants<'t> {
    nodes: Box<dyn Iterator<Item = Box<dyn Node<'t> + 't>> + 't>,
}

impl<'t> Descendants<'t> {
    /// The traversal of `nodes`, in their order.
    pub(crate) fn new(nodes: impl Iterator<Item = Box<dyn Node<'t> + 't>> + 't) -> Descendants<'t> {
        Descendants { nodes: Box::new(nodes) }
    }

    /// Mirrors [`Iterator::filter`] on techy's
    /// [`Descendants`](flm_core::techy::core::node::Descendants): the nodes not
    /// taken yet whose element has one of the names `names` (the name
    /// [`Node::element_name`] answers), in their order; the descendants of a
    /// node that does not match are still visited. What differs: the
    /// criterion is a list of element names; a name that no element has
    /// matches no node. Across a binding the comparison runs on the host, and
    /// a node that does not match is not lent.
    pub fn elements(self, names: impl IntoIterator<Item = impl Into<String>>) -> Descendants<'t> {
        let names: Vec<String> = names.into_iter().map(Into::into).collect();
        Descendants::new(self.nodes.filter(move |node| names.contains(&node.element_name())))
    }

    /// Mirrors [`Iterator::filter`] on techy's
    /// [`Descendants`](flm_core::techy::core::node::Descendants): the nodes not
    /// taken yet whose kind ([`Node::kind`]) is one of `kinds`, in their
    /// order. What differs: the criterion is a list of kinds. Across a
    /// binding the comparison runs on the host, and a node that does not
    /// match is not lent.
    pub fn kinds(self, kinds: &[NodeKindName]) -> Descendants<'t> {
        let kinds = kinds.to_vec();
        Descendants::new(self.nodes.filter(move |node| kinds.contains(&node.kind())))
    }
}

impl<'t> Iterator for Descendants<'t> {
    type Item = Box<dyn Node<'t> + 't>;

    fn next(&mut self) -> Option<Box<dyn Node<'t> + 't>> {
        self.nodes.next()
    }
}

/// Mirrors `Arc<NodeTree<Flm>>`, techy's
/// [`NodeTree`](flm_core::techy::core::node::NodeTree) over the FLM language
/// shared through an `Arc`: a whole tree, as `flm-core` answers one (a
/// document's tree, a rewrite's output tree, a lowered tree, a synthesized
/// value, an instantiated template) and takes one (the value of an extra
/// placeholder, a value to splice).
///
/// What differs: a façade with one reading, the root node
/// ([`root`](Tree::root)), through which every node of the tree is reached.
/// The façade shares the tree: a tree given to a function that takes one is
/// the value `flm-core` answered, never a copy.
///
/// The trait is sealed: the adapter alone implements it.
pub trait Tree: TreeCore {
    /// Mirrors [`NodeTree::root`](flm_core::techy::core::node::NodeTree::root):
    /// the tree's root node. What differs: the node is a façade that shares
    /// the tree, as do its relatives, so each stays valid for as long as it is
    /// held.
    fn root<'s>(&self) -> Box<dyn Node<'s> + 's>;
}

/// Mirrors the variants of techy's
/// [`NodeKind`](flm_core::techy::core::node::NodeKind), by name: a closed enum,
/// since `NodeKind` is generic over the language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKindName {
    /// A run of characters.
    Chars,
    /// A group (braces, math delimiters, …).
    Group,
    /// A macro, an environment or a specials sequence.
    Callable,
    /// A comment.
    Comment,
    /// A list of nodes.
    List,
}

/// Mirrors techy's [`SourceSpan`](flm_core::techy::source::SourceSpan): where a
/// node stands. What differs: the byte offsets alone, into the node's own
/// source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Span {
    /// The offset of the node's first byte.
    pub start: usize,
    /// The offset just past the node's last byte.
    pub end: usize,
}

fn _object_safe(_: &dyn Node<'_>, _: &dyn Tree) {}
