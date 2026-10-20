//! The three aspects the lowering driver reads on presentation elements:
//! [`Decoration`], [`OpaqueContent`] and [`ContentInOpaque`].
//!
//! An **aspect** is a piece of behavior attached to an element when the
//! feature registers it and read back through a trait object; see [elements
//! and aspects](crate::guide::concepts_overview#elements-and-aspects). The
//! lowering driver asks an element for an aspect instead of comparing its
//! name with a fixed list:
//!
//! - [`Decoration`]: a node of the element is a decoration, which the driver
//!   moves out of a presentation node's content into an attached slot.
//! - [`OpaqueContent`]: the node's content is source text, copied unchanged.
//! - [`ContentInOpaque`]: the node's content is document content even inside
//!   opaque content.
//!
//! [`register`](super::register()) attaches them to the elements listed in
//! [`DECORATIONS`](super::DECORATIONS), [`OPAQUE`](super::OPAQUE) and
//! [`CONTENT_IN_OPAQUE`](super::CONTENT_IN_OPAQUE). Because the driver reads
//! the aspect and not those lists, an extension that registers a decoration
//! element of its own, or an element of its own whose content is opaque, is
//! handled exactly like the elements of the vocabulary.

// Why the driver reads aspects rather than element names, and why content
// inside opaque content is an element of this vocabulary with an aspect of its
// own: see decision rationale at [§flm-dd-dr:rend-pres-aspects] and
// [§flm-dd-dr:rend-pres-math-text].

/// The aspect that makes an element a decoration element.
///
/// A **decoration** is a node whose element carries this aspect (a run-in
/// heading, a margin note, a closing mark). When a lowering rule stages a
/// presentation node, the lowering driver moves every decoration among the
/// direct children of one of the node's content slots into an **attached
/// slot** of that node, named after the decoration's element, and leaves a
/// `flm.pres.deco-mark` node (the **decoration mark**) at the decoration's
/// former position; the mark's payload gives the index of the attached slot.
/// An emitter writes the decoration at the mark's position (see
/// [`decoration_at`](super::decoration_at) and [decorations, roles and
/// kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)).
///
/// A backend that writes any element carrying this aspect natively must
/// therefore write `flm.pres.deco-mark` natively too. The registry build
/// checks this through the `lowering` feature's [native-set
/// implications](crate::stdcore::backends::lowering::native_implications)
/// and refuses a backend that does not.
///
/// Attach it by its trait-object type:
/// `b.attach::<dyn Decoration>(name, Arc::new(IsDecoration))`
/// ([`FeatureBuilder::attach`](crate::core::registry::FeatureBuilder::attach)).
pub trait Decoration: Send + Sync {}

/// The aspect of an element whose content is opaque source text, such as
/// mathematics or verbatim content (see [opaque
/// regions](crate::guide::concepts_overview#opaque-regions)).
///
/// The lowering driver copies such a node's whole subtree unchanged: it
/// lowers nothing inside it and counts no element inside it when it decides
/// whether another round is needed. An emitter writes that content again byte
/// for byte. The only exception is a node carrying [`ContentInOpaque`], whose
/// content the driver lowers. `flm.pres.verbatim` and `flm.pres.math-passthrough`
/// carry this aspect.
///
/// Attach it by its trait-object type:
/// `b.attach::<dyn OpaqueContent>(name, Arc::new(IsOpaque))`
/// ([`FeatureBuilder::attach`](crate::core::registry::FeatureBuilder::attach)).
pub trait OpaqueContent: Send + Sync {}

/// The aspect of an element that holds document content, not source text,
/// even when its node is inside [opaque content](OpaqueContent).
///
/// It is the only exception to the rule that nothing inside an opaque region
/// is lowered: the lowering driver lowers such a node's content like any
/// other content, counts the elements left in it when it decides whether
/// another round is needed, and gives it the same rounds as content anywhere
/// else. Everything else inside the opaque region is still copied as source
/// text. `flm.pres.math-text` (the argument of `\text{…}` inside a formula)
/// is the only element of this vocabulary that carries it.
///
/// An emitter that writes an opaque node's content as source text must
/// therefore expect such a node in that content and write the node's content
/// itself.
///
/// Attach it by its trait-object type:
/// `b.attach::<dyn ContentInOpaque>(name, Arc::new(IsContentInOpaque))`
/// ([`FeatureBuilder::attach`](crate::core::registry::FeatureBuilder::attach)).
pub trait ContentInOpaque: Send + Sync {}

/// The implementation of [`Decoration`] that [`register`](super::register())
/// attaches. It carries no data: the presence of the aspect is the whole
/// statement.
#[derive(Debug, Clone, Copy, Default)]
pub struct IsDecoration;

impl Decoration for IsDecoration {}

/// The implementation of [`OpaqueContent`] that
/// [`register`](super::register()) attaches. It carries no data: the presence
/// of the aspect is the whole statement.
#[derive(Debug, Clone, Copy, Default)]
pub struct IsOpaque;

impl OpaqueContent for IsOpaque {}

/// The implementation of [`ContentInOpaque`] that
/// [`register`](super::register()) attaches. It carries no data: the presence
/// of the aspect is the whole statement.
#[derive(Debug, Clone, Copy, Default)]
pub struct IsContentInOpaque;

impl ContentInOpaque for IsContentInOpaque {}
