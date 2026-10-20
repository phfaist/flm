//! Tables and the ways to refer to them: the trait [`Table`] that a table's
//! value type implements, the declaration [`TableDef`], the captured value type
//! [`TableType`], the registry-assigned number [`TableId`], and the typed table
//! keys [`TableName<T>`] and [`TableHandle<T>`].
//!
//! A table is typed data that one stage writes beside the frozen trees and other
//! stages read; its identity is its registered dotted name, never its Rust type,
//! and that name is also a [capability](crate::core::registry::CapabilityDef),
//! the capability of the table having been written ([`Table`]).
//!
//! # Names, value types and table numbers
//!
//! A feature declares a table ([`TableDef`]) under a dotted name that the
//! feature owns (`flm.labels`, `flm.lowered.html`), with one Rust value type.
//! When the registry is built, each declared table receives a [`TableId`], a
//! number that the store uses as the table's key.
//!
//! The name, not the Rust type, identifies a table, because one Rust type can
//! hold several tables: the type holding lowered trees is declared once per
//! backend, so the type alone cannot say which table is meant. The value type is
//! captured at declaration as a [`TableType`] (its Rust `TypeId` together with
//! its operations as plain function pointers), so that the store, its fold
//! ([`fold_outs`](crate::core::data::store::fold_outs)) and the inspection dumps
//! ([`crate::dump`]) can handle every table without naming its Rust type.
//!
//! When the registry is built, it compares the value type of every table
//! reference in a stage's declarations with the table's declared value type. At
//! every access through a typed table key,
//! [`ComputedView::get`](crate::core::data::store::ComputedView::get),
//! [`StageOut::table`](crate::core::data::store::StageOut::table) and
//! [`Run::table`](crate::core::run::Run::table) compare the key's value type
//! with the declared one and return
//! [`AccessError::TypeMismatch`](crate::core::data::store::AccessError::TypeMismatch)
//! on a mismatch, so the conversion back to the value type behind the key cannot
//! fail.
//!
//! # Typed table keys
//!
//! A reader or a writer refers to a table through a typed table key, which
//! carries the table's name and its value type, so that a read returns the
//! value already typed:
//!
//! - a [`TableName<T>`] constant, for a table with a name fixed at compile
//!   time; the crate that defines the value type exports it beside the type
//!   (`pub const LABELS: TableName<Labels> = TableName::new("flm.labels")`);
//! - a [`TableHandle<T>`], for a table declared under a name computed at run
//!   time;
//! - the value type itself, when it implements [`SingletonTable`] because
//!   exactly one table uses it: `T::NAME` is then the name that the store's
//!   singleton methods use.
//!
//! The read and write methods of a stage
//! ([`ComputedView::get`](crate::core::data::store::ComputedView::get),
//! [`StageOut::table`](crate::core::data::store::StageOut::table),
//! [`StageCx::get`](crate::core::run::StageCx::get)), [`Run::table`](crate::core::run::Run::table)
//! and [`Externals::install`](crate::core::run::Externals::install) accept any
//! [`TableKey`]. Every typed table key has a
//! type-erased form, [`TableRef`] (the name plus the identity of the value
//! type), which stage declarations, plans and messages carry.
//!
//! # A table's name is a capability
//!
//! Every table's name is also a
//! [capability](crate::core::registry::CapabilityDef): the capability of the
//! table having been written. A stage that declares a write of the table therefore provides
//! that capability, and a stage that declares a read of it requires it; the
//! registry derives both. See [stages, capabilities, and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans).

// This module uses the name newtypes of `crate::model::names` and techy's
// serialized-value type; it does not depend on the registry, on stages, on plan
// construction or on the runner.

use std::any::{Any, TypeId};
use std::fmt;
use std::marker::PhantomData;

use techy::core::node::NodeTree;
use techy::serialize::SerialValue;

use crate::lang::Flm;
use crate::model::addr::{NodeAddr, TreeId};
use crate::model::names::{CapName, FeatureName};
use crate::model::options::short_type_name;

/// The context through which a table writes node addresses, tree identifiers
/// and trees when it serializes itself for inspection ([`Table::project`]).
///
/// A table that holds node addresses or tree identifiers calls these methods
/// rather than writing an address itself, so that the serialized form of every
/// table writes addresses the same way. The library's implementation is
/// [`TableProjection`](crate::dump::project::TableProjection), which resolves
/// each address against the trees it was built over.
///
/// A table that holds a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) writes that tree
/// in full through [`ProjectCx::tree`], never by its identifier: value catch-up
/// gives a synthesized value its identifier, numbered per stage call
/// ([`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value)), so the
/// identifier means something only inside the stage call that brought the value
/// up to date, and a reader of the serialized table could not resolve it. See
/// [`TreeId`].
///
/// See also [`crate::dump::project`].
pub trait ProjectCx {
    /// Writes a node address; the library's implementation writes the node's
    /// element name and source position, as `element@line:col`.
    fn node_ref(&self, addr: NodeAddr) -> SerialValue;

    /// Writes a tree identifier.
    fn tree_ref(&self, id: TreeId) -> SerialValue;

    /// Writes a synthesized value that a table holds: the tree itself, in
    /// full. The library's implementation writes its nodes as it writes any
    /// tree; the default implementation writes the tree's plain text
    /// ([`plain_text`](crate::lang::text::plain_text)).
    fn tree(&self, tree: &NodeTree<Flm>) -> SerialValue {
        SerialValue::Str(crate::lang::text::plain_text(tree))
    }
}

/// What a table piece holds: data keyed by node address, nothing, or a fact
/// about the document as a whole.
///
/// A table piece is the part of one table that one stage invocation produced;
/// the runner folds the pieces into the store ([`Table::absorb`]), and a
/// piece's class says whether it states a fact about the document as a whole.
/// A table's value type classifies its own table pieces with
/// [`Table::piece_class`].
///
/// The class matters only during [value
/// catch-up](crate::guide::concepts_overview#value-catch-up), which runs
/// per-tree stages (stages of
/// [`Scope::PerTree`](crate::core::stage::Scope::PerTree), run once for each
/// tree) again over a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) and folds their
/// table pieces into a catch-up store of its own. A table piece keyed by node
/// address, or an empty one, is accepted there. A table piece that states a
/// fact about the document as a whole (a label declared inside the synthesized
/// value, a numbered point) is refused with
/// [`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue),
/// because a synthesized value is not part of the document and must not add to
/// the document's facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceClass {
    /// The table piece holds data keyed by node address; an empty table keyed
    /// by node address also returns this class.
    NodeKeyed,
    /// The table piece of a table not keyed by node address holds nothing.
    Empty,
    /// The table piece states a fact about the document as a whole, such as
    /// an entry keyed by a label or a count; a synthesized value may not
    /// produce one.
    DocumentFact,
}

/// The trait of a table's value type.
///
/// A table is typed data that one stage writes beside the frozen trees and other
/// stages read; its identity is its registered dotted name, never its Rust type,
/// and that name is also a [capability](crate::core::registry::CapabilityDef).
/// See [frozen trees and computed
/// data](crate::guide::concepts_overview#frozen-trees-and-computed-data) for
/// why computed data is kept in tables, and the [module documentation](self) for
/// how names, value types and typed table keys relate.
///
/// A map keyed by node address and a map keyed by a symbolic name are the usual
/// shapes of a table, but any value type can be one: a document-wide count is a
/// table whose [`absorb`](Table::absorb) adds.
///
/// To add a table, implement this trait on its value type, declare the table
/// in a feature with
/// [`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table) (or
/// [`FeatureBuilder::external_table`](crate::core::registry::FeatureBuilder::external_table)
/// for a table no stage writes), declare it as a write of the stage that
/// produces it ([`StageDecl::writes`](crate::core::stage::StageDecl::writes)) and
/// as a read of each stage that reads it
/// ([`Needs::reads`](crate::core::stage::Needs::reads)), and export a
/// [`TableName`] constant for its readers. A stage
/// writes its part of the table through
/// [`StageOut::table`](crate::core::data::store::StageOut::table), and a later
/// stage reads the whole table through
/// [`StageCx::get`](crate::core::run::StageCx::get).
///
/// The value type must implement `Default`: a stage's table piece starts as the
/// default value, and a stage that declared a write but produced nothing still
/// installs a default (empty) table.
///
/// See also [`SingletonTable`] for a value type used by exactly one table, and
/// [`TableType`] for the type-erased form of a value type.
pub trait Table: Any + Send + Sync + Default {
    /// Combines `piece`, a table piece produced by the same stage, into
    /// `self`.
    ///
    /// The runner calls this when it folds one stage's table pieces into the
    /// store: it keeps the first table piece and absorbs each following one
    /// into it, in the run's order of the trees
    /// ([`Run::tree_order`](crate::core::run::Run::tree_order)). The resulting
    /// table must therefore depend only on the table pieces and their order.
    /// The table type decides what happens on a collision: a table of labels,
    /// for example, records the duplicate so that a later check stage can
    /// report it.
    fn absorb(&mut self, piece: Self);

    /// Serializes the table as plain data, for the inspection dumps
    /// ([`crate::dump`]) and tests.
    ///
    /// The default implementation returns a placeholder holding the short name
    /// of the Rust type, `<TypeName>`. A table with a readable serialized form
    /// overrides it and writes every node address, tree identifier or
    /// synthesized value it holds through `cx`; see [`ProjectCx`].
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        let _ = cx;
        SerialValue::Str(format!("<{}>", short_type_name(std::any::type_name::<Self>())))
    }

    /// Classifies `self`, taken as a table piece: see [`PieceClass`].
    ///
    /// The runner calls this only on the table pieces produced during value
    /// catch-up. The default implementation returns
    /// [`NodeKeyed`](PieceClass::NodeKeyed), which fits every table keyed by
    /// node address. A table keyed symbolically, or holding a document-wide
    /// value, returns [`Empty`](PieceClass::Empty) when it holds nothing and
    /// [`DocumentFact`](PieceClass::DocumentFact) otherwise.
    fn piece_class(&self) -> PieceClass {
        PieceClass::NodeKeyed
    }
}

/// A table value type that exactly one table uses: [`NAME`](SingletonTable::NAME)
/// is that table's name.
///
/// A value type that implements this trait lets a reader or a writer refer to
/// its table by the Rust type alone: [`StageCx::get_singleton`],
/// [`ComputedView::get_singleton`] and [`StageOut::table_singleton`] find the
/// name through it.
///
/// A value type declared under several names, such as the type holding one
/// backend's lowered trees, must not implement this trait, since it has no
/// single name. For that reason `NAME` is defined on this separate trait rather
/// than on [`Table`]: on [`Table`], it would force such a value type to invent a
/// name.
///
/// [`StageCx::get_singleton`]: crate::core::run::StageCx::get_singleton
/// [`ComputedView::get_singleton`]: crate::core::data::store::ComputedView::get_singleton
/// [`StageOut::table_singleton`]: crate::core::data::store::StageOut::table_singleton
pub trait SingletonTable: Table {
    /// The name of the one table that uses this value type.
    const NAME: &'static str;

    /// The typed table key ([`TableName`]) of that one table.
    fn table_name() -> TableName<Self>
    where
        Self: Sized,
    {
        TableName::new(Self::NAME)
    }
}

type ErasedTable = Box<dyn Any + Send + Sync>;

// The three helpers below downcast a type-erased table back to `T`. The
// downcast is unreachable-by-construction — the store keys by `TableId` and the
// registry verified the value type at declaration — so they `expect` with a
// message naming the invariant rather than returning a result nobody could act
// on. See design rationale at [§flm-dd-dr:err-unreachable-typed].

fn make_default_erased<T: Table>() -> ErasedTable {
    Box::new(T::default())
}

fn absorb_erased<T: Table>(acc: &mut ErasedTable, piece: ErasedTable) {
    let piece = *piece
        .downcast::<T>()
        .expect("a table piece is combined only with a piece of its declared type");
    acc.downcast_mut::<T>()
        .expect("a table piece is combined only with a piece of its declared type")
        .absorb(piece);
}

fn project_erased<T: Table>(table: &(dyn Any + Send + Sync), cx: &dyn ProjectCx) -> SerialValue {
    table.downcast_ref::<T>().expect("a table is projected through its declared type").project(cx)
}

fn piece_class_erased<T: Table>(table: &(dyn Any + Send + Sync)) -> PieceClass {
    table
        .downcast_ref::<T>()
        .expect("a table is classified through its declared type")
        .piece_class()
}

/// A table value type's identity and operations, captured when the table is
/// declared, so that the store, its fold
/// ([`fold_outs`](crate::core::data::store::fold_outs)) and the inspection dumps
/// ([`crate::dump`]) can handle every table without naming its Rust type.
///
/// A `TableType` holds the value type's `TypeId` (checked so that converting a
/// type-erased table back to its value type cannot fail), its Rust name (for
/// messages), a constructor of an empty value (so that a declared write with no
/// table piece still installs a table), and the three operations of [`Table`]:
/// [`absorb`](Table::absorb), [`project`](Table::project) and
/// [`piece_class`](Table::piece_class).
///
/// Create one with [`TableType::of`]. A [`TableDef`] carries one, and so do the
/// resolved reads and writes of a stage
/// ([`ReadEntry`](crate::core::data::store::ReadEntry),
/// [`WriteEntry`](crate::core::data::store::WriteEntry)).
///
/// Two `TableType`s are equal exactly when they capture the same Rust type.
#[derive(Clone, Copy)]
pub struct TableType {
    type_id: TypeId,
    type_name: &'static str,
    make_default: fn() -> ErasedTable,
    absorb: fn(&mut ErasedTable, ErasedTable),
    project: fn(&(dyn Any + Send + Sync), &dyn ProjectCx) -> SerialValue,
    piece_class: fn(&(dyn Any + Send + Sync)) -> PieceClass,
}

impl TableType {
    /// Captures the identity and operations of the value type `T`.
    pub fn of<T: Table>() -> TableType {
        TableType {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            make_default: make_default_erased::<T>,
            absorb: absorb_erased::<T>,
            project: project_erased::<T>,
            piece_class: piece_class_erased::<T>,
        }
    }

    /// The value type's `TypeId`.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// The value type's full Rust name.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// The value type's short name, for messages.
    pub fn short_name(&self) -> &'static str {
        short_type_name(self.type_name)
    }

    /// Whether `T` is the captured value type.
    pub fn is<T: Any>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    /// Creates an empty table of the value type (its `Default` value),
    /// type-erased.
    pub fn make_default(&self) -> ErasedTable {
        (self.make_default)()
    }

    /// Absorbs the table piece `piece` into `acc`, through the value type's
    /// [`Table::absorb`].
    ///
    /// # Panics
    ///
    /// Panics if `acc` or `piece` is not a value of the captured value type.
    /// The store keys tables by [`TableId`] and the registry checked the value
    /// type when the table was declared, so a caller working through the store
    /// cannot cause this panic.
    pub fn absorb(&self, acc: &mut ErasedTable, piece: ErasedTable) {
        (self.absorb)(acc, piece)
    }

    /// Serializes `table` for inspection, through the value type's
    /// [`Table::project`].
    ///
    /// # Panics
    ///
    /// Panics if `table` is not a value of the captured value type; see
    /// [`absorb`](TableType::absorb).
    pub fn project(&self, table: &(dyn Any + Send + Sync), cx: &dyn ProjectCx) -> SerialValue {
        (self.project)(table, cx)
    }

    /// Classifies the table piece `table`, through the value type's
    /// [`Table::piece_class`].
    ///
    /// # Panics
    ///
    /// Panics if `table` is not a value of the captured value type; see
    /// [`absorb`](TableType::absorb).
    pub fn piece_class(&self, table: &(dyn Any + Send + Sync)) -> PieceClass {
        (self.piece_class)(table)
    }
}

impl PartialEq for TableType {
    fn eq(&self, other: &TableType) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for TableType {}

impl fmt::Debug for TableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TableType({})", self.short_name())
    }
}

/// A table as a feature declares it: its name, which is also its capability;
/// its value type; its description; and the feature that owns it.
///
/// [`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table) and
/// [`FeatureBuilder::external_table`](crate::core::registry::FeatureBuilder::external_table)
/// create it; the built registry holds it in the table's
/// [`TableEntry`](crate::core::registry::TableEntry).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TableDef {
    /// The table's dotted name, which is also the name of the capability of
    /// the table having been written.
    pub name: CapName,
    /// The value type's captured identity and operations.
    #[serde(skip)]
    pub ty: TableType,
    /// A description of what the table holds, written for a person. For an
    /// external table that a run needs and the caller did not install, the
    /// error [`RunError::MissingExternal`](crate::core::run::RunError::MissingExternal)
    /// includes it.
    pub docs: String,
    /// The feature that declared the table.
    pub owner: FeatureName,
}

/// The number of a declared table within one registry; the store keys its
/// tables by it.
///
/// The registry assigns the numbers when it is built, consecutively and in its
/// own deterministic order. A `TableId` identifies a table only within the
/// registry that assigned it: the same number in another registry may refer to
/// another table. Its `Display` form is `table#N`.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TableId(pub u32);

impl fmt::Display for TableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "table#{}", self.0)
    }
}

/// The type-erased form of a typed table key: the table's name together with
/// the identity of the value type the key expects.
///
/// Stage declarations, plans and messages carry a `TableRef`, because none of
/// them is generic over the value type. The registry checks every `TableRef`
/// of a stage declaration against the [`TableDef`] of the same name when it is
/// built; a typed table key used at an access site is checked again there.
/// Its `Display` form is the name followed by the value type's short name in
/// parentheses.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TableRef {
    /// The table's name.
    pub name: CapName,
    /// The expected value type's identity.
    #[serde(skip)]
    pub type_id: TypeId,
    /// The expected value type's full Rust name, for messages.
    pub type_name: &'static str,
}

impl TableRef {
    /// Creates a reference to the table `name`, whose value type is `T`.
    pub fn of<T: Table>(name: impl Into<CapName>) -> TableRef {
        TableRef {
            name: name.into(),
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
        }
    }

    /// The value type's short name, for messages.
    pub fn short_type_name(&self) -> &'static str {
        short_type_name(self.type_name)
    }
}

impl fmt::Display for TableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.short_type_name())
    }
}

/// A typed table key: a value that refers to a table together with its value
/// type, namely a [`TableName`], a [`TableHandle`], or a reference to either.
///
/// The read and write methods of [`ComputedView`], [`StageOut`], [`StageCx`]
/// and [`Run`], and [`Externals::install`], are generic over this trait, so a
/// caller passes whichever key it has.
///
/// [`ComputedView`]: crate::core::data::store::ComputedView
/// [`StageOut`]: crate::core::data::store::StageOut
/// [`StageCx`]: crate::core::run::StageCx
/// [`Run`]: crate::core::run::Run
/// [`Externals::install`]: crate::core::run::Externals::install
pub trait TableKey {
    /// The table's value type.
    type Table: Table;

    /// The type-erased form of this key.
    fn table_ref(&self) -> TableRef;

    /// The table's name.
    fn table_name(&self) -> CapName {
        self.table_ref().name
    }
}

impl<K: TableKey + ?Sized> TableKey for &K {
    type Table = K::Table;

    fn table_ref(&self) -> TableRef {
        (**self).table_ref()
    }
}

/// A typed table key for a table whose name is fixed at compile time.
///
/// The crate that defines a table's value type exports one constant of this
/// type beside the value type for each table it declares under a fixed name:
///
/// ```
/// use flm_core::core::data::table::{Table, TableName};
///
/// #[derive(Default)]
/// struct Labels;
/// impl Table for Labels {
///     fn absorb(&mut self, _piece: Labels) {}
/// }
///
/// pub const LABELS: TableName<Labels> = TableName::new("example.labels");
/// assert_eq!(LABELS.name(), "example.labels");
/// ```
///
/// Use a [`TableHandle`] instead when the name is known only at run time;
/// [`TableName::handle`] converts a `TableName` into one.
pub struct TableName<T> {
    name: &'static str,
    _table: PhantomData<fn() -> T>,
}

impl<T: Table> TableName<T> {
    /// Creates the key of the table `name`, whose value type is `T`. The
    /// function is `const`, so the key can be a constant.
    pub const fn new(name: &'static str) -> TableName<T> {
        TableName { name, _table: PhantomData }
    }

    /// The table's name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The [`TableHandle`] of the same table.
    pub fn handle(&self) -> TableHandle<T> {
        TableHandle::new(self.name)
    }
}

impl<T> Clone for TableName<T> {
    fn clone(&self) -> TableName<T> {
        *self
    }
}

impl<T> Copy for TableName<T> {}

impl<T> fmt::Debug for TableName<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TableName<{}>({:?})", short_type_name(std::any::type_name::<T>()), self.name)
    }
}

impl<T: Table> TableKey for TableName<T> {
    type Table = T;

    fn table_ref(&self) -> TableRef {
        TableRef::of::<T>(self.name)
    }
}

/// A typed table key for a table whose name is known only at run time.
///
/// Use it for a table declared under a computed name, such as the table of
/// lowered trees of each backend, `flm.lowered.<backend>`, or under a name read
/// from configuration. Where the name is fixed at compile time, use a
/// [`TableName`] instead.
///
/// [`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table)
/// returns a handle for the table it declares. The registry checks the name and
/// the value type when it is built, so a handle returned by a declaration always
/// reads the table as its own value type. A handle created by hand, pairing a
/// name with a value type that the registry does not pair it with, is refused
/// at the access site with
/// [`AccessError::TypeMismatch`](crate::core::data::store::AccessError::TypeMismatch);
/// it does not cause a panic.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableHandle<T> {
    name: CapName,
    _table: PhantomData<fn() -> T>,
}

impl<T: Table> TableHandle<T> {
    /// Creates the handle of the table `name`, whose value type is `T`.
    pub fn new(name: impl Into<CapName>) -> TableHandle<T> {
        TableHandle { name: name.into(), _table: PhantomData }
    }

    /// The table's name.
    pub fn name(&self) -> &CapName {
        &self.name
    }
}

impl<T> Clone for TableHandle<T> {
    fn clone(&self) -> TableHandle<T> {
        TableHandle { name: self.name.clone(), _table: PhantomData }
    }
}

impl<T> fmt::Debug for TableHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TableHandle<{}>({:?})", short_type_name(std::any::type_name::<T>()), self.name)
    }
}

impl<T: Table> TableKey for TableHandle<T> {
    type Table = T;

    fn table_ref(&self) -> TableRef {
        TableRef::of::<T>(self.name.clone())
    }
}

impl<T: Table> From<TableName<T>> for TableHandle<T> {
    fn from(name: TableName<T>) -> TableHandle<T> {
        name.handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Count(u32);

    impl Table for Count {
        fn absorb(&mut self, piece: Count) {
            self.0 += piece.0;
        }
    }

    impl SingletonTable for Count {
        const NAME: &'static str = "test.count";
    }

    const COUNT: TableName<Count> = TableName::new("test.count");

    #[test]
    fn a_table_type_absorbs_and_defaults_through_the_vtable() {
        let ty = TableType::of::<Count>();
        let mut acc = ty.make_default();
        ty.absorb(&mut acc, Box::new(Count(2)));
        ty.absorb(&mut acc, Box::new(Count(3)));
        assert_eq!(acc.downcast_ref::<Count>().unwrap().0, 5);
        assert!(ty.is::<Count>());
        assert_eq!(ty.short_name(), "Count");
    }

    #[test]
    fn typed_keys_erase_to_the_same_reference() {
        let by_const = COUNT.table_ref();
        let by_handle = TableHandle::<Count>::new("test.count").table_ref();
        let by_singleton = Count::table_name().table_ref();
        assert_eq!(by_const, by_handle);
        assert_eq!(by_const, by_singleton);
        let by_reference = <&TableName<Count> as TableKey>::table_ref(&&COUNT);
        assert_eq!(by_reference, by_const);
        assert_eq!(by_const.short_type_name(), "Count");
    }

    #[test]
    fn the_default_projection_names_the_type() {
        struct NoCx;
        impl ProjectCx for NoCx {
            fn node_ref(&self, _: NodeAddr) -> SerialValue {
                SerialValue::Null
            }
            fn tree_ref(&self, _: TreeId) -> SerialValue {
                SerialValue::Null
            }
        }
        assert_eq!(Count(1).project(&NoCx), SerialValue::Str("<Count>".to_string()));
        assert_eq!(Count(1).piece_class(), PieceClass::NodeKeyed);
    }
}
