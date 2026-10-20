//! The `literals` feature (`flm_core::stdcore::features::literals`): the nine
//! spellings that stand for a single character are nine values of one element
//! type, each lowering to a text node holding that character; `\@` lowers to
//! nothing; and the tokenizer facts the declarations rely on — that a `%`, a
//! brace or a space after the escape character is read as a command and not as
//! a comment, a group or whitespace.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::literals::{
    self, Literal, LiteralElement, LITERAL, SENTENCE_SPACING,
};

use common::lowering::{lowered, text as text_of};
use common::stdcore::{registry_over, run_workflow};

/// A registry over the `literals` feature, the lowering machinery, the text
/// backend and the `blocks` feature.
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        literals::feature(),
    ])
}

/// The text of a run's lowered first tree, gathered from its text nodes. This
/// is the tree the backends read, before the text emitter lays it out.
fn lowered_text(run: &flm_core::core::run::Run) -> String {
    lowered(run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter_map(|node| node.chars().map(str::to_string))
        .collect()
}

#[test]
fn the_nine_spellings_are_nine_values_of_one_element_type() {
    let registry = registry();
    let literals: Vec<Literal> = registry
        .elements_named(LITERAL)
        .iter()
        .map(|idx| {
            let entry = registry.element(*idx).expect("interned");
            entry.element.value_as::<LiteralElement>().expect("valued").literal
        })
        .collect();
    assert_eq!(literals, Literal::ALL);
}

/// The tokenizer reads every one of the eight macro spellings as a command,
/// which is what the declarations rely on. In particular the `%` of `\%` does
/// not start a comment and the brace of `\{` does not open a group: the
/// tokenizer looks for a command before it looks for a comment, a special
/// character sequence or a forbidden character, and `\{` is not one of the
/// delimiter pairs it looks for even earlier.
#[test]
fn every_spelling_parses_and_produces_exactly_its_one_character() {
    for literal in Literal::ALL {
        // A macro whose name is made of letters swallows the whitespace that
        // follows it, so `\textbackslash` needs a space before the next letter
        // and the other spellings need none.
        let separator = match literal {
            Literal::Backslash => " ",
            _ => "",
        };
        let source = format!("a{}{}b", literal.source(), separator);
        let run = run_workflow(&registry(), text::WORKFLOW, &[&source], &Overrides::none());
        assert!(
            run.diagnostics().iter().next().is_none(),
            "{} was not read cleanly: {}",
            literal.source(),
            run.diagnostics().render_all()
        );
        assert_eq!(
            lowered_text(&run),
            format!("a{}b", literal.text()),
            "{} lowered to the wrong text",
            literal.source()
        );
    }
}

/// `\@` is an element of its own that lowers to no nodes at all.
#[test]
fn the_sentence_spacing_hint_parses_and_lowers_to_nothing() {
    let run = run_workflow(&registry(), text::WORKFLOW, &["cf.\\@ the appendix"], &Overrides::none());
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    let hints = lowered(&run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == SENTENCE_SPACING)
        .count();
    assert_eq!(hints, 0);
    assert_eq!(text_of(&run), "cf. the appendix\n");
}

/// The two spellings that stand for a space survive the whitespace collapsing
/// of block structuring: that pass runs over the parsed tree, where each of
/// them is still an element node and not text, so it has no whitespace to
/// collapse. The characters are in the lowered tree.
#[test]
fn the_two_space_spellings_survive_whitespace_collapsing() {
    let run = run_workflow(&registry(), text::WORKFLOW, &["a  \\ b"], &Overrides::none());
    // The two source spaces collapse to one; the space `\ ` stands for is
    // produced after that pass and is a second space beside it.
    assert_eq!(lowered_text(&run), "a  b");

    let run = run_workflow(&registry(), text::WORKFLOW, &["a~b  c"], &Overrides::none());
    assert_eq!(lowered_text(&run), "a\u{a0}b c");
}

/// What the text backend prints for the two spellings that stand for a space.
/// A non-breaking space is not a word separator, so it reaches the output as
/// the character U+00A0 itself; the space `\ ` stands for is an ordinary space,
/// so it separates words like any other and prints as one space.
#[test]
fn the_text_backend_prints_a_non_breaking_space_and_an_ordinary_one() {
    let run = run_workflow(&registry(), text::WORKFLOW, &["a~b"], &Overrides::none());
    assert_eq!(text_of(&run), "a\u{a0}b\n");
    let run = run_workflow(&registry(), text::WORKFLOW, &["a\\ b"], &Overrides::none());
    assert_eq!(text_of(&run), "a b\n");
}

/// The other seven characters reach the text output unchanged.
#[test]
fn the_text_backend_prints_every_other_literal_character() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["\\%\\#\\&\\$\\{\\}\\textbackslash"],
        &Overrides::none(),
    );
    assert_eq!(text_of(&run), "%#&${}\\\n");
}

/// `~` is declared as a special character sequence, so it is recognized with no
/// escape character before it, and nothing else in the standard package claims
/// that trigger.
#[test]
fn the_tilde_is_a_special_character_sequence_and_clashes_with_nothing() {
    let registry = flm_core::stdcore::package(flm_core::stdcore::StdcoreConfig::new());
    let mut builder = flm_core::core::registry::RegistryBuilder::new();
    builder.use_package(registry);
    let built = builder
        .build(&flm_core::core::registry::RegistryInputs::new())
        .expect("the standard package builds with the literals feature in it");
    assert!(built.has_feature(literals::FEATURE));
    let errors: Vec<String> =
        built.build_diagnostics().iter().map(|diag| diag.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

/// Inside a mathematics region the content is carried through exactly as
/// written, so a literal spelling there is *not* replaced by its character: the
/// mathematics source keeps `\%`, which is what a mathematics typesetter needs.
/// The declaration still matters, because it is what stops the `%` from being
/// reported as a forbidden character.
#[test]
fn a_literal_inside_mathematics_stays_as_written() {
    let mut features = vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()) as Arc<dyn flm_core::core::registry::FlmFeature>,
        literals::feature(),
    ];
    features.push(flm_core::stdcore::features::math::feature());
    features.push(flm_core::math_passthrough::feature());
    let run = run_workflow(
        &registry_over(features),
        text::WORKFLOW,
        &["x \\(50\\% y\\) z"],
        &Overrides::none(),
    );
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "x \\(50\\% y\\) z\n");
}

/// A user definition's template is parsed once, when the registry is built, and
/// under the same parsing configuration as a document. A lone `%` in a template
/// is therefore refused there too, and the refusal is one of the registry's
/// build diagnostics rather than a diagnostic of any one document's parse.
#[test]
fn a_lone_percent_in_a_definition_template_is_refused_at_registry_build() {
    use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
    use flm_core::lang::rewrites::userdefs::Definition;
    use flm_core::stdcore::features::userdefs::UserdefsConfig;
    use flm_core::stdcore::{StdcoreConfig, package};

    let definition = Definition::macro_("mostly", Vec::new()).text("100% sure");
    let config =
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![definition]));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");

    let identifiers: Vec<&str> =
        registry.build_diagnostics().iter().map(|diag| diag.identifier()).collect();
    assert!(
        identifiers.contains(&"flm.lang.percent-forbidden"),
        "the template's lone `%` was not refused: {identifiers:?}"
    );
}
