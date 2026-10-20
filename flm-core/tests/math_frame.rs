//! The frame library of the `math` feature
//! (`flm_core::stdcore::features::math::frame`): `formula_display` reads a
//! math group's class; `line_frames` gives each equation line its anchor from
//! the anchors table and shows its number from the counter table, its `\tag`,
//! or nothing, and says so where a table is absent; `number_text` writes the
//! number as characters. The tables are taken as values, as a stage reads
//! them from a run. (The LaTeX parts, `math::latex`, are pinned by the
//! recomposed-LaTeX goldens, which the passthrough rules write through them.)

mod common;

use flm_core::core::plan::Overrides;
use flm_core::lang::Flm;
use flm_core::lang::structural::MATH;
use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::model::element::element_of;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::math::frame::{
    LineShows, formula_display, line_frames, number_text,
};
use flm_core::stdcore::features::math::ENVIRONMENT;
use flm_core::stdcore::features::numbering::COUNTERS;
use flm_core::stdcore::features::refs::ANCHORS;
use flm_core::techy::core::node::NodeRef;

use common::stdcore::{registry, run_workflow};

const DOCUMENT: &str = "Inline \\(a\\) and displayed \\[b\\].\n\n\\begin{align}\n  a \\\\\n  b \\label{eq:b} \\\\\n  c \\tag{T} \\\\\n  d \\tag*{U} \\\\\n  e \\nonumber\n\\end{align}\n\n\\begin{equation*}\n  f\n\\end{equation*}\n";

/// What a line shows, in a few words: `nothing`, the number's text, or the
/// tag with its star.
fn shown(shows: &LineShows<'_>, text: Option<String>) -> String {
    match shows {
        LineShows::Nothing => String::from("nothing"),
        LineShows::Number(_) => format!("number {}", text.unwrap_or_default()),
        LineShows::Tag { starred, content } => {
            let written: String = content.iter().map(|node| node.span_content()).collect();
            format!("tag{} {written}", if *starred { "*" } else { "" })
        }
    }
}

#[test]
fn the_frames_read_the_anchors_and_numbers_the_run_computed() {
    let run = run_workflow(&registry(StdcoreConfig::new()), text::WORKFLOW, &[DOCUMENT], &Overrides::none());
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    let counters = run.table(&COUNTERS).expect("declared").expect("written");
    let tree = run.frozen_tree(TreeId::tree(0)).expect("the tree");
    let nodes: Vec<NodeRef<'_, Flm>> = tree.iter_storage_order().collect();

    let groups: Vec<bool> =
        nodes.iter().filter(|node| element_of(node).name == MATH).map(formula_display).collect();
    assert_eq!(groups, [false, true]);

    let environments: Vec<&NodeRef<'_, Flm>> =
        nodes.iter().filter(|node| element_of(node).name == ENVIRONMENT).collect();
    assert_eq!(environments.len(), 2);
    let frames_of = |node: &NodeRef<'_, Flm>, with_tables: bool| -> Vec<(Option<String>, String)> {
        let addr = NodeAddr::new(TreeId::tree(0), node.id());
        let (anchors, counters) =
            if with_tables { (Some(anchors), Some(counters)) } else { (None, None) };
        line_frames(anchors, counters, addr, node)
            .expect("the numbers are written")
            .iter()
            .map(|frame| (frame.anchor.clone(), shown(&frame.shows, number_text(frame))))
            .collect()
    };

    let align = frames_of(environments[0], true);
    let shows: Vec<&str> = align.iter().map(|(_, shows)| shows.as_str()).collect();
    assert_eq!(shows, ["number (1)", "number (2)", "tag T", "tag* U", "nothing"]);
    assert_eq!(align[1].0.as_deref(), Some("eq-b"), "a labeled line is anchored on its label");
    assert!(align.iter().all(|(anchor, _)| anchor.is_some()), "every line has an anchor");

    // A starred environment shows nothing on its lines.
    let starred = frames_of(environments[1], true);
    assert_eq!(starred.iter().map(|(_, shows)| shows.as_str()).collect::<Vec<_>>(), ["nothing"]);

    // Without the tables, no anchor, and an empty number where one would be.
    let bare = frames_of(environments[0], false);
    assert_eq!(frame_shows_without_tables(&bare), ["number ", "number ", "tag T", "tag* U", "nothing"]);
}

/// The shows of frames computed without tables, each checked to carry no
/// anchor.
fn frame_shows_without_tables(frames: &[(Option<String>, String)]) -> Vec<&str> {
    assert!(frames.iter().all(|(anchor, _)| anchor.is_none()));
    frames.iter().map(|(_, shows)| shows.as_str()).collect()
}
