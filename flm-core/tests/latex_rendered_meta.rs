//! The document's metadata on the `latex-rendered` backend, end to end over
//! the standard package: the document `tests/meta.rs` renders (a title with
//! markup and a formula, two authors, a date), a golden under
//! `tests/goldens/latex-rendered/` holding the document and the style file,
//! which `tests/latex_rendered_compile.rs` compiles with `lualatex`; the meta
//! feature's style identifier `flm.meta.style`, which its general rules
//! require at every piece they stage and which brings the feature's
//! definition piece into the style file; and the HTML page, for which the
//! identifier names the feature's stylesheet.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Run, RunOptions};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{html, latex_rendered};
use flm_core::stdcore::features::meta::latex_rendered::PIECE;
use flm_core::stdcore::features::meta::{DocMeta, MetaText, STYLE};

use common::latex_rendered::{assert_golden, document, style};

/// The standard package under its default configuration.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// A title with markup and a formula, two authors, one with markup, and a
/// date.
fn meta() -> DocMeta {
    DocMeta::default()
        .title(MetaText::new(r"On \emph{flexible} markup and \(x^2\)"))
        .author(MetaText::new("Ada Lovelace"))
        .author(MetaText::new(r"Grace \textbf{Hopper}"))
        .date(MetaText::new("September 2026"))
}

/// The run of `workflow` over `text` carrying `meta`; any diagnostic is a
/// test failure.
fn run(workflow: &str, text: &str, meta: Option<DocMeta>) -> Run {
    let registry = registry();
    let options = meta.map_or_else(OptionValues::new, |meta| OptionValues::new().with(meta));
    let doc = common::pipeline::doc_with_options(&registry, &[text], options);
    let plan = common::stdcore::plan(&registry, workflow, &Overrides::none());
    let run = common::pipeline::run(&doc, &plan, &RunOptions::sequential());
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    run
}

#[test]
fn the_title_block_comes_before_the_content_and_the_style_file_holds_the_piece() {
    let run = run(latex_rendered::WORKFLOW, "The content.", Some(meta()));
    let written = document(&run);
    let title = written.find("[role=flm.meta.title, kinds={meta}]").unwrap_or_else(|| panic!("{written}"));
    let author = written.find("[role=flm.meta.author, kinds={meta}]").expect("the authors");
    let date = written.find("[role=flm.meta.date, kinds={meta}]").expect("the date");
    let content = written.find("The content.").expect("the content");
    assert!(title < author && author < date && date < content, "{written}");
    assert!(written.contains("\\flmxSpan[role=flm.meta.author.name]{Ada Lovelace}"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("meta.tex", &run);
}

#[test]
fn a_document_without_metadata_does_not_hold_the_piece() {
    let run = run(latex_rendered::WORKFLOW, "The content.", None);
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
}

#[test]
fn the_html_page_carries_the_feature_s_stylesheet() {
    let run = run(html::WORKFLOW, "The content.", Some(meta()));
    let definitions = run.table(html::DEFINITIONS_TABLE).expect("declared").expect("written");
    assert!(definitions.definitions.iter().any(|row| row.id.as_str() == STYLE), "required");
    let page = common::html::page(&run);
    assert!(page.contains(&format!("<style data-flm-def=\"{STYLE}\">")), "{page}");
    assert!(page.contains(".flm-pres-block.flm-meta-title[data-flm-kind~=\"meta\"]"), "{page}");
}
