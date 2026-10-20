//! Mirrors [`flm_core::stdcore::backends::latex::packages`]: the service
//! slot of the style file's package requests, the identifiers of the
//! family's standard packages, and the request a feature contributes.
//!
//! A rule or a preamble piece requires a package by its identifier: one of
//! the family's own packages by [`package_id`]`(name)`; a package the family
//! does not declare, by the `id` of the request a feature contributes as
//! data `{ id, package, options: [text], docs }` to [`SLOT`] with
//! [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot).
//! An extension's own request has an `id` that begins with the extension's
//! stem and a dot (`ex.foo`), and its rules require that `id`;
//! [`package_id`] names the family's own packages only.
//!
//! What differs from `flm-core`: the names are `&str` constants and the
//! identifier a `String`. Left out: the requests' merging, the engines and
//! the requests derived from the Unicode-to-LaTeX table, which are the
//! backend's own.

use flm_core::stdcore::backends::latex::packages as core_packages;

/// Mirrors [`SLOT`](flm_core::stdcore::backends::latex::packages::SLOT): the
/// service slot of the package requests, `flm.latex.packages`, which takes a
/// [`LatexPackageDef`]. Nothing differs.
pub const SLOT: &str = core_packages::SLOT;

/// Mirrors [`ID_PREFIX`](flm_core::stdcore::backends::latex::packages::ID_PREFIX):
/// the prefix of a package's identifier, `flm.latex.pkg.`. Nothing differs.
pub const ID_PREFIX: &str = core_packages::ID_PREFIX;

/// Mirrors [`package_id`](flm_core::stdcore::backends::latex::packages::package_id):
/// the identifier of the package `name`, [`ID_PREFIX`] followed by `name`.
/// What differs: the identifier is a `String`.
pub fn package_id(name: &str) -> String {
    core_packages::package_id(name).to_string()
}

/// Mirrors [`AMSMATH`](flm_core::stdcore::backends::latex::packages::AMSMATH): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const AMSMATH: &str = core_packages::AMSMATH;

/// Mirrors [`ZREF_CLEVER`](flm_core::stdcore::backends::latex::packages::ZREF_CLEVER): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const ZREF_CLEVER: &str = core_packages::ZREF_CLEVER;

/// Mirrors [`TEXTCASE`](flm_core::stdcore::backends::latex::packages::TEXTCASE): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const TEXTCASE: &str = core_packages::TEXTCASE;

/// Mirrors [`HYPERREF`](flm_core::stdcore::backends::latex::packages::HYPERREF): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const HYPERREF: &str = core_packages::HYPERREF;

/// Mirrors [`FVEXTRA`](flm_core::stdcore::backends::latex::packages::FVEXTRA): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const FVEXTRA: &str = core_packages::FVEXTRA;

/// Mirrors [`NEWUNICODECHAR`](flm_core::stdcore::backends::latex::packages::NEWUNICODECHAR): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const NEWUNICODECHAR: &str = core_packages::NEWUNICODECHAR;

/// Mirrors [`ENUMITEM`](flm_core::stdcore::backends::latex::packages::ENUMITEM): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const ENUMITEM: &str = core_packages::ENUMITEM;

/// Mirrors [`AMSTHM`](flm_core::stdcore::backends::latex::packages::AMSTHM): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const AMSTHM: &str = core_packages::AMSTHM;

/// Mirrors [`PHFTHM`](flm_core::stdcore::backends::latex::packages::PHFTHM): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const PHFTHM: &str = core_packages::PHFTHM;

/// Mirrors [`PHFCC`](flm_core::stdcore::backends::latex::packages::PHFCC): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const PHFCC: &str = core_packages::PHFCC;

/// Mirrors [`GRAPHICX`](flm_core::stdcore::backends::latex::packages::GRAPHICX): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const GRAPHICX: &str = core_packages::GRAPHICX;

/// Mirrors [`ADJUSTBOX`](flm_core::stdcore::backends::latex::packages::ADJUSTBOX): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const ADJUSTBOX: &str = core_packages::ADJUSTBOX;

/// Mirrors [`CAPTION`](flm_core::stdcore::backends::latex::packages::CAPTION): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const CAPTION: &str = core_packages::CAPTION;

/// Mirrors [`TABULARRAY`](flm_core::stdcore::backends::latex::packages::TABULARRAY): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const TABULARRAY: &str = core_packages::TABULARRAY;

/// Mirrors [`XCOLOR`](flm_core::stdcore::backends::latex::packages::XCOLOR): the
/// identifier of one of the family's standard packages. Nothing differs.
pub const XCOLOR: &str = core_packages::XCOLOR;

/// Mirrors `flm-core`'s [`LatexPackageDef`]:
/// a package request. Nothing differs: it is `flm-core`'s own, re-exported;
/// a binding gives it as data in its serde form `{ id, package, options,
/// docs }`.
pub use flm_core::stdcore::backends::latex::packages::LatexPackageDef;
