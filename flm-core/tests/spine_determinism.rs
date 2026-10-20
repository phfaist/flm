//! Determinism of a whole run: rendering twice from one `Doc` gives identical
//! output, and the parallel runner's tables and text output are bit-identical
//! to the sequential runner's over a document of several trees of uneven size.
//! Under `--no-default-features` the parallel path is the sequential one, so
//! the second property then holds trivially; under the default `parallel`
//! feature it is the real comparison.

mod common;

use std::sync::Arc;

use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::stdcore::features::refs::UnresolvedRefs;

use common::lowering::output;
use common::pipeline;
use common::spine::{
    FEATURES_DOCUMENT, SESSION06_DOCUMENT, overrides, registry, snapshot, text_plan,
};

/// The trees of the determinism document: the two spine documents and a
/// run of small trees in between, so the per-tree map has room to finish
/// out of order and the folds' order is what makes the result.
fn texts() -> Vec<String> {
    let mut texts = vec![FEATURES_DOCUMENT.to_string()];
    for i in 0..12 {
        texts.push(format!("Tree {i} with \\emph{{some}} \\(x_{i}\\) content and \\hello{{{i}}}."));
        if i % 4 == 0 {
            texts.push(SESSION06_DOCUMENT.to_string());
        }
    }
    texts
}

/// One run of the `text` workflow at width 40 over `texts`, with `options`.
fn run(texts: &[String], options: &RunOptions) -> Run {
    let registry = registry();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let doc = pipeline::doc(&registry, &refs);
    let plan = text_plan(&registry, &overrides(Some(40), UnresolvedRefs::Allow));
    pipeline::run_with(&doc, &plan, Externals::none(), options)
}

#[test]
fn rendering_twice_from_one_doc_gives_identical_output() {
    let registry = registry();
    let doc = pipeline::doc(&registry, &[FEATURES_DOCUMENT, SESSION06_DOCUMENT]);
    let plan = text_plan(&registry, &overrides(Some(40), UnresolvedRefs::Allow));
    let first = pipeline::run_with(&doc, &Arc::clone(&plan), Externals::none(), &RunOptions::new());
    let second = pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    // The whole output, marks included (a mark's address is by position in
    // the lowered tree), then every table and every diagnostic.
    assert_eq!(output(&first), output(&second));
    assert_eq!(snapshot(&first), snapshot(&second));
}

#[test]
fn the_parallel_and_the_sequential_runner_give_bit_identical_tables_and_output() {
    let texts = texts();
    let sequential = run(&texts, &RunOptions::sequential());
    let parallel = run(&texts, &RunOptions::new());
    assert_eq!(output(&parallel), output(&sequential));
    let expected = snapshot(&sequential);
    assert_eq!(snapshot(&parallel), expected);
    assert!(expected.contains("flm.labels = "), "the snapshot is not vacuous:\n{expected}");
    assert!(expected.contains("flm.output.text = "), "the snapshot holds the output:\n{expected}");
}

#[test]
fn a_second_parallel_run_repeats_the_first() {
    let texts = texts();
    let first = run(&texts, &RunOptions::new());
    let second = run(&texts, &RunOptions::new());
    assert_eq!(snapshot(&first), snapshot(&second));
}
