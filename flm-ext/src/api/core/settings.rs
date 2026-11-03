//! Mirrors [`flm_core::core::settings`]: the fault a feature's configuration
//! reader answers. What differs: a feature of an extension declares its
//! settings as data, in its report
//! ([`SettingsReport`](crate::engine::SettingsReport)), and the adapter
//! turns the report into the feature's declaration, so the declaration and
//! the settings read are not mirrored.

/// Mirrors [`flm_core::core::settings::ReadError`]. Nothing differs: it is
/// re-exported. Across a binding, `{ path, detail }`, `path` optional.
pub use flm_core::core::settings::ReadError;
