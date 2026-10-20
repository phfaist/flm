//! Mirrors [`flm_core::core::services`]: the arity of a service slot, which
//! [`FeatureBuilder::service_slot`](crate::api::core::registry::FeatureBuilder::service_slot)
//! takes. What differs: the slot declarations, the contributions and the
//! assembled services are the build's own records and are not mirrored; a
//! feature declares and contributes through the builder and reads through
//! its context ([`StageCx::service`](crate::api::core::run::StageCx::service)).

/// Mirrors [`flm_core::core::services::Arity`]. Nothing differs: it is
/// re-exported. Across a binding, `"single"` or `"ordered-many"`.
pub use flm_core::core::services::Arity;
