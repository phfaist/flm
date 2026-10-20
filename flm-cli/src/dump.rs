//! The command `flm dump`: an inspection tool that processes one file with the library
//! [`flm_core`] alone and prints, as JSON on standard output, one view of what the run
//! holds: a document tree at a chosen point of the processing, a computed table, or the
//! artifacts of an output.
//!
//! It serves a reader who wants to see what the library makes of a document: the tree the
//! parser produced, the tree after the rewrite stages, the presentation tree a backend
//! built, or the data a stage computed. It reads no front matter and no settings: the
//! document it processes is the text of the file given on the command line, and its
//! registry is built from the library's standard package under its default
//! configuration. Its arguments, its output and its exit statuses are its own,
//! independent of the commands `render` and `check`.
//!
//! The work is done by the engine [`flm_info::dump`]; this module parses the command line
//! into the engine's request, reads the file, and prints what the engine gives: the
//! diagnostics on standard error, the view on standard output.
//!
//! # Synopsis
//!
//! ```text
//! flm dump FILE [--at parsed|frozen|lowered:<backend>] [--view compact|wire]
//!               [--table NAME | --output NAME] [--workflow NAME] [--width N]
//! ```
//!
//! The options may be given in any order, before or after `FILE`. Exactly one file is
//! accepted.
//!
//! # What `flm dump` does
//!
//! In order, it:
//!
//! 1. builds a [registry](flm_core::core::registry), the frozen record of everything the
//!    language and the pipeline consult for a set of features, from the standard package
//!    ([`flm_core::stdcore::package`]) under its default configuration and the package
//!    `math-passthrough` ([`flm_core::math_passthrough::package`]), which processes the
//!    mathematics;
//! 2. parses `FILE` into a [`Fragment`](flm_core::model::doc::Fragment) and places that
//!    one fragment in a [`Doc`](flm_core::model::doc::Doc);
//! 3. unless only the parsed tree is asked for, builds a
//!    [`Plan`](flm_core::core::plan::Plan) for a workflow and executes one
//!    [`Run`](flm_core::core::run::Run) of it;
//! 4. prints one view of what that run holds.
//!
//! # Choosing what is printed
//!
//! `--at` chooses a point of the processing and prints the file's tree as it is at that
//! point:
//!
//! - `parsed` (the default) prints the tree exactly as the parser produced it. No workflow
//!   runs for it.
//! - `frozen` prints the tree after the plan's rewrite stages have run: user definitions
//!   expanded, content grouped into paragraphs and blocks. Every later stage of the run
//!   reads this form, and the tree is never changed again.
//! - `lowered:<backend>` prints the tree that the named backend built from the frozen tree
//!   for its output: the same content rewritten into elements that the backend writes
//!   natively, such as the presentation elements of
//!   [`flm_core::stdcore::backends::pres`]. `lowered:text` is the tree into which the text
//!   output's position marks
//!   ([`TextMark`](flm_core::stdcore::backends::text::TextMark)) point.
//!
//! `--table NAME` prints, under its name, the computed table `NAME` that the run wrote.
//! `--output NAME` prints the artifact set of the output `NAME` the same way, under the
//! name of the table that holds it; an artifact's bytes are printed in the JSON form that
//! techy, the parser toolkit FLM is built on, gives binary data: the object
//! `{"$bytes": "<base64>"}`. `--table` and `--output` cannot be given together, and either
//! of them replaces `--at`.
//!
//! A table, output or backend that the registry does not declare is a failure, and the
//! message lists the names that exist: the registry's tables for `--table`, its outputs for
//! `--output`, and the backends of the plan for `lowered:<backend>`. A table or output that
//! the chosen workflow does not write is a failure too, and so is a backend that the
//! chosen workflow does not use (exit statuses below).
//!
//! # The two views
//!
//! `--view` chooses how a tree is printed. Tables and outputs are always printed as
//! projections.
//!
//! - `--view compact` (the default) prints the tree's *projection*, a one-way, readable
//!   view of it in the library's own vocabulary (`elem`, `chars`, `args`, `children`,
//!   `body`, `payload`), as indented JSON. See [`flm_core::dump::project`].
//! - `--view wire` prints techy's serialization of the tree, as a single JSON line. That
//!   form is lossless and reads back into live objects with their sharing intact, which the
//!   projection does not. See [`flm_core::dump::session`].
//!
//! # Choosing the run
//!
//! `--workflow NAME` chooses the workflow to plan and run. The default is
//! [`text`](flm_core::stdcore::backends::text::WORKFLOW), which renders the document as
//! plain text. The standard package also has
//! [`latex-recomposed`](flm_core::stdcore::backends::latex_recomposed::WORKFLOW), which
//! recomposes the document into LaTeX (its output is the recomposed LaTeX and, when the
//! document needs one, a style file; its tables `flm.labels.safe` and
//! `flm.output.latex-recomposed` are written under that workflow only);
//! [`latex-rendered`](flm_core::stdcore::backends::latex_rendered::WORKFLOW), which
//! writes the document as LaTeX for LuaLaTeX with everything FLM resolves already
//! resolved (its output is that LaTeX and a style file; its table
//! `flm.output.latex-rendered` and the tables named under it are written under that
//! workflow only);
//! [`html`](flm_core::stdcore::backends::html::WORKFLOW), which renders the document as one
//! HTML page (its tables `flm.output.html` and `flm.output.html.document` are written
//! under that workflow only); and [`check`](flm_core::stdcore::CHECK_WORKFLOW), which
//! checks the document and produces no output. Every rendering workflow runs the same
//! checks as `check`, because each one also asks the plan for the collection of checks
//! [`flm.validated`](flm_core::stdcore::VALIDATED). A document therefore gets the same
//! diagnostics whichever workflow runs, with one exception: an unresolved reference is a
//! Warning under `check` and an Error under a rendering workflow.
//!
//! `--width N` sets the text backend's line width in columns
//! ([`TextRenderOptions`](flm_core::stdcore::backends::text::TextRenderOptions)); the
//! default is 80.
//!
//! `--workflow` and `--width` apply to a run, and `--at parsed` performs none. Giving
//! either of them with `--at parsed`, including when `--at` is left at its default, is
//! therefore a usage error rather than being silently ignored: otherwise a misspelled
//! workflow name would pass unnoticed.
//!
//! # Diagnostics
//!
//! Every diagnostic, from the registry build, the parse or the run, is printed on standard
//! error, so standard output holds the requested JSON and nothing else. Diagnostics do not
//! decide the exit status: a document that reports errors still prints its view and exits
//! with status 0.
//!
//! # Exit status
//!
//! - `0`: the requested view was printed.
//! - `1`: a usage error; a file that cannot be read; a registry, plan or run that cannot
//!   be built or fails; a name that matches nothing (an unknown table, output or backend);
//!   or a table or output that the chosen workflow does not write.
//! - `2`: the tree asked for does not exist: the parse produced none (for instance, for
//!   content nested too deeply), a rewrite stage produced none, or the chosen workflow did not lower the
//!   tree for the backend asked for.

use std::process::ExitCode;

use flm_info::dump::{At, Error, Printed, Request, Subject, View, dump};

/// The usage line, printed alone when no file is given and after the message of most other
/// usage errors.
const USAGE: &str = "usage: flm dump FILE [--at parsed|frozen|lowered:<backend>] \
[--view compact|wire] [--table NAME | --output NAME] [--workflow NAME] [--width N]";

/// The command line of `flm dump`, parsed and checked for consistency.
#[derive(Debug)]
struct DumpArgs {
    /// The path of the file to process.
    file: String,
    /// What the engine gives, and the run it makes.
    request: Request,
}

/// Returns an [`Error`] with `message` and exit status 1, the status of a usage error and
/// of a file that cannot be read.
fn failure(message: impl Into<String>) -> Error {
    Error { message: message.into(), status: 1 }
}

/// Parses the arguments that follow `dump` into a [`DumpArgs`].
///
/// Options may appear in any order, before or after the file name; the one
/// argument that does not begin with `--` and is not an option's value is taken
/// as the file.
///
/// # Errors
///
/// Returns an [`Error`] with status 1 when: an option is unknown; an option's
/// value is missing; `--at`, `--view` or `--width` is given a value it does not
/// accept; a second file name appears; no file name appears; `--table` and
/// `--output` are given together; or `--workflow` or `--width` is given while the
/// subject is the parsed tree, which runs no workflow. Every message but the one
/// for a `--width` value carries the usage line.
fn parse_args(args: &[String]) -> Result<DumpArgs, Error> {
    let mut file = None;
    let mut at = At::Parsed;
    let mut view = View::Compact;
    let mut table = None;
    let mut output = None;
    let mut workflow = None;
    let mut width = None;
    let mut run_options_given = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let mut value = |name: &str| -> Result<String, Error> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| failure(format!("`{name}` must be followed by a value\n{USAGE}")))
        };
        match arg.as_str() {
            "--at" => {
                let spec = value("--at")?;
                at = match spec.as_str() {
                    "parsed" => At::Parsed,
                    "frozen" => At::Frozen,
                    other => match other.strip_prefix("lowered:") {
                        Some(backend) if !backend.is_empty() => At::Lowered(backend.to_string()),
                        _ => {
                            return Err(failure(format!(
                                "`--at` takes `parsed`, `frozen` or `lowered:<backend>`, \
                                 not `{spec}`\n{USAGE}"
                            )));
                        }
                    },
                };
            }
            "--view" => {
                let spec = value("--view")?;
                view = match spec.as_str() {
                    "compact" => View::Compact,
                    "wire" => View::Wire,
                    _ => {
                        return Err(failure(format!(
                            "`--view` takes `compact` or `wire`, not `{spec}`\n{USAGE}"
                        )));
                    }
                };
            }
            "--table" => table = Some(value("--table")?),
            "--output" => output = Some(value("--output")?),
            "--workflow" => {
                workflow = Some(value("--workflow")?);
                run_options_given = true;
            }
            "--width" => {
                let spec = value("--width")?;
                width = Some(
                    spec.parse::<u32>()
                        .map_err(|_| {
                            failure(format!(
                                "`--width` takes a whole number of columns, not `{spec}`"
                            ))
                        })?,
                );
                run_options_given = true;
            }
            other if other.starts_with("--") => {
                return Err(failure(format!("`{other}` is not an option of `flm dump`\n{USAGE}")));
            }
            _ if file.is_none() => file = Some(arg.clone()),
            _ => return Err(failure(format!("`flm dump` takes one file only, and `{arg}` is a second one\n{USAGE}"))),
        }
        i += 1;
    }
    let file = file.ok_or_else(|| failure(USAGE))?;
    let subject = match (table, output) {
        (Some(_), Some(_)) => {
            return Err(failure(format!(
                "`--table` and `--output` cannot be given together\n{USAGE}"
            )));
        }
        (Some(table), None) => Subject::Table(table),
        (None, Some(output)) => Subject::Output(output),
        (None, None) => Subject::Tree(at),
    };
    if run_options_given && subject == Subject::Tree(At::Parsed) {
        return Err(failure(format!(
            "`--workflow` and `--width` apply to a run, and `--at parsed` (the default) runs none: \
             give `--at frozen`, `--at lowered:<backend>`, `--table` or `--output`\n{USAGE}"
        )));
    }
    let mut request = Request::new(subject);
    request.view = view;
    if let Some(workflow) = workflow {
        request.workflow = workflow;
    }
    request.width = width;
    Ok(DumpArgs { file, request })
}

/// Reads the file of `args` and runs the engine over its text, printing each diagnostics
/// text on standard error and the view on standard output.
///
/// # Errors
///
/// An [`Error`] with status 1 when the file cannot be read; otherwise the engine's.
fn read_and_dump(args: &DumpArgs) -> Result<(), Error> {
    let text = std::fs::read_to_string(&args.file)
        .map_err(|error| failure(format!("`{}` cannot be read: {error}", args.file)))?;
    dump(&args.request, text, &args.file, &mut |printed| match printed {
        Printed::Diagnostics(text) => eprintln!("{text}"),
        Printed::View(text) => println!("{text}"),
    })
}

/// Runs `flm dump` over `args`, the arguments that follow the command's name,
/// and returns the process's exit status (listed in the [module
/// documentation](self#exit-status)).
///
/// A failure's message is printed on standard error after the prefix `flm: `.
pub(crate) fn main(args: &[String]) -> ExitCode {
    let result = parse_args(args).and_then(|args| read_and_dump(&args));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error { message, status }) => {
            eprintln!("flm: {message}");
            ExitCode::from(status)
        }
    }
}
