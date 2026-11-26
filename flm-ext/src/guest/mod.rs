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
//! (on a declaration a helper made they ask the host), and the record-and-lend
//! constructors
//! [`LoweringRule::new`](core::lowering::LoweringRule::new) and
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
//! macros [`extension!`], for a crate that is one extension, and
//! [`extensions!`], for a module that holds the extensions of a table of code
//! ([`NamedExtension`]). It is the twin of the JavaScript prelude, item for
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
//!   browser, for instance). Every crossing is then a direct, typed call in
//!   the same process, with no JSON value built or read (the typed route,
//!   [`abi`]): a request of a façade's method or a module's function calls
//!   the extension API on the handle table of the running handler call, the
//!   guest's values converted into the API's and the answer back; the
//!   lending of a handler object builds its host object with the body of the
//!   lending native every binding shares and hands it to the API method that
//!   takes it; a line of console output goes to the log sink the engine
//!   gave the instance; and the engine calls the entries directly, their
//!   arguments and replies the guest's values. Only the data that is JSON by nature
//!   stays JSON: a feature's settings, which its `register` and its
//!   template modules' `describe` receive as a `serde_json::Value`, and the
//!   declarations of the extension's packages, which every engine reads as
//!   data.
//!
//! The two forms run the same source and reach the same code of the host
//! (the handle tables, the objects that call the extension's handlers, the
//! bodies of the lending natives); they differ only in how each crossing is
//! carried. The guest client alone chooses the form, from `flm-ext`'s own
//! cargo feature `native`, not from the target alone; the extension's crate
//! turns on the feature `guest` alone. Within one build, cargo compiles
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
//! while the engine `native` drops the objects it recorded. A request sent
//! while no handler call of the extension is the innermost one (from
//! `define`, from the `Drop` of a recorded object) is refused in the words
//! the host uses for the same request sent as JSON text from a module. Every
//! other request answers [`ExtError::Unsupported`](crate::ExtError::Unsupported):
//! in the client's own tests, on a thread the extension started, and while a
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
//!     ext.package("_", Package::new().feature(
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
//!         for pair in cx.trees()? {
//!             let (id, tree) = pair?;
//!             let _ = (id, tree, &out);
//!         }
//!         Ok(())
//!     }
//! }
//! ```
//!
//! A crate can also hold several extensions, each in a module of its own. Each
//! module declares the code of its extension as a static [`CompiledExtension`],
//! made with [`CompiledExtension::new`] from its declaring function, its name
//! and its version. The crate's root lists the extensions in a *table of code*,
//! a static slice of [`NamedExtension`] rows (a stem and its code), and invokes
//! [`extensions!`] once over the table, in place of [`extension!`]:
//!
//! ```ignore
//! pub static COMPILED_EXTENSIONS: &[flm_ext::guest::NamedExtension] = &[
//!     flm_ext::guest::NamedExtension { stem: "todo", compiled: &todo::FLM_EXTENSION },
//!     flm_ext::guest::NamedExtension { stem: "wordcount", compiled: &wordcount::FLM_EXTENSION },
//! ];
//!
//! flm_ext::guest::extensions!(COMPILED_EXTENSIONS);
//! ```
//!
//! A program that compiles the crate in gives the table to the engine `native`.
//! Built as a WebAssembly module, the crate runs, in one instance, every
//! extension that the host defines in it: each entry names its extension by
//! its stem ([`abi`]). Such a module takes one of two forms. Named by the
//! manifest of an extension's folder, it holds code alone, and the extension's
//! files stand beside it in the folder. As a *collection*, it also holds the
//! files of its extensions in its custom section `flmextensions`
//! ([`crate::collection`]), and a program loads it itself. The crate
//! `flm-extensions` holds the extensions shipped with FLM in this way.
//!
//! # The packages and the features
//!
//! [`Extension::package`] declares a package, as the JavaScript prelude's
//! `ext.package(name, object)` does: [`Package`] holds its `uses`, its `flm`
//! block, its `templates`, its `host` services, its `caches` and its
//! features in order;
//! [`Feature`] a feature's `title`, `docs`, `version`, `settings`, its
//! optional configuration reader ([`Feature::read`], the JavaScript
//! feature object's `read`) and its `register`, a function or a closure
//! taking the builder and the feature's settings. A feature without a
//! `register` is refused when the module loads.
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
//!    re-exported; compiled into a program it crosses as itself, and in a
//!    WebAssembly module in its JSON form, the host's. A type that can hold
//!    a reference to a host object or a handler object is the guest's own,
//!    with the api's name, constructors and methods:
//!    `ElementDef`, `ElementSpec`, `Aspect`,
//!    `FallbackDef`, `ScopedPackageDef`, `BackendDef`,
//!    `BackendStage`, `Contributed`, `Diagnostic`, `Expansion`, `Lowering`,
//!    `Restage`, `PresTree`, `RefPoint`, `ContentRef`, `CitationAnswer`,
//!    `ExtraValues`, ….
//!    A declaration a helper made (`ud.element_def(owner, name)`) is an
//!    `ElementDef` holding the host's object: its accessors `owner()`,
//!    `names()`, `syntax()` and `element()` answer `None`. An argument,
//!    [`ArgDef`](lang::mint::ArgDef), is the guest's own too, data like
//!    every argument the host gives it (`label_fields_arg` answers one):
//!    `ArgDef::new` and `ArgDef::named` take a standard kind
//!    (`ArgStdKind::CharsGroup`), where the api's take a parser: a standard
//!    kind is passed as in the api, and a parser of a feature's own does not
//!    cross. `ArgDef::from_code` and `ArgDef::named_from_code` read an
//!    argument code (`"r()"`) where the argument is made, with `flm-core`'s
//!    reader and no request to the host, so that `define` reads one too;
//!    they stand for the api's `arg_def_from_code`, which the guest does not
//!    forward. The guest adds [`ArgListData`](lang::mint::ArgListData),
//!    the mirror of `flm-core`'s argument list written as data, which no
//!    item of the api names, for an extension's settings that declare
//!    arguments. The types that hold arguments are the
//!    guest's own with `ArgDef`: [`ArgStdKind`](lang::mint::ArgStdKind), whose
//!    tack-on argument's fields ([`TackOnField`](lang::mint::TackOnField))
//!    hold arguments, [`SyntaxDef`](lang::mint::SyntaxDef) and
//!    [`Definition`](lang::rewrites::userdefs::Definition).
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
//!    in use, as in the JavaScript binding; a compiled template's
//!    `placeholders` is asked of the host on its durable token.
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
//!    extension's entries. A `Sectioning` is lent with its three methods: the
//!    host calls `in_toc` and `toc_title` every time, and an object that
//!    keeps their defaults answers them from the guest; likewise
//!    `TemplateModule::files` and `CitationProvider::answer_all` always run
//!    in the guest, their defaults there.
//! 8. **`Result<T, E>`** is `Result<T, ExtError>`: an error the host
//!    answers is the [`ExtError`](crate::ExtError) the api answers (in a
//!    WebAssembly module, read back from the data the host writes); a
//!    handler's own error crosses back as it was returned.
//! 9. **`Option<T>`** is `Option<T>`; a `Vec<T>` and a tuple as in the api;
//!    a borrowed result is answered owned (`&str` is `String`, `&[T]` is
//!    `Vec<T>`). Two kinds of struct stand for the collections whose length
//!    can grow with the document; each holds one handle, whose items the
//!    struct pulls from the host in groups:
//!    - A traversal of the api (`Walk`, `Descendants`, `Trees`) is a struct
//!      of the same name that implements `Iterator`, iterated once. Each item
//!      is a `Result`, since a pull crosses to the host and can fail.
//!      `Walk` and `Descendants` have `elements(names)` and `kinds(kinds)`,
//!      which return the traversal restricted, on the host, to the nodes of
//!      the named elements or of the given kinds:
//!      `root.descendants()?.kinds(&[NodeKindName::Chars])?`. A node that does
//!      not match costs no crossing.
//!    - A list of nodes of the api (a node's `children()`, a `body()`, a
//!      line's `content`, …) is [`Nodes`](crate::guest::techy::node::Nodes),
//!      and a list of staged nodes is
//!      [`StagedNodes`](crate::guest::lang::rewrites::userdefs::StagedNodes):
//!      a *sequence*, with `len()` and `is_empty()` (no crossing), `get(i)`
//!      and `iter()` (which cross). `to_vec()` pulls every item into a `Vec`,
//!      one value per item. The rows of an extension's table are `Rows`,
//!      a sequence with `len()`, `is_empty()`, `get(i)` and `iter()`.
//!
//!    What takes several nodes takes `&Nodes`, and what takes several
//!    staged nodes takes a `StagedNodes` (`Lowerer::pres`, `Lowering::Emit`,
//!    `PresTree::Build`, …): a sequence the host answered, or one the
//!    extension built, empty (`Default`) or from a `Vec`, and extended with
//!    `push` (one item) and `push_all` (another sequence) without any
//!    crossing. A sequence the host answered crosses back as its handle,
//!    whatever its length, so content passed on this way is never pulled.
//! 10. **Values are JSON.** A value of the api's `serde_json::Value` is the
//!     same. Compiled into a program, it is passed as it is; in a
//!     WebAssembly module, a number that is not an integer and a reserved
//!     map key (one beginning with `$`, for instance) are refused in a
//!     call's arguments, in its answer and in a template module's data. In
//!     both forms the api itself refuses them where it keeps the value as
//!     data: an item of an extension's table, a presentation node's data
//!     payload, a diagnostic's data, the data contributed to a service slot.
//!     Bytes (`b.digest_input(bytes)`) cross as bytes of any kind: as they
//!     are compiled into a program, and in a WebAssembly module in two of
//!     the forms of an artifact's content, a text when they are UTF-8,
//!     `{"base64": text}` otherwise.
//! 11. **A diagnostic** a handler reports is the guest's `Diagnostic`, made
//!     with its constructors (`Diagnostic::note`, `Diagnostic::error`), under
//!     any identifier, a library condition's included.
//! 12. **A `const` name** is the api's constant, re-exported.
//!
//! # The API modules
//!
//! One module per module of the extension API, generated from `src/api/`
//! with the JavaScript prelude's modules (`core`, `host`, `lang`, `model`,
//! `stdlib`, `techy`, and their submodules); the hand-written types and
//! traits are re-exported from their module. The guest's own types (rule 2)
//! are written by hand, each checked against the host's reading and writing
//! of the same JSON form by a test that converts in both directions; beside
//! its JSON form, each has the conversions of the typed route (the cargo
//! feature `native`), of the same shape: the same fields, the same handles
//! and durable tokens resolved.
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
//! registry digest; the engine `native` enters the extension's name and
//! version ([`CompiledExtension`]). The guest client adds nothing.
//!
//! # `unsafe`
//!
//! The guest's `unsafe` is at two places, both compiled only for `wasm32` in a
//! build without the feature `native` (the crate a WebAssembly module of its
//! own): the import block of [`abi`], and the attribute `#[unsafe(no_mangle)]`
//! on the four exports [`extensions!`] writes in the author's crate, each a
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
/// extension: the function that declares its packages, and the extension's
/// name and version. The macro [`extension!`] writes one, the static
/// `FLM_EXTENSION` at the root of a crate of one extension, named after the
/// crate; a crate of several extensions writes one per extension, each
/// calling [`CompiledExtension::new`] with the extension's own name. A program
/// that carries the crate gives the static, in a [`NamedExtension`] row such as
/// `NamedExtension { stem: "my-extension", compiled: &my_extension::FLM_EXTENSION }`,
/// to the engine `native` (cargo feature `native`), which runs the extension
/// through the guest client in the same process. The type exists with the
/// feature `guest` alone, the one an extension's crate turns on.
#[derive(Debug, Clone, Copy)]
pub struct CompiledExtension {
    /// The function that declares the extension's packages.
    define: fn(&mut Extension),
    /// The extension's name: the crate's name, `CARGO_PKG_NAME`, for a crate
    /// of one extension; the name the crate gives, for a crate of several.
    name: &'static str,
    /// The crate's version, `CARGO_PKG_VERSION`.
    version: &'static str,
}

impl CompiledExtension {
    /// The extension `name` at the version `version`, whose packages `define`
    /// declares. The macro [`extension!`] calls it with the crate's own name
    /// and version; a crate of several extensions calls it once per
    /// extension, with the extension's name and the crate's version.
    pub const fn new(define: fn(&mut Extension), name: &'static str, version: &'static str) -> CompiledExtension {
        CompiledExtension { define, name, version }
    }

    /// The extension's name.
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

/// One row of a table of code: an extension written in Rust that a program or
/// a module carries, by its stem, with its code. A table of code is a static
/// slice of rows, `&'static [NamedExtension]`: the engine `native` takes rows
/// of this type (cargo feature `native`), and the macro [`extensions!`] writes
/// the exports of a module that holds the extensions of a table. The files of
/// the extension, its manifest among them, are a row of a table of files
/// ([`crate::source::BundledExtension`]), so that no file is linked into a
/// module's memory. The type exists with the feature `guest` alone.
#[derive(Debug, Clone, Copy)]
pub struct NamedExtension {
    /// The extension's stem, the name a `use` entry gives it (`pdf-latex`).
    pub stem: &'static str,
    /// The extension's code: `&my_extension::FLM_EXTENSION`.
    pub compiled: &'static CompiledExtension,
}

/// The extension's entry points, routing the entry `define` to `$define`, a
/// function `fn(&mut Extension)` that declares the extension's packages
/// ([`Extension::package`]); invoked once, at the root of a crate that is one
/// extension: `flm_ext::guest::extension!(extension);`.
///
/// It writes the public static `FLM_EXTENSION`, a [`CompiledExtension`]
/// holding `$define` and the crate's name and version, through which a
/// program that compiles the crate in reaches it (the engine `native`). The
/// name is the same in every extension's crate; a static has no exported
/// symbol, so several extension crates link into one program. It then writes
/// what [`extensions!`] writes over the table of that one row, whose stem is
/// the crate's name; a module of one row runs that row for any stem, since
/// the folder that holds the module can be renamed.
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_guest_extension {
    ($define:expr) => {
        /// What a program that compiles this crate in needs to run the
        /// extension: the function that declares its packages, and the
        /// crate's name and version.
        pub static FLM_EXTENSION: $crate::guest::CompiledExtension =
            $crate::guest::CompiledExtension::new($define, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

        $crate::__flm_guest_extensions!(&[$crate::guest::NamedExtension {
            stem: env!("CARGO_PKG_NAME"),
            compiled: &FLM_EXTENSION,
        }]);
    };
}

/// The entry points of a module that holds the extensions of a table of code,
/// `$extensions` being an expression of the type `&[NamedExtension]`
/// ([`NamedExtension`]); invoked once, at the root of the crate:
/// `flm_ext::guest::extensions!(COMPILED_EXTENSIONS);`.
///
/// The entry `define(stem, module)` runs the declaring function of the row
/// whose stem is `stem`. A table of one row runs that row for any stem. A
/// table of several rows that holds no row of the stem answers an error
/// that gives the stems it holds. Every entry names the extension it is for,
/// by the stem `define` was given, and runs with that extension's own state
/// (what `define` declared, what each feature's `register` recorded), so that
/// one instance of the module runs several extensions.
///
/// Compiled for `wasm32` in a build where `flm-ext`'s cargo feature `native`
/// is off, the crate is a WebAssembly module of its own, and the macro
/// writes the four exports of the ABI ([`abi`]), each a call of a safe
/// function there: `flm_alloc` ([`abi::alloc`]), `flm_free` ([`abi::free`]),
/// `flm_entry` ([`abi::entry`], whose first call installs the panic hook that
/// keeps a panic's message) and `flm_last_panic` ([`abi::last_panic`]). The
/// module exports its memory as `memory`, as every Rust module for
/// `wasm32-unknown-unknown` does. Compiled for another target, or in a build
/// where the feature `native` is on (the build of a program that carries the
/// engine `native`, for any target, `wasm32` included), it writes nothing:
/// two extension crates linked into one program would clash. Within one
/// build, cargo compiles `flm-ext` once, with one set of features, so a
/// module built in a cargo run where any crate turns on `native` lacks the
/// exports, and the engine `wasm` refuses it when it loads, saying which
/// export it lacks. Two invocations in one module, or this macro beside
/// [`extension!`], would write each export twice.
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_guest_extensions {
    ($extensions:expr) => {
        $crate::__flm_guest_exports!($extensions);
    };
}

/// The four exports of the guest's ABI for [`extensions!`], in `flm-ext`'s
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
    // engine: a program built for `wasm32` may carry several extensions compiled in, whose
    // exports would clash.
    ($extensions:expr) => {
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
            $crate::guest::abi::entry(ptr, len, $extensions)
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

/// The four exports of the guest's ABI for [`extensions!`], in `flm-ext`'s
/// form with the cargo feature `native`: nothing, on every target, since the
/// crate is compiled into a program that carries the engine `native` (see the
/// form without the feature).
#[cfg(feature = "native")]
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_guest_exports {
    ($extensions:expr) => {};
}

#[doc(inline)]
pub use crate::__flm_guest_extension as extension;
#[doc(inline)]
pub use crate::__flm_guest_extensions as extensions;

#[cfg(test)]
mod tests {
    /// The module is compiled in under the feature, on the host; the guest's
    /// target is checked by building the crate for it.
    #[test]
    fn the_module_builds_on_the_host() {
        assert_eq!(module_path!(), "flm_ext::guest::tests");
    }
}
