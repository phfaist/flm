//! Requirement identifiers resolved per backend: a feature's style identifier
//! that a general rule requires reaches the HTML definitions table, is
//! recorded and resolved by nothing on the text backend, and on the
//! `latex-rendered` backend brings in every definition piece that delivers it
//! (`LatexPreambleDef::delivers`); an identifier that only a stylesheet
//! declares writes nothing there, and one known nowhere is an Error.

mod common;

use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmFeature, FlmRegistry, StyleDef};
use flm_core::core::run::Run;
use flm_core::dump::diag::expect;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ElementDef, SyntaxDef};
use flm_core::model::addr::TreeId;
use flm_core::model::element::ElementSpec;
use flm_core::model::names::Name;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::{self, DEFINITIONS_TABLE};
use flm_core::stdcore::backends::latex::{LatexPreambleDef, preamble};
use flm_core::stdcore::backends::latex_rendered::{self, DOCUMENT, style_artifact};
use flm_core::stdcore::backends::lowering::lowered_table;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::{quote, theorems};
use techy::core::node::NodeRef;

/// A theorem, which the theorems feature's general rule lowers.
const THEOREM: &str = "\\begin{theorem}\\noproofref The square root of two is irrational.\\end{theorem}";

/// A quotation, which the quote feature's general rule lowers.
const QUOTE: &str = "\\begin{quote}\\text{To be or not to be.}\\end{quote}";

/// The condition this backend reports for an identifier known nowhere.
const UNKNOWN: &str = "flm.latex-rendered.unknown-requirement";

// --- helpers -------------------------------------------------------------------------------

/// Runs the workflow `workflow` over `text` with `registry`.
fn render(registry: &Arc<FlmRegistry>, workflow: &str, text: &str) -> Run {
    common::stdcore::run_workflow(registry, workflow, &[text], &Overrides::none())
}

/// Asserts that `run` reported nothing.
fn no_diagnostics(run: &Run) {
    assert_eq!(run.diagnostics().iter().count(), 0, "{}", run.diagnostics().render_all());
}

/// The style file of a `latex-rendered` run.
fn style(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
        panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let style = style_artifact(set).expect("the style file");
    String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The rule of the test element `\treq`: it requires the identifiers it was
/// made with and writes nothing.
#[derive(Debug)]
struct RequireRule(Vec<&'static str>);

impl Lower for RequireRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        for id in &self.0 {
            lw.require(&Name::from(*id));
        }
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// The test feature `t`: the element `\treq`, which requires `required` on
/// every backend; the stylesheet `t.style`, declared for the HTML page
/// alone; and the definition pieces `pieces`.
fn t_feature(required: Vec<&'static str>, pieces: Vec<LatexPreambleDef>) -> Arc<dyn FlmFeature> {
    let required = Arc::new(required);
    let pieces = Arc::new(pieces);
    common::pipeline::feature("t", move |b| {
        b.element(ElementDef::new(
            "t",
            ["treq"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(ElementSpec::new("t.treq").docs("a test element requiring identifiers")),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "t.treq.lower.default",
            "t.treq",
            LoweringDecl::new(),
            Arc::new(RequireRule(required.to_vec())),
            DefaultMark::Everywhere,
        ))?;
        b.style_def(StyleDef {
            id: Name::from("t.style"),
            content: vec![DefContent::Inline(String::from("/* t.style */\n"))],
            docs: String::from("a stylesheet of the HTML page alone"),
        })?;
        for piece in pieces.iter() {
            b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece.clone()))?;
        }
        Ok(())
    })
}

/// The standard package with the test feature `t`.
fn registry_with(required: Vec<&'static str>, pieces: Vec<LatexPreambleDef>) -> Arc<FlmRegistry> {
    let mut features = common::stdcore::package_features_but(StdcoreConfig::new(), "");
    features.push(t_feature(required, pieces));
    common::stdcore::registry_over(features)
}

/// The standard package under its default configuration.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

// --- a general rule's requirement on each backend ------------------------------------------

#[test]
fn a_theorem_s_requirement_reaches_the_html_definitions_table() {
    let run = render(&registry(), html::WORKFLOW, THEOREM);
    let ids: Vec<String> = run
        .table(DEFINITIONS_TABLE)
        .expect("declared")
        .expect("written")
        .definitions
        .iter()
        .map(|row| row.id.to_string())
        .collect();
    assert!(ids.iter().any(|id| id == theorems::html::STYLE), "{ids:?}");
    // A page with no theorem does not carry the stylesheet.
    let plain = render(&registry(), html::WORKFLOW, "Plain text.");
    let definitions = plain.table(DEFINITIONS_TABLE).expect("declared").expect("written");
    assert!(definitions.definitions.iter().all(|row| row.id.as_str() != theorems::html::STYLE));
}

#[test]
fn the_text_backend_records_the_requirement_and_resolves_nothing() {
    let run = render(&registry(), text::WORKFLOW, THEOREM);
    no_diagnostics(&run);
    let lowered = run
        .table(lowered_table(&text::BACKEND.into()))
        .expect("declared")
        .expect("written");
    let tree = lowered.get(TreeId::tree(0)).expect("lowered");
    assert!(tree.requires.contains(&Name::from(theorems::html::STYLE)), "{:?}", tree.requires);
    let printed = common::lowering::text(&run);
    assert!(!printed.contains("flm.theorems.style") && !printed.contains("@layer"), "{printed}");
    assert!(printed.contains("The square root of two is irrational."), "{printed}");
}

#[test]
fn a_theorem_brings_in_the_piece_delivering_the_theorems_style() {
    let piece = LatexPreambleDef::new("t.theorems", "% the test piece for theorems\n", "a test piece")
        .delivering([theorems::html::STYLE]);
    let registry = registry_with(Vec::new(), vec![piece]);
    let run = render(&registry, latex_rendered::WORKFLOW, THEOREM);
    no_diagnostics(&run);
    assert!(style(&run).contains("% the test piece for theorems\n"), "{}", style(&run));
    // A document with no theorem does not hold the piece.
    let plain = render(&registry, latex_rendered::WORKFLOW, "Plain text.");
    assert!(!style(&plain).contains("% the test piece for theorems"), "{}", style(&plain));
}

// --- the resolver's outcomes ----------------------------------------------------------------

#[test]
fn a_piece_delivering_two_identifiers_is_written_once_for_either_or_both() {
    let piece = LatexPreambleDef::new("t.both", "% the test piece for two styles\n", "a test piece")
        .delivering([theorems::html::STYLE, quote::html::STYLE]);
    assert_eq!(
        piece.delivers,
        [Name::from("flm.theorems.style"), Name::from("flm.quote.style")]
    );
    let registry = registry_with(Vec::new(), vec![piece]);
    for text in [THEOREM, QUOTE, &format!("{THEOREM}\n\n{QUOTE}")] {
        let run = render(&registry, latex_rendered::WORKFLOW, text);
        no_diagnostics(&run);
        assert_eq!(style(&run).matches("% the test piece for two styles\n").count(), 1, "{}", style(&run));
    }
}

#[test]
fn two_pieces_delivering_one_identifier_are_both_written() {
    let first = LatexPreambleDef::new("t.first", "% the first test piece\n", "a test piece")
        .delivering([quote::html::STYLE]);
    let second = LatexPreambleDef::new("t.second", "% the second test piece\n", "a test piece")
        .delivering([quote::html::STYLE]);
    let registry = registry_with(Vec::new(), vec![first, second]);
    let run = render(&registry, latex_rendered::WORKFLOW, QUOTE);
    no_diagnostics(&run);
    let written = style(&run);
    let first = written.find("% the first test piece\n").unwrap_or_else(|| panic!("{written}"));
    let second = written.find("% the second test piece\n").unwrap_or_else(|| panic!("{written}"));
    // Both, once each, in the order the registry holds them.
    assert!(first < second, "{written}");
    assert_eq!(written.matches("% the first test piece\n").count(), 1, "{written}");
    assert_eq!(written.matches("% the second test piece\n").count(), 1, "{written}");
}

#[test]
fn an_identifier_only_a_stylesheet_declares_writes_nothing_and_reports_nothing() {
    let registry = registry_with(vec!["t.style"], Vec::new());
    let run = render(&registry, latex_rendered::WORKFLOW, "A \\treq{} b");
    no_diagnostics(&run);
    let plain = render(&registry, latex_rendered::WORKFLOW, "A  b");
    assert_eq!(style(&run), style(&plain));
}

#[test]
fn an_identifier_known_nowhere_is_an_error_naming_it() {
    let registry = registry_with(vec!["t.nowhere"], Vec::new());
    let run = render(&registry, latex_rendered::WORKFLOW, "A \\treq{} b");
    expect(run.diagnostics().iter(), UNKNOWN).count(1);
    let message = run
        .diagnostics()
        .iter()
        .find(|report| report.identifier() == UNKNOWN)
        .expect("reported")
        .message();
    assert_eq!(
        message,
        "the requirement `t.nowhere` names no LaTeX package request, no definition piece, and no \
         style or script definition"
    );
}

#[test]
fn the_default_piece_delivers_nothing() {
    let piece = LatexPreambleDef::new("t.plain", "% plain\n", "a test piece");
    assert!(piece.delivers.is_empty());
    for chunk in latex_rendered::chunks() {
        assert!(chunk.delivers.is_empty(), "{}", chunk.id);
    }
}
