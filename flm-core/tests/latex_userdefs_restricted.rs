//! A user definition kept as a LaTeX definition on the `latex-recomposed`
//! backend (`flm_core::stdcore::features::userdefs::kept`) whose template
//! holds a label, a reference or an equation reference: a template that
//! declares a reference point is refused with the `userdefs` feature's typed
//! Error, at each use site, naming the remedy, and renders once the definition is marked
//! to be expanded in every plan; a reference whose target holds a placeholder
//! has every use site of its definition expanded, which then writes and
//! reports exactly what the definition marked `always` does — a target naming
//! nothing reported and written under a name of the use site's value, on this
//! backend and through `instantiate-at-lowering` alike, and again wherever the
//! content holding the use site is shown; a fixed target keeps its safe label,
//! and one naming nothing is reported at each use site; a placeholder in a list's
//! own label template expands the use sites the same way; and a template lowering
//! discarded for an expansion leaves no diagnostic and no package request behind.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{LoweringSelection, Overrides};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::mint::{ArgDef, ArgStdKind};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::backends::latex_recomposed::{BACKEND, WORKFLOW};
use flm_core::stdcore::features::userdefs::{UserdefsConfig, instantiate_rule_name};
use flm_core::stdcore::StdcoreConfig;
use techy::error::Severity;

use common::latex::{assert_golden, output, style, style_if_any};
use common::stdcore::{identifiers, reports, run_workflow};

/// The identifier of the typed Error.
const DECLARES_POINT: &str = "flm.userdefs.kept-template-declares-point";

fn registry_over(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

fn recompose(registry: &Arc<FlmRegistry>, text: &str) -> Run {
    run_workflow(registry, WORKFLOW, &[text], &Overrides::none())
}

/// The style file's line defining the user definition `name`.
fn definition_line(run: &Run, name: &str) -> String {
    let (style, _) = style(run);
    let prefix = format!("\\providecommand\\flmUD{name}");
    style
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no definition of {name} in:\n{style}"))
        .to_string()
}

// --- a template that declares a reference point --------------------------------------------

fn titled(expand: ExpandPolicy) -> Definition {
    Definition::macro_("titled", vec![ArgDef::mandatory("title")])
        .text("\\section{#1}\\label{sec:fixed}")
        .expand(expand)
}

fn keyed(expand: ExpandPolicy) -> Definition {
    Definition::macro_("keyed", vec![ArgDef::mandatory("key")])
        .text("\\section{T}\\label{sec:#1}")
        .expand(expand)
}

#[test]
fn a_label_in_a_kept_template_is_refused_at_each_use_site_with_its_remedy() {
    let registry = registry_over(vec![titled(ExpandPolicy::PerPlan)]);
    let run = recompose(&registry, "\\titled{A}\n\n\\titled{B}\n\nText.");
    assert_eq!(
        reports(&run),
        [(String::from(DECLARES_POINT), Severity::Error), (String::from(DECLARES_POINT), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    let message = run.diagnostics().render_all();
    assert!(message.contains("`\\titled`"), "{message}");
    assert!(message.contains("`\\section`"), "{message}");
    assert!(message.contains("expanded in the document itself"), "{message}");
    assert!(message.contains("expansion policy `always`"), "{message}");
    assert!(!message.contains("instantiate"), "{message}");
    // The recovery is as it was: each use site lowers to the stand-in of a
    // failed rule, a comment, and nothing of the definition is written.
    let body = output(&run);
    assert_eq!(body.matches("% flm: unsupported element flm.userdefs.macro.titled").count(), 2, "{body}");
    assert!(!body.contains("flmUDtitled"), "{body}");
}

#[test]
fn the_same_definition_marked_always_renders() {
    let registry = registry_over(vec![titled(ExpandPolicy::Always)]);
    let run = recompose(&registry, "\\titled{A}\n\nSee \\ref{sec:fixed}.");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    assert!(body.contains("\\section{A}{\\flmLabels{\\label{ref1}}}"), "{body}");
    assert!(body.contains("See \\flmRef{ref1}."), "{body}");
}

#[test]
fn a_label_completed_by_a_placeholder_is_refused_too() {
    let registry = registry_over(vec![keyed(ExpandPolicy::PerPlan)]);
    let run = recompose(&registry, "\\keyed{a}");
    assert_eq!(identifiers(&run), [DECLARES_POINT], "{}", run.diagnostics().render_all());
    assert!(run.diagnostics().render_all().contains("(`\\label`)"));
    // Marked `always`, the label is the use site's.
    let registry = registry_over(vec![keyed(ExpandPolicy::Always)]);
    let run = recompose(&registry, "\\keyed{a}\n\nSee \\ref{sec:a}.");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(output(&run).contains("See \\flmRef{ref1}."), "{}", output(&run));
}

#[test]
fn instantiating_the_definition_at_lowering_is_no_remedy() {
    let definition = titled(ExpandPolicy::PerPlan);
    let registry = registry_over(vec![definition.clone()]);
    let selection = LoweringSelection::new().prefer(instantiate_rule_name(&definition).as_str());
    let overrides =
        Overrides { lowering: [(BACKEND.into(), selection)].into_iter().collect(), ..Overrides::none() };
    let run = run_workflow(&registry, WORKFLOW, &["\\titled{A} and \\titled{B}"], &overrides);
    assert_eq!(identifiers(&run), [DECLARES_POINT, DECLARES_POINT], "{}", run.diagnostics().render_all());
    // The message holds for an instance as for a definition written into the
    // style file: it names no LaTeX definition.
    let message = run.diagnostics().render_all();
    assert!(message.contains("declares a labeled place"), "{message}");
    assert!(!message.contains("LaTeX definition"), "{message}");
}

#[test]
fn a_labeled_xnote_in_a_kept_template_is_refused_by_the_same_error() {
    let registry = registry_over(vec![
        Definition::macro_("noted", Vec::new()).text("text\\footnote[\\label{fn:k}]{n}"),
    ]);
    let run = recompose(&registry, "\\noted{} and \\noted{}.");
    assert_eq!(identifiers(&run), [DECLARES_POINT, DECLARES_POINT], "{}", run.diagnostics().render_all());
}

// --- a reference whose target holds a placeholder ------------------------------------------

/// The recomposition of `text` with `definitions` kept (the default policy,
/// `per-plan`), in which nothing of the kept definitions reaches the style
/// file: a placeholder in a reference target expands every use site.
fn expanded(definitions: &[Definition], text: &str) -> Run {
    let kept = recompose(&registry_over(definitions.to_vec()), text);
    assert!(
        style_if_any(&kept).is_none_or(|(style, _)| !style.contains("\\flmUD")),
        "{}",
        style(&kept).0
    );
    kept
}

/// [`expanded`], checked against the same definitions marked `always`: the two
/// write the same LaTeX and report the same conditions.
fn expanded_as_always(definitions: &[Definition], text: &str) -> Run {
    let kept = expanded(definitions, text);
    let always: Vec<Definition> =
        definitions.iter().cloned().map(|definition| definition.expand(ExpandPolicy::Always)).collect();
    let expanded = recompose(&registry_over(always), text);
    assert_eq!(output(&kept), output(&expanded));
    assert_eq!(reports(&kept), reports(&expanded), "{}", kept.diagnostics().render_all());
    kept
}

/// The identifier of an unresolved reference.
const UNRESOLVED: &str = "flm.refs.unresolved";

/// The identifier of a target not spelled `type:label`.
const BAD_LABEL: &str = "flm.refs.bad-label";

/// [`expanded`], checked against the same definitions marked `always` for what
/// the run reports — the same identifiers and messages, compared sorted, not
/// the places — where the written LaTeX differs in the shape of a name only:
/// a target naming nothing takes a name of its use site's value (`ref-v1`)
/// where it is kept and a document name (`ref2`) where it is marked `always`.
///
/// The reports are the same only for a template whose arguments hold no
/// reference: an argument's reference is reported where the document holds
/// it and again in each copy the use site's value holds of it where the
/// definition is kept, and once per copy under `always`, where the copies are
/// expanded into the document.
fn same_reports_as_always(definitions: &[Definition], text: &str) -> Run {
    let kept = expanded(definitions, text);
    let always: Vec<Definition> =
        definitions.iter().cloned().map(|definition| definition.expand(ExpandPolicy::Always)).collect();
    let expanded = recompose(&registry_over(always), text);
    let rendered = |run: &Run| {
        let mut all: Vec<String> =
            run.diagnostics().iter().map(|d| format!("{} {}", d.identifier(), d.message())).collect();
        all.sort();
        all
    };
    assert_eq!(rendered(&kept), rendered(&expanded), "{}", kept.diagnostics().render_all());
    kept
}

/// The three reference commands, each with a placeholder for its whole target.
fn whole_targets() -> Vec<Definition> {
    vec![
        Definition::macro_("myref", vec![ArgDef::mandatory("key")]).text("(see \\ref{#1})"),
        Definition::macro_("mylink", vec![ArgDef::mandatory("key")])
            .text("\\hyperref[#1]{that section}"),
        Definition::macro_("myeq", vec![ArgDef::mandatory("key")]).text("\\eqref{#1}"),
    ]
}

#[test]
fn a_reference_to_a_placeholder_expands_every_use_site() {
    // A label's safe name is computed from its characters, which a call
    // supplies: each use site is expanded, and each reference is written around
    // the safe name of the label that use site names, with nothing reported.
    let run = expanded_as_always(
        &whole_targets(),
        "\\section{A}\\label{sec:a}\n\n\\begin{equation}\n  a = b \\label{eq:b}\n\\end{equation}\n\n\
         \\myref{sec:a} \\mylink{sec:a} \\myeq{eq:b}\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    assert!(
        body.contains("(see \\flmRef{ref1}) \\flmHyperref{ref1}{that section} \\eqref{ref2}"),
        "{body}"
    );
    assert_golden("userdefs-refs-placeholder.tex", &run);
}

#[test]
fn the_characters_around_a_placeholder_are_the_use_sites_target() {
    // `sec:#1` is completed at each use site into the label it names, which
    // is written under its safe name, as a reference written out is.
    let tagged = Definition::macro_("tagged", vec![ArgDef::mandatory("key")]).text("\\ref{sec:#1}");
    let run = expanded_as_always(&[tagged], "\\section{A}\\label{sec:a}\n\n\\tagged{a}");
    assert!(output(&run).contains("\\flmRef{ref1}"), "{}", output(&run));
    // A target completed into a label nothing carries is reported, and
    // written under the name its use site's value gives it.
    let tagged =
        Definition::macro_("tagged", vec![ArgDef::mandatory("key")]).text("\\ref{my_type:#1}");
    let run = same_reports_as_always(&[tagged], "\\tagged{a}");
    assert_eq!(output(&run), "\\flmRef{ref-v1}%%\n");
    assert_eq!(reports(&run), [(String::from(UNRESOLVED), Severity::Error)]);
}

#[test]
fn a_placeholder_in_a_heading_option_value_expands_the_use_site() {
    // Whether the heading is numbered is read from the value's characters,
    // which a call supplies: the use site is expanded.
    let heading = Definition::macro_("numbered", vec![ArgDef::mandatory("flag")])
        .text("\\section[numbered=#1]{T}");
    let run = expanded_as_always(&[heading], "\\numbered{false}\n\n\\numbered{true}");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_fixed_target_in_a_kept_template_keeps_its_safe_label() {
    let registry = registry_over(vec![
        Definition::macro_("seeintro", Vec::new()).text("(see \\ref{sec:intro})"),
        Definition::macro_("linkintro", vec![ArgDef::mandatory("text")])
            .text("\\hyperref[sec:intro]{#1}"),
    ]);
    let run = recompose(
        &registry,
        "\\section{Intro}\\label{sec:intro}\n\n\\seeintro{} and \\linkintro{there}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(
        definition_line(&run, "seeintro"),
        "\\providecommand\\flmUDseeintro{(see \\flmRef{ref1})}"
    );
    assert_eq!(
        definition_line(&run, "linkintro"),
        "\\providecommand\\flmUDlinkintro[1]{\\flmHyperref{ref1}{#1}}"
    );
}

#[test]
fn a_placeholder_target_naming_nothing_is_reported_and_written_under_a_value_name() {
    // Each use site is expanded; its target names nothing. The value of each
    // use site names it `ref-v1` (a name of that value's own, numbered from one
    // in each value), and the run reports it as the definition marked `always`
    // does: a target not spelled `type:label` is a bad label, a well-formed one
    // naming nothing is unresolved, once per use site.
    let run = same_reports_as_always(
        &whole_targets(),
        "\\myref{nolabel} \\mylink{nolabel} \\myeq{eq:none} \\myref{sec:none}",
    );
    assert_eq!(
        output(&run),
        "(see \\flmRef{ref-v1}) \\flmHyperref{ref-v1}{that section} \\eqref{ref-v1} (see \
          \\flmRef{ref-v1})%%\n"
    );
    assert_eq!(
        reports(&run),
        [
            (String::from(BAD_LABEL), Severity::Error),
            (String::from(BAD_LABEL), Severity::Error),
            (String::from(UNRESOLVED), Severity::Error),
            (String::from(UNRESOLVED), Severity::Error),
        ],
        "{}",
        run.diagnostics().render_all()
    );
    assert_golden("userdefs-refs-placeholder-unresolved.tex", &run);
}

#[test]
fn a_target_a_verbatim_argument_completes_is_written_under_a_name() {
    // The characters of a verbatim argument never reach the key: `x%y` and
    // `u\v` are written under a name, and the output compiles (the compile
    // suite compiles the golden).
    let shape = ArgStdKind::Verbatim { delimiters: Some(('{', '}')) };
    let vref = Definition::macro_("vref", vec![ArgDef::named("key", shape)]).text("(see \\ref{#1})");
    let run = same_reports_as_always(&[vref], "\\vref{x%y} \\vref{u\\v} \\vref{sec:a_b}");
    assert_eq!(
        output(&run),
        "(see \\flmRef{ref-v1}) (see \\flmRef{ref-v1}) (see \\flmRef{ref-v1})%%\n"
    );
    assert_eq!(
        identifiers(&run),
        [BAD_LABEL, BAD_LABEL, UNRESOLVED],
        "{}",
        run.diagnostics().render_all()
    );
    assert_golden("userdefs-refs-placeholder-verbatim.tex", &run);
}

#[test]
fn a_value_names_each_target_once_in_the_order_it_first_mentions_it() {
    // One use site naming two targets that name nothing, the first twice: the
    // value gives them `ref-v1` and `ref-v2`, and the first keeps its name.
    // A target the document mentions too is written under the document's
    // name.
    let three = Definition::macro_(
        "three",
        vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")],
    )
    .text("\\ref{#1} \\ref{#2} \\ref{#1} \\ref{sec:elsewhere}");
    let run = same_reports_as_always(&[three], "\\ref{sec:elsewhere}\n\n\\three{sec:x}{sec:y}");
    assert_eq!(
        output(&run),
        "\\flmRef{ref1}\n\n\\flmRef{ref-v1} \\flmRef{ref-v2} \\flmRef{ref-v1} \\flmRef{ref1}%%\n"
    );
}

#[test]
fn a_reference_an_argument_carries_into_the_value_is_reported_there_too() {
    // The second argument is document content, copied into the value of the
    // expanded use site: its reference is reported where the document was
    // resolved, and again over the copy. The template's own reference is the
    // value's, and is reported.
    let two = Definition::macro_("two", vec![ArgDef::mandatory("key"), ArgDef::mandatory("text")])
        .text("\\ref{#1} [#2]");
    let run = expanded(&[two], "\\two{sec:x}{see \\ref{sec:y}}");
    let mut found: Vec<String> =
        run.diagnostics().iter().map(|d| format!("{} {}", d.identifier(), d.message())).collect();
    found.sort();
    assert_eq!(
        found,
        [
            "flm.refs.unresolved the reference points at the label `sec:x`, which is not defined",
            "flm.refs.unresolved the reference points at the label `sec:y`, which is not defined",
            "flm.refs.unresolved the reference points at the label `sec:y`, which is not defined",
        ],
        "{}",
        run.diagnostics().render_all()
    );
}

#[test]
fn a_term_link_to_a_placeholder_naming_no_term_is_reported_and_named() {
    let linkto = Definition::macro_("linkto", vec![ArgDef::mandatory("term")])
        .text("\\term[#1]{this term}");
    let document = "\\begin{defterm}{Entropy}\nA measure.\n\\end{defterm}\n\n\\linkto{Entropy} \\linkto{Enthalpy}\n";
    let run = same_reports_as_always(&[linkto], document);
    assert!(
        output(&run).contains("\\flmTerm{ref1}{this term} \\flmTerm{ref-v1}{this term}"),
        "{}",
        output(&run)
    );
    assert_eq!(identifiers(&run), [UNRESOLVED], "{}", run.diagnostics().render_all());
    assert_golden("userdefs-refs-placeholder-terms.tex", &run);
}

#[test]
fn instantiating_at_lowering_names_and_reports_the_same_way() {
    // The `instantiate-at-lowering` rule splices the same value as the
    // expansion of a use site `emit-call` makes, and the same catch-up names
    // and reports its references.
    let definitions = whole_targets();
    let registry = registry_over(definitions.clone());
    let mut selection = LoweringSelection::new();
    for definition in &definitions {
        selection = selection.prefer(instantiate_rule_name(definition).as_str());
    }
    let overrides =
        Overrides { lowering: [(BACKEND.into(), selection)].into_iter().collect(), ..Overrides::none() };
    let text = "\\section{A}\\label{sec:a}\n\n\\myref{sec:a} \\myref{nolabel} \\myeq{eq:none}";
    let run = run_workflow(&registry, WORKFLOW, &[text], &overrides);
    assert!(
        output(&run).contains("(see \\flmRef{ref1}) (see \\flmRef{ref-v1}) \\eqref{ref-v1}"),
        "{}",
        output(&run)
    );
    assert_eq!(identifiers(&run), [BAD_LABEL, UNRESOLVED], "{}", run.diagnostics().render_all());
}

#[test]
fn instantiating_at_lowering_on_html_reports_what_the_expanded_document_reports() {
    // The same catch-up serves every backend: on `html` an instance reports
    // what the expanded document reports, and a reference its argument
    // carries once more, over the instance's copy of it.
    let definitions = vec![
        Definition::macro_("two", vec![ArgDef::mandatory("key"), ArgDef::mandatory("text")])
            .text("\\ref{#1} [#2]"),
    ];
    let text = "\\two{sec:x}{see \\ref{sec:y}} \\two{nolabel}{z}";
    let mut selection = LoweringSelection::new();
    for definition in &definitions {
        selection = selection.prefer(instantiate_rule_name(definition).as_str());
    }
    let overrides = Overrides {
        lowering: [(html::BACKEND.into(), selection)].into_iter().collect(),
        ..Overrides::none()
    };
    let instantiated =
        run_workflow(&registry_over(definitions.clone()), html::WORKFLOW, &[text], &overrides);
    let expanded =
        run_workflow(&registry_over(definitions), html::WORKFLOW, &[text], &Overrides::none());
    let rendered = |run: &Run| {
        let mut all: Vec<String> = run.diagnostics().iter().map(|d| d.message()).collect();
        all.sort();
        all
    };
    let mut with_the_copy = rendered(&expanded);
    with_the_copy.push(String::from("the reference points at the label `sec:y`, which is not defined"));
    with_the_copy.sort();
    assert_eq!(rendered(&instantiated), with_the_copy, "{}", instantiated.diagnostics().render_all());
    assert_eq!(rendered(&expanded).len(), 3, "{}", expanded.diagnostics().render_all());
}

/// The reports of `flm.refs.unresolved` in `run`.
fn unresolved_count(run: &Run) -> usize {
    identifiers(run).iter().filter(|identifier| identifier.as_str() == UNRESOLVED).count()
}

#[test]
fn a_use_site_in_a_heading_title_is_reported_wherever_the_heading_is_shown() {
    // A reference shows the heading's title, copied out of the document: the
    // use site inside it is instantiated again wherever the title is shown,
    // and its template's reference is reported where the title stands and at
    // each reference to the heading. The recomposed LaTeX shows no copy of
    // the title: LaTeX reuses it.
    let definitions = vec![Definition::macro_("vr", vec![ArgDef::mandatory("key")]).text("\\ref{#1}")];
    let registry = registry_over(definitions.clone());
    let mut selection = LoweringSelection::new();
    for definition in &definitions {
        selection = selection.prefer(instantiate_rule_name(definition).as_str());
    }
    for k in [0, 1, 4] {
        let text = format!("\\section{{T \\vr{{sec:none}}}}\\label{{sec:t}}\n\n{}", "\\ref{sec:t} ".repeat(k));
        for (backend, workflow) in [(html::BACKEND, html::WORKFLOW), (text::BACKEND, text::WORKFLOW)] {
            let overrides = Overrides {
                lowering: [(backend.into(), selection.clone())].into_iter().collect(),
                ..Overrides::none()
            };
            let run = run_workflow(&registry, workflow, &[text.as_str()], &overrides);
            assert_eq!(unresolved_count(&run), 1 + k, "{workflow}, k = {k}: {}", run.diagnostics().render_all());
        }
        let kept = recompose(&registry, &text);
        assert_eq!(unresolved_count(&kept), 1, "kept, k = {k}: {}", kept.diagnostics().render_all());
    }
}

#[test]
fn the_names_and_the_reports_are_the_same_under_every_execution_setting() {
    let registry = registry_over(whole_targets());
    let texts = [
        "\\section{A}\\label{sec:a}\n\n\\myref{sec:a} \\myref{nolabel} \\mylink{sec:b}",
        "\\myeq{eq:none} \\myref{sec:c} \\myref{sec:a}",
    ];
    let doc = common::pipeline::doc(&registry, &texts);
    let shared = common::stdcore::plan(&registry, WORKFLOW, &Overrides::none());
    let sequential =
        common::pipeline::run_with(&doc, &shared, Externals::none(), &RunOptions::sequential());
    let parallel =
        common::pipeline::run_with(&doc, &shared, Externals::none(), &RunOptions::default());
    assert_eq!(common::latex::body(&sequential), common::latex::body(&parallel));
    assert_eq!(reports(&sequential), reports(&parallel));
    assert_eq!(identifiers(&sequential), [BAD_LABEL, UNRESOLVED, UNRESOLVED, UNRESOLVED]);
}

#[test]
fn a_fixed_target_naming_nothing_in_a_kept_template_is_reported_at_each_use_site_and_written_under_a_name() {
    // The template is written into the style file once; its references name
    // nothing, and are written under names of the template's value rather
    // than as their characters. The template is the caller's text for the
    // document, so they are reported as the document's own are — at each use
    // site that stages the definition.
    let registry = registry_over(vec![
        Definition::macro_("seenone", Vec::new()).text("(see \\ref{sec:none} \\ref{nolabel})"),
    ]);
    let run = recompose(&registry, "\\seenone{} and \\seenone{}");
    assert_eq!(
        definition_line(&run, "seenone"),
        "\\providecommand\\flmUDseenone{(see \\flmRef{ref-v1} \\flmRef{ref-v2})}"
    );
    let mut found = identifiers(&run);
    found.sort();
    assert_eq!(found, [BAD_LABEL, BAD_LABEL, UNRESOLVED, UNRESOLVED], "{}", run.diagnostics().render_all());
    let rendered = run.diagnostics().render_all();
    assert!(rendered.contains("`sec:none`") && rendered.contains("`nolabel`"), "{rendered}");
    // Each report is raised inside the copy of the template one use site
    // shows, and names that use site.
    let text = "\\seenone{} and \\seenone{}";
    let at = |offset| vec![(String::from(common::stdcore::SHOWN_HERE), offset)];
    let (first, second) = (at(0), at(text.rfind("\\seenone").expect("a second use site")));
    let mut found = common::stdcore::frames(&run);
    found.sort();
    assert_eq!(found, [first.clone(), first, second.clone(), second], "{rendered}");
}

#[test]
fn hard_targets_in_a_kept_template_neither_panic_nor_loop() {
    let definitions = [
        Definition::macro_("empty", Vec::new()).text("\\ref{}"),
        Definition::macro_("two", vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")])
            .text("\\ref{#1,#2}"),
        Definition::macro_("nested", vec![ArgDef::mandatory("a")]).text("\\ref{x:{#1}}"),
    ];
    let text = "\\section{A}\\label{sec:a}\n\n\\section{B}\\label{sec:b}\n\n\
                \\empty{} \\two{sec:a}{sec:b} \\nested{c}";
    let run = recompose(&registry_over(definitions.to_vec()), text);
    // `\empty` names no placeholder and is kept; the other two are expanded.
    assert!(definition_line(&run, "empty").starts_with("\\providecommand\\flmUDempty{\\flmRef{"));
    let (style, _) = style(&run);
    assert!(!style.contains("\\flmUDtwo") && !style.contains("\\flmUDnested"), "{style}");
    assert!(output(&run).contains("\\flmRef{ref1,ref2}"), "{}", output(&run));
}

// --- what a discarded lowering of a template leaves behind ---------------------------------

/// The lines of the style file of `run` that request a package, in order.
fn package_requests(run: &Run) -> Vec<String> {
    style_if_any(run)
        .map(|(style, _)| {
            style.lines().filter(|line| line.contains("RequirePackage")).map(String::from).collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_discarded_template_lowering_leaves_no_diagnostic_behind() {
    // The template is lowered as a value, reports its unknown key, then meets
    // the address a call supplies and is discarded; each use site is expanded
    // and reports the key once, as the definition marked `always` does.
    let linked = Definition::macro_("linked", vec![ArgDef::mandatory("address")])
        .text("\\section[bogus=1]{T}\\url{#1}");
    let run = expanded_as_always(&[linked], "\\linked{https://a.org}\n\n\\linked{https://b.org}");
    let unknown = identifiers(&run).iter().filter(|id| *id == "flm.heading.unknown-key").count();
    assert_eq!(unknown, 2, "{}", run.diagnostics().render_all());
}

#[test]
fn a_discarded_template_lowering_requests_no_package() {
    // The link in the blank test's first branch is lowered with the template,
    // which is discarded when the verbatim content meets the placeholder: the
    // use site, whose second argument is not blank, writes no link, and the
    // run requests no `hyperref`, as the definition marked `always` does not.
    let link = Definition::macro_("lk", vec![ArgDef::mandatory("code"), ArgDef::mandatory("flag")])
        .text("\\ifblank{#2}{\\href{https://a.org}{x}}{}\\verbcode{#1}");
    let kept = expanded(std::slice::from_ref(&link), "\\lk{a}{b}");
    let always = recompose(&registry_over(vec![link.expand(ExpandPolicy::Always)]), "\\lk{a}{b}");
    assert_eq!(output(&kept), output(&always));
    assert_eq!(package_requests(&kept), package_requests(&always));
    assert!(
        !package_requests(&kept).iter().any(|line| line.contains("{hyperref}")),
        "{:?}",
        package_requests(&kept)
    );
}

// --- a list's own label template ------------------------------------------------------------

#[test]
fn a_placeholder_in_a_lists_label_expands_the_use_site() {
    // A list's label template is compiled from its spelling, which a call
    // completes: each use site is expanded, and its list written with the
    // label the call gives.
    let listed = Definition::macro_("listed", vec![ArgDef::mandatory("mark")])
        .text("\\begin{enumerate}[label={(#1)}]\\item a\\end{enumerate}");
    let run = expanded_as_always(&[listed], "\\listed{Q}");
    assert!(output(&run).contains("\\begin{enumerate}[label={(Q)}]"), "{}", output(&run));
}
