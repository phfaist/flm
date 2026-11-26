//! What the tests of `flm-host` expect of the diagnostics of a build that reports
//! nothing of its own. A build whose registry holds `math-passthrough` writes its
//! formulas, on the page and in the two LaTeX outputs, for a program that reads them
//! after FLM, and nothing checks them: its plan keeps one unchecked output per such
//! backend, which the run reports as a Warning before its own diagnostics. Shared by
//! the tests of `flm-host`, each including this file as a module.

#![allow(dead_code)]

use flm_core::core::run::UncheckedOutput;
use flm_core::lang::FlmDiagnostic;
use flm_core::techy::error::Severity;

/// The identifier of the Warning a run reports for each output its plan keeps as
/// unchecked.
pub const UNCHECKED_OUTPUT: &str = "flm.run.unchecked-output";

/// What the helper compares of a diagnostic: its identifier, its severity, and the
/// feature and the backend of an [`UncheckedOutput`] it carries.
type Seen<'d> = (&'d str, Severity, Option<(&'d str, &'d str)>);

/// Asserts that `diagnostics` are exactly the Warnings [`UNCHECKED_OUTPUT`] of the
/// unchecked formulas of `math-passthrough` on `backends`, one per backend in that
/// order, and nothing else: what a test whose subject is not the plan asserts of a
/// build that reports nothing of its own.
#[track_caller]
pub fn assert_only_unchecked_output(diagnostics: &[FlmDiagnostic], backends: &[&str]) {
    let reported: Vec<Seen<'_>> = diagnostics
        .iter()
        .map(|d| {
            let unchecked = d
                .data()
                .downcast_ref::<UncheckedOutput>()
                .map(|u| (u.feature.as_str(), u.backend.as_str()));
            (d.identifier(), d.severity(), unchecked)
        })
        .collect();
    let expected: Vec<Seen<'_>> = backends
        .iter()
        .map(|backend| (UNCHECKED_OUTPUT, Severity::Warning, Some(("math-passthrough", *backend))))
        .collect();
    assert_eq!(reported, expected, "expected the unchecked formulas on {backends:?} alone");
}
