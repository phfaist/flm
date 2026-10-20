//! The `expand-in-tree` lowering rule of
//! `flm_core::stdcore::features::userdefs`, through the whole pipeline: every
//! definition's element is reachable on the text backend through its own rule
//! and no hole; an `always` definition schedules `expand-always` in every plan
//! that lowers; a `per-plan` definition cannot be kept beside a backend; a use
//! site inside content a rule synthesizes at lowering and splices is expanded
//! by the splice's catch-up, with its diagnostics reported through the splice.
//! And a verbatim argument
//! substituted by a definition stands for its characters, whatever precedes it
//! in its inline run: the `check` workflow reports nothing, the lowering stages
//! no `verbatim`, and the text prints the characters as the words of the
//! paragraph.

mod common;

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};

use flm_core::model::addr::TreeId;
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::lang::FlmGroupType;
use flm_core::stdcore::CHECK_WORKFLOW;
use flm_core::stdcore::backends::pres;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::model::names::{CapName, ElementName};
use flm_core::core::plan::{Overrides, Plan, PlanError};
use flm_core::core::registry::{FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::data::table::{Table, TableName};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::refs::UnresolvedRefs;
use flm_core::stdcore::features::userdefs::{
    KEPT, UserdefsConfig, element_name, expand_in_tree_rule, rule_name,
};
use flm_core::stdcore::{StdcoreConfig, package};

use common::lowering::{lowered, text as text_of};
use common::pipeline;
use common::spine::{overrides, render, text_plan};
use common::stdcore::{identifiers, run_workflow};

/// A registry over the package with `definitions` and the test-only
/// features `extra`.
fn registry_over(
    definitions: Vec<Definition>,
    extra: Vec<Arc<dyn FlmFeature>>,
) -> Arc<FlmRegistry> {
    let config = StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    for feature in extra {
        builder.feature(feature);
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// `\hello{name}`, a `per-plan` macro.
fn hello() -> Definition {
    Definition::macro_("hello", vec![ArgDef::mandatory("name")]).text("Hello, #{name}!")
}

/// `\begin{note}…\end{note}`, a `per-plan` inline environment.
fn note() -> Definition {
    Definition::environment("note", Vec::new()).text("[#{body}]")
}

/// The specials `~`, a `per-plan` definition. The `literals` feature of the
/// standard package declares `~` for a non-breaking space, so a definition of
/// the same trigger must say that it deliberately replaces one. The
/// declaration it replaces is text-only; this one applies in both modes, a
/// definition's default, which covers it.
fn tilde() -> Definition {
    Definition::specials("~", Vec::new()).overriding().content("TILDE")
}

/// `\version`, an `always` macro.
fn version() -> Definition {
    Definition::macro_("version", Vec::new()).text("1.0").expand(ExpandPolicy::Always)
}

/// `\refused`, a macro whose template refers to an argument it does not have:
/// the registry build refuses it, and it stays registered but uncompiled, so
/// every use site of it recovers at expansion time.
fn refused() -> Definition {
    Definition::macro_("refused", Vec::new()).text("#1")
}

/// `\code|…|`, a macro substituting its verbatim argument's characters as
/// they are, into text.
fn code() -> Definition {
    Definition::macro_("code", vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })])
        .text("#1")
}

/// Five documents placing a verbatim run at the head of its inline run and
/// after ordinary text — the two positions text resolution treats
/// differently, since it merges a run under the first node's parsing state.
const VERBATIM_DOCUMENTS: [&str; 5] = [
    "\\code|let x = 1;\nlet y = 2;|",
    "\\code|a %% not a comment \\notacommand|",
    "\\code|a\nb| x",
    "x \\code|a\nb|",
    "Here: \\code|a\nb| there.",
];

/// What each document's substituted characters print as: text, its line
/// breaks reflowed as spaces.
const VERBATIM_RUNS: [&str; 5] =
    ["let x = 1; let y = 2;", "a %% not a comment \\notacommand", "a b", "a b", "a b"];

fn stage_names(plan: &Plan) -> Vec<String> {
    plan.rewrite_names().iter().map(|name| name.to_string()).collect()
}

// --- the rule in the plan -------------------------------------------------------------------

#[test]
fn every_definition_kind_is_reachable_through_its_expand_in_tree_rule() {
    let definitions = vec![hello(), note(), tilde()];
    let registry = registry_over(definitions.clone(), Vec::new());
    let plan = text_plan(&registry, &Overrides::none());
    let selected = plan.selection(text::BACKEND).expect("the text backend is active");
    for definition in &definitions {
        let rule = selected.by_element.get(&element_name(definition)).expect("a rule per element");
        assert_eq!(registry.rule(*rule).rule.name, rule_name(definition));
    }
    assert_eq!(rule_name(&tilde()), "flm.userdefs.specials.~.lower.expand-in-tree");
}

#[test]
fn the_rules_carry_the_definitions_expansion_requirement() {
    let per_plan = expand_in_tree_rule(&hello());
    assert_eq!(per_plan.decl.requires, [CapName::from("flm.userdefs.expanded")]);
    let always = expand_in_tree_rule(&version());
    assert_eq!(always.decl.requires, [CapName::from("flm.userdefs.always-expanded")]);
}

#[test]
fn an_always_definition_schedules_expand_always_in_the_text_plan() {
    let registry = registry_over(vec![version()], Vec::new());
    let plan = text_plan(&registry, &Overrides::none());
    assert_eq!(
        stage_names(&plan),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    let run = render(&registry, &["Version \\version."], &Overrides::none());
    assert_eq!(text_of(&run), "Version 1.0.\n");
}

#[test]
fn without_an_always_definition_the_text_plan_still_runs_the_always_expansion() {
    // The render demands the validation collection, and the tree-contract
    // check requires the `always` expansion in every plan; with no `always`
    // definition that rewrite has nothing to do, and runs all the same.
    let registry = registry_over(vec![hello()], Vec::new());
    let plan = text_plan(&registry, &Overrides::none());
    assert_eq!(
        stage_names(&plan),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
}

#[test]
fn every_kind_of_definition_expands_before_lowering() {
    let registry = registry_over(vec![hello(), note(), tilde()], Vec::new());
    let document = "\\hello{you}, \\begin{note}n\\end{note}~end";
    let run = render(&registry, &[document], &Overrides::none());
    assert_eq!(text_of(&run), "Hello, you!, [n]TILDEend\n");
}

/// Demanding `flm.userdefs.kept` beside the text backend over a `per-plan`
/// definition is refused at plan build: the definition's lowering rule
/// requires `flm.userdefs.expanded`, the alternative refinement of the
/// demanded one.
#[test]
fn a_per_plan_definition_cannot_be_kept_beside_a_backend_in_step_1() {
    let registry = registry_over(vec![hello()], Vec::new());
    let overrides = Overrides { demands: vec![CapName::from(KEPT)], ..Overrides::none() };
    let error = Plan::for_workflow(&registry, text::WORKFLOW, &overrides).expect_err("refused");
    assert!(matches!(error, PlanError::Unsatisfiable { .. }), "{error}");
}

#[test]
fn an_always_definition_can_be_kept_beside_a_backend() {
    // `kept` decides the per-plan definitions only; an always definition's
    // rule requires `always-expanded`, which is compatible with both.
    let registry = registry_over(vec![version()], Vec::new());
    let overrides = Overrides { demands: vec![CapName::from(KEPT)], ..Overrides::none() };
    let plan = text_plan(&registry, &overrides);
    assert_eq!(stage_names(&plan), ["flm.input.splice", "flm.userdefs.expand-always", "flm.blocks.structure"]);
}

// --- the rule body: a use site met at lowering ---------------------------------------------

/// A tree a host hands the run — content of another document no rewrite of
/// this run ever saw — as an external table.
#[derive(Default)]
struct Snippet(Option<Arc<NodeTree<Flm>>>);

impl Table for Snippet {
    fn absorb(&mut self, piece: Snippet) {
        if piece.0.is_some() {
            self.0 = piece.0;
        }
    }
}

const SNIPPET: TableName<Snippet> = TableName::new("y.snippet");

/// `\again` → the snippet spliced: content synthesized at lowering that
/// holds use sites, which the splice's catch-up expands.
struct AgainRule;

impl Lower for AgainRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let snippet = lw
            .get(SNIPPET)?
            .and_then(|table| table.0.clone())
            .ok_or_else(|| LowerError::other("no snippet installed"))?;
        Ok(Lowering::Emit(lw.splice(cx, &snippet)?))
    }
}

/// The test-only feature `y`: the external table and `\again`.
fn y_feature() -> Arc<dyn FlmFeature> {
    pipeline::feature("y", |b| {
        b.external_table(SNIPPET, "A tree the host hands the run.")?;
        b.element(ElementDef::new(
            "y",
            ["again"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(ElementSpec::new("y.again").docs("Lowers the snippet in place.")),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "y.again.lower.default",
            "y.again",
            LoweringDecl::new().reads(SNIPPET),
            Arc::new(AgainRule),
            DefaultMark::Everywhere,
        ))?;
        Ok(())
    })
}

/// Renders `text` with `snippet` (parsed through the registry's language,
/// never rewritten) installed as the external table.
fn render_with_snippet(registry: &Arc<FlmRegistry>, snippet: &str, text: &str) -> Run {
    let parsed = common::parse(registry.lang(), snippet);
    assert!(!parsed.diags().has_errors(), "{}", parsed.diags().render_all());
    let externals =
        Externals::none().install_unstamped(SNIPPET, Snippet(Some(Arc::clone(parsed.tree()))));
    let doc = pipeline::doc(registry, &[text]);
    let plan = text_plan(registry, &overrides(None, UnresolvedRefs::Error));
    pipeline::run_with(&doc, &plan, externals, &RunOptions::sequential())
}

#[test]
fn a_use_site_inside_content_spliced_at_lowering_is_expanded() {
    let registry = registry_over(vec![hello(), version()], vec![y_feature()]);
    let run = render_with_snippet(&registry, "\\hello{again} v\\version", "Say \\again.");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "Say Hello, again! v1.0.\n");
    let survivors = lowered(&run, TreeId::tree(0))
        .tree
        .iter_storage_order()
        .filter(|node| element_of(node).name.as_str().starts_with("flm.userdefs."))
        .count();
    assert_eq!(survivors, 0);
}

#[test]
fn a_use_site_the_expander_recovers_inside_a_splice_reports_through_the_splice() {
    let registry = registry_over(vec![refused()], vec![y_feature()]);
    let run = render_with_snippet(&registry, "\\refused", "Say \\again.");
    assert_eq!(identifiers(&run), ["flm.userdefs.bad-definition"]);
    // The recovery is the use site's literal text.
    assert_eq!(text_of(&run), "Say \\refused.\n");
}

#[test]
fn the_rule_targets_the_definitions_element_by_name() {
    let rule = expand_in_tree_rule(&note());
    assert_eq!(rule.element, ElementName::from("flm.userdefs.environment.note"));
    assert!(rule.decl.produces.is_empty());
}

// --- a substituted verbatim argument stands for its characters -----------------------------

#[test]
fn a_substituted_verbatim_argument_leaves_no_verbatim_group() {
    let registry = registry_over(vec![code()], Vec::new());
    for document in VERBATIM_DOCUMENTS {
        let run = render(&registry, &[document], &Overrides::none());
        let frozen = run.frozen_tree(TreeId::tree(0)).expect("the tree is frozen");
        let groups = frozen
            .root()
            .descendants()
            .filter(|node| node.group_type() == Some(FlmGroupType::Verbatim))
            .count();
        assert_eq!(groups, 0, "{document:?}");
        let verbatims = lowered(&run, TreeId::tree(0))
            .tree
            .iter_storage_order()
            .filter(|node| element_of(node).name == pres::VERBATIM)
            .count();
        assert_eq!(verbatims, 0, "{document:?}");
    }
}

#[test]
fn the_check_workflow_reports_nothing_for_a_substituted_verbatim_argument() {
    let registry = registry_over(vec![code()], Vec::new());
    for document in VERBATIM_DOCUMENTS {
        let run = run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
        assert!(
            !run.diagnostics().any_recorded(),
            "{document:?}:\n{}",
            run.diagnostics().render_all()
        );
    }
}

#[test]
fn a_substituted_verbatim_argument_prints_as_text() {
    let registry = registry_over(vec![code()], Vec::new());
    for (document, verbatim_run) in VERBATIM_DOCUMENTS.iter().zip(VERBATIM_RUNS) {
        let run = render(&registry, &[document], &Overrides::none());
        assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
        let text = text_of(&run);
        assert!(text.contains(verbatim_run), "{document:?} printed:\n{text}");
    }
}
