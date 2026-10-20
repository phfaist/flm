//! FLM's extension layer: packages found at run time, written as data, in
//! JavaScript, or in Rust (compiled to WebAssembly, or compiled into the
//! program that loads them), contribute elements, stages, tables, settings
//! and templates to the registry of [`flm_core`]; a package with behavior
//! (JavaScript or Rust) also contributes lowering rules, rewrite stages,
//! presentation elements of its own, and whole backends with their outputs.
//!
//! An *extension* is the unit a host finds and loads: a manifest, and for an
//! extension written as a folder, the files beside it. A *package* is the unit
//! a `use` entry names (`myext`, `myext#svg`); an extension provides one or
//! more. An *engine* turns an extension's files into packages: `data` reads an
//! extension written as data, `js` runs one written in JavaScript (cargo
//! feature `quickjs`), `wasm` runs one compiled to a WebAssembly module (cargo
//! feature `wasm`), and `native` runs one written in Rust and compiled into
//! the program (cargo feature `native`). The crate reads no file: the host
//! gives it *sources*, which find an extension by name and read its files.
//!
//! # Loading extensions
//!
//! - [`source`]: where extensions come from — the host's [`ExtensionSource`]s
//!   (a locator in, an extension out) and each extension's [`ExtensionFiles`],
//!   with the in-memory [`MemorySource`] and [`MemoryFiles`];
//!   [`paths`] is the relative-path arithmetic over an extension's files.
//! - [`manifest`]: the manifest ([`Manifest`]): the `extension:` block
//!   (description, author, required FLM version, engine, the keys of the
//!   engines `js` and `wasm`)
//!   and the data engine's `flm:` block.
//! - [`entry`]: the `use` entry and its grammar ([`UseEntry`]: `LOCATOR` or
//!   `LOCATOR#PACKAGE`, `_` the default package, a locator never a path).
//! - [`block`]: the block step, which reads a parsed `flm` block (a data
//!   extension's, or a document's front matter) into a package's
//!   contributions.
//! - [`engine`]: the engine contract ([`Engine`]) and what an engine reports
//!   ([`Loaded`], [`PackageReport`], [`FeatureReport`]); [`data`] is the data
//!   engine, and [`templates`] reads compose templates from an extension's
//!   files for every engine.
//! - [`host`]: the host services contract ([`host::HostService`]): named
//!   services a host offers on the loader
//!   ([`ExtensionLoader::with_host_service`]), which a package declares in its
//!   `host` list and a handler calls through a handle; a package declaring a
//!   service the host does not offer fails its load. The crate states the
//!   protocol of one service, `files` (a file of the extension's folder or of
//!   the document's folder); a host may offer others (the `flm` program also
//!   offers `programs`, which finds and runs a program installed on the
//!   machine).
//! - [`loader`]: [`ExtensionLoader`], a package resolver of `flm-core` over
//!   the sources and the engines, which a host adds to its package catalog;
//!   it loads each extension once and answers a factory per package, and
//!   gives the settings each package writes ([`ExtensionLoader::blocks`]) for
//!   the host's settings cascade.
//!
//! # The extension API and the adapter
//!
//! - [`api`], the extension API: the part of `flm-core`'s public interface an
//!   extension may use, mirrored item for item (same module paths, same names,
//!   same method names). Objects with methods are object-safe traits; plain
//!   data is the `flm-core` type itself, or a data type of the same name where
//!   the `flm-core` type cannot be built from data.
//! - [`runtime`], the contract a language binding implements: [`ExtRuntime`]
//!   (what FLM calls in an extension) and [`ExtError`] (a failure at the
//!   boundary).
//! - [`adapter`], which implements `flm-core`'s own traits (a feature, a stage,
//!   a rewrite stage, an expansion hook, a lowering rule's body, a table) by
//!   calling the extension, registers a scripted backend with the library's
//!   lowering stage and the extension's emit and final stages, builds a scripted
//!   feature's settings from data, and holds the per-call handle table and the
//!   by-name dispatcher through which a binding whose values cross as JSON
//!   reaches the API. It also implements the aspects an extension answers
//!   through handlers, `Referenceable` and `Sectioning`
//!   ([`adapter::aspects`]); reads the values an extension contributes as
//!   data to another feature's service slots, such as a counter for the
//!   numbering feature or a preamble piece for the LaTeX backends
//!   ([`adapter::slots`]); and keeps, per feature and registry, the durable
//!   table ([`adapter::durable`]): the template languages and compiled
//!   templates a handler may hold beyond the call that made them, each named
//!   by a token valid on every thread for the registry's life.
//!
//! # The QuickJS binding
//!
//! `quickjs` (cargo feature `quickjs`) is the engine `js`: an extension whose
//! main module, written in JavaScript, declares its packages from its default
//! export; the module documentation describes how the API appears in
//! JavaScript, the sandbox, the limits and the engine instances. A loader
//! gains it with `ExtensionLoader::with_js`. The binding's JavaScript modules
//! of the API (the prelude) and the TypeScript declarations `flm.d.ts` are
//! generated from the sources of [`api`], by the crate's test
//! `tests/api_surface.rs`.
//!
//! # The WebAssembly binding, the engine `native` and the guest client
//!
//! `wasm` (cargo feature `wasm`, over the interpreter wasmi) is the engine
//! `wasm`: an extension whose code is one WebAssembly module, which reaches
//! FLM through one import carrying the same requests the JavaScript prelude
//! makes, and answers the same entry points, in JSON text. A loader gains it
//! with `ExtensionLoader::with_wasm`; the module documentation states the
//! interface between the host and the module, the limits (fuel per handler
//! call, memory per instance) and the instances.
//!
//! `guest` (cargo feature `guest`, no dependency) is the guest client: the
//! library an extension written in Rust is compiled against, the mirrored API
//! as Rust types whose methods forward to the host, with the macro
//! `extension!`. A crate written against [`api`] moves to it by changing its
//! module paths and the façade types in its signatures. Its surface is
//! generated from the sources of [`api`] by the same test as the prelude's.
//! One crate written against it builds for `wasm32-unknown-unknown`, as a
//! module of the engine `wasm`, and compiles unchanged into a program, built
//! for any target, that carries the engine `native`.
//!
//! `native` (cargo feature `native`, which turns on `guest`) is the engine
//! `native`: it runs the extensions compiled into the program, each a crate
//! written against the guest client, whose requests it answers in the same
//! process, as JSON values, with no sandbox and no limit. A program lists the
//! extensions it carries for the engine and adds it to a loader with
//! `ExtensionLoader::with_engine`. The feature, not the target, makes the
//! guest client answer requests in the same process, so a program built for
//! `wasm32` carries extensions compiled in too; an extension's own
//! WebAssembly module is therefore built in a cargo run where no crate turns
//! on `native` (the module documentation of `flm_ext::native` states why).
//! The handlers of the three bindings run through one shared host side, so a
//! handler of any of them answers FLM the same way.
//!
//! # Trust
//!
//! An extension runs code. The sandbox of the engine `js` keeps a script
//! away from files, the network and programs, and a module of the engine
//! `wasm` reaches nothing but the host's imports (an extension of the engine
//! `native` is the program's own code, with the program's rights), but the
//! host services a host offers are the way out: through a service that runs programs, such as the
//! `flm` program's `programs`, an extension can run any program on the
//! machine with the rights of the user. Install extensions from trusted
//! sources only. Extensions found beside a document (the `flm` program's
//! folder `_flmextensions`) come with the document: building a document
//! received from someone else runs the extensions in its folder.
//!
//! The crate reaches the parser toolkit only through the library's re-export
//! [`flm_core::techy`].

pub mod adapter;
pub mod api;
#[cfg(any(feature = "quickjs", feature = "wasm", feature = "native"))]
pub(crate) mod binding;
pub mod block;
pub mod data;
pub mod engine;
pub mod entry;
#[cfg(feature = "guest")]
pub mod guest;
pub mod host;
pub mod loader;
pub mod manifest;
#[cfg(feature = "native")]
pub mod native;
pub mod paths;
#[cfg(feature = "quickjs")]
pub mod quickjs;
pub mod runtime;
mod sealed;
pub mod source;
pub mod templates;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use engine::{Engine, ExtensionIdentity, FeatureReport, LoadError, Loaded, PackageReport};
pub use entry::{Locator, UseEntry};
pub use loader::{ExtensionLoader, LoadedBlock};
pub use manifest::Manifest;
pub use runtime::{ExtError, ExtRuntime};
pub use source::{ExtensionFiles, ExtensionSource, FoundExtension, MemoryFiles, MemorySource};
