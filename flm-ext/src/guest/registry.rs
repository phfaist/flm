//! The guest's registry: what each feature's `register` recorded on this
//! instance, and the record of the `register` running now; with the
//! requests that lend a recorded object to the host. The twin of the
//! JavaScript prelude's `flm:_runtime` (`REGISTERING`, `record_*`, `lend_*`)
//! and of the record `REGISTRY` of its `flm:_entry`.
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
//! **Re-entry.** A request suspends the guest inside its call of `flm.call`,
//! and the host may run a nested handler call on the same instance meanwhile.
//! So no borrow of the registry or of a record is held across a request:
//! each function here records, releases its borrow, and only then sends.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::adapter::handles::HandleKind;
use crate::adapter::values::{handle_to_value, JsonForm};
use crate::api::core::stage::{Needs, RewriteDecl, StageDecl};
use crate::guest::abi;
use crate::guest::core::lowering::Lower;
use crate::guest::core::registry::{Contributed, Finalizer, LateInit};
use crate::guest::core::stage::{RewriteStage, Stage};
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

/// Whether the running `register` is a rehearsal.
pub(crate) fn rehearsing() -> bool {
    running().is_some_and(|record| record.borrow().rehearsal)
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

/// Sends the lending request `request`; answers the handle the host lent,
/// which must be of the kind `kind`.
fn lend(request: &Value, kind: HandleKind) -> Result<Value, ExtError> {
    let reply = abi::native(request)?;
    Ok(handle_to_value(abi::handle_of(&reply, kind)?))
}

/// Lends a stage of the extension's own, its declaration and needs given:
/// answers the handle of kind `stage`.
pub(crate) fn lend_stage(decl: &StageDecl, needs: &Needs) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_stage", "decl": decl.to_value(), "needs": needs.to_value() }), HandleKind::Stage)
}

/// Lends a rewrite stage of the extension's own, its declaration and needs
/// given: answers the handle of kind `rewrite_stage`.
pub(crate) fn lend_rewrite(decl: &RewriteDecl, needs: &Needs) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_rewrite", "decl": decl.to_value(), "needs": needs.to_value() }), HandleKind::RewriteStage)
}

/// Records the expansion hook `hook` of the element `element` and lends it:
/// answers the handle of kind `expand`.
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]), or the host's refusal.
pub(crate) fn lend_expand(element: &str, hook: &Arc<dyn Expand>) -> Result<Value, ExtError> {
    let done = registering(|| format!("the expansion hook of the element `{element}` must be attached"))?;
    done.borrow_mut().hooks.insert(element.to_string(), Arc::clone(hook));
    drop(done);
    let policy = hook.policy().to_value();
    lend(&json!({ "native": "lend_expand", "element": element, "policy": policy }), HandleKind::Expand)
}

/// Lends the body of the lowering rule `rule`, recorded by [`record_lower`]:
/// answers the handle of kind `lower`.
pub(crate) fn lend_lower(rule: &str) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_lower", "rule": rule }), HandleKind::Lower)
}

/// Lends the citation provider named `name`: answers the handle of kind
/// `citation_provider`.
fn lend_provider(name: &str) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_provider", "name": name }), HandleKind::CitationProvider)
}

/// Lends the template module named `name`: answers the handle of kind
/// `template_module`.
fn lend_template(name: &str) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_template", "name": name }), HandleKind::TemplateModule)
}

/// Records the handler object of `aspect` under the element `element` and
/// lends it; answers the aspect's crossing form (`{"Expand": handle}`, …;
/// a data aspect as its data). A `Referenceable` is lent with its method
/// `points`, a `Sectioning` with its three methods `level`, `in_toc` and
/// `toc_title`, so that the host calls each (an object that keeps the
/// defaults answers them from the guest).
///
/// # Errors
///
/// No `register` runs ([`ExtError::Handler`]), or the host's refusal.
pub(crate) fn lend_aspect(element: &str, aspect: &Aspect) -> Result<Value, ExtError> {
    let (kind, methods, handle_kind) = match aspect {
        Aspect::BlockRoleAspect(role) => return Ok(json!({ "BlockRoleAspect": role.0.to_value() })),
        Aspect::Expand(hook) => return Ok(json!({ "Expand": lend_expand(element, hook)? })),
        Aspect::Referenceable(_) => ("Referenceable", json!(["points"]), HandleKind::Referenceable),
        Aspect::Sectioning(_) => ("Sectioning", json!(["level", "in_toc", "toc_title"]), HandleKind::Sectioning),
    };
    record_aspect(element, aspect)?;
    let lent = lend(&json!({ "native": "lend_aspect", "kind": kind, "element": element, "methods": methods }), handle_kind)?;
    Ok(json!({ kind: lent }))
}

/// What the builder's `contribute_to`, `contribute_first_to` and
/// `peer_slot` send for `value`: data as it is; a citation provider or a
/// template module recorded by its name (one with an empty name is not
/// recorded, and the host refuses it) and, unless `rehearsal`, lent: its
/// handle (`null` in a rehearsal, which sends nothing).
///
/// # Errors
///
/// What recording or lending answers.
pub(crate) fn contributed(value: &Contributed, rehearsal: bool) -> Result<Value, ExtError> {
    match value {
        Contributed::Data(data) => Ok(data.clone()),
        Contributed::CitationProvider(provider) => {
            let name = provider.name();
            if !name.is_empty() {
                record_provider(&name, Arc::clone(provider))?;
            }
            if rehearsal { Ok(Value::Null) } else { lend_provider(&name) }
        }
        Contributed::TemplateModule(template) => {
            let name = template.name();
            if !name.is_empty() {
                record_template(&name, Arc::clone(template))?;
            }
            if rehearsal { Ok(Value::Null) } else { lend_template(&name) }
        }
    }
}

/// Lends the late-initialization hook at the position `index`: answers the
/// handle of kind `late_init`.
pub(crate) fn lend_late_init(index: usize) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_late_init", "index": index }), HandleKind::LateInit)
}

/// Lends the finalizer at the position `index`: answers the handle of kind
/// `finalizer`.
pub(crate) fn lend_finalizer(index: usize) -> Result<Value, ExtError> {
    lend(&json!({ "native": "lend_finalizer", "index": index }), HandleKind::Finalizer)
}

#[cfg(test)]
mod tests {
    //! The cross tests of the values holding handler objects, as the
    //! record-and-lend methods build them (refresh section 6.3): the test
    //! hook answers each lending as the host does, lending a host object of
    //! the kind into a call's table, and the host's reader reads what the
    //! guest wrote. Also the accepting half of a citation answer's entry
    //! (ruling S9).

    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::{json, Value};

    use super::*;
    use crate::adapter::handles::{CallTable, Entry};
    use crate::adapter::values::{self as host, Args, FromArg};
    use crate::api;
    use crate::guest::abi::{hook, ToArg};
    use crate::guest::core::data::store::StageOut;
    use crate::guest::core::registry::{BackendDef, BackendStage, FeatureBuilder, LibraryStage};
    use crate::guest::model::element::ElementSpec;
    use crate::guest::core::run::StageCx;
    use crate::guest::lang::mint::{ArgDef, ElementDef, FallbackDef, ScopedPackageDef, SyntaxDef};
    use crate::guest::lang::rewrites::userdefs::{ExpandCx, Expander, Expansion};
    use crate::guest::model::aspects::RefPoint;
    use crate::guest::stdcore::features::citations::{CitationAnswer, CitationRequest, CitationResult, ProviderCx};
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
    fn a_backend_crosses_with_its_lent_stages_and_refuses_a_changed_one() {
        let table: Table = Rc::new(RefCell::new(CallTable::new(1)));
        let calls = Rc::default();
        host_lending(&table, &calls);
        let library = api::stdcore::backends::lowering::LoweringStage::new("html");
        let library = LibraryStage::from_value(&handle_to_value(table.borrow_mut().lend(Entry::LibraryStage(library)))).expect("a library stage");
        let own: Arc<dyn Stage> = Arc::new(GuestObject);
        let def = registering(|| BackendDef::new("wc", vec!["wc.box".to_string()], BackendStage::Library(library), BackendStage::Own(Arc::clone(&own)), BackendStage::Own(own)))
            .expect("the backend's own stages are lent")
            .docs("A backend.");
        let read = host::backend_def_from_table(&mut table.borrow_mut(), &def.arg().expect("unchanged")).expect("the host reads the backend");
        assert!(matches!(read.lowering, api::core::registry::BackendStage::Library(_)));
        assert!(matches!(read.emit, api::core::registry::BackendStage::Own(_)) && matches!(read.final_stage, api::core::registry::BackendStage::Own(_)));
        assert_eq!(read.docs, "A backend.");
        let mut changed = def.clone();
        changed.emit = BackendStage::Own(Arc::new(GuestObject));
        let error = changed.arg().expect_err("a changed stage");
        assert_eq!(
            error,
            ExtError::Argument { method: String::new(), detail: "the stage `emit` of the backend `wc` must be the stage given to `BackendDef::new`".to_string() }
        );
        let mut swapped = def;
        swapped.lowering = swapped.emit.clone();
        assert!(swapped.arg().is_err(), "an own stage in place of the library's");
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
