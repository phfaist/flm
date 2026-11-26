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
//! | a value a Rust caller can hold but a script cannot read (`ArgDef`, `LoweringRule`, `Definition`) | the `flm-core` type, re-exported | built from its data shape at the call that takes it; an `ArgDef` or a `Definition` a function answers is written in that shape, a `LoweringRule` as a handle |
//! | a reference to an object with methods (`&mut FeatureBuilder`, `&StageCx`, a node) | an object-safe trait with the same method names, implemented by the adapter over the real object | a handle, its methods dispatched by name |
//! | a method generic over a type (`b.table::<T>`, `cx.get::<K>`) | the same method, the type named by the registered name | the same |
//! | a free function (`chars_text`, `element_name_under`) | the same function in the mirrored module, over the façades | the same, dispatched on the module's handle |
//! | a trait an extension implements (`Stage`, `Expand`) | an object-safe trait of the same name and methods, passed as `Arc<dyn Trait>` where `flm-core` takes one | an object whose methods the binding calls back |
//! | `Result<T, E>` | `Result<T, ExtError>`, `E` rendered into [`ExtError::Core`](crate::ExtError::Core) | `T`, or a thrown error carrying the rendering |
//! | `Option<T>` | `Option<T>` | `T` or `null` |
//! | a node slice | `Vec<Box<dyn Node<'t> + 't>>` (taken as `&[Box<dyn Node<'t> + 't>]`) | a sequence of nodes (below); taken as a sequence of nodes or as a joined list of nodes and sequences of nodes |
//! | a list of staged nodes (`Vec<BuildId>`) | `Vec<Staged>` | a sequence of staged nodes; taken as a sequence of staged nodes or as a joined list of staged nodes and sequences of staged nodes |
//! | an iterator over a document's nodes or trees (`StageCx::walk`, `NodeRef::descendants`, `StageCx::trees`) | a struct that implements `Iterator` ([`Walk`](crate::api::core::run::Walk), [`Descendants`](crate::api::techy::node::Descendants), [`Trees`](crate::api::core::run::Trees)) | a traversal (below) |
//! | the rows of an extension's own table ([`ExtTableRead::items`](crate::api::core::data::table::ExtTableRead::items), no `flm-core` counterpart) | [`Rows`](crate::api::core::data::table::Rows) | a sequence of rows; a row is data, copied when it is read |
//! | a `&'static str` name constant (`TableName`) | `&str` | a string |
//!
//! A *traversal* is one handle of the running handler call. A binding pulls
//! the items of a traversal once, in order, a bounded group of items per
//! crossing, and hands them to the extension one by one. Calling the method
//! again returns a new traversal, which starts from the first item. The
//! traversals of nodes ([`Walk`](crate::api::core::run::Walk),
//! [`Descendants`](crate::api::techy::node::Descendants)) can be restricted
//! to the nodes of given elements or of given kinds; the host compares the
//! nodes, so a node that does not match never reaches the extension.
//!
//! A *sequence* is one handle of the running handler call together with its
//! length. A binding reads an item of a sequence by its index, or iterates
//! the items, as often as it needs, pulling them in groups. Where the API
//! takes several nodes or several staged nodes, a binding passes a
//! sequence, a list of single items, or a *joined list*: a list whose parts
//! are single items and sequences, which the host joins in order. A
//! sequence passed back crosses as its handle, whatever its length.
//!
//! An item of a traversal or of a sequence is lent into the call's table
//! only when the binding pulls or reads it, and the engine holds at most one
//! group of items that the extension has not been handed yet. The memory
//! that an engine needs for a handler therefore depends on the items the
//! handler keeps, not on the size of the document. The typed API itself keeps a `Vec` for a node slice
//! and a list of staged nodes: it runs on the host, whose memory has no
//! limit.
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
pub mod math_passthrough;
pub mod model;
pub mod progress;
pub mod stdlib;
pub mod techy;
