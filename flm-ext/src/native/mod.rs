//! The engine `native` ([`NativeEngine`], cargo feature `native`): extensions
//! written in Rust against the guest client ([`crate::guest`]) and compiled
//! into the program that loads them, run in the same process.
//!
//! # One crate, two forms
//!
//! An extension written against [`crate::guest`] is one crate. Built as a
//! WebAssembly module, it is run by the engine `wasm` (module
//! `flm_ext::wasm`, cargo feature `wasm`); compiled into a program, it is a
//! Rust library the program links, which this engine runs. The crate's source
//! is the same: it depends on `flm-ext` with the feature `guest` alone, and
//! its root invokes the macro [`extension!`](crate::guest::extension), which
//! writes, on every target, the static `FLM_EXTENSION`
//! ([`CompiledExtension`]) through which a program reaches the crate.
//!
//! The program turns on `flm-ext`'s feature `native`, and that feature, not
//! the target, decides the form: with it, on every target, the extension's
//! requests are answered in the same process, and the macro writes no export.
//! So a program built for `wasm32` (a program running the library in a web
//! browser, for instance) carries extensions compiled in as a program built
//! for any other target does. Without the feature, a crate compiled for
//! `wasm32` is a WebAssembly module of its own: the macro writes the module's
//! four exports (`flm_alloc`, `flm_free`, `flm_entry`, `flm_last_panic`) and
//! the guest client imports the functions `flm.call` and `flm.take`.
//!
//! Within one build, cargo compiles `flm-ext` once, with one set of features,
//! for every crate of the build. An extension's WebAssembly module is
//! therefore built in a cargo run where no crate turns on `native`, with the
//! extension's package selected alone (`cargo rustc -p NAME --target
//! wasm32-unknown-unknown …`). A module built in a run where some crate turns
//! on `native` lacks the four exports, and the engine `wasm` refuses it when it
//! loads, with a message saying which export the module lacks.
//!
//! # What the program gives the engine
//!
//! The program lists the extensions it compiles in ([`NativeExtension`]):
//! each one's stem, the text of its manifest, and its `FLM_EXTENSION`. The
//! engine keeps the list. The program's [source](crate::source) answers the
//! extension's stem with its manifest, `extension.engine` being `native`; the
//! loader then gives the extension to this engine, which finds it by its stem
//! ([`Engine::load`]). The identity is the source's, as for every engine. A
//! manifest that specifies the engine `native` for a stem the engine does not
//! hold is refused, with the stems it holds.
//!
//! ```
//! use std::sync::Arc;
//!
//! use flm_core::core::registry::PackageResolver;
//! use flm_ext::guest::core::registry::FeatureBuilder;
//! use flm_ext::guest::{Extension, Feature, Package};
//! use flm_ext::native::{NativeEngine, NativeExtension};
//! use flm_ext::{ExtError, ExtensionLoader, MemoryFiles, MemorySource};
//! use serde_json::{json, Value};
//!
//! // The extension crate's root (here in the same crate): the macro writes the static
//! // `FLM_EXTENSION`.
//! flm_ext::guest::extension!(extension);
//!
//! fn extension(ext: &mut Extension) {
//!     ext.package("_", Package::new().uses(["stdcore"]).feature("myext", Feature::new().register(register)));
//! }
//!
//! fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
//!     b.table("myext.notes", "notes of the extension")
//! }
//!
//! // The program: the engine holding the extension, and a source answering its stem.
//! let engine = NativeEngine::new([NativeExtension {
//!     stem: "myext",
//!     manifest: "extension:\n  engine: native\n",
//!     compiled: &FLM_EXTENSION,
//! }]);
//! let manifest = json!({ "extension": { "engine": "native" } });
//! let source = MemorySource::new().with("myext", "compiled-in:myext", manifest, MemoryFiles::new());
//! let loader = ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine));
//! let factory = loader.resolve("myext").expect("the extension loads").expect("found");
//! assert_eq!(factory.name().as_str(), "compiled-in:myext#_");
//! ```
//!
//! # How an extension runs
//!
//! The guest client runs in the program as it runs in a WebAssembly module,
//! with two differences:
//!
//! - **Requests.** A request of the extension (a call of an API method, the
//!   lending of a handler object, a line of `console` output) is answered in
//!   the same process by the host side every binding shares, the request and
//!   its answer passed as JSON values, never as JSON text.
//! - **Entries.** The engine calls the guest's entry points (`define`,
//!   `packages`, `register`, `rehearse`, and the handlers' entries) directly,
//!   their arguments and their answers passed as JSON values.
//!
//! Each thread has one instance of each extension the engine loaded, as each
//! thread has one WebAssembly instance of a module: the guest runtime's state
//! (what `define` declared, what each feature's `register` recorded) belongs
//! to the instance, and is installed as the thread's current state while one
//! of its entries runs. Two extensions never see each other's state, and a
//! nested entry (a handler of one extension run while a request of another,
//! or of the same one, runs) restores each instance's state on its way out.
//! A request is answered only while an instance's state is current: one sent
//! from a thread the extension started answers
//! [`ExtError::Unsupported`](crate::ExtError::Unsupported).
//!
//! The objects an instance holds (what `define` declared, the stages, rule
//! bodies and hooks each feature's `register` recorded) are Rust values of the
//! extension's crate, so their `Drop` runs when the engine drops the
//! instance: after a panic (below); when no one holds the loaded extension any
//! more (at the next handler call on the thread); when the instance's entry
//! `define` fails, or the load's `packages`; and when the thread ends. That
//! code runs with the instance's own state current and outside any handler
//! call: a request it sends that needs a running handler call (an API method,
//! the lending of a handler object) is refused, as one sent from `define` is,
//! and when the thread ends every request it sends answers
//! [`ExtError::Unsupported`](crate::ExtError::Unsupported). A panic in such a
//! `Drop` is caught and fails no handler call, since none is running: Rust's
//! panic hook alone reports it (the default hook prints it to standard error).
//! (In a WebAssembly module the objects are never dropped one by one: the
//! engine `wasm` discards the instance's memory whole.)
//!
//! # Limits and failures
//!
//! The engine sets no limit: no fuel, no time budget, no memory budget. An
//! extension compiled into a program is trusted code with the program's
//! rights. Nor is the depth of the values a request carries bounded: the
//! engines `js` and `wasm` refuse a value nested deeper than 127 levels when
//! they read JSON text, a bound of their text reader, and this engine reads no
//! text.
//!
//! A panic during an entry fails the handler call with an
//! [`ExtError::Handler`](crate::ExtError::Handler) carrying the panic's
//! message ("… must run without a panic: MESSAGE"), and the thread's instance
//! of the extension is discarded (on its way out of the outermost entry into
//! it): the next handler call on the thread creates a fresh one, which runs
//! `define` and rehearses the feature. This holds for a panic raised inside
//! the library while it answers one of the extension's requests, too: the
//! panic unwinds to the extension's entry, the one place the engine catches
//! it. The engine installs no panic hook; a program built with
//! `panic = "abort"` (the strategy of the target `wasm32-unknown-unknown`)
//! stops at the panic.
//!
//! # The digest
//!
//! The engine's part of the registry digest is the engine's name, the
//! version of `flm-ext`, and the extension crate's name and version (where the
//! engine `wasm` takes the module's bytes), not the extension's code. A change
//! to the extension that keeps the crate's version leaves the digest
//! unchanged, as a change to a feature written in Rust does when the feature
//! keeps its version string, unless the program's build identity
//! ([`RegistryInputs::build_identity`](flm_core::core::registry::RegistryInputs::build_identity),
//! such as a hash of the program's binary) changes with it.

mod instance;

use std::fmt;
use std::sync::Arc;

use instance::{NativeInstance, NativeLoaded};

use crate::binding::packages::read_packages;
use crate::binding::{self, BindingRuntime};
use crate::engine::{Engine, ExtensionIdentity, LoadError, Loaded};
use crate::guest::CompiledExtension;
use crate::host::ConsoleSink;
use crate::manifest::Manifest;
use crate::source::ExtensionFiles;

/// The name of the engine, as a manifest's `extension.engine` gives it.
pub const NATIVE: &str = "native";

/// One extension compiled into the program, as the program lists it for the
/// engine `native`: the stem the engine finds it by, the text of its
/// manifest, and the static the macro
/// [`extension!`](crate::guest::extension) writes at the root of its crate.
#[derive(Debug, Clone, Copy)]
pub struct NativeExtension {
    /// The extension's stem, the name a `use` entry gives it (`pdf-latex`).
    pub stem: &'static str,
    /// The text of the extension's manifest, for the program's source; the
    /// engine itself does not read it.
    pub manifest: &'static str,
    /// The extension's code: `&my_extension::FLM_EXTENSION`.
    pub compiled: &'static CompiledExtension,
}

/// The engine `native`: the extensions compiled into the program
/// ([`NativeExtension`]), run through the guest client in the same process
/// (the [module documentation](self)). Created with the program's list of
/// extensions; `console` output goes to standard error unless
/// [`with_console`](NativeEngine::with_console) routes it elsewhere.
#[derive(Clone)]
pub struct NativeEngine {
    /// The extensions the engine holds, in the order given.
    extensions: Vec<NativeExtension>,
    /// Where `console` output goes.
    console: ConsoleSink,
}

impl NativeEngine {
    /// The engine holding `extensions`, its `console` output written to
    /// standard error as `[native <level>] <text>`.
    pub fn new(extensions: impl IntoIterator<Item = NativeExtension>) -> NativeEngine {
        NativeEngine {
            extensions: extensions.into_iter().collect(),
            console: Arc::new(|level, text| eprintln!("[native {level}] {text}")),
        }
    }

    /// Routes the `console` output of the extensions this engine loads from
    /// now on to `sink`.
    pub fn with_console(mut self, sink: ConsoleSink) -> NativeEngine {
        self.console = sink;
        self
    }

    /// The extensions the engine holds, in the order given.
    pub fn extensions(&self) -> &[NativeExtension] {
        &self.extensions
    }
}

impl fmt::Debug for NativeEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stems: Vec<&str> = self.extensions.iter().map(|extension| extension.stem).collect();
        f.debug_struct("NativeEngine").field("extensions", &stems).finish_non_exhaustive()
    }
}

/// The engine's part of the digest: the bytes `flm-ext native `, the version
/// of `flm-ext`, a NUL byte, the crate's name, a NUL byte, the crate's
/// version, a NUL byte.
fn digest(compiled: &CompiledExtension) -> Vec<u8> {
    let mut digest = format!("flm-ext {NATIVE} {}", env!("CARGO_PKG_VERSION")).into_bytes();
    for part in [compiled.name(), compiled.version()] {
        digest.push(0);
        digest.extend_from_slice(part.as_bytes());
    }
    digest.push(0);
    digest
}

impl Engine for NativeEngine {
    /// `native`.
    fn name(&self) -> &str {
        NATIVE
    }

    /// Loads the extension: finds it by its stem among the extensions the
    /// engine holds, creates its instance on this thread (its entry `define`
    /// called), reads the packages from its entry `packages`, the templates
    /// from `files`; the instance is then kept on this thread for the
    /// registry build.
    fn load(
        &self,
        extension: &ExtensionIdentity,
        _manifest: &Manifest,
        files: Box<dyn ExtensionFiles>,
    ) -> Result<Loaded, Vec<LoadError>> {
        let stem = &extension.stem;
        let Some(found) = self.extensions.iter().find(|held| held.stem == stem.as_str()) else {
            return Err(vec![LoadError::Manifest { path: "extension.engine".to_string(), detail: self.not_held(stem) }]);
        };
        let loaded = Arc::new(NativeLoaded {
            id: binding::next_extension_id(),
            extension: extension.clone(),
            compiled: found.compiled,
            console: Arc::clone(&self.console),
        });
        let created = NativeInstance::create(&loaded).map_err(|error| vec![LoadError::Load { detail: error.to_string() }])?;
        let defined =
            created.call_load_entry(&loaded, "packages", &[]).map_err(|error| vec![LoadError::Load { detail: error.to_string() }])?;
        let packages = read_packages(extension, &defined.to_string(), &*files)?;
        drop(files);
        instance::keep(&loaded, created);
        Ok(Loaded { packages, runtime: Arc::new(BindingRuntime { binding: loaded, digest: digest(found.compiled) }) })
    }
}

impl NativeEngine {
    /// The refusal of the stem `stem`, which no extension the engine holds
    /// has.
    fn not_held(&self, stem: &str) -> String {
        let held: Vec<String> = self.extensions.iter().map(|extension| format!("`{}`", extension.stem)).collect();
        let holds = if held.is_empty() {
            "no extension is compiled into it".to_string()
        } else {
            format!("the extensions compiled into it are {}", held.join(", "))
        };
        format!(
            "the extension `{stem}` specifies the engine `{NATIVE}` (`extension.engine`), and no extension `{stem}` is \
             compiled into this program; {holds}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guest::Extension;

    fn nothing(_: &mut Extension) {}

    static NOTHING: CompiledExtension = CompiledExtension::new(nothing, "nothing", "1.2.3");

    #[test]
    fn the_digest_holds_the_engine_flm_ext_and_the_crate() {
        let expected = format!("flm-ext native {}\0nothing\x001.2.3\0", env!("CARGO_PKG_VERSION"));
        assert_eq!(digest(&NOTHING), expected.into_bytes());
    }

    #[test]
    fn a_stem_the_engine_does_not_hold_is_refused_with_the_stems_it_holds() {
        let held = |stem| NativeExtension { stem, manifest: "", compiled: &NOTHING };
        assert_eq!(
            NativeEngine::new([held("pdf-latex"), held("other")]).not_held("pdf"),
            "the extension `pdf` specifies the engine `native` (`extension.engine`), and no extension `pdf` is compiled \
             into this program; the extensions compiled into it are `pdf-latex`, `other`"
        );
        assert_eq!(
            NativeEngine::new([]).not_held("pdf"),
            "the extension `pdf` specifies the engine `native` (`extension.engine`), and no extension `pdf` is compiled \
             into this program; no extension is compiled into it"
        );
    }
}
