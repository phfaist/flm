//! External tables, the values that a caller installs into them when it
//! constructs a run, and the stamp that records what such a value was produced
//! under.
//!
//! An external table is a table that no stage of the run writes: the caller
//! installs its value when the run is constructed ([`Externals`]). External
//! tables bring data from outside one document into the stages of a run, for
//! example an index of the labels defined in the other documents of a
//! collection, so that references between the documents resolve. A feature
//! declares an external table with
//! [`FeatureBuilder::external_table`](crate::core::registry::FeatureBuilder::external_table);
//! [`FlmRegistry::external_tables`](crate::core::registry::FlmRegistry::external_tables)
//! lists those a registry declares, and
//! [`Plan::externals`](crate::core::plan::Plan::externals) those a plan needs.
//!
//! The installed values are inputs of the run and do not change during its
//! lifetime. [`Run::new`](crate::core::run::Run::new) refuses to construct a run
//! when a value is missing for an external table that the plan needs, when an
//! install refers to a table that the registry does not declare or does not
//! declare as external, when a value is not of the table's declared type, or when a value's
//! stamp differs from the run's own (see [`RunError`](crate::core::run::RunError)).
//!
//! # Stamps
//!
//! A value that an earlier run of this library produced can carry a [`Stamp`]:
//! the digest of the registry it was produced under
//! ([`crate::model::digest`]), the rewrite stages that ran, and a digest of the
//! values of the options those rewrite stages read.
//! [`Run::new`](crate::core::run::Run::new) compares the stamp of each
//! installed value with the stamp of the run being constructed and refuses a
//! mismatch, so that a run never uses data computed under another registry,
//! other rewrite stages or other option values.
//!
//! Data that did not come from a run of this library has no stamp, and is
//! installed with [`Externals::install_unstamped`]. Nothing then checks that it
//! agrees with the document; the caller is responsible for that.
//!
//! See [external tables and two-run
//! composition](crate::guide::embedding#external-tables-and-two-run-composition)
//! for how an embedding program uses them.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::model::names::{CapName, StageName};
use crate::core::data::table::{TableKey, TableRef};

/// A record of what a value produced by a run of this library was produced
/// under: the registry digest, the rewrite stages that ran, and a digest of
/// the option values that those rewrite stages read.
///
/// Two runs with equal stamps read a document the same way: the same registry,
/// the same rewrite stages in the same order, and the same values for the
/// options those rewrite stages read, so they produce the same frozen trees
/// from the same document. A value whose stamp equals a run's own stamp
/// ([`Run::stamp`](crate::core::run::Run::stamp)) is therefore data that the
/// run could have computed itself. Install such a value with
/// [`Externals::install`]; [`Run::new`](crate::core::run::Run::new) refuses it
/// with [`RunError::StampMismatch`](crate::core::run::RunError::StampMismatch)
/// when the stamps differ.
///
/// Its `Display` form lists the three parts, the two digests in hexadecimal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stamp {
    /// The digest of the registry that the value was produced under
    /// ([`crate::model::digest`]).
    pub digest: u64,
    /// The rewrite stages that ran, in the order in which they ran.
    pub active_rewrites: Vec<StageName>,
    /// A digest of the values of every option that those rewrite stages
    /// read.
    pub option_reads: u64,
}

impl fmt::Display for Stamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "registry digest {:016x}, rewrite stages [{}], digest of the option values they \
             read {:016x}",
            self.digest,
            self.active_rewrites.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", "),
            self.option_reads
        )
    }
}

/// One value installed into one external table, with the value's stamp.
///
/// [`Externals`] holds one `Install` for each table, and
/// [`Run::new`](crate::core::run::Run::new) checks it.
pub struct Install {
    /// The table that the value is installed into, with the value type that
    /// the value was installed as.
    pub table: TableRef,
    /// The value, type-erased. [`Run::new`](crate::core::run::Run::new)
    /// checks its type against the table's declaration.
    pub value: Arc<dyn Any + Send + Sync>,
    /// What the value was produced under, or `None` when it did not come from
    /// a run of this library.
    pub stamp: Option<Stamp>,
}

impl fmt::Debug for Install {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Install").field("table", &self.table).field("stamp", &self.stamp).finish()
    }
}

/// The values that a caller installs into a run's external tables, by table
/// name.
///
/// An external table is a table that no stage of the run writes: the caller
/// installs its value when the run is constructed. See the [module
/// documentation](self) for what external tables are for and for the checks
/// that [`Run::new`](crate::core::run::Run::new) performs.
///
/// Create an empty set with [`Externals::none`], add values with
/// [`Externals::install`] (a value produced by a run of this library, with its
/// [`Stamp`]) or [`Externals::install_unstamped`] (any other value), and pass
/// the set to [`Run::new`](crate::core::run::Run::new). Installing twice into
/// one table keeps the later value.
///
/// # Examples
///
/// ```
/// use flm_core::core::run::Externals;
/// use flm_core::core::data::table::{Table, TableName};
///
/// #[derive(Debug, Default)]
/// struct KnownLabels(Vec<String>);
///
/// impl Table for KnownLabels {
///     fn absorb(&mut self, piece: KnownLabels) {
///         self.0.extend(piece.0);
///     }
/// }
///
/// const KNOWN: TableName<KnownLabels> = TableName::new("example.known-labels");
///
/// // Data from outside this library: nothing checks it against the document.
/// let externals = Externals::none()
///     .install_unstamped(KNOWN, KnownLabels(vec!["thm:main".to_string()]));
///
/// assert_eq!(externals.len(), 1);
/// assert!(externals.get("example.known-labels").is_some());
/// ```
#[derive(Debug, Default)]
pub struct Externals {
    installs: BTreeMap<CapName, Install>,
}

impl Externals {
    /// Creates an empty set: no value installed.
    pub fn none() -> Externals {
        Externals::default()
    }

    /// Installs `value`, produced by an earlier run of this library, into the
    /// external table of `key`, with the stamp that run returned
    /// ([`Run::stamp`](crate::core::run::Run::stamp)).
    ///
    /// [`Run::new`](crate::core::run::Run::new) compares `stamp` with the
    /// stamp of the run being constructed and refuses a mismatch. Use
    /// [`Externals::install_unstamped`] for data that did not come from a run
    /// of this library.
    ///
    /// `value` is taken by value. [`Run::table`](crate::core::run::Run::table)
    /// returns a reference, so a value read from another run is passed as an
    /// owned copy, for example a clone of it or a value rebuilt from it.
    pub fn install<K: TableKey>(mut self, key: K, value: K::Table, stamp: Stamp) -> Externals {
        let table = key.table_ref();
        self.installs.insert(
            table.name.clone(),
            Install { table, value: Arc::new(value), stamp: Some(stamp) },
        );
        self
    }

    /// Installs `value`, which did not come from a run of this library, into
    /// the external table of `key`.
    ///
    /// Nothing checks that it agrees with the document being processed; the
    /// caller is responsible for that.
    pub fn install_unstamped<K: TableKey>(mut self, key: K, value: K::Table) -> Externals {
        let table = key.table_ref();
        self.installs
            .insert(table.name.clone(), Install { table, value: Arc::new(value), stamp: None });
        self
    }

    /// Returns the install of the table `name`, if a value is installed.
    pub fn get(&self, name: &str) -> Option<&Install> {
        self.installs.get(name)
    }

    /// Every install, in table-name order.
    pub fn iter(&self) -> impl Iterator<Item = (&CapName, &Install)> + '_ {
        self.installs.iter()
    }

    /// The number of tables that have an installed value.
    pub fn len(&self) -> usize {
        self.installs.len()
    }

    /// Whether no value is installed.
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty()
    }
}
