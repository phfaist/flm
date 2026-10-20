//! The text backend's run-in decoration idiom
//! (`flm_core::stdcore::backends::text`): the stop this backend adds after a
//! run-in decoration's content, the content it leaves alone, and the render
//! option that decides what the stop is.
//!
//! The test "does the title already end in sentence punctuation?" is shared
//! with every other backend
//! (`flm_core::stdcore::backends::pres::ends_in_sentence_punctuation`); it is
//! exercised here over the trees the emitter walks, and in `tests/html_features.rs`
//! over the same question on the HTML backend.

mod common;

use std::sync::Arc;

use techy::core::node::{BuildId, NodeTree};

use flm_core::core::registry::FlmRegistry;
use flm_core::lang::Flm;
use flm_core::stdcore::backends::pres::{
    self, DecoMarkPayload, DecorationPayload, ParPayload, SpanPayload, SpanStyle,
    ends_in_sentence_punctuation,
};
use flm_core::stdcore::backends::text::{Emitted, TextRenderOptions};

use common::lowering::{PresSite, emit_tree, emit_trees_under, registry};

/// A registry to stage presentation nodes against.
fn vocabulary() -> Arc<FlmRegistry> {
    registry(Vec::new(), Vec::new())
}

/// The text of an emission that ran and reported nothing.
fn text_of(emitted: &Emitted) -> String {
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    emitted.output.as_ref().expect("the emission ran").text.clone()
}

fn decoration(site: &mut PresSite, element: &str, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(element, DecorationPayload, vec![text])
}

/// The tree of a paragraph whose run-in decoration reads `label`, followed by
/// `body`.
fn paragraph(label: &str, body: &str) -> Arc<NodeTree<Flm>> {
    let mut site = PresSite::new(&vocabulary());
    let deco = decoration(&mut site, pres::DECO_RUNIN, label);
    let mark = site.pres(pres::DECO_MARK, DecoMarkPayload { slot: 1 }, Vec::new());
    let text = site.text(body);
    let paragraph = site.pres_attached(
        pres::PAR,
        ParPayload::default(),
        vec![mark, text],
        vec![(pres::DECO_RUNIN, deco)],
    );
    site.finish(vec![paragraph])
}

/// That paragraph printed under the backend's default options.
fn run_in(label: &str, body: &str) -> String {
    text_of(&emit_tree(paragraph(label, body), Some(80)))
}

/// That paragraph printed with `stop` asked for as the run-in stop.
fn run_in_stopped(label: &str, body: &str, stop: &str) -> String {
    let options =
        TextRenderOptions { run_in_stop: String::from(stop), ..TextRenderOptions::default() };
    text_of(&emit_trees_under(vec![paragraph(label, body)], &options))
}

#[test]
fn a_run_in_decoration_is_followed_by_a_period_and_a_space() {
    assert_eq!(run_in("Lemma", "Every set is a set."), "Lemma. Every set is a set.\n");
}

#[test]
fn a_run_in_decoration_that_ends_in_punctuation_takes_no_second_stop() {
    for label in ["Already?", "Entropy:", "Wait...", "Really!", "One,", "Two;"] {
        let printed = run_in(label, "Text.");
        assert_eq!(printed, format!("{label} Text.\n"));
    }
}

#[test]
fn the_stop_after_a_run_in_decoration_is_a_render_option() {
    // The option has the same name and the same meaning on the HTML backend,
    // which `tests/html_features.rs` pins.
    assert_eq!(run_in_stopped("Lemma", "Text.", ":"), "Lemma: Text.\n");
    assert_eq!(run_in_stopped("Lemma", "Text.", ""), "Lemma Text.\n");
    assert_eq!(run_in_stopped("Lemma", "Text.", " —"), "Lemma — Text.\n");
    // A title that already ends in punctuation takes no stop whatever the
    // option asks for.
    assert_eq!(run_in_stopped("Why?", "Text.", ":"), "Why? Text.\n");
}

#[test]
fn the_shared_test_reads_the_whole_content_tree_not_its_last_child() {
    // `ends_in_sentence_punctuation` answers from the plain text of the run,
    // so a title whose last element wraps the final character is judged by
    // that character and not by the element.
    let mut site = PresSite::new(&vocabulary());
    let inner = site.text("this!");
    let style = SpanPayload { style: Some(SpanStyle::Emphasis), ..SpanPayload::default() };
    let span = site.pres(pres::SPAN, style, vec![inner]);
    let lead = site.text("See ");
    let tree = site.finish(vec![lead, span]);
    let root = tree.root();
    let nodes: Vec<_> = root.children().iter().collect();
    assert!(ends_in_sentence_punctuation(nodes.iter().copied()));

    // The same run without the exclamation mark ends in no sentence
    // punctuation at all.
    let mut site = PresSite::new(&vocabulary());
    let inner = site.text("this");
    let style = SpanPayload { style: Some(SpanStyle::Emphasis), ..SpanPayload::default() };
    let span = site.pres(pres::SPAN, style, vec![inner]);
    let lead = site.text("See ");
    let tree = site.finish(vec![lead, span]);
    let root = tree.root();
    let nodes: Vec<_> = root.children().iter().collect();
    assert!(!ends_in_sentence_punctuation(nodes.iter().copied()));
}

#[test]
fn the_shared_test_ignores_the_whitespace_after_the_title() {
    let mut site = PresSite::new(&vocabulary());
    let spaced = site.text("Note.\u{a0} ");
    let tree = site.finish(vec![spaced]);
    let root = tree.root();
    let nodes: Vec<_> = root.children().iter().collect();
    assert!(ends_in_sentence_punctuation(nodes.iter().copied()));
}
