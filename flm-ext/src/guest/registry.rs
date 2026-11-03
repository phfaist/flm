//! The guest's registry: what each feature's `register` recorded on this
//! instance, and the record of the `register` running now; with the lending
//! of a recorded object to the host. The twin of the JavaScript prelude's
//! `flm:_runtime` (`REGISTERING`, `record_*`, `lend_*`) and of the record
//! `REGISTRY` of its `flm:_entry`.
//!
//! A record holds the handler objects a `register` handed to FLM, found
//! again by the entries ([`crate::guest`]'s rehearsal): the stages, the
//! expansion hooks (by element), the lowering rules' bodies, the rewrite
//! stages, the citation providers and the template modules (by name), the
//! handler aspects (by element and kind), and the late-initialization hooks
//! and the finalizers (by position: the n-th call of
//! `FeatureBuilder::late_init` is at n). A finalizer records into the record
//! of the `register` that registered it: the two share the one record.
//!
//! **Recording, then lending.** A record-and-lend method first records its
//! object here, reading from it what its lending takes (a stage's
//! declaration and needs, a hook's policy, a provider's name: the
//! extension's code, which runs before any lending): an aspect's
//! [`AspectLending`], a contribution's [`ContributionLending`]. The lending
//! itself takes the route of the build. In a WebAssembly module it is a
//! request, the lending native of the host (`lend_*` here), which answers a
//! handle. Compiled into a program (the cargo feature `native`), the method
//! builds the host's handler object with the native's body on the frame of
//! the running call ([`abi::in_frame`]), and hands it to the API method that
//! takes it in the same closure, or lends it into the call's table where an
//! element's aspect keeps its handle.
//!
//! **Re-entry.** A request suspends the guest inside its call of `flm.call`,
//! and the host may run a nested handler call on the same instance meanwhile;
//! compiled into a program, the API method a lending is handed to may run
//! one too. So no borrow of the registry or of a record is held across a
//! lending: each function here records, releases its borrow, and only then
//! lends.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::Value;
#[cfg(not(feature = "native"))]
use serde_json::json;

#[cfg(not(feature = "native"))]
use crate::adapter::handles::{Handle, HandleKind};
#[cfg(feature = "native")]
use crate::adapter::typed::FromApi;
#[cfg(not(feature = "native"))]
use crate::adapter::values::{handle_to_value, JsonForm};
#[cfg(feature = "native")]
use crate::api;
#[cfg(not(feature = "native"))]
use crate::api::core::stage::{Needs, RewriteDecl, StageDecl};
use crate::api::lang::rewrites::userdefs::ExpandPolicy;
use crate::api::model::aspects::FixedBlockRole;
#[cfg(feature = "native")]
use crate::binding::frames::FrameView;
#[cfg(feature = "native")]
use crate::binding::natives::{self, Act};
use crate::guest::abi;
use crate::guest::core::lowering::Lower;
use crate::guest::core::registry::{Contributed, Finalizer, LateInit};
use crate::guest::core::stage::{RewriteStage, Stage};
use crate::guest::hand::model_element::AspectForm;
use crate::guest::lang::rewrites::userdefs::Expand;
use crate::guest::model::aspects::{Referenceable, Sectioning};
use crate::guest::model::element::Aspect;
use crate::guest::stdcore::features::citations::CitationProvider;
use crate::guest::state;
use crate::guest::stdcore::features::compose::TemplateModule;
use crate::runtime::ExtError;

/// The handler aspects recorded for one element.
#[derive(Clone, Default)]
pub(crate) struct Aspects {
    /// Its `Referenceable`.
    pub(crate) referenceable: Option<Arc<dyn Referenceable>>,
    /// Its `Sectioning`.
    pub(crate) sectioning: Option<Arc<dyn Sectioning>>,
}

/// What one feature's `register` recorded, and its finalizers after it.
pub(crate) struct Record {
    /// The extension's stem.
    pub(crate) stem: String,
    /// The package of the feature.
    pub(crate) package: String,
    /// The feature whose `register` runs.
    pub(crate) feature: String,
    /// Whether the run is a rehearsal (a builder that sends nothing).
    pub(crate) rehearsal: bool,
    /// The stages, by name.
    pub(crate) stages: BTreeMap<String, Arc<dyn Stage>>,
    /// The expansion hooks, by element.
    pub(crate) hooks: BTreeMap<String, Arc<dyn Expand>>,
    /// The bodies of the lowering rules, by rule.
    pub(crate) lowers: BTreeMap<String, Arc<dyn Lower>>,
    /// The rewrite stages, by stage name.
    pub(crate) rewrites: BTreeMap<String, Arc<dyn RewriteStage>>,
    /// The citation providers, by name.
    pub(crate) providers: BTreeMap<String, Arc<dyn CitationProvider>>,
    /// The template modules, by name.
    pub(crate) templates: BTreeMap<String, Arc<dyn TemplateModule>>,
    /// The handler aspects, by element.
    pub(crate) aspects: BTreeMap<String, Aspects>,
    /// The late-initialization hooks, by position.
    pub(crate) late_inits: Vec<Arc<dyn LateInit>>,
    /// The finalizers, by position.
    pub(crate) finalizers: Vec<Arc<dyn Finalizer>>,
}

/// A record, shared by the registry and the running `register` (a finalizer
/// records into the record of its feature).
pub(crate) type Shared = Rc<RefCell<Record>>;

/// Begins the record of the `register` of the feature `feature` of the
/// package `package`; `rehearsal` when its builder sends nothing.
pub(crate) fn begin(stem: &str, package: &str, feature: &str, rehearsal: bool) {
    let record = Record {
        stem: stem.to_string(),
        package: package.to_string(),
        feature: feature.to_string(),
        rehearsal,
        stages: BTreeMap::new(),
        hooks: BTreeMap::new(),
        lowers: BTreeMap::new(),
        rewrites: BTreeMap::new(),
        providers: BTreeMap::new(),
        templates: BTreeMap::new(),
        aspects: BTreeMap::new(),
        late_inits: Vec::new(),
        finalizers: Vec::new(),
    };
    *state::current().running.borrow_mut() = Some(Rc::new(RefCell::new(record)));
}

/// Records into `done`, what an earlier `register` of the feature recorded
/// (a finalizer declares on behalf of its feature), `rehearsal` as given.
pub(crate) fn continue_with(done: &Shared, rehearsal: bool) {
    done.borrow_mut().rehearsal = rehearsal;
    *state::current().running.borrow_mut() = Some(Rc::clone(done));
}

/// Ends the running record; answers it.
pub(crate) fn end() -> Option<Shared> {
    state::current().running.borrow_mut().take()
}

/// The record of the `register` running now, if any: a template module's
/// `describe` runs inside the `register` that contributes the module.
pub(crate) fn running() -> Option<Shared> {
    state::current().running.borrow().clone()
}

/// Keeps `done` as what the feature `feature` of the package `package`
/// recorded on this instance.
pub(crate) fn keep(package: &str, feature: &str, done: Shared) {
    state::current().records.borrow_mut().insert((package.to_string(), feature.to_string()), done);
}

/// What the last `register` of the feature `feature` of the package
/// `package` recorded on this instance.
pub(crate) fn recorded(package: &str, feature: &str) -> Option<Shared> {
    state::current().records.borrow().get(&(package.to_string(), feature.to_string())).cloned()
}

/// The running record; `what` is the text of the refusal when no `register`
/// runs ("the stage `x` must be declared").
fn registering(what: impl FnOnce() -> String) -> Result<Shared, ExtError> {
    running().ok_or_else(|| ExtError::Handler { detail: format!("{} while a feature's `register` runs", what()), data: Value::Null })
}

/// The feature whose `register` runs.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn registering_feature() -> Result<String, ExtError> {
    let done = registering(|| "the builder's `feature()` must be called".to_string())?;
    let feature = done.borrow().feature.clone();
    Ok(feature)
}

/// Records the stage `stage` under `name`.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_stage(name: &str, stage: Arc<dyn Stage>) -> Result<(), ExtError> {
    let done = registering(|| format!("the stage `{name}` must be declared"))?;
    done.borrow_mut().stages.insert(name.to_string(), stage);
    Ok(())
}

/// Records the rewrite stage `stage` under its stage name `name`.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_rewrite(name: &str, stage: Arc<dyn RewriteStage>) -> Result<(), ExtError> {
    let done = registering(|| format!("the rewrite stage `{name}` must be declared"))?;
    done.borrow_mut().rewrites.insert(name.to_string(), stage);
    Ok(())
}

/// Records the body `body` of the lowering rule `rule`.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_lower(rule: &str, body: Arc<dyn Lower>) -> Result<(), ExtError> {
    let done = registering(|| format!("the lowering rule `{rule}` must be built"))?;
    done.borrow_mut().lowers.insert(rule.to_string(), body);
    Ok(())
}

/// The refusal of a second handler object under one name in one `register`;
/// `what` names its kind ("the citation provider").
fn contributed_twice(done: &Record, what: &str, name: &str) -> ExtError {
    ExtError::Handler {
        detail: format!(
            "{what} `{name}` must be contributed once by `register` of the feature `{}` of the package `{}` of the extension `{}`",
            done.feature, done.package, done.stem
        ),
        data: Value::Null,
    }
}

/// Records the citation provider `provider` under its name `name`.
///
/// # Errors
///
/// No `register` runs, or one has recorded a provider under that name
/// already ([`ExtError::Handler`]).
pub(crate) fn record_provider(name: &str, provider: Arc<dyn CitationProvider>) -> Result<(), ExtError> {
    let done = registering(|| format!("the citation provider `{name}` must be contributed"))?;
    let mut done = done.borrow_mut();
    if done.providers.contains_key(name) {
        return Err(contributed_twice(&done, "the citation provider", name));
    }
    done.providers.insert(name.to_string(), provider);
    Ok(())
}

/// Records the template module `template` under its name `name`.
///
/// # Errors
///
/// No `register` runs, or one has recorded a module under that name already
/// ([`ExtError::Handler`]).
pub(crate) fn record_template(name: &str, template: Arc<dyn TemplateModule>) -> Result<(), ExtError> {
    let done = registering(|| format!("the template module `{name}` must be contributed"))?;
    let mut done = done.borrow_mut();
    if done.templates.contains_key(name) {
        return Err(contributed_twice(&done, "the template module", name));
    }
    done.templates.insert(name.to_string(), template);
    Ok(())
}

/// Records the handler object of `aspect` (an `Expand`, a `Referenceable`
/// or a `Sectioning`) under the element `element`; a data aspect records
/// nothing.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_aspect(element: &str, aspect: &Aspect) -> Result<(), ExtError> {
    if matches!(aspect, Aspect::BlockRoleAspect(_)) {
        return Ok(());
    }
    let done = registering(|| format!("the aspect `{}` of the element `{element}` must be attached", aspect.kind()))?;
    let mut done = done.borrow_mut();
    match aspect {
        Aspect::BlockRoleAspect(_) => {}
        Aspect::Expand(hook) => {
            done.hooks.insert(element.to_string(), Arc::clone(hook));
        }
        Aspect::Referenceable(object) => done.aspects.entry(element.to_string()).or_default().referenceable = Some(Arc::clone(object)),
        Aspect::Sectioning(object) => done.aspects.entry(element.to_string()).or_default().sectioning = Some(Arc::clone(object)),
    }
    Ok(())
}

/// Records the late-initialization hook `hook`; answers its position.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_late_init(hook: Arc<dyn LateInit>) -> Result<usize, ExtError> {
    let done = registering(|| "a late-initialization hook must be registered".to_string())?;
    let mut done = done.borrow_mut();
    done.late_inits.push(hook);
    Ok(done.late_inits.len() - 1)
}

/// Records the finalizer `hook`; answers its position.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_finalizer(hook: Arc<dyn Finalizer>) -> Result<usize, ExtError> {
    let done = registering(|| "a finalizer must be registered".to_string())?;
    let mut done = done.borrow_mut();
    done.finalizers.push(hook);
    Ok(done.finalizers.len() - 1)
}

/// Records the expansion hook `hook` of the element `element`; answers its
/// policy, which its lending takes, read here: it is the extension's code.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]).
pub(crate) fn record_expand(element: &str, hook: &Arc<dyn Expand>) -> Result<ExpandPolicy, ExtError> {
    let done = registering(|| format!("the expansion hook of the element `{element}` must be attached"))?;
    done.borrow_mut().hooks.insert(element.to_string(), Arc::clone(hook));
    drop(done);
    Ok(hook.policy())
}

/// The methods a `Sectioning` is lent with, so that the host calls each (an
/// object that keeps a default answers it from the guest).
const SECTIONING_METHODS: [&str; 3] = ["level", "in_toc", "toc_title"];

/// What lending an aspect takes once it is recorded: its data, or the
/// arguments of the lending of its handler object, read from the
/// extension's object when it is recorded ([`record_lent_aspect`]).
#[derive(Clone, Copy)]
pub(crate) enum AspectLending {
    /// A data aspect, the element's block role: nothing is lent.
    Data(FixedBlockRole),
    /// An expansion hook, of this policy.
    Expand(ExpandPolicy),
    /// A `Referenceable`, lent with its method `points`.
    Referenceable,
    /// A `Sectioning`, lent with its three methods `level`, `in_toc` and
    /// `toc_title`.
    Sectioning,
}

/// Records the handler object of `aspect` under the element `element`, as
/// its lending records it (an expansion hook as [`record_expand`] does, a
/// `Referenceable` or a `Sectioning` as [`record_aspect`] does; a data
/// aspect records nothing); answers what its lending takes.
///
/// # Errors
///
/// No `register` runs, for an aspect that holds a handler object
/// ([`ExtError::Handler`]).
pub(crate) fn record_lent_aspect(element: &str, aspect: &Aspect) -> Result<AspectLending, ExtError> {
    Ok(match aspect {
        Aspect::BlockRoleAspect(role) => AspectLending::Data(*role),
        Aspect::Expand(hook) => AspectLending::Expand(record_expand(element, hook)?),
        Aspect::Referenceable(_) => {
            record_aspect(element, aspect)?;
            AspectLending::Referenceable
        }
        Aspect::Sectioning(_) => {
            record_aspect(element, aspect)?;
            AspectLending::Sectioning
        }
    })
}

impl AspectLending {
    /// The aspect's crossing form, as an element carries it
    /// ([`ElementSpec::with`](crate::guest::model::element::ElementSpec::with)):
    /// its data, or its handler object lent into the running call and the
    /// handle it was lent under. In a WebAssembly module the lending is a
    /// request (`lend_expand`, `lend_aspect`); compiled into a program, the
    /// natives' body builds the object on the running call's frame
    /// ([`abi::in_frame`]), which lends it into the call's table.
    ///
    /// # Errors
    ///
    /// The host's refusal of the lending.
    pub(crate) fn form(self, element: &str) -> Result<AspectForm, ExtError> {
        #[cfg(not(feature = "native"))]
        {
            Ok(match self {
                AspectLending::Data(role) => AspectForm::BlockRole(role),
                AspectLending::Expand(policy) => AspectForm::Expand(lend_expand(element, policy)?),
                AspectLending::Referenceable => {
                    let methods = ["points"];
                    let request = json!({ "native": "lend_aspect", "kind": "Referenceable", "element": element, "methods": methods });
                    AspectForm::Referenceable(lend(&request, HandleKind::Referenceable)?)
                }
                AspectLending::Sectioning => {
                    let methods = SECTIONING_METHODS;
                    let request = json!({ "native": "lend_aspect", "kind": "Sectioning", "element": element, "methods": methods });
                    AspectForm::Sectioning(lend(&request, HandleKind::Sectioning)?)
                }
            })
        }
        #[cfg(feature = "native")]
        {
            const METHOD: &str = "ElementSpec::with";
            if let AspectLending::Data(role) = self {
                return Ok(AspectForm::BlockRole(role));
            }
            abi::in_frame(self.act(element, METHOD), METHOD, |view| FromApi::from_api(self.aspect(view, element), view.table))
        }
    }

    /// What the refusal of the lending names when no handler call of the
    /// extension runs: the lending of the handler object, or, for a data
    /// aspect, the call of `method` that sends the aspect.
    #[cfg(feature = "native")]
    pub(crate) fn act<'a>(&self, element: &'a str, method: &'a str) -> Act<'a> {
        match self {
            AspectLending::Data(_) => Act::Call(method),
            AspectLending::Expand(_) => Act::Expand(element),
            AspectLending::Referenceable => Act::Aspect { kind: "Referenceable", element },
            AspectLending::Sectioning => Act::Aspect { kind: "Sectioning", element },
        }
    }

    /// The api's aspect for the element `element`: its data, or its handler
    /// object, built on the frame `view` by the natives' body
    /// (`lend_expand`, `lend_aspect`).
    #[cfg(feature = "native")]
    pub(crate) fn aspect(self, view: &FrameView<'_, '_>, element: &str) -> api::model::element::Aspect {
        use api::model::element::Aspect as ApiAspect;
        let element = element.to_string();
        match self {
            AspectLending::Data(role) => ApiAspect::BlockRoleAspect(role),
            AspectLending::Expand(policy) => ApiAspect::Expand(natives::lend_expand(view, element, policy)),
            AspectLending::Referenceable => ApiAspect::Referenceable(natives::lend_referenceable(view, element)),
            AspectLending::Sectioning => {
                let methods = SECTIONING_METHODS.iter().map(|method| method.to_string()).collect();
                ApiAspect::Sectioning(natives::lend_sectioning(view, element, methods))
            }
        }
    }
}

/// What lending a contribution takes once it is recorded: its data, or the
/// name of its handler object, read from the extension's object when it is
/// recorded ([`record_contributed`]).
pub(crate) enum ContributionLending<'a> {
    /// Data, contributed as it is.
    Data(&'a Value),
    /// A citation provider, by its name.
    CitationProvider(String),
    /// A template module, by its name.
    TemplateModule(String),
}

/// Records the handler object of the contribution `value` (a citation
/// provider or a template module) under its name; one with an empty name is
/// not recorded, and the host refuses it. Answers what its lending takes.
///
/// # Errors
///
/// No `register` runs, or one has recorded a handler object of the kind
/// under that name already ([`ExtError::Handler`]).
pub(crate) fn record_contributed(value: &Contributed) -> Result<ContributionLending<'_>, ExtError> {
    Ok(match value {
        Contributed::Data(data) => ContributionLending::Data(data),
        Contributed::CitationProvider(provider) => {
            let name = provider.name();
            if !name.is_empty() {
                record_provider(&name, Arc::clone(provider))?;
            }
            ContributionLending::CitationProvider(name)
        }
        Contributed::TemplateModule(template) => {
            let name = template.name();
            if !name.is_empty() {
                record_template(&name, Arc::clone(template))?;
            }
            ContributionLending::TemplateModule(name)
        }
    })
}

impl ContributionLending<'_> {
    /// What the builder's `contribute_to`, `contribute_first_to` and
    /// `peer_slot` send in a WebAssembly module: data as it is, a handler
    /// object lent (`lend_provider`, `lend_template`) as its handle.
    ///
    /// # Errors
    ///
    /// The host's refusal of the lending.
    #[cfg(not(feature = "native"))]
    pub(crate) fn sent(&self) -> Result<Value, ExtError> {
        match self {
            ContributionLending::Data(data) => Ok((*data).clone()),
            ContributionLending::CitationProvider(name) => {
                lend(&json!({ "native": "lend_provider", "name": name }), HandleKind::CitationProvider).map(handle_to_value)
            }
            ContributionLending::TemplateModule(name) => {
                lend(&json!({ "native": "lend_template", "name": name }), HandleKind::TemplateModule).map(handle_to_value)
            }
        }
    }

    /// What the refusal of the contribution names when no handler call of
    /// the extension runs: the lending of the handler object, or, for data,
    /// the call of `method`.
    #[cfg(feature = "native")]
    pub(crate) fn act<'a>(&'a self, method: &'a str) -> Act<'a> {
        match self {
            ContributionLending::Data(_) => Act::Call(method),
            ContributionLending::CitationProvider(name) => Act::Provider(name),
            ContributionLending::TemplateModule(name) => Act::Template(name),
        }
    }

    /// The api's contribution: its data, or its handler object, built on the
    /// frame `view` by the natives' body (`lend_provider`, `lend_template`).
    #[cfg(feature = "native")]
    pub(crate) fn contributed(&self, view: &FrameView<'_, '_>) -> api::core::registry::Contributed {
        use api::core::registry::Contributed as ApiContributed;
        match self {
            ContributionLending::Data(data) => ApiContributed::Data((*data).clone()),
            ContributionLending::CitationProvider(name) => {
                ApiContributed::CitationProvider(natives::lend_provider(view, name.clone()))
            }
            ContributionLending::TemplateModule(name) => ApiContributed::TemplateModule(natives::lend_template(view, name.clone())),
        }
    }
}

/// Sends the lending request `request`; answers the handle the host lent,
/// which must be of the kind `kind`.
#[cfg(not(feature = "native"))]
fn lend(request: &Value, kind: HandleKind) -> Result<Handle, ExtError> {
    abi::handle_of(&abi::native(request)?, kind)
}

/// Lends a stage of the extension's own, its declaration and needs given:
/// answers the handle of kind `stage`.
#[cfg(not(feature = "native"))]
pub(crate) fn lend_stage(decl: &StageDecl, needs: &Needs) -> Result<Handle, ExtError> {
    lend(&json!({ "native": "lend_stage", "decl": decl.to_value(), "needs": needs.to_value() }), HandleKind::Stage)
}

/// Lends a rewrite stage of the extension's own, its declaration and needs
/// given: answers the handle of kind `rewrite_stage`.
#[cfg(not(feature = "native"))]
pub(crate) fn lend_rewrite(decl: &RewriteDecl, needs: &Needs) -> Result<Handle, ExtError> {
    lend(&json!({ "native": "lend_rewrite", "decl": decl.to_value(), "needs": needs.to_value() }), HandleKind::RewriteStage)
}

/// Lends the expansion hook of the element `element`, of the policy
/// `policy`, recorded by [`record_expand`]: answers the handle of kind
/// `expand`.
#[cfg(not(feature = "native"))]
pub(crate) fn lend_expand(element: &str, policy: ExpandPolicy) -> Result<Handle, ExtError> {
    lend(&json!({ "native": "lend_expand", "element": element, "policy": policy.to_value() }), HandleKind::Expand)
}

/// Lends the body of the lowering rule `rule`, recorded by [`record_lower`]:
/// answers the handle of kind `lower`.
#[cfg(not(feature = "native"))]
pub(crate) fn lend_lower(rule: &str) -> Result<Handle, ExtError> {
    lend(&json!({ "native": "lend_lower", "rule": rule }), HandleKind::Lower)
}

/// Lends the late-initialization hook at the position `index`: answers the
/// handle of kind `late_init`.
#[cfg(not(feature = "native"))]
pub(crate) fn lend_late_init(index: usize) -> Result<Handle, ExtError> {
    lend(&json!({ "native": "lend_late_init", "index": index }), HandleKind::LateInit)
}

/// Lends the finalizer at the position `index`: answers the handle of kind
/// `finalizer`.
#[cfg(not(feature = "native"))]
pub(crate) fn lend_finalizer(index: usize) -> Result<Handle, ExtError> {
    lend(&json!({ "native": "lend_finalizer", "index": index }), HandleKind::Finalizer)
}

#[cfg(all(test, not(feature = "native")))]
mod tests {
    //! The cross tests of the values holding handler objects, as the
    //! record-and-lend methods build them: the test
    //! hook answers each lending as the host does, lending a host object of
    //! the kind into a call's table, and the host's reader reads what the
    //! guest wrote. The hook answers the requests sent as JSON, so these run
    //! without the cargo feature `native`, as the tests of `entry.rs` that
    //! send requests.

    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::{json, Value};

    use super::*;
    use crate::adapter::handles::{CallTable, Entry};
    use crate::adapter::values::{self as host, Args, FromArg};
    use crate::api;
    use crate::guest::abi::hook;
    use crate::guest::core::data::store::StageOut;
    use crate::guest::core::registry::{BackendDef, BackendStage, FeatureBuilder, LibraryStage};
    use crate::guest::model::element::ElementSpec;
    use crate::guest::core::run::StageCx;
    use crate::guest::lang::mint::{ArgDef, ElementDef, FallbackDef, ScopedPackageDef, SyntaxDef};
    use crate::guest::lang::rewrites::userdefs::{ExpandCx, Expander, Expansion};
    use crate::guest::model::aspects::RefPoint;
    use crate::guest::stdcore::features::citations::{CitationRequest, CitationResult, ProviderCx};
    use crate::guest::stdcore::features::compose::{TemplateContext, TemplateCx};
    use crate::guest::techy::node::Node;
    use crate::api::stdcore::features::compose::TemplateDescription;

    /// The host's stand-ins for the objects the binding lends in.
    struct HostObject;

    impl api::lang::rewrites::userdefs::Expand for HostObject {
        fn expand<'c>(
            &self,
            _: &'c dyn api::techy::node::Node<'c>,
            _: &'c mut dyn api::lang::rewrites::userdefs::ExpandCx,
            _: &'c mut dyn api::lang::rewrites::userdefs::Expander,
        ) -> Result<api::lang::rewrites::userdefs::Expansion, ExtError> {
            Ok(api::lang::rewrites::userdefs::Expansion::Default)
        }
    }

    impl api::model::aspects::Referenceable for HostObject {
        fn points<'c>(&self, _: &'c dyn api::techy::node::Node<'c>) -> Result<Vec<api::model::aspects::RefPoint<'c>>, ExtError> {
            Ok(Vec::new())
        }
    }

    impl api::model::aspects::Sectioning for HostObject {
        fn level<'c>(&self, _: &'c dyn api::techy::node::Node<'c>) -> Result<u32, ExtError> {
            Ok(1)
        }
    }

    impl api::core::stage::Stage for HostObject {
        fn decl(&self) -> StageDecl {
            StageDecl::new("wc.own", api::core::stage::Effect::Compute, api::core::stage::Scope::Document)
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn run<'c>(&self, _: &'c dyn api::core::run::StageCx, _: &'c mut dyn api::core::data::store::StageOut) -> Result<(), ExtError> {
            Ok(())
        }
    }

    impl api::stdcore::features::citations::CitationProvider for HostObject {
        fn name(&self) -> String {
            "wc.cites".to_string()
        }
        fn answer(
            &self,
            _: &api::stdcore::features::citations::CitationRequest,
            _: &dyn api::stdcore::features::citations::ProviderCx,
        ) -> Result<api::stdcore::features::citations::CitationResult, ExtError> {
            Ok(Ok(None))
        }
    }

    impl api::stdcore::features::compose::TemplateModule for HostObject {
        fn name(&self) -> String {
            "wc.page".to_string()
        }
        fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
            TemplateDescription::from_value(&json!({ "output": "html", "accepts": "all", "requires": [], "config": {} }))
        }
        fn render(&self, _: &api::stdcore::features::compose::TemplateContext, _: &dyn api::stdcore::features::compose::TemplateCx) -> Result<String, ExtError> {
            Ok(String::new())
        }
    }

    /// The guest's objects of each kind.
    struct GuestObject;

    impl Expand for GuestObject {
        fn expand(&self, _: &Node, _: &mut ExpandCx, _: &mut Expander) -> Result<Expansion, ExtError> {
            Ok(Expansion::Default)
        }
    }

    impl Referenceable for GuestObject {
        fn points(&self, _: &Node) -> Result<Vec<RefPoint>, ExtError> {
            Ok(Vec::new())
        }
    }

    impl Sectioning for GuestObject {
        fn level(&self, _: &Node) -> Result<u32, ExtError> {
            Ok(1)
        }
    }

    impl Stage for GuestObject {
        fn decl(&self) -> StageDecl {
            StageDecl::new("wc.own", api::core::stage::Effect::Compute, api::core::stage::Scope::Document)
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn run(&self, _: &StageCx, _: &mut StageOut) -> Result<(), ExtError> {
            Ok(())
        }
    }

    impl CitationProvider for GuestObject {
        fn name(&self) -> String {
            "wc.cites".to_string()
        }
        fn answer(&self, _: &CitationRequest, _: &ProviderCx) -> Result<CitationResult, ExtError> {
            Ok(Ok(None))
        }
    }

    impl TemplateModule for GuestObject {
        fn name(&self) -> String {
            "wc.page".to_string()
        }
        fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
            TemplateDescription::from_value(&json!({ "output": "html", "accepts": "all", "requires": [], "config": {} }))
        }
        fn render(&self, _: &TemplateContext, _: &TemplateCx) -> Result<String, ExtError> {
            Ok(String::new())
        }
    }

    /// The call's table the hook lends into, shared with the test.
    type Table = Rc<RefCell<CallTable<'static>>>;

    /// Installs a hook that lends, as the host does, a host object of each
    /// lending's kind into `table`, and logs every call's arguments.
    fn host_lending(table: &Table, calls: &Rc<RefCell<Vec<Value>>>) {
        let table = Rc::clone(table);
        let calls = Rc::clone(calls);
        hook::install(move |request| {
            let object = Arc::new(HostObject);
            let entry = match request["native"].as_str() {
                Some("lend_expand") => Entry::Expand(object),
                Some("lend_aspect") if request["kind"] == "Sectioning" => Entry::Sectioning(object),
                Some("lend_aspect") => Entry::Referenceable(object),
                Some("lend_stage") => Entry::Stage(object),
                Some("lend_provider") => Entry::CitationProvider(object),
                Some("lend_template") => Entry::TemplateModule(object),
                _ => {
                    calls.borrow_mut().push(request["args"].clone());
                    return json!({ "ok": null });
                }
            };
            json!({ "ok": handle_to_value(table.borrow_mut().lend(entry)) })
        });
    }

    /// Runs `body` while a `register` (not a rehearsal) runs.
    fn registering<T>(body: impl FnOnce() -> T) -> T {
        begin("wc", "_", "wc", false);
        let answer = body();
        end();
        answer
    }

    /// The kinds of the aspects of a host's element.
    fn kinds(spec: &api::model::element::ElementSpec) -> Vec<&'static str> {
        spec.aspects().iter().map(api::model::element::Aspect::kind).collect()
    }

    #[test]
    fn element_specs_declarations_fallbacks_and_scoped_packages_cross_with_their_lent_aspects() {
        let table: Table = Rc::new(RefCell::new(CallTable::new(1)));
        let calls = Rc::default();
        host_lending(&table, &calls);
        let spec = registering(|| {
            ElementSpec::new("wc.badge")
                .docs("A badge.")
                .with(Aspect::BlockRoleAspect(api::model::aspects::FixedBlockRole(api::model::aspects::BlockRole::Block)))?
                .with(Aspect::Expand(Arc::new(GuestObject)))?
                .with(Aspect::Referenceable(Arc::new(GuestObject)))?
                .with(Aspect::Sectioning(Arc::new(GuestObject)))
        })
        .expect("the aspects are lent");
        let syntax = SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] };
        let def = ElementDef::new("wc", ["badge"], syntax, spec.clone()).docs("Declared.").expect("data");
        let read = host::element_def_from_table(&mut table.borrow_mut(), &def.to_value()).expect("the host reads the declaration");
        assert_eq!(kinds(read.element().expect("a data declaration")), ["BlockRoleAspect", "Expand", "Referenceable", "Sectioning"]);
        let fallback = FallbackDef::new([api::lang::mint::Mode::Text], api::lang::mint::FallbackKind::Macro, spec.clone());
        let read = host::fallback_def_from_table(&mut table.borrow_mut(), &fallback.to_value()).expect("the host reads the fallback");
        assert_eq!(kinds(&read.element), ["BlockRoleAspect", "Expand", "Referenceable", "Sectioning"]);
        let scoped = ScopedPackageDef::new("wc.scoped", vec![def.clone()], "Scoped.");
        let read = host::scoped_package_def_from_table(&mut table.borrow_mut(), &scoped.to_value()).expect("the host reads the package");
        assert_eq!(read.elements.len(), 1);
        // The guest reads its own forms back as the forms it wrote.
        assert_eq!(ElementSpec::from_value(&spec.to_value()).map(|spec| spec.to_value()).ok(), Some(spec.to_value()));
        assert_eq!(ElementDef::from_value(&def.to_value()).map(|def| def.to_value()).ok(), Some(def.to_value()));
        let error = registering(|| ElementSpec::new("wc.badge").with(Aspect::Expand(Arc::new(GuestObject))).and_then(|spec| spec.with(Aspect::Expand(Arc::new(GuestObject)))));
        assert!(matches!(&error, Err(ExtError::Argument { method, detail }) if method == "ElementSpec::with" && detail == "the aspect `Expand` is attached twice to the element `wc.badge`"), "{:?}", error.err());
        hook::remove();
    }

    #[test]
    fn a_helpers_declaration_reads_back_as_itself_and_forwards_docs_and_modes() {
        let calls: Rc<RefCell<Vec<Value>>> = Rc::default();
        let table: Table = Rc::new(RefCell::new(CallTable::new(1)));
        host_lending(&table, &calls);
        let lent = json!({ "@handle": { "kind": "element_def", "call": 1, "index": 4 } });
        let def = ElementDef::from_value(&lent).expect("a helper's declaration");
        assert_eq!(def.to_value(), lent);
        assert!(def.owner().is_none() && def.element().is_none());
        let error = def.clone().docs("forwarded").expect_err("the hook answers `null`, which is no declaration");
        assert!(matches!(error, ExtError::Value { .. }), "{error:?}");
        let _ = def.modes([api::lang::mint::Mode::Text]);
        assert_eq!(*calls.borrow(), [json!(["forwarded"]), json!([["text"]])], "both changes are sent to the host");
        hook::remove();
    }

    #[test]
    fn a_backend_crosses_with_its_lent_stages() {
        let table: Table = Rc::new(RefCell::new(CallTable::new(1)));
        let calls: Rc<RefCell<Vec<Value>>> = Rc::default();
        host_lending(&table, &calls);
        let library = api::stdcore::backends::lowering::LoweringStage::new("html");
        let library = LibraryStage::from_value(&handle_to_value(table.borrow_mut().lend(Entry::LibraryStage(library)))).expect("a library stage");
        let own: Arc<dyn Stage> = Arc::new(GuestObject);
        let def = BackendDef::new("wc", vec!["wc.box".to_string()], BackendStage::Library(library), BackendStage::Own(Arc::clone(&own)), BackendStage::Own(own))
            .docs("A backend.");
        assert_eq!(format!("{def:?}"), "BackendDef { name: \"wc\", native: [\"wc.box\"], .. }");
        let mut b = FeatureBuilder { handle: Some(crate::adapter::handles::Handle { kind: HandleKind::FeatureBuilder, call: 1, index: 0 }) };
        registering(|| b.backend(def)).expect("the backend's own stages are lent");
        let calls = calls.borrow();
        let read = host::backend_def_from_table(&mut table.borrow_mut(), &calls[0][0]).expect("the host reads the backend");
        assert!(matches!(read.lowering, api::core::registry::BackendStage::Library(_)));
        assert!(matches!(read.emit, api::core::registry::BackendStage::Own(_)) && matches!(read.final_stage, api::core::registry::BackendStage::Own(_)));
        assert_eq!(read.docs, "A backend.");
        hook::remove();
    }

    #[test]
    fn contributions_cross_as_data_or_as_their_lent_handler_objects() {
        let table: Table = Rc::new(RefCell::new(CallTable::new(1)));
        let calls: Rc<RefCell<Vec<Value>>> = Rc::default();
        host_lending(&table, &calls);
        let mut b = FeatureBuilder { handle: Some(crate::adapter::handles::Handle { kind: HandleKind::FeatureBuilder, call: 1, index: 0 }) };
        registering(|| -> Result<(), ExtError> {
            b.contribute_to("flm.citations.providers", Contributed::CitationProvider(Arc::new(GuestObject)))?;
            b.peer_slot("flm.compose.templates", Contributed::TemplateModule(Arc::new(GuestObject)))?;
            b.contribute_first_to("wc.data", Contributed::Data(json!({ "k": [1, 2] })))
        })
        .expect("the contributions are sent");
        let calls = calls.borrow();
        let read: Vec<api::core::registry::Contributed> = calls
            .iter()
            .map(|args| api::core::registry::Contributed::from_arg(&mut table.borrow_mut(), &Args::new(args).expect("a list"), 1).expect("the host reads it"))
            .collect();
        assert!(matches!(read[0], api::core::registry::Contributed::CitationProvider(_)));
        assert!(matches!(read[1], api::core::registry::Contributed::TemplateModule(_)));
        assert!(matches!(&read[2], api::core::registry::Contributed::Data(data) if *data == json!({ "k": [1, 2] })));
        hook::remove();
    }
}

#[cfg(test)]
mod citation_tests {
    //! The accepting half of a citation answer's entry: the host's reader
    //! accepts the entry the guest read back.

    use std::sync::Arc;

    use crate::adapter::handles::{CallTable, Entry};
    use crate::adapter::values::{self as host, handle_to_value, JsonForm};
    use crate::guest::stdcore::features::citations::{CitationAnswer, CitationResult};

    #[test]
    fn a_citation_answer_s_entry_is_accepted_by_the_host() {
        use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
        use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
        use flm_core::model::doc::FragmentLayout;
        use flm_core::stdcore::{self, StdcoreConfig};
        let mut builder = RegistryBuilder::new();
        builder.use_package(stdcore::package(StdcoreConfig::new()));
        let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
        let source = Arc::new(FlmSource::new("Ann, 2001").with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default()).expect("the text parses");
        let entry = flm_core::stdcore::features::citations::CitationEntry::new(Arc::clone(fragment.tree()));
        let mut table = CallTable::new(1);
        let lent = handle_to_value(table.lend(Entry::CitationEntry(Box::new(crate::adapter::citations::CitationEntryFacade(entry)))));
        let entry = crate::guest::stdcore::features::citations::CitationEntry::from_value(&lent).expect("the guest reads the entry");
        let results: Vec<CitationResult> = vec![Ok(Some(CitationAnswer::new(entry).with_identity("ann2001"))), Ok(None)];
        let read = host::citation_results_from_value(&mut table, &results.to_value()).expect("the host accepts the entry");
        assert!(matches!(&read[0], Ok(Some(answer)) if answer.identity.as_deref() == Some("ann2001")));
        assert!(matches!(read[1], Ok(None)));
    }
}
