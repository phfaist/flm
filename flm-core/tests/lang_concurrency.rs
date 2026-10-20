//! Thread safety of the language layer (`flm_core::lang`): the static
//! `Send + Sync` assertions, and parallel parsing through one built language.

mod common;

use flm_core::model::doc::{Doc, DocContent, Fragment};
use flm_core::model::element::ElementSpec;
use flm_core::lang::{BuiltLang, Flm, FlmNodeExt, FlmStateExt, StructuralElements};
use techy::core::node::NodeTree;
use techy::core::{Language, ParsingState};

use common::{callables, lang, macro_def, parse};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn the_language_layer_types_are_send_and_sync() {
    assert_send_sync::<BuiltLang>();
    assert_send_sync::<Language<Flm>>();
    assert_send_sync::<ParsingState<Flm>>();
    assert_send_sync::<NodeTree<Flm>>();
    assert_send_sync::<Fragment>();
    assert_send_sync::<DocContent>();
    assert_send_sync::<Doc>();
    assert_send_sync::<ElementSpec>();
    assert_send_sync::<StructuralElements>();
    assert_send_sync::<FlmNodeExt>();
    assert_send_sync::<FlmStateExt>();
}

#[test]
fn many_sources_parse_in_parallel_through_one_language() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let texts: Vec<String> = (0..16).map(|i| format!("\\emph{{{i}}} text {i}")).collect();
    let counts: Vec<usize> = std::thread::scope(|scope| {
        let handles: Vec<_> = texts
            .iter()
            .map(|text| {
                scope.spawn(|| {
                    let fragment = parse(&lang, text);
                    callables(&fragment).len()
                })
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().unwrap()).collect()
    });
    assert_eq!(counts, vec![1; 16]);
}
