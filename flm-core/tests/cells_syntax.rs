//! The `cells` feature's syntax (`flm_core::stdcore::features::cells`): the
//! environment and the commands of its body parsed where they are visible, each
//! command's block role by position (test 1), blank lines in the body and in a
//! `\celldata`'s data (test 2), the styles argument's parser (test 16), the
//! switch `global` (test 19, its parse and digest parts), and the slot of style
//! declarations at registry build (test 4, its registry parts).

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::Run;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{BlockRole, block_role_of};
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::cells::{
    self, CellStyle, CellStylesParser, CellsConfig, STYLES_SLOT,
};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::core::node::NodeRef;

use common::stdcore::{identifiers, run_workflow};

/// The report of a macro no declaration defines.
const UNKNOWN_MACRO: &str = "core.specs.unresolvable-command";

/// The report of an environment no declaration defines.
const UNKNOWN_ENVIRONMENT: &str = "latexlike.environments.unknown-environment";

/// The standard package with the `cells` environment an ordinary environment.
fn global() -> StdcoreConfig {
    StdcoreConfig::new().cells(CellsConfig::new().global(true))
}

/// The registry of the standard package under `config`, with `features` after
/// it.
fn registry_with(config: StdcoreConfig, features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
    try_registry(config, features).unwrap_or_else(|error| panic!("the registry build failed: {error}"))
}

/// The registry of the standard package under `config`, with `features` after
/// it, or the build's error rendered.
fn try_registry(config: StdcoreConfig, features: Vec<Arc<dyn FlmFeature>>) -> Result<Arc<FlmRegistry>, String> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    for feature in features {
        builder.feature(feature);
    }
    builder.build(&RegistryInputs::new()).map_err(|error| error.render())
}

/// Runs the `check` workflow over `document`: the tree it freezes is laid out
/// as blocks.
fn check(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, "check", &[document], &Overrides::none())
}

/// The nodes of the frozen first tree whose element is `name`, in document
/// order.
fn nodes_named<'t>(run: &'t Run, name: &str) -> Vec<NodeRef<'t, Flm>> {
    let tree = run.frozen_tree(TreeId::tree(0)).expect("the first tree is frozen");
    tree.root().descendants().filter(|node| element_of(node).name.as_str() == name).collect()
}

// --- test 1: the environment and the commands of its body --------------------------------

#[test]
fn the_body_holds_its_instructions_as_blocks_and_nothing_between_them() {
    let registry = registry_with(global(), Vec::new());
    let run = check(&registry, "\\begin{cells}\\cell{A} \\cell{B}\n\\\\ \\cell{C}\n\\end{cells}");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let tables = nodes_named(&run, cells::ENVIRONMENT);
    assert_eq!(tables.len(), 1);
    let table = tables[0];
    assert_eq!(block_role_of(&table), BlockRole::Block);
    let body: Vec<NodeRef<'_, Flm>> = table.body().expect("a body").iter().collect();
    let names: Vec<String> = body.iter().map(|node| element_of(node).name.to_string()).collect();
    assert_eq!(names, [cells::CELL, cells::CELL, cells::ROW, cells::CELL]);
    for node in &body {
        assert_eq!(block_role_of(node), BlockRole::Block, "{}", element_of(node).name);
        assert!(cells::stands_in_body(node));
    }
}

#[test]
fn celldata_parses_its_three_arguments() {
    let registry = registry_with(global(), Vec::new());
    let run = check(
        &registry,
        "\\begin{cells}\\celldata<H c, l>[2;-]{A & \\cell<r>[\\merge{+2}]{B} \\\\ C}\\end{cells}",
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let data = nodes_named(&run, cells::DATA);
    assert_eq!(data.len(), 1);
    let data = data[0];
    let styles = data.argument_content_nodes_named(cells::STYLES_ARG).unwrap().expect("styles");
    assert_eq!(styles.source_text(), Some("H c, l"));
    let mapping = data.argument_content_nodes_named(cells::MAPPING_ARG).unwrap().expect("mapping");
    assert_eq!(mapping.source_text(), Some("2;-"));
    let entries = data.argument_content_nodes_named(cells::DATA_ARG).unwrap().expect("data");
    let callables: Vec<String> = entries
        .iter()
        .filter(|node| node.is_callable())
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(callables, [cells::COLUMN, cells::CELL, cells::ROW]);
    // Inside the data, `\cell` and `\\` are inline content.
    for node in entries.iter().filter(|node| node.is_callable()) {
        assert!(matches!(block_role_of(&node), BlockRole::Inline(_)), "{}", element_of(&node).name);
    }
    let merge = nodes_named(&run, cells::MERGE);
    assert_eq!(merge.len(), 1);
    let range = merge[0].argument_content_nodes_named(cells::RANGE_ARG).unwrap().expect("range");
    assert_eq!(range.source_text(), Some("+2"));
}

#[test]
fn the_commands_are_unknown_outside_their_scope() {
    let registry = registry_with(global(), Vec::new());
    for document in ["\\cell{A}", "\\celldata{A}", "\\rownames{a}", "\\merge{1}"] {
        let run = check(&registry, document);
        assert_eq!(identifiers(&run), [UNKNOWN_MACRO], "{document}");
    }
    // Outside a cells body, `&` is an ordinary character and `\\` is no
    // command of this feature.
    let run = check(&registry, "A & B");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert!(nodes_named(&run, cells::COLUMN).is_empty());
    // `\merge` is visible in a placement alone, not in a cell's content.
    let run = check(&registry, "\\begin{cells}\\cell{\\merge{1}}\\end{cells}");
    assert_eq!(identifiers(&run), [UNKNOWN_MACRO]);
}

#[test]
fn an_ampersand_in_an_align_environment_in_a_cell_is_the_mathematics_own() {
    let registry = registry_with(global(), Vec::new());
    let run = check(
        &registry,
        "\\begin{cells}\\cell{\\begin{align}a &= b\\end{align}}\\end{cells}",
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert!(nodes_named(&run, cells::COLUMN).is_empty());
}

#[test]
fn a_cell_in_a_cells_content_is_inline_there() {
    let registry = registry_with(global(), Vec::new());
    let run = check(&registry, "\\begin{cells}\\cell{x \\cell{y} z}\\end{cells}");
    let cells_found = nodes_named(&run, cells::CELL);
    assert_eq!(cells_found.len(), 2);
    assert_eq!(block_role_of(&cells_found[0]), BlockRole::Block);
    assert!(matches!(block_role_of(&cells_found[1]), BlockRole::Inline(_)));
    assert!(!cells::stands_in_body(&cells_found[1]));
}

// --- test 2: blank lines ------------------------------------------------------------------

#[test]
fn a_blank_line_between_two_rows_is_whitespace() {
    let registry = registry_with(global(), Vec::new());
    let run = check(&registry, "\\begin{cells}\n\\cell{A}\n\n\\\\\n\n\\cell{B}\n\\end{cells}");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let table = nodes_named(&run, cells::ENVIRONMENT)[0];
    let body: Vec<NodeRef<'_, Flm>> = table.body().expect("a body").iter().collect();
    let names: Vec<String> = body.iter().map(|node| element_of(node).name.to_string()).collect();
    assert_eq!(names, [cells::CELL, cells::ROW, cells::CELL]);
}

#[test]
fn a_blank_line_inside_celldata_is_the_languages_paragraph_break_error() {
    let registry = registry_with(global(), Vec::new());
    let document = "\\begin{cells}\\celldata{A\n\nB & C}\\end{cells}";
    for workflow in ["check", text::WORKFLOW] {
        let run = run_workflow(&registry, workflow, &[document], &Overrides::none());
        assert!(
            identifiers(&run).contains(&String::from("flm.blocks.parbreak-in-inline")),
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
    }
}

// --- test 16: the styles argument -------------------------------------------------------

/// The styles argument of the first `\cell` of `document`, as its source text;
/// `None` where it is absent.
fn first_styles(registry: &Arc<FlmRegistry>, document: &str) -> (Option<String>, Run) {
    let run = check(registry, document);
    let styles = {
        let cells_found = nodes_named(&run, cells::CELL);
        let cell = cells_found.first().expect("a cell");
        cell.argument_content_nodes_named(cells::STYLES_ARG)
            .unwrap()
            .map(|content| content.source_text().unwrap_or_default().to_string())
    };
    (styles, run)
}

#[test]
fn the_styles_argument_is_an_optional_angle_group() {
    let registry = registry_with(global(), Vec::new());
    let wrap = |cell: &str| format!("\\begin{{cells}}{cell}\\end{{cells}}");
    assert_eq!(first_styles(&registry, &wrap("\\cell<a<b>>{x}")).0.as_deref(), Some("a<b>"));
    assert_eq!(first_styles(&registry, &wrap("\\cell{x}")).0, None);
    assert_eq!(first_styles(&registry, &wrap("\\cell <H>{x}")).0.as_deref(), Some("H"));
    assert_eq!(first_styles(&registry, &wrap("\\cell%%c\n<H>{x}")).0.as_deref(), Some("H"));
    assert_eq!(first_styles(&registry, &wrap("\\cell<clr{>}>{x}")).0.as_deref(), Some("clr{>}"));
    let (styles, run) = first_styles(&registry, &wrap("\\cell<H{x}"));
    assert!(styles.is_some());
    assert!(!identifiers(&run).is_empty(), "an unclosed `<` is reported");
}

/// A test feature declaring `\probe` with one argument: the cells styles
/// parser, or the standard optional `[…]` group.
struct Probe {
    angles: bool,
}

impl FlmFeature for Probe {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("probe", "A command with one argument")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let arg = if self.angles {
            ArgDef::named("x", CellStylesParser::new())
        } else {
            ArgDef::named("x", ArgStdKind::Optional)
        };
        b.element(ElementDef::new(
            "probe",
            ["probe"],
            SyntaxDef::Macro { args: vec![arg] },
            Arc::new(ElementSpec::new("probe.probe")),
        ))
    }
}

#[test]
fn the_parsers_identity_is_in_the_registry_digest() {
    let angles = registry_with(StdcoreConfig::new(), vec![Arc::new(Probe { angles: true })]);
    let brackets = registry_with(StdcoreConfig::new(), vec![Arc::new(Probe { angles: false })]);
    assert_ne!(angles.digest(), brackets.digest());
}

// --- test 19: the switch ------------------------------------------------------------------

#[test]
fn under_the_default_the_environment_is_unknown_between_paragraphs() {
    let registry = registry_with(StdcoreConfig::new(), Vec::new());
    let run = check(&registry, "Before.\n\n\\begin{cells}\\cell{A}\\end{cells}\n\nAfter.");
    assert!(identifiers(&run).contains(&String::from(UNKNOWN_ENVIRONMENT)));
    assert!(nodes_named(&run, cells::ENVIRONMENT).is_empty());
}

#[test]
fn two_registries_differing_in_the_switch_have_different_digests() {
    let default = registry_with(StdcoreConfig::new(), Vec::new());
    let global = registry_with(global(), Vec::new());
    assert_ne!(default.digest(), global.digest());
}

#[test]
fn without_floats_under_the_default_the_registry_builds() {
    let features = common::stdcore::package_features_but(StdcoreConfig::new(), "floats");
    let registry = common::stdcore::registry_over(features);
    let run = check(&registry, "\\begin{cells}\\cell{A}\\end{cells}");
    assert!(identifiers(&run).contains(&String::from(UNKNOWN_ENVIRONMENT)));
}

// --- test 4, the registry parts: the slot of style declarations ---------------------------

/// A test feature contributing custom styles to the slot, as a peer.
struct HostStyles(Vec<&'static str>);

impl FlmFeature for HostStyles {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("host", "A host's cell styles")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        for name in &self.0 {
            b.peer_slot::<CellStyle>(STYLES_SLOT, Arc::new(CellStyle::custom(*name, "a host's style")))?;
        }
        Ok(())
    }
}

#[test]
fn a_contribution_redeclaring_a_shipped_style_is_a_registry_error() {
    let error = try_registry(StdcoreConfig::new(), vec![Arc::new(HostStyles(vec!["green"]))])
        .expect_err("a clash");
    assert!(error.contains("`green` may not be declared twice"), "{error}");
}

#[test]
fn a_custom_style_name_must_be_lowercase_letters_and_digits() {
    for name in ["Orange", "or-ange", "1st"] {
        let error = try_registry(StdcoreConfig::new(), vec![Arc::new(HostStyles(vec![name]))])
            .expect_err("refused");
        assert!(error.contains(name), "{error}");
    }
    assert!(try_registry(StdcoreConfig::new(), vec![Arc::new(HostStyles(vec!["orange2"]))]).is_ok());
}

#[test]
fn a_feature_contributing_styles_builds_without_the_cells_feature() {
    let mut features = common::stdcore::package_features_but(StdcoreConfig::new(), "cells");
    features.push(Arc::new(HostStyles(vec!["orange"])));
    let _ = common::stdcore::registry_over(features);
}

