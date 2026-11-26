//! Combining settings written in several ranked sources into one value per
//! feature and key, each value keeping where it was written.
//!
//! A host that reads settings from several places (a program's defaults, the
//! packages a document uses, the document's front matter, a command line, …)
//! records every value written in a [`Cascade`], each with its *origin*: where
//! it was written, as the host knows it. The origin is the host's own type,
//! which implements [`CascadeOrigin`]: it gives the cascade the source's rank,
//! and, for a source that is a package, the package's identity. The cascade
//! decides which value results for each key by the rule below, reads the result
//! through each feature's [settings declaration](crate::core::settings::SettingsDecl)
//! ([`Cascade::read`]) and reports every problem found at the origin of the
//! value at fault ([`Problem`]). The rank order, the sources and the texts that
//! show an origin are the host's; this module reads no file, parses no
//! notation and does not touch the machine.
//!
//! One rule applies to every key: the source of the highest rank that writes a
//! key gives its value, whole, replacing what lower sources wrote. Within a rank
//! whose origins have no [peer](CascadeOrigin::peer), the value written last
//! wins. Every value written is kept, and the rule is applied when the settings
//! are read.
//!
//! # The rule between packages
//!
//! Between packages (the origins of a rank that answer
//! [`peer`](CascadeOrigin::peer) with the package's identity), the packages'
//! uses decide. A package *reaches* the packages it uses, and the packages those
//! reach ([`Reach`], computed by
//! [`PackageCatalog::reach`](crate::core::registry::PackageCatalog::reach) and
//! given to [`Cascade::with_reach`]). Among the packages that write one key, a
//! package that another writer reaches does not count, unless it reaches that
//! writer too. So a package wins over the packages it uses, directly or through
//! others, without any message: a package `my-article` that uses `article` and
//! changes one of its values gives that value. The packages that count are
//! *peers*: sources of one rank, none of which wins over another. Peers that
//! write the same value for a key agree; peers that write two different values
//! are a problem, unless a source of a higher rank also writes the key and so
//! decides it. Two packages that reach each other (a loop of uses) are peers,
//! and so are two packages of which neither reaches the other. Neither the order
//! of the uses nor the order in which the values are added changes the result.
//! The settings that several packages give a workflow follow the same rule, by
//! the packages they are written in.
//!
//! # Values combined key by key
//!
//! The marks of a feature's settings declaration ([`Mark`]) decide how a value
//! is combined, at any depth below a key; the cascade names no feature and no
//! key of its own.
//!
//! - **[`Mark::MergeObjectKeyValues`]**: the sources' values of an object that
//!   carries the mark are combined key by key. Each key of the object is
//!   decided by the rule above (the source of the highest rank that writes it
//!   gives its value whole, and two peers must agree on it); where the node of
//!   that key carries the mark too, the rule applies again one level down. The
//!   feature's own default of the object (the default of its node,
//!   [`default_entries`](SettingsDecl::default_entries), or the entry of the
//!   default above it) is the lowest source, so that a source adds a key and
//!   leaves the others. The keys are ordered by source, lowest rank first,
//!   the default's first, each source's keys in the order written, and a
//!   replaced key keeps its position. A value that is not an object where the
//!   mark stands is a problem at its origin.
//! - **[`Mark::RemoveKeyWhenValueIsFalse`]**: in an object that carries the
//!   mark, a key whose combined value is `false` is left out, a key of the
//!   feature's own default included.
//! - **[`Mark::SetToDefaultWithNull`]**: a setting that carries the mark and
//!   whose combined value is `null` is left out of the combined settings, so
//!   that whatever reads them applies the setting's default.
//!
//! Nothing else is combined inside a value. Each value that the rule decides
//! keeps the origin of the write that gives it, at every depth
//! ([`KeyInEffect::entry_origins`], [`KeyInEffect::below`]), and
//! [`Cascade::writes_at`] lists every source's value at one place, and
//! [`Cascade::decided_at`] finds the place decided whole that holds it.
//!
//! # The fields of a workflow
//!
//! The fields of a workflow that packages write (`extends`, `outputs`,
//! `demands`, `prefer`, `docs`) are values too, one per workflow and field, and
//! follow the same rule between peers ([`Cascade::add_workflow_fields`]).
//! [`Cascade::workflows`] returns the resulting fields per workflow, which
//! [`workflow_def`] turns into the library's [`WorkflowDef`]; the host gives
//! these to the library, which combines them with the workflow of that name
//! defined by the compiled packages, if they define one
//! ([`RegistryBuilder::workflow_fields`](crate::core::registry::RegistryBuilder::workflow_fields)).
//! Each field keeps its origin, so that a problem with it is reported there.
//!
//! The settings a package gives a workflow are values of an origin that names
//! the workflow ([`CascadeOrigin::workflow`]): such an origin may set only keys
//! that are set per render request ([`Bind::Render`]).
//!
//! # Switching a feature off
//!
//! A source may switch a feature off ([`Cascade::switch_off`]) or on
//! ([`Cascade::switch_on`]), and records the feature's keys apart
//! ([`Cascade::set`]), whichever way it switches it: a source may set keys of a
//! feature it switches off. The source of the highest rank that switches the
//! feature decides whether it is off, and two peers of which one switches it off
//! and the other on are a problem. So a higher source switches on a feature that
//! a package switched off. When the feature is on, its keys are combined over
//! every source by the rule above; when it is off, none of its keys is read,
//! whatever source set it, the source that switched it off included. A key
//! recorded with [`Cascade::set`] alone does not switch a feature on.
//!
//! # A feature that no package in use has
//!
//! Whatever a source writes for a feature that no package in use has, a switch
//! either way or any key, is accepted and read by nothing: a source may keep
//! the settings of a package that the document does not use at the moment.
//! Since the name may be misspelled, each source that writes such a feature
//! gives one warning, at the first origin recorded for it, returned whether or
//! not the reading finds problems ([`ReadSettings::warnings`],
//! [`ReadFailure::warnings`]). A source is told apart from the others by its
//! origins' rank, [`peer`](CascadeOrigin::peer) and
//! [`workflow`](CascadeOrigin::workflow): in a rank in which the value written
//! last wins, every origin is of one source. Such a feature has no entry among
//! the settings in effect ([`Cascade::in_effect`]).
//!
//! # Entries that another feature declares
//!
//! A feature may declare the schema of one entry of a map that a key of
//! another feature holds, at a place that the owner of the key opens
//! ([`EntryDecl`], [`SettingsDecl::open_entries`], [`SettingsDecl::entry_data`]).
//! The declaration of an entry is in force when its feature and the owner are
//! among the features of the packages in use and neither is switched off; the
//! cascade then combines and reads the owner's settings by the owner's
//! declaration combined with those of its entries in force
//! ([`declaration_in_force`]):
//!
//! - the marks of an entry's schema apply to the values written under the
//!   entry;
//! - a key written under an entry that the entry's schema does not declare is
//!   left out of the combined settings, with a warning at the origin of each
//!   write of it ([`WarningKind::UndeclaredKey`]);
//! - an entry that nothing declares is accepted as written and read by
//!   whatever reads the owner's key;
//! - two features that declare one entry, and a declaration of an entry at a
//!   place that the owner does not open, are problems of [`Cascade::read`]
//!   without an origin, which name the features' packages.
//!
//! # Reading the combined settings
//!
//! [`Cascade::read`] reads the combined data through each feature's settings
//! declaration, as the package factories list them, into the settings each
//! package is built from and the option values set on the document and on the
//! render request, with the warnings ([`ReadSettings`]), or, when it finds problems, those
//! problems with the warnings ([`ReadFailure`]); [`Cascade::parsing`] reads the parsing
//! settings ([`FlmParsing`]), combined key by key by the same rule. Each path
//! in a value that the feature marks as a path to a file ([`Mark::Path`]) is
//! first given by its origin the form its reader receives
//! ([`CascadeOrigin::resolve_path`]).
//!
//! Every feature of the packages in use that declares settings is read at each
//! reading, whether or not anything is written under it. A feature nothing is
//! written for is read from the empty map, so that its package receives the
//! configuration its declaration reads from it ([`SettingsDecl::read`]): the
//! type's default for a configuration declared with a Rust type, and the
//! reader's answer for one declared with a reader
//! ([`SettingsDecl::config_data_read`]). A problem found then has no origin.
//!
//! [`Cascade::in_effect`] returns the combined data that [`Cascade::read`]
//! reads, before any declaration reads it: for each feature written, whether
//! it is switched off and where, and the value in effect of each key written,
//! with where it was written ([`FeatureInEffect`]). A host uses it to show each
//! setting's value and its origin.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Map, Value};

use crate::core::registry::{PackageFactory, WorkflowDef};
use crate::core::settings::{EntryDecl, Mark, PackageSettings, SettingsDecl, texts_at};
use crate::lang::FlmParsing;
use crate::model::names::Name;
use crate::model::options::{Bind, OptionValues};

/// Where a value was written, as the host knows it: the host's own type, which
/// the cascade compares, clones and shows (its `Display` text is how a message
/// refers to it, "`a.yaml:2:1`", "the program's defaults").
///
/// Every origin of one rank answers [`peer`](CascadeOrigin::peer) alike: all
/// with a package's identity (a rank of packages, decided by the reach), or all
/// with `None` (a rank in which the value written last wins).
pub trait CascadeOrigin: Clone + fmt::Display + fmt::Debug + PartialEq {
    /// The rank of the source: a higher rank replaces what a lower one writes.
    fn rank(&self) -> u32;
    /// For a source that is a package: the package's identity (its factory
    /// name), by which the reach decides between two packages of one rank.
    /// `None` for a rank in which the value written last wins. Every origin of
    /// one rank answers alike.
    fn peer(&self) -> Option<&str>;
    /// For the settings a package gives a workflow: the workflow's name. Such a
    /// source may set only keys bound to the render request.
    fn workflow(&self) -> Option<&str>;
    /// The source of a higher rank in which a person can write a setting, as a
    /// message names it ("the document's front matter"); `None` when there is
    /// none. The message about packages that disagree suggests it.
    fn higher_source(&self) -> Option<&str>;
    /// A path in a value marked [`Mark::Path`], as its reader receives it,
    /// given the place where the origin was written; `Ok(None)` leaves the
    /// path as written.
    ///
    /// # Errors
    ///
    /// The text of a path that has no meaning where the origin was written,
    /// which the cascade reports as a problem at the origin.
    fn resolve_path(&self, path: &str) -> Result<Option<String>, String>;
}

/// Which packages each package *reaches*: for each package, by its factory's
/// name, the names of the factories of the packages it uses, directly or
/// through others ([`PackageCatalog::reach`](crate::core::registry::PackageCatalog::reach)).
/// A package wins over the packages it reaches; see the [module
/// documentation](self#the-rule-between-packages).
pub type Reach = BTreeMap<Name, BTreeSet<Name>>;

/// One setting's value, as JSON data, with where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingValue<O> {
    /// The value, as data.
    pub value: Value,
    /// Where it was written.
    pub origin: O,
}

/// A problem with the settings: the message, and where the value at fault was
/// written when that is known. Its `Display` text is
/// [`detail`](Problem::detail). [`Cascade::read`] returns the problems that are
/// errors in [`ReadFailure::errors`], and those that are warnings, each with its
/// kind ([`Warning`]), in [`ReadSettings::warnings`] or, beside errors, in
/// [`ReadFailure::warnings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem<O> {
    /// Where the value at fault was written; `None` when that is not known,
    /// as for a feature nothing is written for, read from the empty map.
    pub origin: Option<O>,
    /// The problem, as a text for a reader.
    pub detail: String,
}

/// A value that the settings accept and that may be a mistake, as
/// [`Cascade::read`] finds it: what kind of value it is, where it was written
/// and the message ([`ReadSettings::warnings`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning<O> {
    /// What the value is.
    pub kind: WarningKind,
    /// Where the value was written, and the message.
    pub problem: Problem<O>,
}

/// The kinds of [`Warning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarningKind {
    /// A source writes a feature that no package in use has (see the [module
    /// documentation](self#a-feature-that-no-package-in-use-has)): accepted and
    /// read by nothing. One warning per source, at its first origin.
    UnknownFeature,
    /// A source writes, under an entry in force that another feature declares
    /// ([`EntryDecl`]), a key that the entry does not declare: left out of the
    /// combined settings. One warning per write of the key, at its origin.
    UndeclaredKey,
}

impl<O> fmt::Display for Problem<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl<O: fmt::Debug> std::error::Error for Problem<O> {}

/// Whether the source `winner` wins over `other`, a source of the same rank, by
/// the packages they are written in: `winner`'s package reaches `other`'s, and
/// `other`'s does not reach `winner`'s (two packages that reach each other are
/// peers).
fn wins_over<O: CascadeOrigin>(reach: &Reach, winner: &O, other: &O) -> bool {
    let (Some(winner), Some(other)) = (winner.peer(), other.peer()) else { return false };
    let reaches = |from: &str, to: &str| reach.get(from).is_some_and(|reached| reached.contains(to));
    winner != other && reaches(winner, other) && !reaches(other, winner)
}

/// The resulting value for one key or one switch, and the values that its peers
/// wrote that differ from it.
#[derive(Debug, Clone)]
struct Written<T, O> {
    value: T,
    origin: O,
    disagreeing: Vec<(T, O)>,
}

/// Returns the position in `writes` (every value each source wrote for one
/// setting, in the order written) of the resulting value by the rule of the
/// ranks, with the positions of the values its peers wrote that differ from
/// it. The sources of the highest rank that writes the setting decide. Within
/// a rank without peers, the value written last results. Within a rank of
/// peers, the sources that count are those over which no other wins
/// ([`wins_over`], by `reach`); the first of them written results, and the
/// others that differ from it are disagreeing. Which sources count does not
/// depend on the order written.
// Peers must agree, and a package changes another's value only by naming it
// in its `use` list: a package added beside the others never silently
// changes a value, and each value in force comes from one source.
fn decide_at<T: PartialEq, O: CascadeOrigin>(writes: &[(&T, &O)], reach: &Reach) -> Option<(usize, Vec<usize>)> {
    let top = writes.iter().map(|(_, origin)| origin.rank()).max()?;
    let at_top: Vec<usize> = (0..writes.len()).filter(|at| writes[*at].1.rank() == top).collect();
    if !at_top.iter().any(|at| writes[*at].1.peer().is_some()) {
        return Some((*at_top.last()?, Vec::new()));
    }
    let mut counting =
        at_top.iter().copied().filter(|at| !at_top.iter().any(|other| wins_over(reach, writes[*other].1, writes[*at].1)));
    let first = counting.next()?;
    let disagreeing = counting.filter(|at| writes[*at].0 != writes[first].0).collect();
    Some((first, disagreeing))
}

impl<T: PartialEq + Clone, O: CascadeOrigin> Written<T, O> {
    /// Returns the resulting value among `writes`, every value each source wrote
    /// for one setting, in the order written, by the rule of the ranks
    /// ([`decide_at`]), with the values of its peers that differ from it.
    fn decide<'w>(writes: impl IntoIterator<Item = (&'w T, &'w O)>, reach: &Reach) -> Option<Written<T, O>>
    where
        T: 'w,
        O: 'w,
    {
        let writes: Vec<(&T, &O)> = writes.into_iter().collect();
        let (first, disagreeing) = decide_at(&writes, reach)?;
        let (value, origin) = writes[first];
        let disagreeing = disagreeing.into_iter().map(|at| (writes[at].0.clone(), writes[at].1.clone())).collect();
        Some(Written { value: value.clone(), origin: origin.clone(), disagreeing })
    }

    /// Returns the problem of peers that disagree, if they do: `what` identifies
    /// the setting, `show` writes one of its values, and `higher_writes` says
    /// whether the higher source the resulting origin names
    /// ([`CascadeOrigin::higher_source`]) can write the setting, and so decide
    /// it.
    fn disagreement(&self, what: &str, show: impl Fn(&T) -> String, higher_writes: bool) -> Option<Problem<O>> {
        if self.disagreeing.is_empty() {
            return None;
        }
        let others: Vec<String> =
            self.disagreeing.iter().map(|(value, origin)| format!("{} at {origin}", show(value))).collect();
        let settle = match self.origin.higher_source() {
            Some(higher) if higher_writes => format!(
                "; make the values agree, make one of these packages use the other, or set the value in {higher}"
            ),
            _ => String::from("; make the values agree, or make one of these packages use the other"),
        };
        Some(Problem {
            origin: Some(self.origin.clone()),
            detail: format!(
                "the packages that set {what} must agree on its value when none of them uses another, directly \
                 or through other packages (the value of a package replaces those of the packages it uses), but {} \
                 is set at {} and {}{settle}",
                show(&self.value),
                self.origin,
                others.join(" and ")
            ),
        })
    }
}

/// The settings of one feature: every value each source wrote for each key, in
/// the order written, combined when they are read (a value combined key by key
/// is so by the marks of the feature's declaration alone).
#[derive(Debug, Clone)]
struct FeatureValues<O> {
    keys: BTreeMap<String, Vec<SettingValue<O>>>,
    /// Whether the feature is off, as each source that switches it off (`true`
    /// here) or on (`false`) says, in the order recorded.
    off: Vec<(bool, O)>,
    /// The origin of every key and every switch recorded for the feature, in the
    /// order recorded.
    written: Vec<O>,
}

impl<O> Default for FeatureValues<O> {
    fn default() -> FeatureValues<O> {
        FeatureValues { keys: BTreeMap::new(), off: Vec::new(), written: Vec::new() }
    }
}

/// Whether the origins `a` and `b` are of one source: of one rank, one peer and
/// one workflow (see the [module
/// documentation](self#a-feature-that-no-package-in-use-has)).
fn same_source<O: CascadeOrigin>(a: &O, b: &O) -> bool {
    a.rank() == b.rank() && a.peer() == b.peer() && a.workflow() == b.workflow()
}

/// The settings written in every source, each value with its origin, combined
/// by the rule of the ranks when they are read. See the [module
/// documentation](self).
///
/// The host records every value written ([`set`](Cascade::set),
/// [`switch_off`](Cascade::switch_off), [`switch_on`](Cascade::switch_on),
/// [`set_parsing`](Cascade::set_parsing),
/// [`add_workflow_fields`](Cascade::add_workflow_fields)), in any order, then
/// reads the result with [`read`](Cascade::read),
/// [`parsing`](Cascade::parsing) and [`workflows`](Cascade::workflows).
#[derive(Debug, Clone)]
pub struct Cascade<O: CascadeOrigin> {
    features: BTreeMap<String, FeatureValues<O>>,
    parsing: BTreeMap<String, Vec<(Value, O)>>,
    /// The fields of workflows, by the workflow's name and then the field's; a
    /// workflow a package writes with settings alone is here with no field.
    workflow_fields: BTreeMap<String, BTreeMap<String, Vec<(Value, O)>>>,
    /// Which package reaches which, deciding between sources of one rank.
    reach: Reach,
}

impl<O: CascadeOrigin> Default for Cascade<O> {
    fn default() -> Cascade<O> {
        Cascade {
            features: BTreeMap::new(),
            parsing: BTreeMap::new(),
            workflow_fields: BTreeMap::new(),
            reach: Reach::new(),
        }
    }
}

/// The settings once read through the features' declarations, as
/// [`Cascade::read`] returns them, with the warnings found while reading them.
#[derive(Debug, Clone)]
pub struct ReadSettings<O> {
    /// The settings each package is built from, by the package's name.
    pub packages: BTreeMap<Name, PackageSettings>,
    /// The option values to set on the document.
    pub document: OptionValues,
    /// The option values to set on the render request (the plan request's
    /// render options).
    pub render: OptionValues,
    /// The values accepted that may be mistakes, each with its kind and its
    /// origin, in the order found: one for each source that writes a feature
    /// that no package in use has (see the [module
    /// documentation](self#a-feature-that-no-package-in-use-has)), and one for
    /// each write of a key that an entry in force does not declare (see the
    /// [module documentation](self#entries-that-another-feature-declares)).
    /// The host reports each of them as a warning.
    pub warnings: Vec<Warning<O>>,
}

/// The error of [`Cascade::read`] when the settings have problems: the problems,
/// and the warnings found while reading them, which the host reports with the
/// problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFailure<O> {
    /// The problems, each at the origin of the value at fault; a problem
    /// found in reading a feature that nothing is written for has no origin.
    pub errors: Vec<Problem<O>>,
    /// The values accepted that may be mistakes, as in [`ReadSettings::warnings`].
    pub warnings: Vec<Warning<O>>,
}

impl<O> Default for ReadSettings<O> {
    fn default() -> ReadSettings<O> {
        ReadSettings {
            packages: BTreeMap::new(),
            document: OptionValues::default(),
            render: OptionValues::default(),
            warnings: Vec::new(),
        }
    }
}

/// The settings of one feature in effect, as [`Cascade::in_effect`] returns
/// them: whether a source switched the feature off, and the value in effect of
/// each key that a source wrote, each with where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureInEffect<O> {
    /// Where the feature was switched off, when the source that decides
    /// whether it is off switches it off (see the [module
    /// documentation](self#switching-a-feature-off)). None of the keys of a
    /// feature switched off is in effect, so [`keys`](FeatureInEffect::keys) is
    /// then empty.
    pub switched_off: Option<O>,
    /// The value in effect of each key that a source wrote, by the key's name.
    /// A key that no source wrote is absent.
    pub keys: BTreeMap<String, KeyInEffect<O>>,
}

/// The value in effect of one key of a feature: the value combined from every
/// source that wrote the key, by the rule of the ranks, with where it was
/// written.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyInEffect<O> {
    /// The value, as data, with every path that the feature marks as a path to
    /// a file in the form its origin gives it ([`CascadeOrigin::resolve_path`]). For a key whose value is
    /// combined key by key, the feature's own default combined with every
    /// source's value, one key at a time, at every depth that the marks name
    /// (see the [module documentation](self#values-combined-key-by-key)).
    pub value: Value,
    /// Where the value was written. For a key whose value is combined key by
    /// key, the origin of the highest rank among those that wrote the key (of
    /// several of that rank, the one recorded last); each key of the value has
    /// its own origin in [`entry_origins`](KeyInEffect::entry_origins).
    pub origin: O,
    /// For a key whose value is combined key by key: where each key of the
    /// value that a source wrote was written, by its name (the origin of the
    /// write that gives its value). A key of the feature's own default that no
    /// source wrote has no origin here; a key that is left out of
    /// [`value`](KeyInEffect::value) (its combined value is `false` in an
    /// object marked [`Mark::RemoveKeyWhenValueIsFalse`], or `null` where
    /// [`Mark::SetToDefaultWithNull`] stands) has one. Empty for any other
    /// key.
    pub entry_origins: BTreeMap<String, O>,
    /// For each key of the value that is itself combined key by key, the
    /// origins of the values below it, in the same form, at every depth: the
    /// levels below [`entry_origins`](KeyInEffect::entry_origins). A key
    /// whose value no source writes below it is absent.
    pub below: BTreeMap<String, EntryOrigins<O>>,
}

/// Where the keys of a value combined key by key were written, one level of
/// the origins that [`KeyInEffect`] gives below a key.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryOrigins<O> {
    /// Where each key of the value that a source wrote was written, by its
    /// name, as [`KeyInEffect::entry_origins`] gives them for a key.
    pub entry_origins: BTreeMap<String, O>,
    /// For each key of the value that is itself combined key by key, the
    /// origins of the values below it.
    pub below: BTreeMap<String, EntryOrigins<O>>,
}

impl<O> Default for EntryOrigins<O> {
    fn default() -> EntryOrigins<O> {
        EntryOrigins { entry_origins: BTreeMap::new(), below: BTreeMap::new() }
    }
}

impl<O> EntryOrigins<O> {
    /// Whether no origin is recorded at any depth.
    fn is_empty(&self) -> bool {
        self.entry_origins.is_empty() && self.below.is_empty()
    }
}

impl<O> KeyInEffect<O> {
    /// Returns where the value at `path` below the key was written (`path`
    /// holds the names of the keys, outermost first): the origin of the write
    /// that gives that value, or, for a value that no source wrote at that
    /// place (a value of the feature's own default) or a place inside a value
    /// decided whole, the origin of the closest value above it.
    pub fn origin_at(&self, path: &[&str]) -> &O {
        let mut origin = &self.origin;
        let (mut origins, mut below) = (&self.entry_origins, &self.below);
        for name in path {
            match origins.get(*name) {
                Some(at) => origin = at,
                None => break,
            }
            match below.get(*name) {
                Some(level) => (origins, below) = (&level.entry_origins, &level.below),
                None => break,
            }
        }
        origin
    }
}

/// One value that a source wrote at a place of the settings, as
/// [`Cascade::writes_at`] lists them: the value at that place, as written, where
/// it was written, and whether it is the value in effect there.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteAt<O> {
    /// The value the write holds at the place, as written (a path marked
    /// [`Mark::Path`] as written too).
    pub value: Value,
    /// Where the value was written.
    pub origin: O,
    /// Whether this write gives the value in effect at the place, by the rule
    /// of the ranks.
    pub in_effect: bool,
}

/// The place decided whole that holds a place of the settings, as
/// [`Cascade::decided_at`] finds it: how deep it lies, and what gives its
/// value.
#[derive(Debug, Clone, PartialEq)]
pub struct DecidedAt<O> {
    /// The number of names below the key that lead to the place (`0` for the
    /// key itself).
    pub depth: usize,
    /// The origin of the write that gives the value there, by the rule of the
    /// ranks; `None` when no source writes there, and the feature's own
    /// default gives it.
    pub origin: Option<O>,
}

/// Returns the values `writes` of one key, in the order written, combined whole
/// by the rule of the ranks, `reach` deciding between packages: the resulting
/// value, and those its peers wrote that differ from it.
fn whole<O: CascadeOrigin>(writes: &[SettingValue<O>], reach: &Reach) -> Option<Written<Value, O>> {
    Written::decide(writes.iter().map(|write| (&write.value, &write.origin)), reach)
}

/// Whether the path `pattern` of a mark ([`KeyMark`](crate::core::settings::KeyMark),
/// `*` naming every key and every item) names the place `path`.
pub(super) fn names(pattern: &str, path: &[String]) -> bool {
    let segments: Vec<&str> = pattern.split('.').collect();
    segments.len() == path.len() && segments.iter().zip(path).all(|(segment, name)| *segment == "*" || segment == name)
}

/// The values the sources wrote at one place, each with its origin, in the
/// order written.
type Writes<'w, O> = Vec<(&'w Value, &'w O)>;

/// A value combined from the sources' values at one place: the value, where it
/// was written (for a value combined key by key, the origin of the highest
/// rank among those written there) and the origins below it.
struct Combined<O> {
    value: Value,
    origin: O,
    below: EntryOrigins<O>,
}

/// Combines the sources' values of the settings of one feature by the marks
/// of its declaration, and by the entries in force in it
/// ([`SettingsDecl::entry_in_force`]).
struct Combiner<'c> {
    feature: &'c str,
    decl: &'c SettingsDecl,
    reach: &'c Reach,
}

/// What combining finds beside the combined values: the problems, and the
/// warnings (a key written under an entry in force that the entry does not
/// declare).
struct Found<O> {
    problems: Vec<Problem<O>>,
    warnings: Vec<Warning<O>>,
}

impl<O> Default for Found<O> {
    fn default() -> Found<O> {
        Found { problems: Vec::new(), warnings: Vec::new() }
    }
}

impl Combiner<'_> {
    /// Returns the place `path` as messages name it: the feature, then the
    /// names joined by `.`.
    fn dotted(&self, path: &[String]) -> String {
        format!("{}.{}", self.feature, path.join("."))
    }

    /// Returns the values `writes` at the place `path` combined by the rule of
    /// the ranks, `default` being the feature's own value there, or `None` when
    /// no source writes the place. Where `path` carries
    /// [`Mark::MergeObjectKeyValues`], the values are combined key by key, each
    /// key by this rule one level down, over `default` (or the default entries
    /// of the place's node) as the lowest source; elsewhere the value is
    /// decided whole. Under an entry in force, a key that the entry does not
    /// declare is left out, with a warning at the origin of each write of it.
    /// The problems found (a value that is not an object where it is combined
    /// key by key, peers that disagree) and the warnings are added to `found`.
    // A value decided whole is never merged below, so that it comes from one
    // source, which a problem can name.
    fn combine<O: CascadeOrigin>(
        &self,
        path: &mut Vec<String>,
        writes: Writes<'_, O>,
        default: Option<&Value>,
        found: &mut Found<O>,
    ) -> Option<Combined<O>> {
        let entry = self.decl.entry_in_force(&path.join("."));
        if !self.decl.is_marked(path, Mark::MergeObjectKeyValues) {
            let written = Written::decide(writes, self.reach)?;
            let what = match path.split_last() {
                Some((name, parent)) if !parent.is_empty() => {
                    format!("the entry `{name}` of `{}`", self.dotted(parent))
                }
                _ => format!("`{}`", self.dotted(path)),
            };
            found.problems.extend(written.disagreement(&what, shown, true));
            let mut value = written.value;
            if let (Some(entry), Value::Object(map)) = (entry, &mut value) {
                map.retain(|name, _| {
                    let declared = entry.keys().contains(name);
                    if !declared {
                        found.warnings.push(self.undeclared(path, name, &written.origin));
                    }
                    declared
                });
            }
            self.leave_out(path, &mut value);
            return Some(Combined { value, origin: written.origin, below: EntryOrigins::default() });
        }
        let mut sorted = writes;
        sorted.sort_by_key(|(_, origin)| origin.rank());
        let origin = sorted.last()?.1.clone();
        let defaults = match default {
            Some(Value::Object(entries)) => Some(entries),
            _ => self
                .decl
                .marks()
                .iter()
                .filter(|marks| marks.has(Mark::MergeObjectKeyValues) && names(&marks.path, path))
                .find_map(|marks| self.decl.default_entries(&marks.path)),
        };
        // Each key: its name, the feature's default, and what the sources wrote.
        let mut entries: Vec<(String, Option<&Value>, Writes<'_, O>)> = defaults
            .into_iter()
            .flatten()
            .map(|(name, value)| (name.clone(), Some(value), Vec::new()))
            .collect();
        for (value, origin) in &sorted {
            let Value::Object(map) = value else {
                found.problems.push(Problem {
                    origin: Some((*origin).clone()),
                    detail: format!(
                        "the value of `{}` must be a map from each entry's name to the entry",
                        self.dotted(path)
                    ),
                });
                continue;
            };
            for (name, value) in map {
                if entry.is_some_and(|entry| !entry.keys().contains(name)) {
                    found.warnings.push(self.undeclared(path, name, origin));
                    continue;
                }
                let at = match entries.iter().position(|(entry, _, _)| entry == name) {
                    Some(at) => at,
                    None => {
                        entries.push((name.clone(), None, Vec::new()));
                        entries.len() - 1
                    }
                };
                entries[at].2.push((value, *origin));
            }
        }
        let remove_false = self.decl.is_marked(path, Mark::RemoveKeyWhenValueIsFalse);
        let mut value = Map::new();
        let mut below = EntryOrigins::default();
        for (name, default, writes) in entries {
            path.push(name.clone());
            let in_force = match self.combine(path, writes, default, found) {
                Some(combined) => {
                    below.entry_origins.insert(name.clone(), combined.origin);
                    if !combined.below.is_empty() {
                        below.below.insert(name.clone(), combined.below);
                    }
                    combined.value
                }
                None => {
                    let mut value = default.cloned().unwrap_or(Value::Null);
                    self.leave_out(path, &mut value);
                    value
                }
            };
            let to_default = in_force.is_null() && self.decl.is_marked(path, Mark::SetToDefaultWithNull);
            path.pop();
            if !to_default && !(remove_false && in_force == Value::Bool(false)) {
                value.insert(name, in_force);
            }
        }
        Some(Combined { value: Value::Object(value), origin, below })
    }

    /// The warning for the key `name`, written at `origin` under the entry in
    /// force at `path`, which does not declare it.
    fn undeclared<O: CascadeOrigin>(&self, path: &[String], name: &str, origin: &O) -> Warning<O> {
        let detail = format!("Unknown setting ‘{name}’ in ‘{}’", self.dotted(path));
        Warning { kind: WarningKind::UndeclaredKey, problem: Problem { origin: Some(origin.clone()), detail } }
    }

    /// Leaves out, inside `value` (the value at the place `path`, decided
    /// whole), each key whose value is `false` in an object marked
    /// [`Mark::RemoveKeyWhenValueIsFalse`], and each value `null` marked
    /// [`Mark::SetToDefaultWithNull`].
    fn leave_out(&self, path: &mut Vec<String>, value: &mut Value) {
        match value {
            Value::Object(map) => {
                let remove_false = self.decl.is_marked(path, Mark::RemoveKeyWhenValueIsFalse);
                map.retain(|name, item| {
                    path.push(name.clone());
                    self.leave_out(path, item);
                    let to_default = item.is_null() && self.decl.is_marked(path, Mark::SetToDefaultWithNull);
                    path.pop();
                    !to_default && !(remove_false && *item == Value::Bool(false))
                });
            }
            Value::Array(items) => {
                for (at, item) in items.iter_mut().enumerate() {
                    path.push(at.to_string());
                    self.leave_out(path, item);
                    path.pop();
                }
            }
            _ => {}
        }
    }
}

/// Rewrites, in `combined` (the value of the key `key` of the feature that
/// `decl` declares), every path at a place that `decl` marks as a path to a
/// file ([`Mark::Path`]), as the origin of the value resolves it
/// ([`CascadeOrigin::resolve_path`]), and adds to `problems` each path that
/// the origin refuses, at the origin. A value combined key by key is resolved
/// by the origin of each of its keys; the values of the feature's own default,
/// written in no source, are left unchanged.
// Paths are resolved here, where the origin of each value is known, so that one
// rule serves every feature and a path written in a file is relative to the
// file's folder.
fn resolve_paths<O: CascadeOrigin>(
    decl: &SettingsDecl,
    key: &str,
    combined: &mut KeyInEffect<O>,
    problems: &mut Vec<Problem<O>>,
) {
    for mark in decl.marks().iter().filter(|mark| mark.has(Mark::Path)) {
        let mut segments = mark.path.split('.');
        if segments.next() != Some(key) {
            continue;
        }
        let below: Vec<&str> = segments.collect();
        let mut path = vec![key.to_string()];
        let origins = (&combined.entry_origins, &combined.below);
        resolve_in(decl, &mut path, &mut combined.value, &below, &combined.origin, origins, problems);
    }
}

/// Resolves, in `value` (the value at the place `path`, written at `origin`),
/// every text at the place that `segments` designates below it, as
/// [`resolve_at`] does; where `path` is combined key by key, each key by its
/// own origin among `origins` (and the origins below it), a key that no source
/// wrote left unchanged; adds to `problems` each path refused.
fn resolve_in<O: CascadeOrigin>(
    decl: &SettingsDecl,
    path: &mut Vec<String>,
    value: &mut Value,
    segments: &[&str],
    origin: &O,
    origins: (&BTreeMap<String, O>, &BTreeMap<String, EntryOrigins<O>>),
    problems: &mut Vec<Problem<O>>,
) {
    if !decl.is_marked(path, Mark::MergeObjectKeyValues) {
        resolve_at(value, segments, origin, problems);
        return;
    }
    let Value::Object(map) = value else { return };
    let none = EntryOrigins::default();
    for (name, entry) in map.iter_mut() {
        let Some(entry_origin) = origins.0.get(name) else { continue };
        let rest = match segments.split_first() {
            None => segments,
            Some((first, rest)) if *first == "*" || first == name => rest,
            Some(_) => continue,
        };
        let below = origins.1.get(name).unwrap_or(&none);
        path.push(name.clone());
        resolve_in(decl, path, entry, rest, entry_origin, (&below.entry_origins, &below.below), problems);
        path.pop();
    }
}

/// Resolves, in `value`, every text at the place that `segments` designates
/// below it ([`texts_at`]), as written at `origin`. A text that the origin
/// refuses is left as written, and its problem added to `problems`.
fn resolve_at<O: CascadeOrigin>(value: &mut Value, segments: &[&str], origin: &O, problems: &mut Vec<Problem<O>>) {
    texts_at(value, segments, &mut |text| match origin.resolve_path(text) {
        Ok(Some(resolved)) => *text = resolved,
        Ok(None) => {}
        Err(detail) => problems.push(Problem { origin: Some(origin.clone()), detail }),
    });
}

/// Returns the value at `path` inside `value` (`path` holds the names of the
/// keys, outermost first, and the positions of the items of a list), if
/// `value` holds one there.
fn value_at<'v>(value: &'v Value, path: &[&str]) -> Option<&'v Value> {
    path.iter().try_fold(value, |value, name| match value {
        Value::Object(map) => map.get(*name),
        Value::Array(items) => items.get(name.parse::<usize>().ok()?),
        _ => None,
    })
}

/// Returns the factory among `factories` that holds the feature `feature`, if
/// one does.
fn holder<'f>(factories: &[&'f PackageFactory], feature: &str) -> Option<&'f PackageFactory> {
    factories.iter().copied().find(|factory| factory.features().iter().any(|entry| entry.name == feature))
}

/// Returns the settings declaration of the feature `feature` among
/// `factories` (`None` when no factory holds it or it takes no settings),
/// combined with the declarations of its entries in force
/// ([`SettingsDecl::with_entries`]): those that the features of `factories`
/// declare, each feature for which `off` answers `false`.
///
/// This is the declaration by which the cascade combines and reads the
/// feature's settings, `off` answering whether the settings switch a feature
/// off; a host that describes the settings of the packages in use (a JSON
/// Schema of a front matter) gives `off` that answers `false` for every
/// feature.
pub fn declaration_in_force<'f>(
    factories: &[&'f PackageFactory],
    feature: &str,
    off: impl Fn(&str) -> bool,
) -> Option<Cow<'f, SettingsDecl>> {
    entries_in_force(factories, feature, off, &mut Vec::new())
}

/// The declaration of [`declaration_in_force`], adding to `problems` the
/// faults of the declarations of entries: an entry that two features declare
/// (the problem names the entry by its label, [`EntryDecl::label`], and both
/// features by their packages), and an entry at a place that the owner does
/// not open ([`SettingsDecl::open_entries`]).
fn entries_in_force<'f>(
    factories: &[&'f PackageFactory],
    feature: &str,
    off: impl Fn(&str) -> bool,
    problems: &mut Vec<String>,
) -> Option<Cow<'f, SettingsDecl>> {
    let owner = holder(factories, feature)?.settings(feature)?;
    let mut declared: Vec<(&Name, &str, &EntryDecl)> = Vec::new();
    for factory in factories {
        for entry in factory.features() {
            let Some(decl) = &entry.settings else { continue };
            if off(&entry.name) {
                continue;
            }
            for declaration in decl.entries().iter().filter(|declaration| declaration.owner().as_str() == feature) {
                let path = declaration.path();
                if !owner.opens(path) {
                    problems.push(format!(
                        "Misconfigured feature ‘{}’: the feature ‘{feature}’ does not allow other features to \
                         declare an entry at ‘{feature}.{path}’",
                        entry.name
                    ));
                    continue;
                }
                let label = declaration.label();
                match declared.iter().find(|(_, _, other)| other.path() == path) {
                    Some((package, other, _)) if *package == factory.name() => problems.push(format!(
                        "Duplicate {label} (features ‘{other}’ and ‘{}’ of the package ‘{package}’)",
                        entry.name
                    )),
                    Some((package, _, _)) => problems.push(format!(
                        "Duplicate {label} (packages ‘{package}’ and ‘{}’)",
                        factory.name()
                    )),
                    None => declared.push((factory.name(), entry.name.as_str(), declaration)),
                }
            }
        }
    }
    if declared.is_empty() {
        return Some(Cow::Borrowed(owner));
    }
    Some(Cow::Owned(owner.with_entries(declared.into_iter().map(|(_, _, declaration)| declaration))))
}

/// Returns a value as a message shows it: its one-line JSON form, between
/// backticks.
fn shown(value: &Value) -> String {
    format!("`{value}`")
}

impl<O: CascadeOrigin> Cascade<O> {
    /// Returns an empty cascade in which no package reaches another, so that
    /// every two packages are peers.
    pub fn new() -> Cascade<O> {
        Cascade::default()
    }

    /// Returns an empty cascade in which the packages reach one another as
    /// `reach` says: a package wins over the packages it reaches, for every
    /// setting both write. See the [module
    /// documentation](self#the-rule-between-packages).
    pub fn with_reach(reach: Reach) -> Cascade<O> {
        Cascade { reach, ..Cascade::default() }
    }

    /// Records `value` for the key `key` of the feature `feature`, written at
    /// `origin`. Every value recorded is kept; the rule of the ranks decides
    /// which one results when the settings are read, and
    /// [`read`](Cascade::read) reports peers that disagree. It does not switch
    /// the feature on.
    pub fn set(&mut self, feature: &str, key: &str, value: Value, origin: O) {
        let values = self.features.entry(feature.to_string()).or_default();
        values.written.push(origin.clone());
        values.keys.entry(key.to_string()).or_default().push(SettingValue { value, origin });
    }

    /// Records that `origin` switches the feature `feature` off; the keys its
    /// source sets for the feature are recorded apart, with [`Cascade::set`],
    /// and are not read while the feature is off. The rule of the ranks decides
    /// between the sources that switch the feature on or off.
    pub fn switch_off(&mut self, feature: &str, origin: O) {
        self.switch(feature, true, origin);
    }

    /// Records that `origin` switches the feature `feature` on; the keys its
    /// source sets for the feature are recorded apart, with [`Cascade::set`].
    /// The rule of the ranks decides between the sources that switch the
    /// feature on or off.
    pub fn switch_on(&mut self, feature: &str, origin: O) {
        self.switch(feature, false, origin);
    }

    /// Records that `origin` switches the feature `feature` off (`off`) or on.
    fn switch(&mut self, feature: &str, off: bool, origin: O) {
        let values = self.features.entry(feature.to_string()).or_default();
        values.written.push(origin.clone());
        values.off.push((off, origin));
    }

    /// Records `value` for the parsing setting `key` ([`FlmParsing`]), written
    /// at `origin`; the parsing settings are combined key by key by the rule of
    /// the ranks ([`parsing`](Cascade::parsing)).
    pub fn set_parsing(&mut self, key: &str, value: Value, origin: O) {
        self.parsing.entry(key.to_string()).or_default().push((value, origin));
    }

    /// Records `fields`, the fields that a package writes for the workflow
    /// `workflow`: each a field's name, its value and its origin. With no
    /// field, it records the workflow alone (a package that gives the workflow
    /// settings only). The rule of the ranks decides between the packages that
    /// write one field, and [`read`](Cascade::read) reports peers that
    /// disagree.
    pub fn add_workflow_fields(&mut self, workflow: &str, fields: impl IntoIterator<Item = (String, Value, O)>) {
        let written = self.workflow_fields.entry(workflow.to_string()).or_default();
        for (field, value, origin) in fields {
            written.entry(field).or_default().push((value, origin));
        }
    }

    /// Returns the resulting value of the key `key` of the feature `feature`,
    /// combined whole, with its origin, if a source wrote one. When peers
    /// disagree, it is the first written. A value marked as combined key by key
    /// is combined so only when the settings are [read](Cascade::read), which
    /// has the feature's declaration.
    pub fn value(&self, feature: &str, key: &str) -> Option<SettingValue<O>> {
        let written = whole(self.features.get(feature)?.keys.get(key)?, &self.reach)?;
        Some(SettingValue { value: written.value, origin: written.origin })
    }

    /// Returns where the feature `feature` was switched off, if the source that
    /// decides its switch switches it off.
    pub fn switched_off(&self, feature: &str) -> Option<O> {
        let off = self.decide(&self.features.get(feature)?.off)?;
        if off.value { Some(off.origin) } else { None }
    }

    /// Returns the resulting value of the field `field` of the workflow
    /// `workflow`, with its origin, if a package wrote one. When peers
    /// disagree, it is the first written.
    pub fn workflow_field(&self, workflow: &str, field: &str) -> Option<SettingValue<O>> {
        let writes = self.workflow_fields.get(workflow)?.get(field)?;
        let written = self.decide(writes)?;
        Some(SettingValue { value: written.value, origin: written.origin })
    }

    /// Returns every workflow that a package writes, with fields or with
    /// settings only, by its name: its resulting fields, each with its name,
    /// value and origin (none for a workflow written with settings only). The
    /// host gives these to the library, once per workflow ([`workflow_def`],
    /// then
    /// [`RegistryBuilder::workflow_fields`](crate::core::registry::RegistryBuilder::workflow_fields)),
    /// so they are the fields the workflow runs with.
    pub fn workflows(&self) -> BTreeMap<String, Vec<(String, SettingValue<O>)>> {
        self.workflow_fields
            .keys()
            .map(|workflow| {
                let fields = self.workflow_fields[workflow]
                    .keys()
                    .filter_map(|field| Some((field.clone(), self.workflow_field(workflow, field)?)))
                    .collect();
                (workflow.clone(), fields)
            })
            .collect()
    }

    /// The declaration of the feature `feature` among `factories` by which its
    /// settings are combined and read: with its entries in force, those that
    /// the features of `factories` that are not switched off declare
    /// ([`declaration_in_force`]). The faults of the declarations of entries
    /// are added to `problems`.
    fn declaration<'f>(
        &self,
        factories: &[&'f PackageFactory],
        feature: &str,
        problems: &mut Vec<String>,
    ) -> Option<Cow<'f, SettingsDecl>> {
        entries_in_force(factories, feature, |name| self.switched_off(name).is_some(), problems)
    }

    /// Returns the resulting value among `writes`, by the rule of the ranks and
    /// the packages' reach.
    fn decide<T: PartialEq + Clone>(&self, writes: &[(T, O)]) -> Option<Written<T, O>> {
        Written::decide(writes.iter().map(|(value, origin)| (value, origin)), &self.reach)
    }

    /// Returns the resulting parsing settings: each key as written by the
    /// source of the highest rank that writes it, the others at their
    /// defaults.
    ///
    /// # Errors
    ///
    /// A [`Problem`] for each key to which peers give different values, and one
    /// when the values cannot be read as the parsing settings' types.
    pub fn parsing(&self) -> Result<FlmParsing, Vec<Problem<O>>> {
        let mut problems = Vec::new();
        let mut data = Map::new();
        for (key, writes) in &self.parsing {
            let Some(written) = self.decide(writes) else { continue };
            problems.extend(written.disagreement(&format!("`parsing.{key}`"), shown, true));
            data.insert(key.clone(), written.value.clone());
        }
        if !problems.is_empty() {
            return Err(problems);
        }
        serde_json::from_value(Value::Object(data)).map_err(|error| {
            vec![Problem { origin: None, detail: format!("each parsing setting must hold a value of its type: {error}") }]
        })
    }

    /// Returns the resulting value of each parsing setting that a source wrote,
    /// by its key, with its origin: the value that
    /// [`parsing`](Cascade::parsing) reads for the key. When peers disagree, it
    /// is the first written. A key that no source wrote is absent.
    pub fn parsing_in_effect(&self) -> BTreeMap<String, SettingValue<O>> {
        self.parsing
            .iter()
            .filter_map(|(key, writes)| {
                let written = self.decide(writes)?;
                Some((key.clone(), SettingValue { value: written.value, origin: written.origin }))
            })
            .collect()
    }

    /// Returns the settings in effect for the package factories `factories`,
    /// by the feature's name: for each feature that a source wrote and that one
    /// of the factories holds, whether it is switched off and where, and the
    /// value in effect of each key written, combined from every source by the
    /// rule of the ranks, with where it was written ([`FeatureInEffect`]). A
    /// value marked as combined key by key is combined so, each key with its
    /// origin, at every depth (see the [module
    /// documentation](self#values-combined-key-by-key)), and relative paths are resolved (see the [module
    /// documentation](self#reading-the-combined-settings)). This is the data
    /// that [`read`](Cascade::read) reads through the features' settings
    /// declarations, combined by the same code.
    ///
    /// A host uses it to show each setting's value and where it was written.
    /// Nothing is checked: a key that the feature does not declare is listed
    /// like the others, and when peers disagree the value is the first written,
    /// as for [`value`](Cascade::value); [`read`](Cascade::read) reports these
    /// problems. A feature that no factory holds, and the keys of a feature
    /// that takes no settings, are left out.
    pub fn in_effect(&self, factories: &[&PackageFactory]) -> BTreeMap<String, FeatureInEffect<O>> {
        // The problems and the warnings are those `read` reports; they are not returned here.
        let mut found = Found::default();
        self.features
            .iter()
            .filter_map(|(feature, values)| {
                holder(factories, feature)?;
                let decl = self.declaration(factories, feature, &mut Vec::new());
                Some((feature.clone(), self.combine(feature, values, decl.as_deref(), &mut found)))
            })
            .collect()
    }

    /// Returns every value that a source wrote at the place `path` below the
    /// key `key` of the feature `feature` (`path` holds the names of the keys,
    /// outermost first; empty for the key itself), lowest rank first and, within
    /// a rank, in the order written: each write of the key that holds a value
    /// at that place, with that value as written, its origin, and whether it
    /// gives the value in effect there.
    ///
    /// The value in effect is decided as [`in_effect`](Cascade::in_effect)
    /// decides it, by the marks of `decl`, the declaration in force of the
    /// feature ([`declaration_in_force`], whose `off` answers whether this
    /// cascade switches a feature off; `None` for a feature that takes no
    /// settings or that no factory holds): down the path, through each place
    /// combined key by key, to the first place decided whole, where the write
    /// that the rule of the ranks chooses gives the value in effect when it
    /// holds a value at `path`. No write is in effect at a place combined key
    /// by key (its value is made of several writes), at a place that no source
    /// writes (where the feature's own default is in effect), nor in a feature
    /// switched off, without settings, or that no factory holds. When peers
    /// disagree, the first written is in effect, as for
    /// [`value`](Cascade::value). A host that lists the writes at several
    /// places of one feature computes the declaration once.
    pub fn writes_at(
        &self,
        decl: Option<&SettingsDecl>,
        feature: &str,
        key: &str,
        path: &[&str],
    ) -> Vec<WriteAt<O>> {
        let Some(writes) = self.features.get(feature).and_then(|values| values.keys.get(key)) else {
            return Vec::new();
        };
        let in_effect = match decl {
            Some(decl) if self.switched_off(feature).is_none() => self.write_in_effect(decl, key, path, writes),
            _ => None,
        };
        let mut found: Vec<(usize, Value)> = writes
            .iter()
            .enumerate()
            .filter_map(|(at, write)| Some((at, value_at(&write.value, path)?.clone())))
            .collect();
        found.sort_by_key(|(at, _)| writes[*at].origin.rank());
        found
            .into_iter()
            .map(|(at, value)| WriteAt { value, origin: writes[at].origin.clone(), in_effect: in_effect == Some(at) })
            .collect()
    }

    /// Returns the place decided whole that holds the place `path` below the
    /// key `key` of the feature `feature` (as for
    /// [`writes_at`](Cascade::writes_at), whose `decl` it takes too): the
    /// first place, down the path through each place combined key by key, that
    /// is not combined so, with the origin of the write that gives its value
    /// by the rule of the ranks, or no origin when no source writes there (the
    /// feature's own default then gives it, an entry of its default of the
    /// value combined key by key above it when the place lies below the key).
    /// The value at `path` lies at the place returned or inside it. There is
    /// none when `path` is itself combined key by key or lies in such a place
    /// along its names, and in a feature switched off, without settings, or
    /// that no factory holds. [`writes_at`](Cascade::writes_at) lists the
    /// writes there.
    pub fn decided_at(
        &self,
        decl: Option<&SettingsDecl>,
        feature: &str,
        key: &str,
        path: &[&str],
    ) -> Option<DecidedAt<O>> {
        let decl = decl.filter(|_| self.switched_off(feature).is_none())?;
        let writes = self.features.get(feature).and_then(|values| values.keys.get(key));
        let writes = writes.map(Vec::as_slice).unwrap_or_default();
        let (depth, decided) = self.decide_whole(decl, key, path, writes)?;
        Some(DecidedAt { depth, origin: decided.map(|(at, _)| writes[at].origin.clone()) })
    }

    /// Returns the position among `writes` (the writes of the key `key` of the
    /// feature that `decl` declares) of the write that gives the value in
    /// effect at `path` below the key, if one does (see
    /// [`writes_at`](Cascade::writes_at)).
    fn write_in_effect(
        &self,
        decl: &SettingsDecl,
        key: &str,
        path: &[&str],
        writes: &[SettingValue<O>],
    ) -> Option<usize> {
        let (depth, (at, value)) = match self.decide_whole(decl, key, path, writes)? {
            (depth, Some(decided)) => (depth, decided),
            (_, None) => return None,
        };
        value_at(value, &path[depth..]).map(|_| at)
    }

    /// Returns the place decided whole that holds `path` below the key `key`
    /// ([`decided_at`](Cascade::decided_at)), `writes` being the key's writes
    /// and `decl` the declaration in force: the number of names of `path` that
    /// lead to it, and the position among `writes` of the write that the rule
    /// of the ranks chooses there with its value at that place, if a write
    /// holds a value there.
    fn decide_whole<'w>(
        &self,
        decl: &SettingsDecl,
        key: &str,
        path: &[&str],
        writes: &'w [SettingValue<O>],
    ) -> Option<(usize, Option<(usize, &'w Value)>)> {
        let mut place = vec![key.to_string()];
        let mut level: Vec<(usize, &Value)> = writes.iter().enumerate().map(|(at, write)| (at, &write.value)).collect();
        while decl.is_marked(&place, Mark::MergeObjectKeyValues) {
            let name = path.get(place.len() - 1)?;
            level = level.into_iter().filter_map(|(at, value)| Some((at, value.as_object()?.get(*name)?))).collect();
            place.push(name.to_string());
        }
        let candidates: Vec<(&Value, &O)> = level.iter().map(|(at, value)| (*value, &writes[*at].origin)).collect();
        let decided = decide_at(&candidates, &self.reach).map(|(first, _)| level[first]);
        Some((place.len() - 1, decided))
    }

    /// Returns the settings in effect of the feature `feature`, from `values`,
    /// what the sources wrote for it, read by `decl`, its settings declaration
    /// (`None` for a feature that takes no settings, whose keys are then left
    /// out): whether it is switched off and where, and, when it is not, each
    /// key's value combined by the rule of the ranks, its paths resolved.
    /// Adds to `found` the problems found on the way (peers that disagree on
    /// whether the feature is off, on a key or on a value below one; a value
    /// that is not an object where it is combined key by key; a key that a
    /// workflow's settings set and that is not set per render request), and
    /// the warnings (a key written under an entry in force that the entry does
    /// not declare, left out).
    fn combine(
        &self,
        feature: &str,
        values: &FeatureValues<O>,
        decl: Option<&SettingsDecl>,
        found: &mut Found<O>,
    ) -> FeatureInEffect<O> {
        let mut in_effect = FeatureInEffect { switched_off: None, keys: BTreeMap::new() };
        if let Some(off) = self.decide(&values.off) {
            let state = |off: &bool| String::from(if *off { "`false`" } else { "settings" });
            found.problems.extend(off.disagreement(&format!("the feature `{feature}`"), state, true));
            // A feature switched off is absent: none of its keys is in effect, whatever
            // source wrote them.
            if off.value {
                in_effect.switched_off = Some(off.origin);
                return in_effect;
            }
        }
        let Some(decl) = decl else { return in_effect };
        for (key, writes) in &values.keys {
            for write in writes {
                if let (Some(workflow), Some(declared)) = (write.origin.workflow(), decl.key(key)) {
                    if declared.bind != Bind::Render {
                        found.problems.push(Problem {
                            origin: Some(write.origin.clone()),
                            detail: format!("the settings of the workflow `{workflow}` must not set `{feature}.{key}`: a workflow's settings may set only keys that are set per render request, and `{feature}.{key}` is not one"),
                        });
                    }
                }
            }
            let combiner = Combiner { feature, decl, reach: &self.reach };
            let mut path = vec![key.clone()];
            let written = writes.iter().map(|write| (&write.value, &write.origin)).collect();
            let Some(combined) = combiner.combine(&mut path, written, None, found) else { continue };
            // A setting whose value `null` gives it its default is left out, so that its
            // reader applies the default.
            if combined.value.is_null() && decl.is_marked(&path, Mark::SetToDefaultWithNull) {
                continue;
            }
            let mut combined = KeyInEffect {
                value: combined.value,
                origin: combined.origin,
                entry_origins: combined.below.entry_origins,
                below: combined.below.below,
            };
            resolve_paths(decl, key, &mut combined, &mut found.problems);
            in_effect.keys.insert(key.clone(), combined);
        }
        in_effect
    }

    /// Reads the combined settings through the settings declarations of the
    /// package factories `factories`: every feature of a factory that declares
    /// settings and is not switched off is read by its declaration, from the
    /// data written for it, or from the empty map when nothing is written under
    /// it (see [`SettingsDecl::read`] for the configuration read from the empty
    /// map). Path values are resolved first (see the [module
    /// documentation](self#reading-the-combined-settings)). The data read for
    /// a feature that a source wrote is what [`in_effect`](Cascade::in_effect)
    /// returns for it.
    ///
    /// Every factory gets an entry in [`ReadSettings::packages`], which holds
    /// the settings read of each of its features that declares settings,
    /// written or not; a feature switched off is recorded as such in its
    /// package's settings. What a source writes for a feature that no factory
    /// holds is read by nothing: each source that writes such a feature gives
    /// one warning ([`WarningKind::UnknownFeature`]), at the first origin
    /// recorded for it, and nothing else (see the [module
    /// documentation](self#a-feature-that-no-package-in-use-has)). A key
    /// written under an entry in force that the entry does not declare is left
    /// out of the combined settings, with one warning
    /// ([`WarningKind::UndeclaredKey`]) at the origin of each write of it (see
    /// the [module documentation](self#entries-that-another-feature-declares)).
    /// The warnings are in [`ReadSettings::warnings`], or in
    /// [`ReadFailure::warnings`] when the reading finds problems.
    ///
    /// # Errors
    ///
    /// A [`ReadFailure`] with every problem found, each at the origin of the
    /// value at fault, and the warnings found beside them. The problems: settings
    /// written for a feature that takes no settings; peers that disagree on
    /// a key, on a value below one, or on whether a feature is off; a value
    /// that is not an object where the feature's declaration combines it key
    /// by key ([`Mark::MergeObjectKeyValues`]); a key the feature does not
    /// declare, or a value its declaration cannot read; a key that a workflow's
    /// settings set and that is not set per render request; peers that disagree
    /// on a field of a workflow; an entry that two features in use declare, and
    /// a declaration of an entry at a place that the owner does not open
    /// ([`SettingsDecl::open_entries`]); a path in a value marked
    /// [`Mark::Path`] that its origin refuses
    /// ([`CascadeOrigin::resolve_path`]). A problem found in reading a
    /// feature nothing is written for (a configuration reader that refuses the
    /// empty map), and a fault of the declarations of entries, have no origin.
    pub fn read(&self, factories: &[&PackageFactory]) -> Result<ReadSettings<O>, ReadFailure<O>> {
        let mut read = ReadSettings::default();
        for factory in factories {
            read.packages.insert(factory.name().clone(), PackageSettings::new());
        }
        let mut problems = Vec::new();
        for (workflow, field, writes) in self
            .workflow_fields
            .iter()
            .flat_map(|(workflow, fields)| fields.iter().map(move |(field, writes)| (workflow, field, writes)))
        {
            let Some(written) = self.decide(writes) else { continue };
            let what = format!("the field `{field}` of the workflow `{workflow}`");
            problems.extend(written.disagreement(&what, shown, false));
        }
        // The features written, combined: each one not switched off whose declaration
        // reads it, with its settings in effect and the first origin written for it.
        let mut written_features: BTreeMap<&str, (FeatureInEffect<O>, Option<O>)> = BTreeMap::new();
        for (feature, values) in &self.features {
            let first_origin = values
                .keys
                .values()
                .find_map(|writes| writes.first())
                .map(|write| write.origin.clone())
                .or(values.off.first().map(|(_, origin)| origin.clone()));
            let Some(factory) = holder(factories, feature) else {
                // Accepted and read by nothing, with one warning for each source that writes
                // it, at the first origin of that source, since the name may be misspelled.
                let mut firsts: Vec<&O> = Vec::new();
                for origin in &values.written {
                    if !firsts.iter().any(|first| same_source(*first, origin)) {
                        firsts.push(origin);
                    }
                }
                read.warnings.extend(firsts.into_iter().map(|origin| Warning {
                    kind: WarningKind::UnknownFeature,
                    problem: Problem {
                        origin: Some(origin.clone()),
                        detail: format!("Unknown feature ‘{feature}’: no package in use has this feature"),
                    },
                }));
                continue;
            };
            let decl = self.declaration(factories, feature, &mut Vec::new()).filter(|decl| decl.takes_settings());
            let mut found = Found::default();
            let in_effect = self.combine(feature, values, decl.as_deref(), &mut found);
            problems.extend(found.problems);
            read.warnings.extend(found.warnings);
            if in_effect.switched_off.is_some() {
                read.packages.entry(factory.name().clone()).or_default().switch_off(feature.as_str());
            } else if decl.is_some() {
                written_features.insert(feature.as_str(), (in_effect, first_origin));
            } else if !values.keys.is_empty() {
                problems.push(Problem {
                    origin: first_origin,
                    detail: format!("the feature `{feature}` takes no settings, so nothing may be written under it"),
                });
            }
        }
        // Every feature in use that declares settings and is not switched off is read: from
        // its settings in effect, or from the empty map when nothing is written under it.
        for factory in factories {
            let package = read.packages.entry(factory.name().clone()).or_default();
            for entry in factory.features() {
                if entry.settings.is_none() || package.is_off(&entry.name) {
                    continue;
                }
                let mut declared = Vec::new();
                let Some(decl) = self.declaration(factories, &entry.name, &mut declared) else { continue };
                problems.extend(declared.into_iter().map(|detail| Problem { origin: None, detail }));
                let (in_effect, first_origin) = match written_features.get(entry.name.as_str()) {
                    Some((in_effect, first_origin)) => (Some(in_effect), first_origin.as_ref()),
                    None => (None, None),
                };
                let data: Map<String, Value> = in_effect
                    .iter()
                    .flat_map(|in_effect| &in_effect.keys)
                    .map(|(key, combined)| (key.clone(), combined.value.clone()))
                    .collect();
                match decl.read(&data) {
                    Ok(settings) => {
                        read.document.merge_from(settings.document_options());
                        read.render.merge_from(settings.render_options());
                        package.insert(settings);
                    }
                    Err(errors) => problems.extend(errors.into_iter().map(|error| {
                        // At the key or the entry at fault when it is written, else at the
                        // first value written for the feature; nowhere when nothing is.
                        let mut path = error.path.split('.').skip(1);
                        let at = path.next().and_then(|key| in_effect?.keys.get(key)).map(|combined| {
                            let below: Vec<&str> = path.collect();
                            combined.origin_at(&below).clone()
                        });
                        Problem { origin: at.or_else(|| first_origin.cloned()), detail: error.detail }
                    })),
                }
            }
        }
        if problems.is_empty() { Ok(read) } else { Err(ReadFailure { errors: problems, warnings: read.warnings }) }
    }
}

/// Returns the fields `fields` of the workflow `name` (each a field's name and
/// its value as a package writes it) as the library holds them: a
/// [`WorkflowDef`] with the fields given and the others unset. A field that the
/// library does not declare is ignored.
///
/// The host calls it with the resulting fields among every package that writes
/// the workflow ([`Cascade::workflows`]), and registers the result through
/// [`RegistryBuilder::workflow_fields`](crate::core::registry::RegistryBuilder::workflow_fields).
pub fn workflow_def<'v>(name: &str, fields: impl IntoIterator<Item = (&'v str, &'v Value)>) -> WorkflowDef {
    let mut def = WorkflowDef::new(name);
    let names = |value: &Value| -> Vec<String> {
        value.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
    };
    for (field, value) in fields {
        match field {
            "extends" => def.extends = value.as_str().map(Into::into),
            "outputs" => def.outputs = names(value).into_iter().map(Into::into).collect(),
            "demands" => def.demands = names(value).into_iter().map(Into::into).collect(),
            "prefer" => def.prefer = names(value).into_iter().map(Into::into).collect(),
            // Through the builder, so that the cargo feature `docs` decides for
            // a text read from data as for one given in Rust.
            "docs" => def = def.docs(value.as_str().unwrap_or_default()),
            _ => {}
        }
    }
    def
}
