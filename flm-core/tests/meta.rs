//! The document's metadata through the library alone: the `meta` feature's
//! option value set on the document, its three pieces placed before the
//! content by the text and HTML backends, and a problem in a text of the
//! metadata reported at its place.

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::TableName;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::DOCUMENT;
use flm_core::stdcore::backends::text::TEXT_DOCUMENT;
use flm_core::stdcore::features::meta::{DocMeta, MetaPlace, MetaText};
use flm_core::stdcore::{StdcoreConfig, package, CHECK_WORKFLOW};
use techy::error::Severity;

fn registry() -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// A run of `workflow` over the document `text` carrying `meta`.
fn run(registry: &Arc<FlmRegistry>, workflow: &str, text: &str, meta: Option<DocMeta>) -> Run {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::new("paper.flm")));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("a parse");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let options = meta.map_or_else(OptionValues::new, |meta| OptionValues::new().with(meta));
    let doc = Arc::new(Doc::with_options(content, options));
    let plan = Arc::new(Plan::for_workflow(registry, workflow, &Overrides::none()).expect("a plan"));
    let mut run = Run::new(doc, plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

fn entry(run: &Run, table: TableName<ArtifactSet>) -> String {
    let set = run.table(table).expect("declared").unwrap_or_else(|| {
        panic!("no output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

fn meta() -> DocMeta {
    DocMeta::default()
        .title(MetaText::new(r"On \emph{flexible} markup and \(x^2\)"))
        .author(MetaText::new("Ada Lovelace"))
        .author(MetaText::new(r"Grace \textbf{Hopper}"))
        .date(MetaText::new("September 2026"))
}

#[test]
fn the_text_output_shows_the_title_the_authors_and_the_date_before_the_content() {
    let registry = registry();
    let run = run(&registry, "text", "The content.", Some(meta()));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let text = entry(&run, TEXT_DOCUMENT);
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("`{needle}` in:\n{text}"));
    assert!(at("On flexible markup and") < at("Ada Lovelace, Grace Hopper"), "{text}");
    assert!(at("x^2") < at("Ada Lovelace"), "{text}");
    assert!(at("Ada Lovelace, Grace Hopper") < at("September 2026"), "{text}");
    assert!(at("September 2026") < at("The content."), "{text}");
}

#[test]
fn the_html_output_keeps_each_author_apart_and_the_title_formatted() {
    let registry = registry();
    let run = run(&registry, "html", "The content.", Some(meta()));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let page = entry(&run, DOCUMENT);
    let at = |needle: &str| page.find(needle).unwrap_or_else(|| panic!("`{needle}` in:\n{page}"));
    assert!(page.contains("<em"), "{page}");
    assert!(at("flm-meta-title") < at("flm-meta-author"), "{page}");
    assert_eq!(page.matches("flm-meta-author-name").count(), 2, "{page}");
    assert!(at("Grace") < at("September 2026"), "{page}");
    assert!(at("September 2026") < at("The content."), "{page}");
}

#[test]
fn a_document_with_no_metadata_places_nothing_and_reports_nothing() {
    let registry = registry();
    let without = run(&registry, "text", "The content.", None);
    let empty = run(&registry, "text", "The content.", Some(DocMeta::default()));
    assert!(!without.diagnostics().any_recorded());
    assert_eq!(entry(&without, TEXT_DOCUMENT), entry(&empty, TEXT_DOCUMENT));
    assert_eq!(entry(&without, TEXT_DOCUMENT), "The content.\n");
}

#[test]
fn a_problem_in_the_title_is_reported_at_its_line_in_the_file_even_when_only_checking() {
    let registry = registry();
    let meta = DocMeta::default()
        .title(MetaText::new(r"A \nosuchcommand title").at(MetaPlace::new("paper.flm", 2, 8)));
    for workflow in ["text", CHECK_WORKFLOW] {
        let run = run(&registry, workflow, "The content.", Some(meta.clone()));
        let error = run
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.severity() == Severity::Error)
            .unwrap_or_else(|| panic!("an error under `{workflow}`"));
        let rendered = run.diagnostics().render_all();
        assert!(rendered.contains("(line 2, col 10) [paper.flm]"), "{workflow}: {rendered}");
        assert!(error.span().content().contains("nosuchcommand"), "{workflow}: {rendered}");
    }
}
