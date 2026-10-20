//! Mirrors [`flm_core::lang`]: declarations, the text readers, the rewrites,
//! the names of the structural elements, and the built language and its
//! diagnostics, which a late-initialization hook is lent.

pub mod mint;
pub mod rewrites;
pub mod structural;
pub mod text;

use crate::api::techy::error::Diagnostic;
use crate::runtime::ExtError;
use crate::sealed::{BuiltLangCore, Sealed};

/// Mirrors [`BuiltLang`](flm_core::lang::BuiltLang): the finished language
/// of a registry, lent to a late-initialization hook
/// ([`LateInit`](crate::api::core::registry::LateInit)) and answered by
/// [`Lowerer::lang`](crate::api::core::lowering::Lowerer::lang) and
/// [`Expander::lang`](crate::api::lang::rewrites::userdefs::Expander::lang).
///
/// What differs: the façade has no method of its own; it is what the
/// extension API's functions over the built language take
/// ([`TemplateLanguage::new`](crate::api::lang::rewrites::userdefs::TemplateLanguage::new),
/// [`compile_template_with`](crate::api::lang::rewrites::userdefs::compile_template_with)),
/// and it carries the durable table of the running handler's feature, which
/// those functions fill. It is lent for one call: the registry owns the
/// language. The trait is sealed: the adapter alone implements it.
pub trait BuiltLang: BuiltLangCore {}

/// Mirrors [`FlmDiagnostics`](flm_core::lang::FlmDiagnostics): the
/// collection a late-initialization hook writes its problems to
/// ([`LateInit`](crate::api::core::registry::LateInit)).
///
/// What differs: one method, [`push`](FlmDiagnostics::push), which takes the
/// extension API's [`Diagnostic`]. The trait is sealed: the adapter alone
/// implements it.
pub trait FlmDiagnostics: Sealed {
    /// Mirrors [`FlmDiagnostics::push`](flm_core::lang::FlmDiagnostics::push):
    /// adds `diagnostic` to the registry's build diagnostics. What differs:
    /// the diagnostic is the extension API's; its identifier must begin with
    /// the extension's stem and a dot, and one that names no node stands
    /// nowhere in the document (an empty span over an empty source).
    ///
    /// # Errors
    ///
    /// The identifier does not begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), or the diagnostic's data cannot
    /// cross ([`ExtError::Value`]).
    fn push(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError>;
}

fn _object_safe(_: &dyn BuiltLang, _: &dyn FlmDiagnostics) {}
