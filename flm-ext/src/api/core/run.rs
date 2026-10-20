//! Mirrors [`flm_core::core::run`]: the context a stage reads through.

use flm_core::model::addr::NodeAddr;
use serde_json::Value;

use crate::api::core::data::table::TableValue;
use crate::api::core::stage::Scope;
use crate::api::host::HostHandle;
use crate::api::techy::node::Node;
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
/// crosses as its display text; a tree is answered as its root node. The
/// nodes and tables it answers borrow the context.
///
/// The trait is sealed: the adapter alone implements it.
pub trait StageCx: StageCxCore {
    /// Mirrors [`StageCx::scope`](flm_core::core::run::StageCx::scope).
    fn scope(&self) -> Scope;

    /// Mirrors [`StageCx::tree_id`](flm_core::core::run::StageCx::tree_id): the
    /// tree a per-tree stage runs on. What differs: the id as text.
    fn tree_id(&self) -> Option<String>;

    /// Mirrors [`StageCx::tree`](flm_core::core::run::StageCx::tree): the
    /// current state of the tree of id `id`. What differs: the id as text, the
    /// tree as its root node.
    fn tree(&self, id: &str) -> Option<Box<dyn Node<'_> + '_>>;

    /// Mirrors [`StageCx::frozen_tree`](flm_core::core::run::StageCx::frozen_tree):
    /// the frozen tree of id `id`. What differs: the id as text, the tree as its
    /// root node.
    fn frozen_tree(&self, id: &str) -> Option<Box<dyn Node<'_> + '_>>;

    /// Mirrors [`StageCx::my_tree`](flm_core::core::run::StageCx::my_tree): the
    /// tree a per-tree stage runs on. What differs: its root node.
    fn my_tree(&self) -> Option<Box<dyn Node<'_> + '_>>;

    /// Mirrors [`StageCx::trees`](flm_core::core::run::StageCx::trees): every
    /// frozen tree of the document. What differs: a list of ids as text and
    /// root nodes.
    fn trees(&self) -> Vec<(String, Box<dyn Node<'_> + '_>)>;

    /// Mirrors [`StageCx::walk`](flm_core::core::run::StageCx::walk): every node
    /// of every tree of the run, in document order (for a per-tree stage too).
    /// What differs: a list, built eagerly.
    fn walk(&self) -> Vec<WalkEntry<'_>>;

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
