//! The connection between the standard provider
//! ([`StandardProvider`](super::StandardProvider)) and the crate `autocitefetch` for one
//! call of the provider: the fetcher that reads files and internet addresses, the report
//! of the crate's progress on the progress sheet of the build, and the loop that runs the
//! crate's work on the calling thread.
//!
//! The public items are [`FETCH_KIND`], the kind of the row in which the provider counts
//! the citation keys of one pass of a fetch, and [`network_fetcher`], the fetcher that
//! connects to the internet, which goes into [`CitationsHost`](super::CitationsHost).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use autocitefetch::csl::cite_id;
use autocitefetch::{BoxFuture, Event, FetchError, Fetcher, Reporter, Request, Response, Wait};
use flm_core::progress::{Counted, Kind, LogLevel, Scope, Unit};

use flm_ext::paths::{Root, WrittenPath};

// The text of the error that an HTTP request fails with when the program embedding the
// host gave no network fetcher (`CitationsHost::fetcher` is `None`), as a download without a
// downloader is refused.
use crate::download::NO_NETWORK;
use crate::roots::Roots;

/// The kind of the row of the progress sheet in which the standard provider counts the
/// citation keys of one pass of a fetch, `flm.citations.fetch`, titled "Fetching citation
/// data".
///
/// The crate `autocitefetch` fetches in passes: a pass sends the requests for the citation
/// keys whose records are not in the cache, and the next pass fetches the records that the
/// first pointed to (the published version of a preprint). Under the scope of the call
/// ([`ProviderCx::progress`](flm_core::stdlib::features::citations::ProviderCx::progress)),
/// the provider declares one row of this kind for each pass that has citation keys to
/// fetch. Its total is the number of those keys, the keys answered from the cache file not
/// counted. A source answers its keys in groups (one request for many arXiv identifiers,
/// one request for each DOI): the row advances by the number of keys of a group when the
/// source has answered that group, whether a record was found for a key or not.
pub const FETCH_KIND: Kind = Kind::titled("flm.citations.fetch", "Fetching citation data");

/// Returns a fetcher that connects to the internet: the fetcher of the crate
/// `autocitefetch-std`, built on the HTTP client `ureq`. It follows redirections and gives
/// up on an HTTP request that has not completed within two minutes. The provider sends the
/// program's name with every HTTP request it makes through it, in the header `user-agent`
/// ([`ProviderConfig::user_agent`](super::ProviderConfig::user_agent)).
///
/// The fetcher uses the proxy that the environment variables set (such as `HTTPS_PROXY`
/// and `ALL_PROXY`), as programs that connect to the internet usually do.
///
/// A program embedding the host gives it in
/// [`CitationsHost::fetcher`](super::CitationsHost::fetcher); the `flm` program does.
pub fn network_fetcher() -> Arc<dyn Fetcher + Send + Sync> {
    Arc::new(autocitefetch_std::UreqFetcher::new())
}

/// The report of the crate's progress events on the progress sheet, for one call of the
/// provider: a row [`FETCH_KIND`] for each pass that has citation keys to fetch, advanced
/// by the number of keys of each group of keys that a source has answered, and an `Info`
/// log line for each HTTP request and each wait, written under the row of the pass when
/// there is one. The wait for the cache to be written gets a line only when the cache is a
/// file. It also notes each kept record that the crate uses after a failed fetch of it
/// ([`take_served`](FetchProgress::take_served)).
pub(super) struct FetchProgress {
    /// The scope of the call of the provider.
    scope: Scope,
    /// Whether the cache of the call is a file.
    cache_file: bool,
    /// The row of the pass under way, when it has citation keys to fetch.
    pass: RefCell<Option<PassRow>>,
    /// The id of each kept record used after a failed fetch of it, with the failure.
    served: RefCell<Vec<(String, String)>>,
}

/// The row of a pass and, for each source prefix, the number of keys of the pass that the
/// source has answered and the row has counted.
struct PassRow {
    row: Counted,
    done: BTreeMap<String, u64>,
}

impl FetchProgress {
    /// Returns the report under `scope`, the scope of the call of the provider, for a cache
    /// that is a file when `cache_file` is true and kept in memory otherwise.
    pub(super) fn new(scope: &Scope, cache_file: bool) -> FetchProgress {
        FetchProgress { scope: scope.clone(), cache_file, pass: RefCell::new(None), served: RefCell::new(Vec::new()) }
    }

    /// Returns the id of each kept record that the crate used after a failed fetch of it, as
    /// the crate writes it (`doi:10.1/x`), with the text of the failure, and forgets them.
    pub(super) fn take_served(&self) -> Vec<(String, String)> {
        self.served.take()
    }

    /// Writes the `Info` log line that `text` makes, when the host's log sink wants it.
    fn log(&self, text: impl FnOnce() -> String) {
        let pass = self.pass.borrow().as_ref().map(|pass| pass.row.scope());
        let scope = pass.as_ref().unwrap_or(&self.scope);
        if scope.log_enabled(LogLevel::Info) {
            scope.log(LogLevel::Info, &text());
        }
    }
}

impl Reporter for FetchProgress {
    fn report(&self, event: &Event<'_>) {
        match event {
            Event::PassStarted { to_fetch, .. } => {
                let pass = (*to_fetch > 0).then(|| PassRow {
                    row: self.scope.child(&FETCH_KIND, None).start_counted(Unit::Items, *to_fetch as u64),
                    done: BTreeMap::new(),
                });
                // The crate finishes each pass before it starts the next: a row still held
                // here would end as failed.
                drop(self.pass.replace(pass));
            }
            // Each event gives the number of keys the source has answered so far in this
            // pass; the events of different sources may interleave.
            Event::SourceProgress { prefix, done, .. } | Event::SourceFinished { prefix, done } => {
                if let Some(pass) = self.pass.borrow_mut().as_mut() {
                    let done = *done as u64;
                    let counted = match pass.done.get_mut(*prefix) {
                        Some(counted) => counted,
                        None => pass.done.entry(prefix.to_string()).or_insert(0),
                    };
                    pass.row.advance(done.saturating_sub(*counted));
                    *counted = (*counted).max(done);
                }
            }
            Event::PassFinished { .. } => {
                let pass = self.pass.borrow_mut().take();
                if let Some(pass) = pass {
                    pass.row.finish();
                }
            }
            Event::CiteFailed { prefix, key, err, grace_served: true } => {
                self.served.borrow_mut().push((cite_id(prefix, key), err.to_string()));
            }
            Event::RequestStarted { url, .. } => self.log(|| format!("Fetching {url}")),
            Event::WaitStarted { what: Wait::CacheFlush, .. } if !self.cache_file => {}
            Event::WaitStarted { what, expected } => self.log(|| waiting(what, *expected)),
            _ => {}
        }
    }
}

/// The log line of a wait of the crate, `what`, expected to last `expected`.
fn waiting(what: &Wait<'_>, expected: Option<Duration>) -> String {
    let duration = seconds(expected);
    match what {
        Wait::RateLimit { prefix } => format!("Waiting {duration} before the next request for ‘{prefix}:’"),
        Wait::Backoff { url, attempt } => {
            format!("Waiting {duration} before trying {url} again (attempt {})", attempt + 2)
        }
        Wait::CacheFlush => format!("Waiting {duration} to write the citation cache file"),
        _ => format!("Waiting {duration}"),
    }
}

/// A duration as a log line writes it, in seconds with one decimal (`3.1 s`), or `a moment`
/// when the duration is unknown.
fn seconds(duration: Option<Duration>) -> String {
    duration.map_or_else(|| String::from("a moment"), |duration| format!("{:.1} s", duration.as_secs_f64()))
}

/// The fetcher of one call of the provider. An internet address (`http://`, `https://`)
/// goes to the network fetcher, with the header `user-agent` set to `user_agent`, and
/// fails with [`NO_NETWORK`] when there is none. Any other address is the qualified path
/// of a file — a bibliography file, a file of DOIs given by hand for arXiv identifiers —
/// read through the roots of the preparation ([`read_file`]), so that a file of the
/// document's folder is on the build's list of files read.
pub(super) struct CallFetcher<'a> {
    pub(super) roots: &'a Roots,
    pub(super) network: Option<&'a (dyn Fetcher + Send + Sync)>,
    pub(super) user_agent: &'a str,
}

impl Fetcher for CallFetcher<'_> {
    fn fetch(&self, request: Request) -> BoxFuture<'_, Result<Response, FetchError>> {
        if request.url.starts_with("http://") || request.url.starts_with("https://") {
            return match self.network {
                Some(network) => network.fetch(request.header("user-agent", self.user_agent)),
                None => Box::pin(async { Err(FetchError::Other(String::from(NO_NETWORK))) }),
            };
        }
        let response = read_file(self.roots, &request.url)
            .map(|body| Response { status: 200, headers: Default::default(), body })
            .map_err(FetchError::Other);
        Box::pin(async move { response })
    }
}

/// The bytes of the file at `path`, a qualified path as the settings give it
/// ([`crate::settings`]), read through `roots`; or the text of why they cannot be read: the
/// path is an address (`http://`, `https://`, or the root `https`), it has no meaning, it
/// has no root or the root `self`, there is no file at it, or the reading's own text.
pub(super) fn read_file(roots: &Roots, path: &str) -> Result<Vec<u8>, String> {
    let address = || format!("Invalid file ‘{path}’: the file may not be an address");
    if path.starts_with("http://") || path.starts_with("https://") {
        return Err(address());
    }
    match WrittenPath::parse(path).map_err(|error| error.to_string())? {
        WrittenPath::Qualified(qualified) if *qualified.root() == Root::Https => Err(address()),
        WrittenPath::Qualified(qualified) => match roots.read(&qualified) {
            Ok(Some(file)) => Ok(file.bytes),
            Ok(None) => Err(format!("Missing file ‘{qualified}’")),
            Err(error) => Err(error.to_string()),
        },
        WrittenPath::Own(rest) => {
            Err(format!("Invalid root ‘self’ in the path ‘{path}’: write ‘doc:{rest}’ or ‘ext+NAME:{rest}’"))
        }
        WrittenPath::Bare(rest) => {
            Err(format!("Missing root in the path ‘{path}’: write ‘doc:{rest}’ or ‘ext+NAME:{rest}’"))
        }
    }
}

/// Runs `future` to completion on the calling thread and returns its output. A plain loop
/// polls the future again until it is ready, as the crate's documentation describes: the
/// crate's implementations for the disk, the clock and the network complete their work
/// when polled and never wait for a wake-up, so the loop needs no asynchronous runtime. A
/// fetcher whose future stays pending until something wakes it keeps this loop polling it,
/// with only a yield of the thread between two polls, until the future is ready.
pub(super) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        std::thread::yield_now();
    }
}
