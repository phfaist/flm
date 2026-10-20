//! The check stage of the `citations` feature, which reports every cited key
//! that has no citation entry.

use techy::error::Diagnostic;

use crate::core::data::store::StageOut;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};

use super::diagnostics::NoEntry;
use super::entries::{MissingEntry, Retrieval, ENTRIES};
use super::numbers::{stage_failed, KEYS};

/// The name of the check stage and of the capability it provides,
/// `flm.citations.check`. The capability is a member of the collection
/// [`flm.validated`](crate::stdcore::VALIDATED), which every workflow
/// demands (asks the plan to run a stage that provides it; see
/// [`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)), so the
/// check runs under every workflow.
pub const CHECK: &str = "flm.citations.check";

/// The check stage, [`CHECK`]: a check stage of document scope. It reads the
/// cited keys ([`KEYS`]) and their citation entries ([`ENTRIES`]), and reports
/// every cited key with no citation entry ([`NoEntry`]), in order of first
/// citation, at the citation key's first citation: one report per citation
/// key, even when several citation keys refer to one work.
///
/// Because its capability is a member of `flm.validated`, it runs under every
/// workflow, the `check` workflow included, with the same reports. Because it
/// reads [`ENTRIES`], the retrieve stage runs before it, so the `check`
/// workflow asks the citation providers as a rendering workflow does.
#[derive(Debug, Default)]
pub(super) struct Check;

impl Stage for Check {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK, Effect::Check, Scope::Document).provides(CHECK).docs(
            "Reports, as an Error, every citation key the document cites that has no citation \
             entry in `flm.citations.entries`.",
        )
    }

    /// Reads the cited keys, with where each is first cited, and their
    /// citation entries.
    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(KEYS).reads(ENTRIES)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (cited, entries) = match (cx.get(KEYS), cx.get(ENTRIES)) {
            (Ok(Some(cited)), Ok(entries)) => (cited, entries),
            (Err(error), _) | (_, Err(error)) => return stage_failed(out, CHECK, error),
            _ => return,
        };
        for (key, span) in cited.iter() {
            let missing = match entries.and_then(|entries| entries.get(key)) {
                Some(Retrieval::Found(_)) => continue,
                Some(Retrieval::Missing(missing)) => missing.clone(),
                // Both tables hold the same keys: a key the entries table does
                // not hold was asked of no provider.
                None => MissingEntry::Unknown,
            };
            out.diags.push(Diagnostic::error(NoEntry { key: key.to_string(), reason: missing.reason() }, span.clone()));
        }
    }
}
