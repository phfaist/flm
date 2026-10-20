//! The connection between the standard provider
//! ([`StandardProvider`](super::StandardProvider)) and the crate `autocitefetch` for one
//! call of the provider: the fetcher that reads files and internet addresses, the cache
//! kept in memory when no cache folder is given, the relay of the crate's progress events
//! to the program's listener, and the loop that runs the crate's work on the calling
//! thread.
//!
//! The public items are [`FetchListener`], which a program embedding the host implements
//! to follow the progress of a fetch, and [`network_fetcher`], the fetcher that connects
//! to the internet; both go into [`CitationsHost`](super::CitationsHost).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use autocitefetch::{
    BoxFuture, CacheRecord, CacheStore, Event, FetchError, Fetcher, Reporter, Request, Response, StoreError,
};

use crate::files::BuildFiles;

/// A receiver of the progress events of a fetch: the standard provider passes it each
/// event ([`Event`]) that the crate [`autocitefetch`] reports while it fetches the records
/// of the cited works — a pass over the citation keys (one round of HTTP requests) started
/// or finished, an HTTP request
/// started or finished, a wait started with its expected duration (the arXiv asks for about
/// three seconds between two requests), a citation key resolved or failed.
///
/// A program embedding the host implements it to show progress, and gives it to the
/// provider in [`CitationsHost::listener`](super::CitationsHost::listener); the `flm`
/// program prints a line on standard error for some events under `-v`. The events are not
/// diagnostics: a citation key whose record could not be obtained is reported by the
/// library, at the citation.
///
/// The provider calls [`hear`](FetchListener::hear) on the thread that runs the provider.
/// This may be one of the library's worker threads
/// ([`on_worker_thread`](flm_core::lang::guard::on_worker_thread)) rather than the thread
/// that started the build; the trait therefore requires `Send` and `Sync`. Each call comes
/// between two HTTP requests and holds up the fetch until it returns, so a listener that
/// shows an event should show it at once and return quickly.
///
/// # Examples
///
/// A listener that counts the waits:
///
/// ```
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use flm_main::citations::{Event, FetchListener};
///
/// #[derive(Default)]
/// struct Waits(AtomicUsize);
///
/// impl FetchListener for Waits {
///     fn hear(&self, event: &Event<'_>) {
///         if let Event::WaitStarted { .. } = event {
///             self.0.fetch_add(1, Ordering::Relaxed);
///         }
///     }
/// }
/// ```
pub trait FetchListener: Send + Sync {
    /// Receives `event`, one progress event of the fetch.
    fn hear(&self, event: &Event<'_>);
}

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

/// The relay of the crate's progress events to the listener that the program embedding the
/// host gave.
pub(super) struct Relay(pub(super) Arc<dyn FetchListener>);

impl Reporter for Relay {
    fn report(&self, event: &Event<'_>) {
        self.0.hear(event);
    }
}

/// The text of the error that an HTTP request fails with when the program embedding the host
/// gave no network fetcher ([`CitationsHost::fetcher`](super::CitationsHost::fetcher) is
/// `None`).
const NO_NETWORK: &str = "this program was given no access to the network";

/// The fetcher of one call of the provider. An internet address (`http://`, `https://`)
/// goes to the network fetcher, with the header `user-agent` set to `user_agent`, and
/// fails with [`NO_NETWORK`] when there is none. Any other address is the path of a file —
/// a bibliography file, a file of DOIs given by hand for arXiv identifiers — read through
/// the build's reading of files ([`BuildFiles`]), so that the file is on the build's list
/// of files read.
pub(super) struct CallFetcher<'a> {
    pub(super) files: BuildFiles,
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
        let read = self.files.read_bytes(Path::new(""), Path::new(&request.url));
        let response = read
            .map(|body| Response { status: 200, headers: Default::default(), body })
            .map_err(|error| FetchError::Other(error.to_string()));
        Box::pin(async move { response })
    }
}

/// The cache of one call when the program embedding the host gives no cache folder
/// ([`CitationsHost::cache_folder`](super::CitationsHost::cache_folder) is `None`): the
/// records are kept in memory for the call and dropped after it.
#[derive(Default)]
pub(super) struct MemoryStore(RefCell<BTreeMap<String, CacheRecord>>);

impl CacheStore for MemoryStore {
    fn get(&self, id: &str) -> BoxFuture<'_, Result<Option<CacheRecord>, StoreError>> {
        let record = self.0.borrow().get(id).cloned();
        Box::pin(async move { Ok(record) })
    }

    fn put(&self, id: &str, record: CacheRecord) -> BoxFuture<'_, Result<(), StoreError>> {
        self.0.borrow_mut().insert(id.to_string(), record);
        Box::pin(async { Ok(()) })
    }

    fn remove(&self, id: &str) -> BoxFuture<'_, Result<(), StoreError>> {
        self.0.borrow_mut().remove(id);
        Box::pin(async { Ok(()) })
    }

    fn entries(&self) -> BoxFuture<'_, Result<Vec<(String, CacheRecord)>, StoreError>> {
        let entries = self.0.borrow().iter().map(|(id, record)| (id.clone(), record.clone())).collect();
        Box::pin(async move { Ok(entries) })
    }
}

/// Runs `future` to completion on the calling thread and returns its output. A plain loop
/// polls the future again until it is ready, as the crate's documentation describes: the
/// crate's implementations for the disk, the clock and the network complete their work
/// when polled and never wait for a wake-up, so the loop needs no asynchronous runtime. A
/// fetcher whose future stays pending until something wakes it keeps this loop polling it,
/// with only a yield of the thread between two polls, until the future is ready.
// See decision rationale at [§flm-dd-dr:host-citations-per-call]
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
