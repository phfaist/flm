//! The document's settings for the live preview: the settings state that the tools page
//! shows, and the document's file, read and written.
//!
//! The preview's server answers the requests of a page that shows the document's settings
//! and lets the author change them, at the addresses [`super::server::SETTINGS`],
//! [`super::server::SETTINGS_YAML`], [`super::server::SETTINGS_SOURCES`] and
//! [`super::server::FRONT_MATTER`]. It answers them
//! through a [`DocumentSettings`], which holds two parts:
//!
//! - what the preparations of the document give, computed once per build that prepares it
//!   again: the JSON Schema of a front matter for the packages the document uses
//!   ([`crate::seed::front_matter_schema`]); the values in effect, each with its origin; and
//!   the choices that depend on the document's environment (the compose templates, the
//!   packages found, the math processors, the build types). Before the first preparation,
//!   the schema is the one the settings were created with, and the other two are empty
//!   (but for what a preparation that failed gives, below);
//! - the document's file, read as it is at the moment of each request: the front matter's
//!   *revision* (a digest of its text) and what it writes, as data; or, when the front
//!   matter cannot be read, the reason, on one line.
//!
//! **After a preparation that failed.** A preparation that succeeded gives all three
//! ([`DocumentSettings::set_prepared`]). One that failed gives none, and the settings state
//! keeps each part as current as the document allows ([`DocumentSettings::set_failed`]):
//! when the document's packages can still be found
//! ([`crate::session::document_packages`]), the schema, the packages found, the packages in
//! use and the math processors are those of these packages; the values in effect, and the
//! choices that need the registry (the compose templates, the outputs, the build types),
//! stay those of the last preparation that succeeded (none, before the first: the choices
//! then hold the packages found, the packages in use and the math processors only). When the packages cannot be found either,
//! all of it stays that of the last preparation that succeeded. The settings state then
//! says why the preparation failed, and whether its schema is that of the document as it
//! is.
//!
//! [`DocumentSettings::state`] joins the two into the *settings state*, a JSON object whose
//! fields the watch client's protocol describes (`PROTOCOL.md`, "Reading the settings
//! state"). It holds what the settings are and where each comes from, and nothing of how a
//! page presents them. [`DocumentSettings::yaml_at`] gives the text that an editor of the
//! value at one path of the front matter starts from.
//!
//! **Origins.** Each value in effect carries its origin: the settings source that gives it
//! (the program's defaults, a package, the front matter, a workflow, the command line) and
//! the program's text for it, which a page shows as it is. One function of this module
//! composes it, for the features' keys ([`Origin`]) and for the keys of `flm` itself
//! ([`KeyOrigin`]) alike.
//!
//! Each value in effect carries the origin of the write that gives it at every depth: a key
//! combined key by key gives the origin of each key of its value, and so on below.
//! [`DocumentSettings::sources_at`] gives, for one path, every source's value there, which
//! one is in effect, and what is in effect as a result.
//!
//! A write ([`DocumentSettings::write`]) makes the edits of one request in the front matter,
//! together, in the file on the disk, when the page saw the front matter's revision as it
//! is; the watch loop sees that change of the file and builds the document again, as after
//! any other save of the file. A value set inside a value that the settings decide whole
//! comes with the other values of that value, copied into the front matter, so that the
//! write changes no other value in effect; a single name of `compose.template` is written
//! out as the name of each output. Such values are added only from a last preparation that
//! succeeded: otherwise a write that needs them is refused.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use flm_core::core::settings::cascade::{EntryOrigins, declaration_in_force};
use flm_core::core::settings::{ENABLED_KEY, Mark, SettingsDecl};
use flm_core::model::digest::digest_of;
use flm_core::stdlib::features::compose::{self, TEMPLATES_SLOT, Template};
use saphyr::{Scalar, ScalarStyle, Yaml, YamlEmitter};
use serde_json::{Map, Value, json};

use super::front_matter::{Change, Edit, KeyPath, edit_front_matter};
use crate::files::Files;
use crate::output::OutputError;
use crate::seed::{MathChoice, RESERVED_KEY, SeedError, read_front_matter, split};
use crate::session::{DocumentPackages, KeyOrigin, PrepareError, Prepared, Problem};
use crate::settings::{Origin, Rank, Settings, Source};

/// The function that writes a file's bytes: [`crate::output::write_file`] for the disk.
type Writer = Box<dyn Fn(&str, &[u8]) -> Result<(), OutputError> + Send + Sync>;

/// What the settings state takes from the preparations: the fields `schema`, `in_effect`,
/// `choices`, `preparation_failed` and `schema_current`, as data.
struct FromPreparation {
    schema: Value,
    in_effect: Value,
    choices: Value,
    /// Why the last preparation failed, one line per problem; `null` after one that
    /// succeeded.
    failed: Value,
    /// Whether `schema` is that of the document's packages as the last build found them.
    schema_current: bool,
    /// The combined settings of the last preparation that succeeded, for the sources of a
    /// value and the copies of a write; `None` before the first.
    cascaded: Option<Arc<Cascaded>>,
}

impl FromPreparation {
    /// Whether this is what a last preparation that succeeded gives: not after one that
    /// failed, nor before the first.
    fn succeeded(&self) -> bool {
        self.failed.is_null() && self.cascaded.is_some()
    }
}

/// The combined settings of a preparation, kept to answer the sources of a value and the
/// copies of a write: every value that each source wrote, and the declaration in force of
/// each feature of the packages in use ([`declaration_in_force`]), computed once for every
/// request that the preparation answers.
struct Cascaded {
    settings: Settings,
    /// The declaration in force of each feature that a package in use holds; `None` for a
    /// feature that takes no settings.
    declarations: BTreeMap<String, Option<SettingsDecl>>,
}

impl Cascaded {
    /// The combined settings of `prepared`, with the declarations in force of its features.
    fn of(prepared: &Prepared) -> Cascaded {
        let settings = prepared.settings().clone();
        let factories: Vec<_> = prepared.factories().iter().collect();
        let off = |name: &str| settings.switched_off(name).is_some();
        let features: BTreeSet<String> =
            factories.iter().flat_map(|factory| factory.features()).map(|feature| feature.name.to_string()).collect();
        let declarations = features
            .into_iter()
            .map(|feature| {
                let decl = declaration_in_force(&factories, &feature, off).map(Cow::into_owned);
                (feature, decl)
            })
            .collect();
        Cascaded { settings, declarations }
    }

    /// Every write of the key `key` of the feature `feature` that holds a value at `below`,
    /// the names below the key ([`Cascade::writes_at`](flm_core::core::settings::cascade::Cascade::writes_at)):
    /// lowest rank first, each with whether it gives the value in effect there.
    fn writes_at(&self, feature: &str, key: &str, below: &[String]) -> Vec<WriteAt> {
        let below: Vec<&str> = below.iter().map(String::as_str).collect();
        self.settings.writes_at(self.declaration(feature), feature, key, &below)
    }

    /// The place decided whole that holds the place `below`, the names below the key `key`
    /// of the feature `feature`, with what gives its value
    /// ([`Cascade::decided_at`](flm_core::core::settings::cascade::Cascade::decided_at)).
    fn decided_at(&self, feature: &str, key: &str, below: &[String]) -> Option<DecidedAt> {
        let below: Vec<&str> = below.iter().map(String::as_str).collect();
        self.settings.decided_at(self.declaration(feature), feature, key, &below)
    }

    /// The declaration in force of the feature `feature`; `None` for a feature that takes
    /// no settings, or that no package in use holds.
    fn declaration(&self, feature: &str) -> Option<&SettingsDecl> {
        self.declarations.get(feature).and_then(Option::as_ref)
    }

    /// Whether the feature `feature` combines the value at `place` (its key, then the names
    /// below it) key by key ([`Mark::MergeObjectKeyValues`]).
    fn combined_key_by_key(&self, feature: &str, place: &[String]) -> bool {
        self.declaration(feature).is_some_and(|decl| decl.is_marked(place, Mark::MergeObjectKeyValues))
    }
}

/// One write of a value at a place of the settings, as the cascade lists them.
type WriteAt = flm_core::core::settings::cascade::WriteAt<Origin>;

/// The place decided whole that holds a place of the settings, as the cascade finds it.
type DecidedAt = flm_core::core::settings::cascade::DecidedAt<Origin>;

/// A value that a write of the front matter copies into it, so that the value set at a path
/// inside a value that the settings decide whole leaves every other value of it as it was
/// ([`DocumentSettings::write`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Copied {
    /// The path of the value copied, the keys from the root of the front matter.
    pub path: Vec<String>,
    /// The value copied: the value in effect at the path before the write, a path to a file
    /// written with its root.
    pub value: Value,
}

/// What the preparations gave: the last that succeeded, and what the settings state shows.
struct Preparations {
    /// What the last preparation that succeeded gave, or the settings' first state before it.
    succeeded: Arc<FromPreparation>,
    /// What the settings state shows: `succeeded` after a preparation that succeeded; after
    /// one that failed, `succeeded` with what the document's packages give in place of
    /// its parts that follow from them.
    shown: Arc<FromPreparation>,
}

/// What the preview's server needs to answer the requests of the document's settings: what
/// the preparations give, and the document's file with the functions that
/// read and write it. See the [module documentation](self).
///
/// Create it with [`DocumentSettings::on_disk`] for a file on the disk, or with
/// [`DocumentSettings::new`] to read and write through functions of the caller's.
pub struct DocumentSettings {
    prepared: Mutex<Preparations>,
    path: String,
    files: Arc<dyn Files>,
    write: Writer,
}

impl fmt::Debug for DocumentSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentSettings").field("path", &self.path).finish_non_exhaustive()
    }
}

/// The error of the requests of [`DocumentSettings`]: why the document's settings could not
/// be read or written. Its text (`Display`) is the message the server sends to the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// The document's file cannot be read, or cannot be written; the server answers with
    /// the status `500`.
    File(String),
    /// The file's front matter cannot be read (it is not YAML, or does not have the shape
    /// of a front matter), or the edits cannot be written into it
    /// ([`super::front_matter::edit_front_matter`]). Nothing was written. The server
    /// answers with the status `409`. The text begins with the file's path and, when known,
    /// the place in the file.
    FrontMatter(String),
    /// The revision of the front matter that a write gives is not the file's. Nothing was
    /// written. The server answers with the status `412`.
    Stale(String),
    /// A write needs values copied into the front matter (or a single name of
    /// `compose.template` written out), and the last preparation did not succeed, or none
    /// has yet: the values in effect are not those of the document as it is. Nothing was
    /// written. The server answers with the status `409`.
    Unprepared(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::File(detail)
            | SettingsError::FrontMatter(detail)
            | SettingsError::Stale(detail)
            | SettingsError::Unprepared(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for SettingsError {}

impl DocumentSettings {
    /// Creates the settings of the document whose file is `path` (written as the host writes
    /// paths, [`crate::files`]), read through `files` and written through `write`, with
    /// `schema` the schema of a front matter ([`crate::seed::front_matter_schema`]) that the
    /// settings state gives until a preparation replaces it; until then, the state holds no
    /// values in effect and no choices, and its schema counts as current.
    pub fn new(
        schema: Value,
        path: impl Into<String>,
        files: Arc<dyn Files>,
        write: impl Fn(&str, &[u8]) -> Result<(), OutputError> + Send + Sync + 'static,
    ) -> DocumentSettings {
        let first = Arc::new(FromPreparation {
            schema,
            in_effect: Value::Null,
            choices: Value::Null,
            failed: Value::Null,
            schema_current: true,
            cascaded: None,
        });
        let prepared = Preparations { succeeded: Arc::clone(&first), shown: first };
        DocumentSettings { prepared: Mutex::new(prepared), path: path.into(), files, write: Box::new(write) }
    }

    /// Creates the settings of the document whose file on the disk is `path`, read through
    /// [`crate::files::DiskFiles`] and written through [`crate::output::write_file`].
    pub fn on_disk(schema: Value, path: impl Into<String>) -> DocumentSettings {
        DocumentSettings::new(schema, path, Arc::new(crate::files::DiskFiles), crate::output::write_file)
    }

    /// Replaces what the settings state takes from a preparation with what `prepared` gives:
    /// the schema of a front matter for its packages ([`Prepared::factories`]), suggesting
    /// the packages and extensions listed on the document's search path
    /// ([`Prepared::suggestions`]), as [`crate::seed::front_matter_schema`] makes it; the
    /// values in effect, with their origins ([`Prepared::settings_in_effect`] for the
    /// features' keys, [`Prepared::flm_in_effect`] and [`Prepared::parsing_in_effect`] for
    /// the keys of `flm` itself); and the choices that depend on the environment (the
    /// compose templates of the slot `flm.compose.templates` by output, the outputs of the
    /// build type with those they are made from, the packages found, the math processors,
    /// and the build types, [`Prepared::profiles`]). The requests that follow answer it, as
    /// that of a preparation that succeeded, with a schema that is current.
    pub fn set_prepared(&self, prepared: &Prepared) {
        let factories: Vec<_> = prepared.factories().iter().collect();
        let schema = crate::seed::front_matter_schema(&factories, prepared.suggestions());
        let in_effect = prepared.settings_in_effect();
        let cascaded = Cascaded::of(prepared);
        let state = Arc::new(FromPreparation {
            schema,
            in_effect: in_effect_record(prepared, &cascaded, in_effect),
            choices: choices_record(prepared),
            failed: Value::Null,
            schema_current: true,
            cascaded: Some(Arc::new(cascaded)),
        });
        *self.prepared.lock().unwrap_or_else(PoisonError::into_inner) =
            Preparations { succeeded: Arc::clone(&state), shown: state };
    }

    /// Records a preparation that failed with `error`, after which `packages` are the
    /// document's packages as [`crate::session::document_packages`] finds them, or `None`
    /// when they cannot be found either. The requests that follow answer the last
    /// preparation that succeeded, except that, with `packages`, the schema of a front
    /// matter is that of these packages ([`crate::seed::front_matter_schema`]), and so are
    /// the packages found, the packages in use and the math processors among the choices
    /// (the only choices, before the first preparation that succeeded); the settings state
    /// gives the reason, the text of each problem of `error` on one line, and whether its
    /// schema is current (it is with `packages`).
    pub fn set_failed(&self, error: &PrepareError, packages: Option<&DocumentPackages>) {
        let mut prepared = self.prepared.lock().unwrap_or_else(PoisonError::into_inner);
        let succeeded = &prepared.succeeded;
        let failed = Value::String(reason_of(error));
        let shown = match packages {
            Some(packages) => {
                let factories: Vec<_> = packages.factories.iter().collect();
                let mut choices = succeeded.choices.as_object().cloned().unwrap_or_default();
                choices.insert(String::from("packages"), packages_record(&packages.suggestions, &packages.descriptions));
                choices.insert(String::from("packages_in_use"), json!(packages.names_in_use));
                choices.insert(String::from("math_processors"), json!(crate::seed::math_processors(&packages.suggestions)));
                FromPreparation {
                    schema: crate::seed::front_matter_schema(&factories, &packages.suggestions),
                    in_effect: succeeded.in_effect.clone(),
                    choices: Value::Object(choices),
                    failed,
                    schema_current: true,
                    cascaded: succeeded.cascaded.clone(),
                }
            }
            None => FromPreparation {
                schema: succeeded.schema.clone(),
                in_effect: succeeded.in_effect.clone(),
                choices: succeeded.choices.clone(),
                failed,
                schema_current: false,
                cascaded: succeeded.cascaded.clone(),
            },
        };
        prepared.shown = Arc::new(shown);
    }

    /// Returns the path of the document's file.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Reads the file now and returns the settings state, a JSON object: `revision`, a
    /// digest of the front matter's text, and `front_matter`, what the front matter writes,
    /// as data (`null` for a file without a front matter); or, when the front matter cannot
    /// be read, `revision` and `front_matter` `null` and `unreadable` the reason, on one
    /// line, which begins with the file's path and the place; then `schema`, `in_effect` and
    /// `choices`, as [`DocumentSettings::set_prepared`] describes them (`null` for the last
    /// two before the first preparation), or as [`DocumentSettings::set_failed`] keeps them
    /// after a preparation that failed (then, before the first that succeeded, `choices`
    /// holds the packages found and the math processors only, when they can be found);
    /// `preparation_failed`, why the last preparation failed, one line per problem (`null`
    /// after one that succeeded, and before the first); and `schema_current`, whether the
    /// schema is that of the document's packages as the last build found them.
    /// `PROTOCOL.md` gives every field.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError::File`] when the file cannot be read.
    pub fn state(&self) -> Result<Value, SettingsError> {
        let text = self.text()?;
        let prepared = self.last_prepared();
        let (revision, front_matter, unreadable) = match front_matter_of(&text) {
            Ok((revision, data)) => (Value::String(revision), data, Value::Null),
            Err(error) => (Value::Null, Value::Null, Value::String(self.front_matter_error(&error).to_string())),
        };
        let mut state = Map::new();
        state.insert(String::from("revision"), revision);
        state.insert(String::from("unreadable"), unreadable);
        state.insert(String::from("front_matter"), front_matter);
        state.insert(String::from("schema"), prepared.schema.clone());
        state.insert(String::from("in_effect"), prepared.in_effect.clone());
        state.insert(String::from("choices"), prepared.choices.clone());
        state.insert(String::from("preparation_failed"), prepared.failed.clone());
        state.insert(String::from("schema_current"), Value::Bool(prepared.schema_current));
        Ok(Value::Object(state))
    }

    /// Reads the file now and returns, as YAML text, the value at `path` (the keys from the
    /// root of the front matter, an index for an entry of a list): the value the front matter
    /// writes there, else the value in effect, else the default that the schema gives; the
    /// empty text when there is none. The text ends with a line end, and reads back as the
    /// value: a text of several lines is written as a literal block where that block reads
    /// back as the text, and in double quotes otherwise.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the file cannot be read, or when its front matter
    /// cannot be read.
    pub fn yaml_at(&self, path: &[String]) -> Result<String, SettingsError> {
        let text = self.text()?;
        let (_, written) = front_matter_of(&text).map_err(|error| self.front_matter_error(&error))?;
        let prepared = self.last_prepared();
        let value = value_at(&written, path)
            .cloned()
            .or_else(|| in_effect_at(&prepared.in_effect, path))
            .or_else(|| default_at(&prepared.schema, path));
        Ok(value.map(|value| yaml_text(&value)).unwrap_or_default())
    }

    /// Reads the file now and returns what the settings give at `path` (the keys from the
    /// root of the front matter), a JSON object: `sources`, every value that a settings
    /// source writes at the path, lowest rank first, each `{"value", "origin", "in_effect"}`
    /// (whether it gives the value in effect there, as
    /// [`Cascade::writes_at`](flm_core::core::settings::cascade::Cascade::writes_at) decides
    /// it), `null` for a path that does not lie under a feature's key and before the first
    /// preparation; `value` and `origin`, the value in effect at the path and its origin, both
    /// left out when there is none, the origin alone for a value of the feature's own default
    /// and for a value that several keys make (`use`, `parsing`, a feature's entry);
    /// `default`, when there is no value in effect, the default that applies there (a compose
    /// template's, as the choices give it, else the schema's), left out when there is none or
    /// when the value in effect is left out of the settings (a `false` that removes a key, a
    /// key that a declaration lacks); and `write_copies`, the values that a write of a value
    /// at the path copies into the front matter, each `{"path", "value"}`
    /// ([`DocumentSettings::write`]; below `compose.template`, a single name gives the name of
    /// each other output that has a compose template), which the write refuses to copy
    /// unless the last preparation succeeded. `PROTOCOL.md` gives every field.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the file cannot be read, or when its front matter
    /// cannot be read.
    pub fn sources_at(&self, path: &[String]) -> Result<Value, SettingsError> {
        let text = self.text()?;
        let (_, written) = front_matter_of(&text).map_err(|error| self.front_matter_error(&error))?;
        let prepared = self.last_prepared();
        let writes = match (feature_key(path), &prepared.cascaded) {
            (Some((feature, key, below)), Some(cascaded)) => Some(cascaded.writes_at(feature, key, below)),
            _ => None,
        };
        let mut answer = Map::new();
        let sources = writes.as_ref().map(|writes| {
            let listed = writes.iter().map(|write| {
                json!({ "value": write.value, "origin": origin_record(&write.origin), "in_effect": write.in_effect })
            });
            Value::Array(listed.collect())
        });
        answer.insert(String::from("sources"), sources.unwrap_or(Value::Null));
        match value_in_effect(&prepared.in_effect, path) {
            Some((value, origin)) => {
                answer.insert(String::from("value"), value);
                if let Some(origin) = origin {
                    answer.insert(String::from("origin"), origin);
                }
            }
            None => {
                // A value in effect that the settings leave out: `null` gives the default,
                // any other value gives nothing.
                let left_out = writes.iter().flatten().any(|write| write.in_effect && !write.value.is_null());
                let applies = !left_out && !switched_off(&prepared.in_effect, path);
                if let Some(default) = applies.then(|| default_in_effect(&prepared, path)).flatten() {
                    answer.insert(String::from("default"), default);
                }
            }
        }
        let (before, after) = copies_of(&prepared, &written, path);
        let copies = before.iter().chain(&after).map(copied_record).collect();
        answer.insert(String::from("write_copies"), Value::Array(copies));
        Ok(Value::Object(answer))
    }

    /// Writes the file with the edits `edits` made in its front matter, in order, edited in
    /// place from the file's text as it is now ([`edit_front_matter`]): only the values that
    /// differ are rewritten, and every other byte stays. `revision` is the revision of the
    /// front matter that the edits were made against, as [`DocumentSettings::state`] gives
    /// it. The file is written once, with all the edits, or not at all; edits that leave the
    /// text as it is write nothing.
    ///
    /// **A write keeps every other value.** A value set at a path inside a value that the
    /// settings decide whole (a key that is not combined key by key, an entry of a value
    /// combined key by key, or a value below one) makes the front matter write that value,
    /// which replaces the whole value that a lower source (the feature's own default, the
    /// program's defaults, a package) gives, when the front matter writes nothing there. The
    /// write then copies into the front matter, in the same edit, every other value of the
    /// value in effect along the path, from the values in effect of the last preparation, when
    /// it succeeded (a path to a file with its root), each beside the value set in the order
    /// of its keys. A copy is left to an edit of `edits` when the copy's path is the path
    /// that the edit sets or removes, lies inside that path, or holds it. A copied value is
    /// the front matter's from then on.
    ///
    /// **A single name of `compose.template`** counts as the map of each output that has a
    /// compose template (the outputs of the choice `templates`) to that name, so that a value
    /// set at one output's name copies the name for each other output. When the front matter
    /// itself writes the single name, an edit below `compose.template` (a value set or a
    /// removal) is made whatever source is in effect: the same edit first replaces the single
    /// name with that map, the outputs that `edits` change as they change them (no name left
    /// removes the key), once per call; a call whose `edits` change `compose.template` itself,
    /// or a key that holds it, writes no such map. The names written are the front matter's
    /// from then on; an output that gains a compose template later takes the default
    /// template. Returns the values copied, the names written out included, in the order
    /// written.
    ///
    /// **Only after a preparation that succeeded.** The write adds such an edit (a value
    /// copied, the names written out) only when the last preparation succeeded: after one
    /// that failed, and before the first, a write that needs one is refused
    /// ([`SettingsError::Unprepared`]), since the values in effect are not those of the
    /// document as it is. A write whose edits are exactly `edits` is made in any state.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the file cannot be read or written, when the front
    /// matter cannot be read, when `revision` is not the front matter's
    /// ([`SettingsError::Stale`]), when the write needs an edit added and the last
    /// preparation did not succeed ([`SettingsError::Unprepared`]), and when an edit cannot
    /// be written into it; nothing is written then.
    pub fn write(&self, revision: &str, edits: &[Edit]) -> Result<Vec<Copied>, SettingsError> {
        let text = self.text()?;
        let (current, front_matter) = front_matter_of(&text).map_err(|error| self.front_matter_error(&error))?;
        if current != revision {
            let file = &self.path;
            return Err(SettingsError::Stale(format!(
                "Outdated revision ‘{revision}’ of the front matter of ‘{file}’ (its revision is ‘{current}’)"
            )));
        }
        let prepared = self.last_prepared();
        let mut copied = Vec::new();
        let mut planned = Vec::new();
        let mut names_planned = false;
        // The first edit of `edits` for which an edit is added to the plan.
        let mut needs_addition = None;
        for edit in edits {
            let planned_before = planned.len();
            let names = (!names_planned).then(|| front_matter_names(&prepared, &front_matter, edit.path.keys())).flatten();
            if let Some(names) = names {
                names_planned = true;
                plan_names(&edit.path.keys()[..4], names, edits, &mut copied, &mut planned);
            }
            let (before, after) = match edit.change {
                Change::Set(_) => copies_of(&prepared, &front_matter, edit.path.keys()),
                Change::Remove => (Vec::new(), Vec::new()),
            };
            plan_copies(before, edits, &mut copied, &mut planned);
            planned.push(edit.clone());
            plan_copies(after, edits, &mut copied, &mut planned);
            if needs_addition.is_none() && planned.len() > planned_before + 1 {
                needs_addition = Some(edit);
            }
        }
        if let Some(edit) = needs_addition.filter(|_| !prepared.succeeded()) {
            let (key, file) = (edit.path.keys().join("."), &self.path);
            return Err(SettingsError::Unprepared(format!(
                "‘{key}’ of ‘{file}’ may not be changed before a successful build"
            )));
        }
        let written = edit_front_matter(&text, &planned).map_err(|error| self.front_matter_error(&error))?;
        if written == text {
            return Ok(copied);
        }
        (self.write)(&self.path, written.as_bytes()).map_err(|error| SettingsError::File(error.to_string()))?;
        Ok(copied)
    }

    /// What the settings state takes from the preparations.
    fn last_prepared(&self) -> Arc<FromPreparation> {
        Arc::clone(&self.prepared.lock().unwrap_or_else(PoisonError::into_inner).shown)
    }

    /// Reads the file's text as it is now.
    fn text(&self) -> Result<String, SettingsError> {
        let bytes = self.files.read(&self.path).map_err(|error| SettingsError::File(error.to_string()))?;
        String::from_utf8(bytes)
            .map_err(|_| SettingsError::File(format!("The file ‘{}’ must be text encoded as UTF-8", self.path)))
    }

    /// The error `error` of the file's front matter, prefixed with the file and the place.
    fn front_matter_error(&self, error: &SeedError) -> SettingsError {
        let file = &self.path;
        SettingsError::FrontMatter(match error.place {
            Some(place) => format!("{file}:{place}: {error}"),
            None => format!("{file}: {error}"),
        })
    }
}

/// The revision of the front matter of `text`, a file's text, and what it writes, as data
/// (`null` for a file without a front matter). The revision is a digest of the front
/// matter's text, written as 16 hexadecimal digits; a file without a front matter has one
/// too.
fn front_matter_of(text: &str) -> Result<(String, Value), SeedError> {
    let split = split(text)?;
    let revision = format!("{:016x}", digest_of(&split.front_matter));
    let data = match split.front_matter {
        Some(front_matter) => read_front_matter(front_matter, split.front_matter_line)?.to_json(),
        None => Value::Null,
    };
    Ok((revision, data))
}

/// The record of `origin`, as the settings state gives it: `source`, the kind of settings
/// source; `text`, the program's text for the origin (its `Display`); and, when they are
/// known, `file`, `line` and `column`, the place where the value is written, and for a
/// package's value or a workflow's, `package` (the package's name,
/// `<identity>#<package>`), `extension` (the extension's stem) and `workflow`.
fn origin_record(origin: &Origin) -> Value {
    let (source, package, workflow) = match &origin.source {
        Source::Defaults => ("defaults", None, None),
        Source::Package { factory, stem, .. } => ("package", Some((factory, stem)), None),
        Source::FrontMatter(_) => ("front_matter", None, None),
        Source::Workflow { workflow, factory, stem, .. } => ("workflow", Some((factory, stem)), Some(workflow)),
        Source::CommandLine(..) => ("command_line", None, None),
    };
    let mut record = record(source, origin.to_string());
    if let Some(file) = origin.source.file() {
        record.insert(String::from("file"), json!(file));
    }
    if let Some(place) = origin.place {
        record.insert(String::from("line"), json!(place.line));
        record.insert(String::from("column"), json!(place.column));
    }
    if let Some((package, extension)) = package {
        record.insert(String::from("package"), json!(package));
        record.insert(String::from("extension"), json!(extension));
    }
    if let Some(workflow) = workflow {
        record.insert(String::from("workflow"), json!(workflow));
    }
    Value::Object(record)
}

/// The record of `origin`, the origin of a key of `flm` itself, in the form of
/// [`origin_record`]: a value that the request gives (the command line's) has the source
/// `command_line` and, for its text, the request's words.
fn key_origin_record(origin: &KeyOrigin) -> Value {
    match origin {
        KeyOrigin::Source(origin) => origin_record(origin),
        KeyOrigin::Given(words) => Value::Object(record("command_line", words.clone())),
    }
}

/// The two fields that every origin's record has: `source` and `text`.
fn record(source: &str, text: String) -> Map<String, Value> {
    let mut record = Map::new();
    record.insert(String::from("source"), json!(source));
    record.insert(String::from("text"), json!(text));
    record
}

/// The values in effect of `prepared`, `features` its settings in effect
/// ([`Prepared::settings_in_effect`]) and `cascaded` its combined settings, as the field
/// `in_effect` of the settings state gives them: `features`, by feature, `switched_off` (the
/// origin of the switch, or `null`) and `keys`, by key, `value` and `origin`, and for a key
/// combined key by key `entries`, the origins below it ([`entries_record`]); and `flm`, the
/// keys of `flm` itself.
fn in_effect_record(
    prepared: &Prepared,
    cascaded: &Cascaded,
    features: BTreeMap<String, crate::settings::FeatureInEffect>,
) -> Value {
    let features: Map<String, Value> = features
        .into_iter()
        .map(|(feature, in_effect)| {
            let keys: Map<String, Value> = in_effect
                .keys
                .into_iter()
                .map(|(key, key_in_effect)| {
                    let mut record = json!({ "value": key_in_effect.value, "origin": origin_record(&key_in_effect.origin) });
                    let mut place = vec![key.clone()];
                    if !key_in_effect.entry_origins.is_empty() || cascaded.combined_key_by_key(&feature, &place) {
                        let merged = |place: &[String]| cascaded.combined_key_by_key(&feature, place);
                        record["entries"] =
                            entries_record(&key_in_effect.entry_origins, &key_in_effect.below, &merged, &mut place);
                    }
                    (key, record)
                })
                .collect();
            let switched_off = in_effect.switched_off.as_ref().map(origin_record);
            (feature, json!({ "switched_off": switched_off, "keys": keys }))
        })
        .collect();
    let flm = prepared.flm_in_effect();
    let entry = |value: Value, origin: &KeyOrigin| json!({ "value": value, "origin": key_origin_record(origin) });
    let math = match &flm.math.value {
        MathChoice::Processor(name) => json!(name),
        MathChoice::NoProcessor => json!(false),
    };
    let parsing: Map<String, Value> = prepared
        .parsing_in_effect()
        .into_iter()
        .map(|(key, written)| (key, json!({ "value": written.value, "origin": origin_record(&written.origin) })))
        .collect();
    json!({
        "features": features,
        "flm": {
            "initial_use": entry(json!(flm.initial_use.value), &flm.initial_use.origin),
            "use": flm.uses.iter().map(|used| entry(json!(used.value), &used.origin)).collect::<Vec<_>>(),
            "math": entry(math, &flm.math.origin),
            "default_profile": entry(json!(flm.default_profile.value), &flm.default_profile.origin),
            "parsing": parsing,
        },
    })
}

/// The origins of the keys of a value combined key by key, as the field `entries` of a
/// value in effect gives them: each key that a source writes, by its name, mapped to
/// `{"origin": ORIGIN}`, with a field `entries` of the same form when the key's value is
/// itself combined key by key. A key of the feature's own default that no source writes is
/// absent. `origins` are the origins of the keys of the value at `place` (the feature's key,
/// then the names below it), `below` those of the levels below, and `merged` tells whether
/// the value at a place is combined key by key.
fn entries_record(
    origins: &BTreeMap<String, Origin>,
    below: &BTreeMap<String, EntryOrigins<Origin>>,
    merged: &dyn Fn(&[String]) -> bool,
    place: &mut Vec<String>,
) -> Value {
    let entries: Map<String, Value> = origins
        .iter()
        .map(|(name, origin)| {
            let mut record = json!({ "origin": origin_record(origin) });
            place.push(name.clone());
            let level = below.get(name).filter(|level| !level.entry_origins.is_empty());
            if let Some(level) = level {
                record["entries"] = entries_record(&level.entry_origins, &level.below, merged, place);
            } else if merged(place) {
                record["entries"] = Value::Object(Map::new());
            }
            place.pop();
            (name.clone(), record)
        })
        .collect();
    Value::Object(entries)
}

/// The choices of `prepared` that depend on the environment, as the field `choices` of the
/// settings state gives them: `templates`, by output, the compose templates registered for
/// it; `outputs`, the outputs of the build type, then those they are made from; `packages`,
/// the packages found; `packages_in_use`, the packages in use ([`Prepared::names_in_use`]);
/// `math_processors`; and `profiles`, the build types.
fn choices_record(prepared: &Prepared) -> Value {
    let registry = prepared.registry();
    let mut templates: Map<String, Value> = Map::new();
    for template in registry.services().services_named::<Template>(TEMPLATES_SLOT) {
        let listed = templates.entry(template.output.clone()).or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(listed) = listed {
            listed.push(json!({ "name": template.name, "package": template.package, "config": template.config }));
        }
    }
    let outputs: Vec<Value> = crate::session::with_sources(registry, prepared.outputs())
        .iter()
        .map(|output| {
            let made_from: Vec<String> = registry
                .output(output)
                .map(|entry| entry.def.made_from.iter().map(ToString::to_string).collect())
                .unwrap_or_default();
            let composed = compose::served_output(registry, output).map(|served| served.output.to_string());
            json!({ "name": output, "made_from": made_from, "composed_as": composed })
        })
        .collect();
    json!({
        "templates": templates,
        "outputs": outputs,
        "packages": packages_record(prepared.suggestions(), prepared.extension_descriptions()),
        "packages_in_use": prepared.names_in_use(),
        "math_processors": crate::seed::math_processors(prepared.suggestions()),
        "profiles": prepared.profiles(),
    })
}

/// The packages found, as the field `packages` of the choices gives them: each name of
/// `suggestions`, with its description in `descriptions` when it has one.
fn packages_record(suggestions: &[String], descriptions: &BTreeMap<String, String>) -> Value {
    let packages: Vec<Value> = suggestions
        .iter()
        .map(|name| match descriptions.get(name) {
            Some(description) => json!({ "name": name, "description": description }),
            None => json!({ "name": name }),
        })
        .collect();
    Value::Array(packages)
}

/// Why the preparation that `error` stopped failed, as the settings state gives it: the
/// first line of each problem's text, one line per problem, after the file's path and the
/// place when they are known.
fn reason_of(error: &PrepareError) -> String {
    let first_line = |text: &str| text.lines().next().unwrap_or_default().to_string();
    let lines: Vec<String> =
        error.problems.iter().map(|problem| problem_line(problem, first_line(&problem.detail))).collect();
    lines.join("\n")
}

/// `detail`, the text of `problem`, after the file's path and the place when they are known.
fn problem_line(problem: &Problem, detail: String) -> String {
    match (&problem.file, problem.place) {
        (Some(file), Some(place)) => format!("{file}:{}:{}: {detail}", place.line, place.column),
        (Some(file), None) => format!("{file}: {detail}"),
        _ => detail,
    }
}

/// The feature, the key and the names below the key that `path`, a path of the front
/// matter, names, when it lies under a feature's key (`flm.features.FEATURE.KEY…`).
fn feature_key(path: &[String]) -> Option<(&str, &str, &[String])> {
    match path {
        [root, features, feature, key, below @ ..] if root == RESERVED_KEY && features == "features" => {
            Some((feature, key, below))
        }
        _ => None,
    }
}

/// The value in effect at `path` of a front matter and its origin's record, read from
/// `in_effect`, the field `in_effect` of the settings state. Under a feature's key, the value
/// there (`null` included) and the origin at the deepest level of the key's `entries` that
/// the path reaches: the origin of the value decided whole that holds it, or none for a
/// value of the feature's own default, which an `entries` map lacks. Elsewhere, the value
/// that [`in_effect_at`] reads, with the origin of the key of `flm` that holds it (of the
/// entry of `use`, of the parsing key), and none at a place that several keys make (`use`,
/// `parsing`, a feature's entry).
fn value_in_effect(in_effect: &Value, path: &[String]) -> Option<(Value, Option<Value>)> {
    if let Some((feature, key, below)) = feature_key(path) {
        let record = in_effect["features"].get(feature)?["keys"].get(key)?;
        let value = value_at(record.get("value")?, below)?.clone();
        let (mut origin, mut level) = (Some(&record["origin"]), record.get("entries"));
        for name in below {
            let Some(entries) = level else { break };
            let entry = entries.get(name);
            (origin, level) = (entry.map(|entry| &entry["origin"]), entry.and_then(|entry| entry.get("entries")));
            if entry.is_none() {
                break;
            }
        }
        return Some((value, origin.cloned()));
    }
    let value = in_effect_at(in_effect, path)?;
    let flm = &in_effect["flm"];
    let origin = match path.get(1..).unwrap_or_default() {
        [parsing, key, ..] if parsing == "parsing" => flm["parsing"].get(key.as_str()),
        [uses, at, ..] if uses == "use" => at.parse::<usize>().ok().and_then(|at| flm["use"].get(at)),
        [key, ..] if matches!(key.as_str(), "initial_use" | "math" | "default_profile") => flm.get(key.as_str()),
        _ => None,
    };
    Some((value, origin.and_then(|key| key.get("origin")).cloned()))
}

/// Whether `path`, a path of the front matter, lies under a feature that a source switches
/// off, as `in_effect`, the field `in_effect` of the settings state, gives it.
fn switched_off(in_effect: &Value, path: &[String]) -> bool {
    feature_key(path).is_some_and(|(feature, _, _)| !in_effect["features"][feature]["switched_off"].is_null())
}

/// The default that applies at `path` of a front matter: under the settings of a compose
/// template (`flm.features.compose.template_config.OUTPUT.NAME…`), the compose template's
/// default as the choices give it (the defaults that a template module computes included);
/// else the default of the schema ([`default_at`]).
fn default_in_effect(prepared: &FromPreparation, path: &[String]) -> Option<Value> {
    let setting = feature_key(path).filter(|(_, _, below)| below.len() > 2);
    if let Some((compose::FEATURE, "template_config", [output, name, setting @ ..])) = setting {
        let templates = prepared.choices["templates"].get(output.as_str())?.as_array()?;
        let template = templates.iter().find(|template| template["name"] == name.as_str())?;
        return value_at(&template["config"], setting).cloned();
    }
    default_at(&prepared.schema, path)
}

/// The values that a value set at `path` of the front matter copies into it
/// ([`DocumentSettings::write`]), those placed before the value set, then those after it,
/// `front_matter` being what the front matter writes and `prepared` what the last
/// preparation gives. The copies are the keys of the value in effect at the place decided
/// whole that holds `path` ([`whole_place`]), and of each value below it along `path`,
/// other than the key that leads to `path`; a single name of `compose.template` counts as
/// the map of each output to that name ([`template_names`]).
fn copies_of(prepared: &FromPreparation, front_matter: &Value, path: &[String]) -> (Vec<Copied>, Vec<Copied>) {
    let none = (Vec::new(), Vec::new());
    let Some((place, mut value)) = whole_place(prepared, front_matter, path) else { return none };
    let (mut before, mut after) = (Vec::new(), Vec::new());
    let mut at = path[..place].to_vec();
    for segment in &path[place..] {
        let Value::Object(map) = &value else { break };
        let mut level = Vec::new();
        let mut next = None;
        for (name, item) in map {
            if name == segment {
                next = Some(item.clone());
                continue;
            }
            let mut copied = at.clone();
            copied.push(name.clone());
            let copy = Copied { path: copied, value: item.clone() };
            if next.is_some() { level.push(copy) } else { before.push(copy) }
        }
        after.push(level);
        at.push(segment.clone());
        match next {
            Some(item) => value = item,
            None => break,
        }
    }
    (before, after.into_iter().rev().flatten().collect())
}

/// The place decided whole that holds `path` of the front matter, when a value set at `path`
/// replaces there the value that a lower source gives ([`copies_of`]): the number of keys of
/// `path` that lead to it, and the value in effect there, a single name of
/// `compose.template` as the map of each output to that name ([`template_names`]).
///
/// It is the place that the cascade decides whole ([`Cascaded::decided_at`]), given by a
/// write or, below the key, by an entry of the feature's own default of a value combined key
/// by key; there is one only when that place lies above `path`, what gives its value ranks
/// below the front matter (a default does), and the front matter writes nothing there. A
/// single name of `compose.template` that the front matter writes is that place for a value
/// set at one output's name, whatever source is in effect.
fn whole_place(prepared: &FromPreparation, front_matter: &Value, path: &[String]) -> Option<(usize, Value)> {
    let (feature, key, below) = feature_key(path)?;
    let cascaded = prepared.cascaded.as_ref()?;
    let key_place = path.len() - below.len();
    if let Some(names) = front_matter_names(prepared, front_matter, path) {
        return Some((key_place, Value::Object(names)));
    }
    let decided = cascaded.decided_at(feature, key, below)?;
    let depth = decided.depth;
    let below_front_matter = match &decided.origin {
        Some(origin) => origin.source.rank() < Rank::FrontMatter,
        // A key that no source writes, decided whole, holds no value to copy.
        None => depth > 0,
    };
    if depth == below.len() || !below_front_matter || value_at(front_matter, &path[..key_place + depth]).is_some() {
        return None;
    }
    let value = match prepared.in_effect["features"][feature]["keys"].get(key) {
        Some(record) => value_at(&record["value"], &below[..depth])?.clone(),
        // No source writes the key: its value is the feature's own default entries.
        None => value_at(&Value::Object(cascaded.declaration(feature)?.default_entries(key)?.clone()), &below[..depth])?.clone(),
    };
    let value = match (feature, key) {
        (compose::FEATURE, "template") => template_names(prepared, &value).map(Value::Object).unwrap_or(value),
        _ => value,
    };
    Some((key_place + depth, value))
}

/// The map of each output to the name of its compose template that `value`, a value of
/// `compose.template`, gives when it is a single name ([`compose::TemplateChoice::Name`]),
/// the outputs being those for which a compose template is registered (the outputs of the
/// choice `templates`, in its order; none before the first preparation); `None` for any
/// other value.
fn template_names(prepared: &FromPreparation, value: &Value) -> Option<Map<String, Value>> {
    let choice @ compose::TemplateChoice::Name(_) = serde_json::from_value(value.clone()).ok()? else { return None };
    let outputs = prepared.choices["templates"].as_object().into_iter().flat_map(Map::keys);
    Some(outputs.map(|output| (output.clone(), json!(choice.for_output(output)))).collect())
}

/// The map of each output to its compose template's name ([`template_names`]), when `path`
/// lies below `compose.template` and the front matter writes a single name there; `None`
/// otherwise.
fn front_matter_names(prepared: &FromPreparation, front_matter: &Value, path: &[String]) -> Option<Map<String, Value>> {
    let (compose::FEATURE, "template", [_, ..]) = feature_key(path)? else { return None };
    template_names(prepared, value_at(front_matter, &path[..4])?)
}

/// Adds to `planned` an edit that sets each of `copies`, and the copy to `copied`, except a
/// copy at a path that one of `edits` changes, or that holds or lies inside the path of
/// one, and a copy already planned.
fn plan_copies(copies: Vec<Copied>, edits: &[Edit], copied: &mut Vec<Copied>, planned: &mut Vec<Edit>) {
    for copy in copies {
        let touched = edits.iter().any(|edit| edit.path.keys().starts_with(&copy.path) || copy.path.starts_with(edit.path.keys()));
        if touched || copied.iter().any(|done: &Copied| done.path == copy.path) {
            continue;
        }
        let Some(path) = KeyPath::new(copy.path.clone()) else { continue };
        planned.push(Edit { path, change: Change::Set(copy.value.clone()) });
        copied.push(copy);
    }
}

/// Adds to `planned` the edit that writes out the single name of `compose.template` that the
/// front matter writes, at `key`, as `names`, the map of each output to that name
/// ([`template_names`]): an output whose name one of `edits` changes takes the name that
/// the last of them sets, or no entry when it removes the name or changes a value below it
/// (that edit, made after, gives the same result), each other output keeps the name, which
/// goes to `copied`; no name left removes the key. Nothing is planned when one of `edits`
/// changes `compose.template` itself, or a key that holds it.
fn plan_names(key: &[String], names: Map<String, Value>, edits: &[Edit], copied: &mut Vec<Copied>, planned: &mut Vec<Edit>) {
    if edits.iter().any(|edit| key.starts_with(edit.path.keys())) {
        return;
    }
    let mut kept = Map::new();
    for (output, name) in names {
        let mut path = key.to_vec();
        path.push(output.clone());
        let changes: Vec<&Edit> = edits.iter().filter(|edit| edit.path.keys().starts_with(&path)).collect();
        match changes.last() {
            None => {
                copied.push(Copied { path, value: name.clone() });
                kept.insert(output, name);
            }
            Some(Edit { path: changed, change: Change::Set(value) }) if changed.keys() == path => {
                kept.insert(output, value.clone());
            }
            Some(_) => {}
        }
    }
    let Some(path) = KeyPath::new(key.to_vec()) else { return };
    let change = if kept.is_empty() { Change::Remove } else { Change::Set(Value::Object(kept)) };
    planned.push(Edit { path, change });
}

/// The record of `copied`, as the answers of the server give it: `{"path", "value"}`.
pub(super) fn copied_record(copied: &Copied) -> Value {
    json!({ "path": copied.path, "value": copied.value })
}

/// The value at `path` inside `value`: a key of a map, or the index of an entry of a list,
/// at each step.
fn value_at<'v>(value: &'v Value, path: &[String]) -> Option<&'v Value> {
    path.iter().try_fold(value, |value, segment| match value {
        Value::Object(map) => map.get(segment),
        Value::Array(list) => list.get(segment.parse::<usize>().ok()?),
        _ => None,
    })
}

/// The value in effect at `path` of a front matter, read from `in_effect`, the field
/// `in_effect` of the settings state: under a feature's key, under a parsing key, and under
/// `use`, `initial_use`, `math` and `default_profile`; at a feature's entry, the map of the
/// keys that a source writes, with `enabled: false` for a feature switched off; at
/// `parsing`, the map of the parsing keys that a source writes; `None` elsewhere, and where
/// no source writes a value.
fn in_effect_at(in_effect: &Value, path: &[String]) -> Option<Value> {
    let [root, below @ ..] = path else { return None };
    if root != RESERVED_KEY {
        return None;
    }
    let flm = &in_effect["flm"];
    let (value, rest) = match below {
        [features, feature, key, rest @ ..] if features == "features" => {
            (in_effect["features"][feature.as_str()]["keys"][key.as_str()]["value"].clone(), rest)
        }
        [features, feature] if features == "features" => {
            let record = &in_effect["features"][feature.as_str()];
            let mut values = values_of(&record["keys"]);
            if !record["switched_off"].is_null() {
                values.insert(String::from(ENABLED_KEY), Value::Bool(false));
            }
            return (!values.is_empty()).then(|| Value::Object(values));
        }
        [parsing] if parsing == "parsing" => {
            let values = values_of(&flm["parsing"]);
            return (!values.is_empty()).then(|| Value::Object(values));
        }
        [parsing, key, rest @ ..] if parsing == "parsing" => (flm["parsing"][key.as_str()]["value"].clone(), rest),
        [uses, rest @ ..] if uses == "use" => {
            let values = flm["use"].as_array()?.iter().map(|used| used["value"].clone()).collect();
            (Value::Array(values), rest)
        }
        [key, rest @ ..] if matches!(key.as_str(), "initial_use" | "math" | "default_profile") => {
            (flm[key.as_str()]["value"].clone(), rest)
        }
        _ => return None,
    };
    if value.is_null() {
        return None;
    }
    value_at(&value, rest).cloned()
}

/// The values of `keys`, a map of the settings state that gives each key's `value` and
/// `origin`, by key; a key without a value is left out.
fn values_of(keys: &Value) -> Map<String, Value> {
    let Some(keys) = keys.as_object() else { return Map::new() };
    keys.iter().filter(|(_, key)| !key["value"].is_null()).map(|(name, key)| (name.clone(), key["value"].clone())).collect()
}

/// The default that `schema`, the schema of a front matter, gives at `path`: the default of
/// the schema of the value there, else the value at the rest of the path inside the
/// default of the nearest schema above it that has one. References into the schema's
/// `$defs` are followed, and every branch of `anyOf`, `oneOf` and `allOf` is looked in.
fn default_at(schema: &Value, path: &[String]) -> Option<Value> {
    default_below(schema, schema, path, 0)
}

/// [`default_at`] for the schema `node` inside `root`, at a depth of `depth` schemas.
fn default_below(root: &Value, node: &Value, path: &[String], depth: usize) -> Option<Value> {
    // A schema that refers to itself ends here.
    if depth > 64 {
        return None;
    }
    let node = match node.get("$ref").and_then(Value::as_str) {
        Some(reference) => root.pointer(reference.strip_prefix('#')?)?,
        None => node,
    };
    let deeper = path.split_first().and_then(|(segment, rest)| {
        let child = node
            .get("properties")
            .and_then(|properties| properties.get(segment))
            .or_else(|| node.get("additionalProperties").filter(|child| child.is_object()))
            .or_else(|| node.get("items").filter(|_| segment.parse::<usize>().is_ok()))?;
        default_below(root, child, rest, depth + 1)
    });
    let branches = || {
        ["anyOf", "oneOf", "allOf"]
            .iter()
            .filter_map(|keyword| node.get(*keyword).and_then(Value::as_array))
            .flatten()
            .find_map(|branch| default_below(root, branch, path, depth + 1))
    };
    let here = || node.get("default").and_then(|default| value_at(default, path)).cloned();
    deeper.or_else(|| if path.is_empty() { here().or_else(branches) } else { branches().or_else(here) })
}

/// `value` written as YAML text, ending with a line end.
fn yaml_text(value: &Value) -> String {
    let mut text = String::new();
    let mut emitter = YamlEmitter::new(&mut text);
    emitter.multiline(true);
    // Writing into a `String` cannot fail.
    let _ = emitter.dump(&yaml_of(value));
    let mut text = text.strip_prefix("---").unwrap_or(&text).trim_start_matches([' ', '\n']).to_string();
    text.push('\n');
    text
}

/// `value` as a YAML value, for [`yaml_text`].
fn yaml_of(value: &Value) -> Yaml<'static> {
    let text = |text: &str| Yaml::Value(Scalar::String(Cow::Owned(text.to_string())));
    match value {
        Value::Null => Yaml::Value(Scalar::Null),
        Value::Bool(value) => Yaml::Value(Scalar::Boolean(*value)),
        Value::Number(number) => match number.as_i64() {
            Some(integer) => Yaml::Value(Scalar::Integer(integer)),
            None => Yaml::Representation(Cow::Owned(number.to_string()), ScalarStyle::Plain, None),
        },
        Value::String(value) if value.contains('\n') && !reads_back_as_literal_block(value) => {
            let quoted = Value::String(value.clone()).to_string();
            let escaped = &quoted[1..quoted.len() - 1];
            Yaml::Representation(Cow::Owned(escaped.to_string()), ScalarStyle::DoubleQuoted, None)
        }
        Value::String(value) => text(value),
        Value::Array(list) => Yaml::Sequence(list.iter().map(yaml_of).collect()),
        Value::Object(map) => Yaml::Mapping(map.iter().map(|(key, value)| (text(key), yaml_of(value))).collect()),
    }
}

/// Whether the text `text`, of several lines, reads back as itself when the emitter of
/// [`yaml_text`] writes it as a literal block: each line as it is, indented, after `|` (a
/// text that ends with a line end) or `|-`. Such a block reads the indentation from its
/// first line that is not empty, keeps a single final line end, splits lines at `\r` as
/// well, and ends at a line `---` or `...` when it is written at the root; a text that
/// meets any of these is written in double quotes, with escapes.
fn reads_back_as_literal_block(text: &str) -> bool {
    let first_written = text.trim_start_matches('\n');
    !text.contains('\r')
        && !text.ends_with("\n\n")
        && !first_written.is_empty()
        && !first_written.starts_with([' ', '\t'])
        && !text.split('\n').any(|line| line.starts_with("---") || line.starts_with("..."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_as_yaml_reads_back_as_the_value() {
        let values = [
            json!(null), json!("true"), json!("a: b"), json!(1.5), json!(-3), json!([1, "x", {}]), json!({ "a": [] }),
            json!("a\nb"), json!({ "a": "b\nc\n" }), json!("a\n\"b\"\n"),
            // Texts of several lines that a literal block does not carry.
            json!("  a\nb"), json!({ "a": "\n\tb\nc" }), json!("a\n\n"), json!({ "a": "\n" }), json!("a\r\nb"), json!("a\n---\nb"),
        ];
        for value in values {
            let text = yaml_text(&value);
            let read = read_front_matter(&text, 1).unwrap().to_json();
            assert_eq!(read, value, "{text:?}");
        }
    }
}
