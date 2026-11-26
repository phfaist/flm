//! Mirrors [`flm_core::math_passthrough`]: the formula rules of the
//! `math-passthrough` feature that write a formula as LaTeX, which another
//! feature that processes mathematics registers under rule names of its own
//! (the rules of the `latex-recomposed` backend are in [`latex`]).
//!
//! Each constructor answers the library's rule, with the library's body,
//! declaration and description, under the name and, here, the default mark
//! it is given. An extension registers it with
//! [`FeatureBuilder::lowering_rule`](crate::api::core::registry::FeatureBuilder::lowering_rule),
//! under a name that begins with the extension's stem and a dot, and makes
//! beside it the peer declarations each constructor's documentation lists.
//! The registering feature owns the rule: a plan that keeps one of the
//! rule's unchecked outputs names that feature in its warning, with the
//! library's texts. The extension `math-techxt` registers the four rules
//! this way on `latex-rendered` and `latex-recomposed`.
//!
//! The module keeps the library's module name, with its underscore, in
//! every binding, while the feature and its package are named
//! `math-passthrough`: in JavaScript, its items are imported from
//! `flm:math_passthrough` and those of [`latex`] from
//! `flm:math_passthrough/latex`.
//!
//! What differs from `flm-core`, for every item: the rule's name is a
//! `&str`, and the rule is the extension API's [`LoweringRule`], which holds
//! the rule `flm-core` built, as a helper makes it.
//!
//! Left out: the feature, its package and its other items (the fallback's
//! element, the check, the opaque command's rule, the rules' bodies), which
//! only `math-passthrough` registers.

pub mod latex;

use flm_core::math_passthrough as core_passthrough;

pub use crate::api::core::lowering::{DefaultMark, LoweringRule};

/// Mirrors [`math_rule_named`](flm_core::math_passthrough::math_rule_named):
/// the library's rule of `flm.math` that stages `flm.pres.math-passthrough`,
/// under the rule name `name` and the default mark `mark`. The rule reads
/// no table, so a feature that registers it declares nothing beside it.
pub fn math_rule_named(name: &str, mark: DefaultMark) -> LoweringRule {
    LoweringRule::from_core(core_passthrough::math_rule_named(name, mark))
}

/// Mirrors [`environment_rule_named`](flm_core::math_passthrough::environment_rule_named):
/// the library's rule of `flm.math.environment` that stages
/// `flm.pres.math-passthrough`, under the rule name `name` and the default
/// mark `mark`. A feature that registers it declares beside it a peer
/// table
/// ([`FeatureBuilder::peer_table`](crate::api::core::registry::FeatureBuilder::peer_table))
/// of the anchors table and of the counter table
/// ([`ANCHORS`](crate::api::stdlib::features::math::ANCHORS),
/// [`COUNTERS`](crate::api::stdlib::features::math::COUNTERS)), which the
/// rule reads optionally.
pub fn environment_rule_named(name: &str, mark: DefaultMark) -> LoweringRule {
    LoweringRule::from_core(core_passthrough::environment_rule_named(name, mark))
}
