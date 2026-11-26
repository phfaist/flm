//! Mirrors [`flm_core::core::run`]: the context a stage reads through.

use flm_core::model::addr::NodeAddr;
use serde_json::Value;

use crate::api::core::data::table::TableValue;
use crate::api::core::stage::Scope;
use crate::api::host::HostHandle;
use crate::api::progress::ProgressScope;
use crate::api::techy::node::{Node, NodeKindName, Tree};
use crate::runtime::ExtError;
use crate::sealed::{RewriteCxCore, StageCxCore};

/// Mirrors [`StageCx`](flm_core::core::run::StageCx): what a running stage
/// reads — its scope, the document's trees, the tables it declared it reads.
///
/// What differs: the methods below, and nothing else (the document, the
/// registry, the plan and positions have no façade yet); a service slot is
/// read by its name ([`service`](StageCx::service),
/// [`services`](StageCx::services)), and [`host`](StageCx::host), which has no
/// counterpart, lends a host service. A tree id
/// crosses as its display text. The nodes and tables it answers borrow the
/// context; the trees share what they hold.
///
/// The trait is sealed: the adapter alone implements it.
pub trait StageCx: StageCxCore {
    /// Mirrors [`StageCx::scope`](flm_core::core::run::StageCx::scope).
    fn scope(&self) -> Scope;

    /// Mirrors [`StageCx::tree_id`](flm_core::core::run::StageCx::tree_id): the
    /// tree a per-tree stage runs on. What differs: the id as text.
    fn tree_id(&self) -> Option<String>;

    /// Mirrors [`StageCx::tree`](flm_core::core::run::StageCx::tree): the
    /// current state of the tree of id `id`. What differs: the id as text.
    fn tree(&self, id: &str) -> Option<Box<dyn Tree>>;

    /// Mirrors [`StageCx::frozen_tree`](flm_core::core::run::StageCx::frozen_tree):
    /// the frozen tree of id `id`. What differs: the id as text.
    fn frozen_tree(&self, id: &str) -> Option<Box<dyn Tree>>;

    /// Mirrors [`StageCx::my_tree`](flm_core::core::run::StageCx::my_tree): the
    /// tree a per-tree stage runs on.
    fn my_tree(&self) -> Option<Box<dyn Tree>>;

    /// Mirrors [`StageCx::trees`](flm_core::core::run::StageCx::trees): every
    /// frozen tree of the document, with its id. What differs: the ids are
    /// text, and the iterator is a [`Trees`], a traversal across a binding.
    fn trees(&self) -> Trees;

    /// Mirrors [`StageCx::walk`](flm_core::core::run::StageCx::walk): every node
    /// of every tree of the run, in document order (for a per-tree stage too).
    /// What differs: the iterator is a [`Walk`], whose items are
    /// [`WalkEntry`]s. Across a binding, the iterator is a traversal, which
    /// the extension can restrict to given elements ([`Walk::elements`]) or
    /// node kinds ([`Walk::kinds`]).
    fn walk(&self) -> Walk<'_>;

    /// Mirrors [`StageCx::addr`](flm_core::core::run::StageCx::addr): the
    /// address of `node`, by which computed data about the node is keyed.
    ///
    /// The address is valid in a document's trees and in a value copy (a
    /// synthesized tree this stage call has brought up to date, as value
    /// catch-up runs a stage over a copy); `None` when the node belongs to
    /// neither. What differs: the name (`addr_of`, as
    /// [`Lowerer::addr_of`](crate::api::core::lowering::Lowerer::addr_of));
    /// across a binding the address is data (`{"tree", "node"}`), the form
    /// that [`ExtTableRead::items_at`](crate::api::core::data::table::ExtTableRead::items_at)
    /// compares, so a row keyed by it is found by a rule's `get_at`. A Rust
    /// stage that keys its rows for `items_at` writes the address in that form
    /// with [`node_addr_to_value`](crate::adapter::values::node_addr_to_value).
    fn addr_of(&self, node: &dyn Node<'_>) -> Option<NodeAddr>;

    /// Mirrors [`StageCx::get`](flm_core::core::run::StageCx::get): the table
    /// named `name`, which the stage declared it reads. What differs: the table
    /// by its name, answered as a [`TableValue`] by its type. A table of an
    /// output's files is answered as [`TableValue::Projection`] holding the
    /// files and the path of each (see the [module documentation of
    /// `api::core::artifact`](crate::api::core::artifact#reading-the-files-of-an-output)).
    ///
    /// # Errors
    ///
    /// The stage did not declare the read, or the store refuses it
    /// ([`ExtError::Core`]); a standard table's projection holds a value that
    /// cannot cross (a number that is not a 64-bit signed integer, bytes, or a
    /// reserved map key: [`ExtError::Value`]); a file of an output refers to a
    /// file holding its bytes instead of holding them ([`ExtError::Value`]).
    fn get(&self, name: &str) -> Result<Option<TableValue<'_>>, ExtError>;

    /// Mirrors [`StageCx::option`](flm_core::core::run::StageCx::option): the
    /// value of an option type the stage declared it reads
    /// ([`Needs::option`](crate::api::core::stage::Needs::option)). What
    /// differs: the option type is named by its name, as a feature's
    /// `settings.options` declares it (or a Rust feature's settings declared
    /// from data); the value comes as data. Implemented over
    /// [`StageCx::option_named`](flm_core::core::run::StageCx::option_named):
    /// the value set on the render request, else the one set on the document,
    /// else the empty map.
    ///
    /// # Errors
    ///
    /// The stage did not declare the read ([`ExtError::Core`] of kind
    /// `option`).
    fn option(&self, name: &str) -> Result<Value, ExtError>;

    /// Mirrors [`StageCx::service`](flm_core::core::run::StageCx::service): the
    /// value of the service slot `name` that takes precedence, as data: the
    /// first in the order of precedence
    /// ([`services_by_precedence`](StageCx::services_by_precedence)), so that
    /// between features the value of the feature placed last in the feature
    /// order wins. What differs: the slot by its name (as
    /// [`Services::service_named`](flm_core::core::services::Services::service_named));
    /// the value as data. A slot a script declared
    /// ([`FeatureBuilder::service_slot`](crate::api::core::registry::FeatureBuilder::service_slot))
    /// holds data; a slot no feature declares, or an empty one, answers
    /// `None`.
    ///
    /// # Errors
    ///
    /// The slot's values are not data: a slot of a Rust feature's own type
    /// ([`ExtError::Unsupported`]).
    fn service(&self, name: &str) -> Result<Option<Value>, ExtError>;

    /// Mirrors [`StageCx::services`](flm_core::core::run::StageCx::services):
    /// every value of the service slot `name`, as data, in their assembled
    /// order (the values contributed first, then the others, each group in
    /// the order contributed). What differs: the slot by its name (as
    /// [`Services::services_named`](flm_core::core::services::Services::services_named));
    /// a list of data. A slot no feature declares answers an empty list.
    ///
    /// # Errors
    ///
    /// As [`service`](StageCx::service).
    fn services(&self, name: &str) -> Result<Vec<Value>, ExtError>;

    /// Mirrors
    /// [`Services::services_by_precedence_named`](flm_core::core::services::Services::services_by_precedence_named),
    /// reached through
    /// [`StageCx::services_all`](flm_core::core::run::StageCx::services_all):
    /// every value of the service slot `name`, as data, in the order of
    /// precedence (the values contributed first, then the others; within each
    /// group, the values of the feature placed last in the feature order
    /// first, each feature's in the order it contributed them, and the host's
    /// last). A slot whose values are asked in turn until one answers is read
    /// in this order. What differs: the slot by its name; a list of data. A
    /// slot no feature declares answers an empty list.
    ///
    /// # Errors
    ///
    /// As [`service`](StageCx::service).
    fn services_by_precedence(&self, name: &str) -> Result<Vec<Value>, ExtError>;

    /// No `flm-core` counterpart: the host service `name`, which the package
    /// of the stage declared in its `host` list, lent for the call — the
    /// `files` service as [`Files`](crate::api::host::Files), any other as a
    /// [`HostService`](crate::api::host::HostService).
    ///
    /// # Errors
    ///
    /// The package did not declare the service ([`ExtError::Argument`]).
    fn host(&self, name: &str) -> Result<HostHandle, ExtError>;

    /// Mirrors [`StageCx::progress`](flm_core::core::run::StageCx::progress):
    /// the scope of the stage's row on the progress sheet of the build, under
    /// which the stage declares the rows of its own work and asks whether the
    /// build was asked to stop. The per-tree calls of one stage share it.
    /// What differs: the scope as the façade [`ProgressScope`].
    fn progress(&self) -> Box<dyn ProgressScope>;
}

/// Mirrors an item of [`FrozenWalk`](flm_core::core::run::FrozenWalk): a node
/// and its address. What differs: a struct rather than a pair; the address is
/// opaque data a handler may store and compare, and nothing takes it back yet.
/// `'t` is the lifetime of the node's tree.
pub struct WalkEntry<'t> {
    /// The node's address.
    pub addr: NodeAddr,
    /// The node.
    pub node: Box<dyn Node<'t> + 't>,
}

/// Mirrors [`FrozenWalk`](flm_core::core::run::FrozenWalk): an iterator over
/// every node of every tree of the run, in document order, with its address.
///
/// What differs: each item is a [`WalkEntry`]. Across a binding, the
/// iterator is a [traversal](crate::api#the-mapping-rules): one handle, whose
/// entries the binding pulls once, in order, a bounded group at a time. A
/// stage that looks for the nodes of a few elements restricts the traversal
/// with [`elements`](Walk::elements) or [`kinds`](Walk::kinds) before it
/// iterates: the other nodes then cost the extension no crossing and no
/// memory of its engine.
pub struct Walk<'a> {
    entries: Box<dyn Iterator<Item = WalkEntry<'a>> + 'a>,
}

impl<'a> Walk<'a> {
    /// The traversal of `entries`, in their order.
    pub(crate) fn new(entries: impl Iterator<Item = WalkEntry<'a>> + 'a) -> Walk<'a> {
        Walk { entries: Box::new(entries) }
    }

    /// Mirrors [`Iterator::filter`] on [`FrozenWalk`](flm_core::core::run::FrozenWalk):
    /// the entries not taken yet whose node's element has one of the names
    /// `names` (the name [`Node::element_name`] answers), in their order.
    /// What differs: the criterion is a list of element names; a name that
    /// no element has matches no node. Across a binding the comparison runs
    /// on the host, and a node that does not match is not lent.
    pub fn elements(self, names: impl IntoIterator<Item = impl Into<String>>) -> Walk<'a> {
        let names: Vec<String> = names.into_iter().map(Into::into).collect();
        Walk::new(self.entries.filter(move |entry| names.contains(&entry.node.element_name())))
    }

    /// Mirrors [`Iterator::filter`] on [`FrozenWalk`](flm_core::core::run::FrozenWalk):
    /// the entries not taken yet whose node's kind ([`Node::kind`]) is one of
    /// `kinds`, in their order. What differs: the criterion is a list of
    /// kinds. Across a binding the comparison runs on the host, and a node
    /// that does not match is not lent.
    pub fn kinds(self, kinds: &[NodeKindName]) -> Walk<'a> {
        let kinds = kinds.to_vec();
        Walk::new(self.entries.filter(move |entry| kinds.contains(&entry.node.kind())))
    }
}

impl<'a> Iterator for Walk<'a> {
    type Item = WalkEntry<'a>;

    fn next(&mut self) -> Option<WalkEntry<'a>> {
        self.entries.next()
    }
}

/// Mirrors the iterator that [`StageCx::trees`](flm_core::core::run::StageCx::trees)
/// answers: trees, each with its id.
///
/// What differs: the ids as text; the iterator owns the trees, each shared
/// with what it was read from, so that it borrows nothing. It is also what
/// [`TreesTable::trees`](crate::api::core::stage::TreesTable::trees) and
/// [`LoweredTrees::trees`](crate::api::stdlib::backends::lowering::LoweredTrees::trees)
/// answer. Across a binding, the iterator is a
/// [traversal](crate::api#the-mapping-rules): one handle, whose pairs the
/// binding pulls once, in order, a bounded group at a time.
pub struct Trees {
    trees: std::vec::IntoIter<(String, Box<dyn Tree>)>,
}

impl Trees {
    /// The traversal of `trees`, in their order.
    pub(crate) fn new(trees: Vec<(String, Box<dyn Tree>)>) -> Trees {
        Trees { trees: trees.into_iter() }
    }
}

impl Iterator for Trees {
    type Item = (String, Box<dyn Tree>);

    fn next(&mut self) -> Option<(String, Box<dyn Tree>)> {
        self.trees.next()
    }
}

/// Mirrors [`RewriteCx`](flm_core::core::run::RewriteCx): what a rewrite
/// stage's handler reads, lent for one call
/// ([`RewriteStage::restage`](crate::api::core::stage::RewriteStage::restage)).
///
/// What differs: the methods below, and nothing else. Left out:
/// `registry()`, since the registry has no façade yet; `structural()`, since
/// the structural elements' names are constants; the typed `option::<T>`,
/// whose by-name form [`option`](RewriteCx::option) replaces it, as for
/// [`StageCx::option`].
///
/// The trait is sealed: the adapter alone implements it.
pub trait RewriteCx: RewriteCxCore {
    /// Mirrors [`RewriteCx::tree_id`](flm_core::core::run::RewriteCx::tree_id):
    /// the tree being rewritten, a document tree or a synthesized value. What
    /// differs: the id's display text (`tree#0`, `value#1`), as
    /// [`StageCx::tree_id`] answers it.
    fn tree_id(&self) -> String;

    /// Mirrors [`RewriteCx::layout`](flm_core::core::run::RewriteCx::layout):
    /// how the tree's top level is laid out. What differs: the layout's name,
    /// `blocks`, `inline` or `inline-or-blocks`.
    fn layout(&self) -> String;

    /// Mirrors [`RewriteCx::option_named`](flm_core::core::run::RewriteCx::option_named):
    /// the value of the option type `name` the stage declared it reads
    /// ([`Needs::option`](crate::api::core::stage::Needs::option)): the value
    /// set on the render request, else the one set on the document, else the
    /// empty map. What differs: the name `option`, as
    /// [`StageCx::option`]; the value comes as data.
    ///
    /// # Errors
    ///
    /// The stage did not declare the read ([`ExtError::Core`] of kind
    /// `option`).
    fn option(&self, name: &str) -> Result<Value, ExtError>;
}

fn _object_safe(_: &dyn StageCx, _: &dyn RewriteCx) {}
