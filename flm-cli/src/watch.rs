//! `flm watch`, and `flm INPUT`: the program's side of the live preview. This module checks
//! the command line, prints what the terminal shows, opens the browser and ends the program
//! on Ctrl+C. The host library runs the rest ([`flm_main::watch`]): the loop that builds
//! the document again whenever a file it read changes, and the server that serves each
//! build to the browser and opens places of the document in an editor.

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::registry::PackageFactory;
use flm_main::files::DiskFiles;
use flm_main::output::Naming;
use flm_main::watch::editor::Editors;
use flm_main::watch::server::{Opening, Port, Server};
use flm_main::watch::settings::DocumentSettings;
use flm_main::watch::{Built, Event, Watch};

use crate::args::WatchArgs;
use crate::schema::{SchemaOf, schema};
use crate::opener::{BROWSER_VARIABLE, open_address};
use crate::{CANNOT_START, DONE, reporter, stopped, tell_chosen, tell_read};

/// Returns the message for an argument that the preview refuses, or `None` when it refuses
/// none. The parser accepts these arguments, which a person may type by habit, so that the
/// program can explain them: `-V` and `-o` ask for an output to be written (and opened, for
/// `-V`); `-c TEXT` and `-` give no file to watch. Each message states what the preview
/// does not accept and gives the `flm render` command that does what was meant, which a
/// person cannot guess.
fn refused(args: &WatchArgs) -> Option<String> {
    if args.view {
        return Some(String::from(
            "the preview (`flm watch`) does not accept `-V`, because it opens the browser by \
             itself; `flm render -V -o OUTPUT INPUT` writes the output and opens it",
        ));
    }
    if args.output.is_some() {
        return Some(String::from(
            "the preview (`flm watch`) does not accept `-o`, because it writes no output file; \
             `flm render -o OUTPUT INPUT` writes the output",
        ));
    }
    if args.build.content.is_some() {
        return Some(String::from(
            "the preview (`flm watch`) does not accept `-c`, because it watches a file; \
             `flm render -c TEXT` renders a text given on the command line",
        ));
    }
    if args.build.input.as_deref() == Some("-") {
        return Some(String::from(
            "the preview (`flm watch`) cannot read standard input, because it watches a file; \
             `flm render -` renders standard input",
        ));
    }
    None
}

/// Runs `flm watch`: builds the document, serves it on this machine, opens the browser on
/// it unless `--no-browser` is given, and builds the document again whenever a file it read
/// changes, until Ctrl+C. Ctrl+C ends the program at once with status 0, during a build
/// too. Returns status 2 when the preview cannot start or its file watcher fails.
pub(crate) fn watch(args: &WatchArgs, packages: &[PackageFactory]) -> u8 {
    if let Some(message) = refused(args) {
        reporter().error(&message);
        return CANNOT_START;
    }
    let mut request = match args.build.request() {
        Ok(request) => request,
        Err(message) => {
            reporter().error(&message);
            return CANNOT_START;
        }
    };
    let editors = match Editors::find(args.editor.as_deref(), std::env::var_os("PATH").as_deref(), &application_folders()) {
        Ok(editors) => editors,
        Err(error) => {
            reporter().error(&error.to_string());
            return CANNOT_START;
        }
    };
    // See decision rationale at [§flm-dd-dr:host-watch-serving].
    request.naming = Naming::Folder;
    let document = args.build.input.clone().unwrap_or_default();
    // The *Settings* tool starts with the front matter's schema made without a document,
    // suggesting what the document's search path holds; each build that prepares the
    // document again replaces it with the schema of the packages the document uses
    // (`Server::tell`, before the terminal's `build N done`).
    let settings = match schema(SchemaOf::FrontMatter, packages, request.extension_folders.clone()) {
        Ok(schema) => DocumentSettings::on_disk(schema, &document),
        Err(detail) => {
            reporter().error(&detail);
            return CANNOT_START;
        }
    };
    let watch = Watch::new(request, Arc::new(DiskFiles), packages.to_vec()).watching_files();
    let port = args.port.map_or_else(Port::default, Port::Exactly);
    let found = editors_found(&editors);
    let server = match Server::start(port, document, watch.handle(), editors, told, settings) {
        Ok(server) => server,
        Err(error) => {
            reporter().error(&error.to_string());
            return CANNOT_START;
        }
    };
    // Ctrl+C ends the process at once with status 0, a build under way or not, once the
    // programs that the host service `programs` runs are stopped: the preview writes no file,
    // and the system closes the connections and releases the watcher with the process. The
    // handling exists for those programs and for the status, which the system's own end of the
    // process makes non-zero. See decision rationale at [§flm-dd-dr:cli-watch-interrupt].
    if let Err(error) = crate::signals::stop_programs_on_signals(crate::signals::OnInterrupt::ExitDone) {
        reporter().error(&format!("the preview cannot be set to end on Ctrl+C: {error}"));
        return CANNOT_START;
    }
    reporter().log(&format!("the preview is at {}; Ctrl+C ends it", server.url()));
    reporter().log(&found);
    if !args.no_browser
        && let Err(error) = open_address(&server.url(), std::env::var_os(BROWSER_VARIABLE))
    {
        reporter().error(&format!("the browser cannot be opened: {error}"));
    }
    let verbose = args.build.verbose;
    // The number of the build that starts, counted as the watch loop counts its builds: each
    // start of a build is followed by its end.
    let mut started: u64 = 0;
    let ran = watch.run(|event| {
        server.tell(&event);
        if matches!(event, Event::Building { .. }) {
            started += 1;
            reporter().build(started);
        }
        show(&event, verbose);
    });
    match ran {
        Ok(()) => DONE,
        Err(error) => {
            reporter().error(&error.to_string());
            CANNOT_START
        }
    }
}

/// The folders in which the editors' application bundles are looked for
/// ([`Editors::find`]): on macOS, `/Applications` and the folder `Applications` of the home
/// folder (the environment variable `HOME`, when it is set); none elsewhere.
// See decision rationale at [§flm-dd-dr:host-watch-editor-menu].
fn application_folders() -> Vec<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let mut folders = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        folders.push(PathBuf::from(home).join("Applications"));
    }
    folders
}

/// The line the terminal shows at the start about the editors: each editor found, with the
/// path of its program, and the default, the editor the page's menu chooses at first; or
/// that no editor was found, and where the editors were looked for.
fn editors_found(editors: &Editors) -> String {
    let Some(default) = editors.default() else {
        return editors.none_found();
    };
    let found: Vec<String> =
        editors.iter().map(|editor| format!("`{}` ({})", editor.name(), editor.program().display())).collect();
    format!(
        "editors found: {}; a place clicked on the page opens in the one chosen in the page's menu, `{}` by default",
        found.join(", "),
        default.name()
    )
}

/// Prints on standard error what the terminal shows of an event of the preview's loop: the
/// files whose change starts a build, each finished build ([`shown_built`]), and the errors
/// of the file watcher. (Under `--json-diagnostics`, the start of each build is also a
/// record of its own, which [`watch`] writes.)
fn show(event: &Event, verbose: bool) {
    match event {
        Event::Building { changed } if !changed.is_empty() => {
            let names: Vec<String> = changed.iter().map(|path| path.display().to_string()).collect();
            reporter().log(&format!("changed {}", names.join(", ")));
        }
        Event::Building { .. } => {}
        Event::Built(built) => shown_built(built, verbose),
        Event::Watcher(error) => reporter().error(&error.to_string()),
    }
}

/// Prints on standard error the outcome of a request of the page to open a place in the
/// editor: the file, line and column opened, or why nothing was opened (the same text as
/// the page shows), or how an editor that had started failed.
fn told(opening: Opening) {
    match opening {
        Opening::Opened { editor, file, line, column } => {
            reporter().log(&format!("opened {} at line {line}, column {column}, in `{editor}`", file.display()));
        }
        Opening::Refused { reason } => reporter().error(&reason),
        Opening::Failed(failure) => reporter().error(&failure.to_string()),
    }
}

/// Prints a finished build on standard error: under `-v`, the files it read and, when the
/// build prepared the document again, what the preparation chose, as `flm render -v`
/// prints them; then the build's problems, as `flm render` prints them; then the line
/// `flm: build N done`, N being the build's number.
fn shown_built(built: &Built, verbose: bool) {
    if verbose {
        tell_read(&built.files_read);
        if let Some(chosen) = &built.prepared {
            tell_chosen(&chosen.packages, &chosen.workflow, &chosen.outputs);
        }
    }
    match &built.outcome {
        Ok(diagnostics) if !diagnostics.is_empty() => reporter().diagnostics(diagnostics, &built.content_offsets),
        Ok(_) => {}
        Err(error) => stopped(error),
    }
    reporter().log(&format!("build {} done", built.update.build));
}
