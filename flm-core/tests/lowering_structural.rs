//! The default lowering rules of the structural elements
//! (`flm_core::stdcore::backends::lowering`) over trees the block-structure
//! pass produced: paragraphs to `par`, decorators hoisted into the paragraph's
//! attached slots, math groups and a math environment to `math-passthrough` with
//! their content carried verbatim (the math fallback and a paragraph break
//! inside included), verbatim bodies and arguments to `verbatim` with the
//! inferred display, an unattributed callable lowered transparently, a block
//! environment over its structured body, a whole small document as a
//! projection golden, and a run staged with the whitespace at its edges
//! trimmed (`stage_trimmed`).

mod common;

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};

use flm_core::core::lowering::{DefaultMark, Lowering};
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef, TackOnField};
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::dump::golden::assert_json_golden;
use flm_core::lang::Flm;
use flm_core::stdcore::backends::lowering::{FieldNotRead, stage_trimmed};
use flm_core::stdcore::backends::pres::{
    self, MathDisplay, MathPassthroughPayload, SpanPayload, SpanStyle, UnknownKind, UnknownPayload,
    VerbatimDisplay, VerbatimPayload, attached_slots, payload_of, plain_text,
};

use common::lowering::{golden_path, lowered, lowered_projection, render, toy_registry};

/// The first node of `element` in storage order.
fn find<'t>(tree: &'t NodeTree<Flm>, element: &str) -> Option<NodeRef<'t, Flm>> {
    tree.iter_storage_order().find(|node| element_of(node).name == element)
}

/// The element names of the root's children.
fn top_level(tree: &NodeTree<Flm>) -> Vec<String> {
    tree.root().children().iter().map(|node| element_of(&node).name.to_string()).collect()
}

/// The element names of a node's body, in order.
fn body_names(node: &NodeRef<'_, Flm>) -> Vec<String> {
    node.body()
        .map(|body| body.iter().map(|node| element_of(&node).name.to_string()).collect())
        .unwrap_or_default()
}

/// The element names of every node below a node's body, in document order.
fn body_descendants(node: &NodeRef<'_, Flm>) -> Vec<String> {
    node.body()
        .map(|body| {
            body.iter()
                .flat_map(|node| std::iter::once(node).chain(node.descendants()))
                .map(|node| element_of(&node).name.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn first() -> TreeId {
    TreeId::tree(0)
}

#[test]
fn paragraphs_lower_to_par() {
    let run = render(&toy_registry(), &["Hello world.\n\nSecond one."]);
    let lowered = lowered(&run, first());
    assert_eq!(top_level(&lowered.tree), [pres::PAR, pres::PAR]);
    assert_eq!(lowered.rounds, 1);
}

#[test]
fn decorators_are_hoisted_out_of_the_paragraph() {
    // Two paragraphs: the root run is then a blocks region.
    let run = render(&toy_registry(), &["\\runin{Theorem} Every x. \\qed\n\nNext."]);
    let lowered = lowered(&run, first());
    let par = find(&lowered.tree, pres::PAR).expect("a par");
    assert_eq!(body_names(&par), [pres::DECO_MARK, "flm.text", pres::DECO_MARK]);
    let attached: Vec<Option<&str>> = attached_slots(&par).iter().map(|slot| slot.name).collect();
    assert_eq!(attached, [Some(pres::DECO_RUNIN), Some(pres::DECO_FINAL)]);
    assert_json_golden(golden_path("lowering-decorated.json"), &lowered_projection(&run));
}

#[test]
fn math_groups_lower_to_passthrough_formulas_with_their_content_verbatim() {
    let run = render(&toy_registry(), &["a \\(x \\alpha y\\) b \\[ z \\]"]);
    let lowered = lowered(&run, first());
    let maths: Vec<NodeRef<'_, Flm>> = lowered
        .tree
        .iter_storage_order()
        .filter(|node| element_of(node).name == pres::MATH_PASSTHROUGH)
        .collect();
    assert_eq!(maths.len(), 2);
    let displays: Vec<MathDisplay> =
        maths.iter().map(|node| payload_of::<MathPassthroughPayload>(node).unwrap().display).collect();
    assert_eq!(displays, [MathDisplay::Inline, MathDisplay::Display]);
    // The fallback element inside the inline math is carried, not lowered.
    assert!(body_descendants(&maths[0]).iter().any(|name| name == "flm.math-passthrough.opaque"));
    assert_json_golden(golden_path("lowering-math.json"), &lowered_projection(&run));
}

#[test]
fn a_math_environment_lowers_with_its_environment_name() {
    let run = render(&toy_registry(), &["\\begin{equation} x = 1 \\end{equation}"]);
    let lowered = lowered(&run, first());
    let math = find(&lowered.tree, pres::MATH_PASSTHROUGH).expect("math-passthrough");
    let payload = payload_of::<MathPassthroughPayload>(&math).unwrap();
    assert_eq!(payload.env.as_deref(), Some("equation"));
    assert_eq!(payload.display, MathDisplay::Display);
    assert_eq!(plain_text(math.body().unwrap().iter()), " x = 1 ");
}

#[test]
fn a_parbreak_inside_math_is_preserved() {
    let run = render(&toy_registry(), &["\\(a\n\nb\\)"]);
    let lowered = lowered(&run, first());
    let math = find(&lowered.tree, pres::MATH_PASSTHROUGH).expect("math-passthrough");
    assert!(body_descendants(&math).iter().any(|name| name == "flm.parbreak"));
}

#[test]
fn a_verbatim_environment_body_lowers_to_a_block_verbatim() {
    let run = render(&toy_registry(), &["\\begin{code}\nlet x;\n\\end{code}"]);
    let lowered = lowered(&run, first());
    let verbatim = find(&lowered.tree, pres::VERBATIM).expect("verbatim");
    assert_eq!(payload_of::<VerbatimPayload>(&verbatim).unwrap().display, VerbatimDisplay::Block);
    assert_eq!(plain_text(verbatim.body().unwrap().iter()), "let x;\n");
}

#[test]
fn a_verbatim_argument_lowers_to_an_inline_verbatim() {
    let run = render(&toy_registry(), &["a \\verb|x{y| b"]);
    let lowered = lowered(&run, first());
    let verbatim = find(&lowered.tree, pres::VERBATIM).expect("verbatim");
    assert_eq!(payload_of::<VerbatimPayload>(&verbatim).unwrap().display, VerbatimDisplay::Inline);
    assert_eq!(plain_text(verbatim.body().unwrap().iter()), "x{y");
}

/// An unattributed environment lowers to `flm.pres.unknown` over its lowered
/// body, and an unattributed macro to a childless one carrying its spelling.
#[test]
fn an_unknown_environment_lowers_to_a_presentation_unknown_over_its_body() {
    let run = render(&toy_registry(), &["\\begin{nope}x\\end{nope}"]);
    let lowered = lowered(&run, first());
    let unknown = find(&lowered.tree, pres::UNKNOWN).expect("an unknown node");
    let payload = payload_of::<UnknownPayload>(&unknown).expect("a payload");
    assert_eq!(payload.kind, UnknownKind::Environment);
    assert_eq!(payload.name, "nope");
    assert_eq!(payload.spelling, "nope");
    assert_eq!(body_names(&unknown), [pres::PAR]);
    let texts: Vec<&str> =
        lowered.tree.iter_storage_order().filter_map(|node| node.chars()).collect();
    assert_eq!(texts, ["x"], "its content flows through");
}

#[test]
fn an_unknown_macro_lowers_to_a_childless_presentation_unknown_of_its_spelling() {
    let run = render(&toy_registry(), &["a \\nope b"]);
    let lowered = lowered(&run, first());
    let unknown = find(&lowered.tree, pres::UNKNOWN).expect("an unknown node");
    let payload = payload_of::<UnknownPayload>(&unknown).expect("a payload");
    assert_eq!(payload.kind, UnknownKind::Macro);
    assert_eq!(payload.name, "nope");
    assert_eq!(payload.spelling, "\\nope");
    assert_eq!(unknown.child_count(), 0);
}

#[test]
fn a_block_environment_lowers_over_its_structured_body() {
    let run = render(&toy_registry(), &["\\begin{quote}p1\n\np2\\end{quote}"]);
    let lowered = lowered(&run, first());
    let block = find(&lowered.tree, pres::BLOCK).expect("a block");
    assert_eq!(body_names(&block), [pres::PAR, pres::PAR]);
}

#[test]
fn an_inline_element_lowers_to_a_span_with_its_style() {
    let run = render(&toy_registry(), &["\\emph{x}"]);
    let lowered = lowered(&run, first());
    let span = find(&lowered.tree, pres::SPAN).expect("a span");
    assert_eq!(payload_of::<SpanPayload>(&span).unwrap().style, Some(SpanStyle::Emphasis));
}

#[test]
fn a_small_document_lowers_as_the_golden_shows() {
    let text = "\\section{Introduction}\n\n\
                See \\emph{this} and \\(x_1\\) and \\verb|a{b|.\n\n\
                \\runin{Theorem} Every finite thing is finite. \\qed\n\n\
                \\begin{quote}\nQuoted \\marginnote{aside} text.\n\\end{quote}\n\n\
                \\begin{code}\nlet x = 1;\n\\end{code}\n\n\
                \\begin{equation}\n  a = b\n\\end{equation}";
    let run = render(&toy_registry(), &[text]);
    assert!(
        !run.diagnostics().has_errors(),
        "no error expected:\n{}",
        run.diagnostics().render_all()
    );
    assert_json_golden(golden_path("lowering-document.json"), &lowered_projection(&run));
}

// --- a field nobody read ----------------------------------------------------------------------

/// A registry over a toy construct `\marked{text}` that declares a `\mark`
/// field and a rule that lowers *every* argument it has, the trailing one
/// included — the shape a feature must not have.
///
/// A field is data the construct it was written after reads; lowering it is
/// how a construct fails to read its data, and that is what the field
/// element's one rule reports.
fn registry_lowering_its_own_field() -> Arc<FlmRegistry> {
    let field = TackOnField::new("mark", vec![ArgDef::new(ArgStdKind::CharsGroup)]).repeatable();
    let marked = ElementDef::new(
        "t",
        ["marked"],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named("text", ArgStdKind::Mandatory),
                ArgDef::named("fields", ArgStdKind::TackOn { fields: vec![field] }),
            ],
        },
        common::element("t.marked"),
    );
    let rule = common::lowering::rule(
        "t.marked.lower.default",
        "t.marked",
        common::lowering::produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let mut body = lw.lower_arg_named(cx, &node, "text")?;
            body.extend(lw.lower_arg_named(cx, &node, "fields")?);
            let payload = SpanPayload::default();
            Ok(Lowering::Emit(vec![lw.pres_with(cx, &"flm.pres.span".into(), payload, body)?]))
        },
    );
    common::lowering::registry(vec![marked], vec![rule])
}

#[test]
fn a_field_that_reaches_a_backend_is_reported_and_writes_nothing() {
    let run = render(&registry_lowering_its_own_field(), &["\\marked{x}\\mark{a}\\mark{b}"]);
    // One report per field the construct left for the backend, at the field.
    let ids: Vec<&str> = run.diagnostics().iter().map(|d| d.identifier()).collect();
    assert_eq!(ids, ["flm.lowering.field-not-read", "flm.lowering.field-not-read"]);
    let reported = expect(run.diagnostics().iter(), "flm.lowering.field-not-read")
        .at("\\mark{a}")
        .once();
    let condition = reported.data().downcast_ref::<FieldNotRead>().expect("typed");
    assert_eq!(condition.field, "mark");
    assert_eq!(condition.backend, "text");
    // Nothing is written for the field: the span holds the text alone, and no
    // stand-in node is left in its place.
    let lowered = lowered(&run, first());
    let span = find(&lowered.tree, pres::SPAN).expect("a span");
    assert_eq!(plain_text(span.children().iter()), "x");
    assert!(find(&lowered.tree, pres::UNSUPPORTED).is_none());
}

/// A registry over three toy commands: `\skip{}`, which writes nothing, and
/// `\trimmed{text}` and `\trimmedall{text}`, which write a span over their
/// argument staged through `stage_trimmed` — the first passing over `\skip{}`
/// at the edges, the second passing over nothing.
fn registry_trimming() -> Arc<FlmRegistry> {
    let command = |name: &str, args: Vec<ArgDef>| {
        ElementDef::new("t", [name], SyntaxDef::Macro { args }, common::element(&format!("t.{name}")))
    };
    let trimming = |name: &str, passes_over_skip: bool| {
        common::lowering::rule(
            &format!("t.{name}.lower.default"),
            &format!("t.{name}"),
            common::lowering::produces(&[pres::SPAN]),
            DefaultMark::Everywhere,
            move |node, cx, lw| {
                let content: Vec<NodeRef<'_, Flm>> =
                    node.argument_content_nodes(0).map(|nodes| nodes.iter().collect()).unwrap_or_default();
                let is_skip = |node: &NodeRef<'_, Flm>| element_of(node).name == "t.skip";
                let body = stage_trimmed(cx, lw, &content, |node| passes_over_skip && is_skip(node))?;
                let payload = SpanPayload::default();
                Ok(Lowering::Emit(vec![lw.pres_with(cx, &pres::SPAN.into(), payload, body)?]))
            },
        )
    };
    let skip = common::lowering::rule(
        "t.skip.lower.default",
        "t.skip",
        common::lowering::produces(&[]),
        DefaultMark::Everywhere,
        |_, _, _| Ok(Lowering::Emit(Vec::new())),
    );
    common::lowering::registry(
        vec![
            command("skip", vec![ArgDef::mandatory("nothing")]),
            command("trimmed", vec![ArgDef::mandatory("text")]),
            command("trimmedall", vec![ArgDef::mandatory("text")]),
        ],
        vec![trimming("trimmed", true), trimming("trimmedall", false), skip],
    )
}

/// The plain text of the outermost span of the first tree, with the element
/// names of its children.
fn trimmed_span(document: &str) -> (String, Vec<String>) {
    let run = render(&registry_trimming(), &[document]);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let lowered = lowered(&run, first());
    let span = find(&lowered.tree, pres::SPAN).expect("a span");
    let children = span.children().iter().map(|node| element_of(&node).name.to_string()).collect();
    (plain_text(span.children().iter()), children)
}

#[test]
fn a_trimmed_run_leaves_out_the_whitespace_and_comments_at_its_edges() {
    // Leading and trailing spaces, a comment at the head: all left out, the
    // space inside kept.
    assert_eq!(trimmed_span("\\trimmed{ %% a comment\n  x y  }").0, "x y");
    // An edge that is not text is staged as it stands, and the whitespace
    // beside it is left out.
    let (text, children) = trimmed_span("\\trimmed{ \\trimmedall{ a } }");
    assert_eq!(text, "a");
    assert_eq!(children, [pres::SPAN]);
    // A no-break space is content, never trimmed.
    assert_eq!(trimmed_span("\\trimmed{\u{a0}x }").0, "\u{a0}x");
    // Nothing but whitespace and comments stages nothing.
    assert_eq!(trimmed_span("\\trimmed{ %% only a comment\n }").1, Vec::<String>::new());
}

#[test]
fn a_trimmed_run_passes_over_the_nodes_its_caller_names_at_its_edges_only() {
    // `\trimmed` passes over `\skip{}`: the edges are the two texts, trimmed;
    // the `\skip{}` between them is lowered as it stands and writes nothing.
    assert_eq!(trimmed_span("\\trimmed{\\skip{} x \\skip{} y \\skip{}}").0, "x  y");
    // `\trimmedall` passes over nothing: the `\skip{}`s are its edges, and the
    // texts keep their spaces.
    assert_eq!(trimmed_span("\\trimmedall{\\skip{} x \\skip{} y \\skip{}}").0, " x  y ");
}
