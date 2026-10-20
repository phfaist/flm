//! Citation entries on the host (cargo feature `citations`): the standard provider of the
//! feature `citations-standard` setting each record in a citation style through the
//! formatter `hayagriva` — the style the program ships, a style of the formatter's archive,
//! a style file of the author's — and appending the links to the DOI, the arXiv entry and
//! the record's address.
//!
//! The records come from a bibliography file in memory: no test opens a connection. The two
//! records of `data/probe-records.json` are those of an article of Physical Review A and of
//! an arXiv preprint as the fetching crate gives them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::techy::error::Severity;
use flm_main::files::{Files, MemoryFiles};
use flm_main::output::{MemorySink, Sink};
use flm_main::session::{Built, Input, Request, build, prepare};

/// The two records of the probe: `calderbank96`, an article of Physical Review A with a DOI
/// and an arXiv identifier, and `li22`, a preprint with a DOI and an address.
const PROBE: &str = include_str!("data/probe-records.json");

/// The document citing `keys`, each in a `\cite` of its own, its bibliography the file
/// `refs.json`, with `settings` under the settings of the feature `citations-standard`
/// (indented by six spaces).
fn paper(settings: &str, keys: &[&str]) -> String {
    let cites: String = keys.iter().map(|key| format!("\\cite{{{key}}} ")).collect();
    format!(
        "---\nflm:\n  features:\n    citations-standard:\n      bibliography: [refs.json]\n{settings}---\nSee {cites}.\n"
    )
}

/// The build of `doc/paper.flm` in `files` to the output `format`, and the output's text.
fn rendered_files(files: MemoryFiles, format: &str) -> (Built, String) {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.profile = Some(format.to_string());
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    request.continue_with_errors = true;
    let prepared = prepare(request, Arc::new(files) as Arc<dyn Files>, &[]).unwrap_or_else(|error| panic!("{error:?}"));
    let built = build(&prepared).unwrap_or_else(|error| panic!("{error:?}"));
    let entry = sink.files().into_values().find(|file| file.entry).expect("an output");
    (built, String::from_utf8(entry.bytes).unwrap())
}

/// The build of the document citing `keys` under `settings`, its bibliography `records`,
/// to the web page, and the content of each entry of its list of references, in order.
fn entries(settings: &str, keys: &[&str], records: &str) -> (Built, Vec<String>) {
    let files = MemoryFiles::new().with("doc/paper.flm", &paper(settings, keys)).with("doc/refs.json", records);
    let (built, html) = rendered_files(files, "html");
    (built, entries_of(&html))
}

/// The content of each entry of the list of references of the web page `html`: what stands
/// between the paragraph's tags in each item `citation-N`.
fn entries_of(html: &str) -> Vec<String> {
    html.split("<li id=\"citation-")
        .skip(1)
        .map(|item| {
            let start = item.find("<p class=\"flm-pres-par\">").expect("a paragraph") + "<p class=\"flm-pres-par\">".len();
            let end = item[start..].find("</p>").expect("the paragraph's end");
            item[start..start + end].to_string()
        })
        .collect()
}

/// The text of each Error of `built`.
fn errors(built: &Built) -> Vec<String> {
    built.diagnostics.iter().filter(|diagnostic| diagnostic.severity() == Severity::Error).map(ToString::to_string).collect()
}

/// A bold run of the page, as the web page writes `\textbf`.
fn bold(text: &str) -> String {
    format!("<strong class=\"flm-pres-span\" data-flm-style=\"bold\">{text}</strong>")
}

/// A link of the page showing `text`.
fn link(address: &str, text: &str) -> String {
    format!("<a class=\"flm-pres-link\" href=\"{address}\" data-flm-kind=\"url\">{text}</a>")
}

// --- acceptance 1: the probe's entries in the shipped style, special characters, links -----

#[test]
fn the_probe_s_records_are_set_in_the_shipped_style_with_their_links() {
    let (built, entries) = entries("", &["bib:calderbank96", "bib:li22"], PROBE);
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    // The authors, the journal, the volume in bold, the first page, the year in brackets;
    // the style writes no title for an article or a preprint.
    let doi = link("https://doi.org/10.1103/physreva.54.1098", "DOI");
    let arxiv = link("https://arxiv.org/abs/quant-ph/9512032", "quant-ph/9512032");
    assert_eq!(
        entries[0],
        format!("A. R. Calderbank and P. W. Shor, Physical Review A {}, 1098 (1996). {doi} {arxiv}", bold("54"))
    );
    // A DOI and an address: the link to the DOI alone.
    let doi = link("https://doi.org/10.48550/arXiv.2208.06306", "DOI");
    assert_eq!(entries[1], format!("L. Li, K. Bu, D. E. Koh, A. Jaffe, and S. Lloyd, (2022). {doi}"));
}

#[test]
fn a_formula_in_a_record_is_a_formula_and_characters_special_to_flm_are_shown() {
    let records = r#"[
        {"id": "book", "type": "book", "title": "On $\\alpha$ and prices", "issued": {"date-parts": [[2020]]}},
        {"id": "odd", "type": "article-journal", "container-title": "R&D: 50% of {X} #1 \\relax ~", "volume": "3"}
    ]"#;
    let (built, entries) = entries("", &["bib:book", "bib:odd"], records);
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    // The formula is the page's inline formula, whatever the document says of dollar signs.
    assert!(entries[0].contains("<span class=\"flm-pres-math--passthrough\" data-flm-display=\"inline\">\\(\\alpha\\)</span>"), "{}", entries[0]);
    assert!(entries[1].starts_with("R&amp;D: 50% of {X} #1 \\relax ~ "), "{}", entries[1]);
}

#[test]
fn a_formula_in_a_record_that_does_not_parse_is_an_error_at_its_citation() {
    let records = r#"[
        {"id": "bad", "type": "book", "title": "On $x^{2$ waves", "issued": {"date-parts": [[2020]]}},
        {"id": "good", "type": "book", "title": "Waves", "issued": {"date-parts": [[2021]]}}
    ]"#;
    let (built, entries) = entries("", &["bib:bad", "bib:good"], records);
    let errors = errors(&built);
    assert!(!errors.is_empty() && errors.iter().all(|error| error.contains("`bib:bad`")), "{errors:?}");
    // The document still renders, the other work's entry among its references.
    assert!(entries.iter().any(|entry| entry.contains("Waves</em> (2021)")), "{entries:?}");
}

#[test]
fn a_ready_entry_is_flm_or_literal_characters_and_takes_no_link() {
    let records = r#"[
        {"id": "flm", "DOI": "10.1000/x", "_ready_formatted": {"flm": "J. Preskill, \\emph{Lecture notes}"}},
        {"id": "html", "DOI": "10.1000/y", "_ready_formatted": {"html": "<i>Lecture</i> 50%"}}
    ]"#;
    let (built, entries) = entries("", &["bib:flm", "bib:html"], records);
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    assert_eq!(entries[0], "J. Preskill, <em class=\"flm-pres-span\" data-flm-style=\"emph\">Lecture notes</em>");
    assert_eq!(entries[1], "&lt;i&gt;Lecture&lt;/i&gt; 50%");
}

#[test]
fn each_link_is_appended_as_its_setting_says() {
    let records = r#"[
        {"id": "all", "type": "article-journal", "container-title": "J", "DOI": "10.1000/a", "arxivid": "2001.00001", "URL": "https://example.org/a"},
        {"id": "url", "type": "article-journal", "container-title": "K", "URL": "https://example.org/b"}
    ]"#;
    let keys = ["bib:all", "bib:url"];
    let (doi, arxiv) = (link("https://doi.org/10.1000/a", "DOI"), link("https://arxiv.org/abs/2001.00001", "2001.00001"));
    let (url_a, url_b) = (link("https://example.org/a", "URL"), link("https://example.org/b", "URL"));
    // The defaults: the DOI and the arXiv entry; the address only where neither is.
    let (_, default) = entries("", &keys, records);
    assert_eq!(default, [format!("J. {doi} {arxiv}"), format!("K. {url_b}")]);
    // No DOI, no arXiv entry: the address is still not linked where the record has either.
    let (_, none) = entries("      doi_link: false\n      arxiv_link: false\n", &keys, records);
    assert_eq!(none, [String::from("J."), format!("K. {url_b}")]);
    let (_, always) = entries("      url_link: always\n", &keys, records);
    assert_eq!(always, [format!("J. {doi} {arxiv} {url_a}"), format!("K. {url_b}")]);
    let (_, never) = entries("      url_link: never\n", &keys, records);
    assert_eq!(never, [format!("J. {doi} {arxiv}"), String::from("K.")]);
}

#[test]
fn a_record_whose_date_is_a_range_has_no_entry() {
    let records = r#"[{"id": "notes", "type": "book", "title": "Notes", "issued": {"date-parts": [[1997], [2020]]}}]"#;
    let (built, _) = entries("", &["bib:notes"], records);
    assert!(errors(&built).iter().any(|error| error.contains("must be a single date, not a range")), "{:?}", errors(&built));
}

// --- acceptance 2: an entry depends on its record alone -----------------------------------

#[test]
fn an_entry_formatted_alone_and_among_others_are_equal() {
    let (_, alone) = entries("", &["bib:calderbank96"], PROBE);
    let (_, among) = entries("", &["bib:li22", "bib:calderbank96"], PROBE);
    assert_eq!(alone[0], among[1]);
    let (_, alone) = entries("      style: {name: ieee}\n", &["bib:li22"], PROBE);
    let (_, among) = entries("      style: {name: ieee}\n", &["bib:calderbank96", "bib:li22"], PROBE);
    assert_eq!(alone[0], among[1]);
}

// --- acceptance 3: an article of Physical Review A ----------------------------------------

#[test]
fn the_entry_of_an_article_of_physical_review_a_as_it_reads() {
    // The shipped style writes the journal's name as the record has it.
    let (_, shipped) = entries("      doi_link: false\n      arxiv_link: false\n", &["bib:calderbank96"], PROBE);
    assert_eq!(shipped[0], format!("A. R. Calderbank and P. W. Shor, Physical Review A {}, 1098 (1996).", bold("54")));
    // The formatter's archive's own style of the American Physical Society sets the
    // journal's name in title case, which the formatter writes with a lowercase `a`, as it
    // writes a short word in a title: a known defect of the formatter, stated as it is so
    // that a change is seen.
    let settings = "      style: {name: american-physics-society}\n      doi_link: false\n      arxiv_link: false\n";
    let (_, archived) = entries(settings, &["bib:calderbank96"], PROBE);
    assert_eq!(
        archived[0],
        format!("A. R. Calderbank and P. W. Shor, Good quantum error-correcting codes exist, Physical Review a {}, 1098 (1996).", bold("54"))
    );
}

// --- acceptance 4: a style of the archive, a style file, and the styles refused -------------

/// A style writing a work's title alone, as a style file of the author's.
const TITLES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><title>Titles</title><id>titles</id><updated>2026-10-01T00:00:00+00:00</updated></info>
  <citation><layout><text variable="citation-number"/></layout></citation>
  <bibliography><layout><text variable="title" prefix="Title: "/></layout></bibliography>
</style>
"#;

#[test]
fn a_style_of_the_archive_and_a_style_file_of_the_author_s_are_used() {
    let settings = "      style: {name: ieee}\n      doi_link: false\n";
    let (built, ieee) = entries(settings, &["bib:li22"], PROBE);
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    assert!(ieee[0].contains("Wasserstein Complexity of Quantum Circuits"), "{}", ieee[0]);
    assert!(ieee[0].starts_with("L. Li, K. Bu, D. E. Koh, A. Jaffe, and S. Lloyd"), "{}", ieee[0]);
    // The file is found beside the document, and read through the build.
    let files = MemoryFiles::new()
        .with("doc/paper.flm", &paper("      style: {file: styles/titles.csl}\n      doi_link: false\n", &["bib:li22"]))
        .with("doc/refs.json", PROBE)
        .with("doc/styles/titles.csl", TITLES);
    let (built, html) = rendered_files(files, "html");
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    assert_eq!(entries_of(&html), ["Title: Wasserstein Complexity of Quantum Circuits"]);
    assert!(built.files_read.iter().any(|read| read.path == Path::new("doc/styles/titles.csl") && read.found));
}

/// A style with a citation part alone, and no bibliography part.
const NUMBERS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><title>Numbers</title><id>numbers</id><updated>2026-10-01T00:00:00+00:00</updated></info>
  <citation><layout><text variable="citation-number"/></layout></citation>
</style>
"#;

#[test]
fn an_unknown_style_and_a_file_that_is_no_style_are_reported() {
    let files = MemoryFiles::new()
        .with("doc/paper.flm", &paper("      style: {name: no-such-style}\n", &["bib:li22"]))
        .with("doc/refs.json", PROBE);
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.format = Some(String::from("html"));
    let error = prepare(request, Arc::new(files) as Arc<dyn Files>, &[]).err().expect("the registry is refused");
    assert!(format!("{error:?}").contains("the citation style `no-such-style` must be the name of a style"), "{error:?}");
    // A file that is no style, a style with no bibliography part, which can set no entry,
    // and a file that does not exist: an Error at the key.
    for (style, expected) in [
        ("refs.json", "must be a style of the Citation Style Language"),
        ("numbers.csl", "must have a `<bibliography>` element"),
        ("absent.csl", "absent.csl"),
    ] {
        let files = MemoryFiles::new()
            .with("doc/paper.flm", &paper(&format!("      style: {{file: {style}}}\n"), &["bib:li22"]))
            .with("doc/refs.json", PROBE)
            .with("doc/numbers.csl", NUMBERS);
        let (built, _) = rendered_files(files, "html");
        assert!(errors(&built).iter().any(|error| error.contains(expected)), "{style}: {:?}", errors(&built));
    }
}
