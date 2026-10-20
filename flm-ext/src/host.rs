//! The host services contract: named services a host offers to extensions
//! ([`HostService`]), which a package declares in its report's `host` list
//! ([`PackageReport::host`](crate::engine::PackageReport::host)) and a handler
//! reaches through a handle
//! ([`StageCx::host`](crate::api::core::run::StageCx::host)).
//!
//! A host service is how a handler reaches what the sandbox keeps out (a
//! file of the document's folder, in this version). It is declared by the
//! package, never ambient: a host registers its services on the loader
//! ([`ExtensionLoader::with_host_service`](crate::loader::ExtensionLoader::with_host_service)),
//! the loader refuses a package declaring a service the host does not offer,
//! the declaration enters the extension's digest, and a handler reaches a
//! declared service alone. No `flm-core` counterpart: a host service is this
//! crate's own concept.
//!
//! The one service whose protocol this crate states is [`files`] (the
//! submodule); a host may offer others under names of its own (the `flm`
//! program also offers `programs`, which finds and runs a program installed
//! on the machine). A service that runs programs lets an extension do what
//! the user running the host may do (see [Trust](crate#trust)).
//!
//! It also holds where an extension's `console` output goes
//! ([`ConsoleLevel`], [`ConsoleSink`]): every binding's host takes the same
//! sink.

use std::fmt;
use std::sync::Arc;

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
    /// `caller` names.
    ///
    /// `caller` says which extension and package call: a service may name it
    /// in its messages or decide by it; the [`files`] service ignores it.
    ///
    /// # Errors
    ///
    /// The service's own failure, as text for a person
    /// ([`HostServiceError`]): an unknown method, arguments of the wrong
    /// shape, a refused request, a failed read. The handler receives it as
    /// [`ExtError::Host`](crate::ExtError::Host).
    fn call(&self, caller: &HostCaller, method: &str, args: &Value) -> Result<HostAnswer, HostServiceError>;
}

/// Who calls a host service: the extension, the package within it, and the
/// bytes the loader computes the extension's digest from. No `flm-core`
/// counterpart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCaller {
    /// The extension: its identity (the source's string for it) and its stem
    /// (the name messages use).
    pub extension: ExtensionIdentity,
    /// The package's name within the extension (`_` is the default package).
    pub package: String,
    /// The bytes the loader feeds into the registry digest for the extension,
    /// shared by every caller of the extension (no copy per call): its
    /// manifest's canonical JSON, each package's `host` list, and the
    /// engine's bytes (its modules, its version). They change when the
    /// extension's contents change; no hash function is offered, so a host
    /// that keys a decision on the extension's contents hashes them itself.
    pub digest_input: Arc<[u8]>,
}

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

/// The protocol of the `files` service: the text or the bytes of a file of
/// the extension's folder or of the document's folder.
///
/// | Method | Arguments | Answer |
/// |---|---|---|
/// | `read_text` | `{ "scope": "extension" \| "document", "path": text }` | `Data(text)`, or `Data(null)` when nothing is at the path |
/// | `read_bytes` | the same | `Bytes`, or `Data(null)` when nothing is at the path |
///
/// The two scopes are the service's two roots:
///
/// - `extension`: the extension's own files. This crate serves the scope
///   itself, from the files the host's source gave the loader for the
///   extension ([`ExtensionFiles`](crate::source::ExtensionFiles)): the path
///   normalized ([`paths::normalize`](crate::paths::normalize), which refuses
///   a climb, an absolute path and a `\`), then read. A single-file extension
///   has no folder and answers `null` to every path. The host's object never
///   receives a call of this scope.
/// - `document`: the document's folder, served by the host's object, which
///   refuses a path leaving that folder and reads through the build's file
///   reader (so that a live preview watches the file).
///
/// A package declares `files` in its `host` list for either scope. `read_text`
/// is offered to scripts; `read_bytes` to a Rust caller of the adapter only.
pub mod files {
    /// The service's name.
    pub const NAME: &str = "files";
    /// The method answering a file's text.
    pub const READ_TEXT: &str = "read_text";
    /// The method answering a file's bytes.
    pub const READ_BYTES: &str = "read_bytes";
    /// The scope of the extension's own files.
    pub const EXTENSION: &str = "extension";
    /// The scope of the document's folder.
    pub const DOCUMENT: &str = "document";
}

/// The level of one line of `console` output: the method the script called
/// (`console.log`, `console.info`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    /// `console.log`.
    Log,
    /// `console.info`.
    Info,
    /// `console.debug`.
    Debug,
    /// `console.warn`.
    Warn,
    /// `console.error`.
    Error,
}

impl ConsoleLevel {
    /// The level named by the `console` method `name`; an unknown name is
    /// [`ConsoleLevel::Log`].
    #[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
    pub(crate) fn from_name(name: &str) -> ConsoleLevel {
        match name {
            "info" => ConsoleLevel::Info,
            "debug" => ConsoleLevel::Debug,
            "warn" => ConsoleLevel::Warn,
            "error" => ConsoleLevel::Error,
            _ => ConsoleLevel::Log,
        }
    }
}

impl fmt::Display for ConsoleLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ConsoleLevel::Log => "log",
            ConsoleLevel::Info => "info",
            ConsoleLevel::Debug => "debug",
            ConsoleLevel::Warn => "warn",
            ConsoleLevel::Error => "error",
        })
    }
}

/// Where `console` output goes: a level and the joined, stringified arguments.
pub type ConsoleSink = Arc<dyn Fn(ConsoleLevel, &str) + Send + Sync>;
