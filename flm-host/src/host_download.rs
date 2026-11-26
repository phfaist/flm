//! The host service `download`: the file at an `https` address, for an extension's
//! handlers.
//!
//! The service's name and its method are those of the extension interface
//! ([`flm_ext::host::download`]).
//!
//! A package declares the service in its `host` list; a handler calls it as
//! `cx.host("download").call("get", { url })`. The loader of a preparation offers it when the
//! program gave a downloader ([`Request::downloads`](crate::session::Request::downloads),
//! [`packages::loader`](crate::packages::loader)); without one, the loader refuses a
//! package that declares it, as it refuses any service the program does not offer.
//!
//! # The method `get`
//!
//! `get { url }` downloads the file at `url`, an address that begins with `https://`,
//! through the program's downloads ([`Downloads::get`]), by the rules of
//! [`crate::download`]: the file is kept in the program's cache, shared by documents. It
//! answers:
//!
//! ```json
//! { "content": { "text": "…" }, "media_type": "text/css", "warning": "…" }
//! ```
//!
//! - `content` is `{ "text" }` when the bytes are UTF-8 text and `{ "base64" }` otherwise,
//!   in the standard alphabet with padding, as the host services `cache` and `programs`
//!   answer a content;
//! - `media_type` is the media type that the server stated, without its parameters, or
//!   `null` when it stated none;
//! - `warning` is there only when the file is a kept copy past its hard expiry that could
//!   not be downloaded again: the handler reports it as it sees fit. A file that a template
//!   module ships from an address is downloaded through this service, and its warning is
//!   reported as a warning of the compose template.
//!
//! A request under way runs to its end when the build is cancelled.
//!
//! # Failures
//!
//! An unknown method and an argument of the wrong shape are refused; so are an address that
//! does not begin with `https://` and a failed request with no kept copy, with the text of
//! [`DownloadError`](crate::download::DownloadError).

use flm_core::progress::Scope;
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError, download};
use serde_json::{Map, Value};

use crate::cache::ServiceCaches;
use crate::download::Downloads;
use crate::host_cache::content_value;

/// The host service `download` over the program's downloads. See the
/// [module documentation](self).
#[derive(Clone, Debug)]
pub struct DownloadService {
    /// The program's downloads.
    downloads: Downloads,
    /// The handle of the registry's caches, through which a download opens its cache.
    caches: ServiceCaches,
}

impl DownloadService {
    /// Returns the service downloading through `downloads` ([`Downloads::get`]), which
    /// keeps the files in the caches of `caches`.
    pub fn new(downloads: Downloads, caches: ServiceCaches) -> DownloadService {
        DownloadService { downloads, caches }
    }
}

impl HostService for DownloadService {
    /// `download`.
    fn name(&self) -> &str {
        download::NAME
    }

    /// `get { url }`.
    fn call(&self, _caller: &HostCaller, method: &str, args: &Value, _progress: &Scope) -> Result<HostAnswer, HostServiceError> {
        if method != download::GET {
            return Err(refused(format!("the method of the host service ‘download’ must be ‘get’, not ‘{method}’")));
        }
        let url = match args.get("url") {
            None => return Err(refused(String::from("missing argument ‘url’"))),
            Some(value) => value.as_str().ok_or_else(|| refused(String::from("‘url’ must be a text")))?,
        };
        let downloaded = self.downloads.get(&self.caches, url).map_err(|error| refused(error.to_string()))?;
        let mut answer = Map::new();
        answer.insert(String::from("content"), content_value(downloaded.bytes));
        answer.insert(String::from("media_type"), downloaded.media_type.map_or(Value::Null, Value::String));
        if let Some(warning) = downloaded.warning {
            answer.insert(String::from("warning"), Value::String(warning));
        }
        Ok(HostAnswer::Data(Value::Object(answer)))
    }
}

/// A refusal with the text `detail`.
fn refused(detail: String) -> HostServiceError {
    HostServiceError { detail }
}
