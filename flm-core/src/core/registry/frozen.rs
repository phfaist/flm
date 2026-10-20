//! The finished registry, [`FlmRegistry`]; the entry types its accessors
//! return; and the dense numeric identifiers that address its entries.
//!
//! Once the registry build has checked every declaration, it **numbers** the
//! declarations: each element value, capability, table, stage and lowering
//! rule gets a number, and the build creates lookup structures over those
//! numbers. The result is an [`FlmRegistry`], which never changes again and
//! is shared as an `Arc<FlmRegistry>` by every document, plan and run.
//!
//! # Dense identifiers
//!
//! [`CapId`], [`StageIdx`], [`RuleIdx`], [`ElemIdx`] and [`TableId`] are
//! positions in the lists
//! of one registry. With them, the frequent questions ("does this element
//! carry that aspect", "which stage provides this capability") are answered
//! by a numeric lookup instead of a comparison of names. A number means
//! nothing without the registry that assigned it: used with a different
//! registry, it reads the wrong entry, or the accessor panics. Look a name up
//! once, keep the identifier, and use it only with that registry.
//!
//! [`ElemBitset`] applies the same idea to a set of elements: one bit per
//! element index. [`FlmRegistry::aspect_bitset`] returns the set of every
//! element that carries a given aspect in this form.
//!
//! # Numbering order
//!
//! Numbers follow declaration order throughout, so two builds from the same
//! features in the same order produce the same numbers:
//!
//! - elements: the language's structural elements first, in their fixed
//!   order; then the elements of the element declarations, in declaration
//!   order, each followed by the elements of its arguments' fields; then the
//!   elements of the fallbacks, then those of the scoped packages, then the
//!   presentation elements, each group in declaration order (a value that
//!   several declarations share is numbered once);
//! - capabilities: the capabilities declared on their own, then one for each
//!   table;
//! - tables, stages and lowering rules: each in declaration order.
//!
//! The stage numbering also breaks ties when a plan has nothing else to
//! decide between two stages.
//!
//! # Element identity
//!
//! The registry keeps an element value's number in a separate table, keyed by
//! the address of the shared value, and not on the value itself. A value can
//! therefore outlive the registry that numbered it (as a node in a tree does)
//! without carrying a number that is wrong for the next registry.
//!
//! See also [`super::build`] for how a registry is built and
//! [`super::builder`] for the declarations it is built from.

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::model::element::{AmbiguousElement, ElemIdx, ElementSpec};
use crate::core::lowering::LoweringRule;
use crate::model::names::{
    BackendName, CapName, ElementName, FeatureName, Name, OutputName, RuleName, StageName,
    WorkflowName,
};
use crate::model::options::{OptionDecl, OptionKey, TypeKey};
use crate::core::registry::builder::{BuildError, Roster, Site, StageKind};
use crate::core::registry::declarations::Declarations;
use crate::core::registry::entities::{
    NativeImplication, OrderingDef, OutputDef, PeerName, ScriptDef, StyleDef, WorkflowDef,
};
use crate::core::registry::feature::FeatureInfo;
use crate::core::services::{PeerContribution, Services};
use crate::core::stage::{RewriteDecl, StageDecl};
use crate::core::data::table::{TableDef, TableId, TableType};
use crate::lang::{BuiltLang, FlmDiagnostics};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::lang::structural::{self, StructuralElements};

/// The number of a capability in one registry, in declaration order.
///
/// Get one with [`FlmRegistry::cap_id`], and read the capability's entry with
/// [`FlmRegistry::capability`]. The number is meaningful only together with
/// the registry that assigned it.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct CapId(pub u32);

/// The number of a stage in one registry, in registration order, counting
/// stages of both forms (rewrite stages and ordinary stages).
///
/// The same order breaks ties when a plan has nothing else to decide between
/// two stages. Get one with [`FlmRegistry::stage_idx`], and read the stage's
/// entry with [`FlmRegistry::stage`]. The number is meaningful only together
/// with the registry that assigned it.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct StageIdx(pub u32);

/// The number of a lowering rule in one registry, in registration order.
///
/// Get one with [`FlmRegistry::rule_idx`] or [`FlmRegistry::rules_for`], and
/// read the rule's entry with [`FlmRegistry::rule`]. The number is meaningful
/// only together with the registry that assigned it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleIdx(pub u32);

/// A set of element indices, stored as one bit per index.
///
/// [`FlmRegistry::aspect_bitset`] returns the set of every element that
/// carries a given aspect in this form, without comparing names: a stage that
/// must visit every referenceable element asks once, then tests each node's
/// element by its index.
///
/// # Examples
///
/// ```
/// use flm_core::model::element::ElemIdx;
/// use flm_core::core::registry::ElemBitset;
///
/// let mut set = ElemBitset::new();
/// set.insert(ElemIdx(3));
/// set.insert(ElemIdx(70));
/// assert!(set.contains(ElemIdx(3)));
/// assert!(!set.contains(ElemIdx(4)));
/// assert_eq!(set.iter().collect::<Vec<_>>(), [ElemIdx(3), ElemIdx(70)]);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElemBitset {
    bits: Vec<u64>,
}

impl ElemBitset {
    /// A set holding no index.
    pub fn new() -> ElemBitset {
        ElemBitset::default()
    }

    /// Adds `idx` to the set; adding an index the set already holds changes
    /// nothing.
    ///
    /// The storage grows to cover the index, so adding a very large index to
    /// an otherwise small set allocates memory in proportion to that index.
    pub fn insert(&mut self, idx: ElemIdx) {
        let (word, bit) = (idx.0 as usize / 64, idx.0 % 64);
        if self.bits.len() <= word {
            self.bits.resize(word + 1, 0);
        }
        self.bits[word] |= 1 << bit;
    }

    /// Whether the set holds `idx`.
    pub fn contains(&self, idx: ElemIdx) -> bool {
        let (word, bit) = (idx.0 as usize / 64, idx.0 % 64);
        self.bits.get(word).map(|w| w & (1 << bit) != 0).unwrap_or(false)
    }

    /// The indices the set holds, in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = ElemIdx> + '_ {
        self.bits.iter().enumerate().flat_map(|(word, bits)| {
            (0..64u32)
                .filter(move |bit| bits & (1 << bit) != 0)
                .map(move |bit| ElemIdx(word as u32 * 64 + bit))
        })
    }

    /// The number of indices the set holds. Each call counts them, in a time
    /// proportional to the largest index ever added.
    pub fn len(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether the set holds no index.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The origin of a registered element value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    /// One of the language's structural elements, the elements of the node
    /// forms that no feature declares (text, groups, paragraphs and the
    /// others; see [`StructuralElements`]), which exist in every registry.
    Structural,
    /// An element that can appear in a parsed document: an element declared
    /// with a syntax, an element a declaration creates without syntax, the
    /// element of a field, or the element of a fallback.
    Source,
    /// A presentation element, registered with
    /// [`FeatureBuilder::presentation_element`](super::FeatureBuilder::presentation_element).
    /// It appears only in the trees a backend writes, never in a parsed
    /// document.
    Presentation,
}

/// One element value as the registry holds it.
#[derive(Debug, Clone)]
pub struct ElementEntry {
    /// The element value itself, shared with every node carrying it.
    pub element: Arc<ElementSpec>,
    /// The feature that declared it; of several features that declare one
    /// value, the one placed last in the feature order. For a structural
    /// element that no feature declares, the owner is the language's own
    /// name, `flm`.
    pub owner: FeatureName,
    /// This value's number in this registry.
    pub idx: ElemIdx,
    /// The origin of the value.
    pub kind: ElementKind,
}

/// One capability as the registry holds it, whether it was declared on its
/// own or together with a table.
///
/// Every table's name is also a capability, the capability of the table having
/// been written; the registry holds both kinds in one list, and
/// [`table`](CapabilityEntry::table) tells them apart.
#[derive(Debug, Clone)]
pub struct CapabilityEntry {
    /// The capability's name.
    pub name: CapName,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// The feature that declared it.
    pub owner: FeatureName,
    /// This capability's number in this registry.
    pub id: CapId,
    /// The capability this one refines (is a stronger form of), if any.
    pub refines: Option<CapId>,
    /// Whether this is the default refinement of its parent: the refinement
    /// that a demand for the parent alone selects when nothing else decides.
    pub default: bool,
    /// The table whose capability this is, or `None` for a capability declared
    /// on its own.
    pub table: Option<TableId>,
    /// Whether that table is external: no stage writes it, and only the value
    /// the caller installs when a run is constructed satisfies this
    /// capability.
    pub external: bool,
    /// What a plan error says when a plan needs this capability and no
    /// registered stage provides it, from the declaration
    /// ([`CapabilityDef::unprovided`](super::CapabilityDef::unprovided)); `None`
    /// for a table's capability.
    pub unprovided: Option<String>,
    ancestors: Vec<CapId>,
    children: Vec<CapId>,
}

impl CapabilityEntry {
    /// This capability, followed by the chain of capabilities it refines, the
    /// nearest first.
    ///
    /// Providing this capability provides every capability in the chain;
    /// a stage's [`provides`](StageEntry::provides) therefore includes the whole
    /// chain of each capability it provides.
    pub fn ancestors(&self) -> &[CapId] {
        &self.ancestors
    }

    /// The capabilities that refine this one directly, in identifier order;
    /// their own refinements are not included.
    pub fn refinements(&self) -> &[CapId] {
        &self.children
    }
}

/// One collection as the registry holds it: a name for a set of capabilities,
/// with the members that features added to it.
#[derive(Debug, Clone)]
pub struct CollectionEntry {
    /// The collection's name.
    pub name: CapName,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// The feature that declared the collection. Any feature may add members.
    pub owner: FeatureName,
    /// The capabilities added as members, in identifier order. The build
    /// dropped the members added to a collection that no feature declares.
    pub members: BTreeSet<CapId>,
}

/// One table as the registry holds it.
#[derive(Debug, Clone)]
pub struct TableEntry {
    /// The declaration: the table's name, its value type, its documentation
    /// and the feature that declared it.
    pub def: TableDef,
    /// This table's number in this registry.
    pub id: TableId,
    /// The capability that comes with the table: the capability of the table
    /// having been written.
    pub cap: CapId,
    /// Whether the table is external: no stage writes it, and its values are
    /// installed when a run is constructed.
    pub external: bool,
}

/// One stage as the registry holds it, of either form, with its declaration
/// resolved against the rest of the registry.
pub struct StageEntry {
    /// The stage's name, unique across the registry.
    pub name: StageName,
    /// The stage itself, in its form (rewrite stage or ordinary stage).
    pub kind: StageKind,
    /// The declaration read from the stage when it was registered. For a
    /// rewrite stage, the builder made it from the rewrite's own declaration,
    /// so that both forms are scheduled the same way.
    pub decl: StageDecl,
    /// A rewrite stage's own declaration; `None` for an ordinary stage.
    pub rewrite: Option<RewriteDecl>,
    /// The feature that registered it.
    pub owner: FeatureName,
    /// This stage's number in this registry.
    pub idx: StageIdx,
    /// Every capability that running this stage brings about: those it
    /// declares, the capabilities of the tables it writes, and every
    /// capability those refine. Because the refined capabilities are
    /// included, a plan matches a demand by set membership alone.
    pub provides: BTreeSet<CapId>,
    /// The tables the stage writes, in the order declared.
    pub writes: Vec<TableId>,
}

impl StageEntry {
    /// Whether this is a rewrite stage, which changes the trees before they are
    /// frozen, and not an ordinary stage.
    pub fn is_rewrite(&self) -> bool {
        matches!(self.kind, StageKind::Rewrite(_))
    }
}

impl fmt::Debug for StageEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StageEntry")
            .field("name", &self.name)
            .field("rewrite", &self.is_rewrite())
            .field("provides", &self.provides)
            .finish()
    }
}

/// One lowering rule as the registry holds it.
#[derive(Debug, Clone)]
pub struct RuleEntry {
    /// The rule: the element it lowers, what it produces, and what it needs.
    pub rule: LoweringRule,
    /// The feature that registered it.
    pub owner: FeatureName,
    /// This rule's number in this registry.
    pub idx: RuleIdx,
    /// Why the rule is **set aside**, or empty for a rule that takes part. A
    /// lowering rule is set aside when a peer declaration it depends on
    /// resolves to nothing; this list holds those [peer
    /// declarations](PeerName) of the rule's own feature (a backend in its
    /// default mark, an element in its `produces`, a table in its `reads`)
    /// that no feature of this registry declares.
    ///
    /// A rule that is set aside is never a candidate when a plan chooses
    /// rules, and the check that a backend can write every element ignores
    /// it, exactly as if it had not been registered. It stays listed so that
    /// a reader can find it and see why. Each unresolved name is also reported
    /// once as a Note among the [build
    /// diagnostics](FlmRegistry::build_diagnostics). A rule that refers to an
    /// unresolved peer table only in `reads_opt` is not set aside: it runs and
    /// finds the table absent.
    pub set_aside: Vec<PeerName>,
}

impl RuleEntry {
    /// Whether the rule is set aside, that is, whether
    /// [`set_aside`](RuleEntry::set_aside) is not empty.
    pub fn is_set_aside(&self) -> bool {
        !self.set_aside.is_empty()
    }
}

/// One [peer declaration](PeerName) of a backend as the registry holds it.
#[derive(Debug, Clone)]
pub struct PeerBackendEntry {
    /// The backend's name.
    pub name: BackendName,
    /// The feature that made the declaration.
    pub owner: FeatureName,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// Whether some feature of this registry registers the backend. When
    /// `false`, the owner's rules that list the backend in their default mark
    /// are set aside.
    pub resolved: bool,
}

/// One [peer declaration](PeerName) of a presentation element as the registry
/// holds it.
#[derive(Debug, Clone)]
pub struct PeerElementEntry {
    /// The element's name.
    pub name: ElementName,
    /// The payload type given as the declaration's signature. When the
    /// declaration is resolved, the registered presentation element has this
    /// payload type: the build refuses any other.
    pub payload: TypeKey,
    /// The feature that made the declaration.
    pub owner: FeatureName,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// Whether some feature of this registry registers a presentation element
    /// of this name. When `false`, the owner's rules producing the element are
    /// set aside.
    pub resolved: bool,
}

/// One [peer declaration](PeerName) of an element value as the registry holds
/// it ([`FeatureBuilder::peer_element_value`](super::FeatureBuilder::peer_element_value)).
#[derive(Debug, Clone)]
pub struct PeerElementValueEntry {
    /// The element type's name.
    pub element: ElementName,
    /// The callable names (commands) the declaration defines.
    pub names: Vec<Name>,
    /// The feature that made the declaration.
    pub owner: FeatureName,
    /// Whether some feature of this registry declares the element type. When
    /// `false`, the build dropped the declaration and its commands are not
    /// defined; when `true`, the declared value is one of the registry's
    /// elements like any other.
    pub resolved: bool,
}

/// One [peer declaration](PeerName) of a scoped package as the registry holds
/// it.
#[derive(Debug, Clone)]
pub struct PeerScopedPackageEntry {
    /// The package's name.
    pub name: Name,
    /// The feature that made the declaration.
    pub owner: FeatureName,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// Whether some feature of this registry declares a scoped package of this
    /// name. When `false`, the owner's environment bodies and arguments that
    /// refer to the package get no definitions from it.
    pub resolved: bool,
}

/// One [peer declaration](PeerName) of a table as the registry holds it.
#[derive(Debug, Clone)]
pub struct PeerTableEntry {
    /// The table's name.
    pub name: CapName,
    /// The value type given as the declaration's signature. When the
    /// declaration is resolved, the declared table holds this type: the build
    /// refuses any other.
    pub value: TableType,
    /// The feature that made the declaration.
    pub owner: FeatureName,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// Whether some feature of this registry declares a table of this name.
    /// When `false`, the owner's rules that read the table as a hard
    /// dependency are set aside, its rules that read it optionally run and
    /// find the table absent, and its stages that write it are not
    /// registered.
    pub resolved: bool,
}

/// One argument role as the registry holds it: a registered name for what an
/// argument holds, declared with
/// [`FeatureBuilder::argument_role`](super::FeatureBuilder::argument_role)
/// and set on arguments with [`ArgDef::role`](crate::lang::mint::ArgDef::role).
///
/// [`FlmRegistry::argument_roles`] lists every role;
/// [`Declarations::role_uses`](super::Declarations::role_uses) lists the
/// arguments that have one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentRoleEntry {
    /// The role's dotted name, unique among the registry's argument roles.
    pub name: Name,
    /// Documentation prose from the declaration: what an argument of this
    /// role holds.
    pub docs: String,
    /// The feature that declared the role.
    pub owner: FeatureName,
}

/// One [peer declaration](PeerName) of an argument role as the registry holds
/// it ([`FeatureBuilder::peer_argument_role`](super::FeatureBuilder::peer_argument_role)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerArgumentRoleEntry {
    /// The role's name.
    pub name: Name,
    /// The feature that made the declaration.
    pub owner: FeatureName,
    /// Whether some feature of this registry declares the role. When
    /// `false`, the owner's arguments that have the role keep it, and the
    /// registry holds no description of it.
    pub resolved: bool,
}

/// One presentation element as the registry holds it.
#[derive(Debug, Clone)]
pub struct PresentationEntry {
    /// The element value created for it, shared with every node that carries
    /// it.
    pub element: Arc<ElementSpec>,
    /// The type of the payload its nodes carry.
    pub payload: TypeKey,
    /// Documentation prose from the declaration.
    pub docs: String,
    /// The feature that registered it.
    pub owner: FeatureName,
}

/// One backend as the registry holds it, with its three stages given by their
/// numbers.
#[derive(Debug, Clone)]
pub struct BackendEntry {
    /// The backend's name, which is also the last segment of the names of its
    /// tables.
    pub name: BackendName,
    /// The native set: the elements this backend's emit stage writes directly.
    /// Lowering rules must turn every other element into these.
    pub native: BTreeSet<ElementName>,
    /// The lowering stage, which builds the presentation trees from the frozen
    /// trees.
    pub lowering: StageIdx,
    /// The emit stage, which turns the presentation trees into bytes.
    pub emit: StageIdx,
    /// The final stage, which assembles the artifact set the caller
    /// receives.
    pub final_stage: StageIdx,
    /// The feature that registered it.
    pub owner: FeatureName,
    /// Documentation prose from the declaration.
    pub docs: String,
}

/// One output as the registry holds it, with its table and its capability
/// given by their numbers. The module root re-exports this type as
/// [`RegisteredOutput`](super::RegisteredOutput); the unrelated
/// [`entities::OutputEntry`](super::entities::OutputEntry) describes an
/// output's entry artifact.
#[derive(Debug, Clone)]
pub struct OutputEntry {
    /// The declaration: the output's name, its entry artifact's media type and
    /// file extension, and its documentation.
    pub def: OutputDef,
    /// The feature that registered it.
    pub owner: FeatureName,
    /// The table that holds this output's artifacts, as an artifact set.
    pub table: TableId,
    /// The capability a plan must reach for this output to exist.
    pub capability: CapId,
}

/// One workflow as the registry holds it, as declared, without the fields it
/// inherits; [`FlmRegistry::combined_workflow`] returns the workflow with
/// them.
#[derive(Debug, Clone)]
pub struct WorkflowEntry {
    /// The declaration: the outputs, demands, preferences and option values it
    /// holds, and the workflow it extends. After the build, the declaration
    /// includes the fields registered for this workflow by name.
    pub def: WorkflowDef,
    /// The feature that registered it.
    pub owner: FeatureName,
}

// The numbering order and its determinism: see the design rationale at
// [§flm-dd-dr:feat-build-order]. The element index is a registry-owned side
// table rather than a field on the element value:
// [§flm-dd-dr:dm-element-index-side-table].
/// A registry: the frozen record of everything the language and the pipeline
/// consult for a set of features, built once and shared by every document,
/// plan and run.
///
/// [`RegistryBuilder::build`](super::RegistryBuilder::build) and
/// [`build_registry`](super::build_registry) produce it. It never changes
/// afterwards and is shared across threads as an `Arc<FlmRegistry>`.
///
/// # Reading a registry
///
/// Most entities can be looked up in two ways. An accessor that takes a name
/// returns an `Option`, because any name may be asked for. An accessor that
/// takes a capability, table, stage or rule number this registry assigned
/// returns the entry directly, because such a number is valid by
/// construction; it panics when the number is beyond this registry's lists,
/// which can happen only when another registry assigned it. The element
/// accessors are the exception: [`element`](FlmRegistry::element) returns
/// `None` and [`has_aspect`](FlmRegistry::has_aspect) returns `false` for an
/// index beyond this registry's elements. A number that another registry
/// assigned and that falls within this registry's lists reads the wrong
/// entry. The accessors, by group:
///
/// - **Features and the language.** [`features`](FlmRegistry::features),
///   [`feature`](FlmRegistry::feature), [`has_feature`](FlmRegistry::has_feature),
///   [`lang`](FlmRegistry::lang), [`structural`](FlmRegistry::structural),
///   [`build_diagnostics`](FlmRegistry::build_diagnostics).
/// - **The declarations of what an author may write.**
///   [`declarations`](FlmRegistry::declarations) (the element declarations,
///   scoped packages, fallbacks and overrides, as declared),
///   [`argument_roles`](FlmRegistry::argument_roles),
///   [`argument_role_named`](FlmRegistry::argument_role_named).
/// - **Elements and aspects.** [`elements`](FlmRegistry::elements),
///   [`element`](FlmRegistry::element), [`elem_idx`](FlmRegistry::elem_idx),
///   [`elements_named`](FlmRegistry::elements_named),
///   [`element_named`](FlmRegistry::element_named),
///   [`element_value`](FlmRegistry::element_value),
///   [`aspect_bitset`](FlmRegistry::aspect_bitset),
///   [`has_aspect`](FlmRegistry::has_aspect),
///   [`aspect_names`](FlmRegistry::aspect_names),
///   [`occurrable_elements`](FlmRegistry::occurrable_elements).
/// - **Capabilities, collections and tables.**
///   [`capabilities`](FlmRegistry::capabilities), [`cap_id`](FlmRegistry::cap_id),
///   [`capability`](FlmRegistry::capability),
///   [`capability_named`](FlmRegistry::capability_named),
///   [`default_refinement`](FlmRegistry::default_refinement),
///   [`collections`](FlmRegistry::collections),
///   [`collection`](FlmRegistry::collection), [`tables`](FlmRegistry::tables),
///   [`table_id`](FlmRegistry::table_id), [`table`](FlmRegistry::table),
///   [`table_named`](FlmRegistry::table_named),
///   [`external_tables`](FlmRegistry::external_tables),
///   [`table_has_writer`](FlmRegistry::table_has_writer).
/// - **Stages and orderings.** [`stages`](FlmRegistry::stages),
///   [`stage_idx`](FlmRegistry::stage_idx), [`stage`](FlmRegistry::stage),
///   [`stage_named`](FlmRegistry::stage_named),
///   [`orderings`](FlmRegistry::orderings).
/// - **Lowering and presentation.** [`rules`](FlmRegistry::rules),
///   [`rule_idx`](FlmRegistry::rule_idx), [`rule`](FlmRegistry::rule),
///   [`rules_for`](FlmRegistry::rules_for),
///   [`set_aside_rules`](FlmRegistry::set_aside_rules),
///   [`presentation_elements`](FlmRegistry::presentation_elements),
///   [`presentation_element`](FlmRegistry::presentation_element),
///   [`peer_backends`](FlmRegistry::peer_backends),
///   [`peer_elements`](FlmRegistry::peer_elements),
///   [`peer_tables`](FlmRegistry::peer_tables),
///   [`peer_slots`](FlmRegistry::peer_slots),
///   [`peer_scoped_packages`](FlmRegistry::peer_scoped_packages),
///   [`peer_element_values`](FlmRegistry::peer_element_values),
///   [`peer_argument_roles`](FlmRegistry::peer_argument_roles).
/// - **Backends, outputs and workflows.** [`backends`](FlmRegistry::backends),
///   [`backend`](FlmRegistry::backend), [`outputs`](FlmRegistry::outputs),
///   [`output`](FlmRegistry::output), [`workflows`](FlmRegistry::workflows),
///   [`workflow`](FlmRegistry::workflow).
/// - **Workflows with what they inherit.**
///   [`workflow_chain`](FlmRegistry::workflow_chain),
///   [`combined_workflow`](FlmRegistry::combined_workflow).
/// - **Options, services, style and script definitions, and the digest.**
///   [`options`](FlmRegistry::options),
///   [`option_decl`](FlmRegistry::option_decl),
///   [`services`](FlmRegistry::services), [`styles`](FlmRegistry::styles),
///   [`scripts`](FlmRegistry::scripts),
///   [`native_implications`](FlmRegistry::native_implications),
///   [`digest`](FlmRegistry::digest).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{
///     BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature, build_registry,
/// };
///
/// struct Counting;
/// impl FlmFeature for Counting {
///     fn info(&self) -> FeatureInfo { FeatureInfo::new("counting", "Counting") }
///     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
///         b.capability(CapabilityDef::new("counting.done"))
///     }
/// }
///
/// let registry = build_registry([Arc::new(Counting) as Arc<dyn FlmFeature>])?;
/// let id = registry.cap_id("counting.done").expect("declared above");
/// assert_eq!(registry.capability(id).owner, "counting");
/// assert!(registry.cap_id("counting.undone").is_none());
/// # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
/// ```
pub struct FlmRegistry {
    features: Vec<FeatureInfo>,
    lang: BuiltLang,
    structural: Arc<StructuralElements>,
    elements: Vec<ElementEntry>,
    elem_index: BTreeMap<usize, ElemIdx>,
    elements_by_name: BTreeMap<ElementName, Vec<ElemIdx>>,
    aspects: BTreeMap<TypeId, (&'static str, ElemBitset)>,
    occurrable: BTreeSet<ElementName>,
    capabilities: Vec<CapabilityEntry>,
    cap_ids: BTreeMap<CapName, CapId>,
    collections: BTreeMap<CapName, CollectionEntry>,
    tables: Vec<TableEntry>,
    table_ids: BTreeMap<CapName, TableId>,
    stages: Vec<StageEntry>,
    stage_ids: BTreeMap<StageName, StageIdx>,
    orderings: Vec<OrderingDef>,
    rules: Vec<RuleEntry>,
    rule_ids: BTreeMap<RuleName, RuleIdx>,
    rules_by_element: BTreeMap<ElementName, Vec<RuleIdx>>,
    presentation: BTreeMap<ElementName, PresentationEntry>,
    backends: BTreeMap<BackendName, BackendEntry>,
    outputs: BTreeMap<OutputName, OutputEntry>,
    workflows: BTreeMap<WorkflowName, WorkflowEntry>,
    options: BTreeMap<OptionKey, OptionDecl>,
    services: Services,
    styles: Vec<(FeatureName, StyleDef)>,
    scripts: Vec<(FeatureName, ScriptDef)>,
    native_implications: Vec<(FeatureName, NativeImplication)>,
    peer_backends: Vec<PeerBackendEntry>,
    peer_elements: Vec<PeerElementEntry>,
    peer_tables: Vec<PeerTableEntry>,
    peer_scoped_packages: Vec<PeerScopedPackageEntry>,
    peer_element_values: Vec<PeerElementValueEntry>,
    argument_roles: Vec<ArgumentRoleEntry>,
    peer_argument_roles: Vec<PeerArgumentRoleEntry>,
    declarations: Declarations,
    digest: u64,
    build_diagnostics: FlmDiagnostics,
}

/// Adds to `out` the element values that the declaration `def` creates, each
/// with its owner, in declaration order: the declaration's own element, then
/// the element of each field of each of its tack-on arguments.
fn element_values_of(def: &ElementDef, out: &mut Vec<(Arc<ElementSpec>, FeatureName)>) {
    out.push((Arc::clone(&def.element), def.owner.clone()));
    let args = match &def.syntax {
        SyntaxDef::Macro { args } | SyntaxDef::Inclusion { args } | SyntaxDef::Specials { args, .. } => args.as_slice(),
        SyntaxDef::Environment { args, .. } => args.as_slice(),
        SyntaxDef::Synthesized => &[],
    };
    for arg in args {
        for field in arg.kind.tack_on_fields().unwrap_or_default() {
            out.push((Arc::clone(field.element()), def.owner.clone()));
        }
    }
}

impl FlmRegistry {
    /// Numbers the checked declarations `roster` and builds the lookup
    /// structures over them, together with the other results of the build.
    ///
    /// `roster` must have passed the checks of the validate phase. Numbering
    /// resolves names to numbers once more, and returns a reference that the
    /// checks let through as an error instead of panicking, so a gap in the
    /// checks makes the build fail instead of a later step. `set_aside` is
    /// the result of resolving the peer declarations: for each rule name, the
    /// unresolved names the rule depends on.
    ///
    /// # Errors
    ///
    /// Every [`BuildError::Undeclared`] found while numbering.
    pub(crate) fn assemble(
        roster: Roster,
        structural: Arc<StructuralElements>,
        lang: BuiltLang,
        services: Services,
        digest: u64,
        build_diagnostics: FlmDiagnostics,
        set_aside: BTreeMap<RuleName, Vec<PeerName>>,
    ) -> Result<FlmRegistry, Vec<BuildError>> {
        let mut errors: Vec<BuildError> = Vec::new();

        // --- elements -----------------------------------------------------------------
        let mut elements: Vec<ElementEntry> = Vec::new();
        let mut elem_index: BTreeMap<usize, ElemIdx> = BTreeMap::new();
        let mut elements_by_name: BTreeMap<ElementName, Vec<ElemIdx>> = BTreeMap::new();
        // An element value two features declare is numbered where it is first
        // met, and owned by the feature placed last in the final order: between
        // features, the feature placed last wins. The language's own elements
        // are owned by no feature, so a feature that declares one owns it.
        // See decision rationale at [§flm-dd-dr:feat-order].
        let final_order = &roster.features;
        let position = |owner: &FeatureName| final_order.iter().position(|info| info.name == *owner);
        let mut intern_element =
            |element: &Arc<ElementSpec>, owner: FeatureName, kind: ElementKind| {
                let key = Arc::as_ptr(element) as usize;
                if let Some(idx) = elem_index.get(&key) {
                    let entry = &mut elements[idx.0 as usize];
                    if position(&owner) > position(&entry.owner) {
                        entry.owner = owner;
                    }
                    return;
                }
                let idx = ElemIdx(elements.len() as u32);
                elem_index.insert(key, idx);
                elements_by_name.entry(element.name.clone()).or_default().push(idx);
                elements.push(ElementEntry { element: Arc::clone(element), owner, idx, kind });
            };
        let language_owner: FeatureName = FeatureName::from(crate::lang::mint::LANGUAGE_OWNER);
        for element in structural.all() {
            intern_element(element, language_owner.clone(), ElementKind::Structural);
        }
        let mut values: Vec<(Arc<ElementSpec>, FeatureName)> = Vec::new();
        for def in &roster.elements {
            element_values_of(def, &mut values);
        }
        for fallback in &roster.fallbacks {
            values.push((Arc::clone(&fallback.element), fallback.owner.clone()));
        }
        for package in &roster.scoped_packages {
            for def in &package.elements {
                element_values_of(def, &mut values);
            }
        }
        for (element, owner) in &values {
            intern_element(element, owner.clone(), ElementKind::Source);
        }
        for record in &roster.presentation {
            intern_element(&record.element, record.owner.clone(), ElementKind::Presentation);
        }
        let mut aspects: BTreeMap<TypeId, (&'static str, ElemBitset)> = BTreeMap::new();
        for entry in &elements {
            for (id, name, _) in entry.element.aspects.entries() {
                aspects.entry(id).or_insert_with(|| (name, ElemBitset::new())).1.insert(entry.idx);
            }
        }
        let occurrable: BTreeSet<ElementName> = elements
            .iter()
            .filter(|entry| entry.kind != ElementKind::Presentation)
            .filter(|entry| entry.element.name != structural::PLACEHOLDER)
            .map(|entry| entry.element.name.clone())
            .collect();

        // --- capabilities and tables ----------------------------------------------------
        let mut capabilities: Vec<CapabilityEntry> = Vec::new();
        let mut cap_ids: BTreeMap<CapName, CapId> = BTreeMap::new();
        let mut refines_names: Vec<Option<CapName>> = Vec::new();
        for (owner, def) in &roster.capabilities {
            let id = CapId(capabilities.len() as u32);
            cap_ids.insert(def.name.clone(), id);
            refines_names.push(def.refines.clone());
            capabilities.push(CapabilityEntry {
                name: def.name.clone(),
                docs: def.docs.clone(),
                owner: owner.clone(),
                id,
                refines: None,
                default: def.default,
                table: None,
                external: false,
                unprovided: def.unprovided.clone(),
                ancestors: Vec::new(),
                children: Vec::new(),
            });
        }
        let mut tables: Vec<TableEntry> = Vec::new();
        let mut table_ids: BTreeMap<CapName, TableId> = BTreeMap::new();
        for record in &roster.tables {
            let table_id = TableId(tables.len() as u32);
            let cap_id = CapId(capabilities.len() as u32);
            cap_ids.insert(record.def.name.clone(), cap_id);
            table_ids.insert(record.def.name.clone(), table_id);
            refines_names.push(record.refines.clone());
            capabilities.push(CapabilityEntry {
                name: record.def.name.clone(),
                docs: record.def.docs.clone(),
                owner: record.def.owner.clone(),
                id: cap_id,
                refines: None,
                default: record.default,
                table: Some(table_id),
                external: record.external,
                unprovided: None,
                ancestors: Vec::new(),
                children: Vec::new(),
            });
            tables.push(TableEntry {
                def: record.def.clone(),
                id: table_id,
                cap: cap_id,
                external: record.external,
            });
        }
        for (index, parent) in refines_names.iter().enumerate() {
            if let Some(parent) = parent {
                match cap_ids.get(parent) {
                    Some(parent_id) => {
                        capabilities[index].refines = Some(*parent_id);
                        capabilities[parent_id.0 as usize].children.push(CapId(index as u32));
                    }
                    None => errors.push(BuildError::Undeclared {
                        feature: capabilities[index].owner.clone(),
                        kind: "capability",
                        site: Site::CapabilityRefines {
                            capability: capabilities[index].name.clone(),
                        },
                        name: parent.name().clone(),
                        roster: cap_ids.keys().map(|n| n.name().clone()).collect(),
                    }),
                }
            }
        }
        for index in 0..capabilities.len() {
            let mut chain = vec![CapId(index as u32)];
            let mut current = capabilities[index].refines;
            while let Some(parent) = current {
                if chain.contains(&parent) {
                    break;
                }
                chain.push(parent);
                current = capabilities[parent.0 as usize].refines;
            }
            capabilities[index].ancestors = chain;
        }
        let cap_roster: Vec<Name> = cap_ids.keys().map(|n| n.name().clone()).collect();
        let mut collections: BTreeMap<CapName, CollectionEntry> = BTreeMap::new();
        for (owner, def) in &roster.collections {
            collections.insert(
                def.name.clone(),
                CollectionEntry {
                    name: def.name.clone(),
                    docs: def.docs.clone(),
                    owner: owner.clone(),
                    members: BTreeSet::new(),
                },
            );
        }
        for (owner, collection, member) in &roster.members {
            match (collections.get_mut(collection), cap_ids.get(member)) {
                (Some(entry), Some(id)) => {
                    entry.members.insert(*id);
                }
                _ => errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "capability",
                    site: Site::CollectionMember {
                        collection: collection.clone(),
                        member: member.clone(),
                    },
                    name: member.name().clone(),
                    roster: cap_roster.clone(),
                }),
            }
        }

        // --- stages -----------------------------------------------------------------------
        let mut stages: Vec<StageEntry> = Vec::new();
        let mut stage_ids: BTreeMap<StageName, StageIdx> = BTreeMap::new();
        for record in roster.stages {
            let idx = StageIdx(stages.len() as u32);
            let mut provides: BTreeSet<CapId> = BTreeSet::new();
            let mut writes: Vec<TableId> = Vec::new();
            let add_cap = |id: CapId, provides: &mut BTreeSet<CapId>| {
                provides.extend(capabilities[id.0 as usize].ancestors.iter().copied());
            };
            for capability in &record.decl.provides {
                match cap_ids.get(capability) {
                    Some(id) => add_cap(*id, &mut provides),
                    None => errors.push(BuildError::Undeclared {
                        feature: record.owner.clone(),
                        kind: "capability",
                        site: Site::StageProvides { stage: record.decl.name.clone() },
                        name: capability.name().clone(),
                        roster: cap_roster.clone(),
                    }),
                }
            }
            for table in &record.decl.writes {
                match table_ids.get(&table.name) {
                    Some(id) => {
                        writes.push(*id);
                        add_cap(tables[id.0 as usize].cap, &mut provides);
                    }
                    None => errors.push(BuildError::Undeclared {
                        feature: record.owner.clone(),
                        kind: "table",
                        site: Site::StageWrites { stage: record.decl.name.clone() },
                        name: table.name.name().clone(),
                        roster: table_ids.keys().map(|n| n.name().clone()).collect(),
                    }),
                }
            }
            stage_ids.insert(record.decl.name.clone(), idx);
            stages.push(StageEntry {
                name: record.decl.name.clone(),
                kind: record.kind,
                decl: record.decl,
                rewrite: record.rewrite,
                owner: record.owner,
                idx,
                provides,
                writes,
            });
        }
        let orderings: Vec<OrderingDef> =
            roster.orderings.into_iter().map(|(_, def)| def).collect();

        // --- rules, presentation, backends, outputs, workflows ------------------------
        let mut rules: Vec<RuleEntry> = Vec::new();
        let mut rule_ids: BTreeMap<RuleName, RuleIdx> = BTreeMap::new();
        let mut rules_by_element: BTreeMap<ElementName, Vec<RuleIdx>> = BTreeMap::new();
        for (owner, rule) in roster.rules {
            let idx = RuleIdx(rules.len() as u32);
            rule_ids.insert(rule.name.clone(), idx);
            rules_by_element.entry(rule.element.clone()).or_default().push(idx);
            let set_aside = set_aside.get(&rule.name).cloned().unwrap_or_default();
            rules.push(RuleEntry { rule, owner, idx, set_aside });
        }
        let presentation: BTreeMap<ElementName, PresentationEntry> = roster
            .presentation
            .into_iter()
            .map(|record| {
                (
                    record.element.name.clone(),
                    PresentationEntry {
                        element: record.element,
                        payload: record.payload,
                        docs: record.docs,
                        owner: record.owner,
                    },
                )
            })
            .collect();
        let backends: BTreeMap<BackendName, BackendEntry> = roster
            .backends
            .into_iter()
            .map(|record| {
                (
                    record.name.clone(),
                    BackendEntry {
                        name: record.name,
                        native: record.native.into_iter().collect(),
                        lowering: StageIdx(record.lowering as u32),
                        emit: StageIdx(record.emit as u32),
                        final_stage: StageIdx(record.final_stage as u32),
                        owner: record.owner,
                        docs: record.docs,
                    },
                )
            })
            .collect();
        let mut outputs: BTreeMap<OutputName, OutputEntry> = BTreeMap::new();
        for (owner, def) in roster.outputs {
            match (table_ids.get(&def.table), cap_ids.get(&def.capability)) {
                (Some(table), Some(capability)) => {
                    outputs.insert(
                        def.name.clone(),
                        OutputEntry { table: *table, capability: *capability, def, owner },
                    );
                }
                (None, _) => errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "table",
                    site: Site::OutputTable { output: def.name.clone() },
                    name: def.table.name().clone(),
                    roster: table_ids.keys().map(|n| n.name().clone()).collect(),
                }),
                (_, None) => errors.push(BuildError::Undeclared {
                    feature: owner.clone(),
                    kind: "capability",
                    site: Site::OutputCapability { output: def.name.clone() },
                    name: def.capability.name().clone(),
                    roster: cap_roster.clone(),
                }),
            }
        }
        let workflows: BTreeMap<WorkflowName, WorkflowEntry> = roster
            .workflows
            .into_iter()
            .map(|(owner, def)| (def.name.clone(), WorkflowEntry { def, owner }))
            .collect();
        let options: BTreeMap<OptionKey, OptionDecl> =
            roster.options.into_iter().map(|decl| (decl.key.clone(), decl)).collect();
        let peer_backends: Vec<PeerBackendEntry> = roster
            .peer_backends
            .into_iter()
            .map(|record| PeerBackendEntry {
                resolved: backends.contains_key(&record.name),
                name: record.name,
                owner: record.owner,
                docs: record.docs,
            })
            .collect();
        let peer_elements: Vec<PeerElementEntry> = roster
            .peer_elements
            .into_iter()
            .map(|record| PeerElementEntry {
                resolved: presentation.contains_key(&record.name),
                name: record.name,
                payload: record.payload,
                owner: record.owner,
                docs: record.docs,
            })
            .collect();
        let peer_tables: Vec<PeerTableEntry> = roster
            .peer_tables
            .into_iter()
            .map(|record| PeerTableEntry {
                resolved: table_ids.contains_key(&record.name),
                name: record.name,
                value: record.value,
                owner: record.owner,
                docs: record.docs,
            })
            .collect();
        let packages: BTreeSet<&Name> =
            roster.scoped_packages.iter().map(|package| &package.name).collect();
        let peer_scoped_packages: Vec<PeerScopedPackageEntry> = roster
            .peer_scoped_packages
            .into_iter()
            .map(|record| PeerScopedPackageEntry {
                resolved: packages.contains(&record.name),
                name: record.name,
                owner: record.owner,
                docs: record.docs,
            })
            .collect();
        let peer_element_values: Vec<PeerElementValueEntry> = roster
            .peer_element_values
            .into_iter()
            .map(|record| PeerElementValueEntry {
                element: record.element,
                names: record.names,
                owner: record.owner,
                resolved: record.resolved,
            })
            .collect();

        let argument_roles: Vec<ArgumentRoleEntry> = roster
            .argument_roles
            .into_iter()
            .map(|(owner, def)| ArgumentRoleEntry { name: def.name, docs: def.docs, owner })
            .collect();
        let peer_argument_roles: Vec<PeerArgumentRoleEntry> = roster
            .peer_argument_roles
            .into_iter()
            .map(|record| PeerArgumentRoleEntry {
                resolved: argument_roles.iter().any(|role| role.name == record.name),
                name: record.name,
                owner: record.owner,
            })
            .collect();
        // The declarations are kept as the build leaves them: moved, not
        // copied, now that nothing else reads them.
        let declarations =
            Declarations::new(roster.elements, roster.scoped_packages, roster.fallbacks, roster.overrides);

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(FlmRegistry {
            features: roster.features,
            lang,
            structural,
            elements,
            elem_index,
            elements_by_name,
            aspects,
            occurrable,
            capabilities,
            cap_ids,
            collections,
            tables,
            table_ids,
            stages,
            stage_ids,
            orderings,
            rules,
            rule_ids,
            rules_by_element,
            presentation,
            backends,
            outputs,
            workflows,
            options,
            services,
            styles: roster.styles,
            scripts: roster.scripts,
            native_implications: roster.native_implications,
            peer_backends,
            peer_elements,
            peer_tables,
            peer_scoped_packages,
            peer_element_values,
            argument_roles,
            peer_argument_roles,
            declarations,
            digest,
            build_diagnostics,
        })
    }

    // --- features and the language ----------------------------------------------------

    /// The registry's build diagnostics: the problems found while the registry
    /// was built that did not make the build fail.
    ///
    /// They are of two kinds:
    ///
    /// - What the [late-initialization hooks](super::LateInit) reported about
    ///   the configuration they prepared (for example, the errors and warnings
    ///   of author-written definitions compiled when the registry was built,
    ///   each identifying the definition it belongs to).
    /// - One Note ([`UnresolvedPeer`](super::UnresolvedPeer)) for each [peer
    ///   declaration](PeerName) that no feature of this registry resolves,
    ///   which gives the feature, the name, and the rules and stages set aside
    ///   because of it. For a service slot or an element type, there is one
    ///   Note per feature and name, however many values the feature gave for
    ///   it.
    ///
    /// Nothing else is recorded here; for example, a member added to a
    /// collection that no feature declares is dropped without a diagnostic.
    ///
    /// The build succeeds even when these diagnostics include errors. (The
    /// user-definitions feature, for example, keeps a refused definition
    /// registered and reports it again wherever a document uses it.) An
    /// embedding program can report this collection once per build, so that
    /// the cause appears next to its consequences.
    pub fn build_diagnostics(&self) -> &FlmDiagnostics {
        &self.build_diagnostics
    }

    /// The description of each registered feature, in registration order.
    pub fn features(&self) -> &[FeatureInfo] {
        &self.features
    }

    /// Whether a feature called `name` is registered. A stage or a host that
    /// adapts to the presence of a feature asks here.
    pub fn has_feature(&self, name: &str) -> bool {
        self.features.iter().any(|info| info.name == name)
    }

    /// The description of the feature called `name`, or `None` when no such
    /// feature is registered.
    pub fn feature(&self, name: &str) -> Option<&FeatureInfo> {
        self.features.iter().find(|info| info.name == name)
    }

    /// The FLM language with which every document is parsed under this
    /// registry: the callable names, their syntax and the parsing settings,
    /// built from the element declarations of every feature.
    pub fn lang(&self) -> &BuiltLang {
        &self.lang
    }

    /// The language's structural elements as this registry froze them, with
    /// the aspects that features attached to them.
    pub fn structural(&self) -> &Arc<StructuralElements> {
        &self.structural
    }

    /// The element declarations, scoped packages, fallbacks and overrides
    /// that this registry's features declared, as declared
    /// ([`Declarations`]).
    ///
    /// The language that documents are parsed with ([`FlmRegistry::lang`])
    /// was built from them, and keeps only what a parse needs. This accessor
    /// gives the declarations themselves, with the names, syntax, arguments,
    /// argument roles, parsing modes and documentation of every command, for
    /// a program that describes what an author may write.
    pub fn declarations(&self) -> &Declarations {
        &self.declarations
    }

    // --- elements ---------------------------------------------------------------------

    /// Every registered element value, in numbering order: entry `n` is the
    /// element with index `n`.
    pub fn elements(&self) -> &[ElementEntry] {
        &self.elements
    }

    /// This registry's number for the element **value** `element`, or `None`
    /// when this registry does not hold that value.
    ///
    /// Values are told apart by identity (which shared value it is), not by
    /// name: a value copied from another registry is not found here, even if
    /// a value of the same name is registered. An index is therefore
    /// unambiguous even when two features register elements under one name.
    pub fn elem_idx(&self, element: &Arc<ElementSpec>) -> Option<ElemIdx> {
        self.elem_index.get(&(Arc::as_ptr(element) as usize)).copied()
    }

    /// The element entry numbered `idx`, or `None` when this registry has no
    /// such number, which means that the index came from another registry.
    pub fn element(&self, idx: ElemIdx) -> Option<&ElementEntry> {
        self.elements.get(idx.0 as usize)
    }

    /// The numbers of every registered value whose element type name is
    /// `name`, in numbering order; empty when no value has that name.
    ///
    /// Two features may each register an element of the same type name, so
    /// this method returns a list.
    pub fn elements_named(&self, name: &str) -> &[ElemIdx] {
        self.elements_by_name.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// A registered value whose element type name is `name`, or `None` when
    /// no value has that name. Where several features register a value under
    /// the name, the value of the feature placed last in the feature order;
    /// of one feature's values, the first in numbering order.
    ///
    /// Use [`element_value`](FlmRegistry::element_value) instead wherever
    /// taking one of several values would be wrong.
    pub fn element_named(&self, name: &str) -> Option<&Arc<ElementSpec>> {
        let mut chosen: Option<&ElementEntry> = None;
        for entry in self.elements_named(name).iter().filter_map(|idx| self.element(*idx)) {
            let later = match chosen {
                Some(best) => self.feature_position(&entry.owner) > self.feature_position(&best.owner),
                None => true,
            };
            if later {
                chosen = Some(entry);
            }
        }
        chosen.map(|entry| &entry.element)
    }

    /// The position of the feature `name` in the final order of the features
    /// (their registration order), counting from zero, or `None` when `name`
    /// is no feature of this registry, such as the language's own name or the
    /// embedding program's ([`HOST_FEATURE`](super::HOST_FEATURE)).
    ///
    /// Comparing positions picks, of several competing items of different
    /// features, the one of the feature placed last; an item of no feature
    /// compares lower than every feature's.
    pub(crate) fn feature_position(&self, name: &str) -> Option<usize> {
        self.features.iter().position(|info| info.name == name)
    }

    // See the design rationale at [§flm-dd-dr:dm-one-value-per-name].
    /// The registered value whose element type name is `name`, when that
    /// value is unique: `Ok(Some(..))` when exactly one value has the name,
    /// `Ok(None)` when none does, and an error when several do.
    ///
    /// Reading a stored tree back uses this lookup. A node written to storage
    /// holds its element's type name, not the element itself, so reading it
    /// back must find the value again by name. Where the name does not
    /// identify one value, reading refuses instead of taking the first match,
    /// because a wrong element would make an incorrect document whose damage
    /// would be found much later.
    ///
    /// # Errors
    ///
    /// [`AmbiguousElement`], which lists every feature that registered a
    /// value under `name`.
    pub fn element_value(&self, name: &str) -> Result<Option<&Arc<ElementSpec>>, AmbiguousElement> {
        match self.elements_named(name) {
            [] => Ok(None),
            [idx] => Ok(self.element(*idx).map(|entry| &entry.element)),
            several => Err(AmbiguousElement {
                name: ElementName::from(name),
                owners: several
                    .iter()
                    .filter_map(|idx| self.element(*idx))
                    .map(|e| e.owner.clone())
                    .collect(),
            }),
        }
    }

    /// The set of every registered element that carries the aspect `A`, given
    /// as its trait-object type: `registry.aspect_bitset::<dyn Referenceable>()`.
    /// Empty when no element carries it.
    ///
    /// The registry build computes the set, so this method copies a small bit
    /// set instead of searching. A stage that visits every element with some
    /// behavior asks once, then tests each node against the result.
    pub fn aspect_bitset<A: ?Sized + Any>(&self) -> ElemBitset {
        self.aspects.get(&TypeId::of::<A>()).map(|(_, bits)| bits.clone()).unwrap_or_default()
    }

    /// Whether the element numbered `idx` carries the aspect `A`. `false` when
    /// no element carries it, and when `idx` is not a number of this
    /// registry.
    pub fn has_aspect<A: ?Sized + Any>(&self, idx: ElemIdx) -> bool {
        self.aspects.get(&TypeId::of::<A>()).map(|(_, bits)| bits.contains(idx)).unwrap_or(false)
    }

    /// The type names of every aspect that some registered element carries, in
    /// name order. This list serves to show a person what a registry offers,
    /// not for lookup.
    pub fn aspect_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.aspects.values().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names
    }

    /// The type names of every element that can appear in a parsed document:
    /// every element of [`ElementKind::Source`] (an element declared with a
    /// syntax, an element a declaration creates without syntax, the element of
    /// a field, the element of a fallback), and every structural element
    /// except the template placeholder, which exists only inside a template.
    ///
    /// Presentation elements are not in the set, because they appear only in
    /// the trees a backend writes. The elements that represent a command no
    /// feature defines (`flm.unknown.macro`, `flm.unknown.environment`) are in
    /// the set, since a document can contain one.
    ///
    /// When a plan is built, the check that a backend can write everything a
    /// document may contain runs over this set: every element in it must have
    /// a lowering into the backend's native set.
    pub fn occurrable_elements(&self) -> &BTreeSet<ElementName> {
        &self.occurrable
    }

    // --- capabilities -----------------------------------------------------------------

    /// Every capability, including the capabilities of tables, in identifier
    /// order: entry `n` is the capability with identifier `n`.
    pub fn capabilities(&self) -> &[CapabilityEntry] {
        &self.capabilities
    }

    /// The identifier of the capability called `name` (one declared on its own
    /// or one that came with a table), or `None` when no capability has that
    /// name.
    pub fn cap_id(&self, name: &str) -> Option<CapId> {
        self.cap_ids.get(name).copied()
    }

    /// The capability with the identifier `id`.
    ///
    /// # Panics
    ///
    /// Panics when `id` is beyond this registry's capabilities, which can
    /// happen only when another registry assigned it; an identifier from
    /// another registry that falls within range returns the wrong entry
    /// instead. Identifiers obtained from this registry, from its entries, or
    /// from a plan built with it are always valid here.
    pub fn capability(&self, id: CapId) -> &CapabilityEntry {
        &self.capabilities[id.0 as usize]
    }

    /// The capability called `name`, or `None` when no capability has that
    /// name.
    pub fn capability_named(&self, name: &str) -> Option<&CapabilityEntry> {
        self.cap_id(name).map(|id| self.capability(id))
    }

    /// The default refinement of the capability `id`, which a demand for that
    /// capability alone selects, or `None` when none of its refinements is
    /// marked as the default.
    ///
    /// The registry build checks that at most one refinement of a capability
    /// is marked as the default.
    ///
    /// # Panics
    ///
    /// Panics when `id` is not an identifier of this registry.
    pub fn default_refinement(&self, id: CapId) -> Option<CapId> {
        self.capability(id)
            .refinements()
            .iter()
            .copied()
            .find(|child| self.capability(*child).default)
    }

    /// Every declared collection, in name order.
    pub fn collections(&self) -> impl Iterator<Item = &CollectionEntry> + '_ {
        self.collections.values()
    }

    /// The collection called `name`, or `None` when no collection has that
    /// name.
    pub fn collection(&self, name: &str) -> Option<&CollectionEntry> {
        self.collections.get(name)
    }

    // --- tables -------------------------------------------------------------------------

    /// Every declared table, in identifier order: entry `n` is the table with
    /// identifier `n`.
    pub fn tables(&self) -> &[TableEntry] {
        &self.tables
    }

    /// The identifier of the table called `name`, or `None` when no table has
    /// that name.
    pub fn table_id(&self, name: &str) -> Option<TableId> {
        self.table_ids.get(name).copied()
    }

    /// The table with the identifier `id`.
    ///
    /// # Panics
    ///
    /// Panics when `id` is not an identifier of this registry.
    pub fn table(&self, id: TableId) -> &TableEntry {
        &self.tables[id.0 as usize]
    }

    /// The table called `name`, or `None` when no table has that name.
    pub fn table_named(&self, name: &str) -> Option<&TableEntry> {
        self.table_id(name).map(|id| self.table(id))
    }

    /// The external tables: the tables that no stage writes, whose values the
    /// caller installs when a run is constructed, in identifier order. A host
    /// asks here which values it must supply.
    pub fn external_tables(&self) -> impl Iterator<Item = &TableEntry> + '_ {
        self.tables.iter().filter(|entry| entry.external)
    }

    /// Whether some registered stage writes the table `id`; only then can a
    /// plan schedule a stage that writes it.
    ///
    /// A declared table is in one of three situations, and this method tells
    /// the first from the other two: some stage writes it; it is an
    /// [external](TableEntry::external) table, whose value the host installs
    /// when a run is constructed and which no stage may write; or it is
    /// neither, because the stage that writes it belongs to a feature this
    /// registry does not hold, and nothing can provide it. `id`
    /// is an identifier of this registry, as for
    /// [`table`](FlmRegistry::table); an identifier beyond this registry's
    /// tables returns `false`.
    pub fn table_has_writer(&self, id: TableId) -> bool {
        self.stages.iter().any(|stage| stage.writes.contains(&id))
    }

    // --- stages -----------------------------------------------------------------------

    /// Every registered stage, of both forms, in registration order: entry `n`
    /// is the stage with index `n`.
    pub fn stages(&self) -> &[StageEntry] {
        &self.stages
    }

    /// The index of the stage called `name`, or `None` when no stage has that
    /// name.
    pub fn stage_idx(&self, name: &str) -> Option<StageIdx> {
        self.stage_ids.get(name).copied()
    }

    /// The stage with index `idx`.
    ///
    /// # Panics
    ///
    /// Panics when `idx` is not an index of this registry.
    pub fn stage(&self, idx: StageIdx) -> &StageEntry {
        &self.stages[idx.0 as usize]
    }

    /// The stage called `name`, or `None` when no stage has that name.
    pub fn stage_named(&self, name: &str) -> Option<&StageEntry> {
        self.stage_idx(name).map(|idx| self.stage(idx))
    }

    /// Every set of ordering edges that a feature placed on a stage, in
    /// registration order. A plan applies them in addition to the order that
    /// the stages' own declarations imply.
    pub fn orderings(&self) -> &[OrderingDef] {
        &self.orderings
    }

    // --- rules and presentation ---------------------------------------------------------

    /// Every registered lowering rule, in registration order: entry `n` is the
    /// rule with index `n`. The list includes the rules that are set aside.
    pub fn rules(&self) -> &[RuleEntry] {
        &self.rules
    }

    /// The index of the lowering rule called `name`, or `None` when no rule has
    /// that name.
    pub fn rule_idx(&self, name: &str) -> Option<RuleIdx> {
        self.rule_ids.get(name).copied()
    }

    /// The lowering rule with index `idx`.
    ///
    /// # Panics
    ///
    /// Panics when `idx` is not an index of this registry.
    pub fn rule(&self, idx: RuleIdx) -> &RuleEntry {
        &self.rules[idx.0 as usize]
    }

    /// The lowering rules that lower the element type called `element`, in
    /// registration order; empty when no rule does. The list includes the
    /// rules that are set aside.
    ///
    /// Several rules may lower one element (one per backend, or several
    /// renderings to choose from). A plan chooses one per backend in the rule
    /// selection order ([`SelectedRules`](crate::core::plan::SelectedRules)),
    /// from the plan request's lowering selections and the rules' default
    /// marks.
    pub fn rules_for(&self, element: &str) -> &[RuleIdx] {
        self.rules_by_element.get(element).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every lowering rule that is set aside, in registration order. A rule is
    /// set aside when it depends on a [peer declaration](PeerName) of its own
    /// feature (a backend, an element, or a table it reads) that no feature
    /// of this registry declares. A plan never selects such a rule, and the
    /// check that a backend can write every element ignores it; each entry's
    /// [`set_aside`](RuleEntry::set_aside) lists the unresolved names. Empty
    /// for a registry in which every peer declaration resolves, such as one
    /// built from the standard package alone.
    pub fn set_aside_rules(&self) -> impl Iterator<Item = &RuleEntry> + '_ {
        self.rules.iter().filter(|entry| entry.is_set_aside())
    }

    /// Every [peer declaration](PeerName) of a backend that a feature made, in
    /// declaration order, each marked as resolved or not.
    pub fn peer_backends(&self) -> &[PeerBackendEntry] {
        &self.peer_backends
    }

    /// Every [peer declaration](PeerName) of a presentation element that a
    /// feature made, in declaration order, each marked as resolved or not. A
    /// resolved entry's payload type is the registered element's: the build
    /// refuses any other.
    pub fn peer_elements(&self) -> &[PeerElementEntry] {
        &self.peer_elements
    }

    /// Every [peer declaration](PeerName) of a table that a feature made, in
    /// declaration order, each marked as resolved or not. A resolved entry's
    /// value type is the declared table's: the build refuses any other.
    pub fn peer_tables(&self) -> &[PeerTableEntry] {
        &self.peer_tables
    }

    /// Every [peer declaration](PeerName) of a scoped package that a feature
    /// made, in declaration order, each marked as resolved or not
    /// ([`FeatureBuilder::peer_scoped_package`](super::FeatureBuilder::peer_scoped_package)).
    pub fn peer_scoped_packages(&self) -> &[PeerScopedPackageEntry] {
        &self.peer_scoped_packages
    }

    /// Every [peer declaration](PeerName) of an element value that a feature
    /// made, in declaration order, each marked as resolved or not
    /// ([`FeatureBuilder::peer_element_value`](super::FeatureBuilder::peer_element_value)).
    /// The value of a resolved declaration is also among the registry's
    /// [elements](FlmRegistry::elements); the value of an unresolved one was
    /// dropped.
    pub fn peer_element_values(&self) -> &[PeerElementValueEntry] {
        &self.peer_element_values
    }

    /// Every [peer declaration](PeerName) of an argument role that a feature
    /// made, in declaration order, each marked as resolved or not
    /// ([`FeatureBuilder::peer_argument_role`](super::FeatureBuilder::peer_argument_role)).
    pub fn peer_argument_roles(&self) -> &[PeerArgumentRoleEntry] {
        &self.peer_argument_roles
    }

    // --- argument roles -----------------------------------------------------------------

    /// Every argument role, in declaration order. An argument role is a
    /// registered name for what an argument holds (a label, a citation key,
    /// the path of a file), declared with
    /// [`FeatureBuilder::argument_role`](super::FeatureBuilder::argument_role);
    /// the arguments that have a role are listed by
    /// [`Declarations::role_uses`](super::Declarations::role_uses).
    pub fn argument_roles(&self) -> &[ArgumentRoleEntry] {
        &self.argument_roles
    }

    /// The argument role called `name`, or `None` when no feature of this
    /// registry declares it.
    pub fn argument_role_named(&self, name: &str) -> Option<&ArgumentRoleEntry> {
        self.argument_roles.iter().find(|role| role.name == *name)
    }

    /// Every peer contribution: every value that a feature contributed to a
    /// service slot another feature owns
    /// ([`FeatureBuilder::peer_slot`](super::FeatureBuilder::peer_slot)), in
    /// contribution order, each marked as resolved or not. An unresolved
    /// contribution refers to a service slot that no feature of this registry
    /// declares, and its value was dropped when the service slots were
    /// assembled.
    ///
    /// This is the service-slot form of a [peer declaration](PeerName). Read
    /// the values themselves through [`services`](FlmRegistry::services), where
    /// a resolved contribution is one value of its service slot like any other.
    pub fn peer_slots(&self) -> &[PeerContribution] {
        self.services.peer_contributions()
    }

    /// Every registered presentation element, in name order.
    pub fn presentation_elements(&self) -> impl Iterator<Item = &PresentationEntry> + '_ {
        self.presentation.values()
    }

    /// The presentation element called `name`, or `None` when no presentation
    /// element has that name.
    pub fn presentation_element(&self, name: &str) -> Option<&PresentationEntry> {
        self.presentation.get(name)
    }

    // --- backends, outputs, workflows -----------------------------------------------------

    /// Every registered backend, in name order.
    pub fn backends(&self) -> impl Iterator<Item = &BackendEntry> + '_ {
        self.backends.values()
    }

    /// The backend called `name`, or `None` when no backend has that name.
    pub fn backend(&self, name: &str) -> Option<&BackendEntry> {
        self.backends.get(name)
    }

    /// Every registered output, in name order. A host lists these to show what
    /// can be produced.
    pub fn outputs(&self) -> impl Iterator<Item = &OutputEntry> + '_ {
        self.outputs.values()
    }

    /// The output called `name`, or `None` when no output has that name.
    pub fn output(&self, name: &str) -> Option<&OutputEntry> {
        self.outputs.get(name)
    }

    /// Every registered workflow, in name order, as declared (without the
    /// fields it inherits).
    pub fn workflows(&self) -> impl Iterator<Item = &WorkflowEntry> + '_ {
        self.workflows.values()
    }

    /// The workflow called `name` as declared (without the fields it
    /// inherits), or `None` when no workflow has that name.
    pub fn workflow(&self, name: &str) -> Option<&WorkflowEntry> {
        self.workflows.get(name)
    }

    /// The workflow called `name` and the workflows it extends, directly or
    /// not, in order from the workflow that extends none to the workflow
    /// called `name`, which comes last. `None` when no workflow has that name.
    ///
    /// A workflow's fields are read along this chain
    /// ([`combined_workflow`](FlmRegistry::combined_workflow)); a host that
    /// keeps data per workflow walks the chain to find what a workflow
    /// inherits. The registry build refuses a workflow that extends an
    /// unregistered workflow, and a chain that leads back to itself, so the
    /// chain always starts at a registered workflow that extends none.
    pub fn workflow_chain(&self, name: &str) -> Option<Vec<&WorkflowDef>> {
        let mut chain = vec![&self.workflow(name)?.def];
        while let Some(parent) = chain.last().and_then(|def| def.extends.as_ref()) {
            // The registry build rules out both ends of this walk; the checks only keep
            // it finite.
            let Some(entry) = self.workflow(parent) else { break };
            if chain.len() > self.workflows.len() {
                break;
            }
            chain.push(&entry.def);
        }
        chain.reverse();
        Some(chain)
    }

    /// The workflow called `name` with the fields it inherits: the fields of
    /// the workflows of its [chain](FlmRegistry::workflow_chain), combined
    /// from the workflow that extends none towards the workflow called
    /// `name`, each field a workflow declares replacing the inherited one
    /// ([`WorkflowDef::replace_declared_fields`]). A workflow that lists
    /// outputs therefore replaces the inherited list instead of adding to it,
    /// and a workflow that lists none inherits the whole list. The result has
    /// the name `name` and the `extends` of the workflow called `name`. `None`
    /// when no workflow has that name.
    pub fn combined_workflow(&self, name: &str) -> Option<WorkflowDef> {
        let chain = self.workflow_chain(name)?;
        let named = chain.last()?.name.clone();
        let mut combined = WorkflowDef::new(named);
        for def in chain {
            combined.replace_declared_fields(def.clone());
        }
        Some(combined)
    }

    // --- options, services, records, digest -----------------------------------------------

    /// Every declared option type, keyed by the type itself, in key order.
    /// See [option types](crate::model::options).
    pub fn options(&self) -> &BTreeMap<OptionKey, OptionDecl> {
        &self.options
    }

    /// The declaration of the option type `key`, or `None` when no feature
    /// declared it. The declaration gives the feature that owns the option
    /// type and its binding: where its values may be set.
    pub fn option_decl(&self, key: &OptionKey) -> Option<&OptionDecl> {
        self.options.get(key)
    }

    /// The assembled services: every service slot that features declared,
    /// filled with the contributions that features and the embedding program
    /// made. See [`crate::core::services`].
    pub fn services(&self) -> &Services {
        &self.services
    }

    /// The [style definitions](StyleDef) that features recorded, each with the
    /// feature that recorded it, in registration order.
    ///
    /// A backend that writes stylesheets into its output resolves the
    /// requirements recorded for its trees against this list, in this order. A
    /// host that assembles the stylesheets itself reads the list directly.
    pub fn styles(&self) -> &[(FeatureName, StyleDef)] {
        &self.styles
    }

    /// The [script definitions](ScriptDef) that features recorded, each with
    /// the feature that recorded it, in registration order: the counterpart of
    /// [`styles`](FlmRegistry::styles) for scripts, resolved the same way.
    pub fn scripts(&self) -> &[(FeatureName, ScriptDef)] {
        &self.scripts
    }

    /// The [native-set implications](NativeImplication) that features
    /// declared, each with the feature that declared it, in registration
    /// order.
    ///
    /// The build checked that every backend of this registry satisfies every
    /// implication, so a backend's native set holds what each implication
    /// requires. This list serves a host or a test that inspects them. They are
    /// not inputs of the [digest](FlmRegistry::digest), because an implication
    /// decides whether a registry builds, never what a run produces.
    pub fn native_implications(&self) -> &[(FeatureName, NativeImplication)] {
        &self.native_implications
    }

    /// This registry's [digest](crate::model::digest): a 64-bit number that
    /// stands for everything about the registry that can change what
    /// processing produces.
    ///
    /// It is part of every cache key the library forms, so that data computed
    /// under one registry is never read back under another.
    pub fn digest(&self) -> u64 {
        self.digest
    }
}

impl fmt::Debug for FlmRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmRegistry")
            .field("features", &self.features.iter().map(|i| &i.name).collect::<Vec<_>>())
            .field("elements", &self.elements.len())
            .field("capabilities", &self.capabilities.len())
            .field("tables", &self.tables.len())
            .field("stages", &self.stages.len())
            .field("rules", &self.rules.len())
            .field("digest", &format_args!("{:016x}", self.digest))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bitset_holds_and_lists_its_indices() {
        let mut set = ElemBitset::new();
        set.insert(ElemIdx(3));
        set.insert(ElemIdx(70));
        assert!(set.contains(ElemIdx(3)));
        assert!(!set.contains(ElemIdx(4)));
        assert_eq!(set.iter().collect::<Vec<_>>(), [ElemIdx(3), ElemIdx(70)]);
        assert_eq!(set.len(), 2);
    }
}
