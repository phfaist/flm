//! The lowering driver and the `Lowerer` facade of
//! `flm_core::stdcore::backends::lowering`, over toy elements and toy rules:
//! the native copy, `Emit` and the empty `Emit`, `Default` on a non-native
//! element and the rank bound, rounds following the rank, decoration hoisting,
//! accepted holes, the `produces` refusal, declared reads and option reads
//! enforced per rule, optional reads, requirements, one synthesized source per
//! node and rule, the splice of a value through catch-up, determinism, the
//! selection ladder seen from the driver, a preference overriding a native
//! claim, the frozen address in a later round, opaque math content with the
//! content that may stand inside it, and the re-entrant lowering methods.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::json;
use techy::core::node::{NodeRef, NodeTree};
use techy::error::Severity;

use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::model::element::element_of;
use flm_core::core::lowering::{DefaultMark, Lowering, LoweringRule, NodeRegion};
use flm_core::model::names::Name;
use flm_core::model::options::{Bind, OptionValues};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, LoweringSelection};
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_json_golden;
use flm_core::lang::mint::{ArgDef, ArgStdKind, BodyDef, ElementDef};
use flm_core::lang::{Flm, PresPayload};
use flm_core::stdcore::backends::lowering::{self as lowering, copy_run_verbatim, rank};
use flm_core::stdcore::backends::pres::{
    self, BlockPayload, DecoMarkPayload, DecorationPayload, LinkPayload, ListPayload,
    MathDisplay, MathPassthroughPayload, MathTextPayload, ParPayload, SpanPayload, UnsupportedPayload,
    attached_slots, decoration_at, list_entries, payload_of, plain_text,
};
use techy::serialize::SerialValue;

use common::lowering::{
    diagnostics, golden_path, lowered, lowered_projection, name, produces, registry, registry_with,
    render, render_with, rule, text_plan, try_lowered,
};
use common::pipeline::{LOG, MARKS, Marks, Note, stage};
use common::{env_def, macro_def, macro_with};

// --- the toy language --------------------------------------------------------------------------

/// `\foo{…}`: the element `t.foo`, one mandatory argument.
fn foo() -> ElementDef {
    macro_def("t", "foo", 1)
}

/// `\bar`: the element `t.bar`, no argument.
fn bar() -> ElementDef {
    macro_def("t", "bar", 0)
}

/// `\named{…}`: the element `t.named`, one mandatory argument named `x`.
fn named() -> ElementDef {
    macro_with("t", "named", vec![ArgDef::named("x", ArgStdKind::Mandatory)])
}

/// `\begin{env}…\end{env}`: the element `t.env`, an inline environment.
fn env() -> ElementDef {
    env_def("t", "env", BodyDef::new())
}

/// The payload of the toy presentation element `t.mid`.
#[derive(Debug)]
struct MidPayload;

impl PresPayload for MidPayload {
    fn project(&self) -> SerialValue {
        SerialValue::Map(Vec::new())
    }
}

/// `\foo{…}` → a span over the lowered argument.
fn span_rule() -> LoweringRule {
    rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_arg(cx, &node, 0)?;
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::SPAN),
                SpanPayload::default(),
                body,
            )?]))
        },
    )
}

/// A `PerTree` stage `t.mark` writing, for every node of its tree, the node's
/// address → `<tree>/<element>` into `t.marks`.
fn marking_feature(
    b: &mut flm_core::core::registry::FeatureBuilder<'_>,
) -> Result<(), flm_core::core::registry::BuildError> {
    b.table(MARKS, "every node's element, by address")?;
    b.stage(stage(
        StageDecl::new("t.mark", Effect::Compute, Scope::PerTree).writes(MARKS),
        Needs::new(),
        |cx, out| {
            let Some(tree) = cx.my_tree() else { return };
            let marks = out.table(MARKS).expect("declared");
            for node in tree.iter_storage_order() {
                if let Some(addr) = cx.addr(&node) {
                    marks.0.insert(addr, format!("{}/{}", addr.tree, element_of(&node).name));
                }
            }
        },
    ))
}

// --- helpers -----------------------------------------------------------------------------------

/// The element names of a tree in storage order.
fn names(tree: &NodeTree<Flm>) -> Vec<String> {
    tree.iter_storage_order().map(|node| element_of(&node).name.to_string()).collect()
}

/// The first node of `element` in storage order.
fn find<'t>(tree: &'t NodeTree<Flm>, element: &str) -> Option<NodeRef<'t, Flm>> {
    tree.iter_storage_order().find(|node| element_of(node).name == element)
}

/// The text nodes of a tree in storage order.
fn texts(tree: &NodeTree<Flm>) -> Vec<String> {
    tree.iter_storage_order().filter_map(|node| node.chars().map(String::from)).collect()
}

/// The plain text of a node's body.
fn body_text(node: &NodeRef<'_, Flm>) -> String {
    node.body().map(|body| plain_text(body.iter())).unwrap_or_default()
}

fn first() -> TreeId {
    TreeId::tree(0)
}

// --- the driver's dispatch ---------------------------------------------------------------------

#[test]
fn a_native_element_is_copied_with_its_children_lowered() {
    let registry = registry(vec![foo()], vec![span_rule()]);
    let run = render(&registry, &["{\\foo{b}}"]);
    let lowered = lowered(&run, first());
    let group = find(&lowered.tree, "flm.group").expect("the group is copied");
    let children: Vec<String> =
        group.children().iter().map(|child| element_of(&child).name.to_string()).collect();
    assert_eq!(children, [pres::SPAN]);
}

#[test]
fn a_rule_emits_a_presentation_node_over_the_lowered_argument() {
    let registry = registry(vec![foo()], vec![span_rule()]);
    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    let span = find(&lowered.tree, pres::SPAN).expect("a span");
    assert_eq!(body_text(&span), "b");
    assert!(find(&lowered.tree, "t.foo").is_none(), "the source node is gone");
    assert_eq!(lowered.rounds, 1);
}

#[test]
fn an_empty_emit_makes_the_node_vanish() {
    let vanish =
        rule("t.foo.lower.default", "t.foo", produces(&[]), DefaultMark::Everywhere, |_, _, _| {
            Ok(Lowering::Emit(Vec::new()))
        });
    let registry = registry(vec![foo()], vec![vanish]);
    let run = render(&registry, &["a\\foo{b}c"]);
    let lowered = lowered(&run, first());
    assert_eq!(texts(&lowered.tree), ["a", "c"]);
    assert!(!names(&lowered.tree).iter().any(|name| name == "t.foo"));
}

#[test]
fn default_on_a_non_native_element_exceeds_the_round_bound() {
    let keep = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |_, _, _| Ok(Lowering::Default),
    );
    let registry = registry(vec![foo()], vec![keep]);
    let run = render(&registry, &["\\foo{b}"]);
    expect(diagnostics(&run), "flm.lowering.rounds-exceeded")
        .severity(Severity::Error)
        .data(json!({ "backend": "text", "rank": 1, "remaining": "t.foo" }))
        .once();
    assert!(try_lowered(&run, first()).is_none(), "the failed tree has no lowered form");
}

#[test]
fn rounds_follow_the_rank_of_the_produces_graph() {
    // `t.foo` → `t.mid` (a toy presentation element) → `span`: rank 2.
    let to_mid = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&["t.mid"]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_arg(cx, &node, 0)?;
            Ok(Lowering::Emit(vec![lw.pres_with(cx, &name("t.mid"), MidPayload, body)?]))
        },
    );
    let to_span = rule(
        "t.mid.lower.default",
        "t.mid",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_body(cx, &node)?;
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::SPAN),
                SpanPayload::default(),
                body,
            )?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![to_mid, to_span], |b| {
        b.presentation_element::<MidPayload>("t.mid", "a toy intermediate element")?;
        Ok(())
    });
    let plan = text_plan(&registry);
    let native: BTreeSet<_> = registry.backend("text").unwrap().native.iter().cloned().collect();
    assert_eq!(rank(&registry, plan.selection("text").unwrap(), &native), 2);

    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    assert_eq!(lowered.rounds, 2);
    assert_eq!(body_text(&find(&lowered.tree, pres::SPAN).expect("a span")), "b");
    assert!(find(&lowered.tree, "t.mid").is_none());
}

#[test]
fn a_decoration_in_a_body_is_hoisted_into_an_attached_slot() {
    let decorated = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::PAR, pres::DECO_RUNIN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let label = vec![lw.text(cx, "Note")?];
            let deco = lw.pres_with(cx, &name(pres::DECO_RUNIN), DecorationPayload, label)?;
            let mut body = vec![deco];
            body.extend(lw.lower_arg(cx, &node, 0)?);
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::PAR),
                ParPayload::default(),
                body,
            )?]))
        },
    );
    let registry = registry(vec![foo()], vec![decorated]);
    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    let par = find(&lowered.tree, pres::PAR).expect("a par");
    let body: Vec<String> =
        par.body().unwrap().iter().map(|node| element_of(&node).name.to_string()).collect();
    assert_eq!(body, [pres::DECO_MARK, "flm.text"]);
    let attached = attached_slots(&par);
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0].name, Some(pres::DECO_RUNIN));
    let mark = par.body().unwrap().first().unwrap();
    let deco = decoration_at(&par, &mark).expect("the mark names the decoration");
    assert_eq!(body_text(&deco), "Note");
    assert_json_golden(golden_path("lowering-hoisted.json"), &lowered_projection(&run));
}

#[test]
fn an_accepted_hole_lowers_to_unsupported_with_one_error() {
    let registry = registry(vec![bar()], vec![]);
    let overrides = Overrides {
        unsupported: [("text".into(), vec!["t.bar".into()])].into_iter().collect(),
        ..Overrides::none()
    };
    let run = render_with(&registry, &["a \\bar b"], &overrides);
    expect(diagnostics(&run), "flm.lowering.unsupported-element")
        .severity(Severity::Error)
        .at_line_col(1, 3)
        .data(json!({ "backend": "text", "element": "t.bar" }))
        .once();
    let lowered = lowered(&run, first());
    let hole = find(&lowered.tree, pres::UNSUPPORTED).expect("the stand-in");
    assert_eq!(hole.body().map(|body| body.len()), Some(0), "an empty body");
    assert_eq!(payload_of::<UnsupportedPayload>(&hole).unwrap().element, "t.bar");
}

#[test]
fn staging_an_element_outside_produces_is_refused() {
    let wrong = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_arg(cx, &node, 0)?;
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::PAR),
                ParPayload::default(),
                body,
            )?]))
        },
    );
    let registry = registry(vec![foo()], vec![wrong]);
    let run = render(&registry, &["\\foo{b}"]);
    expect(diagnostics(&run), "flm.lowering.rule-failed")
        .severity(Severity::Error)
        .at("\\foo{b}")
        .once();
    let lowered = lowered(&run, first());
    assert!(find(&lowered.tree, pres::UNSUPPORTED).is_some(), "the node lowers as a hole");
    assert!(find(&lowered.tree, pres::PAR).is_none());
}

// --- declared reads and options ----------------------------------------------------------------

#[test]
fn a_rule_reads_a_table_it_declares_at_its_own_address() {
    let reading = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).reads(MARKS),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let marks: &Marks = lw.get(MARKS)?.expect("the marks are provided");
            let found = marks.0.get(&lw.addr()).cloned().unwrap_or_default();
            Ok(Lowering::Emit(vec![lw.text(cx, &found)?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![reading], marking_feature);
    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    assert_eq!(texts(&lowered.tree), ["tree#0/t.foo"]);
}

#[test]
fn an_undeclared_read_fails_the_rule() {
    let undeclared = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let _: Option<&Marks> = lw.get(MARKS)?;
            Ok(Lowering::Emit(vec![lw.text(cx, "read")?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![undeclared], marking_feature);
    let run = render(&registry, &["\\foo{b}"]);
    expect(diagnostics(&run), "flm.lowering.rule-failed").at("\\foo{b}").once();
    assert!(find(&lowered(&run, first()).tree, pres::UNSUPPORTED).is_some());
}

#[test]
fn an_optional_read_of_an_absent_table_is_none() {
    let optional = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).reads_opt(LOG),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let text = if lw.get(LOG)?.is_none() { "absent" } else { "present" };
            Ok(Lowering::Emit(vec![lw.text(cx, text)?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![optional], |b| {
        b.table(LOG, "nobody writes it")?;
        Ok(())
    });
    let run = render(&registry, &["\\foo{b}"]);
    assert_eq!(texts(&lowered(&run, first()).tree), ["absent"]);
}

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone)]
struct Width(u32);

fn width_feature(
    b: &mut flm_core::core::registry::FeatureBuilder<'_>,
) -> Result<(), flm_core::core::registry::BuildError> {
    b.options::<Width>(Bind::Render)
}

fn width_overrides(width: u32) -> Overrides {
    Overrides { render_options: OptionValues::new().with(Width(width)), ..Overrides::none() }
}

#[test]
fn a_rule_reads_an_option_it_declares() {
    let reading = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).option::<Width>(),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let width = lw.option::<Width>()?.0;
            Ok(Lowering::Emit(vec![lw.text(cx, &width.to_string())?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![reading], width_feature);
    let run = render_with(&registry, &["\\foo{b}"], &width_overrides(7));
    assert_eq!(texts(&lowered(&run, first()).tree), ["7"]);
}

#[test]
fn an_undeclared_option_read_fails_the_rule() {
    let undeclared = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let width = lw.option::<Width>()?.0;
            Ok(Lowering::Emit(vec![lw.text(cx, &width.to_string())?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![undeclared], width_feature);
    let run = render_with(&registry, &["\\foo{b}"], &width_overrides(7));
    expect(diagnostics(&run), "flm.lowering.rule-failed").at("\\foo{b}").once();
}

// --- requirements, provenance, diagnostics -----------------------------------------------------

#[test]
fn a_requirement_lands_beside_the_lowered_tree() {
    let requiring = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            lw.require(&Name::from("t.style"));
            Ok(Lowering::Emit(vec![lw.text(cx, "x")?]))
        },
    );
    let registry = registry(vec![foo()], vec![requiring]);
    let run = render(&registry, &["\\foo{b}"]);
    let requires: Vec<String> =
        lowered(&run, first()).requires.iter().map(|id| id.to_string()).collect();
    assert_eq!(requires, ["t.style"]);
}

#[test]
fn a_selection_requirement_lands_beside_every_lowered_tree_without_an_occurrence() {
    let selected = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).selection_requires("t.script"),
        DefaultMark::Everywhere,
        |_, cx, lw| Ok(Lowering::Emit(vec![lw.text(cx, "x")?])),
    );
    let registry = registry(vec![foo()], vec![selected]);
    // No `\foo` anywhere: the rule is selected for the backend, never applied.
    let run = render(&registry, &["no occurrence"]);
    let requires: Vec<String> =
        lowered(&run, first()).requires.iter().map(|id| id.to_string()).collect();
    assert_eq!(requires, ["t.script"]);
}

#[test]
fn synthesized_text_sits_on_one_source_per_node_and_rule_triggered_at_the_node() {
    let two = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]),
        DefaultMark::Everywhere,
        |_, cx, lw| Ok(Lowering::Emit(vec![lw.text(cx, "p")?, lw.text(cx, "q")?])),
    );
    let registry = registry(vec![foo()], vec![two]);
    let run = render(&registry, &["\\foo{a} \\foo{b}"]);
    let lowered = lowered(&run, first());
    let staged: Vec<NodeRef<'_, Flm>> = lowered
        .tree
        .iter_storage_order()
        .filter(|node| matches!(node.chars(), Some("p") | Some("q")))
        .collect();
    assert_eq!(staged.len(), 4);
    let sources: Vec<_> = staged.iter().map(|node| Arc::clone(node.span().source())).collect();
    assert!(Arc::ptr_eq(&sources[0], &sources[1]), "one source per (node, rule)");
    assert!(!Arc::ptr_eq(&sources[0], &sources[2]), "another node, another source");
    let triggered: Vec<String> = sources
        .iter()
        .map(|source| {
            source.provenance().triggered_at().expect("synthesized").content().to_string()
        })
        .collect();
    assert_eq!(triggered, ["\\foo{a}", "\\foo{a}", "\\foo{b}", "\\foo{b}"]);
}

#[test]
fn a_rule_diagnoses_at_its_node() {
    let noting = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            lw.warning(Note { detail: "seen".into() }, node.span().clone());
            Ok(Lowering::Emit(vec![lw.text(cx, "x")?]))
        },
    );
    let registry = registry(vec![foo()], vec![noting]);
    let run = render(&registry, &["\\foo{b}"]);
    expect(diagnostics(&run), "t.note").severity(Severity::Warning).at("\\foo{b}").once();
}

// --- splicing a value --------------------------------------------------------------------------

#[test]
fn a_spliced_value_is_caught_up_and_read_at_the_values_address() {
    let inner = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).reads(MARKS),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let marks: &Marks = lw.get(MARKS)?.expect("provided");
            let found = marks.0.get(&lw.addr()).cloned().unwrap_or_default();
            Ok(Lowering::Emit(vec![lw.text(cx, &found)?]))
        },
    );
    let splicing = rule(
        "t.bar.lower.default",
        "t.bar",
        produces(&[]),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let value = common::pipeline::tree(&cx_registry(), "\\foo{v}");
            Ok(Lowering::Emit(lw.splice(cx, &value)?))
        },
    );
    let registry = registry_with(vec![foo(), bar()], vec![inner, splicing], marking_feature);
    set_registry(&registry);
    let run = render(&registry, &["\\bar"]);
    let lowered = lowered(&run, first());
    assert_eq!(texts(&lowered.tree), ["value#0/t.foo"]);
}

// --- lowering a region of another tree --------------------------------------------------------

/// The node a value's rule points back to, stashed by the splicing rule (a
/// value built by hand holds no address; the test lends it one).
static POINTED: std::sync::Mutex<Option<NodeAddr>> = std::sync::Mutex::new(None);

/// The general rule `<element>.lower.default` of `element`.
fn everywhere(
    element: &str,
    decl: flm_core::core::lowering::LoweringDecl,
    body: impl Fn(
        NodeRef<'_, Flm>,
        &mut flm_core::core::lowering::LowerCx<'_>,
        &mut dyn flm_core::core::lowering::Lowerer,
    ) -> Result<Lowering, flm_core::core::lowering::LowerError>
    + Send
    + Sync
    + 'static,
) -> LoweringRule {
    rule(&format!("{element}.lower.default"), element, decl, DefaultMark::Everywhere, body)
}

/// `\foo{…}` → the marks table's entry at the rule's own address, as text.
fn marks_rule() -> LoweringRule {
    rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).reads(MARKS),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let marks: &Marks = lw.get(MARKS)?.expect("provided");
            let found = marks.0.get(&lw.addr()).cloned().unwrap_or_default();
            Ok(Lowering::Emit(vec![lw.text(cx, &found)?]))
        },
    )
}

#[test]
fn a_region_of_the_document_lowered_inside_a_value_reads_at_the_documents_addresses() {
    // `\named{…}` writes nothing where it stands: its content is shown once,
    // where the value's `env` points at it.
    let silent = everywhere("t.named", produces(&[]), |_, _, _| Ok(Lowering::Emit(Vec::new())));
    let splicing = everywhere("t.bar", produces(&[]), |node, cx, lw| {
        let parent = node.parent().expect("a paragraph");
        let named = parent
            .children()
            .iter()
            .find(|child| element_of(child).name == "t.named")
            .expect("a sibling \\named");
        *POINTED.lock().unwrap() = Some(lw.addr_of(&named)?);
        let value = common::pipeline::tree(&cx_registry(), "\\begin{env}\\end{env}");
        Ok(Lowering::Emit(lw.splice(cx, &value)?))
    });
    let pointing = everywhere("t.env", produces(&[]), |_, cx, lw| {
        let at = POINTED.lock().unwrap().expect("stashed");
        assert!(lw.tree_id().is_value());
        let mut seen = None;
        let region = NodeRegion::Argument(0);
        let out = lw.lower_region(cx, at, region, &mut |cx, lw, node, content| {
            let element = element_of(node).name.to_string();
            seen = Some((lw.tree_id(), lw.addr(), element, content.len()));
            let first = content[0].id().index() as u32;
            let slice = content[0].tree().slice(first..first + content.len() as u32).unwrap();
            lw.lower_nodes(cx, slice)
        })?;
        let (tree, addr, element, count) = seen.expect("the shape ran");
        assert_eq!((tree, addr, element.as_str(), count), (at.tree, at, "t.named", 1));
        assert!(lw.tree_id().is_value(), "the value's tree again once the region is lowered");
        Ok(Lowering::Emit(out))
    });
    let registry = registry_with(
        vec![foo(), bar(), named(), env()],
        vec![marks_rule(), silent, splicing, pointing],
        marking_feature,
    );
    set_registry(&registry);
    let run = render(&registry, &["\\named{\\foo{b}}\\bar"]);
    let lowered = lowered(&run, first());
    // The value's own marks table shadows the document's at the value's
    // addresses only: `\foo` of the document reads its document entry.
    assert_eq!(texts(&lowered.tree), ["tree#0/t.foo"]);
    assert!(diagnostics(&run).is_empty(), "{:?}", diagnostics(&run));
}

#[test]
fn a_slot_region_is_lowered_as_an_argument_region_is() {
    // `env` writes nothing where it stands; `\\bar` shows its body, the slot
    // at position 0, by pointing at it.
    let silent = everywhere("t.env", produces(&[]), |_, _, _| Ok(Lowering::Emit(Vec::new())));
    let pointing = everywhere("t.bar", produces(&[]), |node, cx, lw| {
        let parent = node.parent().expect("a parent");
        let env = parent
            .children()
            .iter()
            .find(|child| element_of(child).name == "t.env")
            .expect("a sibling environment");
        let at = lw.addr_of(&env)?;
        Ok(Lowering::Emit(lw.lower_region(cx, at, NodeRegion::Slot(0), &mut lowering::lower_region_content)?))
    });
    let rules = vec![marks_rule(), silent, pointing];
    let registry = registry_with(vec![foo(), bar(), env()], rules, marking_feature);
    let run = render(&registry, &["\\begin{env}\\foo{b}\\end{env}\\bar"]);
    assert!(diagnostics(&run).is_empty(), "{:?}", diagnostics(&run));
    assert_eq!(texts(&lowered(&run, first()).tree), ["tree#0/t.foo"]);
}

#[test]
fn a_region_that_does_not_exist_is_reported_at_the_pointing_node() {
    let pointing = everywhere("t.bar", produces(&[]), |node, cx, lw| {
        let own = lw.addr_of(&node)?;
        Ok(Lowering::Emit(lw.lower_region(cx, own, NodeRegion::Argument(3), &mut lowering::lower_region_content)?))
    });
    let run = render(&registry(vec![bar()], vec![pointing]), &["a \\bar c"]);
    expect(diagnostics(&run), "flm.lowering.region-not-found")
        .severity(Severity::Error)
        .at("\\bar ")
        .once();
    let lowered = lowered(&run, first());
    assert!(names(&lowered.tree).iter().any(|name| name == pres::UNSUPPORTED));
    let far = everywhere("t.bar", produces(&[]), |node, cx, lw| {
        let own = lw.addr_of(&node)?;
        let elsewhere = NodeAddr::new(TreeId::tree(9), own.node);
        Ok(Lowering::Emit(lw.lower_region(cx, elsewhere, NodeRegion::Slot(0), &mut lowering::lower_region_content)?))
    });
    let run = render(&registry(vec![bar()], vec![far]), &["a \\bar c"]);
    expect(diagnostics(&run), "flm.lowering.region-not-found").at("\\bar ").once();
}

#[test]
fn nodes_of_a_tree_the_lowering_does_not_hold_are_refused_by_lower_nodes_and_lower_run() {
    // A tree the rule parsed itself: lowered in place, its nodes would answer
    // addresses of the tree being lowered.
    let lang = flm_core::lang::build_language(flm_core::lang::LangInputs::new());
    let elsewhere: Arc<NodeTree<Flm>> = Arc::clone(common::parse(&lang, "x y").tree());
    for by_run in [false, true] {
        let tree = Arc::clone(&elsewhere);
        let foreign = everywhere("t.bar", produces(&[]), move |_node, cx, lw| {
            let staged = if by_run {
                let nodes: Vec<NodeRef<'_, Flm>> = tree.root().children().iter().collect();
                lowering::lower_run(cx, lw, &nodes)?
            } else {
                lw.lower_nodes(cx, tree.root().children())?
            };
            Ok(Lowering::Emit(staged))
        });
        let run = render(&registry(vec![bar()], vec![foreign]), &["a \\bar c"]);
        expect(diagnostics(&run), "flm.lowering.rule-failed")
            .severity(Severity::Error)
            .at("\\bar ")
            .once();
        let rendered = diagnostics(&run).iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains("lower_region"), "{rendered}");
    }
}

#[test]
fn lowering_a_region_of_its_own_node_is_lowering_its_argument() {
    let own = everywhere("t.foo", produces(&[pres::SPAN]), |node, cx, lw| {
        let at = lw.addr_of(&node)?;
        let body = lw.lower_region(cx, at, NodeRegion::Argument(0), &mut lowering::lower_region_content)?;
        let span = lw.pres_with(cx, &name(pres::SPAN), SpanPayload::default(), body)?;
        Ok(Lowering::Emit(vec![span]))
    });
    let by_region = render(&registry(vec![foo()], vec![own]), &["a \\foo{b} c"]);
    let by_argument = render(&registry(vec![foo()], vec![span_rule()]), &["a \\foo{b} c"]);
    let (by_region, by_argument) = (lowered(&by_region, first()), lowered(&by_argument, first()));
    assert_eq!(names(&by_region.tree), names(&by_argument.tree));
    assert_eq!(texts(&by_region.tree), texts(&by_argument.tree));
}

/// The registry the splicing rule parses its value under (a rule has no
/// registry access through the façade; the test lends it one).
fn cx_registry() -> Arc<FlmRegistry> {
    REGISTRY.lock().unwrap().clone().expect("the test set the registry")
}

fn set_registry(registry: &Arc<FlmRegistry>) {
    *REGISTRY.lock().unwrap() = Some(Arc::clone(registry));
}

static REGISTRY: std::sync::Mutex<Option<Arc<FlmRegistry>>> = std::sync::Mutex::new(None);

// --- determinism -------------------------------------------------------------------------------

#[test]
fn lowering_twice_is_identical() {
    let registry = registry(vec![foo()], vec![span_rule()]);
    let first_run = render(&registry, &["a \\foo{b} c"]);
    let second_run = render(&registry, &["a \\foo{b} c"]);
    assert_eq!(lowered_projection(&first_run), lowered_projection(&second_run));
}

// --- the selection ladder from the driver's side -----------------------------------------------

/// `\foo{…}` → a link over the lowered argument.
fn link_rule(rule_name: &str, default: DefaultMark) -> LoweringRule {
    rule(rule_name, "t.foo", produces(&[pres::LINK]), default, |node, cx, lw| {
        let body = lw.lower_arg(cx, &node, 0)?;
        let payload = LinkPayload {
            href: "#x".into(),
            kind: "ref".into(),
            doc_role: None,
            label: None,
            shows_target: false,
        };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &name(pres::LINK), payload, body)?]))
    })
}

fn lowering_overrides(preferred: &str) -> Overrides {
    Overrides {
        lowering: [("text".into(), LoweringSelection::new().prefer(preferred))]
            .into_iter()
            .collect(),
        ..Overrides::none()
    }
}

#[test]
fn the_backend_default_beats_the_general_default() {
    let registry = registry(
        vec![foo()],
        vec![
            span_rule(),
            link_rule("t.foo.lower.text", DefaultMark::OnBackends(vec!["text".into()])),
        ],
    );
    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    assert!(find(&lowered.tree, pres::LINK).is_some());
    assert!(find(&lowered.tree, pres::SPAN).is_none());
}

#[test]
fn a_preference_settles_a_contest_of_alternatives() {
    let registry = registry(
        vec![foo()],
        vec![
            rule(
                "t.foo.lower.a",
                "t.foo",
                produces(&[pres::SPAN]),
                DefaultMark::Never,
                |node, cx, lw| {
                    let body = lw.lower_arg(cx, &node, 0)?;
                    Ok(Lowering::Emit(vec![lw.pres_with(
                        cx,
                        &name(pres::SPAN),
                        SpanPayload::default(),
                        body,
                    )?]))
                },
            ),
            link_rule("t.foo.lower.b", DefaultMark::Never),
        ],
    );
    let run = render_with(&registry, &["\\foo{b}"], &lowering_overrides("t.foo.lower.b"));
    assert!(find(&lowered(&run, first()).tree, pres::LINK).is_some());
}

#[test]
fn a_unique_alternative_is_selected_without_a_preference() {
    let registry = registry(vec![foo()], vec![link_rule("t.foo.lower.only", DefaultMark::Never)]);
    let run = render(&registry, &["\\foo{b}"]);
    assert!(find(&lowered(&run, first()).tree, pres::LINK).is_some());
}

/// `span` → a link over its lowered body: an alternative rule on a natively
/// claimed presentation element.
fn span_to_link() -> LoweringRule {
    rule(
        "t.span.lower.alt",
        pres::SPAN,
        produces(&[pres::LINK]),
        DefaultMark::Never,
        |node, cx, lw| {
            let body = lw.lower_body(cx, &node)?;
            let payload = LinkPayload {
                href: "#x".into(),
                kind: "ref".into(),
                doc_role: None,
                label: None,
                shows_target: false,
            };
            Ok(Lowering::Emit(vec![lw.pres_with(cx, &name(pres::LINK), payload, body)?]))
        },
    )
}

#[test]
fn a_preference_overrides_a_native_claim_and_costs_a_round() {
    let registry = registry(vec![foo()], vec![span_rule(), span_to_link()]);
    let run = render_with(&registry, &["\\foo{b}"], &lowering_overrides("t.span.lower.alt"));
    let lowered = lowered(&run, first());
    assert_eq!(lowered.rounds, 2);
    assert_eq!(body_text(&find(&lowered.tree, pres::LINK).expect("a link")), "b");
    assert!(find(&lowered.tree, pres::SPAN).is_none());
}

#[test]
fn the_address_in_a_later_round_is_the_frozen_producing_node() {
    let reading_alt = rule(
        "t.span.lower.alt",
        pres::SPAN,
        produces(&[]).reads(MARKS),
        DefaultMark::Never,
        |_, cx, lw| {
            let marks: &Marks = lw.get(MARKS)?.expect("provided");
            let found = marks.0.get(&lw.addr()).cloned().unwrap_or_default();
            Ok(Lowering::Emit(vec![lw.text(cx, &found)?]))
        },
    );
    let registry = registry_with(vec![foo()], vec![span_rule(), reading_alt], marking_feature);
    let run = render_with(&registry, &["\\foo{b}"], &lowering_overrides("t.span.lower.alt"));
    // The span was staged at the frozen `\foo` node; in round two its address
    // is that node's.
    assert_eq!(texts(&lowered(&run, first()).tree), ["tree#0/t.foo"]);
}

// --- opaque content ----------------------------------------------------------------------------

#[test]
fn a_verbatim_copy_into_a_non_opaque_body_costs_rounds_the_bound_does_not_count() {
    // `\foo{\bar}`: the rule for `t.foo` copies its argument verbatim into a
    // span — not an opaque element — so the `t.bar` inside, which has a rule of
    // its own, is carried unlowered, invisible to the round bound.
    let copying = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let content = node.argument_content_nodes(0).expect("a mandatory argument");
            let body = copy_run_verbatim(cx, content)?;
            let span = lw.pres_with(cx, &name(pres::SPAN), SpanPayload::default(), body)?;
            Ok(Lowering::Emit(vec![span]))
        },
    );
    let bar_rule =
        rule("t.bar.lower.default", "t.bar", produces(&[]), DefaultMark::Everywhere, |_, cx, lw| {
            Ok(Lowering::Emit(vec![lw.text(cx, "b")?]))
        });
    let registry = registry(vec![foo(), bar()], vec![copying, bar_rule]);
    let run = render(&registry, &["\\foo{\\bar}"]);
    expect(diagnostics(&run), "flm.lowering.rounds-exceeded")
        .data(json!({ "backend": "text", "rank": 1, "remaining": "t.bar" }))
        .once();
    assert!(try_lowered(&run, first()).is_none());
}

#[test]
fn opaque_math_content_is_carried_verbatim_and_not_counted() {
    // `\bar` is declared for math mode too, so that it is a node of its own
    // inside the formula.
    let bar = bar().modes([techy::latexlike::Mode::Text, techy::latexlike::Mode::Math]);
    let registry = registry(vec![bar], vec![]);
    let overrides = Overrides {
        unsupported: [("text".into(), vec!["t.bar".into()])].into_iter().collect(),
        ..Overrides::none()
    };
    let run = render_with(&registry, &["\\(a \\bar b\\)"], &overrides);
    let lowered = lowered(&run, first());
    let math = find(&lowered.tree, pres::MATH_PASSTHROUGH).expect("math-passthrough");
    let inside: Vec<String> = math
        .body()
        .unwrap()
        .iter()
        .flat_map(|n| std::iter::once(n).chain(n.descendants()))
        .map(|n| element_of(&n).name.to_string())
        .collect();
    assert!(inside.iter().any(|name| name == "t.bar"), "the hole survives inside math: {inside:?}");
    assert!(!diagnostics(&run).iter().any(|d| d.identifier().starts_with("flm.lowering.")));
    assert_eq!(lowered.rounds, 1);
}

/// `\mathish{…}{…}`: a toy stand-in for a rule that hands mathematics to a
/// backend as source with one run of content standing in it — the shape the
/// `math` feature's rules take on a backend that renders formulas. The first
/// argument is copied verbatim into the `math-passthrough` body as the source it
/// is; the second is lowered through the rules into a `math-text` node beside
/// it.
fn mathish_rule(copy_into_content: bool) -> LoweringRule {
    rule(
        "t.mathish.lower.default",
        "t.mathish",
        // `t.bar` is declared as produced because the rule stages a copy of
        // whatever stands in its arguments, `t.bar` included: that is what the
        // round bound is computed from.
        produces(&[pres::MATH_PASSTHROUGH, pres::MATH_TEXT, "t.bar"]),
        DefaultMark::Everywhere,
        move |node, cx, lw| {
            let source = node.argument_content_nodes(0).expect("the first argument");
            let mut body = copy_run_verbatim(cx, source)?;
            let content = if copy_into_content {
                copy_run_verbatim(cx, node.argument_content_nodes(1).expect("the second"))?
            } else {
                lw.lower_arg(cx, &node, 1)?
            };
            let payload = MathTextPayload {
                command: Some(String::from("text")),
                anchor: None,
                at_tag: false,
            };
            body.push(lw.pres_with(cx, &name(pres::MATH_TEXT), payload, content)?);
            let payload = MathPassthroughPayload { display: MathDisplay::Inline, env: None };
            let math = lw.pres_with(cx, &name(pres::MATH_PASSTHROUGH), payload, body)?;
            Ok(Lowering::Emit(vec![math]))
        },
    )
}

/// `\bar` → the text `b`.
fn bar_text_rule() -> LoweringRule {
    rule("t.bar.lower.default", "t.bar", produces(&[]), DefaultMark::Everywhere, |_, cx, lw| {
        Ok(Lowering::Emit(vec![lw.text(cx, "b")?]))
    })
}

/// The element names of a node's body, its descendants included.
fn inside(node: &NodeRef<'_, Flm>) -> Vec<String> {
    node.body()
        .map(|body| {
            body.iter()
                .flat_map(|n| std::iter::once(n).chain(n.descendants()))
                .map(|n| element_of(&n).name.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn content_inside_opaque_content_is_lowered_and_the_source_beside_it_is_not() {
    let registry = registry(vec![macro_def("t", "mathish", 2), bar(), foo()], vec![
        mathish_rule(false),
        span_rule(),
        bar_text_rule(),
    ]);
    let run = render(&registry, &["\\mathish{\\bar}{x \\foo{y}}"]);
    let lowered = lowered(&run, first());
    let math = find(&lowered.tree, pres::MATH_PASSTHROUGH).expect("math-passthrough");
    // The first argument is source: the `t.bar` node stands in the body as it
    // was written, although it has a rule of its own.
    assert!(inside(&math).iter().any(|name| name == "t.bar"), "{:?}", inside(&math));
    let content = find(&lowered.tree, pres::MATH_TEXT).expect("math-text");
    // The second is content: it went through the rules.
    let names = inside(&content);
    assert!(names.iter().any(|name| name == pres::SPAN), "{names:?}");
    assert!(!names.iter().any(|name| name == "t.foo"), "{names:?}");
    assert_eq!(lowered.rounds, 1);
}

#[test]
fn mathematics_nested_in_content_inside_mathematics_stays_a_passthrough_formula_of_its_own() {
    let registry = registry(vec![macro_def("t", "mathish", 2), bar()], vec![
        mathish_rule(false),
        bar_text_rule(),
    ]);
    let run = render(&registry, &["\\mathish{\\bar}{if \\(y > 0\\)}"]);
    let lowered = lowered(&run, first());
    let content = find(&lowered.tree, pres::MATH_TEXT).expect("math-text");
    let names = inside(&content);
    assert!(names.iter().any(|name| name == pres::MATH_PASSTHROUGH), "{names:?}");
    assert!(names.iter().any(|name| name == "flm.text"), "{names:?}");
}

#[test]
fn a_later_round_reaches_content_inside_opaque_content_and_nothing_else_in_it() {
    // The rule copies its second argument into the content node unlowered, so
    // what stands there needs a further round: the driver's count of what is
    // left to lower sees it although its parent is opaque, and the round that
    // follows lowers it. The `t.bar` copied into the source beside it is not
    // counted and is not lowered.
    let registry =
        registry(vec![macro_def("t", "mathish", 2), bar()], vec![
            mathish_rule(true),
            bar_text_rule(),
        ]);
    let run = render(&registry, &["\\mathish{\\bar}{\\bar}"]);
    assert!(
        !diagnostics(&run).iter().any(|d| d.identifier().starts_with("flm.lowering.")),
        "{:?}",
        diagnostics(&run).iter().map(|d| d.identifier()).collect::<Vec<_>>()
    );
    let lowered = lowered(&run, first());
    assert_eq!(lowered.rounds, 2);
    let content = find(&lowered.tree, pres::MATH_TEXT).expect("math-text");
    assert_eq!(inside(&content), ["flm.text"]);
    let math = find(&lowered.tree, pres::MATH_PASSTHROUGH).expect("math-passthrough");
    assert!(inside(&math).iter().any(|name| name == "t.bar"), "{:?}", inside(&math));
}

// --- the re-entrant lowering methods -----------------------------------------------------------

#[test]
fn lower_body_lowers_an_environments_body() {
    let block = rule(
        "t.env.lower.default",
        "t.env",
        produces(&[pres::BLOCK]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_body(cx, &node)?;
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::BLOCK),
                BlockPayload::default(),
                body,
            )?]))
        },
    );
    let registry = registry(vec![foo(), env()], vec![span_rule(), block]);
    let run = render(&registry, &["\\begin{env}\\foo{b}\\end{env}"]);
    let lowered = lowered(&run, first());
    let block = find(&lowered.tree, pres::BLOCK).expect("a block");
    let inside: Vec<String> = block
        .body()
        .unwrap()
        .iter()
        .flat_map(|n| std::iter::once(n).chain(n.descendants()))
        .map(|n| element_of(&n).name.to_string())
        .collect();
    assert!(inside.iter().any(|name| name == pres::SPAN), "{inside:?}");
}

#[test]
fn lower_arg_named_lowers_the_named_argument() {
    let by_name = rule(
        "t.named.lower.default",
        "t.named",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_arg_named(cx, &node, "x")?;
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::SPAN),
                SpanPayload::default(),
                body,
            )?]))
        },
    );
    let registry = registry(vec![named()], vec![by_name]);
    let run = render(&registry, &["\\named{b}"]);
    assert_eq!(body_text(&find(&lowered(&run, first()).tree, pres::SPAN).expect("a span")), "b");
}

#[test]
fn lower_nodes_lowers_a_run_of_nodes() {
    let by_run = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::SPAN]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let content = node.argument_content_nodes(0).expect("a mandatory argument");
            let body = lw.lower_nodes(cx, content)?;
            Ok(Lowering::Emit(vec![lw.pres_with(
                cx,
                &name(pres::SPAN),
                SpanPayload::default(),
                body,
            )?]))
        },
    );
    let registry = registry(vec![foo()], vec![by_run]);
    let run = render(&registry, &["\\foo{b}"]);
    assert_eq!(body_text(&find(&lowered(&run, first()).tree, pres::SPAN).expect("a span")), "b");
}

#[test]
fn a_missing_named_argument_is_a_structure_error() {
    let by_name = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let body = lw.lower_arg_named(cx, &node, "nope")?;
            Ok(Lowering::Emit(body))
        },
    );
    let registry = registry(vec![foo()], vec![by_name]);
    let run = render(&registry, &["\\foo{b}"]);
    expect(diagnostics(&run), "flm.lowering.rule-failed").at("\\foo{b}").once();
}

// --- content slots through the façade ---------------------------------------------------------

#[test]
fn a_rule_stages_a_list_through_the_facade_with_its_label_and_item_slots() {
    let listing = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::LIST, pres::PAR]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let first_label = vec![lw.text(cx, "1.")?];
            let first_body = lw.lower_arg(cx, &node, 0)?;
            let first_item =
                vec![lw.pres_with(cx, &name(pres::PAR), ParPayload::default(), first_body)?];
            let second_label = vec![lw.text(cx, "2.")?];
            let second_body = vec![lw.text(cx, "second")?];
            let second_item =
                vec![lw.pres_with(cx, &name(pres::PAR), ParPayload::default(), second_body)?];
            let slots = vec![
                (String::from(pres::LABEL_SLOT), first_label),
                (String::from(pres::ITEM_SLOT), first_item),
                (String::from(pres::LABEL_SLOT), second_label),
                (String::from(pres::ITEM_SLOT), second_item),
            ];
            let list = lw.pres_slots_with(cx, &name(pres::LIST), ListPayload::default(), slots)?;
            Ok(Lowering::Emit(vec![list]))
        },
    );
    let registry = registry(vec![foo()], vec![listing]);
    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    let list = find(&lowered.tree, pres::LIST).expect("a list");
    let entries = list_entries(&list);
    let labels: Vec<String> = entries.iter().map(|(label, _)| plain_text(label.iter())).collect();
    let items: Vec<String> = entries.iter().map(|(_, item)| plain_text(item.iter())).collect();
    assert_eq!(labels, ["1.", "2."]);
    assert_eq!(items, ["b", "second"]);
}

#[test]
fn a_decoration_in_a_content_slot_is_hoisted_with_its_mark_naming_the_slot() {
    let listing = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[pres::LIST, pres::DECO_FINAL]),
        DefaultMark::Everywhere,
        |node, cx, lw| {
            let label = vec![lw.text(cx, "1.")?];
            let mut item = lw.lower_arg(cx, &node, 0)?;
            let glyph = vec![lw.text(cx, "□")?];
            item.push(lw.pres_with(cx, &name(pres::DECO_FINAL), DecorationPayload, glyph)?);
            let slots = vec![
                (String::from(pres::LABEL_SLOT), label),
                (String::from(pres::ITEM_SLOT), item),
            ];
            let list = lw.pres_slots_with(cx, &name(pres::LIST), ListPayload::default(), slots)?;
            Ok(Lowering::Emit(vec![list]))
        },
    );
    let registry = registry(vec![foo()], vec![listing]);
    let run = render(&registry, &["\\foo{b}"]);
    let lowered = lowered(&run, first());
    let list = find(&lowered.tree, pres::LIST).expect("a list");
    let (_, item) = list_entries(&list).into_iter().next().expect("one entry");
    let mark = item.iter().next_back().expect("the mark stands where the decoration stood");
    assert_eq!(element_of(&mark).name, pres::DECO_MARK);
    // The attached slot follows the two content slots.
    assert_eq!(payload_of::<DecoMarkPayload>(&mark).unwrap().slot, 2);
    let deco = decoration_at(&list, &mark).expect("the mark names the decoration");
    assert_eq!(body_text(&deco), "□");
}
