//! The `annotations` feature's cut: an annotation crossing a paragraph break
//! or holding a block is cut by block structuring into pieces of itself, each
//! carrying its record; one that does not is left as it is; a structured tree
//! structured again is unchanged; the records read back from a serialized
//! tree; and the documents that must neither panic nor loop.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::session::PayloadReaders;
use flm_core::model::addr::TreeId;
use flm_core::stdcore::CHECK_WORKFLOW;
use flm_core::lang::rewrites::blocks::{BlocksConfig, structure};
use flm_core::lang::{Flm, FlmDiagnostics, FlmGroupType, FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::aspects::{BlockRole, block_role_of};
use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::stdcore::backends::pres::read::payload_of;
use flm_core::stdcore::features::annotations::{
    ANNOTATION, AnnotationPiece, AnnotationsConfig, Annotator, TEXT_ARG, payload_readers,
};
use flm_core::stdcore::StdcoreConfig;
use techy::core::node::{NodeRef, NodeTree};

/// The two annotators these tests use: `phf` with the initials `PhF`,
/// annotator 0, and `abc` with none, annotator 1.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().annotations(
            AnnotationsConfig::new()
                .annotator(Annotator::new("phf").initials("PhF"))
                .annotator(Annotator::new("abc")),
        ),
    )
}

/// `text` parsed by `registry`'s language as one fragment laid out as blocks.
fn parsed(registry: &FlmRegistry, text: &str) -> Fragment {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    parse_fragment(registry.lang(), source, FragmentLayout::Blocks, &ParseOptions::default())
        .expect("a parse")
}

/// `tree` block-structured as a fragment laid out as blocks: the structured
/// tree and the identifiers of what the pass reported.
fn structured_tree(tree: &Arc<NodeTree<Flm>>) -> (Arc<NodeTree<Flm>>, Vec<String>) {
    let mut diags = FlmDiagnostics::new();
    let tree = structure(tree, FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags)
        .expect("block structuring succeeds");
    let reports = diags.iter().map(|d| d.identifier().to_string()).collect();
    (tree, reports)
}

/// `text` parsed and block-structured.
fn structured(text: &str) -> (Arc<NodeTree<Flm>>, Vec<String>) {
    let fragment = parsed(&registry(), text);
    structured_tree(fragment.tree())
}

/// A compact picture of `node` and what it holds: a paragraph is `P[…]`, a
/// paragraph-break marker `¶`, text is quoted, a comment is `%`, an
/// annotation is its command, its record `#cut.index/count` when it has one,
/// and its argument's content in parentheses; another command is its name
/// and the content of each of its arguments and slots in parentheses.
fn shape(node: &NodeRef<'_, Flm>) -> String {
    if node.is_comment() {
        return String::from("%");
    }
    if let Some(text) = node.chars() {
        return format!("{text:?}");
    }
    if node.group_type() == Some(FlmGroupType::Paragraph) {
        return format!("P[{}]", shapes(node.children().iter().collect::<Vec<_>>().iter()));
    }
    if node.ext().element.name.as_str() == "flm.parbreak" {
        return String::from("¶");
    }
    let Some(name) = node.name() else {
        return format!("{{{}}}", shapes(node.children().iter().collect::<Vec<_>>().iter()));
    };
    if node.ext().element.name == ANNOTATION {
        let record = payload_of::<AnnotationPiece>(node)
            .map(|piece| format!("#{}.{}/{}", piece.cut, piece.index, piece.count))
            .unwrap_or_default();
        let content = node.argument_content_nodes_named(TEXT_ARG).ok().flatten();
        let content: Vec<NodeRef<'_, Flm>> = content.map(|slice| slice.iter().collect()).unwrap_or_default();
        return format!("{name}{record}({})", shapes(content.iter()));
    }
    let mut parts = Vec::new();
    let arguments = node.arguments().map(|args| args.len()).unwrap_or(0);
    for index in 0..arguments {
        if let Some(content) = node.argument_content_nodes(index) {
            parts.push(format!("({})", shapes(content.iter().collect::<Vec<_>>().iter())));
        }
    }
    let slots = node.slots().map(|slots| slots.len()).unwrap_or(0);
    for index in 0..slots {
        if let Some(content) = node.slot_content_nodes(index) {
            parts.push(format!("({})", shapes(content.iter().collect::<Vec<_>>().iter())));
        }
    }
    format!("{name}{}", parts.concat())
}

/// The shapes of a run of nodes, separated by spaces.
fn shapes<'a, 't: 'a>(nodes: impl Iterator<Item = &'a NodeRef<'t, Flm>>) -> String {
    nodes.map(shape).collect::<Vec<_>>().join(" ")
}

/// The shape of every top-level node of `tree`, one per entry.
fn top(tree: &NodeTree<Flm>) -> Vec<String> {
    tree.root().children().iter().map(|node| shape(&node)).collect()
}

/// Every record of `tree`, in document order.
fn records(tree: &NodeTree<Flm>) -> Vec<AnnotationPiece> {
    tree.root().descendants().filter_map(|node| payload_of::<AnnotationPiece>(&node).copied()).collect()
}

/// A record.
fn piece(cut: usize, index: usize, count: usize) -> AnnotationPiece {
    AnnotationPiece { cut, index, count }
}

// ---------------------------------------------------------------------------------------------
// the cut (P10b; test 11)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_comment_across_a_paragraph_break_continues_the_paragraph_and_opens_the_next() {
    let (tree, reports) = structured("Some text \\phf[A comment.\n\nWith multiple paragraphs.] More text");
    assert_eq!(reports, Vec::<String>::new());
    assert_eq!(
        top(&tree),
        [
            "P[\"Some text \" phf#0.0/2(\"A comment.\")]",
            "P[phf#0.1/2(\"With multiple paragraphs.\") \" More text\"]",
        ]
    );
    assert_eq!(records(&tree), [piece(0, 0, 2), piece(0, 1, 2)]);
    assert!(
        tree.root().descendants().all(|node| node.ext().element.name != ANNOTATION
            || block_role_of(&node) != BlockRole::Block),
        "no piece is a block"
    );
}

#[test]
fn the_two_highlight_forms_are_cut_alike() {
    for text in [
        "Some text \\phf A highlight.\n\nWith multiple paragraphs.\\endphf, more text",
        "Some text \\phf{A highlight.\n\nWith multiple paragraphs.}, more text",
    ] {
        let (tree, reports) = structured(text);
        assert_eq!(reports, Vec::<String>::new());
        assert_eq!(
            top(&tree),
            [
                "P[\"Some text \" phf#0.0/2(\"A highlight.\")]",
                "P[phf#0.1/2(\"With multiple paragraphs.\") \", more text\"]",
            ],
            "{text}"
        );
    }
}

#[test]
fn the_space_after_the_closing_command_belongs_to_it() {
    // As after every command named with letters: the closing command reads
    // the whitespace after it, which is then its own.
    let (tree, _) = structured("a \\phf b\\endphf  c");
    assert_eq!(top(&tree), ["P[\"a \" phf(\"b\") \"c\"]"]);
}

#[test]
fn every_piece_keeps_the_delimiters_its_annotation_was_written_with() {
    for (text, open, close) in [
        ("a \\phf[b\n\nc] d", "[", "]"),
        ("a \\phf{b\n\nc} d", "{", "}"),
        ("a \\phf b\n\nc\\endphf d", "", "\\endphf "),
    ] {
        let (tree, _) = structured(text);
        let pieces: Vec<NodeRef<'_, Flm>> =
            tree.root().descendants().filter(|node| node.ext().element.name == ANNOTATION).collect();
        assert_eq!(pieces.len(), 2);
        for node in &pieces {
            let region = node.arguments().expect("a callable").get_named(TEXT_ARG).expect("declared");
            let group = node.tree().get(region.region.as_ref().expect("written").content_parent()).expect("a node");
            assert_eq!(group.group_delimiters(), Some((open, close)), "{text}");
        }
    }
}

#[test]
fn a_list_in_a_passage_is_a_block_piece_between_two_inline_pieces() {
    let (tree, reports) = structured("\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf");
    assert_eq!(reports, Vec::<String>::new());
    let top = top(&tree);
    assert_eq!(top.len(), 3, "{top:?}");
    assert_eq!(top[0], "P[phf#0.0/3(\"see \")]");
    assert!(top[1].starts_with("phf#0.1/3(itemize"), "{top:?}");
    assert_eq!(top[2], "P[phf#0.2/3(\" here \")]");
    let block_piece = tree.root().child(1).expect("a second block");
    assert_eq!(block_role_of(&block_piece), BlockRole::Block);
}

#[test]
fn a_break_inside_an_inner_annotation_cuts_the_outer_one_too() {
    let (tree, reports) = structured("\\phf[see \\abc{this\n\nthat} end]");
    assert_eq!(reports, Vec::<String>::new());
    // The outer annotation is asked first, so its cut is numbered first.
    assert_eq!(
        top(&tree),
        ["P[phf#0.0/2(\"see \" abc#1.0/2(\"this\"))]", "P[phf#0.1/2(abc#1.1/2(\"that\") \" end\")]"]
    );
    assert_eq!(records(&tree).len(), 4);
}

#[test]
fn an_annotation_with_no_break_and_no_block_is_left_as_it_is() {
    let (tree, reports) = structured("a \\phf{b} \\abc[c] \\phf d\\endphf e");
    assert_eq!(reports, Vec::<String>::new());
    assert_eq!(records(&tree), Vec::<AnnotationPiece>::new());
    assert_eq!(top(&tree), ["P[\"a \" phf(\"b\") \" \" abc(\"c\") \" \" phf(\"d\") \"e\"]"]);
}

#[test]
fn whitespace_beside_a_break_stays_inside_the_pieces_as_parsed() {
    let (tree, _) = structured("x \\phf{a \n\n b} y");
    assert_eq!(top(&tree), ["P[\"x \" phf#0.0/2(\"a \")]", "P[phf#0.1/2(\" b\") \" y\"]"]);
}

#[test]
fn a_break_in_an_annotation_in_a_heading_title_is_reported_and_the_pieces_stay() {
    let (tree, reports) = structured("\\section{T \\phf{a\n\nb}}");
    assert_eq!(reports, ["flm.blocks.parbreak-in-inline"]);
    let top = top(&tree);
    assert_eq!(top.len(), 1, "{top:?}");
    assert!(top[0].contains("phf#0.0/2(\"a\") \" \" phf#0.1/2(\"b\")"), "{top:?}");
}

#[test]
fn a_run_of_breaks_and_comments_alone_makes_no_piece() {
    let (tree, reports) = structured("a \\phf{%%c\n\n} b");
    assert_eq!(reports, Vec::<String>::new());
    assert_eq!(records(&tree), Vec::<AnnotationPiece>::new());
    assert!(tree.root().descendants().all(|node| node.ext().element.name != ANNOTATION));
}

#[test]
fn structuring_a_structured_tree_changes_nothing() {
    for text in [
        "Some text \\phf[A comment.\n\nWith multiple paragraphs.] More text",
        "\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf",
        "\\phf[see \\abc{this\n\nthat} end]",
    ] {
        let (once, _) = structured(text);
        let (twice, reports) = structured_tree(&once);
        assert_eq!(reports, Vec::<String>::new());
        assert_eq!(top(&twice), top(&once), "{text}");
        assert_eq!(records(&twice), records(&once), "{text}");
    }
}

#[test]
fn a_frozen_tree_holding_pieces_reads_back_with_its_records() {
    let registry = registry();
    let text = "Some text \\phf[A comment.\n\nWith multiple paragraphs.] More text";
    let run = common::stdcore::run_workflow(&registry, CHECK_WORKFLOW, &[text], &Overrides::none());
    let tree = run.frozen_tree(TreeId::tree(0)).expect("a frozen tree");
    assert_eq!(records(tree), [piece(0, 0, 2), piece(0, 1, 2)]);
    let elements: Vec<_> = registry.elements().iter().map(|entry| Arc::clone(&entry.element)).collect();
    let (position, segment) = common::dump::write_tree(registry.lang(), tree);
    let read = common::dump::read_tree(registry.lang(), segment.clone(), position, elements.clone(), payload_readers());
    assert_eq!(records(&read), records(tree));
    assert_eq!(top(&read), top(tree));
    // Without the feature's reader, the record cannot be read back.
    let refused = common::dump::try_read_tree(registry.lang(), segment, position, elements, PayloadReaders::new());
    assert!(refused.is_err());
}

#[test]
fn the_record_projects_and_reads_back() {
    use flm_core::lang::PresPayload;
    let record = piece(3, 1, 4);
    assert_eq!(AnnotationPiece::from_projection(&record.project()).expect("reads"), record);
    assert!(!record.is_last() && !record.is_first());
    assert!(piece(0, 0, 1).is_first() && piece(0, 0, 1).is_last());
}

// ---------------------------------------------------------------------------------------------
// documents that must neither panic nor loop (test 20, the annotation half)
// ---------------------------------------------------------------------------------------------

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let many_commands = "\\phf{x} \\abc[y] \\phf z\\endphf ".repeat(1000);
    let many_breaks = format!("\\phf{{{}}}", "a\n\n".repeat(1000));
    let nested = format!("{}x{}", "\\phf{".repeat(50), "}".repeat(50));
    for text in [
        "\\phf{a \\phf{b\n\nc} d}",
        "\\phf{a \\phf[b\n\nc] d}",
        "\\phf[a \\abc b\n\nc\\endabc d]",
        "\\phf",
        "\\phf{",
        "\\phf[",
        "\\phf{}",
        "\\phf[]",
        "\\phf\\endphf",
        many_commands.as_str(),
        many_breaks.as_str(),
        nested.as_str(),
    ] {
        let (once, _) = structured(text);
        let (twice, _) = structured_tree(&once);
        assert_eq!(records(&twice), records(&once));
    }
    let (tree, _) = structured(&many_breaks);
    assert_eq!(records(&tree).len(), 1000);
}
