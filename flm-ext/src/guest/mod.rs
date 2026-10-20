//! The guest client: the library an extension written in Rust is compiled
//! against, as a WebAssembly module or into a program (cargo feature `guest`,
//! no dependency). A crate written against [`flm_ext::api`](crate::api) moves
//! to it mechanically: the module path
//! `flm_ext::api::a::b` becomes `flm_ext::guest::a::b`, a façade type in a
//! signature becomes the struct of the same name (`&dyn StageCx` is
//! `&StageCx`, `&dyn Node<'_>` is `&Node`, `Box<dyn ModeVariant>` is
//! `ModeVariant`), and a call that crosses to the host answers a
//! `Result<_, ExtError>` where the api's answers its value (a `?` more);
//! nothing else changes. The places where the move is not mechanical:
//! [`ElementDef::docs`](lang::mint::ElementDef::docs) and
//! [`ElementDef::modes`](lang::mint::ElementDef::modes) answer a `Result`
//! (on a declaration a helper made they ask the host), and the setters of
//! [`ArgDef`](lang::mint::ArgDef) called on an argument a helper made
//! (`label_fields_arg`) keep the change beside it, which the host refuses
//! when the argument is sent; a stage field of
//! [`BackendDef`](core::registry::BackendDef) changed after
//! `BackendDef::new` is refused when the backend is sent; the
//! record-and-lend constructors
//! [`LoweringRule::new`](core::lowering::LoweringRule::new),
//! [`BackendDef::new`](core::registry::BackendDef::new) and
//! [`UserDefinition::element_def_with`](lang::rewrites::userdefs::UserDefinition::element_def_with)
//! answer a `Result` too.
//!
//! # What the guest client is
//!
//! The guest client is the library an extension written in Rust is compiled
//! against: the mirrored API (one module per module of the extension API, the
//! struct of each façade forwarding every method to the host as a request),
//! the runtime (what each feature's `register` recorded, the rehearsal, the
//! entries), the requests and the module's side of the ABI ([`abi`]), and the
//! macro [`extension!`]. It is the twin of the JavaScript prelude, item for
//! item. One extension crate written against it has two forms, and its source
//! is the same for both:
//!
//! - **A WebAssembly module.** Compiled for `wasm32-unknown-unknown` in a
//!   build where `flm-ext`'s cargo feature `native` is off, the crate is a
//!   module that the engine `wasm` runs (the module `flm_ext::wasm`, cargo
//!   feature `wasm`). The module sees FLM through one import that takes a
//!   request as JSON text, and answers the engine's entries in JSON text.
//! - **Compiled into a program.** A program that depends on the crate and
//!   turns on `flm-ext`'s cargo feature `native` runs it with the engine
//!   `native` (the module `flm_ext::native`), reaching it through the static
//!   the macro writes ([`CompiledExtension`]). The program may be built for
//!   any target, `wasm32` included (a program that runs the library in a web
//!   browser, for instance). Each request is then answered in the same
//!   process by the host side every binding shares, passed as a JSON value,
//!   with no text.
//!
//! `flm-ext` picks the form, and with it how requests reach the host, from its
//! own cargo feature `native`, not from the target alone; the extension's
//! crate turns on the feature `guest` alone. Within one build, cargo compiles
//! `flm-ext` once, with one set of features, for every crate of the build. So
//! an extension's WebAssembly module is built in a cargo run where no crate
//! turns on `native`: the extension's package selected alone (`cargo rustc -p
//! NAME --target wasm32-unknown-unknown …`). Built in a run where some crate
//! turns on `native` (a workspace build that includes a program carrying the
//! engine, say), the module lacks the four exports, and the engine `wasm`
//! refuses it when it loads, with a message saying which export it lacks.
//!
//! Where the module's import is not compiled (in a program built for another
//! target, or with the feature `native`), a request is answered while a native
//! instance's state is current on the thread: during one of its entries, and
//! while the engine `native` drops the objects it recorded. Every other
//! request answers [`ExtError::Unsupported`](crate::ExtError::Unsupported): in
//! the client's own tests, on a thread the extension started, and while a
//! thread ends.
//!
//! # An extension in Rust
//!
//! An extension is a crate depending on `flm-ext` with the feature `guest`
//! alone (no default features). Its WebAssembly form is a library of the type
//! `cdylib` compiled for `wasm32-unknown-unknown` in release, with
//! `panic = "abort"` in its release profile (`cargo build --target
//! wasm32-unknown-unknown --release`, which writes
//! `target/wasm32-unknown-unknown/release/NAME.wasm`, or `cargo rustc
//! --crate-type cdylib` with the same target for a crate whose library keeps
//! the default type), whose module the manifest specifies
//! (`extension.wasm.module`; the module `flm_ext::wasm` states the manifest).
//! Its form compiled into a program is the same crate as a Rust library the
//! program depends on (the module `flm_ext::native`, cargo feature `native`).
//! Its root invokes the macro once, with the function that declares its
//! packages:
//!
//! ```
//! use std::sync::Arc;
//!
//! use flm_ext::guest::core::data::store::StageOut;
//! use flm_ext::guest::core::registry::FeatureBuilder;
//! use flm_ext::guest::core::run::StageCx;
//! use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
//! use flm_ext::guest::{Extension, Feature, Package};
//! use flm_ext::ExtError;
//! use serde_json::Value;
//!
//! flm_ext::guest::extension!(extension);
//!
//! fn extension(ext: &mut Extension) {
//!     ext.package("_", Package::new().uses(["stdcore"]).feature(
//!         "wordcount",
//!         Feature::new().title("Word count").register(register),
//!     ));
//! }
//!
//! fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
//!     b.stage(Arc::new(Count))
//! }
//!
//! struct Count;
//!
//! impl Stage for Count {
//!     fn decl(&self) -> StageDecl {
//!         StageDecl::new("wordcount.count", Effect::Compute, Scope::PerTree)
//!     }
//!     fn needs(&self) -> Needs {
//!         Needs::new()
//!     }
//!     fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
//!         for (id, tree) in cx.trees()? {
//!             let _ = (id, tree, &out);
//!         }
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # The packages and the features
//!
//! [`Extension::package`] declares a package, as the JavaScript prelude's
//! `ext.package(name, object)` does: [`Package`] holds its `uses`, its `flm`
//! block, its `templates`, its `host` services and its features in order;
//! [`Feature`] a feature's `title`, `docs`, `version`, `settings` and its
//! `register`, a function or a closure taking the builder and the feature's
//! settings. A feature without a `register` is refused when the module
//! loads.
//!
//! # The mapping rules
//!
//! How the extension API of [`crate::api`] appears in the guest.
//!
//! 1. **Module paths.** The module `flm_ext::api::a::b` is
//!    `flm_ext::guest::a::b`, the twin of the JavaScript module `flm:a/b`,
//!    and every item keeps its name.
//! 2. **A struct or enum of plain data** (`StageDecl`, `Needs`, `BodyDef`,
//!    `RewriteDecl`, `OutputDef`, `ArtifactId`, …) is the api's own type,
//!    re-exported; it crosses in its JSON form, the host's. A type whose
//!    JSON form can hold a reference to a host object or a handler object
//!    is the guest's own, with the api's name, constructors and methods:
//!    `ArgDef`, `SyntaxDef`, `ElementDef`, `ElementSpec`, `Aspect`,
//!    `FallbackDef`, `ScopedPackageDef`, `Definition`, `BackendDef`,
//!    `BackendStage`, `Contributed`, `Diagnostic`, `Expansion`, `Lowering`,
//!    `Restage`, `PresTree`, `RefPoint`, `ContentRef`, `CitationAnswer`, ….
//!    A declaration a helper made (`ud.element_def(owner, name)`) is an
//!    `ElementDef` holding the host's object: its accessors `owner()`,
//!    `names()`, `syntax()` and `element()` answer `None`.
//! 3. **An enum of unit variants** is the api's, re-exported.
//! 4. **A reference to an object with methods** (`&dyn FeatureBuilder`,
//!    `&dyn StageCx`, `&mut dyn StageOut`, `&dyn Node<'_>`, `Box<dyn
//!    UserDefinition>`, …) is the struct of that name, holding a handle and
//!    nothing else; every method of the api's trait is a method of the
//!    struct, with the same name and arguments, forwarding the call to the
//!    host and answering a `Result`. A handle is valid for the handler call
//!    that received it, while that call is the innermost one running; kept
//!    and used later, every method answers [`ExtError::StaleHandle`](crate::ExtError::StaleHandle).
//!    **A durable object** (a `TemplateLanguage`, a compiled template that
//!    `compile_template_with` answers) is valid in every later call of the
//!    feature that made it, on every instance, for as long as its registry is
//!    in use, as in the JavaScript binding.
//! 5. **A method generic over a type** takes the registered name:
//!    `cx.get("flm.blocks.structured")`, answering a `TableValue`.
//! 6. **A free function** is a function of its module, forwarding:
//!    `chars_text(&nodes)?`, `element_name_under("wc", &definition)?`.
//! 7. **A trait an extension implements** (`Stage`, `RewriteStage`,
//!    `Expand`, `Lower`, `Referenceable`, `Sectioning`, `CitationProvider`,
//!    `TemplateModule`, `LateInit`, `Finalizer`) is the guest's trait of that
//!    name, with the api's methods, defaults and bounds in the guest's
//!    types; an object is passed as the api passes it, an `Arc<dyn Trait>`:
//!    `b.stage(Arc::new(Count))?`, `ElementSpec::new("wc.badge").with(Aspect::Expand(hook))?`,
//!    `ud.element_def_with(owner, name, hook)?`,
//!    `LoweringRule::new(name, element, decl, Arc::new(body), mark)?`. The
//!    method that takes the object records it in the running `register`'s
//!    record and lends it to the host, which calls it back through the
//!    module's entries. A `Sectioning` is lent with its three methods: the
//!    host calls `in_toc` and `toc_title` every time, and an object that
//!    keeps their defaults answers them from the guest; likewise
//!    `TemplateModule::files` and `CitationProvider::answer_all` always run
//!    in the guest, their defaults there.
//! 8. **`Result<T, E>`** is `Result<T, ExtError>`: an error the host
//!    answers is the [`ExtError`](crate::ExtError) its data describe, as a
//!    compiled-in extension would get it; a handler's own error crosses back
//!    as it was returned.
//! 9. **`Option<T>`** is `Option<T>`; a `Vec<T>` and a tuple as in the api;
//!    a borrowed result is answered owned (`&str` is `String`, `&[T]` is
//!    `Vec<T>`).
//! 10. **Values are JSON.** A value of the api's `serde_json::Value` is the
//!     same; a number must be an integer where the api takes one; bytes
//!     given as a text (`b.digest_input(bytes)`) must be UTF-8.
//! 11. **A diagnostic** a handler reports is the guest's `Diagnostic`, made
//!     with its constructors (`Diagnostic::note`, `Diagnostic::error`); its
//!     `id` must begin with the extension's stem and a dot (or with `X` and a
//!     dot, for an extension whose stem is `profile-X`).
//! 12. **A `const` name** is the api's constant, re-exported.
//!
//! # The API modules
//!
//! One module per module of the extension API, generated from `src/api/`
//! with the JavaScript prelude's modules (`core`, `host`, `lang`, `model`,
//! `stdcore`, `techy`, and their submodules); the hand-written types and
//! traits are re-exported from their module. The guest's own types (rule 2)
//! are written by hand, each checked against the host's reading and writing
//! of the same JSON form by a test that converts in both directions.
//!
//! # The limits
//!
//! The guest client adds none. In a WebAssembly module, the engine `wasm`
//! sets a fuel limit per handler call and a memory limit per instance (the
//! module `flm_ext::wasm`). Compiled into a program, the engine `native` sets
//! none: no fuel, time or memory budget, and no bound on how deeply a value
//! nests (the engines `js` and `wasm` refuse a value nested deeper than 127
//! levels when they read JSON text; the engine `native` reads none).
//!
//! # The rehearsal
//!
//! The objects a feature's `register` hands to FLM live in the instance
//! that made them. Each record-and-lend method records its object in the
//! running `register`'s record: a stage and a rewrite stage under its name,
//! an expansion hook and a handler aspect under its element, a rule's body
//! under the rule's name, a citation provider and a template module under
//! its name, a late-initialization hook and a finalizer by its position.
//! When an instance that has not run a feature's `register` must run one of
//! its handlers, the host first calls the entry `rehearse`: `register` runs
//! again with a builder that sends nothing (its methods record and answer
//! `Ok(())`), then each finalizer `register` recorded, likewise; the
//! instance then finds each object again. `ElementSpec::with`,
//! `UserDefinition::element_def_with` and `LoweringRule::new` record and
//! lend in every run, a rehearsal's too. A feature's `register` must
//! declare the same things under the same names, in the same order, every
//! time it runs with one configuration. A template module's `describe` runs
//! inside the `register` that contributes it, which the module is found in.
//!
//! # Failures
//!
//! An `Err` a handler returns crosses back as the [`ExtError`](crate::ExtError)
//! it is. An entry that finds no object answers the refusal the JavaScript
//! prelude's entries answer ("the stage `wc.count` must be declared by
//! `register` of the feature `wordcount` of the package `_` of the extension
//! `wc`"). In an extension's own WebAssembly module, a panic aborts the module
//! (`panic = "abort"`): the host reports a trap, with the panic's message,
//! which the panic hook the macro's entry installs keeps for
//! `flm_last_panic`. Compiled into a program that unwinds on a panic, a panic
//! during an entry unwinds to the engine `native`, which fails the handler
//! call with the panic's message and discards the thread's instance of the
//! extension; a panic in the `Drop` of an object the instance recorded, while
//! the engine drops it, fails no handler call (the module `flm_ext::native`).
//! A program built with `panic = "abort"` (the strategy of the target
//! `wasm32-unknown-unknown`) stops at the panic.
//!
//! # The digest
//!
//! The engine's: the engine `wasm` enters the module's bytes into the
//! registry digest; the engine `native` enters the extension crate's name and
//! version. The guest client adds nothing.
//!
//! # `unsafe`
//!
//! The guest's `unsafe` is at two places, both compiled only for `wasm32` in a
//! build without the feature `native` (the crate a WebAssembly module of its
//! own): the import block of [`abi`], and the attribute `#[unsafe(no_mangle)]`
//! on the four exports [`extension!`] writes in the author's crate, each a
//! call of a safe function of [`abi`]. Compiled for another target, or with
//! the feature `native`, the guest client has no `unsafe`.

pub mod abi;
pub(crate) mod entry;
mod generated;
mod hand;
mod registry;
pub(crate) mod state;

pub use entry::{Extension, Feature, Package};
pub use generated::*;

/// What a program that compiles an extension crate in needs to run the
/// extension: the function that declares its packages, and the crate's name
/// and version. The macro [`extension!`] writes one, the static
/// `FLM_EXTENSION` at the root of the extension's crate; a program that
/// carries the crate gives `&my_extension::FLM_EXTENSION` to the engine
/// `native` (cargo feature `native`), which runs the extension through the
/// guest client in the same process. The type exists with the feature `guest`
/// alone, the one an extension's crate turns on.
#[derive(Debug, Clone, Copy)]
pub struct CompiledExtension {
    /// The function that declares the extension's packages.
    define: fn(&mut Extension),
    /// The crate's name, `CARGO_PKG_NAME`.
    name: &'static str,
    /// The crate's version, `CARGO_PKG_VERSION`.
    version: &'static str,
}

impl CompiledExtension {
    /// The extension whose packages `define` declares, from the crate `name`
    /// at the version `version`. The macro [`extension!`] calls it with the
    /// crate's own name and version.
    pub const fn new(define: fn(&mut Extension), name: &'static str, version: &'static str) -> CompiledExtension {
        CompiledExtension { define, name, version }
    }

    /// The crate's name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The crate's version.
    pub fn version(&self) -> &'static str {
        self.version
    }

    /// The function that declares the extension's packages.
    pub fn define(&self) -> fn(&mut Extension) {
        self.define
    }
}

/// The extension's entry points, routing the entry `define` to `$define`, a
/// function `fn(&mut Extension)` that declares the extension's packages
/// ([`Extension::package`]); invoked once, at the root of the extension's
/// crate: `flm_ext::guest::extension!(extension);`.
///
/// It writes the public static `FLM_EXTENSION`, a [`CompiledExtension`]
/// holding `$define` and the crate's name and version, through which a
/// program that compiles the crate in reaches it (the engine `native`). The
/// name is the same in every extension's crate; a static has no exported
/// symbol, so several extension crates link into one program.
///
/// Compiled for `wasm32` in a build where `flm-ext`'s cargo feature `native`
/// is off, the crate is a WebAssembly module of its own, and the macro also
/// writes the four exports of the ABI ([`abi`]), each a call of a safe
/// function there: `flm_alloc` ([`abi::alloc`]), `flm_free` ([`abi::free`]),
/// `flm_entry` ([`abi::entry`], whose first call installs the panic hook that
/// keeps a panic's message) and `flm_last_panic` ([`abi::last_panic`]). The
/// module exports its memory as `memory`, as every Rust module for
/// `wasm32-unknown-unknown` does. Compiled for another target, or in a build
/// where the feature `native` is on (the build of a program that carries the
/// engine `native`, for any target, `wasm32` included), it writes no export:
/// two extension crates linked into one program would clash. Within one
/// build, cargo compiles `flm-ext` once, with one set of features, so a
/// module built in a cargo run where any crate turns on `native` lacks the
/// exports, and the engine `wasm` refuses it when it loads, saying which
/// export it lacks.
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_guest_extension {
    // One output on every target: the static for a program that compiles the crate in,
    // the exports for a WebAssembly module of its own. See decision rationale at
    // [§flm-dd-dr:ext-native-engine]
    ($define:expr) => {
        /// What a program that compiles this crate in needs to run the
        /// extension: the function that declares its packages, and the
        /// crate's name and version.
        pub static FLM_EXTENSION: $crate::guest::CompiledExtension =
            $crate::guest::CompiledExtension::new($define, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

        $crate::__flm_guest_exports!($define);
    };
}

/// The four exports of the guest's ABI for [`extension!`], in `flm-ext`'s
/// form without the cargo feature `native`: compiled for `wasm32`, the
/// exports; for another target, nothing. A `cfg` written in the body of a
/// `macro_rules!` macro is read in the crate where the macro expands (the
/// extension's crate, which does not turn on `native`), so `flm-ext` chooses
/// by its own feature which form of this macro it defines.
#[cfg(not(feature = "native"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_guest_exports {
    // The feature `native`, not the target alone, is the sign of a program that carries the
    // engine. See decision rationale at [§flm-dd-dr:ext-native-engine]
    ($define:expr) => {
        /// The export `flm_alloc` of the guest's ABI: a buffer of `len`
        /// bytes for the host to write.
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn flm_alloc(len: i32) -> i32 {
            $crate::guest::abi::alloc(len)
        }

        /// The export `flm_free` of the guest's ABI: drops the buffer at
        /// `ptr`.
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn flm_free(ptr: i32, len: i32) {
            $crate::guest::abi::free(ptr, len)
        }

        /// The export `flm_entry` of the guest's ABI: runs the entry whose
        /// request is at `ptr`.
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn flm_entry(ptr: i32, len: i32) -> i64 {
            $crate::guest::abi::entry(ptr, len, $define)
        }

        /// The export `flm_last_panic` of the guest's ABI: the message of
        /// the last panic.
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn flm_last_panic() -> i64 {
            $crate::guest::abi::last_panic()
        }
    };
}

/// The four exports of the guest's ABI for [`extension!`], in `flm-ext`'s
/// form with the cargo feature `native`: nothing, on every target, since the
/// crate is compiled into a program that carries the engine `native` (see the
/// form without the feature).
#[cfg(feature = "native")]
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_guest_exports {
    ($define:expr) => {};
}

#[doc(inline)]
pub use crate::__flm_guest_extension as extension;

#[cfg(test)]
mod tests {
    /// The module is compiled in under the feature, on the host; the guest's
    /// target is checked by building the crate for it.
    #[test]
    fn the_module_builds_on_the_host() {
        assert_eq!(module_path!(), "flm_ext::guest::tests");
    }
}
