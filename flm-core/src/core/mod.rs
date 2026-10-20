//! The processing framework of the library: the values a feature declares
//! itself with, and the machinery that turns those declarations into a
//! registry, a plan and a run over a document.
//!
//! Where [`crate::model`] describes what a document is made of, this module
//! describes what the library does with one. A feature
//! ([`FlmFeature`](registry::FlmFeature)) is a Rust value that contributes a
//! related bundle of things to a registry (elements, stages, tables, lowering
//! rules, backends, services, settings) through the feature builder
//! ([`FeatureBuilder`](registry::FeatureBuilder)). A stage is a unit of work, a
//! table is typed data that a stage writes, and a capability is a named state
//! of processing. A feature can also declare outputs (named results a caller
//! asks for, [`OutputDef`](registry::OutputDef)). The [registry] records and
//! checks those declarations once; a [plan](plan::Plan) chooses the stages
//! that satisfy what a caller asks for; a [run](run::Run) executes one plan
//! over one document. Every item a feature author implements or declares in
//! that process is defined in a submodule of this module.
//!
//! The submodules:
//!
//! - [`stage`] — the two kinds of stage ([`Stage`](stage::Stage) and
//!   [`RewriteStage`](stage::RewriteStage)) and the values that describe a
//!   stage: its declaration, its needs for one plan, and the plan's record
//!   joining both.
//! - [`data`] — tables: their identity, and the store that holds the tables of
//!   one run.
//! - [`services`] — service slots and the values features contribute to them.
//! - [`lowering`] — lowering rules: the trait a rule implements, what it
//!   declares, and the [`Lowerer`](lowering::Lowerer), the interface it works
//!   through.
//! - [`artifact`] — artifacts (one file's worth of output held as a value),
//!   the artifact set of one output, and the naming service a host supplies.
//! - [`piece`] — pieces: the declaration of a document-level value that a
//!   backend places before or after the document's content, and the table type
//!   that holds its content.
//! - [`region`] — output regions: the named parts an output's entry artifact
//!   is made of, which the backend's [final
//!   stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)
//!   or a host's template assembles.
//! - [`registry`] — features, the registry builder, the checks of the registry
//!   build, and the frozen registry.
//! - [`settings`] — settings written as data: a feature's declaration of its
//!   settings, and how they are read into the feature's Rust types.
//! - [`plan`] — plan requests, the choice of stages and lowering rules, and the
//!   checks of a plan.
//! - [`run`] — runs, the runner, the contexts a stage reads through, [value
//!   catch-up](crate::guide::concepts_overview#value-catch-up), and the tables
//!   a caller installs when a run is constructed.
//!
//! # Position in the crate's layering
//!
//! The modules of the crate are layered as the [crate
//! documentation](crate) describes, and a module uses only the layers below
//! its own:
//!
//! ```text
//! {model, lang} < core < dump < stdcore < math_passthrough        layout standalone
//! ```
//!
//! This module uses [`crate::model`] and [`crate::lang`], and neither of those
//! uses this module; [`crate::dump`], [`crate::stdcore`] and
//! [`crate::math_passthrough`] use it. Within this
//! module, [`data`] is the lowest level: it uses [`crate::model`] and a few
//! types of [`crate::lang`], and no other submodule of `core`.

pub mod artifact;
pub mod data;
pub mod lowering;
pub mod piece;
pub mod plan;
pub mod region;
pub mod registry;
pub mod run;
pub mod services;
pub mod settings;
pub mod stage;
