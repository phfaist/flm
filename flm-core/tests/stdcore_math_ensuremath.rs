//! `\ensuremath{…}` (`flm_core::stdcore::features::math::ensuremath`): in text
//! an inline formula read as `\(…\)` is, its braces delimiting a math group; in
//! mathematics its argument, as if written in its place. Pinned here: the
//! shape of the parse in both modes, the missing braces, the serialized round
//! trip, and, under `math-passthrough`, the outputs of the four standard
//! backends compared with the same formula written with `\(…\)`, but for the
//! recomposed LaTeX, which keeps the command as written, a user definition's
//! template included.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::lang::{Flm, FlmGroupType, structural};
use flm_core::model::doc::Fragment;
use flm_core::model::element::element_of;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{html, latex_recomposed, latex_rendered, text};
use flm_core::stdcore::features::math::ensuremath::{ENSUREMATH, RULE};
use flm_core::stdcore::features::math::text::TEXT;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use techy::core::node::{NodeRef, NodeTree};
use techy::latexlike::{MathGroupForm, Mode};

use common::dump::round_trip;
use common::stdcore::{registry, run_workflow};

/// The standard package with `math-passthrough`.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// The one `\ensuremath` node of `fragment`.
fn ensuremath_node(fragment: &Fragment) -> NodeRef<'_, Flm> {
    fragment
        .tree()
        .iter_storage_order()
        .find(|node| element_of(node).name == ENSUREMATH)
        .expect("an `\\ensuremath` was parsed")
}

/// The number of math groups in `fragment`.
fn math_groups(fragment: &Fragment) -> usize {
    fragment.tree().iter_storage_order().filter(|node| element_of(node).name == structural::MATH).count()
}

/// The run of `source` under `workflow`, which must report nothing.
fn clean_run(workflow: &str, source: &str) -> Run {
    let run = run_workflow(&std_registry(), workflow, &[source], &Overrides::none());
    assert!(
        run.diagnostics().iter().next().is_none(),
        "{source}: {}",
        run.diagnostics().render_all()
    );
    run
}

/// The text of the entry artifact of the artifact set `run` wrote into `table`.
fn entry_text(run: &Run, table: flm_core::core::data::table::TableName<flm_core::core::artifact::ArtifactSet>) -> String {
    let set = run.table(table).expect("the output table is declared").expect("written");
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The output of `source` on each of the four standard backends: the HTML
/// fragment, the text, the recomposed LaTeX and the rendered LaTeX.
fn outputs(source: &str) -> [String; 4] {
    [
        common::html::fragment(&clean_run(html::WORKFLOW, source)),
        entry_text(&clean_run(text::WORKFLOW, source), text::TEXT_DOCUMENT),
        common::latex::output(&clean_run(latex_recomposed::WORKFLOW, source)),
        entry_text(&clean_run(latex_rendered::WORKFLOW, source), latex_rendered::DOCUMENT),
    ]
}

#[test]
fn in_text_the_braces_delimit_an_inline_math_group_which_is_the_argument() {
    let registry = std_registry();
    let fragment = common::parse(registry.lang(), "A \\ensuremath{x^{2} + \\alpha} here.");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let node = ensuremath_node(&fragment);
    assert_eq!(node.parsing_state().mode(), Mode::Text);
    let content: Vec<NodeRef<'_, Flm>> =
        node.argument_content_nodes(0).expect("the provided argument").iter().collect();
    assert_eq!(content.len(), 1, "the math group alone is the argument's content");
    let group = content[0];
    assert_eq!(element_of(&group).name, structural::MATH);
    assert_eq!(group.group_type(), Some(FlmGroupType::Math(MathGroupForm::Inline)));
    assert_eq!(group.group_delimiters(), Some(("{", "}")));
    for child in group.children().iter() {
        assert_eq!(child.parsing_state().mode(), Mode::Math, "{}", child.summary());
    }
    // The braces inside the formula are groups of their own, not formulas.
    assert_eq!(math_groups(&fragment), 1);
}

#[test]
fn in_mathematics_the_argument_is_read_in_math_mode() {
    let registry = std_registry();
    let fragment = common::parse(registry.lang(), "\\(a + \\ensuremath{b \\text{if} c}\\)");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let node = ensuremath_node(&fragment);
    assert_eq!(node.parsing_state().mode(), Mode::Math);
    let content: Vec<NodeRef<'_, Flm>> =
        node.argument_content_nodes(0).expect("the provided argument").iter().collect();
    assert!(content.iter().all(|child| child.parsing_state().mode() == Mode::Math));
    assert!(content.iter().any(|child| element_of(child).name == TEXT), "the text command inside");
    assert_eq!(math_groups(&fragment), 1, "no formula of its own inside a formula");
}

#[test]
fn in_text_the_argument_must_be_written_in_braces() {
    let registry = std_registry();
    let fragment = common::parse(registry.lang(), "A \\ensuremath x here.");
    assert!(fragment.diags().has_errors(), "the missing argument is an error");
    assert_eq!(math_groups(&fragment), 0);
}

#[test]
fn the_element_owns_one_rule_on_every_backend() {
    let registry = std_registry();
    let rules: Vec<&str> =
        registry.rules_for(ENSUREMATH).iter().map(|idx| registry.rule(*idx).rule.name.as_str()).collect();
    assert_eq!(rules, [RULE]);
}

#[test]
fn the_parse_survives_the_serialized_round_trip_unchanged() {
    let registry = std_registry();
    let live = Arc::new(common::parse(
        registry.lang(),
        "A \\ensuremath{x \\text{and} y} and \\(z + \\ensuremath{w}\\).",
    ));
    let elements = registry.elements().iter().map(|entry| Arc::clone(&entry.element)).collect();
    let read = round_trip(registry.lang(), &live, elements);
    let project = |tree: &NodeTree<Flm>| Projector::new(ProjectOptions::new().with_spans()).tree(tree);
    assert_eq!(project(read.tree()), project(live.tree()));
}

#[test]
fn in_text_every_backend_writes_what_it_writes_for_the_same_inline_formula() {
    for (with, without) in [
        ("A \\ensuremath{x^2 + y} here.", "A \\(x^2 + y\\) here."),
        (
            "Let \\ensuremath{x \\text{for all \\emph{such}} y}.",
            "Let \\(x \\text{for all \\emph{such}} y\\).",
        ),
        ("\\ensuremath{\\frac{a}{b}}", "\\(\\frac{a}{b}\\)"),
    ] {
        let written = outputs(with);
        let expected = outputs(without);
        for (index, backend) in [(0, "html"), (1, "text"), (3, "latex-rendered")] {
            assert!(written[index].contains("\\("), "{backend}: {}", written[index]);
            assert_eq!(written[index], expected[index], "{backend}: {with}");
        }
        // The recomposed LaTeX keeps the command as written, in place of the
        // delimiters `\(` and `\)`.
        assert!(!written[2].contains("\\("), "{}", written[2]);
        assert_eq!(
            written[2],
            expected[2].replacen("\\(", "\\ensuremath{", 1).replacen("\\)", "}", 1),
            "latex-recomposed: {with}"
        );
    }
}

#[test]
fn a_user_definition_that_writes_ensuremath_is_recomposed_with_the_command() {
    // The template is parsed in text mode, so its `\ensuremath` is the text
    // form; the kept definition and the arguments are written with the
    // command, which LaTeX accepts in text and inside a formula alike.
    let definitions = vec![
        Definition::macro_("vv", vec![ArgDef::mandatory("x")]).content("\\ensuremath{\\vec{#{x}}}"),
    ];
    let registry = registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)));
    let source = "Only \\vv{y}, \\(a = \\vv{y}\\), and \\vv{\\ensuremath{x}}.";
    let run = run_workflow(&registry, latex_recomposed::WORKFLOW, &[source], &Overrides::none());
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("\\providecommand\\flmUDvv[1]{\\ensuremath{\\vec{#1}}}\n"), "{style}");
    assert!(!style.contains("\\("), "{style}");
    assert_eq!(
        common::latex::output(&run),
        "Only \\flmUDvv{y}, \\(a = \\flmUDvv{y}\\), and \\flmUDvv{\\ensuremath{x}}.%%\n"
    );
}

#[test]
fn inside_a_formula_the_source_walk_writes_the_argument_without_the_command() {
    let with = "\\(a + \\ensuremath{b \\text{if} c} + d\\)";
    let without = "\\(a + b \\text{if} c + d\\)";
    let written = outputs(with);
    let expected = outputs(without);
    // The HTML page's MathJax, the text and the rendered LaTeX receive the
    // formula's TeX from the source walk: the command is left out.
    for (index, backend) in [(0, "html"), (1, "text"), (3, "latex-rendered")] {
        assert_eq!(written[index], expected[index], "{backend}");
    }
    // The recomposed LaTeX writes the formula as the author wrote it.
    assert!(written[2].contains("\\ensuremath{b \\text{if} c}"), "{}", written[2]);
}

#[test]
fn a_formula_inside_a_text_command_may_be_written_with_ensuremath() {
    let with = "\\(x \\text{if \\ensuremath{y > 0}} \\)";
    let without = "\\(x \\text{if \\(y > 0\\)} \\)";
    let written = outputs(with);
    let expected = outputs(without);
    for (index, backend) in [(0, "html"), (1, "text"), (3, "latex-rendered")] {
        assert_eq!(written[index], expected[index], "{backend}");
    }
    // The recomposed LaTeX keeps the command as written.
    assert_eq!(written[2], expected[2].replace("\\(y > 0\\)", "\\ensuremath{y > 0}"));
}
