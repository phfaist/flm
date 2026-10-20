//! The block-structure pass of `flm_core::lang::rewrites::blocks`: blocks and
//! inline layout derived from the elements' block roles and from the fragment
//! layout, and the rules that govern opaque regions.

mod common;

use std::sync::Arc;

use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::element_of;
use flm_core::lang::rewrites::blocks::{BlockInInline, BlocksConfig, ParbreakInInline, structure};
use flm_core::lang::{Flm, FlmDiagnostics, FlmGroupType};
use techy::core::node::{NodeBuildError, NodeKind, NodeRef, NodeTreeBuilder};
use techy::core::{Lang, ParsingState, ParsingStateDelta};
use techy::error::DiagnosticInfo;
use techy::latexlike::{MathGroupForm, Mode};
use techy::source::SourceSpan;
use techy::transform::{Restage, RestageContext, RestageError, TreeRestager};

use common::blocks::{
    assert_map_tiles, chars_nodes, is_owned, is_paragraph, paragraphs, run, text_of, toy_lang,
};
use common::source;

// --- the basic blocks layout --------------------------------------------------------

#[test]
fn blocks_layout_builds_one_paragraph_per_run() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "  Hello   world  \n\n  Second   para  \n",
        FragmentLayout::Blocks,
    );
    let children = laid.children();
    assert_eq!(children.len(), 2, "{}", laid.dump());
    assert!(children.iter().all(is_paragraph), "{}", laid.dump());
}

#[test]
fn parbreak_markers_are_dropped() {
    let lang = toy_lang();
    let laid = run(&lang, "a\n\nb", FragmentLayout::Blocks);
    let parbreak = &lang.structural().parbreak;
    assert!(
        laid.root()
            .descendants()
            .all(|node| !Arc::ptr_eq(element_of(&node), parbreak))
    );
}

#[test]
fn paragraph_edges_are_trimmed_and_interior_whitespace_collapsed() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "  Hello   world  \n\n  Second   para  \n",
        FragmentLayout::Blocks,
    );
    let children = laid.children();
    assert_eq!(text_of(children[0]), "Hello world");
    assert_eq!(text_of(children[1]), "Second para");
}

#[test]
fn changed_text_is_owned_and_carries_a_tiling_source_map() {
    let lang = toy_lang();
    let laid = run(&lang, "Hello   world", FragmentLayout::Blocks);
    let chars = chars_nodes(laid.children()[0]);
    assert_eq!(chars.len(), 1);
    assert!(is_owned(&chars[0]));
    assert!(chars[0].ext().source_map.is_some());
    assert_map_tiles(&chars[0]);
}

#[test]
fn a_paragraph_is_a_paragraph_group_with_empty_delimiters_carrying_its_element() {
    let lang = toy_lang();
    let laid = run(&lang, "a", FragmentLayout::Blocks);
    let paragraph = laid.children()[0];
    assert_eq!(paragraph.group_type(), Some(FlmGroupType::Paragraph));
    assert_eq!(paragraph.group_delimiters(), Some(("", "")));
    assert!(Arc::ptr_eq(
        element_of(&paragraph),
        &lang.structural().paragraph
    ));
}

// --- the InlineOrBlocks inference ----------------------------------------------------

#[test]
fn inline_or_blocks_stays_inline_without_a_break_or_a_block() {
    let lang = toy_lang();
    let laid = run(&lang, "just inline   text", FragmentLayout::InlineOrBlocks);
    assert!(paragraphs(laid.root()).is_empty(), "{}", laid.dump());
    let children = laid.children();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].chars(), Some("just inline text"));
}

#[test]
fn inline_or_blocks_switches_to_blocks_on_a_paragraph_break() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "just inline   text\n\nand more",
        FragmentLayout::InlineOrBlocks,
    );
    assert_eq!(paragraphs(laid.root()).len(), 2, "{}", laid.dump());
}

#[test]
fn inline_or_blocks_switches_to_blocks_on_a_block_element() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "text \\begin{quotation}q\\end{quotation} more",
        FragmentLayout::InlineOrBlocks,
    );
    assert_eq!(paragraphs(laid.root()).len(), 2, "{}", laid.dump());
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
}

// --- plain groups are inline containers ---------------------------------------------

#[test]
fn a_plain_group_stays_inside_its_paragraph() {
    let lang = toy_lang();
    let laid = run(&lang, "text {grouped   words} text", FragmentLayout::Blocks);
    let children = laid.children();
    assert_eq!(children.len(), 1, "{}", laid.dump());
    let inside: Vec<NodeRef<'_, Flm>> = children[0].children().iter().collect();
    assert_eq!(inside.len(), 3, "{}", laid.dump());
    assert_eq!(inside[0].chars(), Some("text "));
    assert_eq!(inside[1].group_type(), Some(FlmGroupType::Content));
    assert_eq!(inside[2].chars(), Some(" text"));
    assert_eq!(text_of(inside[1]), "grouped words");
}

#[test]
fn an_inline_region_is_collapsed_but_not_trimmed() {
    let lang = toy_lang();
    let laid = run(&lang, "x{ a }y", FragmentLayout::Blocks);
    let group = laid.children()[0].child(1).expect("the group");
    assert_eq!(text_of(group), " a ", "{}", laid.dump());
}

// --- a block element stands alone ---------------------------------------------------

#[test]
fn a_block_element_stands_between_paragraphs() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "before\n\\begin{quotation}q\\end{quotation}\nafter",
        FragmentLayout::Blocks,
    );
    let children = laid.children();
    assert_eq!(children.len(), 3, "{}", laid.dump());
    assert!(is_paragraph(&children[0]));
    assert_eq!(text_of(children[0]), "before");
    assert_eq!(element_of(&children[1]).name.as_str(), "t.quotation");
    assert!(!is_paragraph(&children[1]));
    assert!(is_paragraph(&children[2]));
    assert_eq!(text_of(children[2]), "after");
}

#[test]
fn a_blocks_body_is_laid_out_as_blocks() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{quotation}q\\end{quotation}",
        FragmentLayout::Blocks,
    );
    let body: Vec<NodeRef<'_, Flm>> = laid.children()[0]
        .descendants()
        .filter(|n| n.group_type() == Some(FlmGroupType::Paragraph))
        .collect();
    assert_eq!(body.len(), 1, "{}", laid.dump());
    assert_eq!(text_of(body[0]), "q");
}

#[test]
fn whitespace_only_text_between_blocks_is_dropped() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{quotation}a\\end{quotation}   \n   \\begin{quotation}b\\end{quotation}",
        FragmentLayout::Blocks,
    );
    assert_eq!(laid.children().len(), 2, "{}", laid.dump());
}

// --- decorators ----------------------------------------------------------------------

#[test]
fn a_leading_decorator_opens_its_paragraph_and_the_text_after_it_is_head_trimmed() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\runin{T}  text after\n\nnext",
        FragmentLayout::Blocks,
    );
    let children = laid.children();
    assert_eq!(children.len(), 2, "{}", laid.dump());
    let first: Vec<NodeRef<'_, Flm>> = children[0].children().iter().collect();
    assert_eq!(first.len(), 2, "{}", laid.dump());
    assert_eq!(element_of(&first[0]).name.as_str(), "t.runin");
    assert_eq!(first[1].chars(), Some("text after"));
    assert_eq!(text_of(children[1]), "next");
}

#[test]
fn blank_text_before_a_leading_decorator_does_not_block_the_head_trim() {
    let lang = toy_lang();
    let laid = run(&lang, "  \\runin{T} text", FragmentLayout::Blocks);
    let first: Vec<NodeRef<'_, Flm>> = laid.children()[0].children().iter().collect();
    assert_eq!(first.len(), 2, "{}", laid.dump());
    assert_eq!(element_of(&first[0]).name.as_str(), "t.runin");
    assert_eq!(first[1].chars(), Some("text"));
}

#[test]
fn a_leading_decorator_closes_the_paragraph_running_before_it() {
    let lang = toy_lang();
    let laid = run(&lang, "lead in \\runin{T} body", FragmentLayout::Blocks);
    let paras = laid.children();
    assert_eq!(paras.len(), 2, "{}", laid.dump());
    assert_eq!(text_of(paras[0]), "lead in");
    assert_eq!(
        element_of(&paras[1].child(0).unwrap()).name.as_str(),
        "t.runin"
    );
    assert_eq!(paras[1].child(1).and_then(|n| n.chars()), Some("body"));
}

#[test]
fn a_trailing_decorator_closes_its_paragraph_and_the_text_before_it_is_tail_trimmed() {
    let lang = toy_lang();
    let laid = run(&lang, "body text \\qed\nmore", FragmentLayout::Blocks);
    let paras = laid.children();
    assert_eq!(paras.len(), 2, "{}", laid.dump());
    let ended: Vec<NodeRef<'_, Flm>> = paras[0].children().iter().collect();
    assert_eq!(ended.len(), 2, "{}", laid.dump());
    assert_eq!(ended[0].chars(), Some("body text"));
    assert_eq!(element_of(&ended[1]).name.as_str(), "t.qed");
    assert_eq!(text_of(paras[1]), "more");
}

#[test]
fn a_free_decorator_introduces_no_boundary() {
    let lang = toy_lang();
    let laid = run(&lang, "one \\marginnote{n} two", FragmentLayout::Blocks);
    let paras = laid.children();
    assert_eq!(paras.len(), 1, "{}", laid.dump());
    let parts: Vec<NodeRef<'_, Flm>> = paras[0].children().iter().collect();
    assert_eq!(parts.len(), 3, "{}", laid.dump());
    assert_eq!(parts[0].chars(), Some("one "));
    assert_eq!(element_of(&parts[1]).name.as_str(), "t.marginnote");
    assert_eq!(parts[2].chars(), Some(" two"));
}

// --- absorption ----------------------------------------------------------------------

#[test]
fn absorb_before_removes_the_preceding_whitespace_outright() {
    let lang = toy_lang();
    // Inline layout: the edges are not trimmed, so absorption is isolated.
    let laid = run(
        &lang,
        "word   \\snug{x} rest",
        FragmentLayout::InlineOrBlocks,
    );
    let children = laid.children();
    assert_eq!(children.len(), 3, "{}", laid.dump());
    assert_eq!(children[0].chars(), Some("word"));
    assert_eq!(element_of(&children[1]).name.as_str(), "t.snug");
    assert_eq!(children[2].chars(), Some(" rest"));
}

#[test]
fn absorb_before_sees_through_a_comment() {
    let lang = toy_lang();
    for source in ["word %% note\n\\snug{x} rest", "word %% note\n   \\snug{x} rest"] {
        let laid = run(&lang, source, FragmentLayout::InlineOrBlocks);
        let texts: Vec<String> =
            laid.children().iter().filter_map(|node| node.chars().map(str::to_string)).collect();
        assert_eq!(texts.first().map(String::as_str), Some("word"), "{source:?}: {}", laid.dump());
        assert!(texts.iter().all(|text| text == "word" || text == " rest"), "{source:?}: {}", laid.dump());
    }
}

#[test]
fn absorb_after_sees_through_a_comment() {
    let lang = toy_lang();
    let laid = run(&lang, "a \\tight %% note\n   b", FragmentLayout::InlineOrBlocks);
    let texts: Vec<String> =
        laid.children().iter().filter_map(|node| node.chars().map(str::to_string)).collect();
    assert_eq!(texts, ["a ", "b"], "{}", laid.dump());
}

#[test]
fn absorb_after_removes_the_following_whitespace_outright() {
    let lang = toy_lang();
    let laid = run(&lang, "a \\tight   b", FragmentLayout::InlineOrBlocks);
    let children = laid.children();
    assert_eq!(children.len(), 3, "{}", laid.dump());
    assert_eq!(children[0].chars(), Some("a "));
    assert_eq!(children[2].chars(), Some("b"));
}

#[test]
fn an_edge_only_edit_needs_no_source_map() {
    let lang = toy_lang();
    let laid = run(&lang, "word   \\snug{x}", FragmentLayout::InlineOrBlocks);
    let word = laid.children()[0];
    assert!(word.ext().source_map.is_none());
    assert!(!is_owned(&word));
    assert_eq!(word.span_content(), "word");
}

// --- violations --------------------------------------------------------------------

#[test]
fn a_block_element_under_the_inline_layout_is_kept_in_place_and_reported() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "before \\begin{quotation}q\\end{quotation} after",
        FragmentLayout::Inline,
    );
    assert_eq!(
        laid.identifiers(),
        [BlockInInline::IDENTIFIER],
        "{}",
        laid.dump()
    );
    assert!(paragraphs(laid.root()).is_empty());
    let kept = laid.children();
    assert_eq!(kept.len(), 3, "{}", laid.dump());
    assert_eq!(element_of(&kept[1]).name.as_str(), "t.quotation");
    let condition = laid.diags.conditions::<BlockInInline>().next().unwrap();
    assert_eq!(condition.element, "t.quotation");
}

#[test]
fn a_paragraph_break_inside_a_group_becomes_a_space_and_is_reported() {
    let lang = toy_lang();
    let laid = run(&lang, "x {a\n\nb} y", FragmentLayout::Blocks);
    assert_eq!(
        laid.identifiers(),
        [ParbreakInInline::IDENTIFIER],
        "{}",
        laid.dump()
    );
    let group = laid.children()[0].child(1).expect("the group");
    assert_eq!(text_of(group), "a b", "{}", laid.dump());
    assert_eq!(
        group.child_count(),
        1,
        "the recovery space merged into one text node"
    );
    let condition = laid.diags.conditions::<ParbreakInInline>().next().unwrap();
    assert_eq!(condition.context, "group");
}

#[test]
fn a_paragraph_break_inside_an_argument_is_recovered_through_the_content_swap_route() {
    let lang = toy_lang();
    let laid = run(&lang, "\\emph{a\n\nb} tail", FragmentLayout::Inline);
    assert_eq!(
        laid.identifiers(),
        [ParbreakInInline::IDENTIFIER],
        "{}",
        laid.dump()
    );
    let emph = laid.children()[0];
    let content = emph
        .argument_content_nodes(0)
        .expect("the argument's content record");
    assert_eq!(content.len(), 1, "{}", laid.dump());
    assert_eq!(content.get(0).and_then(|n| n.chars()), Some("a b"));
}

#[test]
fn a_paragraph_break_at_the_root_of_an_inline_fragment_is_reported() {
    let lang = toy_lang();
    let laid = run(&lang, "a\n\nb", FragmentLayout::Inline);
    assert_eq!(laid.identifiers(), [ParbreakInInline::IDENTIFIER]);
    assert_eq!(laid.children().len(), 1);
    assert_eq!(laid.children()[0].chars(), Some("a b"));
}

// --- regions: bodies per BodyDef, arguments inline -----------------------------------

#[test]
fn an_inline_body_of_an_inline_environment_is_laid_out_inline() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{inlineenv}a\n\nb\\end{inlineenv}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        laid.identifiers(),
        [ParbreakInInline::IDENTIFIER],
        "{}",
        laid.dump()
    );
    let paragraph = laid.children()[0];
    assert!(
        is_paragraph(&paragraph),
        "the inline environment sits in a paragraph"
    );
    let env = paragraph.child(0).unwrap();
    assert_eq!(text_of(env), "a b");
    assert!(env.body().unwrap().iter().all(|n| !is_paragraph(&n)));
}

#[test]
fn a_block_environment_with_an_inline_body_declaration_keeps_its_body_inline() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{blockinline}x\n\ny\\end{blockinline}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        laid.identifiers(),
        [ParbreakInInline::IDENTIFIER],
        "{}",
        laid.dump()
    );
    let env = laid.children()[0];
    assert!(!is_paragraph(&env));
    let body = env.body().unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body.first().and_then(|n| n.chars()), Some("x y"));
}

#[test]
fn a_block_environment_with_the_default_body_declaration_builds_paragraphs_in_its_body() {
    // The default body policy, through `BodyLayout::FromRole`.
    // See design rationale at [§flm-dd-dr:lang-body-fromrole].
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{blockenv}a\n\nb\\end{blockenv}",
        FragmentLayout::Blocks,
    );
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
    let env = laid.children()[0];
    assert!(!is_paragraph(&env));
    let body = env.body().unwrap();
    assert_eq!(body.len(), 2, "{}", laid.dump());
    assert!(body.iter().all(|n| is_paragraph(&n)));
}

#[test]
fn an_inline_environment_with_a_blocks_body_declaration_builds_paragraphs_in_its_body() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{blocksbody}a\n\nb\\end{blocksbody}",
        FragmentLayout::InlineOrBlocks,
    );
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
    let env = laid.children()[0];
    assert!(
        !is_paragraph(&env),
        "the environment itself is inline: no paragraph at the root"
    );
    let body = env.body().unwrap();
    assert_eq!(body.len(), 2, "{}", laid.dump());
    assert!(body.iter().all(|n| is_paragraph(&n)));
}

#[test]
fn an_argument_region_is_inline_by_default() {
    let lang = toy_lang();
    let laid = run(&lang, "\\section{a\n\nb}", FragmentLayout::Blocks);
    assert_eq!(
        laid.identifiers(),
        [ParbreakInInline::IDENTIFIER],
        "{}",
        laid.dump()
    );
    let section = laid.children()[0];
    assert!(!is_paragraph(&section));
    let content = section.argument_content_nodes(0).unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.get(0).and_then(|n| n.chars()), Some("a b"));
}

#[test]
fn a_verbatim_body_is_carried_over_untouched() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{verbatimcode}\n  a--b  \n\\end{verbatimcode}",
        FragmentLayout::Inline,
    );
    let env = laid.children()[0];
    let group = env.body().unwrap().first().unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Verbatim));
    assert_eq!(group.child(0).and_then(|n| n.chars()), Some("  a--b  \n"));
}

#[test]
fn argument_noise_outside_the_content_is_copied_verbatim() {
    let lang = toy_lang();
    let laid = run(&lang, "\\emph  {a}", FragmentLayout::Inline);
    let emph = laid.children()[0];
    assert_eq!(emph.span_content(), "\\emph  {a}");
    assert_eq!(
        emph.argument_content_nodes(0).unwrap().source_text(),
        Some("a")
    );
}

// --- opaque regions ------------------------------------------------------------------

#[test]
fn a_paragraph_break_inside_a_math_group_is_kept_and_not_reported() {
    let lang = toy_lang();
    let laid = run(&lang, "\\(a\n\nb\\)", FragmentLayout::Blocks);
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
    let math = laid.children()[0].child(0).expect("the math group in its paragraph");
    assert_eq!(
        math.group_type(),
        Some(FlmGroupType::Math(MathGroupForm::Inline))
    );
    let parbreak = &lang.structural().parbreak;
    assert!(
        math.children()
            .iter()
            .any(|node| Arc::ptr_eq(element_of(&node), parbreak)),
        "{}",
        laid.dump()
    );
}

#[test]
fn a_paragraph_break_inside_a_math_mode_body_is_kept_and_not_reported() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{equation}a\n\nb\\end{equation}",
        FragmentLayout::Blocks,
    );
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
    let equation = laid.children()[0].child(0).expect("the environment in its paragraph");
    assert_eq!(element_of(&equation).name.as_str(), "t.equation");
    let parbreak = &lang.structural().parbreak;
    assert!(
        equation
            .body()
            .unwrap()
            .iter()
            .any(|node| Arc::ptr_eq(element_of(&node), parbreak)),
        "{}",
        laid.dump()
    );
}

// --- comments are inert --------------------------------------------------------------

#[test]
fn a_comment_between_blocks_stays_a_region_child() {
    let lang = toy_lang();
    let laid = run(&lang, "a\n\n%% c\n\nb", FragmentLayout::Blocks);
    let children = laid.children();
    assert_eq!(children.len(), 3, "{}", laid.dump());
    assert!(is_paragraph(&children[0]));
    assert!(children[1].is_comment());
    assert!(is_paragraph(&children[2]));
}

#[test]
fn a_comment_is_byte_identical() {
    let lang = toy_lang();
    let laid = run(&lang, "a\n\n%% c\n\nb", FragmentLayout::Blocks);
    let comment = laid.children()[1];
    let data = comment.comment().expect("comment payload");
    assert_eq!(data.content.resolve(comment.span().source()), " c");
    assert_eq!(comment.span_content(), "%% c");
}

#[test]
fn a_comment_inside_a_paragraph_run_stays_in_the_paragraph() {
    let lang = toy_lang();
    let laid = run(&lang, "a %% note\nb", FragmentLayout::Blocks);
    let paras = laid.children();
    assert_eq!(paras.len(), 1, "{}", laid.dump());
    let parts: Vec<NodeRef<'_, Flm>> = paras[0].children().iter().collect();
    assert_eq!(parts.len(), 3, "{}", laid.dump());
    assert_eq!(parts[0].chars(), Some("a "));
    assert!(parts[1].is_comment());
    assert_eq!(parts[2].chars(), Some("b"));
}

#[test]
fn a_comment_never_starts_a_paragraph_of_its_own() {
    let lang = toy_lang();
    let laid = run(&lang, "%% c\n\ntext", FragmentLayout::Blocks);
    let children = laid.children();
    assert_eq!(children.len(), 2, "{}", laid.dump());
    assert!(children[0].is_comment());
    assert!(is_paragraph(&children[1]));
}

// --- the techy fact that shapes the pass ---------------------------------------------

/// A deliberately naive visitor: it rebuilds every non-root `List` — the wrapper
/// an environment body's content designation is anchored on — with `count`
/// childless replacements.
struct NaiveWrapperRebuild {
    count: usize,
}

impl techy::transform::RestageVisitor<Flm, (), ()> for NaiveWrapperRebuild {
    type Error = String;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, String> {
        if node.is_list() && node.parent().is_some() {
            let mut ids = Vec::new();
            for _ in 0..self.count {
                let kind = NodeKind::list();
                let span = node.span().clone();
                let state = Arc::clone(node.parsing_state());
                let builder = cx.builder();
                let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))
                    .map_err(|e| format!("{e:?}"))?;
                ids.push(
                    builder
                        .add(kind, span, state, Vec::new(), ext, ())
                        .map_err(|e| format!("{e:?}"))?,
                );
            }
            return Ok(Restage::Emit(ids));
        }
        Ok(Restage::Descend(()))
    }
}

#[test]
fn rebuilding_a_region_wrapper_with_fewer_children_is_a_staging_error() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, "\\begin{quotation}a\n\nb\\end{quotation}");
    let result = TreeRestager::new(&mut NaiveWrapperRebuild { count: 1 }).restage(fragment.tree());
    assert!(
        matches!(
            result,
            Err(RestageError::Build(
                NodeBuildError::ContentOutOfBounds { .. }
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn replacing_a_region_wrapper_by_several_nodes_drops_the_content_parent() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, "\\begin{quotation}a\n\nb\\end{quotation}");
    let result = TreeRestager::new(&mut NaiveWrapperRebuild { count: 2 }).restage(fragment.tree());
    assert!(
        matches!(result, Err(RestageError::ContentParentDropped { .. })),
        "{result:?}"
    );
}

#[test]
fn the_pass_changes_a_body_regions_child_count_through_the_content_swap_route() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{quotation}a\n\nb\\end{quotation}",
        FragmentLayout::Blocks,
    );
    let body = laid.children()[0].body().expect("the body slot");
    assert_eq!(body.len(), 2, "{}", laid.dump());
    assert!(body.iter().all(|n| is_paragraph(&n)));
}

// --- snippet trees --------------------------------------------------------------------

/// A one-node tree holding a text node, staged under the given state.
fn text_root(state: &Arc<ParsingState<Flm>>, text: &str) -> techy::core::node::NodeTree<Flm> {
    let source = source(text);
    let span = SourceSpan::entire(&source);
    let state = Arc::clone(state);
    let mut builder: NodeTreeBuilder<Flm> = NodeTreeBuilder::new();
    let kind = NodeKind::chars(span.span());
    let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[])).unwrap();
    let root = builder.add(kind, span, state, Vec::new(), ext, ()).unwrap();
    builder.finish(root).unwrap()
}

#[test]
fn a_snippet_whose_root_is_a_text_node_is_resolved_in_place() {
    let lang = toy_lang();
    let tree = text_root(lang.initial_state(), "a   b");
    let mut diags = FlmDiagnostics::new();
    let out = structure(
        &tree,
        FragmentLayout::Inline,
        &BlocksConfig::default(),
        &mut diags,
    )
    .unwrap();
    assert_eq!(out.root().chars(), Some("a b"));
}

#[test]
fn a_snippet_root_that_resolves_to_nothing_becomes_an_empty_list() {
    let lang = toy_lang();
    let tree = text_root(lang.initial_state(), "   ");
    let mut diags = FlmDiagnostics::new();
    let out = structure(
        &tree,
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        &mut diags,
    )
    .unwrap();
    assert!(out.root().is_list());
    assert_eq!(out.root().child_count(), 0);
}

#[test]
fn a_snippet_whose_root_was_parsed_in_math_mode_is_untouched() {
    // A math snippet's root run is an opaque region.
    let lang = toy_lang();
    let math = lang
        .derive_state(&ParsingStateDelta::new().mode(Mode::Math))
        .unwrap();
    let tree = text_root(&math, "a   b");
    let mut diags = FlmDiagnostics::new();
    let out = structure(
        &tree,
        FragmentLayout::Inline,
        &BlocksConfig::default(),
        &mut diags,
    )
    .unwrap();
    assert_eq!(out.root().chars(), Some("a   b"));
    assert!(diags.is_empty(), "{}", diags.render_all());
}
