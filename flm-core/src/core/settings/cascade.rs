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
//! # Keys merged entry by entry
//!
//! Some keys hold a collection of named things: a map from a name to an entry,
//! which the feature marks in its declaration
//! ([`SettingsDecl::merge_entries`]). Such a key is combined one entry at a
//! time, by the same rule, with the feature's own entries
//! ([`default_entries`](SettingsDecl::default_entries)) below every source. A
//! source therefore adds an entry and leaves the others; the source of the
//! highest rank that writes an entry replaces it whole (the fields it does not
//! write come from the feature's own entry of that name, never from a lower
//! source); an entry whose resulting value is `false` is removed; two peers that
//! write two different values for one entry are a problem for that entry. The
//! entries are ordered by source, lowest rank first, each source's entries in
//! the order written, and a replaced entry keeps its position. Nothing else is
//! combined inside a value.
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
//! A source writes, for a feature, either `false` ([`Cascade::switch_off`]) or a
//! map of its settings, an empty map included ([`Cascade::switch_on`]). The
//! source of the highest rank that wrote either decides whether the feature is
//! off, and two peers of which one wrote `false` and the other a map are a
//! problem. So a higher source switches on, with an empty map, a feature that a
//! package switched off. When the feature is on, its keys are combined over
//! every source by the rule above; when it is off, none of its keys is read,
//! whatever source wrote it. A key recorded with [`Cascade::set`] alone does not
//! switch a feature on.
//!
//! `false` for a feature that no package in use has is accepted; since the name
//! may be misspelled, each `false` written for such a feature is a warning at its
//! origin, returned whether or not the reading finds problems
//! ([`ReadSettings::warnings`], [`ReadFailure::warnings`]). Settings written for
//! such a feature, an empty map included, are a problem.
//!
//! # Reading the combined settings
//!
//! [`Cascade::read`] reads the combined data through each feature's settings
//! declaration, as the package factories list them, into the settings each
//! package is built from and the option values set on the document and on the
//! render request, with the warnings ([`ReadSettings`]), or, when it finds problems, those
//! problems with the warnings ([`ReadFailure`]); [`Cascade::parsing`] reads the parsing
//! settings ([`FlmParsing`]), combined key by key by the same rule. A relative
//! path in a value that the feature marks as a path to a file
//! ([`ValueKind::Path`]) is first resolved by its origin
//! ([`CascadeOrigin::resolve_path`]).
//!
//! [`Cascade::in_effect`] returns the combined data that [`Cascade::read`]
//! reads, before any declaration reads it: for each feature, whether it is
//! switched off and where, and the value in effect of each key written, with
//! where it was written ([`FeatureInEffect`]). A host uses it to show each
//! setting's value and its origin.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Map, Value};

use crate::core::registry::{PackageFactory, WorkflowDef};
use crate::core::settings::{PackageSettings, SettingsDecl, ValueKind};
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
    /// A relative path in a value marked [`ValueKind::Path`], resolved against
    /// where the origin was written; `None` leaves the path as written.
    fn resolve_path(&self, path: &str) -> Option<String>;
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
/// errors in [`ReadFailure::errors`], and those that are warnings in
/// [`ReadSettings::warnings`] or, beside errors, in [`ReadFailure::warnings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem<O> {
    /// Where the value at fault was written.
    pub origin: Option<O>,
    /// The problem, as a text for a reader.
    pub detail: String,
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

impl<T: PartialEq + Clone, O: CascadeOrigin> Written<T, O> {
    /// Returns the resulting value among `writes`, every value each source wrote
    /// for one setting, in the order written, by the rule of the ranks: the
    /// sources of the highest rank that writes the setting decide. Within a rank
    /// without peers, the value written last results. Within a rank of peers,
    /// the sources that count are those over which no other wins
    /// ([`wins_over`], by `reach`); the first of them written results, and the
    /// others that differ from it are disagreeing. Which sources count does not
    /// depend on the order written.
    // See decision rationale at [§flm-dd-dr:feat-merge-rule]
    fn decide<'w>(writes: impl IntoIterator<Item = (&'w T, &'w O)>, reach: &Reach) -> Option<Written<T, O>>
    where
        T: 'w,
        O: 'w,
    {
        let writes: Vec<(&T, &O)> = writes.into_iter().collect();
        let top = writes.iter().map(|(_, origin)| origin.rank()).max()?;
        let at_top: Vec<(&T, &O)> = writes.into_iter().filter(|(_, origin)| origin.rank() == top).collect();
        if !at_top.iter().any(|(_, origin)| origin.peer().is_some()) {
            let (value, origin) = at_top.last()?;
            return Some(Written { value: (*value).clone(), origin: (*origin).clone(), disagreeing: Vec::new() });
        }
        let mut counting =
            at_top.iter().filter(|(_, origin)| !at_top.iter().any(|(_, other)| wins_over(reach, *other, *origin)));
        let (value, origin) = counting.next()?;
        let disagreeing = counting
            .filter(|(other, _)| *other != *value)
            .map(|(other, at)| ((*other).clone(), (*at).clone()))
            .collect();
        Some(Written { value: (*value).clone(), origin: (*origin).clone(), disagreeing })
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
/// the order written, combined when they are read (a key merged entry by entry
/// is combined one entry at a time, which only the feature's declaration says).
#[derive(Debug, Clone)]
struct FeatureValues<O> {
    keys: BTreeMap<String, Vec<SettingValue<O>>>,
    /// Whether the feature is off, as each source that wrote `false` (`true`
    /// here) or a map (`false`) for it says, in the order written.
    off: Vec<(bool, O)>,
}

impl<O> Default for FeatureValues<O> {
    fn default() -> FeatureValues<O> {
        FeatureValues { keys: BTreeMap::new(), off: Vec::new() }
    }
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
    /// The values accepted that may be mistakes, each at its origin, in the
    /// order found: one for each `false` written for a feature that no package
    /// in use has (see the [module documentation](self#switching-a-feature-off)).
    /// The host reports each of them as a warning.
    pub warnings: Vec<Problem<O>>,
}

/// The error of [`Cascade::read`] when the settings have problems: the problems,
/// and the warnings found while reading them, which the host reports with the
/// problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFailure<O> {
    /// The problems, each at the origin of the value at fault.
    pub errors: Vec<Problem<O>>,
    /// The values accepted that may be mistakes, as in [`ReadSettings::warnings`].
    pub warnings: Vec<Problem<O>>,
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
    /// whether it is off wrote `false` (see the [module
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
    /// The value, as data, with every relative path that the feature marks as
    /// a path to a file resolved by its origin. For a key merged entry by
    /// entry, the feature's own entries combined with every source's, one entry
    /// at a time (see the [module
    /// documentation](self#keys-merged-entry-by-entry)).
    pub value: Value,
    /// Where the value was written. For a key merged entry by entry, the
    /// origin of the highest rank among those that wrote the key (of several
    /// of that rank, the one recorded last); each entry has its own origin in
    /// [`entry_origins`](KeyInEffect::entry_origins).
    pub origin: O,
    /// For a key merged entry by entry: where each entry that a source wrote
    /// was written, by the entry's name. An entry of the feature's own that no
    /// source wrote has no origin here; an entry that a source removed (its
    /// resulting value is `false`) has one, and is absent from
    /// [`value`](KeyInEffect::value). Empty for any other key.
    pub entry_origins: BTreeMap<String, O>,
}

/// Returns the values `writes` of one key, in the order written, combined whole
/// by the rule of the ranks, `reach` deciding between packages: the resulting
/// value, and those its peers wrote that differ from it.
fn whole<O: CascadeOrigin>(writes: &[SettingValue<O>], reach: &Reach) -> Option<Written<Value, O>> {
    Written::decide(writes.iter().map(|write| (&write.value, &write.origin)), reach)
}

/// The values the sources wrote for one entry, each with its origin, in the
/// order written.
type EntryWrites<'w, O> = Vec<(&'w Value, &'w O)>;

/// Returns the values `writes` of the key at `path` (`FEATURE.KEY`), which is
/// merged entry by entry, combined one entry at a time over `defaults`, the
/// feature's own entries: each entry by the rule of the ranks, `reach` deciding
/// between packages; the entries in the order of the sources, lowest first, the
/// defaults first, a replaced entry keeping its position; and an entry whose
/// resulting value is `false` left out. The problems found (a value that is not
/// a map, peers that disagree on an entry) are added to `problems`.
// See decision rationale at [§flm-dd-dr:feat-merge-rule]
fn by_entries<O: CascadeOrigin>(
    path: &str,
    writes: &[SettingValue<O>],
    defaults: Map<String, Value>,
    reach: &Reach,
    problems: &mut Vec<Problem<O>>,
) -> Option<KeyInEffect<O>> {
    let mut sorted: Vec<&SettingValue<O>> = writes.iter().collect();
    sorted.sort_by_key(|write| write.origin.rank());
    // Each entry: its name, the feature's default, and what the sources wrote.
    let mut entries: Vec<(String, Option<Value>, EntryWrites<'_, O>)> =
        defaults.into_iter().map(|(name, value)| (name, Some(value), Vec::new())).collect();
    for write in &sorted {
        let Value::Object(map) = &write.value else {
            problems.push(Problem {
                origin: Some(write.origin.clone()),
                detail: format!("the value of `{path}` must be a map from each entry's name to the entry"),
            });
            continue;
        };
        for (name, value) in map {
            let at = match entries.iter().position(|(entry, _, _)| entry == name) {
                Some(at) => at,
                None => {
                    entries.push((name.clone(), None, Vec::new()));
                    entries.len() - 1
                }
            };
            entries[at].2.push((value, &write.origin));
        }
    }
    let origin = sorted.last()?.origin.clone();
    let mut value = Map::new();
    let mut origins = BTreeMap::new();
    for (name, default, writes) in entries {
        let in_force = match Written::decide(writes, reach) {
            Some(written) => {
                problems.extend(written.disagreement(&format!("the entry `{name}` of `{path}`"), shown, true));
                origins.insert(name.clone(), written.origin);
                written.value
            }
            None => default.unwrap_or(Value::Null),
        };
        if in_force != Value::Bool(false) {
            value.insert(name, in_force);
        }
    }
    Some(KeyInEffect { value: Value::Object(value), origin, entry_origins: origins })
}

/// Rewrites, in `combined` (the value of the key `key` of the feature that
/// `decl` declares), every relative path at a place that `decl` marks as a path
/// to a file ([`ValueKind::Path`]), as the origin of the value resolves it
/// ([`CascadeOrigin::resolve_path`]). An entry of a key merged entry by entry
/// is resolved by its own origin; the feature's own entries, written in no
/// source, are left unchanged.
// See decision rationale at [§flm-dd-dr:host-settings-path-resolve]
fn resolve_paths<O: CascadeOrigin>(decl: &SettingsDecl, key: &str, combined: &mut KeyInEffect<O>) {
    let merged = decl.default_entries(key).is_some();
    for mark in decl.marks().iter().filter(|mark| mark.kind == ValueKind::Path) {
        let mut segments = mark.path.split('.');
        if segments.next() != Some(key) {
            continue;
        }
        let below: Vec<&str> = segments.collect();
        if !merged {
            resolve_at(&mut combined.value, &below, &combined.origin);
            continue;
        }
        let Value::Object(entries) = &mut combined.value else { continue };
        for (name, entry) in entries.iter_mut() {
            let (Some(origin), rest) = (combined.entry_origins.get(name), below.as_slice()) else { continue };
            match rest.split_first() {
                None => resolve_at(entry, &[], origin),
                Some((first, rest)) if *first == "*" || first == name => resolve_at(entry, rest, origin),
                Some(_) => {}
            }
        }
    }
}

/// Resolves, in `value`, every text at the place that `segments` designates
/// below it (`*` matching every entry of a map and every item of a list), as
/// written at `origin`; when the value at that place is a list or a map, every
/// text inside it.
fn resolve_at<O: CascadeOrigin>(value: &mut Value, segments: &[&str], origin: &O) {
    match (segments.split_first(), value) {
        (None, Value::String(text)) => {
            if let Some(resolved) = origin.resolve_path(text) {
                *text = resolved;
            }
        }
        (None, Value::Array(items)) => items.iter_mut().for_each(|item| resolve_at(item, &[], origin)),
        (None, Value::Object(map)) => map.values_mut().for_each(|item| resolve_at(item, &[], origin)),
        (Some((&"*", rest)), Value::Array(items)) => items.iter_mut().for_each(|item| resolve_at(item, rest, origin)),
        (Some((&"*", rest)), Value::Object(map)) => map.values_mut().for_each(|item| resolve_at(item, rest, origin)),
        (Some((name, rest)), Value::Object(map)) => {
            if let Some(item) = map.get_mut(*name) {
                resolve_at(item, rest, origin);
            }
        }
        _ => {}
    }
}

/// Returns the factory among `factories` that holds the feature `feature`, if
/// one does.
fn holder<'f>(factories: &[&'f PackageFactory], feature: &str) -> Option<&'f PackageFactory> {
    factories.iter().copied().find(|factory| factory.features().iter().any(|entry| entry.name == feature))
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
        values.keys.entry(key.to_string()).or_default().push(SettingValue { value, origin });
    }

    /// Records that `origin` switches the feature `feature` off (its source
    /// wrote `false`); the rule of the ranks decides between the sources that
    /// switch the feature on or off.
    pub fn switch_off(&mut self, feature: &str, origin: O) {
        self.features.entry(feature.to_string()).or_default().off.push((true, origin));
    }

    /// Records that `origin` switches the feature `feature` on (its source
    /// wrote a map of the feature's settings); the rule of the ranks decides
    /// between the sources that switch the feature on or off.
    pub fn switch_on(&mut self, feature: &str, origin: O) {
        self.features.entry(feature.to_string()).or_default().off.push((false, origin));
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
    /// disagree, it is the first written. A key merged entry by entry is
    /// combined one entry at a time only when the settings are
    /// [read](Cascade::read), which has the feature's declaration.
    pub fn value(&self, feature: &str, key: &str) -> Option<SettingValue<O>> {
        let written = whole(self.features.get(feature)?.keys.get(key)?, &self.reach)?;
        Some(SettingValue { value: written.value, origin: written.origin })
    }

    /// Returns where the feature `feature` was switched off, if the source that
    /// decides its switch wrote `false`.
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

    /// Returns the settings in effect for the package factories `factories`,
    /// by the feature's name: for each feature that a source wrote and that one
    /// of the factories holds, whether it is switched off and where, and the
    /// value in effect of each key written, combined from every source by the
    /// rule of the ranks, with where it was written ([`FeatureInEffect`]). A
    /// key merged entry by entry is combined one entry at a time, each entry
    /// with its origin, and relative paths are resolved (see the [module
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
        // The problems are those `read` reports; they are not returned here.
        let mut problems = Vec::new();
        self.features
            .iter()
            .filter_map(|(feature, values)| {
                let factory = holder(factories, feature)?;
                Some((feature.clone(), self.combine(feature, values, factory.settings(feature), &mut problems)))
            })
            .collect()
    }

    /// Returns the settings in effect of the feature `feature`, from `values`,
    /// what the sources wrote for it, read by `decl`, its settings declaration
    /// (`None` for a feature that takes no settings, whose keys are then left
    /// out): whether it is switched off and where, and, when it is not, each
    /// key's value combined by the rule of the ranks, its paths resolved.
    /// Adds to `problems` those found on the way: peers that disagree on
    /// whether the feature is off, on a key or on an entry; a key merged entry
    /// by entry whose value is not a map; a key that a workflow's settings set
    /// and that is not set per render request.
    fn combine(
        &self,
        feature: &str,
        values: &FeatureValues<O>,
        decl: Option<&SettingsDecl>,
        problems: &mut Vec<Problem<O>>,
    ) -> FeatureInEffect<O> {
        let mut in_effect = FeatureInEffect { switched_off: None, keys: BTreeMap::new() };
        if let Some(off) = self.decide(&values.off) {
            let state = |off: &bool| String::from(if *off { "`false`" } else { "settings" });
            problems.extend(off.disagreement(&format!("the feature `{feature}`"), state, true));
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
                        problems.push(Problem {
                            origin: Some(write.origin.clone()),
                            detail: format!("the settings of the workflow `{workflow}` must not set `{feature}.{key}`: a workflow's settings may set only keys that are set per render request, and `{feature}.{key}` is not one"),
                        });
                    }
                }
            }
            let combined = match decl.default_entries(key) {
                None => whole(writes, &self.reach).map(|written| {
                    problems.extend(written.disagreement(&format!("`{feature}.{key}`"), shown, true));
                    KeyInEffect { value: written.value, origin: written.origin, entry_origins: BTreeMap::new() }
                }),
                Some(defaults) => {
                    by_entries(&format!("{feature}.{key}"), writes, defaults.clone(), &self.reach, problems)
                }
            };
            if let Some(mut combined) = combined {
                resolve_paths(decl, key, &mut combined);
                in_effect.keys.insert(key.clone(), combined);
            }
        }
        in_effect
    }

    /// Reads the combined settings through the settings declarations of the
    /// package factories `factories`: for each feature written, the declaration
    /// of the factory that holds the feature reads its data. Path values are
    /// resolved first (see the [module
    /// documentation](self#reading-the-combined-settings)). The data read is
    /// what [`in_effect`](Cascade::in_effect) returns.
    ///
    /// Every factory gets an entry in [`ReadSettings::packages`], empty when
    /// nothing is written for its features; a feature switched off is recorded
    /// as such in its package's settings. Each `false` written for a feature
    /// that no factory holds is a warning at its origin, and nothing else: in
    /// [`ReadSettings::warnings`], or in [`ReadFailure::warnings`] when the
    /// reading finds problems.
    ///
    /// # Errors
    ///
    /// A [`ReadFailure`] with every problem found, each at the origin of the
    /// value at fault, and the warnings found beside them. The problems: settings
    /// written for a feature that no factory holds, or for a feature that takes
    /// no settings; peers that disagree on
    /// a key or on whether a feature is off; a key the feature does not
    /// declare, or a value its declaration cannot read; a key that a workflow's
    /// settings set and that is not set per render request; peers that disagree
    /// on a field of a workflow.
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
        for (feature, values) in &self.features {
            let first_origin = values
                .keys
                .values()
                .find_map(|writes| writes.first())
                .map(|write| write.origin.clone())
                .or(values.off.first().map(|(_, origin)| origin.clone()));
            let Some(factory) = holder(factories, feature) else {
                // `false` alone is accepted, with a warning for each `false` written;
                // any settings are a problem, at the first written.
                // See decision rationale at [§flm-dd-dr:false-for-a-feature-not-loaded].
                for (off, origin) in &values.off {
                    if *off {
                        read.warnings.push(Problem {
                            origin: Some(origin.clone()),
                            detail: format!("`{feature}` is switched off, but no package in use has a feature `{feature}`"),
                        });
                    }
                }
                let settings_origin = values
                    .keys
                    .values()
                    .find_map(|writes| writes.first())
                    .map(|write| write.origin.clone())
                    .or(values.off.iter().find(|(off, _)| !off).map(|(_, origin)| origin.clone()));
                if let Some(origin) = settings_origin {
                    problems.push(Problem {
                        origin: Some(origin),
                        detail: format!("`{feature}` must be the name of a feature of a package the document uses"),
                    });
                }
                continue;
            };
            let package = read.packages.entry(factory.name().clone()).or_default();
            let decl = factory.settings(feature);
            let in_effect = self.combine(feature, values, decl, &mut problems);
            if in_effect.switched_off.is_some() {
                package.switch_off(feature.as_str());
                continue;
            }
            if values.keys.is_empty() {
                continue;
            }
            let Some(decl) = decl else {
                problems.push(Problem {
                    origin: first_origin,
                    detail: format!("the feature `{feature}` takes no settings, so nothing may be written under it"),
                });
                continue;
            };
            let data: Map<String, Value> =
                in_effect.keys.iter().map(|(key, combined)| (key.clone(), combined.value.clone())).collect();
            match decl.read(&data) {
                Ok(settings) => {
                    read.document.merge_from(settings.document_options());
                    read.render.merge_from(settings.render_options());
                    package.insert(settings);
                }
                Err(errors) => problems.extend(errors.into_iter().map(|error| {
                    let mut path = error.path.split('.').skip(1);
                    let at = path.next().and_then(|key| in_effect.keys.get(key)).map(|combined| {
                        let entry = path.next().and_then(|entry| combined.entry_origins.get(entry));
                        entry.unwrap_or(&combined.origin).clone()
                    });
                    Problem { origin: at.or(first_origin.clone()), detail: error.detail }
                })),
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
