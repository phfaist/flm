//! Stages (plan A, WP-3): a scripted stage reads the document's trees, the
//! tables it declared it reads (an extension's own, a rewrite's trees, any
//! other as its projection), writes its own tables and reports diagnostics; a
//! failing handler is an Error at the stage, and the run completes.

mod common;

use std::sync::Arc;

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_ext::adapter::table::ExtTable;
use flm_core::stdcore::backends::pres;
use flm_ext::api::core::data::table::TableValue;
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::lang::rewrites::userdefs::{Definition, UserDefinition};
use flm_ext::api::lang::text::chars_text;
use flm_ext::api::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_ext::api::techy::error::Diagnostic;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::{registry_with, try_registry_with};

/// The run of a plan demanding `demands` over a document of one tree per text
/// of `texts`, in order.
fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
    let mut content = DocContent::new();
    for text in texts {
        let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
fn items(run: &Run, name: &str) -> Vec<Value> {
    run.table(TableHandle::<ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// The registry of the standard package and of the package `wc`, whose one
/// feature `count` declares the table `wc.words` and the stage `decl` (which
/// must write it), needing `needs` and running `run`.
fn one_stage(
    decl: StageDecl,
    needs: Needs,
    run: impl Fn(&dyn flm_ext::api::core::run::StageCx, &mut dyn flm_ext::api::core::data::store::StageOut) -> Result<(), ExtError>
        + Send
        + Sync
        + 'static,
) -> Arc<FlmRegistry> {
    let stage = stage(decl, needs, run);
    registry_with([Double::new("wc")
        .feature("count", move |b| {
            b.table("wc.words", "What the stage found.")?;
            b.stage(Arc::clone(&stage))
        })
        .package()])
}

/// The declaration of a stage `wc.count` of scope `scope` writing `wc.words`.
fn count_decl(scope: Scope) -> StageDecl {
    StageDecl::new("wc.count", Effect::Compute, scope).writes("wc.words")
}

/// The number of words of the characters under `node`.
fn words_under(node: &dyn Node<'_>) -> usize {
    node.descendants().iter().filter_map(|node| node.chars()).map(|chars| chars.split_whitespace().count()).sum()
}

// --- case 1 ------------------------------------------------------------------------------------

#[test]
fn a_per_tree_stage_counts_words_into_its_table() {
    let registry = one_stage(count_decl(Scope::PerTree), Needs::new(), |cx, out| {
        assert_eq!(cx.scope(), Scope::PerTree);
        let tree = cx.tree_id().expect("a per-tree stage has a tree");
        let root = cx.my_tree().expect("its tree");
        // The walk covers every tree; this tree's part of it counts the same.
        let walked: usize = cx
            .walk()
            .iter()
            .filter(|entry| entry.addr.tree.to_string() == tree)
            .filter_map(|entry| entry.node.chars())
            .map(|chars| chars.split_whitespace().count())
            .sum();
        assert_eq!(walked, words_under(&*root));
        out.table("wc.words")?.push(json!({ "tree": tree, "words": words_under(&*root) }))
    });
    let run = run_demanding(&registry, &["wc.words"], &["One two \\emph{three}", "Four five"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let mut found = items(&run, "wc.words");
    found.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
    assert_eq!(found, [json!({ "tree": "tree#0", "words": 3 }), json!({ "tree": "tree#1", "words": 2 })]);
}

/// The package `wc` of case 1's marker: `\todo{text}` expands to nothing, and
/// a document-scope stage collects each marker's text into `wc.todos` from the
/// trees after the expansion of the `always` definitions only
/// (`flm.userdefs.always-expanded`), which still hold the marker, since its
/// definition does not have the `always` policy.
fn todos() -> Double {
    Double::new("wc").feature("todo", |b| {
        let definition = Definition::macro_("todo", vec![ArgDef::mandatory("text")]).content("");
        let name = element_name_under("wc", &definition);
        let definition = UserDefinition::new(definition);
        b.lowering_rule(expand_in_tree_rule_for(&name, definition.definition()))?;
        b.element(definition.element_def(&b.feature(), &name))?;
        b.table("wc.todos", "The text of each marker, in document order.")?;
        let decl = StageDecl::new("wc.collect", Effect::Compute, Scope::Document).writes("wc.todos");
        b.stage(stage(decl, Needs::new().reads("flm.userdefs.always-expanded"), |cx, out| {
            let Some(TableValue::Trees(trees)) = cx.get("flm.userdefs.always-expanded")? else {
                return Err(ExtError::Handler { detail: "no trees".to_string(), data: Value::Null });
            };
            for (tree, root) in trees.trees() {
                for node in root.descendants() {
                    if node.name().as_deref() == Some("todo") {
                        let text = node.argument_content_nodes_named("text")?.and_then(|nodes| chars_text(&nodes));
                        out.table("wc.todos")?.push(json!({ "tree": tree, "text": text }))?;
                    }
                }
            }
            Ok(())
        }))
    })
}

#[test]
fn a_stage_collects_the_markers_from_the_trees_before_the_per_plan_definitions_expand() {
    let registry = registry_with([todos().package()]);
    let texts = ["A \\todo{check this} B.\n\nC \\todo{and that}.", "D \\todo{last}."];
    let expected = [
        json!({ "tree": "tree#0", "text": "check this" }),
        json!({ "tree": "tree#0", "text": "and that" }),
        json!({ "tree": "tree#1", "text": "last" }),
    ];
    // The default plan, and the same with the per-plan definitions expanded
    // explicitly: the markers are read before that expansion.
    for demands in [&["wc.todos"][..], &["wc.todos", "flm.userdefs.expanded"]] {
        let run = run_demanding(&registry, demands, &texts);
        assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
        assert_eq!(items(&run, "wc.todos"), expected, "{demands:?}");
    }
    // The marker expands to nothing in the output.
    let (text, diagnostics) = common::text_of(&registry, "A \\todo{check this} B.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A B.\n");
}

// --- reading tables ----------------------------------------------------------------------------

#[test]
fn a_stage_reads_another_scripted_feature_s_table() {
    let counter = stage(count_decl(Scope::PerTree), Needs::new(), |cx, out| {
        let root = cx.my_tree().expect("its tree");
        out.table("wc.words")?.push(json!(words_under(&*root)))
    });
    let report = StageDecl::new("wc.report", Effect::Compute, Scope::Document).writes("wc.total");
    let reporter = stage(report, Needs::new().reads("wc.words"), |cx, out| {
        let Some(TableValue::Ext(words)) = cx.get("wc.words")? else {
            return Err(ExtError::Handler { detail: "no table".to_string(), data: Value::Null });
        };
        let total: i64 = words.items().iter().filter_map(Value::as_i64).sum();
        out.table("wc.total")?.push(json!(total))
    });
    let double = Double::new("wc")
        .feature("count", move |b| {
            b.table("wc.words", "")?;
            b.stage(Arc::clone(&counter))
        })
        .feature("report", move |b| {
            b.table("wc.total", "")?;
            b.stage(Arc::clone(&reporter))
        });
    let registry = registry_with([double.package()]);
    let run = run_demanding(&registry, &["wc.total"], &["One two three.", "Four five."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(items(&run, "wc.total"), [json!(5)]);
}

#[test]
fn a_stage_reads_a_standard_table_as_its_projection() {
    // `flm.userdefs.always-expanded`, which plan A names, holds a rewrite's
    // trees and is answered as such; the labels table is a table of another
    // type, holding node addresses its projection renders.
    let decl = StageDecl::new("wc.count", Effect::Compute, Scope::Document).writes("wc.words");
    let registry = one_stage(decl, Needs::new().reads("flm.labels").reads("flm.userdefs.always-expanded"), |cx, out| {
        let Some(TableValue::Projection(labels)) = cx.get("flm.labels")? else {
            return Err(ExtError::Handler { detail: "no projection".to_string(), data: Value::Null });
        };
        out.table("wc.words")?.push(labels)?;
        let trees = matches!(cx.get("flm.userdefs.always-expanded")?, Some(TableValue::Trees(_)));
        out.table("wc.words")?.push(json!({ "always-expanded is trees": trees }))
    });
    let run = run_demanding(&registry, &["wc.words"], &["\\section{Intro}\\label{sec:intro}\n\nText."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let found = items(&run, "wc.words");
    assert_eq!(found.len(), 2, "{found:?}");
    let labels = found[0].to_string();
    assert!(labels.contains("sec:intro"), "{labels}");
    // A node address renders as `elem@line:col`, through the stage's context.
    assert!(labels.contains("@1:"), "{labels}");
    assert_eq!(found[1], json!({ "always-expanded is trees": true }));
}

#[test]
fn a_read_the_stage_did_not_declare_is_the_handler_s_failure() {
    let registry = one_stage(count_decl(Scope::Document), Needs::new(), |cx, _| cx.get("flm.labels").map(|_| ()));
    let run = run_demanding(&registry, &["wc.words"], &["Text."]);
    let diagnostics = common::diagnostics(&run);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.ext.handler-failed: the stage `wc.count` failed: "), "{diagnostics:?}");
    assert!(diagnostics[0].contains("flm.labels"), "{diagnostics:?}");
}

// --- reading trees -----------------------------------------------------------------------------

#[test]
fn a_document_stage_walks_every_tree() {
    let registry = one_stage(count_decl(Scope::Document), Needs::new(), |cx, out| {
        assert_eq!(cx.scope(), Scope::Document);
        assert_eq!(cx.tree_id(), None);
        assert!(cx.my_tree().is_none());
        let trees: Vec<String> = cx.trees().into_iter().map(|(id, _)| id).collect();
        out.table("wc.words")?.push(json!(trees))?;
        for entry in cx.walk() {
            if let Some(chars) = entry.node.chars() {
                out.table("wc.words")?.push(json!({ "tree": entry.addr.tree.to_string(), "chars": chars }))?;
            }
        }
        Ok(())
    });
    let run = run_demanding(&registry, &["wc.words"], &["One \\emph{two}.", "Three."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        items(&run, "wc.words"),
        [
            json!(["tree#0", "tree#1"]),
            json!({ "tree": "tree#0", "chars": "One " }),
            json!({ "tree": "tree#0", "chars": "two" }),
            json!({ "tree": "tree#0", "chars": "." }),
            json!({ "tree": "tree#1", "chars": "Three." }),
        ]
    );
}

#[test]
fn a_stage_reads_a_tree_by_id() {
    let registry = one_stage(count_decl(Scope::Document), Needs::new(), |cx, out| {
        for (id, root) in cx.trees() {
            let tree = cx.tree(&id).expect("the tree by its id");
            let frozen = cx.frozen_tree(&id).expect("the frozen tree by its id");
            assert_eq!(tree.span(), root.span());
            assert_eq!(frozen.span(), root.span());
            out.table("wc.words")?.push(json!({ "tree": id, "text": tree.span_content() }))?;
        }
        for id in ["tree#7", "value#0", "tree#+0", "tree", ""] {
            assert!(cx.tree(id).is_none(), "{id}");
            assert!(cx.frozen_tree(id).is_none(), "{id}");
        }
        Ok(())
    });
    let run = run_demanding(&registry, &["wc.words"], &["First.", "Second."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        items(&run, "wc.words"),
        [json!({ "tree": "tree#0", "text": "First." }), json!({ "tree": "tree#1", "text": "Second." })]
    );
}

// --- failures ----------------------------------------------------------------------------------

/// The span range of each of `run`'s diagnostics, with its identifier and
/// message.
fn placed(run: &Run) -> Vec<(String, String, std::ops::Range<usize>)> {
    run.diagnostics().iter().map(|d| (d.identifier().to_string(), d.message(), d.span().range())).collect()
}

#[test]
fn a_failing_run_handler_is_an_error_at_the_stage_and_the_run_completes() {
    let registry = one_stage(count_decl(Scope::PerTree), Needs::new(), |cx, out| {
        out.table("wc.words")?.push(json!(cx.tree_id()))?;
        Err(ExtError::Handler { detail: "TypeError: x is undefined".to_string(), data: json!({ "line": 3 }) })
    });
    let run = run_demanding(&registry, &["wc.words"], &["First tree.", "Second."]);
    let message = "the stage `wc.count` failed: TypeError: x is undefined".to_string();
    assert_eq!(
        placed(&run),
        [
            ("flm.ext.handler-failed".to_string(), message.clone(), 0..11),
            ("flm.ext.handler-failed".to_string(), message, 0..7),
        ]
    );
    assert!(run.diagnostics().iter().all(|d| d.severity() == flm_ext::api::techy::error::Severity::Error));
    // What the handler pushed before failing stays pushed.
    assert_eq!(items(&run, "wc.words"), [json!("tree#0"), json!("tree#1")]);
}

#[test]
fn a_write_to_an_undeclared_table_is_the_handler_s_failure() {
    let registry = one_stage(count_decl(Scope::PerTree), Needs::new(), |_, out| out.table("wc.other")?.push(json!(1)));
    let run = run_demanding(&registry, &["wc.words"], &["Text."]);
    let diagnostics = common::diagnostics(&run);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.ext.handler-failed: the stage `wc.count` failed: "), "{diagnostics:?}");
    assert!(diagnostics[0].contains("wc.other"), "{diagnostics:?}");
}

#[test]
fn a_diagnostic_with_an_id_outside_the_package_is_the_handler_s_failure() {
    let registry = one_stage(count_decl(Scope::PerTree), Needs::new(), |_, out| {
        out.diags(Diagnostic::warning("other.found", "something was found"))
    });
    let run = run_demanding(&registry, &["wc.words"], &["Text."]);
    assert_eq!(
        common::diagnostics(&run),
        ["flm.ext.handler-failed: the stage `wc.count` failed: the diagnostic `other.found` declared by the package \
          `_` of the extension `wc` must begin with `wc.`"]
    );
}

#[test]
fn a_diagnostic_at_a_node_carries_that_node_s_span() {
    let registry = one_stage(count_decl(Scope::PerTree), Needs::new(), |cx, out| {
        let root = cx.my_tree().expect("its tree");
        for node in root.descendants() {
            if node.name().as_deref() == Some("emph") {
                out.diags(Diagnostic::note("wc.emphasis", "emphasis is used").with_data(json!({ "n": 1 })).at(&*node))?;
            }
        }
        // With no node, a diagnostic stands at the stage's tree.
        out.diags(Diagnostic::warning("wc.tree", "a tree was counted"))
    });
    let run = run_demanding(&registry, &["wc.words"], &["One \\emph{two}.", "Three."]);
    assert_eq!(
        placed(&run),
        [
            ("wc.emphasis".to_string(), "emphasis is used".to_string(), 4..14),
            ("wc.tree".to_string(), "a tree was counted".to_string(), 0..15),
            ("wc.tree".to_string(), "a tree was counted".to_string(), 0..6),
        ]
    );
}

#[test]
fn a_diagnostic_of_a_document_stage_stands_at_the_first_tree() {
    let registry = one_stage(count_decl(Scope::Document), Needs::new(), |_, out| {
        out.diags(Diagnostic::note("wc.done", "the document was counted"))
    });
    let run = run_demanding(&registry, &["wc.words"], &["First.", "Second tree."]);
    assert_eq!(placed(&run), [("wc.done".to_string(), "the document was counted".to_string(), 0..6)]);
}

#[test]
fn a_pushed_float_is_refused() {
    let registry = one_stage(count_decl(Scope::PerTree), Needs::new(), |_, out| {
        let piece = out.table("wc.words")?;
        let refused = piece.push(json!({ "ratio": 0.5 })).expect_err("a float does not cross");
        piece.push(json!(refused.to_string()))?;
        piece.push(json!([1.5]))
    });
    let run = run_demanding(&registry, &["wc.words"], &["Text."]);
    assert_eq!(items(&run, "wc.words"), [json!("a number must be an integer, not `0.5`")]);
    assert_eq!(
        common::diagnostics(&run),
        ["flm.ext.handler-failed: the stage `wc.count` failed: a number must be an integer, not `1.5`"]
    );
}

// --- capabilities (plan A, A28) ----------------------------------------------------------------

#[test]
fn a_stage_provides_a_capability_another_stage_requires() {
    let double = Double::new("wc").feature("count", |b| {
        b.capability("wc.counted", "The words were counted.", None)?;
        b.table("wc.words", "")?;
        b.table("wc.reported", "")?;
        let count = StageDecl::new("wc.count", Effect::Compute, Scope::PerTree).provides("wc.counted").writes("wc.words");
        b.stage(stage(count, Needs::new(), |_, out| out.table("wc.words")?.push(json!("counted"))))?;
        let report = StageDecl::new("wc.report", Effect::Compute, Scope::Document).writes("wc.reported");
        b.stage(stage(report, Needs::new().requires("wc.counted"), |_, out| out.table("wc.reported")?.push(json!(1))))
    });
    let registry = registry_with([double.package()]);
    let plan = Plan::build(&registry, &PlanRequest::new().demand("wc.reported")).expect("the plan builds");
    let scheduled: Vec<String> = plan.scheduled_names().iter().map(|n| n.to_string()).collect();
    let count = scheduled.iter().position(|n| n == "wc.count").expect("the providing stage is scheduled");
    let report = scheduled.iter().position(|n| n == "wc.report").expect("the requiring stage is scheduled");
    assert!(count < report, "{scheduled:?}");
    let run = run_demanding(&registry, &["wc.reported"], &["Text."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(items(&run, "wc.reported"), [json!(1)]);
}

#[test]
fn a_capability_s_unprovided_text_is_what_the_plan_error_says() {
    // The counterpart of `CapabilityDef::unprovided`: a stage requires the
    // capability, no stage provides it, and the plan error gives the text.
    let text = "no feature counts the words; use a package that does";
    let double = Double::new("wc").feature("count", move |b| {
        b.capability("wc.counted", "The words were counted.", Some(text.to_string()))?;
        b.table("wc.reported", "")?;
        let report = StageDecl::new("wc.report", Effect::Compute, Scope::Document).writes("wc.reported");
        b.stage(stage(report, Needs::new().requires("wc.counted"), |_, out| out.table("wc.reported")?.push(json!(1))))
    });
    let registry = registry_with([double.package()]);
    let error = Plan::build(&registry, &PlanRequest::new().demand("wc.reported")).expect_err("nothing provides the capability");
    assert_eq!(error.to_string(), text);
}

#[test]
fn a_capability_outside_the_package_prefix_fails_the_build() {
    let double = Double::new("wc").feature("count", |b| b.capability("counted", "", None));
    let error = try_registry_with([double.package()]).expect_err("the build fails").to_string();
    assert!(error.contains("flm.ext.name-outside-extension"), "{error}");
    assert!(error.contains("the capability `counted` declared by the package `_` of the extension `wc` must begin with `wc.`"), "{error}");
}

#[test]
fn a_stage_may_provide_a_capability_another_feature_declares() {
    // The `math` feature declares `flm.math.content`, which a processing
    // feature's check provides, as `math-passthrough` does in `flm-core`.
    let double = Double::new("wc").feature("probe", |b| {
        let check = StageDecl::new("wc.check", Effect::Check, Scope::PerTree).provides("flm.math.content");
        b.stage(stage(check, Needs::new(), |_, _| Ok(())))
    });
    let registry = try_registry_with([double.package()]).expect("the registry builds");
    assert!(registry.stages().iter().any(|entry| entry.name.as_str() == "wc.check"));
}

#[test]
fn a_stage_providing_a_capability_no_feature_declares_fails_the_build() {
    let double = Double::new("wc").feature("probe", |b| {
        let check = StageDecl::new("wc.check", Effect::Check, Scope::PerTree).provides("wc.nothing");
        b.stage(stage(check, Needs::new(), |_, _| Ok(())))
    });
    let error = try_registry_with([double.package()]).expect_err("the build fails").to_string();
    assert!(
        error.contains(
            "feature `probe`: stage `wc.check` provides the capability `wc.nothing`, but no feature declares this \
             capability; declared capability names: [flm.anchors, "
        ),
        "{error}"
    );
}

// --- a node's address ------------------------------------------------------------------------------

/// A body that reads the rows of `wc.rows` at the node being lowered and
/// writes `[<tree> <rows>]`; in the document's tree it also splices a copy of
/// that tree (a synthesized value), over which the per-tree stage runs again.
struct RowsAtBox;

impl Lower for RowsAtBox {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let rows = match lw.get("wc.rows")? {
            Some(TableValue::Ext(table)) => table.items_at(&lw.addr()),
            _ => Vec::new(),
        };
        let numbers: Vec<String> = rows.iter().map(|row| row["number"].to_string()).collect();
        let tree = lw.tree_id();
        let mut emit = vec![lw.text(cx, &format!("[{tree} {}]", numbers.join(",")))?];
        if !tree.starts_with("value#") {
            let mut root = node.parent().expect("the node has a parent");
            while let Some(parent) = root.parent() {
                root = parent;
            }
            emit.extend(lw.splice(cx, &*root)?);
        }
        Ok(Lowering::Emit(emit))
    }
}

#[test]
fn a_stage_keys_its_rows_by_a_node_s_address_in_the_document_and_in_a_value_copy() {
    let double = Double::new("wc").feature("boxes", |b| {
        b.element(ElementDef::new(
            b.feature(),
            ["wcbox"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
            ElementSpec::new("wc.box"),
        ))?;
        b.table("wc.rows", "The boxes, by their addresses.")?;
        let decl = StageDecl::new("wc.number", Effect::Compute, Scope::PerTree).writes("wc.rows");
        b.stage(stage(decl, Needs::new(), |cx, out| {
            let tree = cx.tree_id().expect("a per-tree stage has a tree");
            let root = cx.my_tree().expect("its tree");
            let mut number = 0;
            for node in root.descendants() {
                // Every node of the stage's tree has an address in that tree,
                // in the document and in a value copy alike.
                let addr = cx.addr_of(&*node).expect("the node has an address");
                assert_eq!(addr.tree.to_string(), tree);
                if node.element_name() == "wc.box" {
                    number += 1;
                    out.table("wc.rows")?.push(json!({ "addr": node_addr(&addr), "number": number }))?;
                }
            }
            Ok(())
        }))?;
        let decl = LoweringDecl::new().produces(pres::SPAN).reads("wc.rows");
        b.lowering_rule(LoweringRule::new("wc.box.lower.default", "wc.box", decl, Arc::new(RowsAtBox), DefaultMark::Everywhere))
    });
    let registry = registry_with([double.package()]);
    let (text, diagnostics) = common::text_of(&registry, "A \\wcbox{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The rule finds its row at the box in the document's tree and in the copy.
    assert_eq!(text, "A [tree#0 1]A [value#0 1]..\n");
}

/// An address in the data form a rule's `get_at` compares, `{"tree", "node"}`.
fn node_addr(addr: &flm_core::model::addr::NodeAddr) -> Value {
    json!({ "tree": addr.tree.to_string(), "node": addr.node.index() })
}
