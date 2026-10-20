//! `\input`: a document assembled from several files through an answerer held
//! in memory, rendered on the three outputs; the conditions the parser
//! reports; the parsing settings of an included file.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::techy::source::LineIndexCache;
use flm_core::lang::{
    FlmOrigin, FlmSpan, FlmParsing, FlmSource, IncludedFile, IncludedFileProblem, InputError,
    InputHeading, InputResolver, ParseOptions,
};
use flm_core::model::assemble::assemble;
use flm_core::model::doc::{Doc, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::parse_fragment;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::userdefs::UserdefsConfig;

use common::pipeline;
use common::stdcore::{diagnostic_lines, identifiers, plan};

// --- an answerer held in memory -----------------------------------------------------------

/// Files held in memory, each a text and, optionally, parsing settings and
/// problems of its front matter. A reference is joined to the folder of the
/// including file, as a host on a disk would.
#[derive(Default, Clone)]
struct Memory {
    files: BTreeMap<String, IncludedFile>,
    /// For a file that writes parsing settings of its own, how its keys change
    /// the settings in force where it is included.
    keys: BTreeMap<String, fn(FlmParsing) -> FlmParsing>,
}

impl Memory {
    fn with(mut self, name: &str, text: &str) -> Memory {
        self.files.insert(
            name.to_string(),
            IncludedFile { name: name.to_string(), text: text.to_string(), first_line: 1, ..IncludedFile::default() },
        );
        self
    }

    /// A heading input: `text` opens with the heading `command` makes of
    /// `title`, labelled `label`, numbered or not; the title written on the
    /// second line of the file, its content from the fourth.
    fn with_heading(mut self, name: &str, text: &str, command: &str, title: &str, label: Option<&str>, numbered: bool) -> Memory {
        let heading = InputHeading {
            command: command.to_string(),
            title: title.to_string(),
            title_at: Some((2, 8)),
            label: label.map(String::from),
            numbered,
        };
        self.files.insert(
            name.to_string(),
            IncludedFile {
                name: name.to_string(),
                text: text.to_string(),
                first_line: 4,
                heading: Some(heading),
                ..IncludedFile::default()
            },
        );
        self
    }

    fn with_file(mut self, file: IncludedFile) -> Memory {
        self.files.insert(file.name.clone(), file);
        self
    }

    /// The file `name`, holding `text`, whose front matter writes parsing
    /// settings that `keys` puts on top of those in force where it is included.
    fn with_keys(mut self, name: &str, text: &str, keys: fn(FlmParsing) -> FlmParsing) -> Memory {
        self.keys.insert(name.to_string(), keys);
        self.with(name, text)
    }
}

impl InputResolver for Memory {
    fn resolve(&self, reference: &str, including: Option<&str>, parsing: &FlmParsing) -> Result<IncludedFile, InputError> {
        let folder = including.and_then(|name| Path::new(name).parent()).unwrap_or(Path::new(""));
        let name = folder.join(reference).to_string_lossy().into_owned();
        let mut file =
            self.files.get(&name).cloned().ok_or_else(|| InputError::new(format!("there is no file `{name}`")))?;
        if let Some(keys) = self.keys.get(&name) {
            file.parsing = Some(keys(parsing.clone()));
        }
        Ok(file)
    }
}

/// An answerer that answers every reference with a new file including the
/// next one: a chain that never ends.
struct Endless;

impl InputResolver for Endless {
    fn resolve(&self, reference: &str, _including: Option<&str>, _parsing: &FlmParsing) -> Result<IncludedFile, InputError> {
        Ok(IncludedFile {
            name: reference.to_string(),
            text: format!("\\input{{{reference}x}}"),
            first_line: 1,
            ..IncludedFile::default()
        })
    }
}

// --- the pipeline ------------------------------------------------------------------------

fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The document whose primary file `main.flm` holds `text`, its inclusions
/// answered by `resolver`.
fn doc(registry: &FlmRegistry, text: &str, resolver: Option<Arc<dyn InputResolver>>) -> Arc<Doc> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::new("main.flm")));
    let options = ParseOptions { input_resolver: resolver, ..ParseOptions::default() };
    let content = assemble(registry.lang(), source, FragmentLayout::default(), &options)
        .expect("the input is shallow enough to parse");
    Arc::new(Doc::with_options(content, OptionValues::new()))
}

fn run(registry: &Arc<FlmRegistry>, workflow: &str, text: &str, files: Memory) -> Run {
    let doc = doc(registry, text, Some(Arc::new(files)));
    let plan = plan(registry, workflow, &Overrides::none());
    pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential())
}

/// The line and the column, counted from one, where `span` starts.
fn line_col(span: &FlmSpan) -> (usize, usize) {
    LineIndexCache::new().line_col(span.source(), span.start()).expect("a position")
}

fn clean(run: &Run) {
    assert_eq!(identifiers(run), Vec::<String>::new(), "{}", diagnostic_lines(run));
}

fn text_of(text: &str, files: Memory) -> String {
    let run = run(&std(), text::WORKFLOW, text, files);
    clean(&run);
    common::lowering::text(&run)
}

fn html_of(text: &str, files: Memory) -> String {
    let run = run(&std(), html::WORKFLOW, text, files);
    clean(&run);
    common::html::fragment(&run)
}

fn latex_of(text: &str, files: Memory) -> String {
    let run = run(&std(), latex_recomposed::WORKFLOW, text, files);
    clean(&run);
    common::latex::output(&run)
}

// --- acceptance 1: one paragraph across the files ----------------------------------------

#[test]
fn a_paragraph_begun_in_one_file_and_ended_in_the_other_is_one_paragraph() {
    let files = || Memory::default().with("middle.flm", "the middle\nof it");
    let document = "Begin \\input{middle.flm} and end.\n\nNext.";
    assert_eq!(text_of(document, files()), "Begin the middle of it and end.\n\nNext.\n");
    let html = html_of(document, files());
    assert_eq!(html.matches("<p").count(), 2, "{html}");
    assert!(html.contains("Begin the middle of it and end."), "{html}");
    let latex = latex_of(document, files());
    assert!(latex.contains("Begin the middle\nof it and end."), "{latex}");
    assert!(!latex.contains("\\input"), "{latex}");
}

#[test]
fn the_command_works_inside_an_environment_and_inside_a_list_item() {
    let files = Memory::default().with("inner.flm", "inner \\emph{words}");
    let included = "\\begin{itemize}\n\\item A \\input{inner.flm} B\n\\end{itemize}";
    let in_place = "\\begin{itemize}\n\\item A inner \\emph{words} B\n\\end{itemize}";
    assert_eq!(text_of(included, files.clone()), text_of(in_place, Memory::default()));
    assert_eq!(html_of(included, files), html_of(in_place, Memory::default()));
}

// --- acceptance 2: one document ----------------------------------------------------------

#[test]
fn labels_and_numbers_run_through_the_files_in_document_order() {
    let files = Memory::default().with(
        "part.flm",
        "\\section{Two}\\label{sec:two}\nSee \\ref{sec:one}.\\footnote{Second.}",
    );
    let document = "\\section{One}\\label{sec:one}\nFirst.\\footnote{First.}\n\n\\input{part.flm}\n\n\
                    \\section{Three}\nBack to \\ref{sec:two}.\\footnote{Third.}";
    let out = text_of(document, files);
    assert!(out.contains("See One."), "{out}");
    assert!(out.contains("Back to Two."), "{out}");
    let marks: Vec<usize> =
        ["First.¹", "See One.²", "Back to Two.³", "¹ First.", "² Second.", "³ Third."].iter().map(|m| out.find(m).unwrap_or_else(|| panic!("{m} in {out}"))).collect();
    assert!(marks.windows(2).all(|pair| pair[0] < pair[1]), "{out}");
}

#[test]
fn equations_and_headings_are_numbered_through_a_content_input_in_document_order() {
    let files = Memory::default().with("part.flm", "\\section{Two}\n\\begin{equation}b\\end{equation}");
    let document = "\\section{One}\n\\begin{equation}a\\end{equation}\n\n\\input{part.flm}\n\n\
                    \\section{Three}\n\\begin{equation}c\\end{equation}";
    let run = run(&numbered(), text::WORKFLOW, document, files);
    clean(&run);
    let out = common::lowering::text(&run);
    // The including file's before the command, the included file's, then the
    // including file's after it.
    let marks: Vec<usize> =
        ["1. One", "a\\tag*{(1)}", "2. Two", "b\\tag*{(2)}", "3. Three", "c\\tag*{(3)}"]
            .iter()
            .map(|m| out.find(m).unwrap_or_else(|| panic!("{m} in {out}")))
            .collect();
    assert!(marks.windows(2).all(|pair| pair[0] < pair[1]), "{out}");
}

// --- acceptance 3: the parsing settings of an included file ------------------------------

#[test]
fn the_parsing_settings_of_an_included_file_hold_inside_it_and_not_after_it() {
    let dollars = FlmParsing { dollar_math: true, ..FlmParsing::default() };
    let files = Memory::default().with_file(IncludedFile {
        name: String::from("math.flm"),
        text: String::from("inside $x$"),
        first_line: 1,
        parsing: Some(dollars),
        problems: Vec::new(),
        heading: None,
    });
    let run = run(&std(), text::WORKFLOW, "\\input{math.flm} after $y$", files);
    // Each `$` after the command is reported, in the including file; none in
    // the included one.
    let places: Vec<(Option<String>, (usize, usize))> = run
        .diagnostics()
        .iter()
        .map(|d| (d.span().source().origin().canonical_path().map(String::from), line_col(d.span())))
        .collect();
    assert_eq!(identifiers(&run), ["flm.lang.dollar-math-disabled"; 2], "{}", diagnostic_lines(&run));
    let main = Some(String::from("main.flm"));
    assert_eq!(places, [(main.clone(), (1, 24)), (main, (1, 26))]);
}

#[test]
fn a_file_without_settings_of_its_own_hands_on_those_it_inherited() {
    // `a.flm` makes `;;` start a comment; `b.flm` writes nothing and so is read
    // under `a.flm`'s settings; `c.flm` adds `$` math on top of those. Were `c.flm`
    // read under the document's settings with its own key, `;;` would start no
    // comment there and the unknown command after it would be reported.
    let files = Memory::default()
        .with_keys("a.flm", "\\input{b.flm}", |parsing| FlmParsing { comment_prefix: String::from(";;"), ..parsing })
        .with("b.flm", "\\input{c.flm} ;; \\nosuchmacro in b")
        .with_keys("c.flm", "$x$ ;; \\nosuchmacro in c", |parsing| FlmParsing { dollar_math: true, ..parsing });
    let run = run(&std(), text::WORKFLOW, "\\input{a.flm}", files);
    clean(&run);
    assert_eq!(common::lowering::text(&run).trim(), "\\(x\\)");
}

/// The standard registry, its documents read under `parsing`.
fn std_reading(parsing: FlmParsing) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let inputs = RegistryInputs { parsing, ..RegistryInputs::new() };
    builder.build(&inputs).unwrap_or_else(|error| panic!("the registry build failed: {}", error.render()))
}

fn comments_start(prefix: &str) -> FlmParsing {
    FlmParsing { comment_prefix: prefix.to_string(), ..FlmParsing::default() }
}

#[test]
fn a_lone_percent_in_an_included_file_is_reported_under_its_comment_settings() {
    // In the primary file a lone `%` starts a comment; in the included file `;;`
    // does, so a lone `%` there is ambiguous, and the report names `;;`.
    let files = Memory::default()
        .with_keys("part.flm", "10% more", |parsing| FlmParsing { comment_prefix: String::from(";;"), ..parsing });
    let run = run(&std_reading(comments_start("%")), text::WORKFLOW, "\\input{part.flm} % a comment", files);
    assert_eq!(identifiers(&run), ["flm.lang.percent-forbidden"], "{}", diagnostic_lines(&run));
    let lines = diagnostic_lines(&run);
    assert!(lines.contains("‘;;’ to start a comment"), "{lines}");
    let diagnostic = run.diagnostics().iter().next().expect("one");
    assert_eq!(diagnostic.span().source().origin().canonical_path(), Some("part.flm"));
}

#[test]
fn the_report_of_a_lone_percent_names_the_comment_start_of_the_file_it_stands_in() {
    let files = Memory::default()
        .with_keys("part.flm", "10% more", |parsing| FlmParsing { comment_prefix: String::from("!!"), ..parsing });
    let run = run(&std_reading(comments_start(";;")), text::WORKFLOW, "\\input{part.flm} ;; a comment", files);
    assert_eq!(identifiers(&run), ["flm.lang.percent-forbidden"], "{}", diagnostic_lines(&run));
    let lines = diagnostic_lines(&run);
    assert!(lines.contains("‘!!’ to start a comment") && !lines.contains(";;"), "{lines}");
}

// --- acceptance 6: lists -------------------------------------------------------------------

#[test]
fn a_list_in_a_content_input_inside_a_list_item_is_nested() {
    let inner = "\\begin{enumerate}\n\\item Inner\n\\end{enumerate}";
    let files = Memory::default().with("list.flm", inner);
    let included = "\\begin{enumerate}\n\\item Outer \\input{list.flm}\n\\end{enumerate}";
    let in_place = format!("\\begin{{enumerate}}\n\\item Outer {inner}\n\\end{{enumerate}}");
    let out = text_of(included, files.clone());
    assert_eq!(out, text_of(&in_place, Memory::default()));
    // The inner list is marked as a nested list is, not as an outermost one.
    assert!(out.contains("(i) Inner"), "{out}");
    assert_eq!(html_of(included, files), html_of(&in_place, Memory::default()));
}

// --- acceptance 7 and 8: where a problem is reported ---------------------------------------

#[test]
fn a_problem_in_an_included_file_is_reported_at_its_line_with_the_chain() {
    let files = Memory::default()
        .with("docs/a.flm", "\\input{b.flm}")
        .with_file(IncludedFile {
            name: String::from("docs/b.flm"),
            text: String::from("fine\nthen \\nosuchmacro here"),
            first_line: 4,
            ..IncludedFile::default()
        });
    let run = run(&std(), text::WORKFLOW, "Main \\input{docs/a.flm}", files);
    assert_eq!(identifiers(&run), ["core.specs.unresolvable-command"], "{}", diagnostic_lines(&run));
    let diagnostic = run.diagnostics().iter().next().expect("one");
    let span = diagnostic.span();
    assert_eq!(span.source().origin().canonical_path(), Some("docs/b.flm"));
    assert_eq!(line_col(span), (5, 6));
    let chain: Vec<Option<String>> = span
        .source()
        .including_sources()
        .map(|source| source.origin().canonical_path().map(String::from))
        .collect();
    assert_eq!(
        chain,
        [Some(String::from("docs/b.flm")), Some(String::from("docs/a.flm")), Some(String::from("main.flm"))]
    );
}

#[test]
fn a_problem_of_an_included_front_matter_is_reported_at_its_place() {
    let files = Memory::default().with_file(IncludedFile {
        name: String::from("part.flm"),
        text: String::from("words"),
        first_line: 4,
        problems: vec![IncludedFileProblem { line: 2, column: 3, message: String::from("bad key") }],
        ..IncludedFile::default()
    });
    let run = run(&std(), text::WORKFLOW, "\\input{part.flm}", files);
    assert_eq!(identifiers(&run), ["flm.input.front-matter"], "{}", diagnostic_lines(&run));
    let diagnostic = run.diagnostics().iter().next().expect("one");
    assert_eq!(line_col(diagnostic.span()), (2, 3));
    assert_eq!(diagnostic.span().source().origin().canonical_path(), Some("part.flm"));
    assert!(common::lowering::text(&run).contains("words"));
}

// --- acceptance 9 and 12: the conditions, and no input that loops --------------------------

fn conditions(text: &str, resolver: Option<Arc<dyn InputResolver>>) -> (Vec<String>, String) {
    let registry = std();
    let doc = doc(&registry, text, resolver);
    let plan = plan(&registry, text::WORKFLOW, &Overrides::none());
    let run = pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    (identifiers(&run), common::lowering::text(&run))
}

#[test]
fn a_reference_that_is_not_plain_characters_is_reported() {
    let (found, out) = conditions("A \\input{{x}} B", Some(Arc::new(Memory::default())));
    assert_eq!(found, ["flm.input.reference-not-plain"]);
    assert!(out.contains("[dropped"), "{out}");
}

#[test]
fn a_reference_that_names_no_file_is_reported_and_marked() {
    let (found, out) = conditions("A \\input{missing.flm} B", Some(Arc::new(Memory::default())));
    assert_eq!(found, ["flm.input.unreadable"]);
    assert!(out.contains("missing.flm"), "{out}");
}

#[test]
fn a_host_that_supplies_no_answerer_is_reported() {
    let (found, _) = conditions("A \\input{x.flm} B", None);
    assert_eq!(found, ["flm.input.unavailable"]);
}

#[test]
fn a_file_that_includes_itself_is_reported() {
    let files = Memory::default().with("a.flm", "a \\input{b.flm}").with("b.flm", "b \\input{a.flm}");
    let (found, out) = conditions("\\input{a.flm}", Some(Arc::new(files)));
    assert_eq!(found, ["flm.input.includes-itself"]);
    assert!(out.starts_with("a b"), "{out}");
    let (found, _) = conditions("\\input{main.flm}", Some(Arc::new(Memory::default().with("main.flm", "again"))));
    assert_eq!(found, ["flm.input.includes-itself"]);
}

#[test]
fn a_chain_deeper_than_the_limit_is_reported_once() {
    let (found, _) = conditions("\\input{f}", Some(Arc::new(Endless)));
    assert_eq!(found, ["flm.input.too-deep"]);
}

#[test]
fn an_empty_file_brings_in_nothing() {
    let out = text_of("A \\input{empty.flm}B", Memory::default().with("empty.flm", ""));
    assert_eq!(out, "A B\n");
}

// --- acceptance 4 and 5: a heading input -----------------------------------------------------

fn numbered() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)))
}

/// The chapters of a book: the primary file holds its own section, then
/// includes two heading inputs, the second unnumbered.
fn book() -> Memory {
    Memory::default()
        .with_heading("two.flm", "Second text.", "section", "The \\emph{Art}", Some("sec:art"), true)
        .with_heading("three.flm", "Third text.", "section", "Coda", None, false)
}

const BOOK: &str = "\\section{One}\nIntro, see \\ref{sec:art}.\n\n\\input{two.flm}\n\n\\input{three.flm}";

#[test]
fn a_heading_input_shows_its_heading_numbered_in_sequence_and_referenced_by_its_label() {
    let done = run(&numbered(), text::WORKFLOW, BOOK, book());
    let run_ = &done;
    clean(run_);
    let out = common::lowering::text(run_);
    let order: Vec<usize> = ["1. One", "Intro, see §", "2. The Art", "Second text.", "Coda", "Third text."]
        .iter()
        .map(|m| out.find(m).unwrap_or_else(|| panic!("{m} in {out}")))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{out}");
    assert!(!out.contains("3. Coda"), "{out}");

    let done = run(&numbered(), html::WORKFLOW, BOOK, book());
    clean(&done);
    let page = common::html::page(&done);
    let order: Vec<usize> = ["One</h1>", "Intro, see", "id=\"sec-art\"", "<em class=\"flm-pres-span\" data-flm-style=\"emph\">Art</em></h1>", "Second text.", "Coda</h1>", "Third text."]
        .iter()
        .map(|m| page.find(m).unwrap_or_else(|| panic!("{m} in {page}")))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{page}");

    let done = run(&numbered(), latex_recomposed::WORKFLOW, BOOK, book());
    clean(&done);
    let latex = common::latex::output(&done);
    let order: Vec<usize> = ["\\section{One}", "Intro, see", "\\section{The \\emph{Art}}", "Second text.", "\\section*{Coda}", "Third text."]
        .iter()
        .map(|m| latex.find(m).unwrap_or_else(|| panic!("{m} in {latex}")))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{latex}");
    assert!(!latex.contains("\\input"), "{latex}");
}

/// Document order runs through a placed tree at its place: a heading of the
/// including file written after the command is numbered after the heading
/// input's, on the three outputs.
#[test]
fn a_heading_after_a_heading_input_is_numbered_after_it() {
    let files = || Memory::default().with_heading("two.flm", "Text.", "section", "Two", Some("sec:two"), true);
    let document = "\\section{One}\\label{sec:one}\n\n\\input{two.flm}\n\n\\section{Three}\\label{sec:three}\nSee \\ref{sec:two} and \\ref{sec:three}.";
    let done = run(&numbered(), text::WORKFLOW, document, files());
    clean(&done);
    let out = common::lowering::text(&done);
    // The references show the numbers the headings were given.
    in_order(&out, &["1. One", "2. Two", "Text.", "3. Three", "See §\u{202f}2 and §\u{202f}3."]);

    let done = run(&numbered(), html::WORKFLOW, document, files());
    clean(&done);
    let page = common::html::page(&done);
    in_order(&page, &[">1</span>. One</h1>", ">2</span>. Two</h1>", "Text.", ">3</span>. Three</h1>", "href=\"#sec-two\" data-flm-kind=\"ref\">§&#x202F;2</a>", "href=\"#sec-three\" data-flm-kind=\"ref\">§&#x202F;3</a>"]);

    let done = run(&numbered(), latex_recomposed::WORKFLOW, document, files());
    clean(&done);
    let latex = common::latex::output(&done);
    in_order(&latex, &["\\section{One}", "\\section{Two}", "Text.", "\\section{Three}"]);
}

/// Cited works are numbered through a heading input at its place: a key first
/// cited in the input is numbered where the command stands, before a key first
/// cited in the including file after the command.
#[test]
fn a_key_first_cited_in_a_heading_input_is_numbered_where_the_command_stands() {
    let files = || Memory::default().with_heading("two.flm", "Input \\cite{b}.", "section", "Two", None, true);
    let document = "\\section{One}\nOne \\cite{a}.\n\n\\input{two.flm}\n\n\\section{Three}\nThree \\cite{c}\\cite{b}.";
    let registry = common::citations::registry(StdcoreConfig::new());
    let done = run(&registry, text::WORKFLOW, document, files());
    clean(&done);
    let out = common::lowering::text(&done);
    in_order(&out, &["One\u{a0}[1].", "Input\u{a0}[2].", "Three\u{a0}[2,3].", "[1] a", "[2] b", "[3] c"]);
}

/// Asserts that each of `marks` is found in `out`, in that order.
fn in_order(out: &str, marks: &[&str]) {
    let found: Vec<usize> = marks.iter().map(|m| out.find(m).unwrap_or_else(|| panic!("{m} in {out}"))).collect();
    assert!(found.windows(2).all(|pair| pair[0] < pair[1]), "{marks:?} in {out}");
}

/// Equations, footnotes and theorems are numbered through a heading input at
/// its place: the including file's before the command, the input's, the
/// including file's after it.
#[test]
fn equations_footnotes_and_theorems_are_numbered_through_a_heading_input_at_its_place() {
    let files = Memory::default().with_heading(
        "two.flm",
        "B.\\footnote{Note b.}\n\\begin{equation}b\\end{equation}\n\\begin{theorem}Tb.\\end{theorem}",
        "section",
        "Two",
        None,
        true,
    );
    let document = "\\section{One}\nA.\\footnote{Note a.}\n\\begin{equation}a\\end{equation}\n\
                    \\begin{theorem}Ta.\\end{theorem}\n\n\\input{two.flm}\n\n\\section{Three}\n\
                    C.\\footnote{Note c.}\n\\begin{equation}c\\end{equation}\n\\begin{theorem}Tc.\\end{theorem}";
    let done = run(&numbered(), text::WORKFLOW, document, files);
    // Each theorem lacks a proof, which is reported in document order too.
    assert_eq!(identifiers(&done), ["flm.theorems.missing-proof"; 3], "{}", diagnostic_lines(&done));
    let files: Vec<Option<String>> = done
        .diagnostics()
        .iter()
        .map(|d| d.span().source().origin().canonical_path().map(String::from))
        .collect();
    assert_eq!(files, [Some("main.flm".into()), Some("two.flm".into()), Some("main.flm".into())]);
    let out = common::lowering::text(&done);
    in_order(
        &out,
        &[
            "1. One", "A.¹", "a\\tag*{(1)}", "1. Ta.",
            "2. Two", "B.²", "b\\tag*{(2)}", "2. Tb.",
            "3. Three", "C.³", "c\\tag*{(3)}", "3. Tc.",
            "¹ Note a.", "² Note b.", "³ Note c.",
        ],
    );
}

/// The run's order is computed once, before any stage: the parallel and the
/// sequential run of a document with placed trees give the same output.
#[test]
fn the_parallel_and_the_sequential_run_follow_the_same_order() {
    let files = Memory::default()
        .with_heading("a.flm", "A.\\footnote{a}\n\n\\input{b.flm}\n\nA again.\\footnote{a2}", "section", "A", None, true)
        .with_heading("b.flm", "B.\\footnote{b}", "subsection", "B", None, true);
    let registry = numbered();
    let doc = doc(&registry, "\\section{M}\n\n\\input{a.flm}\n\n\\section{N}\nN.\\footnote{n}", Some(Arc::new(files)));
    let plan = plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential = pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    clean(&sequential);
    assert_eq!(sequential.tree_order(), parallel.tree_order());
    assert_eq!(common::lowering::text(&sequential), common::lowering::text(&parallel));
    in_order(&common::lowering::text(&sequential), &["A.¹", "B.²", "A again.³", "N.⁴"]);
}

/// A label written twice, once in a heading input and once after its command
/// in the including file: the input's point comes first in document order and
/// keeps the label; the other is reported where it is written.
#[test]
fn a_label_written_in_a_heading_input_and_again_after_it_is_the_input_s() {
    let files = Memory::default().with_heading("two.flm", "Text.", "section", "Two", Some("sec:x"), true);
    let document = "\\section{One}\n\n\\input{two.flm}\n\n\\section{Three}\\label{sec:x}\nSee \\ref{sec:x}.";
    let done = run(&numbered(), text::WORKFLOW, document, files);
    let out = common::lowering::text(&done);
    assert!(out.contains("See §\u{202f}2."), "{out}");
    let reports: Vec<_> = done.diagnostics().iter().collect();
    assert_eq!(reports.len(), 1, "{}", diagnostic_lines(&done));
    let span = reports[0].span();
    assert_eq!(span.source().origin().canonical_path(), Some("main.flm"), "{}", diagnostic_lines(&done));
    assert_eq!(line_col(span).0, 5, "{}", diagnostic_lines(&done));
}

/// A definition that writes its body twice, the body holding a heading input:
/// the file stands at one place, the first, and the second copy is reported
/// and marked.
#[test]
fn a_heading_input_copied_by_a_definition_is_placed_once_and_the_copy_reported() {
    let definitions = vec![Definition::environment("twice", Vec::new()).text("#{body}\n\n#{body}")];
    let registry = common::stdcore::registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)));
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "Part", None, true);
    let done = run(&registry, text::WORKFLOW, "\\begin{twice}\n\\input{part.flm}\n\\end{twice}", files);
    assert_eq!(identifiers(&done), ["flm.run.placed-twice"], "{}", diagnostic_lines(&done));
    let out = common::lowering::text(&done);
    assert_eq!(out.matches("Text.").count(), 1, "{out}");
    assert!(out.contains("[dropped"), "{out}");
    in_order(&out, &["Text.", "[dropped"]);
    // The check reports it too.
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "Part", None, true);
    let checked = run(&registry, stdcore::CHECK_WORKFLOW, "\\begin{twice}\n\\input{part.flm}\n\\end{twice}", files);
    assert_eq!(identifiers(&checked), ["flm.run.placed-twice"], "{}", diagnostic_lines(&checked));
}

/// A definition whose template does not use its body, around a heading input:
/// the file goes with the body, as any text there does — nothing of it is
/// written, and nothing is reported.
#[test]
fn a_heading_input_in_a_body_a_definition_drops_is_dropped_with_it() {
    let definitions = vec![Definition::environment("drop", Vec::new()).text("Kept.")];
    let registry = common::stdcore::registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)));
    let files = || Memory::default().with_heading("part.flm", "Text.\\footnote{Note.}", "section", "Part", None, true);
    let document = "\\section{One}\nOne.\n\n\\begin{drop}\n\\input{part.flm}\n\\end{drop}\n\n\\section{Two}\nTwo.";
    let done = run(&registry, text::WORKFLOW, document, files());
    clean(&done);
    let out = common::lowering::text(&done);
    for absent in ["Text.", "Part", "Note."] {
        assert!(!out.contains(absent), "{out}");
    }
    in_order(&out, &["One.", "Kept.", "Two."]);
    assert_eq!(done.tree_order().len(), 1, "{:?}", done.tree_order());
    // The check reports nothing either.
    let checked = run(&registry, stdcore::CHECK_WORKFLOW, document, files());
    clean(&checked);
}

#[test]
fn the_document_holds_one_tree_per_heading_input_placed_at_its_node_and_walked_in_order() {
    let files = Memory::default()
        .with_heading("a.flm", "Text of A.\n\n\\input{b.flm}\n\nA again.", "section", "A", None, true)
        .with_heading("b.flm", "Text of B \\input{c.flm}.", "subsection", "B", None, true)
        .with("c.flm", "and of C");
    let registry = std();
    let doc = doc(&registry, "Main.\n\n\\input{a.flm}\n\nMain again.", Some(Arc::new(files)));
    let content = doc.content();
    assert_eq!(content.len(), 3);
    let trees: Vec<_> = content.walk_trees().collect();
    assert_eq!(trees.len(), 3);
    let names: Vec<Option<String>> = trees
        .iter()
        .map(|id| content.fragment(*id).and_then(|f| f.source().origin().canonical_path().map(String::from)))
        .collect();
    assert_eq!(names, [Some("main.flm".into()), Some("a.flm".into()), Some("b.flm".into())]);
    // Each placed tree is placed at a node of the tree including it.
    assert_eq!(content.placed_at(trees[1]).map(|site| site.tree), Some(trees[0]));
    assert_eq!(content.placed_at(trees[2]).map(|site| site.tree), Some(trees[1]));
    // The walk enters each placed tree at its node: "Text of B and of C" comes
    // between "Text of A." and "A again.".
    let words: String = content
        .walk_nodes()
        .filter(|(_, node)| node.is_chars())
        .map(|(_, node)| node.span().content().to_string())
        .collect();
    let order: Vec<usize> = ["Main.", "Text of A.", "Text of B", "and of C", "A again.", "Main again."]
        .iter()
        .map(|m| words.find(m).unwrap_or_else(|| panic!("{m} in {words}")))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{words}");
    let out = text_of("Main.\n\n\\input{a.flm}\n\nMain again.", Memory::default()
        .with_heading("a.flm", "Text of A.\n\n\\input{b.flm}\n\nA again.", "section", "A", None, true)
        .with_heading("b.flm", "Text of B \\input{c.flm}.", "subsection", "B", None, true)
        .with("c.flm", "and of C"));
    assert!(out.contains("Text of B and of C."), "{out}");

    // The numbering follows the walk across the three files.
    let files = Memory::default()
        .with_heading("a.flm", "\\subsection{A1}\n\n\\input{b.flm}\n\n\\subsection{A3}", "section", "A", None, true)
        .with_heading("b.flm", "Text of B \\input{c.flm}.", "subsection", "B", None, true)
        .with("c.flm", "and of C\\footnote{In C.}");
    let document = "\\section{M}\nM.\\footnote{In M.}\n\n\\input{a.flm}\n\n\\section{N}\nN.\\footnote{In N.}";
    let done = run(&numbered(), text::WORKFLOW, document, files);
    clean(&done);
    let out = common::lowering::text(&done);
    in_order(
        &out,
        &["1. M", "M.¹", "2. A", "2.1. A1", "2.2. B", "Text of B and of C².", "2.3. A3", "3. N", "N.³", "¹ In M.", "² In C.", "³ In N."],
    );
}

// --- acceptance 2: a user definition used in an included file ------------------------------

#[test]
fn a_user_definition_used_in_an_included_file_is_expanded() {
    let definitions = vec![Definition::macro_("hello", vec![ArgDef::mandatory("name")]).text("Hello, #{name}!")];
    let registry = common::stdcore::registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)));
    let files = Memory::default()
        .with("part.flm", "\\hello{you}")
        .with_heading("chapter.flm", "\\hello{there}", "section", "C", None, true);
    let run = run(&registry, text::WORKFLOW, "\\input{part.flm}\n\n\\input{chapter.flm}", files);
    clean(&run);
    let out = common::lowering::text(&run);
    assert!(out.contains("Hello, you!") && out.contains("Hello, there!"), "{out}");
}

// --- acceptance 6: a list in a heading input -------------------------------------------------

#[test]
fn a_list_in_a_heading_input_inside_a_list_item_is_an_outermost_list() {
    let inner = "\\begin{enumerate}\n\\item Inner\n\\end{enumerate}";
    let files = Memory::default().with_heading("list.flm", inner, "paragraph", "Part", None, false);
    let run = run(&std(), text::WORKFLOW, "\\begin{enumerate}\n\\item Outer\n\n\\input{list.flm}\n\\end{enumerate}", files);
    clean(&run);
    let out = common::lowering::text(&run);
    // The inner list is marked as an outermost list is, not as a nested one.
    assert!(out.contains("1. Inner"), "{out}");
    assert!(!out.contains("(i) Inner"), "{out}");
}

// --- acceptance 8: a problem in a title ----------------------------------------------------

#[test]
fn a_problem_in_the_title_of_a_heading_input_is_reported_in_its_front_matter() {
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "A \\nosuchmacro title", None, true);
    let run = run(&std(), text::WORKFLOW, "Main.\n\n\\input{part.flm}", files);
    assert_eq!(identifiers(&run), ["core.specs.unresolvable-command"], "{}", diagnostic_lines(&run));
    let span = run.diagnostics().iter().next().expect("one").span().clone();
    assert_eq!(span.source().origin().canonical_path(), Some("part.flm"));
    // The title is written at line 2, column 8: the command at column 10.
    assert_eq!(line_col(&span), (2, 10));
    let chain: Vec<Option<String>> =
        span.source().including_sources().map(|source| source.origin().canonical_path().map(String::from)).collect();
    assert_eq!(chain.last(), Some(&Some(String::from("main.flm"))));
}

// --- acceptance 9 and 12: the conditions of a heading input ----------------------------------

#[test]
fn a_heading_command_that_is_no_command_name_is_reported_and_nothing_brought_in() {
    for command in ["", "sec{tion", "section*"] {
        let files = Memory::default().with_heading("part.flm", "Text.", command, "T", None, true);
        let (found, out) = conditions("\\input{part.flm}", Some(Arc::new(files)));
        assert_eq!(found, ["flm.input.heading-command"], "{command}");
        assert!(!out.contains("Text."), "{out}");
    }
}

#[test]
fn a_heading_label_holding_a_brace_is_reported_and_the_heading_kept() {
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "Title", Some("sec:a}b"), true);
    let (found, out) = conditions("\\input{part.flm}", Some(Arc::new(files)));
    assert_eq!(found, ["flm.input.heading-label"]);
    assert!(out.contains("Title") && out.contains("Text."), "{out}");
}

#[test]
fn a_heading_command_the_language_lacks_is_reported() {
    let files = Memory::default().with_heading("part.flm", "Text.", "nosuchheading", "T", None, true);
    let (found, out) = conditions("\\input{part.flm}", Some(Arc::new(files)));
    assert_eq!(found, ["core.specs.unresolvable-command"]);
    assert!(out.contains("Text."), "{out}");
}

#[test]
fn a_heading_input_where_no_block_may_stand_is_reported() {
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "T", None, true);
    let (found, _) = conditions("Some \\emph{words \\input{part.flm}}.", Some(Arc::new(files)));
    assert_eq!(found, ["flm.blocks.block-in-inline"]);
}

#[test]
fn one_file_placed_as_a_heading_input_twice_is_reported_and_marked() {
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "T", None, true);
    let (found, out) = conditions("\\input{part.flm}\n\n\\input{part.flm}", Some(Arc::new(files)));
    assert_eq!(found, ["flm.input.placed-twice"]);
    assert_eq!(out.matches("Text.").count(), 1, "{out}");
    assert!(out.contains("[dropped"), "{out}");
}

#[test]
fn a_heading_input_outside_a_whole_document_is_reported() {
    let registry = std();
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "T", None, true);
    let source = Arc::new(FlmSource::new("\\input{part.flm}").with_origin(FlmOrigin::new("main.flm")));
    let options = ParseOptions { input_resolver: Some(Arc::new(files)), ..ParseOptions::default() };
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &options).expect("a tree");
    let found: Vec<String> = fragment.diags().iter().map(|d| d.identifier().to_string()).collect();
    assert_eq!(found, ["flm.input.heading-outside-document"]);
}

#[test]
fn input_in_the_template_of_a_user_definition_is_reported() {
    let definitions = vec![Definition::macro_("chapterx", Vec::new()).text("\\input{part.flm}")];
    let registry = common::stdcore::registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)));
    let doc = doc(&registry, "A \\chapterx{} B", Some(Arc::new(Memory::default().with("part.flm", "x"))));
    let plan = plan(&registry, text::WORKFLOW, &Overrides::none());
    let run = pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    // The template is parsed when the registry is built, where the command is
    // refused; the definition is refused with it, and each use says so.
    let found = identifiers(&run);
    assert!(found.iter().any(|id| id == "flm.userdefs.bad-definition"), "{}", diagnostic_lines(&run));
    let built: Vec<String> = registry.build_diagnostics().iter().map(|d| d.identifier().to_string()).collect();
    assert!(built.iter().any(|id| id == "flm.input.in-definition"), "{built:?}");
}

#[test]
fn no_heading_input_loops_or_panics() {
    // A heading input whose title includes the file itself.
    let files = Memory::default().with_heading("self.flm", "Text.", "section", "\\input{self.flm}", None, true);
    let (found, _) = conditions("\\input{self.flm}", Some(Arc::new(files)));
    assert!(found.contains(&String::from("flm.input.includes-itself")), "{found:?}");
    // A heading input named inside a heading's title.
    let files = Memory::default().with_heading("part.flm", "Text.", "section", "T", None, true);
    let (found, _) = conditions("\\section{A \\input{part.flm}}", Some(Arc::new(files)));
    assert!(!found.is_empty());
    // A heading input that is a front matter alone: its heading and nothing else.
    let files = Memory::default().with_heading("bare.flm", "", "section", "Bare", None, true);
    let out = text_of("\\input{bare.flm}", files);
    assert!(out.contains("Bare"), "{out}");
    // Two heading inputs including each other.
    let files = Memory::default()
        .with_heading("a.flm", "\\input{b.flm}", "section", "A", None, true)
        .with_heading("b.flm", "\\input{a.flm}", "section", "B", None, true);
    let (found, _) = conditions("\\input{a.flm}", Some(Arc::new(files)));
    assert_eq!(found, ["flm.input.includes-itself"]);
}
