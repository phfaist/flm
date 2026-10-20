//! Citations on the host (cargo feature `citations`): the standard provider of the feature
//! `citations-standard` fetching records through the crate `autocitefetch`, over a server of
//! the tests' own standing for the arXiv and doi.org, with its cache in a folder under
//! `target/`; the bibliography files read through the build's reading of files; the paths
//! of the settings resolved against the file they are written in; the listener.
//!
//! No test opens a connection: the network is [`Server`], which answers from tables.

#[path = "support/files.rs"]
mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flm_core::stdcore::features::citations::{CITATIONS, ENTRIES, MissingEntry, Retrieval};
use flm_main::citations::autocitefetch::{BoxFuture, FetchError, Request as FetchRequest, Response};
use flm_main::citations::{CitationsHost, Event, FetchListener, Fetcher, Wait};
use flm_main::files::{FileError, Files, MemoryFiles};
use flm_main::output::{MemorySink, Sink};
use flm_main::packages::ExtensionFolder;
use flm_main::session::{Built, Input, Prepared, Request, build, prepare};
use flm_main::settings::CommandLineSetting;
use support::Written;

// --- the network of the tests ----------------------------------------------------------------

/// The arXiv and doi.org as the tests stand them up: the arXiv answers its query for a list
/// of identifiers with an Atom feed of those it knows; doi.org answers a DOI it knows, in any
/// letter case, with its record, and any other with the status 404. Every request is noted,
/// with the name the program presented itself by.
#[derive(Default)]
struct Server {
    /// The preprints: identifier, title, and the DOI of the published version.
    arxiv: BTreeMap<&'static str, (&'static str, Option<&'static str>)>,
    /// The articles: DOI in lowercase, and title.
    dois: BTreeMap<&'static str, &'static str>,
    requests: Mutex<Vec<(String, Option<String>)>>,
}

impl Server {
    fn standard() -> Arc<Server> {
        Arc::new(Server {
            arxiv: BTreeMap::from([
                ("2001.00001", ("First preprint", None)),
                ("2001.00002", ("Second preprint", None)),
                ("2001.00003", ("Third preprint", None)),
                ("2001.00004", ("Fourth preprint", Some("10.1000/Fourth"))),
            ]),
            dois: BTreeMap::from([
                ("10.1000/fourth", "Article four"),
                ("10.1000/one", "Article one"),
                ("10.1000/two", "Article two"),
            ]),
            requests: Mutex::new(Vec::new()),
        })
    }

    /// The addresses requested so far that begin with `start`.
    fn requested(&self, start: &str) -> Vec<String> {
        let requests = self.requests.lock().unwrap();
        requests.iter().filter(|(url, _)| url.starts_with(start)).map(|(url, _)| url.clone()).collect()
    }

    fn answer(&self, url: &str) -> Response {
        let (status, body) = if let Some(query) = url.strip_prefix("https://export.arxiv.org/api/query?id_list=") {
            let ids = decoded(query.split('&').next().unwrap_or_default());
            let entries: String = ids
                .split(',')
                .filter_map(|id| self.arxiv.get(id).map(|entry| (id, entry)))
                .map(|(id, (title, doi))| {
                    let doi = doi.map(|doi| format!("<arxiv:doi>{doi}</arxiv:doi>")).unwrap_or_default();
                    format!(
                        "<entry><id>http://arxiv.org/abs/{id}v1</id><updated>2013-01-02T00:00:00Z</updated>\
                         <published>2012-11-05T00:00:00Z</published><title>{title}</title>\
                         <author><name>Ann Author</name></author>{doi}</entry>"
                    )
                })
                .collect();
            let feed = format!(
                "<?xml version=\"1.0\"?><feed xmlns=\"http://www.w3.org/2005/Atom\" \
                 xmlns:arxiv=\"http://arxiv.org/schemas/atom\">{entries}</feed>"
            );
            (200, feed)
        } else if let Some(doi) = url.strip_prefix("https://doi.org/") {
            let doi = decoded(doi).to_lowercase();
            match self.dois.get(doi.as_str()) {
                Some(title) => {
                    (200, serde_json::json!({"type": "article-journal", "title": title, "DOI": doi}).to_string())
                }
                None => (404, String::from("not found")),
            }
        } else {
            (404, String::new())
        };
        Response { status, headers: Default::default(), body: body.into_bytes() }
    }
}

impl Fetcher for Server {
    fn fetch(&self, request: FetchRequest) -> BoxFuture<'_, Result<Response, FetchError>> {
        let agent = request.headers.get("user-agent").cloned();
        self.requests.lock().unwrap().push((request.url.clone(), agent));
        let response = self.answer(&request.url);
        Box::pin(async move { Ok(response) })
    }
}

/// `text` with its percent-encoded bytes decoded.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

/// A listener noting the events it hears, each as a short word.
#[derive(Default)]
struct Notes(Mutex<Vec<String>>);

impl FetchListener for Notes {
    fn hear(&self, event: &Event<'_>) {
        let note = match event {
            Event::WaitStarted { what: Wait::RateLimit { prefix }, .. } => format!("wait {prefix}"),
            Event::PassFinished { pass, .. } => format!("pass {pass}"),
            _ => return,
        };
        self.0.lock().unwrap().push(note);
    }
}

// --- the host, over files in memory --------------------------------------------------------

/// A folder of its own under `target/` for the cache of the test `name`, emptied.
fn cache_folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("citations").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A citation style that writes a work's title alone, as it stands in the record: these
/// tests look at which record each key was given, not at how an entry is set.
const TITLES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><title>Titles</title><id>titles</id><updated>2026-10-01T00:00:00+00:00</updated></info>
  <citation><layout><text variable="citation-number"/></layout></citation>
  <bibliography><layout><text variable="title"/></layout></bibliography>
</style>
"#;

/// Files in memory holding the style [`TITLES`], at `/titles.csl`.
fn memory() -> MemoryFiles {
    MemoryFiles::new().with("/titles.csl", TITLES)
}

/// The setting of the command line choosing the style [`TITLES`].
fn titles_style() -> CommandLineSetting {
    let assignment = "citations-standard.style={file: /titles.csl}";
    CommandLineSetting::new(assignment, format!("-O {assignment}"))
}

/// A request for `doc/paper.flm`, workflow `text`, over the server `server`, with the cache in
/// `cache`, the entries set in the style [`TITLES`].
fn request(server: &Arc<Server>, cache: Option<&Path>) -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request.options = vec![titles_style()];
    request.citations = CitationsHost {
        fetcher: Some(Arc::clone(server) as Arc<dyn Fetcher + Send + Sync>),
        cache_folder: cache.map(Path::to_path_buf),
        listener: None,
    };
    request
}

/// The document citing `keys`, each in a `\cite` of its own, with `front` under the
/// settings of the feature `citations-standard` (indented by six spaces).
fn paper(front: &str, keys: &[&str]) -> String {
    let cites: String = keys.iter().map(|key| format!("\\cite{{{key}}} ")).collect();
    let front = if front.is_empty() { String::new() } else { format!("    citations-standard:\n{front}") };
    format!("---\nflm:\n  features:\n{front}    text: {{}}\n---\nSee {cites}.\n")
}

fn prepared(files: Arc<dyn Files>, request: Request) -> Prepared {
    prepare(request, files, &[]).unwrap_or_else(|error| panic!("{error:?}"))
}

/// The build of `request` over `files`, and the text output.
fn rendered(files: Arc<dyn Files>, mut request: Request) -> (Built, String) {
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    request.continue_with_errors = true;
    let built = build(&prepared(files, request)).unwrap_or_else(|error| panic!("{error:?}"));
    let entry = sink.files().into_values().find(|file| file.entry).expect("an output");
    (built, String::from_utf8(entry.bytes).unwrap())
}

/// The identity of the work `key` stands for.
fn identity(built: &Built, key: &str) -> String {
    let works = built.run.table(CITATIONS).unwrap().expect("the works");
    works.work(key).unwrap_or_else(|| panic!("no work for `{key}`")).identity.clone()
}

/// What the providers answered for `key`.
fn retrieval(built: &Built, key: &str) -> Retrieval {
    let entries = built.run.table(ENTRIES).unwrap().expect("the entries");
    entries.get(key).unwrap_or_else(|| panic!("no retrieval for `{key}`")).clone()
}

/// The reason `key` has no entry, as the check states it.
fn missing(built: &Built, key: &str) -> String {
    match retrieval(built, key) {
        Retrieval::Missing(reason) => reason.reason(),
        Retrieval::Found(_) => panic!("`{key}` has an entry"),
    }
}

// --- acceptance 1: one request for many keys, the cache, the identities ----------------------

#[test]
fn three_arxiv_keys_make_one_request_and_a_second_run_makes_none() {
    let server = Server::standard();
    let cache = cache_folder("three-arxiv");
    let keys = ["arxiv:2001.00001", "arxiv:2001.00002", "arxiv:2001.00003"];
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper("", &keys)));
    let (built, text) = rendered(Arc::clone(&files), request(&server, Some(&cache)));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert_eq!(server.requested("https://export.arxiv.org").len(), 1);
    for title in ["First preprint", "Second preprint", "Third preprint"] {
        assert!(text.contains(title), "{text}");
    }
    assert!(cache.join(".flm-citations.jsonl").is_file());
    // A second run, a new registry over the same cache, fetches nothing.
    let (again, text_again) = rendered(files, request(&server, Some(&cache)));
    assert!(!again.has_errors(), "{:?}", again.diagnostics);
    assert_eq!(server.requested("https://export.arxiv.org").len(), 1);
    assert_eq!(text_again, text);
}

#[test]
fn every_request_names_the_program() {
    let server = Server::standard();
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper("", &["arxiv:2001.00001"])));
    rendered(Arc::clone(&files), request(&server, None));
    let front = "      user_agent: 'flm-test (someone@example.org)'\n";
    let named: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper(front, &["arxiv:2001.00002"])));
    rendered(named, request(&server, None));
    let agents: Vec<Option<String>> = server.requests.lock().unwrap().iter().map(|(_, agent)| agent.clone()).collect();
    assert_eq!(agents.len(), 2);
    assert!(agents[0].as_deref().is_some_and(|agent| agent.starts_with("flm/")), "{agents:?}");
    assert_eq!(agents[1].as_deref(), Some("flm-test (someone@example.org)"));
}

#[test]
fn a_prefix_in_capitals_and_a_doi_in_two_cases_each_name_one_work() {
    let server = Server::standard();
    let keys = ["arXiv:2001.00001", "arxiv:2001.00001", "doi:10.1000/One", "DOI:10.1000/one"];
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper("", &keys)));
    let (built, _) = rendered(files, request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert_eq!(identity(&built, "arXiv:2001.00001"), "arxiv:2001.00001");
    assert_eq!(identity(&built, "arxiv:2001.00001"), "arxiv:2001.00001");
    assert_eq!(identity(&built, "doi:10.1000/One"), "doi:10.1000/one");
    assert_eq!(identity(&built, "DOI:10.1000/one"), "doi:10.1000/one");
    // Each work is fetched once: the crate brings the spellings together.
    assert_eq!(server.requested("https://export.arxiv.org").len(), 1);
    assert_eq!(server.requested("https://doi.org").len(), 1);
}

// --- acceptance 3: a preprint followed to its published version ------------------------------

#[test]
fn an_arxiv_key_whose_entry_names_a_doi_answers_with_the_article_s_record() {
    let server = Server::standard();
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper("", &["arxiv:2001.00004"])));
    let (built, text) = rendered(files, request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert_eq!(identity(&built, "arxiv:2001.00004"), "arxiv:2001.00004");
    assert!(text.contains("Article four"), "{text}");
    assert!(!text.contains("Fourth preprint"), "{text}");
    assert_eq!(server.requested("https://doi.org").len(), 1);
}

#[test]
fn with_no_source_of_dois_an_arxiv_key_keeps_the_preprint_s_record() {
    let server = Server::standard();
    let front = "      sources:\n        doi: false\n";
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper(front, &["arxiv:2001.00004"])));
    let (built, text) = rendered(files, request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.contains("Fourth preprint"), "{text}");
    assert!(server.requested("https://doi.org").is_empty());
}

#[test]
fn override_dois_replaces_the_doi_the_arxiv_names_and_null_keeps_the_preprint_s_record() {
    let server = Server::standard();
    let cache = cache_folder("override-dois");
    let front = "      sources:\n        arxiv:\n          kind: arxiv\n          \
                 override_dois: {'2001.00001': 10.1000/one, '2001.00004': null}\n";
    let keys = ["arxiv:2001.00001", "arxiv:2001.00004"];
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper(front, &keys)));
    let (built, text) = rendered(files, request(&server, Some(&cache)));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // The DOI given by hand: the article's record, under the arXiv key's identity.
    assert!(text.contains("Article one") && !text.contains("First preprint"), "{text}");
    assert_eq!(identity(&built, "arxiv:2001.00001"), "arxiv:2001.00001");
    // `null`: the arXiv's record, although the arXiv names a DOI.
    assert!(text.contains("Fourth preprint") && !text.contains("Article four"), "{text}");
    // The one request to doi.org is the DOI given by hand.
    assert_eq!(server.requested("https://doi.org").len(), 1);
}

#[test]
fn an_override_dois_file_is_found_beside_the_package_naming_it_and_read_through_the_build() {
    let server = Server::standard();
    let cache = cache_folder("override-dois-file");
    let package = "flm:\n  features:\n    citations-standard:\n      sources:\n        arxiv:\n          kind: arxiv\n          \
                   override_dois_file: dois.json\n";
    let document = "---\nflm:\n  use: [dois-pkg]\n---\nSee \\cite{arxiv:2001.00002}.\n";
    let files = memory()
        .extension("defs/dois-pkg.yaml", package)
        .with("defs/dois.json", "{\"2001.00002\": \"10.1000/two\"}")
        .with("doc/dois.json", "{\"2001.00002\": \"10.1000/one\"}")
        .document("doc/paper.flm", document);
    let mut request = request(&server, Some(&cache));
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    let (built, text) = rendered(Arc::new(files), request);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.contains("Article two") && !text.contains("Second preprint"), "{text}");
    assert!(built.files_read.iter().any(|read| read.path == Path::new("defs/dois.json") && read.found));
}

// --- acceptance 4: keys not this provider's, and keys it cannot answer -----------------------

#[test]
fn a_key_with_no_colon_or_an_unknown_prefix_is_not_the_provider_s() {
    let server = Server::standard();
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper("", &["knuth84", "isbn:12345"])));
    let (built, _) = rendered(files, request(&server, None));
    let unknown = MissingEntry::Unknown.reason();
    assert_eq!(missing(&built, "knuth84"), unknown);
    assert_eq!(missing(&built, "isbn:12345"), unknown);
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn a_source_out_of_reach_and_a_source_without_the_key_are_reported_per_key() {
    let server = Server::standard();
    let keys = ["arxiv:2001.00001", "doi:10.1000/none", "bib:absent", "arxiv:2999.99999"];
    let files = memory()
        .document("doc/paper.flm", &paper("      bibliography: [refs.json]\n", &keys))
        .with("doc/refs.json", "[{\"id\": \"present\", \"title\": \"Present\"}]");
    let files: Arc<dyn Files> = Arc::new(files);
    let (built, _) = rendered(Arc::clone(&files), request(&server, None));
    assert!(missing(&built, "doi:10.1000/none").contains("not found"), "{}", missing(&built, "doi:10.1000/none"));
    assert!(missing(&built, "bib:absent").contains("not found"), "{}", missing(&built, "bib:absent"));
    assert!(missing(&built, "arxiv:2999.99999").contains("no arXiv entry"), "{}", missing(&built, "arxiv:2999.99999"));
    assert!(matches!(retrieval(&built, "arxiv:2001.00001"), Retrieval::Found(_)));
    // A program given no network: the keys fetched over it have no entry, the others do.
    let mut offline = request(&server, None);
    offline.citations.fetcher = None;
    let (built, _) = rendered(files, offline);
    assert!(missing(&built, "arxiv:2001.00001").contains("network"), "{}", missing(&built, "arxiv:2001.00001"));
    assert!(missing(&built, "bib:absent").contains("not found"));
}

// --- acceptance 2: bibliography files ------------------------------------------------------

/// Files in memory that a test changes between two builds.
struct Changing(Mutex<MemoryFiles>);

impl Files for Changing {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
        self.0.lock().unwrap().entries(path)
    }
}

#[test]
fn a_bibliography_file_in_each_format_is_read_through_the_build_and_listed() {
    let server = Server::standard();
    let front = "      bibliography: [refs.json, refs.yaml]\n";
    let files = memory()
        .document("doc/paper.flm", &paper(front, &["bib:knuth84", "bib:Lamport94"]))
        .with("doc/refs.json", "[{\"id\": \"knuth84\", \"title\": \"The TeXbook\"}]")
        .with("doc/refs.yaml", "- id: Lamport94\n  title: 'LaTeX: a document preparation system'\n");
    let (built, text) = rendered(Arc::new(files), request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.contains("The TeXbook"), "{text}");
    assert!(text.contains("LaTeX: a document preparation system"), "{text}");
    assert_eq!(identity(&built, "bib:Lamport94"), "bib:Lamport94");
    for file in ["doc/refs.json", "doc/refs.yaml"] {
        assert!(built.files_read.iter().any(|read| read.path == Path::new(file) && read.found), "{file}");
    }
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn a_bibliography_file_changed_between_two_builds_changes_the_entry() {
    let server = Server::standard();
    let cache = cache_folder("changed-bib");
    let files = memory()
        .document("doc/paper.flm", &paper("      bibliography: [refs.json]\n", &["bib:knuth84"]))
        .with("doc/refs.json", "[{\"id\": \"knuth84\", \"title\": \"First title\"}]");
    let changing = Arc::new(Changing(Mutex::new(files)));
    let mut request = request(&server, Some(&cache));
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = prepared(Arc::clone(&changing) as Arc<dyn Files>, request);
    let text = |sink: &MemorySink| {
        String::from_utf8(sink.files().into_values().find(|file| file.entry).unwrap().bytes).unwrap()
    };
    build(&prepared).unwrap();
    assert!(text(&sink).contains("First title"));
    changing.0.lock().unwrap().insert("doc/refs.json", "[{\"id\": \"knuth84\", \"title\": \"Second title\"}]");
    let again = build(&prepared).unwrap();
    assert!(text(&sink).contains("Second title"), "{}", text(&sink));
    assert!(again.files_read.iter().any(|read| read.path == Path::new("doc/refs.json")));
}

#[test]
fn a_second_bibliography_source_answers_under_its_own_prefix() {
    let server = Server::standard();
    let front = "      bibliography: [refs.json]\n      sources:\n        thesis:\n          kind: bib\n          files: [theses.yaml]\n";
    let files = memory()
        .document("doc/paper.flm", &paper(front, &["bib:knuth84", "thesis:phd"]))
        .with("doc/refs.json", "[{\"id\": \"knuth84\", \"title\": \"The TeXbook\"}]")
        .with("doc/theses.yaml", "phd:\n  title: A thesis\n");
    let (built, text) = rendered(Arc::new(files), request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.contains("The TeXbook") && text.contains("A thesis"), "{text}");
    assert_eq!(identity(&built, "thesis:phd"), "thesis:phd");
}

#[test]
fn a_bibliography_is_found_relative_to_the_file_naming_it() {
    let server = Server::standard();
    let refs = |title: &str| format!("[{{\"id\": \"x\", \"title\": \"{title}\"}}]");
    let files = || {
        memory()
            .extension("defs/refs-pkg.yaml", "flm:\n  features:\n    citations-standard:\n      bibliography: [refs.json]\n")
            .with("defs/refs.json", &refs("Beside the package"))
            .with("doc/refs.json", &refs("Beside the document"))
            .with("refs.json", &refs("In the current folder"))
    };
    let with_package = |front: &str| {
        let uses = format!("---\nflm:\n  use: [refs-pkg]\n{front}---\nSee \\cite{{bib:x}}.\n");
        let files = files().document("doc/paper.flm", &uses);
        let mut request = request(&server, None);
        request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
        (Arc::new(files) as Arc<dyn Files>, request)
    };
    let (files, request_) = with_package("");
    assert!(rendered(files, request_).1.contains("Beside the package"));
    let (files, request_) =
        with_package("  features:\n    citations-standard:\n      bibliography: [refs.json]\n");
    assert!(rendered(files, request_).1.contains("Beside the document"));
    let (files, mut request_) = with_package("");
    let assignment = "citations-standard.bibliography=[refs.json]";
    request_.options.push(CommandLineSetting::new(assignment, format!("-O {assignment}")));
    assert!(rendered(files, request_).1.contains("In the current folder"));
}

#[test]
fn the_schema_marks_the_paths_of_the_feature() {
    let schema = support::front_matter();
    let marks = &schema["$defs"]["citations-standard"]["x-flm-marks"];
    let path_marks: Vec<&str> = marks
        .as_array()
        .unwrap()
        .iter()
        .filter(|mark| mark["kind"] == "path")
        .map(|mark| mark["path"].as_str().unwrap())
        .collect();
    assert_eq!(path_marks, ["bibliography", "sources.*.files", "sources.*.override_dois_file", "style.file"]);
}

// --- acceptance 5: the listener -------------------------------------------------------------

#[test]
fn the_listener_hears_a_wait_and_a_pass_finished() {
    let server = Server::standard();
    let notes = Arc::new(Notes::default());
    let files: Arc<dyn Files> =
        Arc::new(memory().document("doc/paper.flm", &paper("", &["doi:10.1000/one", "doi:10.1000/two"])));
    let mut heard = request(&server, None);
    heard.citations.listener = Some(Arc::clone(&notes) as Arc<dyn FetchListener>);
    rendered(Arc::clone(&files), heard);
    let noted = notes.0.lock().unwrap().clone();
    assert!(noted.contains(&String::from("wait doi")), "{noted:?}");
    assert!(noted.contains(&String::from("pass 1")), "{noted:?}");
    // A host that gives no listener: the run is the same, and the listener above hears
    // nothing more.
    let (built, _) = rendered(files, request(&server, None));
    assert!(!built.has_errors());
    assert_eq!(notes.0.lock().unwrap().len(), noted.len());
}

// --- acceptance 7: the sources combined entry by entry ---------------------------------------

#[test]
fn sources_written_in_a_package_add_one_and_false_removes_one() {
    let server = Server::standard();
    let package = "flm:\n  features:\n    citations-standard:\n      bibliography: [refs.yaml]\n      sources:\n        thesis:\n          \
                   kind: bib\n          files: [theses.yaml]\n        manual: false\n";
    let document = "---\nflm:\n  use: [more-sources]\n---\nSee \\cite{thesis:phd} \\cite{arxiv:2001.00001} \\cite{doi:10.1000/one} \
                    \\cite{bib:knuth84} \\cite{manual:{By hand}}.\n";
    let files = memory()
        .extension("defs/more-sources.yaml", package)
        .with("defs/theses.yaml", "phd:\n  title: A thesis\n")
        .with("defs/refs.yaml", "knuth84:\n  title: The TeXbook\n")
        .document("doc/paper.flm", document);
    let mut request = request(&server, None);
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    let (built, text) = rendered(Arc::new(files), request);
    // The source added answers, and the three defaults not removed still answer.
    for title in ["A thesis", "First preprint", "Article one", "The TeXbook"] {
        assert!(text.contains(title), "{title}: {text}");
    }
    for key in ["thesis:phd", "arxiv:2001.00001", "doi:10.1000/one", "bib:knuth84"] {
        assert!(matches!(retrieval(&built, key), Retrieval::Found(_)), "{key}");
    }
    // The key is the FLM code written for it, braces included.
    assert_eq!(missing(&built, "manual:{By hand}"), MissingEntry::Unknown.reason());
}

#[test]
fn a_sources_prefix_no_key_can_name_makes_a_registry_that_does_not_build() {
    let server = Server::standard();
    for prefix in ["x:y", "Thesis"] {
        let front = format!("      sources:\n        '{prefix}': {{kind: bib}}\n");
        let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper(&front, &["bib:a"])));
        let error = match prepare(request(&server, None), files, &[]) {
            Ok(_) => panic!("the prefix `{prefix}` is refused"),
            Err(error) => format!("{error:?}"),
        };
        assert!(error.contains(&format!("the prefix `{prefix}` in the setting `sources`")), "{error}");
    }
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn a_manual_key_is_its_own_entry_in_flm() {
    let server = Server::standard();
    // A comma inside braces belongs to the key.
    let key = "manual:{X et al., Journal of Future Results (2034)}";
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", &paper("", &[key])));
    let (built, text) = rendered(files, request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.contains("[1] X et al., Journal of Future Results (2034)\n"), "{text}");
    assert_eq!(identity(&built, key), key);
    assert!(server.requests.lock().unwrap().is_empty());
}

/// Whether the entry answered for `key` holds an emphasis.
fn emphasized(built: &Built, key: &str) -> bool {
    match retrieval(built, key) {
        Retrieval::Found(answer) => {
            answer.entry.content.root().descendants().any(|node| node.name() == Some("emph"))
        }
        Retrieval::Missing(reason) => panic!("`{key}` has no entry: {}", reason.reason()),
    }
}

#[test]
fn a_manual_key_holding_a_command_is_an_entry_holding_it() {
    let server = Server::standard();
    let document = "---\nflm:\n  features:\n    userdefs:\n      macros:\n        \
                    man: {content: '\\cite{manual:\\emph{#1}}', args: ['{']}\n    text: {}\n---\n\
                    See \\cite{manual:\\emph{Title}} and \\man{Other}.\n";
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", document));
    let (built, _) = rendered(files, request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // Written, and synthesized by a user definition's expansion.
    assert!(emphasized(&built, "manual:\\emph{Title}"));
    assert!(emphasized(&built, "manual:\\emph{Other}"));
}

#[test]
fn a_ready_entry_in_a_bibliography_file_is_flm_or_literal_characters() {
    let server = Server::standard();
    let refs = "[{\"id\": \"a\", \"_ready_formatted\": {\"flm\": \"Entry \\\\emph{a}\"}},\
                 {\"id\": \"b\", \"_ready_formatted\": {\"html\": \"<i>b</i> \\\\emph\"}},\
                 {\"id\": \"c\", \"_formatted_flm_text\": \"Old \\\\textbf{c}\"}]";
    let files = memory()
        .document("doc/paper.flm", &paper("      bibliography: [refs.json]\n", &["bib:a", "bib:b", "bib:c"]))
        .with("doc/refs.json", refs);
    let (built, text) = rendered(Arc::new(files), request(&server, None));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.contains("Entry a"), "{text}");
    assert!(text.contains("<i>b</i> \\emph"), "{text}");
    assert!(text.contains("Old c"), "{text}");
}

// --- a preprint and its published version cited side by side ----------------------------------

/// The Warnings `flm.citations.provider-warning` of `built`, each its message and the text
/// it stands at.
fn chain_warnings(built: &Built) -> Vec<(String, String)> {
    built
        .diagnostics
        .iter()
        .filter(|d| d.identifier() == "flm.citations.provider-warning")
        .map(|d| {
            assert_eq!(d.severity(), flm_core::techy::error::Severity::Warning);
            (d.message(), d.span().content().to_string())
        })
        .collect()
}

/// The build of `document` over the standard server, the cache in `cache`, its Warnings of
/// a chain, and the text output.
fn chained(server: &Arc<Server>, document: &str, cache: Option<&Path>) -> (Built, Vec<(String, String)>, String) {
    let files: Arc<dyn Files> = Arc::new(memory().document("doc/paper.flm", document));
    let (built, text) = rendered(files, request(server, cache));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let warnings = chain_warnings(&built);
    (built, warnings, text)
}

/// The Warning of the key `key` whose chain leads to the key `other`.
fn warned(key: &str, other: &str) -> String {
    format!("`{key}` and `{other}` are cited as two works, while the record of `{key}` is taken from `{other}`, its published version")
}

#[test]
fn a_preprint_cited_beside_its_published_version_is_two_works_and_a_warning() {
    let server = Server::standard();
    let document = "---\nflm:\n  features:\n    text: {}\n---\n\
                    See \\cite{arxiv:2001.00004}, \\cite{doi:10.1000/fourth} and \\cite{arxiv:2001.00004}.\n";
    let (built, warnings, text) = chained(&server, document, None);
    assert_eq!(
        warnings,
        [(warned("arxiv:2001.00004", "doi:10.1000/fourth"), String::from("\\cite{arxiv:2001.00004}"))]
    );
    // The first citation of the arXiv key, not the second.
    let at = built.diagnostics.iter().find(|d| d.identifier() == "flm.citations.provider-warning").unwrap();
    assert_eq!(at.span().source().content().find("\\cite{arxiv"), Some(at.span().start()));
    // Still two works, two numbers, two lines.
    let works = built.run.table(CITATIONS).unwrap().expect("the works");
    assert_eq!(works.work("arxiv:2001.00004").unwrap().number, 1);
    assert_eq!(works.work("doi:10.1000/fourth").unwrap().number, 2);
    assert_eq!(text.matches("Article four").count(), 2, "{text}");
}

#[test]
fn keys_not_chained_to_each_other_give_no_warning() {
    let server = Server::standard();
    for keys in [["doi:10.1000/one", "doi:10.1000/two"], ["arxiv:2001.00001", "doi:10.1000/one"]] {
        let (_, warnings, _) = chained(&server, &paper("", &keys), None);
        assert!(warnings.is_empty(), "{keys:?}: {warnings:?}");
    }
    // The preprint alone, its published version not cited.
    let (_, warnings, _) = chained(&server, &paper("", &["arxiv:2001.00004"]), None);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn the_two_keys_in_one_cite_or_the_doi_in_another_letter_case_give_the_same_warning() {
    let server = Server::standard();
    let document = "---\nflm:\n  features:\n    text: {}\n---\nSee \\cite{arxiv:2001.00004,doi:10.1000/fourth}.\n";
    let (_, warnings, _) = chained(&server, document, None);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].0, warned("arxiv:2001.00004", "doi:10.1000/fourth"));
    let (_, warnings, _) = chained(&server, &paper("", &["arxiv:2001.00004", "doi:10.1000/FOURTH"]), None);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].0, warned("arxiv:2001.00004", "doi:10.1000/FOURTH"));
}

#[test]
fn a_preprint_answered_from_the_cache_gives_the_same_warning() {
    let server = Server::standard();
    let cache = cache_folder("chain-warning");
    let document = paper("", &["arxiv:2001.00004", "doi:10.1000/fourth"]);
    let (_, first, _) = chained(&server, &document, Some(&cache));
    let (_, again, _) = chained(&server, &document, Some(&cache));
    assert_eq!(server.requested("https://export.arxiv.org").len(), 1);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(again, first);
}
