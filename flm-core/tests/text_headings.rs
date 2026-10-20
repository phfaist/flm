//! How the text backend draws a heading's rule
//! (`flm_core::stdcore::backends::text`): the rule under a level 1 or level 2
//! heading is as wide as the title *measures* in display columns, which is not
//! the number of characters it is written with.

mod common;

use std::sync::Arc;

use flm_core::core::registry::FlmRegistry;
use flm_core::layout::display_width;
use flm_core::stdcore::backends::pres::{self, HeadingPayload};
use flm_core::stdcore::backends::text::Emitted;

use common::lowering::{PresSite, emit_tree, registry};

/// A registry to stage presentation nodes against.
fn vocabulary() -> Arc<FlmRegistry> {
    registry(Vec::new(), Vec::new())
}

/// The text of an emission that ran and reported nothing.
fn text_of(emitted: &Emitted) -> String {
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    emitted.output.as_ref().expect("the emission ran").text.clone()
}

/// The two printed lines of a heading of `level` reading `title`.
fn heading(level: u8, title: &str) -> (String, String) {
    let mut site = PresSite::new(&vocabulary());
    let text = site.text(title);
    let heading =
        site.pres(pres::BLOCK_HEADING, HeadingPayload { name: flm_core::model::names::Name::from("section"), level, in_toc: true, anchor: None, number: None }, vec![text]);
    let tree = site.finish(vec![heading]);
    let printed = text_of(&emit_tree(tree, None));
    let mut lines = printed.lines();
    let title_line = lines.next().expect("the title line").to_string();
    let rule = lines.next().expect("the rule").to_string();
    (title_line, rule)
}

#[test]
fn the_rule_is_as_wide_as_the_title_measures_in_display_columns() {
    for (level, mark) in [(1u8, '='), (2, '-')] {
        for title in ["Introduction", "日本語 Title", "Café", "família 👨‍👩‍👧"] {
            let (printed, rule) = heading(level, title);
            assert_eq!(printed, title);
            assert_eq!(rule, mark.to_string().repeat(display_width(title) as usize), "{title:?}");
        }
    }
}

#[test]
fn a_title_whose_columns_and_characters_differ_gets_the_wider_rule() {
    let title = "日本語 Title";
    // Twelve columns, nine characters: the rule follows the measure.
    assert_eq!(display_width(title), 12);
    assert_eq!(title.chars().count(), 9);
    assert_eq!(heading(1, title).1.chars().count(), 12);
}
