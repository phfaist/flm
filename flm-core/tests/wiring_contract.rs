//! The `Expand` half of the block contract's second invariant over the wired
//! rewrite features: a node of an element carrying the `Expand` aspect that
//! survives into a structured tree is a violation of its own kind, and an
//! expanded one is none; the frozen trees of a structured plan over `per-plan`
//! definitions satisfy both parts of the contract. The checking stage that
//! gates this half on the expansion alternative in force is covered by
//! `stdcore_check.rs`.

mod common;

use flm_core::model::doc::FragmentLayout;
use flm_core::lang::FlmDiagnostics;
use flm_core::lang::rewrites::blocks::{
    BlocksConfig, ContractViolation, ContractViolationKind, check_block_contract,
    check_parse_contract, structure_with_guard,
};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandSet};

use common::userdefs::{expanded_set, setup_definitions};
use common::wiring::{frozen, registry, run_blocks};

fn kinds(violations: &[ContractViolation]) -> Vec<&'static str> {
    violations.iter().map(|violation| violation.kind.as_str()).collect()
}

/// One `per-plan` macro `\p`, expanded with the given set, then structured
/// as blocks under the language's guard.
fn structured(set: ExpandSet) -> Vec<ContractViolation> {
    let setup = setup_definitions(vec![Definition::macro_("p", Vec::new()).text("P")]);
    let (expanded, _, _) = expanded_set(&setup, r"\p", 8, set);
    let mut diags = FlmDiagnostics::new();
    let tree = structure_with_guard(
        &expanded,
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        setup.lang.guard(),
        &mut diags,
    )
    .expect("structured");
    check_block_contract(&tree)
}

#[test]
fn a_surviving_use_site_of_an_expand_carrying_element_is_a_violation() {
    // The `always` set leaves the `per-plan` definition's use site in place.
    let violations = structured(ExpandSet::Always);
    assert_eq!(kinds(&violations), ["UseSiteSurvived"]);
    match &violations[0].kind {
        ContractViolationKind::UseSiteSurvived { element } => assert_eq!(element, "u.p"),
        other => panic!("unexpected violation: {other:?}"),
    }
}

#[test]
fn an_expanded_use_site_is_no_violation() {
    assert_eq!(kinds(&structured(ExpandSet::PerPlan)), Vec::<&str>::new());
}

#[test]
fn a_structured_plans_frozen_tree_satisfies_both_parts_of_the_contract() {
    let definitions = vec![Definition::macro_("foo", Vec::new()).text("a\n\nb")];
    let run = run_blocks(&registry(definitions), r"x \foo.", &["flm.blocks.structured"]);
    assert_eq!(kinds(&check_parse_contract(frozen(&run))), Vec::<&str>::new());
    assert_eq!(kinds(&check_block_contract(frozen(&run))), Vec::<&str>::new());
}
