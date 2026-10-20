//! Mirrors [`flm_core::core::data::table`]: reading a table.

use flm_core::model::addr::NodeAddr;
use serde_json::Value;

use crate::api::core::stage::TreesTable;
use crate::api::stdcore::backends::lowering::LoweredTrees;
use crate::api::stdcore::features::numbering::Counters;
use crate::api::stdcore::features::refs::Anchors;
use crate::sealed::Sealed;

/// No flm-core counterpart: an extension's own table, read by a stage (the
/// method `items` of the adapter's `ExtTable`, a table type of this crate).
///
/// The trait is sealed: the adapter alone implements it.
pub trait ExtTableRead: Sealed {
    /// The items the table's writers pushed, in traversal order.
    fn items(&self) -> Vec<Value>;

    /// No flm-core counterpart: the items at the node of address `addr`, in
    /// traversal order: those that are maps whose field `addr` equals the
    /// address in its data form `{"tree": "<the tree id's display text>",
    /// "node": <the node's index>}`, as a stage's walk answers it and as a
    /// rule's lowerer answers `addr()` and `addr_of(node)`. Only those items
    /// are copied, so that a rule reading one node's rows of a table with a
    /// row per node does not copy the whole table at every node. A Rust stage
    /// writes the field `addr` of its rows in that form with
    /// [`node_addr_to_value`](crate::adapter::values::node_addr_to_value).
    fn items_at(&self, addr: &NodeAddr) -> Vec<Value>;
}

/// Mirrors the value [`StageCx::get`](flm_core::core::run::StageCx::get)
/// answers, a reference to the table typed by its key. What differs: the
/// table's type decides the variant, since a script names a table by its name
/// alone; the numbering feature's counters and the references feature's
/// anchors are read through their façades, any other table of `flm-core` as
/// its projection. `'t` is the lifetime of what the table lends, the context's for
/// [`StageCx::get`](crate::api::core::run::StageCx::get).
pub enum TableValue<'t> {
    /// A table of an extension's own.
    Ext(Box<dyn ExtTableRead + 't>),
    /// A table of a rewrite's output trees
    /// ([`TreesTable`](flm_core::core::stage::TreesTable)).
    Trees(Box<dyn TreesTable<'t> + 't>),
    /// A table of a backend's lowered trees, `flm.lowered.<backend>`
    /// ([`LoweredTrees`](flm_core::stdcore::backends::lowering::LoweredTrees)).
    Lowered(Box<dyn LoweredTrees<'t> + 't>),
    /// Any other table, as its projection
    /// ([`Table::project`](flm_core::core::data::table::Table::project)) in its
    /// JSON form. A table of an output's files
    /// ([`StandardTable::ArtifactSet`](crate::api::core::registry::StandardTable::ArtifactSet))
    /// is answered in a form of its own: `{ artifacts: [ARTIFACT, …],
    /// violations: [text, …] }`, each artifact in the data form of
    /// [`api::core::artifact`](crate::api::core::artifact) with the path at
    /// which the host writes it.
    Projection(Value),
    /// The numbering feature's table of the points' numbers
    /// ([`Counters`](flm_core::stdcore::features::numbering::Counters)), read
    /// through its façade.
    Counters(Box<dyn Counters + 't>),
    /// The references feature's table of the points' anchors
    /// ([`Anchors`](flm_core::stdcore::features::refs::Anchors)), read through
    /// its façade.
    Anchors(Box<dyn Anchors + 't>),
}

fn _object_safe(_: &dyn ExtTableRead) {}
