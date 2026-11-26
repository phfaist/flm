//! The document's metadata through the library alone: the `metadata` feature's
//! option value set on the document, the authors piece its stage builds, its
//! three pieces placed before the content by the text and HTML backends, and a
//! problem in a text of the metadata reported at its place.

mod common;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::{TableHandle, TableName};
use flm_core::core::piece::Piece;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdlib::backends::html::DOCUMENT;
use flm_core::stdlib::backends::text::TEXT_DOCUMENT;
use flm_core::stdlib::features::metadata::{
    AffiliationPayload, DocMeta, MetaAuthor, MetaPlace, MetaText, AUTHOR, AUTHOR_AFFILIATION, AUTHOR_EMAIL,
    AUTHOR_ENTRY, AUTHOR_NAME, AUTHOR_NOTE,
};
use flm_core::stdlib::{StdlibConfig, add_features, CHECK_WORKFLOW};
use techy::core::node::NodeRef;
use techy::error::Severity;

fn registry() -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(add_features(PackageEntry::new("std"), StdlibConfig::new()));
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
        .author(MetaAuthor::new(MetaText::new("Ada Lovelace")))
        .author(MetaAuthor::new(MetaText::new(r"Grace \textbf{Hopper}")))
        .date(MetaText::new("September 2026"))
}

#[test]
fn an_author_s_node_holds_its_parts_in_order_and_one_affiliation_has_one_number() {
    let registry = registry();
    let ada = MetaAuthor::new(MetaText::new("Ada"))
        .affiliation(MetaText::new("University of X"))
        .affiliation(MetaText::new("Institute of Y"))
        .email(MetaText::new("ada@example.com"))
        .note(MetaText::new("Corresponding author"));
    let grace = MetaAuthor::new(MetaText::new("Grace")).affiliation(MetaText::new(" Institute of Y "));
    let run = run(&registry, "text", "The content.", Some(DocMeta::default().author(ada).author(grace)));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let piece = run.table(&TableHandle::<Piece>::new(AUTHOR)).expect("declared").expect("written");
    let tree = piece.content.as_ref().expect("the authors");
    let entries: Vec<NodeRef<'_, Flm>> =
        tree.root().descendants().filter(|node| node.ext().element.name.as_str() == AUTHOR_ENTRY).collect();
    assert_eq!(entries.len(), 2);
    let parts = |entry: &NodeRef<'_, Flm>| -> Vec<String> {
        entry.children().iter().map(|part| part.ext().element.name.as_str().to_string()).collect()
    };
    assert_eq!(parts(&entries[0]), [AUTHOR_NAME, AUTHOR_AFFILIATION, AUTHOR_AFFILIATION, AUTHOR_EMAIL, AUTHOR_NOTE]);
    assert_eq!(parts(&entries[1]), [AUTHOR_NAME, AUTHOR_AFFILIATION]);
    let numbers = |entry: &NodeRef<'_, Flm>| -> Vec<usize> {
        entry.children().iter().filter_map(|part| AffiliationPayload::of(&part).map(|payload| payload.number)).collect()
    };
    assert_eq!(numbers(&entries[0]), [1, 2]);
    assert_eq!(numbers(&entries[1]), [2]);
}

#[test]
fn the_text_output_shows_the_title_the_authors_and_the_date_before_the_content() {
    let registry = registry();
    let run = run(&registry, "text", "The content.", Some(meta()));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let text = entry(&run, TEXT_DOCUMENT);
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("`{needle}` in:\n{text}"));
    assert!(at("On flexible markup and") < at("Ada Lovelace"), "{text}");
    assert!(at("x^2") < at("Ada Lovelace"), "{text}");
    assert!(at("Ada Lovelace") < at("Grace Hopper") && at("Grace Hopper") < at("September 2026"), "{text}");
    assert!(at("September 2026") < at("The content."), "{text}");
}

#[test]
fn the_html_output_keeps_each_author_apart_and_the_title_formatted() {
    let registry = registry();
    let run = run(&registry, "html", "The content.", Some(meta()));
    common::stdlib::assert_only_unchecked_output(&run);
    let page = entry(&run, DOCUMENT);
    let at = |needle: &str| page.find(needle).unwrap_or_else(|| panic!("`{needle}` in:\n{page}"));
    assert!(page.contains("<em"), "{page}");
    assert!(at("flm-metadata-title") < at("flm-metadata-author"), "{page}");
    assert_eq!(page.matches("flm-metadata-author-name").count(), 2, "{page}");
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
        let mut lines = techy::source::LineIndexCache::<FlmOrigin>::new();
        let span = error.span();
        assert_eq!(lines.line_col(span.source(), span.start()), Some((2, 10)), "{workflow}: {rendered}");
        assert_eq!(span.source().origin().canonical_path(), Some("paper.flm"), "{workflow}: {rendered}");
        assert!(error.span().content().contains("nosuchcommand"), "{workflow}: {rendered}");
    }
}
