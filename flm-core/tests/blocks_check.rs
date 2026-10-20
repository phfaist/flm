//! The block contract checker of `flm_core::lang::rewrites::blocks::check`: the
//! two-part contract over trees the block-structure pass produced, and over
//! deliberately broken trees built by hand.

mod common;

use std::sync::Arc;

use flm_core::model::aspects::{BlockRole, Placement};
use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::ElementSpec;
use flm_core::model::names::ElementName;
use flm_core::lang::ext::{
    FlmStateExt, TemplateCommandRule, TemplateInteriorRules, TextSourceMap, TextSourcePiece,
};
use flm_core::lang::mint::{BodyDef, FallbackDef, FallbackKind, FlmSynthesizedSpec};
use flm_core::lang::rewrites::blocks::{
    BlockStructure, BlockStructurer, ContractViolated, ContractViolation, ContractViolationKind,
    StructureCx, StructureError, Structuring, check_block_contract, check_parse_contract,
};
use flm_core::lang::{
    Flm, FlmCallableType, FlmGroupType, FlmNodeExt, FlmSource, LangInputs, RestrictedInterior,
};
use techy::core::node::{
    BuildId, CallableData, ChildRegion, GroupData, NodeBuildError, NodeKind, NodeRef,
    ParsedArgument, ParsedArguments, ParsedSlots,
};
use techy::core::token::GroupRule;
use techy::core::Lang;
use techy::error::DiagnosticInfo;
use techy::latexlike::{InvocationSyntaxData, MacroSpec, Mode};
use techy::source::{Source, SourceSpan};

use common::blocks::{Site, element_with_role, env_of, run, text_of, toy_lang, toy_lang_with};

fn kinds(violations: &[ContractViolation]) -> Vec<&'static str> {
    violations
        .iter()
        .map(|violation| violation.kind.as_str())
        .collect()
}

// --- structured trees conform -------------------------------------------------------------

#[test]
fn a_structured_tree_satisfies_both_parts() {
    let lang = toy_lang();
    let text = "  \\runin{T} a  b {c} \\(x  y\\) \\verb|p  q| \\qed\n\n%% c\n\n\
                \\begin{quotation}q\n\nr\\end{quotation} tail";
    let laid = run(&lang, text, FragmentLayout::Blocks);
    assert_eq!(
        kinds(&check_parse_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
    assert_eq!(
        kinds(&check_block_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
}

#[test]
fn region_noise_is_exempt_from_the_text_rules() {
    // An argument's pre-space and a verbatim environment's gobbled newline are
    // region noise techy copies verbatim by contract; they are not content text.
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\emph  {a} \\begin{verbatimcode}\nx\n\\end{verbatimcode}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        kinds(&check_parse_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
    assert_eq!(
        kinds(&check_block_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
}

#[test]
fn a_paragraph_break_inside_a_math_group_is_no_violation() {
    // A math group is an opaque region, so the marker is kept as it stands.
    // See design rationale at [§flm-dd-dr:lang-regions].
    let lang = toy_lang();
    let laid = run(&lang, "\\(a\n\nb\\)", FragmentLayout::Blocks);
    assert_eq!(
        kinds(&check_block_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
}

#[test]
fn a_paragraph_break_inside_a_math_mode_body_is_no_violation() {
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{equation}a\n\nb\\end{equation}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        kinds(&check_block_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
}

#[test]
fn a_region_read_in_text_mode_inside_mathematics_is_held_to_the_text_rules() {
    // Such a region is document content, not source: the pass lays it out, and
    // the checker holds what it leaves to the text rules like any other text.
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{equation}\\begin{textinmath}a  b\\end{textinmath}\\end{equation}",
        FragmentLayout::Blocks,
    );
    assert_eq!(text_of(laid.tree.root()), "a b", "{}", laid.dump());
    assert_eq!(
        kinds(&check_block_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
    // The same text before the pass — two spaces where one must stand — is a
    // violation there, which is what says the region is no longer exempt.
    let fragment = common::parse(
        &lang,
        "\\begin{equation}\\begin{textinmath}a  b\\end{textinmath}\\end{equation}",
    );
    assert_eq!(kinds(&check_block_contract(fragment.tree())), ["WhitespaceNotCollapsed"]);
}

#[test]
fn the_mathematics_around_a_text_region_stays_exempt() {
    // Two spaces inside the formula itself are what the author wrote.
    let lang = toy_lang();
    let laid = run(
        &lang,
        "\\begin{equation}a  b\\begin{textinmath}c  d\\end{textinmath}\\end{equation}",
        FragmentLayout::Blocks,
    );
    assert_eq!(
        kinds(&check_block_contract(&laid.tree)),
        Vec::<&str>::new(),
        "{}",
        laid.dump()
    );
}

#[test]
fn a_parsed_tree_satisfies_the_parse_level_part() {
    let lang = toy_lang();
    let fragment = common::parse(
        &lang,
        "a\n\n\\verb|x| \\begin{verbatimcode}\ny\n\\end{verbatimcode}",
    );
    assert_eq!(
        kinds(&check_parse_contract(fragment.tree())),
        Vec::<&str>::new()
    );
}

#[test]
fn an_unknown_environment_recovery_satisfies_the_parse_level_part() {
    let lang = toy_lang();
    let fragment = common::blocks::parse_lenient(&lang, "\\begin{nope}x\\end{nope}");
    assert!(fragment.diags().has_errors(), "the recovery is diagnosed");
    assert_eq!(
        kinds(&check_parse_contract(fragment.tree())),
        Vec::<&str>::new()
    );
}

#[test]
fn an_empty_verbatim_group_is_trivially_consistent() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, "\\verb||");
    assert_eq!(
        kinds(&check_parse_contract(fragment.tree())),
        Vec::<&str>::new()
    );
}

#[test]
fn a_parsed_tree_fails_the_block_part_on_its_markers_and_raw_text() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, "a\n\nb  c");
    let violations = check_block_contract(fragment.tree());
    assert!(kinds(&violations).contains(&"ParbreakSurvived"));
    assert!(kinds(&violations).contains(&"WhitespaceNotCollapsed"));
}

// --- hand-built trees ---------------------------------------------------------------------

/// A callable of a synthesized element (a decorator), staged by hand.
fn decorator(site: &mut Site, element: Arc<ElementSpec>) -> BuildId {
    let spec = Arc::new(FlmSynthesizedSpec::new(element));
    site.stage(NodeKind::callable(spec.callable_data()), 0..0, Vec::new())
}

#[test]
fn an_element_of_the_wrong_value_is_a_mismatch() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a");
    let span = site.span(0..1);
    let ext = FlmNodeExt::new(Arc::new(ElementSpec::new("flm.text")));
    let state = Arc::clone(&site.state);
    let text = site.stage_with(NodeKind::chars(span.span()), 0..1, state, ext, Vec::new());
    let root = site.stage(NodeKind::list(), 0..1, vec![text]);
    let tree = site.finish(root);
    let violations = check_parse_contract(&tree);
    assert_eq!(kinds(&violations), ["ElementMismatch"]);
    assert_eq!(violations[0].node, Some(tree.root().child(0).unwrap().id()));
}

/// A spec that is none of the flm-minted types carries no element at all, so the
/// single minting point refuses to stage such a node: `UnmintedSpec`, reported
/// as techy's `ExtMintFailed`. It is a caller-contract violation, never document
/// input.
#[test]
fn a_callable_of_a_techy_spec_cannot_be_minted() {
    let lang = toy_lang();
    let site = Site::new(&lang, "\\foo");
    let kind = NodeKind::callable(CallableData {
        callable_type: FlmCallableType::Macro,
        name: "foo".into(),
        spec: Arc::new(MacroSpec::<Flm>::new(Vec::new())),
        arguments: ParsedArguments::empty(),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Specials,
    });
    let span = site.span(0..4);
    let error = Flm::make_node_ext(
        &kind,
        &span,
        &site.state,
        site.builder.staged_children(&[]),
    )
    .expect_err("no element can be resolved");
    assert!(matches!(error, NodeBuildError::ExtMintFailed { .. }));
}

/// The checker still reports such a node when a caller hands it one built with
/// an element of its own: the spec is not flm-minted.
#[test]
fn a_callable_of_a_techy_spec_is_not_minted() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "\\foo");
    let data = CallableData {
        callable_type: FlmCallableType::Macro,
        name: "foo".into(),
        spec: Arc::new(MacroSpec::<Flm>::new(Vec::new())),
        arguments: ParsedArguments::empty(),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Specials,
    };
    let state = Arc::clone(&site.state);
    let ext = FlmNodeExt::new(Arc::clone(&state.ext().structural.unknown_macro));
    let node = site.stage_with(NodeKind::callable(data), 0..4, state, ext, Vec::new());
    let root = site.stage(NodeKind::list(), 0..4, vec![node]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_parse_contract(&tree)), ["SpecNotMinted"]);
}

/// The unknown-environment element on a node that is not an environment-formed
/// callable is the shape violation.
#[test]
fn the_unknown_environment_element_on_a_macro_node_is_a_shape_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "\\foo");
    let state = Arc::clone(&site.state);
    let element = Arc::clone(&state.ext().structural.unknown_environment);
    let ext = FlmNodeExt::new(Arc::clone(&element));
    let span = site.span(0..4);
    let node = site.stage_with(
        NodeKind::chars(span.span()),
        0..4,
        state,
        ext,
        Vec::new(),
    );
    let root = site.stage(NodeKind::list(), 0..4, vec![node]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_parse_contract(&tree)),
        ["ElementMismatch", "UnknownCallableShape"]
    );
}

#[test]
fn a_fallback_answered_macro_with_an_argument_breaks_the_fallback_shape() {
    let mut inputs = LangInputs::new();
    inputs.fallbacks.push(FallbackDef {
        owner: "math".into(),
        modes: vec![Mode::Math],
        kind: FallbackKind::Macro,
        element: common::element("math.symbol"),
    });
    inputs.elements.push(common::macro_def("t", "emph", 1));
    let lang = common::build(inputs);
    let spec = Arc::clone(
        lang.fallback()
            .spec_for(FallbackKind::Macro, Mode::Math)
            .unwrap(),
    );
    // An argument spec of the language's own: the one `\emph` was parsed against.
    let parsed = common::parse(&lang, "\\emph{x}");
    let emph = common::callables(&parsed).into_iter().next().unwrap();
    let argument_spec = Arc::clone(&emph.arguments().unwrap().get(0).unwrap().spec);
    let mut site = Site::new(&lang, "\\alpha{x}");
    let content = site.text(7..8);
    let group_kind = NodeKind::group(GroupData::new(FlmGroupType::Content, "{", "}"));
    let group = site.stage(group_kind, 6..9, vec![content]);
    let data = CallableData {
        callable_type: FlmCallableType::Macro,
        name: "alpha".into(),
        spec,
        arguments: ParsedArguments::new(vec![ParsedArgument::provided(
            argument_spec,
            ChildRegion::single(0),
            (),
        )]),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Specials,
    };
    let node = site.stage(NodeKind::callable(data), 0..9, vec![group]);
    let root = site.stage(NodeKind::list(), 0..9, vec![node]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_parse_contract(&tree)), ["FallbackShape"]);
}

#[test]
fn a_verbatim_groups_own_text_under_the_ordinary_state_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a");
    let text = site.text(0..1);
    let kind = NodeKind::group(GroupData::new(FlmGroupType::Verbatim, "", ""));
    let group = site.stage(kind, 0..1, vec![text]);
    let root = site.stage(NodeKind::list(), 0..1, vec![group]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_parse_contract(&tree)),
        ["VerbatimInteriorRulesOn"]
    );
}

/// The contract's verdict on a verbatim group whose own text was read under
/// the rules `rules_from` derives and carries the policy `policy`.
fn verbatim_text_under(rules_from: RestrictedInterior, policy: RestrictedInterior) -> Vec<&'static str> {
    let lang = toy_lang();
    let terminator = Arc::new(GroupRule {
        group_type: FlmGroupType::Verbatim,
        open: String::new(),
        close: "|".into(),
    });
    let ext = FlmStateExt::new(Arc::clone(lang.structural())).with_restricted_interior(policy);
    let interior = lang.derive_state(&rules_from.verbatim_delta(terminator).ext(ext)).unwrap();
    let mut site = Site::new(&lang, "a");
    let span = site.span(0..1);
    let kind = NodeKind::chars(span.span());
    let node_ext =
        Flm::make_node_ext(&kind, &span, &interior, site.builder.staged_children(&[])).unwrap();
    let text = site.stage_with(kind, 0..1, interior, node_ext, Vec::new());
    let group = site.stage(NodeKind::group(GroupData::new(FlmGroupType::Verbatim, "", "")), 0..1, vec![text]);
    let root = site.stage(NodeKind::list(), 0..1, vec![group]);
    let tree = site.finish(root);
    kinds(&check_parse_contract(&tree))
}

/// A template policy that keeps the placeholder escape `#` on, with or
/// without special character sequences.
fn template_policy(specials: bool) -> RestrictedInterior {
    RestrictedInterior::Template(TemplateInteriorRules {
        commands: vec![TemplateCommandRule { escape_char: '#', name_chars: String::new() }],
        specials,
    })
}

#[test]
fn a_template_verbatim_interior_may_keep_on_what_its_policy_lists_and_nothing_else() {
    // The placeholder escape on, as the policy lists: no violation.
    assert_eq!(
        verbatim_text_under(template_policy(false), template_policy(false)),
        Vec::<&str>::new()
    );
    // Special character sequences on, which the policy does not list.
    assert_eq!(
        verbatim_text_under(template_policy(true), template_policy(false)),
        ["VerbatimInteriorRulesOn"]
    );
}

#[test]
fn a_placeholder_typed_callable_fails_the_block_part() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "#1");
    let spec = Arc::new(FlmSynthesizedSpec::new(common::element(
        "t.placeholder-spec",
    )));
    let data = CallableData {
        callable_type: FlmCallableType::Placeholder,
        name: "1".into(),
        spec,
        arguments: ParsedArguments::empty(),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Specials,
    };
    let node = site.stage(NodeKind::callable(data), 0..2, Vec::new());
    let root = site.stage(NodeKind::list(), 0..2, vec![node]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_parse_contract(&tree)), Vec::<&str>::new());
    assert_eq!(kinds(&check_block_contract(&tree)), ["PlaceholderSurvived"]);
}

#[test]
fn a_paragraph_with_delimiters_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "{a}");
    let text = site.text(1..2);
    let kind = NodeKind::group(GroupData::new(FlmGroupType::Paragraph, "{", "}"));
    let paragraph = site.stage(kind, 0..3, vec![text]);
    let root = site.stage(NodeKind::list(), 0..3, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["ParagraphDelimiters"]);
}

#[test]
fn an_empty_paragraph_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "");
    let paragraph = site.paragraph(0..0, Vec::new());
    let root = site.stage(NodeKind::list(), 0..0, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["EmptyParagraph"]);
}

#[test]
fn a_paragraph_inside_a_paragraph_is_a_block_in_a_paragraph() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a");
    let text = site.text(0..1);
    let inner = site.paragraph(0..1, vec![text]);
    let outer = site.paragraph(0..1, vec![inner]);
    let root = site.stage(NodeKind::list(), 0..1, vec![outer]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["BlockInParagraph"]);
}

#[test]
fn a_leading_decorator_after_content_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a");
    let text = site.text(0..1);
    let lead = decorator(
        &mut site,
        element_with_role("t.lead", BlockRole::BlockDecorator(Placement::Leads)),
    );
    let paragraph = site.paragraph(0..1, vec![text, lead]);
    let root = site.stage(NodeKind::list(), 0..1, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["LeadsNotFirst"]);
}

#[test]
fn a_trailing_decorator_before_content_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a");
    let trail = decorator(
        &mut site,
        element_with_role("t.trail", BlockRole::BlockDecorator(Placement::Trails)),
    );
    let text = site.text(0..1);
    let paragraph = site.paragraph(0..1, vec![trail, text]);
    let root = site.stage(NodeKind::list(), 0..1, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["TrailsNotLast"]);
}

#[test]
fn uncollapsed_whitespace_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a  b");
    let text = site.text(0..4);
    let paragraph = site.paragraph(0..4, vec![text]);
    let root = site.stage(NodeKind::list(), 0..4, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_block_contract(&tree)),
        ["WhitespaceNotCollapsed"]
    );
}

#[test]
fn an_untrimmed_paragraph_edge_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, " a");
    let text = site.text(0..2);
    let paragraph = site.paragraph(0..2, vec![text]);
    let root = site.stage(NodeKind::list(), 0..2, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_block_contract(&tree)),
        ["ParagraphEdgeNotTrimmed"]
    );
}

#[test]
fn two_adjacent_text_siblings_are_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "ab");
    let a = site.text(0..1);
    let b = site.text(1..2);
    let paragraph = site.paragraph(0..2, vec![a, b]);
    let root = site.stage(NodeKind::list(), 0..2, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["AdjacentText"]);
}

#[test]
fn inline_content_beside_a_paragraph_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "ab");
    let a = site.text(0..1);
    let paragraph = site.paragraph(0..1, vec![a]);
    let b = site.text(1..2);
    let root = site.stage(NodeKind::list(), 0..2, vec![paragraph, b]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_block_contract(&tree)),
        ["InlineInBlocksRegion"]
    );
}

#[test]
fn a_paragraph_inside_a_plain_group_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "{a}");
    let text = site.text(1..2);
    let paragraph = site.paragraph(1..2, vec![text]);
    let kind = NodeKind::group(GroupData::new(FlmGroupType::Content, "{", "}"));
    let group = site.stage(kind, 0..3, vec![paragraph]);
    let root = site.stage(NodeKind::list(), 0..3, vec![group]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_block_contract(&tree)),
        ["ParagraphInInlineContainer"]
    );
}

/// An inline environment whose hook lays its body out as blocks although its
/// declaration (`BodyDef::new()` on an inline-role element) makes the body
/// inline — the shape the checker must catch.
struct BlocksInInlineBody;

impl BlockStructure for BlocksInInlineBody {
    fn structure(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        bs: &mut dyn BlockStructurer,
    ) -> Result<Structuring, StructureError> {
        let run = bs.body_run(&node)?;
        let body = bs.build_blocks(cx, &run)?;
        Ok(Structuring::Emit(vec![bs.with_body(cx, &node, body)?]))
    }
}

#[test]
fn a_paragraph_in_a_body_declared_inline_is_a_violation() {
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.oddenv"))
            .with::<dyn BlockStructure>(Arc::new(BlocksInInlineBody)),
    );
    let lang = toy_lang_with(vec![env_of("oddenv", BodyDef::new(), element)]);
    let laid = run(&lang, "\\begin{oddenv}a\\end{oddenv}", FragmentLayout::Inline);
    let violations = check_block_contract(&laid.tree);
    assert_eq!(
        kinds(&violations),
        ["ParagraphInInlineContainer"],
        "{}",
        laid.dump()
    );
    assert!(matches!(
        &violations[0].kind,
        ContractViolationKind::ParagraphInInlineContainer { element }
            if element.as_str() == "t.oddenv"
    ));
}

#[test]
fn inline_content_in_a_body_declared_blocks_is_a_violation() {
    // The parsed tree: the body of `quotation` (declared a blocks region) still
    // holds bare text.
    let lang = toy_lang();
    let fragment = common::parse(&lang, "\\begin{quotation}a\\end{quotation}");
    assert_eq!(
        kinds(&check_block_contract(fragment.tree())),
        ["InlineInBlocksRegion"]
    );
}

#[test]
fn inline_content_in_a_body_that_is_blocks_by_its_role_is_a_violation() {
    // `blockenv` declares `BodyDef::new()`: the layout comes from the block
    // role (`BodyLayout::FromRole`).
    let lang = toy_lang();
    let fragment = common::parse(&lang, "\\begin{blockenv}a\\end{blockenv}");
    assert_eq!(
        kinds(&check_block_contract(fragment.tree())),
        ["InlineInBlocksRegion"]
    );
}

#[test]
fn a_source_map_that_does_not_tile_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "ab");
    let span = site.span(0..2);
    let kind = NodeKind::chars(span.span());
    let state = Arc::clone(&site.state);
    let map = TextSourceMap::new(vec![TextSourcePiece {
        out_len: 1,
        span: site.span(0..1),
    }]);
    let ext = FlmNodeExt::new(Arc::clone(&lang.structural().text)).with_source_map(Arc::new(map));
    let text = site.stage_with(kind, 0..2, state, ext, Vec::new());
    let root = site.stage(NodeKind::list(), 0..2, vec![text]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_block_contract(&tree)),
        ["SourceMapDoesNotTile"]
    );
}

#[test]
fn a_source_map_on_a_group_is_a_violation() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "{}");
    let kind = NodeKind::group(GroupData::new(FlmGroupType::Content, "{", "}"));
    let state = Arc::clone(&site.state);
    let map = TextSourceMap::new(Vec::new());
    let ext = FlmNodeExt::new(Arc::clone(&lang.structural().group)).with_source_map(Arc::new(map));
    let group = site.stage_with(kind, 0..2, state, ext, Vec::new());
    let root = site.stage(NodeKind::list(), 0..2, vec![group]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_block_contract(&tree)), ["SourceMapOnNonText"]);
}

#[test]
fn a_violation_becomes_an_error_diagnostic_at_its_node() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, "a  b");
    let text = site.text(0..4);
    let paragraph = site.paragraph(0..4, vec![text]);
    let root = site.stage(NodeKind::list(), 0..4, vec![paragraph]);
    let tree = site.finish(root);
    let violation = check_block_contract(&tree).remove(0);
    let diagnostic = violation.diagnostic(&tree);
    assert_eq!(diagnostic.identifier(), ContractViolated::IDENTIFIER);
    assert_eq!(diagnostic.span().content(), "a  b");
    let condition = diagnostic
        .data()
        .downcast_ref::<ContractViolated>()
        .unwrap();
    assert_eq!(
        condition.rule,
        ContractViolationKind::WhitespaceNotCollapsed.as_str()
    );
}

#[test]
fn every_violation_is_reported_in_storage_order() {
    let lang = toy_lang();
    let mut site = Site::new(&lang, " a  b");
    let text = site.text(0..5);
    let paragraph = site.paragraph(0..5, vec![text]);
    let root = site.stage(NodeKind::list(), 0..5, vec![paragraph]);
    let tree = site.finish(root);
    assert_eq!(
        kinds(&check_block_contract(&tree)),
        ["ParagraphEdgeNotTrimmed", "WhitespaceNotCollapsed"]
    );
}

#[test]
fn a_source_from_another_tree_is_not_the_verbatim_groups_own() {
    // Content substituted into a verbatim group from another source (what an
    // expansion does) carries its own state and is not the group's own text.
    let lang = toy_lang();
    let other: Arc<FlmSource> = Arc::new(Source::new("x"));
    let mut site = Site::new(&lang, "");
    let kind = NodeKind::chars(SourceSpan::entire(&other).span());
    let span = SourceSpan::entire(&other);
    let state = Arc::clone(&site.state);
    let ext = Flm::make_node_ext(&kind, &span, &state, site.builder.staged_children(&[])).unwrap();
    let text = site
        .builder
        .add(kind, span, state, Vec::new(), ext, ())
        .unwrap();
    let group_kind = NodeKind::group(GroupData::new(FlmGroupType::Verbatim, "", ""));
    let group = site.stage(group_kind, 0..0, vec![text]);
    let root = site.stage(NodeKind::list(), 0..0, vec![group]);
    let tree = site.finish(root);
    assert_eq!(kinds(&check_parse_contract(&tree)), Vec::<&str>::new());
}
