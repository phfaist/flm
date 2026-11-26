//! No `flm-core` counterpart: the host services a handler reaches, lent by
//! [`StageCx::host`](crate::api::core::run::StageCx::host),
//! [`ProviderCx::host`](crate::api::stdlib::features::citations::ProviderCx::host)
//! and [`TemplateCx::host`](crate::api::stdlib::features::compose::TemplateCx::host)
//! as a handle valid for the call — the `files` service as [`Files`], any other as a
//! [`HostService`].
//!
//! A host service is this crate's own concept ([`crate::host`]): a host
//! offers named services on the loader, a package declares the ones its
//! handlers call (its `host` list), and a handler reaches a declared one
//! alone.

use serde_json::Value;

use crate::paths::{own_beside, WrittenPath};
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

/// No `flm-core` counterpart: the `files` host service, as a handler reads
/// through it ([`crate::host::files`] states the protocol).
///
/// The trait is sealed: the adapter alone implements it.
pub trait Files: Sealed {
    /// The text of the file at `path`, or `None` (`null` to a script) when
    /// nothing is there. The path is a qualified path
    /// ([`crate::paths`]): `self:` reads the extension's own files (a
    /// single-file extension has none) and `ext+NAME:` the files of the
    /// extension that the use name `NAME` found; every other root and an
    /// absolute path are read through the host, which watches a file of the
    /// document's folder for a live preview.
    ///
    /// # Errors
    ///
    /// A path with no root, or whose syntax has no meaning
    /// ([`PathError`](crate::paths::PathError)); a use name that no `use`
    /// entry in effect writes
    /// ([`PathErrorKind::UnknownExtension`](crate::paths::PathErrorKind::UnknownExtension));
    /// an `https:` address (read through the host service `download`); a
    /// path of an extension
    /// that leads outside the extension's folder through a symbolic link
    /// ([paths inside an extension](crate::source#paths-inside-an-extension));
    /// a file that cannot be read or is not UTF-8 text; the host's refusal
    /// of a root ([`ExtError::Host`]).
    fn read_text(&self, path: &str) -> Result<Option<String>, ExtError>;
}

/// No `flm-core` counterpart: the qualified path of the file at `relative`
/// beside a module of an extension, under the root `self`: `relative`
/// joined to the folder of `module`, the module's path inside the extension
/// (in JavaScript, the module's `import.meta`): `path(import.meta,
/// "./style.css")` in `templates/html/letter.js` is
/// `self:templates/html/style.css`.
///
/// The engine `js` sets the property `path` of `import.meta` of each module
/// of an extension, which the JavaScript function reads. An extension
/// compiled from Rust has no such function and writes its `self:` paths.
///
/// # Errors
///
/// A `relative` that has a root or is absolute, or whose `..` climbs above
/// the extension's folder ([`ExtError::Argument`]).
pub fn path(module: &str, relative: &str) -> Result<String, ExtError> {
    let argument = |detail: String| ExtError::Argument { method: String::from("path"), detail };
    match WrittenPath::parse(relative) {
        Ok(WrittenPath::Bare(bare)) => own_beside(module, &bare).map_err(|error| argument(error.to_string())),
        Ok(_) => Err(argument(format!(
            "Invalid path ‘{relative}’ for ‘path’: the path must be relative to the folder of the module, with no root"
        ))),
        Err(error) => Err(argument(error.to_string())),
    }
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
