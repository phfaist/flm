//! The feature builder, through which a feature declares what it contributes
//! to a registry; the collected declarations of one registry build; and the
//! error type that every check of the build reports.
//!
//! [`FeatureBuilder`] is the value a feature receives in
//! [`FlmFeature::register`](super::FlmFeature::register). Each of its methods
//! records one typed declaration (an element, a capability, a table, a stage,
//! a lowering rule, a backend, an output, a workflow, an option type, a
//! service slot, and so on) in the [`Roster`], the collected declarations of
//! one registry build, and attributes the declaration to the feature being
//! registered. The builder checks and resolves nothing while a feature
//! registers: the registry build does all of that once every feature has
//! registered (see [`super::build`]).
//!
//! Because the checks run afterwards, registration order never matters. A
//! feature may refer to a capability, a table or a stage that another feature
//! declares, whether or not that feature has registered yet, and a feature
//! never needs to be listed after the features it refers to. The only problem
//! a builder method reports at once is one it can detect on its own: a
//! [`Finalizer`] calling a method that only the first registration round may
//! call.
//!
//! # The two registration rounds
//!
//! [`FlmFeature::register`](super::FlmFeature::register) is the first round,
//! in which every method of the builder may be called. A feature's
//! [`Finalizer`] is the second round: it runs after every feature has
//! registered, once the build has assembled the service slots, and may read
//! the assembled services and declare entities derived from what other
//! features contributed. The service slots are complete by then, so in the
//! second round a feature may not declare a service slot, contribute to one,
//! require one, or register another finalizer. The builder refuses each of
//! these with [`BuildError::ContributionAfterAssembly`], and a
//! [`RoundTwoAction`] says which one was attempted.
//!
//! # Attribution of declarations
//!
//! No declaration type holds the name of the feature that makes it: the
//! builder records, on every declaration, the feature it was created for. A
//! feature therefore never writes its own name, and the same feature type can
//! be registered under another name without any change to its declarations.
//!
//! # Build errors
//!
//! [`BuildError`] is the error type of every phase of the registry build. Each
//! variant identifies the feature at fault wherever one feature is at fault,
//! and holds the parts of its message as data, not as a finished sentence: a
//! [`Site`] says where in a declaration a reference was written, and a list
//! gives the names that would have been valid. A host can therefore format or
//! translate the message. The `Display` implementation writes the standard
//! English message.
//!
//! See also [`super::entities`] for the declaration values that these methods
//! take, [features and
//! packages](crate::guide::concepts_overview#features-and-packages) for the
//! concepts, and [the builder only
//! records](crate::guide::extending#the-builder-only-records) in the guide to
//! extending FLM.

use std::any::{Any, TypeId};
use std::fmt;
use std::sync::Arc;

use crate::model::element::ElementSpec;
use crate::core::lowering::LoweringRule;
use crate::model::names::{
    BackendName, CapName, ElementName, FeatureName, Name, OutputName, RuleName, StageName,
    WorkflowName,
};
use crate::model::options::{Bind, OptionDecl, Options, TypeKey};
use crate::core::registry::entities::{
    ArgumentRoleDef, BackendDef, CapabilityDef, CollectionDef, Finalizer, LateInit, NativeImplication,
    OrderingDef, OutputDef, ScriptDef, StyleDef, WorkflowDef,
};
use crate::core::registry::feature::FeatureInfo;
use crate::core::registry::order::FeatureChain;
use crate::core::services::{
    Arity, Contribution, ServiceError, ServiceRequirement, Services, SlotDecl, SlotTarget,
};
use crate::core::stage::{RewriteDecl, RewriteStage, Stage, StageDecl};
use crate::core::data::table::{Table, TableDef, TableHandle, TableName, TableType};
use crate::lang::mint::{CallableKind, ElementDef, FallbackDef, OverrideDef, ScopedPackageDef};
use crate::lang::{LangBuildError, PresPayload};

/// The place in a declaration where a reference to another entity was written.
///
/// One declaration may refer to a dozen other entities, so a build error about
/// a reference must say both which name was wrong and where it was written.
/// This enum gives the place as data, with the name of the enclosing
/// declaration in each variant, so that a host can format or translate the
/// message instead of parsing a sentence. [`BuildError::Undeclared`],
/// [`BuildError::TableTypeMismatch`] and [`BuildError::CollectionAsCapability`]
/// carry a site.
///
/// `Display` writes the site as the beginning of a sentence that the error
/// message completes with the entity referred to, for example
/// ``stage `flm.refs.resolve` provides``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Site {
    /// A capability's `refines`.
    CapabilityRefines {
        /// The refining capability.
        capability: CapName,
    },
    /// A member added to a collection; the member is the entity referred
    /// to.
    CollectionMember {
        /// The collection.
        collection: CapName,
        /// The member.
        member: CapName,
    },
    /// A stage's `provides`.
    StageProvides {
        /// The stage.
        stage: StageName,
    },
    /// A stage's `writes`.
    StageWrites {
        /// The stage.
        stage: StageName,
    },
    /// The stage that ordering edges are placed on.
    OrderingEdges,
    /// A capability that an ordering edge on a stage refers to.
    OrderingEdge {
        /// The stage the edge applies to.
        stage: StageName,
    },
    /// A backend's native set.
    BackendNative {
        /// The backend.
        backend: BackendName,
    },
    /// The element a lowering rule lowers.
    RuleLowers {
        /// The rule.
        rule: RuleName,
    },
    /// A rule's `produces`.
    RuleProduces {
        /// The rule.
        rule: RuleName,
    },
    /// A rule's `requires`.
    RuleRequires {
        /// The rule.
        rule: RuleName,
    },
    /// A rule's `reads` or `reads_opt`.
    RuleReads {
        /// The rule.
        rule: RuleName,
    },
    /// A backend in a rule's default mark.
    RuleDefaultMark {
        /// The rule.
        rule: RuleName,
    },
    /// An output's capability.
    OutputCapability {
        /// The output.
        output: OutputName,
    },
    /// An output's table.
    OutputTable {
        /// The output.
        output: OutputName,
    },
    /// An output's [`made_from`](super::OutputDef::made_from).
    OutputMadeFrom {
        /// The output.
        output: OutputName,
    },
    /// A workflow's `extends`.
    WorkflowExtends {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// A workflow's outputs.
    WorkflowOutputs {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// A workflow's demands.
    WorkflowDemands {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// A workflow's provider preferences.
    WorkflowPrefers {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// The backend of one of a workflow's lowering selections.
    WorkflowLoweringSelection {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// A rule that one of a workflow's lowering selections prefers.
    WorkflowLoweringPreference {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// A backend or an element in a workflow's list of elements accepted as
    /// unrenderable.
    WorkflowUnsupported {
        /// The workflow.
        workflow: WorkflowName,
    },
    /// The element that an aspect is attached to.
    Attachment {
        /// The aspect's type name.
        aspect: &'static str,
    },
    /// The element that a native-set implication requires backends to write
    /// natively (`must_claim`).
    NativeImplication,
}

/// Where an argument stands in the declaration of a command: the place that
/// [`BuildError::UndeclaredArgumentRole`] refers to.
///
/// Its [`Display`](fmt::Display) form writes the argument as a message does,
/// for example `` argument #1 of `\k` `` or
/// `` the argument `name` of the field `\tag` of `\k` ``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentPlace {
    /// The command whose declaration holds the argument, as it is written:
    /// `\ref`, `\begin{figure}`, or the trigger of a special character
    /// sequence.
    pub command: String,
    /// The name of the [tack-on field](crate::lang::mint::TackOnField) whose
    /// own argument list holds the argument; `None` for an argument of the
    /// command itself.
    pub field: Option<String>,
    /// The argument's name, or `None` for an argument without one.
    pub argument: Option<String>,
    /// The argument's position, counting from zero, in the argument list that
    /// holds it.
    pub index: usize,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Site::CapabilityRefines { capability } => {
                write!(f, "capability `{capability}` refines")
            }
            Site::CollectionMember { collection, .. } => {
                write!(f, "a member added to collection `{collection}` is")
            }
            Site::StageProvides { stage } => write!(f, "stage `{stage}` provides"),
            Site::StageWrites { stage } => write!(f, "stage `{stage}` writes"),
            Site::OrderingEdges => f.write_str("an ordering declaration applies to"),
            Site::OrderingEdge { stage } => {
                write!(f, "an ordering declaration of stage `{stage}` refers to")
            }
            Site::BackendNative { backend } => {
                write!(f, "backend `{backend}` declares that it writes natively")
            }
            Site::RuleLowers { rule } => write!(f, "rule `{rule}` lowers"),
            Site::RuleProduces { rule } => write!(f, "rule `{rule}` produces"),
            Site::RuleRequires { rule } => write!(f, "rule `{rule}` requires"),
            Site::RuleReads { rule } => write!(f, "rule `{rule}` reads"),
            Site::RuleDefaultMark { rule } => write!(f, "rule `{rule}` is marked as the default on"),
            Site::OutputCapability { output } => write!(f, "output `{output}` demands"),
            Site::OutputTable { output } => write!(f, "output `{output}` keeps its artifacts in"),
            Site::OutputMadeFrom { output } => write!(f, "output `{output}` is made from"),
            Site::WorkflowExtends { workflow } => write!(f, "workflow `{workflow}` extends"),
            Site::WorkflowOutputs { workflow } => write!(f, "workflow `{workflow}` asks for"),
            Site::WorkflowDemands { workflow } => write!(f, "workflow `{workflow}` demands"),
            Site::WorkflowPrefers { workflow } => write!(f, "workflow `{workflow}` prefers"),
            Site::WorkflowLoweringSelection { workflow } => {
                write!(f, "workflow `{workflow}` selects lowering rules for")
            }
            Site::WorkflowLoweringPreference { workflow } => {
                write!(f, "workflow `{workflow}` prefers")
            }
            Site::WorkflowUnsupported { workflow } => {
                write!(f, "workflow `{workflow}`'s list of elements accepted as unrenderable refers to")
            }
            Site::Attachment { aspect } => write!(f, "aspect `{aspect}` is attached to"),
            Site::NativeImplication => {
                f.write_str("a rule that requires backends to write an element natively refers to")
            }
        }
    }
}

impl fmt::Display for ArgumentPlace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.argument {
            Some(name) => write!(f, "the argument `{name}`")?,
            None => write!(f, "argument #{}", self.index + 1)?,
        }
        if let Some(field) = &self.field {
            write!(f, " of the field `\\{field}`")?;
        }
        write!(f, " of `{}`", self.command)
    }
}

/// The action that a [`Finalizer`] attempted although only the first
/// registration round may perform it: the detail of a
/// [`BuildError::ContributionAfterAssembly`].
///
/// The service slots are assembled between the two registration rounds, and
/// there are exactly two rounds, so in the second round nothing may change a
/// service slot or register another finalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundTwoAction {
    /// Declare a service slot.
    DeclareSlot,
    /// Contribute to a service slot.
    Contribute,
    /// Require that a service slot be filled.
    RequireSlot,
    /// Register a finalizer.
    RegisterFinalizer,
}

impl fmt::Display for RoundTwoAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RoundTwoAction::DeclareSlot => "declare a service slot",
            RoundTwoAction::Contribute => "contribute to a service slot",
            RoundTwoAction::RequireSlot => "require a service slot",
            RoundTwoAction::RegisterFinalizer => "register a finalizer",
        })
    }
}

/// An error found while a registry is built, attributed to the feature at
/// fault wherever one feature is at fault.
///
/// Each variant holds the parts of its message as data, not as text; `Display`
/// writes the standard English message. A build collects every error of the
/// phase that fails and returns them together in a
/// [`RegistryBuildError`](super::RegistryBuildError). A feature's
/// [`register`](super::FlmFeature::register) method or a [`Finalizer`] may also
/// return a `BuildError` (usually the one a builder method returned) to refuse
/// the build.
#[derive(Debug, Clone)]
pub enum BuildError {
    /// A feature refused the build, or one of its declarations was refused
    /// with a message of its own: the feature's
    /// [`register`](super::FlmFeature::register) method or one of its
    /// [`Finalizer`]s returned an error, or a workflow the feature registered
    /// sets an option value of an option type that no feature declares, or of
    /// one that cannot be set on a render request.
    Feature {
        /// The feature.
        feature: FeatureName,
        /// What is wrong, as text for a person to read.
        detail: String,
    },
    /// Two features are registered under one name.
    DuplicateFeature {
        /// The name.
        feature: FeatureName,
    },
    /// Two declarations of one name in one namespace. For a peer
    /// declaration, `first` and `second` are the same feature, which declared
    /// the name twice with two different signatures.
    Duplicate {
        /// The namespace (`capability`, `table`, `stage`, `peer table`, and
        /// so on).
        kind: &'static str,
        /// The name.
        name: Name,
        /// The feature that made the first declaration.
        first: FeatureName,
        /// The feature that made the second declaration.
        second: FeatureName,
    },
    /// A table and a capability declared on its own have the same name.
    TableCapabilityClash {
        /// The name.
        name: CapName,
        /// The feature declaring the table.
        table_owner: FeatureName,
        /// The feature declaring the capability.
        capability_owner: FeatureName,
    },
    /// A declaration refers to an entity that no feature declares.
    Undeclared {
        /// The feature whose declaration holds the reference.
        feature: FeatureName,
        /// The namespace of the missing entity (`capability`, `table`,
        /// `stage`, `element`, and so on).
        kind: &'static str,
        /// Where in the declaration the reference is written.
        site: Site,
        /// The name referred to.
        name: Name,
        /// The names declared in that namespace, in name order.
        roster: Vec<Name>,
    },
    /// An argument has a role ([`ArgDef::role`](crate::lang::mint::ArgDef::role))
    /// that no feature declares, and that the feature whose declaration holds
    /// the argument did not declare as a peer
    /// ([`FeatureBuilder::peer_argument_role`]).
    UndeclaredArgumentRole {
        /// The feature whose declaration holds the argument.
        feature: FeatureName,
        /// Where the argument stands in the declaration. The place is boxed,
        /// so that a [`BuildError`] stays small.
        place: Box<ArgumentPlace>,
        /// The role.
        role: Name,
        /// The argument role names that features declare, in name order.
        roster: Vec<Name>,
    },
    /// A declaration refers to a table with a value type other than the
    /// table's declared value type.
    TableTypeMismatch {
        /// The feature whose declaration holds the reference.
        feature: FeatureName,
        /// Where in the declaration the reference is written.
        site: Site,
        /// The table.
        table: CapName,
        /// The declared value type.
        declared: &'static str,
        /// The type the reference expects.
        referenced: &'static str,
    },
    /// A chain of capability refinements leads back to a capability already
    /// in it.
    RefinesCycle {
        /// The capabilities of the cycle, starting at the first in name
        /// order.
        cycle: Vec<CapName>,
    },
    /// Two refinements of one capability are marked as its default.
    TwoDefaults {
        /// The capability they refine.
        parent: CapName,
        /// The first refinement marked default.
        first: CapName,
        /// The second refinement marked default.
        second: CapName,
    },
    /// A capability is marked as a default refinement but refines nothing.
    DefaultWithoutParent {
        /// The capability.
        name: CapName,
    },
    /// A stage whose effect is `Check` declares a write of a table.
    CheckWrites {
        /// The stage.
        stage: StageName,
        /// The table.
        table: CapName,
    },
    /// A stage declares a write of an external table, which only the caller
    /// installs.
    ExternalTableWritten {
        /// The stage.
        stage: StageName,
        /// The table.
        table: CapName,
    },
    /// A stage provides a collection: a collection can be demanded but never
    /// provided.
    CollectionAsCapability {
        /// The stage.
        stage: StageName,
        /// Where in the stage's declaration the collection is written.
        site: Site,
        /// The collection.
        collection: CapName,
    },
    /// Two lowering rules are marked as the default for one element on one
    /// backend, or as the general default for one element.
    TwoDefaultRules {
        /// The element.
        element: ElementName,
        /// The backend, or `None` for the general default.
        backend: Option<BackendName>,
        /// The first rule marked default.
        first: RuleName,
        /// The second rule marked default.
        second: RuleName,
    },
    /// A chain of workflows, each extending the next, leads back to a
    /// workflow already in it.
    ExtendsCycle {
        /// The workflows of the cycle, starting at the first in name order.
        cycle: Vec<WorkflowName>,
    },
    /// An output's capability is neither its table's capability nor a
    /// refinement of it.
    OutputCapabilityMismatch {
        /// The output.
        output: OutputName,
        /// The capability.
        capability: CapName,
        /// The table.
        table: CapName,
    },
    /// An output's table is not an artifact set.
    OutputTableNotArtifactSet {
        /// The output.
        output: OutputName,
        /// The table.
        table: CapName,
        /// The table's value type.
        ty: &'static str,
    },
    /// A part of a style or script definition that takes its text from a
    /// table ([`DefContent::Table`](super::DefContent::Table)) names a table
    /// that no feature declares, or one that does not hold
    /// [`DefinitionText`](super::DefinitionText).
    DefinitionPartTable {
        /// `style` or `script`.
        kind: &'static str,
        /// The definition's identifier.
        definition: Name,
        /// The table the part names.
        table: CapName,
        /// The value type the table is declared with; `None` when no feature
        /// declares it.
        found: Option<&'static str>,
    },
    /// An aspect is attached to an element that already carries it.
    AspectCollision {
        /// The attaching feature.
        feature: FeatureName,
        /// The element.
        element: ElementName,
        /// The aspect's type name.
        aspect: &'static str,
    },
    /// A backend's native set lacks an element that a [native-set
    /// implication](NativeImplication) requires it to hold.
    NativeSetIncomplete {
        /// The backend.
        backend: BackendName,
        /// The element of the backend's native set that made the implication
        /// apply, or `None` when the implication applies to every backend.
        because: Option<ElementName>,
        /// The element the backend's native set must also hold.
        must_claim: ElementName,
    },
    /// A [peer declaration](super::PeerName) of a presentation element
    /// resolved to an element whose payload type is not the signature of the
    /// peer declaration.
    PeerElementMismatch {
        /// The feature that made the peer declaration.
        feature: FeatureName,
        /// The element.
        element: ElementName,
        /// The payload type of the element's own declaration, or `None` when
        /// the name is that of a source element (an element that can appear
        /// in a parsed document), which carries no payload.
        declared: Option<&'static str>,
        /// The payload type given in the peer declaration.
        expected: &'static str,
    },
    /// A [peer declaration](super::PeerName) of a table resolved to a table
    /// whose value type is not the signature of the peer declaration.
    PeerTableMismatch {
        /// The feature that made the peer declaration.
        feature: FeatureName,
        /// The table.
        table: CapName,
        /// The value type of the table's own declaration.
        declared: &'static str,
        /// The value type given in the peer declaration.
        expected: &'static str,
    },
    /// A problem with a service slot, found when the slots are assembled.
    Service(ServiceError),
    /// A [`Finalizer`] attempted an action that only the first registration
    /// round may perform.
    ContributionAfterAssembly {
        /// The feature.
        feature: FeatureName,
        /// The action attempted.
        action: RoundTwoAction,
    },
    /// The build of the FLM language from the element declarations found a
    /// problem, such as two features defining one command.
    Lang(LangBuildError),
    /// A [`LateInit`] hook failed.
    LateInit {
        /// The feature that registered the hook.
        feature: FeatureName,
        /// What failed, as the hook reported it.
        detail: String,
    },
    /// The chains of the feature order, reduced to the features of the
    /// registry, order a feature before itself, so the features have no final
    /// order ([`super::order`]). The build reports one cycle.
    FeatureOrderCycle {
        /// The features of the cycle, each placed before the next and the
        /// last before the first, starting from the feature of the cycle that
        /// comes first in the order the features were added.
        cycle: Vec<FeatureName>,
        /// Every chain that orders two consecutive features of the cycle, as
        /// written and with its origin, in the order of the cycle.
        chains: Vec<FeatureChain>,
    },
}

fn join<'a>(names: impl IntoIterator<Item = &'a Name>) -> String {
    names.into_iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Feature { feature, detail } => {
                write!(f, "feature `{feature}`: {detail}")
            }
            BuildError::DuplicateFeature { feature } => {
                write!(f, "two features are registered under the name `{feature}`")
            }
            BuildError::Duplicate { kind, name, first, second } => {
                write!(f, "{kind} `{name}` is declared twice: first by feature `{first}`, then by feature `{second}`")
            }
            BuildError::TableCapabilityClash { name, table_owner, capability_owner } => write!(
                f,
                "`{name}` is declared both as a table, by feature `{table_owner}`, and as a \
                 capability, by feature `{capability_owner}`; a table's name is already a \
                 capability, so no capability may be declared under it"
            ),
            BuildError::Undeclared { feature, kind, site, name, roster } => write!(
                f,
                "feature `{feature}`: {site} the {kind} `{name}`, but no feature declares this \
                 {kind}; declared {kind} names: [{}]",
                join(roster)
            ),
            BuildError::UndeclaredArgumentRole { feature, place, role, roster } => write!(
                f,
                "feature `{feature}`: {place} has the argument role `{role}`, which no registered \
                 feature declares; a feature that sets another feature's argument role must \
                 declare it with `FeatureBuilder::peer_argument_role`; declared argument role \
                 names: [{}]",
                join(roster)
            ),
            BuildError::TableTypeMismatch { feature, site, table, declared, referenced } => write!(
                f,
                "feature `{feature}`: {site} the table `{table}` as holding `{referenced}`, but \
                 the table is declared as holding `{declared}`"
            ),
            BuildError::RefinesCycle { cycle } => {
                let mut names: Vec<&str> = cycle.iter().map(|n| n.as_str()).collect();
                names.push(cycle[0].as_str());
                write!(f, "capabilities refine each other in a cycle: {}", names.join(" -> "))
            }
            BuildError::TwoDefaults { parent, first, second } => write!(
                f,
                "capability `{parent}` may have only one default refinement, but `{first}` and \
                 `{second}` are both marked default"
            ),
            BuildError::DefaultWithoutParent { name } => {
                write!(f, "capability `{name}` is marked as a default refinement but refines no capability")
            }
            BuildError::CheckWrites { stage, table } => {
                write!(f, "check stage `{stage}` declares a write of table `{table}`; a check stage writes no table")
            }
            BuildError::ExternalTableWritten { stage, table } => {
                write!(
                    f,
                    "stage `{stage}` declares a write of the external table `{table}`; only the \
                     caller installs an external table, and no stage may write it"
                )
            }
            BuildError::CollectionAsCapability { stage, site, collection } => write!(
                f,
                "{site} the collection `{collection}`; a collection can be demanded but never \
                 provided, so stage `{stage}` may not provide it"
            ),
            BuildError::TwoDefaultRules { element, backend, first, second } => match backend {
                Some(backend) => write!(
                    f,
                    "element `{element}` may have only one default lowering rule on backend \
                     `{backend}`, but rules `{first}` and `{second}` are both marked default there"
                ),
                None => write!(
                    f,
                    "element `{element}` may have only one general default lowering rule, but \
                     rules `{first}` and `{second}` are both marked as the general default"
                ),
            },
            BuildError::ExtendsCycle { cycle } => {
                let mut names: Vec<&str> = cycle.iter().map(|n| n.as_str()).collect();
                names.push(cycle[0].as_str());
                write!(f, "workflows extend each other in a cycle: {}", names.join(" -> "))
            }
            BuildError::OutputCapabilityMismatch { output, capability, table } => write!(
                f,
                "output `{output}` demands the capability `{capability}`, which is neither the \
                 capability of its table `{table}` nor a refinement of it"
            ),
            BuildError::OutputTableNotArtifactSet { output, table, ty } => write!(
                f,
                "the table `{table}` of output `{output}` holds `{ty}`; an output's table must \
                 hold an artifact set"
            ),
            BuildError::DefinitionPartTable { kind, definition, table, found: None } => write!(
                f,
                "the table `{table}` that the {kind} definition `{definition}` takes a part from \
                 must be declared by a feature of the registry"
            ),
            BuildError::DefinitionPartTable { kind, definition, table, found: Some(found) } => write!(
                f,
                "the table `{table}` that the {kind} definition `{definition}` takes a part from \
                 must hold `DefinitionText`, not `{found}`"
            ),
            BuildError::AspectCollision { feature, element, aspect } => write!(
                f,
                "feature `{feature}` attaches aspect `{aspect}` to element `{element}`, which \
                 already carries that aspect; an element carries each aspect at most once"
            ),
            BuildError::NativeSetIncomplete { backend, because, must_claim } => match because {
                Some(because) => write!(
                    f,
                    "backend `{backend}` must write the element `{must_claim}` natively, \
                     because it writes the element `{because}` natively"
                ),
                None => write!(
                    f,
                    "backend `{backend}` must write the element `{must_claim}` natively, as \
                     every backend must"
                ),
            },
            BuildError::PeerElementMismatch { feature, element, declared, expected } => {
                match declared {
                    Some(declared) => write!(
                        f,
                        "feature `{feature}` refers to the presentation element `{element}` as \
                         carrying payload `{expected}`, but the element is declared with payload \
                         `{declared}`"
                    ),
                    None => write!(
                        f,
                        "feature `{feature}` refers to the presentation element `{element}` as \
                         carrying payload `{expected}`, but `{element}` is declared as an element \
                         that a parsed document can contain, which carries no payload"
                    ),
                }
            }
            BuildError::PeerTableMismatch { feature, table, declared, expected } => write!(
                f,
                "feature `{feature}` refers to the table `{table}` as holding `{expected}`, but \
                 the table is declared as holding `{declared}`"
            ),
            BuildError::Service(error) => fmt::Display::fmt(error, f),
            BuildError::ContributionAfterAssembly { feature, action } => write!(
                f,
                "a finalizer of feature `{feature}` may not {action}; only the feature's \
                 `register` method may"
            ),
            BuildError::Lang(error) => fmt::Display::fmt(error, f),
            BuildError::LateInit { feature, detail } => {
                write!(f, "the late-initialization hook of feature `{feature}` failed: {detail}")
            }
            BuildError::FeatureOrderCycle { cycle, chains } => {
                let written: Vec<String> =
                    chains.iter().map(|chain| format!("`{chain}` ({})", chain.origin())).collect();
                let (subject, verb) = match written.as_slice() {
                    [one] => (format!("the chain {one}"), "orders"),
                    [first, last] => (format!("the chains {first} and {last}"), "order"),
                    [head @ .., last] => (format!("the chains {}, and {last}", head.join(", ")), "order"),
                    [] => (String::from("the chains"), "order"),
                };
                let mut names: Vec<&str> = cycle.iter().map(|name| name.as_str()).collect();
                if let Some(first) = names.first().copied() {
                    names.push(first);
                }
                write!(
                    f,
                    "the feature order must not contain a cycle, but {subject} {verb} the features \
                     in the cycle `{}`",
                    names.join(" < ")
                )
            }
        }
    }
}

impl std::error::Error for BuildError {}

impl From<ServiceError> for BuildError {
    fn from(error: ServiceError) -> BuildError {
        BuildError::Service(error)
    }
}

/// One feature's request to attach an aspect implementation to every element
/// value whose element type has a given name, including the values that other
/// features declare.
///
/// [`FeatureBuilder::attach`] records it; the build's validate phase merges it
/// into the element values.
pub struct Attachment {
    /// The feature that attaches the aspect.
    pub owner: FeatureName,
    /// The element type name; every element value of that type gets the
    /// aspect.
    pub element: ElementName,
    /// The type identifier of the aspect's trait-object type.
    pub aspect_id: TypeId,
    /// The name of the aspect's trait-object type.
    pub aspect_name: &'static str,
    /// The implementation, in the form the element's aspect map stores
    /// (`AspectMap::insert_erased`).
    pub imp: Arc<dyn Any + Send + Sync>,
}

/// A table as the collected declarations ([`Roster`]) hold it: its
/// declaration, whether it is external and, for the table of trees that a
/// rewrite stage publishes, the capability that table's capability refines and
/// whether it is the default refinement.
pub struct TableRecord {
    /// The table's declaration.
    pub def: TableDef,
    /// Whether the table is external: no stage writes it, and the caller
    /// installs its value when a run is constructed.
    pub external: bool,
    /// The capability that the table's capability refines; set only for the
    /// table of trees a rewrite stage publishes.
    pub refines: Option<CapName>,
    /// Whether the table's capability is the default refinement of
    /// `refines`.
    pub default: bool,
}

/// A registered stage, in either of the two forms a stage takes.
///
/// Both forms are scheduled together and share one numbering, but they do
/// different work: a rewrite stage changes the trees before they are frozen,
/// and an ordinary stage computes data beside the frozen trees, checks
/// something, or writes output.
pub enum StageKind {
    /// An ordinary stage, which runs after the trees are frozen: it computes
    /// data, checks something, or performs input and output. See [`Stage`].
    Pipeline(Arc<dyn Stage>),
    /// A rewrite stage, which runs before the trees are frozen. See
    /// [`RewriteStage`].
    Rewrite(Arc<dyn RewriteStage>),
}

/// A stage as the collected declarations ([`Roster`]) hold it: the stage, the
/// feature that registered it, and its declaration, read from the stage once,
/// at registration, so that the rest of the build and every plan use the same
/// declaration.
pub struct StageRecord {
    /// The feature that registered the stage.
    pub owner: FeatureName,
    /// The stage, in its form.
    pub kind: StageKind,
    /// The stage's declaration. For a rewrite stage, the builder makes it
    /// from the rewrite's own declaration, so both forms are scheduled the
    /// same way.
    pub decl: StageDecl,
    /// A rewrite stage's own declaration; `None` for an ordinary stage.
    pub rewrite: Option<RewriteDecl>,
}

/// A backend as the collected declarations ([`Roster`]) hold it. The builder
/// records the backend's three stages as ordinary stages of the same feature;
/// the three indices here give their positions in the list of stages.
pub struct BackendRecord {
    /// The feature that registered the backend.
    pub owner: FeatureName,
    /// The backend's name.
    pub name: BackendName,
    /// The native set: the elements the emit stage writes directly.
    pub native: Vec<ElementName>,
    /// Documentation prose.
    pub docs: String,
    /// The position of the lowering stage in [`Roster::stages`].
    pub lowering: usize,
    /// The position of the emit stage in [`Roster::stages`].
    pub emit: usize,
    /// The position of the final stage in [`Roster::stages`].
    pub final_stage: usize,
}

/// A presentation element as the collected declarations ([`Roster`]) hold it:
/// the element value the builder created for it, and the type of payload its
/// nodes carry.
pub struct PresentationRecord {
    /// The feature that registered the element.
    pub owner: FeatureName,
    /// The element value the builder created.
    pub element: Arc<ElementSpec>,
    /// The type of the payload the element's nodes carry.
    pub payload: TypeKey,
    /// Documentation prose.
    pub docs: String,
}

/// A peer declaration of a backend as the collected declarations ([`Roster`])
/// hold it: the name of a backend that the declaring feature expects another
/// feature to register. See [`PeerName`](super::PeerName).
pub struct PeerBackendRecord {
    /// The declaring feature.
    pub owner: FeatureName,
    /// The backend's name.
    pub name: BackendName,
    /// Documentation prose: why the feature refers to the backend.
    pub docs: String,
}

/// A peer declaration of a presentation element as the collected declarations
/// ([`Roster`]) hold it: the name of a presentation element that the declaring
/// feature expects another feature to register, with the payload type its
/// nodes are expected to carry as the declaration's signature. See
/// [`PeerName`](super::PeerName).
pub struct PeerElementRecord {
    /// The declaring feature.
    pub owner: FeatureName,
    /// The element's name.
    pub name: ElementName,
    /// The payload type of the nodes that the declaring feature's rules build.
    pub payload: TypeKey,
    /// Documentation prose: why the feature refers to the element.
    pub docs: String,
}

/// A peer declaration of a table as the collected declarations ([`Roster`])
/// hold it: the name of a table that the declaring feature expects another
/// feature to declare, with the value type the table is expected to hold as
/// the declaration's signature. See [`PeerName`](super::PeerName).
pub struct PeerTableRecord {
    /// The declaring feature.
    pub owner: FeatureName,
    /// The table's name.
    pub name: CapName,
    /// The value type as which the declaring feature's rules read the table.
    pub value: TableType,
    /// Documentation prose: why the feature refers to the table.
    pub docs: String,
}

/// A peer declaration of a scoped package as the collected declarations
/// ([`Roster`]) hold it: the name of a [scoped package](ScopedPackageDef) that
/// the declaring feature expects another feature to declare. See
/// [`PeerName`](super::PeerName).
pub struct PeerScopedPackageRecord {
    /// The declaring feature.
    pub owner: FeatureName,
    /// The package's name.
    pub name: Name,
    /// Documentation prose: why the feature's environment bodies or arguments
    /// refer to the package.
    pub docs: String,
}

/// A peer declaration of an argument role as the collected declarations
/// ([`Roster`]) hold it: the name of an [argument role](ArgumentRoleDef) that
/// the declaring feature sets on arguments of its own declarations and
/// expects another feature to declare. See [`PeerName`](super::PeerName).
pub struct PeerArgumentRoleRecord {
    /// The declaring feature.
    pub owner: FeatureName,
    /// The role's name.
    pub name: Name,
}

/// A peer declaration of an element value as the collected declarations
/// ([`Roster`]) hold it: one more value of an element type that the declaring
/// feature expects another feature to declare. The element declaration itself
/// is held among [`Roster::elements`], at `position`, until the build resolves
/// it. See [`PeerName`](super::PeerName).
pub struct PeerElementValueRecord {
    /// The declaring feature.
    pub owner: FeatureName,
    /// The element type's name, which is the name of the declaration's
    /// element value.
    pub element: ElementName,
    /// The callable names (commands) the declaration defines.
    pub names: Vec<Name>,
    /// The position of the element declaration in [`Roster::elements`], valid
    /// until the build resolves the declaration.
    pub position: usize,
    /// Whether the build found a feature that declares the element type;
    /// `false` until the build resolves the declaration.
    pub resolved: bool,
}

/// Bytes that a feature gave as an input of the registry digest, with
/// [`FeatureBuilder::digest_input`].
pub struct DigestInput {
    /// The feature that gave the bytes.
    pub owner: FeatureName,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// The collected declarations of one registry build: every declaration every
/// feature made, in registration order, each attributed to the feature that
/// made it.
///
/// [`FeatureBuilder`] fills it.
/// [`RegistryBuilder::build`](super::RegistryBuilder::build) checks it and
/// numbers its entries into the frozen registry, after which it no longer
/// exists; the registry keeps its element declarations, scoped packages,
/// fallbacks and overrides
/// ([`FlmRegistry::declarations`](super::FlmRegistry::declarations)). Only
/// the registry build creates and reads a `Roster`; the type is public so
/// that its contents are documented.
#[derive(Default)]
pub struct Roster {
    /// The features, in registration order.
    pub features: Vec<FeatureInfo>,
    /// The element declarations, including the peer element values until the
    /// build drops the unresolved ones.
    pub elements: Vec<ElementDef>,
    /// The aspect attachments.
    pub attachments: Vec<Attachment>,
    /// The fallback declarations.
    pub fallbacks: Vec<FallbackDef>,
    /// The scoped packages.
    pub scoped_packages: Vec<ScopedPackageDef>,
    /// The declarations that a feature's definition of a callable name
    /// replaces another feature's ([`FeatureBuilder::override_callable`]).
    pub overrides: Vec<OverrideDef>,
    /// The capabilities declared on their own, each with the declaring
    /// feature.
    pub capabilities: Vec<(FeatureName, CapabilityDef)>,
    /// The collections, each with the declaring feature.
    pub collections: Vec<(FeatureName, CollectionDef)>,
    /// The argument roles, each with the declaring feature.
    pub argument_roles: Vec<(FeatureName, ArgumentRoleDef)>,
    /// The collection members, as (contributing feature, collection,
    /// member).
    pub members: Vec<(FeatureName, CapName, CapName)>,
    /// The tables.
    pub tables: Vec<TableRecord>,
    /// The stages of both forms.
    pub stages: Vec<StageRecord>,
    /// The ordering edges, each with the feature that placed them.
    pub orderings: Vec<(FeatureName, OrderingDef)>,
    /// The lowering rules, each with the registering feature.
    pub rules: Vec<(FeatureName, LoweringRule)>,
    /// The presentation elements.
    pub presentation: Vec<PresentationRecord>,
    /// The backends.
    pub backends: Vec<BackendRecord>,
    /// The outputs, each with the registering feature.
    pub outputs: Vec<(FeatureName, OutputDef)>,
    /// The workflows, each with the registering feature.
    pub workflows: Vec<(FeatureName, WorkflowDef)>,
    /// The fields registered for workflows by name, each with the
    /// registering feature, in registration order; see
    /// [`FeatureBuilder::workflow_fields`].
    pub workflow_fields: Vec<(FeatureName, WorkflowDef)>,
    /// The option type declarations.
    pub options: Vec<OptionDecl>,
    /// The service slot declarations.
    pub slots: Vec<SlotDecl>,
    /// The contributions to service slots.
    pub contributions: Vec<Contribution>,
    /// The requirements that a service slot be filled.
    pub requirements: Vec<ServiceRequirement>,
    /// The finalizers, each with the registering feature.
    pub finalizers: Vec<(FeatureName, Arc<dyn Finalizer>)>,
    /// The late-initialization hooks, each with the registering feature.
    pub late_inits: Vec<(FeatureName, Arc<dyn LateInit>)>,
    /// The style definitions, each with the registering feature.
    pub styles: Vec<(FeatureName, StyleDef)>,
    /// The script definitions, each with the registering feature.
    pub scripts: Vec<(FeatureName, ScriptDef)>,
    /// The native-set implications, each with the declaring feature.
    pub native_implications: Vec<(FeatureName, NativeImplication)>,
    /// The peer declarations of backends.
    pub peer_backends: Vec<PeerBackendRecord>,
    /// The peer declarations of presentation elements.
    pub peer_elements: Vec<PeerElementRecord>,
    /// The peer declarations of tables.
    pub peer_tables: Vec<PeerTableRecord>,
    /// The peer declarations of scoped packages.
    pub peer_scoped_packages: Vec<PeerScopedPackageRecord>,
    /// The peer declarations of element values; each element declaration is
    /// also held in [`elements`](Roster::elements) until the build drops the
    /// unresolved ones.
    pub peer_element_values: Vec<PeerElementValueRecord>,
    /// The peer declarations of argument roles.
    pub peer_argument_roles: Vec<PeerArgumentRoleRecord>,
    /// The bytes features gave as inputs of the registry digest.
    pub digest_inputs: Vec<DigestInput>,
    /// The errors that features' `register` methods and finalizers returned
    /// during the current registration round; the build takes them out at
    /// the end of each round.
    pub errors: Vec<BuildError>,
}

/// Which of the build's two registration rounds a feature builder serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Round {
    One,
    Two,
}

// Recording everything and checking it afterwards is what makes registration
// order irrelevant: see the design rationale at
// [§flm-dd-dr:feat-emergent-deps].
/// The feature builder: the value through which a feature declares what it
/// contributes to a registry. Each method records one declaration in the
/// build's [`Roster`] and attributes it to the feature this builder was created
/// for.
///
/// The registry build creates one builder per feature and round. It hands one
/// to the feature's [`FlmFeature::register`](super::FlmFeature::register)
/// method, in the first registration round, where every method may be called;
/// and one to each of the feature's [`Finalizer`]s, in the second round, where
/// the methods that declare, fill or require service slots and the method that
/// registers a finalizer return [`BuildError::ContributionAfterAssembly`]. Only
/// in the second round does [`services`](FeatureBuilder::services) return the
/// assembled services.
///
/// The methods are grouped by what they declare: the language
/// ([`element`](FeatureBuilder::element),
/// [`attach`](FeatureBuilder::attach), [`fallback`](FeatureBuilder::fallback),
/// [`scoped_package`](FeatureBuilder::scoped_package),
/// [`override_callable`](FeatureBuilder::override_callable),
/// [`argument_role`](FeatureBuilder::argument_role)); capabilities and
/// tables ([`capability`](FeatureBuilder::capability),
/// [`collection`](FeatureBuilder::collection),
/// [`collection_member`](FeatureBuilder::collection_member),
/// [`table`](FeatureBuilder::table),
/// [`external_table`](FeatureBuilder::external_table)); stages
/// ([`stage`](FeatureBuilder::stage),
/// [`rewrite_stage`](FeatureBuilder::rewrite_stage),
/// [`ordering`](FeatureBuilder::ordering)); lowering and output
/// ([`lowering_rule`](FeatureBuilder::lowering_rule),
/// [`presentation_element`](FeatureBuilder::presentation_element),
/// [`backend`](FeatureBuilder::backend),
/// [`native_implication`](FeatureBuilder::native_implication),
/// [`output`](FeatureBuilder::output), [`workflow`](FeatureBuilder::workflow),
/// [`workflow_fields`](FeatureBuilder::workflow_fields)); peer declarations of entities other
/// features own ([`peer_backend`](FeatureBuilder::peer_backend),
/// [`peer_element`](FeatureBuilder::peer_element),
/// [`peer_table`](FeatureBuilder::peer_table),
/// [`peer_slot`](FeatureBuilder::peer_slot),
/// [`peer_scoped_package`](FeatureBuilder::peer_scoped_package),
/// [`peer_element_value`](FeatureBuilder::peer_element_value),
/// [`peer_argument_role`](FeatureBuilder::peer_argument_role)); options and
/// services ([`options`](FeatureBuilder::options),
/// [`service_slot`](FeatureBuilder::service_slot),
/// [`contribute`](FeatureBuilder::contribute),
/// [`contribute_first`](FeatureBuilder::contribute_first),
/// [`require_service`](FeatureBuilder::require_service)); and build hooks and
/// records ([`finalizer`](FeatureBuilder::finalizer),
/// [`late_init`](FeatureBuilder::late_init),
/// [`style_def`](FeatureBuilder::style_def),
/// [`script_def`](FeatureBuilder::script_def),
/// [`digest_input`](FeatureBuilder::digest_input)). Several of these methods
/// have a variant that takes the target by name instead of by type, or that
/// adds documentation prose: [`table_named`](FeatureBuilder::table_named),
/// [`external_table_named`](FeatureBuilder::external_table_named),
/// [`options_with_docs`](FeatureBuilder::options_with_docs),
/// [`contribute_to`](FeatureBuilder::contribute_to),
/// [`contribute_first_to`](FeatureBuilder::contribute_first_to) and
/// [`require_service_named`](FeatureBuilder::require_service_named). Two
/// methods read the builder instead of recording a declaration:
/// [`feature`](FeatureBuilder::feature) and
/// [`services`](FeatureBuilder::services).
///
/// Every method that records a declaration returns a `Result`, so that a feature's `register` method can
/// end each call with `?`, and so that a method can start reporting a problem
/// without a change to its callers. Most methods always return `Ok`; the
/// others say under `# Errors` when they fail. Problems with the declarations
/// themselves are reported when the registry is built.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::{BuildError, CapabilityDef, FeatureBuilder};
///
/// fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
///     b.capability(CapabilityDef::new("counting.done").docs("Counting has run."))?;
///     assert_eq!(b.feature(), "counting");
///     Ok(())
/// }
/// ```
pub struct FeatureBuilder<'a> {
    roster: &'a mut Roster,
    feature: FeatureName,
    round: Round,
    services: Option<&'a Services>,
}

impl<'a> FeatureBuilder<'a> {
    /// A builder for the first registration round of `feature`, with no
    /// services to read.
    pub(crate) fn round_one(roster: &'a mut Roster, feature: FeatureName) -> FeatureBuilder<'a> {
        FeatureBuilder { roster, feature, round: Round::One, services: None }
    }

    /// A builder for the second registration round of `feature`, able to read
    /// the assembled `services`.
    pub(crate) fn round_two(
        roster: &'a mut Roster,
        feature: FeatureName,
        services: &'a Services,
    ) -> FeatureBuilder<'a> {
        FeatureBuilder { roster, feature, round: Round::Two, services: Some(services) }
    }

    /// The name of the feature this builder records for: every declaration made
    /// through this builder is attributed to it.
    ///
    /// A feature reads it to compose the names of its own entities, so that the
    /// name the feature was registered under is the one that appears in the names
    /// of its entities (`flm.<feature>.<...>`).
    pub fn feature(&self) -> &FeatureName {
        &self.feature
    }

    /// The assembled services, or `None` in the first registration round.
    ///
    /// The build assembles the service slots between the two rounds, so only a
    /// [`Finalizer`] can read them. A feature that needs a service to decide what
    /// to declare registers a finalizer and makes those declarations there.
    pub fn services(&self) -> Option<&'a Services> {
        self.services
    }

    fn round_one_only(&self, action: RoundTwoAction) -> Result<(), BuildError> {
        if self.round == Round::Two {
            return Err(BuildError::ContributionAfterAssembly {
                feature: self.feature.clone(),
                action,
            });
        }
        Ok(())
    }

    // --- the language ---------------------------------------------------------------

    /// Declares an element: an element value, the callable names (commands) that
    /// produce it, and the syntax of those commands. See [`ElementDef`] for how a
    /// declaration is written.
    ///
    /// The declaration is attributed to this feature, regardless of the value
    /// of `def.owner`.
    ///
    /// # Errors
    ///
    /// Never fails. A name that clashes with another feature's, or another problem
    /// with the declaration, is reported when the registry is built.
    pub fn element(&mut self, mut def: ElementDef) -> Result<(), BuildError> {
        def.owner = self.feature.clone();
        self.roster.elements.push(def);
        Ok(())
    }

    /// Declares one more value of an element type that another feature owns,
    /// with the commands that produce it: a [peer declaration](super::PeerName) of
    /// the element type, which carries the element declaration that
    /// [`element`](Self::element) takes.
    ///
    /// The declaration lets this feature add a value of the type without
    /// depending on the feature that owns the type. That feature's lowering rules,
    /// stages and aspects serve every value of the type, so the value is
    /// meaningful only when that feature is registered. A feature **declares the
    /// element type** when any of its declarations other than a peer element
    /// value creates an element of that name: an [`element`](Self::element)
    /// declaration (a value of its own, or the type alone with no callable name,
    /// [`SyntaxDef::Synthesized`](crate::lang::mint::SyntaxDef::Synthesized)), an
    /// element of a [scoped package](Self::scoped_package), a
    /// [fallback](Self::fallback), a [presentation
    /// element](Self::presentation_element), or a field that an argument attaches;
    /// the language's structural elements count too.
    ///
    /// When the registry is built, the declaration is resolved against the element
    /// types that the other declarations of the registry create.
    ///
    /// - If a feature declares the element type, this declaration is kept,
    ///   attributed to this feature, exactly as [`element`](Self::element) would
    ///   keep it.
    /// - If no feature declares it, the build still succeeds, the declaration is
    ///   dropped (its commands are then unknown commands, like any command no
    ///   feature defines), and one Note ([`UnresolvedPeer`](super::UnresolvedPeer))
    ///   joins the registry's [build
    ///   diagnostics](super::FlmRegistry::build_diagnostics), one per feature and
    ///   element type, however many values this feature declared for it. Without
    ///   this rule, a value that no lowering rule lowers would make every rendering
    ///   plan of the registry fail.
    ///
    /// Every such declaration, resolved or not, is listed on the finished
    /// registry
    /// ([`FlmRegistry::peer_element_values`](super::FlmRegistry::peer_element_values)).
    /// See [`PeerName`](super::PeerName) for the other kinds of peer declaration.
    ///
    /// # Errors
    ///
    /// Never fails. An unresolved declaration is reported as a Note when the
    /// registry is built, and a kept one is checked then like any element
    /// declaration.
    pub fn peer_element_value(&mut self, mut def: ElementDef) -> Result<(), BuildError> {
        def.owner = self.feature.clone();
        self.roster.peer_element_values.push(PeerElementValueRecord {
            owner: self.feature.clone(),
            element: def.element.name.clone(),
            names: def.names.clone(),
            position: self.roster.elements.len(),
            resolved: false,
        });
        self.roster.elements.push(def);
        Ok(())
    }

    /// Attaches the aspect implementation `imp` to every element value whose
    /// element type is called `element`, including other features' elements and
    /// the language's structural elements.
    ///
    /// An aspect is a piece of behavior attached to an element when the feature
    /// registers it and read back through a trait object (see [elements and
    /// aspects](crate::guide::concepts_overview#elements-and-aspects)). This method
    /// gives behavior to elements a feature does not own: for example, a
    /// cross-referencing feature makes headings referenceable without the headings
    /// feature depending on it. Give the aspect as its **trait-object type**, as in
    /// `b.attach::<dyn Referenceable>("flm.heading", imp)`. A concrete type fails
    /// to compile, because an aspect is looked up by the trait it is requested
    /// as.
    ///
    /// # Errors
    ///
    /// Never fails. When the registry is built, an element type that no feature
    /// declares is reported as [`BuildError::Undeclared`], and an aspect that an
    /// element already carries as [`BuildError::AspectCollision`].
    pub fn attach<A: ?Sized + Any + Send + Sync>(
        &mut self,
        element: impl Into<ElementName>,
        imp: Arc<A>,
    ) -> Result<(), BuildError> {
        const {
            assert!(
                size_of::<&A>() == 2 * size_of::<*const ()>(),
                "the aspect must be given as its trait-object type, as in \
                 .attach::<dyn MyAspect>(…)"
            );
        }
        self.roster.attachments.push(Attachment {
            owner: self.feature.clone(),
            element: element.into(),
            aspect_id: TypeId::of::<A>(),
            aspect_name: std::any::type_name::<A>(),
            imp: Arc::new(imp),
        });
        Ok(())
    }

    /// Declares a fallback: the meaning of a callable name in a parsing mode where
    /// nothing else defines it. See [`FallbackDef`]. The declaration is attributed
    /// to this feature.
    ///
    /// # Errors
    ///
    /// Never fails; problems are reported when the registry is built.
    pub fn fallback(&mut self, mut def: FallbackDef) -> Result<(), BuildError> {
        def.owner = self.feature.clone();
        self.roster.fallbacks.push(def);
        Ok(())
    }

    /// Declares a scoped package. A scoped package is a set of element
    /// declarations that can be written only inside the environment bodies and
    /// arguments whose declarations refer to the package by name
    /// ([`BodyDef::scoped`](crate::lang::mint::BodyDef::scoped),
    /// [`ArgDef::scoped`](crate::lang::mint::ArgDef::scoped)), for instance
    /// `\item` inside a list's body. See [`ScopedPackageDef`].
    ///
    /// The package and each of its elements are attributed to this feature.
    ///
    /// # Errors
    ///
    /// Never fails; problems are reported when the registry is built.
    pub fn scoped_package(&mut self, mut def: ScopedPackageDef) -> Result<(), BuildError> {
        def.owner = self.feature.clone();
        def.docs = crate::kept_docs(def.docs);
        for element in &mut def.elements {
            element.owner = self.feature.clone();
        }
        self.roster.scoped_packages.push(def);
        Ok(())
    }

    /// Declares that this feature's definition of the callable name `name`, of
    /// kind `kind` (macro, environment or special character sequence), explicitly
    /// replaces another feature's definition of it.
    ///
    /// Two features that define one callable name in one parsing mode are
    /// otherwise a build error, unless the feature order places the feature
    /// registered last explicitly after every other feature that defines the
    /// name in that mode ([`crate::core::registry::order`]); with this
    /// declaration, a feature made to replace another states so. The
    /// declaration counts only where this feature's definition applies: in
    /// each mode in which the feature is registered last among those that
    /// define the name. Elsewhere it is not consulted.
    ///
    /// # Errors
    ///
    /// Never fails; the language build decides whether the replacement applies.
    pub fn override_callable(
        &mut self,
        kind: CallableKind,
        name: impl Into<Name>,
    ) -> Result<(), BuildError> {
        self.roster.overrides.push(OverrideDef {
            owner: self.feature.clone(),
            kind,
            name: name.into(),
        });
        Ok(())
    }

    // --- capabilities and tables --------------------------------------------------------

    /// Declares a [capability](CapabilityDef): a registered dotted name for a
    /// state of processing that a stage brings about.
    ///
    /// # Errors
    ///
    /// Never fails. A name already taken in the namespace that capabilities,
    /// tables and collections share is reported when the registry is built.
    pub fn capability(&mut self, def: CapabilityDef) -> Result<(), BuildError> {
        self.roster.capabilities.push((self.feature.clone(), def));
        Ok(())
    }

    /// Declares an [argument role](ArgumentRoleDef) called `name`, described by
    /// `docs`: a registered name for what an argument of a command holds, which
    /// this feature and others set on arguments with
    /// [`ArgDef::role`](crate::lang::mint::ArgDef::role).
    ///
    /// Write `docs` for a person who reads the registry's declarations: one
    /// line saying what an argument of this role holds.
    ///
    /// # Errors
    ///
    /// Never fails. A role that another declaration, of this feature or
    /// another, declares too is reported when the registry is built, as
    /// [`BuildError::Duplicate`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry};
    ///
    /// struct Glossary;
    /// impl FlmFeature for Glossary {
    ///     fn info(&self) -> FeatureInfo { FeatureInfo::new("glossary", "Glossary") }
    ///     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    ///         b.argument_role("glossary.entry", "The key of a glossary entry.")
    ///     }
    /// }
    ///
    /// let registry = build_registry([Arc::new(Glossary) as Arc<dyn FlmFeature>])?;
    /// let role = registry.argument_role_named("glossary.entry").expect("declared above");
    /// assert_eq!(role.owner, "glossary");
    /// # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
    /// ```
    // See decision rationale at [§flm-dd-dr:arg-role].
    pub fn argument_role(&mut self, name: impl Into<Name>, docs: impl Into<String>) -> Result<(), BuildError> {
        self.roster.argument_roles.push((self.feature.clone(), ArgumentRoleDef::new(name, docs)));
        Ok(())
    }

    /// Declares that this feature sets the [argument role](ArgumentRoleDef)
    /// `name`, which another feature declares, on arguments of its own
    /// declarations, without depending on that feature: a peer declaration
    /// ([`PeerName`](super::PeerName)).
    ///
    /// A feature may set a role that another feature declares whenever that
    /// feature is part of the registry. Declaring the role as a peer is what
    /// lets the registry build when that feature is absent. When the registry
    /// is built, the declaration is resolved against the roles every feature
    /// declares:
    ///
    /// - if a feature declares the role, the declaration changes nothing;
    /// - if none does, the build still succeeds, one Note
    ///   ([`UnresolvedPeer`](super::UnresolvedPeer)) joins the registry's
    ///   [build diagnostics](super::FlmRegistry::build_diagnostics), and the
    ///   arguments of this feature keep the role, of which the registry holds
    ///   no description.
    ///
    /// A role that no feature declares and that this feature did not declare
    /// as a peer is a build error ([`BuildError::UndeclaredArgumentRole`]),
    /// which catches a misspelled name. Declaring the same name again from
    /// the same feature has no effect. Two features may each declare the
    /// same name.
    ///
    /// # Errors
    ///
    /// Never fails; an unresolved declaration is reported as a Note when the
    /// registry is built.
    pub fn peer_argument_role(&mut self, name: impl Into<Name>) -> Result<(), BuildError> {
        self.roster.peer_argument_roles.push(PeerArgumentRoleRecord {
            owner: self.feature.clone(),
            name: name.into(),
        });
        Ok(())
    }

    /// Declares a [collection](CollectionDef): a name for a set of capabilities
    /// that any feature may add to.
    ///
    /// # Errors
    ///
    /// Never fails. A name already taken in the namespace that capabilities,
    /// tables and collections share is reported when the registry is built.
    pub fn collection(&mut self, def: CollectionDef) -> Result<(), BuildError> {
        self.roster.collections.push((self.feature.clone(), def));
        Ok(())
    }

    /// Adds the capability `member` to the collection `collection`, which any
    /// feature may declare, in any order of registration.
    ///
    /// When no feature of the registry declares the collection, the build drops
    /// the member without any diagnostic. A feature that adds its check to a
    /// collection another feature declares must still be registrable on its own,
    /// and reporting the absent collection would refuse ordinary registries.
    ///
    /// # Errors
    ///
    /// Never fails. A member that is not a declared capability is reported when
    /// the registry is built.
    pub fn collection_member(
        &mut self,
        collection: impl Into<CapName>,
        member: impl Into<CapName>,
    ) -> Result<(), BuildError> {
        self.roster.members.push((self.feature.clone(), collection.into(), member.into()));
        Ok(())
    }

    /// The value type of the table `name` as the declarations collected so far
    /// hold it: the type of a table some feature declared, else the type of a
    /// peer table this feature declared ([`peer_table`](FeatureBuilder::peer_table)),
    /// else `None`.
    ///
    /// A [`Finalizer`] reads every feature's declarations here, since the
    /// second registration round runs after the first round of every feature;
    /// a feature's `register` sees only the features registered before it.
    pub fn table_type_named(&self, name: &str) -> Option<TableType> {
        let declared = self.roster.tables.iter().find(|record| record.def.name.as_str() == name);
        declared.map(|record| record.def.ty).or_else(|| {
            self.roster
                .peer_tables
                .iter()
                .find(|record| record.owner == self.feature && record.name.as_str() == name)
                .map(|record| record.value)
        })
    }

    fn table_record<T: Table>(
        &mut self,
        name: CapName,
        docs: String,
        external: bool,
    ) -> TableHandle<T> {
        self.roster.tables.push(TableRecord {
            def: TableDef {
                name: name.clone(),
                ty: TableType::of::<T>(),
                docs,
                owner: self.feature.clone(),
            },
            external,
            refines: None,
            default: false,
        });
        TableHandle::new(name)
    }

    /// Declares a [table](crate::core::data::table::Table) holding a value of type
    /// `T`, under the name given by `T`'s [`TableName`] constant. A table is typed
    /// data that one stage writes beside the frozen trees and other stages read.
    ///
    /// A table's name is also a capability, the capability of the table having
    /// been written. This method therefore declares that capability too, and the
    /// name must be free in the namespace that capabilities, tables and
    /// collections share.
    ///
    /// The returned [`TableHandle`] is what a stage declares its writes and reads
    /// with, and what reads the table's value at run time.
    ///
    /// # Errors
    ///
    /// Never fails. A name clash is reported when the registry is built.
    pub fn table<T: Table>(
        &mut self,
        name: TableName<T>,
        docs: impl Into<String>,
    ) -> Result<TableHandle<T>, BuildError> {
        Ok(self.table_record::<T>(CapName::from(name.name()), crate::kept_docs(docs), false))
    }

    /// Declares a table holding a value of type `T` under a name computed at
    /// registration, such as `flm.lowered.<backend>`.
    ///
    /// This method does exactly what [`table`](FeatureBuilder::table) does, for a
    /// name that is not known at compile time and so cannot be a [`TableName`]
    /// constant.
    ///
    /// # Errors
    ///
    /// Never fails. A name clash is reported when the registry is built.
    pub fn table_named<T: Table>(
        &mut self,
        name: impl Into<CapName>,
        docs: impl Into<String>,
    ) -> Result<TableHandle<T>, BuildError> {
        Ok(self.table_record::<T>(name.into(), crate::kept_docs(docs), false))
    }

    /// Declares a table holding a value of type `T` under a name computed at
    /// registration, whose capability refines the capability `parent`: a stage
    /// that writes the table provides `parent` too, as a stage that provides a
    /// refining capability does ([`CapabilityDef::refines`]).
    ///
    /// Otherwise exactly [`table_named`](FeatureBuilder::table_named). A
    /// processing feature declares the table of its rendered formulas this way,
    /// as a refinement of the capability of rendered mathematics.
    ///
    /// # Errors
    ///
    /// Never fails. A name clash, a `parent` that no feature declares, or a
    /// chain of refinements that leads back to itself is reported when the
    /// registry is built.
    pub fn table_refining_named<T: Table>(
        &mut self,
        name: impl Into<CapName>,
        parent: impl Into<CapName>,
        docs: impl Into<String>,
    ) -> Result<TableHandle<T>, BuildError> {
        let handle = self.table_record::<T>(name.into(), crate::kept_docs(docs), false);
        if let Some(record) = self.roster.tables.last_mut() {
            record.refines = Some(parent.into());
        }
        Ok(handle)
    }

    /// Declares an **external table** holding a value of type `T`. An external
    /// table is a table that no stage of the run writes: the caller installs its
    /// value when the run is constructed ([`Externals`](crate::core::run::Externals)).
    ///
    /// External tables bring data from outside the run into the plan, for example
    /// results carried over from a previous run, or a table the host computes
    /// itself. The installation satisfies the table's capability, and a stage that
    /// declares a write of the table is a build error.
    ///
    /// # Errors
    ///
    /// Never fails. A name clash, or a stage that writes this table, is reported
    /// when the registry is built.
    pub fn external_table<T: Table>(
        &mut self,
        name: TableName<T>,
        docs: impl Into<String>,
    ) -> Result<TableHandle<T>, BuildError> {
        Ok(self.table_record::<T>(CapName::from(name.name()), crate::kept_docs(docs), true))
    }

    /// Declares an external table holding a value of type `T` under a name
    /// computed at registration; otherwise exactly
    /// [`external_table`](FeatureBuilder::external_table).
    ///
    /// # Errors
    ///
    /// Never fails; problems are reported when the registry is built.
    pub fn external_table_named<T: Table>(
        &mut self,
        name: impl Into<CapName>,
        docs: impl Into<String>,
    ) -> Result<TableHandle<T>, BuildError> {
        Ok(self.table_record::<T>(name.into(), crate::kept_docs(docs), true))
    }

    // --- stages ---------------------------------------------------------------------------

    fn push_stage(&mut self, stage: Arc<dyn Stage>) -> usize {
        let decl = stage.decl();
        self.roster.stages.push(StageRecord {
            owner: self.feature.clone(),
            kind: StageKind::Pipeline(stage),
            decl,
            rewrite: None,
        });
        self.roster.stages.len() - 1
    }

    /// Registers an ordinary [stage](Stage): one that runs after the trees are
    /// frozen and computes data, checks something, or performs input and output.
    ///
    /// The builder reads the stage's [`decl`](Stage::decl) once, here, and the
    /// build and every plan use that declaration; a stage's declaration is
    /// therefore fixed for the life of the registry. Register a rewrite stage with
    /// [`rewrite_stage`](FeatureBuilder::rewrite_stage) instead.
    ///
    /// # Errors
    ///
    /// Never fails. A duplicate stage name, a check stage that declares a write,
    /// or a reference to an entity no feature declares is reported when the
    /// registry is built.
    pub fn stage(&mut self, stage: Arc<dyn Stage>) -> Result<(), BuildError> {
        self.push_stage(stage);
        Ok(())
    }

    /// Registers a [rewrite stage](RewriteStage): a stage that turns one tree
    /// into another before the trees are frozen.
    ///
    /// The builder reads the stage's [`decl`](RewriteStage::decl) once, here. It
    /// declares at the same time the table of trees that the rewrite publishes,
    /// under the name the declaration gives, with the refinement and the default
    /// mark the declaration gives. A rewrite's output is therefore a table like
    /// any other, and no other part of the library needs to know that a rewrite
    /// produced it.
    ///
    /// # Errors
    ///
    /// Never fails; problems are reported when the registry is built.
    pub fn rewrite_stage(&mut self, stage: Arc<dyn RewriteStage>) -> Result<(), BuildError> {
        let rewrite = stage.decl();
        let decl = StageDecl::new(
            rewrite.name.clone(),
            crate::core::stage::Effect::Compute,
            crate::core::stage::Scope::PerTree,
        )
        .writes(TableHandle::<crate::core::stage::TreesTable>::new(rewrite.publishes.clone()))
        .docs(rewrite.docs.clone());
        self.roster.tables.push(TableRecord {
            def: TableDef {
                name: rewrite.publishes.clone(),
                ty: TableType::of::<crate::core::stage::TreesTable>(),
                docs: rewrite.docs.clone(),
                owner: self.feature.clone(),
            },
            external: false,
            refines: rewrite.refines.clone(),
            default: rewrite.default,
        });
        self.roster.stages.push(StageRecord {
            owner: self.feature.clone(),
            kind: StageKind::Rewrite(stage),
            decl,
            rewrite: Some(rewrite),
        });
        Ok(())
    }

    /// Places [ordering edges](OrderingDef) on a stage, this feature's or
    /// another feature's, saying which capabilities' providers the stage must run
    /// after or before.
    ///
    /// # Errors
    ///
    /// Never fails. Edges on a stage that no feature registers, or edges that
    /// refer to a capability no feature declares, are reported when the registry
    /// is built.
    pub fn ordering(&mut self, def: OrderingDef) -> Result<(), BuildError> {
        self.roster.orderings.push((self.feature.clone(), def));
        Ok(())
    }

    // --- lowering ---------------------------------------------------------------------

    /// Registers a [lowering rule](LoweringRule): a named registry entity written
    /// for one element, which builds the element's presentation for one backend.
    ///
    /// # Errors
    ///
    /// Never fails. A duplicate rule name, an element or a backend that no feature
    /// declares, or two rules marked as the default for one element on one
    /// backend is reported when the registry is built.
    pub fn lowering_rule(&mut self, rule: LoweringRule) -> Result<(), BuildError> {
        self.roster.rules.push((self.feature.clone(), rule));
        Ok(())
    }

    // See the design rationale at [§flm-dd-dr:feat-pres-minted].
    /// Registers a **presentation element**: an element that exists only in the
    /// trees a backend writes, with no syntax and no callable name of its own,
    /// whose nodes carry a payload of type `P`. Returns the element value.
    ///
    /// The builder creates the element value here and returns it, because there
    /// is no source syntax to create it from and the backend's author needs the
    /// value itself to build the nodes that carry it. Attach aspects to it with
    /// [`attach`](FeatureBuilder::attach), as to any element.
    ///
    /// # Errors
    ///
    /// Never fails. A name that is also the name of an element that can appear in
    /// a parsed document is reported when the registry is built.
    pub fn presentation_element<P: PresPayload>(
        &mut self,
        name: impl Into<ElementName>,
        docs: impl Into<String>,
    ) -> Result<Arc<ElementSpec>, BuildError> {
        let name = name.into();
        let docs = crate::kept_docs(docs);
        let element = Arc::new(ElementSpec::new(name).docs(docs.clone()));
        self.roster.presentation.push(PresentationRecord {
            owner: self.feature.clone(),
            element: Arc::clone(&element),
            payload: TypeKey::of::<P>(),
            docs,
        });
        Ok(element)
    }

    /// Registers a [backend](BackendDef): the registration bundle for one output
    /// format, with its native set and its lowering, emit and final stages.
    ///
    /// The builder registers the three stages as this feature's own stages, in
    /// the order lowering, emit, final; they are scheduled like any other stage.
    /// The backend declaration groups them so that a plan recognizes them as one
    /// backend.
    ///
    /// # Errors
    ///
    /// Never fails. A native element that no feature declares, or a duplicate
    /// backend or stage name, is reported when the registry is built; so is a
    /// native set that does not satisfy a [native-set
    /// implication](NativeImplication), as [`BuildError::NativeSetIncomplete`].
    pub fn backend(&mut self, def: BackendDef) -> Result<(), BuildError> {
        let lowering = self.push_stage(def.lowering);
        let emit = self.push_stage(def.emit);
        let final_stage = self.push_stage(def.final_stage);
        self.roster.backends.push(BackendRecord {
            owner: self.feature.clone(),
            name: def.name,
            native: def.native,
            docs: def.docs,
            lowering,
            emit,
            final_stage,
        });
        Ok(())
    }

    /// Declares a [native-set implication](NativeImplication): a requirement on
    /// what a backend's native set must contain, checked against every backend of
    /// the registry, this feature's and every other feature's, when the registry
    /// is built.
    ///
    /// # Errors
    ///
    /// Never fails. An element that no feature declares is reported when the
    /// registry is built as [`BuildError::Undeclared`], and a backend whose native
    /// set does not satisfy the implication as
    /// [`BuildError::NativeSetIncomplete`].
    pub fn native_implication(&mut self, def: NativeImplication) -> Result<(), BuildError> {
        self.roster.native_implications.push((self.feature.clone(), def));
        Ok(())
    }

    /// Declares a **peer backend**: a [peer declaration](super::PeerName) of a
    /// backend called `name` that this feature expects another feature of the
    /// registry to register. With it, this feature's lowering rules may list the
    /// backend in their default mark
    /// ([`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends))
    /// without this feature registering the backend or depending on the feature
    /// that does.
    ///
    /// The declaration registers nothing and counts for this feature alone. When
    /// the registry is built, it is resolved against the declarations of every
    /// feature:
    ///
    /// - if a feature registers the backend, this feature's rules that refer to
    ///   it take part like any other rule;
    /// - if none does, the build still succeeds, one Note
    ///   ([`UnresolvedPeer`](super::UnresolvedPeer)) joins the registry's [build
    ///   diagnostics](super::FlmRegistry::build_diagnostics), and every rule of
    ///   this feature that refers to the backend is set aside: never selected, and
    ///   never counted by the check that the backend can write every element.
    ///
    /// See [`PeerName`](super::PeerName) for the full rules, and
    /// [`peer_element`](FeatureBuilder::peer_element) and
    /// [`peer_table`](FeatureBuilder::peer_table) for the presentation elements
    /// such a rule produces and the tables it reads.
    ///
    /// Declaring the same name again from the same feature has no effect: the
    /// repeat is merged into the first declaration, whose `docs` are kept. Two
    /// features may each declare the same name.
    ///
    /// # Errors
    ///
    /// Never fails; an unresolved declaration is reported as a Note when the
    /// registry is built.
    pub fn peer_backend(
        &mut self,
        name: impl Into<BackendName>,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.peer_backends.push(PeerBackendRecord {
            owner: self.feature.clone(),
            name: name.into(),
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    /// Declares a **peer element**: a [peer declaration](super::PeerName) of a
    /// presentation element called `name`, whose nodes carry a payload of type
    /// `P`, that this feature expects another feature of the registry to register
    /// with [`presentation_element`](FeatureBuilder::presentation_element). With
    /// it, this feature's lowering rules may list the element in their `produces`
    /// without this feature depending on the feature that owns the element.
    ///
    /// The payload type is the declaration's signature. When the registry is
    /// built, the declaration is resolved against the declarations of every
    /// feature:
    ///
    /// - a presentation element registered under this name with payload type `P`
    ///   resolves it;
    /// - a presentation element registered with another payload type, or an
    ///   element of this name that can appear in a parsed document, is refused as
    ///   [`BuildError::PeerElementMismatch`];
    /// - a name that no feature declares leaves the declaration unresolved: every
    ///   rule of this feature that produces the element is set aside, as
    ///   [`peer_backend`](FeatureBuilder::peer_backend) describes.
    ///
    /// Declaring the same name again from the same feature with the same payload
    /// type has no effect: the repeat is merged into the first declaration, whose
    /// `docs` are kept. Declaring it again with another payload type is a
    /// duplicate, refused when the registry is built as [`BuildError::Duplicate`].
    /// Two features may each declare the same name, since a declaration counts for
    /// its own feature alone.
    ///
    /// # Errors
    ///
    /// Never fails. When the registry is built, a repeat with another payload type
    /// is reported as [`BuildError::Duplicate`], and a payload mismatch as
    /// [`BuildError::PeerElementMismatch`].
    pub fn peer_element<P: PresPayload>(
        &mut self,
        name: impl Into<ElementName>,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.peer_elements.push(PeerElementRecord {
            owner: self.feature.clone(),
            name: name.into(),
            payload: TypeKey::of::<P>(),
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    /// Declares a **peer table**: a [peer declaration](super::PeerName) of a table
    /// called `name`, holding a value of type `R`, that this feature expects
    /// another feature of the registry to declare. With it, this feature's
    /// lowering rules may read the table, and its stages may write it, without
    /// this feature depending on the feature that owns the table.
    ///
    /// The value type is the declaration's signature: the type the reading rule
    /// expects, which is the type the owning feature declared the table with.
    /// When the registry is built, the declaration is resolved against the
    /// declarations of every feature: a table declared under this name holding an
    /// `R` resolves it; a table holding another type is refused as
    /// [`BuildError::PeerTableMismatch`]; and a name that no feature declares
    /// leaves the declaration unresolved.
    ///
    /// What an unresolved peer table does to a lowering rule depends on how the
    /// rule reads it:
    ///
    /// - a rule that reads it as a hard dependency
    ///   ([`LoweringDecl::reads`](crate::core::lowering::LoweringDecl::reads)) is
    ///   set aside, as [`peer_backend`](FeatureBuilder::peer_backend) describes,
    ///   because without the table the rule cannot do its work;
    /// - a rule that reads it optionally
    ///   ([`reads_opt`](crate::core::lowering::LoweringDecl::reads_opt)) is
    ///   selected and runs, and finds the table absent; this lets a rule write
    ///   what it can in a registry that does not hold the other feature.
    ///
    /// In both cases, one Note ([`UnresolvedPeer`](super::UnresolvedPeer)) joins
    /// the registry's [build diagnostics](super::FlmRegistry::build_diagnostics).
    ///
    /// A stage of this feature may also *write* a peer table: the other feature
    /// declares the table, and this feature's stage writes it (for example, the
    /// `graphics` feature's publish stage writes the copies of images into an
    /// output's artifact table, which the backend declares). When the peer table
    /// is unresolved, every stage of this feature that writes it is set aside and
    /// not registered, since it would have nowhere to write and the feature that
    /// reads the table is absent; the Note lists those stages. The build also
    /// drops the ordering edges placed on such a stage
    /// ([`FeatureBuilder::ordering`]).
    ///
    /// A peer table is not an [external table](crate::core::run::Externals): an
    /// external table is a table that no stage of the run writes, whose value the
    /// caller installs when the run is constructed, and the feature that declares
    /// it registers it with [`external_table`](FeatureBuilder::external_table);
    /// a peer table belongs to another feature, whose stage computes it like any
    /// other table.
    ///
    /// Declaring the same name again from the same feature with the same value
    /// type has no effect: the repeat is merged into the first declaration, whose
    /// `docs` are kept. Declaring it again with another value type is a duplicate,
    /// refused when the registry is built as [`BuildError::Duplicate`]. Two
    /// features may each declare the same name.
    ///
    /// # Errors
    ///
    /// Never fails. When the registry is built, a repeat with another value type
    /// is reported as [`BuildError::Duplicate`], and a value-type mismatch as
    /// [`BuildError::PeerTableMismatch`].
    pub fn peer_table<R: Table>(
        &mut self,
        name: impl Into<CapName>,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.peer_tables.push(PeerTableRecord {
            owner: self.feature.clone(),
            name: name.into(),
            value: TableType::of::<R>(),
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    /// Declares a peer table whose value type is given as a [`TableType`]
    /// rather than as a type parameter: exactly
    /// [`peer_table`](FeatureBuilder::peer_table) with `R` the type `value`
    /// stands for.
    ///
    /// For a caller that names tables by name and learns their types from the
    /// declarations ([`table_type_named`](FeatureBuilder::table_type_named)),
    /// such as a [`Finalizer`] declaring for a feature written in another
    /// language.
    ///
    /// # Errors
    ///
    /// As [`peer_table`](FeatureBuilder::peer_table).
    pub fn peer_table_of(
        &mut self,
        name: impl Into<CapName>,
        value: TableType,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.peer_tables.push(PeerTableRecord {
            owner: self.feature.clone(),
            name: name.into(),
            value,
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    /// The payload type of the presentation element `name` as the collected
    /// declarations hold it ([`presentation_element`](FeatureBuilder::presentation_element)),
    /// or `None` when no feature registered a presentation element of that
    /// name. Like [`table_type_named`](FeatureBuilder::table_type_named), it
    /// sees every feature's declarations from a [`Finalizer`].
    pub fn presentation_payload_named(&self, name: &str) -> Option<TypeKey> {
        self.roster
            .presentation
            .iter()
            .find(|record| record.element.name.as_str() == name)
            .map(|record| record.payload)
    }

    /// Declares a peer element whose payload type is given as a [`TypeKey`]
    /// rather than as a type parameter: exactly
    /// [`peer_element`](FeatureBuilder::peer_element) with `P` the type
    /// `payload` stands for. For the same callers as
    /// [`peer_table_of`](FeatureBuilder::peer_table_of), with
    /// [`presentation_payload_named`](FeatureBuilder::presentation_payload_named).
    ///
    /// # Errors
    ///
    /// As [`peer_element`](FeatureBuilder::peer_element).
    pub fn peer_element_of(
        &mut self,
        name: impl Into<ElementName>,
        payload: TypeKey,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.peer_elements.push(PeerElementRecord {
            owner: self.feature.clone(),
            name: name.into(),
            payload,
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    /// Declares a **peer scoped package**: a [peer
    /// declaration](super::PeerName) of a [scoped package](ScopedPackageDef)
    /// called `name` that this feature expects another feature of the registry to
    /// declare with [`scoped_package`](FeatureBuilder::scoped_package). With it,
    /// the environment bodies and arguments that this feature declares may refer
    /// to the package ([`BodyDef::scoped`](crate::lang::mint::BodyDef::scoped),
    /// [`ArgDef::scoped`](crate::lang::mint::ArgDef::scoped)) without this feature
    /// depending on the feature that owns it.
    ///
    /// The declaration registers nothing and counts for this feature alone. When
    /// the registry is built, it is resolved against the declarations of every
    /// feature:
    ///
    /// - if a feature declares the package, the bodies and arguments that refer to
    ///   it get its definitions, exactly as without the peer declaration;
    /// - if none does, the build still succeeds, one Note
    ///   ([`UnresolvedPeer`](super::UnresolvedPeer)) joins the registry's [build
    ///   diagnostics](super::FlmRegistry::build_diagnostics), and each body or
    ///   argument of this feature that refers to the package gets no definition
    ///   from it: a command the package would have defined is an unknown command
    ///   there, as anywhere else.
    ///
    /// A scoped package name that no feature declares and that this feature did
    /// not declare as a peer remains the build error
    /// [`MintError::UnknownScopedPackage`](crate::lang::mint::MintError::UnknownScopedPackage),
    /// which catches a misspelled name. See [`PeerName`](super::PeerName) for the
    /// other kinds of peer declaration.
    ///
    /// Declaring the same name again from the same feature has no effect: the
    /// repeat is merged into the first declaration, whose `docs` are kept. Two
    /// features may each declare the same name.
    ///
    /// # Errors
    ///
    /// Never fails; an unresolved declaration is reported as a Note when the
    /// registry is built.
    pub fn peer_scoped_package(
        &mut self,
        name: impl Into<Name>,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.peer_scoped_packages.push(PeerScopedPackageRecord {
            owner: self.feature.clone(),
            name: name.into(),
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    /// Registers an [output](OutputDef): a name a caller asks for to have
    /// something produced, such as `text` or `html`.
    ///
    /// # Errors
    ///
    /// Never fails. An output whose table does not hold an artifact set, or whose
    /// capability is neither its table's capability nor a refinement of it, is
    /// reported when the registry is built.
    pub fn output(&mut self, def: OutputDef) -> Result<(), BuildError> {
        self.roster.outputs.push((self.feature.clone(), def));
        Ok(())
    }

    /// Registers a [workflow](WorkflowDef): a named set of demands together with
    /// the choices that go with them.
    ///
    /// # Errors
    ///
    /// Never fails. A workflow that refers to an output, capability, stage,
    /// backend, element or lowering rule that no feature declares, a chain of
    /// workflows that leads back to itself, or a render option value whose option
    /// type no feature declares or cannot be set on a render request is reported
    /// when the registry is built.
    pub fn workflow(&mut self, def: WorkflowDef) -> Result<(), BuildError> {
        self.roster.workflows.push((self.feature.clone(), def));
        Ok(())
    }

    /// Registers fields for the workflow called `def.name`: every field that
    /// `def` declares replaces that field of the workflow a feature registers
    /// under that name with [`workflow`](FeatureBuilder::workflow), so that the
    /// two make one workflow. When no feature registers a workflow of that name,
    /// `def` is the workflow.
    ///
    /// A field is declared when it is set: `extends` given, a list or a map not
    /// empty, `docs` not empty
    /// ([`WorkflowDef::replace_declared_fields`]). Fields registered for one name
    /// by several features are applied in registration order, the later replacing
    /// the earlier, and the fields the embedding program registers with
    /// [`RegistryBuilder::workflow_fields`](super::RegistryBuilder::workflow_fields)
    /// are applied after all of them. A feature uses this method to change some
    /// fields of a workflow without knowing whether another feature registers a
    /// workflow of that name.
    ///
    /// # Errors
    ///
    /// Never fails; the combined workflow is checked like any other when the
    /// registry is built.
    pub fn workflow_fields(&mut self, def: WorkflowDef) -> Result<(), BuildError> {
        self.roster.workflow_fields.push((self.feature.clone(), def));
        Ok(())
    }

    // --- options and services ---------------------------------------------------------

    /// Declares the option type `T`, whose values are set where its binding
    /// `bind` allows: on the document, on the render request, on either, or
    /// nowhere because the value is fixed when the registry is built (see
    /// [`Bind`] and [option types](crate::model::options)).
    ///
    /// # Errors
    ///
    /// Never fails. A second declaration of the same option type, or a workflow
    /// that sets a value where the binding does not allow it, is reported when
    /// the registry is built.
    pub fn options<T: Options>(&mut self, bind: Bind) -> Result<(), BuildError> {
        self.roster.options.push(OptionDecl::of::<T>(self.feature.clone(), bind));
        Ok(())
    }

    /// Declares the option type `T` with the binding `bind` and with
    /// documentation prose; otherwise exactly [`options`](FeatureBuilder::options).
    ///
    /// # Errors
    ///
    /// Never fails; problems are reported when the registry is built.
    pub fn options_with_docs<T: Options>(
        &mut self,
        bind: Bind,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.roster.options.push(OptionDecl::of::<T>(self.feature.clone(), bind).docs(docs));
        Ok(())
    }

    /// Declares the option type declared from data under the name `name`, with
    /// the binding `bind` and the documentation `docs`. Only a
    /// [`SettingsDecl`](crate::core::settings::SettingsDecl) declares one
    /// ([`config_data`](crate::core::settings::SettingsDecl::config_data),
    /// [`options_data`](crate::core::settings::SettingsDecl::options_data)).
    ///
    /// # Errors
    ///
    /// Never fails; a second declaration of the same name, by this feature or
    /// another, is reported when the registry is built, as for an option type
    /// that is a Rust type.
    pub(crate) fn options_named(&mut self, name: Name, bind: Bind, docs: String) -> Result<(), BuildError> {
        self.roster.options.push(OptionDecl::named(name, self.feature.clone(), bind).docs(docs));
        Ok(())
    }

    /// Declares a [service slot](crate::core::services) called `name`, holding
    /// values of type `S` (usually a trait-object type), and taking as many
    /// contributions as `arity` allows.
    ///
    /// A service is a value that can be shared across threads which one feature
    /// supplies for another feature, or for the embedding program, to use. By
    /// declaring a service slot, a feature states that something it needs is to be
    /// supplied from outside the feature, by another feature or by the embedding
    /// program.
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`]: the build assembles the service slots between the two
    /// registration rounds, and they cannot change afterwards.
    pub fn service_slot<S: ?Sized + Any + Send + Sync>(
        &mut self,
        name: impl Into<Name>,
        arity: Arity,
        docs: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.round_one_only(RoundTwoAction::DeclareSlot)?;
        self.roster.slots.push(SlotDecl {
            name: name.into(),
            ty: TypeKey::of::<S>(),
            arity,
            owner: self.feature.clone(),
            docs: crate::kept_docs(docs),
        });
        Ok(())
    }

    fn contribute_inner<S: ?Sized + Any + Send + Sync>(
        &mut self,
        target: SlotTarget,
        imp: Arc<S>,
        prepend: bool,
    ) -> Result<(), BuildError> {
        self.round_one_only(RoundTwoAction::Contribute)?;
        self.roster.contributions.push(Contribution::new(
            self.feature.clone(),
            target,
            imp,
            prepend,
        ));
        Ok(())
    }

    /// Contributes `imp` to the service slot whose value type is `S`, after every
    /// contribution already made to it.
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`].
    pub fn contribute<S: ?Sized + Any + Send + Sync>(
        &mut self,
        imp: Arc<S>,
    ) -> Result<(), BuildError> {
        self.contribute_inner(SlotTarget::ByType(TypeKey::of::<S>()), imp, false)
    }

    /// Contributes `imp` to the service slot called `slot`, after every
    /// contribution already made to it.
    ///
    /// When no feature declares that service slot, the registry build fails. For
    /// a service slot that another feature owns and that may be absent, use
    /// [`peer_slot`](FeatureBuilder::peer_slot) instead.
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`].
    pub fn contribute_to<S: ?Sized + Any + Send + Sync>(
        &mut self,
        slot: impl Into<Name>,
        imp: Arc<S>,
    ) -> Result<(), BuildError> {
        self.contribute_inner(SlotTarget::ByName(slot.into()), imp, false)
    }

    /// Contributes `imp` to the service slot whose value type is `S`, before
    /// every contribution appended to it.
    ///
    /// Order matters for a service slot that takes several contributions and consults
    /// them in turn; with this method a feature places its contribution first.
    /// Among the contributions placed first this way, a read for precedence
    /// takes those of the feature placed last in the feature order first
    /// ([`Services::services_by_precedence_named`](crate::core::services::Services::services_by_precedence_named)).
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`].
    pub fn contribute_first<S: ?Sized + Any + Send + Sync>(
        &mut self,
        imp: Arc<S>,
    ) -> Result<(), BuildError> {
        self.contribute_inner(SlotTarget::ByType(TypeKey::of::<S>()), imp, true)
    }

    /// Contributes `imp` to the service slot called `slot`, before every
    /// contribution appended to it. Among the contributions placed first this
    /// way, a read for precedence takes those of the feature placed last in the
    /// feature order first
    /// ([`Services::services_by_precedence_named`](crate::core::services::Services::services_by_precedence_named)).
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`].
    pub fn contribute_first_to<S: ?Sized + Any + Send + Sync>(
        &mut self,
        slot: impl Into<Name>,
        imp: Arc<S>,
    ) -> Result<(), BuildError> {
        self.contribute_inner(SlotTarget::ByName(slot.into()), imp, true)
    }

    /// Makes a **peer contribution**: contributes `imp` to the service slot
    /// called `slot`, a service slot that another feature of the registry is
    /// expected to declare. A feature uses it to offer a value to a feature that may not be
    /// registered. This is the service-slot form of a [peer
    /// declaration](super::PeerName).
    ///
    /// [`contribute_to`](FeatureBuilder::contribute_to) is the form for a service
    /// slot this feature owns, or one it knows is declared: contributing to a
    /// service slot that no feature declares is then a registry build error
    /// ([`ServiceError::UndeclaredSlot`]), which catches a misspelled name. With
    /// this method, the service slot belongs to another feature:
    ///
    /// - if a feature declares the service slot, the value fills it exactly as
    ///   [`contribute_to`](FeatureBuilder::contribute_to) would, after every
    ///   contribution already made, and with its type checked against the
    ///   service slot's value type;
    /// - if no feature declares it, the value is dropped, the build still
    ///   succeeds, and one Note ([`UnresolvedPeer`](super::UnresolvedPeer)) joins
    ///   the registry's [build diagnostics](super::FlmRegistry::build_diagnostics),
    ///   one per feature and service slot, however many values this feature
    ///   offered to that service slot.
    ///
    /// The value type `S` is the declaration's signature, written as the type the
    /// service slot declares: a trait-object type, as in
    /// `b.peer_slot::<dyn Greeter>(…)`, when the value is behavior. A value of
    /// another type than the service slot declares
    /// is refused, as it is through
    /// [`contribute_to`](FeatureBuilder::contribute_to).
    ///
    /// Every peer contribution, resolved or not, is listed on the finished
    /// registry ([`FlmRegistry::peer_slots`](super::FlmRegistry::peer_slots)), so a
    /// misspelled service slot name can be found on any registry that holds the feature.
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`]: the build assembles the service slots between the two
    /// registration rounds, and they cannot change afterwards.
    pub fn peer_slot<S: ?Sized + Any + Send + Sync>(
        &mut self,
        slot: impl Into<Name>,
        imp: Arc<S>,
    ) -> Result<(), BuildError> {
        self.round_one_only(RoundTwoAction::Contribute)?;
        self.roster.contributions.push(
            Contribution::new(self.feature.clone(), SlotTarget::ByName(slot.into()), imp, false)
                .as_peer(),
        );
        Ok(())
    }

    /// Declares that this feature cannot work unless the service slot whose value
    /// type is `S` is filled.
    ///
    /// A required service slot that is not filled makes the registry build fail, so a
    /// missing service is reported once, when the registry is built, instead of
    /// as a failure in the middle of processing a document.
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`].
    pub fn require_service<S: ?Sized + Any + Send + Sync>(&mut self) -> Result<(), BuildError> {
        self.round_one_only(RoundTwoAction::RequireSlot)?;
        self.roster.requirements.push(ServiceRequirement {
            owner: self.feature.clone(),
            target: SlotTarget::ByType(TypeKey::of::<S>()),
        });
        Ok(())
    }

    /// Declares that this feature cannot work unless the service slot called
    /// `slot` is filled; otherwise exactly
    /// [`require_service`](FeatureBuilder::require_service).
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`].
    pub fn require_service_named(&mut self, slot: impl Into<Name>) -> Result<(), BuildError> {
        self.round_one_only(RoundTwoAction::RequireSlot)?;
        self.roster.requirements.push(ServiceRequirement {
            owner: self.feature.clone(),
            target: SlotTarget::ByName(slot.into()),
        });
        Ok(())
    }

    // --- hooks and records ------------------------------------------------------------

    /// Registers a [`Finalizer`] of this feature: a build hook that runs in the
    /// build's second registration round, once the service slots are assembled.
    ///
    /// A feature may register several finalizers; they run in the order
    /// registered.
    ///
    /// # Errors
    ///
    /// [`BuildError::ContributionAfterAssembly`] when called from a
    /// [`Finalizer`]: there are exactly two rounds, so a finalizer cannot add a
    /// third.
    pub fn finalizer(&mut self, finalizer: Arc<dyn Finalizer>) -> Result<(), BuildError> {
        self.round_one_only(RoundTwoAction::RegisterFinalizer)?;
        self.roster.finalizers.push((self.feature.clone(), finalizer));
        Ok(())
    }

    /// Registers a [`LateInit`] hook of this feature: a build hook that runs once
    /// the FLM language is built, for example to compile the templates the
    /// feature's configuration holds.
    ///
    /// Unlike [`finalizer`](FeatureBuilder::finalizer), this method may be called
    /// in either round.
    ///
    /// # Errors
    ///
    /// Never fails. A failure of the hook itself makes the build fail in the
    /// late-initialization phase.
    pub fn late_init(&mut self, hook: Arc<dyn LateInit>) -> Result<(), BuildError> {
        self.roster.late_inits.push((self.feature.clone(), hook));
        Ok(())
    }

    /// Records a [style definition](StyleDef) in the registry: a stylesheet that
    /// a feature's output depends on, under an identifier.
    ///
    /// A lowering rule or an emitter then requires the definition by its
    /// identifier where its output needs it; the backend that writes the output
    /// resolves the identifier against the definitions recorded here.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn style_def(&mut self, mut def: StyleDef) -> Result<(), BuildError> {
        def.docs = crate::kept_docs(def.docs);
        self.roster.styles.push((self.feature.clone(), def));
        Ok(())
    }

    /// Records a [script definition](ScriptDef) in the registry: the counterpart
    /// of [`style_def`](FeatureBuilder::style_def) for scripts.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn script_def(&mut self, mut def: ScriptDef) -> Result<(), BuildError> {
        def.docs = crate::kept_docs(def.docs);
        self.roster.scripts.push((self.feature.clone(), def));
        Ok(())
    }

    /// Gives `bytes` as an input of the [registry digest](crate::model::digest),
    /// the number that stands for everything about a registry that can change
    /// what processing produces.
    ///
    /// The build computes the digest from the declarations it can enumerate. A
    /// feature's configuration may hold other data that changes what processing
    /// produces (for example, the text of an author-written template), which the
    /// build cannot see. The feature gives that data here, so that data cached
    /// under the digest is not reused after the configuration changes.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn digest_input(&mut self, bytes: &[u8]) -> Result<(), BuildError> {
        self.roster
            .digest_inputs
            .push(DigestInput { owner: self.feature.clone(), bytes: bytes.to_vec() });
        Ok(())
    }
}

impl fmt::Debug for FeatureBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeatureBuilder")
            .field("feature", &self.feature)
            .field("round", &self.round)
            .finish()
    }
}

/// The feature name that represents the embedding program itself.
///
/// Service contributions made through
/// [`RegistryBuilder::contribute`](super::RegistryBuilder::contribute) and
/// [`RegistryBuilder::contribute_to`](super::RegistryBuilder::contribute_to),
/// and workflow fields given through
/// [`RegistryBuilder::workflow_fields`](super::RegistryBuilder::workflow_fields),
/// come from no feature; they are attributed to this name, so that every
/// contribution has an owner to show in a message. The leading underscore
/// keeps it apart from the dotted lowercase names of features.
pub const HOST_FEATURE: &str = "_host";
