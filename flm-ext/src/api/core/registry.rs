//! Mirrors [`flm_core::core::registry`]: the builder a feature declares its
//! contributions through.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::api::core::stage::{RewriteStage, Stage};
use crate::api::lang::mint::{ElementDef, FallbackDef, ScopedPackageDef};
use crate::api::lang::{BuiltLang, FlmDiagnostics};
use crate::api::model::element::Aspect;
use crate::api::core::lowering::LoweringRule;
use crate::api::core::services::Arity;
use crate::api::stdcore::features::citations::CitationProvider;
use crate::api::stdcore::features::compose::TemplateModule;
use crate::runtime::ExtError;
use crate::sealed::{DurableOwner, Sealed};

pub use flm_core::lang::mint::CallableKind;

/// Mirrors the definitions of [`flm_core::core::registry`] that
/// [`FeatureBuilder::style_def`] and [`FeatureBuilder::script_def`] take:
/// [`StyleDef`], [`ScriptDef`] and their parts, [`DefContent`]. Nothing differs: they
/// are plain data, re-exported.
///
/// A definition is `{ id, content: [parts], docs }`, each part one of
/// `DefContent::Inline` (the text itself), `DefContent::Url` (an address the
/// page loads), `DefContent::Table` (the text a table holds for the document
/// being written) or `DefContent::File`. The first three are the forms an
/// extension uses. A `File` part is written the way the host writes file
/// paths, and the host reads it; the extension's own files are not consulted,
/// so a part an extension ships is given `Inline` (or `Url`).
///
/// A `Table` part names a table of value type `DefinitionText`, declared with
/// [`FeatureBuilder::definition_text_table`] and written by a stage of the
/// feature through [`StageOut::table`](crate::api::core::data::store::StageOut::table)
/// with one string; the registry build fails when the part names a table no
/// feature declares, or a table of another type. The backend that resolves the
/// definition reads the table optionally, and so schedules no writer: the
/// feature that declares the definition schedules the stage that writes the
/// table itself, by a rule's `requires` of the table's capability
/// ([`LoweringDecl::requires`](crate::api::core::lowering::LoweringDecl::requires)),
/// or because that stage writes a table the feature's rules already read.
pub use flm_core::core::registry::{DefContent, ScriptDef, StyleDef};

/// Mirrors [`OutputDef`]: an output, a
/// name a caller asks for (`-f <name>`), tied to the capability a plan must
/// reach, the table holding the output's files (an
/// [`ArtifactSet`](flm_core::core::artifact::ArtifactSet), declared with
/// [`FeatureBuilder::table_named`] and [`StandardTable::ArtifactSet`]), and the
/// media type and file extension of its main file. Nothing differs: it is
/// plain data, re-exported, registered with [`FeatureBuilder::output`].
///
/// An output made from the files of other outputs lists them in
/// [`made_from`](OutputDef::made_from) ([`OutputDef::made_from`] adds one), so
/// that a host prepares those outputs as well: the `flm` program sets up the
/// compose template of each of them. The stage that makes the output declares
/// its read of the other output's table, and reads its files as described in
/// [reading the files of an
/// output](crate::api::core::artifact#reading-the-files-of-an-output).
///
/// Across a binding an output is `{ name, capability, table, media_type,
/// extension, made_from: [name, …], docs }`, `made_from` and `docs` optional.
/// The extension, not the media type, names the file the host writes
/// (`STEM.EXT`).
pub use flm_core::core::registry::OutputDef;

/// Mirrors [`BackendDef`](flm_core::core::registry::BackendDef): a backend,
/// the registration bundle of one output format: its name, the elements its
/// emit stage writes natively, and its lowering, emit and final stages,
/// registered with [`FeatureBuilder::backend`].
///
/// What differs: each stage is a [`BackendStage`], a stage of the library
/// ([`LibraryStage`], the lowering stage
/// [`LoweringStage::new`](crate::api::stdcore::backends::lowering::LoweringStage::new)
/// answers) or an extension's own [`Stage`], where `flm-core` takes three
/// `Arc<dyn Stage>`; the names are `String`s.
///
/// Across a binding a backend is `{ name, native: [name, …], lowering, emit,
/// final_stage, docs }`, each stage an object of the kind `library_stage` or
/// `stage`, `docs` optional.
#[derive(Clone)]
pub struct BackendDef {
    /// Mirrors [`BackendDef::name`](flm_core::core::registry::BackendDef::name):
    /// the backend's name, which users write (`-f <name>`); it is also the
    /// last segment of the names of the library's tables and stages for the
    /// backend (`flm.lowered.<backend>`, `flm.<backend>.lower`). It must be
    /// the extension's stem, or begin with the stem and a dot (an extension
    /// whose stem is `profile-X` may also use `X` in place of its stem).
    pub name: String,
    /// Mirrors [`BackendDef::native`](flm_core::core::registry::BackendDef::native):
    /// the elements the emit stage writes directly, presentation elements
    /// and structural elements
    /// ([`api::lang::structural`](crate::api::lang::structural)) alike. The
    /// registry reports a name no feature declares.
    pub native: Vec<String>,
    /// Mirrors [`BackendDef::lowering`](flm_core::core::registry::BackendDef::lowering):
    /// the lowering stage, which builds the presentation trees, usually the
    /// library's.
    pub lowering: BackendStage,
    /// Mirrors [`BackendDef::emit`](flm_core::core::registry::BackendDef::emit):
    /// the emit stage, which turns the presentation trees into the output.
    pub emit: BackendStage,
    /// Mirrors [`BackendDef::final_stage`](flm_core::core::registry::BackendDef::final_stage):
    /// the final stage, which writes the output's files.
    pub final_stage: BackendStage,
    /// Mirrors [`BackendDef::docs`](flm_core::core::registry::BackendDef::docs).
    pub docs: String,
}

impl BackendDef {
    /// Mirrors [`BackendDef::new`](flm_core::core::registry::BackendDef::new):
    /// the backend `name`, writing the elements `native` directly, with its
    /// three stages and no documentation. What differs: the stages are
    /// [`BackendStage`]s.
    pub fn new(
        name: impl Into<String>,
        native: Vec<String>,
        lowering: BackendStage,
        emit: BackendStage,
        final_stage: BackendStage,
    ) -> BackendDef {
        BackendDef { name: name.into(), native, lowering, emit, final_stage, docs: String::new() }
    }

    /// Mirrors [`BackendDef::docs`](flm_core::core::registry::BackendDef::docs):
    /// sets the documentation prose. Nothing differs.
    pub fn docs(mut self, docs: impl Into<String>) -> BackendDef {
        self.docs = docs.into();
        self
    }
}

impl fmt::Debug for BackendDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendDef").field("name", &self.name).field("native", &self.native).finish_non_exhaustive()
    }
}

/// No flm-core counterpart: one stage of a [`BackendDef`], where `flm-core`
/// takes an `Arc<dyn Stage>` of either origin.
#[derive(Clone)]
pub enum BackendStage {
    /// A stage of the library, such as the lowering stage
    /// [`LoweringStage::new`](crate::api::stdcore::backends::lowering::LoweringStage::new)
    /// answers; registered as it is.
    Library(LibraryStage),
    /// An extension's own stage, as [`FeatureBuilder::stage`] takes it: its
    /// declaration and needs are read when the backend is declared, and it
    /// is handed to `flm-core` in the build's second round, so that each
    /// table it writes has the type its declaration gives.
    Own(Arc<dyn Stage>),
}

impl fmt::Debug for BackendStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendStage::Library(stage) => f.debug_tuple("Library").field(stage).finish(),
            BackendStage::Own(stage) => f.debug_tuple("Own").field(&stage.decl().name).finish(),
        }
    }
}

/// No flm-core counterpart: a stage of the library, held as the
/// `Arc<dyn Stage>` of `flm-core` that a [`BackendDef`] takes. Opaque: only
/// the extension API's constructors make one
/// ([`LoweringStage::new`](crate::api::stdcore::backends::lowering::LoweringStage::new)).
#[derive(Clone)]
pub struct LibraryStage(pub(crate) Arc<dyn flm_core::core::stage::Stage>);

impl fmt::Debug for LibraryStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LibraryStage").field(&self.0.decl().name).finish()
    }
}

/// No flm-core counterpart: the standard table types a scripted feature may
/// declare a table of by name, through [`FeatureBuilder::table_named`]; each
/// variant stands for the `flm-core` type `T` of
/// [`FeatureBuilder::table_named::<T>`](flm_core::core::registry::FeatureBuilder::table_named).
///
/// Across a binding a variant is its name in lower case, words joined by a
/// hyphen (`"lowered-trees"`, `"artifact-set"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardTable {
    /// [`LoweredTrees`](flm_core::stdcore::backends::lowering::LoweredTrees):
    /// a backend's lowered trees, which the library's lowering stage writes and
    /// the backend's emit stage reads
    /// ([`TableValue::Lowered`](crate::api::core::data::table::TableValue::Lowered)).
    /// A scripted stage cannot write it.
    LoweredTrees,
    /// [`LoweredPieces`](flm_core::stdcore::backends::pieces::stage::LoweredPieces):
    /// the lowered pieces a library stage places. A scripted stage cannot
    /// write it, and reads it as its projection.
    LoweredPieces,
    /// [`Regions`](flm_core::core::region::Regions): the regions of an
    /// output, which a library stage writes. A scripted stage cannot write it,
    /// and reads it as its projection.
    Regions,
    /// [`ArtifactSet`](flm_core::core::artifact::ArtifactSet): the files of
    /// an output. A scripted stage that declares the table among its writes
    /// pushes one artifact at a time, in the data form of
    /// [`api::core::artifact`](crate::api::core::artifact). A scripted stage
    /// that declares the table among its reads gets every artifact of the set
    /// in that data form, with the path at which the host writes it
    /// ([`TableValue::Projection`](crate::api::core::data::table::TableValue::Projection);
    /// see [reading the files of an
    /// output](crate::api::core::artifact#reading-the-files-of-an-output)).
    ArtifactSet,
}

/// Mirrors [`FeatureBuilder`](flm_core::core::registry::FeatureBuilder): what a
/// feature's `register` handler declares its contributions through.
///
/// What differs: the subset of methods below; errors are [`ExtError`]. The
/// adapter implements it over the real builder, checking first that every
/// name the feature declares begins with its extension's stem and a dot
/// ([`ExtError::NameOutsideExtension`]). An extension whose stem is
/// `profile-X` may also use `X` in place of its stem, in this rule as each
/// method below states it ([`namespaces`](crate::adapter::names::namespaces)).
///
/// The trait is sealed: the adapter alone implements it.
pub trait FeatureBuilder: Sealed + DurableOwner {
    /// Mirrors [`FeatureBuilder::feature`](flm_core::core::registry::FeatureBuilder::feature):
    /// the name of the feature being registered. What differs: an owned
    /// `String`.
    fn feature(&self) -> String;

    /// Mirrors [`FeatureBuilder::element`](flm_core::core::registry::FeatureBuilder::element):
    /// declares an element and the syntax that produces it. What differs: it
    /// takes the extension API's [`ElementDef`]; its `owner` is overwritten
    /// with the feature's name, as `flm-core` does.
    ///
    /// # Errors
    ///
    /// The element's name does not begin with the extension's stem and a dot, or the builder
    /// refuses the declaration ([`ExtError::Core`]).
    fn element(&mut self, def: ElementDef) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::fallback`](flm_core::core::registry::FeatureBuilder::fallback):
    /// declares the element an unknown macro or environment becomes. What
    /// differs: it takes the extension API's [`FallbackDef`], whose owner is
    /// the builder's feature.
    ///
    /// # Errors
    ///
    /// As [`element`](FeatureBuilder::element).
    fn fallback(&mut self, def: FallbackDef) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::override_callable`](flm_core::core::registry::FeatureBuilder::override_callable):
    /// declares that this feature replaces another feature's macro,
    /// environment or specials named `name`. The override is consulted only
    /// in a parsing mode in which this feature is placed last among the
    /// features that define the name. What differs: `name` is a `&str`.
    ///
    /// # Errors
    ///
    /// The builder refuses the override ([`ExtError::Core`]).
    fn override_callable(&mut self, kind: CallableKind, name: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::table`](flm_core::core::registry::FeatureBuilder::table)
    /// and [`FeatureBuilder::table_named`](flm_core::core::registry::FeatureBuilder::table_named):
    /// declares a table this feature owns. What differs: a scripted table always
    /// holds the adapter's `ExtTable`, so the method is not generic; it answers
    /// nothing, since the name is what a stage declares and writes the table by.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot, or the builder refuses it.
    fn table(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::table_named`](flm_core::core::registry::FeatureBuilder::table_named)
    /// for the standard table types: declares a table this feature owns,
    /// named `name`, holding the `flm-core` type `ty` stands for. What
    /// differs: the type is the value `ty`, one of the four
    /// [`StandardTable`]s, where `flm-core` takes a type parameter; it
    /// answers nothing, since the name is what a stage declares, writes and
    /// reads the table by. The name rule is [`table`](FeatureBuilder::table)'s,
    /// with one more accepted form: `flm.lowered.<backend>`, the table the
    /// library's lowering stage writes for a backend
    /// ([`lowered_table_name`](crate::api::stdcore::backends::lowering::lowered_table_name)),
    /// where `<backend>` is a backend name the package may declare (the
    /// extension's stem, or the stem and a dot, as
    /// [`backend`](FeatureBuilder::backend)'s rule).
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot and is not
    /// of that form, or the builder refuses it.
    fn table_named(&mut self, name: &str, ty: StandardTable, docs: &str) -> Result<(), ExtError>;

    /// Mirrors the table record's
    /// [`refines`](flm_core::core::registry::builder::TableRecord::refines), through
    /// [`FeatureBuilder::table_refining_named`](flm_core::core::registry::FeatureBuilder::table_refining_named):
    /// declares a table this feature owns, as [`table`](FeatureBuilder::table)
    /// does, whose capability refines the capability `parent`, so that the
    /// stage that writes the table provides `parent` too (the table of
    /// rendered formulas of a processing feature refines `flm.math.rendered`).
    /// What differs: the table holds the adapter's `ExtTable`, as every table
    /// of [`table`](FeatureBuilder::table) does.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot, or the builder refuses it.
    /// A `parent` no feature declares is reported when the registry is built.
    fn table_refining(&mut self, name: &str, parent: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::table_named`](flm_core::core::registry::FeatureBuilder::table_named)
    /// with the value type [`DefinitionText`](flm_core::core::registry::DefinitionText):
    /// declares a table this feature owns that holds the text of a part of a
    /// style or script definition ([`DefContent::Table`]). What differs: a
    /// form of its own, since every other table a scripted feature declares
    /// holds the adapter's `ExtTable`. A stage of the feature that declares
    /// the table among its writes writes it through
    /// [`StageOut::table`](crate::api::core::data::store::StageOut::table)
    /// with one string; a later non-empty string replaces an earlier one. The
    /// table and the stage may be declared in either order: the stage's
    /// writes take their types once every feature has registered.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot, or the builder refuses it.
    fn definition_text_table(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::peer_table`](flm_core::core::registry::FeatureBuilder::peer_table):
    /// declares that this feature expects another feature of the registry to
    /// declare the table `name`, so that its rules may read it (a rule that
    /// reads it as a hard dependency is set aside when no feature declares
    /// it; one that reads it optionally finds it absent). What differs: by
    /// name, with no type parameter: the declaration takes the value type the
    /// owning feature declared the table with, once every feature has
    /// registered (in the build's second round, as a rule's reads do), and the
    /// adapter's `ExtTable` when no feature declares the table. The mirror's
    /// table reads are by name and erased
    /// ([`Lowerer::get`](crate::api::core::lowering::Lowerer::get)).
    ///
    /// # Errors
    ///
    /// The builder refuses the declaration ([`ExtError::Core`]).
    fn peer_table(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::peer_backend`](flm_core::core::registry::FeatureBuilder::peer_backend):
    /// declares that this feature expects another feature of the registry to
    /// register the backend `name`, so that its rules may name it in their
    /// default mark; when none does, those rules are set aside. Nothing
    /// differs but the `&str` arguments.
    ///
    /// # Errors
    ///
    /// The builder refuses the declaration ([`ExtError::Core`]).
    fn peer_backend(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::peer_element`](flm_core::core::registry::FeatureBuilder::peer_element):
    /// declares that this feature expects another feature of the registry to
    /// register the presentation element `name`, so that its rules may list it
    /// in their `produces`; when none does, those rules are set aside. What
    /// differs: by name, with no payload type parameter: the declaration takes
    /// the payload type the owning feature registered the element with, once
    /// every feature has registered (in the build's second round).
    ///
    /// # Errors
    ///
    /// The builder refuses the declaration ([`ExtError::Core`]).
    fn peer_element(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::presentation_element`](flm_core::core::registry::FeatureBuilder::presentation_element):
    /// registers a presentation element named `name`, an element that exists
    /// only in the trees a backend writes, whose nodes carry a payload given
    /// as data. What differs: the payload type is the adapter's one data
    /// payload type (a JSON value), where `flm-core` takes a type parameter;
    /// it answers nothing, where `flm-core` answers the element's value: a
    /// rule stages the element by its name
    /// ([`Lowerer::pres`](crate::api::core::lowering::Lowerer::pres), the
    /// payload any data value) when it declares that it produces it
    /// ([`LoweringDecl::produces`](crate::api::core::lowering::LoweringDecl::produces)),
    /// and an emit stage reads the value back from the lowered node
    /// ([`Node::payload`](crate::api::techy::node::Node::payload)). A Rust
    /// rule reads it with
    /// [`payload_of`](flm_core::stdcore::backends::pres::payload_of) as the
    /// adapter's `DataPayload`.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot, or the builder refuses it.
    fn presentation_element(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::backend`](flm_core::core::registry::FeatureBuilder::backend):
    /// registers a backend, its three stages as this feature's own. What
    /// differs: it takes the extension API's [`BackendDef`]; the backend is
    /// handed to `flm-core` once every feature has registered (in the build's
    /// second round, as a stage is), so that each table an extension's own
    /// stage writes has the type its declaration gives. The native names are
    /// not checked here: the registry reports a name no feature declares.
    ///
    /// # Errors
    ///
    /// The backend's name is not the extension's stem and does not begin
    /// with the stem and a dot ([`ExtError::NameOutsideExtension`]); an
    /// extension's own stage's name, or a table it writes, does not begin
    /// with the stem and a dot; or the builder refuses the backend.
    fn backend(&mut self, def: BackendDef) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::output`](flm_core::core::registry::FeatureBuilder::output):
    /// registers an output. What differs: the output's name must be the
    /// extension's stem or begin with the stem and a dot, as a backend's.
    ///
    /// # Errors
    ///
    /// The output's name is outside that rule
    /// ([`ExtError::NameOutsideExtension`]). A table that does not hold an
    /// artifact set, or a capability that is neither the table's nor a
    /// refinement of it, is reported when the registry is built.
    fn output(&mut self, def: OutputDef) -> Result<(), ExtError>;

    /// Sets a workflow's lowering selections through
    /// [`FeatureBuilder::workflow_fields`](flm_core::core::registry::FeatureBuilder::workflow_fields).
    /// The data map is `{ backend: { prefer: [rule, ...] }, ... }`, with
    /// `prefer` optional and empty by default. Rules are most preferred first;
    /// an explicit preference can override a backend's native element handling.
    ///
    /// A nonempty map replaces the workflow's entire `lowering` field, not
    /// individual backends; an empty map changes nothing. Other workflow fields
    /// are untouched. Contributions follow the library's registration order,
    /// with the embedding program's workflow fields applied last. The names
    /// refer to workflows, backends and rules, so need not belong to this extension.
    ///
    /// # Errors
    ///
    /// Malformed data ([`ExtError::Value`]) or the builder's refusal
    /// ([`ExtError::Core`]). The library validates the combined workflow's
    /// referenced names when the registry is built.
    fn workflow_lowering(&mut self, workflow: &str, lowering: &Value) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::style_def`](flm_core::core::registry::FeatureBuilder::style_def):
    /// records a stylesheet the feature's output depends on, which a rule
    /// requires by its identifier
    /// ([`LoweringDecl::selection_requires`](crate::api::core::lowering::LoweringDecl::selection_requires),
    /// [`Lowerer::require`](crate::api::core::lowering::Lowerer::require)).
    /// What differs: the identifier must begin with the extension's stem and a
    /// dot, as every declared name. The parts are [`DefContent`]'s, of which an
    /// extension uses `Inline`, `Url` and `Table` (see [`DefContent`] here).
    ///
    /// # Errors
    ///
    /// The identifier does not begin with the extension's stem and a dot, or the builder refuses
    /// the definition.
    fn style_def(&mut self, def: StyleDef) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::script_def`](flm_core::core::registry::FeatureBuilder::script_def):
    /// the counterpart of [`style_def`](FeatureBuilder::style_def) for
    /// scripts. What differs: as for [`style_def`](FeatureBuilder::style_def).
    ///
    /// # Errors
    ///
    /// As [`style_def`](FeatureBuilder::style_def).
    fn script_def(&mut self, def: ScriptDef) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::stage`](flm_core::core::registry::FeatureBuilder::stage):
    /// declares a stage. What differs: it takes the extension API's
    /// [`Stage`]; its [`decl`](Stage::decl) and [`needs`](Stage::needs) are read
    /// here, once, and the stage is handed to `flm-core` once every feature
    /// has registered (in the build's second round, as a rule is), so that
    /// a table it writes has the type its declaration gives.
    ///
    /// # Errors
    ///
    /// The stage's name, or a table it writes, does not begin with the extension's stem
    /// and a dot, or the builder refuses the stage. A capability the stage provides is
    /// named, not declared, and may be another feature's; the registry's validation
    /// refuses one that no feature declares.
    fn stage(&mut self, stage: Arc<dyn Stage>) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::rewrite_stage`](flm_core::core::registry::FeatureBuilder::rewrite_stage):
    /// declares a rewrite stage, and with it the table of trees it publishes,
    /// named by [`RewriteDecl::publishes`](crate::api::core::stage::RewriteDecl::publishes),
    /// with the refinement and the default mark its declaration gives. What
    /// differs: it takes the extension API's [`RewriteStage`], whose
    /// [`decl`](RewriteStage::decl) and [`needs`](RewriteStage::needs) are
    /// read here, once; the stage is registered at once (its table's type is
    /// its own, a table of trees, so nothing waits for another feature).
    ///
    /// # Errors
    ///
    /// The stage's name, or the name of the table it publishes, does not
    /// begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), or the builder refuses the stage.
    fn rewrite_stage(&mut self, stage: Arc<dyn RewriteStage>) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::capability`](flm_core::core::registry::FeatureBuilder::capability):
    /// declares a capability, a state of the pipeline a stage can provide
    /// ([`StageDecl::provides`](crate::api::core::stage::StageDecl::provides))
    /// and another stage can require. What differs: the capability's name, its
    /// documentation and what a plan error says when no registered stage
    /// provides it are the arguments, where `flm-core` takes a
    /// [`CapabilityDef`](flm_core::core::registry::CapabilityDef) (`unprovided`
    /// is [`CapabilityDef::unprovided`](flm_core::core::registry::CapabilityDef::unprovided),
    /// `None` for the generic text; a script may leave it out); a capability
    /// that refines another cannot be declared yet.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot, or the builder refuses the
    /// capability ([`ExtError::Core`]).
    fn capability(&mut self, name: &str, docs: &str, unprovided: Option<String>) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::argument_role`](flm_core::core::registry::FeatureBuilder::argument_role):
    /// declares the argument role `name`, a registered name for what an
    /// argument holds, described by `docs`; an argument states its role with
    /// [`ArgDef::role`](crate::api::lang::mint::ArgDef::role). What differs:
    /// the name must begin with the extension's stem and a dot, as every
    /// declared name.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), or the builder refuses the role
    /// ([`ExtError::Core`]).
    fn argument_role(&mut self, name: &str, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::peer_argument_role`](flm_core::core::registry::FeatureBuilder::peer_argument_role):
    /// declares that the feature sets the argument role `name`, which another
    /// feature declares, on arguments of its own declarations, so that the
    /// feature still registers in a registry where no feature declares the
    /// role (the registry build then reports one Note). Nothing differs.
    ///
    /// # Errors
    ///
    /// The builder refuses the declaration ([`ExtError::Core`]).
    fn peer_argument_role(&mut self, name: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::lowering_rule`](flm_core::core::registry::FeatureBuilder::lowering_rule):
    /// declares a lowering rule. What differs: the rule is the extension API's
    /// [`LoweringRule`], built with [`LoweringRule::new`] or made by a helper
    /// ([`expand_in_tree_rule_for`](crate::api::stdcore::features::userdefs::expand_in_tree_rule_for));
    /// its name must begin with the extension's stem and a dot. A rule built
    /// with [`LoweringRule::new`] is handed to `flm-core` in the build's second
    /// round, when every feature has declared its tables, so that each table
    /// the rule reads by name is read as the type the registry declares for
    /// it; its body is called through the adapter, which lends it the
    /// lowering façade ([`Lower`](crate::api::core::lowering::Lower)).
    ///
    /// # Errors
    ///
    /// The rule's name does not begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), or the builder refuses the rule
    /// ([`ExtError::Core`]).
    fn lowering_rule(&mut self, rule: LoweringRule) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::require_service_named`](flm_core::core::registry::FeatureBuilder::require_service_named):
    /// declares that the feature needs the service slot `slot` filled. What
    /// differs: the by-name form under the plain name.
    ///
    /// # Errors
    ///
    /// The builder refuses the requirement ([`ExtError::Core`]).
    fn require_service(&mut self, slot: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::service_slot`](flm_core::core::registry::FeatureBuilder::service_slot):
    /// declares the service slot `name`, taking as many values as `arity`
    /// allows. What differs: the slot's values are data (a slot a script
    /// declares holds no other type); no value shape is declared, so a
    /// feature reading the slot checks what it reads.
    ///
    /// # Errors
    ///
    /// The name does not begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), or the builder refuses the
    /// slot ([`ExtError::Core`]).
    fn service_slot(&mut self, name: &str, arity: Arity, docs: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::contribute_to`](flm_core::core::registry::FeatureBuilder::contribute_to):
    /// contributes `value` to the service slot `slot`, after every value
    /// contributed to it so far. What differs: `value` is a [`Contributed`],
    /// whose variant must be what the slot's name calls for: a citation
    /// provider for `flm.citations.providers`, a template module for
    /// `flm.compose.templates` (described here, with the feature's
    /// configuration), data for any other slot; a data value to a slot of
    /// another type fails the registry build, which names the two types.
    ///
    /// Data given to one of the standard data-valued slots is typed at the
    /// call by the slot's name: `flm.numbering.counters` (a counter
    /// declaration `{ name, default_style, docs?, within?, nesting? }`),
    /// `flm.numbering.styles` (a counter style `{ name, number_format?,
    /// sub_levels?, prefix_words?, delimiters?, join?, name_in_link?,
    /// repeat_parents_in_range?, ranges?, number_writer? }`),
    /// `flm.numbering.parent-sets` (`{ name, counters }`),
    /// `flm.latex.preamble` (a definition piece `{ name, content, docs?,
    /// requires? }`) and `flm.latex.packages` (a package definition `{ id,
    /// package, options, docs }`); the forms of the fields are those of the
    /// data types of
    /// [`numbering`](crate::api::stdcore::features::numbering) and
    /// [`latex`](crate::api::stdcore::backends::latex). The value's own name
    /// (`name`, or `id` for a package definition) must begin with the
    /// extension's stem and a dot. Data given to any other slot is
    /// contributed as it is. Like `flm-core`'s, the method is refused in the
    /// build's second round (a [`Finalizer`]).
    ///
    /// # Errors
    ///
    /// The value is not what the slot takes ([`ExtError::Argument`]), data
    /// that cannot cross or does not read as the slot's type
    /// ([`ExtError::Value`]), a citation provider or a standard data-valued
    /// slot's value named outside the extension's stem
    /// ([`ExtError::NameOutsideExtension`]), a template module's `describe`
    /// fails, or the builder refuses the contribution ([`ExtError::Core`]),
    /// such as `ContributionAfterAssembly`.
    fn contribute_to(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::contribute_first_to`](flm_core::core::registry::FeatureBuilder::contribute_first_to):
    /// contributes `value` to the service slot `slot`, before every value
    /// appended to it. Among the values placed first this way, a read for
    /// precedence takes those of the feature placed last in the feature order
    /// first ([`StageCx::services_by_precedence`](crate::api::core::run::StageCx::services_by_precedence)).
    /// What differs: as [`contribute_to`](FeatureBuilder::contribute_to);
    /// the value is data.
    ///
    /// # Errors
    ///
    /// As [`contribute_to`](FeatureBuilder::contribute_to); a value that is
    /// not data ([`ExtError::Argument`]).
    fn contribute_first_to(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::peer_slot`](flm_core::core::registry::FeatureBuilder::peer_slot):
    /// contributes `value` to the service slot `slot` as a peer declaration,
    /// so that the feature still registers in a registry where no feature
    /// declares the slot (the value is then dropped, and the registry build
    /// reports one Note). What differs: as
    /// [`contribute_to`](FeatureBuilder::contribute_to).
    ///
    /// # Errors
    ///
    /// As [`contribute_to`](FeatureBuilder::contribute_to).
    fn peer_slot(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::digest_input`](flm_core::core::registry::FeatureBuilder::digest_input):
    /// feeds `bytes` into the registry digest. Nothing differs.
    ///
    /// # Errors
    ///
    /// The builder refuses the input ([`ExtError::Core`]).
    fn digest_input(&mut self, bytes: &[u8]) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::attach`](flm_core::core::registry::FeatureBuilder::attach):
    /// attaches an aspect to the element `element`, which another feature
    /// may declare. What differs: the element is named by a `&str`, and the
    /// aspect is an [`Aspect`] value, its kind named by its variant rather
    /// than by a type parameter (a block role, an expansion hook, or a
    /// handler aspect, [`Referenceable`](crate::api::model::aspects::Referenceable)
    /// or [`Sectioning`](crate::api::model::aspects::Sectioning)). The
    /// element's name is not checked against the extension's stem: naming
    /// another feature's element is the point.
    ///
    /// # Errors
    ///
    /// The builder refuses the attachment ([`ExtError::Core`]). An element
    /// no feature declares, or an aspect of a kind the element already
    /// carries, is reported when the registry is built, as in `flm-core`.
    fn attach(&mut self, element: &str, aspect: Aspect) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::collection_member`](flm_core::core::registry::FeatureBuilder::collection_member):
    /// adds the capability `member` to the collection `collection`, so that
    /// whatever demands the collection demands `member` too. What differs:
    /// the names are `&str`; `member` must begin with the extension's stem
    /// and a dot, `collection` need not (it is usually another feature's,
    /// such as `flm.validated`). A member no feature declares is reported
    /// when the registry is built; a collection no feature declares drops the
    /// member without a diagnostic, as in `flm-core`.
    ///
    /// # Errors
    ///
    /// `member` does not begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), or the builder refuses the
    /// declaration ([`ExtError::Core`]).
    fn collection_member(&mut self, collection: &str, member: &str) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::scoped_package`](flm_core::core::registry::FeatureBuilder::scoped_package):
    /// declares a scoped package, a set of elements whose commands are legal
    /// only inside the arguments and bodies that name the package
    /// ([`BodyDef::scoped`](flm_core::lang::mint::BodyDef::scoped),
    /// [`ArgDef::scoped`](flm_core::lang::mint::ArgDef::scoped)). What
    /// differs: it takes the extension API's [`ScopedPackageDef`], whose
    /// owner is the builder's feature; the package's name and every
    /// element's name must begin with the extension's stem and a dot. Each
    /// element is converted as [`element`](FeatureBuilder::element) converts
    /// one, so a declaration a helper made and the handler aspects work
    /// inside a scoped package as they do outside.
    ///
    /// # Errors
    ///
    /// A name does not begin with the extension's stem and a dot
    /// ([`ExtError::NameOutsideExtension`]), an element does not convert, or
    /// the builder refuses the package ([`ExtError::Core`]).
    fn scoped_package(&mut self, def: ScopedPackageDef) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::late_init`](flm_core::core::registry::FeatureBuilder::late_init):
    /// registers a hook the build calls once the language is built. What
    /// differs: it takes the extension API's [`LateInit`].
    ///
    /// # Errors
    ///
    /// The builder refuses the hook ([`ExtError::Core`]).
    fn late_init(&mut self, hook: Arc<dyn LateInit>) -> Result<(), ExtError>;

    /// Mirrors [`FeatureBuilder::finalizer`](flm_core::core::registry::FeatureBuilder::finalizer):
    /// registers a finalizer, which the build calls in its second round, once
    /// every feature has registered and the service slots are assembled,
    /// with the builder of this feature. What differs: it takes the
    /// extension API's [`Finalizer`]. In the second round, the builder lent
    /// to a finalizer declares a peer table or a presentation element, a
    /// stage and a lowering rule at once, where the first round's builder
    /// defers them to the second round; a finalizer cannot register another.
    ///
    /// # Errors
    ///
    /// Called in the second round, or the builder refuses the finalizer
    /// ([`ExtError::Core`]: `flm-core`'s `ContributionAfterAssembly`).
    fn finalizer(&mut self, hook: Arc<dyn Finalizer>) -> Result<(), ExtError>;
}

/// Mirrors [`LateInit`](flm_core::core::registry::LateInit): a hook a feature
/// registers ([`FeatureBuilder::late_init`]) that the build calls once the
/// language is built, before the registry is finished.
///
/// What differs: the language and the diagnostics are the extension API's
/// façades, [`BuiltLang`] and [`FlmDiagnostics`], lent for the call; the
/// error is an [`ExtError`]. An `Err` fails the build as `flm-core`'s
/// `LateInitError` does, its text `flm.ext.handler-failed: ` followed by the
/// error's text.
pub trait LateInit: Send + Sync {
    /// Mirrors [`LateInit::late_init`](flm_core::core::registry::LateInit::late_init):
    /// prepares what needs the finished language `lang`, and writes every
    /// problem with the configuration to `diags`. What differs: the two
    /// façades share the call's lifetime `'c`.
    ///
    /// # Errors
    ///
    /// The hook's failure: the build fails.
    fn late_init<'c>(&self, lang: &'c dyn BuiltLang, diags: &'c mut dyn FlmDiagnostics) -> Result<(), ExtError>;
}

/// Mirrors [`Finalizer`](flm_core::core::registry::Finalizer): a hook a
/// feature registers ([`FeatureBuilder::finalizer`]) that the build calls in
/// its second round with the feature's builder.
///
/// What differs: the builder is the extension API's [`FeatureBuilder`] of
/// the second round; the error is an [`ExtError`]. An `Err` fails the build
/// as a failing `register` does, its text `flm.ext.handler-failed: ` followed
/// by the error's text. The assembled services (`FeatureBuilder::services`
/// in `flm-core`) are not offered.
pub trait Finalizer: Send + Sync {
    /// Mirrors [`Finalizer::finalize`](flm_core::core::registry::Finalizer::finalize):
    /// declares, by calling methods of `b`, what this feature derives once
    /// every feature has registered. What differs: the builder is the
    /// extension API's, lent for the call.
    ///
    /// # Errors
    ///
    /// The hook's failure, or a refusal of the builder it passes on: the
    /// build fails.
    fn finalize(&self, b: &mut dyn FeatureBuilder) -> Result<(), ExtError>;
}

/// No `flm-core` counterpart: what a feature contributes to a service slot —
/// data, or a handler object of the trait the slot's name calls for.
/// `flm-core`'s `contribute_to::<S>` takes an `Arc<S>` of the slot's type;
/// here the type is the variant.
///
/// The slots that take a handler object are `flm.citations.providers` (a
/// [`CitationProvider`]) and `flm.compose.templates` (a [`TemplateModule`]);
/// every other slot takes data. Across a binding, data as it is, or a handle
/// of kind `citation_provider` or `template_module`.
pub enum Contributed {
    /// Data: the value of a slot a script declared.
    Data(serde_json::Value),
    /// A citation provider, for `flm.citations.providers`.
    CitationProvider(Arc<dyn CitationProvider>),
    /// A compose template module, for `flm.compose.templates`.
    TemplateModule(Arc<dyn TemplateModule>),
}

fn _object_safe(_: &dyn FeatureBuilder, _: &dyn LateInit, _: &dyn Finalizer) {}
