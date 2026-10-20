//! The extension `profile-pdf-latex`: the workflow `pdf-latex`, which the profile
//! `pdf-latex` runs, and its PDF output `pdf-latex`, made by compiling the files of
//! the output `latex-rendered` (the LaTeX for LuaLaTeX) with `latexmk -lualatex`.
//!
//! The extension is written against the guest client `flm_ext::guest`. The `flm` program
//! carries it compiled in (the engine `native`); `extensions/rust/build-wasm.sh` builds the
//! same crate as a WebAssembly module (the engine `wasm`).
//!
//! Its package `_` uses `stdcore`, declares the host service `programs` and the workflow
//! `pdf-latex` (extending `latex-rendered`, with the output `pdf-latex`), and holds the
//! feature `pdf-latex`. The workflow, the feature and the output are named after the
//! profile, and the names of the table, the stage and the diagnostics begin with
//! `pdf-latex.`: the extension layer lets an extension whose stem is `profile-X` declare its
//! names under `X` as well as under its stem. The feature declares:
//!
//! - the table `pdf-latex.document`, of the type `ArtifactSet`, which holds the PDF file;
//! - the output `pdf-latex` (media type `application/pdf`, extension `pdf`), made from the
//!   output `latex-rendered`, so that a compose template chosen for the command applies to
//!   `latex-rendered`;
//! - the stage `pdf-latex.compile` (effect `Io`, once per document), which reads the files
//!   of `latex-rendered`, finds `latexmk` on `PATH`, runs it through the host service
//!   `programs` in a temporary folder holding those files, and writes the PDF it collects;
//! - the configuration key `timeout_s`: the time, in seconds, that `latexmk` may take
//!   (600 by default).
//!
//! `latexmk` runs as `latexmk -norc -lualatex -f -interaction=nonstopmode -file-line-error
//! ENTRY`: `-norc` keeps a user's `latexmkrc` files from changing where the PDF is written;
//! without `-halt-on-error`, LaTeX goes on past an error, so a PDF usually results; with
//! `-f`, `latexmk` goes on running LaTeX after a pass that reported an error, as many passes
//! as the document needs, so that the cross-references of that PDF are resolved. An exit
//! status other than 0 is the Error `pdf-latex.failed`, and the PDF is still written when
//! LaTeX produced one. `-shell-escape` is never passed.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::data::table::TableValue;
use flm_ext::guest::core::registry::{FeatureBuilder, OutputDef, StandardTable};
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::host::HostHandle;
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Map, Value, json};

/// The extension's stem.
const STEM: &str = "profile-pdf-latex";

/// The name of the profile `pdf-latex`, which loads the extension `profile-pdf-latex`
/// and runs its workflow: the name of the extension's workflow, its feature and its
/// output, and the beginning of the name of every table, stage and diagnostic it declares.
const PROFILE: &str = "pdf-latex";

/// The extension's output.
const OUTPUT: &str = PROFILE;

/// The table holding the PDF file: the output's table and capability.
const DOCUMENT: &str = "pdf-latex.document";

/// The table holding the files of the output `latex-rendered`, which the stage reads.
const LATEX_FILES: &str = "flm.output.latex-rendered.document";

/// The host service that finds and runs `latexmk`.
const PROGRAMS: &str = "programs";

/// The program that compiles the LaTeX.
const LATEXMK: &str = "latexmk";

/// The arguments given to `latexmk` before the entry file's path.
// See decision rationale at [§flm-dd-dr:ext-pdf-latex]
const LATEXMK_ARGS: [&str; 5] = ["-norc", "-lualatex", "-f", "-interaction=nonstopmode", "-file-line-error"];

/// The time `latexmk` may take when the configuration does not set `timeout_s`.
const DEFAULT_TIMEOUT_S: u64 = 600;

/// The largest `timeout_s`: its number of milliseconds, given to the service `programs`, must
/// be a number the extension layer's data model carries (at most `i64::MAX`).
const MAX_TIMEOUT_S: u64 = i64::MAX as u64 / 1000;

/// The Error reported when `latexmk` is not on `PATH`.
const LATEXMK_MISSING: &str = "pdf-latex.latexmk-missing";

/// The Error reported when LaTeX fails, or produces no PDF.
const FAILED: &str = "pdf-latex.failed";

/// The Error reported for a file of `latex-rendered` that has no path.
const NO_PATH: &str = "pdf-latex.no-path";

/// How many lines of the program's output a failure quotes when the log records no error.
const LAST_LINES: usize = 5;

// The static `FLM_EXTENSION`, through which a program carrying the crate compiled in reaches
// it, and the module's exports in a build of the WebAssembly form.
flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    // LaTeX runs in a temporary folder holding the files of `latex-rendered` alone, so each
    // image is copied into those files: by default, `latex-rendered` refers to an image by
    // its path from the folder the output is written to, which the temporary folder is not.
    // See decision rationale at [§flm-dd-dr:ext-pdf-latex]
    let workflow = json!({
        "extends": "latex-rendered",
        "outputs": [OUTPUT],
        "settings": { "graphics": { "strategy": { "latex-rendered": "copy" } } },
        "docs": "The PDF output: the LaTeX for LuaLaTeX (`latex-rendered`), compiled by `latexmk -lualatex`.",
    });
    ext.package(
        "_",
        Package::new().uses(["stdcore"]).host([PROGRAMS]).flm(json!({ "workflows": { PROFILE: workflow } })).feature(
            PROFILE,
            Feature::new()
                .title("PDF through LaTeX")
                .docs(
                    "Makes the PDF output `pdf-latex`: compiles the files of the output `latex-rendered` with \
                     `latexmk -lualatex` in a temporary folder, through the host service `programs`.",
                )
                .settings(settings())
                .register(register),
        ),
    );
}

/// The feature's settings, declared as data: the configuration key `timeout_s`.
fn settings() -> Value {
    json!({
        "config": {
            "docs": "How LaTeX is run.",
            "schema": {
                "type": "object",
                "properties": {
                    "timeout_s": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_TIMEOUT_S,
                        "default": DEFAULT_TIMEOUT_S,
                        "description": "The time `latexmk` may take to compile the document, in seconds.",
                    },
                },
            },
        },
    })
}

fn register(b: &mut FeatureBuilder, settings: &Value) -> Result<(), ExtError> {
    let timeout_s = timeout_s(settings)?;
    b.table_named(DOCUMENT, StandardTable::ArtifactSet, "The PDF file of the output `pdf-latex`.")?;
    b.output(
        OutputDef::new(OUTPUT, DOCUMENT, DOCUMENT, "application/pdf", "pdf")
            .made_from("latex-rendered")
            .docs("The document as a PDF file, compiled by LaTeX from the output `latex-rendered`."),
    )?;
    b.stage(Arc::new(Compile { timeout_s }))
}

/// The configuration's `timeout_s`, or its default.
fn timeout_s(settings: &Value) -> Result<u64, ExtError> {
    match settings.get("timeout_s") {
        None | Some(Value::Null) => Ok(DEFAULT_TIMEOUT_S),
        Some(value) => value.as_u64().filter(|seconds| (1..=MAX_TIMEOUT_S).contains(seconds)).ok_or_else(|| {
            ExtError::Handler {
                detail: format!(
                    "the setting `timeout_s` of the feature `{PROFILE}` must be a positive whole number of seconds, at \
                     most {MAX_TIMEOUT_S}, not `{value}`"
                ),
                data: Value::Null,
            }
        }),
    }
}

/// The stage `pdf-latex.compile`: the files of `latex-rendered` compiled by `latexmk`, the
/// PDF written to `pdf-latex.document`.
struct Compile {
    /// The time `latexmk` may take, in seconds.
    timeout_s: u64,
}

impl Stage for Compile {
    fn decl(&self) -> StageDecl {
        StageDecl::new("pdf-latex.compile", Effect::Io, Scope::Document)
            .writes(DOCUMENT)
            .docs("compiles the files of the output `latex-rendered` into a PDF file with `latexmk -lualatex`")
    }

    fn needs(&self) -> Needs {
        Needs::new().reads(LATEX_FILES)
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        // The set's `violations` are reported by the stage that made the set (the final stage
        // of `latex-rendered` or its compose template).
        let artifacts = match cx.get(LATEX_FILES)? {
            Some(TableValue::Projection(Value::Object(mut set))) => match set.remove("artifacts") {
                Some(Value::Array(artifacts)) => artifacts,
                _ => Vec::new(),
            },
            _ => return Err(ExtError::Value { detail: format!("the table `{LATEX_FILES}` must be read as a set of files") }),
        };
        // Without an entry file, `latex-rendered` reported its own problems.
        let Some(entry) = artifacts.iter().find(|artifact| artifact["entry"] == Value::Bool(true)) else { return Ok(()) };
        let mut unplaced = false;
        for artifact in &artifacts {
            if !artifact["path"].is_string() {
                out.diags(Diagnostic::error(
                    NO_PATH,
                    format!(
                        "the file `{}` of the output `latex-rendered` must have a path, so that LaTeX can read it; the \
                         program running the extension `{STEM}` gives the files of an output no paths",
                        artifact_id(artifact)
                    ),
                ))?;
                unplaced = true;
            }
        }
        if unplaced {
            return Ok(());
        }
        let entry_path = entry["path"].as_str().unwrap_or_default().to_string();
        let files = program_files(artifacts);
        let HostHandle::Service(programs) = cx.host(PROGRAMS)? else {
            return Err(ExtError::Value { detail: format!("the host service `{PROGRAMS}` must be a named service") });
        };
        let found = programs.call("find", json!({ "names": [LATEXMK] }))?;
        let Some(latexmk) = found.as_str() else {
            out.diags(Diagnostic::error(
                LATEXMK_MISSING,
                format!(
                    "the program `{LATEXMK}`, which compiles the LaTeX output `latex-rendered` into the PDF output \
                     `{OUTPUT}`, must be in one of the folders of `PATH`"
                ),
            ))?;
            return Ok(());
        };
        let job = job_name(&entry_path);
        let (pdf, log) = (format!("{job}.pdf"), format!("{job}.log"));
        let timeout_ms = self.timeout_s.saturating_mul(1000);
        let mut args: Vec<Value> = LATEXMK_ARGS.iter().map(|arg| Value::from(*arg)).collect();
        args.push(Value::from(entry_path.as_str()));
        let mut request = Map::new();
        request.insert("program".to_string(), Value::from(latexmk));
        request.insert("args".to_string(), Value::Array(args));
        request.insert("files".to_string(), Value::Object(files));
        request.insert("collect".to_string(), json!([pdf, log]));
        request.insert("timeout_ms".to_string(), Value::from(timeout_ms));
        let mut answer = match programs.call("run", Value::Object(request)) {
            Ok(answer) => answer,
            Err(ExtError::Host { detail, .. }) => {
                // The service answers a program that runs past its time with this text, which
                // ends with the time given. See decision rationale at [§flm-dd-dr:ext-pdf-latex]
                let message = if detail.ends_with(&format!("must finish within {timeout_ms} milliseconds")) {
                    let unit = if self.timeout_s == 1 { "second" } else { "seconds" };
                    format!(
                        "`{LATEXMK}` must finish compiling `{entry_path}` within {} {unit}, the time that the setting \
                         `timeout_s` of the feature `{PROFILE}` allows",
                        self.timeout_s
                    )
                } else {
                    format!("`{LATEXMK}` must run to compile `{entry_path}`: {detail}")
                };
                out.diags(Diagnostic::error(FAILED, message).with_data(json!({ "entry": entry_path })))?;
                return Ok(());
            }
            Err(other) => return Err(other),
        };
        let status = answer["status"].as_i64();
        let mut collected = match answer.get_mut("files").map(Value::take) {
            Some(Value::Object(collected)) => collected,
            _ => Map::new(),
        };
        let content = collected.remove(&pdf);
        if status != Some(0) || content.is_none() {
            let outcome = match status {
                Some(0) => format!("`{LATEXMK}` exited with the status 0 and wrote no file `{pdf}`"),
                Some(code) => format!("`{LATEXMK}` exited with the status {code}"),
                None => format!("`{LATEXMK}` was stopped by a signal"),
            };
            let log_text = collected.get(&log).and_then(|content| content["text"].as_str());
            let detail = match log_text.and_then(first_error) {
                Some(error) => format!("; the log records:\n{}", quoted(&error)),
                None => {
                    let output = last_lines(answer["stdout"].as_str().unwrap_or(""), answer["stderr"].as_str().unwrap_or(""));
                    if output.is_empty() { String::new() } else { format!("; its output ends with:\n{}", quoted(&output)) }
                }
            };
            out.diags(
                Diagnostic::error(FAILED, format!("LaTeX must compile `{entry_path}` without an error; {outcome}{detail}"))
                    .with_data(json!({ "entry": entry_path, "status": status })),
            )?;
        }
        if let Some(content) = content {
            out.table(DOCUMENT)?.push(pdf_artifact(content))?;
        }
        Ok(())
    }
}

/// The identity of `artifact`, read from its data form, as `OWNER:NAME`.
fn artifact_id(artifact: &Value) -> String {
    let part = |key: &str| artifact["id"][key].as_str().unwrap_or("").to_string();
    format!("{}:{}", part("owner"), part("name"))
}

/// The files given to `latexmk`: each artifact's content at its path, each file of a
/// folder artifact at `PATH/FILE`, in the form the service `programs` takes (the form in
/// which a read gives a content, passed on unchanged). Every artifact has a path.
fn program_files(artifacts: Vec<Value>) -> Map<String, Value> {
    let mut files = Map::new();
    for mut artifact in artifacts {
        let path = artifact["path"].as_str().unwrap_or_default().to_string();
        match artifact["data"].take() {
            Value::Object(mut data) => {
                if let Some(content) = data.remove("bytes") {
                    files.insert(path, content);
                } else if let Some(Value::Array(folder)) = data.remove("files") {
                    for file in folder {
                        let mut pair = match file {
                            Value::Array(pair) => pair,
                            _ => continue,
                        };
                        if let (Some(content), Some(Value::String(name))) = (pair.pop(), pair.pop()) {
                            files.insert(format!("{path}/{name}"), content);
                        }
                    }
                }
            }
            _ => continue,
        }
    }
    files
}

/// The name LaTeX gives the files it writes for the entry file `entry`: its file name,
/// without the folders and the extension `.tex`.
fn job_name(entry: &str) -> &str {
    let file = entry.rsplit('/').next().unwrap_or(entry);
    file.strip_suffix(".tex").unwrap_or(file)
}

/// The first error the log `log` records: with `-file-line-error`, the first line of the
/// form `FILE:LINE: message`, with the one or two lines after it that go with it (up to the
/// next blank line); `None` when the log records none.
fn first_error(log: &str) -> Option<String> {
    let lines: Vec<&str> = log.lines().collect();
    let at = lines.iter().position(|line| is_file_line_error(line))?;
    let mut quoted = vec![lines[at].trim_end()];
    quoted.extend(lines[at + 1..].iter().take(2).map(|line| line.trim_end()).take_while(|line| !line.trim().is_empty()));
    Some(quoted.join("\n"))
}

/// Whether `line` holds `:LINE: ` after a file's name: a colon, one or more digits, a colon
/// and a space, with at least one character before it.
fn is_file_line_error(line: &str) -> bool {
    line.match_indices(':').any(|(colon, _)| {
        let rest = &line[colon + 1..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        colon > 0 && digits > 0 && rest[digits..].starts_with(": ")
    })
}

/// The last lines of the program's output that are not blank, at most [`LAST_LINES`]: of its
/// standard error, or of its standard output when its standard error holds none.
fn last_lines(stdout: &str, stderr: &str) -> String {
    let mut lines = lines_of(stderr);
    if lines.is_empty() {
        lines = lines_of(stdout);
    }
    let skip = lines.len().saturating_sub(LAST_LINES);
    lines[skip..].join("\n")
}

/// The lines of `text` that are not blank, their ends trimmed.
fn lines_of(text: &str) -> Vec<String> {
    text.lines().filter(|line| !line.trim().is_empty()).map(|line| line.trim_end().to_string()).collect()
}

/// `text`, each line indented by four spaces, as a message quotes lines of LaTeX's log or
/// of a program's output.
fn quoted(text: &str) -> String {
    text.lines().map(|line| format!("    {line}")).collect::<Vec<_>>().join("\n")
}

/// The entry artifact of the output `pdf-latex`, holding `content`, the PDF in the form the
/// service `programs` collected it (`{ base64 }`, or `{ text }` for a PDF that is UTF-8
/// text).
fn pdf_artifact(content: Value) -> Value {
    let mut data = Map::new();
    data.insert("bytes".to_string(), content);
    let mut artifact = Map::new();
    artifact.insert("id".to_string(), json!({ "owner": OUTPUT, "name": "document" }));
    artifact.insert("media_type".to_string(), Value::from("application/pdf"));
    artifact.insert("entry".to_string(), Value::Bool(true));
    artifact.insert("extension".to_string(), Value::from("pdf"));
    artifact.insert("data".to_string(), Value::Object(data));
    Value::Object(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_error_of_the_log_is_its_first_file_line_error_with_the_lines_after_it() {
        let log = "This is LuaHBTeX\n(./paper.tex\n./paper.tex:3: Undefined control sequence.\nl.3 \\foo\n          \n\
                   ./paper.tex:5: Missing $ inserted.\n";
        assert_eq!(first_error(log).as_deref(), Some("./paper.tex:3: Undefined control sequence.\nl.3 \\foo"));
        let latex_error = "./paper.tex:7: LaTeX Error: Environment foo undefined.\n\nSee the LaTeX manual.\n";
        assert_eq!(first_error(latex_error).as_deref(), Some("./paper.tex:7: LaTeX Error: Environment foo undefined."));
        let two_lines = "x.tex:12: Missing } inserted.\n<inserted text>\n                }\nl.12 a\n";
        assert_eq!(first_error(two_lines).as_deref(), Some("x.tex:12: Missing } inserted.\n<inserted text>\n                }"));
        assert_eq!(first_error("Output written on paper.pdf (1 page).\nsee p. 3: nothing\n:3: no file\n"), None);
    }

    #[test]
    fn the_last_lines_are_those_of_standard_error_else_of_standard_output() {
        let stderr = "a\n\nb\nc\nd\ne\nf\n  \n";
        assert_eq!(last_lines("out", stderr), "b\nc\nd\ne\nf");
        assert_eq!(last_lines("one\ntwo  \n", " \n"), "one\ntwo");
        assert_eq!(last_lines("", ""), "");
    }

    #[test]
    fn the_job_name_is_the_entry_file_s_name_without_tex() {
        assert_eq!(job_name("paper.tex"), "paper");
        assert_eq!(job_name("out/paper.tex"), "paper");
        assert_eq!(job_name("paper"), "paper");
    }

    #[test]
    fn timeout_s_is_a_positive_whole_number_600_by_default() {
        assert_eq!(timeout_s(&json!({})).unwrap(), 600);
        assert_eq!(timeout_s(&json!({ "timeout_s": 30 })).unwrap(), 30);
        assert_eq!(timeout_s(&json!({ "timeout_s": 9_223_372_036_854_775_u64 })).unwrap(), MAX_TIMEOUT_S);
        for wrong in [json!(0), json!(-3), json!("60"), json!(1.5), json!(9_223_372_036_854_776_u64), json!(u64::MAX)] {
            let error = timeout_s(&json!({ "timeout_s": wrong })).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!(
                    "the setting `timeout_s` of the feature `pdf-latex` must be a positive whole number of seconds, at \
                     most 9223372036854775, not `{wrong}`"
                )
            );
        }
    }

    #[test]
    fn the_files_given_to_latexmk_are_at_their_paths_a_folder_s_under_its_path() {
        let artifacts = vec![
            json!({ "id": { "owner": "o", "name": "a" }, "path": "paper.tex", "data": { "bytes": { "text": "x" } } }),
            json!({ "id": { "owner": "o", "name": "b" }, "path": "paper-img", "data": { "files": [["1.png", { "base64": "AA==" }]] } }),
        ];
        let files = program_files(artifacts);
        assert_eq!(Value::Object(files), json!({ "paper.tex": { "text": "x" }, "paper-img/1.png": { "base64": "AA==" } }));
    }
}
