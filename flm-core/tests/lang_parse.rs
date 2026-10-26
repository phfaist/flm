//! Parsing through a built language (`flm_core::lang`): the callable shapes and
//! every argument shape, an argument and a body read by a parser of the test's
//! own in each syntax, the elements every node carries, paragraph breaks,
//! dollar math, the forbidden lone percent sign, the math fallback, and the
//! unknown-environment recovery.

mod common;

use std::sync::Arc;

use flm_core::model::element::element_of;
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry};
use flm_core::lang::mint::{
    arg_layout_of, body_def_of, field_name, ArgDef, ArgLayout, ArgStdKind, BodyDef, BodyLayout,
    DefinitionShape, ElementDef, FallbackDef, FallbackKind, FlmArgumentParser, FlmBodyParser,
    ScopedPackageDef,
    SyntaxDef, TackOnField, FIELD,
};
use flm_core::lang::rewrites::blocks::{BlocksConfig, argument_holds_blocks, structure};
use flm_core::lang::FlmDiagnostics;
use flm_core::model::doc::FragmentLayout;
use flm_core::model::names::Name;
use flm_core::lang::{
    BuiltLang, DollarMathDisabled, Flm, FlmCallableType, FlmGroupType, LangInputs,
    PercentForbidden,
};
use techy::core::constructs::{
    ArgumentParser, ConstructParser, ConstructParserResult, EnvironmentBody, EnvironmentBodyParser,
    OptionalGroupArgumentParser, ParseContext, ParsedArgumentNodes, UnresolvableCommand,
};
use techy::core::ParsingStateDelta;
use techy::error::DiagnosticInfo;
use techy::core::specs::ArgumentSpec;
use techy::core::token::{ForbiddenChar, GroupRule};
use techy::latexlike::{
    EnvironmentBehavior, EnvironmentInvocation, InvocationSyntaxData, MathGroupForm, Mode,
    UnknownEnvironment,
};

use common::{
    build, callable_names, callables, element, env_def, lang, macro_def, macro_with, parse,
};

// --- the callable shapes ------------------------------------------------------------------

#[test]
fn a_macro_parses_to_a_callable_carrying_its_element() {
    let def = macro_def("t", "emph", 1);
    let element = Arc::clone(&def.element);
    let lang = lang(vec![def]);
    let fragment = parse(&lang, r"\emph{x}");
    let nodes = callables(&fragment);
    assert_eq!(nodes.len(), 1);
    assert!(Arc::ptr_eq(element_of(&nodes[0]), &element));
}

#[test]
fn a_macro_argument_is_parsed_as_content() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, r"\emph{x}");
    let node = callables(&fragment).into_iter().next().unwrap();
    let content = node.argument_content_nodes(0).unwrap();
    assert_eq!(content.first().and_then(|node| node.chars()), Some("x"));
}

#[test]
fn an_environment_body_delta_switches_the_mode() {
    let lang = lang(vec![env_def("t", "eq", BodyDef::new().mode(Mode::Math))]);
    let fragment = parse(&lang, r"\begin{eq}x\end{eq}");
    let node = callables(&fragment).into_iter().next().unwrap();
    let body = node.body().unwrap();
    let chars = body.iter().find(|node| node.is_chars()).unwrap();
    assert_eq!(chars.parsing_state().mode(), Mode::Math);
}

#[test]
fn a_text_mode_body_delta_exits_the_math_context() {
    let lang = lang(vec![
        env_def("t", "eq", BodyDef::new().mode(Mode::Math)),
        env_def("t", "tx", BodyDef::new().mode(Mode::Text)).modes([Mode::Math]),
    ]);
    let fragment = parse(&lang, r"\begin{eq}a\begin{tx}b\end{tx}c\end{eq}");
    let modes: Vec<Mode> = fragment
        .tree()
        .root()
        .descendants()
        .filter(|node| node.is_chars())
        .map(|node| node.parsing_state().mode())
        .collect();
    assert_eq!(modes, [Mode::Math, Mode::Text, Mode::Math]);
    assert!(fragment.diags().is_empty());
}

#[test]
fn an_environment_node_carries_its_element() {
    let def = env_def("t", "quote", BodyDef::new());
    let element = Arc::clone(&def.element);
    let lang = lang(vec![def]);
    let fragment = parse(&lang, r"\begin{quote}x\end{quote}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.callable_type(), Some(FlmCallableType::Environment));
    assert!(Arc::ptr_eq(element_of(&node), &element));
}

#[test]
fn a_specials_parses_to_a_specials_callable_carrying_its_element() {
    let element = element("t.nbsp");
    let def = ElementDef::new(
        "t",
        Vec::<&str>::new(),
        SyntaxDef::Specials { trigger: "~".into(), args: Vec::new() },
        Arc::clone(&element),
    );
    let lang = lang(vec![def]);
    let fragment = parse(&lang, "a~b");
    let nodes = callables(&fragment);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].callable_type(), Some(FlmCallableType::Specials));
    assert!(Arc::ptr_eq(element_of(&nodes[0]), &element));
}

fn scoped_lang() -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages.push(ScopedPackageDef {
        owner: "t".into(),
        name: "items".into(),
        elements: vec![macro_def("t", "item", 0)],
        docs: String::new(),
    });
    inputs.elements.push(env_def("t", "enumerate", BodyDef::new().scoped("items")));
    build(inputs)
}

#[test]
fn a_scoped_callable_resolves_inside_its_environment() {
    let lang = scoped_lang();
    let fragment = parse(&lang, r"\begin{enumerate}\item x\end{enumerate}");
    assert_eq!(callable_names(&fragment), ["enumerate", "item"]);
    assert!(fragment.diags().is_empty());
}

#[test]
fn a_scoped_callable_is_unresolvable_outside_its_environment() {
    let lang = scoped_lang();
    let fragment = parse(&lang, r"\item x");
    // Outside its environment the name is unknown, so the language layer's own
    // unknown-macro fallback answers it, with the diagnostic.
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_macro));
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
}

// --- the argument shapes -------------------------------------------------------------------

#[test]
fn an_optional_argument_is_provided_when_bracketed() {
    let args = vec![ArgDef::optional("opt"), ArgDef::mandatory("arg")];
    let lang = lang(vec![macro_with("t", "foo", args)]);
    let fragment = parse(&lang, r"\foo[x]{y}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("x"));
    assert_eq!(node.argument_content_nodes(1).unwrap().source_text(), Some("y"));
}

#[test]
fn an_optional_argument_is_absent_when_omitted() {
    let args = vec![ArgDef::optional("opt"), ArgDef::mandatory("arg")];
    let lang = lang(vec![macro_with("t", "foo", args)]);
    let fragment = parse(&lang, r"\foo{y}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(!node.arguments().unwrap().get(0).unwrap().is_provided());
    assert_eq!(node.argument_content_nodes(1).unwrap().source_text(), Some("y"));
}

#[test]
fn a_star_argument_records_the_star() {
    let lang = lang(vec![macro_with("t", "foo", vec![ArgDef::new(ArgStdKind::Star)])]);
    let fragment = parse(&lang, r"\foo* x");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("*"));
}

#[test]
fn a_star_argument_is_absent_without_the_star() {
    let lang = lang(vec![macro_with("t", "foo", vec![ArgDef::new(ArgStdKind::Star)])]);
    let fragment = parse(&lang, r"\foo x");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(!node.arguments().unwrap().get(0).unwrap().is_provided());
}

#[test]
fn a_keyvals_argument_parses_as_an_optional_group() {
    let lang = lang(vec![macro_with("t", "foo", vec![ArgDef::new(ArgStdKind::KeyVals)])]);
    let fragment = parse(&lang, r"\foo[a=b,c]");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("a=b,c"));
}

#[test]
fn a_chars_group_argument_reads_commands_as_characters() {
    let lang = lang(vec![macro_with("t", "label", vec![ArgDef::new(ArgStdKind::CharsGroup)])]);
    let fragment = parse(&lang, r"\label{a\b~c}");
    let node = callables(&fragment).into_iter().next().unwrap();
    let content = node.argument_content_nodes(0).unwrap();
    assert_eq!(content.source_text(), Some(r"a\b~c"));
    assert!(content.iter().all(|node| node.is_chars()));
    assert!(fragment.diags().is_empty());
}

#[test]
fn an_opaque_wrapped_argument_is_a_wrapper_around_the_content_it_reads() {
    let declared = |arg: ArgDef| lang(vec![macro_with("t", "key", vec![arg]), macro_def("t", "emph", 1)]);
    let plain = declared(ArgDef::mandatory("key"));
    let wrapped = declared(ArgDef::mandatory("key").with_opaque_wrapper());
    // Each content node of `\key`'s argument, by its span and its characters.
    let content = |lang: &BuiltLang, text: &str| -> Vec<(usize, usize, Option<String>)> {
        let fragment = parse(lang, text);
        let node = callables(&fragment).into_iter().next().unwrap();
        let content = node.argument_content_nodes(0).unwrap();
        content.iter().map(|node| (node.span().start(), node.span().end(), node.chars().map(String::from))).collect()
    };
    // A braced argument, whose content is inside its group, and a single
    // token, whose content is the argument's own node.
    for text in [r#"\key{a "b" \emph{c}}"#, r"\key x"] {
        let fragment = parse(&wrapped, text);
        let node = callables(&fragment).into_iter().next().unwrap();
        let region: Vec<_> = node.argument_nodes(0).unwrap().iter().collect();
        assert_eq!(region.len(), 1, "{text}");
        assert_eq!(region[0].group_type(), Some(FlmGroupType::OpaqueWrapper), "{text}");
        assert_eq!(region[0].group_delimiters(), Some(("", "")), "{text}");
        assert!(!content(&plain, text).is_empty(), "{text}");
        assert_eq!(content(&wrapped, text), content(&plain, text), "{text}");
    }
}

#[test]
fn a_tack_on_field_is_absorbed_as_a_node_of_the_field_element() {
    // Whatever a field is called, its node carries the one field element, and
    // the field's name is the element's value: a field is data, and the
    // callable that declared it is what reads it.
    let field = TackOnField::new("label", vec![ArgDef::new(ArgStdKind::CharsGroup)]);
    let args = vec![ArgDef::new(ArgStdKind::TackOn { fields: vec![field] })];
    let lang = lang(vec![macro_with("t", "thm", args)]);
    let fragment = parse(&lang, "\\thm\\label{x} y");
    let node = callables(&fragment).into_iter().next().unwrap();
    let content = node.argument_content_nodes(0).unwrap();
    let field_node = content.iter().find(|node| node.is_callable()).unwrap();
    assert_eq!(field_node.name(), Some("label"));
    assert_eq!(element_of(&field_node).name, FIELD);
    assert_eq!(field_name(&field_node).map(Name::as_str), Some("label"));
    assert_eq!(field_node.argument_content_nodes(0).unwrap().source_text(), Some("x"));
}

#[test]
fn a_tack_on_argument_is_absent_without_a_field() {
    let field = TackOnField::new("label", vec![ArgDef::new(ArgStdKind::CharsGroup)]);
    let args = vec![ArgDef::new(ArgStdKind::TackOn { fields: vec![field] })];
    let lang = lang(vec![macro_with("t", "thm", args)]);
    let fragment = parse(&lang, "\\thm y");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(!node.arguments().unwrap().get(0).unwrap().is_provided());
}

// --- an argument carrying a parser of the test's own ------------------------------------------

/// An optional `<…>` group of the content class, holding document content: a
/// parser of the test's own, around techy's optional-group parser.
#[derive(Debug)]
struct Angles(OptionalGroupArgumentParser<Flm>);

impl Angles {
    fn new() -> Angles {
        let rule = Arc::new(GroupRule {
            group_type: FlmGroupType::Content,
            open: String::from("<"),
            close: String::from(">"),
        });
        Angles(OptionalGroupArgumentParser::new(rule))
    }
}

impl ArgumentParser<Flm> for Angles {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        self.0.parse_argument(cx, spec)
    }
}

impl FlmArgumentParser for Angles {
    fn identity(&self) -> Vec<u8> {
        b"test.angles".to_vec()
    }
    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::NoShape { token: "angles" }
    }
    fn description(&self) -> String {
        String::from("an optional `<…>` group")
    }
}

/// `\note<…>`, whose one argument is read by [`Angles`], laid out inline or as
/// blocks by its content, with the scoped package `t.inner` (holding `\inner`)
/// visible inside it.
fn angles_lang() -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages.push(ScopedPackageDef {
        owner: "t".into(),
        name: "t.inner".into(),
        elements: vec![macro_def("t", "inner", 0).modes([Mode::Text, Mode::Math])],
        docs: String::new(),
    });
    let arg = ArgDef::named("text", Angles::new())
        .layout(ArgLayout::InlineOrBlocks)
        .scoped("t.inner");
    inputs.elements.push(macro_with("t", "note", vec![arg]));
    build(inputs)
}

#[test]
fn an_argument_is_read_by_the_parser_its_declaration_carries() {
    let lang = angles_lang();
    let fragment = parse(&lang, r"\note<x y> z");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.name(), Some("note"));
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("x y"));
    assert!(fragment.diags().is_empty());
}

#[test]
fn the_declared_layout_is_honored_around_a_parser_of_ones_own() {
    let lang = angles_lang();
    let fragment = parse(&lang, "\\note<one> \\note<one\n\ntwo>");
    let notes: Vec<_> =
        callables(&fragment).into_iter().filter(|node| node.name() == Some("note")).collect();
    assert_eq!(notes.len(), 2);
    assert!(!argument_holds_blocks(&notes[0], 0));
    assert!(argument_holds_blocks(&notes[1], 0));
    let mut diags = FlmDiagnostics::new();
    let tree = structure(fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags)
        .expect("the input is shallow enough to structure");
    assert!(diags.is_empty(), "{diags:?}");
    let second = tree.root().descendants().filter(|node| node.name() == Some("note")).nth(1).unwrap();
    let paragraphs = second
        .argument_content_nodes(0)
        .unwrap()
        .iter()
        .filter(|node| node.group_type() == Some(FlmGroupType::Paragraph))
        .count();
    assert_eq!(paragraphs, 2);
}

#[test]
fn a_scoped_package_is_visible_inside_an_argument_read_by_a_parser_of_ones_own() {
    let lang = angles_lang();
    let fragment = parse(&lang, r"\note<a \inner b> \inner");
    let nodes = callables(&fragment);
    assert_eq!(callable_names(&fragment), ["note", "inner", "inner"]);
    // Inside the argument `\inner` is the scoped package's; after it, unknown.
    assert_eq!(element_of(&nodes[1]).name.as_str(), "t.inner");
    assert!(Arc::ptr_eq(element_of(&nodes[2]), &lang.structural().unknown_macro));
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
}

#[test]
fn the_end_of_the_input_before_a_parser_of_ones_own_leaves_the_argument_absent() {
    let lang = angles_lang();
    for text in [r"\note", r"\note<", r"\note<a"] {
        let fragment = parse(&lang, text);
        let node = callables(&fragment).into_iter().next().unwrap();
        assert_eq!(node.name(), Some("note"), "{text}");
    }
    let fragment = parse(&lang, r"\note");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(!node.arguments().unwrap().get(0).unwrap().is_provided());
}

/// A feature declaring `\note`, with either [`Angles`] or the standard
/// optional kind as its argument.
struct NoteFeature {
    angles: bool,
}

impl FlmFeature for NoteFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t", "Test")
    }
    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let arg = if self.angles {
            ArgDef::named("text", Angles::new())
        } else {
            ArgDef::named("text", ArgStdKind::Optional)
        };
        b.element(macro_with("t", "note", vec![arg]))?;
        Ok(())
    }
}

fn digest_of(angles: bool) -> u64 {
    let feature: Arc<dyn FlmFeature> = Arc::new(NoteFeature { angles });
    build_registry([feature]).expect("the registry builds").digest()
}

#[test]
fn a_parser_of_ones_own_enters_the_digest_by_its_identity() {
    assert_eq!(digest_of(true), digest_of(true));
    assert_ne!(digest_of(true), digest_of(false));
}

#[test]
fn a_parser_of_ones_own_describes_itself_for_the_reference() {
    let arg = ArgDef::named("text", Angles::new());
    assert_eq!(arg.kind.description(), "an optional `<…>` group");
    assert!(arg.kind.tack_on_fields().is_none());
    assert_eq!(arg.kind.definition_shape(), DefinitionShape::NoShape { token: "angles" });
    assert_eq!(ArgDef::optional("x").kind.description(), "an optional `[…]` group");
}

// --- a body carrying a parser of the test's own ---------------------------------------------

/// What [`Checked`] reports once per body it reads, which is how a test sees
/// that the body parser the declaration carries is the one that ran.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "test.body-read", message = "the body of ‘{environment}’ was read by the test's parser")]
struct BodyRead {
    environment: String,
}

/// A body parser of the test's own: the ordinary body parse, then one
/// [`BodyRead`] report at the environment's `\begin`.
#[derive(Debug)]
struct Checked;

impl EnvironmentBehavior<Flm> for Checked {
    fn make_body_parser<'p>(
        &'p self,
        invocation: EnvironmentInvocation<'p, Flm>,
    ) -> Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p> {
        Box::new(CheckedBody { invocation })
    }
}

impl FlmBodyParser for Checked {
    fn identity(&self) -> Vec<u8> {
        b"test.checked".to_vec()
    }
    fn description(&self) -> String {
        String::from("content up to `\\end{name}`, then reported")
    }
}

struct CheckedBody<'p> {
    invocation: EnvironmentInvocation<'p, Flm>,
}

impl ConstructParser<Flm> for CheckedBody<'_> {
    type Output = EnvironmentBody<Flm>;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let invocation = &self.invocation;
        let mut inner = EnvironmentBodyParser::new(
            invocation.trigger_span.clone(),
            invocation.name,
            invocation.end_command_name,
            FlmGroupType::Content,
        )
        .with_invocation_name_span(invocation.name_span.clone());
        let (body, delta) = inner.parse(cx)?;
        let condition = BodyRead { environment: String::from(invocation.name) };
        cx.recover(condition, invocation.trigger_span.clone())?;
        Ok((body, delta))
    }
}

/// `checked`, a block body read by [`Checked`] with the scoped package
/// `t.inner` visible in it, and `mchecked`, the same body read in mathematics
/// mode.
fn checked_lang() -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages.push(ScopedPackageDef {
        owner: "t".into(),
        name: "t.inner".into(),
        elements: vec![macro_def("t", "inner", 0).modes([Mode::Text, Mode::Math])],
        docs: String::new(),
    });
    inputs.elements.push(env_def("t", "checked", BodyDef::new().blocks().scoped("t.inner").parser(Checked)));
    inputs.elements.push(env_def("t", "mchecked", BodyDef::new().mode(Mode::Math).parser(Checked)));
    build(inputs)
}

#[test]
fn a_body_is_read_by_the_parser_its_declaration_carries() {
    let lang = checked_lang();
    let fragment = parse(&lang, "\\begin{checked}a \\inner\n\nb\\end{checked} \\inner");
    assert_eq!(fragment.diags().conditions::<BodyRead>().count(), 1);
    // The blank line is a paragraph-break callable; the rest are named.
    let nodes: Vec<_> =
        callables(&fragment).into_iter().filter(|node| node.name() != Some("\n\n")).collect();
    let names: Vec<_> = nodes.iter().filter_map(|node| node.name()).collect();
    assert_eq!(names, ["checked", "inner", "inner"]);
    // The scoped package is visible inside the body, and nowhere else.
    assert_eq!(element_of(&nodes[1]).name.as_str(), "t.inner");
    assert!(Arc::ptr_eq(element_of(&nodes[2]), &lang.structural().unknown_macro));
    let body = body_def_of(&nodes[0]).unwrap();
    assert_eq!(body.layout, BodyLayout::Blocks);
    // The declared layout is honored: the body is laid out as blocks.
    let mut diags = FlmDiagnostics::new();
    let tree = structure(fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags)
        .expect("the input is shallow enough to structure");
    let env = tree.root().descendants().find(|node| node.name() == Some("checked")).unwrap();
    let paragraphs = env
        .body()
        .unwrap()
        .iter()
        .filter(|node| node.group_type() == Some(FlmGroupType::Paragraph))
        .count();
    assert_eq!(paragraphs, 2);
}

#[test]
fn a_body_parser_of_ones_own_reads_under_the_declared_mode() {
    let lang = checked_lang();
    let fragment = parse(&lang, r"\begin{mchecked}x^2\end{mchecked}");
    assert_eq!(fragment.diags().conditions::<BodyRead>().count(), 1);
    let node = callables(&fragment).into_iter().next().unwrap();
    let first = node.body().unwrap().iter().find(|node| node.is_chars()).unwrap();
    assert_eq!(first.parsing_state().mode(), Mode::Math);
}

#[test]
fn a_body_parser_of_ones_own_meeting_the_end_of_the_input_does_not_fail_the_parse() {
    let lang = checked_lang();
    for (text, name) in [
        (r"\begin{checked}", "checked"),
        (r"\begin{checked}a \inner", "checked"),
        (r"\begin{mchecked}x", "mchecked"),
    ] {
        let fragment = parse(&lang, text);
        assert_eq!(callable_names(&fragment).first().map(String::as_str), Some(name), "{text}");
        assert!(fragment.diags().has_errors(), "{text}");
    }
}

#[test]
fn a_verbatim_body_is_read_through_the_body_parser_field() {
    let body = BodyDef::new().verbatim();
    assert!(body.parser.is_some());
    assert!(BodyDef::new().parser.is_none());
}

// --- the parser objects in each syntax ------------------------------------------------------

/// A scoped package `t.inner` holding `\inner`, for the three syntaxes below.
fn inner_package() -> ScopedPackageDef {
    ScopedPackageDef {
        owner: "t".into(),
        name: "t.inner".into(),
        elements: vec![macro_def("t", "inner", 0).modes([Mode::Text, Mode::Math])],
        docs: String::new(),
    }
}

/// The language over `def` and [`inner_package`].
fn lang_with_inner(def: ElementDef) -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages.push(inner_package());
    inputs.elements.push(def);
    build(inputs)
}

#[test]
fn a_macro_whose_argument_carries_a_parser_parses_under_its_element_and_declaration() {
    let own = element("t.own");
    let args = vec![
        ArgDef::named("text", Angles::new()).layout(ArgLayout::InlineOrBlocks).scoped("t.inner"),
        ArgDef::named("formula", Angles::new()).mode(Mode::Math),
    ];
    let def = ElementDef::new("t", ["own"], SyntaxDef::Macro { args }, Arc::clone(&own));
    let lang = lang_with_inner(def);
    let fragment = parse(&lang, r"\own<a \inner><x^2>");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &own));
    assert_eq!(arg_layout_of(&node, 0), Some(ArgLayout::InlineOrBlocks));
    assert_eq!(arg_layout_of(&node, 1), Some(ArgLayout::Inline));
    let text = node.argument_content_nodes(0).unwrap();
    let inner = text.iter().find(|node| node.is_callable()).unwrap();
    assert_eq!(element_of(&inner).name.as_str(), "t.inner");
    let formula = node.argument_content_nodes(1).unwrap();
    assert_eq!(formula.source_text(), Some("x^2"));
    assert!(formula.iter().all(|node| node.parsing_state().mode() == Mode::Math));
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
}

#[test]
fn a_specials_whose_argument_carries_a_parser_parses_under_its_element_and_declaration() {
    let dash = element("t.dash");
    let args =
        vec![ArgDef::named("note", Angles::new()).layout(ArgLayout::InlineOrBlocks).scoped("t.inner")];
    let def = ElementDef::new(
        "t",
        Vec::<&str>::new(),
        SyntaxDef::Specials { trigger: "--".into(), args },
        Arc::clone(&dash),
    );
    let lang = lang_with_inner(def);
    let fragment = parse(&lang, r"a--<b \inner>c");
    let nodes = callables(&fragment);
    assert_eq!(nodes[0].callable_type(), Some(FlmCallableType::Specials));
    assert_eq!(nodes[0].name(), Some("--"));
    assert!(Arc::ptr_eq(element_of(&nodes[0]), &dash));
    assert_eq!(arg_layout_of(&nodes[0], 0), Some(ArgLayout::InlineOrBlocks));
    assert_eq!(nodes[0].argument_content_nodes(0).unwrap().source_text(), Some(r"b \inner"));
    assert_eq!(element_of(&nodes[1]).name.as_str(), "t.inner");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
}

#[test]
fn an_environment_whose_body_carries_a_parser_parses_under_its_element_and_declaration() {
    let own = element("t.own");
    let def = ElementDef::new(
        "t",
        ["own"],
        SyntaxDef::Environment {
            args: vec![ArgDef::named("title", Angles::new())],
            body: BodyDef::new().blocks().mode(Mode::Math).scoped("t.inner").parser(Checked),
        },
        Arc::clone(&own),
    );
    let lang = lang_with_inner(def);
    let fragment = parse(&lang, r"\begin{own}<T>x \inner\end{own}");
    assert_eq!(fragment.diags().conditions::<BodyRead>().count(), 1);
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &own));
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("T"));
    let body = body_def_of(&node).unwrap();
    assert_eq!(body.layout, BodyLayout::Blocks);
    assert_eq!(body.mode, Some(Mode::Math));
    let content = node.body().unwrap();
    assert!(content.iter().filter(|node| node.is_chars()).all(|node| node.parsing_state().mode() == Mode::Math));
    let inner = content.iter().find(|node| node.is_callable()).unwrap();
    assert_eq!(element_of(&inner).name.as_str(), "t.inner");
}

#[test]
fn body_def_of_reads_an_environments_declaration() {
    let lang = lang(vec![env_def("t", "quote", BodyDef::new().blocks())]);
    let fragment = parse(&lang, r"\begin{quote}x\end{quote}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(body_def_of(&node).unwrap().layout, BodyLayout::Blocks);
}

#[test]
fn body_def_of_is_none_for_a_macro() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, r"\emph{x}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(body_def_of(&node).is_none());
}

// --- every node's element ------------------------------------------------------------------

#[test]
fn every_node_carries_its_element() {
    let def = macro_def("t", "emph", 1);
    let macro_element = Arc::clone(&def.element);
    let lang = lang(vec![def]);
    let structural = Arc::clone(lang.structural());
    let fragment = parse(&lang, "a{b}\\emph{c}\\(d\\)%%e\n");
    let root = fragment.tree().root();
    for node in std::iter::once(root).chain(root.descendants()) {
        let expected = if node.is_callable() {
            &macro_element
        } else if node.is_chars() {
            &structural.text
        } else if node.is_comment() {
            &structural.comment
        } else if node.is_list() {
            &structural.container
        } else {
            match node.group_type() {
                Some(FlmGroupType::Content) => &structural.group,
                Some(FlmGroupType::Math(_)) => &structural.math,
                other => panic!("unexpected node shape {other:?}"),
            }
        };
        assert!(Arc::ptr_eq(element_of(&node), expected), "node {}", node.summary());
    }
}

#[test]
fn a_paragraph_break_is_a_specials_callable_carrying_the_parbreak_element() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a\n\nb");
    let nodes = callables(&fragment);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].callable_type(), Some(FlmCallableType::Specials));
    assert!(Arc::ptr_eq(element_of(&nodes[0]), &lang.structural().parbreak));
}

// --- dollar math and comments --------------------------------------------------------------

#[test]
fn dollar_math_off_reports_the_typed_condition_with_the_suggested_spelling() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "$x$");
    let conditions: Vec<&DollarMathDisabled> =
        fragment.diags().conditions::<DollarMathDisabled>().collect();
    assert!(!conditions.is_empty());
    assert_eq!(conditions[0].suggestion, DollarMathDisabled::SUGGESTION);
}

#[test]
fn dollar_math_off_leaves_no_unrefined_forbidden_char_condition() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "$");
    assert_eq!(fragment.diags().len(), 1);
    assert_eq!(fragment.diags().conditions::<ForbiddenChar>().count(), 0);
}

#[test]
fn dollar_math_on_parses_an_inline_math_group() {
    let mut inputs = LangInputs::new();
    inputs.parsing.dollar_math = true;
    let lang = build(inputs);
    let fragment = parse(&lang, "$x$");
    let group = fragment.tree().root().descendants().find(|node| node.is_group()).unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Math(MathGroupForm::Inline)));
    assert!(fragment.diags().is_empty());
}

#[test]
fn backslash_paren_math_parses_in_both_dollar_settings() {
    for dollar_math in [false, true] {
        let mut inputs = LangInputs::new();
        inputs.parsing.dollar_math = dollar_math;
        let lang = build(inputs);
        let fragment = parse(&lang, r"\(x\)");
        let group = fragment.tree().root().descendants().find(|node| node.is_group()).unwrap();
        assert_eq!(group.group_type(), Some(FlmGroupType::Math(MathGroupForm::Inline)));
    }
}

#[test]
fn the_double_percent_opens_a_comment() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a%%b\n");
    assert_eq!(fragment.tree().root().descendants().filter(|node| node.is_comment()).count(), 1);
}

/// Under the default two-character prefix a lone `%` is forbidden: it is
/// reported with the typed condition naming both unambiguous spellings, and the
/// recovery keeps the character in the text.
#[test]
fn a_single_percent_is_forbidden_under_the_default_prefix() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a%b");
    let conditions: Vec<&PercentForbidden> =
        fragment.diags().conditions::<PercentForbidden>().collect();
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0].comment_start.as_deref(), Some("%%"));
    assert_eq!(conditions[0].suggestion, PercentForbidden::suggestion(Some("%%")));
    assert_eq!(fragment.diags().conditions::<ForbiddenChar>().count(), 0);
    let root = fragment.tree().root();
    assert_eq!(root.child_count(), 1);
    assert_eq!(root.child(0).and_then(|node| node.chars()), Some("a%b"));
}

/// A comment prefix of a lone `%` — LaTeX's own — makes `%` unambiguous again,
/// and it is then not forbidden.
#[test]
fn a_single_percent_opens_a_comment_when_that_is_the_prefix() {
    let mut inputs = LangInputs::new();
    inputs.parsing.comment_prefix = String::from("%");
    let lang = build(inputs);
    let fragment = parse(&lang, "a%b
c");
    assert_eq!(fragment.tree().root().descendants().filter(|node| node.is_comment()).count(), 1);
    assert!(fragment.diags().is_empty());
}

/// With comments switched off there is no comment spelling to suggest, and the
/// condition says so.
#[test]
fn a_percent_with_comments_off_is_forbidden_and_suggests_the_escape_alone() {
    let mut inputs = LangInputs::new();
    inputs.parsing.comments = false;
    let lang = build(inputs);
    let fragment = parse(&lang, "a%b");
    let conditions: Vec<&PercentForbidden> =
        fragment.diags().conditions::<PercentForbidden>().collect();
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0].comment_start, None);
    assert_eq!(conditions[0].suggestion, PercentForbidden::suggestion(None));
}

// --- fallbacks and the unknown-environment recovery ---------------------------------------

fn fallback_lang() -> (BuiltLang, Arc<flm_core::model::element::ElementSpec>) {
    let element = element("math.symbol");
    let mut inputs = LangInputs::new();
    inputs.fallbacks.push(FallbackDef {
        owner: "math".into(),
        modes: vec![Mode::Math],
        kind: FallbackKind::Macro,
        element: Arc::clone(&element),
    });
    (build(inputs), element)
}

#[test]
fn the_math_fallback_answers_an_unknown_macro_in_math_mode() {
    let (lang, element) = fallback_lang();
    let fragment = parse(&lang, r"\(\alpha\)");
    let nodes = callables(&fragment);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name(), Some("alpha"));
    assert!(Arc::ptr_eq(element_of(&nodes[0]), &element));
    assert!(fragment.diags().is_empty());
}

/// The math feature's fallback answers in math mode only; in text mode the
/// language layer's own unknown-macro fallback answers instead, with the
/// diagnostic.
#[test]
fn the_math_fallback_does_not_answer_in_text_mode() {
    let (lang, element) = fallback_lang();
    let fragment = parse(&lang, r"\alpha");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(!Arc::ptr_eq(element_of(&node), &element));
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_macro));
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
}

/// An unknown macro resolves to the language layer's own fallback — an
/// flm-minted spec carrying
/// `flm.unknown.macro` whose invocation parser diagnoses the name and takes no
/// argument — and the node keeps the source spelling.
#[test]
fn an_unknown_macro_recovers_to_a_node_carrying_the_unknown_macro_element() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, r"a \nope b");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_macro));
    // The spelling: the name as written, and the escape character that
    // introduced it, so a backend can print `\nope` back.
    assert_eq!(node.name(), Some("nope"));
    assert!(matches!(
        node.invocation_syntax(),
        Some(InvocationSyntaxData::Macro { escape_char: '\\', .. })
    ));
    assert_eq!(node.child_count(), 0);
    assert!(node.arguments().is_none_or(|arguments| arguments.is_empty()));
    assert_eq!(fragment.diags().len(), 1, "exactly one diagnostic");
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
    assert!(fragment.diags().has_errors());
}

/// The fallback answers in expression position too (a mandatory argument taken
/// by a bare callable), so techy's raw-text recovery for an unresolvable command
/// is unreachable from document input.
#[test]
fn an_unknown_macro_in_expression_position_is_a_node_too() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, r"\emph\nope");
    let names = callable_names(&fragment);
    assert_eq!(names, ["emph", "nope"]);
    let node = callables(&fragment).into_iter().nth(1).unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_macro));
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
}

/// With no math feature loaded, math mode gets the language layer's answer too.
#[test]
fn an_unknown_macro_in_math_mode_without_a_feature_fallback_is_diagnosed() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, r"\(\nope\)");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_macro));
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
}

/// An unknown environment resolves to the language layer's own fallback — an
/// flm-minted spec carrying
/// `flm.unknown.environment` whose body parser diagnoses the name and parses the
/// body as block content.
#[test]
fn an_unknown_environment_recovers_to_an_flm_minted_node_carrying_its_element() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, r"\begin{nope}x\end{nope}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_environment));
    assert_eq!(body_def_of(&node).map(|body| body.layout), Some(BodyLayout::Blocks));
    let body = node.body().unwrap();
    assert_eq!(body.iter().find(|node| node.is_chars()).and_then(|node| node.chars()), Some("x"));
    assert_eq!(fragment.diags().conditions::<UnknownEnvironment>().count(), 1);
    assert!(fragment.diags().has_errors());
}

#[test]
fn an_unknown_environment_is_diagnosed_in_math_mode_too() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, r"\(\begin{nope}x\end{nope}\)");
    assert_eq!(fragment.diags().conditions::<UnknownEnvironment>().count(), 1);
}

#[test]
fn the_symbols_table_maps_a_name_to_its_owner() {
    let lang = lang(vec![macro_def("alpha", "foo", 0)]);
    let symbol = flm_core::lang::mint::Symbol {
        kind: flm_core::lang::mint::CallableKind::Macro,
        name: "foo".into(),
    };
    assert_eq!(lang.symbols().get(&symbol).map(|owner| owner.as_str()), Some("alpha"));
}
