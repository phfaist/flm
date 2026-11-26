//! Mirrors [`flm_core::core::data::table`]: reading a table.

use std::sync::Arc;

use flm_core::model::addr::NodeAddr;
use serde_json::Value;

use crate::api::core::stage::TreesTable;
use crate::api::stdlib::backends::lowering::LoweredTrees;
use crate::api::stdlib::features::numbering::Counters;
use crate::api::stdlib::features::refs::Anchors;
use crate::sealed::Sealed;

/// No flm-core counterpart: an extension's own table, read by a stage (the
/// method `items` of the adapter's `ExtTable`, a table type of this crate).
///
/// The trait is sealed: the adapter alone implements it.
pub trait ExtTableRead: Sealed {
    /// No flm-core counterpart: the items the table's writers pushed, in the
    /// order they were pushed, as the sequence [`Rows`], which shares them
    /// with the table. Across a binding, a sequence of rows (see the
    /// [mapping rules](crate::api#the-mapping-rules)).
    fn items(&self) -> Rows;

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
    Trees(Box<dyn TreesTable + 't>),
    /// A table of a backend's lowered trees, `flm.lowered.<backend>`
    /// ([`LoweredTrees`](flm_core::stdlib::backends::lowering::LoweredTrees)).
    Lowered(Box<dyn LoweredTrees + 't>),
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
    /// ([`Counters`](flm_core::stdlib::features::numbering::Counters)), read
    /// through its façade.
    Counters(Box<dyn Counters + 't>),
    /// The references feature's table of the points' anchors
    /// ([`Anchors`](flm_core::stdlib::features::refs::Anchors)), read through
    /// its façade.
    Anchors(Box<dyn Anchors + 't>),
}

/// No flm-core counterpart: the rows of an extension's own table, in the
/// order its writers pushed them, as [`ExtTableRead::items`] answers them.
///
/// The rows are shared with the table: a row is copied when it is read.
/// Across a binding, a [sequence](crate::api#the-mapping-rules): one handle,
/// whose length crosses with it, from which the extension reads a row by its
/// index or iterates the rows, as often as it needs. A binding pulls the rows
/// in groups bounded in number and in total size, so that a large row
/// crosses alone.
#[derive(Debug, Clone)]
pub struct Rows {
    rows: Arc<Vec<Value>>,
}

impl Rows {
    /// The sequence of `rows`, shared.
    pub(crate) fn new(rows: Arc<Vec<Value>>) -> Rows {
        Rows { rows }
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the table has no row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The row at the index `i`, copied, or `None` past the last row.
    pub fn get(&self, i: usize) -> Option<Value> {
        self.rows.get(i).cloned()
    }

    /// The rows, in order, each copied.
    pub fn iter(&self) -> std::iter::Cloned<std::slice::Iter<'_, Value>> {
        self.rows.iter().cloned()
    }

    /// The rows from the index `from` that a binding's pull answers, copied:
    /// at most `max` rows, and no more than fit in `bytes` bytes of JSON text,
    /// except that the first row is answered whatever its size, so that a
    /// large row crosses alone. None from the end on.
    pub(crate) fn group(&self, from: usize, max: usize, bytes: usize) -> Vec<Value> {
        let mut group = Vec::new();
        let mut size = 0;
        for row in self.rows.iter().skip(from).take(max) {
            size += json_size(row);
            if !group.is_empty() && size > bytes {
                break;
            }
            group.push(row.clone());
        }
        group
    }
}

impl<'r> IntoIterator for &'r Rows {
    type Item = Value;
    type IntoIter = std::iter::Cloned<std::slice::Iter<'r, Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// The length of `value`'s JSON text, counted as it is written, without
/// keeping it.
fn json_size(value: &Value) -> usize {
    /// A writer that counts the bytes it is given.
    struct Count(usize);

    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut count = Count(0);
    // A value that cannot be written counts as nothing; writing to a counter
    // fails only on such a value.
    let _ = serde_json::to_writer(&mut count, value);
    count.0
}

fn _object_safe(_: &dyn ExtTableRead) {}
