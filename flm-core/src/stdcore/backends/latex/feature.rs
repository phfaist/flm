//! The `latex` feature ([`LatexFamily`]): the registration of the LaTeX
//! family, the parts every LaTeX backend shares ([`super`]).
//!
//! The feature ([`FlmFeature`]) registers no backend of its own. It registers:
//!
//! - the four LaTeX construct elements ([`super::construct`]);
//! - the two service slots through which a LaTeX backend resolves the
//!   requirements a lowering rule records: [`super::packages::SLOT`]
//!   (`flm.latex.packages`) for package requests and
//!   [`super::preamble::SLOT`] (`flm.latex.preamble`) for definition pieces;
//! - the standard contributions to both slots: the standard package
//!   definitions ([`super::packages::standard_packages`]) and the family's own
//!   definition pieces ([`super::preamble::standard_pieces`]);
//! - the placeholder rule ([`super::construct::placeholder_rule`]), which
//!   writes the LaTeX text a copied template carries at one of its
//!   placeholders ([`LatexPoint`](super::LatexPoint)): for example the
//!   parameter `#1` that a placeholder of a kept user definition
//!   ([`kept`](crate::stdcore::features::userdefs::kept)) becomes, or the
//!   `enumitem` command that a number format of a list label template
//!   becomes.
//!
//! The placeholder rule is the default on the `latex-recomposed` backend
//! only. The feature makes a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of that backend, which
//! the `latex-recomposed` feature
//! ([`LatexRecomposed`](crate::stdcore::backends::latex_recomposed::LatexRecomposed))
//! registers. A registry that holds this feature without the
//! `latex-recomposed` backend therefore still builds, with the placeholder
//! rule set aside (excluded from rule selection; see
//! [`PeerName`](crate::core::registry::PeerName)).
//!
//! Each LaTeX backend is registered by a feature of its own: its native set
//! (the elements its emitter writes directly, [`BackendDef::native`]) holds
//! the construct elements, and its lowering rules require packages and
//! definition pieces by identifier. The registry checks each backend's native
//! set (a backend must write `flm.pres.unsupported` natively, and
//! `flm.pres.deco-mark` when its native set holds an element carrying the
//! decoration aspect); this feature registers no backend, so the check does
//! not apply to it.
//!
//! [`FlmFeature`]: crate::core::registry::FlmFeature
//! [`BackendDef::native`]: crate::core::registry::BackendDef::native

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::services::Arity;

use crate::stdcore::backends::latex_recomposed::BACKEND as RECOMPOSED_BACKEND;

use super::construct;
use super::packages::{self, LatexPackageDef};
use super::preamble::{self, LatexPreambleDef};

/// The name the feature is registered under, `latex`.
pub const FEATURE_NAME: &str = "latex";

/// The `latex` feature, which registers the parts every LaTeX backend
/// shares (see the [module documentation](self)).
///
/// A LaTeX backend is registered beside this feature: the backend's native
/// set and lowering rules refer to the construct elements this feature
/// registers, and its stages resolve requirements through the service slots
/// this feature declares. The standard package of the library
/// ([`stdcore::package`](crate::stdcore::package)) lists it. Use
/// [`feature()`] to obtain it as a value that a package entry
/// ([`PackageEntry`](crate::core::registry::PackageEntry)) can list.
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexFamily;

impl FlmFeature for LatexFamily {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE_NAME, "LaTeX family")
            .docs(
                "registers what every LaTeX backend shares: the LaTeX construct elements, \
                 the placeholder rule, the service slots through which a backend resolves \
                 LaTeX package requests and definition pieces, and the standard package \
                 definitions and definition pieces",
            )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        construct::register(b)?;
        b.peer_backend(
            RECOMPOSED_BACKEND,
            "the latex-recomposed backend, on which the placeholder rule is the default",
        )?;
        b.lowering_rule(construct::placeholder_rule())?;
        b.service_slot::<LatexPackageDef>(
            packages::SLOT,
            Arity::OrderedMany,
            "the package definitions that a requirement identifier `flm.latex.pkg.<name>` \
             resolves to",
        )?;
        b.service_slot::<LatexPreambleDef>(
            preamble::SLOT,
            Arity::OrderedMany,
            "the definition pieces a requirement identifier `flm.latex.def.<name>` resolves to",
        )?;
        for package in packages::standard_packages() {
            b.contribute_to(packages::SLOT, Arc::new(package))?;
        }
        for piece in preamble::standard_pieces() {
            b.contribute_to(preamble::SLOT, Arc::new(piece))?;
        }
        Ok(())
    }
}

/// The [`LatexFamily`] feature as a shared value, ready to be listed in a
/// package entry ([`PackageEntry`](crate::core::registry::PackageEntry)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex;
///
/// assert_eq!(latex::feature().info().name.as_str(), latex::FEATURE_NAME);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(LatexFamily)
}
