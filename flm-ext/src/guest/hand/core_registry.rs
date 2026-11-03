//! The guest's own types of `flm:core/registry` that hold handler objects
//! ([`BackendDef`], [`BackendStage`], [`Contributed`]), the handler traits
//! [`LateInit`] and [`Finalizer`], and the record-and-lend methods of the
//! builder (`FeatureBuilder::feature`, `stage`, `rewrite_stage`, `backend`,
//! `contribute_to`, `contribute_first_to`, `peer_slot`, `attach`,
//! `late_init`, `finalizer`), the twins of the prelude's fragment
//! `core-registry.js`: each records its handler objects in the running
//! `register`'s record, then lends them and sends the call unless the
//! builder is a rehearsal's, which sends nothing. Compiled into a program
//! (the cargo feature `native`), the lending and the call are one typed step
//! on the running call's frame: the natives' body builds each handler
//! object, which the API method takes as it is.

use std::fmt;
use std::sync::Arc;

#[cfg(not(feature = "native"))]
use serde_json::json;
use serde_json::Value;

#[cfg(feature = "native")]
use crate::adapter::dispatch::entry;
#[cfg(feature = "native")]
use crate::adapter::typed::{FromApi, IntoApi};
#[cfg(not(feature = "native"))]
use crate::adapter::values::{handle_to_value, JsonForm};
#[cfg(feature = "native")]
use crate::api;
use crate::api::core::stage::{Needs, StageDecl};
#[cfg(feature = "native")]
use crate::binding::frames::FrameView;
#[cfg(feature = "native")]
use crate::binding::natives::{self, Act};
use crate::guest::abi;
#[cfg(not(feature = "native"))]
use crate::guest::abi::ToArg;
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
    /// Records an own stage under its name, as [`FeatureBuilder::stage`]
    /// records it; answers what lending the stage takes.
    fn recorded(&self) -> Result<StageLending, ExtError> {
        match self {
            BackendStage::Library(stage) => Ok(StageLending::Library(*stage)),
            BackendStage::Own(stage) => {
                let decl = stage.decl();
                let needs = stage.needs();
                registry::record_stage(&decl.name, Arc::clone(stage))?;
                Ok(StageLending::Own(Box::new((decl, needs))))
            }
        }
    }
}

/// What lending a stage of a backend takes once it is recorded: a stage of
/// the library, or the declaration and needs of an own stage, read from the
/// extension's object when it is recorded.
enum StageLending {
    /// A stage of the library.
    Library(LibraryStage),
    /// An own stage, of this declaration and these needs.
    Own(Box<(StageDecl, Needs)>),
}

impl StageLending {
    /// What [`FeatureBuilder::backend`] sends for the stage in a WebAssembly
    /// module: a library stage's handle, an own stage lent (`lend_stage`)
    /// and its handle.
    #[cfg(not(feature = "native"))]
    fn sent(&self) -> Result<Value, ExtError> {
        match self {
            StageLending::Library(stage) => Ok(stage.to_value()),
            StageLending::Own(own) => registry::lend_stage(&own.0, &own.1).map(handle_to_value),
        }
    }

    /// The api's stage: a library stage, the entry of its handle; an own
    /// stage, built on the frame `view` by the natives' body (`lend_stage`).
    #[cfg(feature = "native")]
    fn stage(self, view: &mut FrameView<'_, '_>) -> Result<api::core::registry::BackendStage, ExtError> {
        Ok(match self {
            StageLending::Library(stage) => api::core::registry::BackendStage::Library(stage.into_api(view.table)?),
            StageLending::Own(own) => {
                let (decl, needs) = *own;
                api::core::registry::BackendStage::Own(natives::lend_stage(view, decl, needs))
            }
        })
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
/// an extension declares, plain data: its stages are recorded and lent
/// when it is registered ([`FeatureBuilder::backend`]).
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
}

impl BackendDef {
    /// Mirrors [`BackendDef::new`](crate::api::core::registry::BackendDef::new):
    /// the backend `name`, writing the elements `native` directly, with its
    /// three stages and no documentation.
    pub fn new(
        name: impl Into<String>,
        native: Vec<String>,
        lowering: BackendStage,
        emit: BackendStage,
        final_stage: BackendStage,
    ) -> BackendDef {
        BackendDef { name: name.into(), native, lowering, emit, final_stage, docs: String::new() }
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

/// The builder's methods that contribute to a service slot.
#[derive(Clone, Copy)]
enum Contributing {
    /// `contribute_to`.
    To,
    /// `contribute_first_to`.
    FirstTo,
    /// `peer_slot`.
    PeerSlot,
}

impl Contributing {
    /// The method's name.
    fn method(self) -> &'static str {
        match self {
            Contributing::To => "contribute_to",
            Contributing::FirstTo => "contribute_first_to",
            Contributing::PeerSlot => "peer_slot",
        }
    }

    /// Calls the method on the api's builder `b`.
    #[cfg(feature = "native")]
    fn call(
        self,
        b: &mut dyn api::core::registry::FeatureBuilder,
        slot: &str,
        value: api::core::registry::Contributed,
    ) -> Result<(), ExtError> {
        match self {
            Contributing::To => b.contribute_to(slot, value),
            Contributing::FirstTo => b.contribute_first_to(slot, value),
            Contributing::PeerSlot => b.peer_slot(slot, value),
        }
    }
}

/// The record-and-lend methods of the builder (`GUEST_HAND`), the twins of
/// `core-registry.js`: each records its object, then sends unless the
/// builder is a rehearsal's (its handle `None`). Compiled into a program,
/// each lends and calls in one step on the running call's frame
/// (`abi::in_frame`), refused outside a handler call in the words of the
/// first native it sends as JSON.
impl FeatureBuilder {
    /// Mirrors [`FeatureBuilder::feature`](crate::api::core::registry::FeatureBuilder::feature):
    /// the name of the feature whose `register` runs (in a rehearsal, without
    /// a request).
    ///
    /// # Errors
    ///
    /// The host's refusal; in a rehearsal, none.
    pub fn feature(&self) -> Result<String, ExtError> {
        let Some(handle) = self.handle else { return registry::registering_feature() };
        #[cfg(not(feature = "native"))]
        {
            JsonForm::from_value(&abi::call(handle, "feature", vec![])?)
        }
        #[cfg(feature = "native")]
        {
            abi::typed("feature", |table| {
                let this = entry!(table, handle, Builder, FeatureBuilder);
                let answer = this.feature();
                FromApi::from_api(answer, table)
            })
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
        #[cfg(not(feature = "native"))]
        {
            let lent = registry::lend_stage(&decl, &needs)?;
            abi::call(handle, "stage", vec![handle_to_value(lent)])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(Act::Stage, "stage", |view| {
                let stage = natives::lend_stage(view, decl, needs);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                this.stage(stage)
            })
        }
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
        #[cfg(not(feature = "native"))]
        {
            let lent = registry::lend_rewrite(&decl, &needs)?;
            abi::call(handle, "rewrite_stage", vec![handle_to_value(lent)])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(Act::Rewrite, "rewrite_stage", |view| {
                let stage = natives::lend_rewrite(view, decl, needs);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                this.rewrite_stage(stage)
            })
        }
    }

    /// Mirrors [`FeatureBuilder::backend`](crate::api::core::registry::FeatureBuilder::backend):
    /// registers the backend `def`, its three stages as this feature's own.
    /// The stages are those the fields of `def` hold: each own stage among
    /// them is recorded under its name, as [`stage`](FeatureBuilder::stage)
    /// records it, and lent. In a WebAssembly module the backend crosses as
    /// `{"name", "native", "lowering", "emit", "final_stage", "docs"}`, each
    /// stage its handle, read by the host's `backend_def_from_table`.
    ///
    /// # Errors
    ///
    /// The host's refusal.
    pub fn backend(&mut self, def: BackendDef) -> Result<(), ExtError> {
        let lowering = def.lowering.recorded()?;
        let emit = def.emit.recorded()?;
        let final_stage = def.final_stage.recorded()?;
        let Some(handle) = self.handle else { return Ok(()) };
        #[cfg(not(feature = "native"))]
        {
            let sent = json!({
                "name": def.name,
                "native": def.native,
                "lowering": lowering.sent()?,
                "emit": emit.sent()?,
                "final_stage": final_stage.sent()?,
                "docs": def.docs,
            });
            abi::call(handle, "backend", vec![sent])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            let own = [&lowering, &emit, &final_stage].iter().any(|stage| matches!(stage, StageLending::Own(..)));
            let act = if own { Act::Stage } else { Act::Call("backend") };
            let BackendDef { name, native, docs, .. } = def;
            abi::in_frame(act, "backend", |view| {
                let (lowering, emit, final_stage) = (lowering.stage(view)?, emit.stage(view)?, final_stage.stage(view)?);
                let def = api::core::registry::BackendDef::new(name, native, lowering, emit, final_stage).docs(docs);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                this.backend(def)
            })
        }
    }

    /// Sends the method `contributing` with the slot `slot` and the
    /// contribution `value`, a handler object in it recorded and, unless in a
    /// rehearsal, lent.
    fn contribute(&mut self, contributing: Contributing, slot: &str, value: &Contributed) -> Result<(), ExtError> {
        let lending = registry::record_contributed(value)?;
        let Some(handle) = self.handle else { return Ok(()) };
        let method = contributing.method();
        #[cfg(not(feature = "native"))]
        {
            abi::call(handle, method, vec![ToArg::arg(slot), lending.sent()?])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(lending.act(method), method, |view| {
                let value = lending.contributed(view);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                contributing.call(&mut **this, slot, value)
            })
        }
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
        self.contribute(Contributing::To, slot, &value)
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
        self.contribute(Contributing::FirstTo, slot, &value)
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
        self.contribute(Contributing::PeerSlot, slot, &value)
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
        let lending = registry::record_lent_aspect(element, &aspect)?;
        #[cfg(not(feature = "native"))]
        {
            let given = lending.form(element)?;
            abi::call(handle, "attach", vec![ToArg::arg(element), given.to_value()])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(lending.act(element, "attach"), "attach", |view| {
                let aspect = lending.aspect(view, element);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                this.attach(element, aspect)
            })
        }
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
        #[cfg(not(feature = "native"))]
        {
            let lent = registry::lend_late_init(index)?;
            abi::call(handle, "late_init", vec![handle_to_value(lent)])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(Act::LateInit, "late_init", |view| {
                let hook = natives::lend_late_init(view, index);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                this.late_init(hook)
            })
        }
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
        #[cfg(not(feature = "native"))]
        {
            let lent = registry::lend_finalizer(index)?;
            abi::call(handle, "finalizer", vec![handle_to_value(lent)])?;
            Ok(())
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(Act::Finalizer, "finalizer", |view| {
                let hook = natives::lend_finalizer(view, index);
                let this = entry!(view.table, handle, Builder, FeatureBuilder);
                this.finalizer(hook)
            })
        }
    }
}
