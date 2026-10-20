//! WP-1: the node reader — `NodeFacade` over fragments parsed with the
//! standard package, the key-value façades and the text readers (plan A,
//! section 7.2).

mod common;

use std::sync::Arc;

use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::FragmentLayout;
use flm_core::techy::core::node::NodeTree;
use flm_ext::adapter::node::NodeFacade;
use flm_ext::api::core::lowering::{chars_at_lowering, spelling_at_lowering};
use flm_ext::api::lang::mint::Mode;
use flm_ext::api::lang::rewrites::blocks::argument_holds_blocks;
use flm_ext::api::lang::text::{chars_text, read_chars, CharsContent};
use flm_ext::api::techy::extract::parse_keyval_drop_annotations;
use flm_ext::api::techy::node::{Node, NodeKindName, Span};
use flm_ext::ExtError;

/// A boxed node, as the API answers them.
type BoxNode = Box<dyn Node<'static>>;

/// The tree of the fragment `text`, parsed with the standard package.
fn tree_of(text: &str, layout: FragmentLayout) -> Arc<NodeTree<Flm>> {
    let registry = common::registry_with([]);
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, layout, &ParseOptions::default()).expect("the fragment parses");
    Arc::clone(fragment.tree())
}

/// The root of the fragment `text`, read through `NodeFacade::shared`.
fn root_of(text: &str) -> BoxNode {
    let tree = tree_of(text, FragmentLayout::default());
    let id = tree.root().id();
    Box::new(NodeFacade::shared(tree, id))
}

/// The first descendant of `root` named `name`.
fn named(root: &dyn Node<'static>, name: &str) -> BoxNode {
    root.descendants().into_iter().find(|node| node.name().as_deref() == Some(name)).expect("the node is there")
}

/// The characters of each node of `nodes`.
fn chars_of(nodes: &[BoxNode]) -> Vec<Option<String>> {
    nodes.iter().map(|node| node.chars()).collect()
}

#[test]
fn a_node_answers_its_kind_name_and_chars() {
    let root = root_of("\\emph{ab} cd");
    assert_eq!(root.kind(), NodeKindName::List);
    assert!(root.is_list());
    let emph = root.child(0).expect("a first child");
    assert_eq!(emph.kind(), NodeKindName::Callable);
    assert!(emph.is_callable() && !emph.is_chars() && !emph.is_group() && !emph.is_comment());
    assert_eq!(emph.name().as_deref(), Some("emph"));
    assert_eq!(emph.chars(), None);
    assert_eq!(emph.element_name(), "flm.textstyle");
    let text = root.child(1).expect("a second child");
    assert_eq!(text.kind(), NodeKindName::Chars);
    assert_eq!(text.chars().as_deref(), Some(" cd"));
    assert_eq!(text.name(), None);
    assert!(root.child(2).is_none());
    let group = emph.child(0).expect("the argument's group");
    assert_eq!(group.kind(), NodeKindName::Group);
    assert_eq!(group.group_delimiters(), Some(("{".to_string(), "}".to_string())));
}

#[test]
fn children_and_descendants_are_in_document_order() {
    let root = root_of("\\emph{ab} cd");
    assert_eq!(root.child_count(), 2);
    let kinds: Vec<NodeKindName> = root.children().iter().map(|node| node.kind()).collect();
    assert_eq!(kinds, [NodeKindName::Callable, NodeKindName::Chars]);
    let descendants = root.descendants();
    let read: Vec<(NodeKindName, Option<String>)> = descendants.iter().map(|node| (node.kind(), node.chars())).collect();
    assert_eq!(
        read,
        [
            (NodeKindName::Callable, None),
            (NodeKindName::Group, None),
            (NodeKindName::Chars, Some("ab".to_string())),
            (NodeKindName::Chars, Some(" cd".to_string())),
        ]
    );
    // A relative's parent is the node it was asked on; the relatives outlive
    // the façade they were asked on.
    let leaf = descendants.get(2).expect("the characters");
    let parent = leaf.parent().expect("a parent");
    assert_eq!(parent.kind(), NodeKindName::Group);
    drop(root);
    assert_eq!(parent.parent().and_then(|emph| emph.name()).as_deref(), Some("emph"));
    assert!(parent.parent().and_then(|emph| emph.parent()).and_then(|root| root.parent()).is_none());
}

#[test]
fn a_lent_node_answers_as_a_shared_one() {
    let tree = tree_of("\\emph{ab} cd", FragmentLayout::default());
    let lent = NodeFacade::lent(tree.root());
    let emph = Node::child(&lent, 0).expect("a first child");
    assert_eq!(emph.name().as_deref(), Some("emph"));
    assert_eq!(chars_text(&emph.argument_content_nodes(0).expect("written")).as_deref(), Some("ab"));
    assert_eq!(lent.node_ref().id(), tree.root().id());
}

#[test]
fn named_arguments_are_read_as_nodes_and_as_content() {
    let root = root_of("\\emph{ab} \\section[numbered=false]{Title}");
    let emph = named(&*root, "emph");
    let nodes = emph.argument_nodes_named("content").expect("declared").expect("written");
    assert_eq!(nodes.iter().map(|node| node.kind()).collect::<Vec<_>>(), [NodeKindName::Group]);
    let content = emph.argument_content_nodes_named("content").expect("declared").expect("written");
    assert_eq!(chars_of(&content), [Some("ab".to_string())]);
    assert_eq!(chars_of(&emph.argument_content_nodes(0).expect("written")), [Some("ab".to_string())]);
    assert_eq!(emph.argument_nodes(0).map(|nodes| nodes.len()), Some(1));
    assert!(emph.argument_nodes(5).is_none());
    let section = named(&*root, "section");
    let title = section.argument_content_nodes_named("title").expect("declared").expect("written");
    assert_eq!(chars_text(&title).as_deref(), Some("Title"));
    assert!(section.argument_nodes_named("star").expect("declared").is_none());
    let unknown = emph.argument_nodes_named("nothing").map(|_| ());
    assert!(matches!(unknown, Err(ExtError::Core { ref kind, .. }) if kind == "access"), "{unknown:?}");
    let not_callable = root.argument_content_nodes_named("content").map(|_| ());
    assert!(matches!(not_callable, Err(ExtError::Core { .. })), "{not_callable:?}");
}

#[test]
fn a_body_and_a_slot_are_read_as_nodes() {
    let root = root_of("\\begin{itemize}\\item x\\end{itemize}");
    let list = named(&*root, "itemize");
    let body = list.body().expect("a body");
    assert_eq!(body.first().and_then(|node| node.name()).as_deref(), Some("item"));
    assert!(root.body().is_none());
    assert!(matches!(list.slot_content_nodes_named("nothing"), Err(ExtError::Core { .. })));
}

#[test]
fn a_star_argument_reports_whether_it_was_written() {
    let root = root_of("\\section*{One} \\section{Two}");
    let sections: Vec<BoxNode> = root.descendants().into_iter().filter(|node| node.name().as_deref() == Some("section")).collect();
    let starred: Vec<bool> = sections.iter().map(|node| node.argument_is_provided("star").expect("declared")).collect();
    assert_eq!(starred, [true, false]);
    let options = sections.first().map(|node| node.argument_is_provided("options").expect("declared"));
    assert_eq!(options, Some(false));
    let unknown = sections.first().map(|node| node.argument_is_provided("nothing"));
    assert!(matches!(unknown, Some(Err(ExtError::Core { .. }))), "{unknown:?}");
    assert!(matches!(root.argument_is_provided("star"), Err(ExtError::Core { .. })));
}

#[test]
fn key_values_are_read_from_an_optional_argument() {
    let root = root_of("\\section[numbered=false, toc={a,b}, toc=c, bare]{Title}");
    let section = named(&*root, "section");
    let options = section.argument_content_nodes_named("options").expect("declared").expect("written");
    let list = parse_keyval_drop_annotations(&options).expect("a key-value list");
    // The list and its nodes outlive the nodes it was read from.
    drop(options);
    drop(section);
    drop(root);
    assert_eq!(list.len(), 4);
    assert!(!list.is_empty());
    let keys: Vec<String> = list.iter().iter().map(|entry| entry.key()).collect();
    assert_eq!(keys, ["numbered", "toc", "toc", "bare"]);
    let numbered = list.get("numbered").expect("an entry");
    assert_eq!(chars_text(&numbered.value_content().expect("a value")).as_deref(), Some("false"));
    // `get` answers the last entry of a key.
    let toc = list.get("toc").expect("an entry");
    assert_eq!(chars_text(&toc.value_content().expect("a value")).as_deref(), Some("c"));
    // A value that is one group: `value` keeps it, `value_content` opens it.
    let first_toc = list.keyval(1).expect("the second entry");
    let value = first_toc.value().expect("a value");
    assert_eq!(value.iter().map(|node| node.kind()).collect::<Vec<_>>(), [NodeKindName::Group]);
    assert_eq!(chars_text(&first_toc.value_content().expect("a value")).as_deref(), Some("a,b"));
    assert!(list.get("bare").expect("an entry").value().is_none());
    assert!(list.get("nothing").is_none());
    assert!(list.keyval(4).is_none());
}

#[test]
fn key_values_are_read_only_from_consecutive_children_of_one_node() {
    let root = root_of("\\emph{ab} cd");
    let empty = parse_keyval_drop_annotations(&[]).expect("the empty list");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(empty.iter().is_empty());
    // Two nodes of different parents.
    let scattered: Vec<BoxNode> = root.descendants().into_iter().filter(|node| node.is_chars()).collect();
    let refused = parse_keyval_drop_annotations(&scattered).map(|_| ());
    assert!(matches!(refused, Err(ExtError::Argument { ref method, .. }) if method == "parse_keyval_drop_annotations"), "{refused:?}");
    // Two children out of order.
    let mut reversed = root.children();
    reversed.reverse();
    assert!(matches!(parse_keyval_drop_annotations(&reversed).map(|_| ()), Err(ExtError::Argument { .. })));
    // Two nodes of two trees.
    let other = root_of("ab");
    let two_trees: Vec<BoxNode> = [root.child(0), other.child(0)].into_iter().flatten().collect();
    assert!(matches!(parse_keyval_drop_annotations(&two_trees).map(|_| ()), Err(ExtError::Argument { .. })));
    // A run holding a command is not a key-value list.
    let not_keyvals = parse_keyval_drop_annotations(&root.children()).map(|_| ());
    assert!(matches!(not_keyvals, Err(ExtError::Core { ref kind, .. }) if kind == "extract"), "{not_keyvals:?}");
}

#[test]
fn read_chars_tells_characters_from_a_command() {
    let tree = tree_of("\\emph{a{b}c} x\\emph{y}", FragmentLayout::default());
    let root: BoxNode = Box::new(NodeFacade::shared(Arc::clone(&tree), tree.root().id()));
    let emph = named(&*root, "emph");
    let content = emph.argument_content_nodes(0).expect("written");
    assert_eq!(read_chars(&content), Ok(CharsContent { text: Some("abc".to_string()), has_placeholder: false }));
    assert_eq!(chars_text(&content).as_deref(), Some("abc"));
    let children = root.children();
    let not = read_chars(&children).expect_err("a command");
    let first = children.first().map(|node| node.span());
    assert_eq!(first, Some(Span { start: 0, end: 12 }));
    // The offending node is the first command, a node of the tree read.
    assert_eq!(tree.node(not.node).name(), Some("emph"));
    assert_eq!(chars_text(&children), None);
    assert_eq!(read_chars(&[]), Ok(CharsContent { text: Some(String::new()), has_placeholder: false }));
}

#[test]
fn the_lowering_text_readers_answer_on_an_argument() {
    let root = root_of("\\emph{ab} \\emph{a\\emph{b}}");
    let emphs: Vec<BoxNode> = root.children().into_iter().filter(|node| node.is_callable()).collect();
    let plain = emphs.first().and_then(|node| node.argument_content_nodes(0)).expect("written");
    let marked = emphs.get(1).and_then(|node| node.argument_content_nodes(0)).expect("written");
    assert_eq!(chars_at_lowering(&plain), Ok(Ok("ab".to_string())));
    assert!(matches!(chars_at_lowering(&marked), Ok(Err(_))));
    assert_eq!(spelling_at_lowering(&plain), Ok(()));
    assert_eq!(spelling_at_lowering(&marked), Ok(()));
}

#[test]
fn an_argument_laid_out_as_blocks_holds_blocks() {
    let root = root_of("\\footnote{a\n\nb} \\footnote{c}");
    let notes: Vec<BoxNode> = root.children().into_iter().filter(|node| node.is_callable()).collect();
    let answers: Vec<bool> = notes.iter().map(|node| argument_holds_blocks(&**node, 1)).collect();
    assert_eq!(answers, [true, false]);
    assert!(!argument_holds_blocks(&*root, 0));
}

#[test]
fn element_value_is_none_on_a_standard_element() {
    let root = root_of("\\emph{ab} \\section{T}");
    assert!(root.descendants().iter().all(|node| node.element_value().is_none()));
    assert!(root.element_value().is_none());
}

#[test]
fn a_span_gives_byte_offsets_and_its_text() {
    let root = root_of("é \\emph{ab}");
    let emph = named(&*root, "emph");
    assert_eq!(emph.span(), Span { start: 3, end: 12 });
    assert_eq!(emph.span_content(), "\\emph{ab}");
    let leaf = emph.descendants().into_iter().find(|node| node.is_chars()).expect("the characters");
    assert_eq!(leaf.span(), Span { start: 9, end: 11 });
    assert_eq!(leaf.span_content(), "ab");
    assert_eq!(root.span(), Span { start: 0, end: 12 });
}

#[test]
fn mode_is_math_inside_a_formula() {
    let root = root_of("a \\(x+1\\)");
    assert_eq!(root.mode(), Mode::Text);
    let text = root.child(0).expect("the text");
    assert_eq!(text.mode(), Mode::Text);
    let formula = root.descendants().into_iter().find(|node| node.chars().as_deref() == Some("x+1")).expect("the formula's characters");
    assert_eq!(formula.mode(), Mode::Math);
}

// --- E5: a node's payload and its slots by index ------------------------------------------------

#[test]
fn a_parsed_node_answers_its_slots_by_index_and_no_payload() {
    let root = root_of("\\begin{itemize}\\item x\\end{itemize}");
    let list = named(&*root, "itemize");
    assert_eq!(list.payload(), Ok(None));
    let count = list.slot_count();
    assert!(count >= 1, "{count}");
    // The body slot is one of the slots: its content is the body's.
    let body: Vec<Option<String>> = list.body().expect("a body").iter().map(|node| node.name()).collect();
    let by_index: Vec<Vec<Option<String>>> = (0..count)
        .map(|i| list.slot_content_nodes(i).expect("a slot").iter().map(|node| node.name()).collect())
        .collect();
    assert!(by_index.contains(&body), "{by_index:?}");
    assert!(list.slot_content_nodes(count).is_none());
    assert_eq!(list.slot_name(count), None);
    assert_eq!(root.slot_count(), 0);
    assert!(root.slot_content_nodes(0).is_none());
}

#[test]
fn a_lowered_list_node_answers_its_slots_and_a_heading_its_payload() {
    use std::sync::Mutex;

    use flm_ext::api::core::data::table::TableValue;
    use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
    use flm_core::stdcore::backends::pres;

    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&log);
    let double = common::double::Double::new("nd").feature("read", move |b| {
        b.table("nd.seen", "What the reader saw.")?;
        let seen = Arc::clone(&seen);
        let decl = StageDecl::new("nd.read", Effect::Compute, Scope::Document).writes("nd.seen");
        b.stage(common::double::stage(decl, Needs::new().reads("flm.lowered.text"), move |cx, _| {
            let Some(TableValue::Lowered(lowered)) = cx.get("flm.lowered.text")? else { return Ok(()) };
            let mut lines = Vec::new();
            for (_, root) in lowered.trees() {
                for node in root.descendants() {
                    let element = node.element_name();
                    if element == pres::BLOCK_HEADING {
                        let payload = node.payload()?.unwrap_or_default();
                        lines.push(format!("heading {} {}", payload["level"], payload["name"]));
                    }
                    if element == pres::LIST {
                        let slots: Vec<String> = (0..node.slot_count())
                            .map(|i| {
                                let content = node.slot_content_nodes(i).unwrap_or_default();
                                format!("{}={}", node.slot_name(i).unwrap_or_default(), text_under(&content).trim())
                            })
                            .collect();
                        lines.push(format!("list {}", slots.join(" ")));
                    }
                }
            }
            seen.lock().expect("the log").extend(lines);
            Ok(())
        }))
    });
    let registry = common::registry_with([double.package()]);
    let source = "\\section{Title}\n\n\\begin{itemize}\\item a\\item b\\end{itemize}";
    let ran = run_demanding(&registry, &["nd.seen"], source);
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    let log = log.lock().expect("the log").clone();
    assert_eq!(log, ["heading 1 \"section\"", "list label=• item=a label=• item=b"]);
}

/// The characters of `nodes` and of their descendants, in document order.
fn text_under(nodes: &[Box<dyn Node<'_> + '_>]) -> String {
    let mut text = String::new();
    for node in nodes {
        text.extend(node.chars());
        text.extend(node.descendants().iter().filter_map(|node| node.chars()));
    }
    text
}

/// The run of a plan demanding `demands` over the one-tree document `text`.
fn run_demanding(
    registry: &Arc<flm_core::core::registry::FlmRegistry>,
    demands: &[&str],
    text: &str,
) -> flm_core::core::run::Run {
    use flm_core::core::plan::{Plan, PlanRequest};
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::model::doc::{Doc, DocContent};
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}
