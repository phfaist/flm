//! Mirrors [`flm_core::lang::structural`]: the names of the structural
//! elements, the elements of the node shapes no feature declares (text,
//! comments, containers, groups, paragraphs, …). The module holds names only.
//! What differs: the element values ([`StructuralElements`](flm_core::lang::structural::StructuralElements)
//! and [`StructuralElementSpecs`](flm_core::lang::structural::StructuralElementSpecs))
//! are not mirrored.
//!
//! A backend lists the structural elements its emit stage writes without a
//! lowering rule in its native set
//! ([`BackendDef::native`](crate::api::core::registry::BackendDef::native));
//! the text backend writes five of them: [`TEXT`], [`GROUP`], [`COMMENT`],
//! [`CONTAINER`] and [`PARBREAK`].

use flm_core::lang::structural as names;

/// Mirrors [`TEXT`](names::TEXT): `flm.text`, the element of a run of characters.
pub const TEXT: &str = names::TEXT;

/// Mirrors [`COMMENT`](names::COMMENT): `flm.comment`, the element of a source comment.
pub const COMMENT: &str = names::COMMENT;

/// Mirrors [`CONTAINER`](names::CONTAINER): `flm.container`, the element of a list node (a tree's root, a body).
pub const CONTAINER: &str = names::CONTAINER;

/// Mirrors [`GROUP`](names::GROUP): `flm.group`, the element of a content group `{…}`.
pub const GROUP: &str = names::GROUP;

/// Mirrors [`MATH`](names::MATH): `flm.math`, the element of a mathematics group.
pub const MATH: &str = names::MATH;

/// Mirrors [`VERBATIM`](names::VERBATIM): `flm.verbatim`, the element of a verbatim group.
pub const VERBATIM: &str = names::VERBATIM;

/// Mirrors [`PARAGRAPH`](names::PARAGRAPH): `flm.paragraph`, the element of a paragraph.
pub const PARAGRAPH: &str = names::PARAGRAPH;

/// Mirrors [`PARBREAK`](names::PARBREAK): `flm.parbreak`, the element of a paragraph break.
pub const PARBREAK: &str = names::PARBREAK;

/// Mirrors [`PLACEHOLDER`](names::PLACEHOLDER): `flm.placeholder`, the element of a template's placeholder.
pub const PLACEHOLDER: &str = names::PLACEHOLDER;

/// Mirrors [`UNKNOWN_MACRO`](names::UNKNOWN_MACRO): `flm.unknown.macro`, the element of a macro no feature defines.
pub const UNKNOWN_MACRO: &str = names::UNKNOWN_MACRO;

/// Mirrors [`UNKNOWN_ENVIRONMENT`](names::UNKNOWN_ENVIRONMENT): `flm.unknown.environment`, the element of an environment no feature defines.
pub const UNKNOWN_ENVIRONMENT: &str = names::UNKNOWN_ENVIRONMENT;

/// Mirrors [`NAMES`](names::NAMES): every structural element name, in the fixed order of the set.
pub const NAMES: [&str; 11] = names::NAMES;
