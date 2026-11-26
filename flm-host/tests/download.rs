//! The program's downloads: the host service `download` in a handler written in
//! JavaScript, the cache of the downloaded files across builds and past a value's hard
//! expiry, and the refusal of a download by a program that gave no downloader. No test
//! opens a connection to the network: each gives the program a stand-in for it.

use std::sync::Arc;
#[cfg(feature = "js")]
use std::sync::Mutex;

#[cfg(feature = "js")]
use flm_host::download::{Downloader, Response};

/// The address that the handler of [`DOWNLOADING`] downloads.
const ADDRESS: &str = "https://example.org/notes.txt";

/// A stand-in for the network: it answers every address with the text `notes for print` as
/// `text/plain`, or fails with "the network is unreachable", and records each address
/// requested.
#[cfg(feature = "js")]
struct Network {
    reachable: bool,
    requested: Mutex<Vec<String>>,
}

#[cfg(feature = "js")]
impl Network {
    fn new(reachable: bool) -> Arc<Network> {
        Arc::new(Network { reachable, requested: Mutex::new(Vec::new()) })
    }

    fn requested(&self) -> Vec<String> {
        self.requested.lock().unwrap().clone()
    }
}

#[cfg(feature = "js")]
impl Downloader for Network {
    fn get(&self, url: &str) -> Result<Response, String> {
        self.requested.lock().unwrap().push(url.to_string());
        if !self.reachable {
            return Err(String::from("the network is unreachable"));
        }
        Ok(Response { bytes: b"notes for print".to_vec(), media_type: Some(String::from("text/plain")) })
    }
}

/// The extension `dl`, whose citation provider answers each key with the file at
/// [`ADDRESS`], downloaded through the host service `download`: its text, its media type
/// and the warning of the answer, or `none`.
#[cfg(feature = "js")]
const DOWNLOADING: &str = r#"
import { PROVIDERS_SLOT } from "flm:stdlib/features/citations";

export default function (ext) {
  ext.package("_", {
    host: ["download"],
    features: { bib: { register(b) {
      b.contribute_to(PROVIDERS_SLOT, { name: "dl.web", answer(request, cx) {
        const got = cx.host("download").call("get", { url: "https://example.org/notes.txt" });
        const warning = got.warning === undefined ? "none" : got.warning;
        return { entry: cx.build_entry([{ text: `got ${got.content.text} as ${got.media_type}; warning: ${warning}` }]) };
      } });
    } } } });
}
"#;

/// A clock whose time a test sets.
#[cfg(feature = "js")]
#[derive(Clone, Default)]
struct Clock(Arc<std::sync::atomic::AtomicI64>);

#[cfg(feature = "js")]
impl Clock {
    /// Sets the time to `days` days after an instant of 2001.
    fn set_days(&self, days: i64) {
        self.0.store(1_000_000_000_000 + days * 24 * 60 * 60 * 1000, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(feature = "js")]
impl flm_host::cache::cachette::Clock for Clock {
    fn now(&self) -> flm_host::cache::cachette::Timestamp {
        flm_host::cache::cachette::Timestamp::from_millis(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}

/// A manager of the caches in memory, over `clock`.
#[cfg(feature = "js")]
fn in_memory(clock: &Clock) -> flm_host::cache::cachette::Manager {
    use flm_host::cache::cachette::{DiscardReports, Manager, ManagerSettings};
    Manager::in_memory(ManagerSettings::default(), clock.clone(), DiscardReports).unwrap()
}

/// The text output of the document `doc/paper.flm`, citing `k`, prepared afresh over
/// `manager` with the extension [`DOWNLOADING`], downloading through `network`.
#[cfg(feature = "js")]
fn downloaded(manager: &flm_host::cache::cachette::Manager, network: &Arc<Network>) -> String {
    use flm_core::progress::Scope;
    use flm_host::cache::CacheHost;
    use flm_host::download::Downloads;
    use flm_host::files::MemoryFiles;
    use flm_host::output::{MemorySink, Sink};
    use flm_host::packages::ExtensionFolder;
    use flm_host::session::{Input, Request, build, prepare};

    let files = MemoryFiles::new()
        .with("defs/dl/flmextension.yaml", "extension:\n  engine: js\n  js:\n    main: main.js\n")
        .with("defs/dl/main.js", DOWNLOADING)
        .with("doc/paper.flm", "---\nflm:\n  use: [dl]\n---\nA \\cite{k}.\n");
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request.extension_folders = vec![ExtensionFolder { path: String::from("defs"), required: true }];
    request.cache = CacheHost { manager: manager.clone(), document: None };
    request.downloads = Downloads::new(Arc::clone(network) as Arc<dyn Downloader>);
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    let built = build(&prepared, &Scope::none()).expect("built");
    let diagnostics: Vec<String> = built.diagnostics.iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let entry = sink.files().into_values().find(|file| file.entry).expect("an output");
    String::from_utf8(entry.bytes).unwrap()
}

#[cfg(feature = "js")]
#[test]
fn a_handler_in_javascript_receives_a_text_through_the_service() {
    let clock = Clock::default();
    clock.set_days(0);
    let network = Network::new(true);
    let text = downloaded(&in_memory(&clock), &network);
    assert!(text.contains("[1] got notes for print as text/plain; warning: none"), "{text}");
    assert_eq!(network.requested(), [ADDRESS]);
}

#[cfg(feature = "js")]
#[test]
fn a_second_build_within_the_lifetime_makes_no_request() {
    let clock = Clock::default();
    clock.set_days(0);
    let manager = in_memory(&clock);
    let network = Network::new(true);
    downloaded(&manager, &network);
    clock.set_days(1);
    let second = downloaded(&manager, &network);
    assert!(second.contains("got notes for print"), "{second}");
    assert_eq!(network.requested(), [ADDRESS]);
}

#[cfg(feature = "js")]
#[test]
fn a_value_past_its_hard_expiry_whose_request_fails_is_used_with_one_warning() {
    let clock = Clock::default();
    clock.set_days(0);
    let manager = in_memory(&clock);
    downloaded(&manager, &Network::new(true));
    // Past the lifetime of seven days, whatever its jitter.
    clock.set_days(30);
    let unreachable = Network::new(false);
    // The text output wraps its lines: the words are compared with single spaces.
    let text = downloaded(&manager, &unreachable).split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(text.contains("got notes for print as text/plain"), "{text}");
    let warning = format!("Failed to refresh the expired download of ‘{ADDRESS}’: the network is unreachable");
    assert_eq!(text.matches(&warning).count(), 1, "{text}");
    assert_eq!(unreachable.requested(), [ADDRESS]);
}

#[test]
fn a_program_that_gave_no_download_refuses_with_the_words_of_the_citations() {
    use flm_ext::paths::WrittenPath;
    use flm_host::files::{MemoryFiles, ServiceFiles};
    use flm_host::roots::Roots;

    let files = ServiceFiles::new(Arc::new(MemoryFiles::new()));
    let roots = Roots::new(Default::default(), Arc::new(flm_ext::MemoryFiles::new()), files, String::new());
    let WrittenPath::Qualified(path) = WrittenPath::parse(ADDRESS).unwrap() else { panic!("a qualified path") };
    let error = roots.read(&path).unwrap_err().to_string();
    assert_eq!(error, format!("Failed to download ‘{ADDRESS}’: this program was given no access to the network"));
}
