//! Mirrors [`flm_core::math_passthrough::latex`]: the formula rules of the
//! `math-passthrough` feature for the `latex-recomposed` backend, which
//! another feature that processes mathematics registers under rule names of
//! its own.
//!
//! Each constructor answers the library's rule, with the library's body,
//! declaration, description and default mark (the default on the
//! `latex-recomposed` backend only), under the name it is given; the
//! [parent module](super) says how an extension registers it and who owns
//! it.
//!
//! What differs from `flm-core`, for every item: the rule's name is a
//! `&str`, and the rule is the extension API's [`LoweringRule`], which holds
//! the rule `flm-core` built, as a helper makes it.
//!
//! Left out: the rules' bodies and `register`, which only `math-passthrough`
//! calls.

use flm_core::math_passthrough::latex as core_latex;

pub use crate::api::core::lowering::LoweringRule;

/// Mirrors [`math_rule_named`](flm_core::math_passthrough::latex::math_rule_named):
/// the library's rule of `flm.math` for the `latex-recomposed` backend, under
/// the rule name `name`. A feature that registers it declares beside it a
/// peer declaration of the backend
/// ([`FeatureBuilder::peer_backend`](crate::api::core::registry::FeatureBuilder::peer_backend))
/// and of the raw and definition constructs
/// ([`FeatureBuilder::peer_element`](crate::api::core::registry::FeatureBuilder::peer_element)).
pub fn math_rule_named(name: &str) -> LoweringRule {
    LoweringRule::from_core(core_latex::math_rule_named(name))
}

/// Mirrors [`environment_rule_named`](flm_core::math_passthrough::latex::environment_rule_named):
/// the library's rule of `flm.math.environment` for the `latex-recomposed`
/// backend, under the rule name `name`. A feature that registers it declares
/// beside it a peer declaration of the backend
/// ([`FeatureBuilder::peer_backend`](crate::api::core::registry::FeatureBuilder::peer_backend)),
/// of the macro, raw and definition constructs
/// ([`FeatureBuilder::peer_element`](crate::api::core::registry::FeatureBuilder::peer_element))
/// and of the safe-labels table
/// ([`FeatureBuilder::peer_table`](crate::api::core::registry::FeatureBuilder::peer_table)),
/// which the rule reads optionally.
pub fn environment_rule_named(name: &str) -> LoweringRule {
    LoweringRule::from_core(core_latex::environment_rule_named(name))
}
