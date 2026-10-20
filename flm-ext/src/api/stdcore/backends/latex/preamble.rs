//! Mirrors [`flm_core::stdcore::backends::latex::preamble`]: the service
//! slot of the style file's definition pieces, the identifiers of the
//! family's own pieces, and the piece a feature contributes.
//!
//! A piece is contributed as data `{ name, content, docs?, requires?: [id]
//! }` to [`SLOT`] with
//! [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot);
//! its identifier is [`piece_id`]`(name)`, which a rule requires while it
//! lowers ([`Lowerer::require`](crate::api::core::lowering::Lowerer::require)),
//! or declares as required when it is selected
//! ([`LoweringDecl::selection_requires`](crate::api::core::lowering::LoweringDecl::selection_requires)),
//! so that the recomposed LaTeX backend writes the piece into the style file.
//!
//! What differs from `flm-core`: the names are `&str` constants and the
//! identifier a `String`. Left out: the pieces' builders (`refs_piece`, …),
//! the commands' names and the hooks' texts, which are the family's own.

use flm_core::stdcore::backends::latex::preamble as core_preamble;

/// Mirrors [`SLOT`](flm_core::stdcore::backends::latex::preamble::SLOT): the
/// service slot of the definition pieces, `flm.latex.preamble`, which takes
/// a [`LatexPreambleDef`]. Nothing differs.
pub const SLOT: &str = core_preamble::SLOT;

/// Mirrors [`ID_PREFIX`](flm_core::stdcore::backends::latex::preamble::ID_PREFIX):
/// the prefix of a piece's identifier, `flm.latex.def.`. Nothing differs.
pub const ID_PREFIX: &str = core_preamble::ID_PREFIX;

/// Mirrors [`piece_id`](flm_core::stdcore::backends::latex::preamble::piece_id):
/// the identifier of the piece `name`, [`ID_PREFIX`] followed by `name`.
/// What differs: the identifier is a `String`.
pub fn piece_id(name: &str) -> String {
    core_preamble::piece_id(name).to_string()
}

/// Mirrors [`REFS`](flm_core::stdcore::backends::latex::preamble::REFS): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const REFS: &str = core_preamble::REFS;

/// Mirrors [`HYPERREF`](flm_core::stdcore::backends::latex::preamble::HYPERREF): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const HYPERREF: &str = core_preamble::HYPERREF;

/// Mirrors [`LABEL_DISPLAY_TEXT`](flm_core::stdcore::backends::latex::preamble::LABEL_DISPLAY_TEXT): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const LABEL_DISPLAY_TEXT: &str = core_preamble::LABEL_DISPLAY_TEXT;

/// Mirrors [`ZREF_CLEVER_TYPE`](flm_core::stdcore::backends::latex::preamble::ZREF_CLEVER_TYPE): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const ZREF_CLEVER_TYPE: &str = core_preamble::ZREF_CLEVER_TYPE;

/// Mirrors [`LABELS`](flm_core::stdcore::backends::latex::preamble::LABELS): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const LABELS: &str = core_preamble::LABELS;

/// Mirrors [`HEADING`](flm_core::stdcore::backends::latex::preamble::HEADING): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const HEADING: &str = core_preamble::HEADING;

/// Mirrors [`VERBATIM`](flm_core::stdcore::backends::latex::preamble::VERBATIM): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const VERBATIM: &str = core_preamble::VERBATIM;

/// Mirrors [`MATH_ENVIRONMENT`](flm_core::stdcore::backends::latex::preamble::MATH_ENVIRONMENT): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const MATH_ENVIRONMENT: &str = core_preamble::MATH_ENVIRONMENT;

/// Mirrors [`IF_BLANK`](flm_core::stdcore::backends::latex::preamble::IF_BLANK): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const IF_BLANK: &str = core_preamble::IF_BLANK;

/// Mirrors [`USER_ARGS`](flm_core::stdcore::backends::latex::preamble::USER_ARGS): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const USER_ARGS: &str = core_preamble::USER_ARGS;

/// Mirrors [`TAB_WIDTH`](flm_core::stdcore::backends::latex::preamble::TAB_WIDTH): the
/// identifier of one of the family's own pieces. Nothing differs.
pub const TAB_WIDTH: &str = core_preamble::TAB_WIDTH;

/// Mirrors `flm-core`'s [`LatexPreambleDef`]:
/// a definition piece of the style file. Nothing differs: it is
/// `flm-core`'s own, re-exported; a binding gives it as data `{ name,
/// content, docs?, requires? }`, read through its `new` and `requires`.
pub use flm_core::stdcore::backends::latex::preamble::LatexPreambleDef;
