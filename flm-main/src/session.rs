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
//! # Building again
//!
//! What *prepare* made serves every build as long as it is valid:
//! [`Prepared::is_current`] says whether it is, for the files as they are now. It is valid
//! while the input's front matter is the one *prepare* read, and every other file *prepare*
//! read (an extension's manifest, a folder of the search path, a file of an extension)
//! gives what it gave then. A change to the content below the front matter, or to a file
//! that only a build reads (an included file, an image, a bibliography file), needs a new
//! build only; any other change needs a new [`prepare`]. Preparing again makes a new
//! registry, so what is kept together with the registry (the results an image converter
//! keeps) is made again; building only keeps it.
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
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, workflow_request};
use flm_core::core::registry::{
    BuildError, FlmRegistry, PackageCatalog, PackageFactory, RegistryBuilder, RegistryInputs, Site,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmDiagnostic, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::assemble::assemble;
use flm_core::model::doc::{Doc, FragmentLayout};
use flm_core::core::settings::PackageSettings;
use flm_core::model::names::{CapName, Name};
use flm_core::model::options::{OptionKey, OptionValues};
use flm_core::stdcore::features::meta::DocMeta;

use crate::files::{BuildFiles, FileRead, Files, ServiceFiles};
use crate::inclusion::Inclusion;
use crate::report::ContentOffsets;
use crate::output::{Naming, OutputFolder, Sink};
use crate::extensions::ExtensionFinder;
use crate::packages::{EntrySource, ExtensionFolder, PackageProblem, catalog, loader, resolve, suggestions};
use flm_ext::{ExtensionLoader, LoadedBlock, Manifest};
use crate::seed::{
    MathChoice, Node, Place, Seed, read_front_matter, read_metadata, read_packages_seed, read_seed, split,
};
use crate::settings::{
    CommandLineSetting, FeatureInEffect, Origin, Reach, ReadSettings, Settings, SettingsProblem, Source,
    add_assignment, add_block, warning_diagnostic,
};
use flm_core::core::settings::cascade::workflow_def;
use flm_core::stdcore::features::compose::{self, ComposeOptions};
use crate::stdpkg::{self, StdParts};
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;

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

/// The input of a request: what a command processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The file at this path, relative to the current folder or absolute.
    File(PathBuf),
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
    /// one. By default `std` ([`stdpkg::PACKAGE`]), which uses the library's `stdcore` and
    /// the host's `stdmain`, from [`DEFAULT_INITIAL_LIST`]; a host sets the list its
    /// documents start from. The list is used exactly as given: a package it does not name
    /// is not used unless the document lists it ([`crate::packages`]). The math processor
    /// is chosen apart from this list ([`default_math`](Request::default_math)).
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
    /// The host of the JavaScript extensions the request's documents use (cargo feature
    /// `js`): the settings of their engine instances, the modules the program offers them,
    /// where their `console` output goes; by default the default settings, no module, and
    /// `console` output written to standard error. An extension's manifest may raise the
    /// limits for its own instances (`extension.js.limits`); the host's stack setting stays
    /// what bounds an instance created on a thread that is not a worker thread, such as the
    /// thread that loads the extension (`JsSettings::stack_size`).
    #[cfg(feature = "js")]
    pub js_host: flm_ext::quickjs::JsHost,
    /// The host of the extensions compiled to WebAssembly that the request's documents use
    /// (cargo feature `wasm`): the settings of their instances (the memory and the fuel of
    /// a handler call), where their `console` output goes; by default the default settings,
    /// and `console` output written to standard error. An extension's manifest may set the
    /// limits for its own instances (`extension.wasm.limits`).
    #[cfg(feature = "wasm")]
    pub wasm_host: flm_ext::wasm::WasmHost,
    /// Whether the extensions may run programs installed on the machine (cargo feature
    /// `fs`): when `true`, the loader of every preparation offers the host service
    /// `programs` ([`ProgramsService`](crate::host_programs::ProgramsService)) beside
    /// `files`, and a package that declares it reaches it; when `false`, the default, the
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
    /// a name the operating system looks for in its folders of programs, or a path, given to
    /// the system as written. The `flm` program takes it from the environment variable
    /// `MAGICK` when it is set. It is a property of the machine, given here as the output
    /// folder and the search path are, and never a setting, so that no document can choose a
    /// program for the host to start. A converter added later that runs a program gets its
    /// program the same way, from a field of its own.
    // See decision rationale at [§flm-dd-dr:host-magick-program]
    #[cfg(feature = "graphics")]
    pub magick_program: std::ffi::OsString,
    /// What the host's citation provider is given of the machine (cargo feature
    /// `citations`; [`CitationsHost`](crate::citations::CitationsHost)): the network, the
    /// folder of the cache file, and a listener to the progress of a fetch; by default none
    /// of them. The `flm` program gives access to the internet, the folder of the document,
    /// and, with `-v`, a listener that writes on standard error. Like the program the
    /// converter `magick` runs, these are properties of the machine, never settings.
    #[cfg(feature = "citations")]
    pub citations: crate::citations::CitationsHost,
}

impl Request {
    /// Returns a request for `input` with every other field at its default: no output
    /// format and no profile given, the initial list `std` and none given on the command
    /// line, no math processor and none given on the command line, no package added on the
    /// command line, no setting, no sink (nothing is written), artifacts named into a
    /// folder, the current folder as the output folder, an empty search path, no host
    /// service `programs`.
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
    /// for a feature that no package in use has or that takes no settings, a key that no
    /// feature declares
    /// or a value its declaration cannot read, a key in a workflow's settings that is not set
    /// per render request, a parsing setting of the wrong type, or two packages that
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
            let text = files.read_text(Path::new(""), path).map_err(|error| {
                PrepareError::one(
                    PrepareErrorKind::Input,
                    Problem { file: Some(path.display().to_string()), place: None, detail: error.to_string() },
                )
            })?;
            Ok(ReadInput { name: Some(path.display().to_string()), text })
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

impl Prepared {
    /// Returns whether what *prepare* made is still valid for the files as they are now:
    /// the input's front matter is the text *prepare* read, and every other path *prepare*
    /// looked for gives the same result (the same bytes, the same entries of a folder,
    /// nothing where nothing was). See "Building again" in the [module
    /// documentation](self). It reads each of those paths again, without recording them on
    /// any list of files read.
    pub fn is_current(&self) -> bool {
        let primary = match &self.request.input {
            Input::File(path) => Some(BuildFiles::resolve(Path::new(""), path)),
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
    /// uses: what the preview's settings tool builds its schema from.
    pub fn factories(&self) -> &[PackageFactory] {
        &self.factories
    }

    /// Returns the values that the JSON Schemas suggest for an entry of `use` and of
    /// `initial_use`, listed on the request's search path when the packages were found
    /// ([`crate::packages::suggestions`]): what the preview's settings tool suggests with
    /// its schema.
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
    /// warnings ([`UnknownFeatureSwitchedOff`](crate::settings::UnknownFeatureSwitchedOff)) and those of the
    /// registry build.
    pub fn diagnostics(&self) -> &[FlmDiagnostic] {
        &self.diagnostics
    }
}

/// Prepares the work that `request` asks for, reading files through `files`; `compiled` are
/// the compiled packages the program carries, besides `stdcore`, `stdmain` and `std`. See
/// the [module documentation](self).
///
/// # Errors
///
/// A [`PrepareError`] giving the step that failed, every problem found there, the
/// diagnostics found before that step, and the paths read up to that point.
pub fn prepare(request: Request, files: Arc<dyn Files>, compiled: &[PackageFactory]) -> Result<Prepared, PrepareError> {
    let reads = BuildFiles::new(Arc::clone(&files));
    let mut diagnostics = Vec::new();
    prepare_reading(request, files, compiled, reads.clone(), &mut diagnostics)
        .map_err(|error| PrepareError { diagnostics, files_read: reads.list(), ..error })
}

/// What [`document_packages`] finds for a document: its packages, and what the JSON
/// Schemas suggest for an entry of `use` on its search path.
#[derive(Clone)]
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
        .map(|found| DocumentPackages { factories: found.factories, suggestions: found.suggestions })
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
    finder: Arc<ExtensionFinder>,
    loader: Arc<ExtensionLoader>,
    catalog: PackageCatalog,
    factories: Vec<PackageFactory>,
    suggestions: Vec<String>,
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
/// its front matter (as `reading` says), makes `stdmain`, the finder of extensions, the
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
    let parts = StdParts {
        namer: Arc::new(request.naming.clone()),
        sink: request.sink.clone(),
        files: service_files.clone(),
        output_folder: request.output_folder.clone(),
        #[cfg(feature = "graphics")]
        magick_program: request.magick_program.clone(),
        #[cfg(feature = "citations")]
        citations: request.citations.clone(),
    };
    let stdmain = stdpkg::stdmain_factory(parts).map_err(|error| {
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
    // The document's folder, for the host service `files`: the folder of the input file,
    // the current folder (the empty path) for a text.
    let document = match &request.input {
        Input::File(path) => path.parent().map(Path::to_path_buf).unwrap_or_default(),
        Input::Text(_) => PathBuf::new(),
    };
    let loader = loader(&finder, request, &service_files, &document);
    let catalog = catalog(stdmain, compiled, Arc::clone(&loader))
        .map_err(|error| PrepareError::one(PrepareErrorKind::Packages, Problem::new(error.to_string())))?;
    // The profile's package comes last, unless no compiled package, no folder of the search
    // path and no bundled extension holds its name: that absence adds nothing and is no
    // problem. A lookup that fails adds the entry, whose resolution below reports the failure
    // as an entry's of `use` is reported.
    // See decision rationale at [§flm-dd-dr:profiles]
    let (name, place) = profile_name(request, &seed);
    let package = format!("{PROFILE_PACKAGE_PREFIX}{name}");
    let known = !matches!(catalog.resolve(&package), Ok(None));
    let profile_entry = known.then(|| (package.clone(), EntrySource::Given(format!("the profile `{name}`"))));
    let profile = Profile { name, place, package: known.then_some(package) };
    // The request's packages follow those of `use`, as if written last there, and come
    // before the profile's package.
    // See decision rationale at [§flm-dd-dr:use-option]
    let named: Vec<(String, EntrySource)> = initial_list(request, &seed)
        .into_iter()
        .chain(math_entry(request, &seed))
        .chain(seed.uses.iter().map(|(name, place)| (name.clone(), EntrySource::File(*place))))
        .chain(request.uses.iter().flat_map(given_entries))
        .chain(profile_entry)
        .collect();
    let factories = resolve(&catalog, &loader, &finder, &named, input.name.as_deref()).map_err(packages_problem)?;
    let suggestions = suggestions(catalog.compiled_names().map(|name| name.as_str()), &finder);
    Ok(Found { input, seed, profile, meta, service_files, finder, loader, catalog, factories, suggestions })
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
/// the words that say where its list comes from ([`GivenList::source`]).
fn initial_list(request: &Request, seed: &Seed) -> Vec<(String, EntrySource)> {
    match (&request.initial_use, &seed.initial_use) {
        (Some(list), _) => given_entries(list).collect(),
        (None, Some(written)) => written.iter().map(|(name, place)| (name.clone(), EntrySource::File(*place))).collect(),
        (None, None) => given_entries(&request.default_initial_use).collect(),
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
/// when the front matter chooses it, else the words that say where the choice comes from.
/// `None` when the choice is no processor.
fn math_entry(request: &Request, seed: &Seed) -> Option<(String, EntrySource)> {
    // The processor is chosen apart from the initial list: a list that the front matter or
    // the request writes does not change it, and the processor's package follows that list
    // whatever its source.
    // See decision rationale at [§flm-dd-dr:math-processor-choice]
    let (choice, source) = match (&request.math, &seed.math) {
        (Some(given), _) => (&given.choice, EntrySource::Given(given.source.clone())),
        (None, Some((written, place))) => (written, EntrySource::File(*place)),
        (None, None) => (&request.default_math, EntrySource::Given(String::from(DEFAULT_MATH_PROCESSOR))),
    };
    choice.package().map(|package| (package, source))
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
) -> Result<Prepared, PrepareError> {
    let Found { input, seed, profile, meta, service_files, finder, loader, catalog, factories, suggestions } =
        find_packages(&request, &files, compiled, &reads, Reading::Whole)?;
    let front_matter = split(&input.text).ok().and_then(|split| split.front_matter.map(str::to_string));
    // The blocks of the packages in use, in the order of the factories.
    let loaded = loader.blocks();
    let blocks: Vec<LoadedBlock> = factories
        .iter()
        .filter_map(|factory| loaded.iter().find(|block| block.package == *factory.name()).cloned())
        .collect();
    let manifests = manifests(&blocks, &loader);
    let mut settings = combine(&request, &input, &seed, &blocks, &finder, catalog.reach(&factories))?;
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
    // See decision rationale at [§flm-dd-dr:rend-output-made-from].
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
    // The metadata is set only where a feature of the registry declares it: a
    // registry without the `meta` feature shows none.
    // See decision rationale at [§flm-dd-dr:host-meta-declared].
    if !meta.is_empty() && registry.option_decl(&OptionKey::of::<DocMeta>()).is_some() {
        document_options.set(meta);
    }
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
    let package = factory.strip_prefix(&format!("{file}#")).unwrap_or(&factory).to_string();
    match workflow {
        None => Source::Package { factory, file, stem, package },
        Some(workflow) => Source::Workflow { workflow: workflow.to_string(), factory, file, stem, package },
    }
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

/// Returns the settings of the request, collected from the program's defaults, the blocks
/// of the packages in use `blocks` (their settings and the fields of their workflows),
/// which reach one another as `reach` says, the front matter and the command line.
fn combine(
    request: &Request,
    input: &ReadInput,
    seed: &Seed,
    blocks: &[LoadedBlock],
    finder: &ExtensionFinder,
    reach: Reach,
) -> Result<Settings, PrepareError> {
    let mut settings = Settings::with_reach(reach);
    let defaults = stdpkg::defaults()
        .map_err(|detail| PrepareError::one(PrepareErrorKind::Settings, Problem::new(detail)))?;
    for (feature, key, value) in defaults {
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
fn with_sources(registry: &FlmRegistry, outputs: &[String]) -> Vec<String> {
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
    // See decision rationale at [§flm-dd-dr:profiles]
    let workflow = profile.name.clone();
    if registry.workflow(&workflow).is_none() {
        let detail = match &profile.package {
            Some(package) => format!("the package `{package}` fails to define a workflow named `{workflow}`"),
            None => profile_not_found(registry, &workflow),
        };
        let problem = Problem { file: profile.place.and(file).map(str::to_string), place: profile.place, detail };
        return Err(PrepareError::one(PrepareErrorKind::Workflow, problem));
    }
    // See decision rationale at [§flm-dd-dr:feat-format-order]
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
/// file reader ([`ServiceFiles`]).
///
/// # Errors
///
/// A [`PrepareError`] when the input can no longer be read or its front matter is no longer
/// closed, and one of kind [`PrepareErrorKind::Run`] when the run cannot be made or fails as
/// a program (never for a document with errors, whose problems are in
/// [`Built::diagnostics`]); either carries *prepare*'s diagnostics
/// ([`Prepared::diagnostics`]) and lists the paths read up to that point, *prepare*'s
/// first.
pub fn build(prepared: &Prepared) -> Result<Built, PrepareError> {
    let reads = BuildFiles::new(Arc::clone(&prepared.files));
    build_reading(prepared, reads.clone()).map_err(|error| {
        let mut files_read = prepared.files_read();
        files_read.extend(reads.list());
        PrepareError { diagnostics: prepared.diagnostics.clone(), files_read, ..error }
    })
}

/// [`build`], reading through `reads`.
fn build_reading(prepared: &Prepared, reads: BuildFiles) -> Result<Built, PrepareError> {
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
    let mut run = Run::new(doc, Arc::clone(&prepared.plan), Externals::none())
        .map_err(|error| PrepareError::one(PrepareErrorKind::Run, Problem::new(error.to_string())))?;
    let mut options = RunOptions::new();
    options.continue_with_errors = prepared.request.continue_with_errors;
    // The services of the registry read through this build's reading while it runs.
    // See decision rationale at [§flm-dd-dr:host-service-files].
    let attached = prepared.service_files.attach(reads.clone());
    let executed = run.execute(&options);
    drop(attached);
    executed.map_err(|error| PrepareError::one(PrepareErrorKind::Run, Problem::new(error.to_string())))?;
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
}
