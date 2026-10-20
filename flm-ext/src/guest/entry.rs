//! The guest's entries: the builders an extension declares its packages
//! with ([`Extension`], [`Package`], [`Feature`]) and the 18 entries the
//! engine `wasm` calls through the export `flm_entry`, routed by name; the
//! twin of the JavaScript prelude's `flm:_entry`.
//!
//! An entry's request is `{"entry": NAME, "args": [ … ]}`; its reply the
//! envelope `{"ok": value}` or `{"err": data, "message": text}`, `data`
//! being the error's [`ExtError::data`]. `define(stem, module)` runs the
//! function the entry macro names with the extension builder;
//! `packages()` answers the package objects it declared; `register` and
//! `rehearse` run a feature's `register` (the rehearsal with a builder that
//! sends nothing, then each finalizer that `register` recorded, likewise);
//! every other entry finds the handler object the feature's `register`
//! recorded ([`crate::guest::registry`]) and calls it, or refuses with the
//! text the prelude's entries use when it is absent.
//!
//! **Re-entry.** The host may enter the instance again while a request of
//! the guest runs (a hook run while a stage's request runs): an entry clones
//! the handler object out of the registry, releasing every borrow, before it
//! calls the object.
//!
//! What `define` declared is part of the instance's state
//! ([`crate::guest::state`]); compiled into a program, the engine `native`
//! routes an entry through `call`, with the arguments as values.

use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::adapter::values::JsonForm;
use crate::guest::core::data::store::StageOut;
use crate::guest::core::lowering::{LowerCx, Lowerer};
use crate::guest::core::registry::FeatureBuilder;
use crate::guest::core::run::{RewriteCx, StageCx};
use crate::guest::lang::rewrites::userdefs::{ExpandCx, Expander};
use crate::guest::lang::{BuiltLang, FlmDiagnostics};
use crate::guest::registry::{self, Record, Shared};
use crate::guest::state;
use crate::guest::stdcore::features::citations::{CitationRequest, ProviderCx};
use crate::guest::stdcore::features::compose::{TemplateContext, TemplateCx};
use crate::guest::techy::node::Node;
use crate::guest::techy::transform::Restager;
use crate::runtime::ExtError;

/// A feature's `register`: the builder of the registration and the
/// feature's settings.
type Register = Arc<dyn Fn(&mut FeatureBuilder, &Value) -> Result<(), ExtError> + Send + Sync>;

/// The extension builder, which the function the entry macro names
/// ([`extension!`](crate::guest::extension)) receives once per instance: its
/// one method declares a package. The twin of the JavaScript prelude's `ext`.
pub struct Extension {
    /// The extension's stem.
    stem: String,
    /// The packages declared, the first of each name.
    packages: Vec<(String, Package)>,
    /// The reports of the packages, one per [`package`](Extension::package)
    /// call, in call order.
    reports: Vec<Value>,
    /// The first refusal met, which `define` answers.
    refusal: Option<ExtError>,
}

impl Extension {
    /// Declares the package `name` of the extension, as the JavaScript
    /// prelude's `ext.package(name, object)` does: its uses, its `flm`
    /// block, its templates, its host services and its features. Each call
    /// is reported in call order; a second package of one name is reported,
    /// and the features of the first are the ones run.
    ///
    /// A feature without a `register` function ([`Feature::register`]) is
    /// refused: `define` answers the refusal.
    pub fn package(&mut self, name: impl Into<String>, package: Package) {
        let name = name.into();
        let mut features = Vec::new();
        for (feature, object) in &package.features {
            if object.register.is_none() && self.refusal.is_none() {
                self.refusal = Some(ExtError::Argument {
                    method: "package".to_string(),
                    detail: format!(
                        "the feature `{feature}` of the package `{name}` of the extension `{}` must have a `register` function",
                        self.stem
                    ),
                });
            }
            features.push(json!({ "name": feature, "object": object.object() }));
        }
        self.reports.push(json!({
            "name": name,
            "uses": package.uses,
            "flm": package.flm,
            "templates": package.templates,
            "host": package.host,
            "features": features,
        }));
        if !self.packages.iter().any(|(other, _)| *other == name) {
            self.packages.push((name, package));
        }
    }
}

/// A package of an extension, as [`Extension::package`] takes it: the
/// twin of the JavaScript prelude's package object. Each field left unset
/// is absent from the package's report.
#[derive(Clone, Default)]
pub struct Package {
    /// The packages it uses (`uses`).
    uses: Option<Vec<String>>,
    /// Its `flm` block.
    flm: Option<Value>,
    /// The paths of its compose templates' descriptions (`templates`).
    templates: Option<Vec<String>>,
    /// The host services its handlers call (`host`).
    host: Option<Vec<String>>,
    /// Its features, in the order given.
    features: Vec<(String, Feature)>,
}

impl Package {
    /// A package with nothing declared.
    pub fn new() -> Package {
        Package::default()
    }

    /// The packages it uses, by their `use` entries (`"stdcore"`, …).
    pub fn uses(mut self, uses: impl IntoIterator<Item = impl Into<String>>) -> Package {
        self.uses = Some(uses.into_iter().map(Into::into).collect());
        self
    }

    /// Its `flm` block, a map of the keys a package's `flm` block takes.
    pub fn flm(mut self, flm: Value) -> Package {
        self.flm = Some(flm);
        self
    }

    /// The paths of its compose templates' descriptions, inside the
    /// extension's folder: `templates/OUTPUT/NAME.yaml` or
    /// `templates/OUTPUT/NAME/flmtemplate.yaml`, the compose template's name
    /// being `NAME`; the host refuses a path of neither shape.
    pub fn templates(mut self, templates: impl IntoIterator<Item = impl Into<String>>) -> Package {
        self.templates = Some(templates.into_iter().map(Into::into).collect());
        self
    }

    /// The host services its handlers call (`"files"`, …).
    pub fn host(mut self, host: impl IntoIterator<Item = impl Into<String>>) -> Package {
        self.host = Some(host.into_iter().map(Into::into).collect());
        self
    }

    /// Adds the feature `name`, after the features added so far.
    pub fn feature(mut self, name: impl Into<String>, feature: Feature) -> Package {
        self.features.push((name.into(), feature));
        self
    }
}

/// A feature of a package, as [`Package::feature`] takes it: the twin of
/// the JavaScript prelude's feature object, its `register` function and the
/// data beside it. Each field left unset is absent from the feature's
/// report.
#[derive(Clone, Default)]
pub struct Feature {
    /// Its one-line title.
    title: Option<String>,
    /// Its documentation.
    docs: Option<String>,
    /// Its version.
    version: Option<String>,
    /// Its settings: `config` and `options`.
    settings: Option<Value>,
    /// Its `register`.
    register: Option<Register>,
}

impl Feature {
    /// A feature with nothing declared; it needs a [`register`](Feature::register).
    pub fn new() -> Feature {
        Feature::default()
    }

    /// Its one-line title.
    pub fn title(mut self, title: impl Into<String>) -> Feature {
        self.title = Some(title.into());
        self
    }

    /// Its documentation prose.
    pub fn docs(mut self, docs: impl Into<String>) -> Feature {
        self.docs = Some(docs.into());
        self
    }

    /// Its version.
    pub fn version(mut self, version: impl Into<String>) -> Feature {
        self.version = Some(version.into());
        self
    }

    /// Its settings, as the JavaScript feature object's `settings`: a map of
    /// `config` and `options`.
    pub fn settings(mut self, settings: Value) -> Feature {
        self.settings = Some(settings);
        self
    }

    /// Its `register`: a function or a closure that declares the feature's
    /// contributions through the builder, given the feature's settings. It
    /// runs once per registration, and again on each instance that must run
    /// one of the feature's handlers (the rehearsal), with a builder that
    /// sends nothing: it must declare the same things under the same names,
    /// in the same order, every time it runs with one configuration.
    pub fn register(mut self, register: impl Fn(&mut FeatureBuilder, &Value) -> Result<(), ExtError> + Send + Sync + 'static) -> Feature {
        self.register = Some(Arc::new(register));
        self
    }

    /// The feature's object as its report writes it: the fields set.
    fn object(&self) -> Value {
        let mut object = Map::new();
        let texts = [("title", &self.title), ("docs", &self.docs), ("version", &self.version)];
        for (key, text) in texts {
            if let Some(text) = text {
                object.insert(key.to_string(), Value::String(text.clone()));
            }
        }
        if let Some(settings) = &self.settings {
            object.insert("settings".to_string(), settings.clone());
        }
        Value::Object(object)
    }
}

/// What `define` declared on this instance: the stem, the packages, their
/// reports. Part of the instance's state ([`state::State`]).
#[derive(Default)]
pub(crate) struct Defined {
    /// The extension's stem.
    stem: String,
    /// The packages, the first of each name.
    packages: Vec<(String, Package)>,
    /// The reports `packages` answers.
    reports: Vec<Value>,
}

/// The extension's stem.
fn stem() -> String {
    state::current().defined.borrow().stem.clone()
}

/// The names of the entries, in the order of the prelude's.
const ENTRIES: [&str; 18] = [
    "define",
    "packages",
    "register",
    "rehearse",
    "run_stage",
    "expand",
    "lower",
    "restage",
    "answer_all",
    "template_describe",
    "template_files",
    "template_render",
    "points",
    "level",
    "in_toc",
    "toc_title",
    "late_init",
    "finalize",
];

/// The reply to the entry request `request` (the bytes the host wrote, or
/// `None` when no buffer of the guest holds them), `define` the function
/// the entry macro names: the envelope's JSON text.
pub(crate) fn answer(request: Option<&[u8]>, define: fn(&mut Extension)) -> Vec<u8> {
    let reply = match request.and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok()) {
        Some(Value::Object(mut request)) => {
            let name = request.remove("entry");
            match (name.as_ref().and_then(Value::as_str), request.remove("args")) {
                (Some(name), Some(Value::Array(args))) => route(name, &Args { entry: name, items: &args }, define),
                _ => Err(not_a_request()),
            }
        }
        _ => Err(not_a_request()),
    };
    let envelope = match reply {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "err": error.data(), "message": error.to_string() }),
    };
    envelope.to_string().into_bytes()
}

/// Runs the entry `name` with the arguments `args` in the same process (the
/// engine `native`, [`crate::native`]): the routing of [`answer`], without
/// the JSON text either way. `define` is the function the entry macro gives.
///
/// # Errors
///
/// The entry's refusal (an unknown entry, an argument of the wrong form, a
/// handler object `register` did not declare) or what the handler answers.
#[cfg(feature = "native")]
pub(crate) fn call(name: &str, args: &[Value], define: fn(&mut Extension)) -> Result<Value, ExtError> {
    route(name, &Args { entry: name, items: args }, define)
}

/// The refusal of a request that is not an entry's.
fn not_a_request() -> ExtError {
    ExtError::Handler {
        detail: "the request of an entry must be the JSON text `{\"entry\": name, \"args\": [...]}` in a buffer of the guest".to_string(),
        data: Value::Null,
    }
}

/// The arguments of the entry `entry`.
struct Args<'a> {
    /// The entry's name.
    entry: &'a str,
    /// Its arguments.
    items: &'a [Value],
}

impl Args<'_> {
    /// The argument at `index` (`null` when absent).
    fn get(&self, index: usize) -> &Value {
        self.items.get(index).unwrap_or(&Value::Null)
    }

    /// The argument at `index`, read through its JSON form.
    fn form<T: JsonForm>(&self, index: usize) -> Result<T, ExtError> {
        T::from_value(self.get(index)).map_err(|error| match error {
            ExtError::Value { detail } => {
                ExtError::Argument { method: self.entry.to_string(), detail: format!("argument {}: {detail}", index + 1) }
            }
            other => other,
        })
    }

    /// The argument at `index`, a text.
    fn text(&self, index: usize) -> Result<String, ExtError> {
        self.form(index)
    }
}

/// Runs the entry `name`.
fn route(name: &str, args: &Args<'_>, define: fn(&mut Extension)) -> Result<Value, ExtError> {
    if !ENTRIES.contains(&name) {
        return Err(unknown(name));
    }
    match name {
        "define" => define_with(&args.text(0)?, define).map(|()| Value::Null),
        "packages" => Ok(Value::Array(state::current().defined.borrow().reports.clone())),
        "register" => {
            let builder: FeatureBuilder = args.form(3)?;
            register_with(&args.text(0)?, &args.text(1)?, args.get(2), builder, false).map(|()| Value::Null)
        }
        "rehearse" => rehearse(&args.text(0)?, &args.text(1)?, args.get(2)).map(|()| Value::Null),
        "late_init" => late_init(args).map(|()| Value::Null),
        "finalize" => finalize(args).map(|()| Value::Null),
        _ => handler(name, args, &args.text(0)?, &args.text(1)?),
    }
}

/// The refusal of an entry the guest does not answer.
fn unknown(name: &str) -> ExtError {
    let names = ENTRIES.map(|name| format!("`{name}`")).join(", ");
    refusal(format!("the entry `{name}` must be one of the guest's entries: {names}"))
}

/// `define(stem, module)`: runs `define` with the extension builder, and
/// keeps what it declared.
fn define_with(stem: &str, define: fn(&mut Extension)) -> Result<(), ExtError> {
    *state::current().defined.borrow_mut() = Defined { stem: stem.to_string(), ..Defined::default() };
    let mut ext = Extension { stem: stem.to_string(), packages: Vec::new(), reports: Vec::new(), refusal: None };
    define(&mut ext);
    {
        let state = state::current();
        let mut defined = state.defined.borrow_mut();
        defined.packages = ext.packages;
        defined.reports = ext.reports;
    }
    ext.refusal.map_or(Ok(()), Err)
}

/// A refusal of the handler's entry, `detail` its text.
fn refusal(detail: String) -> ExtError {
    ExtError::Handler { detail, data: Value::Null }
}

/// The `register` of the feature `feature` of the package `pkg`, as the
/// messages of the entries name it.
fn owner(pkg: &str, feature: &str) -> String {
    format!("`register` of the feature `{feature}` of the package `{pkg}` of the extension `{}`", stem())
}

/// The `register` function of the feature `feature` of the package `pkg`.
fn find_feature(pkg: &str, feature: &str) -> Result<Register, ExtError> {
    let found = {
        let state = state::current();
        let defined = state.defined.borrow();
        let package = defined.packages.iter().find(|(name, _)| name == pkg).map(|(_, package)| package);
        let feature = package.and_then(|package| package.features.iter().find(|(name, _)| name == feature));
        feature.and_then(|(_, feature)| feature.register.clone())
    };
    found.ok_or_else(|| refusal(format!("the package `{pkg}` of the extension `{}` must have a feature `{feature}`", stem())))
}

/// Runs the feature's `register(b, settings)` with `builder`, recording what
/// it declares; `rehearsal` when the builder sends nothing.
fn register_with(pkg: &str, feature: &str, settings: &Value, mut builder: FeatureBuilder, rehearsal: bool) -> Result<(), ExtError> {
    registry::begin(&stem(), pkg, feature, rehearsal);
    let outcome = find_feature(pkg, feature).and_then(|register| register(&mut builder, settings));
    let done = registry::end();
    outcome?;
    if let Some(done) = done {
        registry::keep(pkg, feature, done);
    }
    Ok(())
}

/// The rehearsal: the feature's `register` again, with the settings of the
/// real registration and a builder that sends nothing, then each finalizer
/// it registered with such a builder (those `register` recorded: a finalizer
/// a finalizer registers is not replayed), so that this instance holds the
/// objects the feature declared.
fn rehearse(pkg: &str, feature: &str, settings: &Value) -> Result<(), ExtError> {
    register_with(pkg, feature, settings, FeatureBuilder { handle: None }, true)?;
    let Some(done) = registry::recorded(pkg, feature) else { return Ok(()) };
    let count = done.borrow().finalizers.len();
    for index in 0..count {
        finalize_with(&done, index, FeatureBuilder { handle: None }, true)?;
    }
    Ok(())
}

/// Runs the finalizer at the position `index` of what `done` recorded with
/// `builder`, recording what it declares into `done`.
fn finalize_with(done: &Shared, index: usize, mut builder: FeatureBuilder, rehearsal: bool) -> Result<(), ExtError> {
    let Some(finalizer) = done.borrow().finalizers.get(index).cloned() else { return Ok(()) };
    registry::continue_with(done, rehearsal);
    let outcome = finalizer.finalize(&mut builder);
    registry::end();
    outcome
}

/// The object `pick` finds in what the last `register` of the feature
/// recorded, cloned out of the record.
fn find<T>(pkg: &str, feature: &str, pick: impl FnOnce(&Record) -> Option<T>) -> Option<T> {
    registry::recorded(pkg, feature).and_then(|done| pick(&done.borrow()))
}

/// The handler object `pick` finds among what the feature contributed:
/// from the `register` of that feature running now (a template module's
/// `describe` runs inside it), else from what its last `register` on this
/// instance recorded. `what` names it in the refusal.
fn contributed<T>(pkg: &str, feature: &str, what: &str, pick: impl Fn(&Record) -> Option<T>) -> Result<T, ExtError> {
    let live = registry::running().filter(|live| {
        let live = live.borrow();
        live.package == pkg && live.feature == feature
    });
    let found = match live {
        Some(live) => pick(&live.borrow()),
        None => find(pkg, feature, pick),
    };
    found.ok_or_else(|| refusal(format!("{what} must be contributed by {}", owner(pkg, feature))))
}

/// The element's handler aspect `pick` finds, or the refusal whose text
/// begins with `what`.
fn aspect<T>(pkg: &str, feature: &str, element: &str, what: &str, pick: impl FnOnce(&registry::Aspects) -> Option<T>) -> Result<T, ExtError> {
    find(pkg, feature, |done| done.aspects.get(element).and_then(pick)).ok_or_else(|| {
        refusal(format!("{what} of the element `{element}` must be answered by an aspect attached by {}", owner(pkg, feature)))
    })
}

/// The handler entries that answer a value: the stage, the hook, the rule's
/// body, the rewrite stage, the provider, the template module and the
/// aspects, each found by its name or element.
fn handler(entry: &str, args: &Args<'_>, pkg: &str, feature: &str) -> Result<Value, ExtError> {
    match entry {
        "run_stage" => {
            let name = args.text(2)?;
            let stage = find(pkg, feature, |done| done.stages.get(&name).cloned()).ok_or_else(|| {
                refusal(format!("the stage `{name}` must be declared by {}", owner(pkg, feature)))
            })?;
            let cx: StageCx = args.form(3)?;
            let mut out: StageOut = args.form(4)?;
            stage.run(&cx, &mut out)?;
            Ok(Value::Null)
        }
        "expand" => {
            let element = args.text(2)?;
            let hook = find(pkg, feature, |done| done.hooks.get(&element).cloned()).ok_or_else(|| {
                refusal(format!("the expansion hook of the element `{element}` must be attached by {}", owner(pkg, feature)))
            })?;
            let node: Node = args.form(3)?;
            let mut cx: ExpandCx = args.form(4)?;
            let mut xp: Expander = args.form(5)?;
            Ok(hook.expand(&node, &mut cx, &mut xp)?.to_value())
        }
        "lower" => {
            let rule = args.text(2)?;
            let body = find(pkg, feature, |done| done.lowers.get(&rule).cloned()).ok_or_else(|| {
                refusal(format!("the lowering rule `{rule}` must be built by {}", owner(pkg, feature)))
            })?;
            let node: Node = args.form(3)?;
            let mut cx: LowerCx = args.form(4)?;
            let mut lw: Lowerer = args.form(5)?;
            Ok(body.lower(&node, &mut cx, &mut lw)?.to_value())
        }
        "restage" => {
            let name = args.text(2)?;
            let stage = find(pkg, feature, |done| done.rewrites.get(&name).cloned()).ok_or_else(|| {
                refusal(format!("the rewrite stage `{name}` must be declared by {}", owner(pkg, feature)))
            })?;
            let node: Node = args.form(3)?;
            let cx: RewriteCx = args.form(4)?;
            let mut rs: Restager = args.form(5)?;
            Ok(stage.restage(&node, &cx, &mut rs)?.to_value())
        }
        "answer_all" => {
            let name = args.text(2)?;
            let what = format!("the citation provider `{name}`");
            let provider = contributed(pkg, feature, &what, |done| done.providers.get(&name).cloned())?;
            let requests: Vec<CitationRequest> = args.form(3)?;
            let cx: ProviderCx = args.form(4)?;
            Ok(provider.answer_all(&requests, &cx)?.to_value())
        }
        "template_describe" | "template_files" | "template_render" => {
            let name = args.text(2)?;
            let what = format!("the template module `{name}`");
            let template = contributed(pkg, feature, &what, |done| done.templates.get(&name).cloned())?;
            match entry {
                "template_describe" => Ok(template.describe(args.get(3))?.to_value()),
                "template_files" => {
                    let data: TemplateContext = args.form(3)?;
                    let cx: TemplateCx = args.form(4)?;
                    Ok(template.files(&data, &cx)?.to_value())
                }
                _ => {
                    let data: TemplateContext = args.form(3)?;
                    let cx: TemplateCx = args.form(4)?;
                    Ok(Value::String(template.render(&data, &cx)?))
                }
            }
        }
        "points" => {
            let element = args.text(2)?;
            let object = aspect(pkg, feature, &element, "the reference points", |aspects| aspects.referenceable.clone())?;
            let node: Node = args.form(3)?;
            Ok(object.points(&node)?.to_value())
        }
        "level" | "in_toc" | "toc_title" => {
            let element = args.text(2)?;
            let object = aspect(pkg, feature, &element, "the sectioning", |aspects| aspects.sectioning.clone())?;
            let node: Node = args.form(3)?;
            match entry {
                "level" => Ok(object.level(&node)?.to_value()),
                "in_toc" => Ok(Value::Bool(object.in_toc(&node)?)),
                _ => Ok(object.toc_title(&node)?.to_value()),
            }
        }
        other => Err(unknown(other)),
    }
}

/// `late_init(pkg, feature, index, lang, diags)`: the `late_init(lang,
/// diags)` of the feature's late-initialization hook at the position
/// `index`.
fn late_init(args: &Args<'_>) -> Result<(), ExtError> {
    let (pkg, feature) = (args.text(0)?, args.text(1)?);
    let index = args.get(2);
    let position = index.as_u64().and_then(|index| usize::try_from(index).ok());
    let hook = position.and_then(|position| find(&pkg, &feature, |done| done.late_inits.get(position).cloned())).ok_or_else(|| {
        refusal(format!("the late-initialization hook {index} must be registered by {}", owner(&pkg, &feature)))
    })?;
    let lang: BuiltLang = args.form(3)?;
    let mut diags: FlmDiagnostics = args.form(4)?;
    hook.late_init(&lang, &mut diags)
}

/// `finalize(pkg, feature, index, b)`: the `finalize(b)` of the feature's
/// finalizer at the position `index`, with the builder of the build's
/// second round; what it declares is recorded with what the feature's
/// `register` declared.
fn finalize(args: &Args<'_>) -> Result<(), ExtError> {
    let (pkg, feature) = (args.text(0)?, args.text(1)?);
    let index = args.get(2);
    let position = index.as_u64().and_then(|index| usize::try_from(index).ok());
    let done = registry::recorded(&pkg, &feature)
        .filter(|done| position.is_some_and(|position| position < done.borrow().finalizers.len()))
        .ok_or_else(|| refusal(format!("the finalizer {index} must be registered by {}", owner(&pkg, &feature))))?;
    let builder: FeatureBuilder = args.form(3)?;
    finalize_with(&done, position.unwrap_or_default(), builder, false)
}

#[cfg(test)]
pub(crate) mod tests {
    //! The runtime on the host, the ABI's test hook answering in place of the
    //! host: what a `register` records and lends, the rehearsal, the 18
    //! entries, `define` and `packages`, and re-entry.

    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use serde_json::{json, Value};

    use super::*;
    use crate::adapter::handles::{Handle, HandleKind};
    use crate::adapter::values::handle_to_value;
    use crate::api::core::lowering::{DefaultMark, LoweringDecl};
    use crate::api::core::stage::{Effect, Needs, RewriteDecl, Scope, StageDecl};
    use crate::api::stdcore::features::compose::{TemplateDescription, TemplateFile};
    use crate::guest::abi::hook;
    use crate::guest::core::lowering::{Lower, Lowering, LoweringRule};
    use crate::guest::core::registry::{BackendDef, BackendStage, Contributed, Finalizer, LateInit, LibraryStage};
    use crate::guest::core::stage::{RewriteStage, Stage};
    use crate::guest::lang::rewrites::userdefs::{Expand, Expansion, UserDefinition};
    use crate::guest::model::aspects::{RefPoint, Referenceable, Sectioning};
    use crate::guest::model::element::{Aspect, ElementSpec};
    use crate::guest::stdcore::features::citations::{CitationProvider, CitationResult};
    use crate::guest::stdcore::features::compose::TemplateModule;
    use crate::guest::techy::transform::Restage;

    /// A handle of the kind `kind` at `index`.
    pub(crate) fn handle(kind: HandleKind, index: u32) -> Value {
        handle_to_value(Handle { kind, call: 1, index })
    }

    /// The kind of the handle a lending request answers.
    pub(crate) fn lent_kind(request: &Value) -> Option<HandleKind> {
        Some(match request["native"].as_str()? {
            "lend_stage" => HandleKind::Stage,
            "lend_expand" => HandleKind::Expand,
            "lend_lower" => HandleKind::Lower,
            "lend_rewrite" => HandleKind::RewriteStage,
            "lend_provider" => HandleKind::CitationProvider,
            "lend_template" => HandleKind::TemplateModule,
            "lend_aspect" if request["kind"] == "Sectioning" => HandleKind::Sectioning,
            "lend_aspect" => HandleKind::Referenceable,
            "lend_late_init" => HandleKind::LateInit,
            "lend_finalizer" => HandleKind::Finalizer,
            "module_handle" => HandleKind::Module,
            _ => return None,
        })
    }

    /// The requests the hook received, in order.
    type Log = Rc<RefCell<Vec<Value>>>;

    /// Installs a hook that logs every request and answers it as the host
    /// would: a lending with a handle of its kind, a call with `reply(method)`
    /// (`null` when it answers `None`).
    fn install(reply: impl Fn(&Value) -> Option<Value> + 'static) -> Log {
        let log: Log = Rc::default();
        let kept = Rc::clone(&log);
        let count = Cell::new(0_u32);
        hook::install(move |request| {
            kept.borrow_mut().push(request.clone());
            count.set(count.get() + 1);
            if let Some(kind) = lent_kind(request) {
                return json!({ "ok": handle(kind, count.get()) });
            }
            json!({ "ok": reply(request).unwrap_or(Value::Null) })
        });
        log
    }

    /// The host's answer to the calls the test feature's `register` makes.
    fn host_reply(request: &Value) -> Option<Value> {
        match request["method"].as_str()? {
            "feature" => Some(json!("wordcount")),
            "UserDefinition::new" => Some(handle(HandleKind::UserDefinition, 90)),
            "element_def_with" => Some(handle(HandleKind::ElementDef, 91)),
            "LoweringRule::new" => Some(handle(HandleKind::LoweringRule, 92)),
            "trees" => Some(json!([])),
            _ => None,
        }
    }

    thread_local! {
        /// What the test's handler objects saw, on this thread.
        static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    /// Notes that `what` ran.
    fn saw(what: &str) {
        SEEN.with(|seen| seen.borrow_mut().push(what.to_string()));
    }

    /// What ran so far.
    fn seen() -> Vec<String> {
        SEEN.with(|seen| seen.borrow().clone())
    }

    struct Count;
    impl Stage for Count {
        fn decl(&self) -> StageDecl {
            StageDecl::new("wc.count", Effect::Compute, Scope::PerTree)
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn run(&self, cx: &StageCx, _out: &mut StageOut) -> Result<(), ExtError> {
            saw("run");
            cx.trees()?;
            Ok(())
        }
    }

    struct Emit;
    impl Stage for Emit {
        fn decl(&self) -> StageDecl {
            StageDecl::new("wc.emit", Effect::Compute, Scope::Document)
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn run(&self, _: &StageCx, _: &mut StageOut) -> Result<(), ExtError> {
            Ok(())
        }
    }

    struct Late;
    impl Stage for Late {
        fn decl(&self) -> StageDecl {
            StageDecl::new("wc.final", Effect::Compute, Scope::Document)
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn run(&self, _: &StageCx, _: &mut StageOut) -> Result<(), ExtError> {
            Ok(())
        }
    }

    struct Rewrite;
    impl RewriteStage for Rewrite {
        fn decl(&self) -> RewriteDecl {
            RewriteDecl::new("wc.rewrite", "wc.rewritten")
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn restage(&self, _: &Node, _: &RewriteCx, _: &mut Restager) -> Result<Restage, ExtError> {
            Ok(Restage::Descend)
        }
    }

    struct Hook;
    impl Expand for Hook {
        fn expand(&self, _: &Node, _: &mut ExpandCx, _: &mut Expander) -> Result<Expansion, ExtError> {
            saw("expand");
            Ok(Expansion::Default)
        }
    }

    struct Body;
    impl Lower for Body {
        fn lower(&self, _: &Node, _: &mut LowerCx, _: &mut Lowerer) -> Result<Lowering, ExtError> {
            Ok(Lowering::Default)
        }
    }

    struct Points;
    impl Referenceable for Points {
        fn points(&self, _: &Node) -> Result<Vec<RefPoint>, ExtError> {
            Ok(Vec::new())
        }
    }

    struct Heading;
    impl Sectioning for Heading {
        fn level(&self, _: &Node) -> Result<u32, ExtError> {
            Ok(2)
        }
    }

    struct Provider;
    impl CitationProvider for Provider {
        fn name(&self) -> String {
            "wc.cites".to_string()
        }
        fn answer(&self, _: &CitationRequest, _: &ProviderCx) -> Result<CitationResult, ExtError> {
            Ok(Ok(None))
        }
    }

    struct Template;
    impl TemplateModule for Template {
        fn name(&self) -> String {
            "wc.page".to_string()
        }
        fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
            TemplateDescription::from_value(&json!({ "output": "html", "accepts": "all", "requires": [], "config": {} }))
        }
        fn render(&self, _: &TemplateContext, _: &TemplateCx) -> Result<String, ExtError> {
            Ok("rendered".to_string())
        }
    }

    struct LateHook;
    impl LateInit for LateHook {
        fn late_init(&self, _: &BuiltLang, _: &mut FlmDiagnostics) -> Result<(), ExtError> {
            saw("late_init");
            Ok(())
        }
    }

    struct Final;
    impl Finalizer for Final {
        fn finalize(&self, b: &mut FeatureBuilder) -> Result<(), ExtError> {
            saw("finalize");
            b.stage(Arc::new(Late))
        }
    }

    /// The test feature's `register`: one object of each kind.
    fn register(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
        assert_eq!(b.feature()?, "wordcount");
        b.stage(Arc::new(Count))?;
        b.rewrite_stage(Arc::new(Rewrite))?;
        b.attach("wc.heading", Aspect::Sectioning(Arc::new(Heading)))?;
        b.attach("wc.ref", Aspect::Referenceable(Arc::new(Points)))?;
        b.attach("wc.hooked", Aspect::Expand(Arc::new(Hook)))?;
        b.contribute_to("flm.citations.providers", Contributed::CitationProvider(Arc::new(Provider)))?;
        b.peer_slot("flm.compose.templates", Contributed::TemplateModule(Arc::new(Template)))?;
        b.contribute_first_to("wc.data", Contributed::Data(json!({ "k": 1 })))?;
        b.late_init(Arc::new(LateHook))?;
        b.finalizer(Arc::new(Final))?;
        LoweringRule::new("wc.rule", "wc.badge", LoweringDecl::new(), Arc::new(Body), DefaultMark::Everywhere)?;
        ElementSpec::new("wc.badge").with(Aspect::Expand(Arc::new(Hook)))?;
        let definition = crate::guest::lang::rewrites::userdefs::Definition::new(
            crate::api::lang::rewrites::userdefs::DefinitionKind::Macro,
            "shout",
            Vec::new(),
        );
        UserDefinition::new(definition)?.element_def_with("wc", "wc.shout", Arc::new(Hook))?;
        let library = BackendStage::Library(LibraryStage { handle: Handle { kind: HandleKind::LibraryStage, call: 1, index: 80 } });
        BackendDef::new("wc", Vec::new(), library, BackendStage::Own(Arc::new(Emit)), BackendStage::Own(Arc::new(Emit)))?;
        Ok(())
    }

    /// The test extension: one package, its one feature.
    fn define(ext: &mut Extension) {
        ext.package("_", Package::new().uses(["stdcore"]).feature("wordcount", Feature::new().title("Word count").register(register)));
    }

    /// Runs the entry `entry` with `args`; answers its envelope.
    pub(crate) fn run(entry: &str, args: Value) -> Value {
        let request = json!({ "entry": entry, "args": args }).to_string();
        serde_json::from_slice(&answer(Some(request.as_bytes()), define)).expect("an envelope")
    }

    /// The `ok` of the entry's envelope.
    fn ok(entry: &str, args: Value) -> Value {
        let reply = run(entry, args);
        assert!(reply.get("ok").is_some(), "the entry `{entry}` answered {reply}");
        reply["ok"].clone()
    }

    /// The message of the entry's `err`.
    fn refused(entry: &str, args: Value) -> String {
        let reply = run(entry, args);
        reply["message"].as_str().unwrap_or_else(|| panic!("the entry `{entry}` answered {reply}")).to_string()
    }

    /// The natives of the requests logged, in order.
    fn natives(log: &Log) -> Vec<String> {
        log.borrow().iter().map(|request| match request["native"].as_str() {
            Some("call") => format!("call {}", request["method"].as_str().unwrap_or_default()),
            other => other.unwrap_or_default().to_string(),
        }).collect()
    }

    /// Defines the extension and registers its feature for real.
    fn registered() -> Log {
        let log = install(host_reply);
        assert_eq!(ok("define", json!(["wc", "wc.wasm"])), Value::Null);
        assert_eq!(ok("register", json!(["_", "wordcount", {}, handle(HandleKind::FeatureBuilder, 0)])), Value::Null);
        log
    }

    #[test]
    fn a_register_records_and_lends_each_kind_of_object() {
        let log = registered();
        assert_eq!(
            natives(&log),
            [
                "call feature",
                "lend_stage",
                "call stage",
                "lend_rewrite",
                "call rewrite_stage",
                "lend_aspect",
                "call attach",
                "lend_aspect",
                "call attach",
                "lend_expand",
                "call attach",
                "lend_provider",
                "call contribute_to",
                "lend_template",
                "call peer_slot",
                "call contribute_first_to",
                "lend_late_init",
                "call late_init",
                "lend_finalizer",
                "call finalizer",
                "lend_lower",
                "module_handle",
                "call LoweringRule::new",
                "lend_expand",
                "module_handle",
                "call UserDefinition::new",
                "lend_expand",
                "call element_def_with",
                "lend_stage",
                "lend_stage",
            ]
        );
        let requests = log.borrow();
        // A `Sectioning` is lent with its three methods, a `Referenceable` with `points`.
        assert_eq!(requests[5], json!({ "native": "lend_aspect", "kind": "Sectioning", "element": "wc.heading", "methods": ["level", "in_toc", "toc_title"] }));
        assert_eq!(requests[7]["methods"], json!(["points"]));
        assert_eq!(requests[6]["args"], json!(["wc.heading", { "Sectioning": handle(HandleKind::Sectioning, 6) }]));
        assert_eq!(requests[9], json!({ "native": "lend_expand", "element": "wc.hooked", "policy": "per-plan" }));
        assert_eq!(requests[1]["decl"]["name"], "wc.count");
        assert_eq!(requests[15]["args"], json!(["wc.data", { "k": 1 }]));
        assert_eq!(requests[16], json!({ "native": "lend_late_init", "index": 0 }));
        assert_eq!(requests[22]["args"][3], handle(HandleKind::Lower, 21), "the rule's body is sent as the handle its lending answered");
        drop(requests);
        let done = registry::recorded("_", "wordcount").expect("the record is kept");
        let done = done.borrow();
        assert_eq!(done.stages.keys().collect::<Vec<_>>(), ["wc.count", "wc.emit"]);
        assert_eq!(done.rewrites.keys().collect::<Vec<_>>(), ["wc.rewrite"]);
        assert_eq!(done.hooks.keys().collect::<Vec<_>>(), ["wc.badge", "wc.hooked", "wc.shout"]);
        assert_eq!(done.lowers.keys().collect::<Vec<_>>(), ["wc.rule"]);
        assert_eq!(done.providers.keys().collect::<Vec<_>>(), ["wc.cites"]);
        assert_eq!(done.templates.keys().collect::<Vec<_>>(), ["wc.page"]);
        assert!(done.aspects["wc.heading"].sectioning.is_some() && done.aspects["wc.ref"].referenceable.is_some());
        assert_eq!((done.late_inits.len(), done.finalizers.len()), (1, 1));
        assert!(registry::running().is_none(), "the running record ends with `register`");
    }

    #[test]
    fn a_rehearsal_records_and_sends_nothing_through_the_builder() {
        let log = install(host_reply);
        ok("define", json!(["wc", "wc.wasm"]));
        assert_eq!(ok("rehearse", json!(["_", "wordcount", {}])), Value::Null);
        // Only what lends in every run sends: the rule's constructor, the
        // spec's aspect, the helper's hook. The builder sends nothing, and a
        // backend's own stages are recorded, not lent.
        assert_eq!(
            natives(&log),
            ["lend_lower", "module_handle", "call LoweringRule::new", "lend_expand", "module_handle", "call UserDefinition::new", "lend_expand", "call element_def_with"]
        );
        let done = registry::recorded("_", "wordcount").expect("the rehearsal's record is kept");
        let done = done.borrow();
        // The finalizer `register` recorded was replayed, recording its stage.
        assert_eq!(done.stages.keys().collect::<Vec<_>>(), ["wc.count", "wc.emit", "wc.final"]);
        assert_eq!(done.providers.len() + done.templates.len() + done.late_inits.len() + done.finalizers.len(), 4);
        assert!(done.rehearsal);
        assert_eq!(seen(), ["finalize"]);
    }

    #[test]
    fn each_entry_finds_its_object() {
        registered();
        let node = handle(HandleKind::Node, 1);
        let at = |kind| handle(kind, 2);
        assert_eq!(ok("run_stage", json!(["_", "wordcount", "wc.count", at(HandleKind::StageCx), at(HandleKind::StageOut)])), Value::Null);
        assert_eq!(ok("expand", json!(["_", "wordcount", "wc.hooked", node, at(HandleKind::ExpandCx), at(HandleKind::Expander)])), json!("default"));
        assert_eq!(ok("lower", json!(["_", "wordcount", "wc.rule", node, at(HandleKind::LowerCx), at(HandleKind::Lowerer)])), json!("default"));
        assert_eq!(ok("restage", json!(["_", "wordcount", "wc.rewrite", node, at(HandleKind::RewriteCx), at(HandleKind::Restager)])), json!("descend"));
        assert_eq!(ok("answer_all", json!(["_", "wordcount", "wc.cites", [{ "key": "a" }, { "key": "b" }], at(HandleKind::ProviderCx)])), json!([null, null]));
        let described = ok("template_describe", json!(["_", "wordcount", "wc.page", {}]));
        assert_eq!(described["output"], "html");
        let data = TemplateContext::from_value(&json!({ "html": true, "regions": [], "pieces": [], "meta": {}, "config": {}, "artifacts": [] })).map(|data| data.to_value()).expect("template data");
        assert_eq!(ok("template_files", json!(["_", "wordcount", "wc.page", data, at(HandleKind::TemplateCx)])), json!(Vec::<TemplateFile>::new().to_value()));
        assert_eq!(ok("template_render", json!(["_", "wordcount", "wc.page", data, at(HandleKind::TemplateCx)])), json!("rendered"));
        assert_eq!(ok("points", json!(["_", "wordcount", "wc.ref", node])), json!([]));
        assert_eq!(ok("level", json!(["_", "wordcount", "wc.heading", node])), json!(2));
        // The defaults of `in_toc` and `toc_title` run in the guest.
        assert_eq!(ok("in_toc", json!(["_", "wordcount", "wc.heading", node])), json!(true));
        assert_eq!(ok("toc_title", json!(["_", "wordcount", "wc.heading", node])), Value::Null);
        assert_eq!(ok("late_init", json!(["_", "wordcount", 0, at(HandleKind::BuiltLang), at(HandleKind::FlmDiagnostics)])), Value::Null);
        assert_eq!(ok("finalize", json!(["_", "wordcount", 0, handle(HandleKind::FeatureBuilder, 3)])), Value::Null);
        assert_eq!(seen(), ["run", "expand", "late_init", "finalize"]);
        // What a finalizer declares is recorded with what `register` declared.
        let done = registry::recorded("_", "wordcount").expect("a record");
        assert!(done.borrow().stages.contains_key("wc.final"));
    }

    #[test]
    fn each_entry_refuses_with_the_preludes_text_when_its_object_is_absent() {
        registered();
        let node = handle(HandleKind::Node, 1);
        let owner = "`register` of the feature `wordcount` of the package `_` of the extension `wc`";
        let cases = [
            ("run_stage", json!(["_", "wordcount", "wc.none", node, node]), format!("the stage `wc.none` must be declared by {owner}")),
            ("expand", json!(["_", "wordcount", "wc.none", node, node, node]), format!("the expansion hook of the element `wc.none` must be attached by {owner}")),
            ("lower", json!(["_", "wordcount", "wc.none", node, node, node]), format!("the lowering rule `wc.none` must be built by {owner}")),
            ("restage", json!(["_", "wordcount", "wc.none", node, node, node]), format!("the rewrite stage `wc.none` must be declared by {owner}")),
            ("answer_all", json!(["_", "wordcount", "wc.none", [], node]), format!("the citation provider `wc.none` must be contributed by {owner}")),
            ("template_describe", json!(["_", "wordcount", "wc.none", {}]), format!("the template module `wc.none` must be contributed by {owner}")),
            ("template_files", json!(["_", "wordcount", "wc.none", {}, node]), format!("the template module `wc.none` must be contributed by {owner}")),
            ("template_render", json!(["_", "wordcount", "wc.none", {}, node]), format!("the template module `wc.none` must be contributed by {owner}")),
            ("points", json!(["_", "wordcount", "wc.none", node]), format!("the reference points of the element `wc.none` must be answered by an aspect attached by {owner}")),
            ("level", json!(["_", "wordcount", "wc.ref", node]), format!("the sectioning of the element `wc.ref` must be answered by an aspect attached by {owner}")),
            ("in_toc", json!(["_", "wordcount", "wc.none", node]), format!("the sectioning of the element `wc.none` must be answered by an aspect attached by {owner}")),
            ("toc_title", json!(["_", "wordcount", "wc.none", node]), format!("the sectioning of the element `wc.none` must be answered by an aspect attached by {owner}")),
            ("late_init", json!(["_", "wordcount", 4, node, node]), format!("the late-initialization hook 4 must be registered by {owner}")),
            ("finalize", json!(["_", "wordcount", 4, node]), format!("the finalizer 4 must be registered by {owner}")),
            ("register", json!(["_", "nofeature", {}, handle(HandleKind::FeatureBuilder, 0)]), "the package `_` of the extension `wc` must have a feature `nofeature`".to_string()),
            ("rehearse", json!(["other", "wordcount", {}]), "the package `other` of the extension `wc` must have a feature `wordcount`".to_string()),
        ];
        for (entry, args, text) in cases {
            assert_eq!(refused(entry, args), text, "the entry `{entry}`");
            assert_eq!(run(entry, json!([]))["err"]["kind"], "argument", "the entry `{entry}` without its arguments");
        }
        assert_eq!(run("run_stage", json!(["_", "wordcount", "wc.none", node, node]))["err"]["kind"], "handler");
        let unknown = refused("unknown", json!([]));
        assert!(unknown.starts_with("the entry `unknown` must be one of the guest's entries: `define`, `packages`, "), "{unknown}");
        let request = String::from_utf8(answer(Some(b"not json"), define)).expect("a text");
        assert!(request.contains("the request of an entry must be the JSON text"), "{request}");
        assert!(String::from_utf8(answer(None, define)).expect("a text").contains("the request of an entry must be"));
    }

    #[test]
    fn a_handler_object_given_outside_register_is_refused() {
        install(host_reply);
        let error = FeatureBuilder { handle: None }.stage(Arc::new(Count)).expect_err("no register runs");
        assert_eq!(error.to_string(), ExtError::Handler { detail: "the stage `wc.count` must be declared while a feature's `register` runs".to_string(), data: Value::Null }.to_string());
        let error = ElementSpec::new("wc.badge").with(Aspect::Referenceable(Arc::new(Points))).expect_err("no register runs");
        assert!(error.to_string().contains("the aspect `Referenceable` of the element `wc.badge` must be attached while a feature's `register` runs"), "{error}");
        let error = FeatureBuilder { handle: None }.feature().expect_err("no register runs");
        assert!(error.to_string().contains("the builder's `feature()` must be called while a feature's `register` runs"), "{error}");
    }

    #[test]
    fn a_second_handler_object_under_one_name_is_refused() {
        install(host_reply);
        registry::begin("wc", "_", "wordcount", true);
        let mut b = FeatureBuilder { handle: None };
        b.contribute_to("flm.citations.providers", Contributed::CitationProvider(Arc::new(Provider))).expect("the first");
        let error = b.contribute_to("flm.citations.providers", Contributed::CitationProvider(Arc::new(Provider))).expect_err("the second");
        registry::end();
        assert!(
            error.to_string().contains("the citation provider `wc.cites` must be contributed once by `register` of the feature `wordcount` of the package `_` of the extension `wc`"),
            "{error}"
        );
    }

    #[test]
    fn rehearse_replays_the_recorded_finalizers() {
        registered();
        SEEN.with(|seen| seen.borrow_mut().clear());
        ok("rehearse", json!(["_", "wordcount", {}]));
        assert_eq!(seen(), ["finalize"], "the one finalizer `register` recorded is replayed once");
    }

    #[test]
    fn template_describe_finds_a_module_in_the_running_register() {
        // The host asks the module for its description while the `register`
        // that contributes it runs: inside the builder's request.
        let nested_reply = Rc::new(RefCell::new(Value::Null));
        let kept = Rc::clone(&nested_reply);
        let log = install(move |request| {
            if request["method"] == "peer_slot" {
                *kept.borrow_mut() = run("template_describe", json!(["_", "describing", "wc.page", {}]));
            }
            None
        });
        fn define(ext: &mut Extension) {
            ext.package("_", Package::new().feature("describing", Feature::new().register(|b, _| {
                b.peer_slot("flm.compose.templates", Contributed::TemplateModule(Arc::new(Template)))
            })));
        }
        let request = json!({ "entry": "define", "args": ["wc", "wc.wasm"] }).to_string();
        answer(Some(request.as_bytes()), define);
        let request = json!({ "entry": "register", "args": ["_", "describing", {}, handle(HandleKind::FeatureBuilder, 0)] }).to_string();
        let reply: Value = serde_json::from_slice(&answer(Some(request.as_bytes()), define)).expect("an envelope");
        assert_eq!(reply, json!({ "ok": null }));
        let nested = log.borrow().iter().find(|request| request["method"] == "peer_slot").cloned().expect("the builder's request");
        assert_eq!(nested["native"], "call");
        // The nested entry found the module the running `register` recorded.
        let described = nested_reply.borrow().clone();
        assert_eq!(described["ok"]["output"], "html", "{described}");
        // Outside its `register`, the module is found in what it recorded.
        let described = ok("template_describe", json!(["_", "describing", "wc.page", {}]));
        assert_eq!(described["output"], "html");
    }

    #[test]
    fn no_borrow_is_held_across_a_request() {
        // A hook entered from inside a stage's request: the stage asks for
        // the trees, and the host runs the element's expansion hook on the
        // same instance before it answers.
        registered();
        let nested = Rc::new(RefCell::new(Value::Null));
        let kept = Rc::clone(&nested);
        install(move |request| {
            if request["method"] == "trees" {
                let args = json!(["_", "wordcount", "wc.hooked", handle(HandleKind::Node, 1), handle(HandleKind::ExpandCx, 5), handle(HandleKind::Expander, 5)]);
                *kept.borrow_mut() = run("expand", args);
                return Some(json!([]));
            }
            None
        });
        let at = |kind| handle(kind, 2);
        assert_eq!(ok("run_stage", json!(["_", "wordcount", "wc.count", at(HandleKind::StageCx), at(HandleKind::StageOut)])), Value::Null);
        assert_eq!(*nested.borrow(), json!({ "ok": "default" }));
        assert_eq!(seen(), ["run", "expand"]);
    }

    #[test]
    fn define_and_packages_answer_the_reports_shape() {
        fn define(ext: &mut Extension) {
            ext.package("_", Package::new().uses(["stdcore"]).feature("wordcount", Feature::new().title("Word count").register(|_, _| Ok(()))));
            ext.package(
                "full",
                Package::new()
                    .uses(["stdcore", "_"])
                    .flm(json!({ "features": {} }))
                    .templates(["templates/html/fancy.yaml"])
                    .host(["files"])
                    .feature("a", Feature::new().title("A").docs("The a.").version("1.0").settings(json!({ "config": {} })).register(|_, _| Ok(())))
                    .feature("b", Feature::new().register(|_, _| Ok(()))),
            );
        }
        let entry = |name: &str, args: Value| -> Value {
            let request = json!({ "entry": name, "args": args }).to_string();
            serde_json::from_slice(&answer(Some(request.as_bytes()), define)).expect("an envelope")
        };
        assert_eq!(entry("define", json!(["wc", "wc.wasm"])), json!({ "ok": null }));
        assert_eq!(
            entry("packages", json!([])),
            json!({ "ok": [
                { "name": "_", "uses": ["stdcore"], "flm": null, "templates": null, "host": null,
                  "features": [{ "name": "wordcount", "object": { "title": "Word count" } }] },
                { "name": "full", "uses": ["stdcore", "_"], "flm": { "features": {} }, "templates": ["templates/html/fancy.yaml"], "host": ["files"],
                  "features": [
                      { "name": "a", "object": { "title": "A", "docs": "The a.", "version": "1.0", "settings": { "config": {} } } },
                      { "name": "b", "object": {} },
                  ] },
            ] })
        );
        fn without_register(ext: &mut Extension) {
            ext.package("_", Package::new().feature("lonely", Feature::new().title("Lonely")));
        }
        let request = json!({ "entry": "define", "args": ["wc", "wc.wasm"] }).to_string();
        let reply: Value = serde_json::from_slice(&answer(Some(request.as_bytes()), without_register)).expect("an envelope");
        assert_eq!(reply["err"]["kind"], "argument");
        assert_eq!(reply["err"]["method"], "package");
        assert_eq!(reply["message"].as_str().map(|text| text.ends_with("the feature `lonely` of the package `_` of the extension `wc` must have a `register` function")), Some(true), "{reply}");
    }

    /// Off `wasm32`, and with the feature `native` on every target, the macro
    /// writes no export (the four exports are checked by building a guest
    /// crate for `wasm32` without `native`), and on every target the static
    /// through which a program reaches the crate, with the crate's name and
    /// version (here the crate compiling the macro's expansion, `flm-ext`).
    #[test]
    fn the_macro_writes_the_static_of_the_extension() {
        mod written {
            fn define(ext: &mut super::Extension) {
                ext.package("_", super::Package::new());
            }
            crate::guest::extension!(define);
        }
        assert_eq!(written::FLM_EXTENSION.name(), "flm-ext");
        assert_eq!(written::FLM_EXTENSION.version(), env!("CARGO_PKG_VERSION"));
    }
}
