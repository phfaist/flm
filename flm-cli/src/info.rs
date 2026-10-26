//! `flm info`: describes the environment in which a document is processed, as pages of
//! documentation, and prints them as text or as JSON, or writes one page as an FLM document.
//!
//! The environment is what the host library prepares for a document before it reads the
//! document's content: the packages in use, the registry their features build (the
//! commands, the element types, the stages, the workflows and every other name), the
//! settings in effect and the plan of the profile in view. Without `-d`, it is the
//! environment of a document that specifies nothing, prepared over an empty text, as `flm
//! schema` prepares it; with `-d DOCUMENT`, that of the document, prepared as `flm render`
//! prepares it. The build options (`-p`, `--use`, `--initial-use`, `--math`,
//! `--extensions-dir`, `-O`) shape it as they shape a build ([`InfoArgs::build_args`]), and
//! nothing else. The diagnostics that the preparation reports without stopping (a warning
//! about a setting, for example) are printed on standard error before the pages.
//!
//! The library [`flm_info`] describes the environment and makes its pages
//! ([`flm_info::pages`]); this module gives it what only the program knows
//! ([`ProgramInfo`]: the commands and options, the environment variables, the folders the
//! program reads, the packages and extensions a document may use), chooses the pages, and
//! prints them:
//!
//! - as JSON with `--json`: one object holding a header and the pages
//!   ([`Output`](flm_info::pages::Output)), each page as [`Page::to_json`] writes it;
//! - as text otherwise: each page's source in FLM ([`source`]) rendered by the host library
//!   with the profile `text`, with the documentation markup of the extension `flmdoc`,
//!   which the program carries among its bundled extensions. The pages are rendered with the
//!   program's defaults, and nothing but the terminal's width shapes them: when standard
//!   output is a terminal, the lines are as wide as the terminal (`COLUMNS`, else the width
//!   the terminal reports); otherwise, the text backend's default width applies. No build
//!   option reaches this rendering;
//! - as an FLM document with `--write-flm FILE`: the source in FLM of the one page chosen,
//!   the source that the text output renders, written to FILE (or on standard output for
//!   `-`), so that `flm render FILE` renders the page ([`write_flm`]).
//!
//! Before a page is rendered or written as an FLM document, each documentation text it
//! embeds as FLM is parsed with the registry that renders it ([`text_problems`]); the
//! library lists these texts, each with whose text it is ([`texts`]). The program's own
//! texts are tested, so a text that does not parse is one the program does not own: an
//! extension's, or one written in a front matter or with `-O`. Such a text is written into
//! the page as plain text
//! ([`SourceOptions::plain_texts`]), and a warning shows it as written, with the page, its
//! source as the library gives it and the problems found; the exit status stays 0. The
//! source of a text written as data in a settings value (the documentation text of a
//! workflow, or of a user definition whose text no other entry of the settings shares) is
//! where it was written: a package and the place in its file, a front matter, or the option
//! `-O`; that of any other text is the feature that declares it, with that feature's
//! package.

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::Arc;

use flm_core::core::registry::PackageFactory;
use flm_core::lang::{BuiltLang, FlmDiagnostic, FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::doc::FragmentLayout;
use flm_info::host::OriginView;
use flm_info::pages::{
    EmbeddedText, EnvironmentVariable, ImageFolders, Output, Page, ProgramCommand, ProgramExtensions, ProgramFolder,
    ProgramInfo, ProgramOption, ProgramPaths, SourceOptions, TextHolder, TextSource, Tree, source, texts,
};
use flm_info::{Environment, summary_of};
use flm_main::extensions::ExtensionFinder;
use flm_main::files::{BuildFiles, DiskFiles, Files, ServiceFiles};
use flm_main::output::{MemorySink, Naming, write_file};
use flm_main::packages::ExtensionFolder;
use flm_main::report::{self, ContentOffsets};
use flm_main::session::{Input, Prepared, Request, build, prepare};
use flm_main::settings::CommandLineSetting;

use crate::args::{FolderOrigin, InfoArgs, MAGICK_VARIABLE, cache_folder, environment_search_path_with_origins};
use crate::{
    BROWSER_VARIABLE, CANNOT_START, Cli, DOCUMENT_ERRORS, DONE, PROGRAM_FAILURE, cannot_start, reporter, stopped,
};

/// The environment variable that gives the width of the terminal in columns, `COLUMNS`,
/// with which `flm info` writes its pages when standard output is a terminal.
pub const COLUMNS_VARIABLE: &str = "COLUMNS";

/// The path of the page that `flm info` shows when no path is given: `/`, the environment at
/// a glance.
const ROOT: &str = "/";

/// The rule by which the host reads each image folder of the setting
/// `graphics-files.search_path`, as the page `program/paths` states it.
const IMAGE_FOLDER_RULE: &str = "Each folder is relative to the folder of the file in which the image is written \
                                 (the document, or a file it includes), or absolute.";

/// The most pages that the message for a path with no page proposes.
const NEAREST: usize = 5;

/// The name of the source of a documentation text parsed on its own ([`text_problems`]), as
/// the place of a problem in it shows it (`text:1:12`).
const TEXT_ORIGIN: &str = "text";

/// Runs `flm info`: prepares the environment, chooses the pages of `args.paths` and prints
/// them on standard output, as JSON under `--json`, else as text; under `--write-flm`, it
/// writes the one page chosen as an FLM document instead ([`write_flm`]). Returns the exit
/// status: 0 when every page was shown or written; 1 when the file of `--write-flm` cannot
/// be written; 2 when the arguments are wrong, the environment cannot be prepared (the
/// document of `-d` cannot be read, a package cannot be found, a setting cannot be read), a
/// path gives no page, or `--write-flm` is given and the paths do not choose exactly one
/// page; 3 on a failure of the program, such as a page that does not render or standard
/// output that cannot be written.
pub(crate) fn run(args: &InfoArgs, packages: &[PackageFactory]) -> u8 {
    let request = match args.build_args().request() {
        Ok(request) => request,
        Err(message) => {
            reporter().error(&message);
            return CANNOT_START;
        }
    };
    let input = request.input.clone();
    let files: Arc<dyn Files> = Arc::new(DiskFiles);
    // The environment of a document that specifies nothing (the request's input is then an
    // empty text), or of the document of `-d`, shaped by the build options of `flm render`.
    // See decision rationale at [§flm-dd-dr:info-command-line].
    let prepared = match prepare(request, Arc::clone(&files), packages) {
        Ok(prepared) => prepared,
        Err(error) => return cannot_start(&error),
    };
    // What the preparation found without stopping, as `flm check` reports it.
    if !prepared.diagnostics().is_empty() {
        reporter().diagnostics(prepared.diagnostics(), &ContentOffsets::new());
    }
    let env = Environment::of_prepared(&prepared);
    let program = match program_info(args, &env, packages, &input, &files) {
        Ok(program) => program,
        Err(message) => {
            reporter().error(&message);
            return PROGRAM_FAILURE;
        }
    };
    let tree = Tree::new(&env, Some(program));
    let pages = match chosen(&tree, &args.paths) {
        Ok(pages) => pages,
        Err(messages) => {
            for message in messages {
                reporter().error(&message);
            }
            return CANNOT_START;
        }
    };
    match (&args.write_flm, args.json) {
        (Some(path), _) => write_flm(&pages, path, packages),
        (None, true) => print_json(&pages, args.recursive),
        (None, false) => print_text(&tree, &pages, args.recursive, packages),
    }
}

/// Returns the pages that `paths` ask for, in order: for each path, its page
/// ([`Tree::page`]), else the pages of the entities and commands of that name
/// ([`Tree::lookup`]), every one, in tree order; `/` when `paths` is empty.
///
/// # Errors
///
/// One message per path that gives no page, each with the nearest pages ([`nearest`]).
fn chosen(tree: &Tree, paths: &[String]) -> Result<Vec<Page>, Vec<String>> {
    if paths.is_empty() {
        return Ok(tree.page(ROOT).into_iter().collect());
    }
    let mut pages = Vec::new();
    let mut missing = Vec::new();
    for path in paths {
        match tree.page(path) {
            Some(page) => pages.push(page),
            None => {
                let hits = tree.lookup(path);
                if hits.is_empty() {
                    missing.push(path.as_str());
                }
                pages.extend(hits);
            }
        }
    }
    if missing.is_empty() {
        return Ok(pages);
    }
    let every: Vec<String> = tree.walk().map(|page| page.path).collect();
    Err(missing
        .into_iter()
        .map(|path| {
            let near = nearest(&every, path);
            if near.is_empty() {
                format!("no page or name `{path}`")
            } else {
                format!("no page or name `{path}`; the nearest pages are: `{}`", near.join("`, `"))
            }
        })
        .collect())
}

/// Returns the paths among `every` (in tree order) nearest to `path`, which is no page's
/// path and no name: first those whose last segment is `path`'s last segment, then those
/// that share with `path` the longest run of first characters, when that run covers at least
/// `path`'s first segment (up to its first `/`, or the whole path when it has none), each
/// once, at most [`NEAREST`] in all.
fn nearest(every: &[String], path: &str) -> Vec<String> {
    let last = |text: &str| text.trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_string();
    let wanted = last(path);
    let mut near: Vec<String> =
        every.iter().filter(|candidate| !wanted.is_empty() && last(candidate) == wanted).cloned().collect();
    let shared = |candidate: &str| candidate.chars().zip(path.chars()).take_while(|(one, other)| one == other).count();
    let first_segment = path.split('/').next().unwrap_or_default().chars().count().max(1);
    let longest = every.iter().map(|candidate| shared(candidate)).max().unwrap_or(0);
    if longest >= first_segment {
        for candidate in every.iter().filter(|candidate| shared(candidate) == longest) {
            if !near.contains(candidate) {
                near.push(candidate.clone());
            }
        }
    }
    near.truncate(NEAREST);
    near
}

/// Prints `pages` on standard output as one JSON object ([`Output`]), indented, followed
/// by a newline; under `-r` (`recursive`) each page holds the pages below it in full.
/// Returns the exit status: 0, or 3 when standard output cannot be written.
fn print_json(pages: &[Page], recursive: bool) -> u8 {
    // The header first, then the pages, always in that one shape. See decision rationale at
    // [§flm-dd-dr:json-conventions].
    let output = Output::new(pages.iter().map(|page| page.to_json(recursive)).collect());
    let text = match serde_json::to_string_pretty(&output) {
        Ok(text) => text,
        Err(error) => {
            reporter().error(&format!("the pages cannot be written as JSON: {error}"));
            return PROGRAM_FAILURE;
        }
    };
    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(text.as_bytes()).and_then(|()| stdout.write_all(b"\n")).and_then(|()| stdout.flush()) {
        Ok(()) => DONE,
        Err(error) => {
            reporter().error(&format!("standard output cannot be written: {error}"));
            PROGRAM_FAILURE
        }
    }
}

/// Writes the one page of `pages` as an FLM document ([`page_source`]), ending with one
/// newline, to the file `path` (through [`write_file`], which keeps no record of the file),
/// or on standard output when `path` is `-`. Returns the exit status: 0; 1 when the file
/// cannot be written; 2 when `pages` does not hold exactly one page; 3 when the page cannot
/// be prepared or standard output cannot be written.
fn write_flm(pages: &[Page], path: &str, packages: &[PackageFactory]) -> u8 {
    let [page] = pages else {
        let paths: Vec<&str> = pages.iter().map(|page| page.path.as_str()).collect();
        reporter().error(&format!(
            "the paths given must choose exactly one page with `--write-flm`; they choose {}: `{}`",
            pages.len(),
            paths.join("`, `")
        ));
        return CANNOT_START;
    };
    let text = match page_source(page, packages) {
        Ok(text) => format!("{}\n", text.trim_end_matches('\n')),
        Err(status) => return status,
    };
    if path == "-" {
        let mut stdout = std::io::stdout().lock();
        if let Err(error) = stdout.write_all(text.as_bytes()).and_then(|()| stdout.flush()) {
            reporter().error(&format!("standard output cannot be written: {error}"));
            return PROGRAM_FAILURE;
        }
    } else if let Err(error) = write_file(Path::new(path), text.as_bytes()) {
        reporter().error(&error.detail);
        return DOCUMENT_ERRORS;
    }
    DONE
}

/// Prints `pages` on standard output as text, each rendered by [`render`], separated by one
/// blank line; under `-r` (`recursive`), each page is followed by every page below it, in
/// tree order. Each page is printed once rendered. Returns the exit status: 0, or 3 when a
/// page does not render (the pages before it are printed) or standard output cannot be
/// written.
fn print_text(tree: &Tree, pages: &[Page], recursive: bool, packages: &[PackageFactory]) -> u8 {
    let settings = text_settings();
    let mut stdout = std::io::stdout().lock();
    let mut first = true;
    let mut show = |page: &Page| -> Result<(), u8> {
        let text = render(page, &settings, packages)?;
        let separator: &[u8] = if first { b"" } else { b"\n" };
        first = false;
        stdout
            .write_all(separator)
            .and_then(|()| stdout.write_all(text.as_bytes()))
            .and_then(|()| stdout.write_all(b"\n"))
            .and_then(|()| stdout.flush())
            .map_err(|error| {
                reporter().error(&format!("standard output cannot be written: {error}"));
                PROGRAM_FAILURE
            })
    };
    for page in pages {
        if let Err(status) = visit(tree, page, recursive, &mut show) {
            return status;
        }
    }
    DONE
}

/// Hands `page` to `show`, then, when `recursive`, every page below it, in tree order (a
/// page, then the pages below it, in the order of its children).
fn visit(tree: &Tree, page: &Page, recursive: bool, show: &mut dyn FnMut(&Page) -> Result<(), u8>) -> Result<(), u8> {
    show(page)?;
    if recursive {
        for entry in &page.children {
            if let Some(child) = tree.page(&entry.path) {
                visit(tree, &child, true, show)?;
            }
        }
    }
    Ok(())
}

/// Returns the settings with which the pages are rendered as text: when standard output is
/// a terminal, its width as `text.width` ([`terminal_width`]); otherwise none, and the text
/// backend's default width applies. Nothing else shapes the pages: the build options of the
/// command (`-p`, `-O`, …) shape the environment described, not its pages.
fn text_settings() -> Vec<CommandLineSetting> {
    let mut settings = Vec::new();
    // A terminal gets the text at its width. The output for a terminal (styles, colors) of a
    // later version goes here: when standard output is a terminal, the pages would be
    // rendered with it in place of the text.
    if std::io::stdout().is_terminal()
        && let Some(width) = terminal_width()
    {
        settings.push(CommandLineSetting::new(format!("text.width={width}"), "the width of the terminal"));
    }
    settings
}

/// Returns the width of the terminal in columns: the value of [`COLUMNS_VARIABLE`] when it
/// is a whole number above 0, else the width the terminal reports (`terminal_size`), when
/// it reports one.
fn terminal_width() -> Option<u32> {
    let columns = std::env::var(COLUMNS_VARIABLE).ok().and_then(|value| value.trim().parse::<u32>().ok());
    columns.filter(|width| *width > 0).or_else(|| {
        terminal_size::terminal_size()
            .map(|(terminal_size::Width(width), _)| u32::from(width))
            .filter(|width| *width > 0)
    })
}

/// Renders `page` as text: its source in FLM, prepared ([`prepare_page`]) and built by the
/// host library with the profile `text`, the settings `settings` and the program's defaults
/// otherwise, its output kept in memory. Returns the text output with the line endings at
/// its end removed.
///
/// Before the page is built, each documentation text it embeds as FLM is parsed with the
/// registry of the rendering; a text that does not parse is written as plain text, after a
/// warning on standard error ([`checked_options`]).
///
/// # Errors
///
/// The exit status 3, once the problems are printed on standard error, when the page does
/// not render without a problem: a page that does not render is a failure of the program.
fn render(page: &Page, settings: &[CommandLineSetting], packages: &[PackageFactory]) -> Result<String, u8> {
    let sink = Arc::new(MemorySink::new());
    let mut prepared = prepare_page(page, &SourceOptions::default(), settings, Some(&sink), packages)?;
    let options = checked_options(page, &prepared);
    if !options.plain_texts.is_empty() {
        // Nothing is built before this, so the sink is still empty when the page is
        // prepared again.
        prepared = prepare_page(page, &options, settings, Some(&sink), packages)?;
    }
    let built = build(&prepared).map_err(|error| {
        stopped(&error);
        page_failed(page)
    })?;
    if !built.diagnostics.is_empty() {
        reporter().diagnostics(&built.diagnostics, &built.content_offsets);
        return Err(page_failed(page));
    }
    let files = sink.files();
    let Some(file) = files.values().find(|file| file.entry) else {
        return Err(page_failed(page));
    };
    let text = String::from_utf8_lossy(&file.bytes);
    Ok(text.trim_end_matches(['\n', '\r']).to_string())
}

/// Returns `page`'s source in FLM ([`source`]) as [`render`] renders it: the source is
/// prepared once with no settings ([`prepare_page`]), so that each documentation text it
/// embeds as FLM is parsed with the registry of the rendering, and a text that does not
/// parse is written as plain text, after a warning on standard error ([`checked_options`]).
///
/// # Errors
///
/// The exit status 3, once the problems are printed on standard error, when the source
/// cannot be prepared: a page that does not render is a failure of the program.
fn page_source(page: &Page, packages: &[PackageFactory]) -> Result<String, u8> {
    let prepared = prepare_page(page, &SourceOptions::default(), &[], None, packages)?;
    Ok(source(page, &checked_options(page, &prepared)))
}

/// Prepares `page`'s source in FLM, written with `options` ([`source`]), whose front matter
/// uses the extension `flmdoc` and no math processor: with the profile `text`, the settings
/// `settings` and the program's defaults otherwise, its output sent to `sink` when it is
/// built, if a sink is given. No folder of extensions is searched, so that the extension
/// `flmdoc` is the one the program carries.
///
/// # Errors
///
/// The exit status 3, once the problems are printed on standard error, when the source
/// cannot be prepared: a page that does not render is a failure of the program.
fn prepare_page(
    page: &Page,
    options: &SourceOptions,
    settings: &[CommandLineSetting],
    sink: Option<&Arc<MemorySink>>,
    packages: &[PackageFactory],
) -> Result<Prepared, u8> {
    let mut request = Request::new(Input::Text(source(page, options)));
    request.profile = Some(String::from("text"));
    request.default_math = crate::default_math();
    request.options = settings.to_vec();
    request.naming = Naming::File { name: String::from("page") };
    request.sink = sink.map(|sink| Arc::clone(sink) as Arc<dyn flm_main::output::Sink>);
    prepare(request, Arc::new(DiskFiles), packages).map_err(|error| {
        stopped(&error);
        page_failed(page)
    })
}

/// Prints on standard error that `page` must render without problems, and returns the exit
/// status 3: a page that does not render is a failure of the program.
fn page_failed(page: &Page) -> u8 {
    reporter().error(&format!("the page `{}` must render without problems", page.path));
    PROGRAM_FAILURE
}

/// Returns the options with which `page`'s source is written ([`source`]) so that it
/// renders. Each documentation text the page embeds as FLM ([`texts`]) is parsed with the
/// registry of `prepared`, the page's source prepared for rendering ([`text_problems`]); a
/// text that does not parse is among the texts to write as plain text
/// ([`SourceOptions::plain_texts`]), after a warning on standard error ([`unparsed_text`])
/// that shows it, its page, its source as [`texts`] gives it and the problems found. A
/// failing text that is the summary of another failing text of the same source gets no
/// warning of its own: the warning about the whole text covers it.
fn checked_options(page: &Page, prepared: &Prepared) -> SourceOptions {
    // The program's own texts are tested, so a documentation text that does not parse is
    // one the program does not own: an extension's, or one written in a front matter or with
    // `-O`. The page shows it as plain text.
    // Each distinct text is parsed once, though several sources may give it.
    let mut parsed: Vec<(String, Vec<FlmDiagnostic>)> = Vec::new();
    let mut failing: Vec<(EmbeddedText, Vec<FlmDiagnostic>)> = Vec::new();
    for text in texts(page) {
        let problems = match parsed.iter().find(|(known, _)| *known == text.text) {
            Some((_, problems)) => problems.clone(),
            None => {
                let problems = text_problems(prepared.registry().lang(), &text.text);
                parsed.push((text.text.clone(), problems.clone()));
                problems
            }
        };
        if !problems.is_empty() {
            failing.push((text, problems));
        }
    }
    for (text, problems) in &failing {
        let summarizes = failing.iter().any(|(other, _)| {
            other.source == text.source && other.text != text.text && summary_of(&other.text) == text.text
        });
        if !summarizes {
            reporter().warning(&unparsed_text(&text.text, &source_text(&text.source), &page.path, problems));
        }
    }
    let mut options = SourceOptions::default();
    options.plain_texts = failing.into_iter().map(|(text, _)| text.text).collect();
    options
}

/// The warning about `text`, a documentation text that does not parse, shown on the page
/// `path`: the text's source after the words "the documentation text" (`source`, as
/// [`source_text`] writes it), the page, the condition, then the text exactly as
/// written, each of its lines after `  | `, and the problems found, each as `flm check`
/// writes it, indented. A problem's place counts lines and columns in the text as shown.
fn unparsed_text(text: &str, source: &str, path: &str, problems: &[FlmDiagnostic]) -> String {
    let mut message = format!("the documentation text{source} on the page `{path}` must be valid FLM:");
    for line in text.lines() {
        message.push_str(if line.is_empty() { "\n  |" } else { "\n  | " });
        message.push_str(line);
    }
    for problem in problems {
        message.push_str("\n  ");
        message.push_str(&report::diagnostic(problem).replace('\n', "\n  "));
    }
    message
}

/// The source of a documentation text (`source`, as [`texts`] gives it), as a warning about
/// the text says it after the words "the documentation text": `declared by the feature `F`
/// (package `P`)` for a text a feature declares; for a text written as data, what holds it
/// and where it was written ([`written_at`]): `of the workflow `W` (…)`, `of the entry `E`
/// of the setting `F.K` (…)` or `of the setting `F.K` (…)`; nothing for a text of FLM's own.
fn source_text(source: &TextSource) -> String {
    match source {
        TextSource::Feature { feature, package: Some(package) } => {
            format!(" declared by the feature `{feature}` (package `{package}`)")
        }
        TextSource::Feature { feature, package: None } => format!(" declared by the feature `{feature}`"),
        TextSource::Written { holder, origin } => {
            let holder = match holder {
                TextHolder::Workflow { name } => format!("the workflow `{name}`"),
                TextHolder::Setting { feature, key, entry: Some(entry) } => {
                    format!("the entry `{entry}` of the setting `{feature}.{key}`")
                }
                TextHolder::Setting { feature, key, entry: None } => format!("the setting `{feature}.{key}`"),
                _ => String::from("a settings value"),
            };
            format!(" of {holder} ({})", written_at(origin))
        }
        _ => String::new(),
    }
}

/// Where a settings value was written (`origin`), as a warning about a documentation text
/// written there says it: for a package's value, the package, as the page `packages/` names
/// it, and the place in the extension's manifest when it is known
/// (``package `doc/_flmextensions/drafts.yaml#_`, doc/_flmextensions/drafts.yaml:9:14``); for
/// any other source, the origin as the host writes it (a place in a front matter, the option
/// of the command line).
fn written_at(origin: &OriginView) -> String {
    match (&origin.package, origin.line) {
        (Some(package), Some(_)) => format!("package `{package}`, {}", origin.text),
        (Some(package), None) => format!("package `{package}`"),
        (None, _) => origin.text.clone(),
    }
}

/// Parses `text`, a documentation text, as FLM with `lang`, the language of the registry
/// that renders the pages (the package `std` and the extension `flmdoc`), as a fragment of
/// blocks, and returns the problems found: none when the text parses without a problem. A
/// problem's place is in the text alone, which it names `text` (`text:1:12`).
///
/// The language comes from the host's preparation of a page's source, or of any text whose
/// front matter uses `flmdoc` with the profile `text`
/// ([`Prepared::registry`](flm_main::session::Prepared::registry), then
/// [`FlmRegistry::lang`](flm_core::core::registry::FlmRegistry::lang)).
pub fn text_problems(lang: &BuiltLang, text: &str) -> Vec<FlmDiagnostic> {
    let source = Arc::new(FlmSource::new(text.to_string()).with_origin(FlmOrigin::new(TEXT_ORIGIN)));
    match parse_fragment(lang, source, FragmentLayout::Blocks, &ParseOptions::default()) {
        Ok(fragment) => fragment.diags().as_slice().to_vec(),
        Err(error) => vec![error.diagnostic()],
    }
}

/// Returns what the program tells the pages about itself ([`ProgramInfo`]): its version, its
/// commands and their options, the environment variables it reads, the folders it reads
/// for the document `input` (the request `args` make) and the packages and extensions a
/// document may use.
///
/// # Errors
///
/// A message when the factory of a compiled package fails.
fn program_info(
    args: &InfoArgs,
    env: &Environment,
    packages: &[PackageFactory],
    input: &Input,
    files: &Arc<dyn Files>,
) -> Result<ProgramInfo, String> {
    let folders = environment_search_path_with_origins(&args.extensions_dirs, input);
    let search_path: Vec<ExtensionFolder> = folders.iter().map(|(folder, _)| folder.clone()).collect();
    let cache = cache_folder(input);
    let cache_origin = if matches!(input, Input::File(_)) && cache != Path::new(".") {
        "the document's folder"
    } else {
        "the current folder"
    };
    let finder = ExtensionFinder::new(BuildFiles::new(Arc::clone(files)), ServiceFiles::new(Arc::clone(files)), search_path);
    let mut compiled: Vec<String> =
        crate::schema::compiled(packages)?.iter().map(|factory| factory.name().to_string()).collect();
    compiled.sort();
    #[cfg(feature = "bundled-extensions")]
    let bundled: Vec<String> = flm_main::bundled::stems().into_iter().map(str::to_string).collect();
    #[cfg(not(feature = "bundled-extensions"))]
    let bundled: Vec<String> = Vec::new();
    Ok(ProgramInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: commands(),
        environment_variables: environment_variables(),
        paths: ProgramPaths {
            search_path: folders.iter().map(|(folder, origin)| program_folder(&folder.path, origin_text(*origin))).collect(),
            cache_folder: Some(program_folder(&cache, cache_origin)),
            image_folders: image_folders(env),
        },
        extensions: ProgramExtensions { compiled, found: finder.locators(), bundled },
    })
}

/// The folder `path`, with where it comes from (`origin`) and whether it exists.
fn program_folder(path: &Path, origin: &str) -> ProgramFolder {
    ProgramFolder { path: path.display().to_string(), origin: origin.to_string(), exists: path.is_dir() }
}

/// Where a folder of the search path comes from, as the page `program/paths` writes it.
fn origin_text(origin: FolderOrigin) -> &'static str {
    match origin {
        FolderOrigin::Option => "--extensions-dir",
        FolderOrigin::BesideDocument => "beside the document",
        FolderOrigin::CurrentFolder => "in the current folder",
        FolderOrigin::Flmpath => "FLMPATH",
        FolderOrigin::User => "the user's folder",
    }
}

/// The folders in which an image's file is looked for: the value in effect of the setting
/// `graphics-files.search_path` (else its default), with the rule by which the host reads
/// each folder ([`IMAGE_FOLDER_RULE`]). The host computes no list of absolute folders,
/// since each folder is relative to the file in which an image is written. `None` when no
/// package in use has the feature `graphics-files`, or when it is switched off.
fn image_folders(env: &Environment) -> Option<ImageFolders> {
    let settings = env.settings(flm_main::stdpkg::graphics_files::FEATURE)?;
    if settings.switched_off.is_some() {
        return None;
    }
    let key = settings.keys.iter().find(|key| key.name == "search_path")?;
    let value = key.in_effect.as_ref().map(|in_effect| &in_effect.value).or(key.default.as_ref())?;
    let folders = value.as_array()?.iter().filter_map(|folder| folder.as_str().map(str::to_string)).collect();
    Some(ImageFolders { folders, rule: String::from(IMAGE_FOLDER_RULE) })
}

/// The program's commands, with their options and positional arguments, as the command
/// line's declaration gives them ([`Cli::command`]), in its order: each option with its
/// spellings, the name of its value, its help and its default. The options and commands
/// hidden from the help, and those the parser adds of itself (`--help`, `help`), are left
/// out; the option `--json-diagnostics`, which every command accepts, is listed under each.
/// `flm dump`, whose arguments are not declared to the parser, is not listed.
fn commands() -> Vec<ProgramCommand> {
    let mut command = Cli::command();
    command.build();
    // The order of the help, which is the order of declaration: `Cli::command` rebuilds a
    // subcommand to add the templates to the help of `-t` (`mut_subcommand`), which moves it
    // to the end of the list; sorting by display order restores the order of declaration.
    let mut subcommands: Vec<&clap::Command> =
        command.get_subcommands().filter(|subcommand| !subcommand.is_hide_set() && subcommand.get_name() != "help").collect();
    subcommands.sort_by_key(|subcommand| subcommand.get_display_order());
    subcommands
        .into_iter()
        .map(|subcommand| ProgramCommand {
            name: subcommand.get_name().to_string(),
            help: subcommand.get_long_about().or(subcommand.get_about()).map(ToString::to_string).unwrap_or_default(),
            options: subcommand
                .get_arguments()
                .filter(|arg| !arg.is_hide_set() && !matches!(arg.get_id().as_str(), "help" | "version"))
                .map(|arg| {
                    let mut names: Vec<String> = Vec::new();
                    names.extend(arg.get_short().map(|short| format!("-{short}")));
                    names.extend(arg.get_long().map(|long| format!("--{long}")));
                    let takes_value = arg.get_num_args().is_some_and(|count| count.takes_values());
                    let value_name = arg
                        .get_value_names()
                        .and_then(|values| values.first())
                        .map(ToString::to_string)
                        .filter(|_| takes_value || arg.is_positional());
                    let default = arg.get_default_values().first().map(|value| value.to_string_lossy().into_owned());
                    ProgramOption {
                        names,
                        value_name,
                        help: arg.get_long_help().or(arg.get_help()).map(ToString::to_string).unwrap_or_default(),
                        default,
                    }
                })
                .collect(),
        })
        .collect()
}

/// The environment variables the program reads, each with what it reads it for.
fn environment_variables() -> Vec<EnvironmentVariable> {
    let variable = |name: &str, help: &str| EnvironmentVariable { name: name.to_string(), help: help.to_string() };
    vec![
        variable(
            crate::args::FLMPATH_VARIABLE,
            "A list of folders in which extensions are looked for, separated as in PATH, after the folders of \
             --extensions-dir (or the folder _flmextensions beside the document) and before ~/.flm/extensions.",
        ),
        variable(
            BROWSER_VARIABLE,
            "The program that opens the address of the preview (flm watch), in place of the default browser.",
        ),
        variable(
            MAGICK_VARIABLE,
            "The program that converts an image from one format into another (default: ImageMagick's magick).",
        ),
        variable(
            COLUMNS_VARIABLE,
            "The width of the terminal in columns, with which flm info writes its pages when standard output is a \
             terminal (default: the width the terminal reports); nothing else sets the width of the pages.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_pages_share_the_last_segment_or_the_longest_prefix() {
        let every: Vec<String> = [
            "/",
            "features/",
            "features/headings",
            "features/headings/macros",
            "features/headings/macros/section",
            "features/refs/macros/ref",
            "namespaces/",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(nearest(&every, "features/heading/macros/section"), ["features/headings/macros/section", "features/headings", "features/headings/macros"]);
        // One shared character is not near: the run must cover the first segment.
        assert_eq!(nearest(&every, "nothing/here"), Vec::<String>::new());
        assert_eq!(nearest(&every, "feat"), ["features/", "features/headings", "features/headings/macros", "features/headings/macros/section", "features/refs/macros/ref"]);
        assert_eq!(nearest(&every, "zzz"), Vec::<String>::new());
        let many: Vec<String> = (0..10).map(|n| format!("features/f{n}")).collect();
        assert_eq!(nearest(&many, "features/x").len(), NEAREST);
    }

    #[test]
    fn the_pages_are_rendered_with_the_width_of_the_terminal_alone() {
        let settings = text_settings();
        // The width of the terminal when standard output is one (a test run in a terminal),
        // else nothing.
        assert!(settings.len() <= 1, "{settings:?}");
        for setting in &settings {
            assert!(setting.assignment.starts_with("text.width="), "{setting:?}");
            assert_eq!(setting.label, "the width of the terminal");
        }
        assert_eq!(settings.is_empty(), !std::io::stdout().is_terminal() || terminal_width().is_none());
    }

    #[test]
    fn every_command_of_the_parser_is_listed_with_its_options() {
        let commands = commands();
        let names: Vec<&str> = commands.iter().map(|command| command.name.as_str()).collect();
        assert_eq!(names, ["watch", "render", "check", "schema", "info"]);
        let info = commands.iter().find(|command| command.name == "info").unwrap();
        let recursive = info.options.iter().find(|option| option.names == ["-r", "--recursive"]).unwrap();
        assert_eq!(recursive.value_name, None);
        let profile = info.options.iter().find(|option| option.names == ["-p", "--profile"]).unwrap();
        assert_eq!(profile.value_name.as_deref(), Some("PROFILE"));
        let paths = info.options.iter().find(|option| option.names.is_empty()).unwrap();
        assert_eq!(paths.value_name.as_deref(), Some("PATH"));
        assert!(info.options.iter().any(|option| option.names == ["--json-diagnostics"]));
        assert!(info.options.iter().all(|option| option.names != ["-h", "--help"]));
        let schema = commands.iter().find(|command| command.name == "schema").unwrap();
        let of = schema.options.iter().find(|option| option.names == ["--of"]).unwrap();
        assert_eq!(of.default.as_deref(), Some("extension"));
        let watch = commands.iter().find(|command| command.name == "watch").unwrap();
        assert!(watch.options.iter().all(|option| option.names != ["-o", "--output"]), "hidden");
    }
}
