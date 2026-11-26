//! Measures the table of expected durations with which a program weighs the rows of a
//! build's progress sheet, over the documents given on the command line, and prints it as
//! JSON on standard output.
//!
//! ```text
//! cargo run -p flm --example expected_durations -- [-n BUILDS] [OPTION]... DOCUMENT...
//! ```
//!
//! Each document is built `BUILDS` times (3 unless `-n` gives another number) as
//! `flm render -o FOLDER/ OPTION...` builds it, each option of `flm render` written as one
//! argument (`-Omath-mathjax.mode=mathml`), with the program's request
//! ([`BuildArgs::request`](flm::BuildArgs::request)), into a folder of its own under
//! the system's folder for temporary files, which the example removes at the end. Its
//! builds write beside a document what `flm render` writes there: the citation cache file
//! `.flm-citations.jsonl`, for a document that cites works fetched over the network. Every
//! build reports on one progress sheet, whose observer stamps each row with the time
//! elapsed since the example started. The table is measured from the sheet's last
//! snapshot ([`measure`]): for each kind, the mean duration of its rows that finished, and
//! the duration of one unit of the rows that counted their work. The rows of the example's
//! own kinds are left out. A build that cannot start, or a document with errors, is told
//! on standard error; the rows of a build that did not finish measure nothing.
//!
//! The table that the host library ships
//! ([`expected_durations`](flm_host::report::expected_durations)) is made from this
//! example's output over the sample documents of the repository.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use flm::{Cli, Command};
use flm_core::progress::{Kind, Observer, Scope, Sheet, Snapshot, measure};
use flm_host::files::DiskFiles;
use flm_host::files::system::from_system;
use flm_host::output::{OutputFolder, Sink, disk_destination};
use flm_host::session::{BUILD_KIND, build, prepare};

/// The kind of the root row of the example's sheet.
const MEASURING: Kind = Kind::new("example.measuring");

/// The kind of the row of one build of one document, labeled with the document.
const DOCUMENT: Kind = Kind::new("example.document");

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut builds: usize = 3;
    if args.first().is_some_and(|first| first == "-n") {
        match args.get(1).and_then(|count| count.parse().ok()) {
            Some(count) => builds = count,
            None => {
                eprintln!("error: `-n` must be followed by a number of builds");
                return ExitCode::from(2);
            }
        }
        args.drain(..2);
    }
    let options: Vec<String> = args.iter().take_while(|arg| arg.starts_with('-')).cloned().collect();
    args.drain(..options.len());
    if args.is_empty() {
        eprintln!("error: at least one document must be given");
        return ExitCode::from(2);
    }

    let origin = Instant::now();
    let observer = Observer {
        row_started: Some(Box::new(move |_row| Some(origin.elapsed()))),
        row_ended: Some(Box::new(move |_row| Some(origin.elapsed()))),
        ..Observer::default()
    };
    let (sheet, root) = Sheet::start(&MEASURING, None, observer, None);
    let scratch = std::env::temp_dir().join(format!("flm-expected-durations-{}", std::process::id()));
    for (number, document) in args.iter().enumerate() {
        let folder = scratch.join(number.to_string());
        for _ in 0..builds {
            if let Err(message) = build_once(document, &options, &folder, &root.scope()) {
                eprintln!("{document}: {message}");
            }
        }
    }
    root.finish();
    let _ = std::fs::remove_dir_all(&scratch);

    let snapshot = sheet.snapshot();
    let rows = snapshot.rows.into_iter().filter(|row| row.kind != MEASURING.name() && row.kind != DOCUMENT.name());
    let expected = measure(&Snapshot { cancelled: snapshot.cancelled, rows: rows.collect() });
    match serde_json::to_string_pretty(&expected) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: the table cannot be written as JSON: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Builds `document` into `folder` as `flm render -o FOLDER/ OPTIONS... DOCUMENT` does,
/// under a row of its own declared under `scope`, which finishes when the build does.
///
/// # Errors
///
/// A message for a person when the build cannot start or the document has errors.
fn build_once(document: &str, options: &[String], folder: &Path, scope: &Scope) -> Result<(), String> {
    let folder = format!("{}/", from_system(folder).map_err(|error| error.to_string())?);
    let mut line = vec!["flm", "render", "-o", &folder];
    line.extend(options.iter().map(String::as_str));
    line.push(document);
    let cli = Cli::parse_line(line).map_err(|error| error.to_string())?;
    let Command::Render(render) = cli.command else {
        return Err(String::from("the command line is not one of `flm render`"));
    };
    let mut request = render.build.request(flm::shipped_extensions())?;
    // The example prints its table alone: the lines of the build's log go nowhere.
    request.log_sink = None;
    let (sink, naming) = disk_destination(&folder);
    request.output_folder = OutputFolder::new(sink.location().to_string());
    request.naming = naming;
    request.sink = Some(Arc::new(sink) as Arc<dyn Sink>);
    let task = scope.child(&DOCUMENT, Some(document)).start();
    // The build's row is declared before *prepare*, as `flm render` declares it.
    let build_row = task.scope().child(&BUILD_KIND, None);
    let prepared = prepare(request, Arc::new(DiskFiles), &[], &task.scope())
        .map_err(|error| format!("the build cannot start ({:?})", error.kind))?;
    let build_task = build_row.start();
    let built = build(&prepared, &build_task.scope()).map_err(|error| format!("the build stopped ({:?})", error.kind))?;
    if built.has_errors() {
        return Err(String::from("the document has errors"));
    }
    build_task.finish();
    task.finish();
    Ok(())
}
