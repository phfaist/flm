//! The projection of `flm_core::dump::project`: the shape of every node kind,
//! arguments by name, absent arguments, spans on request, payloads, node and
//! tree references, diagnostics and tables — and the goldens under
//! `tests/goldens/dump/` (the parse layer, a recovered parse, and the staged
//! shapes a parse never yields).

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::dump::diag::ExpectDiag;
use flm_core::dump::golden::assert_json_golden;
use flm_core::dump::project::{
    node_ref_value, project_diagnostic, project_diagnostics, project_node, project_table,
    project_tree, tree_ref_value, ProjectOptions, Projector,
};
use flm_core::lang::mint::{
    ArgDef, ArgStdKind, BodyDef, ElementDef, FlmPresentationSpec, SyntaxDef, TackOnField,
};
use flm_core::lang::{
    parse_fragment, BuiltLang, Flm, FlmGroupType, FlmNodeExt, FlmOrigin, ParseOptions,
    PresPayload, TextSourceMap, TextSourcePiece,
};
use techy::core::node::{GroupData, NodeKind, NodeTree, ParsedArguments, ParsedSlots};
use techy::error::DiagnosticInfo;
use techy::latexlike::UnknownEnvironment;
use techy::serialize::SerialValue;
use techy::source::{Source, SourceSpan};

use common::dump::{golden, Site};
use common::{callables, element, env_def, lang, macro_def, macro_with, parse, source};

fn s(text: &str) -> SerialValue {
    SerialValue::Str(String::from(text))
}

fn map(entries: Vec<(&str, SerialValue)>) -> SerialValue {
    SerialValue::Map(entries.into_iter().map(|(key, value)| (String::from(key), value)).collect())
}

fn list(items: Vec<SerialValue>) -> SerialValue {
    SerialValue::List(items)
}

fn chars(text: &str) -> SerialValue {
    map(vec![("chars", s(text))])
}

fn tree_of(fragment: &Fragment) -> SerialValue {
    project_tree(fragment.tree(), &ProjectOptions::new())
}

// --- the shape, node kind by node kind ----------------------------------------------------------

#[test]
fn a_text_run_projects_as_its_characters() {
    let lang = lang(Vec::new());
    assert_eq!(tree_of(&parse(&lang, "ab")), list(vec![chars("ab")]));
}

#[test]
fn a_macro_projects_its_element_name_and_arguments_by_name() {
    let lang = lang(vec![macro_with("t", "emph", vec![ArgDef::mandatory("body")])]);
    assert_eq!(
        tree_of(&parse(&lang, r"\emph{w}")),
        list(vec![map(vec![
            ("elem", s("t.emph")),
            ("name", s("emph")),
            ("args", map(vec![("body", list(vec![chars("w")]))])),
        ])])
    );
}

#[test]
fn an_unnamed_argument_is_keyed_by_its_index() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let node = project_node(&callables(&parse(&lang, r"\emph{w}"))[0], &ProjectOptions::new());
    assert_eq!(
        node,
        map(vec![
            ("elem", s("t.emph")),
            ("name", s("emph")),
            ("args", map(vec![("0", list(vec![chars("w")]))])),
        ])
    );
}

#[test]
fn an_absent_optional_argument_is_omitted_and_a_provided_one_shown() {
    let lang = lang(vec![macro_with("t", "foo", vec![ArgDef::optional("opt")])]);
    let absent = project_node(&callables(&parse(&lang, r"\foo x"))[0], &ProjectOptions::new());
    assert_eq!(absent, map(vec![("elem", s("t.foo")), ("name", s("foo"))]));
    let provided = project_node(&callables(&parse(&lang, r"\foo[] x"))[0], &ProjectOptions::new());
    assert_eq!(
        provided,
        map(vec![
            ("elem", s("t.foo")),
            ("name", s("foo")),
            ("args", map(vec![("opt", list(Vec::new()))])),
        ])
    );
}

#[test]
fn an_environment_projects_its_body() {
    let lang = lang(vec![env_def("t", "quote", BodyDef::new())]);
    assert_eq!(
        tree_of(&parse(&lang, r"\begin{quote}q\end{quote}")),
        list(vec![map(vec![
            ("elem", s("t.quote")),
            ("name", s("quote")),
            ("body", list(vec![chars("q")])),
        ])])
    );
}

#[test]
fn a_math_group_projects_its_element_and_form() {
    let lang = lang(Vec::new());
    assert_eq!(
        tree_of(&parse(&lang, r"\[x\]")),
        list(vec![map(vec![
            ("elem", s("flm.math")),
            ("form", s("display")),
            ("children", list(vec![chars("x")])),
        ])])
    );
}

#[test]
fn a_content_group_projects_its_children() {
    let lang = lang(Vec::new());
    assert_eq!(
        tree_of(&parse(&lang, "{x}")),
        list(vec![map(vec![("elem", s("flm.group")), ("children", list(vec![chars("x")]))])])
    );
}

#[test]
fn a_comment_projects_as_a_comment_with_its_text() {
    let lang = lang(Vec::new());
    assert_eq!(
        tree_of(&parse(&lang, "%% note")),
        list(vec![map(vec![("kind", s("comment")), ("text", s(" note"))])])
    );
}

#[test]
fn a_specials_projects_its_element_alone() {
    let def = ElementDef::new(
        "t",
        Vec::<&str>::new(),
        SyntaxDef::Specials { trigger: "~".into(), args: Vec::new() },
        element("t.nbsp"),
    );
    let lang = lang(vec![def]);
    assert_eq!(
        tree_of(&parse(&lang, "a~b")),
        list(vec![chars("a"), map(vec![("elem", s("t.nbsp"))]), chars("b")])
    );
}

#[test]
fn a_nested_list_projects_as_a_list_kind() {
    let lang = lang(Vec::new());
    let mut site = Site::new(&lang);
    let inner = site.stage(NodeKind::chars("x"), Vec::new()).unwrap();
    let nested = site.stage(NodeKind::list(), vec![inner]).unwrap();
    let tree = site.finish(vec![nested]);
    assert_eq!(
        project_tree(&tree, &ProjectOptions::new()),
        list(vec![map(vec![("kind", s("list")), ("children", list(vec![chars("x")]))])])
    );
}

#[test]
fn a_root_that_is_not_a_list_projects_as_a_one_element_list() {
    let lang = lang(Vec::new());
    let mut site = Site::new(&lang);
    let root = site.stage(NodeKind::chars("x"), Vec::new()).unwrap();
    let tree = Arc::new(site.builder.finish(root).unwrap());
    assert_eq!(project_tree(&tree, &ProjectOptions::new()), list(vec![chars("x")]));
}

// --- the options ------------------------------------------------------------------------------

#[test]
fn spans_appear_only_on_request() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, "a\n\\emph{w}");
    let node = callables(&fragment)[0];
    assert_eq!(
        project_node(&node, &ProjectOptions::new().with_spans()),
        map(vec![
            ("elem", s("t.emph")),
            ("name", s("emph")),
            (
                "args",
                map(vec![("0", list(vec![map(vec![("chars", s("w")), ("span", s("2:7-2:8"))])]))]),
            ),
            ("span", s("2:1-2:9")),
        ])
    );
    assert!(!format!("{:?}", project_node(&node, &ProjectOptions::new())).contains("span"));
}

#[test]
fn an_empty_span_projects_as_a_position() {
    let text = source("ab");
    let projector = Projector::new(ProjectOptions::new());
    assert_eq!(projector.span(&SourceSpan::new(&text, 1..1)), "1:2");
}

#[test]
fn a_span_projects_as_its_start_and_its_exclusive_end() {
    let text = source("ab");
    let projector = Projector::new(ProjectOptions::new());
    assert_eq!(projector.span(&SourceSpan::new(&text, 0..2)), "1:1-1:3");
}

/// A payload of a test presentation element.
#[derive(Debug)]
struct Style(&'static str);

impl PresPayload for Style {
    fn project(&self) -> SerialValue {
        map(vec![("style", s(self.0))])
    }
}

fn presentation_tree(lang: &BuiltLang) -> Arc<NodeTree<Flm>> {
    let pres = element("t.pres");
    let spec = Arc::new(FlmPresentationSpec::new(Arc::clone(&pres)));
    let mut site = Site::new(lang);
    let data = spec.callable_data(ParsedArguments::empty(), ParsedSlots::empty());
    let ext = FlmNodeExt::new(pres).with_payload(Arc::new(Style("bold")));
    let node = site.stage_with_ext(NodeKind::callable(data), Vec::new(), ext).unwrap();
    site.finish(vec![node])
}

#[test]
fn a_presentation_payload_projects_through_its_project_method() {
    let lang = lang(Vec::new());
    let tree = presentation_tree(&lang);
    assert_eq!(
        project_tree(&tree, &ProjectOptions::new()),
        list(vec![map(vec![("elem", s("t.pres")), ("payload", map(vec![("style", s("bold"))]))])])
    );
}

#[test]
fn a_document_role_and_an_accessible_name_project_by_name() {
    use flm_core::model::names::Name;
    use flm_core::stdcore::backends::pres::{DocRole, LinkPayload};

    let lang = lang(Vec::new());
    let pres = element("t.pres");
    let spec = Arc::new(FlmPresentationSpec::new(Arc::clone(&pres)));
    let mut site = Site::new(&lang);
    let data = spec.callable_data(ParsedArguments::empty(), ParsedSlots::empty());
    let payload = LinkPayload {
        href: String::from("#fn-1"),
        kind: Name::from("xnote"),
        doc_role: Some(DocRole::NoteRef),
        label: Some(String::from("Footnote 1")),
        shows_target: false,
    };
    let ext = FlmNodeExt::new(pres).with_payload(Arc::new(payload));
    let node = site.stage_with_ext(NodeKind::callable(data), Vec::new(), ext).unwrap();
    let tree = site.finish(vec![node]);
    assert_eq!(
        project_tree(&tree, &ProjectOptions::new()),
        list(vec![map(vec![
            ("elem", s("t.pres")),
            (
                "payload",
                map(vec![
                    ("href", s("#fn-1")),
                    ("kind", s("xnote")),
                    ("doc_role", s("noteref")),
                    ("label", s("Footnote 1")),
                ])
            ),
        ])])
    );
}

#[test]
fn payloads_can_be_switched_off() {
    let lang = lang(Vec::new());
    let tree = presentation_tree(&lang);
    assert_eq!(
        project_tree(&tree, &ProjectOptions::new().without_payloads()),
        list(vec![map(vec![("elem", s("t.pres"))])])
    );
}

#[test]
fn a_source_map_projects_on_request() {
    let lang = lang(Vec::new());
    let text = source("a  b");
    let source_map = TextSourceMap::new(vec![
        TextSourcePiece { out_len: 1, span: SourceSpan::new(&text, 0..1) },
        TextSourcePiece { out_len: 1, span: SourceSpan::new(&text, 1..3) },
        TextSourcePiece { out_len: 1, span: SourceSpan::new(&text, 3..4) },
    ]);
    let mut site = Site::new(&lang);
    let ext = FlmNodeExt::new(Arc::clone(&lang.structural().text))
        .with_source_map(Arc::new(source_map));
    let node = site.stage_with_ext(NodeKind::chars("a b"), Vec::new(), ext).unwrap();
    let tree = site.finish(vec![node]);
    assert_eq!(project_tree(&tree, &ProjectOptions::new()), list(vec![chars("a b")]));
    assert_eq!(
        project_tree(&tree, &ProjectOptions::new().with_source_maps()),
        list(vec![map(vec![
            ("chars", s("a b")),
            (
                "map",
                list(vec![
                    map(vec![("len", SerialValue::Int(1)), ("span", s("1:1-1:2"))]),
                    map(vec![("len", SerialValue::Int(1)), ("span", s("1:2-1:4"))]),
                    map(vec![("len", SerialValue::Int(1)), ("span", s("1:4-1:5"))]),
                ]),
            ),
        ])])
    );
}

// --- references and tables ----------------------------------------------------------------------

#[test]
fn a_node_reference_is_the_element_at_its_line_and_column() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, "a\n \\emph{w}");
    let node = callables(&fragment)[0];
    assert_eq!(node_ref_value(fragment.tree(), node.id(), &ProjectOptions::new()), s("t.emph@2:2"));
}

#[test]
fn a_tree_reference_is_the_tree_ids_display() {
    assert_eq!(tree_ref_value(TreeId::tree(0)), s("tree#0"));
    assert_eq!(tree_ref_value(TreeId::value(3)), s("value#3"));
}

#[test]
fn a_table_projects_under_its_name() {
    let entries = map(vec![("t.emph@1:1", SerialValue::Int(1))]);
    assert_eq!(
        project_table("flm.numbering", entries.clone()),
        map(vec![("flm.numbering", entries)])
    );
}

// --- diagnostics -----------------------------------------------------------------------------

#[test]
fn a_diagnostic_projects_its_identifier_severity_span_and_data() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a\n$b$");
    let diagnostic = fragment.diags().iter().next().expect("the dollar diagnostic");
    let projection = project_diagnostic(diagnostic, &ProjectOptions::new());
    assert_eq!(
        projection,
        map(vec![
            ("id", s("flm.lang.dollar-math-disabled")),
            ("severity", s("error")),
            ("span", s("2:1-2:2")),
            ("data", map(vec![("suggestion", s(flm_core::lang::DollarMathDisabled::SUGGESTION))])),
        ])
    );
}

#[test]
fn a_diagnostic_of_a_named_source_projects_its_file() {
    let lang = lang(Vec::new());
    let text = Arc::new(Source::new("$x$").with_origin(FlmOrigin::new("a.flm")));
    let fragment =
        parse_fragment(&lang, text, FragmentLayout::default(), &ParseOptions::default()).unwrap();
    let projection = project_diagnostics(fragment.diags().iter(), &ProjectOptions::new());
    let first = match &projection {
        SerialValue::List(items) => items[0].clone(),
        other => panic!("a list, not {other:?}"),
    };
    let file = match &first {
        SerialValue::Map(entries) => {
            entries.iter().find(|(key, _)| key == "file").map(|(_, value)| value.clone())
        }
        other => panic!("a map, not {other:?}"),
    };
    assert_eq!(file, Some(s("a.flm")));
}

// --- the parse-layer goldens -------------------------------------------------------------------

fn parse_layer_defs() -> Vec<ElementDef> {
    let label = TackOnField::new("label", vec![ArgDef::named("label", ArgStdKind::CharsGroup)]);
    let fields = ArgDef::named("fields", ArgStdKind::TackOn { fields: vec![label] });
    let verbatim = ArgDef::named("code", ArgStdKind::Verbatim { delimiters: None });
    vec![
        macro_with("t", "section", vec![ArgDef::mandatory("title"), fields]),
        macro_with("t", "emph", vec![ArgDef::mandatory("body")]),
        macro_with("t", "verb", vec![verbatim]),
        env_def("t", "quote", BodyDef::new().blocks()),
        env_def("t", "code", BodyDef::new().verbatim()),
        ElementDef::new(
            "t",
            Vec::<&str>::new(),
            SyntaxDef::Specials { trigger: "~".into(), args: Vec::new() },
            element("t.nbsp"),
        ),
    ]
}

/// A verbatim *argument* (`\\verb|raw|`) projects as its content alone; the
/// verbatim *environment* (`code`) is what shows the verbatim wrap — the
/// `flm.verbatim` group the parse contract guarantees — in the golden.
const PARSE_LAYER: &str = "\\section{Intro}\\label{sec:intro}\n\n\
    Hello \\emph{world}, \\(x^2\\) and \\verb|raw| here~there.\n\
    \\begin{quote}\n  Quoted. %% a note\n\\end{quote}\n\
    \\begin{code}\n\\emph{x} % raw\n\\end{code}\n";

#[test]
fn the_parse_layer_projection_matches_its_golden() {
    let lang = lang(parse_layer_defs());
    let fragment = parse(&lang, PARSE_LAYER);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_json_golden(golden("parse_layer.json"), &tree_of(&fragment));
}

#[test]
fn the_parse_layer_projection_with_spans_matches_its_golden() {
    let lang = lang(parse_layer_defs());
    let fragment = parse(&lang, PARSE_LAYER);
    assert_json_golden(
        golden("parse_layer_spans.json"),
        &project_tree(fragment.tree(), &ProjectOptions::new().with_spans()),
    );
}

const RECOVERY: &str = "a $b$ \\begin{nope}x\\end{nope} \\unknown c";

#[test]
fn a_recovered_parse_projects_its_tree_and_its_own_diagnostics() {
    // The golden pins flm-owned data only: techy's diagnostics carry rendered
    // prose in their data (the provider stack searched), which is techy's to
    // change — they are asserted by identifier and position in the next test.
    let lang = lang(parse_layer_defs());
    let fragment = parse(&lang, RECOVERY);
    let projector = Projector::new(ProjectOptions::new().with_spans());
    let own =
        fragment.diags().iter().filter(|diagnostic| diagnostic.identifier().starts_with("flm."));
    let value = map(vec![
        ("tree", projector.tree(fragment.tree())),
        ("diagnostics", projector.diagnostics(own)),
    ]);
    assert_json_golden(golden("recovery.json"), &value);
}

#[test]
fn a_recovered_parse_reports_techys_diagnostics_at_their_positions() {
    let lang = lang(parse_layer_defs());
    let fragment = parse(&lang, RECOVERY);
    fragment.diags().expect(UnknownEnvironment::IDENTIFIER).at("nope").once();
    fragment.diags().expect("core.specs.unresolvable-command").at("\\unknown ").once();
}

// --- the staged shapes: what a parse never yields ------------------------------------------

#[test]
fn the_staged_shapes_projection_matches_its_golden() {
    // A paragraph group (block structuring's), a nested list node (a rewrite
    // grouping several nodes where one is expected) and a presentation node with
    // its payload (the lowering's): the shapes the parse-layer golden cannot show.
    let lang = lang(Vec::new());
    let mut site = Site::new(&lang);
    let text = site.stage(NodeKind::chars("a "), Vec::new()).unwrap();
    let inner = site.stage(NodeKind::chars("b"), Vec::new()).unwrap();
    let nested = site.stage(NodeKind::list(), vec![inner]).unwrap();
    let paragraph = GroupData::new(FlmGroupType::Paragraph, "", "");
    let paragraph = site.stage(NodeKind::group(paragraph), vec![text, nested]).unwrap();
    let pres = element("t.pres");
    let spec = Arc::new(FlmPresentationSpec::new(Arc::clone(&pres)));
    let data = spec.callable_data(ParsedArguments::empty(), ParsedSlots::empty());
    let ext = FlmNodeExt::new(pres).with_payload(Arc::new(Style("bold")));
    let styled = site.stage_with_ext(NodeKind::callable(data), Vec::new(), ext).unwrap();
    let tree = site.finish(vec![paragraph, styled]);
    assert_json_golden(golden("staged.json"), &project_tree(&tree, &ProjectOptions::new()));
}
