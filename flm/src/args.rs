//! The arguments of the commands `flm watch`, `flm render`, `flm check`, `flm schema` and
//! `flm info`, declared for the command-line parser `clap`, and their translation into a
//! request to the host library.
//!
//! The doc comments of the option fields below, of the variants of [`Command`], [`Layout`]
//! and [`SchemaOf`](crate::SchemaOf), and the `after_help` texts are what `flm --help` and
//! `flm COMMAND --help` print: each option's comment says what the option does, what value
//! it takes and, where it has one, its default. The doc comments of the structures
//! themselves are not printed. The long help of the option `-t` also lists the names of
//! the compose templates of the package `std`, by output, with the default marked:
//! [`Cli::command`] adds that list when the program starts ([`template_help`]). The
//! chapter "The flm command-line tool" of the guide, in the documentation of `flm-core`,
//! explains the same options with examples.
//!
//! [`BuildArgs::request`] turns the arguments shared by the commands that build a document
//! into a [`Request`] of the host library ([`flm_host::session`]), adding what the program
//! takes from its environment: the search path of extensions ([`search_path`]), the program that
//! converts images ([`MAGICK_VARIABLE`]) and the access to the machine that fetching cited
//! works needs (a network fetcher and a cache folder), the downloader of the files at `https`
//! addresses, and the program's cache
//! ([`crate::cache`]). The command `flm dump` is not declared here: its arguments
//! are parsed by [`crate::dump`].
//!
//! The program meets the system here: a path given on the command line or in the
//! environment is converted to the form in which the host writes paths, `/` between names
//! on every system ([`flm_path`]), and a path that is not valid Unicode text is refused
//! with a message naming it.

use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use flm_core::model::doc::FragmentLayout;
use flm_core::progress::LogSink;
use flm_host::bundled::Bundled;
use flm_host::cache::CacheHost;
use flm_host::citations::{CitationsHost, network_fetcher};
use flm_host::download::{Downloads, network_downloader};
use flm_host::files::BuildFiles;
use flm_host::files::system::{from_system, home_folder};
use flm_host::packages::ExtensionFolder;
use flm_host::seed::{MathChoice, read_front_matter, read_initial_use, read_math};
use flm_host::session::{GivenList, GivenMath, Input, Request};
use flm_host::settings::CommandLineSetting;

/// The command line of the `flm` program, as `clap` parses it: one of the commands `watch`,
/// `render`, `check`, `schema` and `info`, with its arguments, and the option
/// `--json-messages`. The command `flm dump` is not part of it: [`run`](crate::run)
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
    /// program does, and, every half second while a build runs, the progress of the build.
    /// Accepted before or after the command's name; `flm dump` does not accept it.
    #[arg(long = "json-messages", global = true)]
    pub json_messages: bool,
    /// The command to run, with its arguments.
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Returns the command line's declaration for `clap`, with, in the long help of the
    /// option `-t` of each command that builds a document, the compose templates of the
    /// package `std` listed by output.
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

/// Returns the list of the names of the compose templates of the package `std`
/// ([`flm_host::stdpkg::compose_standard::templates::std_templates`]), by output, in their
/// order, with the default marked, as the long help of `-t` prints it.
pub(crate) fn template_help() -> String {
    use flm_core::stdlib::features::compose::DEFAULT_TEMPLATE;
    use flm_host::stdpkg::compose_standard::templates::std_templates;
    // Each output with the names of its compose templates, in the order of their first one.
    let mut outputs: Vec<(String, Vec<String>)> = Vec::new();
    for template in std_templates().unwrap_or_default() {
        let name = if template.name == DEFAULT_TEMPLATE { format!("{} (default)", template.name) } else { template.name };
        match outputs.iter_mut().find(|(output, _)| *output == template.output) {
            Some((_, names)) => names.push(name),
            None => outputs.push((template.output, vec![name])),
        }
    }
    let mut text = String::from("The compose templates of `std`, by output:");
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
                            `PATH`; `MAGICK`, `FLM_CACHE_DIR` and `XDG_CACHE_HOME`, as for \
                            `flm render`.")]
    Watch(WatchArgs),
    /// Render a document and write its outputs to standard output, a file or a folder.
    #[command(after_help = "Environment variables: `FLMPATH`, a list of folders in which \
                            extensions are looked for, separated as in `PATH`; `MAGICK`, the program \
                            that converts an image from one format into another (default: \
                            ImageMagick's `magick`); `FLM_CACHE_DIR`, the folder of the cache \
                            (see `--cache-dir`); `XDG_CACHE_HOME`, a folder of caches, whose \
                            folder `flm` holds the cache when `FLM_CACHE_DIR` is not set and the \
                            variable holds an absolute path.")]
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
                            reports); `NO_COLOR`, set and not empty, whatever its value: the \
                            pages painted for a terminal show no color. On a terminal, the pages \
                            are painted for it (bold and colored headings, colored rules under \
                            the headings, tables in a frame, hyperlinks); in a file or a pipe, \
                            they are plain text. Nothing but the terminal and `NO_COLOR` shapes \
                            the pages: the options above shape the environment described.")]
    Info(InfoArgs),
    /// List the instances of the cache in which `flm render`, `flm check` and `flm watch`
    /// keep the results of costly work, or remove them.
    #[command(after_help = "Environment variables: `FLM_CACHE_DIR` and `XDG_CACHE_HOME`, as for \
                            `flm render`.")]
    Cache(crate::cache_command::CacheArgs),
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
    pub input: Option<PathBuf>,
    /// The document's text, given on the command line in place of a file (`render` and
    /// `check` only).
    #[arg(short = 'c', long = "content", value_name = "TEXT")]
    pub content: Option<String>,
    /// The profile to build with, which determines what kind of output is produced. An
    /// extension named `profile-PROFILE`, found on the search path or compiled into the
    /// program, is used after the document's packages when it exists; then the workflow
    /// PROFILE runs. The program's profiles `html`, `text`, `latex-rendered`,
    /// `latex-recomposed`, `bibtex`, `csl-json` and `check` are workflows of its own
    /// packages; `pdf-latex` and `tty` are the extensions `profile-pdf-latex` and
    /// `profile-tty` compiled into the program (`tty` is the text for a terminal, with
    /// bold, italics, underlining, colors and hyperlinks); any other profile is a workflow
    /// that a package in use declares, or an extension `profile-NAME` that declares it.
    /// Every format the workflow provides is produced, unless `-f` selects one of them.
    /// Default: for `flm render` and `flm watch`, the front matter's `default_profile`,
    /// else `html`; for `flm check`, `check`.
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
    /// The compose template TEMPLATE: one of `std`'s, or one an extension the document uses
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
    /// packages and the workflow it uses, and the files it writes, each list once the step
    /// that made it is over; while it fetches the details of cited works, each request it
    /// sends and each wait; and the lines that extensions write on their console at the
    /// level of information (their warnings and errors are printed without `-v`).
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
    /// The folder of the cache, in which the program keeps the results of costly work from
    /// one run to the next. Default: the folder of the environment variable
    /// `FLM_CACHE_DIR`; else the folder `flm` in the folder of `XDG_CACHE_HOME`, when that
    /// variable holds an absolute path; else the system's folder of caches:
    /// `~/.cache/flm` on Linux and other Unix systems, `~/Library/Caches/flm` on macOS, and
    /// `flm\cache` in the folder of local application data on Windows.
    #[arg(long = "cache-dir", value_name = "DIR", conflicts_with = "no_cache")]
    pub cache_dir: Option<PathBuf>,
    /// Read and write no cache on disk: the results are kept in memory, for this run only
    /// (for `flm watch`, until the program ends).
    #[arg(long = "no-cache")]
    pub no_cache: bool,
    /// The size that the cache should not exceed: a whole number of bytes, or a whole
    /// number followed by `K`, `M` or `G` for KiB, MiB or GiB (`--cache-max-size 500M`).
    /// Default: 2% of the drive that holds the cache, between 256 MiB and 2 GiB.
    #[arg(long = "cache-max-size", value_name = "SIZE", value_parser = crate::cache::parse_size, conflicts_with = "no_cache")]
    pub cache_max_size: Option<NonZeroU64>,
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
    pub output: Option<PathBuf>,
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
    /// The editor in which a place of the document opens, for this run, over the viewer
    /// settings' editor (`~/.flm/watch.json`) until an editor is chosen on the preview's
    /// page: `code`, `cursor`, `codium`, `zed`, `emacsclient`, `subl`, `gvim`, `mvim`, `vim`,
    /// `nvim`, or `custom` for the editor command. Default (and when NAME is not available): the editor of the viewer
    /// settings, else the editor command, else the first of the editors found, in this
    /// order, in the folders of `PATH` or, on macOS, installed as an application in
    /// `/Applications` or `~/Applications`.
    #[arg(long = "editor", value_name = "NAME")]
    pub editor: Option<String>,
    /// The command of an editor of your own, for this run, over the viewer settings'
    /// `editor_command`: words separated by spaces (a part between quotation marks keeps
    /// its spaces), the program first, run without a shell; `{file}`, `{line}` and
    /// `{column}` are replaced in each word, and the file is added as the last word when no
    /// word holds `{file}`. Example: `--editor-command "myeditor --goto {file}:{line}"`.
    #[arg(long = "editor-command", value_name = "COMMAND")]
    pub editor_command: Option<String>,
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
    // Hidden from the help. The crate documentation (`lib.rs`) described the option, in the
    // list of the commands:
    //   It prints the pages as text, or as JSON with `--json`; with `--write-flm FILE`, it
    //   writes one page as an FLM document, which `flm render` renders ([`InfoArgs`]).
    // and in the exit statuses:
    //   `flm info` exits with 0 when every page asked for was shown, or written with
    //   `--write-flm`, with or without warnings (a documentation text that does not parse,
    //   shown as plain text, is one); 1 when the file of `--write-flm` cannot be written; 2
    //   when the arguments are wrong, the document given with `-d` cannot be read or
    //   prepared, the environment cannot be prepared, a path is no page's path and no name it
    //   looks up, or the paths given with `--write-flm` do not choose exactly one page; and 3
    //   on a failure of the program itself, such as a page that does not render.
    /// Write the page as an FLM document to FILE, in place of printing it: the source that
    /// is otherwise rendered as text, which `flm render FILE` renders. `-` prints it on
    /// standard output. The paths given must choose exactly one page.
    #[arg(long = "write-flm", value_name = "FILE", conflicts_with_all = ["json", "recursive"], hide = true)]
    pub write_flm: Option<PathBuf>,
    /// The document whose environment is described: its front matter decides the packages
    /// it uses and the settings in effect. Default: the environment of a document that
    /// specifies nothing.
    #[arg(short = 'd', long = "document", value_name = "DOCUMENT")]
    pub document: Option<PathBuf>,
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
            // The command builds nothing, so it keeps nothing on disk.
            cache_dir: None,
            no_cache: true,
            cache_max_size: None,
        }
    }
}

impl BuildArgs {
    /// The options of the command line that change the build, each word as typed, in this
    /// order: each `--use`, `--initial-use`, `--math`, `-t`, each `-O`, `-B` or `--layout`,
    /// and each `--extensions-dir`. Left out: the document, `-c`, `-p` and `-f` (an export
    /// of the preview chooses its build type and writes every output of it), and `-v`.
    pub fn build_options(&self) -> Vec<String> {
        let mut words: Vec<String> = Vec::new();
        for package in &self.uses {
            words.extend([String::from("--use"), package.clone()]);
        }
        if let Some(packages) = &self.initial_use {
            words.extend([String::from("--initial-use"), packages.clone()]);
        }
        if let Some(processor) = &self.math {
            words.extend([String::from("--math"), processor.clone()]);
        }
        if let Some(template) = &self.template {
            words.extend([String::from("-t"), template.clone()]);
        }
        for setting in &self.options {
            words.extend([String::from("-O"), setting.clone()]);
        }
        if self.blocks {
            words.push(String::from("-B"));
        } else if let Some(layout) = self.layout {
            let name = match layout {
                Layout::Blocks => "blocks",
                Layout::Inline => "inline",
                Layout::Auto => "auto",
            };
            words.extend([String::from("--layout"), String::from(name)]);
        }
        for folder in &self.extensions_dirs {
            words.extend([String::from("--extensions-dir"), folder.display().to_string()]);
        }
        words
    }

    /// Returns the request to the host library ([`Request`]) that these arguments make.
    ///
    /// The request holds the input, the format, the profile, the initial list of
    /// `--initial-use` ([`Request::initial_use`]), the math processor of `--math`
    /// ([`Request::math`]), the packages of `--use` ([`Request::uses`]) and the layout, and
    /// carries `-t` and every `-O` as command-line settings, `-t` first so that a later
    /// `-O compose.template=…` replaces it. It writes nothing: the caller sets where the output
    /// goes ([`Request::naming`] and [`Request::sink`]). The request offers the extensions of
    /// `bundled`, the tables that the executable carries ([`Request::bundled`]), and the host's
    /// math processor is the program's over them ([`default_math`]). The request also receives
    /// what the program takes from its environment:
    ///
    /// - the search path of extensions: the folders of `--extensions-dir`, or else
    ///   `_flmextensions` beside the document; the folders of the environment variable `FLMPATH`; and
    ///   `.flm/extensions` in the home folder, which the environment variable `HOME` gives
    ///   (or `USERPROFILE` where `HOME` is not set or empty); the extensions of `bundled` are
    ///   found after all of these;
    /// - the program of the environment variable `MAGICK`, where it is set;
    /// - for fetching the details of cited works: a fetcher over the network
    ///   ([`network_fetcher`]) and the folder of the cache file (the document's folder, or the current folder for a document given as text);
    /// - for downloading the file at an `https` address: the downloader over the network
    ///   ([`network_downloader`], [`Request::downloads`]);
    /// - the program's cache ([`Request::cache`]): the manager of the cache, made on the
    ///   folder of `--cache-dir`, of the environment variable `FLM_CACHE_DIR`, of
    ///   `XDG_CACHE_HOME` or of the system's convention, with the size
    ///   limit of `--cache-max-size`, or kept in memory under `--no-cache` or when none of
    ///   these gives a folder; and the document's text, the canonical absolute path of its
    ///   file (none for a document given as text).
    ///
    /// When the input is `-`, this method reads standard input to its end.
    ///
    /// # Errors
    ///
    /// A message for a person when no document is given (no file and no `-c`), when the
    /// value of `--initial-use` is not valid YAML or is neither a package's name nor a list
    /// of packages' names, when the value of `--math` is not valid YAML or is neither a math
    /// processor's name nor `false`, when standard input cannot be read, or when a path of
    /// the command line or of the environment is not valid Unicode text
    /// ([`from_system`](flm_host::files::system::from_system)).
    pub fn request(&self, bundled: Bundled) -> Result<Request, String> {
        let initial_use = self.initial_use.as_deref().map(initial_use).transpose()?;
        let math = self.math.as_deref().map(math_processor).transpose()?;
        let input = match (&self.input, &self.content) {
            (_, Some(text)) => Input::Text(text.clone()),
            (Some(path), None) if path.as_os_str() == "-" => {
                let text = std::io::read_to_string(std::io::stdin())
                    .map_err(|error| format!("standard input cannot be read: {error}"))?;
                Input::Text(text)
            }
            (Some(path), None) => Input::File(flm_path(path)?),
            (None, None) => {
                return Err(String::from(
                    "a document must be given: a file, `-` for standard input, or `-c TEXT`",
                ));
            }
        };
        let folders = environment_search_path(&self.extensions_dirs, &input)?;
        let cache_folder = cache_folder(&input);
        let document = crate::cache::document_text(&input);
        let mut request = Request::new(input);
        request.default_math = default_math(&bundled);
        request.bundled = bundled;
        request.extension_folders = folders;
        // The program offers the extensions the host service `programs`, always.
        request.offer_programs = true;
        request.citations = CitationsHost {
            fetcher: Some(network_fetcher()),
            cache_folder: Some(cache_folder),
        };
        request.downloads = Downloads::new(network_downloader());
        if let Some(program) = std::env::var_os(MAGICK_VARIABLE) {
            request.magick_program = flm_path(Path::new(&program))?;
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
        // What the extensions write on their console goes through the reporter, as text or
        // as `log` records, the lines at the level `Info` under `-v` only; so do the
        // reports of the cache.
        let log: Arc<dyn LogSink> = Arc::new(crate::ProgramLog { verbose: self.verbose });
        let root = if self.no_cache { None } else { crate::cache::environment_root(self.cache_dir.as_deref()) };
        request.cache = CacheHost {
            manager: crate::cache::manager(root.as_deref(), self.cache_max_size, Arc::clone(&log)),
            document,
        };
        request.log_sink = Some(log);
        Ok(request)
    }
}

/// Returns the folder of the citations' cache file for the document `input`, as
/// [`BuildArgs::request`] gives it to the fetching of cited works: the folder of the
/// document's file, or the current folder (`.`) for a document given as text and for a file
/// given without a folder (`paper.flm`, whose folder is the empty path, which the fetching
/// would read as the root).
pub(crate) fn cache_folder(input: &Input) -> String {
    let beside = match input {
        Input::File(path) => BuildFiles::folder_of(path),
        Input::Text(_) => "",
    };
    String::from(if beside.is_empty() { "." } else { beside })
}

/// Returns `path`, a path given on the command line or in the environment, in the form in
/// which the host writes paths, `/` between names on every system
/// ([`from_system`](flm_host::files::system::from_system)).
///
/// # Errors
///
/// A message naming the path when it is not valid Unicode text, or, on Windows, when a path
/// relative to a drive (`C:x`) cannot be made absolute.
pub(crate) fn flm_path(path: &Path) -> Result<String, String> {
    from_system(path).map_err(|error| error.to_string())
}

/// Returns the program's math processor, used when neither `--math` nor the front matter's
/// `math` chooses one ([`Request::default_math`]), for a program that offers the extensions
/// of `bundled`: `mathjax`, the bundled extension `math-mathjax` that typesets formulas
/// with MathJax, when the program runs JavaScript extensions (cargo feature `js`) and
/// `bundled` offers `math-mathjax`; else `passthrough`, the library's package
/// `math-passthrough`, since the program lacks the JavaScript engine or the extension.
/// This default is not guaranteed to stay the same: a document that needs a stable result
/// chooses its processor with the front matter's `math`. A problem
/// with its package opens with the words of
/// [`DEFAULT_MATH_PROCESSOR`](flm_host::session::DEFAULT_MATH_PROCESSOR).
pub fn default_math(bundled: &Bundled) -> MathChoice {
    if cfg!(feature = "js") && bundled.stems().contains(&"math-mathjax") {
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
/// gives (or `USERPROFILE` where `HOME` is not set or empty;
/// [`home_folder`](flm_host::files::system::home_folder)).
///
/// # Errors
///
/// A message naming the path, when a folder of `named` or of `FLMPATH`, or the home folder,
/// is not valid Unicode text ([`flm_path`]).
pub(crate) fn environment_search_path(named: &[PathBuf], input: &Input) -> Result<Vec<ExtensionFolder>, String> {
    let home = home_folder().map_err(|error| error.to_string())?;
    search_path(named, input, std::env::var_os(FLMPATH_VARIABLE), home)
}

/// Returns the search path of [`environment_search_path`], each folder with where it comes
/// from ([`search_path_with_origins`]).
///
/// # Errors
///
/// The message of [`environment_search_path`].
pub(crate) fn environment_search_path_with_origins(
    named: &[PathBuf],
    input: &Input,
) -> Result<Vec<(ExtensionFolder, FolderOrigin)>, String> {
    let home = home_folder().map_err(|error| error.to_string())?;
    search_path_with_origins(named, input, std::env::var_os(FLMPATH_VARIABLE), home)
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
/// 2. the folders listed in `flmpath`, the value of [`FLMPATH_VARIABLE`], separated as the
///    system separates the folders of `PATH`, empty entries skipped;
/// 3. the folder [`USER_FOLDER`] in `home`, the user's home folder, when it is known.
///
/// Each folder of `named` and of `flmpath` is converted to the form in which the host writes
/// paths ([`flm_path`]); `home` is written in that form already.
///
/// # Errors
///
/// A message naming the path, when a folder of `named` or of `flmpath` is not valid Unicode
/// text.
pub fn search_path(
    named: &[PathBuf],
    input: &Input,
    flmpath: Option<OsString>,
    home: Option<String>,
) -> Result<Vec<ExtensionFolder>, String> {
    let folders = search_path_with_origins(named, input, flmpath, home)?;
    Ok(folders.into_iter().map(|(folder, _)| folder).collect())
}

/// Returns the search path of extensions of [`search_path`], each folder with where it
/// comes from, for the page `program/paths` of `flm info`.
fn search_path_with_origins(
    named: &[PathBuf],
    input: &Input,
    flmpath: Option<OsString>,
    home: Option<String>,
) -> Result<Vec<(ExtensionFolder, FolderOrigin)>, String> {
    let mut folders: Vec<(ExtensionFolder, FolderOrigin)> = Vec::new();
    for path in named {
        folders.push((ExtensionFolder { path: flm_path(path)?, required: true }, FolderOrigin::Option));
    }
    if folders.is_empty() {
        let beside = match input {
            Input::File(path) => Some(BuildFiles::resolve(BuildFiles::folder_of(path), DEFAULT_FOLDER)),
            Input::Text(_) => None,
        };
        let origin = if beside.is_some() { FolderOrigin::BesideDocument } else { FolderOrigin::CurrentFolder };
        folders.push((
            ExtensionFolder { path: beside.unwrap_or_else(|| String::from(DEFAULT_FOLDER)), required: false },
            origin,
        ));
    }
    let listed = flmpath.map(|value| std::env::split_paths(&value).collect::<Vec<_>>()).unwrap_or_default();
    for path in listed.iter().filter(|path| !path.as_os_str().is_empty()) {
        folders.push((ExtensionFolder { path: flm_path(path)?, required: false }, FolderOrigin::Flmpath));
    }
    if let Some(home) = home {
        let path = BuildFiles::resolve(&home, USER_FOLDER);
        folders.push((ExtensionFolder { path, required: false }, FolderOrigin::User));
    }
    Ok(folders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flm_host::bundled::BundledExtension;

    #[test]
    fn the_program_s_math_processor_is_mathjax_with_the_bundled_extensions_else_passthrough() {
        static MATHJAX: [BundledExtension; 1] = [BundledExtension { folder: "js", stem: "math-mathjax", files: &[] }];
        let offered = Bundled::new(&MATHJAX);
        let expected = if cfg!(feature = "js") { "mathjax" } else { "passthrough" };
        assert_eq!(default_math(&offered), MathChoice::processor(expected));
        assert_eq!(default_math(&Bundled::default()), MathChoice::processor("passthrough"));
    }

    #[test]
    fn math_is_a_processor_s_name_or_false_in_yaml_s_one_line_form() {
        assert_eq!(math_processor("passthrough"), Ok(MathChoice::processor("passthrough")));
        assert_eq!(math_processor("false"), Ok(MathChoice::NoProcessor));
        for refused in ["true", "", "[mathjax]", "a/b", "[a"] {
            assert!(math_processor(refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn initial_use_is_one_name_or_a_list_in_yaml_s_one_line_form() {
        assert_eq!(initial_use("std"), Ok(vec![String::from("std")]));
        assert_eq!(initial_use("[std, myext#svg]"), Ok(vec![String::from("std"), String::from("myext#svg")]));
        assert_eq!(initial_use("[]"), Ok(Vec::new()));
        for refused in ["{a: 1}", "", "[std, 3]", "[std"] {
            assert!(initial_use(refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn the_initial_list_of_the_command_line_reaches_the_request() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Check(build) => build,
            _ => panic!("check"),
        };
        let request = build(&["flm", "check", "--initial-use", "[std, mystyle]", "-c", "Hello."]).request(Bundled::default()).unwrap();
        let given = request.initial_use.expect("the command line's list");
        assert_eq!(given.packages, [String::from("std"), String::from("mystyle")]);
        assert_eq!(given.source, "the option `--initial-use`");
        assert_eq!(request.default_initial_use.packages, [String::from("std")]);
        let request = build(&["flm", "check", "-c", "Hello."]).request(Bundled::default()).unwrap();
        assert_eq!(request.initial_use, None);
        assert!(build(&["flm", "check", "--initial-use", "{a: 1}", "-c", "Hello."]).request(Bundled::default()).is_err());
    }

    #[test]
    fn the_packages_of_use_reach_the_request_in_order() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Render(render) => render.build,
            _ => panic!("render"),
        };
        let request = build(&["flm", "render", "--use", "two", "--use", "one", "-c", "Hello."]).request(Bundled::default()).unwrap();
        let given = request.uses.expect("the command line's packages");
        assert_eq!(given.packages, [String::from("two"), String::from("one")]);
        assert_eq!(given.source, "the option `--use`");
        assert_eq!(build(&["flm", "render", "-c", "Hello."]).request(Bundled::default()).unwrap().uses, None);
    }

    #[test]
    fn the_math_processor_of_the_command_line_reaches_the_request() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Render(render) => render.build,
            _ => panic!("render"),
        };
        let request = build(&["flm", "render", "--math", "passthrough", "-c", "Hello."]).request(Bundled::default()).unwrap();
        assert_eq!(request.math, Some(GivenMath::new(MathChoice::processor("passthrough"), "the option `--math`")));
        assert_eq!(request.default_math, default_math(&Bundled::default()));
        let request = build(&["flm", "render", "--math", "false", "-c", "Hello."]).request(Bundled::default()).unwrap();
        assert_eq!(request.math.map(|given| given.choice), Some(MathChoice::NoProcessor));
        let request = build(&["flm", "render", "-c", "Hello."]).request(Bundled::default()).unwrap();
        assert_eq!(request.math, None);
        assert_eq!(request.default_math, default_math(&Bundled::default()));
        assert!(build(&["flm", "render", "--math", "a/b", "-c", "Hello."]).request(Bundled::default()).is_err());
    }

    #[test]
    fn the_profile_of_the_command_line_reaches_the_request() {
        let build = |line: &[&str]| match Cli::parse_line(line).unwrap().command {
            Command::Render(render) => render.build,
            _ => panic!("render"),
        };
        let request = build(&["flm", "render", "-p", "pdf-latex", "-c", "Hello."]).request(Bundled::default()).unwrap();
        assert_eq!(request.profile.as_deref(), Some("pdf-latex"));
        let request = build(&["flm", "render", "--profile", "text", "-c", "Hello."]).request(Bundled::default()).unwrap();
        assert_eq!(request.profile.as_deref(), Some("text"));
        assert_eq!(build(&["flm", "render", "-c", "Hello."]).request(Bundled::default()).unwrap().profile, None);
    }

    #[test]
    fn the_search_path_lists_the_named_folders_then_flmpath_then_the_user_s() {
        let input = Input::File(String::from("doc/paper.flm"));
        let paths = |folders: Result<Vec<ExtensionFolder>, String>| -> Vec<(String, bool)> {
            folders.unwrap().into_iter().map(|folder| (folder.path, folder.required)).collect()
        };
        let flmpath = std::env::join_paths(["a", "b"]).ok();
        assert_eq!(
            paths(search_path(&[], &input, flmpath.clone(), Some(String::from("/home/u")))),
            [
                (String::from("doc/_flmextensions"), false),
                (String::from("a"), false),
                (String::from("b"), false),
                (String::from("/home/u/.flm/extensions"), false),
            ]
        );
        assert_eq!(paths(search_path(&[PathBuf::from("d")], &Input::Text(String::new()), None, None)), [(String::from("d"), true)]);
        assert_eq!(paths(search_path(&[], &Input::Text(String::new()), None, None)), [(String::from("_flmextensions"), false)]);
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_is_not_unicode_text_is_refused_with_a_message_naming_it() {
        use std::os::unix::ffi::OsStrExt;
        let named = PathBuf::from(std::ffi::OsStr::from_bytes(b"ext\xff"));
        let error = search_path(&[named], &Input::Text(String::new()), None, None).unwrap_err();
        assert!(error.contains("ext\u{fffd}"), "{error}");
        let flmpath = Some(OsString::from(std::ffi::OsStr::from_bytes(b"listed\xff")));
        let error = search_path(&[], &Input::Text(String::new()), flmpath, None).unwrap_err();
        assert!(error.contains("listed\u{fffd}"), "{error}");
    }
}
#[cfg(test)]
mod template_help_tests {
    use super::*;
    use flm_core::stdlib::features::compose::DEFAULT_TEMPLATE;

    #[test]
    fn the_long_help_of_t_lists_every_compose_template_of_std_by_output() {
        let templates = flm_host::stdpkg::compose_standard::templates::std_templates().unwrap();
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
