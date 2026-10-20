//! The extension API: the part of [`flm_core`]'s public interface an extension
//! may use, mirrored item for item.
//!
//! # The mirror principle
//!
//! Every item here names, in the first sentence of its documentation, the
//! `flm-core` item it mirrors, and then says only what differs. The module
//! paths are `flm-core`'s (`api::core::registry` mirrors
//! [`flm_core::core::registry`]; `api::techy::node` mirrors techy's node module,
//! reached as [`flm_core::techy::core::node`]); the names are the Rust names
//! verbatim, `snake_case` included. No alias is documented.
//!
//! # The mapping rules
//!
//! | `flm-core` construct | Here | Across a JSON binding |
//! |---|---|---|
//! | plain data with a serde form or public fields (`BlockRole`, `Effect`) | the `flm-core` type, re-exported under the mirrored path | its serde form, or the shape the adapter states for a type without one |
//! | a value a Rust caller can hold but a script cannot read (`ArgDef`, `LoweringRule`, `Definition`) | the `flm-core` type, re-exported | built from its data shape at the call that takes it; a value a helper produced crosses as a handle |
//! | a reference to an object with methods (`&mut FeatureBuilder`, `&StageCx`, a node) | an object-safe trait with the same method names, implemented by the adapter over the real object | a handle, its methods dispatched by name |
//! | a method generic over a type (`b.table::<T>`, `cx.get::<K>`) | the same method, the type named by the registered name | the same |
//! | a free function (`chars_text`, `element_name_under`) | the same function in the mirrored module, over the façades | the same, dispatched on the module's handle |
//! | a trait an extension implements (`Stage`, `Expand`) | an object-safe trait of the same name and methods, passed as `Arc<dyn Trait>` where `flm-core` takes one | an object whose methods the binding calls back |
//! | `Result<T, E>` | `Result<T, ExtError>`, `E` rendered into [`ExtError::Core`](crate::ExtError::Core) | `T`, or a thrown error carrying the rendering |
//! | `Option<T>` | `Option<T>` | `T` or `null` |
//! | a node slice or an iterator of nodes | `Vec<Box<dyn Node<'t> + 't>>` | a list of node handles |
//! | a `&'static str` name constant (`TableName`) | `&str` | a string |
//!
//! A *façade* is one of these object-safe traits; the adapter implements each
//! over the real object and lends it to a handler for the duration of one call.
//! The façade traits are sealed: the adapter alone implements them, so every
//! façade a handler receives is the adapter's. Their seal carries the methods
//! through which the adapter reaches the `flm-core` object behind a façade; code
//! outside this crate cannot call them. The traits an extension implements
//! (`Stage`, `Expand`) and [`ExtRuntime`](crate::ExtRuntime) are not sealed.
//!
//! Code outside the crate cannot implement a façade trait:
//!
//! ```compile_fail
//! use flm_ext::api::lang::rewrites::userdefs::CompiledTemplate;
//!
//! struct Mine;
//! impl CompiledTemplate for Mine {}
//! ```
//!
//! nor reach the `flm-core` object behind a façade, since the accessor takes a
//! proof of being inside the crate that only the crate can make:
//!
//! ```compile_fail
//! use flm_ext::api::lang::rewrites::userdefs::CompiledTemplate;
//!
//! fn reach(template: &dyn CompiledTemplate) {
//!     template.with_core(flm_ext::sealed::Token::new(), &mut |_| {});
//! }
//! ```

pub mod core;
pub mod host;
pub mod lang;
pub mod model;
pub mod stdcore;
pub mod techy;
