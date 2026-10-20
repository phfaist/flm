//! How a formula's nodes are written back as LaTeX source, in every output
//! that hands a formula on as source text: a control word followed by a letter
//! from another node — the `x` a user definition's `\alpha#1` substitutes — is
//! separated from it, so that the written source means what the tree means.

mod common;

use flm_core::core::plan::Overrides;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;

use common::stdcore::{identifiers, registry, run_workflow};

/// The standard package with the definitions `\pre{…}`, whose template is
/// `\alpha#1`, and `\dbl{…}`, whose template is `#1\beta`, each expanded in
/// every workflow (so that the recomposed LaTeX writes its expansion too).
fn registry_with_glue() -> std::sync::Arc<flm_core::core::registry::FlmRegistry> {
    registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![
        Definition::macro_("pre", vec![ArgDef::mandatory("a")]).content(r"\alpha#1")
            .expand(ExpandPolicy::Always),
        Definition::macro_("dbl", vec![ArgDef::mandatory("a")]).content(r"#1\beta")
            .expand(ExpandPolicy::Always),
    ])))
}

/// The output of `workflow` over `document`, with no diagnostic.
fn rendered(workflow: &str, document: &str) -> String {
    let run = run_workflow(&registry_with_glue(), workflow, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    match workflow {
        text::WORKFLOW => common::lowering::text(&run),
        html::WORKFLOW => common::html::fragment(&run),
        _ => common::latex::output(&run),
    }
}

/// The text, page and recomposed LaTeX outputs of `document`.
fn outputs(document: &str) -> [String; 3] {
    [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW].map(|w| rendered(w, document))
}

#[test]
fn a_control_word_before_a_substituted_letter_is_separated_from_it_in_every_output() {
    for out in outputs("\\(\\pre{x}\\)\n") {
        assert!(out.contains(r"\alpha x"), "{out}");
    }
}

#[test]
fn a_control_word_ending_a_template_is_separated_from_the_letter_after_the_use() {
    for out in outputs("\\(\\dbl{y}z\\)\n") {
        assert!(out.contains(r"y\beta z"), "{out}");
    }
}

#[test]
fn nothing_else_of_the_written_source_changes() {
    // A control word before another control word, a brace or a digit, and a
    // formula parsed from one source, are written as before.
    for out in outputs("\\(\\pre{\\gamma} \\pre{{x}} \\pre{2} \\alpha y\\alpha{x}\\)\n") {
        assert!(out.contains(r"\alpha\gamma \alpha{x} \alpha2 \alpha y\alpha{x}"), "{out}");
    }
}
