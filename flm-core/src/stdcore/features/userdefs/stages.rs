//! The stages and the build hook of the [`userdefs`](super) feature: the two
//! rewrite stages that expand use sites, the `flm.userdefs.keep` marker stage,
//! and the late-initialization build hook that compiles the templates.
//!
//! Each expansion stage runs the expand pass
//! ([`expand`](crate::lang::rewrites::userdefs::expand)) over one tree, for one
//! set of definitions: `flm.userdefs.expand-always` for the definitions whose
//! expansion policy is `always`, `flm.userdefs.expand` for those whose policy
//! is `per-plan`. The two stages differ in nothing else. The pass takes the
//! expansion depth limit from the feature's configuration and the recursion
//! guard's policy ([`GuardPolicy`](crate::lang::guard::GuardPolicy)) from the
//! built language.
//!
//! When the pass fails over a tree, the stage returns a [`RewriteError`]
//! carrying one Error diagnostic, and the tree is absent from every later
//! table. Content nested more deeply than the [recursion
//! guard](crate::lang::guard) allows gives [`RewriteError::NestingTooDeep`];
//! when the failure happens inside a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), value catch-up
//! reports it again under a condition of its own
//! ([`StageCx::resolve_value`]). Every other failure, a refused worker thread
//! included, gives [`RewriteError::Failed`]. The pass runs on a [worker
//! thread](crate::lang::guard::on_worker_thread) of its own when the calling
//! thread is not one; the runner's threads always are.

use std::sync::Arc;

use techy::core::node::NodeTree;

use crate::model::addr::TreeId;
use crate::core::plan::PlanCx;
use crate::core::registry::{LateInit, LateInitError};
use crate::core::run::{RewriteCx, StageCx};
use crate::core::stage::{
    Effect, Needs, RewriteDecl, RewriteError, RewriteStage, Scope, Stage, StageDecl,
};
use crate::core::data::store::StageOut;
use crate::lang::mint::MintError;
use crate::lang::rewrites::userdefs::template::minting_diagnostics;
use crate::lang::rewrites::userdefs::{
    ExpandFailure, ExpandOptions, ExpandSet, TemplateLanguage, UserDefinition, expand,
};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};

use super::{ALWAYS_EXPANDED, EXPAND, EXPAND_ALWAYS, EXPANDED, KEEP, KEPT, USERDEFS, kept};

/// Which definitions one expansion stage expands, by their expansion policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Expands {
    /// The definitions whose policy is `always`: the stage
    /// `flm.userdefs.expand-always`.
    Always,
    /// The definitions whose policy is `per-plan`: the stage
    /// `flm.userdefs.expand`.
    PerPlan,
}

/// One of the two expansion rewrite stages, for the definitions `expands`
/// selects. See the module documentation.
pub(super) struct ExpandStage {
    /// Which definitions this stage expands.
    pub(super) expands: Expands,
    /// The expansion depth limit of the feature's configuration
    /// ([`UserdefsConfig::recursion_limit`](super::UserdefsConfig::recursion_limit)).
    pub(super) recursion_limit: usize,
}

impl RewriteStage for ExpandStage {
    fn decl(&self) -> RewriteDecl {
        match self.expands {
            Expands::Always => RewriteDecl::new(EXPAND_ALWAYS, ALWAYS_EXPANDED.name()).docs(
                "Expands the use sites of the user definitions whose expansion policy is \
                 `always`. Publishes the trees as `flm.userdefs.always-expanded`, a capability \
                 that goes with either alternative of `flm.userdefs`. The lowering rule of \
                 every `always` definition requires it, so this stage is in every plan that \
                 lowers a registry holding such a definition.",
            ),
            Expands::PerPlan => RewriteDecl::new(EXPAND, EXPANDED.name())
                .refines(USERDEFS)
                .default()
                .docs(
                    "Expands the use sites of the user definitions whose expansion policy \
                     is `per-plan`. Publishes the trees as `flm.userdefs.expanded`, the \
                     default refinement of `flm.userdefs`. Runs after \
                     `flm.userdefs.expand-always` when both are in the plan.",
                ),
        }
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        match self.expands {
            Expands::Always => Needs::new(),
            // Ordering only: the `always` stage is activated by whoever
            // requires its capability — an `always` definition's lowering rule
            // — never by this edge. See design rationale at
            // [§flm-dd-dr:pipe-after-orders-only].
            Expands::PerPlan => Needs::new().after(ALWAYS_EXPANDED.name()),
        }
    }

    fn rewrite(
        &self,
        _tree_id: TreeId,
        tree: &Arc<NodeTree<Flm>>,
        cx: &RewriteCx<'_>,
        diags: &mut FlmDiagnostics,
    ) -> Result<Arc<NodeTree<Flm>>, RewriteError> {
        let options = ExpandOptions {
            recursion_limit: self.recursion_limit,
            guard: cx.registry().lang().guard(),
        };
        let set = match self.expands {
            Expands::Always => ExpandSet::Always,
            Expands::PerPlan => ExpandSet::PerPlan,
        };
        expand(tree, cx.registry().lang(), &options, set, diags).map_err(|failure| match failure {
            ExpandFailure::NestingTooDeep(diagnostic) => RewriteError::NestingTooDeep(diagnostic),
            ExpandFailure::Failed(diagnostic) | ExpandFailure::WorkerThread(diagnostic) => {
                RewriteError::Failed(diagnostic)
            }
        })
    }
}

/// The marker stage `flm.userdefs.keep`: it does nothing and provides
/// `flm.userdefs.kept`, the refinement of `flm.userdefs` under which the use
/// sites of the `per-plan` definitions stay in the tree for the lowering
/// rules.
pub(super) struct KeepMarker;

impl Stage for KeepMarker {
    fn decl(&self) -> StageDecl {
        StageDecl::new(KEEP, Effect::Compute, Scope::PerTree).provides(KEPT).docs(
            "Does nothing and provides `flm.userdefs.kept`, so that a plan that requires the \
             use sites of the `per-plan` user definitions kept in the tree excludes the \
             `flm.userdefs.expand` rewrite stage, and those use sites reach the lowering \
             rules.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
}

/// The late-initialization build hook ([`LateInit`]) of the `userdefs`
/// feature: it compiles the templates of every definition against the finished
/// language, over one template language
/// ([`TemplateLanguage`]) built for all the definitions the feature registers.
///
/// The hook runs once the language that documents are parsed with is built.
/// Every diagnostic of a compilation (the warnings of a definition that
/// compiled, the errors of one that was refused) goes to the registry's build
/// diagnostics, so a mistake in a template is reported when the registry is
/// built rather than in every document that uses it. A refused definition
/// stays registered without compiled templates, and each of its use sites is
/// replaced by its literal source text and reported with
/// `flm.userdefs.bad-definition`.
///
/// If the template language itself cannot be built, every definition is
/// refused with that failure as its diagnostic (`flm.userdefs.bad-definition`;
/// [`refuse_all`]), as the expand pass's own compilation does. No
/// configuration can cause this: the template language is built from this
/// crate's constant declarations alone, and the branch only guards against a
/// fault in the library.
///
/// Once the templates are compiled, the hook records on every definition
/// whether it is recomposable, and the arguments one call of it writes
/// ([`kept::record_recomposability`]), which reports nothing.
///
/// The hook always returns `Ok`: none of the above makes the registry build
/// fail.
pub(super) struct CompileTemplates {
    /// The data of each definition's element, in the order the definitions
    /// were declared.
    pub(super) definitions: Vec<Arc<UserDefinition>>,
}

impl LateInit for CompileTemplates {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let language = match TemplateLanguage::new(lang) {
            Ok(language) => language,
            Err(errors) => {
                refuse_all(&self.definitions, &errors, diags);
                kept::record_recomposability(&self.definitions);
                return Ok(());
            }
        };
        for definition in &self.definitions {
            let (Ok(reported) | Err(reported)) = definition.compile_with(lang, &language);
            for diagnostic in reported {
                diags.push(diagnostic);
            }
        }
        kept::record_recomposability(&self.definitions);
        Ok(())
    }
}

/// Refuses every definition with the failure to build the template language as
/// its diagnostic: one `flm.userdefs.bad-definition` per error and per
/// definition, or one per definition when the build reported no error and
/// produced no techy package. The definitions stay registered without compiled
/// templates.
fn refuse_all(
    definitions: &[Arc<UserDefinition>],
    errors: &[MintError],
    diags: &mut FlmDiagnostics,
) {
    for definition in definitions {
        let display = definition.definition().display_name();
        for diagnostic in minting_diagnostics(&display, errors) {
            diags.push(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::rewrites::userdefs::Definition;

    #[test]
    fn a_minting_failure_refuses_every_definition_with_its_own_diagnostic() {
        let definitions = vec![
            UserDefinition::new(Definition::macro_("a", Vec::new()).text("A")),
            UserDefinition::new(Definition::macro_("b", Vec::new()).text("B")),
        ];
        let mut diags = FlmDiagnostics::new();
        refuse_all(&definitions, &[], &mut diags);
        let identifiers: Vec<&str> = diags.iter().map(|d| d.identifier()).collect();
        assert_eq!(identifiers, ["flm.userdefs.bad-definition"; 2]);
        assert!(definitions.iter().all(|definition| definition.compiled().is_none()));
    }
}
