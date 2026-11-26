//! The host services contract: named services a host offers to extensions
//! ([`HostService`]), which a package declares in its report's `host` list
//! ([`PackageReport::host`](crate::engine::PackageReport::host)) and a handler
//! reaches through a handle
//! ([`StageCx::host`](crate::api::core::run::StageCx::host)).
//!
//! A host service is how a handler reaches what the sandbox keeps out (a
//! file of the document's folder, or a file at an `https` address). It is declared by the
//! package, never ambient: a host registers its services on the loader
//! ([`ExtensionLoader::with_host_service`](crate::loader::ExtensionLoader::with_host_service)),
//! the loader refuses a package declaring a service the host does not offer,
//! the declaration enters the extension's digest, and a handler reaches a
//! declared service alone. No `flm-core` counterpart: a host service is this
//! crate's own concept.
//!
//! This crate states the protocols of two services, each in its submodule:
//! [`files`] (a file at a qualified path) and [`download`] (the file at an
//! `https` address, through which a template module ships such a file). A host
//! may offer others under names of its own (the `flm`
//! program also offers `cache`, which stores and reads the values of the
//! caches a package declares, and `programs`, which finds and runs a program
//! installed on the machine). A service that runs programs lets an extension do what
//! the user running the host may do (see [Trust](crate#trust)).
//!
//! # Caches
//!
//! A package also declares the caches its handlers use, beside its `host`
//! list, under the key `caches` ([`CacheDeclaration`]); the declarations are
//! read with the package's report, reach every service the package calls
//! ([`HostCaller::caches`]), and stay out of the extension's digest. This
//! crate keeps no cache: a host service that keeps caches for extensions
//! checks a package's declarations when the package loads
//! ([`HostService::check_package`]) and serves them.
//!
//! # Console output
//!
//! What an extension writes on its `console` goes to the log sink that the
//! engine was given ([`flm_core::progress::LogSink`]): the host of each
//! binding (`JsHost::with_log_sink`, `WasmHost::with_log_sink`,
//! `NativeEngine::with_log_sink`) takes one. A line is a
//! [`flm_core::progress::LogRecord`] whose source is the extension's stem,
//! the name the messages give it, and whose level follows the method the
//! extension called:
//!
//! | Method | Level |
//! |---|---|
//! | `console.error` | [`Error`](flm_core::progress::LogLevel::Error) |
//! | `console.warn` | [`Warn`](flm_core::progress::LogLevel::Warn) |
//! | `console.log`, `console.info`, `console.debug` | [`Info`](flm_core::progress::LogLevel::Info) |
//!
//! An engine given no sink writes its extensions' console output nowhere.
//! The console does not go through the progress sheet of a build: an
//! extension also writes while it is loaded and when its instance is
//! created, outside any build.

use std::collections::BTreeMap;
use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

#[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
use flm_core::progress::{LogLevel, LogRecord, LogSink};
use flm_core::progress::Scope;
use serde_json::Value;

use crate::engine::ExtensionIdentity;

/// A service the host offers to extensions by name, called with data and
/// answering data (or bytes, for a Rust caller of the adapter). No `flm-core`
/// counterpart: a host service is how a handler reaches what the sandbox
/// keeps out, declared by the package, never ambient.
///
/// It is shared by every thread a registry's stages run on, so it is `Send +
/// Sync`; a call may come from any of them.
pub trait HostService: Send + Sync {
    /// The service's name, as a package declares it (`files`).
    fn name(&self) -> &str;

    /// The method `method` with the arguments `args`, called by the package
    /// `caller` names, from a handler call whose scope on the progress sheet
    /// of the build is `progress`.
    ///
    /// `caller` says which extension and package call: a service may name it
    /// in its messages or decide by it; the [`files`] service ignores it.
    /// `progress` is the scope of the stage or the citation provider whose
    /// handler makes the call: a service that does slow work declares its
    /// rows under it and asks it whether the build was asked to stop. A call
    /// made from a context that has no scope (a template module's) receives
    /// [`Scope::none()`].
    ///
    /// # Errors
    ///
    /// The service's own failure, as text for a person
    /// ([`HostServiceError`]): an unknown method, arguments of the wrong
    /// shape, a refused request, a failed read. The handler receives it as
    /// [`ExtError::Host`](crate::ExtError::Host).
    fn call(&self, caller: &HostCaller, method: &str, args: &Value, progress: &Scope) -> Result<HostAnswer, HostServiceError>;

    /// Checks the package `caller` names, which declares this service in its
    /// `host` list, when the [loader](crate::loader::ExtensionLoader) loads
    /// it: a service may refuse what the package declares for it (its
    /// [caches](HostCaller::caches), for a service that keeps them). The
    /// default accepts every package.
    ///
    /// # Errors
    ///
    /// The refusal, as text for a person ([`HostServiceError`]); the loader
    /// reports it as it reports a service the program does not offer, and
    /// the extension does not load.
    fn check_package(&self, caller: &HostCaller) -> Result<(), HostServiceError> {
        let _ = caller;
        Ok(())
    }
}

/// Who calls a host service: the extension, the package within it, the
/// caches the package declares, and the bytes the loader computes the
/// extension's digest from. No `flm-core` counterpart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCaller {
    /// The extension: its identity (the source's string for it) and its stem
    /// (the name messages use).
    pub extension: ExtensionIdentity,
    /// The package's name within the extension (`_` is the default package).
    pub package: String,
    /// The caches the package declares
    /// ([`PackageReport::caches`](crate::engine::PackageReport::caches)), by
    /// name; empty when it declares none.
    pub caches: CacheDeclarations,
    /// The bytes the loader feeds into the registry digest for the extension,
    /// shared by every caller of the extension (no copy per call): its
    /// manifest's canonical JSON, each package's `host` list, and the
    /// engine's bytes (its modules, its version), not the caches the packages
    /// declare (a cache changes no output). They change when the
    /// extension's contents change; no hash function is offered, so a host
    /// that keys a decision on the extension's contents hashes them itself.
    pub digest_input: Arc<[u8]>,
}

/// The caches a package declares, by name: the package object's key `caches`
/// (the guest client's [`Package::caches`](crate::guest::Package::caches)).
pub type CacheDeclarations = BTreeMap<String, CacheDeclaration>;

/// A cache that a package declares for its handlers: a values cache or a
/// folder cache, with its version, its lifetimes, its cost and its scope. No
/// `flm-core` counterpart: a host that keeps caches gives each declaration a
/// cache of its own, named after the extension's stem and the declared name,
/// and selected by the extension's identity; this crate keeps no cache.
///
/// A package object writes it as a map, under a name of segments of
/// lowercase ASCII letters, digits, `-` and `_` joined by `.`
/// (`typeset.mathml`):
///
/// | Key | Value |
/// |---|---|
/// | `version` | a whole number, at least 1; another version has data of its own |
/// | `values` | a values cache: `engine` (`jsonl`, `files` or `sqlite`; `sqlite` when absent) and `blobs` (`false` when absent) |
/// | `folder` | a folder cache, an empty map; one build at a time uses the folder |
/// | `unused_s` | the values are stable: the time, in seconds, after its last use at which a value, or the folder, is removed; `unlimited` for never |
/// | `lifetime_s` | values only: the values are volatile, and each is due for a refresh after this time, in seconds |
/// | `expiry` | with `lifetime_s` only: the expiry policy, a map of `stale_percent`, `jitter_percent` and `grace_s` ([`CacheExpiry`]), each optional |
/// | `cost_ms` | the estimate of the time, in milliseconds, needed to produce one value, or the folder's content, again; a whole number, at least 1 |
/// | `scope` | `shared` (one instance for all documents) or `document` (one per document) |
///
/// Exactly one of `values` and `folder` is given, and exactly one of
/// `unused_s` and `lifetime_s`; a folder takes `unused_s`. A key whose value
/// is `null` is read as absent. A declaration of another shape fails the
/// extension's load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDeclaration {
    /// The version, at least 1.
    pub version: NonZeroU32,
    /// A values cache or a folder cache, with its lifetimes.
    pub kind: CacheKind,
    /// The estimate of the time needed to produce one value, or the folder's
    /// content, again, in milliseconds (`cost_ms`), at least 1.
    pub cost_ms: NonZeroU64,
    /// One instance for all documents, or one per document.
    pub scope: CacheScope,
}

/// The kind of a declared cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheKind {
    /// A values cache (`values`).
    Values {
        /// The engine that keeps the values.
        engine: CacheEngine,
        /// Whether the values have blobs.
        blobs: bool,
        /// Whether the values are stable or volatile, with their lifetime.
        sort: CacheSort,
    },
    /// A folder cache (`folder`), which one build at a time uses.
    Folder {
        /// The time after its last use at which the folder is removed
        /// (`unused_s`).
        unused: CacheTime,
    },
}

/// The engine that keeps the values of a declared values cache
/// (`values.engine`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheEngine {
    /// One JSONL file (`jsonl`), every value in memory; values without blobs
    /// only.
    Jsonl,
    /// Files per value (`files`).
    Files,
    /// One SQLite database (`sqlite`), when the host offers it.
    Sqlite,
}

/// Whether the values of a declared values cache are stable or volatile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheSort {
    /// Stable values (`unused_s`): a value would be the same if produced
    /// again from its key.
    Stable {
        /// The time after its last use at which a value is removed.
        unused: CacheTime,
    },
    /// Volatile values (`lifetime_s`): a value could differ if produced
    /// again.
    Volatile {
        /// The time after which a value is due for a refresh, in seconds.
        lifetime_s: u64,
        /// The expiry policy (`expiry`).
        expiry: CacheExpiry,
    },
}

/// The expiry policy of volatile values (`expiry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheExpiry {
    /// The part of its lifetime after which a refresh of a value is
    /// preferred (`stale_percent`; 80 when absent).
    pub stale_percent: CachePercent,
    /// The largest shift of a value's hard expiry, which spreads the
    /// refreshes of values stored together, as a part of the lifetime
    /// (`jitter_percent`; 15 when absent).
    pub jitter_percent: CachePercent,
    /// The time after its hard expiry during which a value is kept, for use
    /// when it cannot be produced again (`grace_s`; 14 days when absent).
    pub grace: CacheTime,
}

impl Default for CacheExpiry {
    /// The policy of an `expiry` that gives none of its keys: 80, 15, and
    /// 14 days.
    fn default() -> CacheExpiry {
        CacheExpiry {
            stale_percent: CachePercent(80),
            jitter_percent: CachePercent(15),
            grace: CacheTime::Seconds(14 * 24 * 60 * 60),
        }
    }
}

/// A part of a lifetime, in percent: a whole number from 0 to 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CachePercent(u8);

impl CachePercent {
    /// Returns `percent` as a part of a lifetime; `None` above 100. In a
    /// `const` item, `CachePercent::new(50).unwrap()` turns a number above
    /// 100 into an error of the compilation.
    pub const fn new(percent: u8) -> Option<CachePercent> {
        if percent <= 100 { Some(CachePercent(percent)) } else { None }
    }

    /// Returns the number, from 0 to 100.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A time of a cache declaration: whole seconds, or `unlimited`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTime {
    /// This many seconds.
    Seconds(u64),
    /// No limit (`unlimited`).
    Unlimited,
}

/// The scope of a declared cache (`scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheScope {
    /// One instance for all documents (`shared`).
    Shared,
    /// One instance per document (`document`).
    Document,
}

#[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
mod cache_reader;
#[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
pub(crate) use cache_reader::read_caches;
#[cfg(feature = "guest")]
mod cache_writer;
#[cfg(feature = "guest")]
pub(crate) use cache_writer::write_caches;

/// What a host service answers: data, or bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum HostAnswer {
    /// Data (`null` for "nothing there").
    Data(Value),
    /// Bytes: a Rust caller of the adapter receives them; a script cannot
    /// (the handle refuses them).
    Bytes(Vec<u8>),
}

/// A host service's own failure: text for a person, complete (it names what
/// failed and why).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostServiceError {
    /// What failed, as text for a person.
    pub detail: String,
}

impl fmt::Display for HostServiceError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for HostServiceError {}

/// The protocol of the `files` service: the text or the bytes of a file at a
/// qualified path ([`crate::paths`]).
///
/// | Method | Arguments | Answer |
/// |---|---|---|
/// | `read_text` | `{ "path": text }` | `Data(text)`, or `Data(null)` when nothing is at the path |
/// | `read_bytes` | the same | `Bytes`, or `Data(null)` when nothing is at the path |
///
/// A handler gives the path with its root ([`Files::read_text`](crate::api::host::Files::read_text)):
///
/// - `self:` names the extension's own files. This crate serves the root
///   itself, from the files the host's source gave the loader for the
///   extension ([`ExtensionFiles`](crate::source::ExtensionFiles)): the path
///   stays inside the extension's folder, and the files refuse a path that a
///   symbolic link leads outside it ([paths inside an
///   extension](crate::source#paths-inside-an-extension)). A single-file
///   extension has no folder and answers `null` to every path. The host's
///   object never receives this root.
/// - `ext+NAME:` names the files of the extension that the use name `NAME`
///   found. This crate serves the root too, from the files of the extensions
///   that the loader loaded, by use name
///   ([`ExtensionsByName`](crate::paths::ExtensionsByName)). A use name that
///   no `use` entry in effect writes is refused. The host's object never
///   receives this root.
/// - `std:`, `doc:` and `cwd:` are read by the host's object, which receives
///   the path written `ROOT:PATH`, an absolute path as `doc:` followed by
///   itself. It reads a file of the document's folder through the build's
///   file reader (so that a live preview watches the file).
/// - A path with no root is refused. An `https:` address is refused too: it
///   is read through the host service [`download`]. Neither reaches the
///   host's object.
///
/// A package declares `files` in its `host` list for any root. `read_text`
/// is offered to scripts; `read_bytes` to a Rust caller of the adapter only.
pub mod files {
    /// The service's name.
    pub const NAME: &str = "files";
    /// The method answering a file's text.
    pub const READ_TEXT: &str = "read_text";
    /// The method answering a file's bytes.
    pub const READ_BYTES: &str = "read_bytes";
}

/// The protocol of the `download` service: the file at an `https` address.
/// A host offers it when it can reach the network.
///
/// | Method | Arguments | Answer |
/// |---|---|---|
/// | `get` | `{ "url": text }` | `Data({ "content": CONTENT, "media_type": text or null, "warning": text })` |
///
/// `CONTENT` is `{ "text": text }` for UTF-8 bytes and `{ "base64": text }`
/// otherwise (the standard alphabet with padding); `media_type` is the media
/// type that the server stated, or `null`; `warning`, present only when there
/// is one, is a text for a person to read about the file answered (a kept
/// copy that could not be downloaded again). An address that does not begin
/// with `https://`, and a download that gives no file, are refused.
///
/// A template module ships a file at an `https:` address through this
/// service ([`TemplateFile::File`](crate::api::stdlib::features::compose::TemplateFile::File)),
/// when its package declares it in its `host` list.
pub mod download {
    /// The service's name.
    pub const NAME: &str = "download";
    /// The method answering the file at an address.
    pub const GET: &str = "get";
}

/// Where the `console` output of an engine's extensions goes: the engine's
/// log sink, or nowhere.
#[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
#[derive(Clone, Default)]
pub(crate) struct Console(Option<Arc<dyn LogSink>>);

#[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
impl Console {
    /// The console that writes to `sink`.
    pub(crate) fn to_sink(sink: Arc<dyn LogSink>) -> Console {
        Console(Some(sink))
    }

    /// Writes the line `text` of the extension whose stem is `extension`, at
    /// `level`, when the sink wants lines of that level.
    pub(crate) fn write(&self, extension: &str, level: LogLevel, text: &str) {
        if let Some(sink) = self.0.as_ref().filter(|sink| sink.enabled(level)) {
            sink.log(&LogRecord::new(level, extension, text));
        }
    }
}

/// The level of a line written with the `console` method `name`: `error`
/// and `warn` their own, every other method [`LogLevel::Info`].
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn console_level(name: &str) -> LogLevel {
    match name {
        "error" => LogLevel::Error,
        "warn" => LogLevel::Warn,
        _ => LogLevel::Info,
    }
}
