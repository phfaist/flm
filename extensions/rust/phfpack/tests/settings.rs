//! The settings of `phfqit`: declared macros in the three shapes, a declared
//! macro replacing a built-in one, the named values, the digest, and the
//! conditions a declaration can meet.

mod common;

use std::sync::Arc;

use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::backends::text;
use flm_core::techy::error::Severity;

use common::{registry_with, run, text_of, try_registry_with};

/// The error of the registry built with the settings `data`, rendered: the
/// feature reads its settings when the registry is built.
fn refusal(data: serde_json::Value) -> String {
    match try_registry_with(data) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.render(),
    }
}

/// The problems of reading the settings `data` with the feature's declaration,
/// each its path and its message: the feature's configuration reader reads them
/// when the settings are read.
fn read_problems(data: serde_json::Value) -> Vec<(String, String)> {
    let factory = common::factory();
    let decl = factory.settings(common::FEATURE).expect("the feature declares settings");
    match decl.read(data.as_object().expect("the settings are a map")) {
        Ok(_) => panic!("the settings read"),
        Err(errors) => errors.into_iter().map(|error| (error.path, error.detail)).collect(),
    }
}

/// The text output of one formula, which must report nothing.
fn formula(registry: &Arc<FlmRegistry>, source: &str) -> String {
    let (text, diagnostics) = text_of(registry, &format!(r"\({source}\)"));
    assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    text.strip_prefix(r"\(").and_then(|t| t.strip_suffix("\\)\n")).unwrap_or(&text).to_string()
}

#[test]
fn a_declaration_written_as_a_text_is_a_macro_without_arguments() {
    let registry = registry_with(serde_json::json!({
        "macro_definitions": { "Hilb": r"\mathcal{H}" }
    }));
    assert_eq!(formula(&registry, r"\Hilb"), r"\mathcal{H}");
}

#[test]
fn a_delimited_declaration_takes_the_three_size_slots() {
    // The norm of the Python package's example documents.
    let registry = registry_with(serde_json::json!({
        "macro_definitions": { "opnorm": {
            "delimited_args": ["{"],
            "content": r"#[\lVert]{DelimLeft}{#1}#[\rVert]{DelimRight}_\infty",
        } }
    }));
    assert_eq!(formula(&registry, r"\opnorm{A}"), r"\lVert{A}\rVert_\infty");
    assert_eq!(formula(&registry, r"\opnorm*{A}"), r"\mathopen{}\left \lVert{A}\right \rVert\mathclose{}_\infty");
    assert_eq!(formula(&registry, r"\opnorm`\Big{A}"), r"\Bigl \lVert{A}\Bigr \rVert_\infty");
}

#[test]
fn a_declaration_with_args_names_the_size_slots_and_codes() {
    let registry = registry_with(serde_json::json!({
        "macro_definitions": { "paren": {
            "args": ["SizeArgBacktick", "SizeArgStar", "SizeArgOptArg", "r()"],
            "argument_number_offset": 3,
            "content": "#[(]{DelimLeft}#1#[)]{DelimRight}",
        } }
    }));
    assert_eq!(formula(&registry, r"\paren(a)"), "(a)");
    assert_eq!(formula(&registry, r"\paren[\big](a)"), r"\bigl (a\bigr )");
}

#[test]
fn a_declared_macro_named_as_a_built_in_one_replaces_it() {
    let registry = registry_with(serde_json::json!({
        "macro_definitions": { "ket": {
            "delimited_args": "{",
            "content": "#[|]{DelimLeft}#1#[>]{DelimRight}",
        } }
    }));
    assert_eq!(formula(&registry, r"\ket{x}"), "|x>");
    assert_eq!(formula(&registry, r"\ket`\big{x}"), r"\bigl |x\bigr >");
}

#[test]
fn a_template_reads_each_named_value_and_a_setting_wins_over_the_default() {
    let template = "[#[spaceKets.Bar]{config}|#[spaceKets.RLAngle]{config}|#[spaceOKets.Bar]{config}|\
                    #[spaceOKets.RLAngle]{config}|#[space.BeforeComma]{config}|#[space.AfterComma]{config}]";
    let declared = serde_json::json!({ "v": template });
    let defaults = registry_with(serde_json::json!({ "macro_definitions": declared }));
    assert_eq!(
        formula(&defaults, r"\v"),
        r"[\mkern 1.5mu |\mkern -1.8mu |\mkern 1.5mu |\mkern -1.8mu ||\mkern 1.5mu ]"
    );
    let set = registry_with(serde_json::json!({
        "macro_definitions": declared,
        "spaceKets": { "Bar": r"\,", "RLAngle": "R" },
        "space": { "BeforeComma": "C", "AfterComma": "A" },
    }));
    assert_eq!(formula(&set, r"\v"), r"[\,|R|\mkern 1.5mu |\mkern -1.8mu |C|A]");
    assert_eq!(formula(&set, r"\braket{x}{y}"), r"\langle{x}\,\vert\,{y}\rangle");
}

#[test]
fn a_named_value_no_setting_holds_is_reported_at_each_use() {
    let registry = registry_with(serde_json::json!({
        "macro_definitions": { "w": "#[spaceKets.Nothing]{config}" }
    }));
    let reported: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(reported.is_empty(), "{reported:?}");
    // One use in running text and one in a formula: each is an Error, and
    // the document is rendered, each use standing as written.
    let run = run(&registry, text::WORKFLOW, r"A \w and \(\w\).");
    let found: Vec<(&str, Severity)> = run.diagnostics().iter().map(|d| (d.identifier(), d.severity())).collect();
    let unknown = ("phfpack.phfqit.unknown-value", Severity::Error);
    assert_eq!(found, [unknown, unknown]);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(out, "A \\w and \\(\\w\\).\n");
}

#[test]
fn the_registry_digest_differs_when_the_settings_differ() {
    let one = registry_with(serde_json::json!({ "macro_definitions": { "w": "a" } }));
    let other = registry_with(serde_json::json!({ "macro_definitions": { "w": "b" } }));
    let spaced = registry_with(serde_json::json!({ "space": { "AfterComma": "x" } }));
    let plain = registry_with(serde_json::json!({}));
    assert_ne!(one.digest(), other.digest());
    assert_ne!(spaced.digest(), plain.digest());
    assert_eq!(plain.digest(), registry_with(serde_json::json!({})).digest());
}

#[test]
fn a_declaration_with_a_code_that_is_not_one_is_refused_when_the_settings_are_read() {
    let problems = read_problems(serde_json::json!({
        "macro_definitions": { "w": { "args": ["q"], "content": "x" } }
    }));
    let [(path, detail)] = &problems[..] else { panic!("one problem: {problems:?}") };
    assert_eq!(path, "phfqit.macro_definitions.w.args");
    assert!(detail.starts_with("the value of `phfqit.macro_definitions.w.args` is invalid: argument 1: "), "{detail}");
    assert!(detail.contains("`q` is not an argument code"), "{detail}");
    let problems = read_problems(serde_json::json!({
        "macro_definitions": { "w": { "args": "{", "delimited_args": "{", "content": "x" } }
    }));
    let both = "the value of `phfqit.macro_definitions.w` is invalid: either `delimited_args` or `args` may be given, not both";
    assert_eq!(problems, [("phfqit.macro_definitions.w".to_string(), both.to_string())]);
}

#[test]
fn a_declared_template_that_does_not_compile_is_reported_when_the_registry_is_built() {
    let registry = registry_with(serde_json::json!({ "macro_definitions": { "w": "#2" } }));
    let identifiers: Vec<&str> = registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    // The one text is compiled in each of the two modes, and fails in each.
    assert_eq!(identifiers, ["flm.userdefs.bad-placeholder", "flm.userdefs.bad-placeholder"]);
}

// --- no panic, no loop -------------------------------------------------------------------

#[test]
fn no_input_panics_or_loops() {
    let registry = registry_with(serde_json::json!({ "macro_definitions": { "self": r"\self" } }));
    let deep = format!("{}x{}", "`(".repeat(50), ")".repeat(50));
    for source in [r"\(\abs`\)", r"\(`\)", r"\(\abs[\Big\)", r"\(`(a\)", r"\(\abs`\Big\)", r"\(\self\)"] {
        let _ = text_of(&registry, source);
    }
    // Each backtick form expands inside the one around it, one level deeper,
    // and the expansion depth limit stops the chain.
    let (_, diagnostics) = text_of(&registry, &format!(r"\({deep}\)"));
    assert!(diagnostics.iter().any(|d| d.starts_with("flm.userdefs.recursion-limit")), "{diagnostics:?}");
    let (text, diagnostics) = text_of(&registry, &format!(r"\({}x{}\)", "`(".repeat(20), ")".repeat(20)));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(text.contains("((x))"), "{text}");
    let (_, diagnostics) = text_of(&registry, r"\(\self\)");
    assert!(diagnostics.iter().any(|d| d.starts_with("flm.userdefs.recursion-limit")), "{diagnostics:?}");
}

#[test]
fn a_group_left_open_after_the_backtick_forms_size_is_reported_and_the_document_rendered() {
    let (text, diagnostics) = text_of(&registry_with(serde_json::json!({})), r"\(`(a\)");
    assert!(
        diagnostics.iter().any(|d| d.starts_with("core.groups.unclosed-group")),
        "{diagnostics:?}"
    );
    assert!(text.contains('a'), "{text}");
}

#[test]
fn a_declared_macro_named_as_another_features_macro_is_the_librarys_ordinary_clash() {
    // `\emph` belongs to the standard package's `textstyles` feature: two
    // features declaring one name in one mode is the clash every registry
    // refuses, with both features named, and nothing of the package's own.
    let rendered = refusal(serde_json::json!({ "macro_definitions": { "emph": "E" } }));
    assert!(rendered.contains("emph"), "{rendered}");
    assert!(rendered.contains("textstyles") && rendered.contains("phfqit"), "{rendered}");
}

#[test]
fn a_template_valid_in_running_text_has_its_sized_delimiters_in_formulas_alone() {
    // `(` parses in running text and `\bigl (` does not: the registry builds
    // without a report, the natural size works in both modes, and a size asked
    // for in running text is an Error at the use.
    let registry = registry_with(serde_json::json!({
        "macro_definitions": { "paren": {
            "args": ["SizeArgBacktick", "SizeArgStar", "SizeArgOptArg", "r()"],
            "argument_number_offset": 3,
            "content": "#[(]{DelimLeft}#1#[)]{DelimRight}",
        } }
    }));
    let reported: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(reported.is_empty(), "{reported:?}");
    assert_eq!(formula(&registry, r"\paren[\big](a)"), r"\bigl (a\bigr )");
    let (text, diagnostics) = text_of(&registry, r"say \paren(a) here");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "say (a) here\n");
    let (_, diagnostics) = text_of(&registry, r"say \paren[\big](a) here");
    assert!(diagnostics.iter().any(|d| d.starts_with("flm.userdefs.bad-placeholder")), "{diagnostics:?}");
}
