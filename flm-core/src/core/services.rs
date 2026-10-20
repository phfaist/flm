//! Services: values that one feature supplies for another feature, or for the
//! program embedding the library, to use, and the service slots through which
//! they are declared, supplied and read.
//!
//! A service is a value that can be shared across threads (behavior behind a
//! trait object, or plain data) which one feature supplies for another
//! feature, or for the embedding program, to use: something that fetches
//! citation data, something that renders mathematics, or plain data another
//! feature reads. A service slot is the declaration, by the feature that owns
//! an interaction, of a dotted name, the type of the values the slot takes and
//! its arity (at most one value, or any number, [`Arity`]); any feature, and
//! the embedding program through the registry builder, may contribute a value
//! to it, and the registry build assembles all contributions once. A
//! contribution is a value a feature places in a service slot; a peer
//! contribution is a contribution to a slot another feature owns, which the
//! registry build drops with one Note when that feature is not registered.
//! The section [services and service
//! slots](crate::guide::concepts_overview#services-and-service-slots) of the
//! concepts overview relates them to the rest of the library.
//!
//! A service slot makes the interaction safe: the feature that needs a service
//! states what it needs and of which type, instead of checking at run time
//! whether some other feature is present and then calling a method that may
//! not exist.
//!
//! This module holds the values the registry build records and checks
//! ([`SlotDecl`], [`Contribution`], [`SlotTarget`], [`ServiceRequirement`]),
//! the assembly itself ([`assemble`], with its errors [`ServiceError`]), and
//! the finished, read-only result: [`Services`], with the record of every peer
//! contribution ([`PeerContribution`]). A feature does not build these values
//! itself; it calls the methods of the feature builder listed below.
//!
//! # Declaring, contributing, reading
//!
//! - A feature declares a service slot with
//!   [`FeatureBuilder::service_slot`](crate::core::registry::FeatureBuilder::service_slot).
//! - A feature contributes a value with
//!   [`FeatureBuilder::contribute`](crate::core::registry::FeatureBuilder::contribute)
//!   (the slot found by the value's type) or
//!   [`FeatureBuilder::contribute_to`](crate::core::registry::FeatureBuilder::contribute_to)
//!   (the slot given by name); the forms `contribute_first` and
//!   `contribute_first_to` place the value ahead of the others in a slot that
//!   takes several. Where a slot's order is a precedence, the values of the
//!   feature placed last in the feature order come first (see [the order of a
//!   slot's values](#the-order-of-a-slots-values)). The embedding program contributes through
//!   [`RegistryBuilder::contribute`](crate::core::registry::RegistryBuilder::contribute)
//!   and
//!   [`RegistryBuilder::contribute_to`](crate::core::registry::RegistryBuilder::contribute_to),
//!   attributed to the name
//!   [`HOST_FEATURE`](crate::core::registry::HOST_FEATURE). The registry build
//!   checks contributions after every feature has registered, so a feature may
//!   contribute to a service slot that a feature registered later declares.
//! - A contribution to a service slot that no feature declares is a registry
//!   build error, since it usually comes from a misspelled name. A feature that
//!   contributes to a service slot of another feature that may not be
//!   registered uses
//!   [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)
//!   instead: that peer contribution is dropped when no feature declares the
//!   slot, and the registry build records one Note. A feature can therefore
//!   offer a value to a feature that may not be registered.
//! - A stage reads a service from its context, with
//!   [`StageCx::service`](crate::core::run::StageCx::service) for one value and
//!   [`StageCx::services`](crate::core::run::StageCx::services) for all of
//!   them. The read of one value returns an `Option`: an absent service is
//!   never a failure in the middle of a render, and the stage reports the
//!   absence as a diagnostic of its own.
//! - A feature that cannot work in *any* plan without a service marks the
//!   service slot as required, with [`require_service`][req] (the slot found
//!   by type) or
//!   [`require_service_named`](crate::core::registry::FeatureBuilder::require_service_named);
//!   a required slot that receives no value is then an error of the registry
//!   build. A service that only some plans need stays an ordinary read that
//!   may return `None`, so that a program that never builds such a plan can
//!   still build its registry.
//!
//! # Service slots do not change once the registry is built
//!
//! [`assemble`] produces the [`Services`] of a registry, and nothing changes
//! them afterwards: every run shares them, and no service is filled while a
//! run is under way. A value that exists only after some work has been done
//! (an index built by scanning a collection of documents, citation data
//! fetched ahead of time) is not a service but an [external
//! table](crate::core::run::Externals), installed when a run is constructed. A
//! value that is empty at one moment and filled at another would make the
//! result of a run depend on when a stage read it.
//!
//! A service implementation may keep state of its own behind a lock, on one
//! condition: returning a stored result must give the same result as
//! computing it again. Apart from such caches, nothing that a stage can reach
//! changes while a run is under way.
//!
//! # Finding a service slot by type or by name
//!
//! A service slot is stored under its name, and the registry build checks the
//! type of every contribution against the slot's declared type. A
//! contribution or a read may still identify the slot by its value type
//! alone, which works as long as exactly one slot has that type. Where two
//! slots have the same type, the forms that take the slot's name
//! ([`Services::service_named`], [`Services::services_named`],
//! `contribute_to`) select one, and a contribution by type alone is refused
//! as ambiguous ([`ServiceError::AmbiguousSlot`]). The reads by type alone
//! take the slots of that type in name order: [`Services::service`] returns
//! the value that takes precedence in the first such slot that holds one, and
//! [`Services::services`] every value of every such slot.
//!
//! # The order of a slot's values
//!
//! A slot that takes several values holds them in two orders, and each read
//! says which one it gives.
//!
//! - The **assembled order** ([`Services::services_named`]): the values
//!   contributed with `contribute_first` or `contribute_first_to` first, then
//!   the others, each group in the order the values were contributed (the
//!   features in their registration order, then the embedding program). A
//!   slot whose values form a list, such as definitions written one after the
//!   other, is read in this order.
//! - The **order of precedence** ([`Services::services_by_precedence_named`]):
//!   the values contributed with `contribute_first` or `contribute_first_to`
//!   first, then the others; within each of these two groups, the values of
//!   the feature placed last in the feature order first, each feature's values
//!   in the order the feature contributed them, and the embedding program's
//!   values last. A slot whose values are asked in turn until one answers,
//!   such as the citation providers or the image converters, is read in this
//!   order, and the reads of one value ([`Services::service`],
//!   [`Services::service_named`]) give the first value in it: between
//!   features, the feature placed last wins.
//!
//! # Examples
//!
//! One feature declares a service slot, another feature contributes a value
//! to it, and the finished registry returns the value:
//!
//! ```
//! use std::sync::Arc;
//!
//! use flm_core::core::registry::{
//!     BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry,
//! };
//! use flm_core::core::services::Arity;
//!
//! // A service is usually behavior behind a trait object.
//! trait Greeter: Send + Sync {
//!     fn greet(&self) -> String;
//! }
//!
//! struct Hello;
//! impl Greeter for Hello {
//!     fn greet(&self) -> String {
//!         "hello".to_string()
//!     }
//! }
//!
//! struct Owner;
//! impl FlmFeature for Owner {
//!     fn info(&self) -> FeatureInfo { FeatureInfo::new("owner", "Owner") }
//!     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
//!         b.service_slot::<dyn Greeter>("owner.greeter", Arity::Single, "the greeter")
//!     }
//! }
//!
//! struct Filler;
//! impl FlmFeature for Filler {
//!     fn info(&self) -> FeatureInfo { FeatureInfo::new("filler", "Filler") }
//!     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
//!         b.contribute_to("owner.greeter", Arc::new(Hello) as Arc<dyn Greeter>)
//!     }
//! }
//!
//! let registry = build_registry([
//!     Arc::new(Owner) as Arc<dyn FlmFeature>,
//!     Arc::new(Filler) as Arc<dyn FlmFeature>,
//! ])?;
//!
//! let greeter =
//!     registry.services().service::<dyn Greeter>().expect("contributed above");
//! assert_eq!(greeter.greet(), "hello");
//! # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
//! ```
//!
//! [req]: crate::core::registry::FeatureBuilder::require_service

use std::any::Any;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::model::names::{FeatureName, Name};
use crate::model::options::TypeKey;

/// The arity of a service slot: how many values it takes.
///
/// A feature gives it when it declares the slot
/// ([`FeatureBuilder::service_slot`](crate::core::registry::FeatureBuilder::service_slot)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arity {
    /// At most one value: the slot may stay empty, and a second contribution
    /// is a registry build error ([`ServiceError::SingleSlotRefilled`]) that
    /// identifies both contributing features; neither value replaces the other.
    Single,
    /// Any number of values: the values contributed with `contribute_first`
    /// or `contribute_first_to` come first, then the others, each group in the
    /// order the values were contributed. A read for precedence orders them
    /// otherwise within each group (see [the order of a slot's
    /// values](self#the-order-of-a-slots-values)).
    OrderedMany,
}

/// A feature's declaration of one service slot: its name, the type of its
/// values, its arity and its owner.
///
/// [`FeatureBuilder::service_slot`](crate::core::registry::FeatureBuilder::service_slot)
/// records one; [`Services::slot`] and [`Services::slots`] return them from the
/// finished registry.
#[derive(Debug, Clone)]
pub struct SlotDecl {
    /// The service slot's dotted name, such as `flm.citations.providers`,
    /// which stays the same across versions of the feature.
    pub name: Name,
    /// The type of the slot's values: usually a trait object type, since a
    /// service is usually behavior.
    pub ty: TypeKey,
    /// How many values the slot takes.
    pub arity: Arity,
    /// The feature that declares the slot, and so owns the interaction.
    pub owner: FeatureName,
    /// A description of what the slot is for, for a person reading the
    /// registry; empty when the feature gave none.
    pub docs: String,
}

/// How a contribution or a requirement identifies its service slot: by the
/// slot's value type, or by its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotTarget {
    /// The service slot whose values have this type. It identifies a slot only
    /// when exactly one declared slot has the type; several such slots are
    /// reported as [`ServiceError::AmbiguousSlot`], and none is chosen.
    ByType(TypeKey),
    /// The service slot with this name, regardless of its type.
    ByName(Name),
}

impl fmt::Display for SlotTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotTarget::ByType(ty) => write!(f, "the service slot of type `{ty}`"),
            SlotTarget::ByName(name) => write!(f, "the service slot `{name}`"),
        }
    }
}

type ErasedService = Arc<dyn Any + Send + Sync>;

/// A placed value with what decides its precedence: its group (`false` for
/// the values contributed to come first), the position of its feature in the
/// final order, reversed so that the feature placed last sorts first, and
/// its place among the slot's contributions.
type Ranked = (bool, Reverse<Option<usize>>, usize, ErasedService);

/// One value offered to a service slot, as the registry builder records it.
///
/// The feature builder's `contribute` methods and
/// [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)
/// create these. [`assemble`] checks each one against the slot's declaration
/// and, when it fits, places the value in the slot.
pub struct Contribution {
    /// The feature that contributes the value, or
    /// [`HOST_FEATURE`](crate::core::registry::HOST_FEATURE) when the
    /// embedding program contributes it through the registry builder.
    pub owner: FeatureName,
    /// The service slot the value is offered to.
    pub target: SlotTarget,
    /// The value itself, held type-erased.
    pub value: ErasedService,
    /// The type the value was contributed as, checked against the slot's
    /// declared type.
    pub ty: TypeKey,
    /// Whether the value precedes, in a slot that takes several values, the
    /// contributions that do not ask to come first.
    pub prepend: bool,
    /// Whether this is a **peer contribution**: a value offered to a service
    /// slot another feature owns, which is dropped, rather than failing the
    /// registry build, when no feature of the registry declares that slot.
    ///
    /// A plain contribution to a slot no feature declares is
    /// [`ServiceError::UndeclaredSlot`], since it usually comes from a
    /// misspelled name. A peer contribution states instead that the slot
    /// belongs to another feature, which may or may not be registered: when no
    /// feature declares the slot, [`assemble`] drops the value and records the
    /// contribution as unresolved ([`Services::peer_contributions`]), and the
    /// registry build reports one Note. Set it with [`Contribution::as_peer`].
    ///
    /// This applies only to a contribution that gives its slot's name
    /// ([`SlotTarget::ByName`]). A contribution that identifies its slot by
    /// type alone has no name the build could report as unresolved, so a type
    /// that no slot has stays [`ServiceError::UndeclaredSlot`].
    pub peer: bool,
}

impl Contribution {
    /// Returns the contribution of the value `imp` to the service slot
    /// `target`, by the feature `owner`; `prepend` says whether the value
    /// comes first ([`Contribution::prepend`]).
    ///
    /// Write `S` as the type the slot declares: a trait object type, as in
    /// `Contribution::new::<dyn Fetcher>(…)`, whenever the service is
    /// behavior. A concrete `Arc<MyFetcher>` may still be passed as `imp`,
    /// because Rust converts it at the call.
    pub fn new<S: ?Sized + Any + Send + Sync>(
        owner: impl Into<FeatureName>,
        target: SlotTarget,
        imp: Arc<S>,
        prepend: bool,
    ) -> Contribution {
        // `S` may be unsized (a trait object), and an `Arc<dyn Trait>` cannot
        // be erased to `dyn Any` — only a sized value can — so the sized
        // `Arc<S>` is itself put behind one more `Arc`: the stored value is an
        // `Arc<Arc<S>>` erased as `Arc<dyn Any>`, and `contents_of` downcasts
        // it back to `Arc<S>`.
        Contribution {
            owner: owner.into(),
            target,
            value: Arc::new(imp),
            ty: TypeKey::of::<S>(),
            prepend,
            peer: false,
        }
    }

    /// Marks this contribution as a [peer contribution](Contribution::peer),
    /// and returns it: the service slot belongs to another feature, and the
    /// value is dropped, rather than the registry build failing, when no
    /// feature declares the slot.
    pub fn as_peer(mut self) -> Contribution {
        self.peer = true;
        self
    }
}

impl fmt::Debug for Contribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Contribution")
            .field("owner", &self.owner)
            .field("target", &self.target)
            .field("ty", &self.ty)
            .field("prepend", &self.prepend)
            .field("peer", &self.peer)
            .finish()
    }
}

/// A feature's statement that it cannot work in any plan unless a service
/// slot receives at least one value; a required slot that receives none is a
/// registry build error ([`ServiceError::RequiredSlotUnfilled`]).
///
/// [`FeatureBuilder::require_service`](crate::core::registry::FeatureBuilder::require_service)
/// and
/// [`FeatureBuilder::require_service_named`](crate::core::registry::FeatureBuilder::require_service_named)
/// record one. A service that only some plans need is not required this way:
/// the stage that needs it reads an `Option` and reports the absence itself,
/// so that a program that never builds such a plan can still build its
/// registry.
#[derive(Debug, Clone)]
pub struct ServiceRequirement {
    /// The feature that requires the slot to be filled.
    pub owner: FeatureName,
    /// The service slot that must receive at least one value.
    pub target: SlotTarget,
}

/// The record of one [peer contribution](Contribution::peer) in the finished
/// registry: which feature offered a value to which service slot, and whether
/// any feature of the registry declares that slot.
///
/// A feature makes a peer contribution with
/// [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot).
/// Read the records through [`Services::peer_contributions`], or through
/// [`FlmRegistry::peer_slots`](crate::core::registry::FlmRegistry::peer_slots)
/// together with the other peer declarations, so that a misspelled slot name
/// can be found on any registry that holds the feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerContribution {
    /// The feature that contributed the value.
    pub owner: FeatureName,
    /// The name of the service slot the value was offered to.
    pub slot: Name,
    /// The type the value was contributed as, which is the peer declaration's
    /// signature: when the slot is declared, [`assemble`] checked this type
    /// against the slot's own.
    pub ty: TypeKey,
    /// Whether some feature of this registry declares the slot. When `false`,
    /// the value was dropped and the registry build recorded one Note.
    pub resolved: bool,
}

/// A problem that makes [`assemble`] refuse the service slot declarations,
/// contributions and requirements of a registry.
///
/// The registry build reports each one as a build error
/// ([`BuildError::Service`](crate::core::registry::BuildError::Service)).
/// Every variant identifies the features involved, so that the report says
/// which feature to correct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    /// Two features declared a service slot under one name. A slot name
    /// identifies one interaction, owned by one feature.
    DuplicateSlot {
        /// The name declared twice.
        slot: Name,
        /// The feature that declared it first.
        first: FeatureName,
        /// The feature that declared it again.
        second: FeatureName,
    },
    /// A contribution or a requirement refers to a service slot that no
    /// feature declares.
    UndeclaredSlot {
        /// The feature that contributed or required.
        feature: FeatureName,
        /// The service slot it refers to.
        target: SlotTarget,
        /// The name of every declared service slot, so that the report can
        /// list the alternatives.
        roster: Vec<Name>,
    },
    /// A contribution or a requirement identifies its service slot by type
    /// alone, and several slots have that type. Giving the slot's name
    /// removes the ambiguity.
    AmbiguousSlot {
        /// The feature that contributed or required.
        feature: FeatureName,
        /// The type it gave.
        ty: TypeKey,
        /// The service slots that have that type, in name order.
        slots: Vec<Name>,
    },
    /// A value was contributed as a type other than the one its service slot
    /// declares.
    TypeMismatch {
        /// The contributing feature.
        feature: FeatureName,
        /// The service slot contributed to.
        slot: Name,
        /// The type the slot declares.
        declared: TypeKey,
        /// The type the value was contributed as.
        contributed: TypeKey,
    },
    /// A second value was contributed to a service slot that takes exactly
    /// one ([`Arity::Single`]).
    SingleSlotRefilled {
        /// The service slot.
        slot: Name,
        /// The feature whose value fills it.
        first: FeatureName,
        /// The feature that contributed a second value.
        second: FeatureName,
    },
    /// A service slot that a feature requires
    /// ([`ServiceRequirement`]) received no value.
    RequiredSlotUnfilled {
        /// The empty service slot.
        slot: Name,
        /// The feature that requires it.
        required_by: FeatureName,
    },
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::DuplicateSlot { slot, first, second } => {
                write!(
                    f,
                    "service slot `{slot}` must be declared only once, but feature `{first}` \
                     declares it and feature `{second}` declares it again"
                )
            }
            ServiceError::UndeclaredSlot { feature, target, roster } => write!(
                f,
                "feature `{feature}` refers to {target}, but no feature declares a service slot \
                 of that {}; declared service slots: [{}]",
                match target {
                    SlotTarget::ByName(_) => "name",
                    SlotTarget::ByType(_) => "type",
                },
                roster.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
            ),
            ServiceError::AmbiguousSlot { feature, ty, slots } => write!(
                f,
                "feature `{feature}` must refer to a service slot by name when several slots \
                 have the type `{ty}` ([{}])",
                slots.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
            ),
            ServiceError::TypeMismatch { feature, slot, declared, contributed } => write!(
                f,
                "a value contributed to service slot `{slot}` must have the slot's type \
                 `{declared}`, but feature `{feature}` contributes a `{contributed}`"
            ),
            ServiceError::SingleSlotRefilled { slot, first, second } => write!(
                f,
                "service slot `{slot}` takes at most one value, but feature `{first}` \
                 contributes one and feature `{second}` contributes another"
            ),
            ServiceError::RequiredSlotUnfilled { slot, required_by } => {
                write!(
                    f,
                    "service slot `{slot}` must receive a value, because feature \
                     `{required_by}` requires it"
                )
            }
        }
    }
}

impl std::error::Error for ServiceError {}

/// The service slots of a registry and the values contributed to them, fixed
/// once the registry is built.
///
/// [`assemble`] produces it during the registry build, and
/// [`FlmRegistry::services`](crate::core::registry::FlmRegistry::services)
/// returns it. Every run shares it, and nothing changes it. Read a service by
/// its type ([`Services::service`], [`Services::services`]) or, where two slots
/// have one type, by slot name ([`Services::service_named`],
/// [`Services::services_named`]). A stage normally reads a service through its
/// context ([`StageCx::service`](crate::core::run::StageCx::service)) rather
/// than here.
#[derive(Default)]
pub struct Services {
    slots: BTreeMap<Name, SlotDecl>,
    /// Each slot's values in the assembled order.
    contents: BTreeMap<Name, Vec<ErasedService>>,
    /// Each slot's values in the order of precedence.
    ranked: BTreeMap<Name, Vec<ErasedService>>,
    by_type: BTreeMap<TypeKey, Vec<Name>>,
    peers: Vec<PeerContribution>,
}

impl Services {
    /// Returns services with no slot and no value: what a registry holds when
    /// no feature declares a service slot.
    pub fn new() -> Services {
        Services::default()
    }

    /// Returns the declaration of the service slot named `name`, or `None`
    /// when no slot has that name.
    pub fn slot(&self, name: &str) -> Option<&SlotDecl> {
        self.slots.get(name)
    }

    /// Returns every declared service slot, in name order.
    ///
    /// The order does not depend on the order in which the features
    /// registered, so a listing built from it is the same in every build.
    pub fn slots(&self) -> impl Iterator<Item = &SlotDecl> + '_ {
        self.slots.values()
    }

    /// Returns the names of the service slots whose values have the type `S`,
    /// in name order.
    ///
    /// When it returns more than one name, a contribution by type alone is
    /// refused as ambiguous, and a read by type alone takes the slots in this
    /// order; the operations that take a slot name select one slot.
    pub fn slots_of<S: ?Sized + Any>(&self) -> &[Name] {
        self.by_type.get(&TypeKey::of::<S>()).map(Vec::as_slice).unwrap_or(&[])
    }

    fn contents_of<S: ?Sized + Any + Send + Sync>(
        &self,
        name: &str,
    ) -> impl Iterator<Item = &S> + '_ {
        self.shared_named::<S>(name).map(|arc| &**arc)
    }

    /// Returns one service of type `S`: of the service slots whose values have
    /// that type, taken in name order, the first that holds a value gives the
    /// value that takes precedence in it (the first in the order of
    /// [`Services::services_by_precedence_named`]).
    ///
    /// Write `S` as the type the slot declares:
    /// `services.service::<dyn Fetcher>()`. Returns `None` when no slot has the
    /// type, or when every such slot is empty; an absent service is a normal
    /// result, and the caller decides what to do about it.
    ///
    /// See also [`Services::service_named`], for a slot named directly.
    pub fn service<S: ?Sized + Any + Send + Sync>(&self) -> Option<&S> {
        self.slots_of::<S>().iter().find_map(|name| self.service_named::<S>(name))
    }

    /// Returns every service of type `S`: the values of every service slot
    /// whose values have that type, the slots taken in name order and the
    /// values of each slot in their assembled order (see [`Arity::OrderedMany`]).
    pub fn services<S: ?Sized + Any + Send + Sync>(&self) -> impl Iterator<Item = &S> + '_ {
        self.slots_of::<S>().iter().flat_map(|name| self.contents_of::<S>(name))
    }

    /// Returns the value of the service slot named `name` that takes
    /// precedence, read as an `S`: the first in the order of
    /// [`Services::services_by_precedence_named`], so that between features,
    /// the value of the feature placed last in the feature order.
    ///
    /// Returns `None` when the slot does not exist, holds no value, or holds
    /// values of another type.
    pub fn service_named<S: ?Sized + Any + Send + Sync>(&self, name: &str) -> Option<&S> {
        self.services_by_precedence_named::<S>(name).next()
    }

    /// Returns every value of the service slot named `name`, read as `S`, in
    /// their assembled order: the values contributed with `contribute_first`
    /// or `contribute_first_to` come first, then the others, each group in the
    /// order the values were contributed. Returns nothing when the slot does
    /// not exist or holds values of another type.
    pub fn services_named<S: ?Sized + Any + Send + Sync>(
        &self,
        name: &str,
    ) -> impl Iterator<Item = &S> + '_ {
        self.contents_of::<S>(name)
    }

    /// Returns every value of the service slot named `name`, read as `S`, in
    /// the order of precedence: the values contributed with
    /// `contribute_first` or `contribute_first_to` first, then the others;
    /// within each of these two groups, the values of the feature placed last
    /// in the feature order first, each feature's values in the order the
    /// feature contributed them, and the embedding program's values last.
    /// Returns nothing when the slot does not exist or holds values of another
    /// type.
    ///
    /// Read a slot in this order where its values are asked in turn until one
    /// answers, so that between features, the feature placed last wins (see
    /// [the order of a slot's values](self#the-order-of-a-slots-values)).
    pub fn services_by_precedence_named<S: ?Sized + Any + Send + Sync>(
        &self,
        name: &str,
    ) -> impl Iterator<Item = &S> + '_ {
        self.ranked
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(|value| value.downcast_ref::<Arc<S>>())
            .map(|arc| &**arc)
    }

    /// Returns every value of the service slot named `name`, read as `S`, in
    /// the order of [`Services::services_named`], each as the shared pointer
    /// it was contributed as.
    ///
    /// Use it to keep a service beyond the registry build. A lowering rule,
    /// for example, has no access to the registry's services while it runs,
    /// so a finalizer ([`Finalizer`](crate::core::registry::Finalizer)) hands
    /// it the values of a slot: the pointer is cloned, and the value itself is
    /// shared, not copied.
    pub fn shared_named<S: ?Sized + Any + Send + Sync>(
        &self,
        name: &str,
    ) -> impl Iterator<Item = &Arc<S>> + '_ {
        self.contents
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(|value| value.downcast_ref::<Arc<S>>())
    }

    /// Returns how many values the service slot named `name` holds: zero for
    /// a slot that is empty or does not exist.
    pub fn count(&self, name: &str) -> usize {
        self.contents.get(name).map(Vec::len).unwrap_or(0)
    }

    /// Returns the record of every [peer contribution](Contribution::peer)
    /// the features made, in the order they made them, each stating whether
    /// its service slot is declared.
    ///
    /// A peer contribution whose slot no feature declares has `resolved` set
    /// to `false`, and its value was dropped when the slots were assembled.
    /// Every peer contribution has its own record, while the registry build
    /// reports one Note per feature and slot.
    pub fn peer_contributions(&self) -> &[PeerContribution] {
        &self.peers
    }
}

impl fmt::Debug for Services {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.slots.keys().map(|name| (name, self.count(name)))).finish()
    }
}

// Resolves a target to the name of the slot it means, against the declared
// slots: a name is checked for existence, a type must be carried by exactly one
// slot.
fn resolve_target(
    feature: &FeatureName,
    target: &SlotTarget,
    slots: &BTreeMap<Name, SlotDecl>,
    by_type: &BTreeMap<TypeKey, Vec<Name>>,
) -> Result<Name, ServiceError> {
    match target {
        SlotTarget::ByName(name) => {
            if slots.contains_key(name) {
                Ok(name.clone())
            } else {
                Err(ServiceError::UndeclaredSlot {
                    feature: feature.clone(),
                    target: target.clone(),
                    roster: slots.keys().cloned().collect(),
                })
            }
        }
        SlotTarget::ByType(ty) => match by_type.get(ty).map(Vec::as_slice) {
            Some([name]) => Ok(name.clone()),
            Some(names) if names.len() > 1 => Err(ServiceError::AmbiguousSlot {
                feature: feature.clone(),
                ty: *ty,
                slots: names.to_vec(),
            }),
            _ => Err(ServiceError::UndeclaredSlot {
                feature: feature.clone(),
                target: target.clone(),
                roster: slots.keys().cloned().collect(),
            }),
        },
    }
}

/// Assembles the [`Services`] of a registry from the service slots the
/// features declared, the values they contributed and the slots they require.
///
/// The registry build calls it once, after every feature has registered;
/// a feature does not call it. Each contribution is matched to its service
/// slot, checked against the slot's declared type and arity, and placed: the
/// values contributed with `contribute_first` or `contribute_first_to` come
/// first, then the others, each group in the order the values were
/// contributed. `features` gives the features in their final order, from
/// which the order of precedence of each slot's values follows (see [the
/// order of a slot's values](self#the-order-of-a-slots-values)); a
/// contribution whose owner is not among them, such as the embedding
/// program's, comes after every feature's in its group. Each requirement is
/// then checked against the filled slots.
///
/// A [peer contribution](Contribution::peer), a value offered to a slot
/// another feature owns, is the only contribution that a missing slot does
/// not make an error: its value is dropped and the contribution is recorded as
/// unresolved ([`Services::peer_contributions`]), for the registry build to
/// report as a Note. Every peer contribution, dropped or not, is recorded
/// there.
///
/// # Errors
///
/// Returns every problem found, not only the first, so that a registry can be
/// corrected in one pass. The problems are in the order they were found: the
/// declarations first, then the contributions, then the requirements, each in
/// registration order. See [`ServiceError`] for each problem.
pub fn assemble(
    decls: Vec<SlotDecl>,
    contributions: Vec<Contribution>,
    requirements: Vec<ServiceRequirement>,
    features: &[FeatureName],
) -> Result<Services, Vec<ServiceError>> {
    let mut errors = Vec::new();
    let mut slots: BTreeMap<Name, SlotDecl> = BTreeMap::new();
    let mut by_type: BTreeMap<TypeKey, Vec<Name>> = BTreeMap::new();
    for decl in decls {
        if let Some(existing) = slots.get(&decl.name) {
            errors.push(ServiceError::DuplicateSlot {
                slot: decl.name.clone(),
                first: existing.owner.clone(),
                second: decl.owner.clone(),
            });
            continue;
        }
        by_type.entry(decl.ty).or_default().push(decl.name.clone());
        slots.insert(decl.name.clone(), decl);
    }
    for names in by_type.values_mut() {
        names.sort();
    }

    let mut prepends: BTreeMap<Name, Vec<ErasedService>> = BTreeMap::new();
    let mut appends: BTreeMap<Name, Vec<ErasedService>> = BTreeMap::new();
    let mut ranking: BTreeMap<Name, Vec<Ranked>> = BTreeMap::new();
    let mut single_filled: BTreeMap<Name, FeatureName> = BTreeMap::new();
    let mut peers: Vec<PeerContribution> = Vec::new();
    for contribution in contributions {
        let name = match resolve_target(&contribution.owner, &contribution.target, &slots, &by_type)
        {
            Ok(name) => name,
            Err(error) => {
                // A peer contribution names a slot another feature owns: when
                // nobody declares it, the value is dropped and recorded as
                // unresolved, rather than the build failing over a name this
                // feature does not own. Only a contribution that names its
                // slot can be one; see `Contribution::peer`.
                match (contribution.peer, &contribution.target, &error) {
                    (true, SlotTarget::ByName(slot), ServiceError::UndeclaredSlot { .. }) => {
                        peers.push(PeerContribution {
                            owner: contribution.owner.clone(),
                            slot: slot.clone(),
                            ty: contribution.ty,
                            resolved: false,
                        });
                    }
                    _ => errors.push(error),
                }
                continue;
            }
        };
        if contribution.peer {
            peers.push(PeerContribution {
                owner: contribution.owner.clone(),
                slot: name.clone(),
                ty: contribution.ty,
                resolved: true,
            });
        }
        let decl = &slots[&name];
        if decl.ty != contribution.ty {
            errors.push(ServiceError::TypeMismatch {
                feature: contribution.owner,
                slot: name,
                declared: decl.ty,
                contributed: contribution.ty,
            });
            continue;
        }
        if decl.arity == Arity::Single {
            if let Some(first) = single_filled.get(&name) {
                errors.push(ServiceError::SingleSlotRefilled {
                    slot: name,
                    first: first.clone(),
                    second: contribution.owner,
                });
                continue;
            }
            single_filled.insert(name.clone(), contribution.owner.clone());
        }
        let position = features.iter().position(|feature| *feature == contribution.owner);
        let entry = ranking.entry(name.clone()).or_default();
        let seq = entry.len();
        entry.push((!contribution.prepend, Reverse(position), seq, Arc::clone(&contribution.value)));
        let bucket = if contribution.prepend { &mut prepends } else { &mut appends };
        bucket.entry(name).or_default().push(contribution.value);
    }
    let ranked: BTreeMap<Name, Vec<ErasedService>> = ranking
        .into_iter()
        .map(|(name, mut values)| {
            values.sort_by_key(|(group, position, seq, _)| (*group, *position, *seq));
            (name, values.into_iter().map(|(_, _, _, value)| value).collect())
        })
        .collect();
    let mut contents: BTreeMap<Name, Vec<ErasedService>> = BTreeMap::new();
    for (name, mut values) in prepends {
        values.extend(appends.remove(&name).unwrap_or_default());
        contents.insert(name, values);
    }
    contents.extend(appends);

    for requirement in requirements {
        match resolve_target(&requirement.owner, &requirement.target, &slots, &by_type) {
            Ok(name) => {
                if contents.get(&name).map(Vec::is_empty).unwrap_or(true) {
                    errors.push(ServiceError::RequiredSlotUnfilled {
                        slot: name,
                        required_by: requirement.owner,
                    });
                }
            }
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(Services { slots, contents, ranked, by_type, peers })
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Greeter: Send + Sync {
        fn greet(&self) -> &'static str;
    }

    struct Hello;

    impl Greeter for Hello {
        fn greet(&self) -> &'static str {
            "hello"
        }
    }

    fn slot(name: &str, arity: Arity) -> SlotDecl {
        SlotDecl {
            name: name.into(),
            ty: TypeKey::of::<dyn Greeter>(),
            arity,
            owner: "a".into(),
            docs: String::new(),
        }
    }

    #[test]
    fn a_contribution_by_type_lands_in_the_one_slot_of_that_type() {
        let services = assemble(
            vec![slot("x.greeter", Arity::Single)],
            vec![Contribution::new::<dyn Greeter>(
                "b",
                SlotTarget::ByType(TypeKey::of::<dyn Greeter>()),
                Arc::new(Hello),
                false,
            )],
            Vec::new(),
            &[],
        )
        .unwrap();
        assert_eq!(services.service::<dyn Greeter>().map(Greeter::greet), Some("hello"));
        assert_eq!(
            services.service_named::<dyn Greeter>("x.greeter").map(Greeter::greet),
            Some("hello")
        );
        assert!(services.service::<String>().is_none());
    }

    #[test]
    fn a_contribution_by_type_is_ambiguous_when_two_slots_carry_it() {
        let errors = assemble(
            vec![slot("x.a", Arity::Single), slot("x.b", Arity::Single)],
            vec![Contribution::new::<dyn Greeter>(
                "b",
                SlotTarget::ByType(TypeKey::of::<dyn Greeter>()),
                Arc::new(Hello),
                false,
            )],
            Vec::new(),
            &[],
        )
        .unwrap_err();
        assert!(
            matches!(&errors[0], ServiceError::AmbiguousSlot { slots, .. } if slots.len() == 2)
        );
    }

    #[test]
    fn a_required_unfilled_slot_is_an_error() {
        let errors = assemble(
            vec![slot("x.greeter", Arity::Single)],
            Vec::new(),
            vec![ServiceRequirement {
                owner: "a".into(),
                target: SlotTarget::ByName("x.greeter".into()),
            }],
            &[],
        )
        .unwrap_err();
        assert!(
            matches!(
                &errors[0],
                ServiceError::RequiredSlotUnfilled { slot, .. } if slot == "x.greeter"
            )
        );
    }

    #[test]
    fn a_read_for_precedence_takes_the_feature_placed_last_first_in_each_group() {
        let value = |owner: &str, text: &'static str, prepend: bool| {
            Contribution::new::<String>(
                owner,
                SlotTarget::ByName("x.list".into()),
                Arc::new(text.to_string()),
                prepend,
            )
        };
        let decl = SlotDecl {
            name: "x.list".into(),
            ty: TypeKey::of::<String>(),
            arity: Arity::OrderedMany,
            owner: "a".into(),
            docs: String::new(),
        };
        let services = assemble(
            vec![decl],
            vec![
                value("a", "a1", false),
                value("a", "a-first", true),
                value("a", "a2", false),
                value("b", "b1", false),
                value("b", "b-first", true),
                value("b", "b2", false),
                value("host", "host1", false),
            ],
            Vec::new(),
            &["a".into(), "b".into()],
        )
        .unwrap();
        let assembled: Vec<&str> = services.services_named::<String>("x.list").map(String::as_str).collect();
        assert_eq!(assembled, ["a-first", "b-first", "a1", "a2", "b1", "b2", "host1"]);
        let ranked: Vec<&str> =
            services.services_by_precedence_named::<String>("x.list").map(String::as_str).collect();
        assert_eq!(ranked, ["b-first", "a-first", "b1", "b2", "a1", "a2", "host1"]);
        assert_eq!(services.service_named::<String>("x.list").map(String::as_str), Some("b-first"));
        assert_eq!(services.service::<String>().map(String::as_str), Some("b-first"));
    }
}
