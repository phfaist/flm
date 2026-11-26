//! Service slots and host services as the adapter serves them: the value type
//! of a slot a script declares ([`ExtService`]), the route of a contribution
//! by the slot's name (`contribute`), the readers of a slot's values as
//! data (`read_first`, `read_all`, `read_by_precedence`), and the host
//! services a package
//! declared ([`DeclaredServices`]) with the façades a handler reaches them
//! through.
//!
//! # The route of a contribution
//!
//! A contribution is made in the registry build's first round, before any
//! slot is assembled, so the adapter cannot ask the slot's type: the slot's
//! name decides what it takes.
//!
//! 1. The two slots that take a handler object (`HANDLER_SLOTS`).
//!    `flm.citations.providers` takes a
//!    [`CitationProvider`](crate::api::stdlib::features::citations::CitationProvider),
//!    wrapped in an [`ExtCitationProvider`]; `flm.compose.templates` takes a
//!    [`TemplateModule`],
//!    described with the feature's configuration, whose compose template
//!    goes to that slot and whose own [`ExtTemplateEngine`] goes to
//!    `flm.compose.engines`. Each is contributed as the feature said
//!    (`contribute_to` or `peer_slot`); any other value is refused.
//! 2. A slot of the table of standard data-valued slots
//!    (`STANDARD_DATA_SLOTS`): data, read into the slot's value type by the
//!    slot's reader in [`slots`], the value's own
//!    name checked under the extension's stem. The table holds the five
//!    slots of the numbering feature and of the recomposed LaTeX that take
//!    a value written as data: `flm.numbering.counters`,
//!    `flm.numbering.styles`, `flm.numbering.parent-sets`,
//!    `flm.latex.packages` and `flm.latex.preamble`.
//! 3. Any other slot: data, as an [`ExtService`]. A handler object there is
//!    refused. A data value contributed to a slot of another type fails the
//!    registry build with `flm-core`'s own check, which names the slot and
//!    the two types.

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder as CoreFeatureBuilder};
use flm_core::core::services::Services;
use flm_core::stdlib::features::citations::CitationProvider as CoreCitationProvider;
use flm_core::stdlib::features::compose::{Template, TemplateEngine, ENGINES_SLOT, TEMPLATES_SLOT};
use flm_core::model::options::TypeKey;
use serde_json::{json, Value};

use crate::adapter::citations::ExtCitationProvider;
use crate::adapter::compose::{digest_of, template_of, ExtTemplateEngine};
use crate::adapter::names::Owner;
use crate::adapter::slots;
use crate::adapter::values::{check_data, content_from_value};
use crate::api::core::registry::Contributed;
use crate::api::stdlib::features::compose::TemplateModule;
use crate::engine::ModuleReport;
use crate::api::host::{Files, HostHandle, HostService as HostServiceFacade};
use crate::host::{download, files, HostAnswer, HostCaller, HostService};
use crate::paths::{ExtensionsByName, PathError, PathErrorKind, Root, WrittenPath};
use flm_core::progress::Scope;
use crate::runtime::ExtError;
use crate::sealed::Sealed;
use crate::source::{ExtensionFiles, FilesError, FilesErrorKind};

/// The value of a service slot a script declares: data. Every slot declared
/// through [`FeatureBuilder::service_slot`](crate::api::core::registry::FeatureBuilder::service_slot)
/// holds values of this type, as every table a script declares holds an
/// [`ExtTable`](crate::adapter::table::ExtTable).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtService(pub Value);

/// The slots that take a handler object, each with what it takes, as the
/// refusal of a data value words it.
pub(crate) const HANDLER_SLOTS: &[(&str, &str)] = &[
    ("flm.citations.providers", "a citation provider (an object with `name` and `answer` or `answer_all`)"),
    ("flm.compose.templates", "a compose template module (an object with `name`, `describe` and `render`)"),
];

/// A standard slot whose values are data of a type of its own: how a data
/// value is contributed to it, and how its values are read as data.
pub(crate) struct StandardDataSlot {
    /// The slot's name.
    pub(crate) name: &'static str,
    /// Contributes the data value to the slot for the extension and package
    /// `owner`, converted to the slot's type.
    pub(crate) contribute: fn(&mut CoreFeatureBuilder<'_>, &Owner, &str, Value, How) -> Result<(), ExtError>,
    /// The slot's values, as data, in the order asked; `None` when the
    /// slot's type has no data form to read back.
    pub(crate) read: Option<SlotReader>,
}

/// A reader of a slot's values as data, in the order asked.
pub(crate) type SlotReader = fn(&Services, &str, Order) -> Result<Vec<Value>, ExtError>;

/// The order in which a slot's values are read (`flm-core`'s two orders of a
/// slot, [`flm_core::core::services`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Order {
    /// The assembled order ([`Services::services_named`]).
    Assembled,
    /// The order of precedence ([`Services::services_by_precedence_named`]).
    Precedence,
}

/// The table of standard data-valued slots, which the route consults before
/// its default and the readers before refusing a slot whose values are not
/// data. Each slot is contributed through its reader in
/// [`slots`]; none is read back as data (its type has
/// no data form). A data value for a standard slot of another type reaches
/// `flm-core`'s type check.
pub(crate) const STANDARD_DATA_SLOTS: &[StandardDataSlot] = &[
    StandardDataSlot { name: slots::COUNTERS, contribute: slots::contribute, read: None },
    StandardDataSlot { name: slots::STYLES, contribute: slots::contribute, read: None },
    StandardDataSlot { name: slots::PARENT_SETS, contribute: slots::contribute, read: None },
    StandardDataSlot { name: slots::PACKAGES, contribute: slots::contribute, read: None },
    StandardDataSlot { name: slots::PREAMBLE, contribute: slots::contribute, read: None },
];

/// How a value is contributed: which method of the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum How {
    /// `contribute_to`: after every value contributed so far.
    Append,
    /// `contribute_first_to`: before every value appended.
    First,
    /// `peer_slot`: to a slot another feature is expected to declare.
    Peer,
}

impl How {
    /// The method's name, for the messages.
    fn method(self) -> &'static str {
        match self {
            How::Append => "contribute_to",
            How::First => "contribute_first_to",
            How::Peer => "peer_slot",
        }
    }
}

/// A build error of the `flm-core` builder, as the handler sees it.
fn core_error(error: BuildError) -> ExtError {
    ExtError::Core { kind: "build".to_string(), detail: error.to_string(), data: Value::Null }
}

/// Who contributes, as the route needs it: the extension and the package,
/// the feature, its final configuration (which a template module's
/// `describe` receives), the template modules it declares, and the host
/// services the package declared (which a handler object's context lends).
pub(crate) struct Contributor<'a> {
    /// The extension and the package.
    pub(crate) owner: &'a Owner,
    /// The feature's name.
    pub(crate) feature: &'a str,
    /// The feature's final configuration.
    pub(crate) configuration: &'a Value,
    /// The template modules the feature declares, by name.
    pub(crate) modules: &'a BTreeMap<String, ModuleReport>,
    /// The host services the package declared.
    pub(crate) services: &'a Arc<DeclaredServices>,
}

/// What a contributed value is, in the words of the refusals.
fn what_of(value: &Contributed) -> &'static str {
    match value {
        Contributed::Data(_) => "data",
        Contributed::CitationProvider(_) => "a citation provider",
        Contributed::TemplateModule(_) => "a template module",
    }
}

/// Contributes `value` to the slot `slot` through `b`, for `who`, as the
/// [module documentation](self) routes it.
///
/// # Errors
///
/// A value that is not what the slot takes, a handler object contributed
/// first, or a handler object with no name ([`ExtError::Argument`]); data
/// that cannot cross or does not read as a standard data-valued slot's type
/// ([`ExtError::Value`]); a citation provider, or such a slot's value, named
/// outside the extension's stem ([`ExtError::NameOutsideExtension`]); a
/// template module that the feature does not declare
/// ([`ExtError::Argument`]); a template module's failing `describe`, or a
/// description whose `config` lacks a default for a declared setting or holds
/// a key that the declaration lacks ([`ExtError::Value`]); the builder's refusal
/// ([`ExtError::Core`]).
pub(crate) fn contribute(
    b: &mut CoreFeatureBuilder<'_>,
    who: &Contributor<'_>,
    slot: &str,
    value: Contributed,
    how: How,
) -> Result<(), ExtError> {
    let (package, stem) = (&who.owner.package, &who.owner.stem);
    let refused = |detail: String| ExtError::Argument { method: how.method().to_string(), detail };
    let is_data = matches!(value, Contributed::Data(_));
    if !is_data && how == How::First {
        return Err(refused(
            "the value must be data; a citation provider or a template module is contributed with `contribute_to` or \
             `peer_slot`"
                .to_string(),
        ));
    }
    if let Some((name, takes)) = HANDLER_SLOTS.iter().find(|(name, _)| *name == slot) {
        return match (value, *name) {
            (Contributed::CitationProvider(provider), PROVIDERS) => {
                let named = provider.name();
                if named.is_empty() {
                    return Err(refused(not_a_handler(PROVIDERS, who.owner)));
                }
                who.owner.check("citation provider", &named)?;
                let provider = ExtCitationProvider::new(provider, who.owner, who.feature, who.services);
                let provider: Arc<dyn CoreCitationProvider> = Arc::new(provider);
                match how {
                    How::Peer => b.peer_slot::<dyn CoreCitationProvider>(slot, provider),
                    How::Append | How::First => b.contribute_to::<dyn CoreCitationProvider>(slot, provider),
                }
                .map_err(core_error)
            }
            (Contributed::TemplateModule(module), TEMPLATES) => contribute_module(b, who, module, how),
            (value, _) => Err(refused(format!(
                "the service slot `{name}` takes {takes}, and the package `{package}` of the extension `{stem}` \
                 contributed {}",
                what_of(&value)
            ))),
        };
    }
    let Contributed::Data(data) = value else {
        return Err(refused(format!(
            "the service slot `{slot}` takes data, and the package `{package}` of the extension `{stem}` contributed \
             an object with methods; the slots that take an object are `flm.citations.providers` (a citation \
             provider) and `flm.compose.templates` (a template module)"
        )));
    };
    check_data(&data)?;
    if let Some(standard) = STANDARD_DATA_SLOTS.iter().find(|standard| standard.name == slot) {
        return (standard.contribute)(b, who.owner, slot, data, how);
    }
    let value = Arc::new(ExtService(data));
    match how {
        How::Append => b.contribute_to::<ExtService>(slot, value),
        How::First => b.contribute_first_to::<ExtService>(slot, value),
        How::Peer => b.peer_slot::<ExtService>(slot, value),
    }
    .map_err(core_error)
}

/// The slot of the citation providers.
const PROVIDERS: &str = "flm.citations.providers";

/// The slot of the compose templates.
const TEMPLATES: &str = "flm.compose.templates";

/// The refusal of a handler object without a name or without the methods
/// the slot `slot` calls, contributed by `owner`'s package (a binding checks
/// the methods before it lends the object).
pub(crate) fn not_a_handler(slot: &str, owner: &Owner) -> String {
    let (package, stem) = (&owner.package, &owner.stem);
    if slot == TEMPLATES {
        format!(
            "a template module contributed to `{slot}` by the package `{package}` of the extension `{stem}` must be an \
             object with `name`, `describe` and `render`"
        )
    } else {
        format!(
            "a citation provider contributed to `{slot}` by the package `{package}` of the extension `{stem}` must be \
             an object with `name` and `answer` or `answer_all`"
        )
    }
}

/// A template module's two contributions: its description, given the
/// feature's configuration, as a compose template with the output and the
/// settings that the feature declares for it ([`checked_config`]), and its
/// own engine; the template fed into the registry digest.
fn contribute_module(
    b: &mut CoreFeatureBuilder<'_>,
    who: &Contributor<'_>,
    module: Arc<dyn TemplateModule>,
    how: How,
) -> Result<(), ExtError> {
    let name = module.name();
    if name.is_empty() {
        return Err(ExtError::Argument { method: how.method().to_string(), detail: not_a_handler(TEMPLATES, who.owner) });
    }
    let Some(declared) = who.modules.get(&name) else {
        let (feature, package, stem) = (who.feature, &who.owner.package, &who.owner.stem);
        return Err(ExtError::Argument {
            method: how.method().to_string(),
            detail: format!(
                "the template module ‘{name}’ must be declared under ‘templates’ of the feature ‘{feature}’ of the \
                 package ‘{package}’ of the extension ‘{stem}’"
            ),
        });
    };
    let description = module.describe(who.configuration)?;
    checked_config(&name, who, declared, &description.config)?;
    let caller = who.services.caller();
    let package = caller.extension.factory_name(&caller.package);
    let engine_name = format!("{package}:{name}");
    let template = template_of(&name, &package, &engine_name, declared, description);
    b.digest_input(&digest_of(&template)).map_err(core_error)?;
    let engine = ExtTemplateEngine {
        module,
        engine_name,
        template_name: name,
        owner: who.owner.clone(),
        feature: who.feature.to_string(),
        services: Arc::clone(who.services),
    };
    let engine: Arc<dyn TemplateEngine> = Arc::new(engine);
    let template = Arc::new(template);
    match how {
        // The template before its engine: a registry without `compose`
        // reports the templates' slot, the one the author wrote.
        How::Peer => {
            b.peer_slot::<Template>(TEMPLATES_SLOT, template).map_err(core_error)?;
            b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, engine).map_err(core_error)
        }
        How::Append | How::First => {
            b.contribute_to::<Template>(TEMPLATES_SLOT, template).map_err(core_error)?;
            b.contribute_to::<dyn TemplateEngine>(ENGINES_SLOT, engine).map_err(core_error)
        }
    }
}

/// Checks that `config`, the defaults that the description of the template
/// module `name` gives, holds one for each setting that the feature `who`
/// declares for the module (`declared`, the keys of its schema's
/// `properties`) and no other key.
///
/// # Errors
///
/// [`ExtError::Value`], naming each setting without a default and each
/// undeclared key.
fn checked_config(
    name: &str,
    who: &Contributor<'_>,
    declared: &ModuleReport,
    config: &serde_json::Map<String, Value>,
) -> Result<(), ExtError> {
    let settings = declared.settings.get("properties").and_then(Value::as_object);
    let is_declared = |key: &str| settings.is_some_and(|settings| settings.contains_key(key));
    let quoted = |keys: Vec<&str>| keys.iter().map(|key| format!("‘{key}’")).collect::<Vec<_>>().join(", ");
    let missing: Vec<&str> =
        settings.into_iter().flat_map(|settings| settings.keys()).map(String::as_str).filter(|key| !config.contains_key(*key)).collect();
    let undeclared: Vec<&str> = config.keys().map(String::as_str).filter(|key| !is_declared(key)).collect();
    let (feature, package, stem) = (who.feature, &who.owner.package, &who.owner.stem);
    let mut problems = Vec::new();
    if !missing.is_empty() {
        let defaults = if missing.len() == 1 { "default" } else { "defaults" };
        problems.push(format!("missing {defaults} for {}", quoted(missing)));
    }
    if !undeclared.is_empty() {
        let (keys, them) = if undeclared.len() == 1 { ("key", "it") } else { ("keys", "them") };
        problems.push(format!(
            "undeclared {keys} {} (declare {them} under ‘templates.{name}.settings’ or remove {them} from ‘config’)",
            quoted(undeclared)
        ));
    }
    if problems.is_empty() {
        return Ok(());
    }
    let detail = format!(
        "Invalid ‘config’ of the template module ‘{name}’ (feature ‘{feature}’, package ‘{package}’, extension ‘{stem}’): {}",
        problems.join("; ")
    );
    Err(ExtError::Value { detail })
}

/// The values of the slot `name` as data, in the order `order`: an empty
/// list for a slot no feature declares.
fn read(services: &Services, name: &str, order: Order) -> Result<Vec<Value>, ExtError> {
    let Some(decl) = services.slot(name) else { return Ok(Vec::new()) };
    if decl.ty == TypeKey::of::<ExtService>() {
        let values: Vec<Value> = match order {
            Order::Assembled => services.services_named::<ExtService>(name).map(|value| value.0.clone()).collect(),
            Order::Precedence => {
                services.services_by_precedence_named::<ExtService>(name).map(|value| value.0.clone()).collect()
            }
        };
        return Ok(values);
    }
    if let Some(read) = STANDARD_DATA_SLOTS.iter().find(|standard| standard.name == name).and_then(|standard| standard.read) {
        return read(services, name, order);
    }
    Err(ExtError::Unsupported { what: format!("reading the service slot `{name}`, whose values are not data,") })
}

/// The value of the slot `name` that takes precedence, as data
/// ([`StageCx::service`](crate::api::core::run::StageCx::service)): the first
/// in the order of precedence.
///
/// # Errors
///
/// The slot's values are not data ([`ExtError::Unsupported`]).
pub(crate) fn read_first(services: &Services, name: &str) -> Result<Option<Value>, ExtError> {
    Ok(read(services, name, Order::Precedence)?.into_iter().next())
}

/// Every value of the slot `name` as data, in their assembled order
/// ([`StageCx::services`](crate::api::core::run::StageCx::services)).
///
/// # Errors
///
/// The slot's values are not data ([`ExtError::Unsupported`]).
pub(crate) fn read_all(services: &Services, name: &str) -> Result<Vec<Value>, ExtError> {
    read(services, name, Order::Assembled)
}

/// Every value of the slot `name` as data, in the order of precedence
/// ([`StageCx::services_by_precedence`](crate::api::core::run::StageCx::services_by_precedence)).
///
/// # Errors
///
/// The slot's values are not data ([`ExtError::Unsupported`]).
pub(crate) fn read_by_precedence(services: &Services, name: &str) -> Result<Vec<Value>, ExtError> {
    read(services, name, Order::Precedence)
}

// --- host services -------------------------------------------------------------------------------

/// The host services one package declared, resolved at load: each declared
/// service with the host's object, the names of every service the host
/// offers (for the messages), the extension's files (for the root `self` of
/// `files`), the files of the extensions loaded by use name (for the root
/// `ext+NAME` of `files`), and the caller every call names. Every scripted
/// feature and stage of the package holds it.
pub struct DeclaredServices {
    caller: HostCaller,
    declared: BTreeMap<String, Arc<dyn HostService>>,
    offered: Vec<String>,
    files: Arc<dyn ExtensionFiles + Send + Sync>,
    extensions: ExtensionsByName,
}

impl DeclaredServices {
    /// The services `declared` (a package's `host` list) taken from
    /// `offered` (the host's services by name), for calls made by `caller`,
    /// the root `self` of `files` read from `files` and the root `ext+NAME`
    /// from `extensions`. A declared name the host does not offer is left
    /// out: the loader refuses such a package before.
    pub fn new(
        caller: HostCaller,
        offered: &BTreeMap<String, Arc<dyn HostService>>,
        declared: &[String],
        files: Arc<dyn ExtensionFiles + Send + Sync>,
        extensions: ExtensionsByName,
    ) -> DeclaredServices {
        let declared = declared
            .iter()
            .filter_map(|name| Some((name.clone(), Arc::clone(offered.get(name)?))))
            .collect();
        DeclaredServices { caller, declared, offered: offered.keys().cloned().collect(), files, extensions }
    }

    /// Who calls: the extension, the package, the digest's bytes.
    pub fn caller(&self) -> &HostCaller {
        &self.caller
    }

    /// The refusal of a service the package did not declare.
    fn undeclared(&self, name: &str, method: &str) -> ExtError {
        let (package, stem) = (&self.caller.package, &self.caller.extension.stem);
        let mut detail = format!(
            "the host service `{name}` must be declared in the `host` list of the package `{package}` of the \
             extension `{stem}` before a handler calls it"
        );
        if !self.offered.iter().any(|offered| offered == name) {
            detail.push_str(&format!("; this program offers no host service `{name}`"));
        }
        ExtError::Argument { method: method.to_string(), detail }
    }

    /// The handle [`StageCx::host`](crate::api::core::run::StageCx::host)
    /// lends for `name`, whose calls pass the service `progress`, the scope
    /// of the handler call that lends it.
    ///
    /// # Errors
    ///
    /// The package did not declare `name` ([`ExtError::Argument`]).
    pub(crate) fn handle(self: &Arc<Self>, name: &str, progress: &Scope) -> Result<HostHandle, ExtError> {
        let Some(service) = self.declared.get(name) else { return Err(self.undeclared(name, "host")) };
        let progress = progress.clone();
        if name == files::NAME {
            return Ok(HostHandle::Files(Box::new(FilesFacade { services: Arc::clone(self), progress })));
        }
        Ok(HostHandle::Service(Box::new(ServiceFacade { services: Arc::clone(self), service: Arc::clone(service), progress })))
    }

    /// Whether the package declared the host service `name` (and the host
    /// offers it: a package declaring a service the host lacks does not
    /// load).
    pub(crate) fn declares(&self, name: &str) -> bool {
        self.declared.contains_key(name)
    }

    /// A read of the `files` service: `read_bytes` when `bytes`, else
    /// `read_text`, of the qualified path `path` ([`crate::paths`]). `self:`
    /// is read from the extension's files and `ext+NAME:` from the files of
    /// the extension that the use name `NAME` found; every other root (an
    /// absolute path read as `doc:`) through the host's object, called with
    /// `progress`, the scope of the handler call that reads, and the path
    /// written `ROOT:PATH`. `None` when nothing is at the path, a path of an
    /// extension that names no file on this system included
    /// ([`FilesErrorKind::NoSystemForm`]), as in the document's folder.
    ///
    /// # Errors
    ///
    /// The package did not declare `files` ([`ExtError::Argument`]); a path
    /// with no root, a path whose syntax has no meaning, a use name that no
    /// `use` entry in effect writes, an `https:` address, a path the host
    /// refused or a failed read ([`ExtError::Host`]); an answer of the host
    /// of the wrong shape ([`ExtError::Value`]).
    pub fn read_file(&self, path: &str, bytes: bool, progress: &Scope) -> Result<Option<HostAnswer>, ExtError> {
        let method = if bytes { files::READ_BYTES } else { files::READ_TEXT };
        let host_error =
            |detail: String| ExtError::Host { service: files::NAME.to_string(), method: method.to_string(), detail };
        let Some(service) = self.declared.get(files::NAME) else { return Err(self.undeclared(files::NAME, method)) };
        let qualified = match WrittenPath::parse(path).map_err(|error| host_error(error.to_string()))? {
            WrittenPath::Own(own) => return read_extension_file(self.files.as_ref(), &own, bytes, host_error),
            WrittenPath::Bare(bare) => {
                return Err(host_error(format!(
                    "Missing root in the path ‘{bare}’: write ‘self:{bare}’ for a file of the extension, ‘doc:{bare}’ \
                     for a file of the document's folder"
                )));
            }
            WrittenPath::Qualified(qualified) => qualified,
        };
        match qualified.root() {
            Root::Ext(name) => {
                let Some(files) = self.extensions.files(name) else {
                    let unknown =
                        PathError { path: qualified.to_string(), kind: PathErrorKind::UnknownExtension(name.clone()) };
                    return Err(host_error(unknown.to_string()));
                };
                return read_extension_file(files.as_ref(), qualified.path(), bytes, host_error);
            }
            Root::Https => {
                return Err(host_error(format!(
                    "Unsupported path ‘{path}’ for the host service ‘files’: an ‘https’ address is read through the \
                     host service ‘download’"
                )));
            }
            Root::Std | Root::Doc | Root::Cwd => {}
        }
        let args = json!({ "path": qualified.to_string() });
        match service.call(&self.caller, method, &args, progress).map_err(|error| host_error(error.detail))? {
            HostAnswer::Data(Value::Null) => Ok(None),
            answer @ HostAnswer::Bytes(_) if bytes => Ok(Some(answer)),
            answer @ HostAnswer::Data(Value::String(_)) if !bytes => Ok(Some(answer)),
            _ => Err(ExtError::Value {
                detail: format!(
                    "the method `{method}` of the host service `files` must answer {} or null",
                    if bytes { "bytes" } else { "a text" }
                ),
            }),
        }
    }

    /// A call of the `download` service: the file at the address `url`
    /// ([`download`]), called with `progress`, the scope of the call that
    /// downloads.
    ///
    /// # Errors
    ///
    /// The package did not declare `download` ([`ExtError::Argument`]); the
    /// host's refusal ([`ExtError::Host`]); an answer of the wrong shape
    /// ([`ExtError::Value`]).
    pub(crate) fn download(&self, url: &str, progress: &Scope) -> Result<Downloaded, ExtError> {
        let Some(service) = self.declared.get(download::NAME) else {
            return Err(self.undeclared(download::NAME, download::GET));
        };
        let answer = service.call(&self.caller, download::GET, &json!({ "url": url }), progress).map_err(|error| {
            ExtError::Host { service: download::NAME.to_string(), method: download::GET.to_string(), detail: error.detail }
        })?;
        let wrong = || ExtError::Value {
            detail: String::from(
                "the method ‘get’ of the host service ‘download’ must answer ‘{ content, media_type, warning? }’",
            ),
        };
        let HostAnswer::Data(Value::Object(answer)) = answer else { return Err(wrong()) };
        let content = answer.get("content").ok_or_else(wrong)?;
        let bytes = content_from_value(content, &|| String::from("the content that the host service ‘download’ answers"))?;
        let text = |key: &str| match answer.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(wrong()),
        };
        Ok(Downloaded { bytes, media_type: text("media_type")?, warning: text("warning")? })
    }
}

/// A file that the `download` service answered ([`DeclaredServices::download`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Downloaded {
    /// The file's bytes.
    pub(crate) bytes: Vec<u8>,
    /// The media type that the server stated, `None` when it stated none.
    pub(crate) media_type: Option<String>,
    /// The warning about the file answered, when there is one.
    pub(crate) warning: Option<String>,
}

/// A read of the `files` service in the files of an extension, `files`, at
/// `path` inside its folder: bytes when `bytes`, else a text. `None` when
/// nothing is at the path, a path that names no file on this system included
/// ([`FilesErrorKind::NoSystemForm`]); any other failure becomes the error
/// `host_error` makes of its text.
fn read_extension_file(
    files: &(dyn ExtensionFiles + Send + Sync),
    path: &str,
    bytes: bool,
    host_error: impl Fn(String) -> ExtError,
) -> Result<Option<HostAnswer>, ExtError> {
    let read = if bytes {
        files.read_bytes("", path).map(|found| found.map(HostAnswer::Bytes))
    } else {
        files.read_text("", path).map(|found| found.map(|text| HostAnswer::Data(Value::String(text))))
    };
    match read {
        Err(FilesError { kind: FilesErrorKind::NoSystemForm(_), .. }) => Ok(None),
        read => read.map_err(|error| host_error(error.to_string())),
    }
}

/// The [`Files`] façade over a package's declared services, lent to a
/// handler call whose scope is `progress`.
struct FilesFacade {
    services: Arc<DeclaredServices>,
    progress: Scope,
}

impl Sealed for FilesFacade {}

impl Files for FilesFacade {
    fn read_text(&self, path: &str) -> Result<Option<String>, ExtError> {
        Ok(match self.services.read_file(path, false, &self.progress)? {
            Some(HostAnswer::Data(Value::String(text))) => Some(text),
            _ => None,
        })
    }
}

/// The [`HostService`](HostServiceFacade) façade over one declared service,
/// lent to a handler call whose scope is `progress`.
struct ServiceFacade {
    services: Arc<DeclaredServices>,
    service: Arc<dyn HostService>,
    progress: Scope,
}

impl Sealed for ServiceFacade {}

impl HostServiceFacade for ServiceFacade {
    fn name(&self) -> String {
        self.service.name().to_string()
    }

    fn call(&self, method: &str, args: Value) -> Result<Value, ExtError> {
        let service = self.service.name();
        match self.service.call(&self.services.caller, method, &args, &self.progress) {
            Ok(HostAnswer::Data(value)) => {
                check_data(&value)?;
                Ok(value)
            }
            Ok(HostAnswer::Bytes(_)) => Err(ExtError::Argument {
                method: "call".to_string(),
                detail: format!(
                    "the method `{method}` of the host service `{service}` answers bytes, which a script cannot receive"
                ),
            }),
            Err(error) => {
                Err(ExtError::Host { service: service.to_string(), method: method.to_string(), detail: error.detail })
            }
        }
    }
}
