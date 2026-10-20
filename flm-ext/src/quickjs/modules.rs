//! The modules of an engine instance: their names, how an import's specifier
//! resolves, where a module's text comes from, and the closing of the
//! resolver.
//!
//! # Three kinds of module, under three forms of name
//!
//! | Kind | Module name | Example |
//! |---|---|---|
//! | a module of the prelude | `flm:` and its path | `flm:core/stage`, `flm:_runtime`, `flm:_entry` |
//! | a module of the extension | `./` and its normalized path inside the extension | `./main.js`, `./src/helpers.js` |
//! | a module the host offers | its name, a bare name | `lodash`, `mathjax/core` |
//!
//! No name of one kind can be a name of another: a host module's name holds
//! no `:` and does not begin with `.` or `/` ([`JsHost::with_module`]).
//! The main module is also reached as `flm:_main`, the one name the entry
//! module imports it by.
//!
//! # The module source
//!
//! The resolver and the loader of one engine instance share one
//! [`ModuleState`]. In the *files mode* (the one instance
//! [`JsEngine::load`](super::JsEngine) creates, on the loading thread) the
//! extension's modules are read through the extension's files as the
//! resolver meets them, and recorded; in the *recorded mode* (every other
//! instance) they come from the recorded set, and the files are never
//! touched.
//!
//! # Closing
//!
//! QuickJS resolves and loads the whole graph of static imports when the
//! entry module is declared, before any code runs. Right after that
//! declaration the binding closes the resolver ([`ModuleState::close`]):
//! from then on every request is refused, so a dynamic `import()` is refused
//! wherever it is written. Each refusal is recorded as its message text
//! before QuickJS sees it; the binding takes the texts after running the
//! queued jobs at the end of every entry into the instance.
//!
//! [`JsHost::with_module`]: super::JsHost::with_module

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{Ctx, Error, Exception, Module, Persistent, Value};

use crate::paths::{self, PathErrorKind};
use crate::source::ExtensionFiles;

/// The name of the prelude's helper module.
pub(crate) const RUNTIME: &str = "flm:_runtime";

/// The name of the module that imports every API module (the generated
/// `js/api/_api.js`), imported by the entry module.
pub(crate) const API: &str = "flm:_api";

/// The name of the prelude's entry module, declared by the binding itself.
pub(crate) const ENTRY: &str = "flm:_entry";

/// The name the entry module imports the extension's main module by.
pub(crate) const MAIN_ALIAS: &str = "flm:_main";

/// The prefix of the names of the extension's modules.
pub(crate) const EXTENSION_PREFIX: &str = "./";

/// The prelude's helper module.
pub(crate) const RUNTIME_JS: &str = include_str!("js/runtime.js");

/// The prelude's entry module.
pub(crate) const ENTRY_JS: &str = include_str!("js/entry.js");

/// The generated module list (`UPDATE_GOLDEN=1 cargo test -p flm-ext --test
/// api_surface` writes it): `API_MODULES`, the API modules as (module name,
/// source), the module `flm:a/b` being the stub `js/api/a/b.js`, which
/// mirrors `flm_ext::api::a::b`; and `API_JS`, the module `flm:_api`, which
/// imports them all.
#[rustfmt::skip]
#[path = "generated_modules.rs"]
mod generated_modules;

pub(crate) use generated_modules::{API_JS, API_MODULES};

/// The source of a module of the prelude, by its name.
fn prelude_source(name: &str) -> Option<&'static str> {
    match name {
        RUNTIME => Some(RUNTIME_JS),
        ENTRY => Some(ENTRY_JS),
        API => Some(API_JS),
        _ => API_MODULES.iter().find(|(api, _)| *api == name).map(|(_, source)| *source),
    }
}

/// The state the resolver and the loader of one engine instance share.
pub(crate) struct ModuleState {
    /// The extension's stem, for the messages.
    stem: String,
    /// The main module's normalized path.
    main: String,
    /// The host's modules.
    host: BTreeMap<String, Arc<str>>,
    /// The files mode: the extension's files, held until the binding closes
    /// the resolver and takes them back.
    files: Option<Box<dyn ExtensionFiles>>,
    /// The extension's modules known so far: normalized path → text. In the
    /// files mode it starts with the main module and grows as the resolver
    /// reads; in the recorded mode it is the recorded set and never grows.
    recorded: BTreeMap<String, Arc<str>>,
    /// Whether the resolver refuses every request.
    closed: bool,
    /// Every refusal since the binding last took them: its message text and
    /// the error object the resolver threw (`None` when the object could not
    /// be made), with which the `import()` promise is rejected.
    refusals: Vec<(String, Option<Persistent<Value<'static>>>)>,
}

/// One engine instance's [`ModuleState`], shared by its resolver, its loader
/// and the binding.
pub(crate) type SharedState = Rc<RefCell<ModuleState>>;

/// Frees the error objects of the refusals a [`ModuleState`] still holds
/// when dropped. The resolver, which holds the state too, is freed after its
/// runtime, and a persistent value must be freed while its runtime is alive:
/// an engine instance keeps this guard in a field declared before its
/// runtime, and the creation of an instance in a local declared after it.
pub(crate) struct FreeRefusals(pub SharedState);

impl Drop for FreeRefusals {
    /// Nothing else borrows the state when an instance is dropped: a broken
    /// invariant fails loudly rather than leaving the objects unfreed.
    fn drop(&mut self) {
        self.0.borrow_mut().refusals.clear();
    }
}

impl ModuleState {
    /// The state of an engine instance of the extension of stem `stem`, whose
    /// main module is `main`; `recorded` the modules known (in the files mode,
    /// the main module alone), `files` the extension's files in the files mode.
    pub(crate) fn shared(
        stem: &str,
        main: &str,
        host: BTreeMap<String, Arc<str>>,
        recorded: BTreeMap<String, Arc<str>>,
        files: Option<Box<dyn ExtensionFiles>>,
    ) -> SharedState {
        Rc::new(RefCell::new(ModuleState {
            stem: stem.to_string(),
            main: main.to_string(),
            host,
            files,
            recorded,
            closed: false,
            refusals: Vec::new(),
        }))
    }

    /// Closes the resolver; answers the files of the files mode.
    pub(crate) fn close(&mut self) -> Option<Box<dyn ExtensionFiles>> {
        self.closed = true;
        self.files.take()
    }

    /// The texts of the refusals recorded since the last call; their error
    /// objects are freed.
    pub(crate) fn take_refusals(&mut self) -> Vec<String> {
        std::mem::take(&mut self.refusals).into_iter().map(|(text, _)| text).collect()
    }

    /// Whether `reason` is the error object of a refusal recorded since the
    /// binding last took them: the same object (`===`), which QuickJS rejects
    /// a refused `import()` with.
    pub(crate) fn is_refusal<'js>(&self, ctx: &Ctx<'js>, reason: &Value<'js>) -> bool {
        self.refusals.iter().any(|(_, object)| {
            object.as_ref().is_some_and(|object| object.clone().restore(ctx).is_ok_and(|object| object == *reason))
        })
    }

    /// The extension's modules known: normalized path → text.
    pub(crate) fn recorded(&self) -> &BTreeMap<String, Arc<str>> {
        &self.recorded
    }

    /// How a message names the module `base`: "the module `x.js` of the
    /// extension `S`", "the module `NAME` this program offers", or "the module
    /// `flm:…`".
    fn who(&self, base: &str) -> String {
        if let Some(path) = base.strip_prefix(EXTENSION_PREFIX) {
            format!("the module `{path}` of the extension `{}`", self.stem)
        } else if self.host.contains_key(base) {
            format!("the module `{base}` this program offers")
        } else {
            format!("the module `{base}`")
        }
    }

    /// The name the specifier `name` imported from the module `base` resolves
    /// to, or the text of the refusal.
    fn resolve(&mut self, base: &str, name: &str) -> Result<String, String> {
        let who = self.who(base);
        if self.closed {
            return Err(format!(
                "{who} calls `import(\"{name}\")`; every module must be imported by an `import` declaration at the top \
                 of a module"
            ));
        }
        if name == MAIN_ALIAS {
            return Ok(format!("{EXTENSION_PREFIX}{}", self.main));
        }
        if name.starts_with("flm:") {
            if prelude_source(name).is_some() {
                return Ok(name.to_string());
            }
            let names: Vec<String> = API_MODULES.iter().map(|(api, _)| format!("`{api}`")).collect();
            return Err(format!(
                "{who} imports `{name}`, which is not a module of the extension API; its modules are {}",
                names.join(", ")
            ));
        }
        if name.starts_with("./") || name.starts_with("../") || name == "." || name == ".." {
            return self.resolve_relative(&who, base, name);
        }
        let bytes = name.as_bytes();
        let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if name.starts_with('/') || name.starts_with('\\') || drive {
            return Err(format!(
                "{who} imports `{name}`, which is an absolute path; a module of the extension is imported by a path \
                 beginning with `./` or `../`"
            ));
        }
        if self.host.contains_key(name) {
            return Ok(name.to_string());
        }
        Err(self.not_offered(&who, base, name))
    }

    /// The refusal of a bare name no host module has.
    fn not_offered(&self, who: &str, base: &str, name: &str) -> String {
        if self.host.contains_key(base) {
            format!("{who} imports `{name}`, which is not a module this program offers")
        } else {
            format!(
                "{who} imports `{name}`, which is neither a module of the extension (`./…`), nor of the extension API \
                 (`flm:…`), nor a module this program offers"
            )
        }
    }

    /// A relative specifier: joined to the importing module's path, among
    /// the extension's modules or the host's.
    fn resolve_relative(&mut self, who: &str, base: &str, name: &str) -> Result<String, String> {
        if let Some(importer) = base.strip_prefix(EXTENSION_PREFIX) {
            let path = match paths::join(importer, name) {
                Ok(path) => path,
                Err(error) if error.kind == PathErrorKind::LeavesRoot => {
                    return Err(format!("{who} imports `{name}`, which leaves the extension's folder"));
                }
                Err(error) if error.kind == PathErrorKind::Backslash => {
                    return Err(format!("{who} imports `{name}`, which holds `\\`; a path inside the extension uses `/`"));
                }
                Err(_) => {
                    return Err(format!(
                        "{who} imports `{name}`, which is an absolute path; a module of the extension is imported by \
                         a path beginning with `./` or `../`"
                    ));
                }
            };
            if path.is_empty() {
                return Err(format!("{who} imports `{name}`, which names the extension's folder, not a file"));
            }
            if !self.recorded.contains_key(&path) {
                let read = match &self.files {
                    Some(files) => files.read_text(&path),
                    None => Ok(None),
                };
                match read {
                    Ok(Some(text)) => {
                        self.recorded.insert(path.clone(), Arc::from(text));
                    }
                    Ok(None) => {
                        return Err(format!("{who} imports `{name}`, and the extension holds no file `{path}`"));
                    }
                    Err(error) => {
                        return Err(format!("{who} imports `{name}`, and the file `{path}` cannot be read: {error}"));
                    }
                }
            }
            return Ok(format!("{EXTENSION_PREFIX}{path}"));
        }
        let from_host = self.host.contains_key(base);
        let joined = paths::join(base, name).ok().filter(|joined| from_host && self.host.contains_key(joined));
        if let Some(joined) = joined {
            return Ok(joined);
        }
        Err(self.not_offered(who, base, name))
    }

    /// The text of the module `name`, which the resolver answered.
    fn source(&self, name: &str) -> Option<Arc<str>> {
        if let Some(path) = name.strip_prefix(EXTENSION_PREFIX) {
            return self.recorded.get(path).cloned();
        }
        if let Some(source) = prelude_source(name) {
            return Some(Arc::from(source));
        }
        self.host.get(name).cloned()
    }
}

/// The resolver of an engine instance: [`ModuleState::resolve`], each
/// refusal recorded before it is answered.
pub(crate) struct ModuleResolver(pub SharedState);

impl Resolver for ModuleResolver {
    fn resolve<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        let mut state = self.0.borrow_mut();
        state.resolve(base, name).map_err(|text| {
            // The error object is thrown into the context, so that the
            // `import()` promise is rejected with this object, which the
            // rejection tracker recognizes by identity.
            match Exception::from_message(ctx.clone(), &text) {
                Ok(exception) => {
                    let object = exception.into_value();
                    state.refusals.push((text, Some(Persistent::save(ctx, object.clone()))));
                    ctx.throw(object)
                }
                Err(error) => {
                    state.refusals.push((text, None));
                    error
                }
            }
        })
    }
}

/// The loader of an engine instance: declares the module the resolver
/// answered, from the prelude's constant, the host's source or the recorded
/// text.
pub(crate) struct ModuleLoader(pub SharedState);

impl Loader for ModuleLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, Declared>> {
        // The borrow ends before the declaration, which resolves the module's
        // own imports through the resolver.
        let source = self.0.borrow().source(name);
        match source {
            Some(source) => Module::declare(ctx.clone(), name, source.as_bytes()),
            None => Err(Error::new_loading(name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MemoryFiles;

    fn state(files: MemoryFiles, host: &[(&str, &str)]) -> ModuleState {
        let host = host.iter().map(|(name, text)| ((*name).to_string(), Arc::from(*text))).collect();
        let recorded = BTreeMap::from([("src/main.js".to_string(), Arc::from(""))]);
        let shared = ModuleState::shared("ext", "src/main.js", host, recorded, Some(Box::new(files)));
        Rc::try_unwrap(shared).ok().map(RefCell::into_inner).expect("one owner")
    }

    #[test]
    fn a_specifier_resolves_by_its_form() {
        let files = MemoryFiles::new().with_text("src/a.js", "").with_text("b.js", "");
        let mut state = state(files, &[("lib", ""), ("lib/core", ""), ("lib/util", "")]);
        assert_eq!(state.resolve(ENTRY, MAIN_ALIAS), Ok("./src/main.js".to_string()));
        assert_eq!(state.resolve("./src/main.js", "./a.js"), Ok("./src/a.js".to_string()));
        assert_eq!(state.resolve("./src/main.js", "../b.js"), Ok("./b.js".to_string()));
        assert_eq!(state.resolve("./src/main.js", "flm:core/stage"), Ok("flm:core/stage".to_string()));
        assert_eq!(state.resolve("./src/main.js", "lib"), Ok("lib".to_string()));
        assert_eq!(state.resolve("lib/core", "./util"), Ok("lib/util".to_string()));
        assert_eq!(state.recorded().keys().collect::<Vec<_>>(), ["b.js", "src/a.js", "src/main.js"]);
        assert!(state.resolve("./src/main.js", "../../x.js").unwrap_err().ends_with("which leaves the extension's folder"));
        assert!(state.resolve("./src/main.js", "./c.js").unwrap_err().ends_with("holds no file `src/c.js`"));
        state.close();
        assert!(state.resolve("./src/main.js", "./a.js").unwrap_err().contains("calls `import(\"./a.js\")`"));
    }
}
