//! No `flm-core` counterpart: the host services a handler reaches, lent by
//! [`StageCx::host`](crate::api::core::run::StageCx::host),
//! [`ProviderCx::host`](crate::api::stdcore::features::citations::ProviderCx::host)
//! and [`TemplateCx::host`](crate::api::stdcore::features::compose::TemplateCx::host)
//! as a handle valid for the call — the `files` service as [`Files`], any other as a
//! [`HostService`].
//!
//! A host service is this crate's own concept ([`crate::host`]): a host
//! offers named services on the loader, a package declares the ones its
//! handlers call (its `host` list), and a handler reaches a declared one
//! alone.

use serde_json::Value;

use crate::runtime::ExtError;
use crate::sealed::Sealed;

/// No `flm-core` counterpart: a host service the package declared, as a
/// handler calls it ([`crate::host::HostService`] behind it).
///
/// The trait is sealed: the adapter alone implements it.
pub trait HostService: Sealed {
    /// The service's name, as the package declares it.
    fn name(&self) -> String;

    /// The method `method` of the service with the arguments `args`; the
    /// service sees which extension and package call it.
    ///
    /// # Errors
    ///
    /// The service's own failure ([`ExtError::Host`], its text the
    /// service's); an answer of bytes, which a script cannot receive
    /// ([`ExtError::Argument`]); an answer that cannot cross
    /// ([`ExtError::Value`]).
    fn call(&self, method: &str, args: Value) -> Result<Value, ExtError>;
}

/// No `flm-core` counterpart: one of the two roots of the `files` service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilesScope {
    /// The extension's own files (`"extension"`).
    Extension,
    /// The document's folder (`"document"`).
    Document,
}

/// No `flm-core` counterpart: the `files` host service, as a handler reads
/// through it ([`crate::host::files`] states the protocol).
///
/// The trait is sealed: the adapter alone implements it.
pub trait Files: Sealed {
    /// The text of the file at `path` in the root `scope`, or `None` (`null`
    /// to a script) when nothing is there. The `extension` scope reads the extension's own
    /// files (a single-file extension has none); the `document` scope reads
    /// through the host, which watches the file for a live preview.
    ///
    /// # Errors
    ///
    /// A path that leaves its root, is absolute or holds `\`, or a file that
    /// cannot be read or is not UTF-8 text ([`ExtError::Host`]).
    fn read_text(&self, scope: FilesScope, path: &str) -> Result<Option<String>, ExtError>;
}

/// No `flm-core` counterpart: what [`StageCx::host`](crate::api::core::run::StageCx::host)
/// (and the `host` of a citation provider's or a template module's context)
/// answers — the `files` service as [`Files`], any other service as a
/// [`HostService`]. Across a binding, a handle of kind `files` or
/// `host_service`.
pub enum HostHandle {
    /// The `files` service.
    Files(Box<dyn Files>),
    /// Any other service.
    Service(Box<dyn HostService>),
}
