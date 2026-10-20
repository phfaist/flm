//! The `math-passthrough` feature
//! (`flm_core::math_passthrough`): registered by its
//! own package after the standard package `stdcore`; a command of another feature written
//! inside a formula is an opaque node and nothing is reported; an empty
//! equation line is reported by the feature's check stage, at the span the
//! parse reported it at before the feature existed; the page of a formula
//! carries the passthrough class and stylesheet; and the feature without
//! `math` fails the registry build, because its rules lower an element and its
//! check provides a capability that only `math` declares.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{BuildError, DefContent, FlmRegistry, Site};
use flm_core::dump::diag::expect;
use flm_core::model::element::element_of;
use flm_core::model::names::Name;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::{self, DefinitionOverride, HtmlRenderOptions};
use flm_core::stdcore::backends::{lowering, pieces, text};
use flm_core::math_passthrough::{self, CHECK, ENVIRONMENT_RULE, OPAQUE};
use flm_core::stdcore::features::math::{self, CONTENT, ENVIRONMENT};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, check_feature, package};
use serde_json::json;

use common::stdcore::{identifiers, plan, registry, run_workflow};

/// The registry of the standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

#[test]
fn the_feature_is_its_own_package_s_and_stdcore_does_not_list_it() {
    let names = |entry: flm_core::core::registry::PackageEntry| -> Vec<String> {
        entry.features.iter().map(|feature| feature.info().name.to_string()).collect()
    };
    let standard = names(package(StdcoreConfig::new()));
    assert!(standard.iter().any(|name| name == math::FEATURE));
    assert!(standard.iter().all(|name| name != math_passthrough::FEATURE), "{standard:?}");
    assert_eq!(names(math_passthrough::package()), [math_passthrough::FEATURE]);
    assert_eq!(math_passthrough::FEATURE, "math-passthrough");
    assert_eq!(math_passthrough::PACKAGE, "math-passthrough");
}

#[test]
fn a_command_of_another_feature_inside_a_formula_is_opaque_and_nothing_is_reported() {
    let registry = std();
    for document in ["\\(\\section{x}\\)", "\\(\\href{u}{x}\\)"] {
        let fragment = common::parse(registry.lang(), document);
        assert!(fragment.diags().is_empty(), "{document}: {:?}", fragment.diags());
        let elements: Vec<String> = common::callables(&fragment)
            .iter()
            .map(|node| element_of(node).name.to_string())
            .collect();
        assert_eq!(elements, [OPAQUE], "{document}");
        let run = run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{document}");
    }
}

#[test]
fn an_empty_align_line_is_reported_by_the_check_stage_at_the_second_break() {
    let registry = std();
    let document = "\\begin{align} a \\\\ \\\\ b \\end{align}";
    // The parse reports nothing: whether a line holds mathematics is the
    // processing feature's question.
    let fragment = common::parse(registry.lang(), document);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    // The check stage of the feature is in the plan of the `check` workflow,
    // and reports the line at the `\\` that ends it, as the parse did.
    let check = plan(&registry, CHECK_WORKFLOW, &Overrides::none());
    assert!(check.stage_names().iter().any(|name| name.as_str() == CHECK));
    let run = run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.math-passthrough.empty-line"]);
    expect(run.diagnostics().iter(), "flm.math-passthrough.empty-line")
        .at("\\\\")
        .data(json!({ "environment": "align", "line": 1 }))
        .once();
    let diagnostic = run.diagnostics().iter().next().expect("one diagnostic");
    assert_eq!(
        diagnostic.message(),
        "line 1 (counting from 0) of the `align` environment must hold mathematics; a \
         `\\label`, `\\tag` or `\\nonumber` alone is not mathematics"
    );
}

#[test]
fn the_page_of_a_formula_carries_the_passthrough_class_and_stylesheet() {
    // The stylesheets by address, so that the page shows which one it loads
    // without carrying their rules.
    let mut definitions = BTreeMap::new();
    for style in html::defs::stylesheets() {
        let id = style.id.to_string();
        let stem = id.rsplit('.').next().expect("an identifier has a last part");
        definitions.insert(
            Name::from(id.as_str()),
            DefinitionOverride::Parts(vec![DefContent::Url(format!("css/{stem}.css"))]),
        );
    }
    let options = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
    let overrides =
        Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() };
    let document = "Let \\(a = \\text{one half}\\) hold.\n\n\\begin{equation}\n  E = mc^2\n\\end{equation}\n";
    let run = run_workflow(&std(), html::WORKFLOW, &[document], &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let page = common::html::page(&run);
    assert!(page.contains("class=\"flm-pres-math--passthrough\""), "{page}");
    assert!(page.contains("data-flm-def=\"flm.html.style.math-passthrough\""), "{page}");
    common::html::assert_golden("math-passthrough.page.html", &page);
}

#[test]
fn the_feature_without_math_fails_the_registry_build() {
    let error = common::pipeline::registry_error(vec![
        lowering::feature(),
        pieces::feature(),
        text::feature(),
        check_feature(),
        math_passthrough::feature(),
    ]);
    let undeclared = |site: &Site, wanted: &str| {
        error.errors.iter().any(|error| match error {
            BuildError::Undeclared { feature, site: at, name, .. } => {
                feature == math_passthrough::FEATURE && at == site && name.as_str() == wanted
            }
            _ => false,
        })
    };
    assert!(
        undeclared(&Site::RuleLowers { rule: ENVIRONMENT_RULE.into() }, ENVIRONMENT),
        "{}",
        error.render()
    );
    assert!(
        undeclared(&Site::StageProvides { stage: CHECK.into() }, CONTENT),
        "{}",
        error.render()
    );
}
