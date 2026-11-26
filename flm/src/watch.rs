//! `flm watch`, and `flm INPUT`: the program's side of the live preview. This module checks
//! the command line, prints what the terminal shows, opens the browser and ends the program
//! on Ctrl+C. The host library runs the rest ([`flm_host::watch`]): the loop that builds
//! the document again whenever a file it read changes, and the server that serves each
//! build to the browser and opens places of the document in an editor. The program gives
//! the server what only it knows, the help pages, the command line of an export, the
//! opening of an exported file and the home folder, in which the server keeps the viewer
//! settings, through its services ([`ProgramServices`]).

use std::path::PathBuf;
use std::sync::Arc;

use flm_host::files::DiskFiles;
use flm_host::output::{Naming, OutputFolder};
use flm_host::session::Input;
use flm_host::watch::editor::{CUSTOM, EditorCommand, Editors};
use flm_host::watch::server::{Opening, Port, Server};
use flm_host::watch::settings::DocumentSettings;
use flm_host::watch::{Built, Event, Watch};

use crate::args::WatchArgs;
use crate::progress::WatchRecords;
use crate::schema::{SchemaOf, schema};
use crate::services::ProgramServices;
use crate::opener::{BROWSER_VARIABLE, open_address};
use crate::{CANNOT_START, Carried, DONE, Reporter, reporter, stopped, tell_chosen, tell_read};

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
    if args.build.input.as_deref().is_some_and(|path| path.as_os_str() == "-") {
        return Some(String::from(
            "the preview (`flm watch`) cannot read standard input, because it watches a file; \
             `flm render -` renders standard input",
        ));
    }
    None
}

/// Runs `flm watch`: builds the document, serves it on this machine, opens the browser on
/// it unless `--no-browser` is given, and builds the document again whenever a file it read
/// changes, until Ctrl+C. Once the preview is served, it carries out the maintenance of
/// the program's cache on a thread of its own. Ctrl+C ends the program at once with status 0, during a build
/// too. Returns status 2 when the preview cannot start or its file watcher fails.
pub(crate) fn watch(args: &WatchArgs, carried: &Carried) -> u8 {
    let verbose = args.build.verbose;
    if let Some(message) = refused(args) {
        reporter().error(&message);
        return CANNOT_START;
    }
    let mut request = match args.build.request(carried.bundled) {
        Ok(request) => request,
        Err(message) => {
            reporter().error(&message);
            return CANNOT_START;
        }
    };
    let command = match args.editor_command.as_deref().map(EditorCommand::parse).transpose() {
        Ok(command) => command,
        Err(error) => {
            reporter().error(&error.to_string());
            return CANNOT_START;
        }
    };
    let editors = match Editors::find(args.editor.as_deref(), std::env::var_os("PATH").as_deref(), &application_folders()) {
        Ok(editors) => editors.with_command(command),
        Err(error) => {
            reporter().error(&error.to_string());
            return CANNOT_START;
        }
    };
    // The output is named as in a folder (`index.html` and the files beside it), so that the
    // preview's addresses do not depend on the document's name.
    request.naming = Naming::Folder;
    // The current folder, as `flm render` gives it: an export of the preview, written in the
    // document's folder, reaches a file from there as `flm render -o` does. The watch loop
    // keeps it for the exports alone (`Watch::new`).
    let current = std::env::current_dir().ok().and_then(|current| crate::args::flm_path(&current).ok()).unwrap_or_default();
    request.output_folder = OutputFolder::default().with_current(current);
    let document = match &request.input {
        Input::File(path) => path.clone(),
        Input::Text(_) => String::new(),
    };
    // The preview's settings start with the front matter's schema made without a document,
    // suggesting what the document's search path holds; each build that prepares the
    // document again replaces it with the schema of the packages the document uses
    // (`Server::tell`, before the terminal's `build N done`).
    let settings = match schema(SchemaOf::FrontMatter, carried, request.extension_folders.clone()) {
        Ok(schema) => DocumentSettings::on_disk(schema, &document),
        Err(detail) => {
            reporter().error(&detail);
            return CANNOT_START;
        }
    };
    let services = ProgramServices::new(carried, &args.build, &request.input);
    let cache = request.cache.manager.clone();
    let watch = Watch::new(request, Arc::new(DiskFiles), carried.packages.clone()).watching_files();
    let port = args.port.map_or_else(Port::default, Port::Exactly);
    let server = match Server::start(port, document, watch.handle(), editors, told, settings, services) {
        Ok(server) => server,
        Err(error) => {
            reporter().error(&error.to_string());
            return CANNOT_START;
        }
    };
    // The cache's maintenance runs once, while the preview serves its first build.
    crate::cache::maintain_in_background(cache, Arc::new(crate::ProgramLog { verbose }));
    // Ctrl+C ends the process at once with status 0, a build under way or not, once the
    // programs that the host service `programs` runs are stopped: the preview writes files
    // only when its page asks (a change of the front matter, an export saved), and the
    // system closes the connections and releases the watcher with the process. The handling
    // exists for those programs and for the status, which the system's own end of the
    // process makes non-zero.
    if let Err(error) = crate::signals::stop_programs_on_signals(crate::signals::OnInterrupt::ExitDone) {
        reporter().error(&format!("Failure to configure interrupt signal for preview (Ctrl+C): {error}"));
        return CANNOT_START;
    }
    reporter().preview_link(&server.tools_url());
    if let Some(unreadable) = server.viewer_unreadable() {
        reporter().error(&unreadable.to_string());
    }
    if verbose { reporter().log(&editors_found(&server)); }
    if !args.no_browser
        && let Err(error) = open_address(&server.tools_url(), std::env::var_os(BROWSER_VARIABLE))
    {
        reporter().error(&format!("the browser cannot be opened: {error}"));
    }
    // The terminal shows no progress line; under `--json-messages`, the progress of the
    // build under way is written as records.
    let records = (reporter() == Reporter::Json).then(|| WatchRecords::start(watch.handle()));
    // The number of the build that starts, counted as the watch loop counts its builds: one
    // for each start, a build that is cancelled included.
    let mut started: u64 = 0;
    let ran = watch.run(|event| {
        let _held = records.as_ref().map(|records| records.told(&event));
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

/// The width of the box of [`link_box`] inside its border, in columns, when its lines fit
/// in it.
const BOX_WIDTH: usize = 72;

/// The space between the border of the box of [`link_box`] and a line set against its left
/// side, in columns.
const BOX_MARGIN: usize = 3;

/// The box the terminal shows when the preview starts, drawn with the box-drawing
/// characters: a title, then `url`, the link at which the preview is viewed, at the center
/// between two arrows, then how to view the preview and how to end it. The box is
/// [`BOX_WIDTH`] columns wide inside its border, or wider when a line needs it, so that a
/// long link stays inside.
// Each character of the box takes one column of the terminal, the link's too (an address
// of this machine), so that counting characters gives the columns. An emoji would not:
// terminals do not agree on its width.
pub(crate) fn link_box(url: &str) -> String {
    let title = "FLM live preview";
    let link = format!("==>  {url}  <==");
    let text = ["Preview your document by opening this link in your browser;", "hit Ctrl+C here to quit."];
    let width = [title, link.as_str(), text[0], text[1]]
        .iter()
        .map(|line| line.chars().count() + 2 * BOX_MARGIN)
        .fold(BOX_WIDTH, usize::max);
    let row = |left: usize, line: &str| {
        let right = width - left - line.chars().count();
        format!("│{}{line}{}│", " ".repeat(left), " ".repeat(right))
    };
    let centered = |line: &str| row((width - line.chars().count()) / 2, line);
    let rule = "─".repeat(width);
    let blank = row(0, "");
    [
        format!("╭{rule}╮"),
        blank.clone(),
        centered(title),
        blank.clone(),
        centered(&link),
        blank.clone(),
        row(BOX_MARGIN, text[0]),
        row(BOX_MARGIN, text[1]),
        blank,
        format!("╰{rule}╯"),
    ]
    .join("\n")
}

/// The folders in which the editors' application bundles are looked for
/// ([`Editors::find`]): on macOS, `/Applications` and the folder `Applications` of the home
/// folder (the environment variable `HOME`, when it is set); none elsewhere.
// The program, not the host library, gives these folders: where a platform keeps its
// applications stays out of the library, and the library's tests can use folders of their own.
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
/// path of its program, the editor command when there is one, and the editor a place clicked
/// on the page opens in; or that there is no editor, and where the editors were looked for.
fn editors_found(server: &Server) -> String {
    let editors = server.editors();
    let Some(chosen) = server.editor() else {
        return editors.none_found();
    };
    let mut found: Vec<String> =
        editors.iter().map(|editor| format!("‘{}’ ({})", editor.name(), editor.program().display())).collect();
    if let Some(command) = server.editor_command() {
        found.push(format!("‘{CUSTOM}’ ({})", command.text()));
    }
    format!("editor: ‘{chosen}’ [found {}]", found.join(", "))
}

/// Prints on standard error what the terminal shows of an event of the preview's loop: the
/// files whose change starts a build, each finished build ([`shown_built`]), and the errors
/// of the file watcher. (Under `--json-messages`, the start of each build is also a
/// record of its own, which [`watch`] writes, and so is the progress of the build under way,
/// every 500 ms, when it changed ([`WatchRecords`]).)
fn show(event: &Event, verbose: bool) {
    match event {
        Event::Building { changed } if !changed.is_empty() => {
            reporter().log(&format!("changed {}", changed.join(", ")));
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
            reporter().log(&format!("opened {file} at line {line}, column {column}, in `{editor}`"));
        }
        Opening::Unopened { place, reason } => reporter().error(&format!("{reason} (place {place})")),
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
        if let Some(chosen) = built.preparation.prepared() {
            tell_chosen(chosen.packages(), chosen.workflow(), chosen.outputs());
        }
    }
    match &built.outcome {
        Ok(diagnostics) if !diagnostics.is_empty() => reporter().diagnostics(diagnostics, &built.content_offsets),
        Ok(_) => {}
        Err(error) => stopped(error),
    }
    reporter().log(&format!("build {} done", built.update.build));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The box's lines are all as wide, a short link's box and a long link's alike, and
    /// the link is on one of them.
    #[test]
    fn the_link_box_s_lines_are_as_wide_as_each_other() {
        for url in ["http://127.0.0.1:18910/.flm-preview/", &format!("http://{}.example:8080/.flm-preview/", "a".repeat(90))] {
            let drawn = link_box(url);
            let widths: Vec<usize> = drawn.lines().map(|line| line.chars().count()).collect();
            assert!(widths.iter().all(|width| *width == widths[0]), "{drawn}");
            assert!(drawn.lines().any(|line| line.contains(url)), "{drawn}");
        }
    }
}
