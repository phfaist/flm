//! The `BlockStructure` hook and the `BlockStructurer` facade of
//! `flm_core::lang::rewrites::blocks::hook`: an element lays out its own
//! content, synthesizes decorators, and reports through the block-structure
//! pass.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole, Placement};
use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::model::names::ElementName;
use flm_core::lang::mint::{BodyDef, ElementDef, FlmSynthesizedSpec, SyntaxDef};
use flm_core::lang::rewrites::blocks::{
    BlockStructure, BlockStructurer, BlocksConfig, DecoratorWithoutHome, RegionLayout, StructureCx,
    StructureError, StructureFailed, Structuring, check_block_contract, check_parse_contract,
    structure,
};
use flm_core::lang::{Flm, FlmCallableType, FlmDiagnostics, FlmOrigin};
use techy::core::node::{
    BodySlotExt, BuildId, ChildRegion, ContentNodes, NodeKind, NodeRef, ParsedSlot, ParsedSlots,
    SlotRole,
};
use techy::core::{Lang, ParsingState};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::latexlike::BodyMarker;
use techy::source::{SourceProvenance, SourceSpan};

use common::blocks::{
    element_with_role, env_of, is_paragraph, macro_of, run, text_of, toy_lang_with,
};

// --- the itemize hook -----------------------------------------------------------------

/// A list environment: block-level and carrier of the hook. It counts its
/// invocations so the tests can assert the pass reached it through the aspect.
struct ItemizeElement {
    invocations: AtomicUsize,
    /// The block-role element of the item containers the hook makes.
    item: Arc<ElementSpec>,
}

/// An item container: a callable of the block-role item element with one body
/// slot designating its children — the shape a real item element has. A blocks
/// region's children are paragraphs, block-role nodes and comments, so a
/// container a hook makes for blocks is itself a block-role node.
fn item_container(
    cx: &mut StructureCx<'_>,
    element: &Arc<ElementSpec>,
    span: SourceSpan<FlmOrigin>,
    state: Arc<ParsingState<Flm>>,
    children: Vec<BuildId>,
) -> Result<BuildId, StructureError> {
    let builder = cx.builder();
    let count = children.len() as u32;
    let list_kind = NodeKind::list();
    let list_ext =
        Flm::make_node_ext(&list_kind, &span, &state, builder.staged_children(&children))?;
    let list = builder.add(list_kind, span.clone(), Arc::clone(&state), children, list_ext, ())?;
    let spec = Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)));
    let mut data = spec.callable_data();
    data.slots = ParsedSlots::new(vec![ParsedSlot::new(
        ChildRegion::new(0..1, ContentNodes::InChildrenOf(list, 0..count)),
        "body",
        SlotRole::Content,
        BodyMarker::make_body(),
    )]);
    let kind = NodeKind::callable(data);
    let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[list]))?;
    Ok(builder.add(kind, span, state, vec![list], ext, ())?)
}

impl BlockRoleAspect for ItemizeElement {
    fn block_role(&self, _node: &NodeRef<'_, Flm>) -> BlockRole {
        BlockRole::Block
    }
}

impl BlockStructure for ItemizeElement {
    fn structure(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        bs: &mut dyn BlockStructurer,
    ) -> Result<Structuring, StructureError> {
        self.invocations.fetch_add(1, Ordering::Relaxed);
        // Split the body's content run at `\item` boundaries.
        let mut items: Vec<Vec<NodeRef<'_, Flm>>> = Vec::new();
        for child in bs.body_run(&node)? {
            if child.name() == Some("item") {
                items.push(Vec::new());
            } else if let Some(last) = items.last_mut() {
                last.push(child);
            }
        }
        // Each item's content is a blocks run, held by an item container.
        let mut containers: Vec<BuildId> = Vec::new();
        for item in &items {
            let children = bs.build_blocks(cx, item)?;
            let span = match (item.first(), item.last()) {
                (Some(first), Some(last)) => SourceSpan::new(
                    first.span().source(),
                    first.span().start()..last.span().end(),
                ),
                _ => node.span().clone(),
            };
            let state = Arc::clone(
                item.first()
                    .map_or(node.parsing_state(), |n| n.parsing_state()),
            );
            containers.push(item_container(cx, &self.item, span, state, children)?);
        }
        Ok(Structuring::Emit(vec![
            bs.with_body(cx, &node, containers)?,
        ]))
    }
}

fn itemize_lang() -> (flm_core::lang::BuiltLang, Arc<ItemizeElement>) {
    let item = element_with_role("t.listitem", BlockRole::Block);
    let itemize = Arc::new(ItemizeElement {
        invocations: AtomicUsize::new(0),
        item: Arc::clone(&item),
    });
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.itemize"))
            .with::<dyn BlockRoleAspect>(itemize.clone())
            .with::<dyn BlockStructure>(itemize.clone()),
    );
    let lang = toy_lang_with(vec![
        env_of("itemize", BodyDef::new(), element),
        macro_of("item", 0, common::element("t.item")),
        ElementDef::new("t", Vec::<&str>::new(), SyntaxDef::Synthesized, item),
    ]);
    (lang, itemize)
}

#[test]
fn the_hook_is_reached_through_the_aspect() {
    let (lang, itemize) = itemize_lang();
    let laid = run(
        &lang,
        "\\begin{itemize}\\item one\\end{itemize}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        itemize.invocations.load(Ordering::Relaxed),
        1,
        "{}",
        laid.dump()
    );
}

#[test]
fn the_hook_builds_blocks_per_item() {
    let (lang, _) = itemize_lang();
    let laid = run(
        &lang,
        "\\begin{itemize}\\item one\n\nstill one\\item two\\end{itemize}",
        FragmentLayout::Blocks,
    );
    let itemize = laid.children()[0];
    assert!(
        !is_paragraph(&itemize),
        "a block element is never wrapped in a paragraph"
    );
    let items: Vec<NodeRef<'_, Flm>> = itemize.body().unwrap().iter().collect();
    assert_eq!(items.len(), 2, "{}", laid.dump());
    let first: Vec<NodeRef<'_, Flm>> = items[0].body().unwrap().iter().collect();
    assert_eq!(first.len(), 2, "{}", laid.dump());
    assert!(first.iter().all(is_paragraph));
    assert_eq!(text_of(first[0]), "one");
    assert_eq!(text_of(first[1]), "still one");
    let second: Vec<NodeRef<'_, Flm>> = items[1].body().unwrap().iter().collect();
    assert_eq!(second.len(), 1, "{}", laid.dump());
    assert_eq!(text_of(second[0]), "two");
}

#[test]
fn the_hooks_item_containers_satisfy_the_parse_level_part() {
    let (lang, _) = itemize_lang();
    let laid = run(
        &lang,
        "\\begin{itemize}\\item one\n\nstill one\\item two\\end{itemize}",
        FragmentLayout::Blocks,
    );
    let violations = check_parse_contract(&laid.tree);
    assert!(violations.is_empty(), "{violations:?}\n{}", laid.dump());
}

#[test]
fn the_hooks_item_containers_satisfy_the_block_part() {
    // The item containers are block-role nodes holding blocks regions, as
    // invariant 3 asks of a blocks region's children.
    let (lang, _) = itemize_lang();
    let laid = run(
        &lang,
        "\\begin{itemize}\\item one\n\nstill one\\item two\\end{itemize}",
        FragmentLayout::Blocks,
    );
    let violations = check_block_contract(&laid.tree);
    assert!(violations.is_empty(), "{violations:?}\n{}", laid.dump());
}

#[test]
fn the_hook_consumes_the_item_markers() {
    let (lang, _) = itemize_lang();
    let laid = run(
        &lang,
        "\\begin{itemize}\\item one\\item two\\end{itemize}",
        FragmentLayout::Blocks,
    );
    assert!(
        laid.root().descendants().all(|n| n.name() != Some("item")),
        "{}",
        laid.dump()
    );
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
}

// --- a theorem-like element: synthesized decorators ------------------------------------

/// A theorem-like block whose hook prepends a run-in heading and appends a QED
/// mark, both decorators of never-parsed elements.
struct TheoremElement {
    heading: Arc<ElementSpec>,
    qed: Option<Arc<ElementSpec>>,
    /// Whether to place the synthesized nodes (a hook that does not is the
    /// `decorator-without-home` case).
    place: bool,
}

impl BlockRoleAspect for TheoremElement {
    fn block_role(&self, _node: &NodeRef<'_, Flm>) -> BlockRole {
        BlockRole::Block
    }
}

impl BlockStructure for TheoremElement {
    fn structure(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        bs: &mut dyn BlockStructurer,
    ) -> Result<Structuring, StructureError> {
        let heading = bs.synthesize_callable(&node, &self.heading)?;
        let qed = match &self.qed {
            Some(element) => Some(bs.synthesize_callable(&node, element)?),
            None => None,
        };
        if !self.place {
            return Ok(Structuring::Default);
        }
        let mut run = vec![heading.node()];
        run.extend(bs.body_run(&node)?);
        if let Some(qed) = &qed {
            run.push(qed.node());
        }
        let laid_out = bs.build_blocks(cx, &run)?;
        Ok(Structuring::Emit(vec![bs.with_body(cx, &node, laid_out)?]))
    }
}

fn theorem_lang(place: bool, with_qed: bool) -> flm_core::lang::BuiltLang {
    let heading = element_with_role("t.heading", BlockRole::BlockDecorator(Placement::Leads));
    let qed = element_with_role("t.qed-mark", BlockRole::BlockDecorator(Placement::Trails));
    let theorem = Arc::new(TheoremElement {
        heading: Arc::clone(&heading),
        qed: with_qed.then(|| Arc::clone(&qed)),
        place,
    });
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.theorem"))
            .with::<dyn BlockRoleAspect>(theorem.clone())
            .with::<dyn BlockStructure>(theorem),
    );
    toy_lang_with(vec![
        env_of("theorem", BodyDef::new().blocks(), element),
        ElementDef::new("t", Vec::<&str>::new(), SyntaxDef::Synthesized, heading),
        ElementDef::new("t", Vec::<&str>::new(), SyntaxDef::Synthesized, qed),
    ])
}

#[test]
fn a_synthesized_leading_decorator_opens_the_first_paragraph_of_the_body() {
    let lang = theorem_lang(true, true);
    let laid = run(
        &lang,
        "\\begin{theorem}  First.\n\nSecond.  \\end{theorem}",
        FragmentLayout::Blocks,
    );
    assert!(laid.diags.is_empty(), "{}", laid.diags.render_all());
    let body: Vec<NodeRef<'_, Flm>> = laid.children()[0].body().unwrap().iter().collect();
    assert_eq!(body.len(), 2, "{}", laid.dump());
    let first: Vec<NodeRef<'_, Flm>> = body[0].children().iter().collect();
    assert_eq!(first.len(), 2, "{}", laid.dump());
    assert_eq!(element_of(&first[0]).name.as_str(), "t.heading");
    assert_eq!(
        first[1].chars(),
        Some("First."),
        "head-trimmed through the decorator"
    );
}

#[test]
fn a_synthesized_trailing_decorator_closes_the_last_paragraph_of_the_body() {
    let lang = theorem_lang(true, true);
    let laid = run(
        &lang,
        "\\begin{theorem}First.\n\nSecond.  \\end{theorem}",
        FragmentLayout::Blocks,
    );
    let body: Vec<NodeRef<'_, Flm>> = laid.children()[0].body().unwrap().iter().collect();
    let last: Vec<NodeRef<'_, Flm>> = body[1].children().iter().collect();
    assert_eq!(last.len(), 2, "{}", laid.dump());
    assert_eq!(
        last[0].chars(),
        Some("Second."),
        "tail-trimmed through the decorator"
    );
    assert_eq!(element_of(&last[1]).name.as_str(), "t.qed-mark");
}

#[test]
fn a_synthesized_node_is_a_specials_formed_callable_of_its_element_with_no_content() {
    let lang = theorem_lang(true, false);
    let laid = run(
        &lang,
        "\\begin{theorem}x\\end{theorem}",
        FragmentLayout::Blocks,
    );
    let heading = laid.children()[0]
        .body()
        .unwrap()
        .first()
        .unwrap()
        .child(0)
        .unwrap();
    assert_eq!(heading.callable_type(), Some(FlmCallableType::Specials));
    assert_eq!(heading.name(), Some("t.heading"));
    assert_eq!(heading.child_count(), 0);
    assert!(heading.arguments().unwrap().is_empty());
    assert!(heading.slots().unwrap().is_empty());
}

#[test]
fn a_synthesized_node_lives_on_a_source_triggered_at_the_hooks_node() {
    let lang = theorem_lang(true, false);
    let laid = run(
        &lang,
        "\\begin{theorem}x\\end{theorem}",
        FragmentLayout::Blocks,
    );
    let theorem = laid.children()[0];
    let heading = theorem.body().unwrap().first().unwrap().child(0).unwrap();
    match heading.span().source().provenance() {
        SourceProvenance::Synthesized { triggered_at, .. } => {
            assert_eq!(triggered_at, theorem.span());
        }
        other => panic!("expected a synthesized source, got {other:?}"),
    }
}

#[test]
fn a_paragraph_opened_by_a_synthesized_decorator_spans_its_real_content() {
    let lang = theorem_lang(true, false);
    let laid = run(
        &lang,
        "\\begin{theorem}xyz\\end{theorem}",
        FragmentLayout::Blocks,
    );
    let paragraph = laid.children()[0].body().unwrap().first().unwrap();
    assert_eq!(paragraph.span_content(), "xyz");
}

#[test]
fn a_synthesized_decorator_the_hook_does_not_place_is_reported_and_dropped() {
    let lang = theorem_lang(false, false);
    let laid = run(
        &lang,
        "\\begin{theorem}x\\end{theorem}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        laid.identifiers(),
        [DecoratorWithoutHome::IDENTIFIER],
        "{}",
        laid.dump()
    );
    let condition = laid
        .diags
        .conditions::<DecoratorWithoutHome>()
        .next()
        .unwrap();
    assert_eq!(condition.element, "t.heading");
    assert_eq!(condition.hook, "t.theorem");
    assert!(
        laid.root()
            .descendants()
            .all(|n| element_of(&n).name.as_str() != "t.heading")
    );
}

// --- the other façade operations ------------------------------------------------------

/// A hook exercising one façade operation, selected per test.
struct Probe {
    op: Op,
    seen: Mutex<Vec<RegionLayout>>,
}

#[derive(Clone, Copy)]
enum Op {
    /// Return the pass's default handling explicitly.
    Default,
    /// Ask for the body of a node that has none.
    BodyOfMacro,
    /// Fail with the hook's own error.
    Fail,
    /// Record the region and fall through.
    Region,
    /// Record a diagnostic of its own and fall through.
    Diagnose,
    /// Emit nothing: the node is dropped.
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "t.probe.noticed", message = "the probe noticed something")]
struct Noticed;

impl BlockStructure for Probe {
    fn structure(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        bs: &mut dyn BlockStructurer,
    ) -> Result<Structuring, StructureError> {
        match self.op {
            Op::Default => Ok(Structuring::Emit(bs.default(cx, &node)?)),
            Op::BodyOfMacro => {
                bs.body_run(&node)?;
                Ok(Structuring::Default)
            }
            Op::Fail => Err(StructureError::hook(element_of(&node).name.clone(), "no")),
            Op::Region => {
                self.seen.lock().unwrap().push(bs.region());
                Ok(Structuring::Default)
            }
            Op::Diagnose => {
                bs.diagnose(Diagnostic::warning(Noticed, node.span().clone()));
                Ok(Structuring::Default)
            }
            Op::Drop => Ok(Structuring::Emit(Vec::new())),
        }
    }
}

/// The toy language plus `\probe{…}`, a macro of the given block role carrying
/// the probe hook.
fn probe_lang_with_role(op: Op, role: BlockRole) -> (flm_core::lang::BuiltLang, Arc<Probe>) {
    let probe = Arc::new(Probe {
        op,
        seen: Mutex::new(Vec::new()),
    });
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.probe"))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(role)))
            .with::<dyn BlockStructure>(probe.clone()),
    );
    (toy_lang_with(vec![macro_of("probe", 1, element)]), probe)
}

/// [`probe_lang_with_role`] with the default (inline) role.
fn probe_lang(op: Op) -> (flm_core::lang::BuiltLang, Arc<Probe>) {
    probe_lang_with_role(op, BlockRole::DEFAULT)
}

#[test]
fn a_hook_returning_the_default_handling_lays_the_node_out_like_no_hook() {
    let (lang, _) = probe_lang(Op::Default);
    let laid = run(&lang, "x \\probe{a   b} y", FragmentLayout::Blocks);
    let probe = laid.children()[0].child(1).unwrap();
    assert_eq!(element_of(&probe).name.as_str(), "t.probe");
    assert_eq!(text_of(probe), "a b");
    assert!(laid.diags.is_empty());
}

#[test]
fn asking_for_the_body_of_a_bodyless_node_is_a_no_body_failure() {
    let (lang, _) = probe_lang(Op::BodyOfMacro);
    let fragment = common::parse(&lang, "\\probe{a}");
    let mut diags = FlmDiagnostics::new();
    let error = structure(
        fragment.tree(),
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        &mut diags,
    )
    .expect_err("the pass fails");
    assert!(matches!(&error, StructureError::NoBody { element } if element.as_str() == "t.probe"));
    assert_eq!(
        error.diagnostic(fragment.tree()).identifier(),
        StructureFailed::IDENTIFIER
    );
}

#[test]
fn a_hooks_own_failure_ends_the_pass_typed() {
    let (lang, _) = probe_lang(Op::Fail);
    let fragment = common::parse(&lang, "\\probe{a}");
    let mut diags = FlmDiagnostics::new();
    let error = structure(
        fragment.tree(),
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        &mut diags,
    )
    .expect_err("the pass fails");
    match &error {
        StructureError::Hook { element, detail } => {
            assert_eq!(element.as_str(), "t.probe");
            assert_eq!(detail, "no");
        }
        other => panic!("expected the hook's own failure, got {other:?}"),
    }
}

#[test]
fn a_hook_on_an_inline_node_sees_the_inline_region_its_node_sits_in() {
    let (lang, probe) = probe_lang(Op::Region);
    run(&lang, "\\probe{a} {\\probe{b}}", FragmentLayout::Blocks);
    // The first probe sits in a paragraph run, the second inside a plain group:
    // both inline regions.
    let seen = probe.seen.lock().unwrap().clone();
    assert_eq!(seen, [RegionLayout::Inline, RegionLayout::Inline]);
}

#[test]
fn a_hook_under_the_inline_fragment_layout_sees_an_inline_region() {
    let (lang, probe) = probe_lang(Op::Region);
    run(&lang, "\\probe{a}", FragmentLayout::Inline);
    assert_eq!(probe.seen.lock().unwrap().clone(), [RegionLayout::Inline]);
}

#[test]
fn a_hook_on_a_block_node_in_a_blocks_region_sees_the_blocks_region() {
    let (lang, probe) = probe_lang_with_role(Op::Region, BlockRole::Block);
    run(&lang, "\\probe{a}", FragmentLayout::Blocks);
    assert_eq!(probe.seen.lock().unwrap().clone(), [RegionLayout::Blocks]);
}

#[test]
fn a_hook_emitting_nothing_for_a_runs_only_content_leaves_no_paragraph() {
    let (lang, _) = probe_lang(Op::Drop);
    let laid = run(&lang, "\\probe{a}", FragmentLayout::Blocks);
    assert!(laid.children().is_empty(), "{}", laid.dump());
}

#[test]
fn a_hook_emitting_nothing_leaves_a_tree_the_checker_accepts() {
    let (lang, _) = probe_lang(Op::Drop);
    let laid = run(&lang, "x\n\n\\probe{a}\n\ny", FragmentLayout::Blocks);
    assert!(
        check_block_contract(&laid.tree).is_empty(),
        "{}",
        laid.dump()
    );
}

#[test]
fn a_hook_can_record_a_diagnostic_of_its_own() {
    let (lang, _) = probe_lang(Op::Diagnose);
    let laid = run(&lang, "\\probe{a}", FragmentLayout::Blocks);
    assert_eq!(laid.identifiers(), [Noticed::IDENTIFIER]);
}
