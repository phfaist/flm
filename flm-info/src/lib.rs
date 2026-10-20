//! Introspection of an FLM environment: a query API over a registry, a plan and the package
//! factories, a typed description of the environment that serializes to JSON, a generator
//! of documentation pages as FLM source, and the engine of the command `flm dump`.
//!
//! FLM is a LaTeX-like markup language. Its library, [`flm_core`], processes a document with
//! a [registry](flm_core::core::registry): the frozen record of every command an author may
//! write, every stage of the processing, every table, setting, workflow and output, each
//! declared by a *feature*. Features come in *packages*, which a document or a program
//! chooses to use. The registry, the [plan](flm_core::core::plan::Plan) that a workflow
//! gives (the stages that run, in order) and the [package
//! factories](flm_core::core::registry::PackageFactory) (each package's features with the
//! declarations of their settings) together make up an *environment*. This crate answers
//! questions about an environment: which packages and features it holds, how each command
//! is written and what its arguments hold, which settings each feature takes, what each
//! workflow produces and how its plan runs, and which names each namespace of the registry
//! holds.
//!
//! # Layers
//!
//! The crate is built in layers, each using the one before it:
//!
//! 1. [`env`](mod@env) holds the queries. An [`Environment`] is made from a registry, an
//!    optional plan and the package factories ([`Environment::new`]); its methods answer typed
//!    values (the *views*: [`env::FeatureView`], [`env::CommandView`],
//!    [`env::WorkflowView`] and the others), which are the primary API of the crate.
//! 2. [`describe`] treats those views as the *description* of the environment: every view
//!    serializes to JSON (`serde::Serialize`) and has a JSON Schema
//!    (`schemars::JsonSchema`); [`describe::schema`] returns the schema of the whole
//!    description. The rules of the JSON form are stated in the [`describe`] module.
//! 3. [`pages`] arranges the description in a tree of pages of documentation: each
//!    [`pages::Page`] has a path (`features/headings/macros/section`), shows one view, and
//!    lists the pages below it; [`pages::source`] writes a page as an FLM document in the
//!    markup of the extension `flmdoc`, for a program to render.
//!
//! Beside these layers, [`dump`] is the engine of the command `flm dump`, which processes
//! one file with the library alone and prints one view of what the run holds (a tree at a
//! chosen point of the processing, a computed table, the files of an output). The command
//! `flm dump` of the `flm` program is the program's view of this engine: the program parses
//! the command line, reads the file and prints what the engine gives it.
//!
//! # The layer over the host library (cargo feature `flm-main`)
//!
//! A document processed by the host library `flm-main` has an environment that its front
//! matter shapes: the packages it uses, the settings it writes, the profile it builds with.
//! Under the cargo feature `flm-main`, `Environment::of_prepared` makes the environment of
//! a document that the host prepared (`flm_main::session::prepare`), and
//! `Environment::of_request` the environment of a request without a document; the module
//! `host` holds what the host adds (`host::HostInfo`: the profile in view, the math
//! processor, where each package comes from with the description, the author and the
//! engine of an extension's manifest, the settings in effect with where they were written,
//! the front matter, the search path), and the views gain fields that only a prepared
//! document gives; the [`pages`] show them, and the page of the keys of a front matter
//! (`settings/front-matter`) exists only under the feature. Without the feature the crate depends on `flm-core` alone and builds for
//! WebAssembly, as `flm-core` does.
//!
//! # The documentation texts
//!
//! The views show the documentation text (`docs`) of each declaration, with its first
//! paragraph as a summary ([`summary_of`]). `flm-core` keeps these texts under its cargo
//! feature `docs`, which is among its default features. This crate depends on `flm-core`
//! without its default features, so that a program chooses them: a program that embeds the
//! crate keeps `flm-core`'s feature `docs` on, through its own dependency on `flm-core`, to
//! see the documentation texts. Without it, the texts of the declarations and their
//! summaries are empty, and the description of a feature's settings as a whole
//! ([`env::SettingsView::description`]) is dropped; the descriptions of the individual
//! settings keys stay, being part of the settings' JSON Schemas.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use flm_core::core::plan::{Overrides, Plan};
//! use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
//! use flm_core::lang::mint::CallableKind;
//! use flm_core::stdcore::{self, StdcoreConfig};
//! use flm_info::Environment;
//! use flm_info::pages::{SourceOptions, Tree, source};
//!
//! let mut builder = RegistryBuilder::new();
//! builder.use_package(stdcore::package(StdcoreConfig::new()));
//! builder.use_package(flm_core::math_passthrough::package());
//! let registry = builder.build(&RegistryInputs::new())?;
//! let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;
//! let factories = vec![stdcore::factory()?, flm_core::math_passthrough::factory()?];
//!
//! let env = Environment::new(registry, Some(Arc::new(plan)), factories);
//!
//! // A query: the declarations of `\section`, and the feature that makes each.
//! let section = env.command(CallableKind::Macro, "section");
//! assert_eq!(section.last().map(|view| view.feature.as_str()), Some("headings"));
//! assert!(env.workflow("text").and_then(|view| view.plan).is_some());
//! // Every view serializes to JSON.
//! let json = serde_json::to_value(&section)?;
//! assert!(json.is_array());
//!
//! // A page, as JSON and as FLM source in the markup of the extension `flmdoc`.
//! let tree = Tree::new(&env, None);
//! let page = tree.page("features/headings/macros/section").expect("the page of `\\section`");
//! assert_eq!(page.to_json(false)["path"], "features/headings/macros/section");
//! let flm = source(&page, &SourceOptions::default());
//! assert!(flm.contains("\\docsignature"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// A crate of its own, above the host library, whose core needs flm-core alone. See decision
// rationale at [§flm-dd-dr:info-crate].

pub mod describe;
pub mod dump;
pub mod env;
#[cfg(feature = "flm-main")]
pub mod host;
pub mod pages;

pub use env::{Environment, summary_of};
