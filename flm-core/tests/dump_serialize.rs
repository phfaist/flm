//! Serialization of the FLM dialect (`flm_core::dump::session` over
//! `flm_core::lang::serialize`): the serialized form of each of the dialect's
//! types, and round trips through the writing and reading sessions — equal
//! projections, and shared-object identity for specs, states, providers,
//! elements and the structural set.

mod common;

use std::cell::Cell;
use std::sync::Arc;

use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::model::element::element_of;
use flm_core::dump::diag::ExpectDiag;
use flm_core::dump::json::segment_line;
use flm_core::dump::project::{project_diagnostics, project_tree, ProjectOptions};
use flm_core::dump::session::{self, AmbiguousElement, PayloadReaders, FRAGMENTS_TABLE};
use flm_core::lang::mint::specs::element_of_spec;
use flm_core::lang::mint::{
    ArgDef, ArgStdKind, ElementDef, FlmCallable, FlmPresentationSpec, FlmSynthesizedSpec, SyntaxDef,
    TackOnField, FIELDS_PACKAGE_NAME,
};
use flm_core::lang::{
    parse_fragment, BuiltLang, Flm, FlmCallableType, FlmDiagnostics, FlmGroupType, FlmNodeExt,
    FlmOrigin, FlmParsing, FlmStateExt, ParseOptions, PresPayload, TemplateCommandRule,
    TemplateInteriorRules, TextSourceMap, TextSourcePiece, RestrictedInterior,
};
use techy::core::constructs::UnresolvableCommand;
use techy::core::node::{CallableData, NodeKind, ParsedArguments, ParsedSlots};
use techy::core::specs::CallableSpec;
use techy::error::{DiagnosticInfo, InconsistentDiagnosticCounts};
use techy::latexlike::{
    InvocationSyntaxData, MacroSpec, MathGroupForm, Mode, ParagraphBreakSpec, UnknownEnvironment,
};
use techy::serialize::{
    DeserializeError, SerialValue, SerializeError, SpecIndex, StandardTableInterning,
};
use techy::source::SourceSpan;

use common::dump::{
    edit_entries, elements_of, field, field_mut, index_value, read_fragment_from, read_tree,
    round_trip, round_trip_value, round_trip_value_with, try_read_tree, write, write_tree, Site,
};
use common::{callables, element, env_def, lang, macro_def, macro_with, parse, source};

fn s(text: &str) -> SerialValue {
    SerialValue::Str(String::from(text))
}

fn map(entries: Vec<(&str, SerialValue)>) -> SerialValue {
    SerialValue::Map(entries.into_iter().map(|(key, value)| (String::from(key), value)).collect())
}

fn keys(value: &SerialValue) -> Vec<&str> {
    match value {
        SerialValue::Map(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
        other => panic!("expected a map, found {other:?}"),
    }
}

fn key<'v>(value: &'v SerialValue, name: &str) -> Option<&'v SerialValue> {
    match value {
        SerialValue::Map(entries) => {
            entries.iter().find(|(key, _)| key == name).map(|(_, value)| value)
        }
        other => panic!("expected a map, found {other:?}"),
    }
}

fn no_readers() -> PayloadReaders {
    PayloadReaders::new()
}

/// The innermost error behind techy's location wrappers (the entry and the node
/// a failure happened in).
fn root_cause(error: &DeserializeError) -> &DeserializeError {
    let mut current = error;
    loop {
        match current {
            DeserializeError::InEntry { cause, .. } | DeserializeError::InNode { cause, .. } => {
                current = &**cause;
            }
            other => return other,
        }
    }
}

/// The element-name ambiguity a read error reports as its cause, if that is
/// what it reports.
fn ambiguity(error: &DeserializeError) -> Option<&AmbiguousElement> {
    std::error::Error::source(root_cause(error))?.downcast_ref::<AmbiguousElement>()
}

// --- the serialized forms of the dialect's types ----------------------------------------------

#[test]
fn a_group_type_serializes_to_its_documented_form() {
    let lang = lang(Vec::new());
    let value = FlmGroupType::Math(MathGroupForm::Display);
    let (written, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), value);
    assert_eq!(written, map(vec![("math", s("display"))]));
    assert_eq!(read.unwrap(), value);
}

#[test]
fn a_callable_type_serializes_to_its_documented_form() {
    let lang = lang(Vec::new());
    let value = FlmCallableType::Presentation;
    let (written, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), value);
    assert_eq!(written, s("presentation"));
    assert_eq!(read.unwrap(), value);
}

#[test]
fn an_origin_serializes_to_its_canonical_path() {
    let lang = lang(Vec::new());
    let (named, read) =
        round_trip_value(&lang, Vec::new(), PayloadReaders::new(), FlmOrigin::new("book/ch1.flm"));
    assert_eq!(named, map(vec![("canonical_path", s("book/ch1.flm"))]));
    assert_eq!(read.unwrap(), FlmOrigin::new("book/ch1.flm"));
}

#[test]
fn an_anonymous_origin_serializes_to_an_empty_map() {
    let lang = lang(Vec::new());
    let (anonymous, read) = round_trip_value(&lang, Vec::new(), no_readers(), FlmOrigin::none());
    assert_eq!(anonymous, map(Vec::new()));
    assert_eq!(read.unwrap(), FlmOrigin::none());
}

#[test]
fn a_text_source_map_serializes_its_pieces_with_their_spans() {
    let lang = lang(Vec::new());
    let text = source("ab  cd");
    let value = TextSourceMap::new(vec![
        TextSourcePiece { out_len: 2, span: SourceSpan::new(&text, 0..2) },
        TextSourcePiece { out_len: 1, span: SourceSpan::new(&text, 2..4) },
    ]);
    let (written, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), value);
    let pieces = match key(&written, "pieces") {
        Some(SerialValue::List(pieces)) => pieces.clone(),
        other => panic!("expected the pieces, found {other:?}"),
    };
    assert_eq!(keys(&pieces[0]), ["len", "span"]);
    assert_eq!(key(&pieces[0], "len"), Some(&SerialValue::Int(2)));
    let read = read.unwrap();
    let ranges: Vec<_> =
        read.pieces().iter().map(|piece| (piece.out_len, piece.span.range())).collect();
    assert_eq!(ranges, [(2, 0..2), (1, 2..4)]);
    assert_eq!(read.pieces()[1].span.content(), "  ");
}

#[test]
fn the_document_restricted_interior_serializes_to_its_documented_form() {
    let lang = lang(Vec::new());
    let (document, read) =
        round_trip_value(&lang, Vec::new(), PayloadReaders::new(), RestrictedInterior::Document);
    assert_eq!(document, s("document"));
    assert_eq!(read.unwrap(), RestrictedInterior::Document);
}

#[test]
fn a_template_restricted_interior_serializes_to_its_documented_form() {
    let lang = lang(Vec::new());
    let template = RestrictedInterior::Template(TemplateInteriorRules {
        commands: vec![TemplateCommandRule { escape_char: '#', name_chars: String::new() }],
        specials: false,
    });
    let (written, read) = round_trip_value(&lang, Vec::new(), no_readers(), template.clone());
    assert_eq!(
        written,
        map(vec![(
            "template",
            map(vec![
                (
                    "commands",
                    SerialValue::List(vec![map(vec![
                        ("escape_char", s("#")),
                        ("name_chars", s("")),
                    ])]),
                ),
                ("specials", SerialValue::Bool(false)),
            ]),
        )])
    );
    assert_eq!(read.unwrap(), template);
}

#[test]
fn a_state_ext_lists_the_structural_names_and_keeps_its_policy_and_parsing_settings() {
    let lang = lang(Vec::new());
    let parsing = FlmParsing { comment_prefix: String::from(";;"), ..FlmParsing::default() };
    let value = FlmStateExt::new(Arc::clone(lang.structural()))
        .with_restricted_interior(RestrictedInterior::Template(TemplateInteriorRules::default()))
        .with_parsing(parsing.clone());
    let (written, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), value);
    assert_eq!(keys(&written), ["structural", "restricted_interior", "parsing"]);
    let names: Vec<SerialValue> =
        flm_core::lang::structural::NAMES.iter().map(|name| s(name)).collect();
    assert_eq!(key(&written, "structural"), Some(&SerialValue::List(names)));
    let read = read.unwrap();
    assert!(read.restricted_interior.is_template());
    assert_eq!(*read.parsing, parsing);
}

#[test]
fn a_state_ext_reads_back_the_reading_languages_shared_structural_set() {
    let lang = lang(Vec::new());
    // Written with the crate's default set, which is not the language's instance.
    let value = FlmStateExt::default();
    assert!(!Arc::ptr_eq(&value.structural, lang.structural()));
    let (_, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), value);
    assert!(Arc::ptr_eq(&read.unwrap().structural, lang.structural()));
}

#[test]
fn a_node_ext_writes_the_element_name_and_the_spec_carrying_it() {
    let def = macro_def("t", "emph", 1);
    let element = Arc::clone(&def.element);
    let lang = lang(vec![def]);
    let fragment = parse(&lang, r"\emph{x}");
    let ext = callables(&fragment)[0].ext().clone();
    let (written, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), ext);
    assert_eq!(keys(&written), ["element", "spec"]);
    assert_eq!(key(&written, "element"), Some(&s("t.emph")));
    assert!(matches!(key(&written, "spec"), Some(SerialValue::Index { .. })));
    assert!(Arc::ptr_eq(&read.unwrap().element, &element));
}

#[test]
fn a_node_ext_of_a_structural_element_writes_the_name_alone() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "x");
    let ext = fragment.tree().root().child(0).unwrap().ext().clone();
    let (written, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), ext);
    assert_eq!(keys(&written), ["element"]);
    assert!(Arc::ptr_eq(&read.unwrap().element, &lang.structural().text));
}

#[test]
fn a_node_ext_of_an_element_the_resolver_does_not_know_is_refused() {
    let lang = lang(Vec::new());
    let ext = FlmNodeExt::new(element("t.ghost"));
    let (_, read) = round_trip_value(&lang, Vec::new(), PayloadReaders::new(), ext);
    assert!(read.is_err());
}

#[test]
fn a_node_ext_of_an_element_the_resolver_knows_resolves_by_name() {
    let lang = lang(Vec::new());
    let known = element("t.known");
    let ext = FlmNodeExt::new(Arc::clone(&known));
    let (_, read) = round_trip_value(&lang, vec![Arc::clone(&known)], PayloadReaders::new(), ext);
    assert!(Arc::ptr_eq(&read.unwrap().element, &known));
}

#[test]
fn a_node_ext_of_a_name_several_values_share_is_refused_as_ambiguous() {
    // One value per name is the rule for the elements that travel by name alone;
    // a name with two values is an error naming each value's owner (the host,
    // for a table), never a guess.
    let lang = lang(Vec::new());
    let (first, second) = (element("t.dup"), element("t.dup"));
    let ext = FlmNodeExt::new(Arc::clone(&first));
    let (_, read) = round_trip_value(&lang, vec![first, second], no_readers(), ext);
    let error = read.unwrap_err();
    let host = flm_core::model::names::FeatureName::from(flm_core::core::registry::HOST_FEATURE);
    let expected = AmbiguousElement { name: "t.dup".into(), owners: vec![host; 2] };
    assert_eq!(ambiguity(&error), Some(&expected), "{error:?}");
}

/// A payload of a test presentation element.
#[derive(Debug, PartialEq, Eq)]
struct Style {
    style: String,
}

impl PresPayload for Style {
    fn project(&self) -> SerialValue {
        map(vec![("style", s(&self.style))])
    }
}

fn style_readers() -> PayloadReaders {
    let mut readers = PayloadReaders::new();
    readers.register("t.pres", |value: &SerialValue, _cx: &mut _| {
        match key(value, "style") {
            Some(SerialValue::Str(style)) => {
                Ok(Arc::new(Style { style: style.clone() }) as Arc<dyn PresPayload>)
            }
            other => Err(DeserializeError::failed(format!("a style payload, not {other:?}"))),
        }
    });
    readers
}

#[test]
fn a_payload_round_trips_through_its_registered_reader() {
    let lang = lang(Vec::new());
    let pres = element("t.pres");
    let ext = FlmNodeExt::new(Arc::clone(&pres))
        .with_payload(Arc::new(Style { style: String::from("bold") }));
    let (written, read) = round_trip_value(&lang, vec![pres], style_readers(), ext);
    assert_eq!(key(&written, "payload"), Some(&map(vec![("style", s("bold"))])));
    let payload = read.unwrap().payload.expect("the payload reads back");
    assert_eq!(payload.downcast_ref::<Style>(), Some(&Style { style: String::from("bold") }));
}

#[test]
fn a_payload_of_an_element_without_a_reader_is_refused() {
    let lang = lang(Vec::new());
    let pres = element("t.pres");
    let ext = FlmNodeExt::new(Arc::clone(&pres))
        .with_payload(Arc::new(Style { style: String::from("bold") }));
    let (_, read) = round_trip_value(&lang, vec![pres], PayloadReaders::new(), ext);
    assert!(read.is_err());
}

// --- fragments through the sessions ---------------------------------------------------------

fn rich_defs() -> Vec<ElementDef> {
    vec![
        macro_with("t", "emph", vec![ArgDef::mandatory("body")]),
        env_def("t", "quote", flm_core::lang::mint::BodyDef::new().blocks()),
        ElementDef::new(
            "t",
            Vec::<&str>::new(),
            SyntaxDef::Specials { trigger: "~".into(), args: Vec::new() },
            element("t.nbsp"),
        ),
    ]
}

const RICH: &str = "Hello \\emph{world}~x \\(y^2\\) % note\n\n\\begin{quote}q\\end{quote}";

#[test]
fn a_parsed_fragment_round_trips_with_an_equal_projection() {
    let defs = rich_defs();
    let elements = elements_of(&defs);
    let lang = lang(defs);
    let live = Arc::new(parse(&lang, RICH));
    let read = round_trip(&lang, &live, elements);
    let options = ProjectOptions::new().with_spans();
    assert_eq!(project_tree(read.tree(), &options), project_tree(live.tree(), &options));
}

#[test]
fn read_back_nodes_share_their_specs_with_the_language() {
    let defs = rich_defs();
    let elements = elements_of(&defs);
    let lang = lang(defs);
    let live = Arc::new(parse(&lang, "\\emph{a}~b \\begin{quote}q\\end{quote}"));
    let read = round_trip(&lang, &live, elements);
    let live_nodes = callables(&live);
    let read_nodes = callables(&read);
    assert_eq!(live_nodes.len(), 3, "the macro, the specials, the environment");
    for (live, read) in live_nodes.iter().zip(&read_nodes) {
        assert!(Arc::ptr_eq(live.spec().unwrap(), read.spec().unwrap()), "{}", live.summary());
    }
}

#[test]
fn a_paragraph_breaks_spec_is_rebuilt_but_its_element_is_the_languages() {
    // techy's paragraph-break spec has a self-contained form: an equivalent spec
    // is rebuilt on read, so its identity is not preserved — the element is.
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, "a\n\nb"));
    let read = round_trip(&lang, &live, Vec::new());
    let node = callables(&read)[0];
    assert!(!Arc::ptr_eq(node.spec().unwrap(), callables(&live)[0].spec().unwrap()));
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().parbreak));
}

#[test]
fn read_back_nodes_share_one_parsing_state() {
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, "a %% c\nb"));
    let read = round_trip(&lang, &live, Vec::new());
    let root = read.tree().root();
    let (first, last) = (root.child(0).unwrap(), root.child(2).unwrap());
    assert!(first.is_chars() && last.is_chars());
    assert!(Arc::ptr_eq(first.parsing_state(), last.parsing_state()));
}

#[test]
fn read_back_states_share_the_languages_providers() {
    let lang = lang(vec![macro_def("t", "x", 0)]);
    let live = Arc::new(parse(&lang, "a"));
    let read = round_trip(&lang, &live, Vec::new());
    let state = read.tree().root().child(0).unwrap().parsing_state();
    let providers = state.scopes().providers();
    let language = lang.initial_state().scopes().providers();
    assert_eq!(providers.len(), language.len());
    for (read, live) in providers.iter().zip(language) {
        assert!(Arc::ptr_eq(read, live), "provider `{}`", live.name());
    }
}

#[test]
fn read_back_states_share_the_languages_structural_set() {
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, "a"));
    let read = round_trip(&lang, &live, Vec::new());
    let state = read.tree().root().parsing_state();
    assert!(Arc::ptr_eq(&state.ext().structural, lang.structural()));
}

#[test]
fn read_back_nodes_carry_the_languages_elements() {
    let def = macro_def("t", "emph", 1);
    let elements = elements_of(std::slice::from_ref(&def));
    let lang = lang(vec![def]);
    let live = Arc::new(parse(&lang, r"a \emph{b}"));
    let read = round_trip(&lang, &live, elements);
    let live_root = live.tree().root();
    let read_root = read.tree().root();
    for index in 0..2 {
        let (live, read) = (live_root.child(index).unwrap(), read_root.child(index).unwrap());
        assert!(Arc::ptr_eq(element_of(&read), element_of(&live)), "{}", live.summary());
    }
}

#[test]
fn two_values_of_one_element_name_read_back_as_their_own_values() {
    let heading = |name: &str| {
        ElementDef::new("t", [name], SyntaxDef::Macro { args: Vec::new() }, element("t.heading"))
    };
    let (section, subsection) = (heading("section"), heading("subsection"));
    let (first, second) = (Arc::clone(&section.element), Arc::clone(&subsection.element));
    assert!(!Arc::ptr_eq(&first, &second));
    let lang = lang(vec![section, subsection]);
    let live = Arc::new(parse(&lang, r"\section \subsection"));
    // The resolver knows one value of the name only; the specs decide anyway.
    let read = round_trip(&lang, &live, vec![Arc::clone(&second)]);
    let nodes = callables(&read);
    assert!(Arc::ptr_eq(element_of(&nodes[0]), &first));
    assert!(Arc::ptr_eq(element_of(&nodes[1]), &second));
}

#[test]
fn a_tack_on_field_node_round_trips_by_identity() {
    let field = TackOnField::new("label", vec![ArgDef::new(ArgStdKind::CharsGroup)]);
    let label = Arc::clone(field.element());
    let def = macro_with(
        "t",
        "section",
        vec![ArgDef::mandatory("title"), ArgDef::new(ArgStdKind::TackOn { fields: vec![field] })],
    );
    let elements = vec![Arc::clone(&def.element), label];
    let lang = lang(vec![def]);
    let live = Arc::new(parse(&lang, r"\section{x}\label{y}"));
    let read = round_trip(&lang, &live, elements);
    let live_field = callables(&live)[1];
    let read_field = callables(&read)[1];
    assert_eq!(read_field.name(), Some("label"));
    assert!(Arc::ptr_eq(read_field.spec().unwrap(), live_field.spec().unwrap()));
    assert!(Arc::ptr_eq(element_of(&read_field), element_of(&live_field)));
}

#[test]
fn an_unknown_macro_recovery_round_trips_by_identity() {
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, r"a \nope b"));
    let read = round_trip(&lang, &live, Vec::new());
    let node = callables(&read)[0];
    assert!(Arc::ptr_eq(node.spec().unwrap(), callables(&live)[0].spec().unwrap()));
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_macro));
    assert_eq!(node.name(), Some("nope"));
    read.diags().expect(UnresolvableCommand::IDENTIFIER).once();
}

#[test]
fn an_unknown_environment_recovery_round_trips_by_identity() {
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, r"\begin{nope}x\end{nope}"));
    let read = round_trip(&lang, &live, Vec::new());
    let node = callables(&read)[0];
    assert!(Arc::ptr_eq(node.spec().unwrap(), callables(&live)[0].spec().unwrap()));
    assert!(Arc::ptr_eq(element_of(&node), &lang.structural().unknown_environment));
    read.diags().expect(UnknownEnvironment::IDENTIFIER).once();
}

#[test]
fn a_fragments_diagnostics_round_trip_with_their_identifiers_data_and_spans() {
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, "a $b$"));
    assert!(!live.diags().is_empty());
    let read = round_trip(&lang, &live, Vec::new());
    let options = ProjectOptions::new();
    assert_eq!(
        project_diagnostics(read.diags().iter(), &options),
        project_diagnostics(live.diags().iter(), &options)
    );
    assert_eq!(read.diags().limit(), live.diags().limit());
}

#[test]
fn a_read_back_collection_keeps_its_suppressed_and_error_counts() {
    // The collection is rebuilt with techy's `Diagnostics::from_parts`, so the
    // counts the cap hid — the suppressed pushes and the errors among them — are
    // exact after the round trip, and `has_errors()` agrees, so the error gate
    // downstream stays truthful.
    let lang = lang(Vec::new());
    let options = ParseOptions { max_diagnostics: Some(1), ..ParseOptions::default() };
    let live = parse_fragment(&lang, source("$a$"), FragmentLayout::default(), &options)
        .expect("the parse yields a tree");
    let counts = |diags: &FlmDiagnostics| (diags.len(), diags.suppressed(), diags.error_count());
    assert_eq!(counts(live.diags()), (1, 1, 2), "one retained, one suppressed, two errors");
    let read = round_trip(&lang, &Arc::new(live), Vec::new());
    assert_eq!(read.diags().limit(), 1);
    assert_eq!(counts(read.diags()), (1, 1, 2), "both counts survive the round trip");
    assert!(read.diags().has_errors());
}

#[test]
fn a_fragment_keeps_its_layout() {
    let lang = lang(Vec::new());
    let options = ParseOptions::default();
    let fragment = parse_fragment(&lang, source("a"), FragmentLayout::Blocks, &options)
        .expect("the parse yields a tree");
    let read = round_trip(&lang, &Arc::new(fragment), Vec::new());
    assert_eq!(read.layout(), FragmentLayout::Blocks);
}

#[test]
fn a_fragment_reads_back_over_a_shared_source() {
    let lang = lang(Vec::new());
    let live = Arc::new(parse(&lang, "a b"));
    let read = round_trip(&lang, &live, Vec::new());
    let root = read.tree().root();
    assert!(Arc::ptr_eq(read.source(), root.span().source()));
    assert_eq!(read.source().content(), "a b");
}

#[test]
fn the_same_fragment_written_twice_is_one_entry() {
    let lang = lang(Vec::new());
    let fragment = Arc::new(parse(&lang, "a"));
    let mut writer = session::writer(&lang);
    let first = session::write_fragment(&mut writer, &fragment).unwrap();
    let second = session::write_fragment(&mut writer, &fragment).unwrap();
    assert_eq!(first, second);
}

// Ported from Python FLM's test/test_flmdump.py::TestFLMDataDumperBasic::test_multiple_dumps.
#[test]
fn two_different_fragments_written_into_one_session_are_two_entries() {
    let lang = lang(Vec::new());
    let first = Arc::new(parse(&lang, "a"));
    let second = Arc::new(parse(&lang, "b"));
    let mut writer = session::writer(&lang);
    let first_position = session::write_fragment(&mut writer, &first).expect("the first writes");
    let second_position = session::write_fragment(&mut writer, &second).expect("the second writes");
    assert_ne!(first_position, second_position);
}

#[test]
fn a_segment_renders_as_one_json_line() {
    let lang = lang(Vec::new());
    let fragment = Arc::new(parse(&lang, "a\nb"));
    let (_, segment) = write(&lang, &fragment);
    let line = segment_line(&segment).unwrap();
    assert!(!line.contains('\n'));
    assert!(line.contains("\"flm.fragments\""));
}

// --- hostile wire input: the read side refuses, never guesses ---------------------------

#[test]
fn a_node_ext_whose_spec_carries_no_element_is_refused() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, r"\emph{x}");
    let ext = callables(&fragment)[0].ext().clone();
    // techy's paragraph-break spec is a spec no element rides on.
    let no_element: Arc<dyn CallableSpec<Flm>> = Arc::new(ParagraphBreakSpec);
    let position: Cell<Option<SpecIndex>> = Cell::new(None);
    let (_, read) = round_trip_value_with(
        &lang,
        Vec::new(),
        no_readers(),
        ext,
        |writer| position.set(Some(writer.intern_spec(&no_element).expect("the spec writes"))),
        |data| {
            *field_mut(data, "spec") = index_value(position.get().expect("the spec was written"));
        },
    );
    assert!(read.is_err());
}

#[test]
fn a_node_ext_whose_spec_disagrees_with_the_written_name_is_refused() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, r"\emph{x}");
    let ext = callables(&fragment)[0].ext().clone();
    // The resolver knows the written name, so a fallback to the name would succeed.
    let (_, read) =
        round_trip_value_with(&lang, vec![element("t.other")], no_readers(), ext, |_| {}, |data| {
            *field_mut(data, "element") = s("t.other");
        });
    assert!(read.is_err());
}

#[test]
fn a_fragment_with_an_unknown_layout_is_refused() {
    let lang = lang(Vec::new());
    let (_, segment) = write(&lang, &Arc::new(parse(&lang, "a")));
    let segment = edit_entries(&segment, FRAGMENTS_TABLE, |entries| {
        *field_mut(&mut entries[0], "layout") = s("sideways");
    });
    let error = read_fragment_from(&lang, segment, Vec::new(), no_readers()).unwrap_err();
    assert!(matches!(root_cause(&error), DeserializeError::Failed { .. }), "{error:?}");
}

#[test]
fn a_fragment_whose_diagnostic_counts_are_inconsistent_is_refused() {
    let lang = lang(Vec::new());
    let (_, segment) = write(&lang, &Arc::new(parse(&lang, "a")));
    let segment = edit_entries(&segment, FRAGMENTS_TABLE, |entries| {
        // A suppressed diagnostic while the cap was not reached: no sequence of
        // pushes records it, so the entry is refused rather than normalized.
        *field_mut(field_mut(&mut entries[0], "diagnostics"), "suppressed") = SerialValue::Int(1);
    });
    let error = read_fragment_from(&lang, segment, Vec::new(), no_readers()).unwrap_err();
    assert!(
        matches!(
            root_cause(&error),
            DeserializeError::InconsistentDiagnosticCounts(InconsistentDiagnosticCounts {
                suppressed: 1,
                retained: 0,
                ..
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_fragment_paired_with_another_source_is_refused() {
    let lang = lang(Vec::new());
    let (own, other) = (Arc::new(parse(&lang, "a")), Arc::new(parse(&lang, "b")));
    let mut writer = session::writer(&lang);
    let position = session::write_fragment(&mut writer, &own).unwrap();
    session::write_fragment(&mut writer, &other).unwrap();
    let segment = writer.take_segment_with_main(position).unwrap();
    let segment = edit_entries(&segment, FRAGMENTS_TABLE, |entries| {
        let others_source = field(&entries[1], "source").cloned().expect("the other's source");
        *field_mut(&mut entries[0], "source") = others_source;
    });
    let error = read_fragment_from(&lang, segment, Vec::new(), no_readers()).unwrap_err();
    assert!(matches!(root_cause(&error), DeserializeError::Failed { .. }), "{error:?}");
}

// --- the fields package: a field spec's wire identity --------------------------------------

/// The keys of the tack-on field specs `lang` minted — its fields package,
/// enumerated through techy's `SpecsProvider::iter_symbols` — sorted.
fn field_keys(lang: &BuiltLang) -> Vec<String> {
    let fields = lang
        .providers()
        .iter()
        .find(|provider| provider.name() == FIELDS_PACKAGE_NAME)
        .expect("the fields package");
    let mut keys: Vec<String> = fields
        .iter_symbols(FlmCallableType::Macro, Mode::Text)
        .expect("a package enumerates")
        .map(|entry| entry.name.to_string())
        .collect();
    keys.sort();
    keys
}

/// `\name{title}` with a `\label` tack-on field.
fn heading_with_label(name: &str) -> ElementDef {
    let field = TackOnField::new("label", vec![ArgDef::new(ArgStdKind::CharsGroup)]);
    let fields = ArgDef::new(ArgStdKind::TackOn { fields: vec![field] });
    macro_with("t", name, vec![ArgDef::mandatory("title"), fields])
}

#[test]
fn a_field_specs_key_is_built_from_its_declaration_alone() {
    // `owner.element.field`: the same key whether or not another declaration
    // with a tack-on field was minted before it.
    let alone = field_keys(&lang(vec![heading_with_label("section")]));
    let preceded =
        field_keys(&lang(vec![heading_with_label("chapter"), heading_with_label("section")]));
    assert_eq!(alone, ["t.t.section.label"]);
    assert_eq!(preceded, ["t.t.chapter.label", "t.t.section.label"]);
}

// --- hand-staged trees: the never-parsed specs ------------------------------------------

#[test]
fn a_synthesized_node_round_trips_through_its_self_contained_form() {
    let lang = lang(Vec::new());
    let qed = element("thm.qed");
    let spec = Arc::new(FlmSynthesizedSpec::new(Arc::clone(&qed)));
    let mut site = Site::new(&lang);
    let node = site.stage(NodeKind::callable(spec.callable_data()), Vec::new()).unwrap();
    let tree = site.finish(vec![node]);
    let (position, segment) = write_tree(&lang, &tree);
    let read = read_tree(&lang, segment, position, vec![Arc::clone(&qed)], PayloadReaders::new());
    let node = read.root().child(0).unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &qed));
    assert!(Arc::ptr_eq(element_of_spec(&**node.spec().unwrap()).unwrap(), &qed));
    assert_eq!(node.callable_type(), Some(FlmCallableType::Specials));
}

#[test]
fn a_synthesized_spec_of_a_name_several_values_share_is_refused_as_ambiguous() {
    let lang = lang(Vec::new());
    let (first, second) = (element("thm.qed"), element("thm.qed"));
    let spec = Arc::new(FlmSynthesizedSpec::new(Arc::clone(&first)));
    let mut site = Site::new(&lang);
    let node = site.stage(NodeKind::callable(spec.callable_data()), Vec::new()).unwrap();
    let tree = site.finish(vec![node]);
    let (position, segment) = write_tree(&lang, &tree);
    let error =
        try_read_tree(&lang, segment, position, vec![first, second], no_readers()).unwrap_err();
    assert!(ambiguity(&error).is_some(), "{error:?}");
}

#[test]
fn a_presentation_node_round_trips_with_its_payload() {
    let lang = lang(Vec::new());
    let pres = element("t.pres");
    let spec = Arc::new(FlmPresentationSpec::new(Arc::clone(&pres)));
    let mut site = Site::new(&lang);
    let data = spec.callable_data(ParsedArguments::empty(), ParsedSlots::empty());
    let payload = Arc::new(Style { style: String::from("it") });
    let ext = FlmNodeExt::new(Arc::clone(&pres)).with_payload(payload);
    let node = site.stage_with_ext(NodeKind::callable(data), Vec::new(), ext).unwrap();
    let tree = site.finish(vec![node]);
    let (position, segment) = write_tree(&lang, &tree);
    let read = read_tree(&lang, segment, position, vec![Arc::clone(&pres)], style_readers());
    let node = read.root().child(0).unwrap();
    assert_eq!(node.callable_type(), Some(FlmCallableType::Presentation));
    assert!(Arc::ptr_eq(element_of(&node), &pres));
    let payload = node.ext().payload.as_ref().expect("the payload reads back");
    assert_eq!(payload.downcast_ref::<Style>(), Some(&Style { style: String::from("it") }));
}

/// Whether a spec without a package home is the cause of `error`.
fn missing_provenance(error: &SerializeError) -> bool {
    let mut current = error;
    loop {
        match current {
            SerializeError::MissingProvenance { .. } => return true,
            SerializeError::InTable { cause, .. } | SerializeError::InNode { cause, .. } => {
                current = &**cause;
            }
            _ => return false,
        }
    }
}

#[test]
fn a_node_whose_spec_has_no_package_home_cannot_be_written() {
    let lang = lang(Vec::new());
    let spec: Arc<dyn CallableSpec<Flm>> =
        Arc::new(FlmCallable::new(MacroSpec::<Flm>::new(Vec::new()), element("t.loose")));
    let data = CallableData {
        callable_type: FlmCallableType::Specials,
        name: "loose".into(),
        spec,
        arguments: ParsedArguments::empty(),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Specials,
    };
    let mut site = Site::new(&lang);
    let node = site.stage(NodeKind::callable(data), Vec::new()).unwrap();
    let tree = site.finish(vec![node]);
    let mut writer = session::writer(&lang);
    let error = session::write_tree(&mut writer, &tree).unwrap_err();
    assert!(missing_provenance(&error), "{error:?}");
}

#[test]
fn a_read_fragment_is_the_arc_the_session_holds() {
    let lang = lang(Vec::new());
    let fragment = Arc::new(parse(&lang, "a"));
    let (_, segment) = write(&lang, &fragment);
    let mut reader = session::reader(&lang, common::dump::resolver(Vec::new())).unwrap();
    let (_, index) = reader.push_segment(segment).unwrap().unwrap();
    let fragments = session::fragments_table(&reader).unwrap();
    let first: Arc<Fragment> =
        session::read_fragment(&mut reader, fragments.position(index)).unwrap();
    let second = session::read_fragment(&mut reader, fragments.position(index)).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}
