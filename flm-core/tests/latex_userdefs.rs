//! The kept alternative of `flm_core::stdcore::features::userdefs` through
//! the `latex-recomposed` backend: a `per-plan` definition's use site is
//! written as a call beside its definition in the style file (`emit-call`),
//! made unconditionally under its recomposed name (every kept definition
//! is renamed under a prefix of its own), or instantiated where
//! the definition cannot be written as one LaTeX definition, which nothing
//! reports — the typed reason is read back off the definition instead; a use
//! site inside mathematics handled the same way
//! by the mathematics writer; the plan of the workflow holds
//! `flm.userdefs.keep` and not `flm.userdefs.expand`; a request may prefer
//! `instantiate-at-lowering` by name on either backend; and the seven staged
//! rows of Python FLM's `test_feature_substmacros.py` recomposer class, each
//! once under `expand: Always` (the Python expectation) and once under the
//! default policy.
//!
//! Every golden holds the recomposed LaTeX, then the style file, under the marker
//! lines of `tests/latex_recomposed.rs`.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{LoweringSelection, Overrides, PlanError};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::diag::{expect, expect_none};
use flm_core::lang::mint::{ArgDef, ArgStdKind, TackOnField};
use flm_core::lang::rewrites::userdefs::{
    Definition, ExpandPolicy, NotRecomposable, RecursionLimit, UserDefinition,
};
use flm_core::model::doc::{Doc, DocContent};
use flm_core::model::element::ElementSpec;
use flm_core::model::names::CapName;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::latex::LatexPackageDef;
use flm_core::stdcore::backends::latex_recomposed::{
    BACKEND, DefinitionNameRefusal, DefinitionNameRefused, DefinitionNameOption,
    LatexRecomposedRenderOptions, WORKFLOW,
};
use flm_core::stdcore::backends::latex::PLACEHOLDER_LATEX_RULE;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::refs::SAFE_LABELS;
use flm_core::stdcore::features::userdefs::{
    UserdefsConfig, element_name, emit_call_rule_name, instantiate_rule_name,
};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use techy::error::Severity;

use common::latex::{assert_golden, body, style, style_if_any, output};
use common::stdcore::{identifiers, plan, run_workflow, try_plan};

// --- helpers -------------------------------------------------------------------------------

/// The registry a host builds over the standard package with `definitions`.
fn registry_over(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

/// Runs the `latex-recomposed` workflow over one document of one tree per
/// text, with no overrides.
fn recompose(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, WORKFLOW, texts, &Overrides::none())
}

/// [`recompose`] over the package with `definitions`, one tree.
fn recomposed(definitions: Vec<Definition>, text: &str) -> Run {
    recompose(&registry_over(definitions), &[text])
}

/// [`recompose`] with the backend's render options set as given.
fn recompose_with(
    registry: &Arc<FlmRegistry>,
    texts: &[&str],
    render: LatexRecomposedRenderOptions,
) -> Run {
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    run_workflow(registry, WORKFLOW, texts, &overrides)
}

/// The render options of a document whose own mathematics names commands
/// `amsmath` defines (`\boldsymbol`, `\lVert`). A mathematics body is written
/// byte for byte, so nothing in it makes the style file request a package; the
/// author says centrally, once, which packages their own mathematics needs.
fn author_needs_amsmath() -> LatexRecomposedRenderOptions {
    LatexRecomposedRenderOptions {
        extra_packages: vec![LatexPackageDef::new(
            "amsmath",
            "what the author's own mathematics names",
        )],
        ..LatexRecomposedRenderOptions::default()
    }
}

/// The identifiers of the registry's build diagnostics, whatever their
/// severity. The `userdefs` feature raises no Note among them at all — see
/// [`no_userdefs_build_diagnostic_is_a_note`].
fn build_diagnostics(registry: &FlmRegistry) -> Vec<String> {
    registry.build_diagnostics().iter().map(|d| d.identifier().to_string()).collect()
}

/// The per-plan definitions of `registry` that are **not** recomposable, each
/// as the definition named and its reason, in the order the registry holds
/// them. Nothing is reported for them: the reason is recorded on the
/// definition and read back here, which is how a host asks.
fn not_recomposable(registry: &FlmRegistry) -> Vec<(String, NotRecomposable)> {
    registry
        .elements()
        .iter()
        .filter_map(|entry| entry.element.value_as::<UserDefinition>())
        .filter(|value| value.definition().expand == ExpandPolicy::PerPlan)
        .filter_map(|value| {
            let reason = value.recomposable().err()?;
            Some((value.definition().display_name(), reason))
        })
        .collect()
}

/// The names of the plan's stages, in the plan's order.
fn stage_names(plan: &flm_core::core::plan::Plan) -> Vec<String> {
    plan.stage_names().iter().map(|name| name.to_string()).collect()
}

/// The names of the plan's rewrites, in the plan's order.
fn rewrite_names(plan: &flm_core::core::plan::Plan) -> Vec<String> {
    plan.rewrite_names().iter().map(|name| name.to_string()).collect()
}

/// A request preferring, on `backend`, the `instantiate-at-lowering` rule of
/// every definition given.
fn prefer_instantiation(backend: &str, definitions: &[Definition]) -> Overrides {
    let mut selection = LoweringSelection::new();
    for definition in definitions {
        selection = selection.prefer(instantiate_rule_name(definition).as_str());
    }
    Overrides { lowering: [(backend.into(), selection)].into_iter().collect(), ..Overrides::none() }
}

// --- the definitions -----------------------------------------------------------------------

/// `\hello{name}`, the spine's per-plan macro.
fn hello() -> Definition {
    Definition::macro_("hello", vec![ArgDef::mandatory("name")]).content("Hello, #{name}!")
}

/// `\begin{note}…\end{note}`, the spine's per-plan inline environment.
fn note() -> Definition {
    Definition::environment("note", Vec::new()).content("[Note: #{body}]")
}

/// `\version`, the spine's `always` macro.
fn version() -> Definition {
    Definition::macro_("version", Vec::new()).content("1.0").expand(ExpandPolicy::Always)
}

/// `\peel{rest}`, a definition that uses itself and stops: it prints its
/// argument and calls itself with an empty one, which the blank test answers.
fn peel() -> Definition {
    Definition::macro_("peel", vec![ArgDef::mandatory("rest")])
        .content("\\ifblank{#{rest}}{.}{#{rest}\\peel{}}")
}

/// Python's `\mymacro[greeting]{name}` with the greeting defaulting to
/// `Hello`, its template using a named and a numbered placeholder.
fn mymacro() -> Definition {
    Definition::macro_("mymacro", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
        .content("\\textbf{#{greeting}}, \\emph{#2}")
        .default("greeting", "Hello")
}

/// `\around[pre]{key}[post]`: an optional argument before the mandatory one and
/// another after it, the trailing one with a default of its own and the
/// leading one with none.
fn around() -> Definition {
    Definition::macro_(
        "around",
        vec![ArgDef::optional("pre"), ArgDef::mandatory("key"), ArgDef::optional("post")],
    )
    .content("\\textbf{#{pre}}\\emph{#2}\\textbf{#{post}}")
    .default("post", "p.")
}

/// `\range{from}[step]{to}`: an optional argument between two mandatory ones,
/// with a default.
fn range() -> Definition {
    Definition::macro_(
        "range",
        vec![ArgDef::mandatory("from"), ArgDef::optional("step"), ArgDef::mandatory("to")],
    )
    .content("#1 to #3 by #{step}")
    .default("step", "one")
}

/// `\sic`, expanded in every plan, standing for a bracketed remark: its
/// expansion opens with a `[`, which is what the emitter's guard is for.
fn sic() -> Definition {
    Definition::macro_("sic", Vec::new()).content("[sic]").expand(ExpandPolicy::Always)
}

/// Python's `\mymacro` whose template tests whether the greeting has a value.
fn mymacro_conditional() -> Definition {
    Definition::macro_("mymacro", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
        .content("\\IfNoValueTF{#1}{Hello, \\emph{#2}}{#1, \\emph{#2}}")
        .default("greeting", "Hello")
}

/// `\mymacro` whose template asks whether the greeting was *written*, over an
/// argument that has a declared default: the one question LaTeX's definition
/// interface cannot answer, since the default is filled in before the body runs.
fn mymacro_provided() -> Definition {
    Definition::macro_("mymacro", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
        .content("\\IfProvidedTF{#1}{#1, \\emph{#2}}{Hello there, \\emph{#2}}")
        .default("greeting", "Hello")
}

/// Python's `{myenv}`, wrapping its body in three brackets.
fn myenv() -> Definition {
    Definition::environment("myenv", Vec::new()).content("[[[#{body}]]]")
}

/// Python's `\calN`, a definition applying in mathematics mode alone.
fn caln() -> Definition {
    Definition::macro_("calN", Vec::new()).math("\\mathcal{N}")
}

/// Python's `@` specials.
fn at() -> Definition {
    Definition::specials("@", Vec::new()).content("[at]")
}

/// The same definition, expanded in every plan.
fn always(definition: Definition) -> Definition {
    definition.expand(ExpandPolicy::Always)
}

// --- the plan ------------------------------------------------------------------------------

/// Asserts that the run recomposed no user definition: either it wrote no
/// style file at all — a document that needs nothing needs none — or the
/// style file it wrote holds no user-definition block.
fn assert_no_user_definition(run: &Run) {
    if let Some((style, _)) = style_if_any(run) {
        assert!(!style.contains("user definitions"), "{style}");
    }
}

#[test]
fn the_recomposed_plan_keeps_the_per_plan_definitions_and_the_check_plan_is_unchanged() {
    let registry = registry_over(vec![hello(), note(), version()]);
    let recomposed = plan(&registry, WORKFLOW, &Overrides::none());
    assert_eq!(rewrite_names(&recomposed), ["flm.input.splice", "flm.userdefs.expand-always"]);
    // The workflow demands the validation collection beside its output, so the
    // three checks and the stages they read are here too.
    assert_eq!(
        stage_names(&recomposed),
        [
            "flm.blocks.raw",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.refs.safe-labels",
            "flm.refs.value-safe-labels",
            "flm.theorems.check-proofs",
            "flm.graphics.inspect",
            "flm.graphics.publish.latex-recomposed",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.userdefs.keep",
            "flm.blocks.check-contract",
            "flm.meta.build",
            "flm.latex-recomposed.pieces",
            "flm.latex-recomposed.lower",
            "flm.latex-recomposed.emit",
            "flm.latex-recomposed.regions",
            "flm.latex-recomposed.final",
            "flm.math-passthrough.check",
        ]
    );
    let selected = recomposed.selection(BACKEND).expect("the backend is active");
    for definition in [hello(), note()] {
        let rule = selected.by_element.get(&element_name(&definition)).expect("a rule");
        assert_eq!(registry.rule(*rule).rule.name, emit_call_rule_name(&definition));
    }
    // The check workflow demands the expanded alternative, as before.
    let check = plan(&registry, CHECK_WORKFLOW, &Overrides::none());
    assert_eq!(
        rewrite_names(&check),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    assert!(!stage_names(&check).iter().any(|name| name == "flm.userdefs.keep"));
    // And the text workflow expands, as before: the general default stands
    // where no backend-specific rule is marked. Its stage list is pinned in
    // full, so that a change to the ladder or to the placeholder rule's
    // declaration fails here rather than silently changing this backend's
    // plan. The placeholder rule is marked for the recomposing backend, so it
    // is no candidate here: `flm.placeholder` has no rule on the text backend,
    // which is no error, since no placeholder survives expansion and the
    // element is outside the completeness check.
    let text_plan = plan(&registry, text::WORKFLOW, &Overrides::none());
    let text_selection = text_plan.selection(text::BACKEND).expect("text is active");
    assert!(text_selection.rule_for("flm.placeholder").is_none());
    let recomposed_selection = plan(&registry, WORKFLOW, &Overrides::none())
        .selection(BACKEND)
        .expect("the recomposing backend is active")
        .rule_for("flm.placeholder")
        .expect("the placeholder rule is selected where it is marked");
    assert_eq!(registry.rule(recomposed_selection).rule.name, PLACEHOLDER_LATEX_RULE);
    assert_eq!(
        rewrite_names(&text_plan),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    assert_eq!(
        stage_names(&text_plan),
        [
            "flm.blocks.check-contract",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.xnotes.collect",
            "flm.xnotes.build-list.footnote",
            "flm.theorems.check-proofs",
            "flm.graphics.inspect",
            "flm.graphics.publish.text",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.citations.build-bibliography",
            "flm.meta.build",
            "flm.text.pieces",
            "flm.text.lower",
            "flm.text.emit",
            "flm.text.regions",
            "flm.text.final",
            "flm.math-passthrough.check",
        ]
    );
}

#[test]
fn the_kept_rules_are_registered_per_per_plan_definition_and_the_placeholder_rule_once() {
    let registry = registry_over(vec![hello(), version()]);
    assert!(registry.rule_idx(instantiate_rule_name(&hello()).as_str()).is_some());
    assert!(registry.rule_idx(emit_call_rule_name(&hello()).as_str()).is_some());
    assert!(registry.rule_idx(instantiate_rule_name(&version()).as_str()).is_none());
    assert!(registry.rule_idx(emit_call_rule_name(&version()).as_str()).is_none());
    assert!(registry.rule_idx("flm.placeholder.lower.latex-recomposed").is_some());
    assert_eq!(registry.set_aside_rules().count(), 0);
    assert!(build_diagnostics(&registry).is_empty(), "{:?}", build_diagnostics(&registry));
}

// --- the acceptance: the spine's definitions -------------------------------------------------

#[test]
fn the_spines_definitions_are_recomposed_as_calls_beside_their_definitions() {
    let document = "\
\\hello{World} from a user macro, and \\begin{note}a user environment\\end{note}
in the same paragraph. It is version \\version.
";
    let run = recomposed(vec![hello(), note(), version()], document);
    assert_eq!(
        output(&run),
        "\\flmUDhello{World} from a user macro, and \
         \\begin{flmUEnote}a user environment\\end{flmUEnote}\n\
         in the same paragraph. It is version 1.0.\n%%\n"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDhello[1]{Hello, #1!}\n"), "{style}");
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUEnote}{}{[Note: }{]}\n"),
        "{style}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-spine.tex", &run);
}

#[test]
fn a_definition_is_guarded_and_its_comment_line_says_so() {
    // No TeX runs here: the shape is asserted on the emitted text. A
    // definition written under a prefix of its own is guarded, so a definition
    // of that name made in the user's own class or preamble, before the style
    // file is loaded, wins; the comment line says so and names the command
    // that changes it after loading.
    let run = recomposed(vec![hello(), note()], "\\hello{x} \\begin{note}y\\end{note}");
    let (style, _) = style(&run);
    assert!(!style.contains("\\ifdefined"), "{style}");
    assert!(!style.contains("\\@ifundefined"), "{style}");
    assert!(!style.contains("\\newcommand"), "{style}");
    assert!(
        style.contains(
            "% \\flmUDhello (the document's \\hello): defined unless the name is already taken, so a definition of it made before this file is loaded wins; \\renewcommand after loading this file changes it\n\\providecommand\\flmUDhello[1]{Hello, #1!}\n"
        ),
        "{style}"
    );
    assert!(
        style.contains(
            "% flmUEnote (the document's note): defined unless the name is already taken, so a definition of it made before this file is loaded wins; \\RenewDocumentEnvironment after loading this file changes it\n\\ProvideDocumentEnvironment{flmUEnote}{}{[Note: }{]}\n"
        ),
        "{style}"
    );
}

// --- one definition per name across use sites and trees ---------------------------------------

#[test]
fn a_definition_used_twice_in_one_tree_and_in_two_trees_is_in_the_style_file_once() {
    let registry = registry_over(vec![hello()]);
    let run = recompose(&registry, &["\\hello{a} and \\hello{b}.", "Again \\hello{c}."]);
    assert_eq!(
        output(&run),
        "\\flmUDhello{a} and \\flmUDhello{b}.%%\nAgain \\flmUDhello{c}.%%\n"
    );
    let (style, _) = style(&run);
    assert_eq!(style.matches("\\providecommand\\flmUDhello").count(), 1, "{style}");
    assert_golden("userdefs-two-trees.tex", &run);
}

// --- the shapes of a recomposable definition -------------------------------------------------

#[test]
fn a_leading_optional_argument_is_written_with_its_default_and_at_the_use_site_when_written() {
    let run = recomposed(vec![mymacro()], "\\mymacro[Hi]{Albert} and \\mymacro{Berta}.");
    assert_eq!(output(&run), "\\flmUDmymacro{Hi}{Albert} and \\flmUDmymacro{Hello}{Berta}.%%\n");
    let (style, _) = style(&run);
    assert!(
        style.contains(
            "\\providecommand\\flmUDmymacro[2]{\\textbf{#1}, \\emph{#2}}\n"
        ),
        "{style}"
    );
    assert_golden("userdefs-optional.tex", &run);
}

#[test]
fn an_optional_argument_stands_anywhere_and_each_one_carries_its_own_default() {
    // An optional argument stands wherever the definition declares it, and
    // the call writes one group for it wherever it stands: what the use site
    // wrote, or the declared default, or nothing. So the recomposed definition
    // takes three plain arguments and the brackets are gone from every call.
    let document = "\
\\section{Sources \\around[see]{Knuth} and \\range{1}[2]{9}}\\label{sec:opt}

\\range{1}{9}, \\range{1}[2]{9}.

\\around{Knuth}, \\around[see]{Knuth}, \\around{Knuth}[p. 7], \\around[see]{Knuth}[p. 7].

See \\ref{sec:opt}.
";
    let run = recomposed(vec![around(), range()], document);
    let text = body(&run);
    // The middle optional argument, written and left out: its group holds the
    // declared default `one` where the use site wrote nothing.
    assert!(text.contains("\\flmUDrange{1}{one}{9}, \\flmUDrange{1}{2}{9}."), "{text}");
    // The leading one, the trailing one, both and neither. Every call writes
    // three groups, whatever the use site left out, so nothing follows a call
    // that LaTeX could read as an argument.
    assert!(
        text.contains(
            "\\flmUDaround{}{Knuth}{p.}, \\flmUDaround{see}{Knuth}{p.}, \\flmUDaround{}{Knuth}{p. 7}, \\flmUDaround{see}{Knuth}{p. 7}."
        ),
        "{text}"
    );
    // The piece's form inside a heading title, which is a moving argument.
    assert!(
        text.contains(
            "\\section{Sources \\flmUDaround{see}{Knuth}{p.} and \\flmUDrange{1}{2}{9}}"
        ),
        "{text}"
    );
    let (style, _) = style(&run);
    assert!(
        style.contains(
            "\\providecommand\\flmUDaround[3]{\\textbf{#1}\\emph{#2}\\textbf{#3}}\n"
        ),
        "{style}"
    );
    assert!(
        style.contains("\\providecommand\\flmUDrange[3]{#1 to #3 by #2}\n"),
        "{style}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-optional-anywhere.tex", &run);
}

#[test]
fn an_optional_argument_anywhere_prints_what_the_expanded_document_prints() {
    // Three use sites, expanded instead of recomposed rather than kept as
    // calls: the text is the same either way, so the widened argument
    // specification says what the definition says.
    let document = "\\range{1}{9} \\around{Knuth} \\around[see]{Knuth}[p. 7]";
    let expanded = recomposed(vec![always(around()), always(range())], document);
    assert_eq!(
        output(&expanded),
        "1 to 9 by one \\textbf{}\\emph{Knuth}\\textbf{p.} \\textbf{see}\\emph{Knuth}\\textbf{p. 7}%%\n"
    );
    let kept = recomposed(vec![around(), range()], document);
    assert_eq!(
        output(&kept),
        "\\flmUDrange{1}{one}{9} \\flmUDaround{}{Knuth}{p.} \\flmUDaround{see}{Knuth}{p. 7}%%\n"
    );
}

#[test]
fn an_ordinary_bracket_after_a_call_needs_no_guard() {
    // A canonical call ends at its last brace group, and the definition
    // declares no optional argument at all, so LaTeX looks for nothing after
    // one: a `[` the document means as an ordinary bracket is printed as it
    // stands, wherever it follows. Here the `[` opens the expansion of
    // `\sic`, which FLM never read as an argument of `\around`. The stop token
    // the earlier form needed after such a call is gone with the search it
    // ended.
    let document = "\\around{Knuth}\\sic \\around{Knuth} \\sic \\around{Knuth}%%aside\n\\sic \\around{Knuth}[p. 7]\\sic. Now \\around{Knuth}more";
    let run = recompose(&registry_over(vec![around(), sic()]), &[document]);
    let text = body(&run);
    assert!(text.contains("\\flmUDaround{}{Knuth}{p.}[sic]"), "{text}");
    assert!(text.contains("\\flmUDaround{}{Knuth}{p.} [sic]"), "{text}");
    assert!(text.contains("{p.}%%aside\n[sic]"), "{text}");
    assert!(text.contains("\\flmUDaround{}{Knuth}{p. 7}[sic]"), "{text}");
    assert!(text.contains("\\flmUDaround{}{Knuth}{p.}more"), "{text}");
    assert!(!text.contains("\\relax"), "{text}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-optional-bracket.tex", &run);
}

#[test]
fn a_call_that_ends_a_tree_needs_no_guard_either() {
    // A document is recomposed one parsed fragment at a time and the
    // fragments are joined by the comment line `%%`. A comment does not end
    // LaTeX's search for an argument — which is why the earlier form had to
    // put a stop token after such a call — but a canonical call leaves no
    // search open, so the bracket the next fragment opens with is printed.
    let run = recompose(&registry_over(vec![around(), sic()]), &["\\around{Knuth}", "\\sic"]);
    let text = body(&run);
    assert!(text.ends_with("\\flmUDaround{}{Knuth}{p.}%%\n[sic]%%\n"), "{text}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-optional-tree-end.tex", &run);
}

#[test]
fn a_template_using_an_always_definition_is_recomposed_with_the_expansion_in_its_body() {
    let greet = Definition::macro_("greet", Vec::new()).content("Hello from \\version");
    let run = recomposed(vec![greet, version()], "\\greet, \\greet.");
    // The whitespace after `\greet` is part of the invocation and is not
    // written back; the emitter keeps the comma apart on its own.
    assert_eq!(output(&run), "\\flmUDgreet, \\flmUDgreet.%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDgreet{Hello from 1.0}"), "{style}");
    assert_golden("userdefs-uses-always.tex", &run);
}

#[test]
fn a_template_using_a_recomposable_definition_puts_both_in_the_style_file() {
    let greet = Definition::macro_("greet", vec![ArgDef::mandatory("who")])
        .content("\\hello{#1} \\emph{again}");
    let run = recomposed(vec![greet, hello()], "\\greet{you}");
    assert_eq!(output(&run), "\\flmUDgreet{you}%%\n");
    let (style, _) = style(&run);
    let hello =
        style.find("\\providecommand\\flmUDhello[1]{Hello, #1!}").expect("hello");
    let greet = style
        .find("\\providecommand\\flmUDgreet[1]{\\flmUDhello{#1} \\emph{again}}")
        .expect("greet");
    assert!(hello < greet, "{style}");
    assert_golden("userdefs-uses-definition.tex", &run);
}

#[test]
fn a_use_site_nested_in_another_definitions_argument_is_a_nested_call() {
    let run = recomposed(vec![hello(), note()], "\\hello{\\begin{note}\\hello{x}\\end{note}}");
    assert_eq!(output(&run), "\\flmUDhello{\\begin{flmUEnote}\\flmUDhello{x}\\end{flmUEnote}}%%\n");
    let (style, _) = style(&run);
    assert_eq!(style.matches("\\providecommand\\flmUDhello").count(), 1);
    assert_eq!(style.matches("\\ProvideDocumentEnvironment{flmUEnote}").count(), 1);
    assert_golden("userdefs-nested.tex", &run);
}

#[test]
fn a_template_holding_a_literal_writes_the_literals_spelling_in_the_definition() {
    let sure = Definition::macro_("sure", Vec::new()).content("100\\% sure~yes");
    let run = recomposed(vec![sure], "I am \\sure.");
    assert_eq!(output(&run), "I am \\flmUDsure.%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDsure{100\\% sure~yes}"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-literal.tex", &run);
}

// --- verbatim and key-value arguments -------------------------------------------------------

/// `\kv[a=1,b=2]{x}`: a key-value argument before a mandatory one.
fn kv() -> Definition {
    Definition::macro_("kv", vec![ArgDef::named("opts", ArgStdKind::KeyVals), ArgDef::mandatory("x")])
        .content("[#1|#2]")
}

/// `\code|…|`: one verbatim argument, shown in bold.
fn code() -> Definition {
    Definition::macro_("code", vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })])
        .content("\\textbf{#1}")
}

#[test]
fn a_key_value_argument_is_a_canonical_value_written_and_left_out() {
    // FLM never reads the keys of a key-value argument apart: the argument is
    // parsed exactly like an optional one and its content is substituted whole.
    // So its effective argument is an ordinary value group holding what the use
    // site wrote, and nothing at all about the recomposition is special to it.
    let document = "\
\\section{Title \\kv[k=v]{t}}\\label{sec:kv}

1:\\kv[a=1,b=2]{t} 2:\\kv{t} 3:\\kv[]{t}

See \\ref{sec:kv}.
";
    let kept = recomposed(vec![kv()], document);
    let (style, _) = style(&kept);
    assert!(style.contains("\\providecommand\\flmUDkv[2]{[#1|#2]}\n"), "{style}");
    let text = body(&kept);
    assert!(text.contains("\\section{Title \\flmUDkv{k=v}{t}}"), "{text}");
    assert!(
        text.contains("1:\\flmUDkv{a=1,b=2}{t} 2:\\flmUDkv{}{t} 3:\\flmUDkv{}{t}"),
        "{text}"
    );
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-keyvals.tex", &kept);
    // The same document expanded: an argument the writer left out and one
    // written empty print alike, which is what the two empty groups say.
    let expanded = recomposed(vec![always(kv())], document);
    assert!(body(&expanded).contains("1:[a=1,b=2|t] 2:[|t] 3:[|t]"), "{}", body(&expanded));
}

#[test]
fn a_verbatim_argument_is_written_as_its_characters_in_its_value_group() {
    // A placeholder stands for the argument's content, the characters between
    // the delimiters, whatever kind the argument was declared; the delimiters
    // are the syntax of the use site. So the recomposition writes those
    // characters into the argument's value group as text, like any other
    // content, and the template's `\textbf` sets them as text. The call is an
    // ordinary canonical call, one brace group per argument: no `v` argument
    // is declared anywhere, which is what lets the definition stay a plain
    // expandable macro.
    let document = "\
\\section{Title \\code|t_1|}\\label{sec:code}

1:\\code|a_b| 2:\\code{c d} 3:\\code<e|f>

See \\ref{sec:code}.
";
    let kept = recomposed(vec![code()], document);
    let (style, _) = style(&kept);
    assert!(style.contains("\\providecommand\\flmUDcode[1]{\\textbf{#1}}\n"), "{style}");
    let text = body(&kept);
    // Two delimiters the writer chose — a bar, braces, angle brackets — and
    // one recomposed spelling: the delimiters were the FLM source's business
    // and are gone.
    assert!(text.contains("1:\\flmUDcode{a\\_b} 2:\\flmUDcode{c d} 3:\\flmUDcode{e|f}"), "{text}");
    assert!(text.contains("\\section{Title \\flmUDcode{t\\_1}}"), "{text}");
    assert_eq!(not_recomposable(&registry_over(vec![code()])), []);
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-verbatim.tex", &kept);
    // The expanded document writes the same characters in the same
    // spelling, inside the definition's own `\textbf` rather than through a
    // call of it.
    let expanded = recomposed(vec![always(code())], document);
    assert!(body(&expanded).contains("1:\\textbf{a\\_b}"), "{}", body(&expanded));
}

#[test]
fn a_verbatim_argument_holding_special_characters_is_escaped_once_and_only_there() {
    // The characters that would end a LaTeX argument, start a comment, or be
    // read as a parameter, inside one verbatim argument. The argument stands
    // for those characters, and the template sets them as text, so each is
    // written the way text writes it (`\#`, `\%`, `\textbackslash{}`, `\{`);
    // the compile suite proves that the result sets those characters.
    let document = "1:\\code|a#b%c\\d_e{f}|\n";
    let kept = recomposed(vec![code()], document);
    let text = body(&kept);
    assert!(text.contains("\\flmUDcode{a\\#b\\%c\\textbackslash{}d\\_e\\{f\\}}"), "{text}");
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-verbatim-specials.tex", &kept);
}

#[test]
fn a_template_holding_verbatim_text_with_a_number_sign_needs_no_doubling() {
    // A literal `#` inside a LaTeX definition's body is read as a parameter
    // number unless it is written twice. It never happens here, and the definition below is the
    // proof the compile suite runs: ordinary text escapes `#` as `\#`, and
    // verbatim content is written in the `\EscVerb` encoding, which puts a
    // backslash before every printable character that is not a letter or a
    // digit — so what reaches the body is the control sequence `\#`, one
    // token, and never a parameter mark. `##` in the template is how a
    // template writes one number sign.
    let sharp = Definition::macro_("sharp", vec![ArgDef::mandatory("x")])
        .content("\\verbcode|a##b| and ## and #1");
    let document = "\
\\section{Title \\sharp{s}}\\label{sec:sharp}

\\sharp{t}

See \\ref{sec:sharp}.
";
    let kept = recomposed(vec![sharp.clone()], document);
    let (style, _) = style(&kept);
    assert!(
        style.contains(
            "\\providecommand\\flmUDsharp[1]\
             {\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{a\\#b} and \\# and #1}\n"
        ),
        "{style}"
    );
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-verbatim-in-template.tex", &kept);
    // The expanded document sets the same characters.
    let expanded = recomposed(vec![always(sharp)], document);
    assert!(
        body(&expanded)
            .contains("\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{a\\#b} and \\# and t"),
        "{}",
        body(&expanded)
    );
}

#[test]
fn a_template_holding_a_verbatim_block_is_recomposed_with_the_escaped_block() {
    // A verbatim *block* — `\begin{verbatimcode}…\end{verbatimcode}` — in a
    // template's own content is no obstacle. A definition's body in the style
    // file is a token list, and LaTeX gives a character its meaning as it
    // reads it out of the file, so a block written out in full could not
    // stand there; written as the escaped call `\flmVerbatimBlock`, which
    // rebuilds the block where it stands, it is an ordinary macro call over
    // brace groups and stands wherever any call stands. Both definitions are
    // therefore recomposed, and both use sites are calls.
    let listing = Definition::macro_("listing", vec![ArgDef::mandatory("lang")])
        .content("\\begin{verbatimcode}[#1]\nf(x)\n\\end{verbatimcode}");
    let framed = Definition::environment("framed", Vec::new())
        .content("#{body}\n\\begin{verbatimcode}\nfixed\n\\end{verbatimcode}");
    let registry = registry_over(vec![listing, framed]);
    assert_eq!(not_recomposable(&registry), []);
    let run = recompose(&registry, &["\\listing{python}\n\n\\begin{framed}b\\end{framed}\n"]);
    let text = body(&run);
    // Both use sites are calls; no block is written out in full in the
    // document, and nothing is expanded.
    assert!(text.contains("\\flmUDlisting{python}"), "{text}");
    assert!(text.contains("\\begin{flmUEframed}b\\end{flmUEframed}"), "{text}");
    assert!(!text.contains("\\begin{flmVerbatimCode}"), "{text}");
    // The style file holds both definitions, each with the block written as
    // the escaped call: the language placeholder in the environment's own
    // optional argument, the content as brace groups.
    let (style, _) = style(&run);
    assert!(
        style.contains(
            "\\providecommand\\flmUDlisting[1]{\\flmVerbatimBlock[#1]{flmVerbatimCode}{{{f\\(x\\)}}}}"
        ),
        "{style}"
    );
    assert!(
        style.contains("\\flmVerbatimBlock[]{flmVerbatimCode}{{{fixed}}}"),
        "{style}"
    );
    // Nothing is reported: the definitions are recomposed and so are the use
    // sites.
    assert_eq!(run.diagnostics().iter().count(), 0);
    // The same document with both definitions marked always expanded writes
    // the same two blocks out in full, with the same content and the same
    // language tag: the escaped call and the block written out in full set the
    // same block (`tests/latex_compile.rs` puts both to pdflatex).
    let expanded = recomposed(
        vec![
            always(
                Definition::macro_("listing", vec![ArgDef::mandatory("lang")])
                    .content("\\begin{verbatimcode}[#1]\nf(x)\n\\end{verbatimcode}"),
            ),
            always(
                Definition::environment("framed", Vec::new())
                    .content("#{body}\n\\begin{verbatimcode}\nfixed\n\\end{verbatimcode}"),
            ),
        ],
        "\\listing{python}\n\n\\begin{framed}b\\end{framed}\n",
    );
    let expanded = body(&expanded);
    assert!(expanded.contains("\\begin{flmVerbatimCode}[python]\nf(x)\n\\end{flmVerbatimCode}"), "{expanded}");
    assert!(expanded.contains("\\begin{flmVerbatimCode}[]\nfixed\n\\end{flmVerbatimCode}"), "{expanded}");
    assert_golden("userdefs-verbatim-block-in-template.tex", &run);
}

#[test]
fn a_verbatim_block_in_a_body_taken_as_an_argument_is_written_as_the_escaped_block() {
    // An environment that takes its body as an argument (`+b`) has LaTeX hand
    // the body over as a group of tokens, so a block written out in full
    // cannot stand there. The emitter writes the escaped call instead, and
    // both use sites are ordinary calls. The definition here sets its body
    // twice as two paragraphs, which is what makes it a `+b` environment at
    // all.
    let stack = Definition::environment("stack", Vec::new()).content("#{body}\n\n#{body}");
    let document = "\
\\begin{stack}an \\verbtext|inline| body\\end{stack}

\\begin{stack}\\begin{verbatimcode}\nf(x)\n\\end{verbatimcode}\\end{stack}
";
    let registry = registry_over(vec![stack]);
    let run = recompose(&registry, &[document]);
    let text = body(&run);
    // The first use site: a call, the inline verbatim content inside the body
    // that `+b` hands over.
    assert!(
        text.contains(
            "\\begin{flmUEstack}an \\EscVerb[formatcom=\\flmFmtVRB{verbtext}]{inline} body\\end{flmUEstack}"
        ),
        "{text}"
    );
    // The second: a call too, the block written as the escaped call inside
    // the body the environment hands over.
    assert!(
        text.contains(
            "\\begin{flmUEstack}\\flmVerbatimBlock[]{flmVerbatimCode}{{{f\\(x\\)}}}\\end{flmUEstack}"
        ),
        "{text}"
    );
    assert!(!text.contains("\\begin{flmVerbatimCode}"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUEstack}{+b}"), "{style}");
    // Nothing is reported, about the definition or about either use site.
    assert_eq!(not_recomposable(&registry), []);
    assert_eq!(run.diagnostics().iter().count(), 0);
    // The same document with the definition marked always expanded writes the
    // block out in full, twice, with the same content.
    let expanded = recomposed(
        vec![always(Definition::environment("stack", Vec::new()).content("#{body}\n\n#{body}"))],
        document,
    );
    let expanded = body(&expanded);
    assert_eq!(
        expanded.matches("\\begin{flmVerbatimCode}[]\nf(x)\n\\end{flmVerbatimCode}").count(),
        2,
        "{expanded}"
    );
    assert_golden("userdefs-body-argument-verbatim.tex", &run);
}

#[test]
fn a_verbatim_block_in_any_value_a_call_carries_is_written_as_the_escaped_block() {
    // The three routes a block takes into a group LaTeX reads as tokens: the
    // body an environment takes as an argument, the value a use site wrote
    // for an argument, and the value a declared default resolves to where the
    // use site wrote none. Each is written as the escaped call, and each use
    // site is a call.
    let block = "\\begin{verbatimcode}\nf(x)\n\\end{verbatimcode}";
    // Route 1: the body of a body-grabbing environment (`#{body}` twice).
    let stack = Definition::environment("stack", Vec::new()).content("#{body}\n\n#{body}");
    // Route 2: the value the use site wrote for an argument.
    let plain = Definition::macro_("plain", vec![ArgDef::mandatory("x")]).content("#{x}");
    // Route 3: the value a declared default resolves to.
    let listing = Definition::macro_("listing", vec![ArgDef::optional("code")])
        .content("#{code}")
        .default("code", block);
    let definitions = vec![stack, plain, listing];
    let registry = registry_over(definitions.clone());
    assert_eq!(not_recomposable(&registry), []);
    for (display, document) in [
        ("\\begin{stack}", format!("\\begin{{stack}}{block}\\end{{stack}}\n")),
        ("\\plain", format!("\\plain{{{block}}}\n")),
        ("\\listing", String::from("\\listing\n")),
    ] {
        let run = recompose(&registry, &[document.as_str()]);
        let text = body(&run);
        // The block travels inside the call, as the escaped call; nothing is
        // written out in full and nothing is expanded.
        assert!(
            text.contains("\\flmVerbatimBlock[]{flmVerbatimCode}{{{f\\(x\\)}}}"),
            "{display}: {text}"
        );
        assert!(!text.contains("\\begin{flmVerbatimCode}"), "{display}: {text}");
        let call = format!("flmU{}", if display.starts_with("\\begin") { "E" } else { "D" });
        assert!(text.contains(&call), "{display}: {text}");
        // No Note at all: nothing is decided per use site any more.
        assert_eq!(run.diagnostics().iter().count(), 0, "{display}");
    }
}

#[test]
fn a_block_a_call_carries_keeps_its_lines_its_tabs_and_its_empty_lines() {
    // The content of an escaped block is one brace group per line and, inside
    // each line, one brace group per tab-separated segment. Every other test
    // of the escaped block carries one line of one segment, which says nothing
    // about the splitting; this one carries the whole content list: tabs, `#`,
    // `%`, `\`, an unbalanced closing brace, a line ending in a backslash,
    // runs of spaces, an empty line inside the block, non-ASCII text, and a
    // content ending in a line feed — which must **not** open an empty last
    // line group, since a block written out in full writes no empty last line
    // either.
    let plain = Definition::macro_("plain", vec![ArgDef::mandatory("x")]).content("[#{x}]");
    let block = "\\begin{verbatimcode}\n\
                 a\tb\tc\n\
                 f(x) = {y} % 100 # h\n\
                 \n\
                 } unbalanced and \\ backslash\n\
                 \u{20}\u{20}\u{20}\u{20}four spaces, then a line ending in \\\n\
                 café — naïve\n\
                 \\end{verbatimcode}";
    let document = format!("\\plain{{{block}}}\n");
    let registry = registry_over(vec![plain]);
    assert_eq!(not_recomposable(&registry), []);
    let run = recompose(&registry, &[document.as_str()]);
    let text = body(&run);
    // One line group per line, one segment group per tab, and six line groups
    // for six lines: the trailing line feed ended the sixth rather than
    // opening a seventh.
    assert!(
        text.contains(
            "\\flmUDplain{\\flmVerbatimBlock[]{flmVerbatimCode}{\
             {{a}{b}{c}}\
             {{f\\(x\\)\\ \\=\\ \\{y\\}\\ \\%\\ 100\\ \\#\\ h}}\
             {{}}\
             {{\\}\\ unbalanced\\ and\\ \\\\\\ backslash}}\
             {{\\ \\ \\ \\ four\\ spaces\\,\\ then\\ a\\ line\\ ending\\ in\\ \\\\}}\
             {{café\\ —\\ naïve}}}}"
        ),
        "{text}"
    );
    // Six line groups for six lines. The content ends in a line feed — the
    // parse keeps the one before `\end` — and that line feed ends the sixth
    // line rather than opening an empty seventh, which is what a block written
    // out in full does too.
    assert_eq!(text.matches("}}{{").count(), 5, "{text}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    // The golden is compiled by `tests/latex_compile.rs`, with and without
    // `hyperref`.
    assert_golden("userdefs-verbatim-block-lines.tex", &run);
}

#[test]
fn a_block_in_a_body_latex_sets_itself_is_written_out_in_full() {
    // The other half of the placement rule. An environment whose template
    // names its body once, among its own top-level nodes, is cut in two there:
    // LaTeX sets the body between the begin code and the end code, reading it
    // from lines of the file, which is exactly where a block written out in
    // full belongs. So a block a use site writes in such a body stays written
    // out in full, and only the call around it is new.
    let note = Definition::environment("note", Vec::new()).content("[note: #{body}]");
    let document = "\\begin{note}\n\\begin{verbatimcode}\nf(x)\n\\end{verbatimcode}\n\\end{note}\n";
    let registry = registry_over(vec![note]);
    let run = recompose(&registry, &[document]);
    let text = body(&run);
    assert!(
        text.contains("\\begin{flmUEnote}\n\\begin{flmVerbatimCode}[]\nf(x)\n\\end{flmVerbatimCode}\n\\end{flmUEnote}"),
        "{text}"
    );
    assert!(!text.contains("flmVerbatimBlock"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUEnote}{}{[note: }{]}"), "{style}");
    assert_golden("userdefs-split-body-verbatim.tex", &run);
}

#[test]
fn many_use_sites_holding_a_block_are_all_calls_and_report_nothing() {
    // What used to be one Note per definition per output tree is now no
    // message at all: a document writing a code block in three bodies of one
    // environment and in an argument of another definition is written as four
    // calls, each carrying its block as the escaped call.
    let stack = Definition::environment("stack", Vec::new()).content("#{body}\n\n#{body}");
    let plain = Definition::macro_("plain", vec![ArgDef::mandatory("x")]).content("#{x}");
    let block = "\\begin{verbatimcode}\nf(x)\n\\end{verbatimcode}";
    let document = format!(
        "\\begin{{stack}}{block}\\end{{stack}}\n\n\\begin{{stack}}{block}\\end{{stack}}\n\n\\begin{{stack}}{block}\\end{{stack}}\n\n\\plain{{{block}}}\n"
    );
    let registry = registry_over(vec![stack, plain]);
    let run = recompose(&registry, &[document.as_str()]);
    let text = body(&run);
    assert_eq!(text.matches("\\flmVerbatimBlock[]{flmVerbatimCode}{{{f\\(x\\)}}}").count(), 4, "{text}");
    assert_eq!(text.matches("\\begin{flmUEstack}").count(), 3, "{text}");
    assert_eq!(text.matches("\\flmUDplain{").count(), 1, "{text}");
    assert_eq!(run.diagnostics().iter().count(), 0);
}

#[test]
fn a_blank_test_over_a_verbatim_argument_answers_what_flm_answers() {
    // A verbatim argument stands for its characters, so FLM's own blank test
    // answers "blank" for an empty verbatim argument and "not blank" for one
    // holding a character. The recomposed value group holds those characters,
    // nothing for the empty one, so LaTeX's test answers the same: the two
    // agree by construction rather than by accident. `tests/latex_compile.rs`
    // puts the same two calls to pdflatex.
    let ask = Definition::macro_("ask", vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })])
        .content("\\ifblank{#1}{BLANK}{FULL}");
    let document = "1:\\ask|x| 2:\\ask||\n";
    let definitions = vec![ask];
    let kept = recomposed(definitions.clone(), document);
    let (style, _) = style(&kept);
    assert!(
        style.contains("\\providecommand\\flmUDask[1]{\\flmIfBlankTF{#1}{BLANK}{FULL}}\n"),
        "{style}"
    );
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-verbatim-blank.tex", &kept);
    // FLM's own answers, which the compile suite compares LaTeX's against.
    let registry = registry_over(definitions);
    let on_text = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    let printed = common::lowering::text(&on_text);
    assert!(printed.contains("1:FULL 2:BLANK"), "{printed}");
}

// --- one name declared for two parsing modes: the mode twins ---------------------------------

/// `\vec` declared twice: bold in text, a bold symbol in mathematics, with a
/// decoration the mathematics one takes and the text one does not.
fn vec_twins() -> Vec<Definition> {
    vec![
        Definition::macro_("vec", vec![ArgDef::mandatory("x")]).text("\\textbf{#1}"),
        Definition::macro_("vec", vec![ArgDef::optional("deco"), ArgDef::mandatory("x")])
            .math("\\mathbf{#2}#1"),
    ]
}

#[test]
fn a_name_declared_for_two_modes_is_recomposed_as_two_definitions_with_two_names() {
    // FLM allows one name to stand for two different things, one in text and
    // one inside mathematics; LaTeX has one command of that name. So the two
    // definitions — the *mode twins* — are written under two names, and the
    // recomposition writes the one the use site's mode calls for. It can,
    // because it knows the mode: `emit-call` runs in text and the mathematics
    // writer inside mathematics. LaTeX is never asked.
    let document = "\\vec{v} and \\(\\vec{w} + \\vec[\\prime]{u}\\).";
    let kept = recomposed(vec_twins(), document);
    assert_eq!(
        output(&kept),
        "\\flmUTDvec{v} and \\(\\flmUMDvec{}{w} + \\flmUMDvec{\\prime}{u}\\).%%\n"
    );
    let (style, _) = style(&kept);
    // Two definitions, two names, and each takes the arguments its own
    // declaration gives: the text twin one, the mathematics twin two.
    assert!(style.contains("\\providecommand\\flmUTDvec[1]{\\textbf{#1}}\n"), "{style}");
    assert!(style.contains("\\providecommand\\flmUMDvec[2]{\\mathbf{#2}#1}\n"), "{style}");
    // Neither name begins with `flmUD`, which is the space of the names the
    // document declares once, so no definition of the document can meet them.
    assert!(!style.contains("flmUDvec"), "{style}");
    assert_eq!(not_recomposable(&registry_over(vec_twins())), []);
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-mode-twins.tex", &kept);
    // What the expanded document prints, which the recomposed one must print
    // too: the text twin in text and the mathematics twin inside mathematics.
    let expanded = recomposed(
        vec_twins().into_iter().map(always).collect(),
        document,
    );
    assert_eq!(
        output(&expanded),
        "\\textbf{v} and \\(\\mathbf{w} + \\mathbf{u}\\prime\\).%%\n"
    );
}

#[test]
fn a_twin_is_chosen_where_the_title_is_written_and_not_where_it_is_read_back() {
    // The reason there is no `\ifmmode` dispatcher in the output. A heading
    // title is written to the table of contents and read back on a later run,
    // and that reading happens outside mathematics whatever the title said. A
    // dispatcher would ask "am I in mathematics?" at that moment and answer the
    // wrong question for a title that held mathematics. The recomposition
    // chooses instead, where the document is read and the mode is known, and
    // writes the chosen twin's own name into the title. `tests/latex_compile.rs`
    // compiles this golden with a table of contents.
    let document = "\
\\section{Vector \\vec{v} and \\(\\vec{w}\\)}\\label{sec:vec}

\\vec{body} and \\(\\vec{math}\\).

See \\ref{sec:vec}.
";
    let kept = recomposed(vec_twins(), document);
    let text = body(&kept);
    assert!(
        text.contains("\\section{Vector \\flmUTDvec{v} and \\(\\flmUMDvec{}{w}\\)}"),
        "{text}"
    );
    assert!(text.contains("\\flmUTDvec{body} and \\(\\flmUMDvec{}{math}\\)."), "{text}");
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-mode-twins-heading.tex", &kept);
}

#[test]
fn an_encoded_name_declared_for_two_modes_takes_the_two_encoded_twin_prefixes() {
    // The same rule for a name that is not ASCII letters: the encoding of the
    // name does not change, only the prefix it hangs under.
    let twins = vec![
        Definition::macro_("~", Vec::new()).text("[t]"),
        Definition::macro_("~", Vec::new()).math("[m]"),
    ];
    let kept = recomposed(twins.clone(), "\\~ and \\(\\~\\).");
    assert_eq!(output(&kept), "\\flmUTXhE and \\(\\flmUMXhE\\).%%\n");
    let (style, _) = style(&kept);
    assert!(style.contains("\\providecommand\\flmUTXhE{[t]}\n"), "{style}");
    assert!(style.contains("\\providecommand\\flmUMXhE{[m]}\n"), "{style}");
    assert_eq!(not_recomposable(&registry_over(twins)), []);
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-mode-twins-encoded.tex", &kept);
}

#[test]
fn an_environment_declared_for_two_modes_takes_the_two_environment_twin_prefixes() {
    // The mathematics twin also names its body inside another command's
    // argument, so it takes its body as an argument (`+b`) while the text twin
    // is split at its body placeholder: the two forms meet in one name here,
    // and each twin is written the way its own template asks for.
    let twins = vec![
        Definition::environment("note", Vec::new()).text("[t: #{body}]"),
        Definition::environment("note", Vec::new()).math("[m: \\mathrm{#{body}}]"),
    ];
    let kept = recomposed(twins.clone(), "\\begin{note}a\\end{note} and \\(\\begin{note}b\\end{note}\\).");
    let text = body(&kept);
    assert!(text.contains("\\begin{flmUTEnote}a\\end{flmUTEnote}"), "{text}");
    assert!(text.contains("\\begin{flmUMEnote}b\\end{flmUMEnote}"), "{text}");
    let (style, _) = style(&kept);
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUTEnote}{}{[t: }{]}\n"), "{style}");
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUMEnote}{+b}{[m: \\mathrm{#1}]}{}\n"),
        "{style}"
    );
    assert_eq!(not_recomposable(&registry_over(twins)), []);
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    // The golden is compiled by `tests/latex_compile.rs`, which is the cheapest
    // proof that a twinned environment's two definitions are well formed.
    assert_golden("userdefs-mode-twins-environment.tex", &kept);
}

#[test]
fn a_bare_name_the_document_declares_for_two_modes_is_refused() {
    // Two definitions written under one bare name would fight over it, which
    // is the collision `bare_names` refuses in two other shapes already. The
    // entry is ignored and both twins are written under their own prefixes.
    let registry = registry_over(vec_twins());
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("vec")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides =
        Overrides { render_options: OptionValues::new().with(render), ..Overrides::none() };
    let run = run_workflow(&registry, WORKFLOW, &["\\vec{v} \\(\\vec{w}\\)"], &overrides);
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = diagnostic.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.name, "vec");
    assert_eq!(condition.reason, DefinitionNameRefusal::ModeTwins);
    assert_eq!(condition.reason.kind(), "mode-twins");
    let text = body(&run);
    assert!(text.contains("\\flmUTDvec{v} \\(\\flmUMDvec{}{w}\\)"), "{text}");
    // A document that uses only one of the two twins is refused just the same:
    // the entry cannot be honored for either of them, so it is answered as
    // soon as a twin reaches the style file rather than only where both do.
    let one = run_workflow(&registry, WORKFLOW, &["\\vec{v}"], &overrides);
    let only = expect(one.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = only.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.reason, DefinitionNameRefusal::ModeTwins);
}

// --- what is not recomposable instantiates, with its reason ---------------------------------

#[test]
fn a_definition_asking_whether_a_defaulted_argument_was_written_is_recomposed_with_a_flag() {
    // FLM's *provided* test asks whether the use site wrote the argument,
    // which a declared default does not answer. The call carries the answer:
    // the definition takes one more effective argument, a flag holding
    // `\BooleanTrue` where the writer wrote the argument and `\BooleanFalse`
    // where they did not, and the body asks LaTeX's own `\IfBooleanTF` about
    // it. The value group holds the default at the call that left the argument
    // out, exactly as the expansion substitutes it.
    let registry = registry_over(vec![mymacro_provided()]);
    let run = recompose(&registry, &["\\mymacro{Albert} and \\mymacro[Howdy]{Berta}."]);
    assert_eq!(
        output(&run),
        "\\flmUDmymacro{Hello}{Albert}{\\BooleanFalse} and \\flmUDmymacro{Howdy}{Berta}{\\BooleanTrue}.%%\n"
    );
    let (style, _) = style(&run);
    assert!(
        style.contains(
            "\\providecommand\\flmUDmymacro[3]{\\IfBooleanTF{#3}{#1, \\emph{#2}}{Hello there, \\emph{#2}}}\n"
        ),
        "{style}"
    );
    assert_eq!(not_recomposable(&registry), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-conditional.tex", &run);
    // What the expanded document prints is what the recomposed definition
    // prints: the question is answered the same way on both sides.
    let expanded = recomposed(
        vec![always(mymacro_provided())],
        "\\mymacro{Albert} and \\mymacro[Howdy]{Berta}.",
    );
    assert_eq!(output(&expanded), "Hello there, \\emph{Albert} and Howdy, \\emph{Berta}.%%\n");
}

#[test]
fn the_three_definitions_written_expanded_are_by_nature_by_choice_and_by_error() {
    // Every definition a document can declare is recomposed, so a definition
    // written expanded instead is one of three, and none of them is a limit
    // of the recomposition:
    //
    // - by **nature**, a specials definition, whose trigger is written with no
    //   backslash and is not a LaTeX command at all;
    // - by the author's **choice**, a definition marked `expand: Always`;
    // - by its own **error**, a definition the registry refused, which has no
    //   template to write and whose refusal was reported when it happened.
    //
    // A definition that uses one of the three is written expanded with it.
    // None of them is reported a second time here: the registry's build
    // diagnostics hold the refusal alone, and the reason is recorded on each
    // definition for a host that asks.
    let refused = Definition::macro_("broken", Vec::new()).content("#1");
    let chosen = Definition::macro_("chosen", Vec::new()).content("c").expand(ExpandPolicy::Always);
    let uses_specials = Definition::macro_("mail", Vec::new()).content("a@b");
    let registry = registry_over(vec![at(), chosen, refused, uses_specials]);
    let run = recompose(&registry, &["user@domain \\chosen \\mail"]);
    assert_eq!(output(&run), "user[at]domain ca[at]b%%\n");
    assert_no_user_definition(&run);
    assert_eq!(
        not_recomposable(&registry),
        [
            (String::from("@"), NotRecomposable::Specials),
            (String::from("\\broken"), NotRecomposable::NotCompiled),
            (String::from("\\mail"), NotRecomposable::Uses { name: String::from("@") }),
        ]
    );
    // The `always` definition is not per-plan, so it is not in that list at
    // all; asked directly it answers the author's choice. This is the whole of
    // what a host can learn about a definition written expanded, and it stays
    // recorded on the definition after the registry is built.
    let chosen_value = registry
        .element_value("flm.userdefs.macro.chosen")
        .expect("one value carries the name")
        .expect("registered")
        .value_as::<UserDefinition>()
        .expect("a user definition");
    assert_eq!(chosen_value.recomposable(), Err(NotRecomposable::AlwaysExpanded));
    assert_eq!(chosen_value.effective_signature(), None);
    // A recomposed definition answers the other way, with the signature its
    // calls are written from.
    let recomposed = registry_over(vec![hello()]);
    let hello_value = recomposed
        .element_value("flm.userdefs.macro.hello")
        .expect("one value carries the name")
        .expect("registered")
        .value_as::<UserDefinition>()
        .expect("a user definition");
    assert_eq!(hello_value.recomposable(), Ok(()));
    assert_eq!(hello_value.effective_signature().map(|s| s.arity()), Some(1));
    // Only the refusal is reported, and it is the definition's own: the
    // template names an argument the definition does not declare, once per
    // parsing mode the definition's text is compiled for.
    assert_eq!(
        build_diagnostics(&registry),
        ["flm.userdefs.bad-placeholder", "flm.userdefs.bad-placeholder"]
    );
    assert_golden("userdefs-written-expanded.tex", &run);
}

#[test]
fn an_environment_whose_template_never_names_its_body_grabs_it_and_drops_it() {
    // An environment that takes its body as an argument — `+b` after its
    // `+m`s — is handed the body by LaTeX, and a begin code that never writes
    // that argument drops it. That is exactly what FLM does with a body its
    // template never names, so such an environment is recomposed like any
    // other, paragraph breaks in the dropped body included.
    //
    // The one difference from the expansion, said rather than hidden: LaTeX
    // *reads* the body it drops, so a body whose braces or environments do not
    // balance stops the compilation where the expansion would have dropped it
    // unread — which is content FLM would not have parsed into a balanced tree
    // in the first place.
    let drops = Definition::environment("drops", Vec::new()).content("[nothing]");
    let document = "\\begin{drops}thrown away\n\nand this too\\end{drops} after.";
    let run = recomposed(vec![drops.clone()], document);
    let (style, _) = style(&run);
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUEdrops}{+b}{[nothing]}{}\n"),
        "{style}"
    );
    let text = body(&run);
    assert!(text.contains("\\begin{flmUEdrops}"), "{text}");
    assert!(text.contains("thrown away"), "{text}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-body-dropped.tex", &run);
    // FLM's own answer, which `tests/latex_compile.rs` compares LaTeX's
    // against: the body is gone and the template's text stands alone.
    let expanded = recomposed(vec![drops.expand(ExpandPolicy::Always)], document);
    assert_eq!(output(&expanded), "[nothing] after.%%\n");
    assert_golden("userdefs-body-dropped-expanded.tex", &expanded);
}

#[test]
fn a_macro_and_an_environment_of_one_name_are_both_recomposed_under_their_own_prefixes() {
    // FLM keeps `\foo` and `\begin{foo}` apart as two definitions, and so does
    // the recomposed output: a macro takes the prefix `flmUD` and an
    // environment the prefix `flmUE`, so the two LaTeX names differ and both
    // definitions reach the style file.
    let as_macro = Definition::macro_("foo", vec![ArgDef::mandatory("x")]).content("\\emph{#1}");
    let as_environment = Definition::environment("foo", Vec::new()).content("[#{body}]");
    let registry = registry_over(vec![as_macro, as_environment]);
    let run = recompose(&registry, &["\\foo{a} and \\begin{foo}b\\end{foo}."]);
    assert_eq!(
        output(&run),
        "\\flmUDfoo{a} and \\begin{flmUEfoo}b\\end{flmUEfoo}.%%\n"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDfoo[1]{\\emph{#1}}\n"), "{style}");
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUEfoo}{}{[}{]}\n"),
        "{style}"
    );
    assert_eq!(not_recomposable(&registry), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-one-name.tex", &run);
}

// --- inside another command's argument ------------------------------------------------------

#[test]
fn a_recomposed_definition_inside_a_heading_title_is_written_as_its_call_there_too() {
    // A heading title is a *moving argument*: LaTeX writes it to the table of
    // contents and to the auxiliary file and reads it back on the next run. A
    // recomposed definition is written as its call there like anywhere else,
    // and the definition the style file carries is robust, so the call
    // survives the trip. `tests/latex_compile.rs` compiles this golden and
    // probes what LaTeX does with it — the table of contents, a caption,
    // case changing and the PDF bookmark. Every later form a definition can
    // take is proved inside a heading the same way.
    let document = "\
\\section{Greeting \\hello{World}}\\label{sec:greet}

\\hello{Berta} in the body, and see \\ref{sec:greet}.

\\section*{Unnumbered \\hello{Carla}}\\label{sec:un}

And see \\ref{sec:un}.
";
    let run = recomposed(vec![hello()], document);
    let text = body(&run);
    assert!(text.contains("\\section{Greeting \\flmUDhello{World}}"), "{text}");
    assert!(text.contains("\\section*{Unnumbered \\flmUDhello{Carla}}"), "{text}");
    assert!(text.contains("\\flmUDhello{Berta} in the body"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDhello[1]{Hello, #1!}\n"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-in-heading.tex", &run);
}

// --- the star argument and the conditionals -------------------------------------------------------

#[test]
fn a_star_argument_is_recomposed_and_every_question_about_it_reads_its_flag() {
    // FLM's `*` marker is an argument like any other: its value group holds
    // the `*` the writer wrote and nothing where they wrote none, which is
    // what FLM substitutes for a placeholder of it. A question about it —
    // provided, value or no value, all the same question of a star — reads the
    // flag the call appended, with LaTeX's own `\IfBooleanTF`. One call stands
    // inside a heading title, which is the standing requirement every new form
    // is proved under.
    let emphasize = Definition::macro_(
        "emphasize",
        vec![ArgDef::new(ArgStdKind::Star), ArgDef::mandatory("x")],
    )
    .content("\\IfBooleanTF{#1}{\\textbf{#2}}{\\emph{#2}}");
    let mark = Definition::macro_("mark", vec![ArgDef::mandatory("x"), ArgDef::new(ArgStdKind::Star)])
        .content("#1\\IfProvidedT{#2}{!}\\IfNoValueF{#2}{?}");
    let document = "\
\\section{Title \\emphasize*{loud}}\\label{sec:star}

\\emphasize*{bold} and \\emphasize{plain}, \\mark{here}* and \\mark{there} then text.

See \\ref{sec:star}.
";
    let run = recomposed(vec![emphasize, mark], document);
    let (style, _) = style(&run);
    // Two declared arguments and one flag: three effective arguments, the flag
    // last.
    assert!(
        style.contains(
            "\\providecommand\\flmUDemphasize[3]{\\IfBooleanTF{#3}{\\textbf{#2}}{\\emph{#2}}}\n"
        ),
        "{style}"
    );
    // `\IfProvidedT` and `\IfNoValueF` ask the same thing of a star, so both
    // read the same flag.
    assert!(
        style.contains(
            "\\providecommand\\flmUDmark[3]{#1\\IfBooleanT{#3}{!}\\IfBooleanT{#3}{?}}\n"
        ),
        "{style}"
    );
    let text = body(&run);
    assert!(text.contains("\\section{Title \\flmUDemphasize{*}{loud}{\\BooleanTrue}}"), "{text}");
    assert!(
        text.contains(
            "\\flmUDemphasize{*}{bold}{\\BooleanTrue} and \\flmUDemphasize{}{plain}{\\BooleanFalse}"
        ),
        "{text}"
    );
    // The star after `\mark{here}` is written in the argument's own group, so
    // nothing of the call is left for LaTeX to look for.
    assert!(
        text.contains(
            "\\flmUDmark{here}{*}{\\BooleanTrue} and \\flmUDmark{there}{}{\\BooleanFalse} then"
        ),
        "{text}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-star.tex", &run);
}

#[test]
fn a_placeholder_of_a_star_argument_is_the_star_the_use_site_wrote() {
    // FLM substitutes the `*` itself for a placeholder of a star argument, and
    // nothing where the writer left the star out. The call writes exactly
    // that in the argument's group, so the body names it with a plain
    // parameter like any other argument.
    let show = Definition::macro_("show", vec![ArgDef::new(ArgStdKind::Star), ArgDef::mandatory("x")])
        .content("[#1|#2]");
    let document = "\\show*{a} and \\show{b}.";
    let kept = recomposed(vec![show.clone()], document);
    assert_eq!(output(&kept), "\\flmUDshow{*}{a} and \\flmUDshow{}{b}.%%\n");
    assert!(
        style(&kept).0.contains("\\providecommand\\flmUDshow[2]{[#1|#2]}"),
        "{}",
        style(&kept).0
    );
    // What the expanded document says is what the recomposed definition says.
    let expanded = recomposed(vec![always(show)], document);
    assert_eq!(output(&expanded), "[*|a] and [|b].%%\n");
}

#[test]
fn a_conditional_over_an_optional_argument_with_no_default_reads_its_flag() {
    // An optional argument a conditional asks about, and whose default the
    // definition does not declare, gets a flag: *no value*, *value* and
    // *provided* all coincide there, so one flag answers all three, and the
    // argument's own group holds what FLM substitutes — the content the writer
    // wrote, or nothing. LaTeX's `-NoValue-` marker never reaches the body, so
    // no placeholder has to be guarded against printing it.
    let opt = Definition::macro_("opt", vec![ArgDef::optional("a"), ArgDef::mandatory("x")])
        .content("<\\IfNoValueTF{#1}{none}{#1}|\\IfValueT{#1}{V}|\\IfNoValueF{#1}{F}|#2>");
    let document = "\
\\section{Title \\opt[t]{h}}\\label{sec:opt}

\\opt[x]{y} and \\opt{z}, see \\ref{sec:opt}.
";
    let kept = recomposed(vec![opt.clone()], document);
    let (style, _) = style(&kept);
    assert!(
        style.contains(
            "\\providecommand\\flmUDopt[3]{<\\IfBooleanTF{#3}{#1}{none}|\\IfBooleanT{#3}{V}|\\IfBooleanT{#3}{F}|#2>}\n"
        ),
        "{style}"
    );
    let text = body(&kept);
    assert!(text.contains("\\section{Title \\flmUDopt{t}{h}{\\BooleanTrue}}"), "{text}");
    assert!(
        text.contains("\\flmUDopt{x}{y}{\\BooleanTrue} and \\flmUDopt{}{z}{\\BooleanFalse}"),
        "{text}"
    );
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-conditional-value.tex", &kept);
    // What the expanded document says.
    let expanded = recomposed(vec![always(opt)], document);
    assert!(body(&expanded).contains("<x|V|F|y> and <none|||z>"), "{}", body(&expanded));
}

#[test]
fn a_conditional_whose_answer_does_not_depend_on_the_document_is_written_as_its_branch() {
    // Three of the five questions read a test argument that is exactly one
    // placeholder and answer about anything else without looking at the
    // document at all: a mandatory argument is always there, a literal is not
    // a placeholder, and an argument with a declared default always has a
    // value. Each is settled when the style file is written.
    let fixed = Definition::macro_("fixed", vec![ArgDef::mandatory("x"), ArgDef::optional("d")])
        .content("\\IfValueTF{#1}{m}{-}\\IfNoValueTF{lit}{n}{l}\\IfValueT{#2}{d}\\IfNoValueT{#2}{x}")
        .default("d", "dd");
    let run = recomposed(vec![fixed], "\\fixed{a}.");
    assert_eq!(output(&run), "\\flmUDfixed{a}{dd}.%%\n");
    assert!(
        style(&run).0.contains("\\providecommand\\flmUDfixed[2]{mld}\n"),
        "{}",
        style(&run).0
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_blank_test_is_written_as_the_style_files_blank_test_and_answers_as_flm_does() {
    // `\ifblank` becomes `\flmIfBlankTF`, the one definition piece this piece
    // adds. The six arguments below are the ones the rendering guide records:
    // nothing, one space, a comment alone, a comment then text, a comment then
    // whitespace, and whitespace then a comment. The text backend answers with
    // FLM's own test; `tests/latex_compile.rs` puts the same six arguments to
    // `pdflatex` and compares. `\notblank` is the same test with its branches
    // the other way round.
    let blank = Definition::macro_("b", vec![ArgDef::mandatory("a")])
        .content("\\ifblank{#1}{BLANK}{FULL}");
    let notblank = Definition::macro_("n", vec![ArgDef::mandatory("a")])
        .content("\\notblank{#1}{FULL}{BLANK}");
    let document = "\
\\section{Title \\b{}}\\label{sec:blank}

1:\\b{} 2:\\b{ } 3:\\b{%% aside
} 4:\\b{%% aside
text} 5:\\b{%% aside
 } 6:\\b{ %% aside
} 7:\\n{} 8:\\n{q}

See \\ref{sec:blank}.
";
    let definitions = vec![blank, notblank];
    let kept = recomposed(definitions.clone(), document);
    let (style, _) = style(&kept);
    assert!(
        style.contains("\\providecommand\\flmUDb[1]{\\flmIfBlankTF{#1}{BLANK}{FULL}}\n"),
        "{style}"
    );
    // `\notblank` is the same test with its branches the other way round.
    assert!(
        style.contains("\\providecommand\\flmUDn[1]{\\flmIfBlankTF{#1}{BLANK}{FULL}}\n"),
        "{style}"
    );
    // The piece is written into the style file, and only because a definition
    // asked for it.
    assert!(style.contains("\\providecommand\\flmIfBlankTF[3]"), "{style}");
    assert!(style.contains("\\ExplSyntaxOn"), "{style}");
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-blank.tex", &kept);
    // FLM's own answers, which the compile suite compares LaTeX's against.
    let registry = registry_over(definitions);
    let on_text = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    let printed = common::lowering::text(&on_text);
    assert!(
        printed.contains(
            "1:BLANK 2:FULL 3:BLANK 4:FULL 5:BLANK 6:FULL 7:BLANK 8:FULL"
        ),
        "{printed}"
    );
}

#[test]
fn a_blank_test_beside_a_value_test_over_one_argument_needs_no_guard() {
    // An optional argument a value test also asks about carries a flag, and
    // its own group still holds what FLM substitutes — nothing where the
    // writer left it out. So the blank test stands over the plain group and
    // answers what FLM answers, with nothing to guard: the `-NoValue-` marker
    // the earlier form had to work around never reaches the body.
    let guarded = Definition::macro_("g", vec![ArgDef::optional("a")])
        .content("\\IfValueT{#1}{V}\\ifblank{#1}{B}{N}");
    let document = "\\g[x] \\g[] \\g and text.";
    let kept = recomposed(vec![guarded.clone()], document);
    let (style, _) = style(&kept);
    assert!(
        style.contains(
            "\\providecommand\\flmUDg[2]{\\IfBooleanT{#2}{V}\\flmIfBlankTF{#1}{B}{N}}\n"
        ),
        "{style}"
    );
    assert!(body(&kept).contains(
        "\\flmUDg{x}{\\BooleanTrue} \\flmUDg{}{\\BooleanTrue} \\flmUDg{}{\\BooleanFalse}and text."
    ), "{}", body(&kept));
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
    assert_golden("userdefs-blank-guarded.tex", &kept);
    // FLM's own answers: written and not blank, written and blank, absent.
    // The space after a bare `\g` is part of the invocation and vanishes with
    // it, as it does in an expanded document.
    let expanded = recomposed(vec![always(guarded)], document);
    assert!(body(&expanded).contains("VN VB Band text."), "{}", body(&expanded));
}

// --- more than nine effective arguments: the packed group ------------------------------------------

/// `\big`, a macro of twelve mandatory arguments — three more than a LaTeX
/// macro body has parameters — whose template uses the first, the seventh and
/// the twelfth.
fn big() -> Definition {
    Definition::macro_("big", (1..=12).map(|i| ArgDef::mandatory(format!("a{i}"))).collect())
        .content("[#{a1}|#{a7}|#{a12}]")
}

/// `\huge`, a macro of twenty mandatory arguments, whose template uses the
/// first, the twelfth and the last.
fn huge() -> Definition {
    Definition::macro_("huge", (1..=20).map(|i| ArgDef::mandatory(format!("a{i}"))).collect())
        .content("(#{a1}/#{a12}/#{a20})")
}

#[test]
fn a_definition_of_more_than_nine_effective_arguments_packs_its_call_into_one_group() {
    // A LaTeX macro body names nine parameters, so a definition of more than
    // nine effective arguments cannot name them one by one. Its call hands
    // every argument over in **one** brace group instead — one pair of braces
    // around each, in the effective signature's order — the definition names
    // one parameter holding all of them, and its body reads argument n out of
    // it with the style file's accessor. One call stands inside a heading
    // title, the standing requirement every new form is proved under, and one
    // stands inside mathematics, which the mathematics writer writes rather
    // than the emitter — the one other place the outer group is opened.
    let document = "\
\\section{Title \\big{A}{b}{c}{d}{e}{f}{G}{h}{i}{j}{k}{L}}\\label{sec:packed}

\\big{one}{b}{c}{d}{e}{f}{seven}{h}{i}{j}{k}{twelve} and
\\huge{1}{2}{3}{4}{5}{6}{7}{8}{9}{10}{11}{TWELVE}{13}{14}{15}{16}{17}{18}{19}{TWENTY}.

In mathematics \\(\\big{m}{b}{c}{d}{e}{f}{M}{h}{i}{j}{k}{Z} + 1\\).

See \\ref{sec:packed}.
";
    let run = recomposed(vec![big(), huge()], document);
    let (style, _) = style(&run);
    // The definition takes one parameter whatever its argument count, and the
    // body reads the arguments it uses out of it.
    assert!(
        style.contains("\\providecommand\\flmUDbig[1]{[\\flmArg{1}{#1}|\\flmArg{7}{#1}|\\flmArg{12}{#1}]}\n"),
        "{style}"
    );
    assert!(
        style.contains("\\providecommand\\flmUDhuge[1]{(\\flmArg{1}{#1}/\\flmArg{12}{#1}/\\flmArg{20}{#1})}\n"),
        "{style}"
    );
    // The style file carries the piece that defines the accessor, and only
    // because a definition packs. The blank test's piece comes with it even
    // though neither definition asks a blank test: the packed blank test calls
    // `\flmIfBlankTF`, so that what "blank" means is written in one place.
    assert!(style.contains("\\providecommand\\flmArg[2]{\\tl_item:nn {#2} {#1}}\n"), "{style}");
    assert!(
        style.contains("\\providecommand\\flmIfBlankTF[3]{\\tl_if_empty:nTF {#1} {#2} {#3}}\n"),
        "{style}"
    );
    let text = body(&run);
    assert!(
        text.contains("\\section{Title \\flmUDbig{{A}{b}{c}{d}{e}{f}{G}{h}{i}{j}{k}{L}}}"),
        "{text}"
    );
    assert!(
        text.contains("\\flmUDbig{{one}{b}{c}{d}{e}{f}{seven}{h}{i}{j}{k}{twelve}} and"),
        "{text}"
    );
    assert!(
        text.contains(
            "\\flmUDhuge{{1}{2}{3}{4}{5}{6}{7}{8}{9}{10}{11}{TWELVE}{13}{14}{15}{16}{17}{18}{19}{TWENTY}}."
        ),
        "{text}"
    );
    // Inside mathematics the same bytes, written by the mathematics writer.
    assert!(
        text.contains("\\(\\flmUDbig{{m}{b}{c}{d}{e}{f}{M}{h}{i}{j}{k}{Z}} + 1\\)"),
        "{text}"
    );
    assert_eq!(not_recomposable(&registry_over(vec![big(), huge()])), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-packed.tex", &run);
    // What the expanded document prints is what the packed calls print.
    let expanded = recomposed(vec![always(big()), always(huge())], document);
    let printed = body(&expanded);
    assert!(printed.contains("[one|seven|twelve] and"), "{printed}");
    assert!(printed.contains("(1/TWELVE/TWENTY)."), "{printed}");
    assert!(printed.contains("\\section{Title [A|G|L]}"), "{printed}");
    assert!(printed.contains("\\([m|M|Z] + 1\\)"), "{printed}");
}

#[test]
fn six_arguments_five_of_them_flagged_are_eleven_effective_arguments_and_pack() {
    // Values and flags count together, so a definition of six arguments that
    // asks about five of them writes eleven groups and packs — nothing unusual
    // in what the author wrote. The flags stand after the values, in declared
    // order, and the body reads a flag with the style file's own test rather
    // than with LaTeX's `\IfBooleanTF`, which cannot be handed one part of a
    // group. The blank test over two arguments at once is asked the same way,
    // over the two parts named together.
    let six = Definition::macro_(
        "six",
        (1..=6).map(|i| ArgDef::optional(format!("b{i}"))).collect(),
    )
    .content(
        "\\IfProvidedT{#{b1}}{1}\\IfProvidedT{#{b2}}{2}\\IfProvidedT{#{b3}}{3}\
         \\IfProvidedT{#{b4}}{4}\\IfProvidedT{#{b5}}{5}\\ifblank{#{b1}#{b6}}{-}{+}#{b6}",
    );
    let document = "\
\\section{Six \\six[p][q]}\\label{sec:six}

\\six[p][q][r][s][t][u] and \\six{} then text. See \\ref{sec:six}.
";
    // `\six`'s six arguments are all optional, so the `{}` the document writes
    // after the second call is the document's own brace group and not an
    // argument of it. It stands after the packed group in the output, where it
    // shows that nothing LaTeX could read as an argument follows a packed
    // call.
    let run = recomposed(vec![six.clone()], document);
    let (style, _) = style(&run);
    assert!(
        style.contains(
            "\\providecommand\\flmUDsix[1]{\
             \\flmIfArgTF{7}{#1}{1}{}\\flmIfArgTF{8}{#1}{2}{}\\flmIfArgTF{9}{#1}{3}{}\
             \\flmIfArgTF{10}{#1}{4}{}\\flmIfArgTF{11}{#1}{5}{}\
             \\flmIfArgBlankTF{1,6}{#1}{-}{+}\\flmArg{6}{#1}}\n"
        ),
        "{style}"
    );
    let text = body(&run);
    // Six values, then the five flags of the arguments the template asks
    // about, all inside one group.
    assert!(
        text.contains(
            "\\flmUDsix{{p}{q}{r}{s}{t}{u}\
             {\\BooleanTrue}{\\BooleanTrue}{\\BooleanTrue}{\\BooleanTrue}{\\BooleanTrue}}"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "\\flmUDsix{{}{}{}{}{}{}\
             {\\BooleanFalse}{\\BooleanFalse}{\\BooleanFalse}{\\BooleanFalse}{\\BooleanFalse}}"
        ),
        "{text}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-packed-flags.tex", &run);
    // FLM's own answers, for the compile probe to measure against.
    let expanded = recomposed(vec![always(six)], document);
    let printed = body(&expanded);
    assert!(printed.contains("12345+u and -"), "{printed}");
}

#[test]
fn an_environment_that_packs_reads_its_arguments_in_its_begin_and_end_code_alike() {
    // An environment can never be expandable — `\begin` opens a group, runs
    // the begin code, sets the body and runs the end code — so it keeps the
    // document-environment interface, and a packed one declares one `+m`
    // holding every argument. Both the begin code and the end code read out of
    // that one argument, which is what makes an argument available after the
    // body.
    let note = Definition::environment(
        "packednote",
        (1..=10).map(|i| ArgDef::mandatory(format!("c{i}"))).collect(),
    )
    .content("(note #{c1}: #{body} — end #{c10})");
    let document = "\
\\section{Note \\big{A}{b}{c}{d}{e}{f}{G}{h}{i}{j}{k}{L}}\\label{sec:env}

\\begin{packednote}{one}{2}{3}{4}{5}{6}{7}{8}{9}{ten}
the body
\\end{packednote}

See \\ref{sec:env}.
";
    let run = recomposed(vec![note, big()], document);
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUEpackednote}{+m}"), "{style}");
    assert!(style.contains("{(note \\flmArg{1}{#1}: }{ — end \\flmArg{10}{#1})}\n"), "{style}");
    let text = body(&run);
    assert!(
        text.contains("\\begin{flmUEpackednote}{{one}{2}{3}{4}{5}{6}{7}{8}{9}{ten}}"),
        "{text}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-packed-environment.tex", &run);
}

#[test]
fn a_packed_definition_naming_its_arguments_inside_a_verbatim_block_expands_its_use_sites() {
    // Five declared arguments, each named both outside verbatim content and
    // inside a block. A block's characters are read when the recomposed LaTeX
    // is written, and a placeholder's are known at a call and not before, so
    // the definition is not written as a LaTeX definition: every use site is
    // expanded, and the recomposed LaTeX is what the expansion writes.
    let five = |content: &str| {
        Definition::macro_(
            "five",
            (1..=5).map(|i| ArgDef::mandatory(format!("a{i}"))).collect(),
        )
        .content(content)
    };
    let content = "\\begin{verbatimcode}\none: #1\ntwo: #2\nthree: #3\nfour: #4\nfive: #5\n\\end{verbatimcode}\n[#1|#2|#3|#4|#5]";
    let document = "\\five{a_b}{c & d}{e}{f}{g}\n";
    let registry = registry_over(vec![five(content)]);
    assert_eq!(not_recomposable(&registry), []);
    let run = recompose(&registry, &[document]);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let expanded = recomposed(vec![always(five(content))], document);
    assert!(
        body(&expanded).contains(
            "\\begin{flmVerbatimCode}[]\none: a_b\ntwo: c & d\nthree: e\nfour: f\nfive: g\n\\end{flmVerbatimCode}\n[a\\_b|c \\& d|e|f|g]"
        ),
        "{}",
        body(&expanded)
    );
    assert_eq!(body(&run), body(&expanded));
    assert!(!style(&run).0.contains("\\flmUDfive"), "{}", style(&run).0);
    assert_golden("userdefs-packed-verbatim-expanded.tex", &expanded);
}

// --- an environment that takes its body as an argument ----------------------------------------

/// `{wrap}{who}`, whose template puts the body **inside another command's
/// argument** and names an argument of its own after it: a begin code and an
/// end code cannot put the body there, so the environment takes the body as an
/// argument of its own.
fn wrap() -> Definition {
    Definition::environment("wrap", vec![ArgDef::mandatory("who")])
        .content("(\\emph{#{body}} — #{who})")
}

/// `{twice}`, whose template names its body **twice**. LaTeX sets a body once,
/// so this environment takes the body as an argument too.
fn twice() -> Definition {
    Definition::environment("twice", Vec::new()).content("#{body}/#{body}")
}

#[test]
fn an_environment_that_wraps_or_repeats_its_body_takes_the_body_as_its_last_argument() {
    // The body of a recomposed environment is set between the begin code and
    // the end code, which is where a template that names it once at its own top
    // level wants it. A template that wants it anywhere else asks LaTeX to hand
    // the body over instead: the argument specification ends with `+b`, the
    // whole template is the begin code with the body written as that last
    // parameter, and the end code is empty. The call is unchanged — what a call
    // writes between `\begin` and `\end` is the body either way.
    //
    // The standing requirement that every new form be proved inside a heading
    // is met here by the definition the environment's *argument* holds: an
    // environment cannot stand in a title, so what travels into the table of
    // contents is the recomposed macro `\hello`, used in the heading and in the
    // environment's argument alike.
    let document = "\
\\section{Greeting \\hello{World}}\\label{sec:wrap}

\\begin{wrap}{\\hello{Ada}}the body\\end{wrap}

\\begin{twice}said twice\\end{twice}

See \\ref{sec:wrap}.
";
    let run = recomposed(vec![wrap(), twice(), hello()], document);
    let (style, _) = style(&run);
    // One `+m` for the declared argument, then `+b` for the body; the body is
    // the last parameter, `#2`, and the end code is empty.
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUEwrap}{+m +b}{(\\emph{#2} — #1)}{}\n"),
        "{style}"
    );
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUEtwice}{+b}{#1/#1}{}\n"),
        "{style}"
    );
    let text = body(&run);
    assert!(text.contains("\\begin{flmUEwrap}{\\flmUDhello{Ada}}the body\\end{flmUEwrap}"), "{text}");
    assert!(text.contains("\\begin{flmUEtwice}said twice\\end{flmUEtwice}"), "{text}");
    assert_eq!(not_recomposable(&registry_over(vec![wrap(), twice(), hello()])), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-body-argument.tex", &run);
    // What the expanded document prints is what the recomposed pair prints.
    let expanded =
        recomposed(vec![always(wrap()), always(twice()), always(hello())], document);
    let printed = body(&expanded);
    assert!(printed.contains("(\\emph{the body} — Hello, Ada!)"), "{printed}");
    assert!(printed.contains("said twice/said twice"), "{printed}");
}

#[test]
fn a_body_taken_as_an_argument_may_hold_a_paragraph_break() {
    // `+b`, like `+m`, makes the argument **long**, so the body may hold a
    // paragraph break. A body of two paragraphs is what a document environment
    // ordinarily holds, and it is the one thing the plain `b` would refuse.
    let document = "\
\\begin{twice}first paragraph

second paragraph\\end{twice}
";
    let run = recomposed(vec![twice()], document);
    let text = body(&run);
    assert!(text.contains("first paragraph\n\nsecond paragraph"), "{text}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-body-argument-paragraphs.tex", &run);
}

#[test]
fn a_packed_environment_that_takes_its_body_as_an_argument_reads_the_body_as_its_second_parameter() {
    // The packing and the body argument meet without either changing: the
    // arguments arrive in one group, which is the first parameter, and the body
    // in the second. The body is never part of the packed group — a call writes
    // it between `\begin` and `\end` as always.
    let note = Definition::environment(
        "packedwrap",
        (1..=10).map(|i| ArgDef::mandatory(format!("c{i}"))).collect(),
    )
    .content("(note #{c1}: \\emph{#{body}} — end #{c10})");
    let document = "\
\\begin{packedwrap}{one}{2}{3}{4}{5}{6}{7}{8}{9}{ten}the body\\end{packedwrap}
";
    let run = recomposed(vec![note], document);
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUEpackedwrap}{+m +b}"), "{style}");
    assert!(
        style.contains("{(note \\flmArg{1}{#1}: \\emph{#2} — end \\flmArg{10}{#1})}{}\n"),
        "{style}"
    );
    let text = body(&run);
    assert!(
        text.contains("\\begin{flmUEpackedwrap}{{one}{2}{3}{4}{5}{6}{7}{8}{9}{ten}}the body"),
        "{text}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-packed-body-argument.tex", &run);
}

// --- a tack-on argument ----------------------------------------------------------------------------

/// The tack-on argument `\thm` declares: a `\label` field of the definition's
/// own, so that `\thm{Pythagoras}\label{thm:p}` parses and what it absorbs
/// belongs to this definition alone.
///
/// The field is a field like every other — its node carries the field element
/// with the name `label` — and it belongs to this definition: no stage of
/// `refs` sees it, because the label collection and the safe-name table read
/// reference points, never `\label` nodes, and this definition declares no
/// reference point. A user may give `\label` whatever meaning they like here.
fn thm_label_arg() -> ArgDef {
    ArgDef::named(
        "fields",
        ArgStdKind::TackOn {
            fields: vec![
                TackOnField::new("label", vec![ArgDef::named("target", ArgStdKind::CharsGroup)])
                    .repeatable(),
            ],
        },
    )
}

/// `\thm{title}` declaring a trailing **tack-on** argument — the macros a call
/// absorbs after it — and naming it nowhere in its template, which is the only
/// thing a definition may do with one today. The template sets the title in
/// bold and nothing else.
fn thm() -> Definition {
    Definition::macro_("thm", vec![ArgDef::mandatory("title"), thm_label_arg()])
        .content("\\textbf{#{title}}")
}

/// The overrides the tack-on tests plan under: none at all.
///
/// A field is covered on every backend by the field element's own rule, which
/// reports a field that reaches a backend, so no host has to accept a hole for
/// one. Nothing ever lowers this field — a call writes no group for a tack-on
/// argument, and an expansion drops what its template names nowhere — so that
/// rule never runs and no diagnostic is raised, which the tests assert.
fn tack_on_holes() -> Overrides {
    Overrides::none()
}

#[test]
fn a_declared_tack_on_argument_writes_no_group_and_what_it_absorbs_goes_nowhere() {
    // A definition may declare a tack-on argument and not name it: it is
    // recomposed like any other, and the argument carries no effective
    // argument at all, so a call writes one group for the title and nothing
    // else — whether the use site tacked something on or not. What a call
    // absorbs is written nowhere, because no template can reach it yet. One
    // call stands inside a heading title, the standing requirement every new
    // form is proved under.
    let document = "\
\\section{Theorem \\thm{Pythagoras}}\\label{sec:thm}

\\thm{Euclid}\\label{thm:euclid} and \\thm{Thales} then text.

In mathematics \\(\\thm{Chi} = 1\\).

See \\ref{sec:thm}.
";
    let registry = registry_over(vec![thm()]);
    let run = run_workflow(&registry, WORKFLOW, &[document], &tack_on_holes());
    let (style, _) = style(&run);
    // One effective argument, the title: the declared tack-on argument is not
    // one.
    assert!(style.contains("\\providecommand\\flmUDthm[1]{\\textbf{#1}}\n"), "{style}");
    let text = body(&run);
    // The call absorbing a `\label` and the call absorbing nothing are written
    // exactly alike, and neither carries what was absorbed.
    assert!(text.contains("\\flmUDthm{Euclid} and \\flmUDthm{Thales} then text."), "{text}");
    // The same form inside a heading title.
    assert!(text.contains("\\section{Theorem \\flmUDthm{Pythagoras}}"), "{text}");
    // And inside mathematics, where the writer writes the same groups as bytes.
    assert!(text.contains("\\(\\flmUDthm{Chi} = 1\\)"), "{text}");
    // Declaring a tack-on argument is no reason to expand the definition.
    assert_eq!(not_recomposable(&registry), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    // What the call absorbed is this definition's business and nobody else's:
    // the definition declares no reference point, so `thm:euclid` is no label
    // of the document, gets no safe key, and is written nowhere. The one key
    // allocated is the heading's own label, named by the `\ref` that follows
    // it.
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 1);
    assert_eq!(table.label_of("ref", "ref1"), Some("sec:thm"));
    assert_eq!(table.safe_name("ref", "thm:euclid"), None);
    assert!(!text.contains("thm:euclid"), "{text}");
    assert_golden("userdefs-tack-on.tex", &run);
    // What the expanded document prints is what the call prints: the expansion
    // drops the absorbed `\label` too, since the template names it nowhere.
    let expanded = run_workflow(
        &registry_over(vec![always(thm())]),
        WORKFLOW,
        &[document],
        &tack_on_holes(),
    );
    let printed = body(&expanded);
    assert!(printed.contains("\\textbf{Euclid} and \\textbf{Thales} then text."), "{printed}");
}

#[test]
fn a_tack_on_only_definition_keeps_the_space_after_its_name_in_mathematics() {
    // A definition whose only declared argument is a tack-on argument writes
    // no group at all at a call, a tack-on argument carrying neither a value
    // nor a flag. Inside mathematics, where the writer writes bytes rather
    // than staging a call construct, the space the author left after the name
    // is therefore written back, so that a letter after it does not extend the
    // name: `\flmUDqed x`, never `\flmUDqedx`. In text the emitter reaches
    // the same end its own way, with one space of its own where the next node
    // begins with a letter.
    //
    let qed = Definition::macro_("qed", vec![thm_label_arg()]).content("QED");
    let registry = registry_over(vec![qed]);
    let document = "\\qed then text.\n\nIn mathematics \\(\\qed x = 1\\).\n";
    let run = run_workflow(&registry, WORKFLOW, &[document], &tack_on_holes());
    let text = body(&run);
    assert!(text.contains("\\(\\flmUDqed x = 1\\)"), "{text}");
    assert!(text.contains("\\flmUDqed then text."), "{text}");
    assert_eq!(not_recomposable(&registry), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_template_naming_a_tack_on_argument_is_refused_on_every_backend() {
    // A template cannot yet use what a tack-on argument absorbs: what the call
    // absorbed is whole macros, and a `\label` written into a parameter is not
    // a command a body can hold. Naming one is a definition error raised where
    // the definition is compiled, so it is the same refusal on the `text`
    // backend as on this one, and the definition exists on neither.
    let named = Definition::macro_("thm", vec![ArgDef::mandatory("title"), thm_label_arg()])
        .content("\\textbf{#{title}}#{fields}");
    let registry = registry_over(vec![named]);
    let refusals: Vec<&flm_core::lang::origin::FlmDiagnostic> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.userdefs.bad-definition")
        .collect();
    assert!(!refusals.is_empty(), "{:?}", build_diagnostics(&registry));
    for refusal in &refusals {
        assert!(
            refusal.message().contains("a template cannot refer to"),
            "{}",
            refusal.message()
        );
    }
    // The same refusal, and the same absence of the definition, on both
    // backends: a use site is left as its own source text.
    let document = "\\thm{Euclid}\\label{thm:euclid}\n";
    for workflow in [WORKFLOW, text::WORKFLOW] {
        let run = run_workflow(&registry, workflow, &[document], &tack_on_holes());
        expect(run.diagnostics().iter(), "flm.userdefs.bad-definition");
    }
}

// --- a computed default ----------------------------------------------------------------------------

/// The document both tests of a computed default read: a call that writes the
/// optional argument and one that leaves it out, for each of the two
/// definitions, one of them inside a heading title and one inside mathematics.
const COMPUTED_DEFAULT_DOCUMENT: &str = "\
\\section{Tagged \\tag{A short heading}}\\label{sec:tag}

\\tag[TL;DR]{A long title} and \\tag{A long title} then \\marked[!]{shout} and
\\marked{shout} and \\marked{}.

In mathematics \\(\\tag{x + y}\\) and \\(\\marked{}\\) hold.

See \\ref{sec:tag}.
";

/// `\tag[short]{long}` whose default for the short form is the *long*
/// argument: a default that is not plain content but a placeholder of another
/// argument, resolved at each use site by what that use site wrote.
fn tag() -> Definition {
    Definition::macro_("tag", vec![ArgDef::optional("short"), ArgDef::mandatory("long")])
        .content("[#{short}] #{long}")
        .default("short", "#{long}")
}

/// `\marked[mark]{text}` whose default for the mark is a conditional over the
/// other argument: the text in bold where there is any, the word `empty`
/// where there is none.
fn marked() -> Definition {
    Definition::macro_("marked", vec![ArgDef::optional("mark"), ArgDef::mandatory("text")])
        .content("(#{mark}) #{text}")
        .default("mark", "\\ifblank{#{text}}{empty}{\\textbf{#{text}}}")
}

#[test]
fn a_computed_default_is_resolved_at_the_use_site_and_written_as_the_value() {
    // A declared default is written into the call wherever the use site left
    // its argument out, and a default that holds placeholders or conditionals
    // of its own is resolved there first — by FLM's own bindings, through the
    // same function the expansion pass instantiates it with. So the call
    // carries what the expansion would have substituted, and the definition
    // in the style file knows nothing about defaults at all. One call stands
    // inside a heading title, the standing requirement every new form is
    // proved under, and one inside mathematics, which the mathematics writer
    // writes as source byte for byte.
    let document = COMPUTED_DEFAULT_DOCUMENT;
    let run = recomposed(vec![tag(), marked()], document);
    let (style, _) = style(&run);
    // The definition carries no default of its own: every call writes one.
    assert!(style.contains("\\providecommand\\flmUDtag[2]{[#1] #2}\n"), "{style}");
    assert!(style.contains("\\providecommand\\flmUDmarked[2]{(#1) #2}\n"), "{style}");
    let text = body(&run);
    // Written: the argument as the use site wrote it. Left out: the default,
    // resolved — here the other argument's content.
    assert!(text.contains("\\flmUDtag{TL;DR}{A long title} and"), "{text}");
    assert!(text.contains("\\flmUDtag{A long title}{A long title} then"), "{text}");
    // A default holding a conditional: the branch the use site's own content
    // selects, and the content of that branch lowered through the rules.
    assert!(text.contains("\\flmUDmarked{!}{shout} and"), "{text}");
    assert!(text.contains("\\flmUDmarked{\\textbf{shout}}{shout} and"), "{text}");
    assert!(text.contains("\\flmUDmarked{empty}{}."), "{text}");
    // Inside a heading title, and inside mathematics, where the resolved
    // default is written from its nodes byte for byte like everything else
    // there.
    assert!(
        text.contains("\\section{Tagged \\flmUDtag{A short heading}{A short heading}}"),
        "{text}"
    );
    assert!(text.contains("\\(\\flmUDtag{x + y}{x + y}\\) and"), "{text}");
    // And where the resolved default is the *declaration's* own text rather
    // than the use site's — the conditional's `empty` branch — the
    // re-sourced instance is readable by the mathematics writer, which writes
    // every value group from its nodes.
    assert!(text.contains("\\(\\flmUDmarked{empty}{}\\) hold."), "{text}");
    assert_eq!(not_recomposable(&registry_over(vec![tag(), marked()])), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-computed-default.tex", &run);
    // What the expanded document prints is what the calls print.
    let expanded = recomposed(vec![always(tag()), always(marked())], document);
    let printed = body(&expanded);
    assert!(printed.contains("[TL;DR] A long title and [A long title] A long title then"), "{printed}");
    assert!(printed.contains("(!) shout and"), "{printed}");
    assert!(printed.contains("(\\textbf{shout}) shout and (empty) ."), "{printed}");
    assert!(printed.contains("\\section{Tagged [A short heading] A short heading}"), "{printed}");
    assert!(printed.contains("\\([x + y] x + y\\) and \\((empty) \\) hold."), "{printed}");
}

#[test]
fn a_computed_default_survives_the_wire_form() {
    // The `dump` round trip over the same document: a fragment written to the
    // wire form and read back writes the same calls, resolved defaults and
    // all. Nothing about a default travels on the wire — a use site that left
    // an argument out carries no content for it, and the default is resolved
    // where the call is written — so this states that the piece added no
    // dependency on anything the wire form drops.
    let registry = registry_over(vec![tag(), marked()]);
    let live = Arc::new(common::parse(registry.lang(), COMPUTED_DEFAULT_DOCUMENT));
    let elements: Vec<Arc<ElementSpec>> =
        registry.elements().iter().map(|entry| Arc::clone(&entry.element)).collect();
    let read = common::dump::round_trip(registry.lang(), &live, elements);
    let mut content = DocContent::new();
    content.place(read);
    let doc = Arc::new(Doc::new(content));
    let plan = plan(&registry, WORKFLOW, &Overrides::none());
    let run =
        common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    assert_eq!(body(&run), body(&recomposed(vec![tag(), marked()], COMPUTED_DEFAULT_DOCUMENT)));
}

#[test]
fn a_computed_default_holding_a_use_site_writes_that_use_site_as_its_own_call() {
    // A default may hold a use site of another definition. The resolved
    // default is spliced at the call site, so the inner use site is lowered
    // there like any other: written as its own call where that definition is
    // recomposable, instantiated where it is not. Nothing of this reaches the
    // outer definition in the style file.
    let inner = Definition::macro_("inner", vec![ArgDef::mandatory("x")]).content("<#{x}>");
    let outer = Definition::macro_("outer", vec![ArgDef::optional("a")])
        .content("[#{a}]")
        .default("a", "\\inner{fallback}");
    let run = recomposed(vec![inner, outer], "\\outer and \\outer[written].\n");
    let text = body(&run);
    // The space after the first `\outer` was read while looking for its
    // optional argument, as it is for any optional argument.
    assert!(text.contains("\\flmUDouter{\\flmUDinner{fallback}}and"), "{text}");
    assert!(text.contains("\\flmUDouter{written}."), "{text}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

// --- a definition overriding the `literals` feature's `~` ----------------------------------------

#[test]
fn an_overriding_specials_definition_of_the_tilde_instantiates_with_its_note() {
    // The `literals` feature declares `~` for a non-breaking space; a user
    // definition of the same trigger says that it deliberately replaces it.
    // A specials definition is never recomposable, so the use site is
    // instantiated and the style file holds no user definition.
    let tilde = Definition::specials("~", Vec::new()).overriding().content("TILDE");
    let registry = registry_over(vec![tilde]);
    let run = recompose(&registry, &["a~b"]);
    assert_eq!(output(&run), "aTILDEb%%\n");
    assert_no_user_definition(&run);
    assert_eq!(not_recomposable(&registry), [(String::from("~"), NotRecomposable::Specials)]);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

// --- use sites inside mathematics -----------------------------------------------------------------

/// `\norm{x}`, a definition applying in mathematics mode alone, with an
/// argument.
fn norm() -> Definition {
    Definition::macro_("norm", vec![ArgDef::mandatory("x")]).math("\\lVert #1 \\rVert")
}

#[test]
fn a_math_only_definition_is_called_inside_mathematics_and_on_an_equation_line() {
    let registry = registry_over(vec![caln()]);
    let document = "\
Test \\(\\calN(\\rho)\\) and \\[ \\calN \\rho \\] then
\\begin{align}
  \\calN(\\rho) &= \\rho \\label{eq:a}\\\\
  x &= \\calN
\\end{align}
";
    let run = recompose(&registry, &[document]);
    assert_eq!(
        output(&run),
        "Test \\(\\flmUDcalN(\\rho)\\) and \\[ \\flmUDcalN \\rho \\] then\n\
         \\begin{align}\n  \\flmUDcalN(\\rho) &= \\rho \\flmLabels{\\label{ref1}}\\\\\n  \
         x &= \\flmUDcalN\n\\end{align}\n%%\n"
    );
    let (style, _) = style(&run);
    assert_eq!(
        style.matches("\\providecommand\\flmUDcalN{\\mathcal{N}}\n").count(),
        1,
        "{style}"
    );
    assert_eq!(not_recomposable(&registry), []);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("userdefs-math.tex", &run);
}

#[test]
fn a_non_recomposable_definition_inside_mathematics_is_expanded_in_place() {
    // The definition below cannot be recomposed because its template uses the
    // document's own specials `@`, and a specials — a trigger that needs no
    // backslash — has no LaTeX form at all, so the whole use site is expanded.
    let bold = Definition::macro_("bv", vec![ArgDef::optional("k"), ArgDef::mandatory("x")])
        .math("\\IfProvidedTF{#1}{\\boldsymbol{#2}}{\\mathbf{#2}}@");
    let registry = registry_over(vec![bold, caln(), at()]);
    let run = recompose_with(
        &registry,
        &["\\(\\bv[b]{x} + \\bv{\\calN}\\) and \\begin{equation}\\bv{y}\\end{equation}"],
        author_needs_amsmath(),
    );
    // The expansion is written as source. As in text, the instance is
    // expanded whole by the pass: the `\calN` substituted for the argument
    // is expanded with it rather than called, and so is the specials.
    assert_eq!(
        output(&run),
        "\\(\\boldsymbol{x}[at] + \\mathbf{\\mathcal{N}}[at]\\) and \
\\begin{equation}\\mathbf{y}[at]\\end{equation}%%\n"
    );
    assert_no_user_definition(&run);
    assert_eq!(
        not_recomposable(&registry),
        [
            (String::from("\\bv"), NotRecomposable::Uses { name: String::from("@") }),
            (String::from("@"), NotRecomposable::Specials),
        ]
    );
    assert_golden("userdefs-math-expanded.tex", &run);
}

#[test]
fn a_both_modes_definition_used_in_text_and_mathematics_is_defined_once() {
    let kw = Definition::macro_("kw", vec![ArgDef::mandatory("x")]).content("\\textbf{#1}");
    let run = recomposed(vec![kw], "\\kw{a} and \\(\\kw{b} + 1\\).");
    assert_eq!(output(&run), "\\flmUDkw{a} and \\(\\flmUDkw{b} + 1\\).%%\n");
    let (style, _) = style(&run);
    assert_eq!(
        style.matches("\\providecommand\\flmUDkw[1]{\\textbf{#1}}\n").count(),
        1,
        "{style}"
    );
    assert_golden("userdefs-math-both-modes.tex", &run);
}

#[test]
fn a_defaults_content_is_lowered_in_text_and_written_as_source_inside_mathematics() {
    // An omitted argument's declared default is written out at the call, and
    // it is written the way the content around the call is written: lowered
    // through the ordinary rules in text, so the command in this default
    // comes out in the emitter's normalized spelling with no space between
    // its name and its argument; and from its nodes, byte for byte, inside
    // mathematics, where nothing is lowered and the source's own spacing
    // stands. Both print the same thing, and the golden compiles both; what
    // the golden pins is that the two are not the same bytes, which is the
    // mathematics writer's standing rule and not a rule of the call.
    let qty = Definition::macro_("qty", vec![ArgDef::optional("unit"), ArgDef::mandatory("n")])
        .content("#2 (#1)")
        .default("unit", "\\textbf  {none}");
    let document = "\
Text: \\qty{5} and \\qty[m]{5}.

Math: \\(\\qty{5} + 1\\).
";
    let run = recomposed(vec![qty], document);
    let text = body(&run);
    assert!(text.contains("\\flmUDqty{\\textbf{none}}{5} and \\flmUDqty{m}{5}."), "{text}");
    assert!(text.contains("\\(\\flmUDqty{\\textbf  {none}}{5} + 1\\)"), "{text}");
    let (style, _) = style(&run);
    assert_eq!(style.matches("\\providecommand\\flmUDqty[2]{#2 (#1)}\n").count(), 1, "{style}");
    assert_golden("userdefs-default-lowered.tex", &run);
}

#[test]
fn a_use_site_nested_in_a_mathematics_argument_or_group_is_handled() {
    let registry = registry_over(vec![norm(), caln()]);
    let run = recompose_with(
        &registry,
        &["\\(\\norm{\\calN(\\rho)} \\le \\frac{\\calN}{2}\\)"],
        author_needs_amsmath(),
    );
    assert_eq!(output(&run), "\\(\\flmUDnorm{\\flmUDcalN(\\rho)} \\le \\frac{\\flmUDcalN}{2}\\)%%\n");
    let (style, _) = style(&run);
    assert_eq!(
        style.matches("\\providecommand\\flmUDnorm[1]{\\lVert #1 \\rVert}\n").count(),
        1,
        "{style}"
    );
    assert_eq!(
        style.matches("\\providecommand\\flmUDcalN{\\mathcal{N}}\n").count(),
        1,
        "{style}"
    );
    assert_golden("userdefs-math-nested.tex", &run);
}

#[test]
fn a_use_site_inside_a_nested_environment_inside_mathematics_is_handled() {
    // `pmatrix` is no FLM element: inside mathematics it is an opaque
    // environment, a callable with a body, neither a group nor a list, so the
    // use site in its body is reached through the node's source with the
    // body's span cut out — inline, and on an equation line.
    let registry = registry_over(vec![caln()]);
    let document = "\
\\(\\begin{pmatrix}\\calN & 0\\\\ 0 & \\calN\\end{pmatrix}\\)
\\begin{align}
  M &= \\begin{pmatrix} \\calN \\end{pmatrix}
\\end{align}
";
    let run = recompose(&registry, &[document]);
    assert_eq!(
        output(&run),
        "\\(\\begin{pmatrix}\\flmUDcalN & 0\\\\ 0 & \\flmUDcalN\\end{pmatrix}\\)\n\
         \\begin{align}\n  M &= \\begin{pmatrix} \\flmUDcalN \\end{pmatrix}\n\\end{align}\n%%\n"
    );
    let (style, _) = style(&run);
    assert_eq!(
        style.matches("\\providecommand\\flmUDcalN{\\mathcal{N}}\n").count(),
        1,
        "{style}"
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("userdefs-math-environment-nested.tex", &run);
}

#[test]
fn a_placeholder_inside_mathematics_in_a_text_template_is_written_as_a_parameter() {
    // The text-mode template is spliced through the rules; the `\(…\)` inside
    // it meets the `math` feature's rule, and the stamped placeholder is
    // written by the mathematics writer. The name is not one the
    // Unicode-to-LaTeX table spells a character with, so it is not reserved
    // and is recomposed as the document declares it.
    let inmath = Definition::macro_("inmath", vec![ArgDef::mandatory("a")]).content("\\(#1\\)");
    let registry = registry_over(vec![inmath]);
    let run = recompose(&registry, &["Take \\inmath{x}."]);
    assert_eq!(output(&run), "Take \\flmUDinmath{x}.%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDinmath[1]{\\(#1\\)}\n"), "{style}");
    assert_eq!(not_recomposable(&registry), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_definition_that_never_stops_expanding_is_written_as_the_macro_that_never_stops() {
    // A definition that uses itself is written as it stands: the style file
    // holds a macro whose body calls that same macro, and the use site is an
    // ordinary call. Writing a definition as a LaTeX definition is asking
    // LaTeX to run its expansion, so a recursion that never stops is LaTeX's
    // to find, and nothing is refused up front.
    //
    // Where the same document is *expanded* — on the `text` backend, and in
    // every check — FLM's own bound still holds: the expansion pass stops at
    // the configured depth limit and reports `flm.userdefs.recursion-limit`
    // as an Error at the use site, which it recovers as its own literal text.
    // So a document with a runaway definition is reported as broken by the
    // workflows that expand it, and by LaTeX where it is only recomposed.
    let recursive = Definition::macro_("foo", Vec::new()).content("\\foo");
    let registry = common::stdcore::registry(StdcoreConfig::new().userdefs(
        UserdefsConfig::new().definitions(vec![recursive]).recursion_limit(2),
    ));
    let run = recompose(&registry, &["\\foo"]);
    assert_eq!(output(&run), "\\flmUDfoo%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDfoo{\\flmUDfoo}\n"), "{style}");
    // Nothing was expanded, so no depth limit was reached.
    expect_none(run.diagnostics().iter(), "flm.userdefs.recursion-limit");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    // The same document expanded: the Error, with the configured limit.
    let on_text = run_workflow(&registry, text::WORKFLOW, &["\\foo"], &Overrides::none());
    let diagnostic = expect(on_text.diagnostics().iter(), "flm.userdefs.recursion-limit").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let condition = diagnostic.data().downcast_ref::<RecursionLimit>().expect("typed");
    assert_eq!(condition.name, "\\foo");
    assert_eq!(condition.limit, 2);
    // The use site at the limit is recovered as its own literal text.
    assert_eq!(common::lowering::text(&on_text), "\\foo\n");
}

#[test]
fn a_recursion_that_stops_is_recomposed_as_a_macro_that_calls_itself() {
    // The other half: a recursion that does stop. `\peel` tests its argument
    // and calls itself with an empty one, so it stops after one step in both
    // worlds. The macro's body calls the macro, and the call in the heading
    // title is the standing requirement — a title is a moving argument, and
    // `tests/latex_compile.rs` compiles this golden, compares what it prints
    // with what the expansion prints, and reads the table of contents back.
    let document = "\\section{A \\peel{y} title}\n\nAnd \\peel{z} in the body.\n";
    let run = recomposed(vec![peel()], document);
    let text = body(&run);
    assert!(text.contains("\\section{A \\flmUDpeel{y} title}"), "{text}");
    assert!(text.contains("And \\flmUDpeel{z} in the body."), "{text}");
    let (style, _) = style(&run);
    assert!(
        style.contains("\\providecommand\\flmUDpeel[1]{\\flmIfBlankTF{#1}{.}{#1\\flmUDpeel{}}}\n"),
        "{style}"
    );
    expect_none(run.diagnostics().iter(), "flm.userdefs.recursion-limit");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-recursive.tex", &run);
    // The same document with the definition always expanded, which is what
    // the compile suite compares the recomposed form's setting against.
    let expanded = recomposed(vec![peel().expand(ExpandPolicy::Always)], document);
    assert!(body(&expanded).contains("\\section{A y. title}"), "{}", body(&expanded));
    assert!(!expanded.diagnostics().any_recorded(), "{}", expanded.diagnostics().render_all());
    assert_golden("userdefs-recursive-expanded.tex", &expanded);
}

#[test]
fn a_definition_that_uses_itself_through_another_is_written_as_two_macros_that_call_each_other() {
    // The mutual cycle: `\ping` uses `\pong` and `\pong` uses `\ping`. The
    // guard that stops a definition's own template from staging its construct
    // a second time is kept per thread, not per call, so it has to hold across
    // the *nested* staging too — while `\ping`'s construct is being built,
    // building `\pong`'s reaches a use site of `\ping` and must write a call
    // there rather than start `\ping` over. Each definition therefore stands
    // once in the style file, and every use site is a call.
    //
    // Both stop: each tests its argument and calls the other with an empty
    // one, which the blank test answers.
    let ping = Definition::macro_("ping", vec![ArgDef::mandatory("rest")])
        .content("\\ifblank{#{rest}}{.}{#{rest}\\pong{}}");
    let pong = Definition::macro_("pong", vec![ArgDef::mandatory("rest")])
        .content("\\ifblank{#{rest}}{!}{#{rest}\\ping{}}");
    let registry = registry_over(vec![ping, pong]);
    let run = recompose(&registry, &["\\ping{a} and \\pong{b}.\n"]);
    let text = body(&run);
    assert!(text.contains("\\flmUDping{a} and \\flmUDpong{b}."), "{text}");
    let (style, _) = style(&run);
    // Each macro is written exactly once, and each body calls the other.
    assert!(
        style.contains("\\providecommand\\flmUDping[1]{\\flmIfBlankTF{#1}{.}{#1\\flmUDpong{}}}\n"),
        "{style}"
    );
    assert!(
        style.contains("\\providecommand\\flmUDpong[1]{\\flmIfBlankTF{#1}{!}{#1\\flmUDping{}}}\n"),
        "{style}"
    );
    assert_eq!(style.matches("\\providecommand\\flmUDping").count(), 1, "{style}");
    assert_eq!(style.matches("\\providecommand\\flmUDpong").count(), 1, "{style}");
    assert_eq!(not_recomposable(&registry), []);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn two_documents_in_one_run_each_using_the_recursion_are_each_written_whole() {
    // The staging guard is kept on the thread that lowers, so a name left on
    // it after one document would make the next document lowered on that same
    // thread answer "already being staged" for that definition and leave it
    // out of its style file. Two documents in one run, each using the
    // recursive definition, is the case that would catch it; the battery runs
    // this file under `RAYON_NUM_THREADS=1` as well, where both documents are
    // certain to be lowered on one thread.
    let registry = registry_over(vec![peel()]);
    let run = recompose(&registry, &["First \\peel{y}.", "Second \\peel{z}."]);
    assert_eq!(output(&run), "First \\flmUDpeel{y}.%%\nSecond \\flmUDpeel{z}.%%\n");
    let (style, _) = style(&run);
    assert!(
        style.contains("\\providecommand\\flmUDpeel[1]{\\flmIfBlankTF{#1}{.}{#1\\flmUDpeel{}}}\n"),
        "{style}"
    );
    assert_eq!(style.matches("\\providecommand\\flmUDpeel").count(), 1, "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn no_userdefs_build_diagnostic_is_a_note() {
    // The retired build Note has no successor. Over a registry holding every
    // definition shape these tests declare — the three classes written
    // expanded, the recursion, every argument shape, the mode twins, a macro
    // and an environment of one name, a packed definition, an environment
    // whose template never names its body, a tack-on argument, and the two
    // refusals — not one diagnostic the registry build records for the
    // `userdefs` feature is a Note. Every one of them is a real problem with
    // the definition, reported at the severity such a problem carries.
    //
    // Why a definition is written expanded is not reported at all: it is
    // recorded on the definition and read back from there (`not_recomposable`,
    // and the tests of the three classes above).
    let mut definitions = vec![
        // Recomposed, of every shape.
        hello(),
        note(),
        mymacro(),
        // The two conditional shapes, under names of their own: a question
        // about an argument's value, and a question about whether it was
        // written at all over an argument that has a declared default.
        Definition::macro_("cond", vec![ArgDef::optional("a"), ArgDef::mandatory("b")])
            .content("\\IfNoValueTF{#1}{H, \\emph{#2}}{#1, \\emph{#2}}")
            .default("a", "Hello"),
        Definition::macro_("prov", vec![ArgDef::optional("a"), ArgDef::mandatory("b")])
            .content("\\IfProvidedTF{#1}{#1, \\emph{#2}}{H there, \\emph{#2}}")
            .default("a", "Hello"),
        around(),
        range(),
        myenv(),
        caln(),
        kv(),
        code(),
        peel(),
        Definition::macro_("star", vec![ArgDef::new(ArgStdKind::Star), ArgDef::mandatory("x")])
            .content("\\IfBooleanTF{#1}{S}{s}#2"),
        Definition::macro_("ten", (0..10).map(|i| ArgDef::mandatory(format!("a{i}"))).collect())
            .content("x"),
        Definition::environment("drops", Vec::new()).content("[nothing]"),
        Definition::macro_("pair", Vec::new()).content("m"),
        Definition::environment("pair", Vec::new()).content("#{body}"),
        Definition::macro_("tack", vec![ArgDef::mandatory("a"), thm_label_arg()]).content("#1!"),
        // Written expanded: by nature, by choice, and by using one of those.
        at(),
        version(),
        sic(),
        Definition::macro_("mail", Vec::new()).content("a@b"),
        // Written expanded by their own error, which is where the build
        // diagnostics below come from: a template naming an argument the
        // definition does not declare, and a template naming what a tack-on
        // argument absorbs.
        Definition::macro_("broken", Vec::new()).content("#1"),
        Definition::macro_("named", vec![ArgDef::mandatory("title"), thm_label_arg()])
            .content("\\textbf{#{title}}#{fields}"),
    ];
    definitions.extend(vec_twins());
    let registry = registry_over(definitions);
    let recorded: Vec<(String, Severity)> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier().starts_with("flm.userdefs."))
        .map(|d| (d.identifier().to_string(), d.severity()))
        .collect();
    // The population really does record diagnostics, so the assertion below
    // stands over something.
    assert!(!recorded.is_empty(), "{recorded:?}");
    for (identifier, severity) in &recorded {
        assert_ne!(*severity, Severity::Note, "{identifier} is a Note");
    }
}

#[test]
fn a_mathematics_template_using_a_definition_puts_both_in_the_style_file() {
    let state = Definition::macro_("state", Vec::new()).math("\\calN(\\rho)");
    let registry = registry_over(vec![state, caln()]);
    let run = recompose(&registry, &["\\(\\state = \\state\\)"]);
    assert_eq!(output(&run), "\\(\\flmUDstate = \\flmUDstate\\)%%\n");
    let (style, _) = style(&run);
    let caln = style.find("\\providecommand\\flmUDcalN{\\mathcal{N}}").expect("calN");
    let state = style
        .find("\\providecommand\\flmUDstate{\\flmUDcalN(\\rho)}")
        .expect("state");
    assert!(caln < state, "{style}");
    assert_eq!(style.matches("\\providecommand").count(), 2, "{style}");
}

#[test]
fn a_placeholder_inside_verbatim_content_expands_the_use_site() {
    // Verbatim content is read as characters, and a placeholder's characters
    // are known at a call and not before: an argument a template names inside
    // inline verbatim content or inside a verbatim block has every use site of
    // its definition expanded, and the recomposed LaTeX is what the expansion
    // writes. The definitions stay recomposable: it is the lowering of their
    // template that expands the use sites.
    let vb = || Definition::macro_("vb", vec![ArgDef::mandatory("x")]).content("\\verbcode{#1}");
    let vbb = || {
        Definition::macro_("vbb", vec![ArgDef::mandatory("x")])
            .content("\\begin{verbatimcode}\nv = #1;\nend\n\\end{verbatimcode}")
    };
    let registry = registry_over(vec![vb(), vbb()]);
    assert_eq!(not_recomposable(&registry), []);
    let document = "\\vb{a_b & c}\n\n\\vbb{a_b & c}\n";
    let run = recompose(&registry, &[document]);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let text = body(&run);
    assert!(text.contains("\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{a\\_b\\ \\&\\ c}"), "{text}");
    assert!(
        text.contains("\\begin{flmVerbatimCode}[]\nv = a_b & c;\nend\n\\end{flmVerbatimCode}"),
        "{text}"
    );
    assert!(!style(&run).0.contains("\\flmUD"), "{}", style(&run).0);
    assert_eq!(text, body(&recomposed(vec![always(vb()), always(vbb())], document)));
    assert_golden("userdefs-verbatim-placeholder-expanded.tex", &run);
}

#[test]
fn a_placeholder_inside_verbatim_content_of_a_default_is_resolved_at_the_call() {
    // A declared default is resolved at the use site and written into its
    // argument's own value group as ordinary content, so a placeholder inside
    // verbatim content of a *default* is answered there, and the definition
    // is written as a LaTeX definition and its use site as a call.
    let vd = Definition::macro_("vd", vec![ArgDef::optional("x"), ArgDef::mandatory("y")])
        .content("#1#2")
        .default("x", "\\verbcode{#2}");
    let registry = registry_over(vec![vd]);
    assert_eq!(not_recomposable(&registry), []);
    let run = recompose(&registry, &["\\vd{z}"]);
    assert_eq!(
        output(&run),
        "\\flmUDvd{\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{z}}{z}%%\n"
    );
}

// --- the recomposed names and the `bare_names` option -------------------------------------------

#[test]
fn every_kept_definition_is_recomposed_under_its_prefixed_name() {
    // `\par` is LaTeX's own and `\label` is written by the recomposed LaTeX for
    // every label, but the prefix is not about them: every kept definition
    // takes it, at the definition and at every call, in text and inside
    // mathematics alike.
    let par = Definition::macro_("par", Vec::new()).content("[par]");
    let label = Definition::macro_("label", vec![ArgDef::mandatory("x")]).overriding().content("[label: #1]");
    let registry = registry_over(vec![par, label]);
    let run = recompose(&registry, &["A\\par B \\label{x} \\(\\par\\)"]);
    assert_eq!(
        output(&run),
        "A\\flmUDpar B \\flmUDlabel{x} \\(\\flmUDpar\\)%%\n"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDpar{[par]}\n"), "{style}");
    assert!(style.contains("\\providecommand\\flmUDlabel[1]{[label: #1]}\n"), "{style}");
    assert!(!style.contains("\\par{}"), "{style}");
    // The comment line above each definition names the author's own
    // spelling beside the name the file defines.
    assert!(style.contains("% \\flmUDpar (the document's \\par):"), "{style}");
    assert_eq!(not_recomposable(&registry), []);
    // Nothing is reported: the renaming is the rule, not an event.
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-prefixed.tex", &run);
}

#[test]
fn a_macro_whose_name_is_not_ascii_letters_is_recomposed_under_a_letters_only_encoding() {
    // A LaTeX control word holds letters only, so a name holding anything
    // else is encoded: each character's code point in hexadecimal with the
    // decimal digits written as the letters `a` to `j`, the characters
    // joined by `x`. `~` is U+007E, so `7E` becomes `hE`; `\u{e9}` is U+00E9,
    // so `E9` becomes `Ej`.
    let tilde = Definition::macro_("~", Vec::new()).content("[tilde]");
    let accented = Definition::macro_("\u{e9}", vec![ArgDef::mandatory("x")]).content("[#1]");
    let registry = registry_over(vec![tilde, accented]);
    let run = recompose(&registry, &["A\\~ B \\\u{e9}{x} \\(\\~\\)"]);
    assert_eq!(
        output(&run),
        "A\\flmUXhE B \\flmUXEj{x} \\(\\flmUXhE\\)%%\n"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUXhE{[tilde]}\n"), "{style}");
    assert!(style.contains("% \\flmUXhE (the document's \\~):"), "{style}");
    assert!(style.contains("\\providecommand\\flmUXEj[1]{[#1]}\n"), "{style}");
    // The recomposed name is a plain LaTeX control word: letters only. The
    // author's own spelling appears in the comment line and nowhere else.
    assert_eq!(style.matches('\u{e9}').count(), 1, "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-encoded-names.tex", &run);
}

#[test]
fn an_environment_takes_the_environment_prefix_and_keeps_its_name() {
    // A macro and an environment share one name space in LaTeX
    // (`\newenvironment{foo}` defines `\foo`), so the two prefixes differ.
    // An environment name is only ever looked up by `\begin{…}`, never
    // spelled as a control word, so it is never encoded.
    let note = Definition::environment("note", Vec::new()).content("[Note: #{body}]");
    let noted = Definition::macro_("noted", Vec::new()).content("[noted]");
    let registry = registry_over(vec![note, noted]);
    let run = recompose(&registry, &["\\begin{note}n\\end{note} \\noted"]);
    assert_eq!(output(&run), "\\begin{flmUEnote}n\\end{flmUEnote} \\flmUDnoted%%\n");
    let (style, _) = style(&run);
    assert!(
        style.contains("\\ProvideDocumentEnvironment{flmUEnote}{}{[Note: }{]}\n"),
        "{style}"
    );
    assert!(style.contains("% flmUEnote (the document's note):"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_name_listed_in_bare_names_is_written_exactly_as_the_document_spelled_it() {
    // The author who lists a name says they want LaTeX's own command
    // replaced for the whole document.
    let emph = Definition::macro_("emph", vec![ArgDef::mandatory("x")]).overriding().content("\\textit{#1}");
    let registry = registry_over(vec![emph]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("emph")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run = run_workflow(&registry, WORKFLOW, &["\\emph{x}"], &overrides);
    assert_eq!(output(&run), "\\emph{x}%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\long\\def\\emph#1{\\textit{#1}}\n"), "{style}");
    // The names match, so the comment line names the definition once.
    assert!(!style.contains("(the document's"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-bare-name.tex", &run);
}

#[test]
fn a_bare_name_beginning_with_flm_is_refused_with_an_error() {
    // `flm` is the prefix of every name this library's own LaTeX output
    // writes; it is the one name space a user definition cannot claim.
    let hello = Definition::macro_("hello", vec![ArgDef::mandatory("x")]).content("Hello, #1!");
    let registry = registry_over(vec![hello]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("flmHeading")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run = run_workflow(&registry, WORKFLOW, &["\\hello{World}"], &overrides);
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = diagnostic.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.name, "flmHeading");
    // The refusal changes nothing else: the document's own definition is
    // recomposed under its prefix as usual.
    assert_eq!(output(&run), "\\flmUDhello{World}%%\n");
}

#[test]
fn a_bare_name_beginning_with_flm_is_refused_even_for_a_document_holding_nothing() {
    // The option is wrong whatever the document says, so the check sits
    // before the final stage's early returns and a document with no user
    // definition — indeed no content at all — still reports it.
    let registry = registry_over(vec![hello()]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("flmRef")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run = run_workflow(&registry, WORKFLOW, &[""], &overrides);
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = diagnostic.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.name, "flmRef");
}

#[test]
fn an_environment_listed_in_bare_names_is_written_bare_too() {
    // One entry covers a macro and an environment of that spelling alike, so
    // the option reaches the environment prefix as well as the macro one.
    let note = Definition::environment("note", Vec::new()).content("[Note: #{body}]");
    let registry = registry_over(vec![note]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("note")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run = run_workflow(&registry, WORKFLOW, &["\\begin{note}n\\end{note}"], &overrides);
    assert_eq!(output(&run), "\\begin{note}n\\end{note}%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\DeclareDocumentEnvironment{note}{}{[Note: }{]}\n"), "{style}");
    assert!(!style.contains("(the document's"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_bare_name_declared_as_both_a_macro_and_an_environment_is_refused() {
    // One `bare_names` entry covers both kinds of its spelling, and LaTeX
    // keeps both kinds in one name space: `\DeclareDocumentEnvironment{foo}`
    // defines `\foo` itself, so the environment's definition would replace
    // the macro's. The entry is refused rather than left to do that quietly.
    let macro_foo = Definition::macro_("foo", vec![ArgDef::mandatory("x")]).content("[macro #1]");
    let env_foo = Definition::environment("foo", Vec::new()).content("[env: #{body}]");
    let registry = registry_over(vec![macro_foo, env_foo]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("foo")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run =
        run_workflow(&registry, WORKFLOW, &["\\foo{a} \\begin{foo}b\\end{foo}"], &overrides);
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = diagnostic.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.name, "foo");
    assert_eq!(condition.reason, DefinitionNameRefusal::MacroAndEnvironment);
    assert_eq!(condition.reason.kind(), "macro-and-environment");
}

#[test]
fn a_bare_macro_named_for_the_end_of_a_bare_environment_is_refused() {
    // The other shape of the one collision: `\DeclareDocumentEnvironment{note}`
    // defines `\endnote` as well as `\note`, so a macro `endnote` written
    // bare beside an environment `note` written bare is replaced just as
    // silently. The entry refused is the macro's, the one that would be lost.
    let note = Definition::environment("note", Vec::new()).content("[Note: #{body}]");
    let endnote = Definition::macro_("endnote", vec![ArgDef::mandatory("x")]).content("[end #1]");
    let registry = registry_over(vec![note, endnote]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("note"), String::from("endnote")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run = run_workflow(
        &registry,
        WORKFLOW,
        &["\\begin{note}n\\end{note} \\endnote{a}"],
        &overrides,
    );
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = diagnostic.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.name, "endnote");
    assert_eq!(condition.reason, DefinitionNameRefusal::EndOfEnvironment);
    assert_eq!(condition.reason.kind(), "end-of-environment");
}

#[test]
fn a_bare_macro_beside_a_prefixed_environment_of_the_end_shape_is_not_refused() {
    // The refusal is about the LaTeX names the style file actually defines.
    // With the environment left under its prefix, its commands are
    // `\flmUEnote` and `\endflmUEnote`, which the bare macro `\endnote`
    // never meets, so nothing is refused.
    let note = Definition::environment("note", Vec::new()).content("[Note: #{body}]");
    let endnote = Definition::macro_("endnote", vec![ArgDef::mandatory("x")]).content("[end #1]");
    let registry = registry_over(vec![note, endnote]);
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("endnote")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides = Overrides {
        render_options: OptionValues::new().with(render),
        ..Overrides::none()
    };
    let run = run_workflow(
        &registry,
        WORKFLOW,
        &["\\begin{note}n\\end{note} \\endnote{a}"],
        &overrides,
    );
    assert_eq!(output(&run), "\\begin{flmUEnote}n\\end{flmUEnote} \\endnote{a}%%\n");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn an_overriding_definition_of_emph_is_prefixed_like_any_other() {
    // `emph` is an FLM element the definition overrides. Overriding plays no
    // part in recomposition: the definition is renamed like every other, and
    // `bare_names` is the only way to have LaTeX's `\emph` replaced.
    let emph = Definition::macro_("emph", vec![ArgDef::mandatory("x")]).overriding().content("\\textit{#1}");
    let registry = registry_over(vec![emph]);
    let run = recompose(&registry, &["\\emph{x}"]);
    assert_eq!(output(&run), "\\flmUDemph{x}%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDemph[1]{\\textit{#1}}\n"), "{style}");
    assert!(!style.contains("\\ifdefined"), "{style}");
    assert_eq!(not_recomposable(&registry), []);
    assert_golden("userdefs-overriding.tex", &run);
}

// --- the `robust_names` option ------------------------------------------------------------------

/// The render options with `robust_names` holding `names` and nothing else set.
fn robust(names: &[&str]) -> LatexRecomposedRenderOptions {
    LatexRecomposedRenderOptions {
        robust_names: names.iter().map(|name| String::from(*name)).collect(),
        ..LatexRecomposedRenderOptions::default()
    }
}

/// The `latex-recomposed` workflow over the package with `definitions`, one
/// tree of `text`, under the render options `render`.
fn recomposed_with(
    definitions: Vec<Definition>,
    text: &str,
    render: LatexRecomposedRenderOptions,
) -> Run {
    let overrides =
        Overrides { render_options: OptionValues::new().with(render), ..Overrides::none() };
    run_workflow(&registry_over(definitions), WORKFLOW, &[text], &overrides)
}

#[test]
fn a_name_listed_in_robust_names_is_written_as_a_document_command_and_works_in_a_heading() {
    // Every recomposed macro is a plain expandable macro, which is what lets a
    // call of it reach a PDF bookmark as the definition's own words. A name
    // listed in `robust_names` is written the other way instead: the document
    // command interface, one `+m` per argument, which makes an
    // engine-protected — *robust* — command. LaTeX then writes the call itself
    // into the table of contents and the auxiliary file rather than what it
    // stands for, and `hyperref` drops it from a PDF bookmark.
    // `tests/latex_compile.rs` compiles this golden and probes all three.
    //
    // The call is not touched: a call writes one brace group per effective
    // argument whichever way the definition is written, which is why nothing
    // about the option can be refused for the shape of a definition.
    //
    // The document also declares a definition of *no* arguments, written
    // robustly as well, so that the empty argument specification
    // `\ProvideDocumentCommand\flmUDsig{}{…}` — which LaTeX accepts, and which
    // no other compiled golden carries — is compiled by `tests/latex_compile.rs`
    // along with the rest.
    let document = "\
\\section{Greeting \\hello{World}}\\label{sec:greet}

\\hello{Berta} in the body, and see \\ref{sec:greet}. \\sig
";
    let sig = Definition::macro_("sig", Vec::new()).content("Yours truly");
    let run = recomposed_with(vec![hello(), sig.clone()], document, robust(&["hello", "sig"]));
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentCommand\\flmUDhello{+m}{Hello, #1!}\n"), "{style}");
    assert!(!style.contains("\\providecommand\\flmUDhello"), "{style}");
    // The definition of no arguments: an empty argument specification, and no
    // `[0]` or bare head anywhere.
    assert!(style.contains("\\ProvideDocumentCommand\\flmUDsig{}{Yours truly}\n"), "{style}");
    // The guard is the same one an expandable definition has, so the comment
    // line above it says the same thing.
    assert!(
        style.contains("% \\flmUDhello (the document's \\hello): defined unless the name is"),
        "{style}"
    );
    let text = body(&run);
    assert!(text.contains("\\section{Greeting \\flmUDhello{World}}"), "{text}");
    // Byte for byte the recomposed LaTeX of the same document without the option:
    // only the style file's definition changes (and with it the hash that
    // names the file, which is why the comparison is of the recomposed LaTeX after
    // its header line).
    let expandable = recomposed(vec![hello(), sig], document);
    assert_eq!(output(&run), output(&expandable));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-robust.tex", &run);
}

#[test]
fn a_bare_name_written_robustly_takes_the_unconditional_document_command() {
    // The two options meet: the name is the document's own, so the definition
    // is made unconditionally — that is what listing it in `bare_names` asks
    // for — and it is robust, so it is made through the document command
    // interface. `\DeclareDocumentCommand` is both.
    let emph = Definition::macro_("emph", vec![ArgDef::mandatory("x")])
        .overriding()
        .content("\\textit{#1}");
    let render = LatexRecomposedRenderOptions {
        bare_names: [String::from("emph")].into_iter().collect(),
        robust_names: [String::from("emph")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recomposed_with(vec![emph], "\\emph{x}", render);
    assert_eq!(output(&run), "\\emph{x}%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\DeclareDocumentCommand\\emph{+m}{\\textit{#1}}\n"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_packed_and_a_flagged_definition_are_written_robustly_like_any_other() {
    // Nothing about a definition's shape can stand in the way. A definition of
    // more than nine effective arguments hands them over in one group, so the
    // robust form declares exactly one `+m` — the same one parameter the
    // expandable form names — and the body reads argument *n* out of it. A
    // definition the template asks a question about carries its flags as
    // ordinary effective arguments, so they are `+m` too.
    let document = "\
\\section{Big \\big{A}{b}{c}{d}{e}{f}{G}{h}{i}{j}{k}{L} and \\mymacro{Ada}}\\label{sec:robust}

\\big{one}{b}{c}{d}{e}{f}{seven}{h}{i}{j}{k}{twelve}, \\mymacro[Hi]{Bo} and
\\mymacro{Cy}. See \\ref{sec:robust}.
";
    let definitions = vec![big(), mymacro_provided()];
    let run = recomposed_with(definitions.clone(), document, robust(&["big", "mymacro"]));
    let (style, _) = style(&run);
    assert!(
        style.contains(
            "\\ProvideDocumentCommand\\flmUDbig{+m}\
             {[\\flmArg{1}{#1}|\\flmArg{7}{#1}|\\flmArg{12}{#1}]}\n"
        ),
        "{style}"
    );
    // Two values and one flag: three effective arguments, three `+m`.
    assert!(
        style.contains(
            "\\ProvideDocumentCommand\\flmUDmymacro{+m +m +m}\
             {\\IfBooleanTF{#3}{#1, \\emph{#2}}{Hello there, \\emph{#2}}}\n"
        ),
        "{style}"
    );
    let text = body(&run);
    assert!(
        text.contains("\\flmUDbig{{one}{b}{c}{d}{e}{f}{seven}{h}{i}{j}{k}{twelve}},"),
        "{text}"
    );
    assert!(text.contains("\\flmUDmymacro{Hi}{Bo}{\\BooleanTrue}"), "{text}");
    assert!(text.contains("\\flmUDmymacro{Hello}{Cy}{\\BooleanFalse}"), "{text}");
    // The calls are the ones the expandable form writes, again byte for byte
    // (after the header line, which names the style file by its content).
    assert_eq!(output(&run), output(&recomposed(definitions, document)));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-robust-packed.tex", &run);
}

#[test]
fn a_twinned_name_listed_in_robust_names_makes_both_twins_robust() {
    // One entry is one name the document declared, and a name declared once
    // for text and once for mathematics is two definitions under that one
    // name, so both are written robustly. (`bare_names` refuses such a name,
    // because two definitions cannot share one LaTeX command; here there is
    // nothing to share — each twin keeps its own name and only the spelling of
    // its definition changes.)
    let document = "\
\\section{Vector \\vec{v} and \\(\\vec{w}\\)}\\label{sec:twin}

\\vec{body} and \\(\\vec{math}\\). See \\ref{sec:twin}.
";
    let run = recomposed_with(vec_twins(), document, robust(&["vec"]));
    let (style, _) = style(&run);
    assert!(
        style.contains("\\ProvideDocumentCommand\\flmUTDvec{+m}{\\textbf{#1}}\n"),
        "{style}"
    );
    assert!(
        style.contains("\\ProvideDocumentCommand\\flmUMDvec{+m +m}{\\mathbf{#2}#1}\n"),
        "{style}"
    );
    let text = body(&run);
    assert!(
        text.contains("\\section{Vector \\flmUTDvec{v} and \\(\\flmUMDvec{}{w}\\)}"),
        "{text}"
    );
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("userdefs-robust-twins.tex", &run);
}

#[test]
fn an_environment_listed_in_robust_names_is_accepted_and_changes_nothing() {
    // An environment is robust however it is written: `\begin` opens a group,
    // runs the begin code, sets the body and runs the end code, and expansion
    // alone could produce none of that. So the option has nothing to do for
    // one, and it does nothing — rather than refusing the entry, which would
    // punish a document that lists every name it declares. This holds for the
    // environment that takes its body as an argument as much as for the one
    // whose body LaTeX sets: both are document environments.
    let document = "\
\\begin{note}n\\end{note}

\\begin{wrap}{a}the body\\end{wrap}

\\begin{twice}said twice\\end{twice}
";
    let definitions = vec![note(), wrap(), twice()];
    let listed = recomposed_with(
        definitions.clone(),
        document,
        robust(&["note", "wrap", "twice"]),
    );
    let plain = recomposed(definitions, document);
    // Byte for byte the same run: the recomposed LaTeX and the whole style file.
    assert_eq!(output(&listed), output(&plain));
    assert_eq!(style(&listed), style(&plain));
    let (style, _) = style(&listed);
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUEnote}{}{[Note: }{]}\n"), "{style}");
    assert!(style.contains("\\ProvideDocumentEnvironment{flmUEtwice}{+b}{#1/#1}{}\n"), "{style}");
    // Nothing is reported: the entry is accepted, not refused.
    assert!(!listed.diagnostics().any_recorded(), "{}", listed.diagnostics().render_all());
}

#[test]
fn a_robust_name_beginning_with_flm_is_refused_with_the_error_bare_names_uses() {
    // One rule about a name, one error for it: `flm` is the prefix of every
    // name this library's own LaTeX output writes, and neither option may
    // claim it. The error says which option listed the name and what happened
    // instead — here the definition is written expandably, as it would have
    // been without the entry.
    let run = recomposed_with(vec![hello()], "\\hello{World}", robust(&["flmHeading"]));
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.latex-recomposed.definition-name-refused").once();
    let condition = diagnostic.data().downcast_ref::<DefinitionNameRefused>().expect("typed");
    assert_eq!(condition.option, DefinitionNameOption::RobustNames);
    assert_eq!(condition.option.as_str(), "robust_names");
    assert_eq!(condition.name, "flmHeading");
    assert_eq!(condition.reason, DefinitionNameRefusal::FamilyPrefix);
    let message = diagnostic.to_string();
    assert!(message.contains("the render option `robust_names` lists `flmHeading`"), "{message}");
    assert!(
        message.contains(
            "a name listed there must not begin with `flm`, the prefix reserved for the LaTeX names that this library writes itself"
        ),
        "{message}"
    );
    // The refusal changes nothing else: the document's own definition is
    // written expandably under its prefix.
    assert_eq!(output(&run), "\\flmUDhello{World}%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDhello[1]{Hello, #1!}\n"), "{style}");
}

#[test]
fn a_name_in_robust_names_that_no_definition_carries_is_accepted_in_silence() {
    // The option names definitions by the spelling the document declared them
    // under, and a name no definition carries is simply not met — as for
    // `bare_names`, an entry is a wish about a name and not a claim that the
    // document declares it.
    let run = recomposed_with(vec![hello()], "\\hello{World}", robust(&["nothing"]));
    assert_eq!(output(&run), "\\flmUDhello{World}%%\n");
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDhello[1]{Hello, #1!}\n"), "{style}");
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

// --- `instantiate-at-lowering`, preferred by name ------------------------------------------------

#[test]
fn instantiate_at_lowering_preferred_by_name_writes_the_instance_in_the_recomposed_latex() {
    let definitions = vec![hello(), note()];
    let registry = registry_over(definitions.clone());
    let overrides = prefer_instantiation(BACKEND, &definitions);
    let kept = plan(&registry, WORKFLOW, &overrides);
    assert!(stage_names(&kept).iter().any(|name| name == "flm.userdefs.keep"));
    let run = run_workflow(&registry, WORKFLOW, &["\\hello{World} \\begin{note}n\\end{note}"], &overrides);
    assert_eq!(output(&run), "Hello, World! [Note: n]%%\n");
    assert_no_user_definition(&run);
    assert_golden("userdefs-instantiated.tex", &run);
}

#[test]
fn instantiate_at_lowering_on_the_text_backend_prints_as_the_expanded_document_does() {
    let definitions = vec![hello(), note()];
    let registry = registry_over(definitions.clone());
    let document = "Block 1\n\n\\hello{World}\n\n\\begin{note}n\\end{note} then \\hello{again}.\n";
    let overrides = prefer_instantiation(text::BACKEND, &definitions);
    let kept_plan = plan(&registry, text::WORKFLOW, &overrides);
    assert!(stage_names(&kept_plan).iter().any(|name| name == "flm.userdefs.keep"));
    assert!(!rewrite_names(&kept_plan).iter().any(|name| name == "flm.userdefs.expand"));
    let kept = run_workflow(&registry, text::WORKFLOW, &[document], &overrides);
    let expanded = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(common::lowering::text(&kept), common::lowering::text(&expanded));
    assert_eq!(common::lowering::text(&kept), "Block 1\n\nHello, World!\n\n[Note: n] then Hello, again!.\n");
    assert!(!kept.diagnostics().any_recorded(), "{}", kept.diagnostics().render_all());
}

#[test]
fn keeping_one_definition_while_expanding_another_on_one_backend_is_refused() {
    // On the text backend `\note` keeps its general default, which requires
    // the expansion, while the preferred rule for `\hello` requires the kept
    // alternative: the two are exclusive refinements of one capability.
    let registry = registry_over(vec![hello(), note()]);
    let overrides = prefer_instantiation(text::BACKEND, &[hello()]);
    match try_plan(&registry, text::WORKFLOW, &overrides) {
        Err(PlanError::Unsatisfiable { .. }) => {}
        other => panic!("expected the refusal, got {other:?}"),
    }
    // Demanding the expanded alternative beside the recomposed backend is
    // refused the same way, from the other side.
    let expanded = Overrides {
        demands: vec![CapName::from("flm.userdefs.expanded")],
        ..Overrides::none()
    };
    assert!(try_plan(&registry, WORKFLOW, &expanded).is_err());
}

// --- the seven staged Python rows, twice each ---------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_macro_with_args.
#[test]
fn recompose_macro_with_args_expanded_and_kept() {
    let document = "\\mymacro[Hi]{Albert}";
    let expanded = recomposed(vec![always(mymacro())], document);
    assert_eq!(output(&expanded), "\\textbf{Hi}, \\emph{Albert}%%\n");
    let kept = recomposed(vec![mymacro()], document);
    assert_eq!(output(&kept), "\\flmUDmymacro{Hi}{Albert}%%\n");
    assert!(
        style(&kept)
            .0
            .contains("\\providecommand\\flmUDmymacro[2]{\\textbf{#1}, \\emph{#2}}")
    );
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_macro_default_arg.
#[test]
fn recompose_macro_default_arg_expanded_and_kept() {
    let document = "\\mymacro{Albert}";
    let expanded = recomposed(vec![always(mymacro())], document);
    assert_eq!(output(&expanded), "\\textbf{Hello}, \\emph{Albert}%%\n");
    let kept = recomposed(vec![mymacro()], document);
    assert_eq!(output(&kept), "\\flmUDmymacro{Hello}{Albert}%%\n");
    assert!(style(&kept).0.contains("\\providecommand\\flmUDmymacro[2]"));
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_math_mode.
#[test]
fn recompose_math_mode_expanded_and_kept() {
    let document = "Test \\(\\calN(\\rho)\\)";
    let expanded = recomposed(vec![always(caln())], document);
    assert_eq!(output(&expanded), "Test \\(\\mathcal{N}(\\rho)\\)%%\n");
    // The kept form keeps the call inside mathematics and defines it in the
    // style file, where the Python recomposer writes the expansion.
    let kept = recomposed(vec![caln()], document);
    assert_eq!(output(&kept), "Test \\(\\flmUDcalN(\\rho)\\)%%\n");
    assert!(
        style(&kept).0.contains("\\providecommand\\flmUDcalN{\\mathcal{N}}"),
        "{}",
        style(&kept).0
    );
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_environment.
#[test]
fn recompose_environment_expanded_and_kept() {
    let document = "Test \\begin{myenv}Hello.\\end{myenv}";
    let expanded = recomposed(vec![always(myenv())], document);
    assert_eq!(output(&expanded), "Test [[[Hello.]]]%%\n");
    let kept = recomposed(vec![myenv()], document);
    assert_eq!(output(&kept), "Test \\begin{flmUEmyenv}Hello.\\end{flmUEmyenv}%%\n");
    assert!(
        style(&kept).0.contains("\\ProvideDocumentEnvironment{flmUEmyenv}{}{[[[}{]]]}"),
        "{}",
        style(&kept).0
    );
    assert_golden("userdefs-environment.tex", &kept);
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_specials.
#[test]
fn recompose_specials_expanded_and_kept() {
    // A specials definition is never recomposable, so both policies write
    // the expansion.
    let document = "user@domain";
    let expanded = recomposed(vec![always(at())], document);
    assert_eq!(output(&expanded), "user[at]domain%%\n");
    let kept = recomposed(vec![at()], document);
    assert_eq!(output(&kept), "user[at]domain%%\n");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_ifnovaluetf_not_provided.
#[test]
fn recompose_ifnovaluetf_not_provided_expanded_and_kept() {
    // A declared default is bound before the conditional runs, so the
    // value branch substitutes the default; the text agrees with Python's by
    // coincidence. The kept plan recomposes the definition: *no value* asked
    // of an argument that has a declared default is answered when the style
    // file is written — the default is always there — so the definition's body
    // is the value branch and nothing is tested at LaTeX run time.
    let document = "\\mymacro{Albert}";
    let expanded = recomposed(vec![always(mymacro_conditional())], document);
    assert_eq!(output(&expanded), "Hello, \\emph{Albert}%%\n");
    let kept = recomposed(vec![mymacro_conditional()], document);
    assert_eq!(output(&kept), "\\flmUDmymacro{Hello}{Albert}%%\n");
    assert!(
        style(&kept)
            .0
            .contains("\\providecommand\\flmUDmymacro[2]{#1, \\emph{#2}}"),
        "{}",
        style(&kept).0
    );
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRecomposer::test_recompose_ifnovaluetf_provided.
#[test]
fn recompose_ifnovaluetf_provided_expanded_and_kept() {
    let document = "\\mymacro[Howdy]{Albert}";
    let expanded = recomposed(vec![always(mymacro_conditional())], document);
    assert_eq!(output(&expanded), "Howdy, \\emph{Albert}%%\n");
    let kept = recomposed(vec![mymacro_conditional()], document);
    assert_eq!(output(&kept), "\\flmUDmymacro{Howdy}{Albert}%%\n");
}

// --- the same bytes under the three execution settings ---------------------------------------

#[test]
fn the_recomposed_latex_is_the_same_bytes_under_every_execution_setting() {
    let registry = registry_over(vec![hello(), note(), version()]);
    let texts = ["\\hello{a} \\begin{note}\\hello{b}\\end{note} \\version", "\\hello{c}"];
    let doc = common::pipeline::doc(&registry, &texts);
    let shared = plan(&registry, WORKFLOW, &Overrides::none());
    let sequential =
        common::pipeline::run_with(&doc, &shared, Externals::none(), &RunOptions::sequential());
    let parallel =
        common::pipeline::run_with(&doc, &shared, Externals::none(), &RunOptions::default());
    assert_eq!(body(&sequential), body(&parallel));
    assert_eq!(style(&sequential), style(&parallel));
}


#[test]
fn text_and_comments_in_mathematics_built_at_lowering_are_written_by_their_rules() {
    // A mathematics-mode template holding `\text{…}` and a comment is written
    // into the style file, and a definition that is not recomposable is
    // written as its expansion in mathematics: in both, the writer builds the
    // nodes it writes (a template's copy, a use site's expansion), and the
    // text command and the comment in them are written by their own rules.
    let defs = vec![
        at(),
        Definition::macro_("tm", Vec::new()).math("\\mathcal{N}\\text{ab} %% c\n x"),
        Definition::macro_("tp", vec![ArgDef::mandatory("x")]).math("\\text{a #1 b}\\alpha"),
        Definition::macro_("tn", Vec::new()).math("\\mathcal{N}\\text{a@b} %% c\n x"),
    ];
    let registry = registry_over(defs);
    let run = recompose(&registry, &["and \\(\\tm + \\tp{\\beta} + \\tn\\) and"]);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_eq!(
        output(&run),
        "and \\(\\flmUDtm + \\flmUDtp{\\beta} + \\mathcal{N}\\text{a[at]b} %% c\n x\\) and%%\n"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\providecommand\\flmUDtm{\\mathcal{N}\\text{ab} %% c\n x}\n"), "{style}");
    assert!(style.contains("\\providecommand\\flmUDtp[1]{\\text{a #1 b}\\alpha}\n"), "{style}");
}

// --- a one-text definition in force in formulas alone --------------------------------------

/// `\ket{…}`, declared by one text with no mode named, whose template holds
/// commands of formulas alone: it is in force in formulas only.
fn formula_ket() -> Definition {
    Definition::macro_("ket", vec![ArgDef::mandatory("a")]).content(r"\lvert{#1}\rangle")
}

#[test]
fn a_kept_one_text_definition_in_force_in_formulas_alone_is_written_as_one_definition() {
    // The definition is kept: it is recomposable, defined once in the style
    // file from its formula template, and every use is written as a call of
    // it. A use in running text is written as a call too, and nothing is
    // reported there by this backend, which expands nothing: the definition's
    // content is LaTeX's to check, as for every kept definition. The
    // document check, which expands every use, reports it.
    let registry = registry_over(vec![formula_ket()]);
    assert_eq!(build_diagnostics(&registry), Vec::<String>::new());
    assert_eq!(not_recomposable(&registry), Vec::new());
    let text = "say \\ket{x} and \\(\\ket{y}\\)\n";
    let run = recompose(&registry, &[text]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(
        output(&run).starts_with(r"say \flmUDket{x} and \(\flmUDket{y}\)"),
        "{}",
        output(&run)
    );
    let (style, _) = style(&run);
    assert!(style.contains(r"\providecommand\flmUDket[1]{\lvert{#1}\rangle}"), "{style}");
    let check = run_workflow(&registry, CHECK_WORKFLOW, &[text], &Overrides::none());
    assert_eq!(identifiers(&check), ["flm.userdefs.bad-definition"]);
}
