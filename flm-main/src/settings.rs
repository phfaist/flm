//! Combining a document's settings from their sources into one value per feature and key,
//! each value keeping a record of where it was written.
//!
//! Settings are a feature's configuration and option values written as data, one map per
//! feature from a settings key to its value, which the library reads into the feature's own
//! Rust types ([`flm_core::core::settings`]). The host also treats the library's parsing
//! settings (`comments`, `comment_prefix`, `dollar_math`; [`FlmParsing`]) as settings. The
//! host combines a document's settings from ranked *settings sources* (the program's
//! defaults, the packages of the extensions the document uses, the front matter, the
//! workflow, the command line), each value keeping a record of where it was written
//! ([`Origin`]). The
//! sources, from the lowest rank to the highest ([`Rank`]):
//!
//! 1. **the program's defaults**: the values the host sets for the
//!    library's features ([`crate::stdpkg::defaults`]), which replace each feature's own
//!    defaults;
//! 2. **the packages of the extensions** the document uses, each a source of its own
//!    ([`crate::packages`]): the block each package writes (for an extension written as
//!    data, its manifest's `flm`);
//! 3. **the front matter** of the document, under `flm.features` and `flm.parsing`
//!    ([`crate::seed`]);
//! 4. **the workflow**: the settings that a package gives the workflow that runs, for
//!    keys set per render request only;
//! 5. **the command line**: assignments `FEATURE.KEY=VALUE`, the value written in YAML's
//!    one-line form (the `flm` program's `-O 'html.heading_tags={section: h2}'`), or
//!    `FEATURE.KEY.NAME=VALUE` to set one place inside the key's value
//!    ([`add_assignment`]), each with the words the user typed
//!    ([`CommandLineSetting`]), which a problem with it quotes.
//!
//! The library's settings cascade ([`flm_core::core::settings::cascade`]) combines them:
//! the source of the highest rank that writes a key gives its value, whole; within the front
//! matter and within the command line, the value written last wins; between packages, a
//! package wins over the packages it uses, directly or through others
//! ([`PackageCatalog::reach`](flm_core::core::registry::PackageCatalog::reach), keyed by the
//! name of each package's factory), and peers must agree; a key that a feature marks as
//! merged entry by entry is combined one entry at a time; a feature switched off by `false`
//! is read by the same rule; the settings a package gives a
//! workflow may set only keys set per render request. That module's documentation states
//! the rule in full. This module holds the host's part: the sources and the texts that
//! name them ([`Source`], [`Origin`], which implements
//! [`CascadeOrigin`]), the readers that record a source's values in the cascade
//! ([`add_block`], [`add_workflow_settings`], [`add_assignment`]), and the
//! names the rest of the host uses ([`Settings`], [`SettingsProblem`], [`SettingValue`]). A
//! relative path in a value that a feature marks as a path to a file is resolved against
//! the folder of the file in which the value is written ([`Origin`]'s [`CascadeOrigin::resolve_path`]); a value
//! of the command line is relative to the current folder already. The front matter
//! switches on, with `NAME: {}`, a feature that a package switched off; a key written by
//! the workflow or on the command line does not switch a feature on.
//!
//! `false` for a feature that no package in use has is accepted, each with a warning at the
//! place where it is written (it may be a misspelled name); settings for such a feature are
//! a problem. [`warning_diagnostic`] gives the cascade's warning as the diagnostic that
//! [`session::prepare`](crate::session::prepare) reports ([`UnknownFeatureSwitchedOff`]),
//! with the problems when the preparation stops.
//!
//! # Render values of a workflow defined in Rust
//!
//! A workflow defined in Rust carries its render option values as typed values
//! ([`WorkflowDef::render_options`](flm_core::core::registry::WorkflowDef#structfield.render_options)),
//! not as data key by key, so they take no part in this rule. The library replaces the
//! workflow's value of an option type, whole, by the value the settings give that type
//! ([`Overrides::render_options`](flm_core::core::plan::Overrides::render_options)): as
//! soon as any source writes one key of an option type, the workflow's typed value of that
//! type is replaced by the type's defaults with the keys written. No workflow of the
//! standard packages carries such a value.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use flm_core::core::settings::cascade::{self, Cascade, CascadeOrigin, Problem};
use flm_core::lang::{FlmDiagnostic, FlmOrigin, FlmSource};
use flm_core::techy::error::{Diagnostic, DiagnosticInfo};
use flm_core::techy::source::SourceSpan;
#[cfg(doc)]
use flm_core::lang::FlmParsing;
use serde_json::{Map, Value};

use flm_ext::block::{Block, FeatureBlock, WorkflowBlock};

use crate::seed::{Node, Place, read_front_matter};

pub use flm_core::core::settings::cascade::{Reach, ReadFailure, ReadSettings};

/// A document's settings, collected from their sources: every value each source wrote, with
/// its [`Origin`], combined by the rule of the ranks when the settings are read
/// ([`Cascade`]).
///
/// [`session::prepare`](crate::session::prepare) fills it from the program's defaults, the
/// packages of the extensions, the front matter, the workflow's settings and the command line, then reads
/// it with [`Cascade::read`] and [`Cascade::parsing`].
pub type Settings = Cascade<Origin>;

/// A problem with the settings, placed at the [`Origin`] of the value at fault
/// ([`Problem`]).
pub type SettingsProblem = Problem<Origin>;

/// One setting's value, as JSON data, with the [`Origin`] it was written at
/// ([`cascade::SettingValue`]).
pub type SettingValue = cascade::SettingValue<Origin>;

/// The settings of one feature in effect: whether it is switched off and where, and the
/// value in effect of each key written, each with its [`Origin`]
/// ([`cascade::FeatureInEffect`]), as
/// [`Prepared::settings_in_effect`](crate::session::Prepared::settings_in_effect) lists them.
pub type FeatureInEffect = cascade::FeatureInEffect<Origin>;

/// The value in effect of one key of a feature, with the [`Origin`] of the value and, for a
/// key merged entry by entry, of each entry ([`cascade::KeyInEffect`]).
pub type KeyInEffect = cascade::KeyInEffect<Origin>;

/// Condition `flm.host.unknown-feature-switched-off`: `false` written for a feature that no
/// package in use has, which the settings accept but which may be a misspelled name, as the
/// cascade finds it ([`ReadSettings::warnings`], [`ReadFailure::warnings`]). Reported as a
/// Warning by [`session::prepare`](crate::session::prepare), once for each `false` written,
/// at the place where it is written ([`warning_diagnostic`]), also when the preparation
/// stops ([`PrepareError::diagnostics`](crate::session::PrepareError::diagnostics)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFeatureSwitchedOff {
    /// The message: the condition, followed by where the value is written when that is not
    /// a place in a file.
    pub detail: String,
}

impl fmt::Display for UnknownFeatureSwitchedOff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl DiagnosticInfo for UnknownFeatureSwitchedOff {
    const IDENTIFIER: &'static str = "flm.host.unknown-feature-switched-off";
}

/// Returns the warning `warning`, one of the cascade's warnings ([`ReadSettings::warnings`],
/// [`ReadFailure::warnings`]), all of which are of one kind, as a Warning
/// ([`UnknownFeatureSwitchedOff`]): placed at the file, the line and the column of its origin
/// when the origin is a place in a file, and quoting the origin in its message otherwise, as
/// a problem that stops the work does.
pub fn warning_diagnostic(warning: &SettingsProblem) -> FlmDiagnostic {
    let (source, detail) = match &warning.origin {
        Some(Origin { source, place: Some(place) }) if source.file().is_some() => {
            let file = source.file().unwrap_or_default();
            let source = FlmSource::new("")
                .with_origin(FlmOrigin::new(file))
                .with_line_column_number_offsets(place.line as usize, place.column as usize);
            (source, warning.detail.clone())
        }
        Some(origin) => (FlmSource::new("").with_origin(FlmOrigin::synthetic("flm")), format!("{} (in {origin})", warning.detail)),
        None => (FlmSource::new("").with_origin(FlmOrigin::synthetic("flm")), warning.detail.clone()),
    };
    Diagnostic::warning(UnknownFeatureSwitchedOff { detail }, SourceSpan::new(&Arc::new(source), 0..0))
}

/// The rank of a settings source: a source of a higher rank replaces the values that a
/// source of a lower rank writes. The variants are ordered from the lowest rank to the
/// highest. See the [module documentation](self).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    /// The program's defaults.
    Defaults,
    /// The packages of the extensions; a package wins over the packages it reaches, and the
    /// others are peers.
    Package,
    /// The document's front matter.
    FrontMatter,
    /// The workflow's settings; they may set only keys that are set per render request.
    /// Their sources are the packages that give the workflow settings, between which the
    /// rule between packages decides.
    Workflow,
    /// The command line.
    CommandLine,
}

/// A settings source, identified: where a setting comes from. [`Source::rank`] gives its
/// rank.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// The program's defaults, which the host sets ([`crate::stdpkg::defaults`]).
    Defaults,
    /// A package of an extension: its block.
    Package {
        /// The factory's name (`<identity>#<package>`): the peer the reach decides by.
        factory: String,
        /// The extension's manifest file (its identity).
        file: String,
        /// The extension's stem.
        stem: String,
        /// The package's name within the extension.
        package: String,
    },
    /// The front matter of the file of this name.
    FrontMatter(String),
    /// The settings a package of an extension gives the workflow `workflow`.
    Workflow {
        /// The workflow's name.
        workflow: String,
        /// The factory's name (`<identity>#<package>`).
        factory: String,
        /// The extension's manifest file (its identity).
        file: String,
        /// The extension's stem.
        stem: String,
        /// The package's name within the extension.
        package: String,
    },
    /// The command line: the setting at this position, counted from 1 among the command
    /// line's settings, with the words the user typed ([`CommandLineSetting::label`]).
    CommandLine(usize, String),
}

impl Source {
    /// Returns the rank of the source.
    pub fn rank(&self) -> Rank {
        match self {
            Source::Defaults => Rank::Defaults,
            Source::Package { .. } => Rank::Package,
            Source::FrontMatter(_) => Rank::FrontMatter,
            Source::Workflow { .. } => Rank::Workflow,
            Source::CommandLine(..) => Rank::CommandLine,
        }
    }

    /// Returns the name of the file the source is written in, when the source is a file (an
    /// extension's manifest, for its packages and the settings they give a workflow; a
    /// front matter).
    pub fn file(&self) -> Option<&str> {
        match self {
            Source::Package { file, .. } | Source::FrontMatter(file) | Source::Workflow { file, .. } => Some(file),
            Source::Defaults | Source::CommandLine(..) => None,
        }
    }
}

/// Where a value was written: its source, and its place in the source's file when the
/// source is a file.
///
/// Its `Display` text is how a message refers to it: "the program's defaults"; "the
/// option" followed by the user's words between backticks; `FILE:LINE:COLUMN`; "the front
/// matter of FILE"; or, for a package's value without a place (a value an engine
/// reported), "the package `PACKAGE` of the extension `STEM` (`FILE`)".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Origin {
    /// The source.
    pub source: Source,
    /// The place in the source's file, when the source is a file and the place is known.
    pub place: Option<Place>,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.source, self.place) {
            (Source::Defaults, _) => f.write_str("the program's defaults"),
            (Source::CommandLine(_, label), _) => write!(f, "the option `{label}`"),
            (source, Some(place)) => write!(f, "{}:{place}", source.file().unwrap_or_default()),
            (Source::FrontMatter(file), None) => write!(f, "the front matter of {file}"),
            (Source::Package { file, stem, package, .. } | Source::Workflow { file, stem, package, .. }, None) => {
                write!(f, "the package `{package}` of the extension `{stem}` (`{file}`)")
            }
        }
    }
}

/// One setting of the command line: an assignment, and the words the user typed.
///
/// The program that reads the command line makes one per setting and gives them, in order,
/// in [`Request::options`](crate::session::Request::options); among them, the value written
/// last wins. The assignment need not be the user's text: a program may offer a short form
/// of an assignment (the `flm` program's `-t NAME` is the assignment
/// `compose.template=NAME`), and the label keeps the user's words, so that a problem with
/// the value quotes what the user typed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandLineSetting {
    /// The assignment, `FEATURE.KEY=VALUE`, the value written in YAML's one-line form.
    pub assignment: String,
    /// The words the user typed, as the program shows them (`-O text.width=60`, `-t none`):
    /// what a problem with the setting quotes.
    pub label: String,
}

impl CommandLineSetting {
    /// Returns the setting `assignment`, which the user typed as `label`.
    pub fn new(assignment: impl Into<String>, label: impl Into<String>) -> CommandLineSetting {
        CommandLineSetting { assignment: assignment.into(), label: label.into() }
    }
}

impl CascadeOrigin for Origin {
    /// The rank of the origin's source, in the order of [`Rank`].
    fn rank(&self) -> u32 {
        self.source.rank() as u32
    }

    /// The name of the factory of a package, for its settings and for the settings it gives
    /// a workflow: the reach decides by it.
    fn peer(&self) -> Option<&str> {
        match &self.source {
            Source::Package { factory, .. } | Source::Workflow { factory, .. } => Some(factory),
            Source::Defaults | Source::FrontMatter(_) | Source::CommandLine(..) => None,
        }
    }

    /// The workflow that a package gives the settings to.
    fn workflow(&self) -> Option<&str> {
        match &self.source {
            Source::Workflow { workflow, .. } => Some(workflow),
            _ => None,
        }
    }

    /// The document's front matter, which decides between packages.
    fn higher_source(&self) -> Option<&str> {
        match self.source.rank() {
            Rank::Package => Some("the document's front matter"),
            _ => None,
        }
    }

    /// A relative path joined to the folder of the file the source is written in, and
    /// normalized ([`fold`](crate::files::fold)); `None` for an absolute path and for a
    /// source that is no file (a value of the command line is relative to the current
    /// folder already).
    // See decision rationale at [§flm-dd-dr:host-settings-path-resolve]
    fn resolve_path(&self, path: &str) -> Option<String> {
        let folder = self.source.file().and_then(|file| Path::new(file).parent())?;
        let path = Path::new(path);
        if path.is_absolute() {
            return None;
        }
        Some(crate::files::fold(&folder.join(path)).to_string_lossy().into_owned())
    }
}

/// Records in `settings` the block `block` of `source` (a front matter, or a loaded
/// package's block): each feature switched on or off and each of its keys, each parsing
/// key, each field of each workflow, each at its own origin. `flm` is the node the block
/// was read from, when there is one, which gives each origin its place
/// (`flm.features.F.KEY`); without it, the origins have no place. The settings a workflow
/// is given are recorded once the workflow is chosen ([`add_workflow_settings`]).
pub fn add_block(settings: &mut Settings, block: &Block, source: &Source, flm: Option<&Node>) {
    let origin = |path: &[&str]| Origin {
        source: source.clone(),
        place: flm.and_then(|node| node.at(path)).map(|node| node.place),
    };
    for (feature, entry) in &block.features {
        match entry {
            FeatureBlock::Off => settings.switch_off(feature, origin(&["features", feature])),
            FeatureBlock::Settings(map) => {
                settings.switch_on(feature, origin(&["features", feature]));
                for (key, value) in map {
                    settings.set(feature, key, value.clone(), origin(&["features", feature, key]));
                }
            }
        }
    }
    for (key, value) in block.parsing.iter().flatten() {
        settings.set_parsing(key, value.clone(), origin(&["parsing", key]));
    }
    for workflow in &block.workflows {
        let fields = workflow
            .fields
            .iter()
            .map(|(field, value)| (field.clone(), value.clone(), origin(&["workflows", &workflow.name, field])));
        settings.add_workflow_fields(&workflow.name, fields);
    }
}

/// Records in `settings` the settings `workflow` gives the workflow `name` of a package's
/// block, written in `source` (a [`Source::Workflow`]), as a source of the workflow rank:
/// each key of each feature, without switching the feature on or off. `flm` gives the
/// places, as for [`add_block`].
pub fn add_workflow_settings(settings: &mut Settings, workflow: &WorkflowBlock, source: &Source, flm: Option<&Node>) {
    for (feature, map) in &workflow.settings {
        for (key, value) in map {
            let path = ["workflows", workflow.name.as_str(), "settings", feature, key];
            let origin = Origin { source: source.clone(), place: flm.and_then(|node| node.at(&path)).map(|node| node.place) };
            settings.set(feature, key, value.clone(), origin);
        }
    }
}

/// Records in `settings` the setting `setting`, one setting of the command line; `index` is its position among
/// the command line's settings, counted from 1. Its origin quotes the setting's
/// [label](CommandLineSetting::label).
///
/// The assignment is `FEATURE.KEY=VALUE`, or `FEATURE.KEY.NAME…=VALUE` to write inside
/// the key's value: the names after the key, separated by `.`, designate one place
/// inside the value, and the assignment writes a value for the key that holds `VALUE` at
/// that place and nothing else: `floats.types.algorithm={…}` is
/// `floats.types={algorithm: {…}}`. So a path to one entry of a key merged entry by entry
/// sets that entry and leaves the others, and a path inside any other key writes that
/// key's value whole, as every source does.
///
/// # Errors
///
/// A [`SettingsProblem`] when the assignment has no `=`, when the part before the `=` is
/// not a feature's name and a key joined by a `.`, when one of its names is empty, or
/// when the value is not valid YAML.
// The problem carries its origin, whose package source names the factory, the file, the
// stem and the package; it is returned once per command-line setting, never in a loop.
#[allow(clippy::result_large_err)]
pub fn add_assignment(settings: &mut Settings, setting: &CommandLineSetting, index: usize) -> Result<(), SettingsProblem> {
    let CommandLineSetting { assignment: text, label } = setting;
    let origin = Origin { source: Source::CommandLine(index, label.clone()), place: None };
    let problem =
        |detail: String| SettingsProblem { origin: Some(origin.clone()), detail };
    let Some((path, value)) = text.split_once('=') else {
        return Err(problem(format!("`{label}` must have the form `FEATURE.KEY=VALUE`")));
    };
    let names: Vec<&str> = path.split('.').collect();
    let [feature, key, below @ ..] = names.as_slice() else {
        return Err(problem(format!("in `{label}`, `{path}` must be a feature's name and a key, joined by a `.`")));
    };
    if names.iter().any(|name| name.is_empty()) {
        return Err(problem(format!("in `{label}`, no name in `{path}` may be empty")));
    }
    let node: Node = read_front_matter(value, 1)
        .map_err(|error| problem(format!("the value of `{label}` must be valid YAML: {}", error.detail)))?;
    let value = below.iter().rev().fold(node.to_json(), |inner, name| {
        let mut map = Map::new();
        map.insert((*name).to_string(), inner);
        Value::Object(map)
    });
    settings.set(feature, key, value, origin);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flm_core::stdcore::backends::html::HtmlRenderOptions;
    use flm_core::stdcore::factory;
    use flm_core::stdcore::features::floats::FloatsConfig;

    fn assign(text: &str) -> CommandLineSetting {
        CommandLineSetting::new(text, format!("-O {text}"))
    }

    #[test]
    fn a_path_below_a_key_on_the_command_line_writes_that_place() {
        let factory = factory().unwrap();
        let mut settings = Settings::new();
        add_assignment(&mut settings, &assign("floats.types.algorithm={handlers: [includegraphics]}"), 1).unwrap();
        add_assignment(&mut settings, &assign("floats.types.table=false"), 2).unwrap();
        add_assignment(&mut settings, &assign("html.heading_tags.section=h2"), 3).unwrap();
        let read = settings.read(&[&factory]).unwrap();
        let floats = read.packages[factory.name()].config::<FloatsConfig>("floats").unwrap();
        let names: Vec<&str> = floats.types.iter().map(|float| float.name.as_str()).collect();
        assert_eq!(names, ["figure", "algorithm"]);
        assert_eq!(settings.value("html", "heading_tags").unwrap().value, serde_json::json!({"section": "h2"}));
        assert!(add_assignment(&mut settings, &assign("floats.types..x=1"), 4).is_err());
        // Below a key that is not merged entry by entry, a path writes the key whole: a
        // second path below it replaces the first.
        add_assignment(&mut settings, &assign("html.heading_tags.subsection=h3"), 5).unwrap();
        let read = settings.read(&[&factory]).unwrap();
        let tags = &read.render.get::<HtmlRenderOptions>().unwrap().heading_tags;
        assert_eq!(tags.iter().map(|(name, tag)| (name.as_str(), tag.as_str())).collect::<Vec<_>>(), [("subsection", "h3")]);
    }

    #[test]
    fn an_assignment_that_is_not_one_is_refused() {
        let mut settings = Settings::new();
        assert!(add_assignment(&mut settings, &assign("text.width"), 1).is_err());
        assert!(add_assignment(&mut settings, &assign("width=3"), 1).is_err());
        assert!(add_assignment(&mut settings, &assign("text.width=["), 1).is_err());
    }
}
