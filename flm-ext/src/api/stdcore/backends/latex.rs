//! Mirrors [`flm_core::stdcore::backends::latex`]: the LaTeX family's
//! construct elements, which a rule on the recomposed LaTeX backend stages,
//! their payloads, and the two service slots of the style file
//! ([`preamble`], [`packages`]).
//!
//! A rule for the backend [`RECOMPOSED_BACKEND`] stages a macro
//! ([`MACRO`]), an environment ([`ENVIRONMENT`]), raw text ([`RAW`]) or a
//! definition written into the style file ([`DEFINITION`]) with
//! [`Lowerer::pres`](crate::api::core::lowering::Lowerer::pres) or
//! [`Lowerer::pres_slots`](crate::api::core::lowering::Lowerer::pres_slots),
//! the payload given as data in its projection form, the arguments' content
//! in slots named [`ARG_SLOT`] in order. A definition the rule's LaTeX needs
//! is a preamble piece ([`preamble::LatexPreambleDef`]) contributed with
//! [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot)
//! to [`preamble::SLOT`] and required by the rule by its identifier
//! ([`preamble::piece_id`]).
//!
//! The payloads' data forms (an optional field may be left out):
//!
//! | Element | Payload |
//! |---|---|
//! | `MACRO` | `{ name, args?: [shape], packed? }` |
//! | `ENVIRONMENT` | `{ name, args?: [shape], verbatim, packed?, body? }` |
//! | `RAW` | `{ text }` |
//! | `DEFINITION` | `{ kind, name, args, … }`, as [`LatexDefinitionPayload`]'s projection writes it |
//!
//! A shape is one of `"star"`, `"optional"`, `"mandatory"`, `"escverb"`
//! ([`LatexArgShape`]).
//!
//! What differs from `flm-core`: the names are `&str` constants; the
//! payloads are `flm-core`'s own types, re-exported, given as data.
//!
//! Left out, each with the reason: `LatexPoint`, `PlaceholderLatexRule`,
//! the style file, the sink and the escaping: the backend's own machinery.

pub mod packages;
pub mod preamble;

use flm_core::stdcore::backends::latex::construct;

/// Mirrors [`MACRO`](flm_core::stdcore::backends::latex::construct::MACRO):
/// the macro call construct, `flm.pres.latex.macro`. Nothing differs.
pub const MACRO: &str = construct::MACRO;

/// Mirrors [`ENVIRONMENT`](flm_core::stdcore::backends::latex::construct::ENVIRONMENT):
/// the environment construct, `flm.pres.latex.environment`. Nothing differs.
pub const ENVIRONMENT: &str = construct::ENVIRONMENT;

/// Mirrors [`RAW`](flm_core::stdcore::backends::latex::construct::RAW): the
/// construct of raw LaTeX text written byte for byte, `flm.pres.latex.raw`.
/// Nothing differs.
pub const RAW: &str = construct::RAW;

/// Mirrors [`DEFINITION`](flm_core::stdcore::backends::latex::construct::DEFINITION):
/// the construct of a definition written into the style file,
/// `flm.pres.latex.definition`. Nothing differs.
pub const DEFINITION: &str = construct::DEFINITION;

/// Mirrors [`ALL`](flm_core::stdcore::backends::latex::construct::ALL): the
/// four construct elements, in registration order. Nothing differs.
pub const ALL: [&str; 4] = construct::ALL;

/// Mirrors [`ARG_SLOT`](flm_core::stdcore::backends::latex::construct::ARG_SLOT):
/// the name of every argument's content slot on a macro or an environment,
/// `arg`, repeated once per argument. Nothing differs.
pub const ARG_SLOT: &str = construct::ARG_SLOT;

/// Mirrors [`BEGIN_SLOT`](flm_core::stdcore::backends::latex::construct::BEGIN_SLOT):
/// the content slot of an environment definition's begin code, `begin`.
/// Nothing differs.
pub const BEGIN_SLOT: &str = construct::BEGIN_SLOT;

/// Mirrors [`END_SLOT`](flm_core::stdcore::backends::latex::construct::END_SLOT):
/// the content slot of an environment definition's end code, `end`. Nothing
/// differs.
pub const END_SLOT: &str = construct::END_SLOT;

/// Mirrors [`BACKEND`](flm_core::stdcore::backends::latex_recomposed::BACKEND):
/// the name of the recomposed LaTeX backend, `latex-recomposed`. What
/// differs: the name, under the family's module.
pub const RECOMPOSED_BACKEND: &str = flm_core::stdcore::backends::latex_recomposed::BACKEND;

/// Mirrors the payloads of the LaTeX family's constructs
/// ([`LatexMacroPayload`], [`LatexEnvironmentPayload`], [`LatexRawPayload`],
/// [`LatexDefinitionPayload`]), the shape of an argument ([`LatexArgShape`])
/// and the kind of a definition ([`LatexDefinitionKind`]). Nothing differs:
/// they are `flm-core`'s own, re-exported; a binding gives a payload as data
/// in its projection form (the module's table).
pub use flm_core::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload, LatexMacroPayload,
    LatexRawPayload,
};

/// Mirrors the definition piece the family's slot `flm.latex.preamble` takes
/// ([`LatexPreambleDef`]), re-exported from [`preamble`] beside the
/// payloads. Nothing differs.
pub use crate::api::stdcore::backends::latex::preamble::LatexPreambleDef;

/// Mirrors the package request the family's slot `flm.latex.packages` takes
/// ([`LatexPackageDef`]), re-exported from [`packages`] beside the
/// payloads. Nothing differs.
pub use crate::api::stdcore::backends::latex::packages::LatexPackageDef;
