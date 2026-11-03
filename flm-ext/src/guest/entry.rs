//! The guest's entries: the builders an extension declares its packages
//! with ([`Extension`], [`Package`], [`Feature`]) and the 19 entries the
//! engine calls, the twin of the JavaScript prelude's `flm:_entry`.
//!
//! **Two readers, one routing.** An entry's request is an [`EntryRequest`],
//! its arguments typed: names and the settings borrowed from the reader's
//! input, the handles of the objects the call lent as the structs of their
//! kinds, a citation provider's requests and a template's data as the
//! guest's values. [`route`] runs it and answers an [`EntryReply`], the
//! guest's values. Two readers build the request, over one statement of
//! every entry's arguments, their positions and forms ([`Reader`]), each
//! reading every argument before the routing finds the handler object the
//! request names:
//!
//! - [`answer`], the text ABI of a module (the export `flm_entry` the engine
//!   `wasm` calls): the request `{"entry": NAME, "args": [ … ]}` read from
//!   its JSON text, and the reply written in the envelope `{"ok": value}` or
//!   `{"err": data, "message": text}`, `data` being the error's
//!   [`ExtError::data`];
//! - `call`, compiled into a program (the cargo feature `native`): the
//!   arguments the engine `native` passes read as they are, and the reply
//!   answered as it is, with no JSON value built or read but the settings
//!   and the reports of `packages`, which are data by nature.
//!
//! `define(stem)` runs the function the entry macro names with the extension
//! builder; `packages()` answers the package objects it declared; `register`
//! and `rehearse` run a feature's `register` (the rehearsal with a builder
//! that sends nothing, then each finalizer that `register` recorded,
//! likewise); `read_settings` runs a feature's configuration reader
//! ([`Feature::read`]), its answer written `{"value": …}` or `{"error":
//! {"path", "detail"}}` in a module's reply; every other entry finds the
//! handler object the feature's `register` recorded
//! ([`crate::guest::registry`]) and calls it, or refuses with the text the
//! prelude's entries use when it is absent.
//!
//! **Re-entry.** The host may enter the instance again while a request of
//! the guest runs (a hook run while a stage's request runs): an entry clones
//! the handler object out of the registry, releasing every borrow, before it
//! calls the object.
//!
//! What `define` declared is part of the instance's state
//! ([`crate::guest::state`]).

use std::borrow::Cow;
use std::sync::Arc;

use serde_json::{json, Map, Value};

#[cfg(feature = "native")]
use crate::adapter::handles::Handle;
use crate::adapter::values::{not_a, JsonForm};
#[cfg(feature = "native")]
use crate::binding::EntryArg;
use crate::guest::core::data::store::StageOut;
use crate::guest::core::lowering::{LowerCx, Lowerer, Lowering};
use crate::guest::core::registry::FeatureBuilder;
use crate::guest::core::run::{RewriteCx, StageCx};
use crate::guest::core::settings::ReadError;
use crate::guest::lang::rewrites::userdefs::{ExpandCx, Expander, Expansion};
use crate::guest::lang::{BuiltLang, FlmDiagnostics};
use crate::guest::model::aspects::{ContentRef, RefPoint, Sectioning};
use crate::guest::registry::{self, Record, Shared};
use crate::guest::state;
use crate::guest::stdcore::features::citations::{CitationRequest, CitationResult, ProviderCx};
use crate::guest::stdcore::features::compose::{TemplateContext, TemplateCx, TemplateDescription, TemplateFile, TemplateModule};
use crate::guest::techy::node::Node;
use crate::guest::techy::transform::{Restage, Restager};
use crate::runtime::ExtError;

/// A feature's `register`: the builder of the registration and the
/// feature's settings.
type Register = Arc<dyn Fn(&mut FeatureBuilder, &Value) -> Result<(), ExtError> + Send + Sync>;

/// A feature's configuration reader: the configuration written, and the
/// configuration `register` receives or the value at fault.
type Read = Arc<dyn Fn(Value) -> Result<Value, ReadError> + Send + Sync>;

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
    /// Its configuration reader.
    read: Option<Read>,
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

    /// Its configuration reader, the twin of the conversion of
    /// `flm-core`'s
    /// [`SettingsDecl::config_from`](flm_core::core::settings::SettingsDecl::config_from)
    /// (the configuration is declared with
    /// [`SettingsDecl::config_data_read`](flm_core::core::settings::SettingsDecl::config_data_read)):
    /// a function or a closure that reads the feature's configuration
    /// whenever the host reads a document's settings, for every feature of a
    /// package the document uses that declares a reader and is not switched
    /// off in the document's settings, before the registry is built, as
    /// `config_from`'s conversion runs. Given the configuration written under
    /// the feature (`{}` when nothing is written under the feature, or none of
    /// the configuration's own keys), it answers the
    /// configuration `register` receives, or the value at fault as a
    /// [`ReadError`], which the host reports where that value was written, and
    /// with no place in the document when its settings do not name the
    /// feature. A reader that only checks the configuration answers it
    /// unchanged. The feature's settings declare the
    /// configuration it reads (their `config`); the feature's report states
    /// that it reads it (`read` in its settings). A feature without a reader
    /// has its configuration's keys checked only, and `register` receives
    /// the configuration as written.
    pub fn read(mut self, read: impl Fn(Value) -> Result<Value, ReadError> + Send + Sync + 'static) -> Feature {
        self.read = Some(Arc::new(read));
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
        let mut settings = self.settings.clone();
        if self.read.is_some() {
            // Settings that are not a map are left as given: the report's reading
            // refuses them.
            if let Value::Object(settings) = settings.get_or_insert_with(|| Value::Object(Map::new())) {
                settings.insert("read".to_string(), Value::Bool(true));
            }
        }
        if let Some(settings) = settings {
            object.insert("settings".to_string(), settings);
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
const ENTRIES: [&str; 19] = [
    "define",
    "packages",
    "register",
    "rehearse",
    "read_settings",
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

/// An entry's request, its arguments typed and borrowed from the reader's
/// input for `'a`: what both readers build ([`answer`]'s from the JSON text
/// of a module's ABI, `call`'s from the arguments the engine `native`
/// passes) and [`route`] runs.
// A request is made for one entry and moved once into the routing, so the size of its
// largest variant costs nothing; boxing it would only complicate the readers.
#[allow(clippy::large_enum_variant)]
pub(crate) enum EntryRequest<'a> {
    /// `define(stem)`.
    Define {
        /// The extension's stem.
        stem: &'a str,
        /// The function the entry macro names, run with the extension
        /// builder.
        define: fn(&mut Extension),
    },
    /// `packages()`.
    Packages,
    /// An entry of the feature `feature` of the package `pkg`.
    Feature {
        /// The package's name.
        pkg: &'a str,
        /// The feature's name.
        feature: &'a str,
        /// The entry, with its arguments after the package and the feature.
        entry: FeatureEntry<'a>,
    },
}

/// An entry of one feature, with its arguments after the names of the
/// package and the feature: the feature's `register`, its rehearsal and its
/// configuration reader, then the entries of the handler objects it
/// recorded, each named by a stage's, a rule's, a provider's or a module's
/// name, an element, or a position.
pub(crate) enum FeatureEntry<'a> {
    /// `register(settings, b)`.
    Register {
        /// The feature's settings.
        settings: &'a Value,
        /// The builder of the registration.
        builder: FeatureBuilder,
    },
    /// `rehearse(settings)`.
    Rehearse {
        /// The settings of the real registration.
        settings: &'a Value,
    },
    /// `read_settings(settings)`.
    ReadSettings {
        /// The configuration written.
        settings: &'a Value,
    },
    /// `run_stage(name, cx, out)`.
    RunStage {
        /// The stage's name.
        name: &'a str,
        /// The stage's context.
        cx: StageCx,
        /// The stage's output.
        out: StageOut,
    },
    /// `expand(element, node, cx, xp)`.
    Expand {
        /// The element whose hook runs.
        element: &'a str,
        /// The use site.
        node: Node,
        /// The context at the use site.
        cx: ExpandCx,
        /// The expander.
        xp: Expander,
    },
    /// `lower(rule, node, cx, lw)`.
    Lower {
        /// The rule's name.
        rule: &'a str,
        /// The node being lowered.
        node: Node,
        /// The presentation tree under construction.
        cx: LowerCx,
        /// The lowerer.
        lw: Lowerer,
    },
    /// `restage(name, node, cx, rs)`.
    Restage {
        /// The rewrite stage's name.
        name: &'a str,
        /// The node of the input tree.
        node: Node,
        /// The rewrite's context.
        cx: RewriteCx,
        /// The restage façade.
        rs: Restager,
    },
    /// `answer_all(name, requests, cx)`.
    AnswerAll {
        /// The citation provider's name.
        name: &'a str,
        /// The requests.
        requests: Vec<CitationRequest>,
        /// The provider's context.
        cx: ProviderCx,
    },
    /// `template_describe(name, settings)`.
    TemplateDescribe {
        /// The template module's name.
        name: &'a str,
        /// The feature's settings.
        settings: &'a Value,
    },
    /// `template_files(name, data, cx)`.
    TemplateFiles {
        /// The template module's name.
        name: &'a str,
        /// The template's data: read from JSON, or borrowed from the host.
        data: Cow<'a, TemplateContext>,
        /// The module's context.
        cx: TemplateCx,
    },
    /// `template_render(name, data, cx)`.
    TemplateRender {
        /// The template module's name.
        name: &'a str,
        /// The template's data: read from JSON, or borrowed from the host.
        data: Cow<'a, TemplateContext>,
        /// The module's context.
        cx: TemplateCx,
    },
    /// `points(element, node)`.
    Points {
        /// The element whose aspect `Referenceable` answers.
        element: &'a str,
        /// The occurrence.
        node: Node,
    },
    /// `level(element, node)`.
    Level {
        /// The element whose aspect `Sectioning` answers.
        element: &'a str,
        /// The occurrence.
        node: Node,
    },
    /// `in_toc(element, node)`.
    InToc {
        /// The element whose aspect `Sectioning` answers.
        element: &'a str,
        /// The occurrence.
        node: Node,
    },
    /// `toc_title(element, node)`.
    TocTitle {
        /// The element whose aspect `Sectioning` answers.
        element: &'a str,
        /// The occurrence.
        node: Node,
    },
    /// `late_init(index, lang, diags)`.
    LateInit {
        /// The hook's position among the feature's late-initialization
        /// hooks.
        index: usize,
        /// The finished language.
        lang: BuiltLang,
        /// The registry's build diagnostics.
        diags: FlmDiagnostics,
    },
    /// `finalize(index, b)`.
    Finalize {
        /// The finalizer's position among the feature's finalizers.
        index: usize,
        /// The builder of the build's second round.
        builder: FeatureBuilder,
    },
}

/// An entry's reply, the guest's values: what [`route`] answers, which
/// [`answer`] writes as the JSON form a module's envelope holds and `call`
/// answers as it is.
#[derive(Debug)]
pub(crate) enum EntryReply {
    /// No value: `define`, `register`, `rehearse`, `run_stage`, `late_init`
    /// and `finalize`.
    Nothing,
    /// The configuration kept, or the value at fault: `read_settings`.
    Read(Result<Value, ReadError>),
    /// The reports of the packages `define` declared, one per
    /// [`Extension::package`] call: `packages`.
    Packages(Vec<Value>),
    /// What an expansion hook answers: `expand`.
    Expansion(Expansion),
    /// What a lowering rule's body answers: `lower`.
    Lowering(Lowering),
    /// What a rewrite stage answers for a node: `restage`.
    Restage(Restage),
    /// A citation provider's results, one per request: `answer_all`.
    Citations(Vec<CitationResult>),
    /// A template module's description: `template_describe`.
    Description(TemplateDescription),
    /// The files a template module ships: `template_files`.
    Files(Vec<TemplateFile>),
    /// The page a template module renders: `template_render`.
    Page(String),
    /// An occurrence's reference points: `points`.
    Points(Vec<RefPoint>),
    /// A section's level: `level`.
    Level(u32),
    /// Whether a section is listed in the table of contents: `in_toc`.
    Listed(bool),
    /// A section's title in the table of contents, if it has one of its
    /// own: `toc_title`.
    Title(Option<ContentRef>),
}

impl EntryReply {
    /// The reply's JSON form: each value's own, `null` for no value.
    fn into_value(self) -> Value {
        match self {
            EntryReply::Nothing => Value::Null,
            // The form `read_answer_from_value` reads.
            EntryReply::Read(Ok(value)) => json!({ "value": value }),
            EntryReply::Read(Err(error)) => json!({ "error": error }),
            EntryReply::Packages(reports) => Value::Array(reports),
            EntryReply::Expansion(expansion) => expansion.to_value(),
            EntryReply::Lowering(lowering) => lowering.to_value(),
            EntryReply::Restage(restage) => restage.to_value(),
            EntryReply::Citations(results) => results.to_value(),
            EntryReply::Description(description) => description.to_value(),
            EntryReply::Files(files) => files.to_value(),
            EntryReply::Page(page) => Value::String(page),
            EntryReply::Points(points) => points.to_value(),
            EntryReply::Level(level) => level.to_value(),
            EntryReply::Listed(listed) => Value::Bool(listed),
            EntryReply::Title(title) => title.to_value(),
        }
    }
}

/// The reply to the entry request `request` (the bytes the host wrote, or
/// `None` when no buffer of the guest holds them), `define` the function
/// the entry macro names: the envelope's JSON text. The JSON reader: the
/// request read into its [`EntryRequest`] by [`Args`], routed, and the reply
/// written as its JSON form.
pub(crate) fn answer(request: Option<&[u8]>, define: fn(&mut Extension)) -> Vec<u8> {
    let reply = match request.and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok()) {
        Some(Value::Object(mut request)) => {
            let name = request.remove("entry");
            match (name.as_ref().and_then(Value::as_str), request.remove("args")) {
                (Some(name), Some(Value::Array(args))) => {
                    Args { entry: name, items: &args }.request(define).and_then(route).map(EntryReply::into_value)
                }
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

/// Runs the entry `name` with the arguments `args` the engine `native`
/// passes, in the same process ([`crate::native`]): the routing of
/// [`answer`], the arguments read as they are into the entry's
/// [`EntryRequest`] by `Passed` and the reply answered as it is, with no
/// JSON value built or read but the settings and the reports of `packages`.
/// `define` is the function the entry macro gives.
///
/// # Errors
///
/// The entry's refusal (an unknown entry, a handler object `register` did
/// not declare) or what the handler answers.
#[cfg(feature = "native")]
pub(crate) fn call(name: &str, args: &[EntryArg<'_>], define: fn(&mut Extension)) -> Result<EntryReply, ExtError> {
    route(Passed { entry: name, items: args }.request(define)?)
}

/// The refusal of a request that is not an entry's.
fn not_a_request() -> ExtError {
    ExtError::Handler {
        detail: "the request of an entry must be the JSON text `{\"entry\": name, \"args\": [...]}` in a buffer of the guest".to_string(),
        data: Value::Null,
    }
}

/// The reading of an entry's request, shared by its two readers ([`Args`],
/// and `Passed` compiled into a program): every entry, and the position and
/// form of each of its arguments, written once over each reader's own
/// reading of one argument.
trait Reader<'a> {
    /// The entry's name.
    fn entry(&self) -> &'a str;

    /// The argument at `index`, a name.
    fn text(&self, index: usize) -> Result<&'a str, ExtError>;

    /// The argument at `index`, the settings.
    fn settings(&self, index: usize) -> &'a Value;

    /// The argument at `index`, a position.
    fn index(&self, index: usize) -> Result<usize, ExtError>;

    /// The argument at `index`, the requests of a citation provider's call.
    fn requests(&self, index: usize) -> Result<Vec<CitationRequest>, ExtError>;

    /// The argument at `index`, the data of a template module's call.
    fn data(&self, index: usize) -> Result<Cow<'a, TemplateContext>, ExtError>;

    /// The argument at `index`, an object the call lent, as the struct of
    /// its kind.
    fn object<T: Lent>(&self, index: usize) -> Result<T, ExtError>;

    /// The entry's request: the package and the feature first for every
    /// entry but `define` and `packages`.
    fn request(&self, define: fn(&mut Extension)) -> Result<EntryRequest<'a>, ExtError> {
        Ok(match self.entry() {
            "define" => EntryRequest::Define { stem: self.text(0)?, define },
            "packages" => EntryRequest::Packages,
            entry if ENTRIES.contains(&entry) => {
                EntryRequest::Feature { pkg: self.text(0)?, feature: self.text(1)?, entry: self.feature_entry()? }
            }
            entry => return Err(unknown(entry)),
        })
    }

    /// The feature's entry, its arguments after the package and the feature.
    fn feature_entry(&self) -> Result<FeatureEntry<'a>, ExtError> {
        Ok(match self.entry() {
            "register" => FeatureEntry::Register { settings: self.settings(2), builder: self.object(3)? },
            "rehearse" => FeatureEntry::Rehearse { settings: self.settings(2) },
            "read_settings" => FeatureEntry::ReadSettings { settings: self.settings(2) },
            "run_stage" => FeatureEntry::RunStage { name: self.text(2)?, cx: self.object(3)?, out: self.object(4)? },
            "expand" => {
                FeatureEntry::Expand { element: self.text(2)?, node: self.object(3)?, cx: self.object(4)?, xp: self.object(5)? }
            }
            "lower" => FeatureEntry::Lower { rule: self.text(2)?, node: self.object(3)?, cx: self.object(4)?, lw: self.object(5)? },
            "restage" => {
                FeatureEntry::Restage { name: self.text(2)?, node: self.object(3)?, cx: self.object(4)?, rs: self.object(5)? }
            }
            "answer_all" => FeatureEntry::AnswerAll { name: self.text(2)?, requests: self.requests(3)?, cx: self.object(4)? },
            "template_describe" => FeatureEntry::TemplateDescribe { name: self.text(2)?, settings: self.settings(3) },
            "template_files" => FeatureEntry::TemplateFiles { name: self.text(2)?, data: self.data(3)?, cx: self.object(4)? },
            "template_render" => FeatureEntry::TemplateRender { name: self.text(2)?, data: self.data(3)?, cx: self.object(4)? },
            "points" => FeatureEntry::Points { element: self.text(2)?, node: self.object(3)? },
            "level" => FeatureEntry::Level { element: self.text(2)?, node: self.object(3)? },
            "in_toc" => FeatureEntry::InToc { element: self.text(2)?, node: self.object(3)? },
            "toc_title" => FeatureEntry::TocTitle { element: self.text(2)?, node: self.object(3)? },
            "late_init" => FeatureEntry::LateInit { index: self.index(2)?, lang: self.object(3)?, diags: self.object(4)? },
            "finalize" => FeatureEntry::Finalize { index: self.index(2)?, builder: self.object(3)? },
            other => return Err(unknown(other)),
        })
    }
}

/// A struct of the guest over the handle of an object the host lends an
/// entry ([`Reader::object`]): read through its JSON form by [`Args`], and
/// built over the handle the engine `native` passes by `Passed`.
trait Lent: JsonForm {
    /// The struct over `handle`.
    #[cfg(feature = "native")]
    fn over(handle: Handle) -> Self;
}

/// [`Lent`] for each struct named, a struct of one field, `handle`.
macro_rules! lent_structs {
    ($($guest:ident),* $(,)?) => {
        $(
            impl Lent for $guest {
                #[cfg(feature = "native")]
                fn over(handle: Handle) -> Self {
                    $guest { handle }
                }
            }
        )*
    };
}

lent_structs!(
    StageCx, StageOut, Node, ExpandCx, Expander, LowerCx, Lowerer, RewriteCx, Restager, ProviderCx, TemplateCx, BuiltLang, FlmDiagnostics
);

impl Lent for FeatureBuilder {
    #[cfg(feature = "native")]
    fn over(handle: Handle) -> Self {
        FeatureBuilder { handle: Some(handle) }
    }
}

/// The JSON arguments of the entry `entry`, as a module's ABI writes them:
/// the JSON reader of [`answer`].
struct Args<'a> {
    /// The entry's name.
    entry: &'a str,
    /// Its arguments.
    items: &'a [Value],
}

impl<'a> Args<'a> {
    /// The argument at `index` (`null` when absent).
    fn get(&self, index: usize) -> &'a Value {
        self.items.get(index).unwrap_or(&Value::Null)
    }

    /// `error`, which the argument at `index` met, as the entry's refusal of
    /// that argument.
    fn refused(&self, index: usize, error: ExtError) -> ExtError {
        match error {
            ExtError::Value { detail } => {
                ExtError::Argument { method: self.entry.to_string(), detail: format!("argument {}: {detail}", index + 1) }
            }
            other => other,
        }
    }

    /// The argument at `index`, read through its JSON form.
    fn form<T: JsonForm>(&self, index: usize) -> Result<T, ExtError> {
        T::from_value(self.get(index)).map_err(|error| self.refused(index, error))
    }
}

impl<'a> Reader<'a> for Args<'a> {
    fn entry(&self) -> &'a str {
        self.entry
    }

    fn text(&self, index: usize) -> Result<&'a str, ExtError> {
        let value = self.get(index);
        value.as_str().ok_or_else(|| self.refused(index, not_a("a text", value)))
    }

    fn settings(&self, index: usize) -> &'a Value {
        self.get(index)
    }

    fn index(&self, index: usize) -> Result<usize, ExtError> {
        self.form(index)
    }

    fn requests(&self, index: usize) -> Result<Vec<CitationRequest>, ExtError> {
        self.form(index)
    }

    fn data(&self, index: usize) -> Result<Cow<'a, TemplateContext>, ExtError> {
        self.form(index).map(Cow::Owned)
    }

    fn object<T: Lent>(&self, index: usize) -> Result<T, ExtError> {
        self.form(index)
    }
}

/// The arguments of the entry `entry` as the engine `native` passes them:
/// the reader of [`call`]. Each is read as it is, a handle into the struct
/// of the entry's object. The host builds every entry's arguments in the
/// entry's forms (its handler rows, `binding::handlers`, and its load), so
/// each argument is of the form the entry reads.
#[cfg(feature = "native")]
struct Passed<'a> {
    /// The entry's name.
    entry: &'a str,
    /// Its arguments.
    items: &'a [EntryArg<'a>],
}

#[cfg(feature = "native")]
impl<'a> Passed<'a> {
    /// The argument at `index`, of another form than the variant `variant`
    /// of [`EntryArg`]. The host passes each argument in the form the entry
    /// reads, so no entry receives one.
    fn other_form(&self, index: usize, variant: &str) -> ! {
        unreachable!("the engine `native` passes argument {} of the entry `{}` as `EntryArg::{variant}`", index + 1, self.entry)
    }

    /// The argument at `index`, the handle of an object the call lent.
    fn handle(&self, index: usize) -> Handle {
        match self.items.get(index) {
            Some(EntryArg::Handle(handle)) => *handle,
            _ => self.other_form(index, "Handle"),
        }
    }
}

#[cfg(feature = "native")]
impl<'a> Reader<'a> for Passed<'a> {
    fn entry(&self) -> &'a str {
        self.entry
    }

    fn text(&self, index: usize) -> Result<&'a str, ExtError> {
        match self.items.get(index) {
            Some(EntryArg::Text(text)) => Ok(text),
            _ => self.other_form(index, "Text"),
        }
    }

    fn settings(&self, index: usize) -> &'a Value {
        match self.items.get(index) {
            Some(EntryArg::Json(settings)) => settings,
            _ => self.other_form(index, "Json"),
        }
    }

    fn index(&self, index: usize) -> Result<usize, ExtError> {
        match self.items.get(index) {
            Some(EntryArg::Index(position)) => Ok(*position),
            _ => self.other_form(index, "Index"),
        }
    }

    fn requests(&self, index: usize) -> Result<Vec<CitationRequest>, ExtError> {
        match self.items.get(index) {
            Some(EntryArg::CitationRequests(requests)) => {
                Ok(requests.iter().map(|request| CitationRequest { key: request.key.clone() }).collect())
            }
            _ => self.other_form(index, "CitationRequests"),
        }
    }

    fn data(&self, index: usize) -> Result<Cow<'a, TemplateContext>, ExtError> {
        match self.items.get(index) {
            Some(EntryArg::TemplateContext(data)) => Ok(Cow::Borrowed(*data)),
            _ => self.other_form(index, "TemplateContext"),
        }
    }

    fn object<T: Lent>(&self, index: usize) -> Result<T, ExtError> {
        Ok(T::over(self.handle(index)))
    }
}

/// Runs the entry `request`: the one routing of both readers.
fn route(request: EntryRequest<'_>) -> Result<EntryReply, ExtError> {
    match request {
        EntryRequest::Define { stem, define } => define_with(stem, define).map(|()| EntryReply::Nothing),
        EntryRequest::Packages => Ok(EntryReply::Packages(state::current().defined.borrow().reports.clone())),
        EntryRequest::Feature { pkg, feature, entry } => feature_entry(pkg, feature, entry),
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

/// What `pick` takes of the feature `feature` of the package `pkg`, as
/// `define` declared it.
fn declared<T>(pkg: &str, feature: &str, pick: impl FnOnce(&Feature) -> Option<T>) -> Option<T> {
    let state = state::current();
    let defined = state.defined.borrow();
    let package = defined.packages.iter().find(|(name, _)| name == pkg).map(|(_, package)| package);
    let feature = package.and_then(|package| package.features.iter().find(|(name, _)| name == feature));
    feature.and_then(|(_, feature)| pick(feature))
}

/// The `register` function of the feature `feature` of the package `pkg`.
fn find_feature(pkg: &str, feature: &str) -> Result<Register, ExtError> {
    let found = declared(pkg, feature, |feature| feature.register.clone());
    found.ok_or_else(|| refusal(format!("the package `{pkg}` of the extension `{}` must have a feature `{feature}`", stem())))
}

/// The configuration reader of the feature `feature` of the package `pkg`.
fn find_reader(pkg: &str, feature: &str) -> Result<Read, ExtError> {
    let found = declared(pkg, feature, |feature| feature.read.clone());
    found.ok_or_else(|| {
        refusal(format!(
            "the feature `{feature}` of the package `{pkg}` of the extension `{}` must have a configuration reader",
            stem()
        ))
    })
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

/// The template module `name` the feature contributed.
fn template(pkg: &str, feature: &str, name: &str) -> Result<Arc<dyn TemplateModule>, ExtError> {
    let what = format!("the template module `{name}`");
    contributed(pkg, feature, &what, |done| done.templates.get(name).cloned())
}

/// The element's handler aspect `pick` finds, or the refusal whose text
/// begins with `what`.
fn aspect<T>(pkg: &str, feature: &str, element: &str, what: &str, pick: impl FnOnce(&registry::Aspects) -> Option<T>) -> Result<T, ExtError> {
    find(pkg, feature, |done| done.aspects.get(element).and_then(pick)).ok_or_else(|| {
        refusal(format!("{what} of the element `{element}` must be answered by an aspect attached by {}", owner(pkg, feature)))
    })
}

/// The aspect `Sectioning` attached to the element `element`.
fn sectioning(pkg: &str, feature: &str, element: &str) -> Result<Arc<dyn Sectioning>, ExtError> {
    aspect(pkg, feature, element, "the sectioning", |aspects| aspects.sectioning.clone())
}

/// Runs the entry `entry` of the feature `feature` of the package `pkg`:
/// its `register`, its rehearsal, its configuration reader, or the handler
/// object the entry names, found among what the feature recorded and called
/// with the entry's arguments.
fn feature_entry(pkg: &str, feature: &str, entry: FeatureEntry<'_>) -> Result<EntryReply, ExtError> {
    match entry {
        FeatureEntry::Register { settings, builder } => {
            register_with(pkg, feature, settings, builder, false).map(|()| EntryReply::Nothing)
        }
        FeatureEntry::Rehearse { settings } => rehearse(pkg, feature, settings).map(|()| EntryReply::Nothing),
        FeatureEntry::ReadSettings { settings } => find_reader(pkg, feature).map(|read| EntryReply::Read(read(settings.clone()))),
        FeatureEntry::RunStage { name, cx, mut out } => {
            let stage = find(pkg, feature, |done| done.stages.get(name).cloned()).ok_or_else(|| {
                refusal(format!("the stage `{name}` must be declared by {}", owner(pkg, feature)))
            })?;
            stage.run(&cx, &mut out).map(|()| EntryReply::Nothing)
        }
        FeatureEntry::Expand { element, node, mut cx, mut xp } => {
            let hook = find(pkg, feature, |done| done.hooks.get(element).cloned()).ok_or_else(|| {
                refusal(format!("the expansion hook of the element `{element}` must be attached by {}", owner(pkg, feature)))
            })?;
            hook.expand(&node, &mut cx, &mut xp).map(EntryReply::Expansion)
        }
        FeatureEntry::Lower { rule, node, mut cx, mut lw } => {
            let body = find(pkg, feature, |done| done.lowers.get(rule).cloned()).ok_or_else(|| {
                refusal(format!("the lowering rule `{rule}` must be built by {}", owner(pkg, feature)))
            })?;
            body.lower(&node, &mut cx, &mut lw).map(EntryReply::Lowering)
        }
        FeatureEntry::Restage { name, node, cx, mut rs } => {
            let stage = find(pkg, feature, |done| done.rewrites.get(name).cloned()).ok_or_else(|| {
                refusal(format!("the rewrite stage `{name}` must be declared by {}", owner(pkg, feature)))
            })?;
            stage.restage(&node, &cx, &mut rs).map(EntryReply::Restage)
        }
        FeatureEntry::AnswerAll { name, requests, cx } => {
            let what = format!("the citation provider `{name}`");
            let provider = contributed(pkg, feature, &what, |done| done.providers.get(name).cloned())?;
            provider.answer_all(&requests, &cx).map(EntryReply::Citations)
        }
        FeatureEntry::TemplateDescribe { name, settings } => {
            template(pkg, feature, name)?.describe(settings).map(EntryReply::Description)
        }
        FeatureEntry::TemplateFiles { name, data, cx } => template(pkg, feature, name)?.files(&data, &cx).map(EntryReply::Files),
        FeatureEntry::TemplateRender { name, data, cx } => template(pkg, feature, name)?.render(&data, &cx).map(EntryReply::Page),
        FeatureEntry::Points { element, node } => {
            let object = aspect(pkg, feature, element, "the reference points", |aspects| aspects.referenceable.clone())?;
            object.points(&node).map(EntryReply::Points)
        }
        FeatureEntry::Level { element, node } => sectioning(pkg, feature, element)?.level(&node).map(EntryReply::Level),
        FeatureEntry::InToc { element, node } => sectioning(pkg, feature, element)?.in_toc(&node).map(EntryReply::Listed),
        FeatureEntry::TocTitle { element, node } => sectioning(pkg, feature, element)?.toc_title(&node).map(EntryReply::Title),
        FeatureEntry::LateInit { index, lang, mut diags } => {
            late_init(pkg, feature, index, &lang, &mut diags).map(|()| EntryReply::Nothing)
        }
        FeatureEntry::Finalize { index, builder } => finalize(pkg, feature, index, builder).map(|()| EntryReply::Nothing),
    }
}

/// `late_init(pkg, feature, index, lang, diags)`: the `late_init(lang,
/// diags)` of the feature's late-initialization hook at the position
/// `index`.
fn late_init(pkg: &str, feature: &str, index: usize, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), ExtError> {
    let hook = find(pkg, feature, |done| done.late_inits.get(index).cloned()).ok_or_else(|| {
        refusal(format!("the late-initialization hook {index} must be registered by {}", owner(pkg, feature)))
    })?;
    hook.late_init(lang, diags)
}

/// `finalize(pkg, feature, index, b)`: the `finalize(b)` of the feature's
/// finalizer at the position `index`, with the builder of the build's
/// second round; what it declares is recorded with what the feature's
/// `register` declared.
fn finalize(pkg: &str, feature: &str, index: usize, builder: FeatureBuilder) -> Result<(), ExtError> {
    let done = registry::recorded(pkg, feature)
        .filter(|done| index < done.borrow().finalizers.len())
        .ok_or_else(|| refusal(format!("the finalizer {index} must be registered by {}", owner(pkg, feature))))?;
    finalize_with(&done, index, builder, false)
}

#[cfg(test)]
pub(crate) mod tests {
    //! The runtime on the host, the ABI's test hook answering in place of the
    //! host: what a `register` records and lends, the rehearsal, the entries
    //! (the configuration reader's in `tests/native_handlers.rs`), `define`
    //! and `packages`, and re-entry. A test whose feature
    //! sends a request (a façade's method, a module's function, the lending
    //! of a handler object) runs without the cargo feature `native` alone,
    //! where the request crosses as JSON, which the hook answers; with
    //! `native`, a request is a typed call on the frame of a handler call,
    //! which the native suites (`tests/native*.rs`) run.

    #[cfg(not(feature = "native"))]
    use std::cell::Cell;
    use std::cell::RefCell;
    #[cfg(not(feature = "native"))]
    use std::rc::Rc;

    use serde_json::{json, Value};

    use super::*;
    use crate::api::core::stage::{Effect, Needs, Scope, StageDecl};
    use crate::guest::core::registry::Contributed;
    use crate::guest::core::stage::Stage;
    use crate::guest::model::aspects::{RefPoint, Referenceable};
    use crate::guest::model::element::{Aspect, ElementSpec};
    use crate::guest::stdcore::features::citations::{CitationProvider, CitationResult};
    // The items of the feature whose requests the test hook answers.
    #[cfg(not(feature = "native"))]
    use crate::adapter::handles::{Handle, HandleKind};
    #[cfg(not(feature = "native"))]
    use crate::adapter::values::handle_to_value;
    #[cfg(not(feature = "native"))]
    use crate::api::core::lowering::{DefaultMark, LoweringDecl};
    #[cfg(not(feature = "native"))]
    use crate::api::core::stage::RewriteDecl;
    #[cfg(not(feature = "native"))]
    use crate::api::stdcore::features::compose::{TemplateDescription, TemplateFile};
    #[cfg(not(feature = "native"))]
    use crate::guest::abi::hook;
    #[cfg(not(feature = "native"))]
    use crate::guest::core::lowering::{Lower, Lowering, LoweringRule};
    #[cfg(not(feature = "native"))]
    use crate::guest::core::registry::{BackendDef, BackendStage, Finalizer, LateInit, LibraryStage};
    #[cfg(not(feature = "native"))]
    use crate::guest::core::stage::RewriteStage;
    #[cfg(not(feature = "native"))]
    use crate::guest::lang::rewrites::userdefs::{Expand, Expansion, UserDefinition};
    #[cfg(not(feature = "native"))]
    use crate::guest::model::aspects::Sectioning;
    #[cfg(not(feature = "native"))]
    use crate::guest::stdcore::features::compose::TemplateModule;
    #[cfg(not(feature = "native"))]
    use crate::guest::techy::transform::Restage;

    /// A handle of the kind `kind` at `index`.
    #[cfg(not(feature = "native"))]
    pub(crate) fn handle(kind: HandleKind, index: u32) -> Value {
        handle_to_value(Handle { kind, call: 1, index })
    }

    /// The kind of the handle a lending request answers.
    #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
    type Log = Rc<RefCell<Vec<Value>>>;

    /// Installs a hook that logs every request and answers it as the host
    /// would: a lending with a handle of its kind, a call with `reply(method)`
    /// (`null` when it answers `None`).
    #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct Emit;
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct Late;
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct Rewrite;
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct Hook;
    #[cfg(not(feature = "native"))]
    impl Expand for Hook {
        fn expand(&self, _: &Node, _: &mut ExpandCx, _: &mut Expander) -> Result<Expansion, ExtError> {
            saw("expand");
            Ok(Expansion::Default)
        }
    }

    #[cfg(not(feature = "native"))]
    struct Body;
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct Heading;
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct Template;
    #[cfg(not(feature = "native"))]
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

    #[cfg(not(feature = "native"))]
    struct LateHook;
    #[cfg(not(feature = "native"))]
    impl LateInit for LateHook {
        fn late_init(&self, _: &BuiltLang, _: &mut FlmDiagnostics) -> Result<(), ExtError> {
            saw("late_init");
            Ok(())
        }
    }

    #[cfg(not(feature = "native"))]
    struct Final;
    #[cfg(not(feature = "native"))]
    impl Finalizer for Final {
        fn finalize(&self, b: &mut FeatureBuilder) -> Result<(), ExtError> {
            saw("finalize");
            b.stage(Arc::new(Late))
        }
    }

    /// The test feature's `register`: one object of each kind.
    #[cfg(not(feature = "native"))]
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
        b.backend(BackendDef::new("wc", Vec::new(), library, BackendStage::Own(Arc::new(Emit)), BackendStage::Own(Arc::new(Emit))))?;
        Ok(())
    }

    /// The test extension: one package, its one feature.
    #[cfg(not(feature = "native"))]
    fn define(ext: &mut Extension) {
        ext.package("_", Package::new().uses(["stdcore"]).feature("wordcount", Feature::new().title("Word count").register(register)));
    }

    /// Runs the entry `entry` with `args`; answers its envelope.
    #[cfg(not(feature = "native"))]
    pub(crate) fn run(entry: &str, args: Value) -> Value {
        let request = json!({ "entry": entry, "args": args }).to_string();
        serde_json::from_slice(&answer(Some(request.as_bytes()), define)).expect("an envelope")
    }

    /// The `ok` of the entry's envelope.
    #[cfg(not(feature = "native"))]
    fn ok(entry: &str, args: Value) -> Value {
        let reply = run(entry, args);
        assert!(reply.get("ok").is_some(), "the entry `{entry}` answered {reply}");
        reply["ok"].clone()
    }

    /// The message of the entry's `err`.
    #[cfg(not(feature = "native"))]
    fn refused(entry: &str, args: Value) -> String {
        let reply = run(entry, args);
        reply["message"].as_str().unwrap_or_else(|| panic!("the entry `{entry}` answered {reply}")).to_string()
    }

    /// The natives of the requests logged, in order.
    #[cfg(not(feature = "native"))]
    fn natives(log: &Log) -> Vec<String> {
        log.borrow().iter().map(|request| match request["native"].as_str() {
            Some("call") => format!("call {}", request["method"].as_str().unwrap_or_default()),
            other => other.unwrap_or_default().to_string(),
        }).collect()
    }

    /// Defines the extension and registers its feature for real.
    #[cfg(not(feature = "native"))]
    fn registered() -> Log {
        let log = install(host_reply);
        assert_eq!(ok("define", json!(["wc", "wc.wasm"])), Value::Null);
        assert_eq!(ok("register", json!(["_", "wordcount", {}, handle(HandleKind::FeatureBuilder, 0)])), Value::Null);
        log
    }

    #[test]
    #[cfg(not(feature = "native"))]
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
                "call backend",
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
    #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
    fn each_entry_refuses_with_the_preludes_text_when_its_object_is_absent() {
        registered();
        let node = handle(HandleKind::Node, 1);
        let at = |kind| handle(kind, 2);
        let data = json!({ "html": true, "regions": [], "pieces": [], "meta": {}, "config": {}, "artifacts": [] });
        let owner = "`register` of the feature `wordcount` of the package `_` of the extension `wc`";
        let cases = [
            ("run_stage", json!(["_", "wordcount", "wc.none", at(HandleKind::StageCx), at(HandleKind::StageOut)]), format!("the stage `wc.none` must be declared by {owner}")),
            ("expand", json!(["_", "wordcount", "wc.none", node, at(HandleKind::ExpandCx), at(HandleKind::Expander)]), format!("the expansion hook of the element `wc.none` must be attached by {owner}")),
            ("lower", json!(["_", "wordcount", "wc.none", node, at(HandleKind::LowerCx), at(HandleKind::Lowerer)]), format!("the lowering rule `wc.none` must be built by {owner}")),
            ("restage", json!(["_", "wordcount", "wc.none", node, at(HandleKind::RewriteCx), at(HandleKind::Restager)]), format!("the rewrite stage `wc.none` must be declared by {owner}")),
            ("answer_all", json!(["_", "wordcount", "wc.none", [], at(HandleKind::ProviderCx)]), format!("the citation provider `wc.none` must be contributed by {owner}")),
            ("template_describe", json!(["_", "wordcount", "wc.none", {}]), format!("the template module `wc.none` must be contributed by {owner}")),
            ("template_files", json!(["_", "wordcount", "wc.none", data, at(HandleKind::TemplateCx)]), format!("the template module `wc.none` must be contributed by {owner}")),
            ("template_render", json!(["_", "wordcount", "wc.none", data, at(HandleKind::TemplateCx)]), format!("the template module `wc.none` must be contributed by {owner}")),
            ("points", json!(["_", "wordcount", "wc.none", node]), format!("the reference points of the element `wc.none` must be answered by an aspect attached by {owner}")),
            ("level", json!(["_", "wordcount", "wc.ref", node]), format!("the sectioning of the element `wc.ref` must be answered by an aspect attached by {owner}")),
            ("in_toc", json!(["_", "wordcount", "wc.none", node]), format!("the sectioning of the element `wc.none` must be answered by an aspect attached by {owner}")),
            ("toc_title", json!(["_", "wordcount", "wc.none", node]), format!("the sectioning of the element `wc.none` must be answered by an aspect attached by {owner}")),
            ("late_init", json!(["_", "wordcount", 4, at(HandleKind::BuiltLang), at(HandleKind::FlmDiagnostics)]), format!("the late-initialization hook 4 must be registered by {owner}")),
            ("finalize", json!(["_", "wordcount", 4, handle(HandleKind::FeatureBuilder, 3)]), format!("the finalizer 4 must be registered by {owner}")),
            ("register", json!(["_", "nofeature", {}, handle(HandleKind::FeatureBuilder, 0)]), "the package `_` of the extension `wc` must have a feature `nofeature`".to_string()),
            ("rehearse", json!(["other", "wordcount", {}]), "the package `other` of the extension `wc` must have a feature `wordcount`".to_string()),
        ];
        for (entry, args, text) in cases {
            assert_eq!(refused(entry, args), text, "the entry `{entry}`");
            assert_eq!(run(entry, json!([]))["err"]["kind"], "argument", "the entry `{entry}` without its arguments");
        }
        assert_eq!(run("run_stage", json!(["_", "wordcount", "wc.none", at(HandleKind::StageCx), at(HandleKind::StageOut)]))["err"]["kind"], "handler");
        let unknown = refused("unknown", json!([]));
        assert!(unknown.starts_with("the entry `unknown` must be one of the guest's entries: `define`, `packages`, "), "{unknown}");
        let request = String::from_utf8(answer(Some(b"not json"), define)).expect("a text");
        assert!(request.contains("the request of an entry must be the JSON text"), "{request}");
        assert!(String::from_utf8(answer(None, define)).expect("a text").contains("the request of an entry must be"));
    }

    #[test]
    fn a_handler_object_given_outside_register_is_refused() {
        #[cfg(not(feature = "native"))]
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
        #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
    fn rehearse_replays_the_recorded_finalizers() {
        registered();
        SEEN.with(|seen| seen.borrow_mut().clear());
        ok("rehearse", json!(["_", "wordcount", {}]));
        assert_eq!(seen(), ["finalize"], "the one finalizer `register` recorded is replayed once");
    }

    #[test]
    #[cfg(not(feature = "native"))]
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
    #[cfg(not(feature = "native"))]
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
