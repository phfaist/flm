//! The `floats` feature on the text workflow: the configuration and its
//! refusals, the environments and their fields, the numbering policy, the
//! reference point every float declares and what a reference to it shows,
//! the content handlers, the staged shape and the line under the content, the
//! text output, the registries without the references, the numbering or the
//! floats feature, and documents that must neither panic nor loop.
//!
//! The images a `figure` admits by default are the graphics feature's; the
//! floats here hold paragraphs, under types admitting any content.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder,
    RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{Flm, FlmGroupType};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::floats::{
    self, CONTENT_SLOT, FloatContentHandler, FloatNumbering, FloatType, FloatsConfig,
};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::core::node::NodeRef;
use techy::error::Severity;
use techy::serialize::SerialValue;

use common::lowering::{lowered_projection, text as printed};
use common::stdcore::{identifiers, package_features_but, registry_over, reports, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries, runs and the shape of a lowered tree
// ---------------------------------------------------------------------------------------------

/// The two shipped types, each admitting any content, and the type `diagram`
/// admitting any content, all three numbered (every float unless
/// `\nonumber`), so that every float of a test has a number to show.
fn any_config() -> FloatsConfig {
    let numbered = FloatNumbering::Numbered;
    FloatsConfig::new()
        .float_type(FloatType::figure().handlers(["any"]).numbering(numbered))
        .float_type(FloatType::table().handlers(["any"]).numbering(numbered))
        .float_type(FloatType::new("diagram", ["any"]).numbering(numbered))
}

/// The standard package with the floats feature under `config`.
fn with_config(config: FloatsConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().floats(config))
}

/// The standard package under [`any_config`].
fn std() -> Arc<FlmRegistry> {
    with_config(any_config())
}

/// The standard package under [`any_config`] without the feature `left_out`.
fn without(left_out: &str) -> Arc<FlmRegistry> {
    registry_over(package_features_but(StdcoreConfig::new().floats(any_config()), left_out))
}

/// Runs the `text` workflow of `registry` over `document`.
fn render(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// The text `document` renders to under `registry`, its trailing newlines
/// dropped.
fn text_with(registry: &Arc<FlmRegistry>, document: &str) -> String {
    printed(&render(registry, document)).trim_end().to_string()
}

/// A lowered tree's projection written compactly, one node per term: a
/// presentation node as its name after `flm.pres.` with its payload's role,
/// kinds and anchor (after `#`) in braces and its body in parentheses; text as
/// a quoted string; any other node as its element's name.
fn shape(value: &SerialValue) -> String {
    let field = |value: &SerialValue, key: &str| -> Option<SerialValue> {
        match value {
            SerialValue::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()),
            _ => None,
        }
    };
    let string = |value: Option<SerialValue>| match value {
        Some(SerialValue::Str(s)) => Some(s),
        _ => None,
    };
    match value {
        SerialValue::List(items) => items.iter().map(shape).collect::<Vec<_>>().join(" "),
        _ => {
            if let Some(chars) = string(field(value, "chars")) {
                return format!("{chars:?}");
            }
            let element = string(field(value, "elem")).unwrap_or_default();
            let name = element.strip_prefix("flm.pres.").unwrap_or(&element).to_string();
            let mut facts = Vec::new();
            if let Some(payload) = field(value, "payload") {
                if let Some(role) = string(field(&payload, "role")) {
                    facts.push(role);
                }
                if let Some(SerialValue::List(kinds)) = field(&payload, "kinds") {
                    facts.extend(kinds.iter().filter_map(|kind| match kind {
                        SerialValue::Str(kind) => Some(kind.clone()),
                        _ => None,
                    }));
                }
                if let Some(anchor) = string(field(&payload, "anchor")) {
                    facts.push(format!("#{anchor}"));
                }
            }
            let facts = if facts.is_empty() { String::new() } else { format!("{{{}}}", facts.join(" ")) };
            let body = match field(value, "body") {
                Some(SerialValue::List(items)) if items.is_empty() => String::new(),
                Some(body) => format!("({})", shape(&body)),
                None => String::new(),
            };
            format!("{name}{facts}{body}")
        }
    }
}

/// The compact shape of the lowered tree of `document` under `registry`,
/// and the identifiers of what the run reported.
fn lowered(registry: &Arc<FlmRegistry>, document: &str) -> (String, Vec<String>) {
    let run = render(registry, document);
    (shape(&lowered_projection(&run)), identifiers(&run))
}

/// [`lowered`] under [`std`], asserting that nothing is reported.
fn clean(document: &str) -> String {
    let (shape, reported) = lowered(&std(), document);
    assert_eq!(reported, Vec::<String>::new(), "{document}");
    shape
}

// ---------------------------------------------------------------------------------------------
// the configuration (P10; test 15)
// ---------------------------------------------------------------------------------------------

/// What the registry build under `config` reported, rendered; a build that
/// succeeds is a test failure.
fn build_error(config: FloatsConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().floats(config)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

#[test]
fn the_package_lists_floats_after_annotations_and_before_userdefs() {
    let names: Vec<String> = package(StdcoreConfig::new())
        .features
        .iter()
        .map(|feature| feature.info().name.to_string())
        .collect();
    let at = |name: &str| names.iter().position(|known| known == name).expect(name);
    assert!(at("annotations") < at(floats::FEATURE));
    assert_eq!(at(floats::FEATURE) + 1, at("cells"));
    assert_eq!(at("cells") + 3, at("userdefs"));
    assert_eq!(names.len(), 33);
}

#[test]
fn a_configuration_the_feature_refuses_names_the_culprit() {
    let refused = build_error(FloatsConfig::new().float_type(FloatType::new("my-float", ["any"])));
    assert!(refused.contains("`my-float`") && refused.contains("must"), "{refused}");
    let refused = build_error(FloatsConfig::new().float_type(FloatType::new("numbered", ["any"])));
    assert!(refused.contains("may not be named `numbered`"), "{refused}");
    let refused = build_error(FloatsConfig::new().float_type(FloatType::new("x", Vec::<String>::new())));
    assert!(refused.contains("`x`") && refused.contains("must list at least one"), "{refused}");
    let twice = FloatsConfig { types: vec![FloatType::figure(), FloatType::figure()] };
    assert!(build_error(twice).contains("may not be declared twice"));
}

#[test]
fn the_configuration_enters_the_digest() {
    let digest = |config: FloatsConfig| with_config(config).digest().to_string();
    assert_ne!(digest(FloatsConfig::new()), digest(any_config()));
    assert_eq!(digest(any_config()), digest(any_config()));
}

#[test]
fn no_float_type_declares_nothing_and_still_builds() {
    let (_, reported) = lowered(&with_config(FloatsConfig::empty()), "\\begin{figure}x\\end{figure}");
    assert_eq!(reported, ["latexlike.environments.unknown-environment"]);
}

// ---------------------------------------------------------------------------------------------
// the staged shape, the numbering policy and the line under the content (P8, P9; test 10)
// ---------------------------------------------------------------------------------------------

/// The four floats of test 10: bare, captioned, labeled, labeled and
/// captioned; the caption written before the content in the last.
const FOUR: &str = "\\begin{figure}A.\\end{figure}\n\n\
                    \\begin{figure}B.\\caption{A photo}\\end{figure}\n\n\
                    \\begin{figure}\\label{figure:c}C.\\end{figure}\n\n\
                    \\begin{figure}\\caption{Last}\\label{figure:d}D.\\end{figure}";

#[test]
fn under_numbered_every_float_is_numbered_and_staged_with_the_line_under_its_content() {
    assert_eq!(
        clean(FOUR),
        concat!(
            r#"block{flm.floats.float figure numbered #figure-1}("#,
            r#"block{flm.floats.float.content}(par("A.")) "#,
            r#"block{flm.floats.float.caption figure numbered}(par(span{flm.floats.float.number}("Figure\u{a0}" "1")))) "#,
            r#"block{flm.floats.float figure numbered #figure-2}("#,
            r#"block{flm.floats.float.content}(par("B.")) "#,
            r#"block{flm.floats.float.caption figure numbered}(par(span{flm.floats.float.number}("Figure\u{a0}" "2") ": " "A photo"))) "#,
            r#"block{flm.floats.float figure numbered #figure-3}("#,
            r#"block{flm.floats.float.content}(par("C.")) "#,
            r#"block{flm.floats.float.caption figure numbered}(par(span{flm.floats.float.number}("Figure\u{a0}" "3")))) "#,
            r#"block{flm.floats.float figure numbered #figure-4}("#,
            r#"block{flm.floats.float.content}(par("D.")) "#,
            r#"block{flm.floats.float.caption figure numbered}(par(span{flm.floats.float.number}("Figure\u{a0}" "4") ": " "Last")))"#,
        )
    );
}

#[test]
fn a_nonumber_leaves_the_caption_alone_or_nothing_under_the_content() {
    assert_eq!(
        clean(
            "\\begin{figure}\\nonumber A.\\end{figure}\n\n\
             \\begin{figure}\\nonumber B.\\caption{A photo}\\end{figure}\n\n\
             \\begin{figure}\\nonumber\\label{figure:c}C.\\end{figure}"
        ),
        concat!(
            r#"block{flm.floats.float figure unnumbered}(block{flm.floats.float.content}(par("A."))) "#,
            r#"block{flm.floats.float figure unnumbered}(block{flm.floats.float.content}(par("B.")) "#,
            r#"block{flm.floats.float.caption figure unnumbered}(par("A photo"))) "#,
            r#"block{flm.floats.float figure unnumbered #figure-c}(block{flm.floats.float.content}(par("C.")))"#,
        )
    );
}

/// The kind tokens of every float the lowered tree of `document` holds, in
/// order, under `registry`, and what the run reported.
fn float_kinds(registry: &Arc<FlmRegistry>, document: &str) -> (Vec<String>, Vec<(String, Severity)>) {
    let run = render(registry, document);
    let shape = shape(&lowered_projection(&run));
    let kinds = shape
        .split("block{flm.floats.float ")
        .skip(1)
        .map(|rest| rest.split(['}', ' ']).take(2).collect::<Vec<_>>().join(" "))
        .collect();
    (kinds, reports(&run))
}

#[test]
fn under_auto_numbered_only_the_labeled_floats_are_numbered() {
    // The policy of a type that sets none, the shipped `figure` included.
    let registry = with_config(any_config().float_type(FloatType::figure().handlers(["any"])));
    let (kinds, reported) = float_kinds(&registry, FOUR);
    assert_eq!(kinds, ["figure unnumbered", "figure unnumbered", "figure numbered", "figure numbered"]);
    assert_eq!(reported, []);
    assert!(text_with(&registry, FOUR).contains("Figure\u{a0}2: Last"));
    // A `\nonumber` in a float with no label changes nothing, and one in a
    // labeled float unnumbers it.
    let (kinds, reported) = float_kinds(
        &registry,
        "\\begin{figure}\\nonumber A.\\end{figure}\\begin{figure}\\nonumber\\label{figure:b}B.\\end{figure}",
    );
    assert_eq!(kinds, ["figure unnumbered", "figure unnumbered"]);
    assert_eq!(reported, [("flm.floats.nonumber-without-effect".to_string(), Severity::Note)]);
}

#[test]
fn under_unnumbered_no_float_is_numbered_and_a_nonumber_is_a_note() {
    let registry = with_config(any_config().float_type(
        FloatType::figure().handlers(["any"]).numbering(FloatNumbering::Unnumbered),
    ));
    let (kinds, reported) = float_kinds(&registry, FOUR);
    assert_eq!(kinds, ["figure unnumbered"; 4]);
    assert_eq!(reported, []);
    let run = render(&registry, "\\begin{figure}\\nonumber A.\\end{figure}");
    assert_eq!(reports(&run), [("flm.floats.nonumber-without-effect".to_string(), Severity::Note)]);
    let message = run.diagnostics().iter().next().expect("one").to_string();
    assert!(message.contains("never numbered"), "{message}");
}

#[test]
fn a_second_nonumber_is_a_note() {
    let run = render(&std(), "\\begin{figure}\\nonumber\\nonumber A.\\end{figure}");
    assert_eq!(reports(&run), [("flm.floats.nonumber-without-effect".to_string(), Severity::Note)]);
}

#[test]
fn a_float_counts_in_its_own_type_s_counter() {
    let shape = clean("\\begin{table}T.\\end{table}\\begin{figure}F.\\end{figure}\\begin{table}U.\\end{table}");
    assert!(shape.contains("#table-1") && shape.contains("#figure-1") && shape.contains("#table-2"), "{shape}");
    assert!(shape.contains(r#"span{flm.floats.float.number}("Table\u{a0}" "2")"#), "{shape}");
}

// ---------------------------------------------------------------------------------------------
// references (P8; test 11)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reference_to_a_numbered_float_shows_the_type_s_words_and_its_number() {
    let registry = std();
    let text = text_with(
        &registry,
        "\\begin{figure}\\label{figure:a}A.\\end{figure}\\begin{figure}\\label{figure:b}B.\\end{figure}\
         \\begin{table}\\label{table:t}T.\\end{table}\n\n\
         See \\ref{figure:a}, \\ref[S]{figure:a}, \\ref{figure:a,figure:b} and \\ref{table:t}.",
    );
    // A float style joins several numbers compactly: two numbers read
    // `Figs.~1,2`, never `Figs.~1 and 2`.
    assert!(
        text.ends_with("See Fig.\u{a0}1, Figure\u{a0}1, Figs.\u{a0}1,2 and Tab.\u{a0}1."),
        "{text}"
    );
    assert_eq!(identifiers(&render(&registry, "\\begin{figure}\\label{figure:a}A.\\end{figure}\\ref{figure:a}")), Vec::<String>::new());
}

#[test]
fn a_configured_type_takes_its_words_from_its_name() {
    let text = text_with(
        &std(),
        "\\begin{diagram}\\label{diagram:x}X.\\caption{Flow}\\end{diagram}\n\nSee \\ref{diagram:x}.",
    );
    assert_eq!(text, "X.\n\nDiagram\u{a0}1: Flow\n\nSee Diagram\u{a0}1.");
}

#[test]
fn a_reference_to_an_unnumbered_float_shows_its_alternative_text_else_its_caption() {
    let registry = std();
    let text = text_with(
        &registry,
        "\\begin{figure}\\nonumber\\label{figure:a}A.\\caption{A long caption}\\end{figure}\n\n\
         \\begin{figure}\\nonumber\\label{figure:b}B.\\caption[alt={Short, one}]{A longer caption}\\end{figure}\n\n\
         \\begin{figure}\\nonumber\\label{figure:c}C.\\caption[alt=Short \\emph{two}]{C caption}\\end{figure}\n\n\
         See \\ref{figure:a}; \\ref{figure:b}; \\ref{figure:c}.",
    );
    assert!(text.ends_with("See A long caption; Short, one; Short two."), "{text}");
    assert!(text.contains("A longer caption"), "{text}");
}

#[test]
fn a_reference_to_an_unnumbered_float_with_no_caption_has_nothing_to_show() {
    let run = render(&std(), "\\begin{figure}\\nonumber\\label{figure:a}A.\\end{figure}\n\nSee \\ref{figure:a}.");
    assert_eq!(reports(&run), [("flm.refs.nothing-to-show".to_string(), Severity::Error)]);
    assert!(printed(&run).contains("See figure:a."));
    let message = run.diagnostics().iter().next().expect("one").to_string();
    assert!(message.contains("`figure:a`") && message.contains("neither"), "{message}");
}

#[test]
fn a_reference_to_an_equation_nothing_numbers_has_nothing_to_show() {
    let registry = without("numbering");
    let run = render(&registry, "\\begin{equation}a\\label{eq:x}\\end{equation}\n\nSee \\ref{eq:x}.");
    assert_eq!(identifiers(&run), ["flm.refs.nothing-to-show"]);
    assert!(printed(&run).contains("See eq:x."));
}

// ---------------------------------------------------------------------------------------------
// the label types, and a label without the references feature (P7, P15; test 12)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_label_of_a_type_the_float_does_not_accept_is_refused_unless_configured() {
    let (_, reported) = lowered(&std(), "\\begin{figure}\\label{fig:x}A.\\end{figure}");
    assert_eq!(reported, ["flm.refs.label-type-not-accepted"]);
    let registry = with_config(any_config().float_type(
        FloatType::figure().handlers(["any"]).label_types(["figure", "fig"]),
    ));
    let text = text_with(&registry, "\\begin{figure}\\label{fig:x}A.\\end{figure}\n\nSee \\ref{fig:x}.");
    assert!(text.ends_with("See Fig.\u{a0}1."), "{text}");
}

#[test]
fn without_the_references_feature_a_label_is_accepted_draws_a_note_and_still_numbers() {
    let registry = registry_over(package_features_but(
        StdcoreConfig::new().floats(any_config().float_type(
            FloatType::figure().handlers(["any"]).numbering(FloatNumbering::AutoNumbered),
        )),
        "refs",
    ));
    let (shape, reported) = lowered(&registry, "\\begin{figure}\\label{figure:x}A.\\caption{C}\\end{figure}");
    assert_eq!(reported, ["flm.floats.label-without-refs"]);
    assert!(shape.starts_with("block{flm.floats.float figure numbered}("), "{shape}");
    assert_eq!(text_with(&registry, "\\begin{figure}\\label{figure:x}A.\\caption{C}\\end{figure}"), "A.\n\nFigure\u{a0}1: C");
}

// ---------------------------------------------------------------------------------------------
// the content handlers (P7; test 13)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_type_admitting_any_content_admits_paragraphs_and_blocks() {
    let shape = clean("\\begin{figure}One.\n\nTwo.\\begin{itemize}\\item x\\end{itemize}\\end{figure}");
    assert!(shape.starts_with(r#"block{flm.floats.float figure numbered #figure-1}(block{flm.floats.float.content}(par("One.") par("Two.") list"#), "{shape}");
}

#[test]
fn content_no_handler_admits_is_an_error_and_is_shown() {
    // The default `figure` admits exactly one image, through a handler the
    // graphics feature contributes; text is none.
    let registry = with_config(FloatsConfig::new());
    let run = render(&registry, "\\begin{figure}Some text.\\caption{C}\\end{figure}");
    let errors: Vec<String> = reports(&run)
        .into_iter()
        .filter(|(_, severity)| *severity == Severity::Error)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(errors, ["flm.floats.content-refused"]);
    assert_eq!(printed(&run).trim_end(), "Some text.\n\nC");
    let message = run
        .diagnostics()
        .iter()
        .find(|d| d.identifier() == "flm.floats.content-refused")
        .expect("reported")
        .to_string();
    // The graphics feature contributes the handler the figure names: the
    // condition is stated in its words. A registry without that feature
    // states it without them (`tests/floats_images.rs`).
    assert!(message.contains("a ‘figure’ must hold exactly one image"), "{message}");
    assert!(!message.contains("includegraphics"), "{message}");
}

#[test]
fn a_handler_nobody_contributes_is_reported_once_per_run() {
    let registry = with_config(FloatsConfig::new().float_type(FloatType::table().handlers(["charts", "any"])));
    let run = render(&registry, "\\begin{table}A.\\end{table}\\begin{table}B.\\end{table}");
    assert_eq!(reports(&run), [("flm.floats.missing-handler".to_string(), Severity::Note)]);
    let message = run.diagnostics().iter().next().expect("one").to_string();
    assert!(
        message.contains("a ‘table’ may hold content of the kind ‘charts’, but the program processing this document does not handle that kind of content"),
        "{message}"
    );
    // A type nobody uses is not reported.
    assert_eq!(identifiers(&render(&registry, "Text.")), Vec::<String>::new());
    // The `check` workflow reports it too.
    let checked = run_workflow(&registry, flm_core::stdcore::CHECK_WORKFLOW, &["\\begin{table}A.\\end{table}"], &Overrides::none());
    assert_eq!(reports(&checked), [("flm.floats.missing-handler".to_string(), Severity::Note)]);
}

/// A handler of the test feature: exactly one paragraph.
struct OneParagraph;

impl FloatContentHandler for OneParagraph {
    fn name(&self) -> &str {
        "one-paragraph"
    }

    fn admit(&self, content: &[NodeRef<'_, Flm>]) -> Result<(), String> {
        match content {
            [one] if one.group_type() == Some(FlmGroupType::Paragraph) => Ok(()),
            _ => Err("exactly one paragraph".to_string()),
        }
    }
}

/// A feature bringing a float type of its own, `listing`, and the handler
/// its type admits.
struct Listings;

impl FlmFeature for Listings {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.listings", "Listings")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.peer_slot::<dyn FloatContentHandler>(CONTENT_SLOT, Arc::new(OneParagraph))?;
        floats::contribute_type(b, Arc::new(FloatType::new("listing", ["one-paragraph"])))
    }
}

/// The standard package under [`any_config`] with the feature [`Listings`].
fn with_listings(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
    let mut all = features;
    all.push(Arc::new(Listings));
    registry_over(all)
}

#[test]
fn a_feature_contributes_a_type_and_a_handler_of_its_own() {
    let registry = with_listings(package_features_but(StdcoreConfig::new().floats(any_config()), ""));
    let text = text_with(&registry, "\\begin{listing}\\label{listing:a}Code.\\caption{A}\\end{listing}\n\nSee \\ref{listing:a}.");
    assert_eq!(text, "Code.\n\nListing\u{a0}1: A\n\nSee Listing\u{a0}1.");
    assert_eq!(identifiers(&render(&registry, "\\begin{listing}Code.\\end{listing}")), Vec::<String>::new());
    let run = render(&registry, "\\begin{listing}One.\n\nTwo.\\end{listing}");
    assert_eq!(identifiers(&run), ["flm.floats.content-refused"]);
    let message = run.diagnostics().iter().next().expect("one").to_string();
    assert!(message.contains("a ‘listing’ must hold exactly one paragraph"), "{message}");
}

#[test]
fn a_feature_contributing_a_type_renders_without_the_floats_feature() {
    use flm_core::stdcore::backends::{html, latex_recomposed};
    let registry = with_listings(package_features_but(StdcoreConfig::new(), floats::FEATURE));
    // The registry builds, with a Note for each peer declaration nobody
    // resolves: the environment among them, dropped.
    let notes: Vec<&str> = registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    assert!(notes.iter().all(|id| *id == "flm.registry.unresolved-peer"), "{notes:?}");
    let messages: Vec<String> = registry.build_diagnostics().iter().map(|d| d.to_string()).collect();
    assert!(
        messages.iter().any(|m| m.contains("refers to the element type `flm.floats.float`")),
        "{messages:?}"
    );
    let values: Vec<(String, bool)> = registry
        .peer_element_values()
        .iter()
        .map(|entry| (entry.names.iter().map(ToString::to_string).collect(), entry.resolved))
        .collect();
    assert_eq!(values, [("listing".to_string(), false)]);
    // Every rendering plan builds, and a plain document renders.
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
        let run = run_workflow(&registry, workflow, &["Text."], &Overrides::none());
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{workflow}");
    }
    assert_eq!(text_with(&registry, "Text."), "Text.");
    // The environment is not defined.
    let run = render(&registry, "\\begin{listing}Code.\\end{listing}");
    assert_eq!(identifiers(&run), ["latexlike.environments.unknown-environment"]);
    // With the floats feature, the same declaration is kept.
    let registry = with_listings(package_features_but(StdcoreConfig::new().floats(any_config()), ""));
    let listing = registry.peer_element_values().iter().find(|entry| entry.names[0].as_str() == "listing");
    assert!(listing.is_some_and(|entry| entry.resolved));
}

// ---------------------------------------------------------------------------------------------
// the fields (P7; test 14)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_second_caption_is_an_error_and_the_first_is_kept() {
    let run = render(&std(), "\\begin{figure}A.\\caption{First}\\caption{Second}\\end{figure}");
    assert_eq!(identifiers(&run), ["flm.floats.caption-repeated"]);
    assert_eq!(printed(&run).trim_end(), "A.\n\nFigure\u{a0}1: First");
}

#[test]
fn a_caption_s_bracket_holds_alt_and_nothing_else() {
    let registry = std();
    for (bracket, expected) in [
        ("[Short]", vec!["flm.floats.caption-unknown-key"]),
        ("[alt=A, width=2]", vec!["flm.floats.caption-unknown-key"]),
        ("[alt]", vec!["flm.floats.caption-missing-value"]),
        ("[alt=A, alt=B]", vec!["flm.floats.caption-repeated-key"]),
        ("[alt=A]", vec![]),
        ("[]", vec![]),
    ] {
        let document = format!("\\begin{{figure}}\\nonumber\\label{{figure:a}}X.\\caption{bracket}{{Long}}\\end{{figure}}\\ref{{figure:a}}");
        let run = render(&registry, &document);
        assert_eq!(identifiers(&run), expected, "{bracket}");
    }
    // The first `alt` is kept.
    let text = text_with(&registry, "\\begin{figure}\\nonumber\\label{figure:a}X.\\caption[alt=A, alt=B]{Long}\\end{figure}\n\n\\ref{figure:a}");
    assert!(text.ends_with("\n\nA"), "{text}");
}

#[test]
fn the_fields_outside_a_float_are_unknown_commands() {
    for document in ["\\caption{x}", "\\nonumber", "\\begin{itemize}\\item \\caption{x}\\end{itemize}"] {
        let run = render(&std(), document);
        assert_eq!(identifiers(&run), ["core.specs.unresolvable-command"], "{document}");
    }
}

#[test]
fn a_field_standing_deeper_than_the_body_is_not_the_float_s() {
    let run = render(&std(), "\\begin{figure}\\emph{A \\caption{x}}\\end{figure}");
    assert_eq!(identifiers(&run), ["flm.lowering.field-not-read"]);
}

#[test]
fn the_fields_stand_anywhere_in_the_body_and_a_paragraph_is_shown_without_them() {
    assert_eq!(
        text_with(&std(), "\\begin{figure}Some \\label{figure:a}text\\nonumber{} here.\\caption{C}\n\nMore.\\end{figure}"),
        "Some text here.\n\nMore.\n\nC"
    );
}

// ---------------------------------------------------------------------------------------------
// the text output (P12; test 17)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_output_prints_the_float_s_blocks_one_blank_line_apart() {
    assert_eq!(
        text_with(&std(), FOUR),
        "A.\n\nFigure\u{a0}1\n\nB.\n\nFigure\u{a0}2: A photo\n\nC.\n\nFigure\u{a0}3\n\nD.\n\nFigure\u{a0}4: Last"
    );
}

// ---------------------------------------------------------------------------------------------
// registries without the references, the numbering or the floats feature (P15; test 20)
// ---------------------------------------------------------------------------------------------

#[test]
fn without_the_numbering_feature_a_numbered_float_is_an_error_and_shown_unnumbered() {
    let registry = without("numbering");
    let (shape, reported) = lowered(&registry, FOUR);
    assert_eq!(reported, ["flm.floats.no-numbering"; 4]);
    assert!(!shape.contains(" numbered"), "{shape}");
    assert_eq!(text_with(&registry, FOUR), "A.\n\nB.\n\nA photo\n\nC.\n\nD.\n\nLast");
    // A reference to a captioned float shows the caption; to one with no
    // caption, it has nothing to show.
    let run = render(
        &registry,
        "\\begin{figure}\\label{figure:a}A.\\caption{Cap}\\end{figure}\\begin{figure}\\label{figure:b}B.\\end{figure}\n\n\\ref{figure:a}; \\ref{figure:b}",
    );
    assert_eq!(identifiers(&run), ["flm.floats.no-numbering", "flm.floats.no-numbering", "flm.refs.nothing-to-show"]);
    assert!(printed(&run).trim_end().ends_with("Cap; figure:b"));
    // A float its policy does not number reports nothing.
    assert_eq!(identifiers(&render(&registry, "\\begin{figure}\\nonumber A.\\end{figure}")), Vec::<String>::new());
}

#[test]
fn without_the_references_feature_every_float_renders() {
    let registry = without("refs");
    let (shape, reported) = lowered(&registry, FOUR);
    assert_eq!(reported, ["flm.floats.label-without-refs"; 2]);
    assert!(!shape.contains('#'), "{shape}");
    assert_eq!(text_with(&registry, FOUR), text_with(&std(), FOUR));
}

#[test]
fn without_the_floats_feature_a_float_is_an_unknown_environment() {
    let registry = without(floats::FEATURE);
    let run = render(&registry, "\\begin{figure}A.\\end{figure}");
    assert_eq!(identifiers(&run), ["latexlike.environments.unknown-environment"]);
}

// ---------------------------------------------------------------------------------------------
// execution settings, and documents that must neither panic nor loop (test 21)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_is_the_same_sequentially_and_in_parallel() {
    let registry = std();
    let texts = [FOUR, "\\begin{table}\\label{table:t}T.\\end{table}\\ref{figure:c}", "\\begin{diagram}\\nonumber X\\end{diagram}"];
    let doc = common::pipeline::doc(&registry, &texts);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
}

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let registry = std();
    assert_eq!(
        clean("\\begin{figure}\\caption{}\\end{figure}"),
        r#"block{flm.floats.float figure numbered #figure-1}(block{flm.floats.float.content} block{flm.floats.float.caption figure numbered}(par(span{flm.floats.float.number}("Figure\u{a0}" "1") ": ")))"#
    );
    assert_eq!(
        clean("\\begin{figure}\\nonumber\\end{figure}"),
        r#"block{flm.floats.float figure unnumbered}(block{flm.floats.float.content})"#
    );
    assert_eq!(
        text_with(&registry, "\\begin{figure}\\begin{figure}Inner.\\caption{In}\\end{figure}\\caption{Out}\\end{figure}"),
        // The outer float's point comes first in document order.
        "Inner.\n\nFigure\u{a0}2: In\n\nFigure\u{a0}1: Out"
    );
    for document in [
        "\\begin{itemize}\\item \\begin{figure}X.\\caption{C}\\end{figure}\\end{itemize}",
        "Text.\\footnote{\\begin{figure}X.\\caption{C}\\end{figure}}",
        "\\begin{quote}\\block{\\begin{figure}X.\\caption{C}\\end{figure}}\\end{quote}",
        "Before \\begin{figure}X.\\end{figure} after.",
    ] {
        let run = render(&registry, document);
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{document}");
        assert!(printed(&run).contains("Figure\u{a0}1"), "{document}");
    }
    let mut nested = String::from("x");
    for _ in 0..10 {
        nested = format!("\\begin{{figure}}{nested}\\caption{{c}}\\end{{figure}}");
    }
    let run = render(&registry, &nested);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(printed(&run).contains("Figure\u{a0}10: c"));
    for document in [
        "\\begin{figure}",
        "\\begin{figure}\\caption{",
        "\\begin{figure}\\caption[alt=",
        "\\begin{figure}\\label{",
        "\\begin{figure}\\caption[alt={\\caption{x}}]{y}\\end{figure}",
    ] {
        let run = render(&registry, document);
        let _ = printed(&run);
    }
}

// ---------------------------------------------------------------------------------------------
// the recomposed LaTeX reads a float as every backend does
// ---------------------------------------------------------------------------------------------

#[test]
fn the_recomposed_latex_writes_the_environment_and_reports_as_the_text_does() {
    use flm_core::stdcore::backends::latex_recomposed;
    let registry = std();
    let latex = |document: &str| {
        run_workflow(&registry, latex_recomposed::WORKFLOW, &[document], &Overrides::none())
    };
    let run = latex("\\begin{figure}\\label{figure:a}A.\n\n\\caption{A photo}\\end{figure}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let output = common::latex::output(&run);
    // The hook environment over the type and the case (`tests/floats_latex.rs`).
    assert!(output.contains("\\begin{flmFloat}{figure}{NumCap}A.\n\n"), "{output}");
    assert!(output.contains("\\caption{A photo}\\flmLabels{\\label{ref1}}\\end{flmFloat}"), "{output}");
    for document in [
        "\\begin{figure}A.\\caption{B}\\caption{C}\\caption[x=1]{D}\\nonumber\\nonumber\\end{figure}",
        "\\begin{table}\\label{fig:x}T.\\end{table}",
    ] {
        assert_eq!(identifiers(&latex(document)), identifiers(&render(&registry, document)), "{document}");
    }
}
