//! Elements, which say what the nodes of a document mean, and the aspect map
//! through which the rest of the library reads an element's behavior.
//!
//! An element is the value that says what a node means: a declaration with a
//! stable dotted type name ([`ElementName`], such as `flm.heading` or
//! `thm.exercise`), identified by the element value itself (its allocation)
//! and not by its name ([`ElementSpec`]). An element carries **aspects**:
//! pieces of behavior attached to the element when a feature registers it,
//! held in its [`AspectMap`] and read back through trait objects. A stage asks
//! an element what it does through an aspect ("is this a block?", "which
//! reference points does this node carry?") instead of matching its name. See
//! [elements and aspects](crate::guide::concepts_overview#elements-and-aspects);
//! the aspects that the standard features use are defined in
//! [`crate::model::aspects`].
//!
//! A feature declares a construct once, and that one declaration produces both
//! the specification the parser uses and the element
//! ([`ElementDef`](crate::lang::mint::ElementDef)). Every node of every parsed
//! tree carries its element in its node extension
//! ([`FlmNodeExt`](crate::lang::FlmNodeExt)), so [`element_of`] reads a node's
//! element without any lookup.
//!
//! # Identity of an element
//!
//! An element is identified by its element value, not by its name and not by
//! a Rust type. `\section` and `\subsection` are two element values of the
//! single type `flm.heading`, which differ in their data
//! ([`ElementSpec::value`]). An element value is shared as `Arc<ElementSpec>`,
//! and two handles denote the same element when they point at the same
//! allocation ([`ElementSpec::same_value`]). A registry gives each registered
//! element value a number of its own ([`ElemIdx`]) to index its lookup
//! structures. Rust's `TypeId` appears in this module only as the key of the
//! aspect map.
//!
//! # Items
//!
//! - [`ElementSpec`] — one element value: its type name, a description, its
//!   aspects and its data.
//! - [`AspectMap`] — the aspects attached to one element, and
//!   [`AspectCollision`], the error of attaching one aspect twice.
//! - [`element_of`] and [`element_value`] — a node's element, and that
//!   element's data.
//! - [`ElemIdx`] — a registry's number for an element value.
//! - [`AmbiguousElement`] — the error of looking up an element by a type name
//!   that several element values share.

use std::any::{Any, TypeId};
use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::model::names::ElementName;
use crate::lang::Flm;

/// The number a registry gives one element value, counted from zero.
///
/// A registry numbers every registered `Arc<ElementSpec>`, the structural
/// elements included, in a deterministic order, and uses the numbers to index
/// its lookup structures, such as its table of elements and its sets of
/// elements per aspect. The registry holds the map from an element value to its
/// number itself ([`FlmRegistry::elem_idx`]); nothing is written on the shared
/// element value, so an element value used by several registries never carries
/// a stale number. An `ElemIdx` therefore has a meaning only together with the
/// registry that assigned it.
///
/// This module defines the type; the [registry](crate::core::registry) assigns
/// the numbers.
///
/// [`FlmRegistry::elem_idx`]: crate::core::registry::FlmRegistry::elem_idx
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElemIdx(pub u32);

/// The error of attaching an aspect to an element that already has an
/// implementation of that aspect.
///
/// An element carries at most one implementation of each aspect, so
/// [`AspectMap::insert`] refuses a second one instead of replacing the first.
/// [`ElementSpec::with`] panics on this error, since it is a mistake in the
/// feature that builds the element. The registry build, which attaches the
/// aspects that features declare for other features' elements
/// ([`FeatureBuilder::attach`](crate::core::registry::FeatureBuilder::attach)),
/// reports it as a build error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AspectCollision {
    /// The Rust type name of the aspect trait, for messages only: the map
    /// identifies an aspect by its `TypeId`.
    pub aspect: &'static str,
}

impl fmt::Display for AspectCollision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "an element can carry one implementation of each aspect, but aspect `{}` \
             is already attached to this element",
            self.aspect
        )
    }
}

impl std::error::Error for AspectCollision {}

/// One entry of an [`AspectMap`]: the aspect trait's `TypeId`, its type name,
/// and the implementation.
///
/// An `Arc<dyn A>` is itself a sized value, so it can be put behind one more
/// `Arc` and erased to `Arc<dyn Any>`; `get` downcasts that back to
/// `Arc<dyn A>`. This keeps the map within safe, stable Rust.
#[derive(Clone)]
struct AspectEntry {
    id: TypeId,
    name: &'static str,
    imp: Arc<dyn Any + Send + Sync>,
}

/// The aspect implementations attached to one element, at most one per
/// aspect, each stored under the aspect's trait.
///
/// An aspect is any object-safe trait, and nothing about an aspect is
/// registered anywhere: a feature, or a single test, can define an aspect of
/// its own and attach it to its elements, and code that does not use that
/// trait is unaffected. An implementation is stored under the `TypeId` of the
/// trait object type, such as `TypeId::of::<dyn BlockRoleAspect>()`.
///
/// [`ElementSpec::with`] is the usual way to attach an aspect;
/// [`AspectMap::insert`] is the fallible form. [`AspectMap::get`],
/// [`AspectMap::get_arc`] and [`AspectMap::has`] read the map, and
/// [`ElementSpec::aspect`] reads it from the element.
#[derive(Clone, Default)]
pub struct AspectMap {
    // A short vector scanned linearly, not a hash map: an element carries a
    // single-digit number of aspects, where a scan beats hashing. Each entry
    // also carries the aspect's type name, which is only ever used in messages.
    // See the design rationale at [§flm-dd-dr:dm-aspects].
    entries: Vec<AspectEntry>,
}

impl AspectMap {
    /// A map with no aspect attached.
    pub fn new() -> AspectMap {
        AspectMap::default()
    }

    /// Attaches `imp` as the implementation of the aspect `A`.
    ///
    /// `A` **must be written as the trait object type**:
    /// `map.insert::<dyn BlockRoleAspect>(imp)`. A concrete `Arc<HeadingElement>`
    /// can still be passed as `imp`, because Rust converts it to
    /// `Arc<dyn BlockRoleAspect>` at the call.
    ///
    /// The trait object type matters because the map stores the implementation
    /// under `A`: stored under a concrete implementation type, it would be
    /// under a key that no reader looks up, and nothing would find it. A check
    /// at compile time therefore rejects a call whose `A` is a sized type, such
    /// as the implementing struct (a reference to a trait object is twice the
    /// width of a plain pointer, a reference to a sized type is not), so a
    /// forgotten `dyn` is a build error rather than an aspect missing at run
    /// time.
    ///
    /// # Errors
    ///
    /// Returns an [`AspectCollision`] when an implementation of `A` is already
    /// attached, and leaves the map unchanged.
    pub fn insert<A: ?Sized + Any + Send + Sync>(
        &mut self,
        imp: Arc<A>,
    ) -> Result<(), AspectCollision> {
        const {
            assert!(
                size_of::<&A>() == 2 * size_of::<*const ()>(),
                "the aspect must be given as its trait-object type, as in \
                 .with::<dyn MyAspect>(…) or .insert::<dyn MyAspect>(…)"
            );
        }
        self.insert_erased(TypeId::of::<A>(), std::any::type_name::<A>(), Arc::new(imp))
    }

    /// Attaches an implementation whose type has already been erased.
    ///
    /// The registry build uses this to attach the aspects that features
    /// declare for other features' elements. An entry read with
    /// [`AspectMap::entries`] can be passed to it unchanged. `id` is the `TypeId` of the
    /// aspect's trait object type `dyn A`; `imp` must hold the `Arc<dyn A>`
    /// value itself, boxed in one more `Arc`, exactly as [`AspectMap::insert`]
    /// stores it, or [`AspectMap::get`] will not find it; `name` is the
    /// aspect's type name, used only in messages.
    ///
    /// # Errors
    ///
    /// Returns an [`AspectCollision`] when an implementation under `id` is
    /// already attached, and leaves the map unchanged.
    pub fn insert_erased(
        &mut self,
        id: TypeId,
        name: &'static str,
        imp: Arc<dyn Any + Send + Sync>,
    ) -> Result<(), AspectCollision> {
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(AspectCollision { aspect: name });
        }
        self.entries.push(AspectEntry { id, name, imp });
        Ok(())
    }

    /// The implementation of the aspect `A`, or `None` when none is attached.
    ///
    /// Written `map.get::<dyn BlockRoleAspect>()`: as for
    /// [`AspectMap::insert`], `A` is the trait object type. A concrete type
    /// compiles here but finds nothing.
    pub fn get<A: ?Sized + Any>(&self) -> Option<&A> {
        let id = TypeId::of::<A>();
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.imp.downcast_ref::<Arc<A>>())
            .map(|arc| &**arc)
    }

    /// A shared handle to the implementation of the aspect `A`, or `None` when
    /// none is attached.
    ///
    /// The same lookup as [`AspectMap::get`], returning an owned `Arc` for a
    /// caller that keeps the implementation after the map is no longer
    /// borrowed.
    pub fn get_arc<A: ?Sized + Any>(&self) -> Option<Arc<A>> {
        let id = TypeId::of::<A>();
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.imp.downcast_ref::<Arc<A>>())
            .map(Arc::clone)
    }

    /// Returns `true` when an implementation of the aspect `A` is attached.
    pub fn has<A: ?Sized + Any>(&self) -> bool {
        let id = TypeId::of::<A>();
        self.entries.iter().any(|entry| entry.id == id)
    }

    /// The attached implementations, in the order they were attached, each as
    /// the aspect's `TypeId`, the aspect's type name and the implementation
    /// with its type erased.
    ///
    /// The registry reads these to build its tables per aspect and its digest,
    /// and each entry can be passed back to [`AspectMap::insert_erased`]
    /// unchanged.
    pub fn entries(
        &self,
    ) -> impl Iterator<Item = (TypeId, &'static str, Arc<dyn Any + Send + Sync>)> + '_ {
        self.entries.iter().map(|entry| (entry.id, entry.name, Arc::clone(&entry.imp)))
    }

    /// The `TypeId`s of the attached aspects, in the order they were attached.
    /// Each is the `TypeId` of an aspect's trait object type.
    pub fn aspect_ids(&self) -> impl Iterator<Item = TypeId> + '_ {
        self.entries.iter().map(|entry| entry.id)
    }

    /// The number of attached aspects.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no aspect is attached.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Debug for AspectMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.entries.iter().map(|entry| entry.name)).finish()
    }
}

/// One element: the value that says what a node means, with a stable type
/// name, a one-line description, the aspects attached to it and its own data.
///
/// An element is the counterpart of the parser's specification of a command (a
/// techy callable specification; techy is the parsing library FLM is built
/// on): the specification says how a construct is *parsed*, the element says what it
/// *means*. An element holds its type name, description, aspects and data,
/// and nothing else; what a backend does with an element it cannot render is
/// decided by the plan request, not by the element.
///
/// One `Arc<ElementSpec>` is one **element value**, and an element is
/// identified by its element value (see [`ElementSpec::same_value`]).
/// Several command names may share one element value (a feature may declare
/// two commands with the same meaning under one element value), and several element values may share one type name (`\section` and
/// `\subsection` are two element values of type `flm.heading`). An element
/// value does not change once it is shared: [`ElementSpec::new`] and the
/// builder methods [`docs`](ElementSpec::docs),
/// [`with_value`](ElementSpec::with_value) and [`with`](ElementSpec::with) fill
/// it in before the `Arc` is made. When another feature attaches an aspect to
/// an element type
/// ([`FeatureBuilder::attach`](crate::core::registry::FeatureBuilder::attach)),
/// the registry build replaces each element value of that type by a copy that
/// carries the added aspect.
///
/// A feature normally builds its elements while it registers its declarations
/// ([`ElementDef`](crate::lang::mint::ElementDef)); a node's element is read
/// with [`element_of`].
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::element::ElementSpec;
///
/// // An aspect is any object-safe trait; nothing registers it anywhere.
/// trait Glossary: Send + Sync {
///     fn term(&self) -> &str;
/// }
/// struct Term(&'static str);
/// impl Glossary for Term {
///     fn term(&self) -> &str { self.0 }
/// }
///
/// let element = ElementSpec::new("thm.term")
///     .docs("a glossary term")
///     .with::<dyn Glossary>(Arc::new(Term("stabilizer")));
///
/// assert_eq!(element.name.as_str(), "thm.term");
/// assert!(element.has_aspect::<dyn Glossary>());
/// assert_eq!(element.aspect::<dyn Glossary>().map(|g| g.term()), Some("stabilizer"));
/// ```
#[derive(Clone)]
pub struct ElementSpec {
    /// The element's type name, such as `flm.heading`. Several element values
    /// may share it.
    pub name: ElementName,
    /// A one-line description of the element, for a person who reads the
    /// registry's declarations. The library stores it with the element and does
    /// not display it.
    ///
    /// It is an owned `String` rather than a `&'static str` because a family
    /// of elements may be built from a configuration while the registry is
    /// built, for example one element per theorem kind, each with a
    /// description computed then.
    pub docs: String,
    /// The aspects attached to this element.
    pub aspects: AspectMap,
    /// The element's own data, or `None` for an element without data: usually
    /// one struct type per element type, whose values tell the element values
    /// of that type apart (a heading element holds its level, for instance).
    /// The aspect implementations that need the data hold it as well.
    ///
    /// Read it with [`ElementSpec::value_as`] given the element, or with
    /// [`element_value`] given a node.
    pub value: Option<Arc<dyn Any + Send + Sync>>,
}

impl ElementSpec {
    /// A new element of the type name `name`, with an empty description, no
    /// aspects and no data.
    ///
    /// The builder methods [`ElementSpec::docs`], [`ElementSpec::with_value`]
    /// and [`ElementSpec::with`] fill it in; once it is shared as an `Arc`, it
    /// is one element value.
    pub fn new(name: impl Into<ElementName>) -> ElementSpec {
        ElementSpec {
            name: name.into(),
            docs: String::new(),
            aspects: AspectMap::new(),
            value: None,
        }
    }

    /// Builder: sets the element's one-line description, replacing any set
    /// before.
    pub fn docs(mut self, docs: impl Into<String>) -> ElementSpec {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Builder: sets the element's own data, replacing any set before. Read it
    /// back with [`ElementSpec::value_as`] or [`element_value`].
    pub fn with_value(mut self, value: Arc<dyn Any + Send + Sync>) -> ElementSpec {
        self.value = Some(value);
        self
    }

    /// Builder: attaches `imp` as the implementation of the aspect `A`.
    ///
    /// `A` is always written as the aspect's **trait object type**, as in
    /// `.with::<dyn BlockRoleAspect>(Arc::clone(&value))`, never as the
    /// implementing struct. A call whose `A` is inferred as the concrete type
    /// does not compile (see [`AspectMap::insert`]), so a forgotten `dyn` is
    /// caught where it is written.
    ///
    /// # Panics
    ///
    /// Panics when an implementation of `A` is already attached. Attaching one
    /// aspect twice while building an element is a mistake in the feature that
    /// builds it, which no document can cause. Code that attaches aspects from
    /// several sources, such as the registry, uses the fallible
    /// [`AspectMap::insert`] instead.
    pub fn with<A: ?Sized + Any + Send + Sync>(mut self, imp: Arc<A>) -> ElementSpec {
        if let Err(collision) = self.aspects.insert::<A>(imp) {
            panic!(
                "element `{}`: {} (code that may attach an aspect twice uses the fallible \
                 `AspectMap::insert` rather than `ElementSpec::with`)",
                self.name, collision
            );
        }
        self
    }

    /// The implementation of the aspect `A` attached to this element, or
    /// `None` when none is attached.
    ///
    /// Written `element.aspect::<dyn Referenceable>()`. Shorthand for
    /// [`AspectMap::get`] on [`ElementSpec::aspects`]. For the standard aspects
    /// that have a default, read the result through the function of
    /// [`crate::model::aspects`] that supplies it (such as
    /// [`block_role_of`](crate::model::aspects::block_role_of)).
    pub fn aspect<A: ?Sized + Any>(&self) -> Option<&A> {
        self.aspects.get::<A>()
    }

    /// Returns `true` when an implementation of the aspect `A` is attached to
    /// this element.
    ///
    /// Shorthand for [`AspectMap::has`] on [`ElementSpec::aspects`].
    pub fn has_aspect<A: ?Sized + Any>(&self) -> bool {
        self.aspects.has::<A>()
    }

    /// The element's own data as a `V`.
    ///
    /// Returns `None` when the element carries no data, or when its data is not
    /// of type `V`. See [`element_value`] for the same read starting from a
    /// node.
    pub fn value_as<V: Any + Send + Sync>(&self) -> Option<&V> {
        self.value.as_deref().and_then(|value| value.downcast_ref::<V>())
    }

    /// Returns `true` when the two handles point at the same allocation, that
    /// is, denote the same element value.
    ///
    /// An element is identified by its allocation, never by its name and never
    /// by a Rust type: two elements built separately with the same name are
    /// different element values. The tree check of the block-structure
    /// rewrites ([`crate::lang::rewrites::blocks::check`]) compares elements
    /// this way, and the registry numbers element values ([`ElemIdx`]) by
    /// their allocation.
    pub fn same_value(this: &Arc<ElementSpec>, other: &Arc<ElementSpec>) -> bool {
        Arc::ptr_eq(this, other)
    }
}

impl fmt::Debug for ElementSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementSpec")
            .field("name", &self.name)
            .field("aspects", &self.aspects)
            .field("value", &self.value.as_ref().map(|_| "…"))
            .finish()
    }
}

/// The element of the node `node`.
///
/// Every node of every FLM tree carries its element in its node extension, so
/// this reads a field: it cannot fail, needs no lookup, and works on parsed
/// trees and on trees produced by a rewrite alike.
pub fn element_of<'t>(node: &NodeRef<'t, Flm>) -> &'t Arc<ElementSpec> {
    &node.ext().element
}

/// The data of the element of the node `node`, as a `V`.
///
/// Returns `None` when the element carries no data, or when its data is not
/// of type `V`. Written `element_value::<HeadingElement>(node)`; the same read
/// as [`ElementSpec::value_as`] on [`element_of`]`(node)`.
pub fn element_value<'t, V: Any + Send + Sync>(node: &NodeRef<'t, Flm>) -> Option<&'t V> {
    node.ext().element.value.as_deref().and_then(|value| value.downcast_ref::<V>())
}

/// The error of looking up an element value by a type name that several
/// registered element values share.
///
/// A type name identifies an element *type*, and one type may have several
/// element values (every heading level is an element value of type
/// `flm.heading`), so a type name alone does not always say which element
/// value is meant. The type names that must be looked up this way, those of
/// synthesized elements and of presentation elements, must each belong to
/// exactly one element value; this error reports a type name that does not,
/// instead of taking the first match.
///
/// Two lookups return it: the registry's [lookup by type name][by-name], and
/// the element resolver that reading a stored tree back uses to find a node's
/// element from its type name ([`crate::dump`]). It is defined in this module,
/// below both, so that they share one type.
///
/// [by-name]: crate::core::registry::FlmRegistry::element_value
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguousElement {
    /// The type name that several element values share.
    pub name: ElementName,
    /// The feature that registered each element value of that type name, in
    /// the order the lookup holds them: one entry per element value, so a
    /// feature that registered two of them appears twice. The element resolver
    /// of [`crate::dump`], which has no record of which feature registered each
    /// element value, gives
    /// [`HOST_FEATURE`](crate::core::registry::HOST_FEATURE) for each one
    /// instead.
    pub owners: Vec<crate::model::names::FeatureName>,
}

impl fmt::Display for AmbiguousElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the element type name `{}` must belong to exactly one registered element, \
             but {} elements are registered under it (by {})",
            self.name,
            self.owners.len(),
            self.owners.iter().map(|o| format!("`{o}`")).collect::<Vec<_>>().join(", ")
        )
    }
}

impl std::error::Error for AmbiguousElement {}

#[cfg(test)]
mod tests {
    use super::*;

    trait Glossary: Send + Sync {
        fn term(&self) -> &str;
    }

    struct Term(&'static str);

    impl Glossary for Term {
        fn term(&self) -> &str {
            self.0
        }
    }

    #[test]
    fn aspect_map_finds_an_attached_aspect_by_trait_object_type() {
        let mut map = AspectMap::new();
        map.insert::<dyn Glossary>(Arc::new(Term("stabilizer"))).unwrap();
        assert_eq!(map.get::<dyn Glossary>().map(|g| g.term()), Some("stabilizer"));
        assert!(map.has::<dyn Glossary>());
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn aspect_map_reports_a_second_attachment_of_one_aspect() {
        let mut map = AspectMap::new();
        map.insert::<dyn Glossary>(Arc::new(Term("a"))).unwrap();
        let collision = map.insert::<dyn Glossary>(Arc::new(Term("b"))).unwrap_err();
        assert_eq!(collision.aspect, std::any::type_name::<dyn Glossary>());
        assert_eq!(map.get::<dyn Glossary>().map(|g| g.term()), Some("a"));
    }

    #[test]
    fn erased_entries_round_trip_through_insert_erased() {
        let mut map = AspectMap::new();
        map.insert::<dyn Glossary>(Arc::new(Term("t"))).unwrap();
        let mut copy = AspectMap::new();
        for (id, name, imp) in map.entries() {
            copy.insert_erased(id, name, imp).unwrap();
        }
        assert_eq!(copy.get::<dyn Glossary>().map(|g| g.term()), Some("t"));
    }

    #[test]
    fn element_value_downcasts_to_the_declared_type() {
        struct Heading {
            level: u8,
        }
        let value = Arc::new(Heading { level: 2 });
        let element = ElementSpec::new("flm.heading").docs("a heading").with_value(value);
        assert_eq!(element.value_as::<Heading>().map(|h| h.level), Some(2));
        assert!(element.value_as::<Term>().is_none());
    }

    #[test]
    #[should_panic]
    fn attaching_one_aspect_twice_on_the_builder_panics() {
        let _ = ElementSpec::new("flm.term")
            .with::<dyn Glossary>(Arc::new(Term("a")))
            .with::<dyn Glossary>(Arc::new(Term("b")));
    }

    #[test]
    fn identity_is_the_shared_value_not_the_name() {
        let a = Arc::new(ElementSpec::new("flm.heading"));
        let b = Arc::new(ElementSpec::new("flm.heading"));
        assert!(ElementSpec::same_value(&a, &Arc::clone(&a)));
        assert!(!ElementSpec::same_value(&a, &b));
    }
}
