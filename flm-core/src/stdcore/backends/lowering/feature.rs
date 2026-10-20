//! The `lowering` feature: the registrations every backend relies on.
//!
//! The `lowering` feature ([`LoweringFeature`], an [`FlmFeature`]) registers
//! what lowering needs on any backend:
//!
//! - the presentation vocabulary ([`super::super::pres`]), through
//!   [`pres::register()`];
//! - the field element ([`FIELD`](crate::lang::mint::FIELD)): every field (a
//!   macro that a construct absorbs as data, such as a `\label` written after
//!   a heading; see [fields: data a construct
//!   reads](crate::guide::concepts_overview#fields-data-a-construct-reads))
//!   carries this element;
//! - the default lowering rules of the structural elements and of the field
//!   element ([`super::rules`]);
//! - the two native-set implications the lowering driver relies on
//!   ([`native_implications`]);
//! - the service slot [`NAMER_SLOT`], through which the embedding program
//!   gives the path of each artifact of an output, and the service slot
//!   [`REGIONS_SLOT`], through which each backend states which of its outputs
//!   are made of output regions.
//!
//! The field element is declared here, with no callable name of its own, so
//! that it and its rule exist in every registry that renders anything: the
//! language layer creates a field node wherever a declaration carries a
//! field, and a construct that fails to read its field must be reported
//! regardless of which features are registered. A backend's own feature
//! ([`TextBackend`](crate::stdcore::backends::text::TextBackend), for
//! instance) is registered beside this one and adds the backend, its stages
//! and its output. Registering this feature twice into one registry is a
//! registry build error, so a package lists it once.

use std::sync::Arc;

use crate::core::artifact::ArtifactNamer;
use crate::core::region::RegionsOutput;
use crate::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, NativeImplication,
};
use crate::core::services::Arity;
use crate::lang::mint::{ElementDef, SyntaxDef, field_type_element};
use crate::stdcore::backends::pres::{self, Decoration};

use super::rules::default_rules;

/// The name under which this feature is registered, `lowering`; also the
/// middle segment of its conditions' identifiers, `flm.lowering.*`
/// ([`super::error`]).
pub const FEATURE_NAME: &str = "lowering";

/// The service slot `flm.artifacts.namer`
/// ([`service_slot`](crate::core::registry::FeatureBuilder::service_slot)),
/// holding at most one [`ArtifactNamer`]: the embedding program's naming
/// scheme, which gives the path of each artifact of an output, relative to
/// the output location.
///
/// A service slot is the declaration, by the feature that owns an interaction,
/// of a dotted name, the type of the values the slot takes and its arity; any
/// feature, and the embedding program through the registry builder, may
/// contribute a value to it (see [`crate::core::services`]). The library reads and writes no files and
/// chooses no file path itself. A host that writes files contributes its
/// naming scheme here, so that a stage of any feature can ask for an
/// artifact's path before the artifact is made, for example to write a link
/// from a page to a file placed beside it. When the host contributes nothing,
/// the slot is empty and a stage then writes no file path. The slot is
/// declared by this feature, beside the vocabulary every backend shares,
/// because the artifacts of every backend get their paths through it.
pub const NAMER_SLOT: &str = "flm.artifacts.namer";

/// The service slot `flm.output.regions`, holding one [`RegionsOutput`] per
/// output whose entry artifact is made of output regions
/// ([`Regions`](crate::core::region::Regions)), in contribution order.
///
/// An output region is one named part of an output's entry artifact (the
/// document's content, each piece placed beside it, and on HTML the
/// stylesheets, the scripts, the title and the language). A stage that
/// assembles the regions in a layout other than the backend's final stage
/// (see [output regions and the final
/// stage](crate::guide::concepts_overview#output-regions-and-the-final-stage))
/// reads this slot to learn which outputs it can assemble. Each backend that
/// writes regions contributes its own value; the slot is declared by this
/// feature, beside the vocabulary every backend shares, since any backend's
/// output may be made of regions.
pub const REGIONS_SLOT: &str = "flm.output.regions";

/// The `lowering` feature: it registers the presentation vocabulary
/// ([`pres::register()`]), the field element, the default lowering rules of
/// the structural elements and of the field element ([`default_rules`]), the
/// native-set implications the driver relies on ([`native_implications`]),
/// and the service slots [`NAMER_SLOT`] and [`REGIONS_SLOT`] (see the module
/// documentation).
///
/// Every registry that renders anything needs this feature. Use
/// [`feature()`] to obtain it as a value a package entry can list.
#[derive(Debug, Default)]
pub struct LoweringFeature;

impl FlmFeature for LoweringFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE_NAME, "Lowering")
            .docs(
                "the presentation vocabulary, the field element and the structural elements' \
                 default lowering rules",
            )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        pres::register(b)?;
        b.service_slot::<dyn ArtifactNamer>(
            NAMER_SLOT,
            Arity::Single,
            "the host's naming scheme: the path of each artifact of an output, relative to the \
             output location",
        )?;
        b.service_slot::<RegionsOutput>(
            REGIONS_SLOT,
            Arity::OrderedMany,
            "the outputs whose entry artifact is made of output regions, each with the table \
             that holds its regions and the tables of the files written beside its entry \
             artifact",
        )?;
        let owner = b.feature().clone();
        b.element(
            ElementDef::new(
                owner,
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                field_type_element(),
            )
            .docs(
                "A field: a macro a construct absorbs as data, such as the `\\label` written \
                 after a heading. It is never written on its own: only the construct that \
                 declares a field accepts it, and the construct that absorbed it reads it.",
            ),
        )?;
        for rule in default_rules() {
            b.lowering_rule(rule)?;
        }
        for implication in native_implications() {
            b.native_implication(implication)?;
        }
        Ok(())
    }
}

/// The two [native-set implications](NativeImplication) the lowering driver
/// relies on, which this feature declares so that the registry build checks
/// every backend against them.
///
/// A backend's native set is the list of elements its emit stage writes
/// directly ([`BackendDef::native`](crate::core::registry::BackendDef::native)).
/// The driver ([`super::driver`]) stages two presentation elements on its
/// own, with no lowering rule declaring that it produces them, so plan
/// construction cannot see that a backend needs them:
///
/// 1. A backend whose native set holds any element carrying the
///    [`Decoration`] aspect must hold `flm.pres.deco-mark` too: the driver
///    moves such a decoration out of a content slot into an attached slot and
///    leaves that mark at its former position.
/// 2. Every backend must hold `flm.pres.unsupported`: the driver stages it
///    for a node of an accepted unrenderable element, for a node whose rule
///    failed, and for a node no rule lowers.
///
/// A backend missing one fails the registry build with
/// [`BuildError::NativeSetIncomplete`] instead of failing at lowering.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::NativeImplicationKind;
/// use flm_core::stdcore::backends::{lowering, pres};
///
/// let implications = lowering::native_implications();
/// assert_eq!(implications[0].must_claim, pres::DECO_MARK);
/// assert_eq!(implications[1].must_claim, pres::UNSUPPORTED);
/// assert_eq!(implications[1].kind, NativeImplicationKind::Always);
/// ```
pub fn native_implications() -> Vec<NativeImplication> {
    vec![
        NativeImplication::aspect::<dyn Decoration>(pres::DECO_MARK).docs(
            "a backend that writes a decoration element natively must write natively the mark \
             the lowering driver leaves where it moved a decoration into an attached slot",
        ),
        NativeImplication::always(pres::UNSUPPORTED).docs(
            "every backend must write natively the stand-in the lowering driver stages for a \
             node it cannot lower: a node of an accepted unrenderable element, a node whose \
             rule failed, or a node no rule lowers",
        ),
    ]
}

/// The `lowering` feature as a shared value, ready to be listed in a package
/// entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::lowering;
///
/// assert_eq!(lowering::feature().info().name.as_str(), lowering::FEATURE_NAME);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(LoweringFeature)
}
