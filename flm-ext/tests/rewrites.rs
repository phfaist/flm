//! Rewrite stages (E5, WP-2): a rewrite stage written in Rust against the
//! extension API, run by the adapter as `flm-core`'s rewrite stage over
//! techy's driver. The stage replaces dashes inside characters (outside
//! mathematics and verbatim), rebuilds a macro call from its parts, and each
//! failure path of the plan's section 6.4 is exercised. The registry switches
//! the `blocks` feature's own dash ligatures off, so that every dash of the
//! text output is the rewrite's.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::blocks::BlocksConfig;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::adapter::table::ExtTable;
use flm_ext::api::core::data::table::TableValue;
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::core::registry::FeatureBuilder;
use flm_ext::api::core::run::RewriteCx;
use flm_ext::api::core::stage::{Effect, Needs, RewriteDecl, RewriteStage, Scope, StageDecl};
use flm_ext::api::lang::mint::{ArgDef, BodyDef, ElementDef, Mode, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::techy::error::Diagnostic;
use flm_ext::api::techy::node::Node;
use flm_ext::api::techy::transform::{Restage, Restager, Staged};
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};

// --- the doubles of the test -------------------------------------------------------------------

/// A handler's work, as a closure.
type RestageFn = dyn Fn(&dyn Node<'_>, &dyn RewriteCx, &mut dyn Restager) -> Result<Restage, ExtError> + Send + Sync;

/// A [`RewriteStage`] whose handler is a closure.
struct RewriteFn {
    decl: RewriteDecl,
    needs: Needs,
    restage: Box<RestageFn>,
}

impl RewriteStage for RewriteFn {
    fn decl(&self) -> RewriteDecl {
        self.decl.clone()
    }

    fn needs(&self) -> Needs {
        self.needs.clone()
    }

    fn restage<'c>(&self, node: &'c dyn Node<'c>, cx: &'c dyn RewriteCx, rs: &'c mut dyn Restager) -> Result<Restage, ExtError> {
        (self.restage)(node, cx, rs)
    }
}

/// The rewrite stage `sd.dash` publishing `sd.dashed`, ordered as the
/// fixture's (after the expansion, before the block structure), whose
/// handler is `f`.
fn rewrite(
    f: impl Fn(&dyn Node<'_>, &dyn RewriteCx, &mut dyn Restager) -> Result<Restage, ExtError> + Send + Sync + 'static,
) -> Arc<dyn RewriteStage> {
    rewrite_with(Needs::new().after("flm.userdefs").before("flm.blocks.structured"), f)
}

/// [`rewrite`] with the needs `needs`.
fn rewrite_with(
    needs: Needs,
    f: impl Fn(&dyn Node<'_>, &dyn RewriteCx, &mut dyn Restager) -> Result<Restage, ExtError> + Send + Sync + 'static,
) -> Arc<dyn RewriteStage> {
    Arc::new(RewriteFn {
        decl: RewriteDecl::new("sd.dash", "sd.dashed").docs("Replaces dashes."),
        needs,
        restage: Box::new(f),
    })
}

/// A rule's body as a closure.
type BodyFn = dyn Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync;

/// A [`Lower`] whose work is a closure.
struct LowerFn(Box<BodyFn>);

impl Lower for LowerFn {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        (self.0)(node, cx, lw)
    }
}

/// The rule `name` of `element`, whose body is `f`.
fn rule(
    name: &str,
    element: &str,
    f: impl Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync + 'static,
) -> LoweringRule {
    LoweringRule::new(name, element, LoweringDecl::new(), Arc::new(LowerFn(Box::new(f))), DefaultMark::Everywhere)
}

/// Declares the test's elements and their rules: `\sdquote{text}{kept}`
/// (`sd.quote`, lowered as `text/kept`), `\begin{sdbox}{title}…\end{sdbox}`
/// (`sd.box`, lowered as `[title]body`), `\sdflat{x}` (`sd.flat`, lowered as
/// its children).
fn elements(b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
    let feature = b.feature();
    let two = vec![ArgDef::mandatory("text"), ArgDef::mandatory("kept")];
    b.element(ElementDef::new(&feature, ["sdquote"], SyntaxDef::Macro { args: two }, ElementSpec::new("sd.quote")))?;
    b.element(ElementDef::new(
        &feature,
        ["sdbox"],
        SyntaxDef::Environment { args: vec![ArgDef::mandatory("title")], body: BodyDef::new() },
        ElementSpec::new("sd.box"),
    ))?;
    let one = vec![ArgDef::mandatory("x")];
    b.element(ElementDef::new(&feature, ["sdflat"], SyntaxDef::Macro { args: one }, ElementSpec::new("sd.flat")))?;
    b.lowering_rule(rule("sd.quote.lower", "sd.quote", |node, cx, lw| {
        let mut out = lw.lower_arg(cx, node, 0)?;
        out.push(lw.text(cx, "/")?);
        out.extend(lw.lower_arg(cx, node, 1)?);
        Ok(Lowering::Emit(out))
    }))?;
    b.lowering_rule(rule("sd.box.lower", "sd.box", |node, cx, lw| {
        let mut out = vec![lw.text(cx, "[")?];
        out.extend(lw.lower_arg_named(cx, node, "title")?);
        out.push(lw.text(cx, "]")?);
        out.extend(lw.lower_body(cx, node)?);
        Ok(Lowering::Emit(out))
    }))?;
    b.lowering_rule(rule("sd.flat.lower", "sd.flat", |node, cx, lw| Ok(Lowering::Emit(lw.lower_children(cx, node)?))))
}

/// The package `sd`, whose feature `dashes` declares the elements and the
/// rewrite stage `stage`.
fn package(stage: Arc<dyn RewriteStage>) -> PackageEntry {
    Double::new("sd")
        .feature("dashes", move |b| {
            elements(b)?;
            b.rewrite_stage(Arc::clone(&stage))
        })
        .package()
}

/// The registry of the standard package, with the `blocks` feature's dash
/// ligatures off, and of `packages`.
fn registry(packages: impl IntoIterator<Item = PackageEntry>) -> Arc<FlmRegistry> {
    let mut config = StdcoreConfig::new();
    config.blocks = BlocksConfig { ligature_unicode_dashes: false, ..BlocksConfig::default() };
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(config));
    builder.use_package(flm_core::math_passthrough::package());
    for package in packages {
        builder.use_package(package);
    }
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// The plan demanding `demands`, or its error's text.
fn plan(registry: &Arc<FlmRegistry>, demands: &[&str]) -> Result<Plan, String> {
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    Plan::build(registry, &request).map_err(|error| error.to_string())
}

/// The run of a plan demanding `demands` over a document of one tree per
/// text of `texts`, in order.
fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
    let mut content = DocContent::new();
    for text in texts {
        let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    let plan = Arc::new(plan(registry, demands).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The text output of `source` with the rewrite's table demanded beside it,
/// and the run's diagnostics.
fn render(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run_demanding(registry, &["sd.dashed", TEXT], &[source]);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, common::diagnostics(&run))
}

/// `text` with `---` and `--` replaced by the em dash and the en dash.
fn dashed(text: &str) -> String {
    text.replace("---", "\u{2014}").replace("--", "\u{2013}")
}

/// Whether `node` stands inside mathematics or verbatim content.
fn opaque(node: &dyn Node<'_>) -> bool {
    if node.mode() == Mode::Math {
        return true;
    }
    let mut up = node.parent();
    while let Some(parent) = up {
        if parent.element_name().starts_with("flm.verbatim") {
            return true;
        }
        up = parent.parent();
    }
    false
}

/// The first shape of a rewrite: characters holding a dash, outside
/// mathematics and verbatim, replaced by one staged characters node.
fn replace_dashes(node: &dyn Node<'_>, rs: &mut dyn Restager) -> Result<Option<Restage>, ExtError> {
    match node.chars() {
        Some(text) if text.contains("--") && !opaque(node) => Ok(Some(Restage::Emit(vec![rs.stage_chars(&dashed(&text), node)?]))),
        _ => Ok(None),
    }
}

/// The second shape: `\sdquote{text}{kept}` rebuilt from its parts between
/// quotation marks, its first argument's content restaged child by child,
/// its second restaged as it is.
fn rebuild_quote(node: &dyn Node<'_>, rs: &mut dyn Restager) -> Result<Restage, ExtError> {
    let content: Vec<Staged> = {
        let children = node.argument_content_nodes(0).unwrap_or_default();
        let mut staged = Vec::new();
        for child in &children {
            staged.extend(rs.restage_subtree(&**child)?);
        }
        staged
    };
    let argument = rs.restage_argument_with_content(node, 0, content)?;
    let kept = rs.restage_argument(node, 1)?;
    assert!(argument.is_provided() && kept.is_provided());
    let call = rs.restage_invocation(node, vec![argument, kept], Vec::new())?;
    Ok(Restage::Emit(vec![rs.stage_chars("\u{201c}", node)?, call, rs.stage_chars("\u{201d}", node)?]))
}

/// The handler of the test's stage: the two shapes, `\begin{sdbox}` rebuilt
/// from its named argument and its body slot (the slot's name noted in
/// `slots`), `\sdflat` replaced by its restaged children.
fn dashes(slots: Arc<Mutex<Vec<Option<String>>>>) -> Arc<dyn RewriteStage> {
    rewrite(move |node, _, rs| {
        if let Some(answer) = replace_dashes(node, rs)? {
            return Ok(answer);
        }
        match node.element_name().as_str() {
            "sd.quote" => rebuild_quote(node, rs),
            "sd.box" => {
                let title = rs.restage_argument_named(node, "title")?;
                let body = rs.restage_slot(node, 0)?;
                slots.lock().expect("the lock").push(body.name());
                Ok(Restage::Emit(vec![rs.restage_invocation(node, vec![title], vec![body])?]))
            }
            "sd.flat" => Ok(Restage::Emit(rs.restage_children(node)?)),
            _ => Ok(Restage::Descend),
        }
    })
}

/// The name of the text output's table, demanded beside the rewrite's.
const TEXT: &str = "flm.output.text";

/// The kind of an [`ExtError::Core`], or the error's own kind.
fn core_kind(error: &ExtError) -> String {
    match error {
        ExtError::Core { kind, .. } => kind.clone(),
        other => other.kind().to_string(),
    }
}

/// The document of the dashes.
const DOCUMENT: &str = "A dash -- an em dash --- and \\sdquote{a quote -- with \\sdquote{in--side}{x}}{kept -- too}.";

// --- the tests ---------------------------------------------------------------------------------

#[test]
fn a_rewrite_replaces_dashes_and_rebuilds_a_call_from_its_parts() {
    let slots = Arc::new(Mutex::new(Vec::new()));
    let registry = registry([package(dashes(Arc::clone(&slots)))]);
    let (text, diagnostics) = render(&registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        text.trim_end(),
        "A dash \u{2013} an em dash \u{2014} and \u{201c}a quote \u{2013} with \u{201c}in\u{2013}side/x\u{201d}/kept \u{2013} too\u{201d}."
    );
    // Without the rewrite's table demanded, the dashes stay (the ligatures
    // are off).
    let run = run_demanding(&registry, &[TEXT], &["a -- b"]);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(out.trim_end(), "a -- b");
    assert!(slots.lock().expect("the lock").is_empty());
}

#[test]
fn mathematics_and_verbatim_are_left_as_they_are() {
    let registry = registry([package(dashes(Arc::new(Mutex::new(Vec::new()))))]);
    let (text, diagnostics) = render(&registry, "Out -- side \\(a--b\\) and \\verbcode{c--d}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The text output writes mathematics as its source.
    assert_eq!(text, "Out \u{2013} side \\(a--b\\) and c--d.\n");
}

#[test]
fn a_named_argument_a_slot_and_the_children_are_restaged() {
    let slots = Arc::new(Mutex::new(Vec::new()));
    let registry = registry([package(dashes(Arc::clone(&slots)))]);
    let (text, diagnostics) =
        render(&registry, "\\begin{sdbox}{a -- title}body -- text\\end{sdbox} and \\sdflat{x -- y}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(text.contains("[a \u{2013} title]body \u{2013} text"), "{text}");
    assert!(text.contains("x \u{2013} y"), "{text}");
    assert_eq!(*slots.lock().expect("the lock"), vec![Some("body".to_string())]);
}

#[test]
fn the_context_answers_the_tree_and_its_layout() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&seen);
    let stage = rewrite(move |node, cx, _| {
        if node.parent().is_none() {
            let option = cx.option("sd.options").map_err(|error| core_kind(&error));
            noted.lock().expect("the lock").push(format!("{} {} {option:?}", cx.tree_id(), cx.layout()));
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let run = run_demanding(&registry, &["sd.dashed"], &["One.", "Two."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        *seen.lock().expect("the lock"),
        vec![
            "tree#0 inline-or-blocks Err(\"option\")".to_string(),
            "tree#1 inline-or-blocks Err(\"option\")".to_string()
        ]
    );
}

#[test]
fn a_later_stage_reads_the_published_trees() {
    let double = Double::new("sd").feature("dashes", |b| {
        elements(b)?;
        b.rewrite_stage(dashes(Arc::new(Mutex::new(Vec::new()))))?;
        b.table("sd.seen", "The characters of the rewritten trees.")?;
        let decl = StageDecl::new("sd.read", Effect::Compute, Scope::Document).writes("sd.seen");
        b.stage(stage(decl, Needs::new().reads("sd.dashed"), |cx, out| {
            let Some(TableValue::Trees(trees)) = cx.get("sd.dashed")? else {
                return Err(ExtError::Handler { detail: "no trees".to_string(), data: Value::Null });
            };
            for (tree, root) in trees.trees() {
                let text: String = root.descendants().iter().filter_map(|node| node.chars()).collect();
                out.table("sd.seen")?.push(json!({ "tree": tree, "text": text }))?;
            }
            Ok(())
        }))
    });
    let registry = registry([double.package()]);
    let run = run_demanding(&registry, &["sd.seen"], &["a -- b", "c --- d"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let items = run
        .table(TableHandle::<ExtTable>::new("sd.seen"))
        .expect("declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default();
    assert_eq!(
        items,
        vec![json!({ "tree": "tree#0", "text": "a \u{2013} b" }), json!({ "tree": "tree#1", "text": "c \u{2014} d" })]
    );
}

#[test]
fn a_failing_handler_keeps_its_node_and_the_rest_is_rewritten() {
    let stage = rewrite(|node, _, rs| {
        if node.chars().is_some_and(|text| text.contains("boom")) {
            // Staged before the failure: dropped with the call's other work.
            rs.stage_chars("lost", node)?;
            return Err(ExtError::Handler { detail: "no dashes here".to_string(), data: Value::Null });
        }
        Ok(replace_dashes(node, rs)?.unwrap_or(Restage::Descend))
    });
    let registry = registry([package(stage)]);
    let (text, diagnostics) = render(&registry, "a -- b \\sdflat{boom -- x} c -- d");
    assert_eq!(diagnostics, vec!["flm.ext.handler-failed: the rewrite stage `sd.dash` failed: no dashes here"]);
    assert_eq!(text.trim_end(), "a \u{2013} b boom -- x c \u{2013} d");
}

#[test]
fn an_emit_names_only_nodes_of_its_own_call_each_once() {
    let kept: Arc<Mutex<Option<Staged>>> = Arc::new(Mutex::new(None));
    let stage = rewrite(move |node, _, rs| {
        let Some(text) = node.chars() else { return Ok(Restage::Descend) };
        let staged = rs.stage_chars(&dashed(&text), node)?;
        match text.trim() {
            "one" => {
                *kept.lock().expect("the lock") = Some(staged);
                Ok(Restage::Emit(vec![staged]))
            }
            // A node another call staged.
            "two" => Ok(Restage::Emit(vec![kept.lock().expect("the lock").expect("staged by the first call")])),
            // A node named twice.
            "three" => Ok(Restage::Emit(vec![staged, staged])),
            _ => Ok(Restage::Emit(vec![staged])),
        }
    });
    let registry = registry([package(stage)]);
    let (text, diagnostics) = render(&registry, "\\sdflat{one}\\sdflat{two}\\sdflat{three}\\sdflat{a -- b}");
    assert_eq!(
        diagnostics,
        vec![
            "flm.ext.handler-failed: the rewrite stage `sd.dash` failed: `restage`: each node placed must have been \
             created during this same rewrite call",
            "flm.ext.handler-failed: the rewrite stage `sd.dash` failed: `restage`: a node may be placed only once",
        ]
    );
    assert_eq!(text.trim_end(), "onetwothreea \u{2013} b");
}

#[test]
fn a_refused_operation_is_an_error_the_handler_catches() {
    let caught = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&caught);
    let stage = rewrite(move |node, _, rs| {
        if node.element_name() == "sd.quote" {
            for error in [
                rs.restage_argument(node, 3).err(),
                rs.restage_argument_named(node, "nothing").err(),
                rs.restage_slot(node, 0).err(),
            ]
            .into_iter()
            .flatten()
            {
                noted.lock().expect("the lock").push(format!("{} {}", core_kind(&error), error));
            }
            // A node of this call's content must have been staged by it.
            let other = rs.restage_argument_with_content(node, 0, Vec::new())?;
            assert!(other.is_provided());
        }
        Ok(replace_dashes(node, rs)?.unwrap_or(Restage::Descend))
    });
    let registry = registry([package(stage)]);
    let (text, diagnostics) = render(&registry, "\\sdquote{a -- b}{c}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text.trim_end(), "a \u{2013} b/c");
    let caught = caught.lock().expect("the lock").clone();
    // The construct as written, with its line; no internal identifier.
    assert_eq!(
        caught,
        [
            "restage `\\sdquote` on line 1 has no argument at index 3: it has 2 arguments",
            "restage `\\sdquote` on line 1 has no argument named `nothing`",
            "restage `\\sdquote` on line 1 has no slot at index 0: it has 0 slots",
        ]
    );
}

#[test]
fn content_not_staged_by_the_call_is_refused() {
    let kept: Arc<Mutex<Option<Staged>>> = Arc::new(Mutex::new(None));
    let stage = rewrite(move |node, _, rs| {
        if node.is_chars() {
            *kept.lock().expect("the lock") = Some(rs.stage_chars("x", node)?);
        }
        if node.element_name() == "sd.quote" {
            let other = kept.lock().expect("the lock").expect("a node staged by an earlier call");
            rs.restage_argument_with_content(node, 0, vec![other])?;
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let (_, diagnostics) = render(&registry, "x \\sdquote{a}{b}");
    assert_eq!(
        diagnostics,
        vec![
            "flm.ext.handler-failed: the rewrite stage `sd.dash` failed: `restage_argument_with_content`: each node \
             placed must have been created during this same rewrite call"
        ]
    );
}

/// Content is recorded as placed, as an `Emit` places its nodes: the same
/// staged nodes given as content twice, or given as content and also
/// emitted, is the method's refusal (message 10), which the handler may
/// catch; the tree is rewritten.
#[test]
fn content_given_twice_or_given_and_emitted_is_refused() {
    let caught = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&caught);
    let stage = rewrite(move |node, _, rs| {
        if node.element_name() == "sd.quote" {
            let children = node.argument_content_nodes(0).unwrap_or_default();
            let mut content = Vec::new();
            for child in &children {
                content.extend(rs.restage_subtree(&**child)?);
            }
            let argument = rs.restage_argument_with_content(node, 0, content.clone())?;
            let again = rs.restage_argument_with_content(node, 0, content.clone()).err().map(|error| error.to_string());
            noted.lock().expect("the lock").extend(again);
            let kept = rs.restage_argument(node, 1)?;
            let call = rs.restage_invocation(node, vec![argument, kept], Vec::new())?;
            let mut placed = vec![call];
            placed.extend(content);
            return Ok(Restage::Emit(placed));
        }
        Ok(replace_dashes(node, rs)?.unwrap_or(Restage::Descend))
    });
    let registry = registry([package(stage)]);
    let (text, diagnostics) = render(&registry, "x \\sdquote{a}{b} c -- d");
    assert_eq!(
        diagnostics,
        vec!["flm.ext.handler-failed: the rewrite stage `sd.dash` failed: `restage`: a node may be placed only once"]
    );
    assert_eq!(
        caught.lock().expect("the lock").clone(),
        ["`restage_argument_with_content`: a node may be placed only once"]
    );
    // The handler failed at `\\sdquote`, which is kept; the rest is rewritten.
    assert_eq!(text.trim_end(), "x a/b c \u{2013} d");
}

/// A run the driver cannot finish (here the root replaced by two nodes) is
/// the Error `flm.ext.rewrite-failed` at the tree's root; the tree is absent
/// from the output and the run completes with the other trees.
#[test]
fn a_run_the_driver_cannot_finish_ends_the_tree_and_the_run_completes_without_it() {
    let stage = rewrite(|node, _, rs| {
        if node.parent().is_none() && node.children().iter().any(|child| child.chars().is_some_and(|text| text.contains("root"))) {
            let one = rs.stage_chars("one", node)?;
            let two = rs.stage_chars("two", node)?;
            return Ok(Restage::Emit(vec![one, two]));
        }
        Ok(replace_dashes(node, rs)?.unwrap_or(Restage::Descend))
    });
    let registry = registry([package(stage)]);
    let run = run_demanding(&registry, &["sd.dashed", TEXT], &["root", "c -- d"]);
    assert_eq!(
        common::diagnostics(&run),
        vec![
            "flm.ext.rewrite-failed: the rewrite stage `sd.dash` must produce one tree from the tree `tree#0`: the root \
             restaged to 2 nodes; a tree needs exactly one root (wrap the replacement in a node of your own via Emit)"
        ]
    );
    // The other tree is rewritten; the failed one is absent.
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(out.trim_end(), "c \u{2013} d");
}

/// The descent guard's refusal, reached from a document: a handler that
/// restages each characters node's parent recurses through the visitor
/// until the guard refuses; each tree reports the nesting condition and is
/// absent from the output.
#[test]
fn the_guard_refuses_a_handler_recursing_through_the_visitor() {
    let stage = rewrite(|node, _, rs| {
        if let Some(parent) = node.parent().filter(|_| node.is_chars()) {
            rs.restage_subtree(&*parent)?;
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let run = run_demanding(&registry, &["sd.dashed", TEXT], &["a \\sdflat{b}", "c"]);
    let identifiers = common::identifiers(&run);
    assert_eq!(identifiers, ["flm.blocks.nesting-too-deep", "flm.blocks.nesting-too-deep"], "{:?}", common::diagnostics(&run));
    for diagnostic in common::diagnostics(&run) {
        assert!(diagnostic.starts_with("flm.blocks.nesting-too-deep: the content nests too deeply to process: "), "{diagnostic}");
    }
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(out.trim(), "");
}

#[test]
fn a_diagnostic_of_the_handler_is_reported_at_the_node() {
    let stage = rewrite(|node, _, rs| {
        if node.element_name() == "sd.quote" {
            rs.diagnose(Diagnostic::warning("sd.quote.seen", "a quote"))?;
            let refused = rs.diagnose(Diagnostic::error("other.seen", "outside")).err().map(|error| error.to_string());
            assert_eq!(
                refused.as_deref(),
                Some("the diagnostic `other.seen` declared by the package `_` of the extension `sd` must begin with `sd.`")
            );
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let run = run_demanding(&registry, &["sd.dashed"], &["x \\sdquote{a}{b}"]);
    let found: Vec<String> = run
        .diagnostics()
        .iter()
        .map(|d| format!("{} {} {}", d.identifier(), d.message(), d.span().range().start))
        .collect();
    assert_eq!(found, vec!["sd.quote.seen a quote 2"]);
}

#[test]
fn a_rewrite_declaring_a_read_is_refused_by_the_plan() {
    let stage = rewrite_with(Needs::new().reads("flm.blocks.structured"), |_, _, _| Ok(Restage::Descend));
    let registry = registry([package(stage)]);
    let error = plan(&registry, &["sd.dashed"]).expect_err("a rewrite stage reads no table");
    assert_eq!(
        error,
        "rewrite stage `sd.dash` declares a read of the table `flm.blocks.structured`, but a rewrite stage must not \
         read tables: it runs before any table is computed"
    );
}

#[test]
fn the_names_of_a_rewrite_stage_and_of_its_table_begin_with_the_stem() {
    for (name, publishes, what) in [("other.dash", "sd.dashed", "stage `other.dash`"), ("sd.dash", "other.dashed", "table `other.dashed`")]
    {
        let stage: Arc<dyn RewriteStage> = Arc::new(RewriteFn {
            decl: RewriteDecl::new(name, publishes),
            needs: Needs::new(),
            restage: Box::new(|_, _, _| Ok(Restage::Descend)),
        });
        let refused = Arc::new(Mutex::new(None));
        let noted = Arc::clone(&refused);
        let double = Double::new("sd").feature("dashes", move |b| {
            *noted.lock().expect("the lock") = b.rewrite_stage(Arc::clone(&stage)).err().map(|error| error.to_string());
            Ok(())
        });
        let _ = registry([double.package()]);
        assert_eq!(
            refused.lock().expect("the lock").clone(),
            Some(format!("the {what} declared by the package `_` of the extension `sd` must begin with `sd.`"))
        );
    }
}

/// A construct is named in a refusal by its callable's type (ruling S19): a
/// macro as written, a control symbol with its backslash, an environment by
/// its opening, specials as the sequence written.
#[test]
fn a_refusal_names_the_construct_by_its_callable_s_type() {
    let caught = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&caught);
    let stage = rewrite(move |node, _, rs| {
        if node.element_name() == "sd.box" || matches!(node.name().as_deref(), Some("%" | "~")) {
            noted.lock().expect("the lock").extend(rs.restage_argument(node, 5).err().map(|error| error.to_string()));
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let (_, diagnostics) = render(&registry, "a\\%b~c\n\n\\begin{sdbox}{t}x\\end{sdbox}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        caught.lock().expect("the lock").clone(),
        [
            "`\\%` on line 1 has no argument at index 5: it has 0 arguments",
            "`~` on line 1 has no argument at index 5: it has 0 arguments",
            "`\\begin{sdbox}` on line 3 has no argument at index 5: it has 1 argument",
        ]
    );
}

/// Content given to an operation techy refuses is not recorded as placed
/// (ruling S20): the handler catches the refusal and emits the same nodes.
#[test]
fn content_given_to_a_refused_operation_may_still_be_emitted() {
    let caught = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&caught);
    let stage = rewrite(move |node, _, rs| {
        if node.element_name() == "sd.quote" {
            let content = vec![rs.stage_chars("new", node)?];
            noted
                .lock()
                .expect("the lock")
                .extend(rs.restage_argument_with_content(node, 5, content.clone()).err().map(|error| error.to_string()));
            return Ok(Restage::Emit(content));
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let (text, diagnostics) = render(&registry, "x \\sdquote{a}{b} y");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        caught.lock().expect("the lock").clone(),
        ["`\\sdquote` on line 1 has no argument at index 5: it has 2 arguments"]
    );
    assert_eq!(text.trim_end(), "x new y");
}

/// A node holding a kept call's argument content, replaced by nothing: the
/// tree cannot be built, message 2 with the wording of the content's parent
/// dropped (ruling S21), in full.
#[test]
fn a_dropped_argument_group_of_a_kept_call_ends_the_tree() {
    let stage = rewrite(|node, _, _| {
        let in_quote = node.parent().is_some_and(|parent| parent.element_name() == "sd.quote");
        if node.is_group() && in_quote {
            return Ok(Restage::Emit(Vec::new()));
        }
        Ok(Restage::Descend)
    });
    let registry = registry([package(stage)]);
    let run = run_demanding(&registry, &["sd.dashed", TEXT], &["x \\sdquote{a}{b}", "c -- d"]);
    assert_eq!(
        common::diagnostics(&run),
        vec![
            "flm.ext.rewrite-failed: the rewrite stage `sd.dash` must produce one tree from the tree `tree#0`: the \
             content of an argument or a slot of `\\sdquote` on line 1 lies inside a node the rewrite stage removed or \
             replaced by other than one node; replace the whole call instead, with a copy made by `restage_invocation`"
        ]
    );
}
