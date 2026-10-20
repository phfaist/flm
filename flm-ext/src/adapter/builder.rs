//! `BuilderFacade`, the extension API's
//! [`FeatureBuilder`] over
//! `flm-core`'s builder.
//!
//! Each method checks the name rule ([`names::check`](crate::adapter::names::check)) on the names the
//! feature declares, converts the declaration, and calls the `flm-core`
//! builder's method of the same name. An [`ElementDef`] a helper made
//! (`UserDefinition::element_def` or `element_def_with`) becomes its `flm-core`
//! declaration here (plan A, A10), with the hook wrapped as an
//! [`ExtExpand`] for the package the façade
//! knows; its definition joins the list the late-init hook compiles unless an
//! expansion hook expands it and it declares no template. A lowering rule the
//! extension built, every stage and every backend are registered in the
//! build's second round ([`RuleFinalizer`], `StageFinalizer`,
//! `BackendFinalizer`).
//!
//! In the second round (the builder a [`Finalizer`] of the extension is
//! lent), the façade registers those at once instead, since a finalizer
//! cannot register another: it knows the round from whether the assembled
//! services are present. The extension's late-initialization hooks and
//! finalizers are wrapped as `ExtLateInit` and `ExtFinalizer`.

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::TableType;
use flm_core::core::region::Regions;
use flm_core::stdcore::backends::lowering::LoweredTrees;
use flm_core::stdcore::backends::pieces::stage::LoweredPieces;
use flm_core::core::registry::{
    BackendDef as CoreBackendDef, BuildError, CapabilityDef, DefinitionText, FeatureBuilder as CoreFeatureBuilder,
    Finalizer, LateInit as CoreLateInit, LateInitError,
};
use flm_core::core::stage::Stage as CoreStage;
use flm_core::lang::mint::ScopedPackageDef as CoreScopedPackageDef;
use flm_core::lang::{BuiltLang as CoreBuiltLang, FlmDiagnostics as CoreFlmDiagnostics};
use flm_core::lang::rewrites::userdefs::Expand as CoreExpand;
use flm_core::model::aspects::{BlockRoleAspect, Referenceable as CoreReferenceable, Sectioning as CoreSectioning};
use flm_core::model::options::TypeKey;
use flm_core::lang::rewrites::userdefs::UserDefinition as CoreUserDefinition;

use crate::adapter::aspects::{ExtReferenceable, ExtSectioning};
use crate::adapter::diag::HANDLER_FAILED;
use crate::adapter::expand::ExtExpand;
use crate::adapter::names::Owner;
use crate::adapter::payloads::DataPayload;
use crate::adapter::services::{self, DeclaredServices, How};
use crate::adapter::stage::nowhere;
use crate::adapter::stage::{ExtStage, WriteType};
use crate::adapter::table::ExtTable;
use crate::adapter::values::{diagnostic_to_core, element_def_to_core, element_name_of, fallback_def_to_core};
use crate::api::core::registry::{
    BackendDef, BackendStage, CallableKind, Contributed, FeatureBuilder, Finalizer as ExtFinalizerHook,
    LateInit as ExtLateInitHook, OutputDef, ScriptDef, StandardTable, StyleDef,
};
use crate::api::core::services::Arity;
use crate::api::core::stage::{Needs, RewriteStage, Stage, StageDecl};
use crate::adapter::rewrite::ExtRewriteStage;
use crate::api::lang::mint::{ElementDef, ElementDefForm, FallbackDef, ScopedPackageDef};
use crate::api::lang::{BuiltLang, FlmDiagnostics};
use crate::api::model::element::Aspect;
use crate::api::techy::error::Diagnostic;
use crate::adapter::lower::RuleFinalizer;
use crate::api::core::lowering::{LoweringRule, RuleForm};
use crate::runtime::ExtError;
use crate::adapter::durable::DurableTable;
use crate::sealed::{BuiltLangCore, DurableOwner, Sealed, Token};

/// The façade a `register` handler declares through: the `flm-core` builder
/// of the feature being registered, the extension's stem and the package's
/// name, the host services the package declared (which its stages and
/// handler objects lend), the feature's final configuration (which a
/// template module's `describe` receives), and the user definitions whose
/// elements were registered so far.
pub(crate) struct BuilderFacade<'a, 'b> {
    b: &'a mut CoreFeatureBuilder<'b>,
    owner: &'a Owner,
    services: &'a Arc<DeclaredServices>,
    configuration: &'a serde_json::Value,
    definitions: Vec<Arc<CoreUserDefinition>>,
}

impl<'a, 'b> BuilderFacade<'a, 'b> {
    /// The façade over `b` for a feature of the package and extension `owner`,
    /// registered with the final configuration `configuration`, whose stages
    /// and handler objects lend the host services `services`.
    pub(crate) fn new(
        b: &'a mut CoreFeatureBuilder<'b>,
        owner: &'a Owner,
        services: &'a Arc<DeclaredServices>,
        configuration: &'a serde_json::Value,
    ) -> BuilderFacade<'a, 'b> {
        BuilderFacade { b, owner, services, configuration, definitions: Vec::new() }
    }

    /// Contributes `value` to `slot` as `how` says ([`services::contribute`]).
    fn contribute(&mut self, slot: &str, value: Contributed, how: How) -> Result<(), ExtError> {
        let feature = self.b.feature().to_string();
        let who = services::Contributor {
            owner: self.owner,
            feature: &feature,
            configuration: self.configuration,
            services: self.services,
        };
        services::contribute(self.b, &who, slot, value, how)
    }

    /// The user definitions the late-init hook compiles, in order: those whose
    /// elements were registered, except a definition that an expansion hook
    /// expands and that declares no template. A definition with neither a hook
    /// nor a template is compiled, so `flm-core` refuses it at build with
    /// `DefinitionProblem::NoTemplate`, as the `userdefs` feature does.
    pub(crate) fn into_definitions(self) -> Vec<Arc<CoreUserDefinition>> {
        self.definitions
    }

    /// Registers what `finalizer` registers: through `flm-core`'s finalizer
    /// in the first round, so that it runs in the second; at once in the
    /// second round (the builder lent to a finalizer), where a finalizer
    /// cannot register another.
    fn in_round_two(&mut self, finalizer: Arc<dyn Finalizer>) -> Result<(), ExtError> {
        if self.b.services().is_some() {
            finalizer.finalize(self.b).map_err(core_error)
        } else {
            self.b.finalizer(finalizer).map_err(core_error)
        }
    }

    /// Converts the element declaration `def` and keeps the user definition
    /// behind it as [`element`](FeatureBuilder::element) does (A54 as A63
    /// reads it: compiled unless a hook expands it without a template).
    fn element_to_core(&mut self, def: ElementDef) -> Result<flm_core::lang::mint::ElementDef, ExtError> {
        let hooked = matches!(def.form, ElementDefForm::Helper { hook: Some(_), .. });
        let (core, definition) = element_def_to_core(def, self.owner)?;
        self.definitions.extend(definition.filter(|definition| !hooked || definition.definition().content.is_some()));
        Ok(core)
    }
}

/// A build error of the `flm-core` builder, as the handler sees it.
fn core_error(error: BuildError) -> ExtError {
    ExtError::Core { kind: "build".to_string(), detail: error.to_string(), data: serde_json::Value::Null }
}

impl Sealed for BuilderFacade<'_, '_> {}

impl DurableOwner for BuilderFacade<'_, '_> {
    fn durable_table(&self, _: Token) -> Arc<DurableTable> {
        Arc::clone(&self.owner.durable)
    }
}

impl FeatureBuilder for BuilderFacade<'_, '_> {
    fn feature(&self) -> String {
        self.b.feature().to_string()
    }

    fn element(&mut self, def: ElementDef) -> Result<(), ExtError> {
        self.owner.check("element", element_name_of(&def))?;
        let core = self.element_to_core(def)?;
        self.b.element(core).map_err(core_error)
    }

    fn fallback(&mut self, def: FallbackDef) -> Result<(), ExtError> {
        self.owner.check("element", &def.element.name)?;
        let core = fallback_def_to_core(&def, self.b.feature().clone(), self.owner)?;
        self.b.fallback(core).map_err(core_error)
    }

    fn override_callable(&mut self, kind: CallableKind, name: &str) -> Result<(), ExtError> {
        self.b.override_callable(kind, name).map_err(core_error)
    }

    fn table(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        self.owner.check("table", name)?;
        self.b.table_named::<ExtTable>(name, docs).map(|_| ()).map_err(core_error)
    }

    fn table_refining(&mut self, name: &str, parent: &str, docs: &str) -> Result<(), ExtError> {
        self.owner.check("table", name)?;
        self.b.table_refining_named::<ExtTable>(name, parent, docs).map(|_| ()).map_err(core_error)
    }

    fn definition_text_table(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        self.owner.check("table", name)?;
        self.b.table_named::<DefinitionText>(name, docs).map(|_| ()).map_err(core_error)
    }

    /// The name rule is [`Owner::check_table`]: one of the extension's
    /// namespaces and a dot, or the table `flm.lowered.<backend>` of a
    /// backend the package may declare.
    fn table_named(&mut self, name: &str, ty: StandardTable, docs: &str) -> Result<(), ExtError> {
        self.owner.check_table(name)?;
        let b = &mut *self.b;
        let declared = match ty {
            StandardTable::LoweredTrees => b.table_named::<LoweredTrees>(name, docs).map(|_| ()),
            StandardTable::LoweredPieces => b.table_named::<LoweredPieces>(name, docs).map(|_| ()),
            StandardTable::Regions => b.table_named::<Regions>(name, docs).map(|_| ()),
            StandardTable::ArtifactSet => b.table_named::<ArtifactSet>(name, docs).map(|_| ()),
        };
        declared.map_err(core_error)
    }

    /// Declared with the adapter's [`DataPayload`]; the element's value
    /// `flm-core` answers is not needed, since a rule stages the element by
    /// its name.
    fn presentation_element(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        self.owner.check("element", name)?;
        self.b.presentation_element::<DataPayload>(name, docs).map(|_| ()).map_err(core_error)
    }

    /// Registered by a [`PeerFinalizer`], in the build's second round.
    fn peer_table(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        let peer = PeerFinalizer { kind: PeerKind::Table, name: name.to_string(), docs: docs.to_string() };
        self.in_round_two(Arc::new(peer))
    }

    fn peer_backend(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        self.b.peer_backend(name, docs).map_err(core_error)
    }

    /// Registered by a [`PeerFinalizer`], in the build's second round.
    fn peer_element(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        let peer = PeerFinalizer { kind: PeerKind::Element, name: name.to_string(), docs: docs.to_string() };
        self.in_round_two(Arc::new(peer))
    }

    fn style_def(&mut self, def: StyleDef) -> Result<(), ExtError> {
        self.owner.check("style", def.id.as_str())?;
        self.b.style_def(def).map_err(core_error)
    }

    fn script_def(&mut self, def: ScriptDef) -> Result<(), ExtError> {
        self.owner.check("script", def.id.as_str())?;
        self.b.script_def(def).map_err(core_error)
    }

    /// Registered by a [`StageFinalizer`], in the build's second round, once
    /// every table it writes has its declared type.
    fn stage(&mut self, stage: Arc<dyn Stage>) -> Result<(), ExtError> {
        let pending = PendingStage::read(stage, self.owner, self.services)?;
        self.in_round_two(Arc::new(StageFinalizer { pending }))
    }

    /// The name rule on the stage's name and on the table it publishes;
    /// the declaration and the needs are read once, here, and the stage is
    /// registered in the first round: `flm-core`'s `rewrite_stage` declares
    /// the table of trees itself, a type of its own.
    fn rewrite_stage(&mut self, stage: Arc<dyn RewriteStage>) -> Result<(), ExtError> {
        let decl = stage.decl();
        let needs = stage.needs();
        self.owner.check("stage", &decl.name)?;
        self.owner.check("table", &decl.publishes)?;
        let core = ExtRewriteStage::new(stage, decl, needs, self.owner.clone());
        self.b.rewrite_stage(Arc::new(core)).map_err(core_error)
    }

    /// The name rule on the backend's name ([`Owner::check_backend`]) and on
    /// each of the extension's own stages, as [`stage`](Self::stage) checks
    /// them; registered by a [`BackendFinalizer`], in the build's second
    /// round, once every table an extension's stage writes has its declared
    /// type. The native names are the registry's to check.
    fn backend(&mut self, def: BackendDef) -> Result<(), ExtError> {
        self.owner.check_backend("backend", &def.name)?;
        let owner = self.owner;
        let services = self.services;
        let pending = |stage: BackendStage| -> Result<PendingBackendStage, ExtError> {
            match stage {
                BackendStage::Library(stage) => Ok(PendingBackendStage::Library(stage.0)),
                BackendStage::Own(stage) => PendingStage::read(stage, owner, services).map(|pending| PendingBackendStage::Own(Box::new(pending))),
            }
        };
        let finalizer = BackendFinalizer {
            name: def.name,
            native: def.native,
            lowering: pending(def.lowering)?,
            emit: pending(def.emit)?,
            final_stage: pending(def.final_stage)?,
            docs: def.docs,
        };
        self.in_round_two(Arc::new(finalizer))
    }

    /// The name rule on the output's name ([`Owner::check_backend`]); the
    /// table and the capability are the registry's to check.
    fn output(&mut self, def: OutputDef) -> Result<(), ExtError> {
        self.owner.check_backend("output", def.name.as_str())?;
        self.b.output(def).map_err(core_error)
    }

    fn workflow_lowering(&mut self, workflow: &str, lowering: &serde_json::Value) -> Result<(), ExtError> {
        let selections: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<String>>> =
            serde_json::from_value(lowering.clone()).map_err(|error| ExtError::Value {
                detail: format!("the lowering selections of workflow `{workflow}` must be a map from backend names to {{ prefer: [rule, ...] }}: {error}"),
            })?;
        let mut def = flm_core::core::registry::WorkflowDef::new(workflow);
        for (backend, mut selection) in selections {
            let rules = selection.remove("prefer").unwrap_or_default();
            if let Some(unknown) = selection.keys().next() {
                return Err(ExtError::Value {
                    detail: format!("the lowering selection for backend `{backend}` of workflow `{workflow}` has unknown field `{unknown}`; only `prefer` is accepted"),
                });
            }
            let mut preferences = flm_core::core::registry::LoweringSelection::new();
            for rule in rules {
                preferences = preferences.prefer(rule);
            }
            def = def.lowering(backend, preferences);
        }
        self.b.workflow_fields(def).map_err(core_error)
    }

    fn capability(&mut self, name: &str, docs: &str, unprovided: Option<String>) -> Result<(), ExtError> {
        self.owner.check("capability", name)?;
        let def = CapabilityDef::new(name).docs(docs);
        let def = match unprovided {
            Some(text) => def.unprovided(text),
            None => def,
        };
        self.b.capability(def).map_err(core_error)
    }

    fn argument_role(&mut self, name: &str, docs: &str) -> Result<(), ExtError> {
        self.owner.check("argument role", name)?;
        self.b.argument_role(name, docs).map_err(core_error)
    }

    fn peer_argument_role(&mut self, name: &str) -> Result<(), ExtError> {
        self.b.peer_argument_role(name).map_err(core_error)
    }

    /// A rule a helper made is registered as it is; a rule the extension
    /// built is registered by a [`RuleFinalizer`], in the build's second
    /// round, once every table it reads has its declared type.
    fn lowering_rule(&mut self, rule: LoweringRule) -> Result<(), ExtError> {
        self.owner.check("rule", rule.name())?;
        match rule.form {
            RuleForm::Core(rule) => self.b.lowering_rule(rule).map_err(core_error),
            RuleForm::Ext(rule) => {
                let finalizer = RuleFinalizer { rule, owner: self.owner.clone() };
                self.in_round_two(Arc::new(finalizer))
            }
        }
    }

    fn require_service(&mut self, slot: &str) -> Result<(), ExtError> {
        self.b.require_service_named(slot).map_err(core_error)
    }

    fn digest_input(&mut self, bytes: &[u8]) -> Result<(), ExtError> {
        self.b.digest_input(bytes).map_err(core_error)
    }

    /// The slot holds [`ExtService`](services::ExtService) values.
    fn service_slot(&mut self, name: &str, arity: Arity, docs: &str) -> Result<(), ExtError> {
        self.owner.check("service slot", name)?;
        self.b.service_slot::<services::ExtService>(name, arity, docs).map_err(core_error)
    }

    /// Routed by the slot's name ([`services::contribute`]).
    fn contribute_to(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError> {
        self.contribute(slot, value, How::Append)
    }

    /// Routed by the slot's name ([`services::contribute`]).
    fn contribute_first_to(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError> {
        self.contribute(slot, value, How::First)
    }

    /// Routed by the slot's name ([`services::contribute`]).
    fn peer_slot(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError> {
        self.contribute(slot, value, How::Peer)
    }

    /// Dispatched to `flm-core`'s `attach::<dyn Trait>` by the aspect's
    /// variant, a hook or a handler aspect wrapped for the package as an
    /// element's own aspect is.
    fn attach(&mut self, element: &str, aspect: Aspect) -> Result<(), ExtError> {
        let owner = self.owner.clone();
        let attached = match aspect {
            Aspect::BlockRoleAspect(role) => self.b.attach::<dyn BlockRoleAspect>(element, Arc::new(role)),
            Aspect::Expand(hook) => {
                self.b.attach::<dyn CoreExpand>(element, Arc::new(ExtExpand { hook, owner, definition: None }))
            }
            Aspect::Referenceable(aspect) => {
                self.b.attach::<dyn CoreReferenceable>(element, Arc::new(ExtReferenceable { aspect, owner }))
            }
            Aspect::Sectioning(aspect) => {
                self.b.attach::<dyn CoreSectioning>(element, Arc::new(ExtSectioning { aspect, owner }))
            }
        };
        attached.map_err(core_error)
    }

    fn collection_member(&mut self, collection: &str, member: &str) -> Result<(), ExtError> {
        self.owner.check("capability", member)?;
        self.b.collection_member(collection, member).map_err(core_error)
    }

    fn scoped_package(&mut self, def: ScopedPackageDef) -> Result<(), ExtError> {
        self.owner.check("scoped package", &def.name)?;
        for element in &def.elements {
            self.owner.check("element", element_name_of(element))?;
        }
        let mut elements = Vec::with_capacity(def.elements.len());
        for element in def.elements {
            elements.push(self.element_to_core(element)?);
        }
        let feature = self.b.feature().clone();
        let core = CoreScopedPackageDef { owner: feature, name: def.name.into(), elements, docs: def.docs };
        self.b.scoped_package(core).map_err(core_error)
    }

    fn late_init(&mut self, hook: Arc<dyn ExtLateInitHook>) -> Result<(), ExtError> {
        self.b.late_init(Arc::new(ExtLateInit { hook, owner: self.owner.clone() })).map_err(core_error)
    }

    fn finalizer(&mut self, hook: Arc<dyn ExtFinalizerHook>) -> Result<(), ExtError> {
        let finalizer = ExtFinalizer {
            hook,
            owner: self.owner.clone(),
            services: Arc::clone(self.services),
            configuration: self.configuration.clone(),
        };
        self.b.finalizer(Arc::new(finalizer)).map_err(core_error)
    }
}

/// The error text of an extension's hook that failed during the build:
/// `flm.ext.handler-failed: ` and the error's text, as a failing `register`
/// gives it.
fn handler_failed(error: &ExtError) -> String {
    format!("{HANDLER_FAILED}: {error}")
}

/// `flm-core`'s late-initialization hook over an extension's
/// [`LateInit`](ExtLateInitHook): lends the hook the built language
/// ([`BuiltLang`]) and the build's diagnostics ([`FlmDiagnostics`]) on the
/// build's thread. An `Err` is the build's `LateInitError`, its text
/// `flm.ext.handler-failed: ` and the error's text.
pub(crate) struct ExtLateInit {
    /// The extension's hook.
    pub(crate) hook: Arc<dyn ExtLateInitHook>,
    /// The extension and the package that registered the hook.
    pub(crate) owner: Owner,
}

impl CoreLateInit for ExtLateInit {
    fn late_init(&self, lang: &CoreBuiltLang, diags: &mut CoreFlmDiagnostics) -> Result<(), LateInitError> {
        let lang = BuiltLangFacade { lang, durable: Arc::clone(&self.owner.durable) };
        let mut diags = FlmDiagnosticsFacade { diags, owner: &self.owner };
        self.hook.late_init(&lang, &mut diags).map_err(|error| LateInitError::new(handler_failed(&error)))
    }
}

/// `flm-core`'s finalizer over an extension's [`Finalizer`](ExtFinalizerHook):
/// lends the hook the builder of the second round, a [`BuilderFacade`]
/// over the feature's builder. An `Err` fails the build as a failing
/// `register` does ([`BuildError::Feature`], its detail
/// `flm.ext.handler-failed: ` and the error's text).
pub(crate) struct ExtFinalizer {
    /// The extension's hook.
    pub(crate) hook: Arc<dyn ExtFinalizerHook>,
    /// The extension and the package that registered the hook.
    pub(crate) owner: Owner,
    /// The host services the package declared, which the second round's
    /// builder lends as the first round's does.
    pub(crate) services: Arc<DeclaredServices>,
    /// The feature's final configuration, which the second round's builder
    /// hands a template module's `describe` as the first round's does.
    pub(crate) configuration: serde_json::Value,
}

impl Finalizer for ExtFinalizer {
    fn finalize(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let feature = b.feature().clone();
        let mut facade = BuilderFacade::new(b, &self.owner, &self.services, &self.configuration);
        let result = self.hook.finalize(&mut facade);
        // A definition a finalizer declares is compiled by a late-init hook
        // of its own, as one `register` declares.
        let definitions = facade.into_definitions();
        result.map_err(|error| BuildError::Feature { feature, detail: handler_failed(&error) })?;
        if !definitions.is_empty() {
            b.late_init(Arc::new(crate::adapter::feature::CompileDefinitions::new(definitions, self.owner.clone(), b.feature().to_string())))?;
        }
        Ok(())
    }
}

/// The extension API's [`BuiltLang`] over `flm-core`'s, lent to a
/// late-initialization hook or answered by a lowerer's or an expander's
/// `lang()`, with the durable table of the running handler's feature.
pub(crate) struct BuiltLangFacade<'a> {
    pub(crate) lang: &'a CoreBuiltLang,
    pub(crate) durable: Arc<DurableTable>,
}

impl Sealed for BuiltLangFacade<'_> {}

impl DurableOwner for BuiltLangFacade<'_> {
    fn durable_table(&self, _: Token) -> Arc<DurableTable> {
        Arc::clone(&self.durable)
    }
}

impl BuiltLangCore for BuiltLangFacade<'_> {
    fn core_lang(&self, _: Token) -> &CoreBuiltLang {
        self.lang
    }
}

impl BuiltLang for BuiltLangFacade<'_> {}

/// The extension API's [`FlmDiagnostics`] over the build's diagnostics, lent
/// to a late-initialization hook: each diagnostic's identifier checked
/// against the extension's stem, a diagnostic that names no node placed
/// nowhere ([`nowhere`]).
pub(crate) struct FlmDiagnosticsFacade<'a> {
    diags: &'a mut CoreFlmDiagnostics,
    owner: &'a Owner,
}

impl Sealed for FlmDiagnosticsFacade<'_> {}

impl FlmDiagnostics for FlmDiagnosticsFacade<'_> {
    fn push(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError> {
        self.owner.check("diagnostic", &diagnostic.id)?;
        self.diags.push(diagnostic_to_core(diagnostic, nowhere())?);
        Ok(())
    }
}

/// An extension's stage as the first round read it: the stage, its
/// declaration and its needs, read once, its owner and the host services
/// the package declared. The name rule was
/// checked on its name and on every table it writes.
struct PendingStage {
    stage: Arc<dyn Stage>,
    decl: StageDecl,
    needs: Needs,
    owner: Owner,
    services: Arc<DeclaredServices>,
}

impl PendingStage {
    /// Reads `stage`'s declaration and needs, and checks the name rule on
    /// its name and on every table it writes; the stage's context will lend
    /// the host services `services`.
    fn read(stage: Arc<dyn Stage>, owner: &Owner, services: &Arc<DeclaredServices>) -> Result<PendingStage, ExtError> {
        let decl = stage.decl();
        let needs = stage.needs();
        owner.check("stage", &decl.name)?;
        // A capability the stage provides is named, not declared: it may be
        // another feature's (a processing feature's check provides
        // `flm.math.content`), and the registry's validation refuses one that
        // no feature declares.
        for table in &decl.writes {
            owner.check("table", table)?;
        }
        Ok(PendingStage { stage, decl, needs, owner: owner.clone(), services: Arc::clone(services) })
    }

    /// The `flm-core` stage over the extension's, in the build's second
    /// round: each table it writes typed ([`WriteType`]); a table of a type a
    /// scripted stage cannot write fails the build ([`BuildError::Feature`]).
    fn to_core(&self, b: &CoreFeatureBuilder<'_>) -> Result<ExtStage, BuildError> {
        let mut writes = Vec::with_capacity(self.decl.writes.len());
        for table in &self.decl.writes {
            let declared = b.table_type_named(table);
            let Some(write) = WriteType::of(declared) else {
                let ty = declared.map_or("?", |ty| ty.short_name());
                let Owner { stem, package, .. } = &self.owner;
                let detail = format!(
                    "the stage `{stage}` of the package `{package}` of the extension `{stem}` writes the table \
                     `{table}`, which holds values of the type `{ty}`; a stage of an extension writes a table of its \
                     own, a table of a definition's text or a table of an artifact set",
                    stage = self.decl.name,
                );
                return Err(BuildError::Feature { feature: b.feature().clone(), detail });
            };
            writes.push((table.clone(), write));
        }
        Ok(ExtStage {
            stage: Arc::clone(&self.stage),
            decl: self.decl.clone(),
            needs: self.needs.clone(),
            owner: self.owner.clone(),
            writes: writes.into(),
            services: Arc::clone(&self.services),
        })
    }
}

/// The finalizer that registers an extension's stage with `flm-core`, in the
/// build's second round: every feature has then declared its tables, so each
/// table the stage writes is written with the type its declaration gives (an
/// extension's own, the text of a definition's part ([`DefinitionText`]), or
/// the files of an output ([`ArtifactSet`])) whatever the order of the
/// table's declaration and the stage's in `register` (S25's precedent for a
/// rule's reads). The declaration and the needs were read in the first round.
struct StageFinalizer {
    pending: PendingStage,
}

impl Finalizer for StageFinalizer {
    /// Types each table the stage writes ([`PendingStage::to_core`]) and
    /// registers the stage.
    fn finalize(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let stage = self.pending.to_core(b)?;
        b.stage(Arc::new(stage))
    }
}

/// One stage of a backend as the first round read it.
enum PendingBackendStage {
    /// A stage of the library, registered as it is.
    Library(Arc<dyn CoreStage>),
    /// An extension's own stage, wrapped in the second round.
    Own(Box<PendingStage>),
}

impl PendingBackendStage {
    /// The `flm-core` stage, in the build's second round.
    fn to_core(&self, b: &CoreFeatureBuilder<'_>) -> Result<Arc<dyn CoreStage>, BuildError> {
        match self {
            PendingBackendStage::Library(stage) => Ok(Arc::clone(stage)),
            PendingBackendStage::Own(pending) => Ok(Arc::new(pending.to_core(b)?)),
        }
    }
}

/// The finalizer that registers an extension's backend with `flm-core`, in
/// the build's second round: each of the extension's own stages is wrapped
/// with its typed writes as a [`StageFinalizer`] wraps a stage, each stage of
/// the library is taken as it is, and `flm-core`'s builder registers the
/// three as the feature's stages, in the order lowering, emit, final.
struct BackendFinalizer {
    name: String,
    native: Vec<String>,
    lowering: PendingBackendStage,
    emit: PendingBackendStage,
    final_stage: PendingBackendStage,
    docs: String,
}

impl Finalizer for BackendFinalizer {
    fn finalize(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let lowering = self.lowering.to_core(b)?;
        let emit = self.emit.to_core(b)?;
        let final_stage = self.final_stage.to_core(b)?;
        let def = CoreBackendDef::new(self.name.as_str(), self.native.iter().map(String::as_str), lowering, emit, final_stage)
            .docs(self.docs.as_str());
        b.backend(def)
    }
}

/// Which peer declaration a [`PeerFinalizer`] makes.
#[derive(Debug, Clone, Copy)]
enum PeerKind {
    /// A peer table.
    Table,
    /// A peer presentation element.
    Element,
}

/// The finalizer that makes a peer declaration an extension made by name, in
/// the build's second round: every feature has then declared its tables and
/// presentation elements, so the declaration takes the type the owner declared
/// (the table's value type, the element's payload type), as `flm-core`'s
/// resolution of peers requires. A name no feature declares gets a type of
/// the adapter's own (`ExtTable` for a table, the unit type for an element)
/// and stays unresolved.
struct PeerFinalizer {
    kind: PeerKind,
    name: String,
    docs: String,
}

impl Finalizer for PeerFinalizer {
    fn finalize(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let name = self.name.as_str();
        match self.kind {
            PeerKind::Table => {
                let value = b.table_type_named(name).unwrap_or_else(TableType::of::<ExtTable>);
                b.peer_table_of(name, value, self.docs.as_str())
            }
            PeerKind::Element => {
                let payload = b.presentation_payload_named(name).unwrap_or_else(TypeKey::of::<()>);
                b.peer_element_of(name, payload, self.docs.as_str())
            }
        }
    }
}
