//! An argument's layout (`flm_core::lang::mint::ArgLayout`): what the minting
//! records and refuses, how the block-structure pass lays each layout out, what
//! the contract checker holds each to, and the layout riding a stored tree.
//!
//! The test package declares one command per layout — `\inl` (inline, the
//! default), `\blk` (blocks), `\auto` (inferred from the content) — an
//! environment `boxed` whose argument is a region of blocks, and a block-level
//! environment `itemize`, over the toy declarations of the block-structure tests.
//! The last part adds a command `\note[…]{…}` whose optional argument names a
//! scoped package holding `\tag`, and checks where `\tag` is known.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::run::RunOptions;
use flm_core::model::aspects::{BlockRole, BlockRoleAspect};
use flm_core::model::element::ElementSpec;
use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::lang::mint::{
    ArgDef, ArgStdKind, ArgLayout, BodyDef, ElementDef, MintError, ScopedPackageDef, SyntaxDef,
    TackOnField, arg_layout_of,
};
use flm_core::lang::rewrites::blocks::{
    ContractViolation, ParbreakInInline, argument_holds_blocks, check_block_contract,
    check_parse_contract, slot_holds_blocks, stands_in_blocks,
};
use flm_core::lang::{
    BuiltLang, Flm, FlmCallableType, FlmGroupType, LangBuildError, LangInputs, build_language,
};
use flm_core::stdcore::features::blocks::{Blocks, STRUCTURED};
use techy::core::node::{
    BuildId, CallableData, ChildRegion, ContentNodes, GroupData, NodeKind, NodeRef, NodeTree, ParsedArgument,
    ParsedArguments, ParsedSlots, display_tree,
};
use techy::latexlike::{InvocationSyntaxData, Mode};

use common::blocks::{
    Laid, Site, element_with_role, is_paragraph, run, text_of, toy_elements,
    toy_lang_with,
};

// --- the test package ------------------------------------------------------------------------

/// The test package's declarations, owned by `owner`; its commands are
/// available in text and in math mode.
fn declarations(owner: &str) -> Vec<ElementDef> {
    let inline = |name: &str| common::element(&format!("{owner}.{name}"));
    let command = |name: &str, arg: ArgDef| {
        ElementDef::new(owner, [name], SyntaxDef::Macro { args: vec![arg] }, inline(name))
            .modes([Mode::Text, Mode::Math])
    };
    vec![
        command("inl", ArgDef::mandatory("text")),
        command("blk", ArgDef::mandatory("text").blocks()),
        command("auto", ArgDef::mandatory("text").layout(ArgLayout::InlineOrBlocks)),
        ElementDef::new(
            owner,
            ["boxed"],
            SyntaxDef::Environment {
                args: vec![ArgDef::mandatory("title").blocks()],
                body: BodyDef::new().blocks(),
            },
            element_with_role(&format!("{owner}.boxed"), BlockRole::Block),
        ),
        ElementDef::new(
            owner,
            ["itemize"],
            SyntaxDef::Environment { args: Vec::new(), body: BodyDef::new().blocks() },
            element_with_role(&format!("{owner}.itemize"), BlockRole::Block),
        ),
    ]
}

/// The toy language plus the test package.
fn lang() -> BuiltLang {
    toy_lang_with(declarations("a"))
}

/// The first callable node named `name` under `node`.
fn find<'t>(node: NodeRef<'t, Flm>, name: &str) -> NodeRef<'t, Flm> {
    node.descendants()
        .find(|n| n.is_callable() && n.name() == Some(name))
        .unwrap_or_else(|| panic!("no `\\{name}` under the node"))
}

/// The content nodes of `node`'s first argument.
fn content<'t>(node: NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    node.argument_content_nodes(0).map(|slice| slice.iter().collect()).unwrap_or_default()
}

/// The shape of a run: `P` for a paragraph, the element's name for another
/// callable, `text` for text.
fn shape(run: &[NodeRef<'_, Flm>]) -> Vec<String> {
    run.iter()
        .map(|node| {
            if is_paragraph(node) {
                "P".to_string()
            } else if node.is_chars() {
                "text".to_string()
            } else {
                flm_core::model::element::element_of(node).name.to_string()
            }
        })
        .collect()
}

/// `text` laid out as blocks under the test language.
fn laid(text: &str) -> Laid {
    run(&lang(), text, FragmentLayout::Blocks)
}

fn kinds(violations: &[ContractViolation]) -> Vec<&'static str> {
    violations.iter().map(|violation| violation.kind.as_str()).collect()
}

/// The mint errors of a language built over `elements`.
fn mint_errors(elements: Vec<ElementDef>) -> Vec<MintError> {
    let mut inputs = LangInputs::new();
    inputs.elements = elements;
    build_language(inputs)
        .errors()
        .iter()
        .filter_map(|error| match error {
            LangBuildError::Mint(error) => Some(error.clone()),
            _ => None,
        })
        .collect()
}

// --- 1. an inline argument -------------------------------------------------------------------

#[test]
fn a_blank_line_in_an_inline_argument_is_reported_and_read_as_one_space() {
    let laid = laid("\\inl{a\n\nb}");
    let conditions: Vec<&ParbreakInInline> = laid.diags.conditions::<ParbreakInInline>().collect();
    assert_eq!(conditions.len(), 1, "{:?}", laid.identifiers());
    assert_eq!(conditions[0].context, "argument");
    let inl = find(laid.root(), "inl");
    assert_eq!(shape(&content(inl)), ["text"]);
    assert_eq!(text_of(content(inl)[0]), "a b");
}

// --- 2. a blocks argument --------------------------------------------------------------------

#[test]
fn a_blank_line_in_a_blocks_argument_makes_two_paragraphs() {
    let laid = laid("\\blk{a\n\nb}");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    let blk = find(laid.root(), "blk");
    assert_eq!(shape(&content(blk)), ["P", "P"]);
    assert_eq!(text_of(content(blk)[0]), "a");
    assert_eq!(text_of(content(blk)[1]), "b");
}

#[test]
fn one_run_in_a_blocks_argument_is_one_paragraph() {
    let laid = laid("\\blk{a  b}");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    assert_eq!(shape(&content(find(laid.root(), "blk"))), ["P"]);
}

#[test]
fn an_environment_argument_declared_blocks_holds_paragraphs() {
    let laid = laid("\\begin{boxed}{a\n\nb}body\\end{boxed}");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    assert_eq!(shape(&content(find(laid.root(), "boxed"))), ["P", "P"]);
}

#[test]
fn an_empty_blocks_argument_and_one_holding_whitespace_stay_empty() {
    for text in ["\\blk{}", "\\blk{ }", "\\blk{\n\n}"] {
        let laid = laid(text);
        assert_eq!(laid.identifiers(), Vec::<&str>::new(), "{text:?}");
        assert_eq!(shape(&content(find(laid.root(), "blk"))), Vec::<String>::new(), "{text:?}");
        assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new(), "{text:?}");
    }
}

#[test]
fn a_blocks_argument_read_in_mathematics_is_carried_over_untouched() {
    // `\blk` inherits the mode it is written in: inside a formula its content
    // is mathematics, an opaque region whatever the declared layout.
    let laid = laid("\\(\\blk{a\n\nb}\\)");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    let blk = find(laid.root(), "blk");
    assert!(content(blk).iter().all(|node| !is_paragraph(node)));
    assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new());
}

// --- 3. an inferred argument -----------------------------------------------------------------

#[test]
fn an_inferred_argument_holding_one_run_is_inline_content() {
    let laid = laid("\\auto{a  b}");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    let auto = find(laid.root(), "auto");
    assert_eq!(shape(&content(auto)), ["text"]);
    assert_eq!(text_of(content(auto)[0]), "a b");
}

#[test]
fn an_inferred_argument_holding_a_blank_line_holds_paragraphs() {
    let laid = laid("\\auto{a\n\nb}");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    assert_eq!(shape(&content(find(laid.root(), "auto"))), ["P", "P"]);
}

#[test]
fn an_inferred_argument_holding_a_list_is_a_blocks_region() {
    let mixed = laid("\\auto{x \\begin{itemize}y\\end{itemize} z}");
    assert_eq!(mixed.identifiers(), Vec::<&str>::new());
    assert_eq!(shape(&content(find(mixed.root(), "auto"))), ["P", "a.itemize", "P"]);
    let alone = laid("\\auto{\\begin{itemize}y\\end{itemize}}");
    assert_eq!(shape(&content(find(alone.root(), "auto"))), ["a.itemize"]);
}

#[test]
fn an_inferred_argument_holding_a_display_equation_is_inline_content() {
    // A display equation is inline content, as it is in a body: it sits inside
    // a paragraph. It does not make the argument a region of blocks.
    let laid = laid("\\auto{a \\[x\\] b}");
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    let run = content(find(laid.root(), "auto"));
    assert!(run.iter().all(|node| !is_paragraph(node)), "{}", laid.dump());
    assert!(run.iter().any(|node| node.group_type().is_some_and(|t| matches!(t, FlmGroupType::Math(_)))));
}

// --- the predicate: the same answer before and after the pass --------------------------------

#[test]
fn argument_holds_blocks_answers_the_same_before_and_after_the_pass() {
    let lang = lang();
    let cases = [
        ("\\inl{a}", false),
        ("\\inl{a\n\nb}", false),
        ("\\inl{\\begin{itemize}y\\end{itemize}}", false),
        ("\\blk{}", true),
        ("\\blk{a}", true),
        ("\\blk{a\n\nb}", true),
        ("\\auto{}", false),
        ("\\auto{a  b}", false),
        ("\\auto{a \\[x\\] b}", false),
        ("\\auto{a\n\nb}", true),
        ("\\auto{\\begin{itemize}y\\end{itemize}}", true),
        ("\\auto{x \\begin{itemize}y\\end{itemize} z}", true),
        ("\\(\\blk{a}\\)", false),
        ("\\(\\auto{a\n\nb}\\)", false),
        ("\\begin{boxed}{t}x\\end{boxed}", true),
    ];
    for (text, expected) in cases {
        let fragment = common::parse(&lang, text);
        let before = common::callables(&fragment)[0];
        assert_eq!(argument_holds_blocks(&before, 0), expected, "before the pass: {text:?}");
        let laid = run(&lang, text, FragmentLayout::Blocks);
        let after = laid.root().descendants().find(|n| n.is_callable()).unwrap();
        assert_eq!(after.name(), before.name());
        assert_eq!(argument_holds_blocks(&after, 0), expected, "after the pass: {text:?}");
    }
}

/// An inferred argument holding no content — blank lines and whitespace — is
/// laid out as an empty region with no report, and the predicate
/// answers `false` before the pass and after it.
#[test]
fn an_inferred_argument_holding_no_content_holds_no_blocks_before_or_after_the_pass() {
    let lang = lang();
    for text in ["\\auto{ \n\n }", "\\auto{\n\n}", "\\auto{ \n\n \n\n }"] {
        let fragment = common::parse(&lang, text);
        assert!(!argument_holds_blocks(&common::callables(&fragment)[0], 0), "before: {text:?}");
        let laid = run(&lang, text, FragmentLayout::Blocks);
        assert_eq!(laid.identifiers(), Vec::<&str>::new(), "{text:?}");
        let auto = find(laid.root(), "auto");
        assert!(!content(auto).iter().any(is_paragraph), "{text:?}");
        assert!(!argument_holds_blocks(&auto, 0), "after: {text:?}");
        assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new(), "{text:?}");
    }
}

#[test]
fn argument_holds_blocks_is_false_past_the_arguments_and_for_an_omitted_one() {
    let lang = toy_lang_with(vec![common::macro_with(
        "b",
        "opt",
        vec![ArgDef::optional("head").blocks(), ArgDef::mandatory("text")],
    )]);
    let fragment = common::parse(&lang, "\\opt{x}");
    let node = common::callables(&fragment)[0];
    assert!(!argument_holds_blocks(&node, 0), "an omitted optional argument holds nothing");
    assert!(!argument_holds_blocks(&node, 1));
    assert!(!argument_holds_blocks(&node, 2));
    let fragment = common::parse(&lang, "\\opt[h]{x}");
    assert!(argument_holds_blocks(&common::callables(&fragment)[0], 0));
}

/// A region's content and whether it is absent: an omitted optional argument
/// is absent and has empty content; an argument written empty is not absent;
/// a position past the arguments has no content at all.
#[test]
fn a_region_left_out_is_absent_and_its_content_is_empty() {
    use flm_core::core::lowering::NodeRegion;
    let lang = toy_lang_with(vec![common::macro_with(
        "b",
        "opt",
        vec![ArgDef::optional("head"), ArgDef::mandatory("text")],
    )]);
    let fragment = common::parse(&lang, "\\opt{x}");
    let node = common::callables(&fragment)[0];
    assert!(NodeRegion::Argument(0).is_absent(&node));
    assert_eq!(NodeRegion::Argument(0).content_nodes(&node).map(|nodes| nodes.len()), Some(0));
    assert!(!NodeRegion::Argument(1).is_absent(&node));
    assert_eq!(NodeRegion::Argument(1).content_nodes(&node).map(|nodes| nodes.len()), Some(1));
    assert!(!NodeRegion::Argument(2).is_absent(&node));
    assert!(NodeRegion::Argument(2).content_nodes(&node).is_none());
    assert!(!NodeRegion::Slot(0).is_absent(&node));
    let fragment = common::parse(&lang, "\\opt[]{x}");
    let node = common::callables(&fragment)[0];
    assert!(!NodeRegion::Argument(0).is_absent(&node), "written empty");
    assert_eq!(NodeRegion::Argument(0).content_nodes(&node).map(|nodes| nodes.len()), Some(0));
}

/// A slot takes the layout of its element's body declaration, before the pass
/// and after it; a node with no slot at the position answers `false`.
#[test]
fn slot_holds_blocks_answers_the_body_declaration_before_and_after_the_pass() {
    let mut declarations = declarations("a");
    declarations.push(common::env_def("a", "inline", BodyDef::new().layout(flm_core::lang::mint::BodyLayout::Inline)));
    let lang = toy_lang_with(declarations);
    let cases = [
        ("\\begin{boxed}{t}x\\end{boxed}", 0, true),
        ("\\begin{itemize}x\\end{itemize}", 0, true),
        ("\\begin{inline}x\\end{inline}", 0, false),
        ("\\begin{boxed}{t}x\\end{boxed}", 1, false),
        ("\\inl{a}", 0, false),
    ];
    for (text, index, expected) in cases {
        let fragment = common::parse(&lang, text);
        let before = common::callables(&fragment)[0];
        assert_eq!(slot_holds_blocks(&before, index), expected, "before the pass: {text:?}");
        let laid = run(&lang, text, FragmentLayout::Blocks);
        let after = laid.root().descendants().find(|n| n.is_callable()).unwrap();
        assert_eq!(slot_holds_blocks(&after, index), expected, "after the pass: {text:?}");
    }
}

/// A block role read from the predicate: a block when the node's argument
/// holds blocks, inline content otherwise — the shape of an element whose own
/// role follows its argument.
struct RoleFromArgument;

impl BlockRoleAspect for RoleFromArgument {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        if argument_holds_blocks(node, 0) { BlockRole::Block } else { BlockRole::DEFAULT }
    }
}

#[test]
fn an_element_whose_role_follows_its_argument_splits_the_paragraph_only_when_it_holds_blocks() {
    let element = Arc::new(
        ElementSpec::new("b.aside")
            .with::<dyn BlockRoleAspect>(Arc::new(RoleFromArgument)),
    );
    let aside = ElementDef::new(
        "b",
        ["aside"],
        SyntaxDef::Macro {
            args: vec![ArgDef::mandatory("text").layout(ArgLayout::InlineOrBlocks)],
        },
        element,
    );
    let lang = toy_lang_with(vec![aside]);
    let one_line = run(&lang, "a \\aside{x} b", FragmentLayout::Blocks);
    assert_eq!(shape(&one_line.children()), ["P"]);
    let blocks = run(&lang, "a \\aside{x\n\ny} b", FragmentLayout::Blocks);
    assert_eq!(shape(&blocks.children()), ["P", "b.aside", "P"]);
    assert_eq!(kinds(&check_block_contract(&blocks.tree)), Vec::<&str>::new(), "{}", blocks.dump());
    // An argument holding blank lines and nothing else is no block, before the
    // pass or after it, so the paragraph is not split and the tree keeps the
    // contract.
    for text in ["a \\aside{ \n\n } b", "a \\aside{\n\n} b"] {
        let empty = run(&lang, text, FragmentLayout::Blocks);
        assert_eq!(shape(&empty.children()), ["P"], "{text:?}");
        assert_eq!(kinds(&check_block_contract(&empty.tree)), Vec::<&str>::new(), "{}", empty.dump());
    }
}

// --- 4. the checker holds each argument to its declaration -----------------------------------

#[test]
fn the_pass_output_over_the_three_layouts_passes_the_checker() {
    let text = "\\inl{a\n\nb} \\blk{a\n\nb} \\blk{c} \\auto{d} \\auto{e\n\nf} \
                \\auto{\\begin{itemize}y\\end{itemize}} \\begin{boxed}{t}b\\end{boxed}";
    let laid = laid(text);
    assert_eq!(kinds(&check_parse_contract(&laid.tree)), Vec::<&str>::new());
    assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new(), "{}", laid.dump());
}

/// A tree whose root holds one `\name{…}` over the source `\name{ab}`, its
/// argument's content being what `content` stages (with `a` at `name.len()+2`
/// and `b` right after it); the specifications are the language's own, taken
/// from a parse.
fn staged(
    lang: &BuiltLang,
    name: &str,
    content: impl FnOnce(&mut Site, usize) -> Vec<BuildId>,
) -> NodeTree<Flm> {
    let parsed = common::parse(lang, &format!("\\{name}{{x}}"));
    let node = common::callables(&parsed).into_iter().next().expect("the command parses");
    let spec = Arc::clone(node.spec().expect("a callable has a spec"));
    let argument_spec = Arc::clone(&node.arguments().unwrap().get(0).unwrap().spec);
    let text = format!("\\{name}{{ab}}");
    let open = name.len() + 1;
    let mut site = Site::new(lang, &text);
    let children = content(&mut site, open + 1);
    let count = u32::try_from(children.len()).expect("a few children");
    let group_kind = NodeKind::group(GroupData::new(FlmGroupType::Content, "{", "}"));
    let group = site.stage(group_kind, open..text.len(), children);
    let data = CallableData {
        callable_type: FlmCallableType::Macro,
        name: name.into(),
        spec,
        arguments: ParsedArguments::new(vec![ParsedArgument::provided(
            argument_spec,
            ChildRegion::new(0..1, ContentNodes::InChildrenOf(group, 0..count)),
            (),
        )]),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Specials,
    };
    let node = site.stage(NodeKind::callable(data), 0..text.len(), vec![group]);
    let root = site.stage(NodeKind::list(), 0..text.len(), vec![node]);
    site.finish(root)
}

/// A paragraph holding `a`, then the text `b` beside it.
fn paragraph_beside_text(site: &mut Site, at: usize) -> Vec<BuildId> {
    let a = site.text(at..at + 1);
    let paragraph = site.paragraph(at..at + 1, vec![a]);
    let b = site.text(at + 1..at + 2);
    vec![paragraph, b]
}

#[test]
fn a_paragraph_in_an_inline_argument_is_a_violation() {
    let lang = lang();
    let tree = staged(&lang, "inl", |site, at| {
        let text = site.text(at..at + 2);
        vec![site.paragraph(at..at + 2, vec![text])]
    });
    assert_eq!(kinds(&check_block_contract(&tree)), ["ParagraphInInlineContainer"]);
}

#[test]
fn an_inline_run_beside_a_paragraph_in_a_blocks_argument_is_a_violation() {
    let lang = lang();
    let tree = staged(&lang, "blk", paragraph_beside_text);
    assert_eq!(kinds(&check_block_contract(&tree)), ["InlineInBlocksRegion"]);
}

#[test]
fn an_inline_run_in_a_blocks_argument_is_a_violation_before_the_pass() {
    let lang = lang();
    let fragment = common::parse(&lang, "\\blk{a}");
    assert_eq!(kinds(&check_block_contract(fragment.tree())), ["InlineInBlocksRegion"]);
}

#[test]
fn an_inline_run_beside_a_paragraph_in_an_inferred_argument_is_a_violation() {
    let lang = lang();
    let tree = staged(&lang, "auto", paragraph_beside_text);
    assert_eq!(kinds(&check_block_contract(&tree)), ["InlineInBlocksRegion"]);
    // Inline content alone is an inline region: no violation.
    let tree = staged(&lang, "auto", |site, at| vec![site.text(at..at + 2)]);
    assert_eq!(kinds(&check_block_contract(&tree)), Vec::<&str>::new());
}

// --- 5. the build error ----------------------------------------------------------------------

#[test]
fn a_layout_on_an_opaque_wrapped_argument_is_a_build_error() {
    let key = ArgDef::mandatory("key").with_opaque_wrapper().layout(ArgLayout::InlineOrBlocks);
    let errors = mint_errors(vec![common::macro_with("t", "lbl", vec![ArgDef::new(ArgStdKind::Star), key])]);
    assert_eq!(
        errors,
        [MintError::LayoutOnOpaqueWrappedArgument {
            owner: "t".into(),
            callable: "t.lbl".into(),
            index: 1,
            field: None,
        }]
    );
    let star = ArgDef::new(ArgStdKind::Star).with_opaque_wrapper().blocks();
    assert_eq!(mint_errors(vec![common::macro_with("t", "s", vec![star])]).len(), 1);
}

#[test]
fn blocks_on_an_argument_read_in_mathematics_is_a_build_error() {
    let math = ArgDef::mandatory("formula").mode(Mode::Math).blocks();
    let errors = mint_errors(vec![common::macro_with("t", "m", vec![math])]);
    assert_eq!(
        errors,
        [MintError::BlocksOnMathArgument {
            owner: "t".into(),
            callable: "t.m".into(),
            index: 0,
            field: None,
        }]
    );
}

#[test]
fn a_refused_layout_is_minted_inline() {
    let math = ArgDef::mandatory("formula").mode(Mode::Math).blocks();
    let mut inputs = LangInputs::new();
    inputs.elements = vec![common::macro_with("t", "m", vec![math])];
    let lang = build_language(inputs);
    assert!(!lang.is_sound());
    let fragment = common::parse(&lang, "\\m{x}");
    let node = common::callables(&fragment).into_iter().next().unwrap();
    assert_eq!(arg_layout_of(&node, 0), Some(ArgLayout::Inline));
}

// --- arg_layout_of ---------------------------------------------------------------------------

#[test]
fn arg_layout_of_answers_each_declared_argument_and_nothing_past_them() {
    let lang = lang();
    let fragment = common::parse(&lang, "\\inl{a}\\blk{b}\\auto{c}\\emph{d}\\begin{boxed}{t}x\\end{boxed}");
    let nodes = common::callables(&fragment);
    let layouts: Vec<Option<ArgLayout>> = nodes.iter().map(|node| arg_layout_of(node, 0)).collect();
    assert_eq!(
        layouts,
        [
            Some(ArgLayout::Inline),
            Some(ArgLayout::Blocks),
            Some(ArgLayout::InlineOrBlocks),
            Some(ArgLayout::Inline),
            Some(ArgLayout::Blocks),
        ]
    );
    assert_eq!(arg_layout_of(&nodes[0], 1), None);
    assert_eq!(arg_layout_of(&fragment.tree().root(), 0), None);
}

// --- 6. the layouts ride a stored and re-read tree -------------------------------------------

#[test]
fn the_layouts_ride_a_stored_and_reread_tree() {
    let defs: Vec<ElementDef> = toy_elements().into_iter().chain(declarations("a")).collect();
    let lang = toy_lang_with(declarations("a"));
    let text = "\\inl{a}\\blk{b}\\auto{c}\\begin{boxed}{t}x\\end{boxed}";
    let expected = [ArgLayout::Inline, ArgLayout::Blocks, ArgLayout::InlineOrBlocks, ArgLayout::Blocks];
    let layouts = |tree: &NodeTree<Flm>| -> Vec<ArgLayout> {
        tree.root()
            .descendants()
            .filter(|node| node.is_callable())
            .map(|node| arg_layout_of(&node, 0).expect("each has one argument"))
            .collect()
    };
    // The parsed fragment, stored and read back.
    let fragment: Arc<Fragment> = Arc::new(common::parse(&lang, text));
    let read = common::dump::round_trip(&lang, &fragment, common::dump::elements_of(&defs));
    assert_eq!(layouts(read.tree()), expected);
    // The structured tree, stored and read back.
    let laid = run(&lang, text, FragmentLayout::Blocks);
    let (position, segment) = common::dump::write_tree(&lang, &laid.tree);
    let tree = common::dump::read_tree(
        &lang,
        segment,
        position,
        common::dump::elements_of(&defs),
        Default::default(),
    );
    assert_eq!(layouts(&tree), expected);
    assert_eq!(display_tree(tree.root()), laid.dump());
}

// --- no document input panics or loops -------------------------------------------------------

#[test]
fn a_blocks_argument_nested_ten_deep_is_laid_out() {
    let mut text = String::from("x");
    for depth in 0..10 {
        text = if depth % 2 == 0 { format!("\\blk{{{text}\n\ny}}") } else { format!("\\auto{{{text}}}") };
    }
    let laid = laid(&text);
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new(), "{}", laid.dump());
}

#[test]
fn a_blank_line_in_a_group_inside_an_inline_argument_is_reported_for_the_group() {
    let laid = laid("\\inl{{a\n\nb}}");
    let contexts: Vec<&str> =
        laid.diags.conditions::<ParbreakInInline>().map(|c| c.context.as_str()).collect();
    assert_eq!(contexts, ["group"]);
    assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new());
}

#[test]
fn a_blank_line_in_a_group_inside_a_blocks_argument_is_reported_for_the_group() {
    let laid = laid("\\blk{a {b\n\nc}}");
    let contexts: Vec<&str> =
        laid.diags.conditions::<ParbreakInInline>().map(|c| c.context.as_str()).collect();
    assert_eq!(contexts, ["group"]);
    assert_eq!(shape(&content(find(laid.root(), "blk"))), ["P"]);
}

// --- the execution settings ------------------------------------------------------------------

/// The test package as a feature, so that a run's frozen trees can be compared.
struct ArgLayouts;

impl FlmFeature for ArgLayouts {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.args", "Argument layouts")
            .docs("One command per argument layout, for the argument-layout tests.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        for def in toy_elements().into_iter().chain(declarations("a")) {
            b.element(def)?;
        }
        Ok(())
    }
}

#[test]
fn the_execution_settings_give_one_tree() {
    let features: Vec<Arc<dyn FlmFeature>> = vec![Arc::new(Blocks::default()), Arc::new(ArgLayouts)];
    let registry = common::pipeline::registry(features);
    let texts = [
        "\\blk{a\n\nb} \\auto{c} \\auto{d\n\ne}",
        "\\inl{x} \\begin{boxed}{t\n\nu}v\\end{boxed}",
        "\\auto{\\begin{itemize}y\\end{itemize}}",
    ];
    let doc = common::wiring::doc(&registry, FragmentLayout::Blocks, &texts);
    let plan = common::pipeline::plan(&registry, &[STRUCTURED.name()]);
    let dumps = |options: &RunOptions| -> Vec<String> {
        let run = common::pipeline::run(&doc, &plan, options);
        (0..texts.len())
            .map(|i| {
                let tree = run
                    .frozen_tree(flm_core::model::addr::TreeId::tree(i as u32))
                    .expect("each tree is frozen");
                display_tree(tree.root())
            })
            .collect()
    };
    let sequential = dumps(&RunOptions::sequential());
    assert_eq!(dumps(&RunOptions::new()), sequential);
    assert!(sequential[0].contains("Paragraph"), "{}", sequential[0]);
}

// --- 5 and 7. scoped packages on an argument -------------------------------------------------

/// A scoped package `name` holding one macro `\tag{…}` of the owner `s`.
fn tag_package(name: &str) -> ScopedPackageDef {
    ScopedPackageDef {
        owner: "s".into(),
        name: name.into(),
        elements: vec![common::macro_def("s", "tag", 1)],
        docs: String::new(),
    }
}

/// `\note[…]{…}`, whose optional argument names the scoped package `package`.
fn note(package: &str) -> ElementDef {
    common::macro_with(
        "s",
        "note",
        vec![ArgDef::optional("tags").scoped(package), ArgDef::mandatory("text")],
    )
}

/// The language over `elements` and `scoped`, sound or not.
fn language(elements: Vec<ElementDef>, scoped: Vec<ScopedPackageDef>) -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.elements = elements;
    inputs.scoped_packages = scoped;
    build_language(inputs)
}

fn mint_errors_of(lang: &BuiltLang) -> Vec<MintError> {
    lang.errors()
        .iter()
        .filter_map(|error| match error {
            LangBuildError::Mint(error) => Some(error.clone()),
            _ => None,
        })
        .collect()
}

/// The element names of the callables of `text` parsed under `lang`, and the
/// identifiers of the parse's diagnostics.
fn parsed(lang: &BuiltLang, text: &str) -> (Vec<String>, Vec<String>) {
    let fragment = common::parse(lang, text);
    let elements = common::callables(&fragment)
        .iter()
        .map(|node| flm_core::model::element::element_of(node).name.to_string())
        .collect();
    let diags = fragment.diags().iter().map(|d| d.identifier().to_string()).collect();
    (elements, diags)
}

#[test]
fn a_scoped_macro_parses_inside_the_argument_naming_it_and_is_unknown_outside_it() {
    let lang = language(vec![note("s.tags")], vec![tag_package("s.tags")]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let (elements, diags) = parsed(&lang, "\\note[\\tag{a}]{text}");
    assert_eq!(elements, ["s.note", "s.tag"]);
    assert!(diags.is_empty(), "{diags:?}");
    // In the other argument, and in running text, `\tag` is not defined.
    for text in ["\\note{\\tag{a}}", "\\tag{a}", "\\note[x]{y} \\tag{a}"] {
        let (elements, diags) = parsed(&lang, text);
        assert!(elements.contains(&"flm.unknown.macro".to_string()), "{text:?}: {elements:?}");
        assert!(!elements.contains(&"s.tag".to_string()), "{text:?}: {elements:?}");
        assert!(!diags.is_empty(), "{text:?}");
    }
}

#[test]
fn a_scoped_macro_parses_inside_an_environment_argument_naming_it_and_not_in_the_body() {
    let tagged = ElementDef::new(
        "s",
        ["tagged"],
        SyntaxDef::Environment {
            args: vec![ArgDef::optional("tags").scoped("s.tags")],
            body: BodyDef::new(),
        },
        common::element("s.tagged"),
    );
    let lang = language(vec![tagged], vec![tag_package("s.tags")]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let (elements, diags) = parsed(&lang, "\\begin{tagged}[\\tag{a}]x\\end{tagged}");
    assert_eq!(elements, ["s.tagged", "s.tag"]);
    assert!(diags.is_empty(), "{diags:?}");
    let (elements, _) = parsed(&lang, "\\begin{tagged}[x]\\tag{a}\\end{tagged}");
    assert!(elements.contains(&"flm.unknown.macro".to_string()), "{elements:?}");
    assert!(!elements.contains(&"s.tag".to_string()), "{elements:?}");
}

#[test]
fn a_scoped_package_named_by_an_argument_is_found_whatever_the_declaration_order() {
    // The package is declared by a feature registered after the one naming it.
    let mut inputs = LangInputs::new();
    inputs.elements = vec![note("t.tags"), common::macro_def("t", "other", 0)];
    inputs.scoped_packages = vec![ScopedPackageDef {
        owner: "z".into(),
        ..tag_package("t.tags")
    }];
    let lang = build_language(inputs);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    assert_eq!(parsed(&lang, "\\note[\\tag{a}]{b}").0, ["s.note", "s.tag"]);
}

#[test]
fn an_argument_of_a_scoped_macro_may_name_another_scoped_package() {
    // `\note` lives in the package `outer`, reached through `\begin{box}`, and
    // its argument names `inner`: `inner` is minted first.
    let boxed = common::env_def("s", "box", BodyDef::new().scoped("outer"));
    let outer = ScopedPackageDef {
        owner: "s".into(),
        name: "outer".into(),
        elements: vec![note("inner")],
        docs: String::new(),
    };
    let lang = language(vec![boxed], vec![outer, tag_package("inner")]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let (elements, diags) = parsed(&lang, "\\begin{box}\\note[\\tag{a}]{b}\\end{box}");
    assert_eq!(elements, ["s.box", "s.note", "s.tag"]);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn an_unknown_package_named_by_an_argument_is_the_existing_build_error() {
    let lang = language(vec![note("nope")], Vec::new());
    assert_eq!(
        mint_errors_of(&lang),
        [MintError::UnknownScopedPackage {
            element: "s.note".into(),
            owner: "s".into(),
            argument: Some(0),
            field: None,
            package: "nope".into(),
        }]
    );
    assert!(mint_errors_of(&lang)[0].to_string().contains("argument #1 of `s.note`"));
    // The language is still built: `\note` parses, its argument without the package.
    assert_eq!(parsed(&lang, "\\note[x]{y}").0, ["s.note"]);
}

#[test]
fn a_cycle_through_an_argument_is_the_existing_build_error() {
    let a = ScopedPackageDef {
        owner: "s".into(),
        name: "a".into(),
        elements: vec![common::macro_with("s", "ma", vec![ArgDef::mandatory("x").scoped("b")])],
        docs: String::new(),
    };
    let b = ScopedPackageDef {
        owner: "s".into(),
        name: "b".into(),
        elements: vec![common::env_def("s", "eb", BodyDef::new().scoped("a"))],
        docs: String::new(),
    };
    let lang = language(Vec::new(), vec![a, b]);
    let errors = mint_errors_of(&lang);
    assert_eq!(errors, [MintError::ScopedPackageCycle { packages: vec!["a".into(), "b".into()] }]);
}

#[test]
fn a_scoped_package_on_a_key_value_argument_is_attached() {
    let options = ArgDef::new(ArgStdKind::KeyVals).scoped("s.tags");
    let lang = language(vec![common::macro_with("s", "opts", vec![options])], vec![tag_package("s.tags")]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let (elements, diags) = parsed(&lang, "\\opts[a=\\tag{b}]");
    assert_eq!(elements, ["s.opts", "s.tag"]);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn a_scoped_argument_combines_with_a_mode_and_a_layout() {
    // The package is pushed with the argument's own mode change, and the
    // argument is laid out as blocks.
    let mut lang_elements = toy_elements();
    lang_elements.push(common::macro_with(
        "s",
        "boxnote",
        vec![ArgDef::mandatory("text").mode(Mode::Text).blocks().scoped("s.tags")],
    ));
    let lang = language(lang_elements, vec![tag_package("s.tags")]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let laid = run(&lang, "\\boxnote{\\tag{a}\n\nb}", FragmentLayout::Blocks);
    assert_eq!(laid.identifiers(), Vec::<&str>::new());
    let boxnote = find(laid.root(), "boxnote");
    assert_eq!(shape(&content(boxnote)), ["P", "P"]);
    assert!(boxnote.descendants().any(|n| n.name() == Some("tag")));
}

// --- a tack-on field's own arguments -----------------------------------------------------------

/// `\fig{…}` accepting the tack-on field `\caption{…}`, whose own argument is
/// `caption_arg`.
fn fig(caption_arg: ArgDef) -> ElementDef {
    let caption = TackOnField::new("caption", vec![caption_arg]);
    common::macro_with(
        "s",
        "fig",
        vec![ArgDef::mandatory("body"), ArgDef::new(ArgStdKind::TackOn { fields: vec![caption] })],
    )
}

#[test]
fn a_tack_on_field_argument_takes_its_declared_layout_and_scoped_package() {
    let mut elements = toy_elements();
    elements.push(fig(ArgDef::mandatory("text").blocks().scoped("s.tags")));
    let lang = language(elements, vec![tag_package("s.tags")]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let laid = run(&lang, "\\fig{x}\\caption{\\tag{a} b\n\nc}", FragmentLayout::Blocks);
    assert_eq!(laid.identifiers(), Vec::<&str>::new(), "{}", laid.dump());
    let caption = find(laid.root(), "caption");
    assert_eq!(arg_layout_of(&caption, 0), Some(ArgLayout::Blocks));
    assert!(argument_holds_blocks(&caption, 0));
    assert_eq!(shape(&content(caption)), ["P", "P"]);
    assert!(caption.descendants().any(|n| n.name() == Some("tag")));
    assert_eq!(kinds(&check_block_contract(&laid.tree)), Vec::<&str>::new(), "{}", laid.dump());
    // The package is the field's argument's alone: not the element's own argument.
    let (elements, _) = parsed(&lang, "\\fig{\\tag{a}}\\caption{b}");
    assert!(!elements.contains(&"s.tag".to_string()), "{elements:?}");
}

#[test]
fn the_build_errors_of_a_tack_on_field_argument_name_the_field() {
    let key = ArgDef::mandatory("key").with_opaque_wrapper().blocks();
    assert_eq!(
        mint_errors(vec![fig(key)]),
        [MintError::LayoutOnOpaqueWrappedArgument {
            owner: "s".into(),
            callable: "s.fig".into(),
            index: 0,
            field: Some("caption".into()),
        }]
    );
    let lang = language(vec![fig(ArgDef::mandatory("text").scoped("nope"))], Vec::new());
    let errors = mint_errors_of(&lang);
    assert_eq!(
        errors,
        [MintError::UnknownScopedPackage {
            element: "s.fig".into(),
            owner: "s".into(),
            argument: Some(0),
            field: Some("caption".into()),
            package: "nope".into(),
        }]
    );
    assert!(
        errors[0].to_string().contains("argument #1 of the field `caption` of `s.fig`"),
        "{}",
        errors[0]
    );
}

// --- a scoped package another feature declares -----------------------------------------------

/// A feature declaring `\\note[…]{…}`, whose optional argument names the
/// scoped package `t.label` of another feature — peer-declared when `peer`.
struct Notes {
    peer: bool,
}

impl FlmFeature for Notes {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.notes", "Notes").docs("A command naming another feature's package.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        if self.peer {
            b.peer_scoped_package("t.label", "the `\\tag` a note's optional argument accepts")?;
        }
        b.element(common::macro_with(
            "t.notes",
            "note",
            vec![ArgDef::optional("tags").scoped("t.label"), ArgDef::mandatory("text")],
        ))
    }
}

/// A feature declaring the scoped package `t.label`, holding `\\tag{…}`.
struct Labels;

impl FlmFeature for Labels {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.labels", "Labels").docs("The package `t.label`.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.scoped_package(ScopedPackageDef {
            owner: "t.labels".into(),
            name: "t.label".into(),
            elements: vec![common::macro_def("t.labels", "tag", 1)],
            docs: String::new(),
        })
    }
}

#[test]
fn a_peer_declared_scoped_package_nobody_declares_is_one_note_and_makes_nothing_visible() {
    let registry = common::pipeline::registry(vec![Arc::new(Notes { peer: true })]);
    let diags: Vec<(String, String)> = registry
        .build_diagnostics()
        .iter()
        .map(|d| (d.identifier().to_string(), d.message()))
        .collect();
    assert_eq!(
        diags,
        [(
            "flm.registry.unresolved-peer".to_string(),
            "feature `t.notes` refers to the scoped package `t.label` and expects another feature to declare it, but no feature of this registry declares it; the environment bodies and arguments that refer to it get no \
             definitions from it"
                .to_string()
        )]
    );
    let peers: Vec<(&str, &str, bool)> = registry
        .peer_scoped_packages()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(peers, [("t.notes", "t.label", false)]);
    assert!(registry.lang().is_sound(), "{:?}", registry.lang().errors());
    // The optional argument accepts nothing: `\\tag` is an unknown command in it.
    let (elements, diags) = parsed(registry.lang(), "\\note[\\tag{a}]{b}");
    assert_eq!(elements, ["t.notes.note", "flm.unknown.macro"]);
    assert!(!diags.is_empty());
}

#[test]
fn a_peer_declared_scoped_package_another_feature_declares_is_visible_in_the_argument() {
    for features in [
        vec![Arc::new(Notes { peer: true }) as Arc<dyn FlmFeature>, Arc::new(Labels)],
        vec![Arc::new(Labels) as Arc<dyn FlmFeature>, Arc::new(Notes { peer: true })],
    ] {
        let registry = common::pipeline::registry(features);
        assert!(registry.build_diagnostics().is_empty());
        assert!(registry.peer_scoped_packages().iter().all(|entry| entry.resolved));
        let (elements, diags) = parsed(registry.lang(), "\\note[\\tag{a}]{b}");
        assert_eq!(elements, ["t.notes.note", "t.labels.tag"]);
        assert!(diags.is_empty(), "{diags:?}");
        let (elements, _) = parsed(registry.lang(), "\\tag{a}");
        assert_eq!(elements, ["flm.unknown.macro"]);
    }
}

#[test]
fn a_scoped_package_nobody_declares_and_not_peer_declared_fails_the_build() {
    let error = common::pipeline::registry_error(vec![Arc::new(Notes { peer: false })]);
    assert_eq!(error.phase, flm_core::core::registry::BuildPhase::Language);
    assert!(
        matches!(
            error.errors.as_slice(),
            [BuildError::Lang(LangBuildError::Mint(MintError::UnknownScopedPackage { .. }))]
        ),
        "{}",
        error.render()
    );
}

/// Two peer declarations of one package by one feature.
struct TwicePeer;

impl FlmFeature for TwicePeer {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.twice", "Twice").docs("Peer-declares one package twice.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.peer_scoped_package("t.label", "once")?;
        b.peer_scoped_package("t.label", "twice")
    }
}

#[test]
fn a_feature_peer_declaring_one_package_twice_declares_it_once() {
    // A repeated peer declaration is merged into the first: one listed
    // declaration, its docs the first's.
    let registry = common::pipeline::registry(vec![Arc::new(TwicePeer)]);
    let declared: Vec<(&str, &str)> = registry
        .peer_scoped_packages()
        .iter()
        .map(|entry| (entry.name.as_str(), entry.docs.as_str()))
        .collect();
    let once = if cfg!(feature = "docs") { "once" } else { "" };
    assert_eq!(declared, [("t.label", once)]);
}

// --- the region a node stands in -------------------------------------------------------------

/// Whether each `\section` of `text`, laid out under `layout`, stands in a
/// region laid out as blocks, in document order — asked before the pass and
/// after it, which must agree.
fn sections_stand_in_blocks(text: &str, layout: FragmentLayout) -> Vec<bool> {
    let lang = lang();
    let answers = |root: NodeRef<'_, Flm>| {
        root.descendants()
            .filter(|node| node.is_callable() && node.name() == Some("section"))
            .map(|node| stands_in_blocks(&node, layout))
            .collect::<Vec<bool>>()
    };
    let fragment = common::parse(&lang, text);
    let before = answers(fragment.tree().root());
    let laid = run(&lang, text, layout);
    let after = answers(laid.root());
    assert_eq!(before, after, "before and after the pass: {text:?}");
    after
}

#[test]
fn a_node_in_an_argument_stands_where_the_argument_is_laid_out() {
    assert_eq!(sections_stand_in_blocks("\\inl{\\section{a}}", FragmentLayout::Blocks), [false]);
    assert_eq!(sections_stand_in_blocks("\\blk{\\section{a}}", FragmentLayout::Blocks), [true]);
    assert_eq!(sections_stand_in_blocks("\\auto{x \\section{a} y}", FragmentLayout::Blocks), [true]);
    // The argument's layout decides, whatever the fragment's.
    assert_eq!(sections_stand_in_blocks("\\blk{\\section{a}}", FragmentLayout::Inline), [true]);
}

#[test]
fn a_node_in_an_environment_body_stands_where_the_body_is_laid_out() {
    let text = "\\begin{quotation}\\section{a}\\end{quotation}";
    assert_eq!(sections_stand_in_blocks(text, FragmentLayout::Blocks), [true]);
    let text = "\\begin{inlineenv}\\section{a}\\end{inlineenv}";
    assert_eq!(sections_stand_in_blocks(text, FragmentLayout::Blocks), [false]);
}

#[test]
fn a_node_in_a_plain_group_stands_in_an_inline_region() {
    assert_eq!(sections_stand_in_blocks("x {\\section{a}} y", FragmentLayout::Blocks), [false]);
}

#[test]
fn a_node_at_the_top_level_stands_where_the_fragment_s_layout_says() {
    let alone = "\\section{a}";
    assert_eq!(sections_stand_in_blocks(alone, FragmentLayout::Blocks), [true]);
    assert_eq!(sections_stand_in_blocks(alone, FragmentLayout::Inline), [false]);
    assert_eq!(sections_stand_in_blocks(alone, FragmentLayout::InlineOrBlocks), [true]);
    let among = "x \\section{a} y";
    assert_eq!(sections_stand_in_blocks(among, FragmentLayout::Blocks), [true]);
    assert_eq!(sections_stand_in_blocks(among, FragmentLayout::Inline), [false]);
    assert_eq!(sections_stand_in_blocks(among, FragmentLayout::InlineOrBlocks), [true]);
}

#[test]
fn a_node_in_a_list_inside_a_region_stands_where_the_list_s_body_is_laid_out() {
    // The body of `itemize` is blocks, even where the list stands in an
    // inline argument; the list itself stands in that argument.
    let text = "\\inl{\\begin{itemize}\\section{a}\\end{itemize}}";
    assert_eq!(sections_stand_in_blocks(text, FragmentLayout::Blocks), [true]);
    let laid = run(&lang(), text, FragmentLayout::Blocks);
    assert!(!stands_in_blocks(&find(laid.root(), "itemize"), FragmentLayout::Blocks));
}

