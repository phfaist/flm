//! Block-transparent elements and the cut step of block structuring
//! (`flm_core::lang::rewrites::blocks::transparent`): an element of the test's
//! own, `\mark{…}{…}{…}`, cut at the paragraph breaks and blocks of any of its
//! three arguments before the layout builds paragraphs.
//!
//! The test element groups what it gets back per argument, argument by
//! argument in order: each maximal stretch of inline items of one argument
//! becomes a piece — `\mark` rebuilt with that stretch as that argument's
//! content and the other two arguments empty —, each block becomes a piece of
//! its own in the same way, the paragraph-break markers are passed through,
//! and a stretch holding only whitespace and comments makes no piece and is
//! passed through as it is. Every piece carries a record `{cut, index, count}`
//! of the test's own type.

mod common;

use std::sync::Arc;

use flm_core::lang::copy::{ValueContent, copy_run_into_value};
use flm_core::lang::guard::run_with_worker_stack;
use flm_core::lang::mint::{ArgDef, ArgLayout, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::blocks::{
    BlockCutter, BlockTransparent, BlocksConfig, CutItem, CutPart, Cutting, ParbreakInInline,
    StructureCx, StructureError, argument_holds_blocks, run_holds_blocks, structure,
    structure_with_guard,
};
use flm_core::lang::{BuiltLang, Flm, FlmDiagnostics, GuardPolicy, PresPayload};
use flm_core::model::aspects::{BlockRole, BlockRoleAspect, InlineSpacing};
use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::model::names::ElementName;
use flm_core::stdcore::backends::pres::read::payload_of;
use techy::core::node::{BuildId, NodeRef, NodeTree, display_tree};
use techy::error::DiagnosticInfo;
use techy::serialize::SerialValue;

use common::blocks::{Laid, is_paragraph, run, text_of, toy_lang_with};

// --- the test element -------------------------------------------------------------------

/// The continuation record of a piece of `\mark`: the cut it came from, its
/// rank from zero, and the number of pieces of that cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkPiece {
    cut: usize,
    index: usize,
    count: usize,
}

impl PresPayload for MarkPiece {
    fn project(&self) -> SerialValue {
        SerialValue::Str(format!("{}/{}/{}", self.cut, self.index, self.count))
    }
}

/// The three arguments of `\mark`.
const ARGUMENTS: usize = 3;

/// `\mark`'s element value: block role by content, block-transparent.
struct Mark;

impl BlockRoleAspect for Mark {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        if (0..ARGUMENTS).any(|index| argument_holds_blocks(node, index)) {
            BlockRole::Block
        } else {
            BlockRole::Inline(InlineSpacing::default())
        }
    }
}

/// What a piece of the node will be, before the records are known.
enum Planned {
    /// A piece holding these staged nodes in this argument; a block piece
    /// when the flag says so.
    Piece(usize, Vec<BuildId>, bool),
    /// An item passed through as it is.
    Through(CutItem),
}

impl BlockTransparent for Mark {
    fn cut(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        cutter: &mut dyn BlockCutter,
    ) -> Result<Cutting, StructureError> {
        let runs: Vec<Vec<NodeRef<'_, Flm>>> = (0..ARGUMENTS)
            .map(|index| {
                node.argument_content_nodes(index)
                    .map(|slice| slice.iter().collect())
                    .unwrap_or_default()
            })
            .collect();
        // A piece is never cut again: a block piece holds its block at the
        // top level of an argument, so a node carrying a record is left as it
        // is when a structured tree is structured anew.
        if record(&node).is_some() || !runs.iter().any(|run| run_holds_blocks(run)) {
            return Ok(Cutting::Default);
        }
        let cut = cutter.new_cut_number();
        let mut planned: Vec<Planned> = Vec::new();
        for (argument, run) in runs.iter().enumerate() {
            let mut stretch: Vec<CutItem> = Vec::new();
            let flush = |stretch: &mut Vec<CutItem>, planned: &mut Vec<Planned>| {
                if stretch.iter().any(|item| matches!(item, CutItem::Inline(_))) {
                    let ids = stretch.iter().map(|item| item.id()).collect();
                    planned.push(Planned::Piece(argument, ids, false));
                    stretch.clear();
                } else {
                    planned.extend(stretch.drain(..).map(Planned::Through));
                }
            };
            for item in cutter.restage_and_classify(cx, run)? {
                match item {
                    CutItem::Inline(_) | CutItem::Blank(_) => stretch.push(item),
                    CutItem::Marker(_) => {
                        flush(&mut stretch, &mut planned);
                        planned.push(Planned::Through(item));
                    }
                    CutItem::Block(id) => {
                        flush(&mut stretch, &mut planned);
                        planned.push(Planned::Piece(argument, vec![id], true));
                    }
                }
            }
            flush(&mut stretch, &mut planned);
        }
        let count = planned.iter().filter(|plan| matches!(plan, Planned::Piece(..))).count();
        let mut items = Vec::with_capacity(planned.len());
        let mut index = 0;
        for plan in planned {
            match plan {
                Planned::Through(item) => items.push(item),
                Planned::Piece(argument, ids, block) => {
                    let mut replacements: Vec<(CutPart, Vec<BuildId>)> = (0..ARGUMENTS)
                        .filter(|other| *other != argument)
                        .map(|other| (CutPart::Argument(other), Vec::new()))
                        .collect();
                    replacements.push((CutPart::Argument(argument), ids));
                    let record = MarkPiece { cut, index, count };
                    let id = cutter.rebuild(cx, &node, replacements, Some(Arc::new(record)))?;
                    items.push(if block { CutItem::Block(id) } else { CutItem::Inline(id) });
                    index += 1;
                }
            }
        }
        Ok(Cutting::Emit(items))
    }
}

fn mark_def() -> ElementDef {
    let mark = Arc::new(Mark);
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.mark"))
            .with::<dyn BlockRoleAspect>(mark.clone())
            .with::<dyn BlockTransparent>(mark),
    );
    let args = (0..ARGUMENTS)
        .map(|_| ArgDef::new(ArgStdKind::Mandatory).layout(ArgLayout::InlineOrBlocks))
        .collect();
    ElementDef::new("t", ["mark"], SyntaxDef::Macro { args }, element)
}

fn lang() -> BuiltLang {
    toy_lang_with(vec![mark_def()])
}

// --- helpers ----------------------------------------------------------------------------

/// Every node of `\mark` in the tree, in document order.
fn marks(root: NodeRef<'_, Flm>) -> Vec<NodeRef<'_, Flm>> {
    root.descendants().filter(|node| node.name() == Some("mark")).collect()
}

/// The text of argument `index` of a node.
fn argument_text(node: &NodeRef<'_, Flm>, index: usize) -> String {
    node.argument_content_nodes(index)
        .map(|slice| slice.iter().map(text_of).collect())
        .unwrap_or_default()
}

/// The texts of a piece's three arguments.
fn texts(node: &NodeRef<'_, Flm>) -> [String; 3] {
    [argument_text(node, 0), argument_text(node, 1), argument_text(node, 2)]
}

fn record(node: &NodeRef<'_, Flm>) -> Option<MarkPiece> {
    payload_of::<MarkPiece>(node).copied()
}

fn structured(text: &str) -> Laid {
    run(&lang(), text, FragmentLayout::Blocks)
}

/// Whether a node holds a paragraph-break marker or a block at the top level
/// of one of its arguments.
fn holds_cut_point(node: &NodeRef<'_, Flm>) -> bool {
    (0..ARGUMENTS).any(|index| {
        let run: Vec<NodeRef<'_, Flm>> = node
            .argument_content_nodes(index)
            .map(|slice| slice.iter().collect())
            .unwrap_or_default();
        run.iter().any(|child| element_of(child).name.as_str() == "flm.parbreak")
            || run.iter().any(is_paragraph)
    })
}

// --- the cut ----------------------------------------------------------------------------

#[test]
fn a_break_in_any_of_the_three_arguments_cuts_the_node_into_the_sibling_run() {
    let cases = [
        (r"x \mark{a

b}{}{} y", 0),
        (r"x \mark{}{a

b}{} y", 1),
        (r"x \mark{}{}{a

b} y", 2),
    ];
    for (text, argument) in cases {
        let laid = structured(text);
        assert!(!laid.diags.has_errors(), "{}", laid.diags.render_all());
        let paragraphs: Vec<NodeRef<'_, Flm>> = laid.children();
        assert_eq!(paragraphs.len(), 2, "{text:?}: {}", laid.dump());
        assert!(paragraphs.iter().all(is_paragraph));
        // Each paragraph holds one piece, spliced in without a wrapper.
        let first = marks(paragraphs[0]);
        let second = marks(paragraphs[1]);
        assert_eq!((first.len(), second.len()), (1, 1), "{}", laid.dump());
        assert_eq!(first[0].parent().map(|parent| parent.id()), Some(paragraphs[0].id()));
        let mut expected = [String::new(), String::new(), String::new()];
        expected[argument] = "a".to_string();
        assert_eq!(texts(&first[0]), expected);
        expected[argument] = "b".to_string();
        assert_eq!(texts(&second[0]), expected);
        assert!(text_of(paragraphs[0]).starts_with('x'));
        assert!(text_of(paragraphs[1]).ends_with('y'));
        assert!(marks(laid.root()).iter().all(|piece| !holds_cut_point(piece)));
    }
}

#[test]
fn inline_pieces_of_several_arguments_share_a_paragraph_until_a_marker() {
    let laid = structured("x \\mark{a}{b\n\nc}{d} y");
    let paragraphs = laid.children();
    assert_eq!(paragraphs.len(), 2, "{}", laid.dump());
    let first: Vec<[String; 3]> = marks(paragraphs[0]).iter().map(texts).collect();
    let second: Vec<[String; 3]> = marks(paragraphs[1]).iter().map(texts).collect();
    let s = |a: &str, b: &str, c: &str| [a.to_string(), b.to_string(), c.to_string()];
    assert_eq!(first, [s("a", "", ""), s("", "b", "")]);
    assert_eq!(second, [s("", "c", ""), s("", "", "d")]);
    let records: Vec<MarkPiece> = marks(laid.root()).iter().filter_map(record).collect();
    let expected: Vec<MarkPiece> =
        (0..4).map(|index| MarkPiece { cut: 0, index, count: 4 }).collect();
    assert_eq!(records, expected);
}

#[test]
fn a_block_it_hands_through_stands_alone_as_a_block_of_the_region() {
    let laid = structured(
        "x \\mark{see \\begin{quotation}q\\end{quotation} here}{}{} y",
    );
    assert!(!laid.diags.has_errors(), "{}", laid.diags.render_all());
    let children = laid.children();
    assert_eq!(children.len(), 3, "{}", laid.dump());
    assert!(is_paragraph(&children[0]));
    assert_eq!(children[1].name(), Some("mark"));
    assert!(is_paragraph(&children[2]));
    let block = children[1];
    assert_eq!(record(&block), Some(MarkPiece { cut: 0, index: 1, count: 3 }));
    let content: Vec<NodeRef<'_, Flm>> =
        block.argument_content_nodes(0).expect("the argument").iter().collect();
    assert_eq!(content.len(), 1, "{}", laid.dump());
    assert_eq!(content[0].name(), Some("quotation"));
    assert_eq!(texts(&marks(children[0])[0])[0].trim_end(), "see");
    assert_eq!(texts(&marks(children[2])[0])[0].trim_start(), "here");
}

#[test]
fn the_records_are_as_the_element_attached_them_and_a_piece_keeps_span_and_state() {
    let lang = lang();
    let text = "x \\mark{a\n\nb\n\nc}{}{} y";
    let fragment = common::parse(&lang, text);
    let input = marks(fragment.tree().root())[0];
    let mut diags = FlmDiagnostics::new();
    let tree = structure(fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::default(), &mut diags)
        .expect("a tree");
    let pieces = marks(tree.root());
    let records: Vec<Option<MarkPiece>> = pieces.iter().map(record).collect();
    assert_eq!(
        records,
        (0..3).map(|index| Some(MarkPiece { cut: 0, index, count: 3 })).collect::<Vec<_>>()
    );
    for piece in &pieces {
        assert_eq!(piece.span(), input.span());
        assert!(Arc::ptr_eq(piece.parsing_state(), input.parsing_state()));
        assert_eq!(element_of(piece).name.as_str(), "t.mark");
    }
}

#[test]
fn a_record_travels_with_a_copy_into_a_value() {
    let laid = structured("\\mark{a\n\nb}{}{}");
    let piece = marks(laid.root())[1];
    let value = run_with_worker_stack(|| {
        copy_run_into_value(&[piece], &piece, ValueContent::Own, GuardPolicy::StackBudget)
    })
    .expect("a worker thread")
    .expect("copied");
    let copied = value.root().children().iter().next().expect("the copy");
    assert_eq!(record(&copied), Some(MarkPiece { cut: 0, index: 1, count: 2 }));
}

#[test]
fn a_node_with_no_cut_point_is_left_alone_and_restaged_once() {
    let laid = structured(r"x \mark{a}{b \emph{c}}{d} y");
    assert_eq!(laid.children().len(), 1);
    let found = marks(laid.root());
    assert_eq!(found.len(), 1, "{}", laid.dump());
    let s = |t: &str| t.to_string();
    assert_eq!(texts(&found[0]), [s("a"), s("b c"), s("d")]);
    assert!(found[0].ext().payload.is_none());
}

#[test]
fn an_inner_break_cuts_the_outer_node_inner_first() {
    let laid = structured("\\mark{see \\mark{this\n\nthat}{}{} end}{}{}");
    assert!(!laid.diags.has_errors(), "{}", laid.diags.render_all());
    let paragraphs = laid.children();
    assert_eq!(paragraphs.len(), 2, "{}", laid.dump());
    let mut records = Vec::new();
    for (paragraph, (outer_text, inner_text)) in
        paragraphs.iter().zip([("see this", "this"), ("that end", "that")])
    {
        let outer: Vec<NodeRef<'_, Flm>> = paragraph
            .children()
            .iter()
            .filter(|node| node.name() == Some("mark"))
            .collect();
        assert_eq!(outer.len(), 1, "{}", laid.dump());
        assert_eq!(texts(&outer[0])[0], outer_text);
        let inner: Vec<NodeRef<'_, Flm>> = marks(outer[0]);
        assert_eq!(inner.len(), 1);
        assert_eq!(texts(&inner[0])[0], inner_text);
        records.push((record(&outer[0]).unwrap(), record(&inner[0]).unwrap()));
    }
    // The outer node asks for its cut number before the inner one is cut.
    assert_eq!(
        records,
        [
            (MarkPiece { cut: 0, index: 0, count: 2 }, MarkPiece { cut: 1, index: 0, count: 2 }),
            (MarkPiece { cut: 0, index: 1, count: 2 }, MarkPiece { cut: 1, index: 1, count: 2 }),
        ]
    );
}

#[test]
fn whitespace_and_comments_alone_make_no_piece_and_keep_their_comments() {
    let laid = structured("x \\mark{a\n\n  %%note\n\nb}{}{} y");
    let pieces = marks(laid.root());
    assert_eq!(pieces.len(), 2, "{}", laid.dump());
    assert!(pieces.iter().all(|piece| record(piece).is_some_and(|r| r.count == 2)));
    let comments: Vec<NodeRef<'_, Flm>> =
        laid.root().descendants().filter(|node| node.is_comment()).collect();
    assert_eq!(comments.len(), 1, "{}", laid.dump());
    assert!(pieces.iter().all(|piece| !piece.descendants().any(|node| node.is_comment())));
}

#[test]
fn the_cut_in_a_heading_title_is_reported_by_the_layout_as_a_break_in_an_inline_region() {
    let laid = structured("\\section{T \\mark{a\n\nb}{}{}}");
    assert_eq!(laid.identifiers(), [ParbreakInInline::IDENTIFIER], "{}", laid.dump());
    let section = laid
        .root()
        .descendants()
        .find(|node| node.name() == Some("section"))
        .expect("the heading");
    let title: Vec<NodeRef<'_, Flm>> =
        section.argument_content_nodes(0).expect("the title").iter().collect();
    let pieces: Vec<&NodeRef<'_, Flm>> =
        title.iter().filter(|node| node.name() == Some("mark")).collect();
    assert_eq!(pieces.len(), 2, "{}", laid.dump());
    // The marker between the two pieces was written as one space.
    let between: String = title
        .iter()
        .skip_while(|node| node.name() != Some("mark"))
        .skip(1)
        .take_while(|node| node.name() != Some("mark"))
        .map(|node| text_of(*node))
        .collect();
    assert_eq!(between, " ");
}

#[test]
fn structuring_a_structured_tree_changes_nothing() {
    for text in [
        "x \\mark{a\n\nb}{c}{} y",
        "\\mark{see \\mark{this\n\nthat}{}{} end}{}{}",
        "x \\mark{see \\begin{quotation}q\\end{quotation} here}{}{} y",
    ] {
        let laid = structured(text);
        let mut diags = FlmDiagnostics::new();
        let again = structure(&laid.tree, FragmentLayout::Blocks, &BlocksConfig::default(), &mut diags)
            .expect("a tree");
        assert_eq!(display_tree(again.root()), laid.dump(), "{text:?}");
        let records = |tree: &NodeTree<Flm>| -> Vec<Option<MarkPiece>> {
            marks(tree.root()).iter().map(record).collect()
        };
        assert_eq!(records(&again), records(&laid.tree));
        assert!(diags.is_empty());
    }
}

#[test]
fn the_cut_number_is_distinct_per_cut_node_and_stable_across_runs() {
    let text = "\\mark{a\n\nb}{}{} and \\mark{c}{} {} then \\mark{}{d\n\ne}{}";
    let cuts = || -> Vec<usize> {
        let laid = structured(text);
        let mut cuts: Vec<usize> =
            marks(laid.root()).iter().filter_map(record).map(|r| r.cut).collect();
        cuts.dedup();
        cuts
    };
    assert_eq!(cuts(), [0, 1]);
    assert_eq!(cuts(), cuts());
}

#[test]
fn one_tree_whether_the_caller_is_on_a_worker_thread_or_not() {
    let text = "\\mark{see \\mark{this\n\nthat}{}{} end}{a\n\nb}{}";
    let here = structured(text).dump();
    let there = run_with_worker_stack(move || structured(text).dump()).expect("a worker thread");
    assert_eq!(here, there);
}

// --- the façade's contract ----------------------------------------------------------------

/// An element whose aspect asks to rebuild a part its node does not have.
struct Misuse;

impl BlockTransparent for Misuse {
    fn cut(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        cutter: &mut dyn BlockCutter,
    ) -> Result<Cutting, StructureError> {
        let id = cutter.rebuild(cx, &node, vec![(CutPart::ArgumentNamed("none".into()), vec![])], None)?;
        Ok(Cutting::Emit(vec![CutItem::Inline(id)]))
    }
}

#[test]
fn rebuilding_a_part_the_node_does_not_have_ends_the_pass_with_the_typed_failure() {
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.misuse")).with::<dyn BlockTransparent>(Arc::new(Misuse)),
    );
    let lang = toy_lang_with(vec![common::blocks::macro_of("misuse", 1, element)]);
    let fragment = common::parse(&lang, "x \\misuse{a} y");
    let mut diags = FlmDiagnostics::new();
    let error = structure(fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::default(), &mut diags)
        .expect_err("the aspect misused the façade");
    assert!(
        matches!(&error, StructureError::Cut { element, .. } if element.as_str() == "t.misuse"),
        "{error:?}"
    );
}

// --- no input panics or loops -------------------------------------------------------------

fn nested(depth: usize) -> String {
    format!("{}a\n\nb{}", "\\mark{".repeat(depth), "}{}{}".repeat(depth))
}

#[test]
fn a_hundred_nested_block_transparent_nodes_are_refused_under_a_depth_limit() {
    let lang = lang();
    let fragment = common::parse(&lang, &nested(100));
    let mut diags = FlmDiagnostics::new();
    let result = structure_with_guard(
        fragment.tree(),
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        GuardPolicy::DepthLimit(20),
        &mut diags,
    );
    assert!(matches!(result, Err(StructureError::NestingTooDeep { .. })), "{result:?}");
}

#[test]
fn a_hundred_nested_block_transparent_nodes_are_cut_under_the_production_guard() {
    let laid = structured(&nested(100));
    let paragraphs = laid.children();
    assert_eq!(paragraphs.len(), 2);
    // Each level is cut into two pieces, one per paragraph.
    assert_eq!(marks(paragraphs[0]).len(), 100);
    assert_eq!(marks(paragraphs[1]).len(), 100);
    assert!(marks(laid.root()).iter().all(|piece| !holds_cut_point(piece)));
}

#[test]
fn a_node_holding_a_thousand_paragraph_breaks_is_cut_under_the_production_guard() {
    let text = format!("\\mark{{{}}}{{}}{{}}", "a\n\n".repeat(1000));
    let laid = structured(&text);
    assert_eq!(laid.children().len(), 1000);
    assert_eq!(marks(laid.root()).len(), 1000);
}
