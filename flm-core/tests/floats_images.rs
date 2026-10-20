//! An image in a float: the graphics feature's content handler
//! `includegraphics`, which admits exactly one image, on the default
//! `figure` and `table` types; what a float's body holding something else is
//! told; and a registry without the graphics feature, where
//! `\includegraphics` is an unknown command in a figure.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::floats::{self, FloatType, FloatsConfig};
use flm_core::stdcore::features::graphics::{self, IncludeGraphicsContent, handler::HANDLER};
use flm_core::stdcore::features::floats::FloatContentHandler;
use techy::error::Severity;

use common::lowering::text as printed;
use common::stdcore::{
    identifiers, identifiers_but_unknown_size, package_features_but, registry_over, reports,
    reports_but_unknown_size, run_workflow,
};

/// The standard package under its default configuration: `figure` admitting
/// one image, `table` admitting a table of cells or one image.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The standard package with the floats feature under `config`.
fn with_floats(config: FloatsConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().floats(config))
}

/// The standard package under its default configuration without the
/// graphics feature.
fn without_graphics() -> Arc<FlmRegistry> {
    registry_over(package_features_but(StdcoreConfig::new(), graphics::FEATURE))
}

/// Runs `workflow` of `registry` over `document`.
fn render_on(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> Run {
    run_workflow(registry, workflow, &[document], &Overrides::none())
}

/// Runs the text workflow of `registry` over `document`.
fn render(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    render_on(registry, text::WORKFLOW, document)
}

/// The text of `run`, its trailing newlines dropped.
fn text_of(run: &Run) -> String {
    printed(run).trim_end().to_string()
}

/// The message of the one diagnostic of `run` with the identifier `id`.
fn message(run: &Run, id: &str) -> String {
    run.diagnostics()
        .iter()
        .find(|d| d.identifier() == id)
        .unwrap_or_else(|| panic!("`{id}` is reported: {}", run.diagnostics().render_all()))
        .to_string()
}

const IMAGE_FIGURE: &str =
    "\\begin{figure}\n\\includegraphics{plot.png}\n\\caption{A photo}\\label{figure:a}\n\\end{figure}";

// ---------------------------------------------------------------------------------------------
// the handler (test 13, its image cases)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_graphics_feature_contributes_the_handler_the_default_types_name() {
    assert_eq!(IncludeGraphicsContent.name(), HANDLER);
    assert!(FloatType::figure().handlers.iter().any(|name| name == HANDLER));
    assert!(FloatType::table().handlers.iter().any(|name| name == HANDLER));
    // The standard package resolves the contribution: no build diagnostic.
    assert_eq!(std().build_diagnostics().iter().count(), 0);
}

#[test]
fn one_image_in_a_figure_is_admitted_on_every_backend() {
    let registry = std();
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
        let run = render_on(&registry, workflow, IMAGE_FIGURE);
        assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{workflow}: {}", run.diagnostics().render_all());
    }
    // An image in a `table` too, and the fields standing before it: the
    // table's first handler, `cells`, refuses it, and `includegraphics`
    // admits it.
    let run = render(&registry, "\\begin{table}\\caption{T}\\label{table:t}\\includegraphics{t.png}\\end{table}");
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "[image: t.png]\n\nTable\u{a0}1: T");
}

#[test]
fn the_text_of_a_figure_holding_an_image_is_the_image_line_then_the_line_under_it() {
    assert_eq!(text_of(&render(&std(), IMAGE_FIGURE)), "[image: plot.png]\n\nFigure\u{a0}1: A photo");
    assert_eq!(
        text_of(&render(&std(), "\\begin{figure}\\includegraphics[alt=A plot]{plot.png}\\nonumber\\end{figure}")),
        "[image: A plot]"
    );
}

#[test]
fn two_images_in_a_float_admitting_any_content_print_as_two_blocks() {
    let registry = with_floats(FloatsConfig::new().float_type(FloatType::figure().handlers(["any"])));
    let run = render(&registry, "\\begin{figure}\\includegraphics{a.png}\n\n\\includegraphics{b.png}\\caption{C}\\end{figure}");
    assert_eq!(reports_but_unknown_size(&run), []);
    assert_eq!(text_of(&run), "[image: a.png]\n\n[image: b.png]\n\nC");
}

#[test]
fn two_images_in_a_figure_are_refused_and_both_are_shown() {
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
        let run = render_on(
            &std(),
            workflow,
            "\\begin{figure}\\includegraphics{a.png}\n\n\\includegraphics{b.png}\\caption{C}\\end{figure}",
        );
        assert_eq!(reports_but_unknown_size(&run), [("flm.floats.content-refused".to_string(), Severity::Error)], "{workflow}");
        assert!(message(&run, "flm.floats.content-refused").contains("a ‘figure’ must hold exactly one image"));
    }
    let run = render(&std(), "\\begin{figure}\\includegraphics{a.png}\n\n\\includegraphics{b.png}\\caption{C}\\end{figure}");
    let shown = text_of(&run);
    assert!(shown.contains("[image: a.png]") && shown.contains("[image: b.png]"), "{shown}");
    assert!(shown.ends_with("[image: b.png]\n\nC"), "{shown}");
}

#[test]
fn text_in_a_figure_is_refused_and_shown() {
    for document in [
        "\\begin{figure}Some text.\\caption{C}\\end{figure}",
        "\\begin{figure}Some text \\includegraphics{a.png}\\caption{C}\\end{figure}",
        "\\begin{figure}\\caption{C}\\end{figure}",
    ] {
        let run = render(&std(), document);
        assert_eq!(identifiers_but_unknown_size(&run), ["flm.floats.content-refused"], "{document}");
        assert!(message(&run, "flm.floats.content-refused").contains("a ‘figure’ must hold exactly one image"));
    }
    let run = render(&std(), "\\begin{figure}Some text \\includegraphics{a.png}\\caption{C}\\end{figure}");
    assert_eq!(text_of(&run), "Some text\n\n[image: a.png]\n\nC");
}

#[test]
fn an_image_in_a_type_admitting_tables_of_cells_alone_is_refused_and_shown() {
    let registry = with_floats(FloatsConfig::new().float_type(FloatType::table().handlers(["cells"])));
    let run = render(&registry, "\\begin{table}\\includegraphics{t.png}\\caption{T}\\end{table}");
    assert_eq!(reports_but_unknown_size(&run), [("flm.floats.content-refused".to_string(), Severity::Error)]);
    assert!(message(&run, "flm.floats.content-refused").contains("exactly one table of cells"));
    assert_eq!(text_of(&run), "[image: t.png]\n\nT");
}

// ---------------------------------------------------------------------------------------------
// without the graphics feature (test 14, its last clause; test 20, its graphics-absent run)
// ---------------------------------------------------------------------------------------------

#[test]
fn without_the_graphics_feature_an_image_is_an_unknown_command_in_a_figure() {
    let registry = without_graphics();
    // The figure names the scoped package of `\includegraphics`, which nobody
    // declares: a Note when the registry is built, nothing more.
    let notes: Vec<&str> = registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    assert!(notes.iter().all(|id| *id == "flm.registry.unresolved-peer"), "{notes:?}");
    let run = render(&registry, IMAGE_FIGURE);
    assert_eq!(
        reports(&run),
        [
            ("core.specs.unresolvable-command".to_string(), Severity::Error),
            ("flm.floats.missing-handler".to_string(), Severity::Note),
            ("flm.floats.content-refused".to_string(), Severity::Error),
        ]
    );
    // The figure is reported under its handlers: none of them is available.
    assert!(message(&run, "flm.floats.content-refused").contains("the program processing this document handles no such content"));
    assert!(text_of(&run).ends_with("Figure\u{a0}1: A photo"), "{}", text_of(&run));
    // Outside a float, the command is unknown as it is with the feature.
    assert_eq!(identifiers(&render(&registry, "\\includegraphics{x.png}")), ["core.specs.unresolvable-command"]);
}

#[test]
fn without_the_graphics_feature_every_float_holding_no_image_renders_on_every_backend() {
    let any = FloatsConfig::new()
        .float_type(FloatType::figure().handlers(["any"]))
        .float_type(FloatType::table().handlers(["any"]));
    let registry = registry_over(package_features_but(StdcoreConfig::new().floats(any), graphics::FEATURE));
    let document = "\\begin{figure}A.\\end{figure}\n\n\\begin{figure}B.\\caption{A photo}\\end{figure}\n\n\
                    \\begin{figure}\\label{figure:c}C.\\end{figure}\n\n\
                    \\begin{table}\\caption{Last}\\label{table:d}D.\\end{table}\n\n\\ref{figure:c}, \\ref{table:d}";
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
        let run = render_on(&registry, workflow, document);
        assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{workflow}: {}", run.diagnostics().render_all());
    }
    assert_eq!(
        text_of(&render(&registry, document)),
        "A.\n\nB.\n\nA photo\n\nC.\n\nFigure\u{a0}1\n\nD.\n\nTable\u{a0}1: Last\n\nFig.\u{a0}1, Tab.\u{a0}1"
    );
}

#[test]
fn without_the_floats_feature_the_handler_is_dropped_with_a_note() {
    let registry = registry_over(package_features_but(StdcoreConfig::new(), floats::FEATURE));
    let messages: Vec<String> = registry.build_diagnostics().iter().map(|d| d.to_string()).collect();
    assert!(messages.iter().any(|m| m.contains("flm.floats.content")), "{messages:?}");
    // Under `global`, an image written between paragraphs is a paragraph
    // holding it, inline content, as it is with the floats feature.
    let config = StdcoreConfig::new().graphics(graphics::GraphicsConfig::new().global(true));
    let registry = registry_over(package_features_but(config, floats::FEATURE));
    let run = render(&registry, "Before.\n\n\\includegraphics{x.png}\n\nAfter.");
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "Before.\n\n[image: x.png]\n\nAfter.");
}
