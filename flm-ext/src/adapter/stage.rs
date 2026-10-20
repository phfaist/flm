//! `ExtStage`, `flm-core`'s stage over the extension API's [`Stage`], and the
//! façades a run of it lends to the extension: [`StageCxFacade`] (what the
//! stage reads), [`StageOutFacade`] (what it writes), [`TreesTableFacade`] (a
//! rewrite's trees, read by name), [`LoweredTreesFacade`] (a backend's lowered
//! trees, read by name) and [`ProjectCxFacade`] (how another table's
//! projection renders its addresses).
//!
//! Built by WP-2 (the declaration and the needs) and WP-3 (the run and the
//! façades it lends).
//!
//! Every node the façades answer is a [`NodeFacade::shared`] over the tree's
//! `Arc`, so it owns what it points at (plan A, A15). Every table the façades
//! answer borrows the store, which lives for the whole call.

use std::sync::Arc;

use flm_core::core::data::store::StageOut as CoreStageOut;
use flm_core::core::artifact::{path_of, ArtifactNamer, ArtifactSet, NamingScope};
use flm_core::core::data::table::{ProjectCx, TableHandle, TableRef, TableType};
use flm_core::core::plan::PlanCx;
use flm_core::core::registry::{DefinitionText, FlmRegistry};
use flm_core::core::run::StageCx as CoreStageCx;
use flm_core::core::stage::{
    Needs as CoreNeeds, Stage as CoreStage, StageDecl as CoreStageDecl, TreesTable as CoreTreesTable,
};
use flm_core::dump::json::to_json;
use flm_core::dump::project::{node_ref_value, tree_ref_value, ProjectOptions};
use flm_core::lang::{Flm, FlmDiagnostic, FlmOrigin, FlmSource, FlmSpan};
use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::model::names::CapName;
use flm_core::model::options::OptionRef;
use flm_core::stdcore::backends::lowering::{LoweredTree, LoweredTrees as CoreLoweredTrees, NAMER_SLOT};
use flm_core::techy::core::node::NodeTree;
use flm_core::techy::serialize::SerialValue;
use flm_core::techy::source::SourceSpan;
use serde_json::Value;

use crate::adapter::diag::ExtCondition;
use crate::adapter::names::Owner;
use crate::adapter::node::NodeFacade;
use crate::adapter::services::{self, DeclaredServices};
use crate::adapter::table::ExtTable;
use crate::adapter::values::{artifact_set_to_value, check_data, diagnostic_to_core};
use crate::api::core::data::store::{StageOut, TablePiece};
use crate::adapter::tables::{AnchorsFacade, CountersFacade, Seen};
use crate::api::core::data::table::{ExtTableRead, TableValue};
use flm_core::stdcore::features::numbering::Counters as CoreCounters;
use flm_core::stdcore::features::refs::Anchors as CoreAnchors;
use crate::api::core::run::{StageCx, WalkEntry};
use crate::api::host::HostHandle;
use crate::api::core::stage::{Needs, Scope, Stage, StageDecl, TreesTable};
use crate::api::stdcore::backends::lowering::LoweredTrees;
use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::adapter::durable::DurableTable;
use crate::sealed::{DurableOwner, Sealed, StageCxCore, Token};

/// `flm-core`'s stage over an extension's [`Stage`]: what
/// [`FeatureBuilder::stage`](crate::api::core::registry::FeatureBuilder::stage)
/// registers. The declaration and the needs were read once, when the stage
/// was registered, and are answered from that copy.
pub struct ExtStage {
    /// The extension's stage.
    pub(crate) stage: Arc<dyn Stage>,
    /// Its declaration, read at registration.
    pub(crate) decl: StageDecl,
    /// Its needs, read at registration.
    pub(crate) needs: Needs,
    /// The extension and the package that registered the stage: the
    /// identifiers of the diagnostics the stage reports must begin with the
    /// extension's stem.
    pub(crate) owner: Owner,
    /// Every table the stage writes, with the type it holds, as the
    /// registry declared it when the stage was registered.
    pub(crate) writes: Arc<[(String, WriteType)]>,
    /// The host services the package declared, which the stage's context
    /// lends ([`StageCx::host`]).
    pub(crate) services: Arc<DeclaredServices>,
}

/// The type of a table a scripted stage writes: the three a stage of an
/// extension can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteType {
    /// An extension's own table ([`ExtTable`]), and a table no feature
    /// declares, which the registry then reports.
    Ext,
    /// The text of a definition's part ([`DefinitionText`]).
    DefinitionText,
    /// The files of an output ([`ArtifactSet`]).
    ArtifactSet,
}

impl WriteType {
    /// The type `ty` a feature declared a table with, as a stage's write;
    /// `None` for a type a scripted stage cannot write.
    pub(crate) fn of(ty: Option<TableType>) -> Option<WriteType> {
        match ty {
            None => Some(WriteType::Ext),
            Some(ty) if ty == TableType::of::<ExtTable>() => Some(WriteType::Ext),
            Some(ty) if ty == TableType::of::<DefinitionText>() => Some(WriteType::DefinitionText),
            Some(ty) if ty == TableType::of::<ArtifactSet>() => Some(WriteType::ArtifactSet),
            Some(_) => None,
        }
    }

    /// The type a write of `table` declared with `writes` has: [`WriteType::Ext`]
    /// for a table the list does not hold.
    fn among(writes: &[(String, WriteType)], table: &str) -> WriteType {
        writes.iter().find(|(name, _)| name == table).map_or(WriteType::Ext, |(_, ty)| *ty)
    }
}

impl CoreStage for ExtStage {
    /// The declaration read at registration. Every table the stage writes is
    /// written with the type the registry declared it with: an [`ExtTable`],
    /// a [`DefinitionText`] or an [`ArtifactSet`] (the finalizer refused any
    /// other).
    fn decl(&self) -> CoreStageDecl {
        let decl = &self.decl;
        let mut core = CoreStageDecl::new(decl.name.as_str(), decl.effect, decl.scope).docs(decl.docs.as_str());
        for capability in &decl.provides {
            core = core.provides(capability.as_str());
        }
        for table in &decl.writes {
            let name = table.as_str();
            core = match WriteType::among(&self.writes, name) {
                WriteType::Ext => core.writes(TableHandle::<ExtTable>::new(name)),
                WriteType::DefinitionText => core.writes(TableHandle::<DefinitionText>::new(name)),
                WriteType::ArtifactSet => core.writes(TableHandle::<ArtifactSet>::new(name)),
            };
        }
        core
    }

    /// The needs read at registration. A table the stage reads is named by
    /// its name alone, so its type is the one the registry declared it with;
    /// a table the registry does not declare is named as an [`ExtTable`], and
    /// the plan reports it as undeclared. An option type is read by its name
    /// ([`OptionRef::named`]).
    fn needs(&self, cx: &PlanCx<'_>) -> CoreNeeds {
        let registry = cx.registry();
        let needs = &self.needs;
        let mut core = CoreNeeds::new();
        core.requires = needs.requires.iter().map(|name| CapName::from(name.as_str())).collect();
        core.reads = needs.reads.iter().map(|name| read_ref(registry, name)).collect();
        core.reads_opt = needs.reads_opt.iter().map(|name| read_ref(registry, name)).collect();
        core.after = needs.after.iter().map(|name| CapName::from(name.as_str())).collect();
        core.before = needs.before.iter().map(|name| CapName::from(name.as_str())).collect();
        core.options = needs.options.iter().map(|name| OptionRef::named(name.as_str())).collect();
        core
    }

    /// Lends the extension's stage a [`StageCxFacade`] over `cx` and a
    /// [`StageOutFacade`] over `out`, and calls it. A failure of the handler is
    /// an Error diagnostic, `flm.ext.handler-failed`, at the root of the
    /// stage's tree (for a document-scope stage, of the first tree in document
    /// order; with no tree, at an empty source of no origin); what the handler
    /// pushed before failing stays pushed (plan A, D8).
    fn run(&self, cx: &CoreStageCx<'_>, out: &mut CoreStageOut<'_>) {
        let span = default_span(cx);
        let cx_facade = StageCxFacade::new(cx, &self.services, Arc::clone(&self.owner.durable));
        let result = {
            let mut out_facade = StageOutFacade::new(&mut *out, &self.owner, span.clone(), &self.writes);
            self.stage.run(&cx_facade, &mut out_facade)
        };
        if let Err(error) = result {
            let handler = format!("the stage `{}`", self.decl.name);
            out.diags.push(FlmDiagnostic::error(ExtCondition::handler_failed(&error, &handler), span));
        }
    }
}

/// The reference a read of the table `name` declares: typed with the type the
/// registry holds for the name, or as an [`ExtTable`] when no table carries
/// it.
pub(crate) fn read_ref(registry: &FlmRegistry, name: &str) -> TableRef {
    match registry.table_named(name) {
        Some(entry) => TableRef {
            name: CapName::from(name),
            type_id: entry.def.ty.type_id(),
            type_name: entry.def.ty.type_name(),
        },
        None => TableRef::of::<ExtTable>(name),
    }
}

/// Where a diagnostic of a stage's run stands when it names no node (plan A,
/// section 4.5): the root of the stage's tree for a per-tree stage; for a
/// document-scope stage, the root of the first tree in document order; with no
/// tree, an empty span over an empty source of no origin ([`nowhere`]).
pub(crate) fn default_span(cx: &CoreStageCx<'_>) -> FlmSpan {
    let tree = cx.my_tree().or_else(|| cx.trees().next().map(|(_, tree)| Arc::clone(tree)));
    match tree {
        Some(tree) => tree.root().span().clone(),
        None => nowhere(),
    }
}

/// An empty span over an empty source of no origin, for a report that has no
/// tree to stand at.
pub(crate) fn nowhere() -> FlmSpan {
    let source = Arc::new(FlmSource::new("").with_origin(FlmOrigin::none()));
    SourceSpan::new(&source, 0..0)
}

/// The tree id whose display text ([`TreeId`]'s `Display`) is `text`:
/// `tree#<n>` or `value#<n>`. `None` for any other text.
pub(crate) fn tree_id_of(text: &str) -> Option<TreeId> {
    let (make, index): (fn(u32) -> TreeId, &str) = match text.strip_prefix("tree#") {
        Some(index) => (TreeId::tree, index),
        None => (TreeId::value, text.strip_prefix("value#")?),
    };
    let index: u32 = index.parse().ok()?;
    // `TreeId::tree` and `TreeId::value` panic on an index with the top bit set.
    if index >= 1 << 31 {
        return None;
    }
    let id = make(index);
    // Only the display text itself names the tree (not `tree#+1`, `tree#01`).
    (id.to_string() == text).then_some(id)
}

/// The root of `tree`, as a node of the extension API.
pub(crate) fn root_of<'s>(tree: &Arc<NodeTree<Flm>>) -> Box<dyn Node<'s> + 's> {
    Box::new(NodeFacade::shared(Arc::clone(tree), tree.root().id()))
}

/// An `AccessError` of the store as the extension API reports it.
fn access(error: impl std::fmt::Display) -> ExtError {
    ExtError::Core { kind: "access".to_string(), detail: error.to_string(), data: Value::Null }
}

// --- what a stage reads ------------------------------------------------------------------------

/// The extension API's [`StageCx`] over `flm-core`'s context of one stage
/// call: what [`ExtStage`] lends the extension's `run` handler.
pub struct StageCxFacade<'r, 'x> {
    cx: &'r CoreStageCx<'x>,
    services: &'r Arc<DeclaredServices>,
    /// The durable table of the stage's feature.
    durable: Arc<DurableTable>,
}

impl<'r, 'x> StageCxFacade<'r, 'x> {
    /// The façade over `cx`, lending the host services `services` and the
    /// durable table `durable`.
    pub(crate) fn new(
        cx: &'r CoreStageCx<'x>,
        services: &'r Arc<DeclaredServices>,
        durable: Arc<DurableTable>,
    ) -> StageCxFacade<'r, 'x> {
        StageCxFacade { cx, services, durable }
    }
}

impl Sealed for StageCxFacade<'_, '_> {}

impl DurableOwner for StageCxFacade<'_, '_> {
    fn durable_table(&self, _: Token) -> Arc<DurableTable> {
        Arc::clone(&self.durable)
    }
}

impl StageCxCore for StageCxFacade<'_, '_> {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&CoreStageCx<'_>)) {
        f(self.cx);
    }
}

impl StageCx for StageCxFacade<'_, '_> {
    fn scope(&self) -> Scope {
        self.cx.scope()
    }

    fn tree_id(&self) -> Option<String> {
        self.cx.tree_id().map(|id| id.to_string())
    }

    fn tree(&self, id: &str) -> Option<Box<dyn Node<'_> + '_>> {
        let tree = self.cx.tree(tree_id_of(id)?)?;
        Some(root_of(&tree))
    }

    fn frozen_tree(&self, id: &str) -> Option<Box<dyn Node<'_> + '_>> {
        self.cx.frozen_tree(tree_id_of(id)?).map(root_of)
    }

    fn my_tree(&self) -> Option<Box<dyn Node<'_> + '_>> {
        self.cx.my_tree().map(|tree| root_of(&tree))
    }

    fn trees(&self) -> Vec<(String, Box<dyn Node<'_> + '_>)> {
        self.cx.trees().map(|(id, tree)| (id.to_string(), root_of(tree))).collect()
    }

    fn walk(&self) -> Vec<WalkEntry<'_>> {
        let cx = self.cx;
        cx.walk()
            .filter_map(|(addr, node)| {
                // The walk answers nodes of frozen trees alone, so the tree is
                // there; a node of none would be left out rather than read.
                let tree = cx.frozen_tree(addr.tree)?;
                let node: Box<dyn Node<'_> + '_> = Box::new(NodeFacade::shared(Arc::clone(tree), node.id()));
                Some(WalkEntry { addr, node })
            })
            .collect()
    }

    fn addr_of(&self, node: &dyn Node<'_>) -> Option<NodeAddr> {
        self.cx.addr(&node.core_node_ref(Token::new()))
    }

    fn get(&self, name: &str) -> Result<Option<TableValue<'_>>, ExtError> {
        let cx = self.cx;
        let Some(table) = cx.view().get_erased(name).map_err(access)? else {
            return Ok(None);
        };
        let table: &(dyn std::any::Any + Send + Sync) = &**table;
        if let Some(table) = table.downcast_ref::<ExtTable>() {
            return Ok(Some(TableValue::Ext(Box::new(table))));
        }
        if let Some(table) = table.downcast_ref::<CoreTreesTable>() {
            return Ok(Some(TableValue::Trees(Box::new(TreesTableFacade::new(table)))));
        }
        if let Some(table) = table.downcast_ref::<CoreLoweredTrees>() {
            return Ok(Some(TableValue::Lowered(Box::new(LoweredTreesFacade::new(table)))));
        }
        if let Some(table) = table.downcast_ref::<CoreCounters>() {
            return Ok(Some(TableValue::Counters(Box::new(CountersFacade::new(table, Seen::Stage(cx))))));
        }
        if let Some(table) = table.downcast_ref::<CoreAnchors>() {
            return Ok(Some(TableValue::Anchors(Box::new(AnchorsFacade::new(table, Seen::Stage(cx))))));
        }
        if let Some(set) = table.downcast_ref::<ArtifactSet>() {
            return Ok(Some(TableValue::Projection(artifact_set_read(cx, name, set)?)));
        }
        let Some(entry) = cx.registry().table_named(name) else {
            return Err(access(format_args!("the table `{name}` must be declared by a feature of the registry")));
        };
        let projection = entry.def.ty.project(table, &ProjectCxFacade::new(cx));
        Ok(Some(TableValue::Projection(projection_value(&projection)?)))
    }

    /// `cx.option_named(name)`, cloned; an [`OptionError`](flm_core::model::options::OptionError)
    /// is an [`ExtError::Core`] of kind `option`.
    fn option(&self, name: &str) -> Result<Value, ExtError> {
        self.cx.option_named(name).cloned().map_err(|error| ExtError::Core {
            kind: "option".to_string(),
            detail: error.to_string(),
            data: Value::Null,
        })
    }

    /// Over [`Services::services_by_precedence_named`](flm_core::core::services::Services::services_by_precedence_named)
    /// for a slot a script declared, its first value, as
    /// [`Services::service_named`](flm_core::core::services::Services::service_named)
    /// reads it.
    fn service(&self, name: &str) -> Result<Option<Value>, ExtError> {
        services::read_first(self.cx.services_all(), name)
    }

    /// Over [`Services::services_named`](flm_core::core::services::Services::services_named)
    /// for a slot a script declared.
    fn services(&self, name: &str) -> Result<Vec<Value>, ExtError> {
        services::read_all(self.cx.services_all(), name)
    }

    /// Over [`Services::services_by_precedence_named`](flm_core::core::services::Services::services_by_precedence_named)
    /// for a slot a script declared.
    fn services_by_precedence(&self, name: &str) -> Result<Vec<Value>, ExtError> {
        services::read_by_precedence(self.cx.services_all(), name)
    }

    fn host(&self, name: &str) -> Result<HostHandle, ExtError> {
        self.services.handle(name)
    }
}

/// The artifact set `set`, the table `name`, as a stage reads it
/// ([`artifact_set_to_value`]): each artifact with the path at which the host
/// writes it ([`path_of`]), computed with the naming of files the host
/// contributes to the service slot [`NAMER_SLOT`] and the scope of the run
/// ([`NamingScope::of`] over the registry and the plan's demands), as the
/// library's own stages compute the paths they write into the files. The
/// output the artifacts belong to is the first output, in the order of their
/// names, whose table is `name`; none for a table no output keeps its files
/// in. Without a naming of files, an artifact's path is its fixed path, the
/// only path known, or `null`.
///
/// # Errors
///
/// An artifact of the set refers to a file instead of holding its bytes
/// ([`ExtError::Value`]).
fn artifact_set_read(cx: &CoreStageCx<'_>, name: &str, set: &ArtifactSet) -> Result<Value, ExtError> {
    // See decision rationale at [§flm-dd-dr:ext-artifact-set-read].
    let registry = cx.registry();
    let output = registry.outputs().find(|output| output.def.table.as_str() == name).map(|output| &output.def);
    let namer = cx.services_all().service_named::<dyn ArtifactNamer>(NAMER_SLOT);
    let scope = NamingScope::of(registry, cx.plan().demands());
    artifact_set_to_value(set, &|artifact| match namer {
        Some(namer) => Some(path_of(artifact, output, namer, &scope)),
        None => artifact.fixed_path.clone(),
    })
}

/// A table's projection as a value that crosses the boundary.
///
/// # Errors
///
/// The projection holds something that cannot cross: a number that is not a
/// 64-bit signed integer, bytes, a reserved map key ([`ExtError::Value`]).
pub(crate) fn projection_value(projection: &SerialValue) -> Result<Value, ExtError> {
    let text = to_json(projection).map_err(|error| ExtError::Value { detail: error.to_string() })?;
    let value: Value = serde_json::from_str(&text).map_err(|error| ExtError::Value { detail: error.to_string() })?;
    check_data(&value)?;
    Ok(value)
}

/// An extension's own table as the extension API reads it, borrowed from the
/// store for the call.
impl Sealed for &ExtTable {}

impl ExtTableRead for &ExtTable {
    fn items(&self) -> Vec<Value> {
        ExtTable::items(self).to_vec()
    }

    fn items_at(&self, addr: &NodeAddr) -> Vec<Value> {
        ExtTable::items_at(self, addr)
    }
}

/// The extension API's [`TreesTable`] over `flm-core`'s
/// [`TreesTable`](CoreTreesTable), borrowed from the store for the call: each
/// tree is answered as its root node, shared with the table.
pub struct TreesTableFacade<'x> {
    table: &'x CoreTreesTable,
}

impl<'x> TreesTableFacade<'x> {
    /// The façade over `table`.
    pub(crate) fn new(table: &'x CoreTreesTable) -> TreesTableFacade<'x> {
        TreesTableFacade { table }
    }
}

impl Sealed for TreesTableFacade<'_> {}

impl<'s> TreesTable<'s> for TreesTableFacade<'_> {
    fn trees(&self) -> Vec<(String, Box<dyn Node<'s> + 's>)> {
        self.table.trees.iter().map(|(id, tree)| (id.to_string(), root_of(tree))).collect()
    }

    fn tree(&self, id: &str) -> Option<Box<dyn Node<'s> + 's>> {
        self.table.trees.get(&tree_id_of(id)?).map(root_of)
    }
}

/// The extension API's [`LoweredTrees`] over `flm-core`'s
/// [`LoweredTrees`](CoreLoweredTrees): a copy of the table's entries, each
/// lowered tree shared with the table through its `Arc` and answered as its
/// root node. A copy, so that a rule's lowerer can answer it as the owned
/// read the dispatcher asks for as well.
pub struct LoweredTreesFacade {
    trees: Vec<(TreeId, LoweredTree)>,
}

impl LoweredTreesFacade {
    /// The façade over a copy of `table`'s entries.
    pub(crate) fn new(table: &CoreLoweredTrees) -> LoweredTreesFacade {
        LoweredTreesFacade { trees: table.trees.iter().map(|(id, tree)| (*id, tree.clone())).collect() }
    }

    /// The lowered form of the document tree `id`.
    fn get(&self, id: &str) -> Option<&LoweredTree> {
        let id = tree_id_of(id)?;
        self.trees.iter().find(|(held, _)| *held == id).map(|(_, tree)| tree)
    }
}

impl Sealed for LoweredTreesFacade {}

impl<'s> LoweredTrees<'s> for LoweredTreesFacade {
    fn trees(&self) -> Vec<(String, Box<dyn Node<'s> + 's>)> {
        self.trees.iter().map(|(id, lowered)| (id.to_string(), root_of(&lowered.tree))).collect()
    }

    fn tree(&self, id: &str) -> Option<Box<dyn Node<'s> + 's>> {
        self.get(id).map(|lowered| root_of(&lowered.tree))
    }

    fn requires(&self, id: &str) -> Option<Vec<String>> {
        self.get(id).map(|lowered| lowered.requires.iter().map(ToString::to_string).collect())
    }

    fn rounds(&self, id: &str) -> Option<u32> {
        self.get(id).map(|lowered| lowered.rounds)
    }
}

/// `flm-core`'s [`ProjectCx`] over a stage call's context: how a table that is
/// neither an extension's nor a rewrite's trees renders the addresses it holds
/// when [`StageCx::get`] answers its projection.
///
/// A node renders as the projection renders it, `elem@line:col`
/// ([`node_ref_value`], default options), through the tree the context
/// answers for the address; an address no tree of the context resolves renders
/// as `?@<tree>/<node index>`, as the library's own projection does. A tree
/// renders as its id's display text ([`tree_ref_value`]).
pub struct ProjectCxFacade<'r, 'x> {
    cx: &'r CoreStageCx<'x>,
}

impl<'r, 'x> ProjectCxFacade<'r, 'x> {
    /// The context over `cx`.
    pub(crate) fn new(cx: &'r CoreStageCx<'x>) -> ProjectCxFacade<'r, 'x> {
        ProjectCxFacade { cx }
    }
}

impl ProjectCx for ProjectCxFacade<'_, '_> {
    fn node_ref(&self, addr: NodeAddr) -> SerialValue {
        match self.cx.tree(addr.tree) {
            Some(tree) if tree.get(addr.node).is_some() => {
                node_ref_value(&tree, addr.node, &ProjectOptions::default())
            }
            _ => SerialValue::Str(format!("?@{}/{}", addr.tree, addr.node.index())),
        }
    }

    fn tree_ref(&self, id: TreeId) -> SerialValue {
        tree_ref_value(id)
    }
}

// --- what a stage writes -----------------------------------------------------------------------

/// The extension API's [`StageOut`] over `flm-core`'s collector of one stage
/// call: what [`ExtStage`] lends the extension's `run` handler.
pub struct StageOutFacade<'r, 'o> {
    out: &'r mut CoreStageOut<'o>,
    /// The extension and the package that registered the stage.
    owner: &'r Owner,
    /// Where a diagnostic that names no node stands.
    span: FlmSpan,
    /// Every table the stage writes, with its type.
    writes: &'r [(String, WriteType)],
}

impl<'r, 'o> StageOutFacade<'r, 'o> {
    /// The façade over `out`, for a stage registered by `owner`, placing a
    /// diagnostic that names no node at `span`; the stage writes each table
    /// of `writes` with the type given beside it.
    pub(crate) fn new(
        out: &'r mut CoreStageOut<'o>,
        owner: &'r Owner,
        span: FlmSpan,
        writes: &'r [(String, WriteType)],
    ) -> StageOutFacade<'r, 'o> {
        StageOutFacade { out, owner, span, writes }
    }
}

impl Sealed for StageOutFacade<'_, '_> {}

impl StageOut for StageOutFacade<'_, '_> {
    fn table(&mut self, name: &str) -> Result<&mut dyn TablePiece, ExtError> {
        let piece: &mut dyn TablePiece = match WriteType::among(self.writes, name) {
            WriteType::Ext => self.out.table(TableHandle::<ExtTable>::new(name)).map_err(access)?,
            WriteType::DefinitionText => self.out.table(TableHandle::<DefinitionText>::new(name)).map_err(access)?,
            WriteType::ArtifactSet => self.out.table(TableHandle::<ArtifactSet>::new(name)).map_err(access)?,
        };
        Ok(piece)
    }

    fn diags(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError> {
        self.owner.check("diagnostic", &diagnostic.id)?;
        let diagnostic = diagnostic_to_core(diagnostic, self.span.clone())?;
        self.out.diags.push(diagnostic);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_id_is_read_from_its_display_text_alone() {
        assert_eq!(tree_id_of("tree#0"), Some(TreeId::tree(0)));
        assert_eq!(tree_id_of("value#12"), Some(TreeId::value(12)));
        for text in ["tree#", "tree#+1", "tree#01", "tree#-1", "tree#2147483648", "value#4294967296", "tree 1", ""] {
            assert_eq!(tree_id_of(text), None, "{text}");
        }
    }

    #[test]
    fn a_report_with_no_tree_stands_at_an_empty_source_of_no_origin() {
        let span = nowhere();
        assert_eq!(span.range(), 0..0);
        assert_eq!(span.source().origin(), &FlmOrigin::none());
    }
}
