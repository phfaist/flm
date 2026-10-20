//! The `annotations` feature: the configuration and its checks at registry
//! build, the render option, the reviewer's command and its parser of the
//! feature's own, the form read from the argument's delimiters, and the
//! documents that must neither panic nor loop while parsing.
//!
//! The cut and the lowering are tested in `annotations_cut.rs` and
//! `annotations_text.rs`.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::lang::mint::{DefinitionShape, FlmArgumentParser};
use flm_core::lang::{Flm, FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::model::element::element_value;
use flm_core::model::names::FeatureName;
use flm_core::model::options::{Bind, OptionKey};
use flm_core::stdcore::features::annotations::{
    ANNOTATION, AnnotationElement, AnnotationParser, AnnotationsConfig, AnnotationsRenderOptions,
    Annotator, FEATURE, Form, TEXT_ARG, form_of,
};
use flm_core::stdcore::features::refs::{ANCHORS, encode_anchor};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};
use techy::core::constructs::ArgumentParser;
use techy::core::node::NodeRef;

/// The two annotators these tests use: `phf` with the initials `PhF`,
/// annotator 0, and `abc` with none, annotator 1.
fn two_annotators() -> AnnotationsConfig {
    AnnotationsConfig::new()
        .annotator(Annotator::new("phf").initials("PhF"))
        .annotator(Annotator::new("abc"))
}

/// The standard package with the annotations feature under `config`.
fn with_config(config: AnnotationsConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().annotations(config))
}

/// What the registry build under `config` reported, rendered; a build that
/// succeeds is a test failure.
fn build_error(config: AnnotationsConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().annotations(config)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

// ---------------------------------------------------------------------------------------------
// the configuration and the render option (P9, P18)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_default_configuration_declares_no_annotator_and_builds() {
    assert!(StdcoreConfig::new().annotations.annotators.is_empty());
    let registry = with_config(AnnotationsConfig::new());
    assert!(registry.elements_named("flm.annotations.annotation").is_empty());
}

#[test]
fn a_configuration_the_feature_refuses_names_the_culprit_at_registry_build() {
    let bad_command = build_error(AnnotationsConfig::new().annotator(Annotator::new("ph1")));
    assert!(bad_command.contains("`ph1`"), "{bad_command}");
    let environment = build_error(AnnotationsConfig::new().annotator(Annotator::new("end")));
    assert!(environment.contains("`\\end`"), "{environment}");
    let twice = build_error(two_annotators().annotator(Annotator::new("phf")));
    assert!(twice.contains("`\\phf`"), "{twice}");
    for initials in ["", " ", "\t"] {
        let blank = build_error(AnnotationsConfig::new().annotator(Annotator::new("phf").initials(initials)));
        assert!(blank.contains("`\\phf`"), "{initials:?}: {blank}");
    }
}

#[test]
fn the_configuration_and_the_render_option_are_declared_with_their_binding() {
    let registry = with_config(two_annotators());
    let config = registry.option_decl(&OptionKey::of::<AnnotationsConfig>()).expect("declared");
    assert_eq!(config.bind, Bind::Registry);
    assert_eq!(config.owner, FeatureName::from(FEATURE));
    let render = registry.option_decl(&OptionKey::of::<AnnotationsRenderOptions>()).expect("declared");
    assert_eq!(render.bind, Bind::Render);
    assert!(!AnnotationsRenderOptions::default().hidden);
}

#[test]
fn the_configuration_enters_the_registry_digest() {
    let none = with_config(AnnotationsConfig::new());
    let two = with_config(two_annotators());
    let again = with_config(two_annotators());
    let initials = with_config(
        AnnotationsConfig::new()
            .annotator(Annotator::new("phf").initials("P"))
            .annotator(Annotator::new("abc")),
    );
    assert_ne!(none.digest(), two.digest());
    assert_eq!(two.digest(), again.digest());
    assert_ne!(two.digest(), initials.digest());
}

#[test]
fn a_command_another_feature_declares_is_the_registry_builds_own_clash() {
    let error = build_error(AnnotationsConfig::new().annotator(Annotator::new("emph")));
    assert!(error.contains("emph"), "{error}");
}

// ---------------------------------------------------------------------------------------------
// the reviewer's command and its parser (P10; tests 11 and 16)
// ---------------------------------------------------------------------------------------------

/// The identifier of the language's report of a command nothing declares.
const UNKNOWN_COMMAND: &str = "core.specs.unresolvable-command";

/// `text` parsed by `registry`'s language as one fragment laid out as blocks.
fn parsed(registry: &FlmRegistry, text: &str) -> Fragment {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    parse_fragment(registry.lang(), source, FragmentLayout::Blocks, &ParseOptions::default())
        .expect("a parse")
}

/// The identifiers of the parse's diagnostics, in order.
fn parse_reports(fragment: &Fragment) -> Vec<String> {
    fragment.diags().iter().map(|d| d.identifier().to_string()).collect()
}

/// The annotation nodes of `fragment` — every node of the annotations
/// element — in document order.
fn annotations(fragment: &Fragment) -> Vec<NodeRef<'_, Flm>> {
    fragment.tree().root().descendants().filter(|node| node.ext().element.name == ANNOTATION).collect()
}

/// The delimiters the argument's group of the annotation `node` recorded.
fn delimiters(node: &NodeRef<'_, Flm>) -> (String, String) {
    let region = node.arguments().expect("a callable").get_named(TEXT_ARG).expect("declared");
    let region = region.region.as_ref().expect("written");
    let group = node.tree().get(region.content_parent()).expect("a node");
    let (open, close) = group.group_delimiters().expect("a group");
    (open.to_string(), close.to_string())
}

/// The characters of the content of the annotation `node`, as written.
fn content_text(node: &NodeRef<'_, Flm>) -> String {
    let content = node.argument_content_nodes_named(TEXT_ARG).expect("declared").expect("written");
    content.iter().map(|child| child.span_content().to_string()).collect()
}

#[test]
fn with_no_annotator_the_command_is_an_unknown_macro() {
    let registry = with_config(AnnotationsConfig::new());
    let fragment = parsed(&registry, "a \\phf{x} b");
    assert!(annotations(&fragment).is_empty());
    assert_eq!(parse_reports(&fragment), [UNKNOWN_COMMAND]);
}

#[test]
fn the_three_forms_are_one_node_with_one_group_argument_recording_what_was_read() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "a \\phf{passage} b \\phf[note] c \\phf passage\\endphf d \\abc{x}");
    assert_eq!(parse_reports(&fragment), Vec::<String>::new());
    let found = annotations(&fragment);
    assert_eq!(found.len(), 4);
    for node in &found {
        assert_eq!(node.arguments().expect("a callable").len(), 1);
    }
    assert_eq!(delimiters(&found[0]), ("{".into(), "}".into()));
    assert_eq!(delimiters(&found[1]), ("[".into(), "]".into()));
    assert_eq!(delimiters(&found[2]), ("".into(), "\\endphf ".into()));
    assert_eq!(delimiters(&found[3]), ("{".into(), "}".into()));
    assert_eq!(content_text(&found[2]), "passage");
    let forms: Vec<Option<Form>> = found.iter().map(form_of).collect();
    assert_eq!(forms, [Some(Form::Highlight), Some(Form::Comment), Some(Form::Highlight), Some(Form::Highlight)]);
    let numbers: Vec<usize> =
        found.iter().map(|node| element_value::<AnnotationElement>(node).expect("a value").number).collect();
    assert_eq!(numbers, [0, 0, 0, 1]);
}

#[test]
fn the_closing_command_at_the_end_of_the_input_records_itself_alone() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "\\phf passage\\endphf");
    assert_eq!(parse_reports(&fragment), Vec::<String>::new());
    assert_eq!(delimiters(&annotations(&fragment)[0]), ("".into(), "\\endphf".into()));
}

#[test]
fn whitespace_and_comments_may_stand_before_a_group_and_start_a_passage_otherwise() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "\\phf {a} \\phf %%c\n [b] \\phf %%c\n c\\endphf");
    assert_eq!(parse_reports(&fragment), Vec::<String>::new());
    let found = annotations(&fragment);
    assert_eq!(delimiters(&found[0]).0, "{");
    assert_eq!(delimiters(&found[1]).0, "[");
    assert_eq!(delimiters(&found[2]).0, "");
    assert_eq!(content_text(&found[2]), "%%c\n c");
}

#[test]
fn a_brace_group_after_a_highlight_is_not_a_second_argument() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "\\phf{x}[y]");
    let found = annotations(&fragment);
    assert_eq!(found.len(), 1);
    assert_eq!(form_of(&found[0]), Some(Form::Highlight));
    assert_eq!(content_text(&found[0]), "x");
    let after = fragment.tree().root().span_content()[found[0].span_content().len()..].to_string();
    assert_eq!(after, "[y]");
}

#[test]
fn a_passage_the_input_ends_before_closing_is_reported_over_what_was_read() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "a \\phf passage");
    assert_eq!(parse_reports(&fragment), ["flm.annotations.unterminated"]);
    let found = annotations(&fragment);
    assert_eq!(delimiters(&found[0]), ("".into(), "".into()));
    assert_eq!(content_text(&found[0]), "passage");
    let rendered = fragment.diags().render_all();
    assert!(rendered.contains("\\endphf"), "{rendered}");
}

#[test]
fn a_passage_ends_where_its_enclosing_group_ends() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "{a \\phf passage} b");
    assert_eq!(parse_reports(&fragment), ["flm.annotations.unterminated"]);
    assert_eq!(content_text(&annotations(&fragment)[0]), "passage");
    // The closing brace is left for the group, which stays whole.
    let fragment = parsed(&registry, "{\\phf a} b");
    assert_eq!(parse_reports(&fragment), ["flm.annotations.unterminated"]);
    let found = annotations(&fragment);
    assert_eq!(delimiters(&found[0]), ("".into(), "".into()));
    assert_eq!(content_text(&found[0]), "a");
    let root = fragment.tree().root();
    let group = root.children().into_iter().next().expect("the group");
    assert_eq!(group.span_content(), "{\\phf a}");
    assert_eq!(group.group_delimiters().map(|(open, close)| (open.to_string(), close.to_string())), Some(("{".into(), "}".into())));
}

#[test]
fn a_passage_ends_where_its_enclosing_environment_ends() {
    // The list's `\end` at the passage's own level ends the passage, which is
    // reported once, and is left for the list: the list stays whole and `b`
    // follows it.
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "\\begin{itemize}\\item \\phf a\\end{itemize} b");
    assert_eq!(parse_reports(&fragment), ["flm.annotations.unterminated"]);
    let found = annotations(&fragment);
    assert_eq!(found.len(), 1);
    assert_eq!(delimiters(&found[0]), ("".into(), "".into()));
    assert_eq!(content_text(&found[0]), "a");
    let root = fragment.tree().root();
    let list = root.descendants().find(|node| node.name() == Some("itemize")).expect("the list");
    assert_eq!(list.span_content(), "\\begin{itemize}\\item \\phf a\\end{itemize}");
    assert!(root.span_content().ends_with("\\end{itemize} b"));
    let after: String = root
        .children()
        .into_iter()
        .skip_while(|node| node.name() != Some("itemize"))
        .skip(1)
        .map(|node| node.span_content().to_string())
        .collect();
    assert_eq!(after.trim(), "b");
}

#[test]
fn a_stray_closing_command_is_an_unknown_macro() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "a \\endphf b");
    assert!(annotations(&fragment).is_empty());
    assert_eq!(parse_reports(&fragment), [UNKNOWN_COMMAND]);
}

#[test]
fn a_closing_command_inside_an_environment_of_the_passage_belongs_to_it() {
    let registry = with_config(two_annotators());
    let text = "\\phf see \\begin{itemize}\\item a \\endphf\\end{itemize} here";
    let fragment = parsed(&registry, text);
    assert_eq!(parse_reports(&fragment), [UNKNOWN_COMMAND, "flm.annotations.unterminated"]);
    let found = annotations(&fragment);
    assert_eq!(found.len(), 1);
    assert!(content_text(&found[0]).ends_with("here"), "{}", content_text(&found[0]));
}

#[test]
fn an_unclosed_bracket_is_the_ordinary_group_error() {
    let registry = with_config(two_annotators());
    let fragment = parsed(&registry, "a \\phf[x");
    let reports = parse_reports(&fragment);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(!reports[0].starts_with("flm.annotations."), "{reports:?}");
    let fragment = parsed(&registry, "a \\phf{x");
    let reports = parse_reports(&fragment);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(!reports[0].starts_with("flm.annotations."), "{reports:?}");
}

#[test]
fn the_parser_answers_the_languages_questions() {
    let parser = AnnotationParser::new("phf");
    assert!(parser.holds_content());
    assert!(!parser.can_match_empty());
    assert_eq!(parser.identity(), b"flm.annotations.text:phf");
    assert_ne!(parser.identity(), AnnotationParser::new("abc").identity());
    assert_eq!(parser.definition_shape(), DefinitionShape::NoShape { token: "annotation" });
    assert!(parser.description().contains("\\endphf"), "{}", parser.description());
}

#[test]
fn the_parser_never_panics_on_an_input_ending_anywhere() {
    let registry = with_config(two_annotators());
    let text = "a \\phf{b \\abc[c %%x\n\nd] e} \\phf f \\begin{itemize}\\item g\\end{itemize}\n\nh \\abc{i}\\endphf \
                \\phf[\\phf j\\endphf] \\endabc k";
    for end in (0..=text.len()).filter(|end| text.is_char_boundary(*end)) {
        let _ = parsed(&registry, &text[..end]);
    }
}

// ---------------------------------------------------------------------------------------------
// apart from the text (P12; test 17)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_heading_is_identified_from_its_highlight_and_its_words_not_from_its_comment() {
    // The comment's remark is the annotator's, no word of the title; the
    // highlighted passage is. So the title below is identified from
    // "Some draft notes" alone — `sec--Some_draft_notes`, where it would be
    // `sec--Some_draft_notescheck_this` with the remark read as words.
    let registry = with_config(two_annotators());
    let document = "\\section{Some \\phf{draft} notes\\phf[check this]}\n\nText.";
    let run = common::stdcore::run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(common::stdcore::identifiers(&run), Vec::<String>::new());
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    let ids: Vec<String> = anchors.ids.values().flatten().cloned().collect();
    assert_eq!(ids, [format!("sec--{}", encode_anchor("Some draft notes"))]);
    assert_eq!(ids[0], "sec--Some_draft_notes");
}
