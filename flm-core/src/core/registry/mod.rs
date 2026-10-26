//! FLM's registry: the frozen record of everything a set of features declares,
//! the features and the builder through which they declare it, and the build
//! that checks the declarations and freezes them.
//!
//! # What a registry holds
//!
//! A **registry** is the frozen record of everything the language and the
//! pipeline consult for a set of features: the elements, stages, tables,
//! capabilities, lowering rules, backends, outputs, workflows, option types and
//! services the features declared, built once and shared by every document,
//! plan and run. The finished value is an [`FlmRegistry`], shared as an
//! `Arc<FlmRegistry>`.
//!
//! The registry holds **elements**. An element is the value that says what a
//! node of a document means: a declaration with a stable dotted type name,
//! identified by the element value itself (its allocation) and not by its name
//! ([`ElementSpec`](crate::model::element::ElementSpec)). An element that an
//! author can write in a document, as a macro, an environment or a special
//! character sequence, is declared together with its argument syntax and the
//! parsing modes (text mode, math mode) it is available in
//! ([`ElementDef`](crate::lang::mint::ElementDef)). The registry keeps these
//! **declarations** as they were made, with each command's names, syntax,
//! arguments and documentation ([`FlmRegistry::declarations`], which returns
//! a [`Declarations`]), so that a program can describe what an author may
//! write.
//!
//! The registry holds **argument roles**. An argument role is a registered
//! dotted name for what an argument of a command holds, beyond how the
//! argument is written: `flm.refs.target` for the label a reference points
//! to, `flm.graphics.path` for the path of an image file. A feature declares
//! a role ([`FeatureBuilder::argument_role`]), and any declaration may set it
//! on an argument ([`ArgDef::role`](crate::lang::mint::ArgDef::role)). Two
//! commands whose arguments hold the same kind of thing then say so in the
//! same words, for a program that describes the language; parsing and
//! processing do not read roles ([`ArgumentRoleDef`],
//! [`FlmRegistry::argument_roles`]).
//!
//! The registry holds the units of work that process a document. A **stage**
//! is one unit of work of the pipeline: it declares the capabilities it
//! provides and requires and the tables it reads and writes
//! ([`Stage`](crate::core::stage::Stage)). A **table** is typed data that one
//! stage writes beside the document's [frozen
//! trees](crate::guide::concepts_overview#frozen-trees-and-computed-data) (the
//! document trees after the rewrite stages, never changed again) and other
//! stages read ([`Table`](crate::core::data::table::Table)). A **capability** is a
//! registered dotted name for a state of processing that a stage brings about
//! ([`CapabilityDef`]); the name of a table is also a capability.
//!
//! The registry holds the output formats. A **backend** is the registration
//! bundle for one output format (plain text, HTML, LaTeX): the elements its
//! [emitter](crate::guide::concepts_overview#backends-lowering-and-outputs)
//! (its emit stage, which writes the output's bytes) writes natively, a
//! lowering stage, an emit stage and a final stage ([`BackendDef`]). The
//! lowering stage rewrites each frozen tree into a presentation tree, made of
//! elements the backend's emitter writes natively, by applying **lowering
//! rules**. A lowering rule is a
//! named registry entity written for one element: it declares what it
//! requires, reads and produces, and builds the element's presentation for one
//! backend ([`LoweringRule`](crate::core::lowering::LoweringRule)).
//!
//! # How features contribute to a registry
//!
//! A **feature** is a Rust value that contributes a related bundle of things to
//! a registry (elements, stages, tables, lowering rules, backends, services,
//! settings) through the feature builder ([`FlmFeature`]). The **feature
//! builder** is the [`FeatureBuilder`] value that the registry build hands to
//! the feature's [`register`](FlmFeature::register) method. Each of its methods
//! records one declaration and attributes it to the feature:
//! [`element`](FeatureBuilder::element) declares an element,
//! [`table`](FeatureBuilder::table) a table, [`stage`](FeatureBuilder::stage) a
//! stage, [`lowering_rule`](FeatureBuilder::lowering_rule) a lowering rule,
//! [`backend`](FeatureBuilder::backend) a backend, and so on. Features are
//! usually shipped and registered together as a **package**, a named bundle of
//! features together with the packages it builds on ([`PackageEntry`]).
//!
//! A registry is made in three steps:
//!
//! 1. **Declaring.** Each feature's [`register`](FlmFeature::register) method
//!    calls methods of its [`FeatureBuilder`]. The builder only records the
//!    declarations: it checks and resolves nothing, so a feature never needs to
//!    know which other features are present or which registered first.
//! 2. **Building.** [`RegistryBuilder::build`], or [`build_registry`] for a plain
//!    list of features, places the features in their final order (see
//!    [`order`]), lets them declare in that order, and runs the collected
//!    declarations through five phases
//!    (described in [`build`]): it checks the declarations against each other,
//!    builds the FLM language, that is, the commands a parse recognizes with
//!    their syntax and the parsing settings ([`FlmRegistry::lang`]), from the
//!    element declarations, and freezes the result. A phase that finds problems stops the build and returns every
//!    problem it found together, as a [`RegistryBuildError`].
//! 3. **Reading.** The accessors of the frozen [`FlmRegistry`] look an entity up
//!    by its name, or by a dense numeric identifier that the build assigned to it
//!    ([`CapId`], [`StageIdx`], [`RuleIdx`]).
//!
//! # Module map
//!
//! - [`feature`] — the [`FlmFeature`] trait and [`FeatureInfo`], what the registry
//!   keeps about a feature; the three forms of a package: [`PackageEntry`] (a
//!   package ready to register), [`PackageFactory`] (a package known by its name
//!   and settings before it is built) and [`PackageCatalog`] (finds a package's
//!   factory by name, through [`PackageResolver`] implementations the host
//!   supplies).
//! - [`entities`] — the declaration values a feature passes to the builder:
//!   capabilities, collections, argument roles, ordering edges, backends,
//!   outputs, workflows, style and script definitions, native-set
//!   implications and peer declarations ([`PeerName`]); and the two build
//!   hooks, [`Finalizer`] and [`LateInit`].
//! - [`builder`] — [`FeatureBuilder`]; [`Roster`](builder::Roster), the collected
//!   declarations of one build; [`BuildError`], the error every check reports;
//!   and [`Site`], the place in a declaration that an error refers to.
//! - [`order`] — [`FeatureChain`], one chain of the feature order (such as
//!   `math-mathjax < phfqit`), [`ChainError`], and how the build places the
//!   features in their final order.
//! - [`build`] — [`RegistryBuilder`], the [`RegistryInputs`] a build takes
//!   besides the features, the five phases of the build, and the Note
//!   [`UnresolvedPeer`].
//! - [`frozen`] — the finished [`FlmRegistry`], the entry types its accessors
//!   return, and the dense identifiers [`CapId`], [`StageIdx`] and [`RuleIdx`].
//! - [`declarations`] — [`Declarations`], the element declarations, scoped
//!   packages, fallbacks and overrides a registry keeps as they were declared,
//!   and the references into them its accessors return ([`ArgumentRef`],
//!   [`ScopedPackageUse`]).
//! - The private `validate` module holds the checks that run over the collected
//!   declarations. The private `resolver` module implements
//!   [`ElementResolver`](crate::lang::serialize::ElementResolver) for
//!   [`FlmRegistry`], so that a stored tree can be read back against the
//!   registry it was produced under.
//!
//! This module re-exports every public item of those modules, so that
//! `use flm_core::core::registry::FlmFeature` works as well as the full path.
//! Two types of those modules are called `OutputEntry`:
//! [`entities::OutputEntry`] describes an output's entry artifact (its media
//! type and file extension), and [`frozen::OutputEntry`] is an output as the
//! finished registry holds it. This module re-exports the second under the name
//! [`RegisteredOutput`], so that both can be used from here.
//!
//! # Rules that hold across the module
//!
//! - **A name is unique in its namespace.** Capabilities, tables and
//!   collections share one namespace: a table's name is also a capability's
//!   name, and no capability or collection may be declared under it. Features,
//!   stages, lowering rules, backends, outputs, workflows and argument roles
//!   each have a namespace of their own.
//! - **The registration order is the final order of the features.** The
//!   features register in the order they were added to the builder, changed
//!   by the chains of the feature order that the packages and the embedding
//!   program state ([`order`]). Where two features define one command name,
//!   the definition of the feature placed last applies, and the chains decide
//!   whether that needs an override (see [`crate::lang::mint::packages`]).
//! - **Every check runs before anything is numbered.** Duplicate declarations,
//!   references to entities no feature declares, cycles of refinements or of
//!   workflows, and the other checks run over the whole set of collected
//!   declarations. Registration order therefore never limits what a feature may
//!   declare or refer to.
//! - **Numbering follows registration order.** Capabilities, tables, stages
//!   and lowering rules are numbered in the order they were declared (the
//!   capabilities declared on their own before those of tables), and elements
//!   in the order of each kind of declaration (see [`frozen`]). The same order breaks ties when a plan has nothing else to decide between
//!   two stages, so two builds from the same features, added in the same order
//!   with the same chains, produce the same numbering and the same plans.
//! - **A dense identifier belongs to one registry.** A [`CapId`], [`StageIdx`],
//!   [`RuleIdx`], [`ElemIdx`](crate::model::element::ElemIdx) or
//!   [`TableId`](crate::core::data::table::TableId) means something only
//!   together with the registry that assigned it. Used with another registry,
//!   it reads the wrong entry, or the accessor panics (see
//!   [the list of panics](crate::guide::panics)).
//!
//! # Serialized forms
//!
//! The plain-data declaration types that a registry and a plan hold are
//! written to data with `serde::Serialize`, and each also has a JSON Schema
//! (`schemars::JsonSchema`), so that a program can describe an environment
//! as JSON without copying these types. The serialized forms are public
//! contract: `flm-info`'s description of an environment embeds them, and
//! versions them by its `meta.format`. Each type is written as a JSON object
//! whose keys are its field names, with these exceptions:
//!
//! - A field that has no data form is left out: the value
//!   type of a table ([`TableDef::ty`](crate::core::data::table::TableDef::ty),
//!   [`ReadEntry::ty`](crate::core::data::store::ReadEntry::ty),
//!   [`WriteEntry::ty`](crate::core::data::store::WriteEntry::ty)), the
//!   identifier of a Rust type ([`TableRef::type_id`](crate::core::data::table::TableRef::type_id);
//!   its `type_name` is written), and a workflow's option values
//!   ([`WorkflowDef::render_options`]).
//! - An option type is written as its name: the key of an
//!   [`OptionDecl`](crate::model::options::OptionDecl), and every
//!   [`OptionRef`](crate::model::options::OptionRef).
//! - A unit enumeration is written as one string per variant, in lowercase
//!   with hyphens: [`Effect`](crate::core::stage::Effect) (`compute`,
//!   `check`, `io`), [`Scope`](crate::core::stage::Scope) (`per-tree`,
//!   `document`), [`Bind`](crate::model::options::Bind) (`registry`,
//!   `document`, `render`, `either`),
//!   [`CallableKind`](crate::lang::mint::CallableKind) (`macro`,
//!   `environment`, `specials`), [`ArgLayout`](crate::lang::mint::ArgLayout)
//!   (`inline`, `blocks`, `inline-or-blocks`) and
//!   [`BodyLayout`](crate::lang::mint::BodyLayout) (`inline`, `blocks`,
//!   `from-role`). A parsing mode is `text` or `math`.
//! - A standard argument kind ([`ArgStdKind`](crate::lang::mint::ArgStdKind))
//!   is an object whose key `syntax` holds the kind's name, in lowercase with
//!   hyphens, beside the kind's parameters, as the kind's documentation lists
//!   them ([the kind as data](crate::lang::mint::ArgStdKind#the-kind-as-data)).
//!   An argument written as a map
//!   ([`ArgDataFields`](crate::lang::mint::ArgDataFields)) holds the keys of
//!   that object among its own, and leaves out `code` when it gives a
//!   structured syntax instead.
//! - A [`PeerName`] is `{"kind", "name"}`, the kind the word that
//!   [`PeerName::kind`] gives (`scoped package`), as in the data of the
//!   condition [`UnresolvedPeer`]; a
//!   [`PlanWarning`](crate::core::plan::PlanWarning) is an object whose
//!   `kind` gives the variant (`preferred-rule-set-aside`) beside the
//!   variant's fields.
//! - A dense identifier ([`CapId`], [`StageIdx`],
//!   [`TableId`](crate::core::data::table::TableId)) is its number, which
//!   means something only together with the registry that assigned it; a
//!   [`ReadSet`](crate::core::data::store::ReadSet) and a
//!   [`WriteSet`](crate::core::data::store::WriteSet) are objects keyed by
//!   table name.
//!
//! The types written this way: [`FeatureInfo`], [`CapabilityDef`],
//! [`CollectionDef`], [`ArgumentRoleDef`], [`OutputDef`] and
//! [`entities::OutputEntry`], [`WorkflowDef`] and [`LoweringSelection`],
//! [`PeerName`], [`CapId`] and [`StageIdx`];
//! [`StageDecl`](crate::core::stage::StageDecl),
//! [`RewriteDecl`](crate::core::stage::RewriteDecl),
//! [`Manifest`](crate::core::stage::Manifest) and
//! [`Needs`](crate::core::stage::Needs); the table types
//! [`TableDef`](crate::core::data::table::TableDef),
//! [`TableRef`](crate::core::data::table::TableRef) and
//! [`TableId`](crate::core::data::table::TableId), and the plan's
//! [`ReadSet`](crate::core::data::store::ReadSet),
//! [`ReadEntry`](crate::core::data::store::ReadEntry),
//! [`WriteSet`](crate::core::data::store::WriteSet) and
//! [`WriteEntry`](crate::core::data::store::WriteEntry);
//! [`OptionDecl`](crate::model::options::OptionDecl),
//! [`Bind`](crate::model::options::Bind) and
//! [`OptionRef`](crate::model::options::OptionRef); the plan's
//! [`StagePlan`](crate::core::plan::StagePlan),
//! [`Excluded`](crate::core::plan::Excluded) and
//! [`PlanWarning`](crate::core::plan::PlanWarning); and the language's
//! [`ArgDataFields`](crate::lang::mint::ArgDataFields),
//! [`ArgStdKind`](crate::lang::mint::ArgStdKind),
//! [`ArgLayout`](crate::lang::mint::ArgLayout),
//! [`BodyLayout`](crate::lang::mint::BodyLayout),
//! [`CallableKind`](crate::lang::mint::CallableKind) and
//! [`Symbol`](crate::lang::mint::Symbol).
//!
//! See also [features and
//! packages](crate::guide::concepts_overview#features-and-packages),
//! [stages, capabilities, and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans) and
//! [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs).
//!
//! # Examples
//!
//! Building a registry from one feature and reading back what it declared:
//!
//! ```
//! use std::sync::Arc;
//! use flm_core::core::registry::{
//!     BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature, build_registry,
//! };
//!
//! struct Spelling;
//!
//! impl FlmFeature for Spelling {
//!     fn info(&self) -> FeatureInfo {
//!         FeatureInfo::new("spelling", "Spell checking")
//!     }
//!
//!     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
//!         let done = CapabilityDef::new("spelling.checked").docs("Spelling was checked.");
//!         b.capability(done)
//!     }
//! }
//!
//! let registry = build_registry([Arc::new(Spelling) as Arc<dyn FlmFeature>])?;
//! assert!(registry.has_feature("spelling"));
//! let cap = registry.capability_named("spelling.checked").expect("declared above");
//! assert_eq!(cap.owner, "spelling");
//! # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
//! ```

pub mod build;
pub mod builder;
pub mod declarations;
pub mod entities;
pub mod feature;
pub mod frozen;
pub mod order;
mod resolver;
mod validate;

pub use build::{
    BuildPhase, RegistryBuildError, RegistryBuilder, RegistryInputs, UnresolvedPeer,
    build_registry,
};
pub use builder::{ArgumentPlace, BuildError, FeatureBuilder, RoundTwoAction, Site, StageKind, HOST_FEATURE};
pub use declarations::{ArgumentRef, Declarations, ScopedPackageUse};
pub use entities::{
    ArgumentRoleDef, BackendDef, CapabilityDef, CollectionDef, DefContent, DefinitionText, Finalizer, LateInit, LateInitError,
    LoweringSelection, NativeImplication, NativeImplicationKind, OrderingDef, OutputDef,
    OutputEntry, PeerName, ScriptDef, StyleDef, WorkflowDef,
};
pub use feature::{
    FeatureInfo, FlmFeature, PackageCatalog, PackageEntry, PackageError, PackageFactory,
    PackageFeature, PackageResolver, ResolveError, UseError, UseErrorKind,
};
pub use order::{ChainError, FeatureChain};
pub use frozen::{
    ArgumentRoleEntry, BackendEntry, CapId, CapabilityEntry, CollectionEntry, ElemBitset, ElementEntry, ElementKind,
    FlmRegistry, OutputEntry as RegisteredOutput, PeerArgumentRoleEntry, PeerBackendEntry, PeerElementEntry,
    PeerElementValueEntry, PeerScopedPackageEntry, PeerTableEntry, PresentationEntry, RuleEntry, RuleIdx, StageEntry, StageIdx, TableEntry,
    WorkflowEntry,
};
pub use crate::model::element::AmbiguousElement;
