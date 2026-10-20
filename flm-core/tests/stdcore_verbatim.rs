//! The `verbatim` feature (`flm_core::stdcore::features::verbatim`): the three
//! macros and two environments that the Python implementation's
//! `flm/feature/verbatim.py` ships, each parsed under the verbatim wrap
//! invariant, lowered to `flm.pres.verbatim` with its kind, display and
//! language, and printed byte for byte by the text backend.

mod common;

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};

use flm_core::model::element::element_of;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::ExpectDiag;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::{Flm, FlmGroupType};
use flm_core::stdcore::backends::pres::{
    self, VerbatimDisplay, VerbatimKind, VerbatimPayload, payload_of, plain_text,
};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::features::verbatim::{
    ENVIRONMENT, ENVIRONMENT_RULE, ENVIRONMENTS, MACRO, MACRO_RULE, MACROS, VerbatimCallable,
};
use flm_core::stdcore::{StdcoreConfig, package};

use common::lowering::text as text_of;
use common::stdcore::{identifiers, registry as stdcore_registry, run_workflow};

/// techy's condition identifier for a delimited verbatim region that never
/// closed.
const UNTERMINATED: &str = "core.verbatim.unterminated-verbatim";

/// techy's condition identifier for an environment body that never met its
/// terminator.
const MISSING_TERMINATOR: &str = "core.environments.missing-terminator";

/// The whole `stdcore` package, the `verbatim` feature among its features.
fn registry() -> Arc<FlmRegistry> {
    stdcore_registry(StdcoreConfig::new())
}

/// The element names of the callable nodes of a parse, in document order.
fn callable_elements(registry: &FlmRegistry, text: &str) -> Vec<String> {
    let fragment = common::parse(registry.lang(), text);
    common::callables(&fragment).iter().map(|node| element_of(node).name.to_string()).collect()
}

/// Every verbatim group of a parse, in storage order.
fn verbatim_groups(registry: &FlmRegistry, text: &str) -> Vec<String> {
    let fragment = common::parse(registry.lang(), text);
    fragment
        .tree()
        .iter_storage_order()
        .filter(|node| node.group_type() == Some(FlmGroupType::Verbatim))
        .map(|node| plain_text(std::iter::once(node)))
        .collect()
}

/// The first `flm.pres.verbatim` node of a lowered tree.
fn first_verbatim<'t>(tree: &'t NodeTree<Flm>) -> NodeRef<'t, Flm> {
    tree.iter_storage_order()
        .find(|node| element_of(node).name == pres::VERBATIM)
        .expect("a lowered verbatim node")
}

/// The payload and the printed content of the one verbatim node the document
/// lowers to.
fn lowered_verbatim(document: &str) -> (VerbatimPayload, String) {
    let registry = registry();
    let run = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    let lowered = common::lowering::lowered(&run, flm_core::model::addr::TreeId::tree(0));
    let node = first_verbatim(&lowered.tree);
    let payload = payload_of::<VerbatimPayload>(&node).expect("a verbatim payload").clone();
    let body = node.body().map(|body| plain_text(body.iter())).unwrap_or_default();
    (payload, body)
}

// --- the declarations ---------------------------------------------------------------

#[test]
fn the_five_callables_are_two_elements_by_invocation_form() {
    let registry = registry();
    assert_eq!(callable_elements(&registry, "\\verbtext+a+"), [MACRO]);
    assert_eq!(callable_elements(&registry, "\\verbcode+a+"), [MACRO]);
    assert_eq!(callable_elements(&registry, "\\verba+a+"), [MACRO]);
    assert_eq!(
        callable_elements(&registry, "\\begin{verbatimtext}a\\end{verbatimtext}"),
        [ENVIRONMENT]
    );
    assert_eq!(
        callable_elements(&registry, "\\begin{verbatimcode}a\\end{verbatimcode}"),
        [ENVIRONMENT]
    );
}

#[test]
fn every_declared_value_carries_its_name_and_kind() {
    let registry = registry();
    let values = |element: &str| -> Vec<(String, String, bool, bool)> {
        registry
            .elements_named(element)
            .iter()
            .map(|idx| {
                let element = registry.element(*idx).unwrap();
                let value = element.element.value_as::<VerbatimCallable>().expect("valued");
                (
                    value.name.to_string(),
                    value.kind.as_str().to_string(),
                    value.lang_arg,
                    value.block,
                )
            })
            .collect()
    };
    let expected_macros: Vec<(String, String, bool, bool)> = MACROS
        .iter()
        .map(|(name, kind, lang)| {
            (name.to_string(), kind.as_str().to_string(), *lang, false)
        })
        .collect();
    assert_eq!(values(MACRO), expected_macros);
    let expected_environments: Vec<(String, String, bool, bool)> = ENVIRONMENTS
        .iter()
        .map(|(name, kind, lang, block)| {
            (name.to_string(), kind.as_str().to_string(), *lang, *block)
        })
        .collect();
    assert_eq!(values(ENVIRONMENT), expected_environments);
}

#[test]
fn each_element_has_its_general_rule_and_the_recomposed_backend_s_rule() {
    let registry = registry();
    let rules = |element: &str| -> Vec<String> {
        registry
            .rules_for(element)
            .iter()
            .map(|idx| registry.rule(*idx).rule.name.to_string())
            .collect()
    };
    // The feature's own rule is the general default; the `latex-recomposed`
    // backend registers a second rule per element, the default on it alone.
    assert_eq!(rules(MACRO), [MACRO_RULE, "flm.verbatim.macro.lower.latex-recomposed"]);
    assert_eq!(
        rules(ENVIRONMENT),
        [ENVIRONMENT_RULE, "flm.verbatim.environment.lower.latex-recomposed"]
    );
}

#[test]
fn the_text_plan_over_the_package_stays_complete() {
    let plan =
        Plan::for_workflow(&registry(), text::WORKFLOW, &Overrides::none()).expect("complete");
    assert!(plan.active_backends().contains(text::BACKEND));
}

#[test]
fn the_package_lists_the_feature_once() {
    let names: Vec<String> = package(StdcoreConfig::new())
        .features
        .iter()
        .map(|feature| feature.info().name.to_string())
        .collect();
    assert_eq!(names.iter().filter(|name| *name == "verbatim").count(), 1);
}

// --- the parse ----------------------------------------------------------------------

#[test]
fn a_macros_content_sits_under_a_verbatim_group() {
    let registry = registry();
    assert_eq!(verbatim_groups(&registry, "\\verbtext+a\\b{c+"), ["a\\b{c"]);
}

#[test]
fn an_environments_body_sits_under_a_verbatim_group() {
    let registry = registry();
    assert_eq!(
        verbatim_groups(&registry, "\\begin{verbatimcode}\nlet x = 1;\n\\end{verbatimcode}"),
        ["let x = 1;\n"]
    );
}

#[test]
fn a_macro_delimiter_is_auto_matched_to_its_partner() {
    let registry = registry();
    // A paired opener closes with its partner and nests: `a{b}c` is read whole.
    assert_eq!(verbatim_groups(&registry, "\\verbcode{a{b}c}"), ["a{b}c"]);
    // Any other character closes with itself.
    assert_eq!(verbatim_groups(&registry, "\\verbcode+x = 1+"), ["x = 1"]);
    assert_eq!(verbatim_groups(&registry, "\\verbcode<a>"), ["a"]);
}

#[test]
fn markup_inside_a_verbatim_region_stays_literal() {
    let registry = registry();
    let text = "\\begin{verbatimcode}\n\\emph{a} %% not a comment\n\\end{verbatimcode}";
    // The only callable is the environment itself: nothing inside was read as
    // markup, and no comment was stripped.
    assert_eq!(callable_elements(&registry, text), [ENVIRONMENT]);
    assert_eq!(verbatim_groups(&registry, text), ["\\emph{a} %% not a comment\n"]);
}

#[test]
fn the_newline_after_a_begin_is_not_part_of_the_content() {
    let registry = registry();
    assert_eq!(
        verbatim_groups(&registry, "\\begin{verbatimtext}\nabc\\end{verbatimtext}"),
        ["abc"]
    );
    // A second newline is content, and so is a leading space.
    assert_eq!(
        verbatim_groups(&registry, "\\begin{verbatimtext}\n\n abc\\end{verbatimtext}"),
        ["\n abc"]
    );
}

#[test]
fn empty_verbatim_content_parses_to_an_empty_group() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\verbtext||");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_eq!(verbatim_groups(&registry, "\\verbtext||"), [""]);
    assert_eq!(
        verbatim_groups(&registry, "\\begin{verbatimtext}\\end{verbatimtext}"),
        [""]
    );
}

#[test]
fn an_unterminated_macro_region_is_the_typed_verbatim_condition() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\verbcode+abc");
    fragment.diags().expect(UNTERMINATED).once();
}

#[test]
fn an_unterminated_environment_is_the_typed_terminator_condition() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\begin{verbatimcode}\nabc\n");
    fragment.diags().expect(MISSING_TERMINATOR).once();
}

#[test]
fn every_callable_satisfies_the_tree_contract_under_the_check_workflow() {
    // The `check` workflow runs the block-structure contract checker, whose
    // verbatim rules are the wrap invariant's independent witness.
    let registry = registry();
    let run = run_workflow(
        &registry,
        "check",
        &[
            "\\verbtext+a+ \\verbcode[py]+b+ \\verba+c+",
            "\\begin{verbatimtext}d\\end{verbatimtext}",
            "\\begin{verbatimcode}[py]\ne\n\\end{verbatimcode}",
        ],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

// --- the lowering -------------------------------------------------------------------

#[test]
fn a_text_macro_lowers_to_an_inline_verbatim_of_kind_text() {
    let (payload, body) = lowered_verbatim("\\verbtext+hello world+");
    assert_eq!(payload.kind, VerbatimKind::Text);
    assert_eq!(payload.display, VerbatimDisplay::Inline);
    assert_eq!(payload.lang, None);
    assert_eq!(body, "hello world");
}

#[test]
fn a_code_macro_lowers_to_an_inline_verbatim_of_kind_code() {
    let (payload, body) = lowered_verbatim("\\verbcode+x = 1+");
    assert_eq!(payload.kind, VerbatimKind::Code);
    assert_eq!(payload.display, VerbatimDisplay::Inline);
    assert_eq!(body, "x = 1");
}

#[test]
fn an_identifier_macro_lowers_to_the_kind_spelled_a() {
    let (payload, _) = lowered_verbatim("\\verba+keyword+");
    assert_eq!(payload.kind, VerbatimKind::Identifier);
    assert_eq!(payload.kind.as_str(), "a");
    assert_eq!(payload.display, VerbatimDisplay::Inline);
}

#[test]
fn the_text_environment_lowers_inline_and_the_code_environment_as_a_block() {
    let (text_payload, text_body) =
        lowered_verbatim("\\begin{verbatimtext}some text here\\end{verbatimtext}");
    assert_eq!(text_payload.kind, VerbatimKind::Text);
    assert_eq!(text_payload.display, VerbatimDisplay::Inline);
    assert_eq!(text_body, "some text here");

    let (code_payload, code_body) =
        lowered_verbatim("\\begin{verbatimcode}\nlet x = 1;\n\\end{verbatimcode}");
    assert_eq!(code_payload.kind, VerbatimKind::Code);
    assert_eq!(code_payload.display, VerbatimDisplay::Block);
    assert_eq!(code_body, "let x = 1;\n");
}

#[test]
fn an_environment_lowers_to_exactly_one_verbatim_node() {
    let registry = registry();
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["\\begin{verbatimcode}\nx\n\\end{verbatimcode}"],
        &Overrides::none(),
    );
    let lowered = common::lowering::lowered(&run, flm_core::model::addr::TreeId::tree(0));
    let count = lowered
        .tree
        .iter_storage_order()
        .filter(|node| element_of(node).name == pres::VERBATIM)
        .count();
    assert_eq!(count, 1);
}

#[test]
fn the_language_argument_reaches_the_payload() {
    let (payload, body) = lowered_verbatim("\\verbcode[python]+def f(): pass+");
    assert_eq!(payload.lang.as_deref(), Some("python"));
    assert_eq!(body, "def f(): pass");
    let (identifier, _) = lowered_verbatim("\\verba[rust]+struct+");
    assert_eq!(identifier.lang.as_deref(), Some("rust"));
    let (environment, _) =
        lowered_verbatim("\\begin{verbatimcode}[python]\ndef f(): pass\n\\end{verbatimcode}");
    assert_eq!(environment.lang.as_deref(), Some("python"));
}

#[test]
fn a_callable_without_a_language_argument_never_carries_one() {
    // `\verbtext` declares no language argument, so `[python]` is the verbatim
    // content between auto-matched brackets — Python's reading too.
    let (payload, body) = lowered_verbatim("\\verbtext[python]");
    assert_eq!(payload.lang, None);
    assert_eq!(body, "python");
}

#[test]
fn an_unterminated_macro_lowers_to_the_content_read_so_far() {
    let (payload, body) = lowered_verbatim("\\verbcode+abc");
    assert_eq!(payload.kind, VerbatimKind::Code);
    assert_eq!(body, "abc");
}

// --- the text output ----------------------------------------------------------------

#[test]
fn an_inline_verbatim_prints_its_content_byte_for_byte() {
    let registry = registry();
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["say \\verbcode+a{b}\\c+ here"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "say a{b}\\c here\n");
}

#[test]
fn a_block_verbatim_prints_on_lines_of_its_own() {
    let registry = registry();
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["Before.\n\n\\begin{verbatimcode}\nif x:\n    y = 1\n\\end{verbatimcode}\n\nAfter."],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "Before.\n\nif x:\n    y = 1\n\nAfter.\n");
}

#[test]
fn the_kind_and_the_language_print_nothing_in_text() {
    let registry = registry();
    let plain = run_workflow(&registry, text::WORKFLOW, &["\\verbcode+x+"], &Overrides::none());
    let tagged =
        run_workflow(&registry, text::WORKFLOW, &["\\verbcode[python]+x+"], &Overrides::none());
    let identified = run_workflow(&registry, text::WORKFLOW, &["\\verba+x+"], &Overrides::none());
    assert_eq!(text_of(&plain), "x\n");
    assert_eq!(text_of(&tagged), "x\n");
    assert_eq!(text_of(&identified), "x\n");
}

#[test]
fn an_inline_text_environment_prints_inside_its_paragraph() {
    let registry = registry();
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["say \\begin{verbatimtext}a b\\end{verbatimtext} here"],
        &Overrides::none(),
    );
    assert_eq!(text_of(&run), "say a b here\n");
}

// --- verbatim content under user-definition expansion --------------------------------

/// The `stdcore` registry with one user definition, `\mymacro{#1}`, whose
/// template wraps its argument in brackets.
fn registry_with_mymacro() -> Arc<FlmRegistry> {
    let definitions = vec![
        Definition::macro_("mymacro", vec![ArgDef::mandatory("code")]).text("[#{code}]"),
        Definition::environment("mybox", Vec::new()).text("<#{body}>"),
    ];
    stdcore_registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

#[test]
fn a_verbatim_environment_in_a_user_macros_argument_stays_verbatim() {
    // Nothing inside the verbatim environment is reinterpreted — the `#1`,
    // `#2`, the `\def` and the braces reach the output as the bytes they are.
    let registry = registry_with_mymacro();
    let document =
        "\\mymacro{\\begin{verbatimcode}\\def\\x#1#2{Hello #1 and #2}\\end{verbatimcode}}";
    let run = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The verbatim block is a block of its own inside the expansion's brackets.
    assert_eq!(text_of(&run), "[\n\n\\def\\x#1#2{Hello #1 and #2}\n\n]\n");
}

#[test]
fn a_verbatim_environment_in_a_user_environments_body_stays_verbatim() {
    let registry = registry_with_mymacro();
    let document =
        "\\begin{mybox}\\begin{verbatimcode}\\def\\x#1{#1}\\end{verbatimcode}\\end{mybox}";
    let run = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "<\n\n\\def\\x#1{#1}\n\n>\n");
}

