//! Lowering rules with bodies of an extension's own, the lowering façade,
//! presentation payloads from data and a tree staged in one call
//! (`api::core::lowering`), through the Rust façade and through the
//! dispatcher, over the standard package and a hand-made package.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::registry::PackageEntry;
use flm_core::dump::json::to_json;
use flm_core::stdcore::backends::{html, pres};
use flm_ext::adapter::payloads::{elements, payload_from_value};
use flm_ext::api::stdcore::backends::latex;
use flm_ext::api::core::lowering::{
    pres_tree, DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, NodeRegion, PresTree,
};
use flm_ext::api::core::data::table::TableValue;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::lang::mint::{ArgDef, BodyDef, ElementDef, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::techy::error::{Diagnostic, Severity};
use flm_ext::api::techy::node::Node;
use flm_ext::engine::SettingsReport;
use flm_ext::ExtError;
use serde_json::{json, Map, Value};

use common::double::{stage, Double};
use common::json_double::{module, JsonDouble};
use common::{html_of, latex_of, registry_with, run, text_of, try_registry_with};

/// A rule's body as a closure.
type BodyFn = dyn Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync;

/// A [`Lower`] whose work is a closure.
struct LowerFn(Box<BodyFn>);

impl Lower for LowerFn {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        (self.0)(node, cx, lw)
    }
}

/// The body whose work is `f`.
fn body(
    f: impl Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync + 'static,
) -> Arc<dyn Lower> {
    Arc::new(LowerFn(Box::new(f)))
}

/// The declaration of the macro `\lwbox{content}`, the element `lw.box`.
fn box_element(feature: &str) -> ElementDef {
    ElementDef::new(feature, ["lwbox"], SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] }, ElementSpec::new("lw.box"))
}

/// The package `lw`, whose feature `boxes` declares `\lwbox` and the rule
/// `lw.box.lower.default` of `decl`, `mark` and `body`.
fn boxes(decl: LoweringDecl, mark: DefaultMark, body: Arc<dyn Lower>) -> PackageEntry {
    Double::new("lw")
        .feature("boxes", move |b| {
            b.element(box_element(&b.feature()))?;
            b.lowering_rule(LoweringRule::new("lw.box.lower.default", "lw.box", decl.clone(), Arc::clone(&body), mark.clone()))
        })
        .package()
}

/// A span's payload of role `role`.
fn span(role: &str) -> Value {
    json!({ "role": role })
}

/// The body that stages a span of role `lw.box` holding a span of role
/// `lw.inner` over the lowered argument, then the text ` (boxed)`, as one
/// tree.
fn boxing() -> Arc<dyn Lower> {
    body(|node, cx, lw| {
        let content = lw.lower_arg(cx, node, 0)?;
        let tree = PresTree::Node {
            element: pres::SPAN.to_string(),
            payload: span("lw.box"),
            children: vec![
                PresTree::Node {
                    element: pres::SPAN.to_string(),
                    payload: span("lw.inner"),
                    children: content.into_iter().map(PresTree::Build).collect(),
                },
                PresTree::Text(" (boxed)".to_string()),
            ],
        };
        Ok(Lowering::Emit(vec![pres_tree(lw, cx, &tree)?]))
    })
}

fn produces_spans() -> LoweringDecl {
    LoweringDecl::new().produces(pres::SPAN)
}

// --- a rule of the extension's own ---------------------------------------------------------------

#[test]
fn a_rule_with_a_rust_body_lowers_on_each_backend() {
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, boxing())]);
    let source = "Before \\lwbox{inside \\emph{it}} after.";

    let (text, diagnostics) = text_of(&registry, source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Before inside it (boxed) after.\n");

    let (page, diagnostics) = html_of(&registry, source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains("inside <em"), "{page}");
    assert!(page.contains(" (boxed)</span>"), "{page}");

    let (latex, diagnostics) = latex_of(&registry, source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(latex.contains("Before inside \\emph{it} (boxed) after."), "{latex}");
}

#[test]
fn a_tree_staged_in_one_call_splices_a_build_identifier_and_a_text_leaf() {
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, boxing())]);
    let (page, diagnostics) = html_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The outer span holds the inner span (the argument's lowered nodes,
    // spliced by their build identifiers), then the text leaf.
    let outer = page.find("lw.box").or_else(|| page.find("lw-box")).expect("the outer span");
    let inner = page[outer..].find("lw.inner").or_else(|| page[outer..].find("lw-inner")).expect("the inner span");
    let leaf = page[outer..].find(" (boxed)").expect("the text leaf");
    assert!(inner < leaf, "{page}");
    assert!(page[outer..].contains(">x</span> (boxed)</span>"), "{page}");
}

#[test]
fn a_never_mark_is_accepted_and_the_rule_is_the_only_candidate() {
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Never, boxing())]);
    let (text, diagnostics) = text_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "x (boxed)\n");
}

#[test]
fn a_rule_marked_for_one_backend_lowers_there() {
    let registry = registry_with([boxes(produces_spans(), DefaultMark::OnBackends(vec![html::BACKEND.into()]), boxing())]);
    let (page, diagnostics) = html_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains(" (boxed)"), "{page}");
}

#[test]
fn a_failing_body_is_an_error_at_the_node_and_the_rest_lowers() {
    let failing = body(|_, _, _| Err(ExtError::Value { detail: "no box today".to_string() }));
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, failing)]);
    let (text, diagnostics) = text_of(&registry, "Before \\lwbox{x} after.");
    assert_eq!(
        diagnostics,
        ["flm.ext.handler-failed: the lowering rule `lw.box.lower.default` failed: no box today"],
    );
    // The node is the unsupported stand-in; the rest of the tree lowers.
    assert_eq!(text, "Before [unsupported: lw.box] after.\n");
}

#[test]
fn a_node_the_call_did_not_stage_or_placed_twice_is_the_body_s_failure() {
    let twice = body(|node, cx, lw| {
        let leaf = lw.text(cx, "a")?;
        let first = lw.pres(cx, pres::SPAN, &json!({}), vec![leaf])?;
        // The leaf is already inside `first`.
        let _ = lw.pres(cx, pres::SPAN, &json!({}), vec![leaf])?;
        let _ = node;
        Ok(Lowering::Emit(vec![first]))
    });
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, twice)]);
    let (_, diagnostics) = text_of(&registry, "\\lwbox{x}");
    assert_eq!(
        diagnostics,
        ["flm.ext.handler-failed: the lowering rule `lw.box.lower.default` failed: `pres`: a node may be placed only once"],
    );
}

#[test]
fn nodes_that_are_not_consecutive_siblings_are_refused_by_lower_nodes() {
    // The node twice; the node and the first node under it, one level apart.
    let refusals = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&refusals);
    let lowering = body(move |node, cx, lw| {
        let child = || node.descendants().into_iter().next().expect("the box holds its argument");
        let itself = || child().parent().expect("the node");
        for nodes in [vec![itself(), itself()], vec![itself(), child()]] {
            let refused = lw.lower_nodes(cx, &nodes).map(|_| ()).map_err(|error| error.to_string());
            seen.lock().expect("the lock").push(refused);
        }
        Ok(Lowering::Emit(vec![lw.text(cx, "box")?]))
    });
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, lowering)]);
    let (_, diagnostics) = text_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let refusal = Err("`lower_nodes`: the nodes must be consecutive siblings in one tree".to_string());
    assert_eq!(*refusals.lock().expect("the lock"), [refusal.clone(), refusal]);
}

#[test]
fn a_core_refusal_while_staging_ends_the_call_with_the_core_error() {
    // `flm.pres.par` is not declared as produced: `flm-core` refuses it, and
    // the rule's failure is `flm-core`'s, whatever the body answers next.
    let refused = body(|_, cx, lw| {
        let _ = lw.pres(cx, pres::PAR, &json!({}), Vec::new());
        Ok(Lowering::Default)
    });
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, refused)]);
    let (_, diagnostics) = text_of(&registry, "\\lwbox{x}");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.lowering.rule-failed: "), "{diagnostics:?}");
    assert!(diagnostics[0].contains("flm.pres.par"), "{diagnostics:?}");
}

#[test]
fn a_rule_named_outside_the_extension_is_refused() {
    let package = Double::new("lw")
        .feature("boxes", |b| {
            b.element(box_element(&b.feature()))?;
            b.lowering_rule(LoweringRule::new("other.lower", "lw.box", produces_spans(), boxing(), DefaultMark::Everywhere))
        })
        .package();
    let error = try_registry_with([package]).expect_err("the rule's name is outside the extension");
    assert!(
        error.to_string().contains(
            "the rule `other.lower` declared by the package `_` of the extension `lw` must begin with `lw.`"
        ),
        "{error}"
    );
}

// --- the façade's methods ------------------------------------------------------------------------

/// What a body saw, shared with the test.
type Log = Arc<Mutex<Vec<String>>>;

/// Records `line` into `log`.
fn note(log: &Log, line: String) {
    log.lock().expect("the log").push(line);
}

#[test]
fn every_method_of_the_lowerer_answers() {
    let log: Log = Arc::default();
    let seen = Arc::clone(&log);
    let probe = body(move |node, cx, lw| {
        let log = &seen;
        note(log, format!("rule {}", lw.rule()));
        note(log, format!("backend {}", lw.backend()));
        note(log, format!("comment_start {:?}", lw.comment_start()));
        note(log, format!("tree_id {}", lw.tree_id()));
        let addr = lw.addr();
        note(log, format!("addr_of {}", lw.addr_of(node)? == addr));
        note(log, format!("placement {:?}", lw.placement()));
        let rows = match lw.get("lw.rows")? {
            Some(flm_ext::api::core::data::table::TableValue::Ext(table)) => Value::Array(table.items()),
            _ => Value::Null,
        };
        note(log, format!("get {rows}"));
        let labels = matches!(lw.get("flm.labels")?, Some(flm_ext::api::core::data::table::TableValue::Projection(_)));
        note(log, format!("get flm.labels {labels}"));
        note(log, format!("producer_of {:?}", lw.producer_of("lw.rows")));
        note(log, format!("option {}", lw.option("lw.style")?));
        let undeclared = match lw.option("lw.other") {
            Err(ExtError::Core { kind, .. }) => Err(kind),
            other => Ok(other.map(|value| value.to_string()).unwrap_or_default()),
        };
        note(log, format!("option undeclared {undeclared:?}"));

        let content = node.argument_content_nodes(0).unwrap_or_default();
        let by_index = lw.lower_arg(cx, node, 0)?;
        let by_name = lw.lower_arg_named(cx, node, "content")?;
        let as_run = lw.lower_nodes(cx, &content)?;
        let as_region = lw.lower_region(cx, addr, NodeRegion::Argument(0))?;
        let emph = content.iter().find(|node| node.name().as_deref() == Some("emph")).expect("an \\emph");
        let copied = lw.default(cx, &**emph)?;
        let children = lw.lower_children(cx, &**emph)?;
        note(log, format!("lowered {} {} {} {} {} {}", by_index.len(), by_name.len(), as_run.len(), as_region.len(), copied.len(), children.len()));

        let a = lw.pres(cx, pres::SPAN, &span("lw.a"), by_index)?;
        let b = lw.pres_slots(cx, pres::SPAN, &span("lw.b"), vec![("body".to_string(), by_name)])?;
        let c = lw.pres(cx, pres::SPAN, &span("lw.c"), as_run)?;
        let d = lw.pres(cx, pres::SPAN, &span("lw.d"), as_region)?;
        let e = lw.pres(cx, pres::SPAN, &span("lw.e"), copied)?;
        let f = lw.pres(cx, pres::SPAN, &span("lw.f"), children)?;
        let g = lw.text(cx, "|")?;
        lw.require("lw.style-def");
        lw.diagnose(Diagnostic::new(Severity::Note, "lw.seen", "the probe ran"))?;
        let outside = lw.diagnose(Diagnostic::new(Severity::Note, "other.seen", "")).map_err(|error| error.to_string());
        note(log, format!("diagnose outside {outside:?}"));
        Ok(Lowering::Emit(vec![a, b, c, d, e, f, g]))
    });
    let collect = stage(
        StageDecl::new("lw.collect", Effect::Compute, Scope::Document).writes("lw.rows"),
        Needs::new(),
        |_, out| out.table("lw.rows")?.push(json!({ "row": 1 })),
    );
    let mut options = Map::new();
    options.insert("lw.style".to_string(), json!({ "bind": "document", "schema": { "type": "object", "properties": {} } }));
    let package = Double::new("lw")
        .feature("boxes", move |b| {
            b.element(box_element(&b.feature()))?;
            b.table("lw.rows", "What the stage collected.")?;
            b.stage(Arc::clone(&collect))?;
            let decl = produces_spans().reads("lw.rows").reads("flm.labels").option("lw.style");
            b.lowering_rule(
                LoweringRule::new("lw.box.lower.default", "lw.box", decl, Arc::clone(&probe), DefaultMark::Everywhere)
                    .docs("Probes the façade."),
            )
        })
        .settings("boxes", SettingsReport { options, ..SettingsReport::default() })
        .package();
    let registry = registry_with([package]);
    let ran = run(&registry, html::WORKFLOW, "\\lwbox{a \\emph{b}}");
    let identifiers = common::identifiers(&ran);
    assert!(identifiers.contains(&"lw.seen".to_string()), "{identifiers:?}");
    assert!(!identifiers.contains(&"flm.ext.handler-failed".to_string()), "{:?}", common::diagnostics(&ran));
    let log = log.lock().expect("the log").clone();
    assert_eq!(
        log,
        [
            "rule lw.box.lower.default",
            "backend html",
            "comment_start Some(\"%%\")",
            "tree_id tree#0",
            "addr_of true",
            "placement None",
            "get [{\"row\":1}]",
            "get flm.labels true",
            "producer_of Some(\"lw.collect\")",
            "option {}",
            "option undeclared Err(\"option\")",
            "lowered 2 2 2 2 1 1",
            "diagnose outside Err(\"the diagnostic `other.seen` declared by the package `_` of the extension `lw` must begin with `lw.`\")",
        ]
    );
}

#[test]
fn a_body_lowers_an_environment_s_body() {
    let element = ElementDef::new(
        "boxes",
        ["lwenv"],
        SyntaxDef::Environment { args: Vec::new(), body: BodyDef::new() },
        ElementSpec::new("lw.env"),
    );
    let lowering = body(|node, cx, lw| {
        let content = lw.lower_body(cx, node)?;
        Ok(Lowering::Emit(vec![lw.pres(cx, pres::SPAN, &span("lw.env"), content)?]))
    });
    let package = Double::new("lw")
        .feature("boxes", move |b| {
            b.element(element.clone())?;
            b.lowering_rule(LoweringRule::new("lw.env.lower.default", "lw.env", produces_spans(), Arc::clone(&lowering), DefaultMark::Everywhere))
        })
        .package();
    let registry = registry_with([package]);
    let (text, diagnostics) = text_of(&registry, "\\begin{lwenv}inside\\end{lwenv}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text.trim(), "inside");
}

#[test]
fn a_body_splices_a_copy_of_a_tree() {
    // In the document's tree, the body splices a copy of that tree; inside
    // the copy (a synthesized value) it writes text.
    let splicing = body(|node, cx, lw| {
        if lw.tree_id().starts_with("value#") {
            return Ok(Lowering::Emit(vec![lw.text(cx, "[copy]")?]));
        }
        let mut root = node.parent().expect("the node has a parent");
        while let Some(parent) = root.parent() {
            root = parent;
        }
        let refused = lw.splice(cx, node).map_err(|error| error.to_string());
        assert_eq!(refused, Err("`splice`: `value` must be the root node of its tree".to_string()));
        let copy = lw.splice(cx, &*root)?;
        Ok(Lowering::Emit(copy))
    });
    let registry = registry_with([boxes(produces_spans(), DefaultMark::Everywhere, splicing)]);
    let (text, diagnostics) = text_of(&registry, "A \\lwbox{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A A [copy]..\n");
}

// --- payloads from data ----------------------------------------------------------------------------

#[test]
fn a_payload_from_data_reads_back_for_every_element_of_the_table() {
    let samples = [
        (pres::PAR, json!({})),
        (pres::BLOCK, json!({ "role": "lw.r", "kinds": ["a"] })),
        (pres::BLOCK_HEADING, json!({ "name": "section", "level": 1, "in_toc": true })),
        (pres::RUNIN_HEADING, json!({ "name": "paragraph", "level": 4, "in_toc": false })),
        (pres::SPAN, json!({ "style": "emph", "role": "lw.r" })),
        (pres::LIST, json!({})),
        (pres::CELLS, json!({ "rows": 1, "cols": 1, "cells": [] })),
        (pres::LINES, json!({})),
        (pres::LINE, json!({ "indent_left": "0tab", "indent_right": "0tab", "align": "left" })),
        (pres::VERBATIM, json!({ "kind": "code", "display": "block" })),
        (pres::MATH_PASSTHROUGH, json!({ "display": "inline" })),
        (pres::MATH_TEXT, json!({})),
        (pres::LINK, json!({ "href": "https://example.org", "kind": "external" })),
        (pres::IMAGE, json!({ "src": "a.png", "display": "inline" })),
        (pres::UNSUPPORTED, json!({ "element": "lw.x", "message": "m" })),
        (pres::DROPPED, json!({ "condition": "lw.x", "detail": "d" })),
        (pres::UNKNOWN, json!({ "kind": "macro", "name": "foo", "spelling": "\\foo" })),
        (pres::DECO_RUNIN, json!({})),
        (pres::DECO_FINAL, json!({})),
        (pres::DECO_MARGIN, json!({})),
        (pres::DECO_MARK, json!({ "slot": 0 })),
        (pres::SUBTREE, json!({ "tree": "tree#1" })),
        (html::construct::TAG, json!({ "tag": "mrow", "attributes": [["class", "a"], ["id", "b"]], "block": true })),
        (latex::MACRO, json!({ "name": "section", "args": ["star", "mandatory"] })),
        (latex::ENVIRONMENT, json!({ "name": "exercise", "args": ["optional"], "verbatim": false })),
        (latex::RAW, json!({ "text": "\\par\n" })),
        (latex::DEFINITION, json!({ "kind": "macro", "name": "lwDef", "args": 1 })),
    ];
    assert_eq!(samples.iter().map(|(element, _)| *element).collect::<Vec<_>>(), elements().collect::<Vec<_>>());
    for (element, payload) in samples {
        let read = payload_from_value("pres", element, &payload).unwrap_or_else(|error| panic!("{element}: {error}"));
        let back: Value = serde_json::from_str(&to_json(&read.project()).expect("JSON")).expect("JSON");
        assert_eq!(back, payload, "{element}");
    }
}

#[test]
fn an_element_outside_the_table_is_refused_with_the_table_s_names() {
    let error = payload_from_value("pres", "lw.mine", &json!({})).expect_err("not in the table").to_string();
    assert!(
        error.starts_with(
            "`pres`: the element `lw.mine` must be one of the presentation elements whose payload can be given as \
             data: `flm.pres.par`, `flm.pres.block`, "
        ),
        "{error}"
    );
    assert!(
        error.ends_with(
            "`flm.pres.subtree`, `flm.pres.html.tag`, `flm.pres.latex.macro`, `flm.pres.latex.environment`, \
             `flm.pres.latex.raw`, `flm.pres.latex.definition`, or a presentation element a package declared with a \
             data payload that this rule declares it produces"
        ),
        "{error}"
    );
    let markup = payload_from_value("pres", html::construct::TAG, &json!({ "tag": "b", "markup": "<i>x</i>" }))
        .expect_err("the markup body is not offered")
        .to_string();
    assert_eq!(markup, "`pres`: the payload of `flm.pres.html.tag` must not hold `markup`");
}

#[test]
fn a_tag_with_children_is_staged_from_data_on_html() {
    let tagging = body(|node, cx, lw| {
        let content = lw.lower_arg(cx, node, 0)?;
        let tree = PresTree::Node {
            element: html::construct::TAG.to_string(),
            payload: json!({ "tag": "mrow", "attributes": [["class", "lw"]] }),
            children: vec![PresTree::Node {
                element: html::construct::TAG.to_string(),
                payload: json!({ "tag": "mtext" }),
                children: content.into_iter().map(PresTree::Build).collect(),
            }],
        };
        Ok(Lowering::Emit(vec![pres_tree(lw, cx, &tree)?]))
    });
    let decl = LoweringDecl::new().produces(html::construct::TAG);
    let registry = registry_with([boxes(decl, DefaultMark::OnBackends(vec![html::BACKEND.into()]), tagging)]);
    let (page, diagnostics) = html_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains("<mrow class=\"lw\"><mtext>x</mtext></mrow>"), "{page}");
}

// --- through the dispatcher ------------------------------------------------------------------------

#[test]
fn a_rule_built_and_lowered_through_the_dispatcher() {
    let double = JsonDouble::new("lw").feature("boxes", |js, b| {
        js.call(
            b,
            "element",
            json!([{
                "owner": "boxes",
                "names": ["lwbox"],
                "syntax": { "macro": { "args": [{ "name": "content", "code": "{" }] } },
                "element": { "name": "lw.box" },
            }]),
        )?;
        let lower = js.lend_lower(|js, node, cx, lw| {
            let backend = js.call(lw, "backend", json!([]))?;
            let content = js.call(lw, "lower_arg", json!([cx, node, 0]))?;
            let tree = json!({
                "element": "flm.pres.span",
                "payload": { "role": "lw.box" },
                "children": [
                    { "element": "flm.pres.span", "children": content.as_array().expect("a list").iter().map(|id| json!({ "build": id })).collect::<Vec<_>>() },
                    { "text": format!(" on {}", backend.as_str().unwrap_or("?")) },
                ],
            });
            let root = js.call(&module("core/lowering"), "pres_tree", json!([lw, cx, tree]))?;
            Ok(json!({ "emit": [root] }))
        });
        let decl = json!({ "produces": ["flm.pres.span"] });
        let rule = js.call(&module("core/lowering"), "LoweringRule::new", json!(["lw.box.lower.default", "lw.box", decl, lower, "everywhere"]))?;
        let rule = js.call(&rule, "docs", json!(["Boxes, through the dispatcher."]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let (text, diagnostics) = text_of(&registry, "A \\lwbox{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A x on text.\n");
    let (latex, diagnostics) = latex_of(&registry, "A \\lwbox{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(latex.contains("A x on latex-recomposed."), "{latex}");
}

#[test]
fn the_dispatcher_offers_the_lowerer_s_methods() {
    let log: Log = Arc::default();
    let seen = Arc::clone(&log);
    let double = JsonDouble::new("lw").feature("boxes", move |js, b| {
        js.call(
            b,
            "element",
            json!([{
                "owner": "boxes",
                "names": ["lwbox"],
                "syntax": { "environment": { "args": [{ "name": "content", "code": "{" }], "body": {} } },
                "element": { "name": "lw.box" },
            }]),
        )?;
        // A table of the extension's own, which a stage writes: an item at
        // each box, one at another address, and one that is not a map.
        js.call(b, "table", json!(["lw.rows", "rows by address"]))?;
        let stage = js.lend_stage(
            &json!({ "name": "lw.rows", "effect": "compute", "scope": "document", "writes": ["lw.rows"] }),
            &json!({}),
            |js, cx, out| {
                let piece = js.call(out, "table", json!(["lw.rows"]))?;
                let walk = js.call(cx, "walk", json!([]))?;
                for entry in walk.as_array().expect("a list") {
                    let name = js.call(&entry["node"], "element_name", json!([]))?;
                    let n = if name == json!("lw.box") { 1 } else { 0 };
                    js.call(&piece, "push", json!([{ "addr": entry["addr"], "n": n }]))?;
                }
                js.call(&piece, "push", json!(["loose"]))?;
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([stage]))?;
        let seen = Arc::clone(&seen);
        let lower = js.lend_lower(move |js, node, cx, lw| {
            // In the spliced copy (a synthesized value), the body writes text.
            let tree = js.call(lw, "tree_id", json!([]))?;
            if tree.as_str().is_some_and(|id| id.starts_with("value#")) {
                return Ok(json!({ "emit": [js.call(lw, "text", json!([cx, "[copy]"]))?] }));
            }
            let mut answers = Vec::new();
            for method in ["rule", "backend", "comment_start", "tree_id", "addr", "placement"] {
                answers.push(js.call(lw, method, json!([]))?);
            }
            answers.push(js.call(lw, "addr_of", json!([node]))?);
            answers.push(js.call(lw, "producer_of", json!(["lw.none"]))?);
            let content = js.call(node, "argument_content_nodes", json!([0]))?;
            let staged = vec![
                js.call(lw, "lower_arg", json!([cx, node, 0]))?,
                js.call(lw, "lower_arg_named", json!([cx, node, "content"]))?,
                js.call(lw, "lower_nodes", json!([cx, content]))?,
                js.call(lw, "lower_children", json!([cx, node]))?,
            ];
            let mut spans = Vec::new();
            for ids in staged {
                spans.push(js.call(lw, "pres", json!([cx, "flm.pres.span", {}, ids]))?);
            }
            let bar = js.call(lw, "text", json!([cx, "|"]))?;
            spans.push(js.call(lw, "pres_slots", json!([cx, "flm.pres.span", {}, [["body", [bar]]]]))?);
            // S29 and S28: the reads, the option, `default`, `lower_body`,
            // `splice` through the dispatcher.
            let rows = js.call(lw, "get", json!(["lw.rows"]))?;
            answers.push(js.call(&rows["ext"], "items", json!([]))?);
            answers.push(js.call(lw, "get_at", json!(["lw.rows", node]))?);
            answers.push(js.call(lw, "option", json!(["lw.style"]))?);
            let first = content.as_array().and_then(|nodes| nodes.first()).cloned().expect("a content node");
            let defaulted = js.call(lw, "default", json!([cx, first]))?;
            spans.push(js.call(lw, "pres", json!([cx, "flm.pres.span", {}, defaulted]))?);
            let body = js.call(lw, "lower_body", json!([cx, node]))?;
            spans.push(js.call(lw, "pres", json!([cx, "flm.pres.span", {}, body]))?);
            let mut root = node.clone();
            while let Value::Object(_) = js.call(&root, "parent", json!([]))? {
                root = js.call(&root, "parent", json!([]))?;
            }
            let copy = js.call(lw, "splice", json!([cx, root]))?;
            spans.push(js.call(lw, "pres", json!([cx, "flm.pres.span", {}, copy]))?);
            js.call(lw, "require", json!(["lw.style-def"]))?;
            js.call(lw, "diagnose", json!([{ "id": "lw.seen", "severity": "note", "message": "seen" }]))?;
            let refused = js.call(lw, "pres", json!([cx, "lw.mine", {}, []])).map_err(|error| error.to_string());
            answers.push(json!(refused));
            seen.lock().expect("the log").push(Value::Array(answers).to_string());
            Ok(json!({ "emit": spans }))
        });
        let rule = js.call(
            &module("core/lowering"),
            "LoweringRule::new",
            json!([
                "lw.box.lower.default",
                "lw.box",
                { "produces": ["flm.pres.span"], "reads": ["lw.rows"], "options": ["lw.style"] },
                lower,
                { "on_backends": ["text"] }
            ]),
        )?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    let mut options = Map::new();
    options.insert("lw.style".to_string(), json!({ "bind": "document", "schema": { "type": "object", "properties": {} } }));
    let double = double.settings("boxes", SettingsReport { options, ..SettingsReport::default() });
    let registry = registry_with([double.package()]);
    let ran = run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "\\begin{lwbox}{a}b\\end{lwbox}");
    assert!(common::identifiers(&ran).contains(&"lw.seen".to_string()), "{:?}", common::diagnostics(&ran));
    let log = log.lock().expect("the log").clone();
    assert_eq!(log.len(), 1, "{log:?}");
    let answers: Value = serde_json::from_str(&log[0]).expect("JSON");
    assert_eq!(answers[0], json!("lw.box.lower.default"));
    assert_eq!(answers[1], json!("text"));
    assert_eq!(answers[2], json!("%%"));
    assert_eq!(answers[3], json!("tree#0"));
    assert_eq!(answers[4]["tree"], json!("tree#0"));
    assert_eq!(answers[5], Value::Null);
    assert_eq!(answers[6], answers[4]);
    assert_eq!(answers[7], Value::Null);
    let rows = answers[8].as_array().expect("the items");
    assert_eq!(rows.last(), Some(&json!("loose")), "{rows:?}");
    assert_eq!(rows.iter().filter(|row| row["n"] == json!(1)).count(), 1, "{rows:?}");
    assert_eq!(answers[9], json!([{ "addr": answers[4], "n": 1 }]));
    assert_eq!(answers[10], json!({}));
    let refused = answers[11]["Err"].as_str().expect("refused");
    assert!(refused.starts_with("`pres`: the element `lw.mine` must be one of"), "{refused}");
    let text = ran
        .table(flm_core::stdcore::backends::text::TEXT_OUTPUT)
        .expect("declared")
        .map(|out| out.text.clone())
        .unwrap_or_default();
    // `lower_arg`, `lower_arg_named`, `lower_nodes`: `a`; `lower_children`:
    // `ab`; the text `|`; `default` of the argument's characters: `a`;
    // `lower_body`: `b`; `splice` of the document's copy: `[copy]`.
    assert_eq!(text, "aaaab|ab[copy]\n");
}

// --- a presentation element with a data payload, and the lowered trees read -----------------------

/// The package `lw` of the data-payload tests: `\lwnote{content}` (the element
/// `lw.note`), the presentation elements `lw.pres.note` and `lw.pres.other`
/// declared with a data payload, a rule staging `lw.pres.note` with the payload
/// `{ kind: "tip", level: 2 }` over the lowered argument, a rule lowering
/// `lw.pres.note` on the text backend (which does not write it natively) into
/// a span that reads the payload back, and a stage reading the text backend's
/// lowered trees by name into the table `lw.seen`. What the handlers see is
/// logged.
fn noting(log: &Log) -> PackageEntry {
    let log = Arc::clone(log);
    Double::new("lw")
        .feature("notes", move |b| {
            b.element(ElementDef::new(
                b.feature(),
                ["lwnote"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
                ElementSpec::new("lw.note"),
            ))?;
            b.presentation_element("lw.pres.note", "A note, with its kind and level as data.")?;
            b.presentation_element("lw.pres.other", "Another element with a data payload.")?;
            let staging = Arc::clone(&log);
            let stage_note = body(move |node, cx, lw| {
                let content = lw.lower_arg(cx, node, 0)?;
                // An element declared with a data payload that the rule does not
                // produce, and an element nobody declared.
                for element in ["lw.pres.other", "lw.pres.never"] {
                    let refused = lw.pres(cx, element, &json!({}), Vec::new()).map_err(|error| error.to_string());
                    if let Err(refused) = refused {
                        note(&staging, refused);
                    }
                }
                let staged = lw.pres(cx, "lw.pres.note", &json!({ "kind": "tip", "level": 2 }), content)?;
                Ok(Lowering::Emit(vec![staged]))
            });
            let decl = LoweringDecl::new().produces("lw.pres.note");
            b.lowering_rule(LoweringRule::new("lw.note.lower", "lw.note", decl, stage_note, DefaultMark::Everywhere))?;
            let reading = Arc::clone(&log);
            let read_note = body(move |node, cx, lw| {
                let payload = node.payload()?.unwrap_or(Value::Null);
                note(&reading, format!("{} {payload}", node.element_name()));
                let kind = payload["kind"].as_str().unwrap_or("?").to_string();
                let mut content = vec![lw.text(cx, &format!("[{kind}] "))?];
                content.extend(lw.lower_children(cx, node)?);
                Ok(Lowering::Emit(vec![lw.pres(cx, pres::SPAN, &json!({ "role": "lw.note" }), content)?]))
            });
            let decl = LoweringDecl::new().produces(pres::SPAN);
            b.lowering_rule(LoweringRule::new("lw.pres.note.lower", "lw.pres.note", decl, read_note, DefaultMark::Everywhere))?;
            b.table("lw.seen", "What the reader of the lowered trees saw.")?;
            let seeing = Arc::clone(&log);
            let decl = StageDecl::new("lw.read", Effect::Compute, Scope::Document).writes("lw.seen");
            b.stage(stage(decl, Needs::new().reads("flm.lowered.text"), move |cx, _| {
                let Some(TableValue::Lowered(lowered)) = cx.get("flm.lowered.text")? else {
                    note(&seeing, "not the lowered trees".to_string());
                    return Ok(());
                };
                for (id, root) in lowered.trees() {
                    let spans: Vec<Value> = root
                        .descendants()
                        .iter()
                        .filter(|node| node.element_name() == pres::SPAN)
                        .map(|node| node.payload().ok().flatten().unwrap_or(Value::Null))
                        .collect();
                    note(
                        &seeing,
                        format!(
                            "{id} {} rounds {:?} requires {:?} same {} spans {}",
                            root.element_name(),
                            lowered.rounds(&id),
                            lowered.requires(&id),
                            lowered.tree(&id).map(|tree| tree.span()) == Some(root.span()),
                            Value::Array(spans),
                        ),
                    );
                }
                note(&seeing, format!("unknown {:?} {:?}", lowered.tree("tree#9").is_none(), lowered.rounds("nothing")));
                Ok(())
            }))
        })
        .package()
}

#[test]
fn an_element_declared_with_a_data_payload_is_staged_from_data_and_read_back() {
    use flm_core::stdcore::backends::text;
    let log: Log = Arc::default();
    let registry = registry_with([noting(&log)]);
    let ran = run_demanding(&registry, &[text::TEXT_OUTPUT.name(), "lw.seen"], "A \\lwnote{x}.");
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    let out = ran.table(text::TEXT_OUTPUT).expect("declared").map(|out| out.text.clone()).unwrap_or_default();
    assert_eq!(out, "A [tip] x.\n");
    let log = log.lock().expect("the log").clone();
    let refusal = |element: &str| {
        format!(
            "`pres`: the element `{element}` must be one of the presentation elements whose payload can be given as \
             data: {}, or a presentation element a package declared with a data payload that this rule declares it \
             produces",
            flm_ext::adapter::payloads::elements().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
        )
    };
    assert_eq!(
        log,
        [
            refusal("lw.pres.other"),
            refusal("lw.pres.never"),
            r#"lw.pres.note {"kind":"tip","level":2}"#.to_string(),
            r#"tree#0 flm.container rounds Some(2) requires Some([]) same true spans [{"role":"lw.note"}]"#.to_string(),
            "unknown true None".to_string(),
        ]
    );
}

/// The run of a plan demanding `demands` over the one-tree document `text`.
fn run_demanding(registry: &Arc<flm_core::core::registry::FlmRegistry>, demands: &[&str], text: &str) -> flm_core::core::run::Run {
    use flm_core::core::plan::{Plan, PlanRequest};
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
    use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

// --- the LaTeX family's constructs from data ------------------------------------------------------

/// The style file the recomposed LaTeX backend writes for `source`.
fn style_of(registry: &Arc<flm_core::core::registry::FlmRegistry>, source: &str) -> String {
    use flm_core::stdcore::backends::latex_recomposed;
    let run = run(registry, latex_recomposed::WORKFLOW, source);
    run.table(latex_recomposed::DOCUMENT)
        .expect("declared")
        .and_then(latex_recomposed::style_artifact)
        .and_then(|style| style.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned()))
        .unwrap_or_default()
}

#[test]
fn the_latex_constructs_are_staged_from_data_and_written() {
    let writing = body(|node, cx, lw| {
        let content = lw.lower_arg(cx, node, 0)?;
        let title = lw.lower_arg(cx, node, 0)?;
        let environment = lw.pres_slots(
            cx,
            latex::ENVIRONMENT,
            &json!({ "name": "lwenv", "args": ["optional"], "verbatim": false }),
            vec![(latex::ARG_SLOT.to_string(), title), (pres::BODY_SLOT.to_string(), content)],
        )?;
        let raw = lw.pres(cx, latex::RAW, &json!({ "text": "\\lwRaw{}" }), vec![])?;
        let label = lw.text(cx, "a")?;
        let mac = lw.pres_slots(cx, latex::MACRO, &json!({ "name": "lwmark", "args": ["mandatory"] }), vec![(latex::ARG_SLOT.to_string(), vec![label])])?;
        let body = lw.text(cx, "#1!")?;
        let definition = lw.pres(cx, latex::DEFINITION, &json!({ "kind": "macro", "name": "lwmark", "args": 1 }), vec![body])?;
        Ok(Lowering::Emit(vec![environment, raw, mac, definition]))
    });
    let decl = LoweringDecl::new().produces(latex::ENVIRONMENT).produces(latex::RAW).produces(latex::MACRO).produces(latex::DEFINITION);
    let registry = registry_with([boxes(decl, DefaultMark::OnBackends(vec![latex::RECOMPOSED_BACKEND.into()]), writing)]);
    let (document, diagnostics) = latex_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(document.contains("\\begin{lwenv}[x]x\\end{lwenv}"), "{document}");
    assert!(document.contains("\\lwRaw{}"), "{document}");
    assert!(document.contains("\\lwmark{a}"), "{document}");
    let style = style_of(&registry, "\\lwbox{x}");
    assert!(style.contains("lwmark"), "{style}");
}

// --- workflow lowering preferences --------------------------------------------------------------

#[test]
fn workflow_preferences_override_native_handling_and_replace_the_whole_map() {
    let registry = registry_with([Double::new("lw").feature("preferences", |b| {
        b.peer_element(pres::VERBATIM, "Verbatim input.")?;
        b.peer_element(pres::SPAN, "Replacement output.")?;
        b.lowering_rule(LoweringRule::new(
            "lw.verbatim", pres::VERBATIM, LoweringDecl::new().produces(pres::SPAN),
            body(|_, cx, lw| {
                let text = lw.text(cx, "replacement")?;
                Ok(Lowering::Emit(vec![lw.pres(cx, pres::SPAN, &json!({}), vec![text])?]))
            }), DefaultMark::Never,
        ))?;
        b.workflow_lowering("html", &json!({"html": {"prefer": ["lw.verbatim"]}, "text": {}}))?;
        b.workflow_lowering("html", &json!({"html": {"prefer": ["lw.verbatim"]}}))?;
        b.workflow_lowering("html", &json!({}))
    }).package()]);
    let workflow = &registry.workflow("html").unwrap().def;
    assert_eq!(workflow.lowering.len(), 1);
    assert_eq!(workflow.outputs.iter().map(|name| name.as_str()).collect::<Vec<_>>(), ["html"]);
    let (html, diagnostics) = html_of(&registry, "\\verbcode[python]|original|");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(html.contains("replacement") && !html.contains("original"), "{html}");
    let (text, diagnostics) = text_of(&registry, "\\verbcode[python]|original|");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(text.contains("original"), "{text}");
}

#[test]
fn workflow_lowering_rejects_malformed_data_without_registering_it() {
    let registry = registry_with([Double::new("lw").feature("preferences", |b| {
        for invalid in [json!(null), json!([]), json!({"html": []}),
            json!({"html": {"prefer": "rule"}}), json!({"html": {"prefer": [1]}}),
            json!({"html": {"unknown": []}})] {
            assert!(matches!(b.workflow_lowering("html", &invalid), Err(ExtError::Value { .. })), "{invalid}");
        }
        Ok(())
    }).package()]);
    assert!(registry.workflow("html").unwrap().def.lowering.is_empty());
}
