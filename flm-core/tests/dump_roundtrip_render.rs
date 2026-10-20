//! A fragment written to the wire form and read back, rendered
//! (`flm_core::dump::session` under `flm_core::stdcore`): a read-back fragment
//! placed in a document prints exactly what the fragment it was written from
//! prints, and a language built a second time from the same inputs reads it
//! back as well.

mod common;

use std::sync::Arc;

use flm_core::lang::text::plain_text_of;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, RunOptions};
use flm_core::dump::session::PayloadReaders;
use flm_core::model::doc::{Doc, DocContent, Fragment};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::core::run::Run;
use flm_core::stdcore::features::refpoints::REFPOINTS;
use flm_core::stdcore::features::refs::{self, ANCHORS, UnresolvedRefs};

use common::dump::{read_fragment_from, round_trip, write};
use common::lowering::text as text_of;
use common::pipeline;
use common::spine::{overrides, text_at, text_plan};
use common::stdcore::registry;

/// A document exercising the constructs a fragment carries across the wire:
/// a heading with a label, a reference, formatting, math and verbatim.
const DOCUMENT: &str = "\\section{Introduction}\\label{sec:intro}\n\n\
                        Some \\emph{text} with \\(x^2\\) math, a reference to \\ref{sec:intro}\n\
                        and the code \\verbcode+f(x)+.\n";

/// Every element value the registry holds, for the reading side's resolver.
fn elements(registry: &Arc<FlmRegistry>) -> Vec<Arc<ElementSpec>> {
    registry.elements().iter().map(|entry| Arc::clone(&entry.element)).collect()
}

/// A run of a document holding the one `fragment`, under the text plan.
fn run_of_fragment(registry: &Arc<FlmRegistry>, fragment: Arc<Fragment>) -> Run {
    let mut content = DocContent::new();
    content.place(fragment);
    let doc = Arc::new(Doc::new(content));
    let plan = text_plan(registry, &overrides(None, UnresolvedRefs::Error));
    pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential())
}

/// The text a document of the one `fragment` prints as under `registry`.
fn text_of_fragment(registry: &Arc<FlmRegistry>, fragment: Arc<Fragment>) -> String {
    text_of(&run_of_fragment(registry, fragment))
}

/// Every reference point of a run, in walk order, as what a reference to it
/// would show and where a link to it lands: the point's kind, whether it
/// states a number source, the plain text of the content it states, and its
/// anchor identifier.
fn points_of(run: &Run) -> Vec<(String, bool, Option<String>, Option<String>)> {
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    points
        .records()
        .iter()
        .map(|record| {
            let tree = run.frozen_tree(record.id.addr.tree).expect("a frozen tree");
            let node = tree.get(record.id.addr.node).expect("the point's node");
            let content = record.point.content.as_ref().map(|content| {
                content.nodes(&node).map(plain_text_of).unwrap_or_default()
            });
            (
                record.point.kind.to_string(),
                record.point.number.is_some(),
                content,
                anchors.id(record.id).map(String::from),
            )
        })
        .collect()
}

// Ported from Python FLM's test/test_flmdump.py::TestFLMDataDumper::test_render_after_dump_and_load_fragment.
#[test]
fn a_fragment_read_back_from_the_wire_form_prints_what_the_live_one_prints() {
    let registry = registry(StdcoreConfig::new());
    let live = Arc::new(common::parse(registry.lang(), DOCUMENT));
    let read = round_trip(registry.lang(), &live, elements(&registry));
    assert_eq!(text_of_fragment(&registry, read), text_at(&registry, DOCUMENT, None));
}

// Ported from Python FLM's test/test_flmdump.py::TestFLMDataDumper::test_render_after_dump_and_load_cases_frag2.
#[test]
fn a_language_built_a_second_time_from_the_same_inputs_reads_the_fragment_back() {
    let writing = registry(StdcoreConfig::new());
    let reading = registry(StdcoreConfig::new());
    let live = Arc::new(common::parse(writing.lang(), DOCUMENT));
    let (_, segment) = write(writing.lang(), &live);
    let read = read_fragment_from(reading.lang(), segment, elements(&reading), PayloadReaders::new())
        .expect("the fragment reads back under the second language");
    assert_eq!(text_of_fragment(&reading, read), text_at(&reading, DOCUMENT, None));
}

/// A document whose reference points take all three shapes: a heading of a
/// numbered kind, which states a number source and content; a line of
/// `equation`, which states a number source alone; and a line of `equation*`,
/// which states neither and still gets an identifier.
const POINTS: &str = "\\section{S}\\label{sec:s}\n\n\\begin{equation}\n  x = y\n\\end{equation}\n\n                      \\begin{equation*}\n  z = w\n\\end{equation*}\n";

#[test]
fn the_points_of_a_fragment_read_back_are_the_points_it_was_written_with() {
    // The points are answered by the elements, which the wire form names
    // rather than carries, so what this pins is that a read-back fragment
    // still answers a heading's number source and content, an equation line's
    // number source and the identifier of a line that states neither.
    let registry = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
    );
    let live = Arc::new(common::parse(registry.lang(), POINTS));
    let read = round_trip(registry.lang(), &live, elements(&registry));
    let expected = vec![
        ("section".to_string(), true, Some("S".to_string()), Some("sec-s".to_string())),
        ("equation".to_string(), true, None, Some(refs::auto_anchor("eq", 1))),
        ("equation".to_string(), false, None, Some(refs::auto_anchor("eq", 2))),
    ];
    assert_eq!(points_of(&run_of_fragment(&registry, Arc::clone(&live))), expected);
    assert_eq!(points_of(&run_of_fragment(&registry, read)), expected);
}
