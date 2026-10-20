//! The contract check stage of the [`blocks`](super) feature,
//! `flm.blocks.check-contract`: it checks every frozen tree against the tree
//! contract and reports each violation.
//!
//! The *tree contract* is the set of rules every tree satisfies after parsing
//! (the parse part) and the further rules a tree satisfies after the
//! block-structure pass (the block-structure part). Its checker,
//! [`crate::lang::rewrites::blocks::check`], is two plain functions over a
//! tree. This module makes it a check stage of the pipeline: a stage of
//! per-tree scope that runs, over every [frozen
//! tree](crate::guide::concepts_overview#frozen-trees-and-computed-data), each
//! part of the contract
//! that the plan's trees are subject to, and reports every violation as the
//! Error diagnostic `flm.blocks.contract-violated` ([`ContractViolated`]). A
//! violation is a fault in the program (the parser, a rewrite pass or an
//! element's aspect), not in the document.
//!
//! The stage provides [`CONTRACT_CHECKED`] (`flm.blocks.contract-checked`),
//! which the `blocks` feature adds to the collection
//! [`flm.validated`](crate::stdcore::VALIDATED). Every workflow of the standard
//! package demands that collection (the [`check`
//! workflow](crate::stdcore::CHECK_WORKFLOW) and the workflow of each standard
//! backend), so the stage runs in every standard plan.
//!
//! # What the stage requires, and which rules it checks
//!
//! The stage requires the decision about block structure, never one
//! particular outcome, so that it fits into any plan. It does not require the
//! decision about user definitions; it is only ordered after it. It finds out
//! which outcomes the plan chose by reading each outcome's trees table as an
//! optional read (a read that finds nothing when the table was not written),
//! and checks the rules that apply to those outcomes:
//!
//! - It requires [`BLOCKS`] (`flm.blocks`), the decision about block
//!   structure, and reads [`STRUCTURED`] (`flm.blocks.structured`) optionally.
//!   Where that table was written, the trees were structured and both parts of
//!   the contract apply. Where it was not, the plan chose the trees as parsed
//!   (`flm.blocks.raw`, which the lowering rules of the `latex-recomposed`
//!   backend require) and only
//!   the parse part applies.
//! - Whenever the registry declares `flm.userdefs.expanded`, it reads that
//!   table optionally and is ordered after `flm.userdefs`, the decision about
//!   user definitions whose expansion policy is `per-plan`. Where the table was
//!   written, no use site of a user definition (an element that carries the
//!   [`Expand`](crate::lang::rewrites::userdefs::Expand) aspect; see
//!   [`crate::lang::rewrites::userdefs`]) may remain in a structured tree, and
//!   a remaining one is a violation. Where it
//!   was not, the plan chose to keep those use sites in the tree for the
//!   lowering rules (`flm.userdefs.kept`), and that one rule of the
//!   block-structure part does not apply; every other rule still does. The
//!   stage never adds the expansion to a plan that does not otherwise need it.
//! - Whenever the registry declares `flm.userdefs.always-expanded`, it
//!   requires that capability: the definitions whose expansion policy is
//!   `always` are expanded in every plan, so the check is meaningful only after
//!   that rewrite stage has run. The capability refines nothing, so requiring
//!   it chooses no alternative.
//!
//! [`ContractViolated`]: crate::lang::rewrites::blocks::ContractViolated

use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::core::data::store::StageOut;
use crate::lang::rewrites::blocks::{
    ContractViolationKind, check_block_contract, check_parse_contract,
};
use crate::stdcore::features::userdefs::{ALWAYS_EXPANDED, EXPANDED, USERDEFS};

use super::{BLOCKS, STRUCTURED};

/// The name of the contract check stage, `flm.blocks.check-contract`. See the
/// module documentation.
pub const CONTRACT_CHECK: &str = "flm.blocks.check-contract";

/// The capability the contract check stage provides,
/// `flm.blocks.contract-checked`: every frozen tree satisfies the parts of the
/// tree contract that apply to the plan's trees, or its violations have been
/// reported. A member of the collection
/// [`flm.validated`](crate::stdcore::VALIDATED). The module documentation says
/// which parts apply to which plan.
pub const CONTRACT_CHECKED: &str = "flm.blocks.contract-checked";

/// The contract check stage, `flm.blocks.check-contract`. See the module
/// documentation.
pub(super) struct ContractCheck;

impl Stage for ContractCheck {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CONTRACT_CHECK, Effect::Check, Scope::PerTree)
            .provides(CONTRACT_CHECKED)
            .docs(
                "Checks every frozen tree against the parts of the tree contract that apply to \
                 the plan's trees, and reports each violation as \
                 `flm.blocks.contract-violated`.",
            )
    }

    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        // Each decision is asked for separately, as in the structure stage, so
        // that a registry declaring one without the other never makes the check
        // require a capability nobody declared. See design rationale at
        // [§flm-dd-dr:std-blocks-check-gate] and
        // [§flm-dd-dr:std-render-validates].
        let registry = cx.registry();
        let mut needs = Needs::new().requires(BLOCKS).reads_opt(STRUCTURED);
        if registry.cap_id(EXPANDED.name()).is_some() {
            needs = needs.reads_opt(EXPANDED).after(USERDEFS);
        }
        if registry.cap_id(ALWAYS_EXPANDED.name()).is_some() {
            needs = needs.requires(ALWAYS_EXPANDED.name());
        }
        needs
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        // A tree whose rewrite failed is absent; its one diagnostic stands.
        let Some(tree) = cx.my_tree() else { return };
        for violation in check_parse_contract(&tree) {
            out.diags.push(violation.diagnostic(&tree));
        }
        // The block-structure part is defined over structured trees only: the
        // structured trees table is written exactly where the structure rewrite
        // ran.
        if cx.get(STRUCTURED).ok().flatten().is_none() {
            return;
        }
        // Under the kept alternative a use site of a user definition is meant to
        // survive, so that one rule of the part does not apply; every other one
        // does.
        let use_sites_kept = cx.get(EXPANDED).ok().flatten().is_none();
        for violation in check_block_contract(&tree) {
            if use_sites_kept
                && matches!(violation.kind, ContractViolationKind::UseSiteSurvived { .. })
            {
                continue;
            }
            out.diags.push(violation.diagnostic(&tree));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_is_a_per_tree_check_providing_its_capability() {
        let decl = ContractCheck.decl();
        assert_eq!(decl.name, CONTRACT_CHECK);
        assert_eq!(decl.effect, Effect::Check);
        assert_eq!(decl.scope, Scope::PerTree);
        assert_eq!(decl.provides, [crate::model::names::CapName::from(CONTRACT_CHECKED)]);
        assert!(decl.writes.is_empty());
    }
}
