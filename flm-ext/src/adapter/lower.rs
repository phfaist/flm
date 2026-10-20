//! `ExtLower`, `flm-core`'s rule body over the extension API's [`Lower`], the
//! façades it lends the body ([`LowerCxFacade`], [`LowererFacade`]), and the
//! finalizer that registers an extension's rule ([`RuleFinalizer`]).
//!
//! # Registration
//!
//! A rule an extension built ([`LoweringRule::new`](crate::api::core::lowering::LoweringRule::new))
//! names the tables it reads by their names alone, while `flm-core`'s
//! declaration types each read by the table's value type. The type of a table
//! of another feature is known once every feature has declared its tables, so
//! [`FeatureBuilder::lowering_rule`](crate::api::core::registry::FeatureBuilder::lowering_rule)
//! registers a [`RuleFinalizer`], and the rule is registered with `flm-core` in
//! the build's second round, each read typed as the registry declares the
//! table ([`FeatureBuilder::table_type_named`](flm_core::core::registry::FeatureBuilder::table_type_named));
//! a table no feature declares is typed as an extension's own
//! ([`ExtTable`]), and the registry's checks report it as for any rule.
//!
//! # One call
//!
//! [`ExtLower::lower`] lends the body the node ([`NodeFacade::lent`]), the
//! context and the lowerer, each over `flm-core`'s own objects for this call.
//! The lowerer records every node it stages and every node placed as content
//! or in the answer: a node may be placed only if this call staged it, and
//! once. A `flm-core` error met by a method that stages is kept, and the call
//! ends with it whatever the body answers, as a compiled rule returns every
//! error the lowerer gives it. A failure of the body is the Error
//! `flm.ext.handler-failed` at the node, returned as
//! [`LowerError::Refused`]: the lowering stage reports it and stages the
//! childless `flm.pres.unsupported` stand-in in the node's place.

use std::any::Any;
use std::collections::HashSet;
use std::sync::Arc;

use flm_core::core::data::table::{ProjectCx, TableRef, TableType};
use flm_core::core::lowering::{
    LowerCx as CoreLowerCx, LowerError, Lower as CoreLower, Lowerer as CoreLowerer, Lowering as CoreLowering,
    LoweringDecl as CoreLoweringDecl, LoweringRule as CoreLoweringRule, NodeRegion,
};
use flm_core::core::registry::{BuildError, FeatureBuilder as CoreFeatureBuilder, Finalizer};
use flm_core::core::stage::TreesTable as CoreTreesTable;
use flm_core::dump::project::{node_ref_value, tree_ref_value, ProjectOptions};
use flm_core::lang::{Flm, FlmDiagnostic, PresPayload};
use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::model::names::{CapName, ElementName, Name};
use flm_core::model::options::{OptionRef, TypeKey};
use flm_core::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions;
use flm_core::stdcore::features::math::MathRecomposeOptions;
use flm_core::stdcore::backends::lowering::{lower_region_content, lower_run, LoweredTrees as CoreLoweredTrees};
use flm_core::techy::core::node::{BuildId, NodeRef, NodeTree};
use flm_core::techy::serialize::SerialValue;
use serde_json::Value;

use crate::adapter::diag::ExtCondition;
use crate::adapter::names::Owner;
use crate::adapter::node::NodeFacade;
use crate::adapter::payloads::{payload_from_value, DataPayload};
use crate::adapter::stage::{projection_value, root_of, tree_id_of, LoweredTreesFacade, TreesTableFacade};
use crate::adapter::table::ExtTable;
use crate::adapter::tables::{AnchorsFacade, CountersFacade, Seen};
use flm_core::stdcore::features::numbering::Counters as CoreCounters;
use flm_core::stdcore::features::refs::Anchors as CoreAnchors;
use crate::adapter::values::{check_data, diagnostic_to_core, node_addr_to_value};
use crate::api::core::data::table::{ExtTableRead, TableValue};
use crate::api::core::lowering::{ExtRule, Lower, LowerCx, Lowerer, Lowering, Staged};
use crate::api::core::stage::TreesTable;
use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::adapter::builder::BuiltLangFacade;
use crate::adapter::durable::DurableTable;
use crate::api::lang::BuiltLang;
use crate::sealed::{CorePart, DurableOwner, LowerCxCore, LowererCore, Sealed, Token};

// --- registration ------------------------------------------------------------------------------

/// The finalizer that registers an extension's rule with `flm-core`, its reads
/// typed by the registry's declarations (see the [module
/// documentation](self#registration)).
pub struct RuleFinalizer {
    /// The rule, as the extension built it.
    pub(crate) rule: ExtRule,
    /// The extension and the package that registered the rule.
    pub(crate) owner: Owner,
}

impl Finalizer for RuleFinalizer {
    fn finalize(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let rule = &self.rule;
        let typed = |name: &String| (name.clone(), b.table_type_named(name).unwrap_or_else(TableType::of::<ExtTable>));
        let reads: Vec<(String, TableType)> = rule.decl.reads.iter().map(typed).collect();
        let reads_opt: Vec<(String, TableType)> = rule.decl.reads_opt.iter().map(typed).collect();
        let decl = CoreLoweringDecl {
            requires: rule.decl.requires.iter().map(|name| CapName::from(name.as_str())).collect(),
            reads: reads.iter().map(table_ref).collect(),
            reads_opt: reads_opt.iter().map(table_ref).collect(),
            options: rule.decl.options.iter().map(|name| option_ref(name)).collect(),
            produces: rule.decl.produces.iter().map(|name| ElementName::from(name.as_str())).collect(),
            selection_requires: rule.decl.selection_requires.iter().map(|name| Name::from(name.as_str())).collect(),
        };
        let mut tables = reads;
        tables.extend(reads_opt);
        // The elements the rule produces that a package declared with the
        // adapter's data payload: `pres` stages them with the value as it is.
        let data_payload = Some(TypeKey::of::<DataPayload>());
        let data_elements: Arc<[String]> = rule
            .decl
            .produces
            .iter()
            .filter(|name| b.presentation_payload_named(name) == data_payload)
            .cloned()
            .collect();
        let body = ExtLower {
            body: Arc::clone(&rule.body),
            rule: rule.name.clone(),
            owner: self.owner.clone(),
            tables: tables.into(),
            data_elements,
        };
        let core = CoreLoweringRule::new(
            rule.name.as_str(),
            rule.element.as_str(),
            decl,
            Arc::new(body),
            rule.mark.clone(),
        )
        .docs(rule.docs.as_str());
        b.lowering_rule(core)
    }
}

/// The option types of the standard package that a rule built by an
/// extension reads by their names, each with its typed read: the two the
/// `math` library's LaTeX parts read through the lowerer
/// ([`api::stdcore::features::math`](crate::api::stdcore::features::math)),
/// which a rule declared from data could not read otherwise, since a read by
/// name is a read of an option type declared from data.
const STANDARD_OPTIONS: [(&str, TypedRead); 2] = [
    ("MathRecomposeOptions", OptionRef::of::<MathRecomposeOptions>),
    ("LatexRecomposedRenderOptions", OptionRef::of::<LatexRecomposedRenderOptions>),
];

/// The typed read of an option type.
type TypedRead = fn() -> OptionRef;

/// The read a rule's declaration of the option type `name` stands for: the
/// typed read of a standard option type of [`STANDARD_OPTIONS`], or the read
/// of the option type declared from data under that name.
fn option_ref(name: &str) -> OptionRef {
    STANDARD_OPTIONS
        .iter()
        .find(|(known, _)| *known == name)
        .map_or_else(|| OptionRef::named(name), |(_, typed)| typed())
}

/// The reference a read of the table `name`, of the value type `ty`, declares.
fn table_ref((name, ty): &(String, TableType)) -> TableRef {
    TableRef { name: CapName::from(name.as_str()), type_id: ty.type_id(), type_name: ty.type_name() }
}

// --- the body ----------------------------------------------------------------------------------

/// `flm-core`'s rule body over an extension's [`Lower`]: what an extension's
/// rule holds once registered.
pub struct ExtLower {
    /// The extension's body.
    body: Arc<dyn Lower>,
    /// The rule's name, for the message of the body's failure.
    rule: String,
    /// The extension and the package that registered the rule: the
    /// identifiers of the diagnostics the body reports must begin with the
    /// extension's stem.
    owner: Owner,
    /// Every table the rule reads, with the value type its read declares.
    tables: Arc<[(String, TableType)]>,
    /// The elements the rule produces whose payload is a [`DataPayload`].
    data_elements: Arc<[String]>,
}

impl CoreLower for ExtLower {
    /// Lends the body the node, the context and the lowerer, and maps its
    /// answer: `Default` as it is, `Emit` with the nodes it names; a `flm-core`
    /// error met while staging is returned as it is; any other failure of the
    /// body is [`LowerError::Refused`] with the Error `flm.ext.handler-failed`
    /// at the node.
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut CoreLowerCx<'_>,
        lw: &mut dyn CoreLowerer,
    ) -> Result<CoreLowering, LowerError> {
        let site = NodeFacade::lent(node);
        let lent_cx = address_of(cx);
        let mut cx_facade = LowerCxFacade { cx };
        let mut lw_facade = LowererFacade {
            lw,
            lent_cx,
            site: node,
            owner: &self.owner,
            tables: &self.tables,
            data_elements: &self.data_elements,
            staged: HashSet::new(),
            placed: HashSet::new(),
            failure: None,
        };
        let answer = self.body.lower(&site, &mut cx_facade, &mut lw_facade);
        if let Some(failure) = lw_facade.failure.take() {
            return Err(failure);
        }
        let answer = answer.and_then(|lowering| match lowering {
            Lowering::Default => Ok(CoreLowering::Default),
            Lowering::Emit(staged) => {
                let ids = lw_facade.placeable("lower", &staged)?;
                Ok(CoreLowering::Emit(ids))
            }
        });
        answer.map_err(|error| {
            let handler = format!("the lowering rule `{}`", self.rule);
            let condition = ExtCondition::handler_failed(&error, &handler);
            LowerError::Refused(Box::new(FlmDiagnostic::error(condition, node.span().clone())))
        })
    }
}

/// The address of a context, which identifies the context lent to a call.
fn address_of(cx: &CoreLowerCx<'_>) -> *const () {
    (cx as *const CoreLowerCx<'_>).cast::<()>()
}

/// A `flm-core` error as the body sees it.
fn lower_error(error: &LowerError) -> ExtError {
    ExtError::Core { kind: "lower".to_string(), detail: error.to_string(), data: Value::Null }
}

/// An argument error of the method `method`.
fn argument(method: &str, detail: &str) -> ExtError {
    ExtError::Argument { method: method.to_string(), detail: detail.to_string() }
}

// --- the context -------------------------------------------------------------------------------

/// The context lent to one call of a rule's body: `flm-core`'s context, the
/// one implementation of [`LowerCx`].
pub struct LowerCxFacade<'a, 'x> {
    cx: &'a mut CoreLowerCx<'x>,
}

impl LowerCxCore for LowerCxFacade<'_, '_> {
    fn with_core(&mut self, _: Token, f: &mut dyn FnMut(&mut CoreLowerCx<'_>)) {
        f(self.cx);
    }
}

impl LowerCx for LowerCxFacade<'_, '_> {}

// --- the lowerer -------------------------------------------------------------------------------

/// The lowerer lent to one call of a rule's body: `flm-core`'s lowerer, what
/// was lent to the call, and what the call staged and placed; the one
/// implementation of [`Lowerer`].
pub struct LowererFacade<'a, 'n> {
    lw: &'a mut dyn CoreLowerer,
    /// The context lent to the call, by its address.
    lent_cx: *const (),
    /// The node being lowered.
    site: NodeRef<'n, Flm>,
    /// The extension and the package that registered the rule.
    owner: &'a Owner,
    /// Every table the rule reads, with its value type.
    tables: &'a [(String, TableType)],
    /// The elements the rule produces whose payload is a [`DataPayload`].
    data_elements: &'a [String],
    /// The nodes this call staged.
    staged: HashSet<BuildId>,
    /// The nodes this call placed, as content or in its answer.
    placed: HashSet<BuildId>,
    /// The first `flm-core` error met while staging.
    failure: Option<LowerError>,
}

impl LowererFacade<'_, '_> {
    /// Runs `f` with `flm-core`'s context behind `cx`, which must be the one
    /// lent to the call, and `flm-core`'s lowerer; keeps a `flm-core` error.
    fn with_cx<T>(
        &mut self,
        method: &str,
        cx: &mut dyn LowerCx,
        f: impl FnOnce(&mut CoreLowerCx<'_>, &mut dyn CoreLowerer) -> Result<T, LowerError>,
    ) -> Result<T, ExtError> {
        let lent = self.lent_cx;
        let lw = &mut *self.lw;
        let mut f = Some(f);
        let mut answer = None;
        cx.with_core(Token::new(), &mut |core_cx| {
            if address_of(core_cx) == lent
                && let Some(f) = f.take()
            {
                answer = Some(f(core_cx, &mut *lw));
            }
        });
        match answer {
            None => Err(argument(method, "`cx` must be the context this rule call was given")),
            Some(Ok(value)) => Ok(value),
            Some(Err(error)) => {
                let reported = lower_error(&error);
                self.failure.get_or_insert(error);
                Err(reported)
            }
        }
    }

    /// Records `ids` as staged by this call; answers them.
    fn mint(&mut self, ids: Vec<BuildId>) -> Vec<Staged> {
        self.staged.extend(ids.iter().copied());
        ids.into_iter().map(Staged).collect()
    }

    /// Records `id` as staged by this call; answers it.
    fn mint_one(&mut self, id: BuildId) -> Staged {
        self.staged.insert(id);
        Staged(id)
    }

    /// The ids of `staged`, each staged by this call and not yet placed,
    /// recorded as placed; the method's failure otherwise (a node staged by
    /// another call, or placed twice, would break the tree being built).
    fn placeable(&mut self, method: &str, staged: &[Staged]) -> Result<Vec<BuildId>, ExtError> {
        let ids = self.check_placeable(method, staged)?;
        self.placed.extend(ids.iter().copied());
        Ok(ids)
    }

    /// [`placeable`](LowererFacade::placeable) without the recording.
    fn check_placeable(&self, method: &str, staged: &[Staged]) -> Result<Vec<BuildId>, ExtError> {
        let mut ids: Vec<BuildId> = Vec::with_capacity(staged.len());
        let mut seen = HashSet::new();
        for Staged(id) in staged {
            if !self.staged.contains(id) {
                return Err(argument(method, "each node placed must have been created during this same rule call"));
            }
            if self.placed.contains(id) || !seen.insert(*id) {
                return Err(argument(method, "a node may be placed only once"));
            }
            ids.push(*id);
        }
        Ok(ids)
    }

    /// The table `name` and the value type its read declares (an extension's
    /// own for a table the rule does not read, which `flm-core` then refuses).
    fn read(&self, name: &str) -> Result<Option<(&(dyn Any + Send + Sync), TableType)>, ExtError> {
        let ty = self
            .tables
            .iter()
            .find(|(table, _)| table == name)
            .map_or_else(TableType::of::<ExtTable>, |(_, ty)| *ty);
        let reference = table_ref(&(name.to_string(), ty));
        let table = self.lw.get_erased(&reference).map_err(|error| ExtError::Core {
            kind: "access".to_string(),
            detail: error.to_string(),
            data: Value::Null,
        })?;
        Ok(table.map(|table| (table, ty)))
    }

    /// The projection of `table`, of the type `ty`, as data.
    fn projection(&self, table: &(dyn Any + Send + Sync), ty: TableType) -> Result<Value, ExtError> {
        let projection = ty.project(table, &LoweringProjectCx { site: self.site, tree: self.lw.tree_id() });
        projection_value(&projection)
    }

    /// Stages through `f`, a method of `flm-core`'s lowerer answering a list,
    /// and records what it staged.
    fn staging(
        &mut self,
        method: &str,
        cx: &mut dyn LowerCx,
        f: impl FnOnce(&mut CoreLowerCx<'_>, &mut dyn CoreLowerer) -> Result<Vec<BuildId>, LowerError>,
    ) -> Result<Vec<Staged>, ExtError> {
        let ids = self.with_cx(method, cx, f)?;
        Ok(self.mint(ids))
    }

    /// Stages a presentation node: the payload read from data (the value
    /// itself, checked, for an element of `data_elements`; through the
    /// payload table otherwise), the content checked, then `flm-core`'s
    /// `pres` or `pres_slots`.
    fn stage_pres(
        &mut self,
        method: &str,
        cx: &mut dyn LowerCx,
        element: &str,
        payload: &Value,
        slots: Vec<(String, Vec<Staged>)>,
        with_slots: bool,
    ) -> Result<Staged, ExtError> {
        let payload: Arc<dyn PresPayload> = if self.data_elements.iter().any(|name| name == element) {
            check_data(payload)?;
            Arc::new(DataPayload(payload.clone()))
        } else {
            payload_from_value(method, element, payload)?
        };
        let mut checked = Vec::with_capacity(slots.len());
        let mut all: Vec<Staged> = Vec::new();
        for (name, nodes) in slots {
            all.extend(nodes.iter().copied());
            checked.push((name, nodes.iter().map(|Staged(id)| *id).collect::<Vec<_>>()));
        }
        self.check_placeable(method, &all)?;
        let element = ElementName::from(element);
        let id = self.with_cx(method, cx, |cx, lw| {
            if with_slots {
                lw.pres_slots(cx, &element, payload, checked)
            } else {
                let body = checked.into_iter().flat_map(|(_, ids)| ids).collect();
                lw.pres(cx, &element, payload, body)
            }
        })?;
        self.placed.extend(all.iter().map(|Staged(id)| *id));
        Ok(self.mint_one(id))
    }
}

impl Sealed for LowererFacade<'_, '_> {}

impl DurableOwner for LowererFacade<'_, '_> {
    fn durable_table(&self, _: Token) -> Arc<DurableTable> {
        Arc::clone(&self.owner.durable)
    }
}

impl LowererCore for LowererFacade<'_, '_> {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&dyn CoreLowerer)) {
        f(&*self.lw);
    }

    fn stage_core(
        &mut self,
        _: Token,
        method: &str,
        cx: &mut dyn LowerCx,
        f: &mut CorePart<'_>,
    ) -> Result<Vec<Staged>, ExtError> {
        self.staging(method, cx, |cx, lw| f(cx, lw))
    }

    fn stage_core_placing(
        &mut self,
        _: Token,
        method: &str,
        cx: &mut dyn LowerCx,
        content: &[Staged],
        f: &mut CorePart<'_>,
    ) -> Result<Vec<Staged>, ExtError> {
        self.check_placeable(method, content)?;
        let staged = self.staging(method, cx, |cx, lw| f(cx, lw))?;
        self.placed.extend(content.iter().map(|Staged(id)| *id));
        Ok(staged)
    }

    /// The numbering feature's counters and the references feature's
    /// anchors are answered as their projections: the counters table cannot
    /// be copied (it holds the compiled joiners of the run, and is not
    /// `Clone`), and the façade of either reads a point against the tree
    /// being lowered, which the lowerer lends only for a borrow. The
    /// dispatcher's `get` records such a table instead, and asks the
    /// lowerer for it again on every call on it
    /// ([`TableEntry::Recorded`](crate::adapter::handles::TableEntry::Recorded)).
    fn get_owned<'s>(&self, _: Token, name: &str) -> Result<Option<TableValue<'s>>, ExtError> {
        let Some((table, ty)) = self.read(name)? else { return Ok(None) };
        if let Some(table) = table.downcast_ref::<ExtTable>() {
            return Ok(Some(TableValue::Ext(Box::new(ItemsCopy(table.items().to_vec())))));
        }
        if let Some(table) = table.downcast_ref::<CoreTreesTable>() {
            let trees = table.trees.iter().map(|(id, tree)| (*id, Arc::clone(tree))).collect();
            return Ok(Some(TableValue::Trees(Box::new(TreesCopy(trees)))));
        }
        if let Some(table) = table.downcast_ref::<CoreLoweredTrees>() {
            return Ok(Some(TableValue::Lowered(Box::new(LoweredTreesFacade::new(table)))));
        }
        Ok(Some(TableValue::Projection(self.projection(table, ty)?)))
    }
}

impl Lowerer for LowererFacade<'_, '_> {
    fn rule(&self) -> String {
        self.lw.rule().to_string()
    }

    fn backend(&self) -> String {
        self.lw.backend().to_string()
    }

    fn comment_start(&self) -> Option<String> {
        self.lw.comment_start().map(str::to_string)
    }

    fn lang(&self) -> Box<dyn BuiltLang + '_> {
        Box::new(BuiltLangFacade { lang: self.lw.lang(), durable: Arc::clone(&self.owner.durable) })
    }

    fn tree_id(&self) -> String {
        self.lw.tree_id().to_string()
    }

    fn addr(&self) -> NodeAddr {
        self.lw.addr()
    }

    fn addr_of(&self, node: &dyn Node<'_>) -> Result<NodeAddr, ExtError> {
        self.lw.addr_of(&node.core_node_ref(Token::new())).map_err(|error| lower_error(&error))
    }

    fn placement(&self) -> Option<String> {
        self.lw.placement().map(|id| id.to_string())
    }

    fn get(&self, name: &str) -> Result<Option<TableValue<'_>>, ExtError> {
        let Some((table, ty)) = self.read(name)? else { return Ok(None) };
        if let Some(table) = table.downcast_ref::<ExtTable>() {
            return Ok(Some(TableValue::Ext(Box::new(table))));
        }
        if let Some(table) = table.downcast_ref::<CoreTreesTable>() {
            return Ok(Some(TableValue::Trees(Box::new(TreesTableFacade::new(table)))));
        }
        if let Some(table) = table.downcast_ref::<CoreLoweredTrees>() {
            return Ok(Some(TableValue::Lowered(Box::new(LoweredTreesFacade::new(table)))));
        }
        let seen = Seen::Rule { id: self.lw.tree_id(), tree: self.site.tree() };
        if let Some(table) = table.downcast_ref::<CoreCounters>() {
            return Ok(Some(TableValue::Counters(Box::new(CountersFacade::new(table, seen)))));
        }
        if let Some(table) = table.downcast_ref::<CoreAnchors>() {
            return Ok(Some(TableValue::Anchors(Box::new(AnchorsFacade::new(table, seen)))));
        }
        Ok(Some(TableValue::Projection(self.projection(table, ty)?)))
    }

    fn producer_of(&self, table: &str) -> Option<String> {
        self.lw.producer_of(&CapName::from(table)).map(|stage| stage.to_string())
    }

    fn option(&self, name: &str) -> Result<Value, ExtError> {
        let lw: &dyn CoreLowerer = &*self.lw;
        lw.option_named(name).cloned().map_err(|error| ExtError::Core {
            kind: "option".to_string(),
            detail: error.to_string(),
            data: Value::Null,
        })
    }

    fn lower_children(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
        let node = node.core_node_ref(Token::new());
        self.staging("lower_children", cx, |cx, lw| lw.lower_children(cx, &node))
    }

    fn lower_body(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
        let node = node.core_node_ref(Token::new());
        self.staging("lower_body", cx, |cx, lw| lw.lower_body(cx, &node))
    }

    fn lower_arg(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>, index: usize) -> Result<Vec<Staged>, ExtError> {
        let node = node.core_node_ref(Token::new());
        self.staging("lower_arg", cx, |cx, lw| lw.lower_arg(cx, &node, index))
    }

    fn lower_arg_named(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>, name: &str) -> Result<Vec<Staged>, ExtError> {
        let node = node.core_node_ref(Token::new());
        self.staging("lower_arg_named", cx, |cx, lw| lw.lower_arg_named(cx, &node, name))
    }

    fn lower_nodes<'t>(&mut self, cx: &mut dyn LowerCx, nodes: &[Box<dyn Node<'t> + 't>]) -> Result<Vec<Staged>, ExtError> {
        const METHOD: &str = "lower_nodes";
        let nodes: Vec<NodeRef<'_, Flm>> = nodes.iter().map(|node| node.core_node_ref(Token::new())).collect();
        // As `flm-core`'s `lower_run` tests it: one tree, consecutive indices,
        // and the range a slice of siblings of that tree.
        let one_tree = nodes.windows(2).all(|pair| {
            std::ptr::eq(pair[0].tree(), pair[1].tree()) && pair[1].id().index() == pair[0].id().index() + 1
        });
        let siblings = match (nodes.first(), nodes.last()) {
            (Some(first), Some(last)) => u32::try_from(first.id().index())
                .ok()
                .zip(u32::try_from(last.id().index()).ok())
                .is_some_and(|(from, to)| first.tree().slice(from..to + 1).is_some()),
            _ => true,
        };
        if !(one_tree && siblings) {
            return Err(argument(METHOD, "the nodes must be consecutive siblings in one tree"));
        }
        self.staging(METHOD, cx, |cx, lw| lower_run(cx, lw, &nodes))
    }

    fn default(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
        let node = node.core_node_ref(Token::new());
        self.staging("default", cx, |cx, lw| lw.default(cx, &node))
    }

    fn pres(&mut self, cx: &mut dyn LowerCx, element: &str, payload: &Value, children: Vec<Staged>) -> Result<Staged, ExtError> {
        self.stage_pres("pres", cx, element, payload, vec![(String::new(), children)], false)
    }

    fn pres_slots(
        &mut self,
        cx: &mut dyn LowerCx,
        element: &str,
        payload: &Value,
        slots: Vec<(String, Vec<Staged>)>,
    ) -> Result<Staged, ExtError> {
        self.stage_pres("pres_slots", cx, element, payload, slots, true)
    }

    fn text(&mut self, cx: &mut dyn LowerCx, text: &str) -> Result<Staged, ExtError> {
        let id = self.with_cx("text", cx, |cx, lw| lw.text(cx, text))?;
        Ok(self.mint_one(id))
    }

    fn splice(&mut self, cx: &mut dyn LowerCx, value: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
        const METHOD: &str = "splice";
        let root = value.core_node_ref(Token::new());
        if root.parent().is_some() {
            return Err(argument(METHOD, "`value` must be the root node of its tree"));
        }
        let tree: Arc<NodeTree<Flm>> = Arc::new(root.tree().clone());
        self.staging(METHOD, cx, |cx, lw| lw.splice(cx, &tree))
    }

    fn lower_region(&mut self, cx: &mut dyn LowerCx, at: NodeAddr, region: NodeRegion) -> Result<Vec<Staged>, ExtError> {
        self.staging("lower_region", cx, |cx, lw| {
            let mut shape = lower_region_content;
            lw.lower_region(cx, at, region, &mut shape)
        })
    }

    fn require(&mut self, id: &str) {
        self.lw.require(&Name::from(id));
    }

    fn diagnose(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError> {
        self.owner.check("diagnostic", &diagnostic.id)?;
        let diagnostic = diagnostic_to_core(diagnostic, self.site.span().clone())?;
        self.lw.diagnose(diagnostic);
        Ok(())
    }
}

// --- what a read answers -----------------------------------------------------------------------

/// An extension's own table, as a copy of its items: what
/// [`LowererCore::get_owned`] answers.
struct ItemsCopy(Vec<Value>);

impl Sealed for ItemsCopy {}

impl ExtTableRead for ItemsCopy {
    fn items(&self) -> Vec<Value> {
        self.0.clone()
    }

    fn items_at(&self, addr: &NodeAddr) -> Vec<Value> {
        let key = node_addr_to_value(addr);
        self.0.iter().filter(|item| item.get("addr") == Some(&key)).cloned().collect()
    }
}

/// A rewrite's trees, as their shared trees: what
/// [`LowererCore::get_owned`] answers.
struct TreesCopy(Vec<(TreeId, Arc<NodeTree<Flm>>)>);

impl Sealed for TreesCopy {}

impl<'s> TreesTable<'s> for TreesCopy {
    fn trees(&self) -> Vec<(String, Box<dyn Node<'s> + 's>)> {
        self.0.iter().map(|(id, tree)| (id.to_string(), root_of(tree))).collect()
    }

    fn tree(&self, id: &str) -> Option<Box<dyn Node<'s> + 's>> {
        let id = tree_id_of(id)?;
        self.0.iter().find(|(held, _)| *held == id).map(|(_, tree)| root_of(tree))
    }
}

/// `flm-core`'s [`ProjectCx`] for a table a rule reads: a node of the tree
/// being lowered renders as `elem@line:col` ([`node_ref_value`], default
/// options); any other address renders as `?@<tree>/<node index>`, as the
/// library's own projection does for an address no tree resolves. A tree
/// renders as its id's display text.
struct LoweringProjectCx<'n> {
    /// The node being lowered, whose tree is `tree`.
    site: NodeRef<'n, Flm>,
    /// The id of the tree being lowered.
    tree: TreeId,
}

impl ProjectCx for LoweringProjectCx<'_> {
    fn node_ref(&self, addr: NodeAddr) -> SerialValue {
        let tree = self.site.tree();
        if addr.tree == self.tree && tree.get(addr.node).is_some() {
            node_ref_value(tree, addr.node, &ProjectOptions::default())
        } else {
            SerialValue::Str(format!("?@{}/{}", addr.tree, addr.node.index()))
        }
    }

    fn tree_ref(&self, id: TreeId) -> SerialValue {
        tree_ref_value(id)
    }
}
