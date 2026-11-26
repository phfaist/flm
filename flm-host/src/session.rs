//! The two operations of the host, *prepare* then *build*, and the session request that
//! drives them.
//!
//! A [`Request`], the session request, says what to build: the input, the profile (which
//! chooses the workflow) and the one of the workflow's outputs to produce, the packages used
//! before those the document lists
//! (the *initial list*), the math processor (the package that processes the document's
//! mathematics), the packages used after those the document lists, the settings given on
//! the command line, the top-level layout, how the output's files are named and where they
//! go, and whether to write the output when the document has errors. A *workflow* is a
//! named set of demands together with the choices that go with them
//! ([`WorkflowDef`](flm_core::core::registry::WorkflowDef)): the outputs it delivers and
//! what else it asks of the plan.
//!
//! [`prepare`] does the work that depends on the request and on the front matter but not on
//! the content: it reads the input's front matter ([`crate::seed`]) and the document's
//! metadata there, finds the packages ([`crate::packages`]: those of the initial list, then
//! the math processor's, then those the front matter lists under `use`, then those of the
//! request's [`uses`](Request::uses), then the profile's extension when it exists; and,
//! listing the search path, the values that the JSON Schemas suggest for an entry of
//! `use`, [`Prepared::suggestions`]), combines the settings
//! ([`crate::settings`]), builds the [registry](flm_core::core::registry) (with the chains
//! of the front matter's `feature_order`, each naming the file and the line where it is
//! written, besides the packages' own), checks that the profile's workflow is registered,
//! chooses its outputs, and builds the [plan](flm_core::core::plan::Plan). It returns a
//! [`Prepared`]. [`build`] reads the input again, parses its content, makes the document and
//! executes one [run](flm_core::core::run::Run) of the plan; when the request gives a sink,
//! the run's last stage gives the files of every output their paths and hands them to the
//! sink, all in one delivery. `build` prints
//! nothing: it returns a [`Built`], with the diagnostics, the list of the files read, and the
//! run, whose tables hold each output's artifact set. A caller that wants the output as
//! named files without a disk gives a [`MemorySink`](crate::output::MemorySink).
//!
//! An error that stops the work before the document is processed (an input that cannot be
//! read, a front matter that is not valid, an unknown package, a setting no feature
//! declares) is a [`PrepareError`]; a problem with the document's content is a diagnostic
//! of the run, in [`Built::diagnostics`]. A `PrepareError` also carries the diagnostics
//! found before the step that stopped ([`PrepareError::diagnostics`]), which a program
//! reports with its problems.
//!
//! # Progress and cancellation
//!
//! [`prepare`] and [`build`] each receive the [`Scope`] of the progress sheet
//! ([`flm_core::progress`]) under which that call reports; a [`Prepared`] keeps none, so
//! that each build reports on a sheet of its own. *Prepare* reports in one row with three
//! parts, *build* in one row for reading and parsing the input and one under which the run
//! reports. A caller that prepares and builds under one sheet declares a row of the kind
//! [`BUILD_KIND`] before it calls `prepare`, starts it when `prepare` has returned, gives its
//! scope to `build`, and finishes it when `build` returns; an estimate then counts the
//! build's work while the preparation runs. A call whose build is cancelled through the sheet stops at the next point where
//! it asks, and returns a [`PrepareError`] of kind [`Cancelled`](PrepareErrorKind::Cancelled)
//! with no problem, the diagnostics found so far and the paths read, as any attempt that
//! stops early.
//!
//! # Building again
//!
//! What *prepare* made serves every build as long as it is valid:
//! [`Prepared::is_current`] says whether it is, for the files as they are now. It is valid
//! while the input's front matter is the one *prepare* read, and every other file *prepare*
//! read (an extension's manifest, a folder of the search path, a file of an extension)
//! gives what it gave then. A change to the content below the front matter, or to a file
//! that only a build reads (an included file, an image, a bibliography file), needs a new
//! build only; any other change needs a new [`prepare`]. The costly results, such
//! as converted images, are kept in the caches of the request's manager
//! ([`Request::cache`]), which a new preparation keeps.
//!
//! An attempt that stops early also records what it read: a [`PrepareError`] lists every
//! path read up to that point, found or not, as a successful attempt would have. A caller
//! that builds again when a file changes therefore knows which files to watch, including a
//! manifest of an extension that the front matter lists and that does not exist yet.
//!
//! # Which profile runs, and its workflow
//!
//! A *profile* is the name that chooses what a request builds: the profile `NAME` runs the
//! workflow `NAME`, and may bring packages of its own, in an extension named `profile-NAME`
//! ([`PROFILE_PACKAGE_PREFIX`]). The profile is the request's
//! [`profile`](Request::profile); when the request gives none, the front matter's
//! `default_profile`; without it, `html` ([`DEFAULT_PROFILE`]).
//!
//! *Prepare* decides the profile before it finds the packages, and asks the package catalog
//! for the package `profile-NAME`, as for an entry of `use`: among the compiled packages,
//! then on the search path, then among the bundled extensions. The catalog answers in one
//! of three ways:
//!
//! - it finds the package: the package is used after the initial list, the math
//!   processor's package and the packages of `use`, as if the front matter listed it last
//!   under `use` (a package listed twice is used once);
//! - no compiled package, no folder of the search path and no bundled extension holds the
//!   name: nothing is added, and nothing is reported;
//! - it fails (a manifest that is not valid YAML, a folder holding both layouts of the
//!   extension, an extension that fails to load or provides no package of that name): the
//!   entry is added, and its resolution reports the failure as for an entry of `use`; a
//!   problem that stays at the entry opens with "in the profile `NAME`".
//!
//! Every lookup reads through the preparation's file reader, so the paths where the
//! extension `profile-NAME` would be are among the files read, found or not, and a preview
//! prepares again when the extension appears there.
//!
//! Once the registry is built, it must hold the workflow `NAME`, whichever package declares
//! it; an extension `profile-NAME` is expected to declare it. Otherwise *prepare* stops with
//! one of two problems: when the package `profile-NAME` was used, "the package
//! `profile-NAME` fails to define a workflow named `NAME`"; else "profile `NAME` not found;
//! ensure that the existing packages define a workflow named `NAME` or that an extension
//! named `profile-NAME` is in scope", followed by "; try perhaps" and the workflows the
//! registry holds, in its order, when it holds any. When the front matter's
//! `default_profile` gave the name, the problem is placed at the key's value. The front
//! matter's `default_profile` is looked for and checked only when it is the profile that
//! runs.
//!
//! The request's [`format`](Request::format) never chooses the profile or the workflow: it
//! selects one of the formats (the outputs) that the workflow that runs provides, and a
//! format that this workflow does not provide stops *prepare*. A request produces every
//! output of the workflow, or the one `format` selects.
//!
//! The messages about the profile and the workflow state the condition and refer to no
//! option of a program: a program that maps its options onto the request says in its own
//! help how they divide the work.

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, workflow_request};
use flm_core::core::registry::{
    BuildError, FlmRegistry, PackageCatalog, PackageFactory, RegistryBuilder, RegistryInputs, Site,
};
use flm_core::core::run::{Externals, Run, RunOptions, RunOutcome};
use flm_core::lang::{FlmDiagnostic, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::assemble::assemble;
use flm_core::model::doc::{Doc, FragmentLayout};
use flm_core::core::settings::PackageSettings;
use flm_core::model::names::{CapName, Name};
use flm_core::model::options::{OptionKey, OptionValues};
use flm_core::progress::{Kind, LogSink, Pending, Scope};
use flm_core::stdlib::features::metadata::DocMeta;

use crate::cache::ServiceCaches;
use crate::files::{BuildFiles, FileRead, Files, ServiceFiles};
use crate::inclusion::Inclusion;
use crate::report::ContentOffsets;
use crate::output::{Naming, OutputFolder, Sink};
use crate::extensions::ExtensionFinder;
use crate::packages::{EntrySource, ExtensionFolder, PackageProblem, catalog, loader, resolve, suggestions};
use crate::roots::Roots;
use flm_ext::{ExtensionLoader, LoadedBlock, Manifest};
use flm_ext::paths::ExtensionsByName;
use crate::seed::{
    MathChoice, Node, Place, Seed, read_front_matter, read_metadata, read_packages_seed, read_seed, split,
};
use crate::settings::{
    CommandLineSetting, FeatureInEffect, Origin, Reach, ReadSettings, SettingValue, Settings, SettingsProblem, Source,
    add_assignment, add_block, warning_diagnostic,
};
use flm_core::core::settings::cascade::workflow_def;
use flm_core::stdlib::features::compose::{self, ComposeOptions};
use crate::stdpkg::{self, StdParts};
use flm_core::stdlib::backends::pieces::PiecesRenderOptions;

/// The profile that runs when neither the request nor the front matter gives one: `html`.
///
/// A *profile* is the name that chooses what a request builds: the profile `NAME` runs the
/// workflow `NAME`, after using the extension `profile-NAME` ([`PROFILE_PACKAGE_PREFIX`])
/// when one exists. See "Which profile runs, and its workflow" in the [module
/// documentation](self).
pub const DEFAULT_PROFILE: &str = "html";

/// The prefix that a profile's name takes to give the name of the profile's package:
/// `profile-`, so that the profile `pdf-latex` uses the extension `profile-pdf-latex` when
/// one exists ([`Prepared::profile_package`]). It is the extension layer's
/// [`PROFILE_PREFIX`](flm_ext::adapter::names::PROFILE_PREFIX), by which an extension
/// `profile-NAME` may also name its output `NAME` and its other names `NAME.…`.
pub const PROFILE_PACKAGE_PREFIX: &str = flm_ext::adapter::names::PROFILE_PREFIX;

/// The name, `<input>`, under which a text given as the input appears in the positions
/// printed for it.
pub const TEXT_INPUT_NAME: &str = "<input>";

/// The kind of the row of the progress sheet under which [`prepare`] reports,
/// `flm.host.prepare`, titled "Preparing". Its parts are the rows [`PACKAGES_KIND`],
/// [`REGISTRY_KIND`] and [`PLAN_KIND`].
pub const PREPARE_KIND: Kind = Kind::titled("flm.host.prepare", "Preparing");

/// The kind of the row that stands for one call of [`build`], `flm.host.build`, with no
/// title. A caller that prepares and builds under one sheet declares this row before it
/// calls [`prepare`], starts it when `prepare` has returned, gives its scope to `build`, and
/// finishes it when `build` returns; an estimate then counts the build's work while the
/// preparation runs.
pub const BUILD_KIND: Kind = Kind::new("flm.host.build");

/// The kind of the part of [`prepare`] that finds the packages and loads the extensions,
/// `flm.host.packages`, titled "Loading packages".
pub const PACKAGES_KIND: Kind = Kind::titled("flm.host.packages", "Loading packages");

/// The kind of the part of [`prepare`] that combines the settings and builds the registry,
/// `flm.host.registry`.
pub const REGISTRY_KIND: Kind = Kind::new("flm.host.registry");

/// The kind of the part of [`prepare`] that chooses the workflow and its outputs and builds
/// the plan, `flm.host.plan`.
pub const PLAN_KIND: Kind = Kind::new("flm.host.plan");

/// The kind of the row of [`build`] that reads the input and parses its content,
/// `flm.host.parse`, titled "Reading document" and labeled with the input's file.
pub const PARSE_KIND: Kind = Kind::titled("flm.host.parse", "Reading document");

/// The kind of the row of [`build`] under which the run of the plan reports
/// ([`Run::with_progress`]), `flm.host.run`, titled "Processing document".
pub const RUN_KIND: Kind = Kind::titled("flm.host.run", "Processing document");

/// The input of a request: what a command processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The file at this path, relative to the current folder or absolute, written as the
    /// host writes paths ([`crate::files`]: `/` between names, on every system).
    File(String),
    /// A text the program read or was given (from standard input, or on the command line):
    /// it has no file name ([`TEXT_INPUT_NAME`] in messages), and paths written in it are
    /// resolved against the current folder.
    Text(String),
}

/// Where the host's default initial list comes from, as [`GivenList::source`] words it:
/// `the program's default list of packages`.
pub const DEFAULT_INITIAL_LIST: &str = "the program's default list of packages";

/// A list of packages that the request gives ([`Request::default_initial_use`],
/// [`Request::initial_use`], [`Request::uses`]): the packages, and the words that say where
/// the list comes from, which a problem of one of its entries opens with, since such an
/// entry is written in no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GivenList {
    /// The packages, each named as in a `use` entry, in order.
    pub packages: Vec<String>,
    /// Where the list comes from, worded so that a message can open with "in" and these
    /// words: for example "the option `--initial-use`", "the option `--use`", or
    /// [`DEFAULT_INITIAL_LIST`].
    pub source: String,
}

impl GivenList {
    /// Returns the list `packages`, which comes from `source`.
    pub fn new<S: Into<String>>(packages: impl IntoIterator<Item = S>, source: impl Into<String>) -> GivenList {
        GivenList { packages: packages.into_iter().map(Into::into).collect(), source: source.into() }
    }
}

/// Where the host's default math processor comes from ([`Request::default_math`]), in the
/// words that a problem with its package opens with: `the program's default choice of math
/// processor`.
pub const DEFAULT_MATH_PROCESSOR: &str = "the program's default choice of math processor";

/// A math processor that the request chooses on the command line ([`Request::math`]): the
/// choice, and the words that say where it comes from, which a problem with the processor's
/// package opens with, since such a choice is written in no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GivenMath {
    /// The processor, or no processor.
    pub choice: MathChoice,
    /// Where the choice comes from, worded so that a message can open with "in" and these
    /// words: for example "the option `--math`".
    pub source: String,
}

impl GivenMath {
    /// Returns the choice `choice`, which comes from `source`.
    pub fn new(choice: MathChoice, source: impl Into<String>) -> GivenMath {
        GivenMath { choice, source: source.into() }
    }
}

/// Where the value in effect of a key of `flm` itself comes from ([`FlmInEffect`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOrigin {
    /// A settings source, as for the features' settings ([`Origin`]): the program's defaults
    /// ([`Source::Defaults`]) for the request's defaults
    /// ([`Request::default_initial_use`], [`Request::default_math`]) and for
    /// [`DEFAULT_PROFILE`]; or the input's front matter ([`Source::FrontMatter`]), at the
    /// place of the value, for a list the place of its first entry (no place for an empty
    /// list).
    Source(Origin),
    /// The request apart from its defaults, in the words that say where the value comes
    /// from ([`GivenList::source`], [`GivenMath::source`]): for the `flm` program, an option
    /// of its command line.
    Given(String),
}

/// The value in effect of a key of `flm` itself, with where it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlmValue<T> {
    /// The value.
    pub value: T,
    /// Where it comes from.
    pub origin: KeyOrigin,
}

/// The values in effect of the keys of `flm` itself that decide the packages, and of
/// `default_profile`, each with where it comes from, as *prepare* merges them from the
/// request and the front matter ([`Prepared::flm_in_effect`]). The parsing keys are
/// combined with the settings ([`Prepared::parsing_in_effect`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlmInEffect {
    /// `initial_use`: the packages of the initial list, as the request's
    /// [`initial_use`](Request::initial_use), the front matter's `initial_use` or the
    /// request's [`default_initial_use`](Request::default_initial_use) gives them.
    pub initial_use: FlmValue<Vec<String>>,
    /// `use`: the entries of the front matter's `use`, then the packages of the request's
    /// [`uses`](Request::uses), which add to them, each with its origin.
    pub uses: Vec<FlmValue<String>>,
    /// `math`: the math processor, as the request's [`math`](Request::math), the front
    /// matter's `math` or the request's [`default_math`](Request::default_math) chooses it.
    pub math: FlmValue<MathChoice>,
    /// `default_profile`: the front matter's, else [`DEFAULT_PROFILE`]. The profile that runs
    /// is the request's [`profile`](Request::profile) when it gives one
    /// ([`Prepared::profile`]).
    pub default_profile: FlmValue<String>,
}

/// The session request: what to build, and where the output goes. See the [module
/// documentation](self).
///
/// [`Request::new`] gives a request for an input with every other field at its default;
/// the caller then sets the fields it needs.
#[derive(Clone)]
pub struct Request {
    /// The input.
    pub input: Input,
    /// The output format: the name of the one output of the workflow to produce (`html`,
    /// `text`, `latex-rendered`, `latex-recomposed`, or another output a package
    /// registers), which the workflow must provide; `None` produces every output of the
    /// workflow. It never chooses the profile or the workflow.
    pub format: Option<String>,
    /// The profile chosen by the caller: the profile `NAME` uses the extension
    /// `profile-NAME` when one exists, after the document's other packages, and runs the
    /// workflow `NAME`. `None` runs the front matter's `default_profile`, else
    /// [`DEFAULT_PROFILE`] (see "Which profile runs, and its workflow" in the [module
    /// documentation](self)).
    pub profile: Option<String>,
    /// The host's initial list: the packages used before those the document lists under
    /// `use`, each named as in a `use` entry, when neither
    /// [`initial_use`](Request::initial_use) nor the front matter's `initial_use` gives
    /// one. By default `std` ([`stdpkg::PACKAGE`]), the host's package that holds the
    /// library's standard features and the host's own, from [`DEFAULT_INITIAL_LIST`]; a
    /// host sets the list its documents start from. The list is used exactly as given: a
    /// package it does not name is not used unless the document lists it
    /// ([`crate::packages`]). The math processor is chosen apart from this list
    /// ([`default_math`](Request::default_math)).
    pub default_initial_use: GivenList,
    /// The initial list given on the command line (`--initial-use`); when given, it
    /// replaces the front matter's `initial_use` and
    /// [`default_initial_use`](Request::default_initial_use). `None` by default.
    pub initial_use: Option<GivenList>,
    /// The host's math processor, the package that processes the document's mathematics
    /// ([`MathChoice`]), when neither [`math`](Request::math) nor the front matter's `math`
    /// chooses one. Its package (`math-NAME` for the processor `NAME`) is used after the
    /// packages of the initial list and before those of `use`, whatever the initial list's
    /// source. [`MathChoice::NoProcessor`] by default: the library chooses no processor,
    /// and a host sets the one its documents use, for example
    /// `MathChoice::processor("passthrough")` for the library's package `math-passthrough`.
    /// A problem with its package opens with [`DEFAULT_MATH_PROCESSOR`].
    pub default_math: MathChoice,
    /// The math processor chosen on the command line (`--math`), with the words that say
    /// where it comes from; when given, it takes precedence over the front matter's `math`
    /// and [`default_math`](Request::default_math). `None` by default.
    pub math: Option<GivenMath>,
    /// The packages given on the command line (`--use`), each named as in a `use` entry,
    /// with the words that say where they come from: they are used after the packages that
    /// the front matter lists under `use`, as if written last in that list, and before the
    /// profile's package; they add to the front matter's `use`. `None` by default.
    pub uses: Option<GivenList>,
    /// The settings given on the command line, in the order written (for one key, the value
    /// written last wins), each an assignment `FEATURE.KEY=VALUE` with the words the user
    /// typed, which a problem with it quotes ([`CommandLineSetting`]). They have the highest
    /// rank among the settings sources ([`crate::settings`]).
    pub options: Vec<CommandLineSetting>,
    /// How the content is read at its top level; when given, it replaces the front matter's
    /// `layout`. Without either, the library's default applies.
    pub layout: Option<FragmentLayout>,
    /// How the artifacts of the outputs are named ([`Naming`]); by default, into a folder
    /// ([`Naming::Folder`]). Every file of the request shares one stem, the file name
    /// that [`Naming::File`] gives less the extension of one of the outputs, or `index`
    /// in a folder: see the [`output`](crate::output) module.
    pub naming: Naming,
    /// Where the output is written, from which the address of a file the output refers to
    /// instead of holding it (such as an image under the graphics strategy `reference`) is
    /// computed; by default the current folder.
    pub output_folder: OutputFolder,
    /// The sink that receives the output's files; `None` writes nothing, and the output is
    /// then read from the run's tables ([`Built::run`]).
    pub sink: Option<Arc<dyn Sink>>,
    /// Whether the output is written although the document has errors (the library's
    /// [`RunOptions::continue_with_errors`](flm_core::core::run::RunOptions::continue_with_errors));
    /// `false` by default.
    pub continue_with_errors: bool,
    /// The search path: the folders in which an extension is looked for by its locator, in
    /// order ([`crate::extensions`]); empty by default.
    pub extension_folders: Vec<ExtensionFolder>,
    /// The extensions that the program offers, compiled into it ([`Bundled`](crate::bundled::Bundled)):
    /// its table of files and, under the cargo feature `native`, its table of code. They are
    /// found by their stems after every folder of the search path ([`crate::bundled`]).
    /// Empty by default: the request offers no such extension.
    pub bundled: crate::bundled::Bundled,
    /// The collections that the program loaded (cargo feature `wasm`;
    /// `flm_ext::wasm::Collection`), each a WebAssembly module that holds several extensions
    /// whole. Their extensions are found by their stems after every folder of the search path
    /// and before the program's own tables, the collections in the order given
    /// ([`crate::bundled`]), and the engine `wasm` runs those that have code on the
    /// collection's module. Empty by default: the `flm` program loads none.
    #[cfg(feature = "wasm")]
    pub collections: Vec<flm_ext::wasm::Collection>,
    /// Where the `console` output of the extensions goes: [`loader`] gives this sink to
    /// each of its engines (the JavaScript and WebAssembly hosts, and the engine of the
    /// extensions compiled into the program). Each line is a log record whose source is the
    /// extension's name and whose level follows the `console` method: `error` and `warn`
    /// their own, `log`, `info` and `debug` `Info`. It is not the log sink of a build's
    /// progress sheet, since an extension also writes while it is loaded, outside any
    /// build; a host may give the same sink to both. `None` by default: the engines then
    /// keep the sink their hosts were given, and the default hosts have none, so the console
    /// output goes nowhere.
    pub log_sink: Option<Arc<dyn LogSink>>,
    /// The host of the JavaScript extensions the request's documents use (cargo feature
    /// `js`): the settings of their engine instances and the modules the program offers
    /// them; by default the default settings and no module. An extension's manifest may
    /// raise the limits for its own instances (`extension.js.limits`); the host's stack
    /// setting stays what bounds an instance created on a thread that is not a worker
    /// thread, such as the thread that loads the extension (`JsSettings::stack_size`).
    #[cfg(feature = "js")]
    pub js_host: flm_ext::quickjs::JsHost,
    /// The host of the extensions compiled to WebAssembly that the request's documents use
    /// (cargo feature `wasm`): the settings of their instances (the memory and the fuel of
    /// a handler call); by default the default settings. An extension's manifest may set the
    /// limits for its own instances (`extension.wasm.limits`).
    #[cfg(feature = "wasm")]
    pub wasm_host: flm_ext::wasm::WasmHost,
    /// Whether the extensions may run programs installed on the machine (cargo feature
    /// `fs`): when `true`, the loader of every preparation offers the host service
    /// `programs` ([`ProgramsService`](crate::host_programs::ProgramsService)) beside
    /// `files` and `cache`, which it always offers, and a package that declares it reaches
    /// it; when `false`, the default, the
    /// loader refuses such a package. The `flm` program sets it. A program that offers the
    /// service lets every extension a document uses run any program with the user's rights,
    /// the extensions of a document's folder `_flmextensions` included: see the module
    /// [`host_programs`](crate::host_programs). Like the program the converter `magick`
    /// runs, it is a property of the host, never a setting. The host process must ignore the
    /// signal `SIGPIPE`, as Rust programs do, or a program that exits without reading the
    /// standard input it is given ends the host.
    #[cfg(feature = "fs")]
    pub offer_programs: bool,
    /// The program that the converter `magick` runs to convert an image (cargo feature
    /// `graphics`; [`MagickConverter`](crate::graphics::MagickConverter)), by default
    /// [`MagickConverter::DEFAULT_PROGRAM`](crate::graphics::MagickConverter::DEFAULT_PROGRAM):
    /// a name the operating system looks for in its folders of programs, or a path, converted
    /// to the system's form when the program is started. The `flm` program takes it from
    /// the environment variable `MAGICK` when it is set. It is a property of the machine,
    /// given here as the output folder and the search path are, and never a setting, so
    /// that no document can choose a program for the host to start. A converter added later
    /// that runs a program gets its program the same way, from a field of its own.
    #[cfg(feature = "graphics")]
    pub magick_program: String,
    /// What the host's citation provider is given of the machine (cargo feature
    /// `citations`; [`CitationsHost`](crate::citations::CitationsHost)): the network and the
    /// folder of the cache file; by default neither. The `flm` program gives access to the
    /// internet and the folder of the document. Like the program the
    /// converter `magick` runs, these are properties of the machine, never settings.
    #[cfg(feature = "citations")]
    pub citations: crate::citations::CitationsHost,
    /// What downloads the file at an `https` address ([`Downloads`](crate::download::Downloads),
    /// over a [`Downloader`](crate::download::Downloader)); by default none, and a download
    /// is then refused. When it gives a downloader, the loader of every preparation offers
    /// the host service `download` ([`crate::host_download`]), and the reader of qualified
    /// paths reads the root `https` through it; the files are kept in the
    /// [`cache`](Request::cache), shared by documents. The `flm` program gives the downloader
    /// that connects to the internet (`download::network_downloader`, cargo feature
    /// `download`). Like the program the converter `magick` runs, it is a property of the
    /// host, never a setting. The citations do not use it: they fetch through
    /// the field `citations` alone.
    pub downloads: crate::download::Downloads,
    /// The caches ([`CacheHost`](crate::cache::CacheHost)): the manager that keeps the
    /// results of costly work, and the text that identifies the document among the
    /// documents that use the manager's root; by default a manager that keeps nothing and
    /// no document. [`prepare`] makes from it the handle of the registry's services
    /// ([`ServiceCaches`]), and each [`build`] attaches to that handle while it runs. Like
    /// the program the converter `magick` runs, it is a property of the host, never a
    /// setting.
    ///
    /// The host keeps these results in the manager's caches: the images that the converter
    /// `magick` converts ([`MagickConverter::CACHE`](crate::graphics::MagickConverter::CACHE),
    /// cargo feature `graphics`), the citation records of all documents
    /// ([`StandardProvider::CACHE`](crate::citations::StandardProvider::CACHE), cargo
    /// feature `citations`), the files downloaded from `https` addresses
    /// ([`download::CACHE`](crate::download::CACHE)), and the caches that extensions
    /// declare, which they reach through the host services `cache` ([`crate::host_cache`])
    /// and `programs` ([`crate::host_programs`]). Since the manager is part of the request, every
    /// preparation and every build made from the request uses the same caches.
    pub cache: crate::cache::CacheHost,
}

impl Request {
    /// Returns a request for `input` with every other field at its default: no output
    /// format and no profile given, the initial list `std` and none given on the command
    /// line, no math processor and none given on the command line, no package added on the
    /// command line, no setting, no sink (nothing is written), artifacts named into a
    /// folder, the current folder as the output folder, an empty search path, no extension
    /// compiled into the program and no collection, no log sink for the extensions' console, no host service
    /// `programs`, no downloader, and caches that keep nothing.
    pub fn new(input: Input) -> Request {
        Request {
            input,
            format: None,
            profile: None,
            default_initial_use: GivenList::new([stdpkg::PACKAGE], DEFAULT_INITIAL_LIST),
            initial_use: None,
            default_math: MathChoice::NoProcessor,
            math: None,
            uses: None,
            options: Vec::new(),
            layout: None,
            naming: Naming::Folder,
            output_folder: OutputFolder::default(),
            sink: None,
            continue_with_errors: false,
            extension_folders: Vec::new(),
            bundled: crate::bundled::Bundled::default(),
            #[cfg(feature = "wasm")]
            collections: Vec::new(),
            log_sink: None,
            #[cfg(feature = "js")]
            js_host: flm_ext::quickjs::JsHost::new(flm_ext::quickjs::JsSettings::default()),
            #[cfg(feature = "wasm")]
            wasm_host: flm_ext::wasm::WasmHost::new(flm_ext::wasm::WasmSettings::default()),
            #[cfg(feature = "fs")]
            offer_programs: false,
            #[cfg(feature = "graphics")]
            magick_program: crate::graphics::MagickConverter::DEFAULT_PROGRAM.into(),
            #[cfg(feature = "citations")]
            citations: crate::citations::CitationsHost::default(),
            downloads: crate::download::Downloads::default(),
            cache: crate::cache::CacheHost::default(),
        }
    }
}

/// A problem that stopped [`prepare`] or [`build`] before the document was processed: its
/// file and place, when known, and its message. [`crate::report::problem`] lays it out as
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The file the problem is in, when it is in one.
    pub file: Option<String>,
    /// The place in the file, when known.
    pub place: Option<Place>,
    /// The message, which states the condition that is not met.
    pub detail: String,
}

impl Problem {
    fn new(detail: impl Into<String>) -> Problem {
        Problem { file: None, place: None, detail: detail.into() }
    }

    /// Returns the problem `detail` about a setting written at `origin`: placed in the file
    /// when the origin is a file, and quoting the origin in its text otherwise.
    fn at_origin(origin: Option<&Origin>, detail: String) -> Problem {
        match origin {
            Some(Origin { source, place }) if source.file().is_some() => {
                Problem { file: source.file().map(str::to_string), place: *place, detail }
            }
            Some(origin) => Problem::new(format!("{detail} (in {origin})")),
            None => Problem::new(detail),
        }
    }
}

/// The step of [`prepare`] or [`build`] that a [`PrepareError`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrepareErrorKind {
    /// The input cannot be read, or its content cannot be assembled into a document.
    Input,
    /// The front matter is not closed, not valid YAML, not a map, holds an unknown key or a
    /// value of the wrong type, or holds metadata the library refuses.
    FrontMatter,
    /// A package cannot be found or read, or a required package folder does not exist.
    Packages,
    /// A problem with a setting: a command-line assignment that cannot be read, settings
    /// for a feature that takes no settings, a key that no feature declares
    /// or a value its declaration cannot read, a key in a workflow's settings that is not set
    /// per render request, a parsing setting of the wrong type, a path setting that has no
    /// meaning where it is written (`self:` in the front matter), or two packages that
    /// disagree.
    Settings,
    /// The registry cannot be built.
    Registry,
    /// The workflow of the profile that runs is not registered, or that workflow does not
    /// provide the requested format.
    Workflow,
    /// No compose template of the chosen name serves the output.
    Template,
    /// The plan cannot be built, or the feature `write` is switched off while the request
    /// gives a sink.
    Plan,
    /// The run of a built plan could not be made or failed: a failure of the program, not
    /// a problem with the document.
    Run,
    /// The build was asked to stop through the progress sheet that the call reports on
    /// ([`Sheet::cancel`](flm_core::progress::Sheet::cancel)). It is not a problem with the
    /// document, and the error holds no problem.
    Cancelled,
}

/// The error of [`prepare`] or [`build`] when the work cannot start: the step that failed,
/// every problem found there, the diagnostics found before it, and the paths the attempt
/// read before it stopped. A program prints each diagnostic with
/// [`crate::report::diagnostic`], then each problem with [`crate::report::problem`].
#[derive(Debug, Clone)]
pub struct PrepareError {
    /// The step.
    pub kind: PrepareErrorKind,
    /// The problems, each placed where it is known.
    pub problems: Vec<Problem>,
    /// The diagnostics found before the step stopped that do not stop the work, in the
    /// order found.
    pub diagnostics: Vec<FlmDiagnostic>,
    /// Every path the attempt read before it stopped, in the order read, found or not: the
    /// list a successful attempt would have given up to that point. For [`build`],
    /// *prepare*'s reads ([`Prepared::files_read`]) come first, then the build's. A path
    /// where nothing was found is listed too, so that a caller watching the files sees a
    /// file appear there (the manifest of an extension listed in the front matter and not written
    /// yet, for example).
    pub files_read: Vec<FileRead>,
}

impl PrepareError {
    /// Returns the error of the step `kind` with `problems`, the diagnostics found before it
    /// and the paths read not filled in yet: [`prepare`] and [`build`] fill them in before
    /// they return.
    fn new(kind: PrepareErrorKind, problems: Vec<Problem>) -> PrepareError {
        PrepareError { kind, problems, diagnostics: Vec::new(), files_read: Vec::new() }
    }

    fn one(kind: PrepareErrorKind, problem: Problem) -> PrepareError {
        PrepareError::new(kind, vec![problem])
    }
}

/// The input as read: its name and its text.
#[derive(Debug, Clone)]
struct ReadInput {
    /// The file's name as the author wrote it, or `None` for a text.
    name: Option<String>,
    text: String,
}

/// Reads `input` through `files`.
fn read_input(input: &Input, files: &BuildFiles) -> Result<ReadInput, PrepareError> {
    match input {
        Input::Text(text) => Ok(ReadInput { name: None, text: text.clone() }),
        Input::File(path) => {
            let text = files.read_text("", path).map_err(|error| {
                PrepareError::one(
                    PrepareErrorKind::Input,
                    Problem { file: Some(path.clone()), place: None, detail: error.to_string() },
                )
            })?;
            Ok(ReadInput { name: Some(path.clone()), text })
        }
    }
}

/// The result of [`prepare`]: the registry, the plan and everything else [`build`] needs,
/// which serves any number of builds while [`is_current`](Prepared::is_current) returns
/// `true`.
#[derive(Clone)]
pub struct Prepared {
    request: Request,
    files: Arc<dyn Files>,
    service_files: ServiceFiles,
    service_caches: ServiceCaches,
    /// The reader of qualified paths, over the files of the extensions loaded, which the
    /// tests read; the readers of the preparation hold their own clones.
    #[cfg(test)]
    roots: Roots,
    seed: Seed,
    registry: Arc<FlmRegistry>,
    plan: Arc<Plan>,
    profile: Profile,
    workflow: String,
    outputs: Vec<String>,
    packages: Vec<String>,
    factories: Vec<PackageFactory>,
    suggestions: Vec<String>,
    /// The manifest of each extension whose packages are in use, by the extension's identity.
    manifests: BTreeMap<String, Manifest>,
    /// The description of the manifest of each extension in use that has one, by each
    /// locator that found the extension.
    descriptions: BTreeMap<String, String>,
    /// The packages in use, each named as an entry of `use` names it ([`names_in_use`]).
    names_in_use: Vec<String>,
    /// The keys of `flm` itself in effect, with their origins.
    flm: FlmInEffect,
    /// The settings combined from every source, as last read.
    settings: Settings,
    /// The settings each package was built from, by the name of its factory.
    package_settings: BTreeMap<Name, PackageSettings>,
    document_options: OptionValues,
    layout: FragmentLayout,
    /// *Prepare*'s file reader, which holds the paths it read.
    reading: BuildFiles,
    front_matter: Option<String>,
    diagnostics: Vec<FlmDiagnostic>,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("input", &self.request.input)
            .field("profile", &self.profile.name)
            .field("outputs", &self.outputs)
            .field("packages", &self.packages)
            .finish_non_exhaustive()
    }
}

impl Prepared {
    /// Returns whether what *prepare* made is still valid for the files as they are now:
    /// the input's front matter is the text *prepare* read, and every other path *prepare*
    /// looked for gives the same result (the same bytes, the same entries of a folder,
    /// nothing where nothing was). See "Building again" in the [module
    /// documentation](self). It reads each of those paths again, without recording them on
    /// any list of files read.
    pub fn is_current(&self) -> bool {
        let primary = match &self.request.input {
            Input::File(path) => Some(BuildFiles::resolve("", path)),
            Input::Text(_) => None,
        };
        for path in self.reading.changed() {
            if Some(&path) != primary.as_ref() {
                return false;
            }
            let Ok(bytes) = self.files.read(&path) else { return false };
            let Ok(text) = String::from_utf8(bytes) else { return false };
            let Ok(split) = split(&text) else { return false };
            if split.front_matter != self.front_matter.as_deref() {
                return false;
            }
        }
        true
    }

    /// Returns the registry *prepare* built.
    pub fn registry(&self) -> &Arc<FlmRegistry> {
        &self.registry
    }

    /// Returns the plan *prepare* built.
    pub fn plan(&self) -> &Arc<Plan> {
        &self.plan
    }

    /// Sets whether the builds over this preparation write their output through the
    /// document's errors ([`Request::continue_with_errors`]): the preview's exports choose it
    /// per export, over one preparation.
    #[cfg(feature = "watch")]
    pub(crate) fn set_continue_with_errors(&mut self, continue_with_errors: bool) {
        self.request.continue_with_errors = continue_with_errors;
    }

    /// Returns the name of the profile that runs: the request's, else the front matter's
    /// `default_profile`, else [`DEFAULT_PROFILE`].
    pub fn profile(&self) -> &str {
        &self.profile.name
    }

    /// Returns the name of the profile's package, `profile-NAME`, when the package catalog
    /// found it and it is among the packages in use ([`packages`](Prepared::packages)); `None`
    /// when no package of that name exists.
    pub fn profile_package(&self) -> Option<&str> {
        self.profile.package.as_deref()
    }

    /// Returns the profiles this document can run, each once, in name order: the name of
    /// each workflow of the registry ([`FlmRegistry::workflows`]), and `NAME` for each
    /// package `profile-NAME` ([`PROFILE_PACKAGE_PREFIX`]) among the packages found when the
    /// document was prepared ([`suggestions`](Prepared::suggestions): the compiled packages,
    /// the extensions on the search path and the bundled extensions).
    ///
    /// A profile of the second kind is listed before its package is loaded, so a request
    /// for it can still fail to prepare (its extension does not load, or does not declare
    /// the workflow `NAME`). The profile that runs ([`profile`](Prepared::profile)) is always
    /// listed, its workflow being in the registry. The live preview offers these profiles
    /// as the *build types* of the document.
    pub fn profiles(&self) -> Vec<String> {
        let workflows = self.registry.workflows().map(|entry| entry.def.name.to_string());
        let packages = self
            .suggestions
            .iter()
            .filter_map(|name| name.strip_prefix(PROFILE_PACKAGE_PREFIX))
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        let mut profiles: Vec<String> = workflows.chain(packages).collect();
        profiles.sort();
        profiles.dedup();
        profiles
    }

    /// Returns the name of the workflow that runs: the profile's name.
    pub fn workflow(&self) -> &str {
        &self.workflow
    }

    /// Returns the names of the outputs the request produces, in the workflow's order:
    /// every output of the workflow, or the one the request's
    /// [`format`](Request::format) names. The first is the *leading output*, the one a
    /// caller shows or opens. A workflow that delivers no output (`check`) gives an empty
    /// list.
    pub fn outputs(&self) -> &[String] {
        &self.outputs
    }

    /// Returns the names of the packages in use, each after the packages it uses: those of
    /// the initial list, then the math processor's, then those the front matter lists under
    /// `use`, then those of the request's [`uses`](Request::uses), then the profile's
    /// package when it exists; the name of a package of an extension is
    /// `<identity>#<package>`, the identity being the path of the extension's manifest.
    pub fn packages(&self) -> &[String] {
        &self.packages
    }

    /// Returns the package factories the document resolved, each after the packages it
    /// uses: what the preview builds the schema of the document's settings from.
    pub fn factories(&self) -> &[PackageFactory] {
        &self.factories
    }

    /// Returns the values that the JSON Schemas suggest for an entry of `use` and of
    /// `initial_use`, listed on the request's search path when the packages were found
    /// ([`crate::packages::suggestions`]): what the preview's schema of the document's
    /// settings suggests.
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    /// Returns the manifest of the extension `identity` when one of its packages is in use,
    /// as the extension loader read it ([`ExtensionLoader::manifest`]): its description, its
    /// author and its engine, among the rest. The identity is what precedes `#` in the
    /// package's name, as [`packages`](Prepared::packages) gives it. `None` for an identity
    /// of no extension in use.
    ///
    /// The manifest's engine is the one that runs the extension. For an extension written in
    /// Rust and compiled into the program (`bundled:rust/STEM`), it is `native`, although the
    /// manifest's file, that of the extension's WebAssembly form, specifies `wasm`.
    pub fn manifest(&self, identity: &str) -> Option<&Manifest> {
        self.manifests.get(identity)
    }

    /// Returns the description of the manifest of each extension in use that has one, by
    /// each locator that found the extension, as written (`notes`, `@acme/notes`): the
    /// descriptions that *prepare* read, which loads no extension beyond those in use.
    pub fn extension_descriptions(&self) -> &BTreeMap<String, String> {
        &self.descriptions
    }

    /// Returns the packages in use ([`packages`](Prepared::packages)), each named as an entry
    /// of `use` names it, each name once, in their order: a package compiled into the program
    /// by its name, and a package of an extension by each locator through which the
    /// extension was found (`notes`, `@acme/notes`, a path). The packages that another
    /// package uses are among them.
    pub fn names_in_use(&self) -> &[String] {
        &self.names_in_use
    }

    /// Returns the values in effect of the keys of `flm` itself that decide the packages,
    /// and of `default_profile`, each with where it comes from ([`FlmInEffect`]): what
    /// *prepare* used, merged from the request and the front matter.
    pub fn flm_in_effect(&self) -> &FlmInEffect {
        &self.flm
    }

    /// Returns the value in effect of each parsing key that a settings source wrote, by the
    /// key, with where it was written: the values that *prepare* gave the registry, combined
    /// by the rule of the ranks
    /// ([`Cascade::parsing_in_effect`](flm_core::core::settings::cascade::Cascade::parsing_in_effect)).
    /// A key that no source wrote is absent; its default applies.
    pub fn parsing_in_effect(&self) -> BTreeMap<String, SettingValue> {
        self.settings.parsing_in_effect()
    }

    /// Returns the input's front matter as read ([`Seed`]).
    pub fn seed(&self) -> &Seed {
        &self.seed
    }

    /// Returns the text of the input's front matter as *prepare* read it, without its two
    /// `---` lines; `None` when the input has no front matter.
    pub fn front_matter_text(&self) -> Option<&str> {
        self.front_matter.as_deref()
    }

    /// Returns the settings *prepare* combined ([`Settings`]): every value that each settings
    /// source wrote (the program's defaults, the packages of the extensions, the front
    /// matter, the settings a package gives the workflow that runs, the command line), each
    /// with where it was written ([`Origin`]). See [`crate::settings`] for the sources and
    /// the rule that combines them. [`settings_in_effect`](Prepared::settings_in_effect)
    /// lists the resulting value of every key written, with where it was written.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Returns the settings in effect for the packages in use, by the feature's name: for
    /// each feature of these packages that a settings source wrote, whether it is switched
    /// off and where, and the value in effect of each key written, combined from every source
    /// by the rule of the ranks, with the [`Origin`] of the value and, for a key merged entry
    /// by entry, of each entry ([`FeatureInEffect`]). A key that no source wrote is absent;
    /// the feature's own default applies to it.
    ///
    /// These are the combined settings ([`settings`](Prepared::settings)), the settings a
    /// package gives the workflow that runs included, as
    /// [`Cascade::in_effect`](flm_core::core::settings::cascade::Cascade::in_effect) gives
    /// them for the [`factories`](Prepared::factories): the values that *prepare* read
    /// through the packages' settings declarations. Each call computes them anew.
    pub fn settings_in_effect(&self) -> BTreeMap<String, FeatureInEffect> {
        let factories: Vec<&PackageFactory> = self.factories.iter().collect();
        self.settings.in_effect(&factories)
    }

    /// Returns the settings that each package in use was built from, by the name of the
    /// package's factory ([`packages`](Prepared::packages)): the settings read through the
    /// packages' settings declarations ([`ReadSettings::packages`]), before the settings a
    /// package gives the workflow, which set only option values of the render request.
    pub fn package_settings(&self) -> &BTreeMap<Name, PackageSettings> {
        &self.package_settings
    }

    /// Returns the option values that [`build`] sets on the document
    /// ([`Doc::with_options`]): the value of each option type that may be set on the document
    /// and of which the settings write at least one key, and the document's metadata read
    /// from the front matter when a feature of the registry declares it.
    pub fn document_options(&self) -> &OptionValues {
        &self.document_options
    }

    /// Returns how [`build`] reads the content at its top level: the request's
    /// [`layout`](Request::layout), else the front matter's `layout`, else the library's
    /// default.
    pub fn layout(&self) -> FragmentLayout {
        self.layout
    }

    /// Returns the request's search path: the folders in which an extension is looked for
    /// by its locator, in the order searched, as the request gives them
    /// ([`Request::extension_folders`]). The values found there are among the
    /// [`suggestions`](Prepared::suggestions).
    pub fn search_path(&self) -> &[ExtensionFolder] {
        &self.request.extension_folders
    }

    /// Returns the paths *prepare* looked for, in the order read, found or not.
    pub fn files_read(&self) -> Vec<FileRead> {
        self.reading.list()
    }

    /// Returns the diagnostics *prepare* found that do not stop the work: the settings'
    /// warnings ([`UnknownFeature`](crate::settings::UnknownFeature)) and those of the
    /// registry build.
    pub fn diagnostics(&self) -> &[FlmDiagnostic] {
        &self.diagnostics
    }
}

/// Prepares the work that `request` asks for, reading files through `files`; `compiled` are
/// the compiled packages the program carries, besides `std` and `math-passthrough`. See
/// the [module documentation](self).
///
/// It reports under `progress` in one row [`PREPARE_KIND`], whose three parts are declared
/// when it starts: [`PACKAGES_KIND`], [`REGISTRY_KIND`] and [`PLAN_KIND`]. Between two
/// parts it asks whether the build is cancelled. A caller that shows no progress passes
/// [`Scope::none()`].
///
/// # Errors
///
/// A [`PrepareError`] giving the step that failed, every problem found there, the
/// diagnostics found before that step, and the paths read up to that point; of kind
/// [`PrepareErrorKind::Cancelled`], with no problem, when the build was cancelled.
pub fn prepare(
    request: Request,
    files: Arc<dyn Files>,
    compiled: &[PackageFactory],
    progress: &Scope,
) -> Result<Prepared, PrepareError> {
    let reads = BuildFiles::new(Arc::clone(&files));
    let mut diagnostics = Vec::new();
    let row = progress.child(&PREPARE_KIND, None).start();
    let scope = row.scope();
    let parts = Parts {
        scope: &scope,
        packages: scope.child(&PACKAGES_KIND, None),
        registry: scope.child(&REGISTRY_KIND, None),
        plan: scope.child(&PLAN_KIND, None),
    };
    let prepared = prepare_reading(request, files, compiled, reads.clone(), &mut diagnostics, parts)
        .map_err(|error| PrepareError { diagnostics, files_read: reads.list(), ..error })?;
    row.finish();
    Ok(prepared)
}

/// The rows of the three parts of [`prepare`], declared when its row starts, and the scope
/// of that row, through which it asks whether the build is cancelled between two parts.
struct Parts<'a> {
    scope: &'a Scope,
    packages: Pending,
    registry: Pending,
    plan: Pending,
}

/// The error of an attempt that stops because the build was cancelled: of kind
/// [`PrepareErrorKind::Cancelled`], with no problem.
fn cancelled() -> PrepareError {
    PrepareError::new(PrepareErrorKind::Cancelled, Vec::new())
}

/// What [`document_packages`] finds for a document: its packages, what the JSON Schemas
/// suggest for an entry of `use` on its search path, and the descriptions of the extensions
/// in use.
#[derive(Debug, Clone)]
pub struct DocumentPackages {
    /// The package factories the document uses: the packages of the initial list, then the
    /// math processor's package, then each package listed under `use`, then those of the
    /// request's [`uses`](Request::uses), then the profile's package when it exists, each
    /// after the packages it uses.
    pub factories: Vec<PackageFactory>,
    /// The values that the JSON Schemas suggest for an entry of `use` and of `initial_use`,
    /// listed on the request's search path ([`crate::packages::suggestions`]), as
    /// [`Prepared::suggestions`] gives them.
    pub suggestions: Vec<String>,
    /// The description of the manifest of each extension in use that has one, by each
    /// locator that found the extension, as [`Prepared::extension_descriptions`] gives them.
    pub descriptions: BTreeMap<String, String>,
    /// The packages in use, each named as an entry of `use` names it, as
    /// [`Prepared::names_in_use`] gives them.
    pub names_in_use: Vec<String>,
}

/// The package factories the document of `request` uses, found as [`prepare`] finds them,
/// with the values that the JSON Schemas suggest for an entry of `use`
/// ([`DocumentPackages`]).
///
/// Of the front matter, it reads only the keys that decide the packages: `use`,
/// `initial_use`, `math` and `default_profile` ([`read_packages_seed`]). A problem
/// elsewhere in the front matter (in the metadata, in a setting, an unknown key under
/// `flm`) does not stop it, so that the packages of a document whose front matter has such
/// a problem are still found; [`prepare`] reports that problem. It reads no settings.
///
/// # Errors
///
/// The problems of reading the input, of splitting off its front matter and reading it as
/// YAML, of the four keys, and of finding the packages, as [`prepare`] reports them, with
/// the paths read up to that point.
pub fn document_packages(
    request: Request,
    files: Arc<dyn Files>,
    compiled: &[PackageFactory],
) -> Result<DocumentPackages, PrepareError> {
    let reads = BuildFiles::new(Arc::clone(&files));
    find_packages(&request, &files, compiled, &reads, Reading::Packages)
        .map(|found| {
            let blocks = blocks_in_use(&found.factories, &found.loader);
            let manifests = manifests(&blocks, &found.loader);
            let descriptions = descriptions(&found.finder, &manifests);
            let names_in_use = names_in_use(&found.factories, &blocks, &found.finder);
            DocumentPackages { factories: found.factories, suggestions: found.suggestions, descriptions, names_in_use }
        })
        .map_err(|error| PrepareError { files_read: reads.list(), ..error })
}

/// What the first part of *prepare* found: the input, its front matter and metadata, the
/// profile, the files of the services, the packages, and the values the schemas suggest for
/// `use`.
struct Found {
    input: ReadInput,
    seed: Seed,
    profile: Profile,
    meta: DocMeta,
    service_files: ServiceFiles,
    service_caches: ServiceCaches,
    /// The reader of qualified paths, kept for the tests ([`Prepared`]).
    #[cfg(test)]
    roots: Roots,
    finder: Arc<ExtensionFinder>,
    loader: Arc<ExtensionLoader>,
    catalog: PackageCatalog,
    factories: Vec<PackageFactory>,
    suggestions: Vec<String>,
    flm: FlmInEffect,
}

/// How [`find_packages`] reads the front matter of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// The whole front matter, as [`prepare`] reads it: every problem in it stops the work.
    Whole,
    /// Only the keys that decide the packages, as [`document_packages`] reads it
    /// ([`read_packages_seed`]); the metadata is not read.
    Packages,
}

/// The first part of [`prepare`], which [`document_packages`] is too: reads the input and
/// its front matter (as `reading` says), makes `std`, the finder of extensions, the
/// loader and the catalog, decides the profile ([`profile_name`]) and looks for its
/// package, resolves the packages of the initial list ([`initial_list`]), the math
/// processor's package ([`math_entry`]), the packages the front matter lists under `use`,
/// those of the request's
/// [`uses`](Request::uses) and the profile's package when the catalog knows it, then lists
/// the values the schemas suggest for an entry of `use`
/// ([`suggestions`](crate::packages::suggestions)) on the request's search path.
fn find_packages(
    request: &Request,
    files: &Arc<dyn Files>,
    compiled: &[PackageFactory],
    reads: &BuildFiles,
    reading: Reading,
) -> Result<Found, PrepareError> {
    let input = read_input(&request.input, reads)?;
    let seed = read_input_seed(&input, reading)?;
    let file = input.name.clone().unwrap_or_else(|| String::from(TEXT_INPUT_NAME));
    let meta = match reading {
        Reading::Whole => read_metadata(&seed.metadata, &file).map_err(|errors| {
            PrepareError::new(
                PrepareErrorKind::FrontMatter,
                errors
                    .into_iter()
                    .map(|error| Problem { file: input.name.clone(), place: error.place, detail: error.detail })
                    .collect(),
            )
        })?,
        Reading::Packages => DocMeta::default(),
    };
    let service_files = ServiceFiles::new(Arc::clone(files));
    let service_caches = ServiceCaches::new(request.cache.clone());
    // The document's folder, for the root `doc` of the reader of qualified paths: the folder
    // of the input file, the current folder (the empty path) for a text.
    let document = match &request.input {
        Input::File(path) => BuildFiles::folder_of(path),
        Input::Text(_) => "",
    };
    // The reader of qualified paths, made before the loader, which fills its table of the
    // extensions by use name as it loads them.
    let std_files = Arc::new(crate::stdpkg::compose_standard::templates::std_files());
    let roots = Roots::new(ExtensionsByName::new(), std_files, service_files.clone(), document.to_string())
        .with_downloads(request.downloads.clone(), service_caches.clone());
    let parts = StdParts {
        namer: Arc::new(request.naming.clone()),
        sink: request.sink.clone(),
        files: service_files.clone(),
        roots: roots.clone(),
        output_folder: request.output_folder.clone(),
        #[cfg(feature = "graphics")]
        magick_program: request.magick_program.clone(),
        #[cfg(feature = "citations")]
        citations: request.citations.clone(),
        cache: service_caches.clone(),
    };
    let std = stdpkg::factory(parts).map_err(|error| {
        PrepareError::one(PrepareErrorKind::Packages, Problem::new(error.to_string()))
    })?;
    let finder = Arc::new(ExtensionFinder::new(reads.clone(), service_files.clone(), request.extension_folders.clone()));
    let packages_problem = |problems: Vec<PackageProblem>| {
        PrepareError::new(
            PrepareErrorKind::Packages,
            problems
                .into_iter()
                .map(|problem| Problem { file: problem.file, place: problem.place, detail: problem.detail })
                .collect(),
        )
    };
    let missing = finder.missing_folders();
    if !missing.is_empty() {
        return Err(packages_problem(missing));
    }
    let loader = loader(&finder, request, &service_caches, &roots);
    let catalog = catalog(std, compiled, Arc::clone(&loader))
        .map_err(|error| PrepareError::one(PrepareErrorKind::Packages, Problem::new(error.to_string())))?;
    // The profile's package comes last, unless no compiled package, no folder of the search
    // path and no bundled extension holds its name: that absence adds nothing and is no
    // problem, since the packages already listed may declare the profile's workflow. A lookup
    // that fails adds the entry, whose resolution below reports the failure as an entry's of
    // `use` is reported, so that an extension that exists but is broken is never skipped.
    let (name, place) = profile_name(request, &seed);
    let package = format!("{PROFILE_PACKAGE_PREFIX}{name}");
    let known = !matches!(catalog.resolve(&package), Ok(None));
    let profile_entry = known.then(|| (package.clone(), EntrySource::Given(format!("the profile `{name}`"))));
    let profile = Profile { name, place, package: known.then_some(package) };
    // The keys of `flm` are merged here, each value with its origin, which the entries below
    // are made from.
    let written = |place: Option<Place>| KeyOrigin::Source(Origin { source: Source::FrontMatter(file.clone()), place });
    let (initial_entries, initial_use) = initial_list(request, &seed, &written);
    let (math_entry, math) = math_entry(request, &seed, &written);
    let uses: Vec<FlmValue<String>> = seed
        .uses
        .iter()
        .map(|(name, place)| FlmValue { value: name.clone(), origin: written(Some(*place)) })
        .chain(request.uses.iter().flat_map(|list| {
            list.packages.iter().map(|name| FlmValue { value: name.clone(), origin: KeyOrigin::Given(list.source.clone()) })
        }))
        .collect();
    let default_profile = match &seed.default_profile {
        Some(written_profile) => FlmValue { value: written_profile.name.clone(), origin: written(Some(written_profile.place)) },
        None => FlmValue { value: String::from(DEFAULT_PROFILE), origin: program_default() },
    };
    // The request's packages follow those of `use`, as if written last there, and come
    // before the profile's package: the request adds an optional package to a document as
    // one more entry of its front matter's `use` would.
    let named: Vec<(String, EntrySource)> = initial_entries
        .into_iter()
        .chain(math_entry)
        .chain(seed.uses.iter().map(|(name, place)| (name.clone(), EntrySource::File(*place))))
        .chain(request.uses.iter().flat_map(given_entries))
        .chain(profile_entry)
        .collect();
    let factories = resolve(&catalog, &loader, &finder, &named, input.name.as_deref()).map_err(packages_problem)?;
    let suggestions = suggestions(catalog.compiled_names().map(|name| name.as_str()), &finder, &request.bundled);
    let flm = FlmInEffect { initial_use, uses, math, default_profile };
    Ok(Found {
        input,
        seed,
        profile,
        meta,
        service_files,
        service_caches,
        #[cfg(test)]
        roots,
        finder,
        loader,
        catalog,
        factories,
        suggestions,
        flm,
    })
}

/// The origin of a value of the program's defaults, for a key of `flm` itself.
fn program_default() -> KeyOrigin {
    KeyOrigin::Source(Origin { source: Source::Defaults, place: None })
}

/// The profile that a request runs, as *prepare* decides it: see "Which profile runs, and
/// its workflow" in the [module documentation](self).
#[derive(Debug, Clone)]
struct Profile {
    /// The profile's name, which is the name of the workflow that runs.
    name: String,
    /// The place of the value of the front matter's `default_profile`, when it gives the
    /// name.
    place: Option<Place>,
    /// The package `profile-NAME`, when the package catalog knows that name, and the
    /// package is then among the packages resolved.
    package: Option<String>,
}

/// The name of the profile that `request` runs for the document whose front matter is
/// `seed`: the request's [`profile`](Request::profile), else the front matter's
/// `default_profile`, with the place of its value, else [`DEFAULT_PROFILE`].
fn profile_name(request: &Request, seed: &Seed) -> (String, Option<Place>) {
    match (&request.profile, &seed.default_profile) {
        (Some(name), _) => (name.clone(), None),
        (None, Some(written)) => (written.name.clone(), Some(written.place)),
        (None, None) => (String::from(DEFAULT_PROFILE), None),
    }
}

/// The initial list of the document whose front matter is `seed`: the request's
/// [`initial_use`](Request::initial_use) (the command line's), else the front matter's
/// `initial_use`, else the host's [`default_initial_use`](Request::default_initial_use);
/// each entry with its place in the input's file when the front matter writes it, else with
/// the words that say where its list comes from ([`GivenList::source`]). Also the list as
/// the value in effect of `initial_use`, with its origin; `written` gives the origin of a
/// place of the front matter.
fn initial_list(
    request: &Request,
    seed: &Seed,
    written: &dyn Fn(Option<Place>) -> KeyOrigin,
) -> (Vec<(String, EntrySource)>, FlmValue<Vec<String>>) {
    let in_effect = |list: &GivenList, origin: KeyOrigin| FlmValue { value: list.packages.clone(), origin };
    match (&request.initial_use, &seed.initial_use) {
        (Some(list), _) => (given_entries(list).collect(), in_effect(list, KeyOrigin::Given(list.source.clone()))),
        (None, Some(entries)) => (
            entries.iter().map(|(name, place)| (name.clone(), EntrySource::File(*place))).collect(),
            FlmValue {
                value: entries.iter().map(|(name, _)| name.clone()).collect(),
                origin: written(entries.first().map(|(_, place)| *place)),
            },
        ),
        (None, None) => {
            let list = &request.default_initial_use;
            (given_entries(list).collect(), in_effect(list, program_default()))
        }
    }
}

/// The entries of `list`, a list that the request gives, each with the words that say where
/// the list comes from ([`GivenList::source`]).
fn given_entries(list: &GivenList) -> impl Iterator<Item = (String, EntrySource)> + '_ {
    list.packages.iter().map(|name| (name.clone(), EntrySource::Given(list.source.clone())))
}

/// The entry of the math processor's package for the document whose front matter is
/// `seed`, which comes after the initial list's entries: the processor that the request's
/// [`math`](Request::math) (the command line's) chooses, else the front matter's `math`,
/// else the host's [`default_math`](Request::default_math); its place in the input's file
/// when the front matter chooses it, else the words that say where the choice comes from;
/// `None` when the choice is no processor. Also the choice as the value in effect of
/// `math`, with its origin; `written` gives the origin of a place of the front matter.
fn math_entry(
    request: &Request,
    seed: &Seed,
    written: &dyn Fn(Option<Place>) -> KeyOrigin,
) -> (Option<(String, EntrySource)>, FlmValue<MathChoice>) {
    // The processor is chosen apart from the initial list, so that two settings that read as
    // independent do not act on each other: a list that the front matter or the request
    // writes does not change it, and the processor's package follows that list whatever its
    // source.
    let (choice, source, origin) = match (&request.math, &seed.math) {
        (Some(given), _) => {
            (&given.choice, EntrySource::Given(given.source.clone()), KeyOrigin::Given(given.source.clone()))
        }
        (None, Some((chosen, place))) => (chosen, EntrySource::File(*place), written(Some(*place))),
        (None, None) => {
            (&request.default_math, EntrySource::Given(String::from(DEFAULT_MATH_PROCESSOR)), program_default())
        }
    };
    (choice.package().map(|package| (package, source)), FlmValue { value: choice.clone(), origin })
}

/// [`prepare`], reading through `reads`; `diagnostics` receives the diagnostics found that
/// do not stop the work, as they are found (the settings' warnings, then the registry's
/// build diagnostics), which [`prepare`] adds to the error when a later step stops.
fn prepare_reading(
    request: Request,
    files: Arc<dyn Files>,
    compiled: &[PackageFactory],
    reads: BuildFiles,
    diagnostics: &mut Vec<FlmDiagnostic>,
    parts: Parts<'_>,
) -> Result<Prepared, PrepareError> {
    let Parts { scope, packages, registry: registry_row, plan: plan_row } = parts;
    let part = packages.start();
    let Found {
        input,
        seed,
        profile,
        meta,
        service_files,
        service_caches,
        #[cfg(test)]
        roots,
        finder,
        loader,
        catalog,
        factories,
        suggestions,
        flm,
    } = find_packages(&request, &files, compiled, &reads, Reading::Whole)?;
    part.finish();
    if scope.cancelled() {
        return Err(cancelled());
    }
    let part = registry_row.start();
    let front_matter = split(&input.text).ok().and_then(|split| split.front_matter.map(str::to_string));
    let blocks = blocks_in_use(&factories, &loader);
    let manifests = manifests(&blocks, &loader);
    let descriptions = descriptions(&finder, &manifests);
    let names_in_use = names_in_use(&factories, &blocks, &finder);
    let mut settings = combine(&request, &input, &seed, &factories, &blocks, &finder, catalog.reach(&factories))?;
    let refs: Vec<&PackageFactory> = factories.iter().collect();
    let read = read_settings(&settings, &refs, diagnostics)?;
    let settings_warnings = diagnostics.len();
    let mut builder = RegistryBuilder::new();
    for factory in &factories {
        let settings = read.packages.get(factory.name()).cloned().unwrap_or_default();
        let entry = factory.build(&settings).map_err(|error| {
            PrepareError::one(PrepareErrorKind::Registry, Problem::new(error.to_string()))
        })?;
        builder.use_package(entry);
    }
    // The chains of the front matter's `feature_order`, each naming the file and the line
    // where it is written; the packages' chains came with their entries.
    let file = input.name.clone().unwrap_or_else(|| String::from(TEXT_INPUT_NAME));
    for (written, place) in &seed.feature_order {
        builder.feature_order(written.chain_with_origin(format!("front matter of {file}, line {}", place.line)));
    }
    // The fields of the workflows the packages of extensions write, as the rule between
    // packages decides them: one set of fields per workflow.
    for (name, fields) in settings.workflows() {
        builder.workflow_fields(workflow_def(&name, fields.iter().map(|(field, written)| (field.as_str(), &written.value))));
    }
    let mut inputs = RegistryInputs::new();
    inputs.parsing = settings.parsing().map_err(settings_error)?;
    let registry = builder.build(&inputs).map_err(|error| {
        PrepareError::new(
            PrepareErrorKind::Registry,
            error.errors.iter().map(|error| registry_problem(error, &settings)).collect(),
        )
    })?;
    diagnostics.extend(registry.build_diagnostics().iter().cloned());
    part.finish();
    if scope.cancelled() {
        return Err(cancelled());
    }
    let part = plan_row.start();
    let (workflow, outputs) = choose_workflow(&registry, &request, &profile, input.name.as_deref())?;
    // The workflow's settings, given by the packages of extensions, are known once the
    // workflow is chosen; they are render settings, read anew with the others. The
    // settings' warnings are those of the last read, which covers every source: they
    // replace the first read's, ahead of the registry's build diagnostics.
    let render = if add_workflow_settings(&mut settings, &registry, &workflow, &blocks, &finder) {
        let mut warnings = Vec::new();
        let read = read_settings(&settings, &refs, &mut warnings);
        diagnostics.splice(..settings_warnings, warnings);
        read?.render
    } else {
        read.render.clone()
    };
    let mut overrides = Overrides::none();
    overrides.render_options = render;
    // The outputs are set up before the run's stages are scheduled, so only an output's
    // declaration (`made_from`) can say which outputs it is made from; each of those gets its
    // compose template too.
    for output in with_sources(&registry, &outputs) {
        compose(&registry, &settings, &output, &mut overrides)?;
    }
    let mut plan_request = workflow_request(&registry, &workflow, &overrides)
        .map_err(|error| PrepareError::one(PrepareErrorKind::Workflow, Problem::new(error.to_string())))?;
    // With `format`, the demands of the workflow's other outputs are dropped.
    let others: Vec<CapName> = outputs_of(&registry, &workflow)
        .iter()
        .filter(|name| !outputs.contains(name))
        .filter_map(|name| registry.output(name.as_str()).map(|entry| entry.def.capability.clone()))
        .collect();
    plan_request.demands.retain(|demand| !others.contains(demand));
    if request.sink.is_some() {
        if let Some(origin) = settings.switched_off(stdpkg::write::FEATURE) {
            let detail = format!(
                "the feature `{}` must not be switched off: this request writes the output of the workflow `{workflow}` through it",
                stdpkg::write::FEATURE
            );
            return Err(PrepareError::one(PrepareErrorKind::Plan, Problem::at_origin(Some(&origin), detail)));
        }
        plan_request.demands.push(CapName::from(stdpkg::write::WRITTEN));
    }
    let plan = Plan::build(&registry, &plan_request)
        .map_err(|error| PrepareError::one(PrepareErrorKind::Plan, Problem::new(error.to_string())))?;
    let layout = request.layout.or(seed.layout.map(|(layout, _)| layout)).unwrap_or_default();
    let ReadSettings { packages: package_settings, document: mut document_options, .. } = read;
    // The metadata is set only where a feature of the registry declares it: a value set for
    // an option type that no feature declares is refused when the run is made, so a document
    // that switches the `metadata` feature off is processed with its metadata unshown.
    if !meta.is_empty() && registry.option_decl(&OptionKey::of::<DocMeta>()).is_some() {
        document_options.set(meta);
    }
    part.finish();
    Ok(Prepared {
        settings,
        package_settings,
        document_options,
        layout,
        reading: reads,
        front_matter,
        request,
        files,
        service_files,
        service_caches,
        #[cfg(test)]
        roots,
        seed,
        registry,
        plan: Arc::new(plan),
        profile,
        workflow,
        outputs,
        packages: factories.iter().map(|factory| factory.name().to_string()).collect(),
        factories,
        suggestions,
        manifests,
        descriptions,
        names_in_use,
        flm,
        diagnostics: std::mem::take(diagnostics),
    })
}

/// Reads `settings` through `factories` ([`Settings::read`](flm_core::core::settings::cascade::Cascade::read))
/// and sets `warnings` to the warnings of this reading as diagnostics
/// ([`warning_diagnostic`]), whether or not it finds problems.
///
/// # Errors
///
/// The problems of the reading, as a [`PrepareError`] of the step
/// [`Settings`](PrepareErrorKind::Settings).
fn read_settings(
    settings: &Settings,
    factories: &[&PackageFactory],
    warnings: &mut Vec<FlmDiagnostic>,
) -> Result<ReadSettings<Origin>, PrepareError> {
    let (read, found) = match settings.read(factories) {
        Ok(read) => {
            let found = read.warnings.iter().map(warning_diagnostic).collect();
            (Ok(read), found)
        }
        Err(failure) => (Err(settings_error(failure.errors)), failure.warnings.iter().map(warning_diagnostic).collect()),
    };
    *warnings = found;
    read
}

/// The error of the step [`Settings`](PrepareErrorKind::Settings) with the problems
/// `problems`, each placed at its origin.
fn settings_error(problems: Vec<SettingsProblem>) -> PrepareError {
    PrepareError::new(
        PrepareErrorKind::Settings,
        problems.into_iter().map(|problem| Problem::at_origin(problem.origin.as_ref(), problem.detail)).collect(),
    )
}

/// Adds to `overrides` what the host adds to a plan for the output `output` when the
/// feature `compose` serves it: a preference for the compose stage, and, as the
/// [pieces](flm_core::core::piece::Piece) the output's backend places (document-level values
/// placed beside the content, such as the title), those the chosen compose template accepts.
/// *Prepare* calls it once per output the request produces, in the workflow's order, then
/// once per output those are made from ([`with_sources`]); two outputs of one backend would
/// both set that backend's pieces, and the later call wins.
///
/// # Errors
///
/// A [`PrepareError`] of kind [`PrepareErrorKind::Template`] when no template of the chosen
/// name serves the output, placed where the setting is written.
fn compose(
    registry: &FlmRegistry,
    settings: &Settings,
    output: &str,
    overrides: &mut Overrides,
) -> Result<(), PrepareError> {
    let Some(served) = compose::served_output(registry, output) else { return Ok(()) };
    let stage = compose::stage_name(served.output.as_str());
    // A registry in which the feature is switched off assembles the output in the
    // library's fixed skeleton.
    if registry.stage_named(&stage).is_none() {
        return Ok(());
    }
    let options = overrides.render_options.get::<ComposeOptions>().cloned().unwrap_or_default();
    let template = compose::chosen_template(registry, &options, served.output.as_str()).map_err(|detail| {
        let origin = settings.value(compose::FEATURE, "template").map(|value| value.origin);
        PrepareError::one(PrepareErrorKind::Template, Problem::at_origin(origin.as_ref(), detail))
    })?;
    let mut pieces = overrides.render_options.get::<PiecesRenderOptions>().cloned().unwrap_or_default();
    compose::select_pieces(registry, template, served, &mut pieces);
    overrides.render_options.set(pieces);
    overrides.prefer.push(stage.into());
    Ok(())
}

/// Returns the problem for the registry build error `error`: when it is a name, given by
/// a field of a workflow, that nothing declares, and a package wrote that field
/// (`settings` records where), the problem is placed at the field in the package's file;
/// otherwise it is the library's message.
fn registry_problem(error: &BuildError, settings: &Settings) -> Problem {
    if let BuildError::Undeclared { kind, site, name, roster, .. } = error {
        let field = match site {
            Site::WorkflowExtends { workflow } => Some((workflow, "extends")),
            Site::WorkflowOutputs { workflow } => Some((workflow, "outputs")),
            Site::WorkflowDemands { workflow } => Some((workflow, "demands")),
            Site::WorkflowPrefers { workflow } => Some((workflow, "prefer")),
            _ => None,
        };
        let names = |value: &serde_json::Value| {
            value.as_str() == Some(name.as_str())
                || value.as_array().is_some_and(|items| items.iter().any(|item| item.as_str() == Some(name.as_str())))
        };
        if let Some((workflow, field)) = field {
            let written = settings.workflow_field(workflow.as_ref(), field).filter(|written| names(&written.value));
            if let Some(written) = written {
                let plural = match *kind {
                    "capability" => String::from("capabilities"),
                    other => format!("{other}s"),
                };
                let known: Vec<&str> = roster.iter().map(|name| name.as_str()).collect();
                let detail = format!(
                    "`{name}`, in the field `{field}` of the workflow `{workflow}`, must be one of the {plural}: `{}`",
                    known.join("`, `")
                );
                return Problem::at_origin(Some(&written.origin), detail);
            }
        }
    }
    Problem::new(error.to_string())
}

/// Returns the input's front matter as read, the whole of it or only the keys that decide
/// the packages, as `reading` says: the default, empty value when the input has none.
fn read_input_seed(input: &ReadInput, reading: Reading) -> Result<Seed, PrepareError> {
    let front_matter = |error: crate::seed::SeedError| Problem {
        file: input.name.clone(),
        place: error.place,
        detail: error.detail,
    };
    let split = split(&input.text)
        .map_err(|error| PrepareError::one(PrepareErrorKind::FrontMatter, front_matter(error)))?;
    let Some(text) = split.front_matter else { return Ok(Seed::default()) };
    let root: Node = read_front_matter(text, split.front_matter_line)
        .map_err(|error| PrepareError::one(PrepareErrorKind::FrontMatter, front_matter(error)))?;
    let read = match reading {
        Reading::Whole => read_seed(&root),
        Reading::Packages => read_packages_seed(&root),
    };
    read.map_err(|errors| {
        PrepareError::new(PrepareErrorKind::FrontMatter, errors.into_iter().map(front_matter).collect())
    })
}

/// The settings source of the block `block`, of a package of an extension: as a package of
/// the packages' rank, or, with `workflow`, as the settings it gives that workflow.
fn package_source(block: &LoadedBlock, workflow: Option<&str>) -> Source {
    let factory = block.package.to_string();
    let file = block.extension.identity.clone();
    let stem = block.extension.stem.clone();
    let name = block.extension.name.clone();
    let package = factory.strip_prefix(&format!("{file}#")).unwrap_or(&factory).to_string();
    match workflow {
        None => Source::Package { factory, file, stem, name, package },
        Some(workflow) => Source::Workflow { workflow: workflow.to_string(), factory, file, stem, name, package },
    }
}

/// The blocks that `loader` loaded for the packages of `factories`, in the order of the
/// factories.
fn blocks_in_use(factories: &[PackageFactory], loader: &ExtensionLoader) -> Vec<LoadedBlock> {
    let loaded = loader.blocks();
    factories
        .iter()
        .filter_map(|factory| loaded.iter().find(|block| block.package == *factory.name()).cloned())
        .collect()
}

/// The description of each manifest of `manifests` that has one, by each locator through
/// which `finder` found its extension, so that an extension found under another locator of
/// the same stem lends it nothing.
fn descriptions(finder: &ExtensionFinder, manifests: &BTreeMap<String, Manifest>) -> BTreeMap<String, String> {
    finder
        .parsed_manifests()
        .found()
        .into_iter()
        .filter_map(|(locator, identity)| Some((locator, manifests.get(&identity)?.description.clone()?)))
        .collect()
}

/// The name of each of `factories` as an entry of `use` names it, each name once, in their
/// order: a factory that no block of `blocks` provides (a package compiled into the program)
/// by its name; a package of an extension by each locator through which `finder` found the
/// extension.
fn names_in_use(factories: &[PackageFactory], blocks: &[LoadedBlock], finder: &ExtensionFinder) -> Vec<String> {
    let found = finder.parsed_manifests().found();
    let mut names: Vec<String> = Vec::new();
    for factory in factories {
        let of_factory: Vec<String> = match blocks.iter().find(|block| block.package == *factory.name()) {
            Some(block) => found
                .iter()
                .filter(|(_, identity)| **identity == block.extension.identity)
                .map(|(locator, _)| locator.clone())
                .collect(),
            None => vec![factory.name().to_string()],
        };
        for name in of_factory {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// The manifest of the extension of each of `blocks`, by the extension's identity, as the
/// loader read it ([`ExtensionLoader::manifest`]).
fn manifests(blocks: &[LoadedBlock], loader: &ExtensionLoader) -> BTreeMap<String, Manifest> {
    blocks
        .iter()
        .filter_map(|block| {
            let identity = &block.extension.identity;
            Some((identity.clone(), loader.manifest(identity)?))
        })
        .collect()
}

/// The node of `flm` in the manifest of the extension of `block`, when the finder parsed
/// one: the places of the block's values.
fn block_node(block: &LoadedBlock, finder: &ExtensionFinder) -> Option<Node> {
    finder.manifest(&block.extension.identity).and_then(|root| root.get(crate::seed::RESERVED_KEY).cloned())
}

/// Adds to `settings` the settings that the packages of `blocks` give the workflow
/// `workflow` of `registry` or, when they give it none, the nearest workflow it extends to
/// which they give some ([`FlmRegistry::workflow_chain`]): a workflow's settings are one of
/// its fields, which a workflow that writes it replaces whole. Returns whether any were
/// added.
fn add_workflow_settings(
    settings: &mut Settings,
    registry: &FlmRegistry,
    workflow: &str,
    blocks: &[LoadedBlock],
    finder: &ExtensionFinder,
) -> bool {
    let chain = registry.workflow_chain(workflow).unwrap_or_default();
    for def in chain.into_iter().rev() {
        let name = def.name.to_string();
        let given: Vec<(&LoadedBlock, &flm_ext::block::WorkflowBlock)> = blocks
            .iter()
            .flat_map(|block| block.block.workflows.iter().map(move |written| (block, written)))
            .filter(|(_, written)| written.name == name && !written.settings.is_empty())
            .collect();
        if !given.is_empty() {
            for (block, written) in given {
                let source = package_source(block, Some(&name));
                crate::settings::add_workflow_settings(settings, written, &source, block_node(block, finder).as_ref());
            }
            return true;
        }
    }
    false
}

/// Returns the settings of the request, collected from the program's defaults for the
/// features of the packages in use `factories`, the blocks of those packages `blocks`
/// (their settings and the fields of their workflows), which reach one another as `reach`
/// says, the front matter and the command line.
///
/// The program's defaults are values for features of the library; a document whose packages
/// lack such a feature gets none of its defaults, so that no warning names a setting that
/// the author did not write.
fn combine(
    request: &Request,
    input: &ReadInput,
    seed: &Seed,
    factories: &[PackageFactory],
    blocks: &[LoadedBlock],
    finder: &ExtensionFinder,
    reach: Reach,
) -> Result<Settings, PrepareError> {
    let mut settings = Settings::with_reach(reach);
    let defaults = stdpkg::defaults()
        .map_err(|detail| PrepareError::one(PrepareErrorKind::Settings, Problem::new(detail)))?;
    let in_use = |feature: &str| {
        factories.iter().any(|factory| factory.features().iter().any(|entry| entry.name == feature))
    };
    for (feature, key, value) in defaults.into_iter().filter(|(feature, ..)| in_use(feature)) {
        settings.set(feature, key, value, Origin { source: Source::Defaults, place: None });
    }
    for block in blocks {
        add_block(&mut settings, &block.block, &package_source(block, None), block_node(block, finder).as_ref());
    }
    let file = input.name.clone().unwrap_or_else(|| String::from(TEXT_INPUT_NAME));
    add_block(&mut settings, &seed.block, &Source::FrontMatter(file), seed.flm.as_ref());
    let mut problems = Vec::new();
    for (index, setting) in request.options.iter().enumerate() {
        if let Err(problem) = add_assignment(&mut settings, setting, index + 1) {
            problems.push(Problem::new(problem.detail));
        }
    }
    if problems.is_empty() { Ok(settings) } else { Err(PrepareError::new(PrepareErrorKind::Settings, problems)) }
}

/// Returns the outputs of the workflow `workflow` of `registry`, as it runs: its own, or
/// those it inherits ([`FlmRegistry::combined_workflow`]).
fn outputs_of(registry: &FlmRegistry, workflow: &str) -> Vec<String> {
    let combined = registry.combined_workflow(workflow);
    combined.map(|def| def.outputs.iter().map(|name| name.to_string()).collect()).unwrap_or_default()
}

/// Returns `outputs`, in their order, followed by every output they are made from
/// ([`OutputDef::made_from`](flm_core::core::registry::OutputDef::made_from)), and every
/// output those are made from in turn: each output once, the outputs reached in the order
/// they are first reached, breadth first.
///
/// *Prepare* sets up the compose template of each output this returns, so that the compose
/// template chosen for `latex-rendered` assembles the LaTeX that an output made from it
/// compiles, also when `latex-rendered` itself is not requested.
pub(crate) fn with_sources(registry: &FlmRegistry, outputs: &[String]) -> Vec<String> {
    let mut reached: Vec<String> = Vec::new();
    for output in outputs {
        if !reached.contains(output) {
            reached.push(output.clone());
        }
    }
    let mut next = 0;
    while next < reached.len() {
        let sources = registry.output(&reached[next]).map(|entry| entry.def.made_from.clone()).unwrap_or_default();
        for source in sources {
            if !reached.iter().any(|output| output == source.as_str()) {
                reached.push(source.to_string());
            }
        }
        next += 1;
    }
    reached
}

/// Returns the workflow of `profile` and the outputs the request produces, in the
/// workflow's order: see "Which profile runs, and its workflow" in the [module
/// documentation](self). `file` is the input's file, where a problem with the front
/// matter's `default_profile` is placed.
fn choose_workflow(
    registry: &FlmRegistry,
    request: &Request,
    profile: &Profile,
    file: Option<&str>,
) -> Result<(String, Vec<String>), PrepareError> {
    // The workflow may come from any package, not only from `profile-NAME`: the registry does
    // not say which package declared a workflow, whose fields may come from several.
    let workflow = profile.name.clone();
    if registry.workflow(&workflow).is_none() {
        let detail = match &profile.package {
            Some(package) => format!("the package `{package}` fails to define a workflow named `{workflow}`"),
            None => profile_not_found(registry, &workflow),
        };
        let problem = Problem { file: profile.place.and(file).map(str::to_string), place: profile.place, detail };
        return Err(PrepareError::one(PrepareErrorKind::Workflow, problem));
    }
    // A format selects one of the outputs of the workflow that runs and never chooses the
    // workflow: the profile says what kind of output to produce, and several workflows may
    // provide one format, with nothing to say which should run.
    let provided = outputs_of(registry, &workflow);
    let outputs = match &request.format {
        None => provided,
        Some(format) if provided.contains(format) => vec![format.clone()],
        Some(format) => {
            let detail = unprovided_format(format, &workflow, &provided);
            return Err(PrepareError::one(PrepareErrorKind::Workflow, Problem::new(detail)));
        }
    };
    Ok((workflow, outputs))
}

/// The message for the profile `name` when no package `profile-NAME` was found and
/// `registry` holds no workflow `name`; it ends with the workflows the registry holds, in
/// its order, when it holds any.
fn profile_not_found(registry: &FlmRegistry, name: &str) -> String {
    let known = registry.workflows().map(|entry| format!("`{}`", entry.def.name)).collect::<Vec<_>>();
    let found = format!(
        "profile `{name}` not found; ensure that the existing packages define a workflow named `{name}` or that \
         an extension named `{PROFILE_PACKAGE_PREFIX}{name}` is in scope"
    );
    if known.is_empty() { found } else { format!("{found}; try perhaps {}", known.join(", ")) }
}

/// The message for the format `format` that the workflow `workflow` does not provide;
/// `provided` holds the formats (the outputs) it provides.
fn unprovided_format(format: &str, workflow: &str, provided: &[String]) -> String {
    let provided = match provided {
        [] => String::from("none"),
        _ => format!("`{}`", provided.join("`, `")),
    };
    format!("the workflow `{workflow}` does not provide the format `{format}`; it provides {provided}")
}

/// The result of [`build`]: the run, every diagnostic, and the paths read. The output
/// itself is not here: the run's tables hold each output's artifact set, and the request's
/// sink received the named files.
pub struct Built {
    /// The run, whose tables a caller reads (each output's artifact set among them).
    pub run: Run,
    /// Every diagnostic, in the order reported: *prepare*'s, then the run's.
    pub diagnostics: Vec<FlmDiagnostic>,
    /// Every path read, in the order read: *prepare*'s reads (as
    /// [`Prepared::files_read`] lists them) first, then the build's.
    pub files_read: Vec<FileRead>,
    /// The content of the input and of each file included with `\input`, by the file's
    /// name, with the byte offset at which it starts in the file: what
    /// [`Record::of_diagnostic`](crate::report::Record::of_diagnostic) needs to count the
    /// byte offsets of the [`diagnostics`](Built::diagnostics) from the top of each file.
    pub content_offsets: ContentOffsets,
}

impl Built {
    /// Returns whether the document has errors: any diagnostic of Error severity.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| diagnostic.severity() == flm_core::techy::error::Severity::Error)
    }
}

/// Builds the document once with what `prepared` made: reads the input again, parses its
/// content with [`Inclusion`] as the input resolver, makes the document and executes one run
/// of the plan. When the request gives a sink, the run's last stage hands the output's named
/// files to it. While the run executes, the registry's services read through this build's
/// file reader ([`ServiceFiles`]), and the sessions of the caches they open are kept for the
/// run and closed when it ends ([`ServiceCaches`]).
///
/// It reports under `progress` in two rows, declared when it begins: [`PARSE_KIND`], for
/// reading the input and parsing it, then [`RUN_KIND`], under which the run reports
/// ([`Run::with_progress`]). It asks whether the build is cancelled once the content is
/// parsed, and the run asks at its own points. A caller that shows no progress passes
/// [`Scope::none()`].
///
/// # Errors
///
/// A [`PrepareError`] when the input can no longer be read or its front matter is no longer
/// closed, one of kind [`PrepareErrorKind::Run`] when the run cannot be made or fails as
/// a program (never for a document with errors, whose problems are in
/// [`Built::diagnostics`]), and one of kind [`PrepareErrorKind::Cancelled`], with no
/// problem, when the build was cancelled; each carries *prepare*'s diagnostics
/// ([`Prepared::diagnostics`]), then, for a cancelled run, those of the steps it completed,
/// and lists the paths read up to that point, *prepare*'s first.
pub fn build(prepared: &Prepared, progress: &Scope) -> Result<Built, PrepareError> {
    let reads = BuildFiles::new(Arc::clone(&prepared.files));
    build_reading(prepared, reads.clone(), progress).map_err(|error| {
        let mut files_read = prepared.files_read();
        files_read.extend(reads.list());
        let mut diagnostics = prepared.diagnostics.clone();
        diagnostics.extend(error.diagnostics);
        PrepareError { diagnostics, files_read, ..error }
    })
}

/// [`build`], reading through `reads`.
fn build_reading(prepared: &Prepared, reads: BuildFiles, progress: &Scope) -> Result<Built, PrepareError> {
    let file = match &prepared.request.input {
        Input::File(path) => Some(path.as_str()),
        Input::Text(_) => None,
    };
    let parse = progress.child(&PARSE_KIND, file).start();
    let run_row = progress.child(&RUN_KIND, None);
    let input = read_input(&prepared.request.input, &reads)?;
    let split = split(&input.text).map_err(|error| {
        PrepareError::one(
            PrepareErrorKind::FrontMatter,
            Problem { file: input.name.clone(), place: error.place, detail: error.detail },
        )
    })?;
    let name = input.name.clone().unwrap_or_else(|| String::from(TEXT_INPUT_NAME));
    let mut content_offsets = ContentOffsets::new();
    content_offsets.insert(name.clone(), &split);
    let source = FlmSource::new(split.content.to_string())
        .with_origin(FlmOrigin::new(name))
        .with_line_column_number_offsets(split.content_line as usize, 1);
    let lang = prepared.registry.lang();
    let answerer = Arc::new(Inclusion::new(reads.clone()));
    let options = ParseOptions { input_resolver: Some(Arc::clone(&answerer) as _), ..ParseOptions::default() };
    let content = assemble(lang, Arc::new(source), prepared.layout, &options)
        .map_err(|error| PrepareError::one(PrepareErrorKind::Input, Problem::new(error.to_string())))?;
    let doc = Arc::new(Doc::with_options(content, prepared.document_options.clone()));
    parse.finish();
    if progress.cancelled() {
        return Err(cancelled());
    }
    let running = run_row.start();
    let mut run = Run::new(doc, Arc::clone(&prepared.plan), Externals::none())
        .map_err(|error| PrepareError::one(PrepareErrorKind::Run, Problem::new(error.to_string())))?
        .with_progress(running.scope());
    let mut options = RunOptions::new();
    options.continue_with_errors = prepared.request.continue_with_errors;
    // The services of the registry read through this build's reading while it runs: the
    // library hands a service no context of the run, so the reading reaches a service only
    // through the handle it already holds, and builds take turns so that their lists of
    // files read stay apart. The sessions of the caches that the services open meanwhile
    // last the run, and are closed when it ends, whatever its outcome.
    let attached = prepared.service_files.attach(reads.clone());
    let attached_caches = prepared.service_caches.attach();
    let executed = run.execute(&options);
    drop(attached_caches);
    drop(attached);
    let outcome =
        executed.map_err(|error| PrepareError::one(PrepareErrorKind::Run, Problem::new(error.to_string())))?;
    if outcome == RunOutcome::Cancelled {
        return Err(PrepareError { diagnostics: run.diagnostics().iter().cloned().collect(), ..cancelled() });
    }
    running.finish();
    let mut diagnostics = prepared.diagnostics.clone();
    diagnostics.extend(run.diagnostics().iter().cloned());
    let mut files_read = prepared.files_read();
    files_read.extend(reads.list());
    content_offsets.extend(answerer.content_offsets());
    Ok(Built { run, diagnostics, files_read, content_offsets })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_the_workflow_does_not_provide_lists_the_formats_it_provides() {
        assert_eq!(
            unprovided_format("text", DEFAULT_PROFILE, &[]),
            "the workflow `html` does not provide the format `text`; it provides none"
        );
        let provided = [String::from("html"), String::from("latex-rendered")];
        assert_eq!(
            unprovided_format("text", "pagepaper", &provided),
            "the workflow `pagepaper` does not provide the format `text`; it provides `html`, `latex-rendered`"
        );
    }

    /// The outputs `a` (made from `b`), `b` (made from `c`) and `c`, and `x` and `y`, each
    /// made from the other.
    struct Chained;

    impl flm_core::core::registry::FlmFeature for Chained {
        fn info(&self) -> flm_core::core::registry::FeatureInfo {
            flm_core::core::registry::FeatureInfo::new("chained", "outputs made from other outputs")
        }

        fn register(&self, b: &mut flm_core::core::registry::FeatureBuilder<'_>) -> Result<(), BuildError> {
            use flm_core::core::artifact::ArtifactSet;
            use flm_core::core::data::table::TableName;
            use flm_core::core::registry::OutputDef;
            const TABLES: [(&str, TableName<ArtifactSet>, &[&str]); 5] = [
                ("a", TableName::new("chained.a"), &["b"]),
                ("b", TableName::new("chained.b"), &["c"]),
                ("c", TableName::new("chained.c"), &[]),
                ("x", TableName::new("chained.x"), &["y"]),
                ("y", TableName::new("chained.y"), &["x"]),
            ];
            for (output, table, sources) in TABLES {
                b.table(table, "an output's files")?;
                let def = OutputDef::new(output, table.name(), table.name(), "text/plain", "txt");
                b.output(sources.iter().fold(def, |def, source| def.made_from(*source)))?;
            }
            Ok(())
        }
    }

    /// The outputs reached from the requested ones through `made_from`: each once, in the
    /// order first reached, breadth first, also through a cycle.
    #[test]
    fn the_outputs_made_from_are_reached_transitively_each_once() {
        let mut builder = RegistryBuilder::new();
        builder.use_package(flm_core::core::registry::PackageEntry::new("chained").feature(Arc::new(Chained)));
        let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
        let reached = |outputs: &[&str]| {
            let outputs: Vec<String> = outputs.iter().map(|output| output.to_string()).collect();
            with_sources(&registry, &outputs)
        };
        assert_eq!(reached(&["a"]), ["a", "b", "c"]);
        // An output both requested and reached, and an output requested twice, appear once.
        assert_eq!(reached(&["a", "c"]), ["a", "c", "b"]);
        assert_eq!(reached(&["c", "a", "a"]), ["c", "a", "b"]);
        assert_eq!(reached(&["x"]), ["x", "y"]);
        // An output no feature registers is kept, and reaches nothing.
        assert_eq!(reached(&["unknown", "b"]), ["unknown", "b", "c"]);
    }

    /// The preparation of `doc/paper.flm`, which uses `outer`, which uses `inner`, both
    /// extensions written as folders in `defs`; `inner` holds `data/note.txt`.
    fn prepared_with_two_extensions() -> Prepared {
        let files = crate::files::MemoryFiles::new()
            .with("doc/paper.flm", "---\nflm:\n  use: [outer]\n---\nHello.\n")
            .with("defs/outer/flmextension.yaml", "flm:\n  use: [inner]\n")
            .with("defs/inner/flmextension.yaml", "extension: {description: Inner.}\n")
            .with("defs/inner/data/note.txt", "A note.");
        let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
        request.default_math = MathChoice::processor("passthrough");
        request.profile = Some(String::from("text"));
        request.extension_folders = vec![ExtensionFolder { path: String::from("defs"), required: false }];
        prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared")
    }

    /// The qualified path `text`.
    fn qualified(text: &str) -> flm_ext::paths::QualifiedPath {
        match flm_ext::paths::WrittenPath::parse(text) {
            Ok(flm_ext::paths::WrittenPath::Qualified(path)) => path,
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn a_path_of_an_extension_a_package_uses_is_read_among_the_files_the_build_read() {
        let prepared = prepared_with_two_extensions();
        let reads = BuildFiles::new(Arc::clone(&prepared.files));
        let attached = prepared.service_files.attach(reads.clone());
        let read = prepared.roots.read(&qualified("ext+inner:data/note.txt"));
        drop(attached);
        let expected = crate::roots::RootsFile { bytes: b"A note.".to_vec(), media_type: None, warning: None };
        assert_eq!(read, Ok(Some(expected)));
        let found = reads.list().into_iter().any(|read| read.path == "defs/inner/data/note.txt" && read.found);
        assert!(found, "{:?}", reads.list());
    }

    #[test]
    fn a_use_name_no_entry_writes_and_a_path_above_an_extension_s_folder_are_refused() {
        let prepared = prepared_with_two_extensions();
        let unknown = prepared.roots.read(&qualified("ext+other:data/note.txt")).unwrap_err();
        let expected = flm_ext::paths::PathError {
            path: String::from("ext+other:data/note.txt"),
            kind: flm_ext::paths::PathErrorKind::UnknownExtension(String::from("other")),
        };
        assert_eq!(unknown.kind, crate::roots::RootsErrorKind::Path(expected));
        let above = flm_ext::paths::WrittenPath::parse("ext+inner:../outer/flmextension.yaml").unwrap_err();
        assert_eq!(above.kind, flm_ext::paths::PathErrorKind::Above(String::from("ext+inner")));
    }
}
