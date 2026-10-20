//! The document model: the types that describe what a document is made of,
//! how its parts are identified, and the option values set on it.
//!
//! This module defines the data a document is described in, and nothing about
//! how a document is processed. Its main concepts are these:
//!
//! - A **document** ([`Doc`](doc::Doc)) is a set of trees of parsed nodes,
//!   built from one or more **fragments** ([`Fragment`](doc::Fragment)), each
//!   fragment being one piece of source as the parser delivered it.
//! - Every node carries an **element** ([`ElementSpec`](element::ElementSpec)),
//!   the value that says what the node means.
//! - An element carries **aspects**: pieces of behavior attached to the element
//!   when a feature registers it, which the rest of the library reads back
//!   through trait objects (see [elements and
//!   aspects](crate::guide::concepts_overview#elements-and-aspects)).
//! - Every node of a document has an **address** ([`NodeAddr`](addr::NodeAddr)),
//!   the key under which computed data about the node is stored.
//! - Every entity registered with the library (a stage, a table, a capability,
//!   a feature, …) is identified by a dotted **name** ([`Name`](names::Name)).
//!   An element has a dotted type name too, but it is identified by its element
//!   value, the allocation of its [`ElementSpec`](element::ElementSpec).
//! - A document also carries the **option values** set on it when it was
//!   built ([`OptionValues`](options::OptionValues)).
//!
//! See [the document tree](crate::guide::concepts_overview#the-document-tree)
//! for an introduction, and the [document model guide](crate::guide::document_model)
//! for the whole picture.
//!
//! The library processes a document with the types of [`crate::core`]: stages,
//! the registry, plans, runs and the store of computed data. This module uses
//! none of them.
//!
//! # The modules
//!
//! - [`names`] — dotted names, and one Rust type per kind of name
//!   ([`ElementName`](names::ElementName), [`CapName`](names::CapName), …).
//! - [`addr`] — [`TreeId`](addr::TreeId) and [`NodeAddr`](addr::NodeAddr), the
//!   identifiers of a tree and of a node, under which computed data is stored
//!   per node.
//! - [`element`] — [`ElementSpec`](element::ElementSpec), the
//!   [`AspectMap`](element::AspectMap) holding an element's aspects, and the
//!   rule that two elements are the same when they are the same allocation.
//! - [`aspects`] — the aspects that the standard features implement and read:
//!   block roles, reference points, reference sites, headings, content outside
//!   the surrounding text, and what replaces a node in a copy of content.
//! - [`doc`] — fragments, a document's content (its tree arena, traversal list
//!   and resolution map), the document itself, and the walk that defines
//!   document order.
//! - [`assemble`] — [`assemble`](assemble::assemble), which parses a document's
//!   primary file together with the files it includes with `\input` into a
//!   document's content.
//! - [`options`] — option types, the values set for them on a document or on a
//!   render request, and how a stage reads them.
//! - [`digest`] — [`Fnv1a64`](digest::Fnv1a64), the deterministic hasher behind
//!   every digest this crate computes, and [`digest_of`](digest::digest_of).
//!
//! # Position in the crate's layers
//!
//! The [crate documentation](crate) orders the crate's modules in layers, each
//! using only the layers below it. This module and [`crate::lang`] form the
//! bottom layer together:
//!
//! ```text
//! {model, lang} < core < dump < stdcore < math_passthrough        layout standalone
//! ```
//!
//! The two modules share one layer because each one uses the other: a parsed
//! node carries its element, so the language uses the model, and a fragment
//! holds a tree typed by the language, so the model uses the language.
//! Together they use nothing else of this crate.

pub mod addr;
pub mod assemble;
pub mod aspects;
pub mod digest;
pub mod doc;
pub mod element;
pub mod names;
pub mod options;
