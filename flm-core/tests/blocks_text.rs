//! Text resolution inside the block-structure pass
//! (`flm_core::lang::rewrites::blocks::text` and its typography): run merging,
//! the sparse source map under the byte-identity rule, typography and the gates
//! that switch it off, and the opaque regions.

mod common;

use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::element_of;
use flm_core::lang::rewrites::blocks::{BlocksConfig, structure};
use flm_core::lang::{Flm, FlmDiagnostics, FlmGroupType};
use techy::core::node::NodeKind;
use techy::latexlike::MathGroupForm;

use common::blocks::{
    Site, assert_map_tiles, chars_nodes, is_owned, lay_out, pieces, run, text_of, toy_lang,
};

// --- merging and the source map ------------------------------------------------------

#[test]
fn an_untouched_run_is_span_backed_with_no_map() {
    let lang = toy_lang();
    let laid = run(&lang, "abc", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("abc"));
    assert!(node.ext().source_map.is_none());
    assert!(!is_owned(&node));
}

#[test]
fn a_collapsed_whitespace_run_is_one_piece() {
    let lang = toy_lang();
    let laid = run(&lang, "a\t\t b", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("a b"));
    assert!(is_owned(&node));
    assert_map_tiles(&node);
    assert_eq!(
        pieces(&node),
        [(1, "a".into()), (1, "\t\t ".into()), (1, "b".into())]
    );
}

#[test]
fn a_length_changing_ligature_is_one_piece_between_coalesced_stretches() {
    let lang = toy_lang();
    let laid = run(&lang, "a--b", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("a\u{2013}b"));
    assert_map_tiles(&node);
    assert_eq!(
        pieces(&node),
        [(1, "a".into()), (3, "--".into()), (1, "b".into())]
    );
}

#[test]
fn a_length_preserving_ligature_is_one_piece_too() {
    // The byte-identity clause: `---` → `—` is three bytes for three, and still
    // visible in the map.
    let lang = toy_lang();
    let laid = run(&lang, "a---b", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("a\u{2014}b"));
    assert!(is_owned(&node));
    assert_eq!(
        pieces(&node),
        [(1, "a".into()), (3, "---".into()), (1, "b".into())]
    );
}

#[test]
fn a_trimmed_run_needs_no_map_and_narrows_its_span() {
    let lang = toy_lang();
    let laid = run(&lang, "   a  \n\n b   ", FragmentLayout::Blocks);
    let paras = laid.children();
    assert_eq!(paras.len(), 2);
    let first = chars_nodes(paras[0]).remove(0);
    assert_eq!(first.chars(), Some("a"));
    assert!(first.ext().source_map.is_none());
    assert_eq!(first.span_content(), "a");
}

#[test]
fn three_adjacent_chunks_merge_into_one_node_with_a_seam_piece() {
    let lang = toy_lang();
    // The recovery space of a paragraph break met inline stands between two
    // source runs; it has no byte-identical stretch of its own.
    let laid = run(&lang, "{x\n\ny}", FragmentLayout::Inline);
    let group = laid.children()[0];
    assert_eq!(group.child_count(), 1, "{}", laid.dump());
    let node = group.child(0).unwrap();
    assert_eq!(node.chars(), Some("x y"));
    assert_map_tiles(&node);
    assert_eq!(
        pieces(&node),
        [(1, "x".into()), (1, "\n\n".into()), (1, "y".into())]
    );
}

#[test]
fn every_piece_points_at_real_source_bytes() {
    let lang = toy_lang();
    let laid = run(&lang, "{x\n\ny}", FragmentLayout::Inline);
    let node = laid.children()[0].child(0).unwrap();
    for piece in node.ext().source_map.as_ref().unwrap().pieces() {
        assert!(piece.span.source().content().contains(piece.span.content()));
    }
}

// --- typography ------------------------------------------------------------------------

const SAMPLE: &str = "``quoted'' -- dash --- em ... dots";

#[test]
fn every_family_applies_under_the_default_options() {
    let lang = toy_lang();
    let laid = run(&lang, SAMPLE, FragmentLayout::Inline);
    assert_eq!(
        laid.children()[0].chars(),
        Some("\u{201c}quoted\u{201d} \u{2013} dash \u{2014} em \u{2026} dots")
    );
}

#[test]
fn a_family_gate_closes_only_its_family() {
    let lang = toy_lang();
    let config = BlocksConfig {
        ligature_unicode_dashes: false,
        ..BlocksConfig::default()
    };
    let laid = lay_out(&lang, SAMPLE, FragmentLayout::Inline, &config);
    assert_eq!(
        laid.children()[0].chars(),
        Some("\u{201c}quoted\u{201d} -- dash --- em \u{2026} dots")
    );
}

#[test]
fn whitespace_is_normalized_even_with_every_gate_closed() {
    let lang = toy_lang();
    let laid = lay_out(
        &lang,
        "a  --  b",
        FragmentLayout::Inline,
        &BlocksConfig::no_typography(),
    );
    assert_eq!(laid.children()[0].chars(), Some("a -- b"));
}

#[test]
fn a_straight_double_quote_is_decided_by_its_position() {
    let lang = toy_lang();
    let laid = run(&lang, "\"a\" b", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("\u{201c}a\u{201d} b"));
}

#[test]
fn a_straight_single_quote_after_a_content_sibling_closes() {
    let lang = toy_lang();
    // The position test sees the math node before the run, under the default
    // options.
    let laid = run(&lang, "\\(A_j\\)'s", FragmentLayout::Inline);
    let children = laid.children();
    assert_eq!(children[1].chars(), Some("\u{2019}s"));
}

#[test]
fn a_straight_single_quote_opening_a_run_opens() {
    let lang = toy_lang();
    let laid = run(&lang, "'a'", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("\u{2018}a\u{2019}"));
}

#[test]
fn an_apostrophe_inside_a_word_closes() {
    let lang = toy_lang();
    let laid = run(&lang, "don't", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("don\u{2019}t"));
}

#[test]
fn an_apostrophe_after_a_dash_opens() {
    // The dash is opening context, as in the Python implementation:
    // the `--` becomes an en dash and the quote pair around `q` curls outwards.
    let lang = toy_lang();
    let laid = run(&lang, "a--'q'", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("a\u{2013}\u{2018}q\u{2019}"));
}

#[test]
fn an_apostrophe_before_a_digit_opens() {
    let lang = toy_lang();
    let laid = run(&lang, "x'9", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("x\u{2018}9"));
}

#[test]
fn the_tex_single_quote_pair_still_reads_as_quotes() {
    let lang = toy_lang();
    let laid = run(&lang, "`hi'", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("\u{2018}hi\u{2019}"));
}

// --- non-ASCII text -----------------------------------------------------------------

#[test]
fn accented_letters_pass_through_untouched_under_typography() {
    let lang = toy_lang();
    let laid = run(&lang, "café au lait", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("café au lait"));
    assert!(node.ext().source_map.is_none());
    assert!(!is_owned(&node));
}

#[test]
fn accented_letters_pass_through_untouched_without_typography() {
    let lang = toy_lang();
    let laid = lay_out(
        &lang,
        "naïve",
        FragmentLayout::Inline,
        &BlocksConfig::no_typography(),
    );
    assert_eq!(laid.children()[0].chars(), Some("naïve"));
}

#[test]
fn curly_quotes_already_in_the_source_pass_through_untouched() {
    let lang = toy_lang();
    let laid = run(&lang, "\u{201c}already curly\u{201d}", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("\u{201c}already curly\u{201d}"));
    assert!(node.ext().source_map.is_none());
}

#[test]
fn cjk_text_passes_through_untouched_and_its_whitespace_is_collapsed() {
    let lang = toy_lang();
    let laid = run(&lang, "日本語  テキスト", FragmentLayout::Inline);
    let node = laid.children()[0];
    assert_eq!(node.chars(), Some("日本語 テキスト"));
    assert_map_tiles(&node);
}

#[test]
fn a_substitution_right_after_a_non_ascii_character_is_found() {
    let lang = toy_lang();
    let laid = run(&lang, "é---b", FragmentLayout::Inline);
    assert_eq!(laid.children()[0].chars(), Some("é\u{2014}b"));
}

// --- re-resolving resolved text ------------------------------------------------------

/// The text nodes of a tree in document order, as `(text, pieces)`.
fn provenance(tree: &techy::core::node::NodeTree<Flm>) -> Vec<(String, Vec<(u32, String)>)> {
    chars_nodes(tree.root())
        .iter()
        .map(|node| (node.chars().unwrap_or("").to_string(), pieces(node)))
        .collect()
}

#[test]
fn structuring_a_structured_tree_leaves_every_source_map_unchanged() {
    let lang = toy_lang();
    let once = run(&lang, "  a\t\tb --- c\n\n{x\n\ny} \"q\"  ", FragmentLayout::Blocks);
    let before = provenance(&once.tree);
    assert!(before.iter().any(|(_, pieces)| !pieces.is_empty()), "the input has maps");
    let mut diags = FlmDiagnostics::new();
    let twice = structure(&once.tree, FragmentLayout::Blocks, &BlocksConfig::default(), &mut diags)
        .unwrap();
    assert_eq!(provenance(&twice), before);
    assert!(diags.is_empty(), "{}", diags.render_all());
}

#[test]
fn a_mapped_node_merged_with_a_neighbor_keeps_its_exact_provenance() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "x\t\tyz");
    let mapped = site.mapped_text("x y", 0..4, vec![(1, 0..1), (1, 1..3), (1, 3..4)]);
    let next = site.text(4..5);
    let root = site.stage(NodeKind::list(), 0..5, vec![mapped, next]);
    let tree = site.finish(root);
    let mut diags = FlmDiagnostics::new();
    let out = structure(&tree, FragmentLayout::Inline, &BlocksConfig::no_typography(), &mut diags)
        .unwrap();
    let node = out.root().child(0).unwrap();
    assert_eq!(out.root().child_count(), 1);
    assert_eq!(node.chars(), Some("x yz"));
    assert_eq!(pieces(&node), [(1, "x".into()), (1, "\t\t".into()), (2, "yz".into())]);
}

#[test]
fn the_pass_accepts_its_own_output() {
    let lang = toy_lang();
    let laid = run(&lang, "a---b ``q'' \"x\"", FragmentLayout::Inline);
    let mut diags = flm_core::lang::FlmDiagnostics::new();
    let again = flm_core::lang::rewrites::blocks::structure(
        &laid.tree,
        FragmentLayout::Inline,
        &BlocksConfig::default(),
        &mut diags,
    )
    .unwrap();
    assert_eq!(
        again.root().child(0).and_then(|n| n.chars()),
        laid.children()[0].chars()
    );
}

#[test]
fn math_content_is_exempt() {
    let lang = toy_lang();
    let laid = run(&lang, "x \\(a--b\\) w", FragmentLayout::Inline);
    let math = laid.children()[1];
    assert_eq!(
        math.group_type(),
        Some(FlmGroupType::Math(MathGroupForm::Inline))
    );
    assert_eq!(text_of(math), "a--b", "{}", laid.dump());
}

#[test]
fn verbatim_content_is_exempt() {
    let lang = toy_lang();
    let laid = run(&lang, "x \\verb|y--z| w", FragmentLayout::Inline);
    let verb = laid.children()[1];
    assert_eq!(element_of(&verb).name.as_str(), "t.verb");
    assert_eq!(text_of(verb), "y--z", "{}", laid.dump());
}

#[test]
fn math_mode_text_in_an_environment_body_is_exempt() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{equation}a  --  b\\end{equation}",
        FragmentLayout::Inline,
    );
    assert_eq!(text_of(laid.children()[0]), "a  --  b", "{}", laid.dump());
}

#[test]
fn a_text_mode_body_nested_in_a_math_mode_body_is_laid_out() {
    // Opacity is a property of the region, and the region's parsing mode is
    // what says which it is: a body a declaration returns to text mode inside
    // mathematics is document content, so it is resolved like any other.
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{equation}x\\begin{textinmath}a  --  b\\end{textinmath}\\end{equation}",
        FragmentLayout::Inline,
    );
    let equation = laid.children()[0];
    let inner = equation
        .body()
        .unwrap()
        .iter()
        .find(|node| node.is_callable())
        .expect("the nested environment");
    assert_eq!(element_of(&inner).name.as_str(), "t.textinmath");
    assert_eq!(text_of(inner), "a – b", "{}", laid.dump());
}

#[test]
fn a_text_mode_argument_inside_mathematics_is_laid_out() {
    // The argument of a text command is document content: whitespace is
    // resolved and typographic substitution reaches it, although it stands
    // inside mathematics.
    let lang = toy_lang();
    let laid = run(&lang, "\\(x\\textarg{a  --  b}y\\)", FragmentLayout::Inline);
    let math = laid.children()[0];
    let command = math
        .descendants()
        .find(|node| element_of(node).name.as_str() == "t.textarg")
        .expect("the text command");
    assert_eq!(text_of(command), "a – b", "{}", laid.dump());
}

#[test]
fn the_mathematics_around_a_text_command_is_untouched() {
    // Only the text-mode argument is laid out; everything else in the formula
    // stays byte for byte what the author wrote.
    let lang = toy_lang();
    let laid = run(&lang, "\\(a  --  b\\textarg{c  --  d}e  --  f\\)", FragmentLayout::Inline);
    let math = laid.children()[0];
    let around: String = math
        .children()
        .iter()
        .filter_map(|node| node.chars().map(String::from))
        .collect();
    assert_eq!(around, "a  --  be  --  f", "{}", laid.dump());
}

#[test]
fn a_paragraph_break_inside_a_text_argument_is_reported_as_in_any_inline_region() {
    let lang = toy_lang();
    let laid = run(&lang, "\\(\\textarg{a\n\nb}\\)", FragmentLayout::Inline);
    assert_eq!(laid.identifiers(), ["flm.blocks.parbreak-in-inline"], "{}", laid.dump());
    let command = laid
        .tree
        .root()
        .descendants()
        .find(|node| element_of(node).name.as_str() == "t.textarg")
        .expect("the text command");
    assert_eq!(text_of(command), "a b", "{}", laid.dump());
}

#[test]
fn an_argument_inside_a_math_group_is_untouched() {
    let lang = toy_lang();
    let laid = run(&lang, "\\(\\emph{a  --  b}\\)", FragmentLayout::Inline);
    let math = laid.children()[0];
    assert_eq!(
        math.group_type(),
        Some(FlmGroupType::Math(MathGroupForm::Inline))
    );
    assert_eq!(text_of(math), "a  --  b", "{}", laid.dump());
}
