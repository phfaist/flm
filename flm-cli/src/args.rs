//! The arguments of the commands `flm watch`, `flm render`, `flm check`, `flm schema` and
//! `flm info`, declared for the command-line parser `clap`, and their translation into a
//! request to the host library.
//!
//! The doc comments of the option fields below, of the variants of [`Command`], [`Layout`]
//! and [`SchemaOf`](crate::SchemaOf), and the `after_help` texts are what `flm --help` and
//! `flm COMMAND --help` print: each option's comment says what the option does, what value
//! it takes and, where it has one, its default. The doc comments of the structures
//! themselves are not printed. The long help of the option `-t` also lists the names of
//! the compose templates of the package `stdmain`, by output, with the default marked:
//! [`Cli::command`] adds that list when the program starts ([`template_help`]). The chapter "The flm command-line tool" of the guide, in the
//! documentation of `flm-core`, explains the same options with examples.
//!
//! [`BuildArgs::request`] turns the arguments shared by the commands that build a document
//! into a [`Request`] of the host library ([`flm_main::session`]), adding what the program
//! takes from its environment: the search path of extensions ([`search_path`]), the program that
//! converts images ([`MAGICK_VARIABLE`]) and the access to the machine that fetching cited
//! works needs (a network fetcher and a cache folder). The command `flm dump` is not declared here: its arguments
//! are parsed by [`crate::dump`].

use std::path::PathBuf;
use std::sync::Arc;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use flm_core::model::doc::FragmentLayout;
use flm_main::citations::{CitationsHost, Event, FetchListener, Wait, network_fetcher};
use flm_main::packages::ExtensionFolder;
use flm_main::seed::{MathChoice, read_front_matter, read_initial_use, read_math};
use flm_main::session::{GivenList, GivenMath, Input, Request};
use flm_main::settings::CommandLineSetting;

use crate::reporter;

/// The command line of the `flm` program, as `clap` parses it: one of the commands `watch`,
/// `render`, `check`, `schema` and `info`, with its arguments, and the option
/// `--json-diagnostics`. The command `flm dump` is not part of it: [`run`](crate::run)
/// hands the arguments that follow `dump` to the parser of [`crate::dump`]. Before parsing,
/// `run` inserts `watch` when the first argument is not a command name, so that `flm INPUT`
/// is `flm watch INPUT`.
#[derive(Debug, Parser)]
#[command(
    name = "flm",
    version,
    about = "Preview, render and check documents written in FLM, a LaTeX-like markup language.",
    disable_version_flag = true,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Print the program's name and version, then exit.
    #[arg(long = "version", action = ArgAction::Version)]
    pub version: (),
    /// Write on standard error one JSON object per line in place of text: a header, then a
    /// record for each problem, each count of problems and each line that tells what the
    /// program does. Accepted before or after the command's name; `flm dump` does not
    /// accept it.
    #[arg(long = "json-diagnostics", global = true)]
    pub json_diagnostics: bool,
    /// The command to run, with its arguments.
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Returns the command line's declaration for `clap`, with, in the long help of the
    /// option `-t` of each command that builds a document, the compose templates of the
    /// package `stdmain` listed by output.
    pub fn command() -> clap::Command {
        let listing = template_help();
        let mut command = <Cli as clap::CommandFactory>::command();
        for name in ["watch", "render", "check"] {
            command = command.mut_subcommand(name, |subcommand| {
                subcommand.mut_arg("template", |arg| {
                    let help = arg.get_help().map(ToString::to_string).unwrap_or_default();
                    arg.long_help(format!("{help}\n\n{listing}"))
                })
            });
        }
        command
    }

    /// Parses the command line `line`, whose first item is the program's name, as
    /// [`Cli::command`] declares it.
    ///
    /// # Errors
    ///
    /// Returns `clap`'s error, which prints the usage or the help asked for.
    pub fn parse_line<I, T>(line: I) -> Result<Cli, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let matches = Cli::command().try_get_matches_from(line)?;
        <Cli as clap::FromArgMatches>::from_arg_matches(&matches)
    }
}

/// Returns the list of the names of the compose templates of the package `stdmain`
/// ([`flm_main::stdpkg::compose_standard::templates::std_templates`]), by output, in their
/// order, with the default marked, as the long help of `-t` prints it.
pub(crate) fn template_help() -> String {
    use flm_core::stdcore::features::compose::DEFAULT_TEMPLATE;
    use flm_main::stdpkg::compose_standard::templates::std_templates;
    // Each output with the names of its compose templates, in the order of their first one.
    let mut outputs: Vec<(String, Vec<String>)> = Vec::new();
    for template in std_templates().unwrap_or_default() {
        let name = if template.name == DEFAULT_TEMPLATE { format!("{} (default)", template.name) } else { template.name };
        match outputs.iter_mut().find(|(output, _)| *output == template.output) {
            Some((_, names)) => names.push(name),
            None => outputs.push((template.output, vec![name])),
        }
    }
    let mut text = String::from("The compose templates of `stdmain`, by output:");
    for (output, names) in outputs {
        text.push_str(&format!("\n  {output}: {}", names.join(", ")));
    }
    text
}

/// A command of the `flm` program and its arguments. The doc comment of each variant is the
/// command's description in `flm --help`.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show a document in the browser, and build it again whenever a file it reads changes
    /// (`flm INPUT` runs the same command).
    #[command(after_help = "Environment variables: `BROWSER`, the program that opens the \
                            preview's address, in place of the default browser; `FLMPATH`, a \
                            list of folders in which extensions are looked for, separated as in \
                            `PATH`; `MAGICK`, as for `flm render`.")]
    Watch(WatchArgs),
    /// Render a document and write its outputs to standard output, a file or a folder.
    #[command(after_help = "Environment variables: `FLMPATH`, a list of folders in which \
                            extensions are looked for, separated as in `PATH`; `MAGICK`, the program \
                            that converts an image from one format into another (default: \
                            ImageMagick's `magick`).")]
    Render(RenderArgs),
    /// Check a document and report its problems, without writing any output.
    Check(BuildArgs),
    /// Print the JSON Schema of a settings file, with which a text editor can check the file.
    Schema(crate::schema::SchemaArgs),
    /// Describe the environment in which a document is processed: the packages, the
    /// features and their commands, the settings, the workflows, the names the registry
    /// holds, and the program.
    #[command(after_help = "Environment variables: `FLMPATH`, a list of folders in which \
                            extensions are looked for, separated as in `PATH`; `COLUMNS`, the \
                            width of the terminal in columns, with which the pages are written \
                            when standard output is a terminal (default: the width the terminal \
                            reports). Nothing but the terminal's width shapes the pages: the \
                            options above shape the environment described.")]
    Info(InfoArgs),
}

/// The values of `--layout`: how the document's content is read at its top level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Layout {
    /// Paragraphs and blocks.
    Blocks,
    /// One run of text, with no paragraphs.
    Inline,
    /// Paragraphs and blocks when the content holds a paragraph break or a block, one run
    /// of text otherwise.
    Auto,
}

/// The arguments shared by the commands that build a document (`watch`, `render` and
/// `check`): the document, and how to build it. [`BuildArgs::request`] turns them into a
/// request to the host library.
#[derive(Debug, Clone, Args)]
pub struct BuildArgs {
    /// The document's file. For `render` and `check`, `-` reads the document from standard
    /// input instead.
    #[arg(value_name = "INPUT", conflicts_with = "content")]
    pub input: Option<String>,
    /// The document's text, given on the command line in place of a file (`render` and
    /// `check` only).
    #[arg(short = 'c', long = "content", value_name = "TEXT")]
    pub content: Option<String>,
    /// The profile to build with, which determines what kind of output is produced. An
    /// extension named `profile-PROFILE`, found on the search path or compiled into the
    /// program, is used after the document's packages when it exists; then the workflow
    /// PROFILE runs. The program's profiles `html`, `text`, `latex-rendered`,
    /// `latex-recomposed`, `bibtex`, `csl-json` and `check` are workflows of its own
    /// packages; `pdf-latex` is the extension `profile-pdf-latex` compiled into the program;
    /// any other profile is a workflow that a package in use declares, or an extension
    /// `profile-NAME` that declares it. Every format the workflow provides is produced,
    /// unless `-f` selects one of them. Default: for `flm render` and `flm watch`, the front
    /// matter's `default_profile`, else `html`; for `flm check`, `check`.
    #[arg(short = 'p', long = "profile", value_name = "PROFILE")]
    pub profile: Option<String>,
    /// Produce only FORMAT, one of the formats that the active workflow provides (the
    /// workflow of the profile chosen with `-p`, or of the default one). `-f` does not
    /// choose the profile: to produce another kind of output, choose its profile with `-p`.
    /// The program's workflows `html`, `text`, `latex-rendered`, `latex-recomposed`, `bibtex`
    /// and `csl-json` each provide the one format of the same name; a workflow that an
    /// extension declares may provide several. Default: every format the workflow provides.
    #[arg(short = 'f', long = "format", value_name = "FORMAT")]
    pub format: Option<String>,
    /// Use PACKAGE in addition to the packages that the document's front matter lists under
    /// `use`, as if it were written last in that list: a compiled package, or an extension
    /// found on the search path or compiled into the program (`--use syntax-highlight`
    /// highlights the code listings). Repeatable; the packages are used in the order given.
    #[arg(long = "use", value_name = "PACKAGE")]
    pub uses: Vec<String>,
    /// The packages to use before those that the document's front matter lists under
    /// `use`: one package's name, or a list written in YAML's one-line form, as `-O` takes
    /// values (`--initial-use '[std, myext]'`). These packages are used with the packages
    /// they build on; a package left out is not used unless the front matter's `use` lists
    /// it or it is the math processor's (`--math`). Takes precedence over the front
    /// matter's `initial_use`. Rarely needed, since it replaces the program's own list of
    /// packages: a package is added with `--use`. Default: the front matter's
    /// `initial_use`, else `std`.
    #[arg(long = "initial-use", value_name = "PACKAGES")]
    pub initial_use: Option<String>,
    /// The math processor, the package that processes the document's mathematics: a
    /// processor's name, which uses the package `math-NAME` (`--math passthrough` uses
    /// `math-passthrough`), or `false` for no processor, written in YAML's one-line form as
    /// the front matter's `math` is (`--math false`). The processor's package is used after
    /// the packages of the initial list and before those of the front matter's `use`. Takes
    /// precedence over the front matter's `math`. Default: the front matter's `math`, else
    /// `mathjax` (`passthrough` in a program built without JavaScript extensions or without
    /// the bundled extensions); this default is not guaranteed to stay the same.
    #[arg(long = "math", value_name = "PROCESSOR")]
    pub math: Option<String>,
    /// The compose template TEMPLATE: one of `stdmain`'s, or one an extension the document uses
    /// ships (the setting `compose.template`, written before every `-O`). Default: `simple`.
    #[arg(short = 't', long = "template", value_name = "TEMPLATE")]
    pub template: Option<String>,
    /// Set a feature's setting: `FEATURE.KEY=VALUE`, with VALUE written in YAML's one-line
    /// form (`-O text.width=60`), or `FEATURE.KEY.NAME=VALUE` to set the entry NAME of a
    /// collection of named entries (`-O floats.types.table=false`). Repeatable; a setting
    /// given here takes precedence over the front matter, the extensions and a
    /// workflow's settings.
    #[arg(short = 'O', long = "option", value_name = "FEATURE.KEY=VALUE")]
    pub options: Vec<String>,
    /// How the document's content is read at its top level. Default: the front matter's
    /// `layout`, else `auto`.
    #[arg(long = "layout", value_enum, conflicts_with = "blocks")]
    pub layout: Option<Layout>,
    /// Read the document's content as paragraphs and blocks (the same as `--layout blocks`).
    #[arg(short = 'B', long = "blocks")]
    pub blocks: bool,
    /// A folder in which extensions (`NAME.yaml`, `NAME/flmextension.yaml`) are looked for
    /// by name, before the folders of the environment variable `FLMPATH` and
    /// `~/.flm/extensions`, and the extensions compiled into the program last. Repeatable;
    /// each folder must exist. Default: the folder `_flmextensions` beside the document.
    #[arg(long = "extensions-dir", value_name = "DIR")]
    pub extensions_dirs: Vec<PathBuf>,
    /// Print on standard error what the program does: the files it reads, the profile, the
    /// packages and the workflow it uses, the requests it sends to fetch the details of cited
    /// works, and the files it writes.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
}

/// The arguments of `flm render`: the document and how to build it ([`BuildArgs`]), and
/// where the output goes.
#[derive(Debug, Clone, Args)]
pub struct RenderArgs {
    /// The document and how to build it.
    #[command(flatten)]
    pub build: BuildArgs,
    /// Where the output goes: a file, whose name gives the stem of every file written
    /// (`-o paper.html` writes `paper.html` and `paper.txt` for a workflow with the outputs
    /// `html` and `text`; `-o paper.tex` writes `paper.tex` and `paper-flmx.HASH.sty` for
    /// `latex-rendered`, and `paper.tex` and `paper-flmrecomposed.HASH.sty` for
    /// `latex-recomposed`); a folder (a path that ends with `/`, or an existing folder), in
    /// which the stem is `index`; or `-` for standard output, the default, which receives
    /// one file only.
    #[arg(short = 'o', long = "output", value_name = "OUTPUT")]
    pub output: Option<String>,
    /// Print the output on standard output without a final newline (by default, the output
    /// printed there ends with exactly one newline).
    #[arg(short = 'n', long = "no-newline")]
    pub no_newline: bool,
    /// Write the output even when the document has errors; the command still exits with
    /// status 1.
    #[arg(long = "continue-with-errors")]
    pub continue_with_errors: bool,
    /// Open the main file of the leading output (the output `-f` names, else the workflow's
    /// first), once written, with the system's default application for it. Requires `-o`.
    #[arg(short = 'V', long = "view")]
    pub view: bool,
}

/// The arguments of `flm watch`: the document and how to build it ([`BuildArgs`]; the
/// preview requires a file), and how the preview is served.
#[derive(Debug, Clone, Args)]
pub struct WatchArgs {
    /// The document and how to build it; the document must be a file, which the preview
    /// watches.
    #[command(flatten)]
    pub build: BuildArgs,
    /// The port of this machine at which the preview is served; 0 lets the system choose a
    /// free port. Default: 18910, or else the first free port among the 64 that follow it.
    #[arg(long = "port", value_name = "PORT")]
    pub port: Option<u16>,
    /// The editor that the page's menu of editors selects at first, when the browser has not
    /// remembered another choice: `code`, `zed`, `emacsclient`, `subl` or `gvim`. A place
    /// clicked on the page opens in the editor selected in that menu. Default (and when NAME
    /// is not found): the first of these, in this order, that is found in the folders of
    /// `PATH` or, on macOS, installed as an application in `/Applications` or
    /// `~/Applications`.
    #[arg(long = "editor", value_name = "NAME")]
    pub editor: Option<String>,
    /// Do not open the preview in the browser; its address is still printed.
    #[arg(long = "no-browser")]
    pub no_browser: bool,
    /// Accepted by the parser only to be refused with a message: the preview writes no
    /// output file (`flm render -o` does). Hidden from the help.
    #[arg(short = 'o', long = "output", value_name = "OUTPUT", hide = true)]
    pub output: Option<String>,
    /// Accepted by the parser only to be refused with a message: the preview opens the
    /// browser by itself (`flm render -V` opens a written output). Hidden from the help.
    #[arg(short = 'V', long = "view", hide = true)]
    pub view: bool,
}

/// The arguments of `flm info`: the pages to show, how to show them (as text, as JSON, or
/// written as an FLM document), and the environment they describe: the program's default
/// environment, or that of the document of `-d`, shaped by the same build options as `flm
/// render` takes ([`InfoArgs::build_args`]).
#[derive(Debug, Clone, Args)]
pub struct InfoArgs {
    /// The pages to show, each a page's path (`features/headings`,
    /// `features/headings/macros/section`, `settings/text`) or a name that the pages are
    /// looked up by: a command (`\section`, `section`), or the name of a package, a feature,
    /// a settings key, a workflow or anything else the registry holds
    /// (`flm.refs.resolve`). A name may give several pages, all shown. Default: `/`, the
    /// environment at a glance, which lists the other sections.
    #[arg(value_name = "PATH")]
    pub paths: Vec<String>,
    /// Print the pages as JSON: an object with a header (`meta`) and the list of the pages
    /// (`pages`), each with its path, kind, title, summary, content and the pages directly
    /// below it.
    #[arg(long = "json")]
    pub json: bool,
    /// Show every page below each page, in full: as JSON, each page's children are full
    /// pages; as text, the pages below follow each page.
    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,
    /// Write the page as an FLM document to FILE, in place of printing it: the source that
    /// is otherwise rendered as text, which `flm render FILE` renders. `-` prints it on
    /// standard output. The paths given must choose exactly one page.
    #[arg(long = "write-flm", value_name = "FILE", conflicts_with_all = ["json", "recursive"])]
    pub write_flm: Option<String>,
    /// The document whose environment is described: its front matter decides the packages
    /// it uses and the settings in effect. Default: the environment of a document that
    /// specifies nothing.
    #[arg(short = 'd', long = "document", value_name = "DOCUMENT")]
    pub document: Option<String>,
    /// The profile in view: the workflow pages show the plan of its workflow, and the
    /// package of an extension named `profile-PROFILE`, when one is found on the search path
    /// or compiled into the program, is among the packages described. Default: the front
    /// matter's `default_profile` (with `-d`), else `html`.
    #[arg(short = 'p', long = "profile", value_name = "PROFILE")]
    pub profile: Option<String>,
    /// Use PACKAGE in addition to the packages that the document's front matter lists under
    /// `use`, as if it were written last in that list: a compiled package, or an extension
    /// found on the search path or compiled into the program (`--use syntax-highlight`).
    /// Repeatable; the packages are used in the order given.
    #[arg(long = "use", value_name = "PACKAGE")]
    pub uses: Vec<String>,
    /// The packages to use before those that the document's front matter lists under
    /// `use`: one package's name, or a list written in YAML's one-line form, as `-O` takes
    /// values (`--initial-use '[std, myext]'`). These packages are used with the packages
    /// they build on; a package left out is not used unless the front matter's `use` lists
    /// it or it is the math processor's (`--math`). Takes precedence over the front
    /// matter's `initial_use`. Rarely needed, since it replaces the program's own list of
    /// packages: a package is added with `--use`. Default: the front matter's
    /// `initial_use`, else `std`.
    #[arg(long = "initial-use", value_name = "PACKAGES")]
    pub initial_use: Option<String>,
    /// The math processor, the package that processes the document's mathematics: a
    /// processor's name, which uses the package `math-NAME` (`--math passthrough` uses
    /// `math-passthrough`), or `false` for no processor, written in YAML's one-line form as
    /// the front matter's `math` is (`--math false`). The processor's package is used after
    /// the packages of the initial list and before those of the front matter's `use`. Takes
    /// precedence over the front matter's `math`. Default: the front matter's `math`, else
    /// `mathjax` (`passthrough` in a program built without JavaScript extensions or without
    /// the bundled extensions); this default is not guaranteed to stay the same.
    #[arg(long = "math", value_name = "PROCESSOR")]
    pub math: Option<String>,
    /// A folder in which extensions (`NAME.yaml`, `NAME/flmextension.yaml`) are looked for
    /// by name, before the folders of the environment variable `FLMPATH` and
    /// `~/.flm/extensions`, and the extensions compiled into the program last. Repeatable;
    /// each folder must exist. Default: the folder `_flmextensions` beside the document of
    /// `-d`, else in the current folder.
    #[arg(long = "extensions-dir", value_name = "DIR")]
    pub extensions_dirs: Vec<PathBuf>,
    /// Set a feature's setting: `FEATURE.KEY=VALUE`, with VALUE written in YAML's one-line
    /// form (`-O text.width=60`), or `FEATURE.KEY.NAME=VALUE` to set the entry NAME of a
    /// collection of named entries (`-O floats.types.table=false`). Repeatable; a setting
    /// given here takes precedence over the front matter, the extensions and a workflow's
    /// settings. It shapes the environment described, not the text of the pages
    /// (`-O text.width=60` shows on the page `settings/text`).
    #[arg(short = 'O', long = "option", value_name = "FEATURE.KEY=VALUE")]
    pub options: Vec<String>,
}

impl InfoArgs {
    /// Returns the arguments of a build that make the environment these arguments describe,
    /// so that `flm info` reads its build options as `flm render` reads them
    /// ([`BuildArgs::request`]): the document of `-d`, or else an empty text given on the
    /// command line (`-c ''`), which specifies nothing and whose extensions are looked for
    /// in the current folder; and the profile, the packages, the math processor, the
    /// folders of extensions and the settings given.
    pub fn build_args(&self) -> BuildArgs {
        BuildArgs {
            input: self.document.clone(),
            content: self.document.is_none().then(String::new),
            format: None,
            profile: self.profile.clone(),
            uses: self.uses.clone(),
            initial_use: self.initial_use.clone(),
            math: self.math.clone(),
            template: None,
            options: self.options.clone(),
            layout: None,
            blocks: false,
            extensions_dirs: self.extensions_dirs.clone(),
            verbose: false,
        }
    }
}

impl BuildArgs {
    /// Returns the request to the host library ([`Request`]) that these arguments make.
    ///
    /// The request holds the input, the format, the profile, the initial list of
    /// `--initial-use` ([`Request::initial_use`]), the math processor of `--math`
    /// ([`Request::math`]), the packages of `--use` ([`Request::uses`]) and the layout, and
    /// carries `-t` and every `-O` as command-line settings, `-t` first so that a later
    /// `-O compose.template=…` replaces it. It writes nothing: the caller sets where the output
    /// goes ([`Request::naming`] and [`Request::sink`]). The host's math processor is the
    /// program's ([`default_math`]). The request also receives what the program takes from its
    /// environment:
    ///
    /// - the search path of extensions: the folders of `--extensions-dir`, or else
    ///   `_flmextensions` beside the document; the folders of the environment variable `FLMPATH`; and
    ///   `.flm/extensions` in the home folder, which the environment variable `HOME` gives
    ///   (or `USERPROFILE` where `HOME` is not set); the extensions compiled into the program
    ///   (cargo feature `bundled-extensions`) are found after all of these;
    /// - the program of the environment variable `MAGICK`, where it is set;
    /// - for fetching the details of cited works: a fetcher over the network
    ///   ([`network_fetcher`]), the folder of the cache file (the document's folder, or the current folder for a document given as text) and,
    ///   under `-v`, a listener that prints the progress of each fetch on standard error.
    ///
    /// When the input is `-`, this method reads standard input to its end.
    ///
    /// # Errors
    ///
    /// A message for a person when no document is given (no file and no `-c`), when the
    /// value of `--initial-use` is not valid YAML or is neither a package's name nor a list
    /// of packages' names, when the value of `--math` is not valid YAML or is neither a math
    /// processor's name nor `false`, or when standard input cannot be read.
    pub fn request(&self) -> Result<Request, String> {
        let initial_use = self.initial_use.as_deref().map(initial_use).transpose()?;
        let math = self.math.as_deref().map(math_processor).transpose()?;
        let input = match (&self.input, &self.content) {
            (_, Some(text)) => Input::Text(text.clone()),
            (Some(path), None) if path == "-" => {
                let text = std::io::read_to_string(std::io::stdin())
                    .map_err(|error| format!("standard input cannot be read: {error}"))?;
                Input::Text(text)
            }
            (Some(path), None) => Input::File(PathBuf::from(path)),
            (None, None) => {
                return Err(String::from(
                    "a document must be given: a file, `-` for standard input, or `-c TEXT`",
                ));
            }
        };
        let folders = environment_search_path(&self.extensions_dirs, &input);
        let cache_folder = cache_folder(&input);
        let mut request = Request::new(input);
        request.default_math = default_math();
        request.extension_folders = folders;
        // The program offers the extensions the host service `programs`, always.
        request.offer_programs = true;
        request.citations = CitationsHost {
            fetcher: Some(network_fetcher()),
            cache_folder: Some(cache_folder),
            listener: self.verbose.then(|| Arc::new(TellFetching) as Arc<dyn FetchListener>),
        };
        if let Some(program) = std::env::var_os(MAGICK_VARIABLE) {
            request.magick_program = program;
        }
        request.format = self.format.clone();
        request.profile = self.profile.clone();
        request.initial_use = initial_use.map(|names| GivenList::new(names, "the option `--initial-use`"));
        request.math = math.map(|choice| GivenMath::new(choice, "the option `--math`"));
        request.uses = (!self.uses.is_empty()).then(|| GivenList::new(self.uses.clone(), "the option `--use`"));
        // `-t` is the assignment of one `-O`, written first, so that an `-O` of the same
        // key, written after it, wins as the later of two values of one source does; it
        // keeps its own words as its label. The name is written in YAML's single quotes,
        // which read it as a text whatever it is.
        request.options = self
            .template
            .iter()
            .map(|name| {
                CommandLineSetting::new(
                    format!("compose.template='{}'", name.replace('\'', "''")),
                    format!("-t {name}"),
                )
            })
            .chain(self.options.iter().map(|text| CommandLineSetting::new(text.clone(), format!("-O {text}"))))
            .collect();
        request.layout = match (self.blocks, self.layout) {
            (true, _) | (false, Some(Layout::Blocks)) => Some(FragmentLayout::Blocks),
            (false, Some(Layout::Inline)) => Some(FragmentLayout::Inline),
            (false, Some(Layout::Auto)) => Some(FragmentLayout::InlineOrBlocks),
            (false, None) => None,
        };
        // Under `--json-diagnostics`, every line on standard error is a record: the `console`
        // output of the JavaScript and WebAssembly extensions becomes `log` records. (The
        // extensions compiled into the program write theirs as text: the host library gives
        // no way to send it elsewhere.)
        #[cfg(any(feature = "js", feature = "wasm"))]
        if crate::reporter() == crate::Reporter::Json {
            #[cfg(feature = "js")]
            {
                request.js_host = request.js_host.clone().with_console(crate::console_records("js"));
            }
            #[cfg(feature = "wasm")]
            {
                request.wasm_host = request.wasm_host.clone().with_console(crate::console_records("wasm"));
            }
        }
        Ok(request)
    }
}

/// Returns the folder of the citations' cache file for the document `input`, as
/// [`BuildArgs::request`] gives it to the fetching of cited works: the folder of the
/// document's file, or the current folder (`.`) for a document given as text and for a file
/// given without a folder (`paper.flm`, whose parent is the empty path, which the fetching
/// would read as the root).
pub(crate) fn cache_folder(input: &Input) -> PathBuf {
    let beside = match input {
        Input::File(path) => path.parent().filter(|parent| !parent.as_os_str().is_empty()),
        Input::Text(_) => None,
    };
    beside.map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// Returns the program's math processor, used when neither `--math` nor the front matter's
/// `math` chooses one ([`Request::default_math`]): `mathjax`, the bundled extension
/// `math-mathjax` that typesets formulas with MathJax, when the program runs JavaScript
/// extensions and carries the bundled extensions (cargo features `js` and
/// `bundled-extensions`); else `passthrough`, the library's package `math-passthrough`,
/// since the program lacks the JavaScript engine or the bundled extensions that
/// `math-mathjax` needs. This default is not guaranteed to stay the same: a document that
/// needs a stable result chooses its processor with the front matter's `math`. A problem
/// with its package opens with the words of
/// [`DEFAULT_MATH_PROCESSOR`](flm_main::session::DEFAULT_MATH_PROCESSOR).
pub fn default_math() -> MathChoice {
    if cfg!(all(feature = "js", feature = "bundled-extensions")) {
        MathChoice::processor("mathjax")
    } else {
        MathChoice::processor("passthrough")
    }
}

/// Reads `text`, the value of `--initial-use`: one package's name, or a list of names in
/// YAML's one-line form ([`read_initial_use`]).
///
/// # Errors
///
/// A message for a person when `text` is not valid YAML, or is neither a package's name nor
/// a list of packages' names (one sentence per entry at fault).
fn initial_use(text: &str) -> Result<Vec<String>, String> {
    let node = read_front_matter(text, 1)
        .map_err(|error| format!("the value of `--initial-use` must be valid YAML: {}", error.detail))?;
    let names = read_initial_use(&node, "--initial-use").map_err(|errors| {
        errors.iter().map(|error| error.detail.as_str()).collect::<Vec<_>>().join("; ")
    })?;
    Ok(names.into_iter().map(|(name, _)| name).collect())
}

/// Reads `text`, the value of `--math`: a math processor's name, or `false`, in YAML's
/// one-line form ([`read_math`]).
///
/// # Errors
///
/// A message for a person when `text` is not valid YAML, or is neither a math processor's
/// name nor `false`.
fn math_processor(text: &str) -> Result<MathChoice, String> {
    let node = read_front_matter(text, 1)
        .map_err(|error| format!("the value of `--math` must be valid YAML: {}", error.detail))?;
    read_math(&node, "--math")
        .map_err(|errors| errors.iter().map(|error| error.detail.as_str()).collect::<Vec<_>>().join("; "))
}

/// The listener that `-v` gives to the fetching of the details of cited works: it prints a
/// line on standard error for each pass over the citation keys, each request sent, and each
/// wait, with its duration.
struct TellFetching;

impl FetchListener for TellFetching {
    fn hear(&self, event: &Event<'_>) {
        match event {
            Event::PassStarted { pass, cached, to_fetch } => {
                reporter().log(&format!("citations, pass {pass}: {to_fetch} to fetch, {cached} from the cache"));
            }
            Event::RequestStarted { url, .. } => reporter().log(&format!("fetching {url}")),
            Event::WaitStarted { what: Wait::RateLimit { prefix }, expected } => {
                reporter().log(&format!("waiting {} before the next request for `{prefix}:`", seconds(*expected)));
            }
            Event::WaitStarted { what: Wait::Backoff { url, attempt }, expected } => {
                reporter().log(&format!("waiting {} before trying {url} again (attempt {})", seconds(*expected), attempt + 2));
            }
            _ => {}
        }
    }
}

/// A duration as `-v` prints it, in seconds with one decimal (`3.1 s`), or `a moment`
/// when the duration is unknown.
fn seconds(duration: Option<std::time::Duration>) -> String {
    duration.map_or_else(|| String::from("a moment"), |duration| format!("{:.1} s", duration.as_secs_f64()))
}

/// The folder, beside the document, in which extensions are looked for when no
/// `--extensions-dir` is given: `_flmextensions`.
pub const DEFAULT_FOLDER: &str = "_flmextensions";

/// The environment variable that lists folders in which extensions are looked for,
/// `FLMPATH`, separated as the system separates the folders of `PATH`.
pub const FLMPATH_VARIABLE: &str = "FLMPATH";

/// The environment variable `MAGICK`: the ImageMagick program that the image conversions
/// run to convert an image from one format into another. Its value is handed to the
/// operating system as written: a program name, looked for in the folders of `PATH`, or a
/// path. Where the variable is not set, the program is `magick`.
pub const MAGICK_VARIABLE: &str = "MAGICK";

/// The user's folder of extensions, relative to the user's home folder:
/// `.flm/extensions`.
pub const USER_FOLDER: &str = ".flm/extensions";

/// Returns the search path of extensions for the document `input` and the folders `named`
/// of `--extensions-dir`, as [`search_path`] makes it from the process's environment: the
/// value of [`FLMPATH_VARIABLE`], and the home folder that the environment variable `HOME`
/// gives (or `USERPROFILE` where `HOME` is not set).
pub(crate) fn environment_search_path(named: &[PathBuf], input: &Input) -> Vec<ExtensionFolder> {
    search_path(named, input, std::env::var_os(FLMPATH_VARIABLE), home_folder())
}

/// Returns the search path of [`environment_search_path`], each folder with where it comes
/// from ([`search_path_with_origins`]).
pub(crate) fn environment_search_path_with_origins(named: &[PathBuf], input: &Input) -> Vec<(ExtensionFolder, FolderOrigin)> {
    search_path_with_origins(named, input, std::env::var_os(FLMPATH_VARIABLE), home_folder())
}

/// The user's home folder, which the environment variable `HOME` gives (or `USERPROFILE`
/// where `HOME` is not set), when either is set.
fn home_folder() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Where a folder of the search path of extensions comes from ([`search_path_with_origins`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FolderOrigin {
    /// A folder given with `--extensions-dir`.
    Option,
    /// The folder [`DEFAULT_FOLDER`] beside the document's file.
    BesideDocument,
    /// The folder [`DEFAULT_FOLDER`] in the current folder, for a document given as text.
    CurrentFolder,
    /// A folder listed in the environment variable [`FLMPATH_VARIABLE`].
    Flmpath,
    /// The folder [`USER_FOLDER`] in the user's home folder.
    User,
}

/// Returns the search path of extensions: the folders in which an extension is looked for
/// by its locator, in this order:
///
/// 1. the folders given with `--extensions-dir` (`named`), each required to exist; or, when
///    `named` is empty, the folder [`DEFAULT_FOLDER`] beside the document's file (in the
///    current folder for a document given as text), which may be missing;
/// 2. the folders listed in `flmpath`, the value of [`FLMPATH_VARIABLE`], empty entries
///    skipped;
/// 3. the folder [`USER_FOLDER`] in `home`, the user's home folder, when it is known.
pub fn search_path(
    named: &[PathBuf],
    input: &Input,
    flmpath: Option<std::ffi::OsString>,
    home: Option<PathBuf>,
) -> Vec<ExtensionFolder> {
    search_path_with_origins(named, input, flmpath, home).into_iter().map(|(folder, _)| folder).collect()
}

/// Returns the search path of extensions of [`search_path`], each folder with where it
/// comes from, for the page `program/paths` of `flm info`.
fn search_path_with_origins(
    named: &[PathBuf],
    input: &Input,
    flmpath: Option<std::ffi::OsString>,
    home: Option<PathBuf>,
) -> Vec<(ExtensionFolder, FolderOrigin)> {
    let mut folders: Vec<(ExtensionFolder, FolderOrigin)> = named
        .iter()
        .map(|path| (ExtensionFolder { path: path.clone(), required: true }, FolderOrigin::Option))
        .collect();
    if folders.is_empty() {
        let beside = match input {
            Input::File(path) => path.parent().map(|parent| parent.join(DEFAULT_FOLDER)),
            Input::Text(_) => None,
        };
        let origin = if beside.is_some() { FolderOrigin::BesideDocument } else { FolderOrigin::CurrentFolder };
        folders.push((
            ExtensionFolder { path: beside.unwrap_or_else(|| PathBuf::from(DEFAULT_FOLDER)), required: false },
            origin,
        ));
    }
    let listed = flmpath.map(|value| std::env::split_paths(&value).collect::<Vec<_>>()).unwrap_or_default();
    folders.extend(
        listed
            .into_iter()
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| (ExtensionFolder { path, required: false }, FolderOrigin::Flmpath)),
    );
    folders.extend(
        home.map(|home| (ExtensionFolder { path: home.join(USER_FOLDER), required: false }, FolderOrigin::User)),
    );
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_s_math_processor_is_mathjax_with_the_bundled_extensions_else_passthrough() {
        let expected = if cfg!(all(feature = "js", feature = "bundled-extensions")) { "mathjax" } else { "passthrough" };
        assert_eq!(default_math(), MathChoice::processor(expected));
    }

    #[test]
    fn math_is_a_processor_s_name_or_false_in_yaml_s_one_line_form() {
        assert_eq!(math_processor("passthrough"), Ok(MathChoice::processor("passthrough")));
        assert_eq!(math_processor("false"), Ok(MathChoice::NoProcessor));
        let shape = String::from("`--math` must be a math processor's name, such as `mathjax` or `passthrough`, or `false`");
        assert_eq!(math_processor("true"), Err(shape.clone()));
        assert_eq!(math_processor(""), Err(shape.clone()));
        assert_eq!(math_processor("[mathjax]"), Err(shape));
        assert_eq!(
            math_processor("a/b"),
            Err(String::from("`--math` must be a math processor's name, made of letters, digits, `-` and `_` only, not `a/b`"))
        );
        assert!(math_processor("[a").unwrap_err().starts_with("the value of `--math` must be valid YAML: "));
    }

    #[test]
    fn initial_use_is_one_name_or_a_list_in_yaml_s_one_line_form() {
        assert_eq!(initial_use("std"), Ok(vec![String::from("std")]));
        assert_eq!(initial_use("[std, myext#svg]"), Ok(vec![String::from("std"), String::from("myext#svg")]));
        assert_eq!(initial_use("[]"), Ok(Vec::new()));
        assert_eq!(
            initial_use("{a: 1}"),
            Err(String::from("`--initial-use` must be a package's name or a list of packages' names"))
        );
        assert_eq!(initial_use(""), Err(String::from("`--initial-use` must be a package's name or a list of packages' names")));
        assert_eq!(
            initial_use("[std, 3]"),
            Err(String::from("the entry `3` of `--initial-use` must be a package's name, written as a text"))
        );
        assert!(initial_use("[std").unwrap_err().starts_with("the value of `--initial-use` must be valid YAML: "));
    }

    #[test]
    fn the_initial_list_of_the_command_line_reaches_the_request() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Check(build) => build,
            _ => panic!("check"),
        };
        let request = build(&["flm", "check", "--initial-use", "[stdcore, std]", "-c", "Hello."]).request().unwrap();
        let given = request.initial_use.expect("the command line's list");
        assert_eq!(given.packages, [String::from("stdcore"), String::from("std")]);
        assert_eq!(given.source, "the option `--initial-use`");
        assert_eq!(request.default_initial_use.packages, [String::from("std")]);
        let request = build(&["flm", "check", "-c", "Hello."]).request().unwrap();
        assert_eq!(request.initial_use, None);
        assert!(build(&["flm", "check", "--initial-use", "{a: 1}", "-c", "Hello."]).request().is_err());
    }

    #[test]
    fn the_packages_of_use_reach_the_request_in_order() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Render(render) => render.build,
            _ => panic!("render"),
        };
        let request = build(&["flm", "render", "--use", "two", "--use", "one", "-c", "Hello."]).request().unwrap();
        let given = request.uses.expect("the command line's packages");
        assert_eq!(given.packages, [String::from("two"), String::from("one")]);
        assert_eq!(given.source, "the option `--use`");
        assert_eq!(build(&["flm", "render", "-c", "Hello."]).request().unwrap().uses, None);
    }

    #[test]
    fn the_math_processor_of_the_command_line_reaches_the_request() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Render(render) => render.build,
            _ => panic!("render"),
        };
        let request = build(&["flm", "render", "--math", "passthrough", "-c", "Hello."]).request().unwrap();
        assert_eq!(request.math, Some(GivenMath::new(MathChoice::processor("passthrough"), "the option `--math`")));
        assert_eq!(request.default_math, default_math());
        let request = build(&["flm", "render", "--math", "false", "-c", "Hello."]).request().unwrap();
        assert_eq!(request.math.map(|given| given.choice), Some(MathChoice::NoProcessor));
        let request = build(&["flm", "render", "-c", "Hello."]).request().unwrap();
        assert_eq!(request.math, None);
        assert_eq!(request.default_math, default_math());
        assert!(build(&["flm", "render", "--math", "a/b", "-c", "Hello."]).request().is_err());
    }

    #[test]
    fn the_profile_of_the_command_line_reaches_the_request() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Render(render) => render.build,
            _ => panic!("render"),
        };
        let request = build(&["flm", "render", "-p", "pdf-latex", "-c", "Hello."]).request().unwrap();
        assert_eq!(request.profile.as_deref(), Some("pdf-latex"));
        let request = build(&["flm", "render", "--profile", "text", "-c", "Hello."]).request().unwrap();
        assert_eq!(request.profile.as_deref(), Some("text"));
        assert_eq!(build(&["flm", "render", "-c", "Hello."]).request().unwrap().profile, None);
    }

    #[test]
    fn the_search_path_lists_the_named_folders_then_flmpath_then_the_user_s() {
        let input = Input::File(PathBuf::from("doc/paper.flm"));
        let paths = |folders: Vec<ExtensionFolder>| -> Vec<(PathBuf, bool)> {
            folders.into_iter().map(|folder| (folder.path, folder.required)).collect()
        };
        let flmpath = std::env::join_paths(["a", "b"]).ok();
        assert_eq!(
            paths(search_path(&[], &input, flmpath.clone(), Some(PathBuf::from("/home/u")))),
            [
                (PathBuf::from("doc/_flmextensions"), false),
                (PathBuf::from("a"), false),
                (PathBuf::from("b"), false),
                (PathBuf::from("/home/u/.flm/extensions"), false),
            ]
        );
        assert_eq!(paths(search_path(&[PathBuf::from("d")], &Input::Text(String::new()), None, None)), [(PathBuf::from("d"), true)]);
        assert_eq!(paths(search_path(&[], &Input::Text(String::new()), None, None)), [(PathBuf::from("_flmextensions"), false)]);
    }
}
#[cfg(test)]
mod template_help_tests {
    use super::*;
    use flm_core::stdcore::features::compose::DEFAULT_TEMPLATE;

    #[test]
    fn the_long_help_of_t_lists_every_compose_template_of_std_by_output() {
        let templates = flm_main::stdpkg::compose_standard::templates::std_templates().unwrap();
        let mut command = Cli::command();
        for name in ["watch", "render", "check"] {
            let subcommand = command.find_subcommand_mut(name).unwrap();
            let arg = subcommand.get_arguments().find(|arg| arg.get_id() == "template").unwrap();
            let help = arg.get_long_help().unwrap().to_string();
            for template in &templates {
                // The line of the template's output, which names it.
                let start = help.find(&format!("\n  {}: ", template.output)).unwrap() + 1;
                let line = help[start..].lines().next().unwrap();
                let names: Vec<&str> = line.split_once(": ").unwrap().1.split(", ").collect();
                let listed = if template.name == DEFAULT_TEMPLATE {
                    format!("{} (default)", template.name)
                } else {
                    template.name.clone()
                };
                assert!(names.contains(&listed.as_str()), "{name}: {help}");
            }
        }
    }
}
