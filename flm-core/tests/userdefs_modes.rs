//! The parsing modes a user definition applies in
//! (`flm_core::lang::rewrites::userdefs::DefinitionModes`): one template text
//! for the modes the definition applies in, a use site outside them answered by
//! the language as an unknown callable, a definition declared by one text in
//! force in each mode where the text parses, two definitions of one name in
//! parsing modes that do not overlap, the modes in the registry digest, and
//! the library's function that holds the rule for any text read in several
//! modes (`compile_in_modes`).

mod common;

use std::sync::Arc;

use flm_core::lang::mint::{ArgDef, ElementDef};
use flm_core::lang::rewrites::userdefs::{
    compile_in_modes, compile_template_with, BadDefinition, Definition, DefinitionModes,
    DefinitionProblem, PlaceholderVocabulary, TemplateLanguage, TemplateSpec, UserDefinition,
};
use techy::latexlike::Mode;
use flm_core::lang::{build_language, structural, LangInputs};
use flm_core::model::element::{element_of, ElementSpec};
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::core::registry::FlmFeature;
use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};
use techy::core::constructs::UnresolvableCommand;
use techy::error::DiagnosticInfo;

use common::userdefs::{expanded, recomposed, setup, setup_definitions, Setup};
use common::dump::round_trip;
use common::stdcore::registry as stdcore_registry;
use common::wiring::{registry, registry_with};
use common::parse;

/// The element every node spanning exactly `spelling` carries, and how many
/// unresolvable-command diagnostics the parse of `text` reported.
fn parsed_use_site(setup: &Setup, text: &str, spelling: &str) -> (String, usize) {
    let fragment = parse(&setup.lang, text);
    let node = fragment
        .tree()
        .iter_storage_order()
        .find(|node| node.span_content() == spelling)
        .expect("the use site");
    let element = element_of(&node).name.to_string();
    let reported = fragment.diags().conditions::<UnresolvableCommand>().count();
    (element, reported)
}

// --- a use site outside the definition's modes ---------------------------------------------

#[test]
fn a_text_only_definition_used_in_math_mode_is_an_unknown_macro() {
    // The definition is declared visible in text mode alone, so in mathematics
    // mode `\foo` is a name no feature defines: the language's own fallback
    // answers it, exactly as for any unknown macro.
    let setup = setup_definitions(vec![Definition::macro_("foo", Vec::new()).text("T")]);
    let (element, reported) = parsed_use_site(&setup, r"\(\foo\)", r"\foo");
    assert_eq!(element, structural::UNKNOWN_MACRO);
    assert_eq!(reported, 1);
}

#[test]
fn a_math_only_definition_used_in_text_mode_is_an_unknown_macro() {
    let setup = setup_definitions(vec![Definition::macro_("foo", Vec::new()).math("M")]);
    let (element, reported) = parsed_use_site(&setup, r"say \foo.", r"\foo");
    assert_eq!(element, structural::UNKNOWN_MACRO);
    assert_eq!(reported, 1);
}

#[test]
fn the_unknown_macro_diagnostic_is_the_languages_own() {
    let setup = setup_definitions(vec![Definition::macro_("foo", Vec::new()).text("T")]);
    let fragment = parse(&setup.lang, r"\(\foo\)");
    let identifiers: Vec<&str> =
        fragment.diags().iter().map(|diagnostic| diagnostic.identifier()).collect();
    assert_eq!(identifiers, [UnresolvableCommand::IDENTIFIER]);
}

// --- one template text for both modes -------------------------------------------------------

#[test]
fn a_definition_of_both_modes_expands_in_both_modes() {
    // One declared template text, compiled under each mode it applies in.
    let setup = setup_definitions(vec![Definition::macro_("foo", Vec::new()).content("F")]);
    let (tree, diagnostics, _) = expanded(&setup, r"\foo \(\foo\)", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    // The whitespace after a macro invocation is part of the invocation and
    // disappears with it, in either mode.
    assert_eq!(recomposed(&tree), r"F\(F\)");
}

// --- one text, in force where it parses ----------------------------------------------------

/// `\mcmd{…}`, a command of formulas alone, beside `definitions`.
fn with_a_formula_command(definitions: Vec<Definition>) -> Setup {
    let formula_command = ElementDef::new(
        "other",
        ["mcmd"],
        flm_core::lang::mint::SyntaxDef::Macro { args: vec![ArgDef::mandatory("a")] },
        common::element("other.mcmd"),
    )
    .modes([Mode::Math]);
    setup(definitions, vec![formula_command])
}

/// `\ket{…}`, declared by the one text `template`, with no mode named.
fn ket(template: &str) -> Definition {
    Definition::macro_("ket", vec![ArgDef::mandatory("a")]).content(template)
}

#[test]
fn a_one_text_definition_holding_a_formula_command_is_built_without_a_report() {
    let setup = with_a_formula_command(vec![ket(r"\mcmd{#1}")]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let compiled = setup.definition("ket").compiled().expect("the definition is kept");
    assert!(compiled.variant(Mode::Math).is_some());
    assert!(compiled.variant(Mode::Text).is_none());
}

#[test]
fn a_one_text_definition_holding_a_formula_command_expands_in_a_formula() {
    let setup = with_a_formula_command(vec![ket(r"\mcmd{#1}")]);
    let (tree, diagnostics, _) = expanded(&setup, r"\(\ket{x}\)", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), r"\(\mcmd{x}\)");
}

#[test]
fn a_one_text_definition_holding_a_formula_command_is_an_error_at_a_use_in_running_text() {
    let setup = with_a_formula_command(vec![ket(r"\mcmd{#1}")]);
    let (tree, diagnostics, _) = expanded(&setup, r"say \ket{x} and \(\ket{y}\)", 8);
    let reported: Vec<&BadDefinition> = diagnostics.conditions::<BadDefinition>().collect();
    assert_eq!(reported.len(), 1, "{}", diagnostics.render_all());
    assert_eq!(reported[0].problem, DefinitionProblem::NotInForceInMode { mode: Mode::Text });
    // The error is reported at the use, and the rest of the document is
    // expanded: the use in the formula still gives its replacement.
    let error = diagnostics.iter().next().expect("the report");
    assert_eq!(error.span().content(), r"\ket{x}");
    assert!(recomposed(&tree).ends_with(r"\(\mcmd{y}\)"), "{}", recomposed(&tree));
}

#[test]
fn a_one_text_template_that_parses_in_neither_mode_is_refused_at_build() {
    let setup = setup_definitions(vec![ket(r"\nosuchcommand{#1}")]);
    assert!(
        setup.build_diagnostics.iter().any(|d| d.severity() == techy::error::Severity::Error),
        "{:?}",
        setup.build_diagnostics
    );
    assert!(setup.definition("ket").compiled().is_none());
}

#[test]
fn a_template_for_running_text_that_does_not_parse_there_is_refused_at_build() {
    let setup = with_a_formula_command(vec![
        Definition::macro_("ket", vec![ArgDef::mandatory("a")]).text(r"\mcmd{#1}"),
    ]);
    assert!(
        setup.build_diagnostics.iter().any(|d| d.severity() == techy::error::Severity::Error),
        "{:?}",
        setup.build_diagnostics
    );
    assert!(setup.definition("ket").compiled().is_none());
}

#[test]
fn both_modes_is_the_default() {
    assert_eq!(DefinitionModes::default(), DefinitionModes::Both);
    assert_eq!(Definition::macro_("foo", Vec::new()).content("F").modes, DefinitionModes::Both);
}

#[test]
fn the_one_mode_builders_set_the_content_and_the_modes() {
    let text = Definition::macro_("foo", Vec::new()).text("T");
    assert_eq!(text.content.as_deref(), Some("T"));
    assert_eq!(text.modes, DefinitionModes::Text);
    let math = Definition::macro_("foo", Vec::new()).math("M");
    assert_eq!(math.content.as_deref(), Some("M"));
    assert_eq!(math.modes, DefinitionModes::Math);
}

#[test]
fn the_second_of_the_two_one_mode_builders_replaces_the_first() {
    // A definition has one template, so the later call wins outright — both the
    // template text and the modes. Two templates means two definitions.
    let definition = Definition::macro_("foo", Vec::new()).text("T").math("M");
    assert_eq!(definition.content.as_deref(), Some("M"));
    assert_eq!(definition.modes, DefinitionModes::Math);
}

// --- two definitions of one name in modes that do not overlap ---------------------------------

#[test]
fn two_definitions_of_one_name_in_disjoint_modes_are_two_definitions() {
    // Their visibility does not overlap, so the language build accepts both and
    // each use site resolves to the definition of its own mode. The two are
    // declared by two features: one feature registers one callable per written
    // name (see the refusal pinned below).
    let in_text = UserDefinition::new(Definition::macro_("foo", Vec::new()).text("TEXT"));
    let in_math = UserDefinition::new(Definition::macro_("foo", Vec::new()).math("MATH"));
    let mut inputs = LangInputs::new();
    inputs.elements.push(in_text.element_def("u", "u.foo"));
    inputs.elements.push(in_math.element_def("v", "v.foo"));
    let lang = build_language(inputs);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let language = TemplateLanguage::new(&lang).expect("the template language mints");
    let mut build_diagnostics = Vec::new();
    for value in [&in_text, &in_math] {
        match value.compile_with(&lang, &language) {
            Ok(warnings) => build_diagnostics.extend(warnings),
            Err(errors) => build_diagnostics.extend(errors),
        }
    }
    assert!(build_diagnostics.is_empty());
    let setup = Setup {
        lang,
        definitions: vec![Arc::clone(&in_text), Arc::clone(&in_math)],
        build_diagnostics,
    };
    let (tree, diagnostics, _) = expanded(&setup, r"\foo \(\foo\)", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), r"TEXT\(MATH\)");
}

/// The `userdefs` feature over `definitions`, as a registry feature.
fn userdefs(definitions: Vec<Definition>) -> Arc<dyn FlmFeature> {
    Arc::new(Userdefs::new(UserdefsConfig::new().definitions(definitions)))
}

#[test]
fn one_configuration_takes_a_text_only_and_a_math_only_definition_of_one_name() {
    // The two apply in parsing modes that do not overlap, so both are declared,
    // each becoming an element of its own, and each use site resolves to the
    // definition of its own mode.
    let registry = registry(vec![
        Definition::macro_("foo", Vec::new()).text("TEXT"),
        Definition::macro_("foo", Vec::new()).math("MATH"),
    ]);
    assert!(registry.element_value("flm.userdefs.macro.foo").unwrap().is_some());
    assert!(registry.element_value("flm.userdefs.macro.math.foo").unwrap().is_some());
    let fragment = parse(registry.lang(), r"\foo \(\foo\)");
    assert!(!fragment.diags().has_errors(), "{}", fragment.diags().render_all());
    let used: Vec<String> = fragment
        .tree()
        .iter_storage_order()
        .filter(|node| node.span_content().trim() == r"\foo")
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(used, ["flm.userdefs.macro.foo", "flm.userdefs.macro.math.foo"]);
}

#[test]
fn one_configuration_refuses_two_definitions_that_apply_in_one_mode() {
    // A text-only definition and a both-modes definition of one name are both
    // in force in text mode, which no build can honor.
    let error = common::pipeline::registry_error(vec![userdefs(vec![
        Definition::macro_("foo", Vec::new()).text("T"),
        Definition::macro_("foo", Vec::new()).content("B"),
    ])]);
    assert!(error.render().contains("the same parsing mode"), "{}", error.render());
}

// --- the registry digest ----------------------------------------------------------------------

#[test]
fn the_modes_enter_the_registry_digest() {
    let of = |definition: Definition| {
        registry_with(Blocks::default(), UserdefsConfig::new().definition(definition)).digest()
    };
    let in_text = of(Definition::macro_("foo", Vec::new()).text("F"));
    let in_math = of(Definition::macro_("foo", Vec::new()).math("F"));
    let in_both = of(Definition::macro_("foo", Vec::new()).content("F"));
    assert_ne!(in_text, in_math);
    assert_ne!(in_text, in_both);
    assert_ne!(in_math, in_both);
}

#[test]
fn a_definition_of_both_modes_reaches_the_registry_as_one_element() {
    let registry = registry(vec![Definition::macro_("foo", Vec::new()).content("F")]);
    assert!(registry.element_value("flm.userdefs.macro.foo").unwrap().is_some());
}

// --- the wire form ------------------------------------------------------------------------

#[test]
fn a_fragment_using_both_definitions_of_one_name_reads_back_from_the_wire_form() {
    // The two definitions are minted into two packages of the one `userdefs`
    // feature, and a stored node names the package its specification came from,
    // so the reading side must find both packages by name.
    let config = UserdefsConfig::new().definitions(vec![
        Definition::macro_("foo", Vec::new()).text("TEXT"),
        Definition::macro_("foo", Vec::new()).math("MATH"),
    ]);
    let registry = stdcore_registry(StdcoreConfig::new().userdefs(config));
    let live = Arc::new(parse(registry.lang(), "\\foo \\(\\foo\\)\n"));
    assert!(!live.diags().has_errors(), "{}", live.diags().render_all());
    let elements: Vec<Arc<ElementSpec>> =
        registry.elements().iter().map(|entry| Arc::clone(&entry.element)).collect();
    let read = round_trip(registry.lang(), &live, elements);
    let names: Vec<String> = read
        .tree()
        .iter_storage_order()
        .filter(|node| node.span_content().trim() == "\\foo")
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(names, ["flm.userdefs.macro.foo", "flm.userdefs.macro.math.foo"]);
}

// --- the rule for any text read in several modes, as a function of its own -------------------

/// `text` compiled through `compile_in_modes` in `modes`, over a language
/// that knows `\onlymath` in formulas alone: the modes kept, and the
/// identifiers of the diagnostics to report.
fn compiled_in_modes(text: &str, modes: &[Mode]) -> (Vec<Mode>, Vec<String>) {
    let only_math = UserDefinition::new(Definition::macro_("onlymath", Vec::new()).math("M"));
    let mut inputs = LangInputs::new();
    inputs.elements.push(only_math.element_def("u", "u.onlymath"));
    let lang = build_language(inputs);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    let language = TemplateLanguage::new(&lang).expect("the template language mints");
    let vocabulary = PlaceholderVocabulary::new(0);
    let (kept, reported) = compile_in_modes(modes.iter().copied(), |mode, found| {
        let spec = TemplateSpec {
            text,
            mode,
            vocabulary: &vocabulary,
            offset: 0,
            definition: "test",
            label: "a text of the test",
            origin_label: "a text of the test",
            user_definition: false,
        };
        match compile_template_with(&spec, &lang, &language) {
            Ok(template) => {
                found.extend(template.diagnostics().iter().cloned());
                Some(template)
            }
            Err(errors) => {
                found.extend(errors);
                None
            }
        }
    });
    for (mode, template) in &kept {
        assert_eq!(template.mode(), *mode);
    }
    let identifiers = reported
        .iter()
        .map(|diagnostic| diagnostic.identifier().to_string())
        .collect();
    (kept.into_iter().map(|(mode, _)| mode).collect(), identifiers)
}

#[test]
fn a_text_is_kept_in_the_modes_where_it_compiles_and_its_failures_dropped() {
    // `\onlymath` is unknown in running text: the text stands in formulas
    // alone, and the failure of the other mode is no problem of the text.
    let (kept, reported) = compiled_in_modes(r"\onlymath", &[Mode::Text, Mode::Math]);
    assert_eq!(kept, [Mode::Math]);
    assert!(reported.is_empty(), "{reported:?}");
    let (kept, reported) = compiled_in_modes("plain", &[Mode::Text, Mode::Math]);
    assert_eq!(kept, [Mode::Text, Mode::Math]);
    assert!(reported.is_empty(), "{reported:?}");
}

#[test]
fn a_text_that_compiles_in_no_mode_has_the_failures_of_every_mode_reported() {
    let (kept, reported) = compiled_in_modes(r"\onlymath", &[Mode::Text]);
    assert!(kept.is_empty());
    assert_eq!(reported, [UnresolvableCommand::IDENTIFIER]);
    let (kept, reported) = compiled_in_modes(r"\nowhere", &[Mode::Text, Mode::Math]);
    assert!(kept.is_empty());
    assert_eq!(reported, [UnresolvableCommand::IDENTIFIER, UnresolvableCommand::IDENTIFIER]);
}
