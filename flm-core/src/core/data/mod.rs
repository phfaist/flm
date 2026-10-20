//! Computed data: the typed tables that pipeline stages write beside the frozen
//! trees, and the store that holds them during a run.
//!
//! A table is typed data that one stage writes beside the frozen trees and other
//! stages read; its identity is its registered dotted name, never its Rust type,
//! and that name is also a [capability](crate::core::registry::CapabilityDef),
//! the capability of the table having been written ([`Table`](table::Table)).
//! Examples are a map of labels, a set of resolved references, a document-wide
//! count and an output's artifact set
//! ([`ArtifactSet`](crate::core::artifact::ArtifactSet)). Tables exist because the document's trees are frozen
//! once the rewrite stages have run: a stage that runs later cannot write into a
//! tree, so it records what it computes in a table; see [frozen trees and
//! computed data](crate::guide::concepts_overview#frozen-trees-and-computed-data).
//!
//! The store is a run's collection of computed tables: one writer per table, a
//! declared write always installs a value, and every read and write is checked
//! against the stage's declarations at the access site
//! ([`Computed`](store::Computed)). A [`Run`](crate::core::run::Run) owns one
//! store; a stage reads it through a checked view and writes into a collector
//! that the run folds into the store.
//!
//! A feature author declares a table with
//! [`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table),
//! implements [`Table`](table::Table) on its value type, and reads and writes it
//! from a stage through [`StageCx::get`](crate::core::run::StageCx::get) and
//! [`StageOut::table`](store::StageOut::table). An embedding program reads a
//! table after a run with [`Run::table`](crate::core::run::Run::table).
//!
//! # Module map
//!
//! - [`table`]: what a table is and how it is referred to. The trait
//!   [`Table`](table::Table) on a table's value type; the declaration
//!   [`TableDef`](table::TableDef); [`TableType`](table::TableType), a value
//!   type's identity and operations captured for type-erased use; the
//!   registry-assigned number [`TableId`](table::TableId); and the typed table
//!   keys [`TableName<T>`](table::TableName) (a name fixed at compile time) and
//!   [`TableHandle<T>`](table::TableHandle) (a name known only at run time).
//! - [`store`]: the store and the access to it. [`Computed`](store::Computed),
//!   one version of the store; [`ComputedView`](store::ComputedView), a stage's
//!   read view, checked against the reads the stage declared;
//!   [`StageOut`](store::StageOut), a stage's write collector; and
//!   [`fold_outs`](store::fold_outs), which the runner uses to fold the
//!   collectors into the store.
//!
//! The addresses that tables are keyed by,
//! [`TreeId`](crate::model::addr::TreeId) and
//! [`NodeAddr`](crate::model::addr::NodeAddr), are defined in
//! [`crate::model::addr`].

// The crate's module layering, of which this module's position is one line:
//     {model, lang} < core < dump < stdcore < math_passthrough        layout standalone
// This module uses `crate::model` and the diagnostic, span and source aliases of
// `crate::lang`, and no other submodule of `crate::core`; nothing in
// `crate::model` or `crate::lang` uses it. It does not depend on any backend nor
// on `crate::layout`, so that the crate can later be split cheaply. The addresses
// tables are keyed by belong to `crate::model::addr` so that the tree and
// document modules can use them without depending on the store.
// See architecture at [§flm-dd-arch:crates] and design rationale at
// [§flm-dd-dr:crate-layering], [§flm-dd-dr:crate-layering-lang-data],
// [§flm-dd-dr:crate-layering-data-in-core].

pub mod store;
pub mod table;
