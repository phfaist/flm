//! The command line of FLM: the library behind the `flm` program. It turns the program's
//! arguments into calls of the host library [`flm_main`], and turns what the host returns
//! into text on the standard streams and an exit status.
//!
//! FLM is a LaTeX-like markup language. The library [`flm_core`] turns the text of a
//! document into outputs and reads or writes no file; the host library [`flm_main`] reads
//! the files, combines the settings and writes the outputs. This crate adds what only an
//! executable has: the process (its arguments, its standard streams, its exit status) and
//! the list of compiled packages that the executable carries. The `flm` binary is one call
//! of [`run`]. A program that carries other compiled packages is a `main` function of the
//! same shape that passes its own list to [`run`]; the example `own_package` of this crate
//! does so for the package `phfpack`.
//!
//! # The commands
//!
//! - `flm watch INPUT`, or `flm INPUT`, is the live preview. It builds the document into an
//!   HTML page, serves the page to a browser on this machine and opens the browser on it.
//!   Whenever a file that the document reads changes, it builds the document again, and the
//!   open page updates itself. It runs until Ctrl+C. A click on the page with modifier keys
//!   opens the clicked element's place in an editor ([`WatchArgs`]).
//! - `flm render INPUT` renders a document into every output of the workflow that runs (the
//!   workflow of the profile that `-p` gives, else of the front matter's `default_profile`,
//!   else `html`), or the one output `-f` names, and writes the files to standard output,
//!   which receives one file only, or to the file or the folder given with `-o`, whose name
//!   gives every file's stem; with `-V`, it then opens the written main file of the leading
//!   output (the output `-f` names, else the workflow's first) ([`RenderArgs`]).
//! - `flm check INPUT` reports the problems of a document and writes no output
//!   ([`BuildArgs`]).
//! - `flm schema` prints the JSON Schema of the settings files, an extension's manifest or a
//!   front matter, or writes it to the file given with `-o`; given a document, the schema
//!   covers the packages that the document uses ([`SchemaArgs`]).
//! - `flm info [PATH…]` describes the environment in which a document is processed: the
//!   packages, the features and the commands they declare, the settings, the workflows, the
//!   names the registry holds and the program itself, as pages of documentation, each with
//!   a path (`features/headings`); a name that is no page's path is looked up
//!   (`flm info '\section'`). It prints the pages as text, or as JSON with `--json`
//!   ([`InfoArgs`]).
//! - `flm dump FILE` prints, as JSON, what one run of the library holds over one file, for
//!   inspection; its arguments and exit statuses are its own ([`dump`]).
//!
//! [`Cli`] is the whole command line of the first five commands as the parser reads it. The
//! chapter "The flm command-line tool" of the guide, in the documentation of `flm-core`,
//! explains every command and option with examples.
//!
//! # Problems as JSON records
//!
//! The option `--json-diagnostics`, accepted before or after the name of any command but
//! `flm dump`, makes the program write on standard error one JSON object per line instead
//! of text for a person: first a header (`{"kind": "diagnostics", "flm": VERSION,
//! "format": 1}`), then one record per thing it reports: a problem in the document
//! (`diagnostic`, with the identifier and data of its condition and its places, each a file,
//! a line, a column and, in a file's content, a byte offset from the top of the file), a
//! problem outside the document's diagnostics (`problem`: one that stopped the work, an
//! error of the program or of its arguments, or a warning of the program that does not stop
//! it, such as the warning of `flm info` about a documentation text it prints as plain
//! text), the counts that end a report (`summary`), the start of a build of the preview
//! (`build`), and a line that tells what the program does (`log`), such as the files `-v`
//! lists, or a line that an extension writes on its console (`[js log] TEXT`, `[wasm warn]
//! TEXT`). The records are those of
//! [`flm_main::report::Record`]. The usage text that the parser prints for a command line
//! it rejects stays text, and so does everything `flm dump` prints. Two more lines stay
//! text: what an extension compiled into the program (a bundled extension written in Rust)
//! writes on its console (`[native log] TEXT`), since the host library gives no way to send
//! it elsewhere, and the message for a compiled package that cannot be made, which the
//! program prints before it reads its arguments.
//!
//! # Exit statuses
//!
//! `flm watch` exits with status 0 when Ctrl+C ends it, and with status 2 when it cannot
//! start (wrong arguments, an unknown editor, a port that cannot be used, or files that
//! cannot be watched) or when watching the files fails while it runs.
//!
//! `flm render` and `flm check` exit with:
//!
//! - 0 when the command did what was asked, with or without warnings;
//! - 1 when the run reported errors: errors in the document, an output that could not be
//!   written (more than one file sent to standard output, two outputs writing different
//!   files at one path, or a file the disk refuses), or a written output that `-V` could
//!   not open; the status is 1 whether or not `--continue-with-errors` wrote the output;
//! - 2 when the command could not start the work: wrong arguments, an input that cannot be
//!   read, a front matter that cannot be read, a name that matches no package, profile,
//!   output or template, a setting that cannot be read, or a registry or plan that cannot
//!   be built;
//! - 3 on a failure of the program itself.
//!
//! On Unix, a signal that asks the program to end (the interrupt that Ctrl+C sends, the
//! quit signal that `Ctrl+\` sends, the hang-up sent when the terminal closes, or `SIGTERM`)
//! first stops the programs that extensions run through the host service `programs`, such
//! as `latexmk` under `-p pdf-latex`, and then ends `flm render` and `flm check` by that
//! signal, as it ends a program that does not handle it: a shell reports 128 plus the
//! signal's number, 130 for Ctrl+C. It ends `flm watch` in the same way, the interrupt
//! aside, which ends it with status 0. A signal that the program's parent left ignored stays
//! ignored (`nohup` ignores the hang-up).
//!
//! `flm schema` exits with 0 when the schema was printed or written, 1 when the file given
//! with `-o` cannot be written, 2 when the document given cannot be read or the packages
//! it uses cannot be found, and 3 on a failure of the program itself.
//!
//! `flm info` exits with 0 when every page asked for was shown, with or without warnings (a
//! documentation text that does not parse, shown as plain text, is one); 2 when the
//! arguments are wrong, the document given with `-d` cannot be read or prepared, the
//! environment cannot be prepared, or a path is no page's path and no name it looks up; and
//! 3 on a failure of the program itself, such as a page that does not render. `flm --help` and
//! `flm --version` exit with 0; a command line that the parser rejects, an empty one
//! included, exits with 2.

mod args;
pub mod dump;
mod info;
mod opener;
mod schema;
mod signals;
mod watch;

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use flm_core::core::registry::PackageFactory;
use flm_core::lang::FlmDiagnostic;
use flm_main::files::{DiskFiles, FileRead};
use flm_main::output::{NamedFile, OutputError, Sink};
use flm_main::report::{self, ContentOffsets, Record, RecordSeverity};
use flm_main::session::{Built, PrepareError, PrepareErrorKind, Prepared, Problem, build, prepare};

pub use args::{BuildArgs, Cli, Command, InfoArgs, RenderArgs, WatchArgs, default_math};
pub use info::text_problems;
pub use opener::BROWSER_VARIABLE;
pub use schema::{SchemaArgs, SchemaOf};

/// The exit status of a command that did what was asked: 0.
const DONE: u8 = 0;
/// The exit status of a run that reported errors, 1: errors in the document, an output
/// that the writing stage could not write, or a written output that `-V` could not open.
/// `flm schema` also returns it when the schema file cannot be written.
const DOCUMENT_ERRORS: u8 = 1;
/// The exit status of a command that could not start the work, 2.
const CANNOT_START: u8 = 2;
/// The exit status of a failure of the program itself, 3.
const PROGRAM_FAILURE: u8 = 3;

/// The first arguments that [`run`] hands to the parser unchanged: the command names and
/// the options of the whole program. Before any other first argument, `run` inserts
/// `watch`, so that `flm INPUT` runs the preview; the option [`JSON_DIAGNOSTICS`] may come
/// first, and `run` then looks at the argument after it.
const LEADING_NAMES: [&str; 9] = ["watch", "render", "check", "schema", "info", "help", "--help", "-h", "--version"];

/// The option that makes the program write JSON records on standard error
/// ([`Reporter::Json`]), accepted before or after the command's name.
const JSON_DIAGNOSTICS: &str = "--json-diagnostics";

/// How the program writes on standard error what it reports: the problems it found, the
/// counts that end a report, the lines that tell what it does (`-v`, the preview), and the
/// errors that stop a command. [`run`] chooses it once from the command line, and every
/// command writes through [`reporter`], from whichever thread prints (the preview's server
/// tells the outcome of each request to open an editor from its own thread).
///
/// The usage text of a command line that the parser rejects, and everything `flm dump`
/// prints, are written as they are, outside the reporter.
// One record per line on standard error, chosen by a global option. See decision rationale
// at [§flm-dd-dr:json-diagnostics].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reporter {
    /// Text for a person, as each command has always printed it.
    Text,
    /// One JSON record per line ([`Record`]), after a header written once per process, before
    /// the first record (`--json-diagnostics`).
    Json,
}

/// Whether the program writes JSON records ([`Reporter::Json`]); set by [`run`].
static JSON_RECORDS: AtomicBool = AtomicBool::new(false);

/// Whether the header of the JSON records ([`Record::meta`]) was written.
static HEADER_WRITTEN: AtomicBool = AtomicBool::new(false);

/// The reporter that [`run`] chose: [`Reporter::Text`] unless the command line holds
/// `--json-diagnostics`.
fn reporter() -> Reporter {
    if JSON_RECORDS.load(Ordering::Relaxed) { Reporter::Json } else { Reporter::Text }
}

impl Reporter {
    /// Writes one record on standard error, as one line of JSON, after the header when it
    /// was not written yet. The header is checked and written while standard error is
    /// locked, so that it comes first whichever thread writes first.
    fn record(record: &Record) {
        let mut stderr = std::io::stderr().lock();
        if !HEADER_WRITTEN.swap(true, Ordering::Relaxed)
            && let Ok(header) = serde_json::to_string(&Record::meta())
        {
            let _ = writeln!(stderr, "{header}");
        }
        if let Ok(line) = serde_json::to_string(record) {
            let _ = writeln!(stderr, "{line}");
        }
    }

    /// Reports the diagnostics of a document's run, then the counts of its Errors and
    /// Warnings: as text ([`report::report`]), or as one record for each diagnostic
    /// ([`Record::of_diagnostic`], with the byte offsets that `offsets` gives) followed by
    /// the counts ([`Record::summary`]).
    fn diagnostics(self, diagnostics: &[FlmDiagnostic], offsets: &ContentOffsets) {
        match self {
            Reporter::Text => eprintln!("{}", report::report(diagnostics)),
            Reporter::Json => {
                for diagnostic in diagnostics {
                    Reporter::record(&Record::of_diagnostic(diagnostic, offsets));
                }
                Reporter::record(&Record::summary(diagnostics));
            }
        }
    }

    /// Reports one diagnostic found before the work stopped, without counts: as text
    /// ([`report::diagnostic`]) or as a record with no byte offset.
    fn diagnostic(self, diagnostic: &FlmDiagnostic) {
        match self {
            Reporter::Text => eprintln!("{}", report::diagnostic(diagnostic)),
            Reporter::Json => Reporter::record(&Record::of_diagnostic(diagnostic, &ContentOffsets::new())),
        }
    }

    /// Reports a problem that stopped the work: as text ([`report::problem`]) or as a
    /// record ([`Record::of_problem`]).
    fn problem(self, problem: &Problem) {
        match self {
            Reporter::Text => eprintln!("{}", report::problem(problem)),
            Reporter::Json => Reporter::record(&Record::of_problem(problem)),
        }
    }

    /// Reports a line that tells what the program does: as `flm: TEXT`, or as a record of
    /// the kind `log` holding `text`.
    fn log(self, text: &str) {
        match self {
            Reporter::Text => eprintln!("flm: {text}"),
            Reporter::Json => Reporter::record(&Record::Log { text: text.to_string() }),
        }
    }

    /// Reports a warning of the program that does not stop the command: as
    /// `warning: MESSAGE`, or as a record of the kind `problem` with the severity `warning`
    /// and no place.
    fn warning(self, message: &str) {
        match self {
            Reporter::Text => eprintln!("warning: {message}"),
            Reporter::Json => Reporter::record(&Record::Problem {
                severity: RecordSeverity::Warning,
                message: message.to_string(),
                place: None,
            }),
        }
    }

    /// Reports an error of the program or of its arguments: as `error: MESSAGE`, or as a
    /// record of the kind `problem` with the severity `error` and no place.
    fn error(self, message: &str) {
        match self {
            Reporter::Text => eprintln!("error: {message}"),
            Reporter::Json => Reporter::record(&Record::Problem {
                severity: RecordSeverity::Error,
                message: message.to_string(),
                place: None,
            }),
        }
    }

    /// Reports the start of the preview's build numbered `number`: a record of the kind
    /// `build` with the time it started; nothing as text, where the files that changed are
    /// a line of their own.
    fn build(self, number: u64) {
        if self == Reporter::Json {
            Reporter::record(&Record::build(number, std::time::SystemTime::now()));
        }
    }
}

/// Where the `console` output of an extension goes: a function of the line's level (of type
/// `L`) and its text. It is the shape of the extension layer's console sink, whose level type
/// (`ConsoleLevel`) this crate does not name, the extension layer not being one of its
/// dependencies.
#[cfg(any(feature = "js", feature = "wasm"))]
type ConsoleSink<L> = Arc<dyn Fn(L, &str) + Send + Sync>;

/// Where the `console` output of the extensions that the engine `engine` runs (`js` or
/// `wasm`) goes under `--json-diagnostics`: each line is a `log` record holding
/// `[ENGINE LEVEL] TEXT`, the words that the text form writes on standard error.
#[cfg(any(feature = "js", feature = "wasm"))]
fn console_records<L: std::fmt::Display + 'static>(engine: &'static str) -> ConsoleSink<L> {
    Arc::new(move |level, text| reporter().log(&format!("[{engine} {level}] {text}")))
}

/// Runs the `flm` program over `args`, the command-line arguments that follow the
/// program's name, and returns the process's exit status (listed in the [crate
/// documentation](crate#exit-statuses)).
///
/// `packages` are the compiled packages that this executable carries in addition to the
/// library's packages `stdcore` and `math-passthrough` and the host's packages `stdmain`
/// and `std`; a document uses the packages of its initial list (`std`, unless the front
/// matter's `initial_use` or `--initial-use` gives another), then the package of its math
/// processor (the program's, [`default_math`], unless the front matter's `math` or `--math`
/// chooses another), and one of `packages` by listing its name under `use` in its front
/// matter or by `--use`.
///
/// When the first argument is `dump`, `run` hands the arguments that follow it to
/// [`dump`]. Otherwise it parses the arguments as [`Cli`], inserting `watch` before the
/// first argument when it is not a command name (looking past a leading
/// `--json-diagnostics`), and runs the command. On standard output it prints only what the
/// command produces (an output, a schema, pages, the help asked for); on standard error it
/// prints the problems found, the usage when the arguments are wrong, and, under `-v`, what
/// the program does, as text or, under `--json-diagnostics`, as JSON records (see the
/// [crate documentation](crate#problems-as-json-records)).
pub fn run(args: impl IntoIterator<Item = OsString>, packages: Vec<PackageFactory>) -> ExitCode {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.first().is_some_and(|first| first == "dump") {
        let rest: Vec<String> = args[1..].iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        return dump::main(&rest);
    }
    // The first argument that names the command, after the global option that may precede it.
    let leading = args.iter().position(|arg| arg != JSON_DIAGNOSTICS).unwrap_or(args.len());
    if args.get(leading).is_some_and(|first| first == "dump") {
        // `flm dump` prints text only; its own parser refuses the option after its name.
        eprintln!("error: `flm dump` does not accept `{JSON_DIAGNOSTICS}`");
        return ExitCode::from(CANNOT_START);
    }
    let mut line: Vec<OsString> = vec![OsString::from("flm")];
    line.extend(args[..leading].iter().cloned());
    match args.get(leading).and_then(|first| first.to_str()) {
        None if leading == args.len() => {}
        Some(first) if LEADING_NAMES.contains(&first) => {}
        _ => line.push(OsString::from("watch")),
    }
    line.extend(args[leading..].iter().cloned());
    let cli = match Cli::parse_line(line) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return ExitCode::from(if error.use_stderr() { CANNOT_START } else { DONE });
        }
    };
    JSON_RECORDS.store(cli.json_diagnostics, Ordering::Relaxed);
    if matches!(cli.command, Command::Render(_) | Command::Check(_))
        && let Err(error) = signals::stop_programs_on_signals(signals::OnInterrupt::EndBySignal)
    {
        reporter().error(&format!(
            "the command cannot be set to stop the programs that extensions run when a signal ends it: {error}"
        ));
        return ExitCode::from(CANNOT_START);
    }
    ExitCode::from(match cli.command {
        Command::Watch(watch_args) => watch::watch(&watch_args, &packages),
        Command::Render(render_args) => render(&render_args, &packages, &opener::open_file),
        Command::Check(build_args) => check(&build_args, &packages),
        Command::Schema(schema_args) => schema::run(&schema_args, &packages),
        Command::Info(info_args) => info::run(&info_args, &packages),
    })
}

/// Prints on standard error what stopped the work ([`stopped`]), and returns the exit
/// status: 3 when the run itself failed (a failure of the program), 2 otherwise.
fn cannot_start(error: &PrepareError) -> u8 {
    stopped(error);
    if error.kind == PrepareErrorKind::Run { PROGRAM_FAILURE } else { CANNOT_START }
}

/// Prints on standard error the diagnostics found before the work stopped (a warning about
/// a setting, for example; [`PrepareError::diagnostics`]), then each problem that stopped
/// it.
fn stopped(error: &PrepareError) {
    for diagnostic in &error.diagnostics {
        reporter().diagnostic(diagnostic);
    }
    for problem in &error.problems {
        reporter().problem(problem);
    }
}

/// Prints on standard error, for `-v`, each path of `reads` in order, one line per read
/// (a path read twice is printed twice), with ` (not found)` after a path that was looked
/// for and not found.
fn tell_read(reads: &[FileRead]) {
    for read in reads {
        reporter().log(&format!("read {}{}", read.path.display(), if read.found { "" } else { " (not found)" }));
    }
}

/// Prints on standard error, for `-v`, the files that the host's preparation
/// ([`prepare`]) read and what it chose: the profile, with the package of its extension
/// when it was used, then what [`tell_chosen`] prints.
fn tell_prepared(prepared: &Prepared) {
    tell_read(&prepared.files_read());
    match prepared.profile_package() {
        Some(package) => reporter().log(&format!("profile {}, with the package {package}", prepared.profile())),
        None => reporter().log(&format!("profile {}", prepared.profile())),
    }
    tell_chosen(prepared.packages(), prepared.workflow(), prepared.outputs());
}

/// Prints on standard error, for `-v`, what the host's preparation chose: the packages in
/// use, the workflow, and the outputs it produces, when it produces any.
fn tell_chosen(packages: &[String], workflow: &str, outputs: &[String]) {
    reporter().log(&format!("packages {}", packages.join(", ")));
    if outputs.is_empty() {
        reporter().log(&format!("workflow {workflow}"));
    } else {
        reporter().log(&format!("workflow {workflow}, outputs {}", outputs.join(", ")));
    }
}

/// Prepares and builds `request` with the host library ([`prepare`], then [`build`]),
/// prints under `-v` (from `args`) what they read and chose, and prints the document's
/// diagnostics on standard error. Returns the build and the outputs the preparation chose
/// ([`Prepared::outputs`]), the leading one first.
///
/// # Errors
///
/// The exit status, once the problems are printed, when the work could not start
/// ([`cannot_start`]).
fn prepare_and_build(
    args: &BuildArgs,
    request: flm_main::session::Request,
    packages: &[PackageFactory],
) -> Result<(Built, Vec<String>), u8> {
    let prepared = prepare(request, Arc::new(DiskFiles), packages).map_err(|error| cannot_start(&error))?;
    if args.verbose {
        tell_prepared(&prepared);
    }
    let built = build(&prepared).map_err(|error| cannot_start(&error))?;
    if args.verbose {
        // `files_read` begins with *prepare*'s reads, printed above; the build's follow.
        tell_read(&built.files_read[prepared.files_read().len()..]);
    }
    if !built.diagnostics.is_empty() {
        reporter().diagnostics(&built.diagnostics, &built.content_offsets);
    }
    Ok((built, prepared.outputs().to_vec()))
}

/// Runs `flm render`: builds the document and writes the files of its outputs to standard
/// output, or to the file or the folder of `-o`; under `-v`, prints the paths of the files written;
/// under `-V`, opens the written entry file of the leading output with `view`. The program passes
/// [`opener::open_file`] as `view` (the system's default application); a test passes a
/// function that records the path. Returns the exit status.
fn render(args: &RenderArgs, packages: &[PackageFactory], view: &dyn Fn(&Path) -> Result<(), String>) -> u8 {
    if args.view && matches!(args.output.as_deref(), None | Some("-")) {
        reporter().error("`-V` requires `-o` with a file or a folder, since it opens the written output");
        return CANNOT_START;
    }
    let mut request = match args.build.request() {
        Ok(request) => request,
        Err(message) => {
            reporter().error(&message);
            return CANNOT_START;
        }
    };
    let mut disk: Option<Arc<NotingSink>> = None;
    let (naming, folder, sink): (flm_main::output::Naming, std::path::PathBuf, Arc<dyn Sink>) =
        match args.output.as_deref() {
            None | Some("-") => (
                flm_main::output::Naming::Folder,
                std::path::PathBuf::new(),
                Arc::new(StdoutSink { newline: !args.no_newline }),
            ),
            Some(path) => {
                let (sink, naming) = flm_main::output::disk_destination(std::path::Path::new(path));
                let folder = sink.location().to_path_buf();
                let noting = Arc::new(NotingSink { sink, written: Mutex::new(Vec::new()), entries: Mutex::new(Vec::new()) });
                disk = Some(Arc::clone(&noting));
                (naming, folder, noting)
            }
        };
    // The current folder is the process's, which the host library does not read: an
    // address from the output's folder to a file needs it where the output lies above it.
    let current = std::env::current_dir().unwrap_or_default();
    request.output_folder = flm_main::output::OutputFolder::new(folder).with_current(current);
    request.naming = naming;
    request.sink = Some(sink);
    request.continue_with_errors = args.continue_with_errors;
    let (built, outputs) = match prepare_and_build(&args.build, request, packages) {
        Ok(done) => done,
        Err(status) => return status,
    };
    if let (true, Some(disk)) = (args.build.verbose, &disk) {
        for path in disk.written.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).iter() {
            reporter().log(&format!("wrote {path}"));
        }
    }
    // `-V` opens the entry of the leading output, the first of the outputs chosen.
    let written_entry = disk.and_then(|disk| {
        let entries = disk.entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let leading = outputs.first()?;
        entries.iter().find(|(output, _)| output == leading).map(|(_, path)| path.clone())
    });
    if let (true, Some(entry)) = (args.view, written_entry)
        && let Err(error) = view(&entry)
    {
        reporter().error(&format!(
            "`{}` cannot be opened with the system's default application: {error}",
            entry.display()
        ));
        return DOCUMENT_ERRORS;
    }
    if built.has_errors() { DOCUMENT_ERRORS } else { DONE }
}

/// Runs `flm check`: builds the document with the profile `check`, unless `-p` gives
/// another profile, and prints its problems; writes no output. Returns the exit status.
fn check(args: &BuildArgs, packages: &[PackageFactory]) -> u8 {
    let mut request = match args.request() {
        Ok(request) => request,
        Err(message) => {
            reporter().error(&message);
            return CANNOT_START;
        }
    };
    if request.profile.is_none() {
        request.profile = Some(String::from(flm_core::stdcore::CHECK_WORKFLOW));
    }
    match prepare_and_build(args, request, packages) {
        Ok((built, _)) if built.has_errors() => DOCUMENT_ERRORS,
        Ok(_) => DONE,
        Err(status) => status,
    }
}

/// The sink of `flm render -o`: it writes the files of the command's outputs to the disk
/// through [`DiskSink`](flm_main::output::DiskSink), and notes the distinct paths of the
/// files written, for `-v`, and the path of each output's main file (its entry), by output
/// name, for `-V`. It notes the files of a delivery only when the delivery wrote them all
/// without a problem.
struct NotingSink {
    sink: flm_main::output::DiskSink,
    written: Mutex<Vec<String>>,
    entries: Mutex<Vec<(String, std::path::PathBuf)>>,
}

impl Sink for NotingSink {
    fn receive(&self, files: &[NamedFile]) -> Result<(), Vec<OutputError>> {
        self.sink.receive(files)?;
        let mut written = self.written.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for file in files {
            if !written.contains(&file.path) {
                written.push(file.path.clone());
            }
        }
        let mut entries = self.entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for file in files.iter().filter(|file| file.entry) {
            entries.push((file.output.clone(), self.sink.location().join(&file.path)));
        }
        Ok(())
    }
}

/// The sink of `flm render` without `-o` (or with `-o -`): it accepts a delivery of one
/// file only (one distinct path, whatever the number of outputs), and prints that file on
/// standard output with the line endings at its end replaced by exactly one newline, or by
/// none under `-n`.
///
/// A delivery of any other number of files is refused with an [`OutputError`], which the
/// host reports as an error of the document's run.
struct StdoutSink {
    newline: bool,
}

impl Sink for StdoutSink {
    fn receive(&self, files: &[NamedFile]) -> Result<(), Vec<OutputError>> {
        let mut paths: Vec<&str> = Vec::new();
        let mut outputs: Vec<&str> = Vec::new();
        for file in files {
            if !paths.contains(&file.path.as_str()) {
                paths.push(&file.path);
            }
            if !outputs.contains(&file.output.as_str()) {
                outputs.push(&file.output);
            }
        }
        let [_] = paths.as_slice() else {
            let held = if outputs.len() == 1 {
                format!("this command's output (`{}`) holds {} files", outputs[0], paths.len())
            } else {
                format!("this command's outputs (`{}`) hold {} files", outputs.join("`, `"), paths.len())
            };
            return Err(vec![OutputError {
                path: None,
                detail: format!(
                    "standard output can receive one file, and {held} (`{}`): write them with `-o FILE` or \
                     `-o FOLDER/`",
                    paths.join("`, `")
                ),
            }]);
        };
        let file = &files[0];
        let end = file.bytes.iter().rposition(|byte| !matches!(byte, b'\n' | b'\r')).map_or(0, |last| last + 1);
        let mut stdout = std::io::stdout().lock();
        let written = stdout
            .write_all(&file.bytes[..end])
            .and_then(|()| if self.newline { stdout.write_all(b"\n") } else { Ok(()) })
            .and_then(|()| stdout.flush());
        written.map_err(|error| {
            vec![OutputError { path: None, detail: format!("standard output cannot be written: {error}") }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// `flm render -V` hands the opener the written entry — the file, not the folder `-o`
    /// names — and nothing when the run cannot start. In process, with an opener that notes
    /// what it was given, so that no window opens.
    #[test]
    fn view_hands_over_the_written_entry() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/flm-cli-unit/view");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("paper.flm"), "---\ntitle: A paper\n---\nHello world.\n").unwrap();
        let given: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
        let view = |path: &Path| {
            given.lock().unwrap().push(path.to_path_buf());
            Ok(())
        };
        let site = format!("{}/", dir.join("site").display());
        let paper = dir.join("paper.flm").display().to_string();
        let Command::Render(args) = Cli::parse_line(["flm", "render", "-V", "-o", &site, &paper]).unwrap().command
        else {
            panic!("`render`");
        };
        assert_eq!(render(&args, &[], &view), DONE);
        let entry = dir.join("site").join("index.html");
        assert_eq!(*given.lock().unwrap(), std::slice::from_ref(&entry));
        assert!(entry.is_file());
    }

    /// Under a workflow of two outputs, `flm render -V` hands the opener the entry of the
    /// leading output: the workflow's first, `paper.html`, or the one `-f` names,
    /// `paper.txt` (`-o paper` names both).
    #[test]
    fn view_hands_over_the_leading_output_s_entry() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/flm-cli-unit/view-leading");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("_flmextensions")).unwrap();
        std::fs::write(
            dir.join("_flmextensions/both.yaml"),
            "flm:\n  workflows:\n    both:\n      extends: html\n      outputs: [html, text]\n",
        )
        .unwrap();
        std::fs::write(dir.join("paper.flm"), "---\nflm: {use: [both]}\n---\nHello world.\n").unwrap();
        let paper = dir.join("paper.flm").display().to_string();
        let named = dir.join("paper").display().to_string();
        for (format, opened) in [(None, "paper.html"), (Some("text"), "paper.txt")] {
            let given: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
            let view = |path: &Path| {
                given.lock().unwrap().push(path.to_path_buf());
                Ok(())
            };
            let mut line = vec!["flm", "render", "-p", "both", "-V", "-o", &named, &paper];
            if let Some(format) = format {
                line.extend(["-f", format]);
            }
            let Command::Render(args) = Cli::parse_line(line).unwrap().command else {
                panic!("`render`");
            };
            assert_eq!(render(&args, &[], &view), DONE);
            assert_eq!(*given.lock().unwrap(), [dir.join(opened)], "{format:?}");
        }
        assert!(dir.join("paper.html").is_file() && dir.join("paper.txt").is_file());
    }
}
