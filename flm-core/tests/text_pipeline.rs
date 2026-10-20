//! The text backend through a plan of the `text` workflow
//! (`flm_core::stdcore::backends::text`): the stages the workflow schedules,
//! the output table and its one-artifact set, the render-bound width, rendering
//! twice, marks mapping to lowered nodes, several trees in document order, an
//! accepted hole in the text, an unattributed callable's spelling, the output
//! table's projection, and a small document as a text golden.

mod common;

use techy::serialize::SerialValue;

use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::core::artifact::ArtifactId;
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::core::plan::Overrides;
use flm_core::core::data::table::{ProjectCx, Table};
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_text_golden;
use flm_core::stdcore::backends::lowering::{LoweredAddr, lowered_table};
use flm_core::stdcore::backends::pres;
use flm_core::stdcore::backends::text::{TEXT_DOCUMENT, TextRenderOptions};

use common::layout::assert_well_formed;
use common::lowering::{
    diagnostics, golden_path, lowered, output, registry, render, render_with, text, text_plan,
    toy_registry,
};
use common::macro_def;

fn width(columns: u32) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(TextRenderOptions { width: Some(columns), ..TextRenderOptions::default() }),
        ..Overrides::none()
    }
}

fn first() -> TreeId {
    TreeId::tree(0)
}

#[test]
fn the_text_workflow_schedules_structure_lowering_emit_and_final_in_order() {
    let plan = text_plan(&toy_registry());
    // The paragraph rule's one requirement (`flm.blocks.structured`) pulls in
    // the whole rewrite chain: the structure stage requires `flm.userdefs`,
    // whose marked default is the expansion stage, ordered before it. The
    // tree-contract check the workflow demands requires the `always`
    // expansion, which heads the chain.
    let rewrites: Vec<String> = plan.rewrite_names().iter().map(|name| name.to_string()).collect();
    assert_eq!(rewrites, ["flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]);
    let names: Vec<String> = plan.stage_names().iter().map(|name| name.to_string()).collect();
    let position = |name: &str| {
        names.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} in {names:?}"))
    };
    assert!(position("flm.text.lower") < position("flm.text.emit"));
    assert!(position("flm.text.emit") < position("flm.text.final"));
}

#[test]
fn the_workflow_writes_the_output_and_the_one_artifact_set() {
    let run = render(&toy_registry(), &["Hello world."]);
    assert_eq!(text(&run), "Hello world.\n");
    let set = run.table(TEXT_DOCUMENT).expect("declared").expect("written");
    assert_eq!(set.len(), 1);
    let entry = set.entry().expect("the entry artifact");
    assert_eq!(entry.id, ArtifactId::new("text", "document"));
    assert_eq!(entry.media_type, "text/plain");
    assert_eq!(entry.as_bytes(), Some("Hello world.\n".as_bytes()));
}

#[test]
fn the_width_is_a_render_bound_option_defaulting_to_eighty() {
    let long = "word ".repeat(30);
    let registry = toy_registry();
    let default = text(&render(&registry, &[&long]));
    assert!(default.lines().all(|line| line.len() <= 80));
    assert_eq!(default.lines().count(), 2);
    let narrow = text(&render_with(&registry, &[&long], &width(20)));
    assert!(narrow.lines().all(|line| line.len() <= 20));
    assert_eq!(narrow.lines().count(), 8);
}

#[test]
fn rendering_twice_is_identical() {
    let registry = toy_registry();
    let doc = "\\section{A}\n\nSome \\emph{text} here.";
    let first_run = render(&registry, &[doc]);
    let second_run = render(&registry, &[doc]);
    // The whole output, marks included: a mark's address is by position in
    // the lowered tree, not by a node id carrying its tree's identity.
    assert_eq!(output(&first_run), output(&second_run));
}

#[test]
fn marks_map_to_the_lowered_nodes() {
    let run = render(&toy_registry(), &["A \\emph{word} here."]);
    let lowered = lowered(&run, first());
    let span = lowered
        .tree
        .iter_storage_order()
        .find(|node| element_of(node).name == pres::SPAN)
        .expect("a span");
    let out = output(&run);
    let marks: Vec<_> = out.marks_at(LoweredAddr::new(first(), span.id())).collect();
    assert_eq!(marks.len(), 1);
    assert_eq!(out.slice(marks[0]), "word");
    // And back: the address resolves in the lowered-trees table.
    let trees = run.table(lowered_table(&"text".into())).unwrap().expect("lowered");
    let resolved = marks[0].addr.resolve(trees).expect("the mark's node");
    assert_eq!(element_of(&resolved).name, pres::SPAN);
}

#[test]
fn trees_print_in_document_order_one_blank_line_apart() {
    let run = render(&toy_registry(), &["one", "two"]);
    assert_eq!(text(&run), "one\n\ntwo\n");
}

#[test]
fn an_accepted_hole_prints_as_an_unsupported_note_with_one_error() {
    let registry = registry(vec![macro_def("t", "bar", 0)], Vec::new());
    let overrides = Overrides {
        unsupported: [("text".into(), vec!["t.bar".into()])].into_iter().collect(),
        ..Overrides::none()
    };
    // (`\bar{}`: the parser gobbles the space after a bare control word.)
    let run = render_with(&registry, &["see \\bar{} here"], &overrides);
    assert_eq!(text(&run), "see [unsupported: t.bar] here\n");
    expect(diagnostics(&run), "flm.lowering.unsupported-element").once();
}

/// An unattributed environment prints its begin and end lines around its body:
/// document content, not error marking.
#[test]
fn an_unknown_environment_prints_its_begin_and_end_lines_around_its_body() {
    let run = render(&toy_registry(), &["\\begin{nope}shown\\end{nope}"]);
    assert_eq!(text(&run), "\\begin{nope}\nshown\n\\end{nope}\n");
    assert!(run.diagnostics().has_errors());
}

/// A blank line inside the body structures into paragraphs, and the parse's own
/// diagnostic stays the only one (no cascade).
#[test]
fn an_unknown_environments_body_structures_into_paragraphs_with_one_diagnostic() {
    let run = render(&toy_registry(), &["\\begin{nope}p1\n\np2\\end{nope}"]);
    assert_eq!(text(&run), "\\begin{nope}\np1\n\np2\n\\end{nope}\n");
    assert_eq!(diagnostics(&run).len(), 1);
}

/// An unknown environment between text is a block of its own; the parse's
/// diagnostic stays the only one.
#[test]
fn an_unknown_environment_between_text_is_a_block_of_its_own() {
    let run = render(&toy_registry(), &["a \\begin{nope}x\\end{nope} b"]);
    assert_eq!(text(&run), "a\n\n\\begin{nope}\nx\n\\end{nope}\n\nb\n");
    assert_eq!(diagnostics(&run).len(), 1);
}

/// An unattributed macro prints its source spelling, inline.
#[test]
fn an_unknown_macro_prints_its_source_spelling_inline() {
    let run = render(&toy_registry(), &["a \\nope{} b"]);
    assert_eq!(text(&run), "a \\nope b\n");
    assert_eq!(diagnostics(&run).len(), 1);
}

struct Refs;

impl ProjectCx for Refs {
    fn node_ref(&self, addr: NodeAddr) -> SerialValue {
        SerialValue::Str(format!("{}/{}", addr.tree, addr.node.index()))
    }

    fn tree_ref(&self, id: TreeId) -> SerialValue {
        SerialValue::Str(id.to_string())
    }
}

#[test]
fn the_output_table_projects_its_text_and_marks() {
    let run = render(&toy_registry(), &["Hi.\n\nThere."]);
    let projected = output(&run).project(&Refs);
    let SerialValue::Map(entries) = projected else { panic!("a map, not {projected:?}") };
    assert_eq!(entries[0].0, "text");
    assert_eq!(entries[0].1, SerialValue::Str("Hi.\n\nThere.\n".into()));
    assert_eq!(entries[1].0, "marks");
    let SerialValue::List(marks) = &entries[1].1 else { panic!("a list") };
    assert_eq!(marks.len(), 2, "one mark per paragraph");
    let SerialValue::Map(mark) = &marks[1] else { panic!("a map") };
    let keys: Vec<&str> = mark.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["addr", "bytes", "start", "end"]);
    assert_eq!(mark[2].1, SerialValue::Str("2:0".into()));
}

#[test]
fn a_small_document_renders_as_the_golden_shows() {
    let doc = "\\section{Introduction}\n\n\
               See \\emph{this emphasized bit} here in a paragraph that is long enough \
               to wrap at forty columns, with \\(x_1\\) inline and \\verb|a{b| too.\n\n\
               \\runin{Theorem} Every finite thing is finite, and this sentence is long \
               enough to wrap somewhere. \\qed\n\n\
               \\begin{quote}\nQuoted \\marginnote{aside} text that is long enough to wrap \
               in the block.\n\\end{quote}\n\n\
               \\begin{code}\nlet x = 1;\nlet y = 2;\n\\end{code}\n\n\
               \\begin{equation}\n  a = b\n\\end{equation}";
    let run = render_with(&toy_registry(), &[doc], &width(40));
    assert!(!run.diagnostics().has_errors(), "{}", run.diagnostics().render_all());
    let text = text(&run);
    assert_well_formed(&text);
    assert!(text.lines().all(|line| line.chars().count() <= 40), "{text}");
    assert_text_golden(golden_path("text-document.txt"), &text);
}
