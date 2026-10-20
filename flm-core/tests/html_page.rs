//! The definitions a page needs and how each one arrives
//! (`flm_core::stdcore::backends::html::defs` and `…::page`): what the
//! resolve stage writes into `flm.html.definitions`, what the page assembly
//! makes of it, what the render option `definitions` changes, the three
//! conditions, and the shape of the stylesheets this backend ships.
//!
//! The page a document reaches is pinned here only where the property is about
//! the definitions; the pages of `html_pipeline.rs` pin the skeleton and the
//! content.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;

use flm_core::core::lowering::{DefaultMark, Lowering};
use flm_core::core::plan::Overrides;
use flm_core::core::data::store::StageOut;
use flm_core::core::data::table::{ProjectCx, Table, TableName};
use flm_core::core::plan::PlanCx;
use flm_core::core::registry::{
    BuildError, DefContent, DefinitionText, FeatureBuilder, FeatureChain, FlmFeature, FlmRegistry,
    RegistryBuilder, RegistryInputs, StyleDef,
};
use flm_core::core::run::StageCx;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::core::run::Run;
use flm_core::dump::diag::{expect, expect_none};
use flm_core::dump::golden::assert_json_golden;
use flm_core::model::names::{BackendName, CapName, Name};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::{
    self, DEFINITIONS_TABLE, DefinitionKind, DefinitionNotInlinable, DefinitionOverride,
    HtmlRenderOptions, MATHJAX_LOADER, NotInlinablePart, RESOLVE_STAGE, ResolvedDefinitions,
    STYLE_PAGE,
};
use flm_core::stdcore::backends::html::conventions::{
    SCRIPT_MATHJAX, STYLE_CORE, STYLE_ERRORS, STYLE_HEADINGS, STYLE_LISTS, STYLE_MATH_PASSTHROUGH,
    STYLE_VERBATIM,
};
use flm_core::stdcore::backends::html::mathjax::mathjax_script;
use flm_core::stdcore::backends::html::names::{class_of_element, class_of_role};
use flm_core::stdcore::backends::pres;
use flm_core::stdcore::features::{cells, citations, defterm, floats, headings, meta, quote, theorems, toc, xnotes};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::error::Severity;

use common::html::{assert_golden, golden_path, page as rendered_page};
use common::lowering::{name, produces, rule};
use common::macro_def;
use common::stdcore::{plan, registry as stdcore_registry, run_workflow};

/// The registry of the standard package under its default configuration.
fn std_registry() -> Arc<FlmRegistry> {
    stdcore_registry(StdcoreConfig::new())
}

/// Runs `texts` under the `html` workflow of the standard package.
fn render(texts: &[&str]) -> Run {
    run_workflow(&std_registry(), html::WORKFLOW, texts, &Overrides::none())
}

/// Runs `texts` under the `html` workflow with these render options.
fn render_with(registry: &Arc<FlmRegistry>, texts: &[&str], options: HtmlRenderOptions) -> Run {
    let overrides = Overrides {
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    run_workflow(registry, html::WORKFLOW, texts, &overrides)
}

/// The render options carrying one override.
fn overriding(id: &str, delivery: DefinitionOverride) -> HtmlRenderOptions {
    let mut definitions = BTreeMap::new();
    definitions.insert(Name::from(id), delivery);
    HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() }
}

/// The definitions table the run wrote; its absence is a test failure.
fn definitions(run: &Run) -> &ResolvedDefinitions {
    run.table(DEFINITIONS_TABLE).expect("the definitions table is declared").unwrap_or_else(
        || panic!("no definitions table; diagnostics:\n{}", run.diagnostics().render_all()),
    )
}

/// The identifiers the table lists, in its own order.
fn listed(run: &Run) -> Vec<String> {
    definitions(run).definitions.iter().map(|d| d.id.to_string()).collect()
}

// --- only what the content needs ---------------------------------------------------------

#[test]
fn the_resolve_stage_stands_between_the_emit_stage_and_the_page() {
    let plan = plan(&std_registry(), html::WORKFLOW, &Overrides::none());
    let stages: Vec<String> = plan.stage_names().iter().map(|n| n.to_string()).collect();
    let at = |name: &str| {
        stages.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} in {stages:?}"))
    };
    assert!(at(html::EMIT_STAGE) < at(RESOLVE_STAGE));
    assert!(at(RESOLVE_STAGE) < at(html::FINAL_STAGE));
}

#[test]
fn a_page_without_mathematics_carries_neither_the_formula_renderer_nor_its_rules() {
    let run = render(&["A plain paragraph.\n\nAnd another.\n"]);
    let listed = listed(&run);
    assert!(!listed.contains(&String::from(SCRIPT_MATHJAX)), "{listed:?}");
    assert!(!listed.contains(&String::from(STYLE_MATH_PASSTHROUGH)), "{listed:?}");
    let page = rendered_page(&run);
    assert!(!page.contains("<script"), "{page}");
    assert!(!page.contains("mathjax"), "{page}");
}

#[test]
fn a_page_with_mathematics_carries_the_formula_renderer_and_its_rules() {
    let run = render(&["A formula \\(x^2\\) here.\n\nMore.\n"]);
    let listed = listed(&run);
    assert!(listed.contains(&String::from(SCRIPT_MATHJAX)), "{listed:?}");
    assert!(listed.contains(&String::from(STYLE_MATH_PASSTHROUGH)), "{listed:?}");
    let page = rendered_page(&run);
    // The configuration first, then the address the program is loaded from:
    // the program reads the configuration when it runs.
    let configuration = page.find("window.MathJax").expect("the configuration");
    let loader = page.find(MATHJAX_LOADER).expect("the loader");
    assert!(configuration < loader, "{page}");
}

#[test]
fn a_page_without_verbatim_content_carries_no_verbatim_rules() {
    let plain = listed(&render(&["A plain paragraph.\n\nAnd another.\n"]));
    assert!(!plain.contains(&String::from(STYLE_VERBATIM)), "{plain:?}");
    let with_code = listed(&render(&["A \\verbcode|snippet| here.\n\nMore.\n"]));
    assert!(with_code.contains(&String::from(STYLE_VERBATIM)), "{with_code:?}");
}

/// A reference whose target no document declared is marked in the page, and
/// the rule that marks it arrives with the chunk such a document requires.
///
/// The mark is a display style of a span, so it stands in the chunk the
/// conventions row of a span names (`flm.html.style.core`) and not in the
/// chunk of the other error looks: a document whose only error is an
/// unresolved reference requires no other chunk, and a rule in one it does not
/// require would never reach it.
#[test]
fn a_page_whose_only_error_is_an_unresolved_reference_carries_the_rule_that_marks_it() {
    let run = render(&["See \\ref{sec:nope} here.\n\nMore text.\n"]);
    let listed = listed(&run);
    assert!(listed.contains(&String::from(STYLE_CORE)), "{listed:?}");
    let page = rendered_page(&run);
    // The document's span is marked as unresolved, and the page holds the rule
    // for that mark.
    assert!(page.contains("data-flm-style=\"unresolved-reference\""), "{page}");
    assert!(
        page.contains(".flm-pres-span[data-flm-style=\"unresolved-reference\"]"),
        "the rule that marks an unresolved reference is missing:\n{page}"
    );
}

#[test]
fn a_definition_two_trees_require_is_listed_once_and_written_once() {
    let run = render(&["First tree.\n\nMore.\n", "Second tree.\n\nMore.\n"]);
    let listed = listed(&run);
    assert_eq!(listed.iter().filter(|id| *id == STYLE_CORE).count(), 1, "{listed:?}");
    let page = rendered_page(&run);
    assert_eq!(page.matches(&format!("data-flm-def=\"{STYLE_CORE}\"")).count(), 1, "{page}");
}

#[test]
fn the_page_carries_the_page_stylesheet_that_no_element_requires() {
    let run = render(&["A plain paragraph.\n\nAnd another.\n"]);
    assert!(listed(&run).contains(&String::from(STYLE_PAGE)));
}

#[test]
fn the_table_follows_registration_order_and_not_the_order_things_were_required() {
    // The document meets the code before the heading, and mathematics last;
    // the table still lists the stylesheets as the features registered them
    // (the headings feature's empty style definition, which the heading rule
    // requires, first), and every script after every stylesheet.
    let run = render(&["\\verbcode|c|\n\n\\section{T}\n\nMore \\(x\\).\n"]);
    assert_eq!(
        listed(&run),
        [headings::STYLE, STYLE_CORE, STYLE_HEADINGS, STYLE_VERBATIM, STYLE_MATH_PASSTHROUGH, STYLE_PAGE, SCRIPT_MATHJAX]
    );
    let kinds: Vec<DefinitionKind> = definitions(&run).definitions.iter().map(|d| d.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == DefinitionKind::Script).count(), 1);
    assert_eq!(kinds.last(), Some(&DefinitionKind::Script));
    // Every row names the feature that declared it: the headings feature its
    // style definition, the backend the others.
    for definition in &definitions(&run).definitions {
        let owner = if definition.id.as_str() == headings::STYLE { "headings" } else { html::FEATURE_NAME };
        assert_eq!(definition.owner.as_str(), owner);
    }
}

// --- the host decides the delivery -------------------------------------------------------

#[test]
fn an_override_writes_the_hosts_own_parts_in_place_of_the_declared_ones() {
    let options = overriding(
        STYLE_CORE,
        DefinitionOverride::Parts(vec![DefContent::Inline(String::from(".flm-pres-par{}"))]),
    );
    let run = render_with(&std_registry(), &["Text.\n\nMore text.\n"], options);
    let page = rendered_page(&run);
    assert!(
        page.contains(&format!("<style data-flm-def=\"{STYLE_CORE}\">.flm-pres-par{{}}</style>")),
        "{page}"
    );
    // What is required has not changed: the row is still there.
    assert!(listed(&run).contains(&String::from(STYLE_CORE)));
}

#[test]
fn an_address_is_written_as_a_link_for_a_stylesheet_and_as_a_source_for_a_script() {
    let registry = std_registry();
    let style = overriding(
        STYLE_CORE,
        DefinitionOverride::Parts(vec![DefContent::Url(String::from("/css/flm-core.css"))]),
    );
    let page = rendered_page(&render_with(&registry, &["Text.\n\nMore text.\n"], style));
    assert!(
        page.contains(&format!(
            "<link rel=\"stylesheet\" href=\"/css/flm-core.css\" data-flm-def=\"{STYLE_CORE}\">"
        )),
        "{page}"
    );

    let script = overriding(
        SCRIPT_MATHJAX,
        DefinitionOverride::Parts(vec![
            DefContent::Inline(String::from("window.MathJax={};")),
            DefContent::Url(String::from("/js/mathjax.js")),
        ]),
    );
    let page = rendered_page(&render_with(&registry, &["A formula \\(x\\).\n\nMore.\n"], script));
    assert!(
        page.contains(&format!(
            "<script src=\"/js/mathjax.js\" defer data-flm-def=\"{SCRIPT_MATHJAX}\"></script>"
        )),
        "{page}"
    );
    // The declared address is gone; the host's own configuration stands.
    assert!(!page.contains(MATHJAX_LOADER), "{page}");
    assert!(page.contains("window.MathJax={};"), "{page}");
}

#[test]
fn a_definition_the_host_supplies_is_absent_from_the_page_and_present_in_the_table() {
    let options = overriding(SCRIPT_MATHJAX, DefinitionOverride::ProvidedByHost);
    let run = render_with(&std_registry(), &["A formula \\(x\\).\n\nMore.\n"], options);
    let page = rendered_page(&run);
    assert!(!page.contains("<script"), "{page}");
    assert!(!page.contains(MATHJAX_LOADER), "{page}");
    // A host reading the table still learns that the document needs it.
    let row = definitions(&run).get(SCRIPT_MATHJAX).expect("the row");
    assert_eq!(row.delivery, DefinitionOverride::ProvidedByHost);
    assert_eq!(row.kind, DefinitionKind::Script);
}

#[test]
fn an_override_naming_no_definition_is_reported_and_changes_nothing() {
    let options = overriding("t.no.such.definition", DefinitionOverride::ProvidedByHost);
    let run = render_with(&std_registry(), &["Text.\n\nMore text.\n"], options);
    expect(run.diagnostics().iter(), "flm.html.unknown-definition")
        .severity(Severity::Warning)
        .once();
    assert_eq!(listed(&run), [STYLE_CORE, STYLE_PAGE]);
    assert!(rendered_page(&run).contains("flm.html.style.core"));
}

#[test]
fn an_override_of_a_definition_the_document_does_not_require_writes_nothing() {
    let options = overriding(
        STYLE_ERRORS,
        DefinitionOverride::Parts(vec![DefContent::Inline(String::from(".x{}"))]),
    );
    let run = render_with(&std_registry(), &["Text.\n\nMore text.\n"], options);
    expect_none(run.diagnostics().iter(), "flm.html.unknown-definition");
    assert!(!listed(&run).contains(&String::from(STYLE_ERRORS)));
    assert!(!rendered_page(&run).contains(".x{}"));
}

/// The render options requiring `ids` for the page.
fn requiring(ids: &[&str]) -> HtmlRenderOptions {
    let required_definitions: BTreeSet<Name> = ids.iter().map(|id| Name::from(*id)).collect();
    HtmlRenderOptions { required_definitions, ..HtmlRenderOptions::default() }
}

#[test]
fn a_definition_the_host_requires_reaches_a_page_whose_content_does_not() {
    let run = render_with(&std_registry(), &["A plain paragraph.\n"], requiring(&[SCRIPT_MATHJAX]));
    assert!(listed(&run).contains(&String::from(SCRIPT_MATHJAX)));
    let page = rendered_page(&run);
    assert_eq!(page.matches(MATHJAX_LOADER).count(), 1, "{page}");
}

#[test]
fn a_definition_the_host_and_the_content_both_require_stands_once() {
    let run = render_with(&std_registry(), &["A formula \\(x^2\\) here.\n"], requiring(&[SCRIPT_MATHJAX]));
    let listed = listed(&run);
    assert_eq!(listed.iter().filter(|id| *id == SCRIPT_MATHJAX).count(), 1, "{listed:?}");
    let page = rendered_page(&run);
    assert_eq!(page.matches(MATHJAX_LOADER).count(), 1, "{page}");
    assert_eq!(page.matches("window.MathJax = {").count(), 1, "{page}");
}

#[test]
fn a_definition_the_host_requires_and_no_feature_declares_is_an_error() {
    let run = render_with(&std_registry(), &["Text.\n\nMore text.\n"], requiring(&["t.no.such.definition"]));
    expect(run.diagnostics().iter(), "flm.html.unknown-requirement").severity(Severity::Error).once();
    assert_eq!(listed(&run), [STYLE_CORE, STYLE_PAGE]);
}

// --- the three conditions ----------------------------------------------------------------

/// The `html` workflow over the standard package with one more feature.
fn registry_with(extra: Arc<dyn FlmFeature>) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(extra);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// A feature declaring `\hl{…}` and one lowering rule for it on the `html`
/// backend, which requires `required` where it fires and `selected` merely by
/// being selected, and registers the style definitions `defs` names.
fn requiring_feature(
    required: Option<&'static str>,
    selected: Option<&'static str>,
    defs: Vec<StyleDef>,
) -> Arc<dyn FlmFeature> {
    common::pipeline::feature("t", move |b: &mut FeatureBuilder<'_>| -> Result<(), BuildError> {
        b.element(macro_def("t", "hl", 1))?;
        let mut decl = produces(&[pres::SPAN]);
        if let Some(id) = selected {
            decl = decl.selection_requires(id);
        }
        b.lowering_rule(rule(
            "t.hl.lower.html",
            "t.hl",
            decl,
            DefaultMark::OnBackends(vec![BackendName::from(html::BACKEND)]),
            move |node, cx, lw| {
                if let Some(id) = required {
                    lw.require(&Name::from(id));
                }
                let body = lw.lower_arg(cx, &node, 0)?;
                Ok(Lowering::Emit(vec![lw.pres_with(
                    cx,
                    &name(pres::SPAN),
                    pres::SpanPayload::default(),
                    body,
                )?]))
            },
        ))?;
        for def in &defs {
            b.style_def(def.clone())?;
        }
        Ok(())
    })
}

/// A stylesheet definition of the toy feature.
fn toy_style(id: &str, content: Vec<DefContent>) -> StyleDef {
    StyleDef { id: Name::from(id), content, docs: String::from("a toy stylesheet") }
}

#[test]
fn a_rules_selection_requirement_and_its_occurrence_requirement_both_reach_the_page() {
    // The syntax-highlighting case in miniature: one stylesheet the rule needs
    // wherever it is selected, one it needs only where it fires.
    let registry = registry_with(requiring_feature(
        Some("t.style.marked"),
        Some("t.style.always"),
        vec![
            toy_style("t.style.always", vec![DefContent::Inline(String::from(".always{}"))]),
            toy_style("t.style.marked", vec![DefContent::Inline(String::from(".marked{}"))]),
        ],
    ));
    let without = run_workflow(&registry, html::WORKFLOW, &["No occurrence.\n\nMore.\n"], &Overrides::none());
    let without = listed(&without);
    assert!(without.contains(&String::from("t.style.always")), "{without:?}");
    assert!(!without.contains(&String::from("t.style.marked")), "{without:?}");

    let run = run_workflow(&registry, html::WORKFLOW, &["A \\hl{word}.\n\nMore.\n"], &Overrides::none());
    let with = listed(&run);
    assert!(with.contains(&String::from("t.style.always")), "{with:?}");
    assert!(with.contains(&String::from("t.style.marked")), "{with:?}");
    let page = rendered_page(&run);
    assert!(page.contains(".always{}"), "{page}");
    assert!(page.contains(".marked{}"), "{page}");
    // Another feature's definitions are written after this backend's own: the
    // table follows the order the features registered them.
    let core = page.find(STYLE_CORE).expect("the core stylesheet");
    assert!(core < page.find("t.style.always").expect("the toy stylesheet"), "{page}");
}

#[test]
fn two_features_declaring_one_identifier_declare_one_definition() {
    // The identifier says the two mean the same stylesheet, so the page writes
    // it once — the declaration of the feature placed last, the toy feature,
    // added after the standard package.
    let registry = registry_with(requiring_feature(
        None,
        None,
        vec![toy_style(STYLE_CORE, vec![DefContent::Inline(String::from(".second{}"))])],
    ));
    let run = run_workflow(&registry, html::WORKFLOW, &["Text.\n\nMore text.\n"], &Overrides::none());
    let listed = listed(&run);
    assert_eq!(listed.iter().filter(|id| *id == STYLE_CORE).count(), 1, "{listed:?}");
    let row = definitions(&run).get(STYLE_CORE).expect("the row");
    assert_eq!(row.owner.as_str(), "t");
    let page = rendered_page(&run);
    assert!(page.contains(".second{}"), "{page}");
    assert_eq!(page.matches(&format!("data-flm-def=\"{STYLE_CORE}\"")).count(), 1, "{page}");
}

#[test]
fn the_feature_order_decides_which_feature_defines_an_identifier_two_declare() {
    // The chain places the toy feature before the backend: the backend's own
    // declaration is the definition.
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(requiring_feature(
        None,
        None,
        vec![toy_style(STYLE_CORE, vec![DefContent::Inline(String::from(".second{}"))])],
    ));
    builder.feature_order(FeatureChain::parse(&format!("t < {}", html::FEATURE_NAME), "test").expect("a chain"));
    let registry = builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()));
    let run = run_workflow(&registry, html::WORKFLOW, &["Text.\n\nMore text.\n"], &Overrides::none());
    let row = definitions(&run).get(STYLE_CORE).expect("the row");
    assert_eq!(row.owner.as_str(), html::FEATURE_NAME);
    let page = rendered_page(&run);
    assert!(!page.contains(".second{}"), "{page}");
}

#[test]
fn a_requirement_no_feature_declares_is_an_error_and_the_page_is_still_produced() {
    let registry = registry_with(requiring_feature(Some("t.style.missing"), None, Vec::new()));
    let run = run_workflow(&registry, html::WORKFLOW, &["A \\hl{word}.\n\nMore.\n"], &Overrides::none());
    expect(run.diagnostics().iter(), "flm.html.unknown-requirement")
        .severity(Severity::Error)
        .once();
    assert!(!listed(&run).contains(&String::from("t.style.missing")));
    let page = rendered_page(&run);
    assert!(page.contains("<main class=\"flm-content\""), "{page}");
    assert!(page.contains("word"), "{page}");
}

/// The table of the toy feature that supplies a part of its stylesheet.
const PART_TABLE: TableName<DefinitionText> = TableName::new("t.part");

/// The document-scope stage of the toy feature that writes [`PART_TABLE`], or
/// writes nothing when its text is empty.
struct WritePart(&'static str);

impl Stage for WritePart {
    fn decl(&self) -> StageDecl {
        StageDecl::new("t.write-part", Effect::Compute, Scope::Document)
            .writes(PART_TABLE)
            .docs("writes the text of a stylesheet's part")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, _cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        if !self.0.is_empty() {
            *out.table(PART_TABLE).expect("declared") = DefinitionText::new(self.0);
        }
    }
}

/// [`requiring_feature`] requiring `t.style.table`, a stylesheet whose first
/// part is the text of [`PART_TABLE`] and whose second is `.after{}`, with the
/// table declared and, when `stage` is given, a stage writing it.
fn table_part_feature(stage: Option<&'static str>) -> Arc<dyn FlmFeature> {
    let inner = requiring_feature(
        Some("t.style.table"),
        None,
        vec![toy_style(
            "t.style.table",
            vec![
                DefContent::Table(CapName::from(PART_TABLE.name())),
                DefContent::Inline(String::from(".after{}")),
            ],
        )],
    );
    common::pipeline::feature("t", move |b: &mut FeatureBuilder<'_>| -> Result<(), BuildError> {
        inner.register(b)?;
        b.table(PART_TABLE, "the text of a stylesheet's part")?;
        if let Some(text) = stage {
            b.stage(Arc::new(WritePart(text)))?;
        }
        Ok(())
    })
}

/// The overrides that schedule the stage writing [`PART_TABLE`] where the
/// registry has one: the resolve stage reads the table optionally, and an
/// optional read schedules no stage, so the feature that declares the
/// definition sees to it (here, the request demands the table).
fn writing_the_part() -> Overrides {
    Overrides { demands: vec![CapName::from(PART_TABLE.name())], ..Overrides::none() }
}

#[test]
fn a_part_supplied_by_a_table_reaches_the_page_as_its_text() {
    let registry = registry_with(table_part_feature(Some(".from-table{}")));
    let run = run_workflow(&registry, html::WORKFLOW, &["A \\hl{word}.\n\nMore.\n"], &writing_the_part());
    expect_none(run.diagnostics().iter(), "flm.html.definition-part-missing");
    let row = definitions(&run).get("t.style.table").expect("the row");
    assert_eq!(
        row.delivery,
        DefinitionOverride::Parts(vec![
            DefContent::Inline(String::from(".from-table{}")),
            DefContent::Inline(String::from(".after{}")),
        ])
    );
    let page = rendered_page(&run);
    let table = page.find(".from-table{}").expect("the table's text");
    assert!(table < page.find(".after{}").expect("the other part"), "{page}");
}

#[test]
fn a_part_whose_table_is_absent_is_an_error_and_the_other_parts_are_written() {
    for stage in [None, Some("")] {
        let registry = registry_with(table_part_feature(stage));
        // Without a stage there is nothing to demand: the table is absent.
        let overrides = if stage.is_some() { writing_the_part() } else { Overrides::none() };
        let run =
            run_workflow(&registry, html::WORKFLOW, &["A \\hl{word}.\n\nMore.\n"], &overrides);
        let reported = expect(run.diagnostics().iter(), "flm.html.definition-part-missing")
            .severity(Severity::Error)
            .once();
        let condition = match stage {
            None => "no stage wrote that table",
            Some(_) => "the table `t.part` holds no text",
        };
        assert_eq!(
            reported.message(),
            format!("the stylesheet `t.style.table` takes a part from the table `t.part`, but {condition}")
        );
        let row = definitions(&run).get("t.style.table").expect("the row");
        assert_eq!(
            row.delivery,
            DefinitionOverride::Parts(vec![
                DefContent::Inline(String::new()),
                DefContent::Inline(String::from(".after{}")),
            ])
        );
        let page = rendered_page(&run);
        assert!(page.contains(".after{}"), "{page}");
    }
}

#[test]
fn a_part_naming_an_undeclared_table_or_a_table_of_another_type_fails_the_registry_build() {
    // The part names `t.missing`, which no feature declares, then `t.counts`,
    // which holds another value type than `DefinitionText`.
    let feature = common::pipeline::feature("t", |b: &mut FeatureBuilder<'_>| -> Result<(), BuildError> {
        b.table_named::<DefinitionText>("t.part", "the text of a part")?;
        b.table_named::<ArtifactSet>("t.counts", "a table of another type")?;
        b.style_def(toy_style(
            "t.style.parts",
            vec![
                DefContent::Table(CapName::from("t.part")),
                DefContent::Table(CapName::from("t.missing")),
                DefContent::Table(CapName::from("t.counts")),
            ],
        ))
    });
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(feature);
    let Err(error) = builder.build(&RegistryInputs::new()) else { panic!("the registry builds") };
    let text = error.render();
    assert!(
        text.contains(
            "the table `t.missing` that the style definition `t.style.parts` takes a part from must be \
             declared by a feature of the registry"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "the table `t.counts` that the style definition `t.style.parts` takes a part from must hold \
             `DefinitionText`, not `ArtifactSet`"
        ),
        "{text}"
    );
    assert!(!text.contains("`t.part`"), "{text}");
}

#[test]
fn a_table_part_projects_as_the_tables_name() {
    let table = ResolvedDefinitions {
        definitions: vec![flm_core::stdcore::backends::html::ResolvedDefinition {
            id: Name::from("t.style.table"),
            kind: DefinitionKind::Style,
            owner: flm_core::model::names::FeatureName::from("t"),
            delivery: DefinitionOverride::Parts(vec![DefContent::Table(CapName::from("t.part"))]),
        }],
    };
    let text = flm_core::dump::json::to_json(&table.project(&NoRefs)).expect("JSON");
    assert!(text.contains(r#""parts":[{"table":"t.part"}]"#), "{text}");
}

/// A projection context for a table that refers to no node and no tree.
struct NoRefs;

impl ProjectCx for NoRefs {
    fn node_ref(&self, _: flm_core::model::addr::NodeAddr) -> techy::serialize::SerialValue {
        techy::serialize::SerialValue::Null
    }

    fn tree_ref(&self, _: flm_core::model::addr::TreeId) -> techy::serialize::SerialValue {
        techy::serialize::SerialValue::Null
    }
}

#[test]
fn a_file_part_is_an_error_and_the_rest_of_the_definition_is_still_written() {
    let registry = registry_with(requiring_feature(
        Some("t.style.file"),
        None,
        vec![toy_style(
            "t.style.file",
            vec![
                DefContent::File(String::from("marked.css")),
                DefContent::Inline(String::from(".marked{}")),
            ],
        )],
    ));
    let run = run_workflow(&registry, html::WORKFLOW, &["A \\hl{word}.\n\nMore.\n"], &Overrides::none());
    let reported = expect(run.diagnostics().iter(), "flm.html.definition-not-inlinable")
        .severity(Severity::Error)
        .once();
    // The condition says which case it was and carries that case's own facts,
    // so a host reads them rather than the message.
    assert_eq!(
        reported.data().downcast_ref::<DefinitionNotInlinable>(),
        Some(&DefinitionNotInlinable {
            definition: String::from("t.style.file"),
            part: NotInlinablePart::File { path: String::from("marked.css") },
        })
    );
    let page = rendered_page(&run);
    assert!(!page.contains("marked.css"), "{page}");
    assert!(page.contains(".marked{}"), "{page}");
    // The table says what the definition declares, whatever the page could
    // write of it.
    let row = definitions(&run).get("t.style.file").expect("the row");
    assert!(matches!(&row.delivery, DefinitionOverride::Parts(parts) if parts.len() == 2));
}

#[test]
fn a_text_part_holding_the_elements_own_closing_tag_is_an_error_and_is_left_out() {
    let options = overriding(
        STYLE_CORE,
        DefinitionOverride::Parts(vec![DefContent::Inline(String::from(
            ".a{} </style><script>alert(1)</script>",
        ))]),
    );
    let run = render_with(&std_registry(), &["Text.\n\nMore text.\n"], options);
    let reported = expect(run.diagnostics().iter(), "flm.html.definition-not-inlinable")
        .severity(Severity::Error)
        .once();
    // The other case of the same condition: the element the text would have
    // gone into, and where in the text its closing tag stands.
    assert_eq!(
        reported.data().downcast_ref::<DefinitionNotInlinable>(),
        Some(&DefinitionNotInlinable {
            definition: String::from(STYLE_CORE),
            part: NotInlinablePart::ClosingTag { tag: String::from("style"), at: 5 },
        })
    );
    let page = rendered_page(&run);
    // Nothing of the refused part reaches the page — not the rule before the
    // closing tag, and not the script after it.
    assert!(!page.contains("alert(1)"), "{page}");
    assert!(!page.contains(".a{}"), "{page}");
    // The page is still produced, with the definitions that could be written.
    assert!(page.contains("<main class=\"flm-content\""), "{page}");
    assert!(page.contains(&format!("data-flm-def=\"{STYLE_PAGE}\"")), "{page}");
}

#[test]
fn a_part_still_naming_its_table_is_refused_by_the_page_and_the_other_parts_are_written() {
    // The resolve stage puts a table's text in place of the part; a host's
    // override is taken as it stands, so a part naming a table reaches the
    // page composer only that way.
    let options = overriding(
        STYLE_CORE,
        DefinitionOverride::Parts(vec![
            DefContent::Table(CapName::from("t.part")),
            DefContent::Inline(String::from(".kept{}")),
        ]),
    );
    let run = render_with(&std_registry(), &["Text.\n\nMore text.\n"], options);
    let reported = expect(run.diagnostics().iter(), "flm.html.definition-not-inlinable")
        .severity(Severity::Error)
        .once();
    let data = reported.data().downcast_ref::<DefinitionNotInlinable>().expect("the condition");
    assert_eq!(
        data,
        &DefinitionNotInlinable {
            definition: String::from(STYLE_CORE),
            part: NotInlinablePart::Table { table: String::from("t.part") },
        }
    );
    assert_eq!(data.part.kind(), "table");
    let page = rendered_page(&run);
    assert!(page.contains(".kept{}"), "{page}");
}

// --- the page, and the manifest ----------------------------------------------------------

/// The one page golden that carries the true bytes of a stylesheet this
/// backend ships, and the one that shows all four deliveries side by side.
///
/// The document requires four definitions, and each arrives a different way:
/// `flm.html.style.core` is **not** overridden, so the page holds the whole of
/// the chunk exactly as this backend declares it — the property no other
/// golden pins, and the diff that shows when a rule of that chunk changes;
/// `flm.html.style.page` arrives as the host's own text; `flm.html.style.math-passthrough`
/// as an address; and the formula renderer is the host's own, so nothing is
/// written for it and the table still lists it. The other page goldens
/// (`tests/html_pipeline.rs`) deliver their stylesheets by address, so that
/// each pins its own property and no golden repeats these bytes.
#[test]
fn a_page_whose_definitions_arrive_in_every_way() {
    let mut definitions = BTreeMap::new();
    definitions.insert(
        Name::from(STYLE_PAGE),
        DefinitionOverride::Parts(vec![DefContent::Inline(String::from(
            "body { margin: 2rem }\n",
        ))]),
    );
    definitions.insert(Name::from(SCRIPT_MATHJAX), DefinitionOverride::ProvidedByHost);
    definitions.insert(
        Name::from(STYLE_MATH_PASSTHROUGH),
        DefinitionOverride::Parts(vec![DefContent::Url(String::from("/css/math-passthrough.css"))]),
    );
    let options = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
    let run = render_with(&std_registry(), &["A formula \\(x^2\\).\n\nMore.\n"], options);
    let page = rendered_page(&run);
    // The chunk nobody overrode reaches the page as it stands, marker line
    // and all.
    assert!(page.contains(&format!("/* {STYLE_CORE} */")), "{page}");
    assert_golden("definitions.page.html", &page);
}

/// The manifest an external bundler works from, as `flm dump --table
/// flm.html.definitions` prints it: one entry per required definition, with
/// its kind, its owner, whether the host supplies it, and its parts.
///
/// Every part here is short on purpose — the chunk this backend ships is
/// replaced by a line of the host's own — because what this golden pins is the
/// manifest's shape, and a manifest holding a whole stylesheet in JSON string
/// escapes is one nobody can read. The bytes of a shipped chunk are pinned by
/// the page golden above.
#[test]
fn the_manifest_lists_every_definition_with_its_delivery() {
    let mut definitions = BTreeMap::new();
    definitions.insert(
        Name::from(STYLE_CORE),
        DefinitionOverride::Parts(vec![DefContent::Inline(String::from(
            ".flm-pres-par { margin: 0 }\n",
        ))]),
    );
    definitions.insert(
        Name::from(STYLE_PAGE),
        DefinitionOverride::Parts(vec![DefContent::Url(String::from("/css/page.css"))]),
    );
    definitions.insert(Name::from(SCRIPT_MATHJAX), DefinitionOverride::ProvidedByHost);
    definitions.insert(
        Name::from(STYLE_MATH_PASSTHROUGH),
        DefinitionOverride::Parts(vec![DefContent::Url(String::from("/css/math-passthrough.css"))]),
    );
    let options = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
    let run = render_with(&std_registry(), &["A formula \\(x^2\\).\n\nMore.\n"], options);
    assert_json_golden(
        golden_path("definitions.json"),
        &common::stdcore::projection(&run, DEFINITIONS_TABLE),
    );
}

// --- what this backend ships -------------------------------------------------------------

#[test]
fn the_eight_stylesheets_and_the_one_script_are_registered_under_this_backend() {
    let registry = std_registry();
    let styles: Vec<String> = registry
        .styles()
        .iter()
        .filter(|(owner, _)| owner.as_str() == html::FEATURE_NAME)
        .map(|(_, style)| style.id.to_string())
        .collect();
    assert_eq!(
        styles,
        [
            STYLE_CORE,
            STYLE_HEADINGS,
            STYLE_VERBATIM,
            STYLE_MATH_PASSTHROUGH,
            STYLE_LISTS,
            STYLE_ERRORS,
            STYLE_PAGE
        ]
    );
    let scripts: Vec<String> =
        registry.scripts().iter().map(|(_, script)| script.id.to_string()).collect();
    assert_eq!(scripts, [SCRIPT_MATHJAX]);
    // The eight other stylesheets of the package are the xnotes, the defterm,
    // the theorems, the quote, the floats, the cells, the citations and the
    // toc features' own; the headings feature's style definition, first, and
    // the meta feature's, before the toc's, hold no part.
    let others: Vec<(String, String)> = registry
        .styles()
        .iter()
        .filter(|(owner, _)| owner.as_str() != html::FEATURE_NAME)
        .map(|(owner, style)| (owner.to_string(), style.id.to_string()))
        .collect();
    assert_eq!(
        others,
        [
            (String::from("headings"), String::from("flm.headings.style")),
            (String::from("xnotes"), String::from("flm.xnotes.style")),
            (String::from("defterm"), String::from("flm.defterm.style")),
            (String::from("theorems"), String::from("flm.theorems.style")),
            (String::from("quote"), String::from("flm.quote.style")),
            (String::from("floats"), String::from("flm.floats.style")),
            (String::from("cells"), String::from("flm.cells.style")),
            (String::from("citations"), String::from("flm.citations.style")),
            (String::from("meta"), String::from("flm.meta.style")),
            (String::from("toc"), String::from("flm.toc.style")),
        ]
    );
    // The one script is this backend's own.
    for (owner, _) in registry.scripts() {
        assert_eq!(owner.as_str(), html::FEATURE_NAME);
    }
}

#[test]
fn the_formula_renderers_configuration_is_what_the_backends_output_needs() {
    let script = mathjax_script();
    assert_eq!(script.content.len(), 2);
    let DefContent::Inline(configuration) = &script.content[0] else {
        panic!("the configuration is the first part");
    };
    let DefContent::Url(address) = &script.content[1] else {
        panic!("the address is the second part");
    };
    assert_eq!(address, MATHJAX_LOADER);
    // The marker every piece of text this backend ships opens with.
    assert_eq!(configuration.lines().next(), Some("/* flm.html.mathjax */"));
    // The configuration object itself, apart from the head comment: the
    // absence check below reads this, so that a sentence of that comment can
    // neither make it pass nor make it fail.
    let object = &configuration[configuration
        .find("window.MathJax")
        .expect("the configuration declares window.MathJax")..];
    // Every key, each one a thing the page depends on. The configuration holds
    // what this backend's output needs and nothing that restricts an author:
    // mathematics is not policed.
    for key in [
        "loader: {load: ['[tex]/texhtml']}",
        "packages: {'[+]': ['texhtml']}",
        "allowTexHTML: true",
        "inlineMath: [['\\\\(', '\\\\)']]",
        "displayMath: [['\\\\[', '\\\\]']]",
        "processEnvironments: true",
        "processRefs: false",
        "tags: 'none'",
        "chtml: {mtextInheritFont: true}",
    ] {
        assert!(object.contains(key), "the configuration is missing `{key}`");
    }
    // Nothing that polices what an author writes inside a formula.
    for absent in ["ui/safe", "safeOptions", "autoload", "'[-]'"] {
        assert!(
            !object.contains(absent),
            "the configuration must not hold `{absent}`: mathematics is not policed",
        );
    }
    // The class the renderer is told to typeset inside is the class of the
    // element that holds a formula's source, derived rather than spelled out:
    // a rename of that element must not leave the configuration behind, and a
    // hand-written literal here would pass a stale configuration.
    let selector = format!(".{}", class_of_element(pres::MATH_PASSTHROUGH));
    assert!(
        object.contains(&format!("startup: {{elements: ['{selector}']}}")),
        "the configuration must typeset inside `{selector}`",
    );
    // After the object stands the listener that makes a link inside a formula
    // follow when it is clicked: the renderer takes the address off such a link
    // and keeps it in `data-mjx-href`. Its selector names the class of the link
    // element, derived here for the same reason as the one above.
    let link = format!("a.{}[data-mjx-href]", class_of_element(pres::LINK));
    assert!(
        object.contains("document.addEventListener('click'"),
        "the configuration must carry the click listener",
    );
    assert!(
        object.contains(&format!("'mjx-container mjx-html {link}'")),
        "the listener must act only on `{link}` inside a typeset formula",
    );
    assert!(
        object.contains("window.location.href = link.getAttribute('data-mjx-href')"),
        "the listener must go to the address the renderer moved",
    );
}

// --- the stylesheets' own rules ----------------------------------------------------------

/// Every stylesheet the standard package registers, in registration order:
/// this backend's own, and those content features ship with their markup.
/// Each is held to the same rules.
fn stylesheets() -> Vec<StyleDef> {
    // The headings feature's style definition holds no part: it names no
    // stylesheet, only the identifier its general rules require.
    std_registry()
        .styles()
        .iter()
        .filter(|(_, style)| !style.content.is_empty())
        .map(|(_, style)| style.clone())
        .collect()
}

/// `text` with every `/* … */` comment removed.
fn without_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        match rest[at..].find("*/") {
            Some(end) => rest = &rest[at + end + 2..],
            None => panic!("a comment is never closed"),
        }
    }
    out.push_str(rest);
    out
}

/// The selectors of one stylesheet: the text standing before each `{` that is
/// neither the layer's own nor a declaration block's, split at each comma.
fn selectors(css: &str) -> Vec<String> {
    let mut found = Vec::new();
    for piece in without_comments(css).split('{') {
        let prelude = piece.rsplit('}').next().unwrap_or("").trim();
        // The layer, and a media condition, are no selectors: the rules
        // inside them are read as every other rule is.
        if prelude.is_empty() || prelude.starts_with("@layer") || prelude.starts_with("@media") {
            continue;
        }
        for selector in prelude.split(',') {
            found.push(selector.trim().to_string());
        }
    }
    found
}

#[test]
fn every_stylesheet_is_one_layer_block_that_declares_what_it_uses() {
    for style in stylesheets() {
        let id = style.id.to_string();
        let DefContent::Inline(css) = &style.content[0] else { panic!("{id} is written text") };
        assert_eq!(style.content.len(), 1, "{id}");
        // The marker naming the definition, then a comment saying what the
        // chunk is for, then one layer block.
        assert_eq!(
            css.lines().next(),
            Some(format!("/* {id} */").as_str()),
            "{id} does not open with its own marker"
        );
        assert!(css.lines().nth(1).is_some_and(|line| line.starts_with("/*")),
            "{id} says nowhere what it is for");
        assert_eq!(css.matches("@layer").count(), 1, "{id}");
        assert!(css.contains("@layer flm {"), "{id}");
        // Every custom property the chunk uses is declared in the chunk, on
        // `:root`, so that no chunk depends on another — or is one the page
        // writes in an element's `style` attribute for a length field, which
        // the chunk applies with a fallback.
        let text = without_comments(css);
        let written: Vec<String> = html::CONVENTIONS
            .iter()
            .flat_map(|convention| convention.fields.iter())
            .filter_map(|field| field.style_property())
            .collect();
        for used in text.match_indices("var(--") {
            let rest = &text[used.0 + 4..];
            let property = &rest[..rest.find([')', ',']).expect("a closed var()")];
            assert!(
                text.contains(&format!("{property}:")) || written.iter().any(|w| w == property),
                "{id} uses `{property}` and does not declare it"
            );
            assert!(property.starts_with("--flm-"), "{id} uses `{property}`");
        }
    }
}

/// The text of the stylesheet `id` declares, comments removed.
fn stylesheet_text(id: &str) -> String {
    let style = stylesheets().into_iter().find(|style| style.id == id).expect(id);
    let DefContent::Inline(css) = &style.content[0] else { panic!("{id} is written text") };
    without_comments(css)
}

/// The value `css` declares for `property`, the first time it declares it.
fn declared(css: &str, property: &str) -> String {
    let at = css.find(&format!("{property}:")).unwrap_or_else(|| panic!("{property} undeclared"));
    let rest = &css[at + property.len() + 1..];
    rest[..rest.find(';').expect("a declaration ends")].trim().to_string()
}

#[test]
fn the_page_leaves_the_column_of_notes_the_core_chunk_sets_them_in() {
    // Both chunks declare the column's width and gap; they must agree, or the
    // page would leave a column of another width than the notes take.
    let core = stylesheet_text(STYLE_CORE);
    let page = stylesheet_text(STYLE_PAGE);
    for property in ["--flm-margin-note-width", "--flm-margin-note-gap"] {
        assert_eq!(declared(&core, property), declared(&page, property), "{property}");
    }
    // The core chunk holds no width condition: the page chunk's one condition
    // turns the core chunk's switch on, where it leaves the column.
    assert!(!core.contains("@media"), "{core}");
    assert_eq!(page.matches("@media").count(), 1, "{page}");
    let condition = &page[page.find("@media").unwrap()..];
    assert!(condition.contains("--flm-margin-note-column: ;"), "{page}");
}

/// One selector each stylesheet must hold: the rule for the content the chunk
/// exists to style.
///
/// The Python implementation ships one stylesheet and its tests look for a
/// substring of it. Here the rules are split into chunks by concern, each
/// delivered only where the content it styles occurs, so the same question is
/// asked of each chunk: does it still style what it is named for? A chunk that
/// lost its rules would otherwise stay green — it would still be one layer
/// block of contract selectors, and still be delivered.
const STYLED: [(&str, &str); 16] = [
    (STYLE_CORE, ".flm-pres-deco-runin"),
    (STYLE_HEADINGS, ".flm-pres-block--heading"),
    (STYLE_VERBATIM, ".flm-pres-verbatim"),
    (STYLE_MATH_PASSTHROUGH, ".flm-pres-math--passthrough[data-flm-display=\"display\"]"),
    (STYLE_LISTS, ".flm-pres-list"),
    (STYLE_ERRORS, ".flm-pres-unknown-marker"),
    (STYLE_PAGE, "body"),
    (xnotes::STYLE, ".flm-pres-block[data-flm-kind~=\"xnotes\"][role=\"doc-endnotes\"]"),
    (defterm::html::STYLE, ".flm-pres-span.flm-defterm-term"),
    (
        theorems::html::STYLE,
        ".flm-theorems-theorem > .flm-pres-par > .flm-pres-deco-runin > .flm-theorems-name",
    ),
    (quote::html::STYLE, ".flm-quote > .flm-quote-attributed"),
    (floats::html::STYLE, ".flm-pres-block.flm-floats-float"),
    (cells::STYLE, ".flm-pres-cells [data-flm-align=\"left\"]"),
    (citations::html::STYLE, ".flm-citations-mark[data-flm-style=\"superscript\"]"),
    (toc::STYLE, ".flm-toc .flm-pres-list[data-flm-kind~=\"toc\"] .flm-pres-list[data-flm-kind~=\"toc\"]"),
    (meta::STYLE, ".flm-pres-block.flm-meta-title[data-flm-kind~=\"meta\"]"),
];

#[test]
fn every_stylesheet_holds_a_rule_for_the_content_it_is_named_for() {
    let mut seen = Vec::new();
    for style in stylesheets() {
        let id = style.id.to_string();
        let DefContent::Inline(css) = &style.content[0] else { panic!("{id} is written text") };
        let (_, wanted) = STYLED
            .iter()
            .find(|(named, _)| *named == id)
            .unwrap_or_else(|| panic!("{id} is in no row of STYLED"));
        assert!(
            selectors(css).iter().any(|selector| selector == wanted),
            "{id} holds no rule for `{wanted}`"
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), STYLED.len(), "a row of STYLED names no stylesheet: {seen:?}");
}

/// Every class the contract holds: the class of each presentation element the
/// conventions table has a row for, the class of each role a node may carry —
/// a registered element's name, or a part of one a feature publishes —
/// and the three fixed classes that stand for no element.
///
/// A role is a registered element's name (a heading's role is the heading
/// element's name), and any registered name may be used as one, so every
/// registered element contributes a role class. That is wider than the roles
/// the standard package actually writes today, and deliberately so: a feature
/// may give a node a role of its own without changing this set.
fn contract_classes() -> BTreeSet<String> {
    let mut classes: BTreeSet<String> = html::CONVENTIONS
        .iter()
        .map(|convention| class_of_element(convention.element))
        .collect();
    for entry in std_registry().elements() {
        classes.insert(class_of_role(entry.element.name.as_str()));
    }
    // The roles that name a part of an element, each published by the feature
    // writing it.
    for role in [
        xnotes::MARK_ROLE,
        xnotes::INPLACE_ROLE,
        xnotes::ENTRY_MARK_ROLE,
        quote::TEXT_ROLE,
        quote::LINES_ROLE,
        quote::ATTRIBUTED_ROLE,
        quote::BLOCK_ROLE,
        floats::CONTENT_ROLE,
        floats::CAPTION_ROLE,
        floats::NUMBER_ROLE,
        citations::MARK_ROLE,
        citations::WORD_ROLE,
        citations::SPACE_ROLE,
        citations::ENTRY_NUMBER_ROLE,
    ] {
        classes.insert(class_of_role(role));
    }
    classes.extend(html::AUXILIARY_CLASSES.iter().map(|class| String::from(*class)));
    classes
}

/// The class a selector begins with: the characters of the first compound
/// selector after its leading `.`, or `None` when the selector does not begin
/// with a class at all.
fn leading_class(selector: &str) -> Option<String> {
    let rest = selector.strip_prefix('.')?;
    let end = rest
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .unwrap_or(rest.len());
    Some(String::from(&rest[..end]))
}

#[test]
fn every_selector_begins_with_a_class_of_the_contract() {
    let contract = contract_classes();
    for style in stylesheets() {
        let id = style.id.to_string();
        let DefContent::Inline(css) = &style.content[0] else { panic!("{id} is written text") };
        for selector in selectors(css) {
            if selector == ":root" {
                continue;
            }
            // The page's own chunk styles the page, which is not a document:
            // its rules on `body` are the one exception, and its own text says
            // so.
            if id == STYLE_PAGE && selector == "body" {
                continue;
            }
            // A class of the contract, not merely a class spelled like one: a
            // misspelling, or a class of a chunk nothing requires, is what a
            // check of the prefix alone lets through.
            let class = leading_class(&selector).unwrap_or_else(|| {
                panic!("{id}: the selector `{selector}` does not begin with a class")
            });
            assert!(
                contract.contains(&class),
                "{id}: the selector `{selector}` begins with `{class}`, which is not a class \
                 of the contract"
            );
            // An attribute selector names an attribute of the contract: every
            // one, `data-flm-*` or not, so that a misspelled `role` or `id` is
            // caught too.
            for (at, _) in selector.match_indices('[') {
                let rest = &selector[at + 1..];
                let attribute = rest[..rest
                    .find(['=', ']', '~', '|', '^', '$', '*'])
                    .expect("closed")]
                    .trim();
                assert!(
                    html::CONVENTIONS.iter().any(|convention| {
                        convention.fields.iter().any(|field| {
                            field.attribute().as_deref() == Some(attribute)
                                // An attribute written on each entry of a list
                                // or each cell of a table, a name ending in a
                                // hyphen being a family of them.
                                || field.entry_attribute().is_some_and(|name| {
                                    name == attribute
                                        || (name.ends_with('-') && attribute.starts_with(name))
                                })
                        })
                    }),
                    "{id}: `{attribute}` is not an attribute the emitter writes"
                );
            }
        }
    }
}
