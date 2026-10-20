//! The guest's own types of `flm:core/registry` that hold handler objects
//! ([`BackendDef`], [`BackendStage`], [`Contributed`]), the handler traits
//! [`LateInit`] and [`Finalizer`], and the record-and-lend methods of the
//! builder (`FeatureBuilder::feature`, `stage`, `rewrite_stage`,
//! `contribute_to`, `contribute_first_to`, `peer_slot`, `attach`,
//! `late_init`, `finalizer`) and of a backend (`BackendDef::new`), the twins
//! of the prelude's fragment `core-registry.js`: each records its handler
//! object in the running `register`'s record, then lends it and sends the
//! call unless the builder is a rehearsal's, which sends nothing.

use std::fmt;
use std::sync::Arc;

use serde_json::{json, Value};

use super::arg;
use crate::adapter::values::JsonForm;
use crate::guest::abi::{self, ToArg};
use crate::guest::core::registry::{FeatureBuilder, LibraryStage};
use crate::guest::core::stage::{RewriteStage, Stage};
use crate::guest::model::element::Aspect;
use crate::guest::registry;
use crate::guest::lang::{BuiltLang, FlmDiagnostics};
use crate::guest::stdcore::features::citations::CitationProvider;
use crate::guest::stdcore::features::compose::TemplateModule;
use crate::runtime::ExtError;

/// Mirrors [`LateInit`](crate::api::core::registry::LateInit): a hook that
/// runs once the language is built.
pub trait LateInit: Send + Sync {
    /// Mirrors [`LateInit::late_init`](crate::api::core::registry::LateInit::late_init).
    ///
    /// # Errors
    ///
    /// What the hook answers.
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), ExtError>;
}

/// Mirrors [`Finalizer`](crate::api::core::registry::Finalizer): a hook that
/// runs in the second round of the registration.
pub trait Finalizer: Send + Sync {
    /// Mirrors [`Finalizer::finalize`](crate::api::core::registry::Finalizer::finalize).
    ///
    /// # Errors
    ///
    /// What the hook answers.
    fn finalize(&self, b: &mut FeatureBuilder) -> Result<(), ExtError>;
}

/// Mirrors [`BackendStage`](crate::api::core::registry::BackendStage): a
/// stage of a backend, the library's or the extension's own.
#[derive(Clone)]
pub enum BackendStage {
    /// A stage of the library.
    Library(LibraryStage),
    /// The extension's own stage.
    Own(Arc<dyn Stage>),
}

impl BackendStage {
    /// Whether `self` is `other`: a library stage by its handle, an own stage
    /// by its object.
    fn is(&self, other: &BackendStage) -> bool {
        match (self, other) {
            (BackendStage::Library(stage), BackendStage::Library(other)) => stage == other,
            (BackendStage::Own(stage), BackendStage::Own(other)) => Arc::ptr_eq(stage, other),
            _ => false,
        }
    }

    /// The stage's crossing form, an own stage recorded under its name and,
    /// unless the running `register` is a rehearsal, lent (`null` in a
    /// rehearsal, whose builder sends nothing).
    fn lent(&self) -> Result<Value, ExtError> {
        match self {
            BackendStage::Library(stage) => Ok(stage.to_value()),
            BackendStage::Own(stage) => {
                let decl = stage.decl();
                let needs = stage.needs();
                registry::record_stage(&decl.name, Arc::clone(stage))?;
                if registry::rehearsing() { Ok(Value::Null) } else { registry::lend_stage(&decl, &needs) }
            }
        }
    }
}

/// As the api writes it: an own stage by its declaration's name.
impl fmt::Debug for BackendStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendStage::Library(stage) => f.debug_tuple("Library").field(stage).finish(),
            BackendStage::Own(stage) => f.debug_tuple("Own").field(&stage.decl().name).finish(),
        }
    }
}

/// Mirrors [`BackendDef`](crate::api::core::registry::BackendDef): a backend
/// an extension declares. It keeps, beside each stage, the stage given to
/// [`BackendDef::new`] and the handle its lending answered (a library
/// stage's own handle; an own stage's lent one), which is what crosses; a
/// stage field changed since is refused when the backend is sent.
#[derive(Clone)]
pub struct BackendDef {
    /// The backend's name.
    pub name: String,
    /// The names of the presentation elements the backend writes natively.
    pub native: Vec<String>,
    /// The lowering stage.
    pub lowering: BackendStage,
    /// The emit stage.
    pub emit: BackendStage,
    /// The final stage.
    pub final_stage: BackendStage,
    /// The backend's documentation.
    pub docs: String,
    /// The stages given to [`BackendDef::new`], `lowering`, `emit` and
    /// `final_stage` in that order, each beside its crossing form: its
    /// handle.
    pub(crate) stages: [(BackendStage, Value); 3],
}

impl BackendDef {
    /// Mirrors [`BackendDef::new`](crate::api::core::registry::BackendDef::new):
    /// the backend `name`, writing the elements `native` directly, with its
    /// three stages and no documentation. Each own stage is recorded under
    /// its name and lent, as [`FeatureBuilder::stage`] does, so the backend
    /// is made while a feature's `register` runs (in a rehearsal it is
    /// recorded and not lent).
    ///
    /// # Errors
    ///
    /// An own stage given while no `register` runs ([`ExtError::Handler`]),
    /// or the host's refusal of its lending.
    pub fn new(
        name: impl Into<String>,
        native: Vec<String>,
        lowering: BackendStage,
        emit: BackendStage,
        final_stage: BackendStage,
    ) -> Result<BackendDef, ExtError> {
        let stages = [
            (lowering.clone(), lowering.lent()?),
            (emit.clone(), emit.lent()?),
            (final_stage.clone(), final_stage.lent()?),
        ];
        Ok(BackendDef { name: name.into(), native, lowering, emit, final_stage, docs: String::new(), stages })
    }

    /// Mirrors [`BackendDef::docs`](crate::api::core::registry::BackendDef::docs).
    pub fn docs(mut self, docs: impl Into<String>) -> BackendDef {
        self.docs = docs.into();
        self
    }
}

/// As the api writes it: the name and the native elements.
impl fmt::Debug for BackendDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendDef").field("name", &self.name).field("native", &self.native).finish_non_exhaustive()
    }
}

/// `{"name", "native", "lowering", "emit", "final_stage", "docs"}`, each
/// stage its handle; read by the host's `backend_def_from_table`. A backend
/// crosses from the guest only. A stage field that is no longer the stage
/// given to [`BackendDef::new`] is refused, naming the field.
impl ToArg for BackendDef {
    fn arg(&self) -> Result<Value, ExtError> {
        let fields = [("lowering", &self.lowering), ("emit", &self.emit), ("final_stage", &self.final_stage)];
        for ((field, stage), (given, _)) in fields.into_iter().zip(&self.stages) {
            if !stage.is(given) {
                return Err(ExtError::Argument {
                    method: String::new(),
                    detail: format!("the stage `{field}` of the backend `{}` must be the stage given to `BackendDef::new`", self.name),
                });
            }
        }
        let [(_, lowering), (_, emit), (_, final_stage)] = &self.stages;
        Ok(json!({
            "name": self.name,
            "native": self.native,
            "lowering": lowering,
            "emit": emit,
            "final_stage": final_stage,
            "docs": self.docs,
        }))
    }
}

/// Mirrors [`Contributed`](crate::api::core::registry::Contributed): what a
/// feature contributes to a service slot. The methods that take it record
/// and lend a handler object.
#[derive(Clone)]
pub enum Contributed {
    /// Data: the value of a slot.
    Data(Value),
    /// A citation provider, for `flm.citations.providers`.
    CitationProvider(Arc<dyn CitationProvider>),
    /// A compose template module, for `flm.compose.templates`.
    TemplateModule(Arc<dyn TemplateModule>),
}

/// The record-and-lend methods of the builder (`GUEST_HAND`), the twins of
/// `core-registry.js`: each records its object, then sends unless the
/// builder is a rehearsal's (its handle `None`).
impl FeatureBuilder {
    /// Mirrors [`FeatureBuilder::feature`](crate::api::core::registry::FeatureBuilder::feature):
    /// the name of the feature whose `register` runs (in a rehearsal, without
    /// a request).
    ///
    /// # Errors
    ///
    /// The host's refusal; in a rehearsal, none.
    pub fn feature(&self) -> Result<String, ExtError> {
        match self.handle {
            None => registry::registering_feature(),
            Some(handle) => JsonForm::from_value(&abi::call(handle, "feature", vec![])?),
        }
    }

    /// Mirrors [`FeatureBuilder::stage`](crate::api::core::registry::FeatureBuilder::stage):
    /// declares the stage `stage`, recorded under its name so that an
    /// instance on another thread finds it again, and lent.
    ///
    /// # Errors
    ///
    /// The host's refusal.
    pub fn stage(&mut self, stage: Arc<dyn Stage>) -> Result<(), ExtError> {
        let decl = stage.decl();
        let needs = stage.needs();
        registry::record_stage(&decl.name, stage)?;
        let Some(handle) = self.handle else { return Ok(()) };
        let lent = registry::lend_stage(&decl, &needs)?;
        abi::call(handle, "stage", vec![lent])?;
        Ok(())
    }

    /// Mirrors [`FeatureBuilder::rewrite_stage`](crate::api::core::registry::FeatureBuilder::rewrite_stage):
    /// declares the rewrite stage `stage`, recorded under its stage name, and
    /// lent.
    ///
    /// # Errors
    ///
    /// The host's refusal.
    pub fn rewrite_stage(&mut self, stage: Arc<dyn RewriteStage>) -> Result<(), ExtError> {
        let decl = stage.decl();
        let needs = stage.needs();
        registry::record_rewrite(&decl.name, stage)?;
        let Some(handle) = self.handle else { return Ok(()) };
        let lent = registry::lend_rewrite(&decl, &needs)?;
        abi::call(handle, "rewrite_stage", vec![lent])?;
        Ok(())
    }

    /// Sends `method` with the slot `slot` and the contribution `value`, a
    /// handler object in it recorded and, unless in a rehearsal, lent.
    fn contribute(&mut self, method: &str, slot: &str, value: &Contributed) -> Result<(), ExtError> {
        let sent = registry::contributed(value, self.handle.is_none())?;
        let Some(handle) = self.handle else { return Ok(()) };
        abi::call(handle, method, vec![arg(slot, method)?, sent])?;
        Ok(())
    }

    /// Mirrors [`FeatureBuilder::contribute_to`](crate::api::core::registry::FeatureBuilder::contribute_to):
    /// contributes `value` (data, a citation provider or a template module)
    /// to the service slot `slot`, after every value contributed to it so
    /// far; a handler object is recorded under its name, and lent.
    ///
    /// # Errors
    ///
    /// A second handler object of the kind under one name in one `register`
    /// ([`ExtError::Handler`]), or the host's refusal.
    pub fn contribute_to(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError> {
        self.contribute("contribute_to", slot, &value)
    }

    /// Mirrors [`FeatureBuilder::contribute_first_to`](crate::api::core::registry::FeatureBuilder::contribute_first_to):
    /// contributes `value` to the service slot `slot`, before every value
    /// appended to it, as [`contribute_to`](FeatureBuilder::contribute_to)
    /// sends it.
    ///
    /// # Errors
    ///
    /// As [`contribute_to`](FeatureBuilder::contribute_to).
    pub fn contribute_first_to(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError> {
        self.contribute("contribute_first_to", slot, &value)
    }

    /// Mirrors [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot):
    /// contributes `value` to the service slot `slot`, which another feature
    /// is expected to declare, as [`contribute_to`](FeatureBuilder::contribute_to)
    /// sends it.
    ///
    /// # Errors
    ///
    /// As [`contribute_to`](FeatureBuilder::contribute_to).
    pub fn peer_slot(&mut self, slot: &str, value: Contributed) -> Result<(), ExtError> {
        self.contribute("peer_slot", slot, &value)
    }

    /// Mirrors [`FeatureBuilder::attach`](crate::api::core::registry::FeatureBuilder::attach):
    /// attaches `aspect` to the element `element`, which another feature may
    /// declare; a handler object in it is recorded under `element`, and lent.
    ///
    /// # Errors
    ///
    /// The host's refusal.
    pub fn attach(&mut self, element: &str, aspect: Aspect) -> Result<(), ExtError> {
        let Some(handle) = self.handle else { return registry::record_aspect(element, &aspect) };
        let given = registry::lend_aspect(element, &aspect)?;
        abi::call(handle, "attach", vec![arg(element, "attach")?, given])?;
        Ok(())
    }

    /// Mirrors [`FeatureBuilder::late_init`](crate::api::core::registry::FeatureBuilder::late_init):
    /// registers the late-initialization hook `hook`, recorded by its
    /// position among the feature's hooks, and lent.
    ///
    /// # Errors
    ///
    /// The host's refusal.
    pub fn late_init(&mut self, hook: Arc<dyn LateInit>) -> Result<(), ExtError> {
        let index = registry::record_late_init(hook)?;
        let Some(handle) = self.handle else { return Ok(()) };
        let lent = registry::lend_late_init(index)?;
        abi::call(handle, "late_init", vec![lent])?;
        Ok(())
    }

    /// Mirrors [`FeatureBuilder::finalizer`](crate::api::core::registry::FeatureBuilder::finalizer):
    /// registers the finalizer `hook`, recorded by its position among the
    /// feature's finalizers, and lent; it is called in the build's second
    /// round with the feature's builder.
    ///
    /// # Errors
    ///
    /// The host's refusal.
    pub fn finalizer(&mut self, hook: Arc<dyn Finalizer>) -> Result<(), ExtError> {
        let index = registry::record_finalizer(hook)?;
        let Some(handle) = self.handle else { return Ok(()) };
        let lent = registry::lend_finalizer(index)?;
        abi::call(handle, "finalizer", vec![lent])?;
        Ok(())
    }
}
