//! Downloads: the file at an `https` address, kept in the program's cache.
//!
//! The program that embeds the host gives what downloads a file, a [`Downloader`], in the
//! field [`downloads`](crate::session::Request::downloads) of its request, wrapped in a
//! [`Downloads`]. By default it gives none; [`Downloads::get`] then refuses every download
//! with the words of [`NO_NETWORK`] (this is what a compose template's `https` path
//! meets), and the loader does not offer the host service `download`
//! ([`crate::host_download`]), so a package that declares it does not load. Under the cargo feature `download`, `network_downloader` connects
//! to the internet; the `flm` program gives it in every request.
//!
//! A download is one `GET` of an address that begins with `https://`. The downloader of
//! the feature `download` waits at most 60 seconds for the whole request, takes at most
//! 32 MiB, follows at most five redirects, each to an `https` address, and honors the
//! proxy variables of the environment (`ALL_PROXY`, `HTTPS_PROXY`, `HTTP_PROXY`, in
//! capitals or in lowercase), as the fetcher of the citations does. A status other than a
//! success is a failed request. The download answers the bytes and the media type that
//! the server states (the header `content-type`, without its parameters, in lowercase).
//!
//! # The cache
//!
//! [`Downloads::get`] keeps each downloaded file in one values cache shared by documents,
//! [`CACHE`] (`flm.host.download.files`), whose values have a lifetime of seven days:
//!
//! - a fresh value is answered without a request;
//! - a stale or expired value is requested again; when the request fails, the kept value is
//!   answered, with a warning when it is past its hard expiry;
//! - with no kept value, the failure of the request is the failure of the download.
//!
//! While the program runs, an address whose request failed is not requested again for five
//! minutes ([`RETRY_AFTER`]): the kept value is answered, or the failure again. The live
//! preview builds at each save, and each build would otherwise wait for the network. The
//! time is measured with the system's monotonic clock ([`Instant`]).
//!
//! A request under way runs to its end: a cancelled build does not interrupt it.
//!
//! The citations do not download through this module: they keep their own fetcher, cache
//! and limits.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::cache::cachette::{BlobName, BlobSelection, Cost, Freshness, PutOptions, Record, TtlPolicy, ValueSort};
use crate::cache::{Instance, ServiceCaches, ValuesDeclaration, ValuesEngine};

/// The words with which a download is refused when the program gave no downloader, the
/// words of the citations' fetch in the same case.
pub const NO_NETWORK: &str = "this program was given no access to the network";

/// The time during which an address whose request failed is not requested again.
pub const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);

/// The name of the blob that holds a downloaded file's bytes.
const CONTENT: &str = "content";

/// What downloads a file: one `GET` of an address that begins with `https://`. See the
/// [module documentation](self).
pub trait Downloader: Send + Sync {
    /// Requests `url` once and answers the file.
    ///
    /// # Errors
    ///
    /// The text, for a person to read, of why the request failed: the address cannot be
    /// reached, the server answered a status other than a success, a limit was passed.
    fn get(&self, url: &str) -> Result<Response, String>;
}

/// A file that a [`Downloader`] answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// The media type that the server states, without its parameters, in lowercase
    /// (`text/css`); `None` when the server states none.
    pub media_type: Option<String>,
}

/// A file that [`Downloads::get`] answers: from the network or from the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// The media type that the server stated; `None` when it stated none.
    pub media_type: Option<String>,
    /// The warning, when the file is a kept value past its hard expiry that could not be
    /// requested again: "Failed to refresh the expired download of ‘URL’: …".
    pub warning: Option<String>,
}

/// A download that [`Downloads::get`] refused or that failed: the address, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadError {
    /// The address, as given.
    pub url: String,
    /// Why there is no file.
    pub kind: DownloadErrorKind,
}

/// Why a download gave no file ([`DownloadError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadErrorKind {
    /// The address does not begin with `https://`.
    Address,
    /// The program gave no downloader ([`NO_NETWORK`]).
    NoNetwork,
    /// The request failed and no value was kept: the downloader's text.
    Failed(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let url = &self.url;
        match &self.kind {
            DownloadErrorKind::Address => write!(f, "Unsupported address ‘{url}’: addresses must begin with ‘https://’"),
            DownloadErrorKind::NoNetwork => write!(f, "Failed to download ‘{url}’: {NO_NETWORK}"),
            DownloadErrorKind::Failed(detail) => write!(f, "Failed to download ‘{url}’: {detail}"),
        }
    }
}

impl std::error::Error for DownloadError {}

/// What the program gives the host to download files: the [`Downloader`], or none, and the
/// addresses whose request failed lately. See the [module documentation](self).
///
/// The default gives no downloader. The clones of a value share the record of the failed
/// requests, so every preparation made from one request, and every build of the live
/// preview, sees the same record.
#[derive(Clone, Default)]
pub struct Downloads {
    /// The downloader; `None` for a program given no network.
    downloader: Option<Arc<dyn Downloader>>,
    /// The failed requests: for each address, when the request failed and why.
    failed: Arc<Mutex<HashMap<String, (Instant, String)>>>,
}

impl Downloads {
    /// Returns the downloads through `downloader`, with no failed request.
    pub fn new(downloader: Arc<dyn Downloader>) -> Downloads {
        Downloads { downloader: Some(downloader), failed: Arc::default() }
    }

    /// Whether the program gave a downloader: the host then offers the host service
    /// `download` ([`crate::host_download`]).
    pub fn is_given(&self) -> bool {
        self.downloader.is_some()
    }

    /// Returns the file at `url`, from the cache [`CACHE`] opened through `caches` or from
    /// the network, by the rules of the [module documentation](self#the-cache).
    ///
    /// # Errors
    ///
    /// [`DownloadErrorKind::Address`] for an address that does not begin with `https://`;
    /// [`DownloadErrorKind::NoNetwork`] when the program gave no downloader;
    /// [`DownloadErrorKind::Failed`] for a failed request with no kept value.
    pub fn get(&self, caches: &ServiceCaches, url: &str) -> Result<Downloaded, DownloadError> {
        let refused = |kind| DownloadError { url: url.to_string(), kind };
        let Some(downloader) = &self.downloader else { return Err(refused(DownloadErrorKind::NoNetwork)) };
        if !url.starts_with("https://") {
            return Err(refused(DownloadErrorKind::Address));
        }
        // A failure of the cache is answered as without a cache.
        let session = caches.open_values(&CACHE, &Instance::Shared(serde_json::Value::Null)).ok();
        let key = key_of(url);
        let kept = session.as_deref().and_then(|session| Kept::read(session, &key));
        if let Some(kept) = kept.as_ref().filter(|kept| kept.freshness == Some(Freshness::Fresh)) {
            return Ok(kept.answer(None));
        }
        let error = match self.failed_lately(url) {
            Some(error) => error,
            None => match downloader.get(url) {
                Ok(response) => {
                    lock(&self.failed).remove(url);
                    if let Some(session) = &session {
                        store(session, &key, url, &response);
                    }
                    let Response { bytes, media_type } = response;
                    return Ok(Downloaded { bytes, media_type, warning: None });
                }
                Err(error) => {
                    lock(&self.failed).insert(url.to_string(), (Instant::now(), error.clone()));
                    error
                }
            },
        };
        match kept {
            Some(kept) => {
                let warning = (kept.freshness == Some(Freshness::Expired))
                    .then(|| format!("Failed to refresh the expired download of ‘{url}’: {error}"));
                Ok(kept.answer(warning))
            }
            None => Err(refused(DownloadErrorKind::Failed(error))),
        }
    }

    /// The text of the failure of the request of `url`, when it failed less than
    /// [`RETRY_AFTER`] ago.
    fn failed_lately(&self, url: &str) -> Option<String> {
        let failed = lock(&self.failed);
        let (at, error) = failed.get(url)?;
        (at.elapsed() < RETRY_AFTER).then(|| error.clone())
    }
}

impl fmt::Debug for Downloads {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Downloads").field("downloader", &self.downloader.as_ref().map(|_| "…")).finish_non_exhaustive()
    }
}

/// The values cache of the downloaded files, shared by documents: each value is the record
/// `{ "url", "media_type" }` with the file's bytes as its blob `content`, under the SHA-256
/// of the address. A value lives seven days; a kept value past its hard expiry stays, to be
/// answered when its address cannot be requested again.
pub const CACHE: ValuesDeclaration = ValuesDeclaration {
    name: Cow::Borrowed("flm.host.download.files"),
    version: 1,
    engine: ValuesEngine::Sqlite,
    blobs: true,
    sort: ValueSort::Volatile {
        lifetime: Duration::from_secs(7 * 24 * 60 * 60),
        policy: TtlPolicy { grace: Duration::MAX, ..TtlPolicy::DEFAULT },
    },
    default_cost: Cost::new(1000).unwrap(),
};

/// A value of [`CACHE`] as read: the file, and where it stands in its lifetime.
struct Kept {
    bytes: Vec<u8>,
    media_type: Option<String>,
    freshness: Option<Freshness>,
}

impl Kept {
    /// The value under `key` in `session`, `None` when there is none or it cannot be read.
    fn read(session: &crate::cache::cachette::ValuesSession, key: &str) -> Option<Kept> {
        let name = BlobName::new(CONTENT).expect("a blob name");
        let found = session.get_with_blobs(key, BlobSelection::Named(std::slice::from_ref(&name)))?;
        let record: serde_json::Value = found.value.record.deserialize().ok()?;
        let bytes = found.bytes.into_iter().find(|(blob, _)| *blob == name).map(|(_, bytes)| bytes)?;
        let media_type = record.get("media_type").and_then(serde_json::Value::as_str).map(str::to_string);
        Some(Kept { bytes, media_type, freshness: found.value.freshness })
    }

    /// The kept file as answered, with `warning`.
    fn answer(&self, warning: Option<String>) -> Downloaded {
        Downloaded { bytes: self.bytes.clone(), media_type: self.media_type.clone(), warning }
    }
}

/// Stores `response`, the file at `url`, under `key` in `session`.
fn store(session: &crate::cache::cachette::ValuesSession, key: &str, url: &str, response: &Response) {
    let record = serde_json::json!({ "url": url, "media_type": response.media_type });
    let Ok(record) = Record::from_serialize(&record) else { return };
    let blob = (BlobName::new(CONTENT).expect("a blob name"), response.bytes.clone());
    session.put(key, record, vec![blob], PutOptions::default());
}

/// The key of the value of `url`: the SHA-256 of the address, in lowercase hexadecimal
/// digits.
fn key_of(url: &str) -> String {
    Sha256::digest(url.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Locks `mutex`, also after a panic of another holder: the data it guards stays valid.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Returns the downloader that connects to the internet (cargo feature `download`), by the
/// limits of the [module documentation](self).
#[cfg(feature = "download")]
pub fn network_downloader() -> Arc<dyn Downloader> {
    Arc::new(network::NetworkDownloader::new())
}

/// The downloader over the crate `ureq` (cargo feature `download`).
#[cfg(feature = "download")]
mod network {
    use std::io::Read;
    use std::time::Duration;

    use super::{Downloader, Response};

    /// The most bytes that a download takes: 32 MiB.
    const MAX_BYTES: u64 = 32 * 1024 * 1024;

    /// The program's name sent to servers.
    const USER_AGENT: &str = concat!("flm/", env!("CARGO_PKG_VERSION"), " (+https://github.com/phfaist/flm)");

    /// A blocking downloader over a `ureq` agent.
    pub(super) struct NetworkDownloader {
        agent: ureq::Agent,
    }

    impl NetworkDownloader {
        pub(super) fn new() -> NetworkDownloader {
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(60))
                // `ureq` counts the first request among its redirects: six allow five
                // redirects.
                .redirects(6)
                // Refuses an address, and a redirect, that is not `https`.
                .https_only(true)
                // Off by default in `ureq` 2: the proxy variables of the environment.
                .try_proxy_from_env(true)
                .user_agent(USER_AGENT)
                .build();
            NetworkDownloader { agent }
        }
    }

    impl Downloader for NetworkDownloader {
        fn get(&self, url: &str) -> Result<Response, String> {
            let response = match self.agent.get(url).call() {
                Ok(response) => response,
                Err(ureq::Error::Status(status, response)) => {
                    return Err(format!("the server answered the status {status} ({})", response.status_text()));
                }
                Err(ureq::Error::Transport(transport)) => return Err(transport.to_string()),
            };
            // `ureq` answers a status below 400 as a success, a redirect it did not follow
            // among them.
            if !(200..300).contains(&response.status()) {
                return Err(format!("the server answered the status {} ({})", response.status(), response.status_text()));
            }
            let media_type = response
                .header("content-type")
                .map(|stated| stated.split(';').next().unwrap_or("").trim().to_ascii_lowercase())
                .filter(|stated| !stated.is_empty());
            let mut bytes = Vec::new();
            response
                .into_reader()
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("the answer cannot be read: {error}"))?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err(String::from("the file is larger than 32 MiB"));
            }
            Ok(Response { bytes, media_type })
        }
    }
}
