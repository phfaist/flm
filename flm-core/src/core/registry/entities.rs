//! The declaration values a feature passes to the [feature
//! builder](super::FeatureBuilder), and the two build hooks through which a
//! feature does work later in the registry build.
//!
//! A declaration value describes one entity that a feature contributes to a
//! registry. Some declaration types are defined next to the code that uses
//! them: element declarations in [`crate::lang::mint`], stage declarations in
//! [`crate::core::stage`], table declarations in [`crate::core::data::table`]
//! and lowering rules in [`crate::core::lowering`]. The others are defined
//! here:
//!
//! - [`CapabilityDef`] declares a capability, a registered dotted name for a
//!   state of processing that a stage brings about; [`CollectionDef`] declares
//!   a collection, a name for a set of capabilities that any feature may add
//!   to.
//! - [`ArgumentRoleDef`] declares an argument role, a registered dotted name
//!   for what an argument of a command holds (a label, a citation key, the
//!   path of a file).
//! - [`OrderingDef`] declares ordering edges, which place a stage (possibly
//!   another feature's) after or before the stages that provide given
//!   capabilities.
//! - [`BackendDef`] declares a backend, the registration bundle for one output
//!   format. [`OutputDef`] declares an output, a name a caller asks for to have
//!   something produced; [`OutputEntry`] describes the output's entry
//!   artifact.
//! - [`WorkflowDef`] declares a workflow, a named set of demands together with
//!   the choices that go with them; [`LoweringSelection`] is a workflow's
//!   preference among the lowering rules of one backend.
//! - [`StyleDef`] and [`ScriptDef`] declare a stylesheet or a script that a
//!   feature's output depends on; [`DefContent`] is one part of such a
//!   definition's content.
//! - [`NativeImplication`] and [`NativeImplicationKind`] declare a requirement
//!   on what every backend's native set must contain, checked when the
//!   registry is built.
//! - [`PeerName`] identifies a peer declaration: a feature's forward
//!   declaration of an entity that another feature owns.
//! - [`Finalizer`] and [`LateInit`] are the two build hooks: a finalizer runs
//!   in the build's second registration round, and a late-initialization hook
//!   runs after the language that documents are parsed with
//!   ([`FlmRegistry::lang`](super::FlmRegistry::lang)) is built.
//!
//! Each declaration type is a plain value with a constructor and setters that
//! return `Self`, so a declaration can be written as one expression. No
//! declaration type holds the name of the feature that declares it: the
//! builder attributes each declaration to the feature it was created for.
//!
//! See also [features and
//! packages](crate::guide::concepts_overview#features-and-packages) and
//! [stages, capabilities, and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans).

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::model::element::ElementSpec;
use crate::model::names::{
    BackendName, CapName, ElementName, Name, OutputName, RuleName, StageName, WorkflowName,
};
use techy::serialize::SerialValue;

use crate::core::data::table::{ProjectCx, Table};
use crate::model::options::OptionValues;
use crate::core::registry::builder::{BuildError, FeatureBuilder};
use crate::core::stage::Stage;
use crate::lang::{BuiltLang, FlmDiagnostics};

/// A capability declaration. A **capability** is a registered dotted name for a
/// state of processing that a stage brings about, such as
/// `flm.blocks.structured`: a stage declares the capabilities it provides and
/// the capabilities it requires, and a caller demands the capabilities it
/// wants.
///
/// A caller demands capabilities ([`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)),
/// and the plan chooses stages that provide them, so nobody writes the
/// sequence of stages by hand. A capability may **refine** another: it is a
/// stronger form of the other one (`flm.blocks.structured` refines
/// `flm.blocks`), and a stage that provides the refinement also satisfies a
/// demand for the parent. At most one refinement of a parent may be marked as
/// its default; a demand for the parent alone selects the default refinement
/// when nothing else decides.
///
/// Every table's name is already a capability, declared together with the
/// table. Declare a capability with this type only for a state that no table
/// represents: a marker, the result of a check, or the parent of
/// refinements. Register the declaration with
/// [`FeatureBuilder::capability`](super::FeatureBuilder::capability).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::CapabilityDef;
///
/// let parent = CapabilityDef::new("flm.blocks").docs("Content is grouped into blocks.");
/// let refined = CapabilityDef::new("flm.blocks.structured")
///     .refines("flm.blocks")
///     .default();
/// assert_eq!(refined.refines, Some(parent.name));
/// assert!(refined.default);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct CapabilityDef {
    /// The capability's dotted name.
    pub name: CapName,
    /// Documentation prose.
    pub docs: String,
    /// The capability this one refines, if any. Some feature of the same
    /// registry must declare it, and a chain of refinements may not lead back
    /// to a capability already in it; the registry build checks both.
    pub refines: Option<CapName>,
    /// Whether this capability is the default refinement of its parent. At
    /// most one refinement of a parent may set it, and setting it on a
    /// capability that refines nothing is a build error
    /// ([`BuildError::DefaultWithoutParent`]).
    pub default: bool,
    /// What a plan error says, after the workflow it names, when a plan needs
    /// this capability and no registered stage provides it, in place of the
    /// generic text: the condition in the words of the feature that declares the
    /// capability, with its fix when the fix is not obvious. `None` keeps the
    /// generic text ([`PlanError::Unsatisfiable`](crate::core::plan::PlanError::Unsatisfiable)).
    pub unprovided: Option<String>,
}

impl CapabilityDef {
    /// A capability called `name`, refining nothing, with no documentation.
    pub fn new(name: impl Into<CapName>) -> CapabilityDef {
        CapabilityDef { name: name.into(), docs: String::new(), refines: None, default: false, unprovided: None }
    }

    /// Sets what a plan error says when no registered stage provides this
    /// capability ([`unprovided`](CapabilityDef::unprovided)). Returns `self`,
    /// for chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::registry::CapabilityDef;
    ///
    /// let checked = CapabilityDef::new("example.checked")
    ///     .unprovided("no feature checks the examples; use a package that does");
    /// assert!(checked.unprovided.is_some());
    /// ```
    pub fn unprovided(mut self, text: impl Into<String>) -> CapabilityDef {
        self.unprovided = Some(text.into());
        self
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> CapabilityDef {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Makes this capability a refinement of `parent`, replacing any parent
    /// set before. Returns `self`, for chaining.
    pub fn refines(mut self, parent: impl Into<CapName>) -> CapabilityDef {
        self.refines = Some(parent.into());
        self
    }

    /// Marks this capability as its parent's default refinement. Returns
    /// `self`, for chaining. Call [`refines`](CapabilityDef::refines) as well:
    /// a default refinement without a parent is a build error.
    pub fn default(mut self) -> CapabilityDef {
        self.default = true;
        self
    }
}

/// A collection declaration. A **collection** is a name for a set of
/// capabilities that any feature may add to, such as the standard package's
/// `flm.validated` ([`VALIDATED`](crate::stdcore::VALIDATED)), which every
/// check of the standard features joins.
///
/// Demanding a collection demands every one of its members. A feature uses a
/// collection to join a set of demands it does not own: it declares its own
/// check capability and adds it as a member with
/// [`FeatureBuilder::collection_member`](super::FeatureBuilder::collection_member),
/// and a caller that demands the collection then runs that check too.
///
/// Collections share one namespace with capabilities and tables, so a
/// collection's name may not be a capability's or a table's. A collection can
/// be demanded but never provided: a stage that lists a collection in its
/// `provides` is a build error ([`BuildError::CollectionAsCapability`]).
/// Register the declaration with
/// [`FeatureBuilder::collection`](super::FeatureBuilder::collection).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::CollectionDef;
///
/// let def = CollectionDef::new("flm.validated").docs("Every registered check has run.");
/// assert_eq!(def.name, "flm.validated");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct CollectionDef {
    /// The collection's name, in the namespace that capabilities and tables
    /// share.
    pub name: CapName,
    /// Documentation prose.
    pub docs: String,
}

impl CollectionDef {
    /// A collection called `name`, with no documentation. Any feature adds
    /// members separately, with
    /// [`FeatureBuilder::collection_member`](super::FeatureBuilder::collection_member).
    pub fn new(name: impl Into<CapName>) -> CollectionDef {
        CollectionDef { name: name.into(), docs: String::new() }
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> CollectionDef {
        self.docs = crate::kept_docs(docs);
        self
    }
}

/// An argument role declaration. An **argument role** is a registered dotted
/// name for what an argument of a command holds, beyond how the argument is
/// written: `flm.refs.target` for the label a reference points to,
/// `flm.graphics.path` for the path of an image file.
///
/// The syntax of an argument says how it is read (a group of characters, raw
/// text up to a closing brace); its role says what the text is. Two commands
/// whose arguments hold the same kind of thing set the same role on them
/// ([`ArgDef::role`](crate::lang::mint::ArgDef::role)), so that a program
/// that describes the language can say so. Parsing and processing do not
/// read roles, and they are not inputs of the [registry
/// digest](crate::model::digest).
///
/// Argument roles have a namespace of their own. A role's name follows the
/// naming rules of every registry name: dotted, its first segment the owner
/// (`flm` for the standard features, an extension's stem for an
/// extension's). The standard features name theirs `flm.<feature>.<thing>`
/// (`flm.refs.target`). Register the declaration with
/// [`FeatureBuilder::argument_role`](super::FeatureBuilder::argument_role);
/// a feature that sets another feature's role on its arguments declares the
/// role as a peer when that feature may be absent
/// ([`FeatureBuilder::peer_argument_role`](super::FeatureBuilder::peer_argument_role)).
/// The frozen registry lists the roles
/// ([`FlmRegistry::argument_roles`](super::FlmRegistry::argument_roles)).
///
/// The serialized form (`Serialize`, with the fields as keys) is read by
/// `flm-info`'s description of the registry and is versioned by its
/// `meta.format`.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::ArgumentRoleDef;
///
/// let def = ArgumentRoleDef::new("flm.refs.target", "A label that a reference points to.");
/// assert_eq!(def.name, "flm.refs.target");
/// ```
// See decision rationale at [§flm-dd-dr:arg-role].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ArgumentRoleDef {
    /// The role's dotted name, unique among the argument roles of a registry.
    pub name: Name,
    /// Documentation prose: what an argument of this role holds, written for
    /// a person who reads the registry's declarations.
    pub docs: String,
}

impl ArgumentRoleDef {
    /// The argument role called `name`, described by `docs`.
    pub fn new(name: impl Into<Name>, docs: impl Into<String>) -> ArgumentRoleDef {
        ArgumentRoleDef { name: name.into(), docs: crate::kept_docs(docs) }
    }
}

/// Ordering edges placed on a stage: the stage runs after the stages that
/// provide the capabilities in `after`, and before those that provide the
/// capabilities in `before`.
///
/// A stage normally gets its place in the order from what it requires and
/// reads. Ordering edges serve a feature whose stage must run before or after
/// another feature's work without needing that work's results. Any feature may
/// place edges on any registered stage, its own or another feature's, in any
/// order of registration.
///
/// Edges decide order only. An edge never makes a stage require a capability
/// and never causes a stage to be scheduled: it constrains when a stage runs,
/// if the plan runs it at all. In an edge, a capability that has refinements
/// means whichever of its refinements the plan selected.
///
/// Register the edges with
/// [`FeatureBuilder::ordering`](super::FeatureBuilder::ordering).
///
/// # Examples
///
/// ```
/// use flm_core::model::names::CapName;
/// use flm_core::core::registry::OrderingDef;
///
/// let edges = OrderingDef::new("flm.refs.resolve")
///     .after("flm.headings.numbered")
///     .before("flm.index.built");
/// assert_eq!(edges.after, [CapName::from("flm.headings.numbered")]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingDef {
    /// The stage these edges apply to. Some feature of the registry must
    /// register a stage of this name.
    pub stage: StageName,
    /// The stage runs after the stage that provides each of these
    /// capabilities.
    pub after: Vec<CapName>,
    /// The stage runs before the stage that provides each of these
    /// capabilities.
    pub before: Vec<CapName>,
}

impl OrderingDef {
    /// An empty set of edges on the stage `stage`.
    pub fn new(stage: impl Into<StageName>) -> OrderingDef {
        OrderingDef { stage: stage.into(), after: Vec::new(), before: Vec::new() }
    }

    /// Adds an edge that places the stage after the stage providing
    /// `capability`. Returns `self`, for chaining.
    pub fn after(mut self, capability: impl Into<CapName>) -> OrderingDef {
        self.after.push(capability.into());
        self
    }

    /// Adds an edge that places the stage before the stage providing
    /// `capability`. Returns `self`, for chaining.
    pub fn before(mut self, capability: impl Into<CapName>) -> OrderingDef {
        self.before.push(capability.into());
        self
    }
}

/// A backend declaration. A **backend** is the registration bundle for one
/// output format: the elements its emitter writes natively, a lowering stage,
/// an emit stage and a final stage.
///
/// The three stages are ordinary stages of the declaring feature, scheduled
/// like any other; this declaration groups them so that a plan recognizes them
/// as one backend:
///
/// - the **lowering** stage rewrites the frozen trees into presentation trees,
///   made of the elements this backend writes, and writes them to the table
///   `flm.lowered.<backend>` (see [backends, lowering, and
///   outputs](crate::guide::concepts_overview#backends-lowering-and-outputs));
/// - the **emit** stage turns the presentation trees into the output's bytes,
///   in the table `flm.output.<backend>`;
/// - the **final** stage (see [output regions and the final
///   stage](crate::guide::concepts_overview#output-regions-and-the-final-stage))
///   assembles the artifact set the caller receives.
///
/// The **native set** (`native`) lists the elements the emit stage writes
/// directly. Lowering rules turn every other element into these. When a plan
/// is built, an element that may occur in a document and that no chain of
/// lowering rules leads into the native set is refused, unless the plan
/// request accepts it as unrenderable (for a workflow, with
/// [`WorkflowDef::unsupported`]). The native set must also satisfy every
/// [native-set implication](NativeImplication) of the registry (an element
/// every backend must write natively, or one a backend must write natively
/// because of another element it writes natively); otherwise the registry
/// build fails with [`BuildError::NativeSetIncomplete`].
///
/// Register the declaration with
/// [`FeatureBuilder::backend`](super::FeatureBuilder::backend). See
/// [`OutputDef`] for how a caller asks for what a backend produces.
#[derive(Clone)]
pub struct BackendDef {
    /// The backend's name; it is also the last segment of the names of its
    /// tables (`flm.lowered.<backend>`, `flm.output.<backend>`).
    pub name: BackendName,
    /// The native set: the elements this backend's emit stage writes
    /// directly, namely the presentation elements it understands and the
    /// structural elements it writes without a lowering rule.
    pub native: Vec<ElementName>,
    /// The lowering stage, which builds the presentation trees.
    pub lowering: Arc<dyn Stage>,
    /// The emit stage, which turns the presentation trees into bytes.
    pub emit: Arc<dyn Stage>,
    /// The final stage, which assembles the artifact set.
    pub final_stage: Arc<dyn Stage>,
    /// Documentation prose.
    pub docs: String,
}

impl BackendDef {
    /// A backend called `name`, whose emit stage writes the elements of
    /// `native` directly, with its lowering, emit and final stages, and with
    /// no documentation.
    pub fn new(
        name: impl Into<BackendName>,
        native: impl IntoIterator<Item = impl Into<ElementName>>,
        lowering: Arc<dyn Stage>,
        emit: Arc<dyn Stage>,
        final_stage: Arc<dyn Stage>,
    ) -> BackendDef {
        BackendDef {
            name: name.into(),
            native: native.into_iter().map(Into::into).collect(),
            lowering,
            emit,
            final_stage,
            docs: String::new(),
        }
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> BackendDef {
        self.docs = crate::kept_docs(docs);
        self
    }
}

impl fmt::Debug for BackendDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendDef")
            .field("name", &self.name)
            .field("native", &self.native)
            .finish_non_exhaustive()
    }
}

/// What is known about an output's entry artifact before anything is
/// rendered: its media type and its file extension.
///
/// A run may produce several artifacts for one output (for example a page and
/// the images it shows); the **entry artifact** is the one that represents the
/// output as a whole ([`ArtifactSet`](crate::core::artifact::ArtifactSet)).
/// With this value, a caller can choose a file name and a content type from
/// the registry alone, before running anything.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct OutputEntry {
    /// The entry artifact's media type, such as `text/plain`.
    pub media_type: String,
    /// The entry artifact's file extension, without a dot, such as `txt`.
    pub extension: String,
}

/// An output declaration. An **output** is a name a caller asks for to have
/// something produced, such as `text` or `html`, together with how to produce
/// it.
///
/// An output ties three things together: the capability a plan must reach
/// for the output to exist, the table that holds the produced artifacts, and
/// the description of its entry artifact. A [`WorkflowDef`] lists outputs by
/// name, and demanding an output means demanding its capability.
///
/// An output may be made from the files of other outputs: a stage that
/// provides it reads their artifact sets, as an output of PDF files is made by
/// compiling the files of a LaTeX output. The declaration lists those outputs
/// in [`made_from`](OutputDef::made_from), so that a host preparing the output
/// prepares them as well.
///
/// The registry build checks three requirements. The capability must be the
/// table's own capability or a refinement of it, so that reaching the
/// capability guarantees that the table is written
/// ([`BuildError::OutputCapabilityMismatch`]). The table must hold an
/// [artifact set](crate::core::artifact::ArtifactSet)
/// ([`BuildError::OutputTableNotArtifactSet`]). Each output listed in
/// [`made_from`](OutputDef::made_from) must be declared by a feature
/// ([`BuildError::Undeclared`]).
///
/// Register the declaration with
/// [`FeatureBuilder::output`](super::FeatureBuilder::output).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::OutputDef;
///
/// let def = OutputDef::new("text", "flm.artifacts.text", "flm.artifacts.text", "text/plain", "txt")
///     .docs("Plain text, one file.");
/// assert_eq!(def.entry.extension, "txt");
///
/// let pdf = OutputDef::new("pdf", "my.pdf", "my.pdf", "application/pdf", "pdf")
///     .made_from("latex-rendered");
/// assert_eq!(pdf.made_from[0].as_str(), "latex-rendered");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct OutputDef {
    /// The output's name, as a caller writes it.
    pub name: OutputName,
    /// The capability demanded to produce this output: the table's own
    /// capability, or a refinement of it.
    pub capability: CapName,
    /// The name of the table that holds the output's artifacts, as an
    /// artifact set.
    pub table: CapName,
    /// The media type and file extension of the entry artifact.
    pub entry: OutputEntry,
    /// The outputs whose files a stage providing this output reads to make it,
    /// in the order they were added; empty for an output made from the
    /// document alone.
    ///
    /// The list tells a host which other outputs the output depends on before
    /// any plan is built: a host that sets up something per output for a
    /// request, such as the compose template that assembles an output's main
    /// file, sets it up for each output the requested outputs are made from,
    /// and for each output those are made from in turn. The stage itself
    /// declares its read of the other output's table among its needs, which is
    /// what schedules the stages that write it. The registry build refuses a
    /// name that no feature declares as an output ([`BuildError::Undeclared`]).
    pub made_from: Vec<OutputName>,
    /// Documentation prose.
    pub docs: String,
}

impl OutputDef {
    /// An output called `name`, produced by demanding `capability`, whose
    /// artifacts are held in the table `table` and whose entry artifact has
    /// the media type `media_type` and the file extension `extension`
    /// (without a dot); made from no other output, with no documentation.
    pub fn new(
        name: impl Into<OutputName>,
        capability: impl Into<CapName>,
        table: impl Into<CapName>,
        media_type: impl Into<String>,
        extension: impl Into<String>,
    ) -> OutputDef {
        OutputDef {
            name: name.into(),
            capability: capability.into(),
            table: table.into(),
            entry: OutputEntry { media_type: media_type.into(), extension: extension.into() },
            made_from: Vec::new(),
            docs: String::new(),
        }
    }

    /// Adds `output` after the outputs already listed in
    /// [`made_from`](OutputDef::made_from): the output's stage reads the files
    /// of `output` to make its own. Returns `self`, for chaining.
    pub fn made_from(mut self, output: impl Into<OutputName>) -> OutputDef {
        self.made_from.push(output.into());
        self
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> OutputDef {
        self.docs = crate::kept_docs(docs);
        self
    }
}

/// The lowering rules a plan prefers on one backend, where several rules can
/// lower the same element.
///
/// The rules are listed most preferred first. A rule not listed is still
/// available: the list only decides between rules that compete for one
/// element, and where it says nothing the plan follows the rest of the rule
/// selection order ([`SelectedRules`](crate::core::plan::SelectedRules)).
///
/// A selection belongs to a plan, not to the registry: a [`WorkflowDef`] holds
/// one per backend, and a caller may replace it in its plan request.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct LoweringSelection {
    /// The preferred rules, most preferred first.
    pub prefer: Vec<RuleName>,
}

impl LoweringSelection {
    /// A selection that prefers no rule.
    pub fn new() -> LoweringSelection {
        LoweringSelection::default()
    }

    /// Adds `rule` after the rules already listed, as less preferred than
    /// them. Returns `self`, for chaining.
    pub fn prefer(mut self, rule: impl Into<RuleName>) -> LoweringSelection {
        self.prefer.push(rule.into());
        self
    }
}

/// A workflow declaration. A **workflow** is a named set of demands together
/// with the choices that go with them: which outputs to produce, which other
/// capabilities to demand, and how to make the choices a plan has to make.
///
/// A workflow is data held in the registry, so a caller chooses one by name
/// instead of writing a plan request by hand (the `flm` program's option
/// `-p html` does so, the profile `html` running the workflow `html`). A plan request built from a workflow demands the
/// capability of each listed output and each extra capability, and carries the
/// workflow's stage preferences, lowering selections, elements accepted as
/// unrenderable and render option values.
///
/// A workflow may extend another (`extends`). The fields are then read along
/// the chain from the workflow that extends none towards this one, and each
/// field is taken from the nearest workflow along the chain that declares it
/// (see [`replace_declared_fields`](WorkflowDef::replace_declared_fields)): a
/// workflow that lists outputs replaces the inherited list instead of adding to
/// it, and a workflow that lists none inherits the whole list. A chain may not
/// lead back to a workflow already in it ([`BuildError::ExtendsCycle`]).
///
/// Register the declaration with
/// [`FeatureBuilder::workflow`](super::FeatureBuilder::workflow). A feature can
/// also change some fields of a workflow that another feature may declare, with
/// [`FeatureBuilder::workflow_fields`](super::FeatureBuilder::workflow_fields).
/// [`FlmRegistry::combined_workflow`](super::FlmRegistry::combined_workflow)
/// returns a workflow with the fields it inherits.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::{LoweringSelection, WorkflowDef};
///
/// let base = WorkflowDef::new("check").demand("flm.validated");
/// let text = WorkflowDef::new("text")
///     .extends("check")
///     .output("text")
///     .lowering("text", LoweringSelection::new().prefer("flm.heading.lower.underline"));
/// assert_eq!(text.extends.as_deref().map(|n| n.as_str()), Some("check"));
/// ```
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct WorkflowDef {
    /// The workflow's name, as a caller writes it.
    pub name: WorkflowName,
    /// The workflow this one extends, if any: the workflow whose fields this
    /// one inherits where it does not declare its own.
    pub extends: Option<WorkflowName>,
    /// The outputs to produce; a plan built from the workflow demands the
    /// capability of each.
    pub outputs: Vec<OutputName>,
    /// The capabilities demanded besides those of the outputs, typically the
    /// checks to run as well. A collection may be listed here.
    pub demands: Vec<CapName>,
    /// The stages preferred where several stages could provide the same
    /// capability, most preferred first.
    pub prefer: Vec<StageName>,
    /// The lowering rules preferred on each backend.
    pub lowering: BTreeMap<BackendName, LoweringSelection>,
    /// For each backend, the elements this workflow accepts as unrenderable:
    /// elements with no lowering into the backend's native set. The plan is
    /// built anyway, and each occurrence of such an element is lowered to a
    /// `flm.pres.unsupported` node (see
    /// [`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported)).
    pub unsupported: BTreeMap<BackendName, Vec<ElementName>>,
    /// The option values this workflow sets on the render request. The
    /// registry build refuses a value whose option type no feature declares or
    /// cannot be set on a render request (see
    /// [`Bind`](crate::model::options::Bind)).
    #[serde(skip)]
    pub render_options: OptionValues,
    /// Documentation prose.
    pub docs: String,
}

impl WorkflowDef {
    /// A workflow called `name` that extends nothing and declares no field.
    pub fn new(name: impl Into<WorkflowName>) -> WorkflowDef {
        WorkflowDef {
            name: name.into(),
            extends: None,
            outputs: Vec::new(),
            demands: Vec::new(),
            prefer: Vec::new(),
            lowering: BTreeMap::new(),
            unsupported: BTreeMap::new(),
            render_options: OptionValues::new(),
            docs: String::new(),
        }
    }

    /// Makes this workflow extend `workflow`, so that it inherits every field
    /// it does not declare. Returns `self`, for chaining.
    pub fn extends(mut self, workflow: impl Into<WorkflowName>) -> WorkflowDef {
        self.extends = Some(workflow.into());
        self
    }

    /// Adds an output to produce, after those already listed. Returns `self`,
    /// for chaining.
    pub fn output(mut self, output: impl Into<OutputName>) -> WorkflowDef {
        self.outputs.push(output.into());
        self
    }

    /// Adds a capability (or a collection) to demand besides those of the
    /// outputs. Returns `self`, for chaining.
    pub fn demand(mut self, capability: impl Into<CapName>) -> WorkflowDef {
        self.demands.push(capability.into());
        self
    }

    /// Adds a preferred stage, after those already listed, as less preferred
    /// than them. Returns `self`, for chaining.
    pub fn prefer(mut self, stage: impl Into<StageName>) -> WorkflowDef {
        self.prefer.push(stage.into());
        self
    }

    /// Sets the lowering selection for the backend `backend`, replacing any
    /// selection already set for it. Returns `self`, for chaining.
    pub fn lowering(
        mut self,
        backend: impl Into<BackendName>,
        selection: LoweringSelection,
    ) -> WorkflowDef {
        self.lowering.insert(backend.into(), selection);
        self
    }

    /// Accepts `element` as unrenderable on the backend `backend`: when the
    /// element has no lowering into the backend's native set, the plan is
    /// built anyway and each occurrence is lowered to a
    /// `flm.pres.unsupported` node. Returns `self`, for chaining.
    pub fn unsupported(
        mut self,
        backend: impl Into<BackendName>,
        element: impl Into<ElementName>,
    ) -> WorkflowDef {
        self.unsupported.entry(backend.into()).or_default().push(element.into());
        self
    }

    /// Sets the option values this workflow sets on the render request,
    /// replacing any values already set. Returns `self`, for chaining.
    pub fn render_options(mut self, options: OptionValues) -> WorkflowDef {
        self.render_options = options;
        self
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> WorkflowDef {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Replaces each field of this workflow that `other` declares by `other`'s
    /// value, and keeps this workflow's name and every field that `other`
    /// does not declare.
    ///
    /// A field is declared when it is set: `extends` given, a list or a map
    /// not empty, `docs` not empty. This is the rule by which a workflow
    /// replaces the fields of the workflow it extends, and by which the fields
    /// registered with
    /// [`FeatureBuilder::workflow_fields`](super::FeatureBuilder::workflow_fields)
    /// replace those of a workflow.
    pub fn replace_declared_fields(&mut self, other: WorkflowDef) {
        let WorkflowDef { name: _, extends, outputs, demands, prefer, lowering, unsupported, render_options, docs } =
            other;
        if extends.is_some() {
            self.extends = extends;
        }
        if !outputs.is_empty() {
            self.outputs = outputs;
        }
        if !demands.is_empty() {
            self.demands = demands;
        }
        if !prefer.is_empty() {
            self.prefer = prefer;
        }
        if !lowering.is_empty() {
            self.lowering = lowering;
        }
        if !unsupported.is_empty() {
            self.unsupported = unsupported;
        }
        if !render_options.is_empty() {
            self.render_options = render_options;
        }
        if !docs.is_empty() {
            self.docs = docs;
        }
    }
}

/// One part of the content of a style or script definition: the text itself,
/// a file that the host reads, an address that the page loads, or the text a
/// document-scope table holds for the document being written.
///
/// A definition's content is an ordered list of parts ([`StyleDef::content`],
/// [`ScriptDef::content`]), so that one identifier can cover one concern even
/// when the concern needs several parts. For example, the HTML backend's
/// formula renderer is one script definition whose first part is its
/// configuration, given as text, and whose second part is the address its
/// program is loaded from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DefContent {
    /// The content, given directly.
    Inline(String),
    /// A file, written the way the host writes file paths. This library never
    /// opens it; the host does.
    File(String),
    /// An address the page loads, such as a stylesheet link or a script
    /// source. This library never fetches it; it only writes it into the
    /// page.
    Url(String),
    /// The text a document-scope table of the value type [`DefinitionText`]
    /// holds, named here: content known only once the document is read (a
    /// configuration listing what the document uses, a stylesheet computed
    /// per document), written by a stage of the feature that declares the
    /// definition and the table.
    ///
    /// The backend that resolves the definitions reads the table and puts its
    /// text in the part's place, as an [`Inline`](DefContent::Inline) part, so
    /// that nothing after the resolution meets this form; where the table is
    /// absent or empty, the part is empty and the backend reports it. The
    /// form names a table of the registry and is not something a host's
    /// settings can give, so the settings do not read it.
    #[serde(skip_deserializing)]
    #[schemars(skip)]
    Table(CapName),
}

/// The value type of a table that supplies a part of a style or script
/// definition ([`DefContent::Table`]): one text, the part's content for the
/// document being written.
///
/// The feature that declares the definition declares the table under this
/// value type, at document scope, and a stage of its own writes it. A piece
/// that holds a non-empty text replaces what the table held, so the table
/// holds the text of the last stage that wrote one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefinitionText {
    /// The part's text.
    pub text: String,
}

impl DefinitionText {
    /// The table value holding `text`.
    pub fn new(text: impl Into<String>) -> DefinitionText {
        DefinitionText { text: text.into() }
    }
}

impl Table for DefinitionText {
    fn absorb(&mut self, piece: DefinitionText) {
        if !piece.text.is_empty() {
            self.text = piece.text;
        }
    }

    /// The text, as a string.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Str(self.text.clone())
    }
}

/// A style definition: a stylesheet that a feature's output depends on.
///
/// A **style definition** (`StyleDef`) or a **script definition**
/// ([`ScriptDef`]) is a stylesheet or a script that a feature's output depends
/// on, recorded in the registry under an identifier; a lowering rule or an
/// emitter *requires* it by identifier, and the backend that writes the output
/// decides its *delivery*: written into the output, loaded from an address, or
/// left to the host.
///
/// A **requirement** is an identifier that a lowering rule or an emitter
/// records to say that its output needs a definition. Two features that
/// require the same definition therefore require it once, because both use
/// its identifier. A backend that has no use for a stylesheet, such as the
/// plain-text backend, records the requirements and reads none.
///
/// Register a style definition with
/// [`FeatureBuilder::style_def`](super::FeatureBuilder::style_def); the
/// finished registry lists them in
/// [`FlmRegistry::styles`](super::FlmRegistry::styles). See also [style and
/// script
/// definitions](crate::guide::concepts_overview#style-and-script-definitions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDef {
    /// The definition's identifier, the one a requirement refers to; two
    /// definitions with the same identifier are the same stylesheet.
    pub id: Name,
    /// The stylesheet, as an ordered list of parts ([`DefContent`]). An output
    /// that includes the definition writes the parts in this order.
    pub content: Vec<DefContent>,
    /// Documentation prose.
    pub docs: String,
}

/// A script definition: a script that a feature's output depends on, recorded
/// in the registry under an identifier; the counterpart of [`StyleDef`] for
/// scripts, with the same fields and the same rules.
///
/// Register a script definition with
/// [`FeatureBuilder::script_def`](super::FeatureBuilder::script_def); the
/// finished registry lists them in
/// [`FlmRegistry::scripts`](super::FlmRegistry::scripts), and the backend that
/// writes the output resolves the requirements against them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptDef {
    /// The definition's identifier, the one a requirement refers to; two
    /// definitions with the same identifier are the same script.
    pub id: Name,
    /// The script, as an ordered list of parts ([`DefContent`]). An output
    /// that includes the definition writes the parts in this order, so a
    /// configuration can be written before the program it configures.
    pub content: Vec<DefContent>,
    /// Documentation prose.
    pub docs: String,
}

/// A build hook that runs in the registry build's second registration round,
/// after every feature has registered, so that a feature can declare things
/// that depend on what other features contributed to service slots.
///
/// Registration has exactly two rounds. In the first round,
/// [`FlmFeature::register`](super::FlmFeature::register), each feature
/// declares what it can declare by itself. Between the rounds, the build
/// assembles the service slots from the contributions (see [services and service
/// slots](crate::guide::concepts_overview#services-and-service-slots)). In the
/// second round, each finalizer reads the assembled services through
/// [`FeatureBuilder::services`](super::FeatureBuilder::services) and declares
/// further entities (an element, a stage, a lowering rule) derived from what it
/// finds.
///
/// The service slots are complete by then, so a finalizer may not declare a
/// service slot, contribute to one, require one, or register another
/// finalizer; the builder refuses each of these with
/// [`BuildError::ContributionAfterAssembly`]. There are exactly two rounds,
/// instead of rounds repeated until nothing changes, so that the build always
/// ends and its order is easy to follow.
///
/// Register a finalizer with
/// [`FeatureBuilder::finalizer`](super::FeatureBuilder::finalizer).
pub trait Finalizer: Send + Sync {
    /// Declares, by calling methods of `b`, what this feature derives from the
    /// assembled services.
    ///
    /// # Errors
    ///
    /// Return a [`BuildError`] to refuse the build. The build records it
    /// attributed to the feature that registered this finalizer (an error
    /// other than [`BuildError::Feature`] and
    /// [`BuildError::ContributionAfterAssembly`] is wrapped in a
    /// [`BuildError::Feature`]), runs the other finalizers, and then fails at
    /// the end of the finalize phase.
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError>;
}

/// The error a [`LateInit`] hook returns when the hook itself cannot do its
/// work.
///
/// A problem with the configuration the hook prepares (for example, a template
/// that does not parse) is not a failure of the hook: the hook writes it to
/// the build diagnostics instead (see [`LateInit::late_init`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateInitError {
    /// What failed, as text for a person to read.
    pub detail: String,
}

impl LateInitError {
    /// An error whose text is `detail`.
    pub fn new(detail: impl Into<String>) -> LateInitError {
        LateInitError { detail: detail.into() }
    }
}

impl fmt::Display for LateInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for LateInitError {}

/// A build hook that runs once the language that documents are parsed with
/// ([`FlmRegistry::lang`](super::FlmRegistry::lang)) is built, for the work a
/// feature can do only with the finished language.
///
/// The usual work is compiling templates. A feature configured with
/// author-written definitions cannot parse them while the language is still
/// being built, because a definition may use commands that other features
/// declare. This hook runs after the language is complete, so each template is
/// parsed once, when the registry is built, instead of once per use.
///
/// The hook keeps two kinds of problem apart. A problem with the
/// *configuration* it prepares (a template that does not parse, or a warning
/// about one) is not a failure of the hook: the hook writes it to `diags`, and
/// the finished registry holds it among its **build diagnostics**
/// ([`FlmRegistry::build_diagnostics`](super::FlmRegistry::build_diagnostics)),
/// which the embedding program can report once per build. The registry is
/// still built. (The user-definitions feature, for example, reports a refused
/// definition again wherever a document uses it.) Returning `Err` means that the hook itself could not do its work, and the
/// registry build fails.
///
/// Register a hook with
/// [`FeatureBuilder::late_init`](super::FeatureBuilder::late_init).
pub trait LateInit: Send + Sync {
    /// Prepares what needs the finished language `lang`, and writes every
    /// problem with the configuration to `diags`.
    ///
    /// # Errors
    ///
    /// A [`LateInitError`] when the hook itself fails. The build records it as
    /// a [`BuildError::LateInit`] attributed to the feature that registered
    /// the hook, runs the other hooks, and then fails at the end of the
    /// late-initialization phase.
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError>;
}

/// A native-set implication: a requirement on what a backend's native set must
/// contain, checked against every backend when the registry is built.
///
/// A backend's *native set* is the list of elements its emit stage writes
/// directly ([`BackendDef::native`]). Lowering rules turn every other element
/// into those elements, and plan construction follows the rules' `produces`
/// declarations to check that the backend can write everything a document may
/// contain. Two elements are outside that check, because the lowering driver
/// creates their nodes itself, with no rule declaring them:
/// `flm.pres.deco-mark`, the node left where a decoration was moved out of a
/// content run into an attached slot (see [decorations, roles and
/// kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)), and
/// `flm.pres.unsupported`, the stand-in node for an element accepted as
/// unrenderable or for an element whose rule failed. A backend must still
/// write these elements natively. Without a declared requirement, a backend
/// that omits one would fail only at lowering, with a missing-rule error, at
/// the first document that needs the element. A native-set implication states
/// the requirement as data: the registry build checks it and fails with
/// [`BuildError::NativeSetIncomplete`], which identifies the backend, the
/// element it must also write natively and, where there is one, the native
/// element that made the implication apply.
///
/// An implication takes one of two forms, given by its
/// [`NativeImplicationKind`]:
///
/// - [`always`](NativeImplication::always): every backend of the registry must
///   write the element natively.
/// - [`aspect::<A>`](NativeImplication::aspect): a backend whose native set
///   holds any element carrying the aspect `A` must write the element natively
///   too. The aspect is given by its trait-object type, exactly as it is
///   attached with [`FeatureBuilder::attach`](super::FeatureBuilder::attach),
///   so the check compares the aspect's identity, not element names: every
///   element that carries the aspect counts, whichever feature registered it,
///   and the check runs after the aspects that features attach to other
///   features' elements have been merged in.
///
/// The `lowering` feature of the standard package declares the two
/// implications described above. A feature whose lowering driver, emitter or
/// rules create nodes of an element without declaring it in a rule's
/// `produces` should declare an implication for it. Register one with
/// [`FeatureBuilder::native_implication`](super::FeatureBuilder::native_implication);
/// [`FlmRegistry::native_implications`](super::FlmRegistry::native_implications)
/// lists those a registry holds. An implication decides whether a registry
/// builds and never what a run produces, so it is not an input of the registry
/// digest.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::{NativeImplication, NativeImplicationKind};
/// use flm_core::stdcore::backends::pres::{self, Decoration};
///
/// let marker = NativeImplication::aspect::<dyn Decoration>(pres::DECO_MARK)
///     .docs("a backend claiming a decoration must claim the marker left in its place");
/// let stand_in = NativeImplication::always(pres::UNSUPPORTED);
/// assert_eq!(stand_in.kind, NativeImplicationKind::Always);
/// assert!(matches!(marker.kind, NativeImplicationKind::Aspect { .. }));
/// assert_eq!(marker.must_claim, pres::DECO_MARK);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeImplication {
    /// The element a backend must write natively when the implication
    /// applies to it. Some feature of the same registry must declare it.
    pub must_claim: ElementName,
    /// When the implication applies to a backend.
    pub kind: NativeImplicationKind,
    /// Documentation prose: why a backend must write the element natively.
    pub docs: String,
}

/// When a [`NativeImplication`] applies to a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeImplicationKind {
    /// To every backend of the registry.
    Always,
    /// To a backend whose native set holds any element that carries the
    /// aspect identified here.
    Aspect {
        /// The identity of the aspect's trait-object type: `TypeId::of::<A>()`
        /// for an aspect attached as `dyn A`, the same key under which an
        /// element's aspect map holds the aspect.
        id: TypeId,
        /// The aspect's type name, for messages and documentation.
        name: &'static str,
    },
}

impl NativeImplication {
    /// An implication that requires every backend to write `element`
    /// natively, with no documentation.
    pub fn always(element: impl Into<ElementName>) -> NativeImplication {
        NativeImplication {
            must_claim: element.into(),
            kind: NativeImplicationKind::Always,
            docs: String::new(),
        }
    }

    /// An implication that requires a backend whose native set holds any
    /// element carrying the aspect `A` to write `element` natively as well;
    /// with no documentation.
    ///
    /// Write `A` as the aspect's trait-object type, as in
    /// `NativeImplication::aspect::<dyn Decoration>(pres::DECO_MARK)`, with
    /// the same bounds as
    /// [`FeatureBuilder::attach`](super::FeatureBuilder::attach). A concrete
    /// type fails to compile: an aspect is attached under its trait-object
    /// type, so an implication keyed by any other type would match no element
    /// and never apply.
    pub fn aspect<A: ?Sized + Any + Send + Sync>(
        element: impl Into<ElementName>,
    ) -> NativeImplication {
        const {
            assert!(
                size_of::<&A>() == 2 * size_of::<*const ()>(),
                "the aspect must be given as its trait-object type, as in \
                 NativeImplication::aspect::<dyn MyAspect>(…)"
            );
        }
        NativeImplication {
            must_claim: element.into(),
            kind: NativeImplicationKind::Aspect {
                id: TypeId::of::<A>(),
                name: std::any::type_name::<A>(),
            },
            docs: String::new(),
        }
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> NativeImplication {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Whether a backend whose native set holds `element` must satisfy this
    /// implication: always, for the [`Always`](NativeImplicationKind::Always)
    /// form; for the [`Aspect`](NativeImplicationKind::Aspect) form, when
    /// `element` carries the aspect.
    pub fn applies_to(&self, element: &ElementSpec) -> bool {
        match self.kind {
            NativeImplicationKind::Always => true,
            NativeImplicationKind::Aspect { id, .. } => {
                element.aspects.aspect_ids().any(|attached| attached == id)
            }
        }
    }
}

/// The kind and name of the entity a peer declaration refers to.
///
/// A **peer declaration** is a feature's forward declaration of an element,
/// element type, table, service slot, scoped package, argument role or
/// backend that another feature owns,
/// with the element's payload type or the table's value type as the
/// signature; the registry build resolves it after every feature has
/// registered, and a lowering rule that depends on a name nobody declares is
/// set aside (excluded from rule selection, with one build Note).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Why peer declarations exist
///
/// The registry build refuses a reference to an entity that no feature
/// declares. A feature may still ship lowering rules that use entities another
/// feature owns: for example, a content feature's rule that writes its element
/// back as LaTeX is the default rule on the LaTeX backend, builds that
/// backend's presentation elements, and reads a table that the references
/// feature computes. The feature declares each such entity as a peer
/// declaration, so that its own rules may refer to it. A peer declaration
/// registers nothing and counts for the declaring feature alone: another
/// feature that refers to the same entity makes its own declaration.
///
/// # The seven kinds
///
/// | Variant | Entity another feature owns | Declared with |
/// |---|---|---|
/// | [`Backend`](PeerName::Backend) | a backend a rule is the default on | [`FeatureBuilder::peer_backend`](super::FeatureBuilder::peer_backend) |
/// | [`Element`](PeerName::Element) | a presentation element a rule produces; its payload type is the signature | [`FeatureBuilder::peer_element`](super::FeatureBuilder::peer_element) |
/// | [`Table`](PeerName::Table) | a table a rule reads or a stage writes; its value type is the signature | [`FeatureBuilder::peer_table`](super::FeatureBuilder::peer_table) |
/// | [`Slot`](PeerName::Slot) | a service slot the feature contributes a value to | [`FeatureBuilder::peer_slot`](super::FeatureBuilder::peer_slot) |
/// | [`ScopedPackage`](PeerName::ScopedPackage) | a scoped package the feature's environment bodies or arguments refer to | [`FeatureBuilder::peer_scoped_package`](super::FeatureBuilder::peer_scoped_package) |
/// | [`ElementType`](PeerName::ElementType) | an element type of which the feature declares one more value, with the commands that produce it | [`FeatureBuilder::peer_element_value`](super::FeatureBuilder::peer_element_value) |
/// | [`ArgumentRole`](PeerName::ArgumentRole) | an argument role the feature sets on arguments of its own declarations | [`FeatureBuilder::peer_argument_role`](super::FeatureBuilder::peer_argument_role) |
///
/// A **peer contribution** (the `Slot` kind) and a peer element value (the
/// `ElementType` kind) carry something besides a name: a value for the service slot,
/// and an element declaration. A feature **declares an element type** when any
/// of its declarations other than a peer element value creates an element of
/// that name: an [`element`](super::FeatureBuilder::element) declaration, an
/// element of a scoped package, a fallback, a presentation element or a field;
/// that feature's rules and stages serve every value of the type.
///
/// # How the build resolves peer declarations
///
/// When the registry is built, after every feature has registered, each peer
/// declaration is **resolved** against the declarations of all features.
///
/// - **The name is declared.** The declaration resolves to that entity, and
///   the rules that refer to it take part like any other rule. For an element,
///   the payload type must match the signature
///   ([`BuildError::PeerElementMismatch`] otherwise); for a table, the value
///   type must match ([`BuildError::PeerTableMismatch`] otherwise). A
///   contribution fills the service slot; a scoped package is available in the bodies
///   and arguments that refer to it; an element value is kept as a plain
///   [`element`](super::FeatureBuilder::element) declaration would be; an
///   argument role is the declared role, described by its declaration.
/// - **No feature declares the name.** The declaration stays *unresolved*. The
///   build still succeeds, and one Note,
///   [`UnresolvedPeer`](super::UnresolvedPeer), joins the registry's
///   [build diagnostics](super::FlmRegistry::build_diagnostics) for each
///   feature and name (one per service slot or element type, however many values the
///   feature gave for it). What else happens depends on the kind:
///   - every lowering rule of the declaring feature that **depends** on the
///     name is **set aside**: excluded from rule selection and from the check
///     that a backend can write every element, as if it had not been
///     registered, while staying listed on the registry
///     ([`RuleEntry::set_aside`](super::RuleEntry::set_aside),
///     [`FlmRegistry::set_aside_rules`](super::FlmRegistry::set_aside_rules)).
///     A rule depends on a backend it is the default on, on an element it
///     produces, and on a table it reads as a hard dependency (`reads`). A rule
///     that reads the table only optionally (`reads_opt`) is not set aside: it
///     runs and finds the table absent, so it writes what it can without the
///     other feature;
///   - every stage of the declaring feature that writes an unresolved peer
///     table is set aside too, and is not registered at all, since it would
///     have nowhere to write;
///   - a contribution to an unresolved service slot is dropped;
///   - the bodies and arguments that refer to an unresolved scoped package get
///     no definitions from it, so a command the package would define is an
///     unknown command there;
///   - an element value of an unresolved element type is dropped, so its
///     commands are unknown commands;
///   - the arguments that have an unresolved argument role keep it, and the
///     registry holds no description of the role.
///
/// No rule depends on a service slot, a scoped package, an element type or an
/// argument role, so none is set aside for one of these.
///
/// The registry lists every peer declaration, resolved or not, so a
/// misspelled name can be found on any registry that holds the feature:
/// [`FlmRegistry::peer_backends`](super::FlmRegistry::peer_backends),
/// [`FlmRegistry::peer_elements`](super::FlmRegistry::peer_elements),
/// [`FlmRegistry::peer_tables`](super::FlmRegistry::peer_tables),
/// [`FlmRegistry::peer_slots`](super::FlmRegistry::peer_slots),
/// [`FlmRegistry::peer_scoped_packages`](super::FlmRegistry::peer_scoped_packages),
/// [`FlmRegistry::peer_element_values`](super::FlmRegistry::peer_element_values)
/// and
/// [`FlmRegistry::peer_argument_roles`](super::FlmRegistry::peer_argument_roles).
///
/// # Repeated declarations
///
/// A feature may declare one name more than once. A declaration registers
/// nothing, so an identical repeat (the same name and, for an element or a
/// table, the same signature type) is merged into the first declaration, whose
/// documentation text is kept. A function that declares on a feature's behalf
/// can therefore declare every name it needs without checking what the feature
/// declared already. A repeat with another signature is refused when the
/// registry is built, as [`BuildError::Duplicate`]. Two features may each
/// declare the same name, since a declaration counts for its own feature
/// alone.
///
/// # Peer declarations and external tables
///
/// A peer table is not an external table
/// ([`Externals`](crate::core::run::Externals)): an external table is a table
/// that no stage of the run writes, whose value the caller installs when the
/// run is constructed, and the feature that declares it registers it with
/// [`FeatureBuilder::external_table`](super::FeatureBuilder::external_table). A
/// peer declaration refers to an entity inside the pipeline that another
/// feature owns.
///
/// Peer declarations are not inputs of the registry digest, except that a
/// resolved peer element value is digested as the element declaration it
/// becomes: whether one resolves follows from which features the registry
/// holds, and the digest already covers those.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
// The serialized kind is the word `PeerName::kind` gives, which the condition
// `flm.registry.unresolved-peer` also carries in its data: one spelling of
// each kind.
#[serde(tag = "kind", content = "name", rename_all = "lowercase")]
pub enum PeerName {
    /// A backend, declared as a peer with
    /// [`FeatureBuilder::peer_backend`](super::FeatureBuilder::peer_backend).
    Backend(BackendName),
    /// A presentation element, declared as a peer with
    /// [`FeatureBuilder::peer_element`](super::FeatureBuilder::peer_element).
    Element(ElementName),
    /// A table, declared as a peer with
    /// [`FeatureBuilder::peer_table`](super::FeatureBuilder::peer_table).
    Table(CapName),
    /// A service slot, given by the contribution a feature makes to it with
    /// [`FeatureBuilder::peer_slot`](super::FeatureBuilder::peer_slot). No
    /// rule depends on a service slot, so this kind never appears in a rule's
    /// [`set_aside`](super::RuleEntry::set_aside) list.
    Slot(Name),
    /// A scoped package, declared as a peer with
    /// [`FeatureBuilder::peer_scoped_package`](super::FeatureBuilder::peer_scoped_package).
    /// No rule depends on a scoped package, so this kind never appears in a
    /// rule's [`set_aside`](super::RuleEntry::set_aside) list.
    #[serde(rename = "scoped package")]
    ScopedPackage(Name),
    /// An element type, given by the element value a feature declares of it
    /// with
    /// [`FeatureBuilder::peer_element_value`](super::FeatureBuilder::peer_element_value).
    /// No rule depends on an element type through this declaration, so this
    /// kind never appears in a rule's
    /// [`set_aside`](super::RuleEntry::set_aside) list.
    #[serde(rename = "element type")]
    ElementType(ElementName),
    /// An argument role, declared as a peer with
    /// [`FeatureBuilder::peer_argument_role`](super::FeatureBuilder::peer_argument_role).
    /// No rule depends on an argument role, so this kind never appears in a
    /// rule's [`set_aside`](super::RuleEntry::set_aside) list.
    #[serde(rename = "argument role")]
    ArgumentRole(Name),
}

impl PeerName {
    /// The word a message uses for this kind of entity: `backend`, `element`,
    /// `table`, `slot`, `scoped package`, `element type` or `argument role`.
    pub fn kind(&self) -> &'static str {
        match self {
            PeerName::Backend(_) => "backend",
            PeerName::Element(_) => "element",
            PeerName::Table(_) => "table",
            PeerName::Slot(_) => "slot",
            PeerName::ScopedPackage(_) => "scoped package",
            PeerName::ElementType(_) => "element type",
            PeerName::ArgumentRole(_) => "argument role",
        }
    }

    /// The entity's name, for every kind.
    pub fn name(&self) -> &Name {
        match self {
            PeerName::Backend(backend) => backend.name(),
            PeerName::Element(element) => element.name(),
            PeerName::Table(table) => table.name(),
            PeerName::Slot(slot) => slot,
            PeerName::ScopedPackage(package) => package,
            PeerName::ElementType(element) => element.name(),
            PeerName::ArgumentRole(role) => role,
        }
    }
}

impl fmt::Display for PeerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} `{}`", self.kind(), self.name())
    }
}
