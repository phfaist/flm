//! The store of computed tables, and the checked access through which a stage
//! reads and writes it.
//!
//! The store is the run's collection of computed tables: one writer per table, a
//! declared write always installs a value, and every read and write is checked
//! against the stage's declarations at the access site ([`Computed`]). A table
//! is typed data that one stage writes beside the frozen trees and other stages
//! read ([`Table`]).
//!
//! A stage never changes the store. It reads the frozen trees and a view of the
//! store ([`ComputedView`]), and writes table pieces and diagnostics into a
//! collector ([`StageOut`]); a table piece is the part of one table that one
//! stage invocation produced ([`PieceClass`]). The runner then folds the
//! collectors into the next version of the store with [`fold_outs`].
//!
//! # Determinism of the store
//!
//! During a run, the store changes in three ways:
//! [`Run::new`](crate::core::run::Run::new) installs the values of the external
//! tables, the runner installs the table of each rewrite stage once the rewrite
//! stages are over, and [`fold_outs`] folds the collectors of each later stage
//! into it. The sequential and the parallel runner use the same fold, which
//! follows these rules, so that the store's content never depends on the order
//! in which the stages finished:
//!
//! - the table pieces of one per-tree stage (a stage of
//!   [`Scope::PerTree`](crate::core::stage::Scope::PerTree), run once for each
//!   tree) are absorbed in **the run's order of the trees**
//!   ([`Run::tree_order`](crate::core::run::Run::tree_order)), the order in
//!   which the runner passes them. In document order, a placed tree (the tree of
//!   an included file that opens with a heading, see [`crate::core::run::order`])
//!   comes in the middle of the tree that includes it, so a table
//!   holding entries of every tree holds them tree after tree, not in document
//!   order; a reader that needs document order across trees sorts by
//!   [`StageCx::position`](crate::core::run::StageCx::position);
//! - the tables of one stage are installed in **[`TableId`] order**;
//! - **one writer per table**: installing a table that is already present is
//!   refused with [`StoreError::SecondWriter`]; plan construction already
//!   guarantees that it never happens;
//! - **a declared write always installs a value**: a write that a stage
//!   declared but produced no table piece for still installs an empty table,
//!   so a read that returns `None` always means "no stage provided this table"
//!   and never "a stage ran and found nothing".
//!
//! # Access checks
//!
//! Every read and every write is checked against the stage's declarations
//! **at the access site**, in every build: an undeclared read or write returns
//! an [`AccessError`] that identifies the stage and the table, never a silent
//! `None`. These checks come in addition to the checks that plan construction
//! makes before any stage runs.
//!
//! # Reading during value catch-up
//!
//! [Value catch-up](crate::guide::concepts_overview#value-catch-up) runs
//! per-tree stages again over a synthesized value, a small tree that a stage
//! built during the run. What these stages compute goes into a catch-up store,
//! a [`Computed`] of its own that a view reads on top of the run's store
//! ([`ComputedView::with_overlay`]); value catch-up never writes the run's
//! store. [`ComputedView::with_overlay`] states the reading rule that a feature
//! author has to know.
//!
//! # Items
//!
//! - [`Computed`]: one version of the store, each present table by its
//!   [`TableId`].
//! - [`ComputedView`]: a stage's read view, checked against the stage's
//!   [`ReadSet`].
//! - [`StageOut`]: a stage's write collector, checked against the stage's
//!   [`WriteSet`].
//! - [`fold_outs`] and [`FoldTarget`]: the runner's fold of a stage's
//!   collectors into the store.
//! - [`StoreError`] and [`AccessError`]: the errors of the store and of the
//!   access sites.
//!
//! [`Table`]: crate::core::data::table::Table
//! [`PieceClass`]: crate::core::data::table::PieceClass
//!
//! # Examples
//!
//! One stage writes a table in two calls, the runner folds the two collectors
//! into the store, and a later stage reads the result. Plan construction
//! resolves each stage's [`ReadSet`] and [`WriteSet`], and the runner does the
//! fold; a feature author writes only the two middle steps, the writing into a
//! [`StageOut`] and the reading through a view.
//!
//! ```
//! use flm_core::model::names::StageName;
//! use flm_core::core::data::store::{
//!     Computed, ComputedView, FoldTarget, ReadEntry, ReadSet, StageOut, WriteEntry, WriteSet,
//!     fold_outs,
//! };
//! use flm_core::core::data::table::{SingletonTable, Table, TableId, TableType};
//! use flm_core::lang::origin::FlmDiagnostics;
//!
//! #[derive(Default)]
//! struct Count(u32);
//!
//! impl Table for Count {
//!     fn absorb(&mut self, piece: Count) {
//!         self.0 += piece.0;
//!     }
//! }
//!
//! impl SingletonTable for Count {
//!     const NAME: &'static str = "example.count";
//! }
//!
//! let stage: StageName = "example.counter".into();
//! let id = TableId(0);
//!
//! let mut writes = WriteSet::new();
//! writes.insert("example.count".into(), WriteEntry { id, ty: TableType::of::<Count>() });
//!
//! // Two calls of the stage, each producing one table piece.
//! let mut first = StageOut::new(&stage, &writes);
//! first.table_singleton::<Count>()?.0 = 2;
//! let mut second = StageOut::new(&stage, &writes);
//! second.table_singleton::<Count>()?.0 = 3;
//!
//! // The runner folds them into the next version of the store.
//! let mut store = Computed::new();
//! let mut diags = FlmDiagnostics::new();
//! fold_outs(&stage, &writes, vec![first, second], FoldTarget::Main, &mut store, &mut diags)?;
//!
//! // A stage that declared the read reads the absorbed table.
//! let mut reads = ReadSet::new();
//! reads.insert(
//!     "example.count".into(),
//!     ReadEntry {
//!         id,
//!         ty: TableType::of::<Count>(),
//!         optional: false,
//!         producer: Some(stage.clone()),
//!     },
//! );
//! let reader: StageName = "example.reader".into();
//! let view = ComputedView::new(&reader, &reads, &store);
//! assert_eq!(view.get_singleton::<Count>()?.unwrap().0, 5);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// This module uses the name newtypes of `crate::model::names` and the
// diagnostics alias of `crate::lang::origin`; it does not depend on the
// registry, on plan construction or on the runner.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::model::names::{CapName, StageName};
use crate::core::data::table::{
    PieceClass, SingletonTable, Table, TableId, TableKey, TableRef, TableType,
};
use crate::lang::origin::FlmDiagnostics;

/// One version of the store: each present table, under its [`TableId`], as a
/// shared value.
///
/// The store is the run's collection of computed tables: one writer per table,
/// a declared write always installs a value, and every read and write is
/// checked against the stage's declarations at the access site. `Computed`
/// itself enforces the first rule ([`Computed::install`]); [`fold_outs`]
/// enforces the second; the third is enforced by [`ComputedView`] and
/// [`StageOut`], through which every stage reads and writes. `Computed`
/// performs no access check of its own.
///
/// A [`Run`](crate::core::run::Run) owns the store; an embedding program reads
/// it after the run with [`Run::table`](crate::core::run::Run::table) or
/// [`Run::computed`](crate::core::run::Run::computed). Value catch-up fills a
/// `Computed` of its own, the catch-up store.
///
/// Cloning a `Computed` shares every table rather than copying it (one
/// reference count per table), so deriving the next version of the store is
/// cheap.
#[derive(Clone, Default)]
pub struct Computed {
    tables: BTreeMap<TableId, Arc<dyn Any + Send + Sync>>,
}

impl Computed {
    /// Creates an empty store.
    pub fn new() -> Computed {
        Computed::default()
    }

    /// Returns the type-erased table with this id, if one is present.
    pub fn get_erased(&self, id: TableId) -> Option<&Arc<dyn Any + Send + Sync>> {
        self.tables.get(&id)
    }

    /// Returns the table with this id as a `T`: `None` when no table is
    /// present under `id`, and also `None` when the table present is not a
    /// `T`. The checked views compare the value type before they read, so
    /// they never ask for the wrong type.
    pub fn get_as<T: Table>(&self, id: TableId) -> Option<&T> {
        self.tables.get(&id).and_then(|table| table.downcast_ref::<T>())
    }

    /// Whether a table with this id is present.
    pub fn contains(&self, id: TableId) -> bool {
        self.tables.contains_key(&id)
    }

    /// The ids of the present tables, in `TableId` order.
    pub fn ids(&self) -> impl Iterator<Item = TableId> + '_ {
        self.tables.keys().copied()
    }

    /// The number of present tables.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Whether no table is present.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// Installs `table` under `id`: the table that the table's one writer
    /// produced, or the value of an external table installed when the run is
    /// constructed.
    ///
    /// This performs no check of the value type: the caller must ensure that
    /// `table` is a value of the type declared for `id`. A typed read of a
    /// mismatched table panics ([`ComputedView::get`]). The runner checks the
    /// type before it installs anything.
    ///
    /// # Errors
    ///
    /// [`StoreError::SecondWriter`] if a table is already present under `id`.
    /// A table has one writer, and plan construction guarantees it.
    pub fn install(
        &mut self,
        id: TableId,
        table: Arc<dyn Any + Send + Sync>,
    ) -> Result<(), StoreError> {
        if self.tables.contains_key(&id) {
            return Err(StoreError::SecondWriter { table: id });
        }
        self.tables.insert(id, table);
        Ok(())
    }
}

impl fmt::Debug for Computed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.tables.keys()).finish()
    }
}

/// An error of the store itself, as opposed to an error at an access site
/// ([`AccessError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A table was installed under a [`TableId`] that already holds one. Plan
    /// construction schedules one writer per table, so this error means that
    /// the runner, or a caller that fills a [`Computed`] directly, broke that
    /// invariant.
    SecondWriter {
        /// The number of the table.
        table: TableId,
    },
    /// During [value
    /// catch-up](crate::guide::concepts_overview#value-catch-up), a stage run
    /// over a synthesized value produced a table piece that states a fact
    /// about the document as a whole ([`PieceClass::DocumentFact`]).
    ///
    /// A synthesized value is not part of the document, so it may not add to
    /// the document's facts.
    DocumentFactInValue {
        /// The stage that produced the table piece.
        stage: StageName,
        /// The name of the table.
        table: CapName,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::SecondWriter { table } => {
                write!(f, "{table} is already installed: a table can be installed only once")
            }
            StoreError::DocumentFactInValue { stage, table } => write!(
                f,
                "stage `{stage}`, run over content generated during processing, wrote data \
                 about the whole document into table `{table}`; generated content must not \
                 add data about the whole document"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

/// An error at a table access site: a stage read or wrote a table it did not
/// declare, or used a typed table key whose value type is not the declared
/// one.
///
/// These checks run in every build, so an undeclared access always returns this
/// error, never a silent `None`. [`Run::table`](crate::core::run::Run::table)
/// returns it too, for a table that the registry does not declare or a key of
/// another value type; the stage it reports is then `_host`
/// ([`HOST_FEATURE`](crate::core::registry::builder::HOST_FEATURE)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    /// A stage read a table that it did not declare as a read
    /// ([`Needs::reads`](crate::core::stage::Needs::reads) or
    /// [`Needs::reads_opt`](crate::core::stage::Needs::reads_opt)).
    UndeclaredRead {
        /// The reading stage.
        stage: StageName,
        /// The name of the table.
        table: CapName,
        /// The tables the stage declared as reads, ordinary and optional, in
        /// name order.
        declared: Vec<CapName>,
    },
    /// A stage wrote a table that it did not declare as a write
    /// ([`StageDecl::writes`](crate::core::stage::StageDecl::writes)).
    UndeclaredWrite {
        /// The writing stage.
        stage: StageName,
        /// The name of the table.
        table: CapName,
        /// The tables the stage declared as writes, in name order.
        declared: Vec<CapName>,
    },
    /// A typed table key's value type is not the value type declared for the
    /// table.
    TypeMismatch {
        /// The stage that accessed the table.
        stage: StageName,
        /// The name of the table.
        table: CapName,
        /// The short name of the declared value type.
        declared: &'static str,
        /// The short name of the value type of the key.
        requested: &'static str,
    },
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccessError::UndeclaredRead { stage, table, declared } => write!(
                f,
                "stage `{stage}` reads table `{table}` without declaring it; declared reads: \
                 [{}]",
                join_names(declared)
            ),
            AccessError::UndeclaredWrite { stage, table, declared } => write!(
                f,
                "stage `{stage}` writes table `{table}` without declaring it; declared writes: \
                 [{}]",
                join_names(declared)
            ),
            AccessError::TypeMismatch { stage, table, declared, requested } => write!(
                f,
                "stage `{stage}` accesses table `{table}` with value type `{requested}`, but the \
                 table is declared with value type `{declared}`"
            ),
        }
    }
}

impl std::error::Error for AccessError {}

fn join_names(names: &[CapName]) -> String {
    names.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
}

/// One read that a stage declared, as plan construction resolved it: the
/// table's number and value type, whether the read is optional, and which
/// stage the plan scheduled to write the table.
///
/// The writer is absent for an external table, whose value the caller installs
/// when the run is constructed, and for an optional read that no stage of the
/// plan provides. A plan holds the resolved reads of each stage in the stage's
/// record ([`Plan::manifest`](crate::core::plan::Plan::manifest)).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ReadEntry {
    /// The number of the table.
    pub id: TableId,
    /// The table's declared value type.
    #[serde(skip)]
    pub ty: TableType,
    /// Whether the table may be absent: the stage declared this read with
    /// [`Needs::reads_opt`](crate::core::stage::Needs::reads_opt).
    pub optional: bool,
    /// The stage the plan scheduled to write the table, when there is one.
    pub producer: Option<StageName>,
}

/// The reads that one stage declared, as plan construction resolved them, by
/// table name. A [`ComputedView`] checks every read against it.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Default)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ReadSet {
    entries: BTreeMap<CapName, ReadEntry>,
}

impl ReadSet {
    /// Creates an empty set.
    pub fn new() -> ReadSet {
        ReadSet::default()
    }

    /// Adds a resolved read.
    pub fn insert(&mut self, name: CapName, entry: ReadEntry) {
        self.entries.insert(name, entry);
    }

    /// Returns the resolved read of the table `name`, if the stage declared
    /// one.
    pub fn get(&self, name: &str) -> Option<&ReadEntry> {
        self.entries.get(name)
    }

    /// The declared table names, in name order.
    pub fn names(&self) -> Vec<CapName> {
        self.entries.keys().cloned().collect()
    }

    /// The entries, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&CapName, &ReadEntry)> + '_ {
        self.entries.iter()
    }
}

/// One write that a stage declared, as plan construction resolved it: the
/// table's number and value type.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct WriteEntry {
    /// The number of the table.
    pub id: TableId,
    /// The table's declared value type.
    #[serde(skip)]
    pub ty: TableType,
}

/// The writes that one stage declared, as plan construction resolved them, by
/// table name. A [`StageOut`] checks every write against it, and [`fold_outs`]
/// installs a table for each of them.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Default)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct WriteSet {
    entries: BTreeMap<CapName, WriteEntry>,
}

impl WriteSet {
    /// Creates an empty set.
    pub fn new() -> WriteSet {
        WriteSet::default()
    }

    /// Adds a resolved write.
    pub fn insert(&mut self, name: CapName, entry: WriteEntry) {
        self.entries.insert(name, entry);
    }

    /// Returns the resolved write of the table `name`, if the stage declared
    /// one.
    pub fn get(&self, name: &str) -> Option<&WriteEntry> {
        self.entries.get(name)
    }

    /// The declared table names, in name order.
    pub fn names(&self) -> Vec<CapName> {
        self.entries.keys().cloned().collect()
    }

    /// The entries, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&CapName, &WriteEntry)> + '_ {
        self.entries.iter()
    }

    /// Whether no write is declared.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A stage's read view of the store: typed reads, each checked at the access
/// site against the reads the stage declared, with an optional catch-up store
/// read before the run's store.
///
/// The runner creates one view for each stage call and hands it to the stage
/// inside the [`StageCx`](crate::core::run::StageCx); a stage usually reads
/// through [`StageCx::get`](crate::core::run::StageCx::get), which reads
/// through this view. Read a declared table with [`get`](ComputedView::get) or
/// [`get_singleton`](ComputedView::get_singleton). The view is `Copy` and
/// borrows the store, so passing it around costs nothing.
#[derive(Clone, Copy)]
pub struct ComputedView<'a> {
    stage: &'a StageName,
    reads: &'a ReadSet,
    store: &'a Computed,
    overlay: Option<&'a Computed>,
}

impl<'a> ComputedView<'a> {
    /// Creates a view of `store` for `stage`, checked against `reads`.
    pub fn new(stage: &'a StageName, reads: &'a ReadSet, store: &'a Computed) -> ComputedView<'a> {
        ComputedView { stage, reads, store, overlay: None }
    }

    /// Returns the same view, reading the catch-up store `overlay` before the
    /// run's store: a table present in `overlay` is read from there, any other
    /// table from the run's store.
    ///
    /// **A catch-up store shadows a table whole, never entry by entry.**
    /// During [value catch-up](crate::guide::concepts_overview#value-catch-up),
    /// the catch-up store holds the synthesized value's own table piece of
    /// every table that the stages run over the value write, and a read of
    /// such a table returns that table piece alone, with none of the entries of
    /// the run's store.
    ///
    /// Consequently, a stage that runs during value catch-up and needs data
    /// about the whole document must read a table that the catch-up store can
    /// never hold, that is, a table that no per-tree stage (a stage of
    /// [`Scope::PerTree`](crate::core::stage::Scope::PerTree), run once for
    /// each tree) run during the catch-up writes. In practice, it is a table
    /// that a document-scope stage (a stage of
    /// [`Scope::Document`](crate::core::stage::Scope::Document), run once for
    /// the whole document) writes. The `refs` feature does so:
    /// its resolve stage reads
    /// [`REF_TARGETS`](crate::stdcore::features::refs::REF_TARGETS), written
    /// once for the document, rather than the reference points of
    /// [`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS), written
    /// per tree.
    ///
    /// This rule is the reading side of the rule that
    /// [`StoreError::DocumentFactInValue`] enforces on the writing side: a
    /// synthesized value neither adds facts to the document nor reads them
    /// through a per-tree table.
    ///
    /// See also [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value)
    /// and [`CaughtUp::view`](crate::core::run::CaughtUp::view).
    pub fn with_overlay(self, overlay: &'a Computed) -> ComputedView<'a> {
        ComputedView { overlay: Some(overlay), ..self }
    }

    /// The stage whose reads this view checks.
    pub fn stage(&self) -> &'a StageName {
        self.stage
    }

    /// The resolved reads that this view checks against.
    pub fn reads(&self) -> &'a ReadSet {
        self.reads
    }

    /// The declared entry of a table, checked against the handle's type.
    fn entry(&self, key: &TableRef) -> Result<&'a ReadEntry, AccessError> {
        let Some(entry) = self.reads.get(&key.name) else {
            return Err(AccessError::UndeclaredRead {
                stage: self.stage.clone(),
                table: key.name.clone(),
                declared: self.reads.names(),
            });
        };
        if entry.ty.type_id() != key.type_id {
            return Err(AccessError::TypeMismatch {
                stage: self.stage.clone(),
                table: key.name.clone(),
                declared: entry.ty.short_name(),
                requested: key.short_type_name(),
            });
        }
        Ok(entry)
    }

    fn lookup(&self, id: TableId) -> Option<&'a Arc<dyn Any + Send + Sync>> {
        self.overlay
            .and_then(|overlay| overlay.get_erased(id))
            .or_else(|| self.store.get_erased(id))
    }

    /// Reads a table that the stage declared as a read, through a typed table
    /// key.
    ///
    /// `Ok(None)` means the table is not present, which happens only for an
    /// optional read that no stage of the plan provides: plan construction
    /// schedules a writer for every other read, and
    /// [`Run::new`](crate::core::run::Run::new) requires a value for every
    /// external table the plan needs.
    ///
    /// # Errors
    ///
    /// [`AccessError::UndeclaredRead`] if the stage did not declare a read of
    /// this table; [`AccessError::TypeMismatch`] if the value type of `key` is
    /// not the declared one.
    ///
    /// # Panics
    ///
    /// Panics if the table installed under this name holds a value of another
    /// type than the declared one. A stage cannot cause this: the declared type
    /// is checked here and when the table is written, and
    /// [`Run::new`](crate::core::run::Run::new) checks the type of every
    /// external table's value before it installs it. The panic can only be
    /// reached through a [`Computed`] that a caller filled directly with
    /// [`Computed::install`], which does not check the value type.
    pub fn get<K: TableKey>(&self, key: K) -> Result<Option<&'a K::Table>, AccessError> {
        let entry = self.entry(&key.table_ref())?;
        Ok(self.lookup(entry.id).map(|table| {
            table
                .downcast_ref::<K::Table>()
                .expect("a table is stored under its declared type, which the key matched")
        }))
    }

    /// Reads the one table of a value type `T` that exactly one table uses
    /// ([`SingletonTable`]): [`get`](ComputedView::get) with the table name
    /// `T::NAME`.
    ///
    /// # Errors
    ///
    /// As for [`get`](ComputedView::get).
    ///
    /// # Panics
    ///
    /// As for [`get`](ComputedView::get).
    pub fn get_singleton<T: SingletonTable>(&self) -> Result<Option<&'a T>, AccessError> {
        self.get(T::table_name())
    }

    /// Reads a table that the stage declared as a read, by name and
    /// type-erased, for a reader that has no Rust type for the value, such as a
    /// dump.
    ///
    /// # Errors
    ///
    /// [`AccessError::UndeclaredRead`] if the stage did not declare a read of
    /// this table. No type check applies, since the call gives no value type.
    pub fn get_erased(
        &self,
        name: &str,
    ) -> Result<Option<&'a Arc<dyn Any + Send + Sync>>, AccessError> {
        let Some(entry) = self.reads.get(name) else {
            return Err(AccessError::UndeclaredRead {
                stage: self.stage.clone(),
                table: CapName::from(name),
                declared: self.reads.names(),
            });
        };
        Ok(self.lookup(entry.id))
    }

    /// Returns the stage that plan construction scheduled to write the table
    /// `name`, which this stage declared as a read. A diagnostic uses it to
    /// identify where a missing table entry should have come from
    /// ([`MissingEntry`](crate::core::run::MissingEntry)). `None` for an
    /// external table, for a table that no stage of the plan provides, and for
    /// a table that this stage did not declare as a read.
    pub fn producer_of(&self, name: &str) -> Option<&'a StageName> {
        self.reads.get(name).and_then(|entry| entry.producer.as_ref())
    }
}

impl fmt::Debug for ComputedView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComputedView")
            .field("stage", self.stage)
            .field("reads", &self.reads.names())
            .field("overlay", &self.overlay.is_some())
            .finish()
    }
}

/// One table piece that a stage call produced, with its value type.
struct Piece {
    ty: TableType,
    table: Box<dyn Any + Send + Sync>,
}

/// A stage's write collector for one call: the table pieces that the call
/// produces, each checked at first access against the writes the stage
/// declared, and the diagnostics that the call reports.
///
/// The runner creates one collector for each stage call and passes it to
/// [`Stage::run`](crate::core::stage::Stage::run) together with the
/// [`StageCx`](crate::core::run::StageCx). A stage writes here, never into the
/// store; once the batch is over, the runner folds the collectors of every call
/// of the stage into the store with [`fold_outs`].
pub struct StageOut<'a> {
    stage: &'a StageName,
    writes: &'a WriteSet,
    pieces: BTreeMap<TableId, Piece>,
    /// The diagnostics that the stage call reports. The runner appends them
    /// to the run's diagnostics in the order in which it folds the calls.
    pub diags: FlmDiagnostics,
}

impl<'a> StageOut<'a> {
    /// Creates a collector for one call of `stage`, checked against `writes`.
    pub fn new(stage: &'a StageName, writes: &'a WriteSet) -> StageOut<'a> {
        StageOut { stage, writes, pieces: BTreeMap::new(), diags: FlmDiagnostics::new() }
    }

    /// The stage whose writes this collector checks.
    pub fn stage(&self) -> &'a StageName {
        self.stage
    }

    /// Returns this call's table piece of a table that the stage declared as a
    /// write. The table piece is created empty (the value type's `Default`)
    /// on first access.
    ///
    /// # Errors
    ///
    /// [`AccessError::UndeclaredWrite`] if the stage did not declare a write of
    /// this table; [`AccessError::TypeMismatch`] if `key`'s value type is not
    /// the declared one.
    pub fn table<K: TableKey>(&mut self, key: K) -> Result<&mut K::Table, AccessError> {
        let key = key.table_ref();
        let Some(entry) = self.writes.get(&key.name) else {
            return Err(AccessError::UndeclaredWrite {
                stage: self.stage.clone(),
                table: key.name.clone(),
                declared: self.writes.names(),
            });
        };
        if entry.ty.type_id() != key.type_id {
            return Err(AccessError::TypeMismatch {
                stage: self.stage.clone(),
                table: key.name.clone(),
                declared: entry.ty.short_name(),
                requested: key.short_type_name(),
            });
        }
        let piece = self
            .pieces
            .entry(entry.id)
            .or_insert_with(|| Piece { ty: entry.ty, table: entry.ty.make_default() });
        Ok(piece
            .table
            .downcast_mut::<K::Table>()
            .expect("a piece is created from its declared type, which the key matched"))
    }

    /// Returns this call's table piece of the one table of a value type `T`
    /// that exactly one table uses ([`SingletonTable`]):
    /// [`table`](StageOut::table) with the table name `T::NAME`.
    ///
    /// # Errors
    ///
    /// As for [`table`](StageOut::table).
    pub fn table_singleton<T: SingletonTable>(&mut self) -> Result<&mut T, AccessError> {
        self.table(T::table_name())
    }

    /// Whether this call produced a table piece of the table `id`, that is,
    /// whether [`table`](StageOut::table) was called for it.
    pub fn has_piece(&self, id: TableId) -> bool {
        self.pieces.contains_key(&id)
    }
}

impl fmt::Debug for StageOut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StageOut")
            .field("stage", self.stage)
            .field("pieces", &self.pieces.keys().collect::<Vec<_>>())
            .field("diags", &self.diags.len())
            .finish()
    }
}

/// Which store [`fold_outs`] installs the tables it folds into: the run's
/// store, or the catch-up store of one value catch-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldTarget {
    /// The run's own store. Every declared write installs a table, an empty
    /// one if the stage produced no table piece for it.
    Main,
    /// The catch-up store of one [value
    /// catch-up](crate::guide::concepts_overview#value-catch-up). A table
    /// piece that states a fact about the document as a whole is refused with
    /// [`StoreError::DocumentFactInValue`]. A declared write without a table
    /// piece still installs an empty table, so the catch-up store holds every
    /// table that the stages run during the catch-up write, and a read of
    /// such a table never falls through to the run's store.
    Scratch,
}

/// Folds the collectors of one stage's calls into `store`.
///
/// The runner uses this function on both the sequential and the parallel path,
/// so both produce identical results. The fold:
///
/// - keeps, for each table, the first table piece and absorbs the following
///   ones into it ([`Table::absorb`]) in the order of `outs`; the runner passes
///   the collectors in the run's order of the trees;
/// - installs a table for each write the stage declared, an empty one when the
///   stage produced no table piece for it;
/// - installs the tables in [`TableId`] order;
/// - appends the diagnostics of each collector to `diags`, in the order of
///   `outs`.
///
/// `outs` must be collectors of calls of the stage that `writes` was resolved
/// for.
///
/// # Errors
///
/// [`StoreError::SecondWriter`] if a table folded here is already present in
/// `store`; [`StoreError::DocumentFactInValue`] if `target` is
/// [`FoldTarget::Scratch`] and a table piece states a fact about the document
/// as a whole. On an error, the tables before the refused one in [`TableId`]
/// order are already installed.
///
/// [`Table::absorb`]: crate::core::data::table::Table::absorb
///
/// # Panics
///
/// Panics if a table piece's value is not of the value type that its write
/// declaration gives. Neither [`StageOut`] nor the store can produce such a
/// table piece.
pub fn fold_outs(
    stage: &StageName,
    writes: &WriteSet,
    outs: Vec<StageOut<'_>>,
    target: FoldTarget,
    store: &mut Computed,
    diags: &mut FlmDiagnostics,
) -> Result<(), StoreError> {
    let mut acc: BTreeMap<TableId, Piece> = BTreeMap::new();
    for out in outs {
        for (id, piece) in out.pieces {
            match acc.get_mut(&id) {
                Some(existing) => existing.ty.absorb(&mut existing.table, piece.table),
                None => {
                    acc.insert(id, piece);
                }
            }
        }
        for diagnostic in out.diags {
            diags.push(diagnostic);
        }
    }
    // Declared writes that produced no piece still install, empty.
    for (_, entry) in writes.iter() {
        acc.entry(entry.id)
            .or_insert_with(|| Piece { ty: entry.ty, table: entry.ty.make_default() });
    }
    for (id, piece) in acc {
        if target == FoldTarget::Scratch
            && piece.ty.piece_class(piece.table.as_ref()) == PieceClass::DocumentFact
        {
            let table = writes
                .iter()
                .find(|(_, entry)| entry.id == id)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| CapName::from(id.to_string()));
            return Err(StoreError::DocumentFactInValue { stage: stage.clone(), table });
        }
        store.install(id, Arc::from(piece.table))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct Count(u32);

    impl Table for Count {
        fn absorb(&mut self, piece: Count) {
            self.0 += piece.0;
        }

        fn piece_class(&self) -> PieceClass {
            if self.0 == 0 { PieceClass::Empty } else { PieceClass::DocumentFact }
        }
    }

    impl SingletonTable for Count {
        const NAME: &'static str = "test.count";
    }

    fn writes() -> WriteSet {
        let mut writes = WriteSet::new();
        writes.insert(
            "test.count".into(),
            WriteEntry { id: TableId(3), ty: TableType::of::<Count>() },
        );
        writes
    }

    fn reads(optional: bool) -> ReadSet {
        let mut reads = ReadSet::new();
        reads.insert(
            "test.count".into(),
            ReadEntry {
                id: TableId(3),
                ty: TableType::of::<Count>(),
                optional,
                producer: Some("s".into()),
            },
        );
        reads
    }

    #[test]
    fn pieces_absorb_in_the_order_given_and_install_once() {
        let stage: StageName = "s".into();
        let writes = writes();
        let mut first = StageOut::new(&stage, &writes);
        first.table_singleton::<Count>().unwrap().0 = 1;
        let mut second = StageOut::new(&stage, &writes);
        second.table_singleton::<Count>().unwrap().0 = 2;
        let mut store = Computed::new();
        let mut diags = FlmDiagnostics::new();
        fold_outs(&stage, &writes, vec![first, second], FoldTarget::Main, &mut store, &mut diags)
            .unwrap();
        assert_eq!(store.get_as::<Count>(TableId(3)).unwrap().0, 3);
        let empty = StageOut::new(&stage, &writes);
        let error =
            fold_outs(&stage, &writes, vec![empty], FoldTarget::Main, &mut store, &mut diags)
                .unwrap_err();
        assert_eq!(error, StoreError::SecondWriter { table: TableId(3) });
    }

    #[test]
    fn a_declared_write_without_a_piece_installs_empty() {
        let stage: StageName = "s".into();
        let writes = writes();
        let mut store = Computed::new();
        let mut diags = FlmDiagnostics::new();
        fold_outs(&stage, &writes, Vec::new(), FoldTarget::Main, &mut store, &mut diags).unwrap();
        assert_eq!(store.get_as::<Count>(TableId(3)).unwrap().0, 0);
    }

    #[test]
    fn an_undeclared_write_is_a_typed_error_naming_stage_and_table() {
        let stage: StageName = "s".into();
        let writes = WriteSet::new();
        let mut out = StageOut::new(&stage, &writes);
        let error = out.table_singleton::<Count>().unwrap_err();
        assert!(
            matches!(error, AccessError::UndeclaredWrite { ref table, .. } if table == "test.count")
        );
    }

    #[test]
    fn an_undeclared_read_is_a_typed_error_never_a_silent_none() {
        let stage: StageName = "s".into();
        let reads = ReadSet::new();
        let store = Computed::new();
        let view = ComputedView::new(&stage, &reads, &store);
        let error = view.get_singleton::<Count>().unwrap_err();
        assert!(
            matches!(error, AccessError::UndeclaredRead { ref table, .. } if table == "test.count")
        );
    }

    #[test]
    fn a_declared_read_of_an_absent_table_is_none() {
        let stage: StageName = "s".into();
        let reads = reads(true);
        let store = Computed::new();
        let view = ComputedView::new(&stage, &reads, &store);
        assert!(view.get_singleton::<Count>().unwrap().is_none());
    }

    #[test]
    fn the_overlay_is_read_before_the_main_store() {
        let stage: StageName = "s".into();
        let reads = reads(false);
        let mut main = Computed::new();
        main.install(TableId(3), Arc::new(Count(1))).unwrap();
        let mut scratch = Computed::new();
        scratch.install(TableId(3), Arc::new(Count(7))).unwrap();
        let plain = ComputedView::new(&stage, &reads, &main);
        assert_eq!(plain.get_singleton::<Count>().unwrap().unwrap().0, 1);
        assert_eq!(plain.with_overlay(&scratch).get_singleton::<Count>().unwrap().unwrap().0, 7);
    }

    #[test]
    fn a_document_fact_piece_is_refused_in_scratch() {
        let stage: StageName = "s".into();
        let writes = writes();
        let mut out = StageOut::new(&stage, &writes);
        out.table_singleton::<Count>().unwrap().0 = 1;
        let mut scratch = Computed::new();
        let mut diags = FlmDiagnostics::new();
        let error =
            fold_outs(&stage, &writes, vec![out], FoldTarget::Scratch, &mut scratch, &mut diags)
                .unwrap_err();
        assert!(
            matches!(
                error,
                StoreError::DocumentFactInValue { ref table, .. } if table == "test.count"
            )
        );
    }

    #[test]
    fn a_cloned_version_shares_its_tables() {
        let mut store = Computed::new();
        store.install(TableId(0), Arc::new(Count(4))).unwrap();
        let version = store.clone();
        assert!(Arc::ptr_eq(
            store.get_erased(TableId(0)).unwrap(),
            version.get_erased(TableId(0)).unwrap()
        ));
    }
}
