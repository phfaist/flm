//! Running a plan over a document: the [`Run`], the contexts through which
//! stages read what they need while they run, and the data that a caller
//! installs into a run from outside.
//!
//! A run is one execution of one plan over one document; it owns the frozen
//! trees, the store and the diagnostics ([`Run`]). A plan is the list of stages
//! that satisfies a plan request, in dependency order and grouped in batches
//! ([`Plan`](crate::core::plan::Plan)). Because a run owns everything it
//! computes, one document may be run several times, under different plans,
//! without one run seeing another's results. See [stages, capabilities, and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans) and
//! [running a plan](crate::guide::pipeline#running-a-plan).
//!
//! An embedding program constructs a run with [`Run::new`] from a document, a
//! plan and the [`Externals`] it installs, executes it with [`Run::execute`] or
//! batch by batch with [`Run::step`] under [`RunOptions`], and reads the results
//! with [`Run::table`] and [`Run::diagnostics`]. A stage author uses this module
//! through [`StageCx`], which the runner hands to every stage call, and through
//! [value catch-up](crate::guide::concepts_overview#value-catch-up)
//! ([`StageCx::resolve_value`]), which brings a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), a small tree that
//! a stage builds during the run, up to date before it is rendered.
//!
//! # Module map
//!
//! - [`runner`]: [`Run`] itself; [`RunOptions`], which choose how a run executes
//!   and whether the error gate may stop it; [`Step`] and [`RunOutcome`], what a
//!   step and a complete execution report; [`RunError`], the errors of
//!   construction and execution; and [`DiagnosticsView`], which chains the
//!   diagnostics from parsing with the diagnostics the run produced.
//! - [`externals`]: [`Externals`], the values a caller installs into external
//!   tables, each as an [`Install`], and the [`Stamp`] that records what a
//!   value was produced under.
//! - [`order`]: the run's document order, the document's walk taken over the
//!   frozen trees, and [`DocPosition`], a node's position in that order.
//! - [`cx`]: [`StageCx`] and [`RewriteCx`], what a stage and a rewrite stage may
//!   read during one call; [`FrozenWalk`], the walk over the frozen trees;
//!   [`CaughtUp`] and [`CatchUpError`], the result of value catch-up.
//!
//! # Diagnostic conditions defined here
//!
//! The runner reports the first four conditions itself. Value catch-up returns
//! the fifth inside its error, and a stage or a lowering rule builds and reports
//! the sixth:
//!
//! - `flm.run.preferred-rule-set-aside` ([`PreferredRuleSetAside`]),
//!   `flm.run.preferred-rule-not-candidate` ([`PreferredRuleNotCandidate`])
//!   and `flm.run.ignored-preference` ([`IgnoredPreference`]), at Warning
//!   severity at the start of the run, one for each preference of the plan
//!   request that the plan could not follow;
//! - `flm.run.placed-twice` ([`PlacedTwice`]), at Error severity, when a
//!   rewrite copied a node that places a tree of the document;
//! - `flm.synth.nesting-too-deep` ([`SynthNestingTooDeep`]), the diagnostic
//!   of [`CatchUpError::NestingTooDeep`], returned when value catch-up fails
//!   because a synthesized value nests too deeply;
//! - `flm.run.missing-entry` ([`MissingEntry`]), at Error severity, which a
//!   stage or a lowering rule reports when a table holds no entry for a node
//!   inside a synthesized value ([`StageCx::missing_entry`] builds it).

pub mod cx;
pub mod externals;
pub mod order;
pub mod runner;

pub use cx::{
    CatchUpError, CaughtUp, FrozenWalk, IgnoredPreference, MissingEntry,
    PreferredRuleNotCandidate, PreferredRuleSetAside, RewriteCx, StageCx,
    SynthNestingTooDeep,
};
pub use externals::{Externals, Install, Stamp};
pub use order::{DocPosition, PlacedTwice};
pub use runner::{DiagnosticsView, Run, RunError, RunOptions, RunOutcome, Step};
