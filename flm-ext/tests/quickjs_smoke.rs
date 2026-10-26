//! A smoke test of the JavaScript API modules' items no other script runs
//! (WP-8's fix round): a data-form element with an expansion hook and a
//! block-role aspect, an environment with a `BodyDef`, an argument from
//! `arg_def_from_code`, specials, and a stage calling the free functions of
//! `flm:lang/text`, `flm:core/lowering` and `flm:lang/rewrites/blocks` and
//! reading definitions back with `ud.definition()`. Rendered through the text
//! workflow on the test thread and on a marked worker thread, where the
//! data-form hooks are rehearsed.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{workflow_request, Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::text;
use flm_ext::adapter::table::ExtTable;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::{json, Value};

use common::registry_with;

/// The script: three data-form elements with hooks, and a stage probing the
/// free functions over the parsed trees.
const MAIN: &str = r##"
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { ArgDef, ArgLayout, BodyDef, BodyLayout, ElementDef, Mode, SyntaxDef, arg_def_from_code } from "flm:lang/mint";
import { chars_text, read_chars } from "flm:lang/text";
import { chars_at_lowering, spelling_at_lowering } from "flm:core/lowering";
import { argument_holds_blocks } from "flm:lang/rewrites/blocks";
import { Definition, DefinitionModes, UserDefinition, Expansion, ExpandPolicy } from "flm:lang/rewrites/userdefs";
import { Aspect, ElementSpec } from "flm:model/element";
import { BlockRole, InlineSpacing, Placement } from "flm:model/aspects";
import { expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";

export default function (ext) { ext.package("_", { features: { smoke: { register } } }); }

// `\badge*{…}` is left as it is; `\badge{…}` becomes its text in capitals.
const BADGE = {
  policy() { return ExpandPolicy.Always; },
  expand(node, cx, xp) {
    if (cx.definition() !== null) throw new Error("a data-form element has no definition");
    if (node.argument_is_provided("star")) return Expansion.Default;
    const text = chars_text(node.argument_content_nodes_named("text")) ?? "?";
    return Expansion.Emit([cx.stage_chars(text.toUpperCase())]);
  },
};

// `\begin{boxed}[title]{note}…\end{boxed}` becomes `BOX title`, then whether
// each argument holds blocks.
const BOXED = {
  policy() { return ExpandPolicy.Always; },
  expand(node, cx, xp) {
    const title = chars_text(node.argument_content_nodes_named("title")) ?? "untitled";
    const blocks = [0, 1].map(index => argument_holds_blocks(node, index)).join(" ");
    return Expansion.Emit([cx.stage_chars(`BOX ${title} ${blocks}`)]);
  },
};

// `@@` becomes `AT`.
const AT = { policy() { return ExpandPolicy.Always; }, expand(node, cx, xp) { return Expansion.Emit([cx.stage_chars("AT")]); } };

// The element `spec` written `syntax` under `names` (a definition `name` of
// the kind `kind` for its lowering rule), expanded by its hook.
function declare(b, name, names, syntax, kind, spec) {
  b.lowering_rule(expand_in_tree_rule_for(spec.name, new Definition(kind, name, []).expand(ExpandPolicy.Always)));
  b.element(new ElementDef(b.feature(), names, syntax, spec).docs(`the element ${spec.name}`).modes([Mode.Text]));
}

function register(b) {
  const star = arg_def_from_code("*");
  star.name = "star";
  const badge = new ElementSpec("wc.badge")
    .docs("a badge")
    .with({ Expand: BADGE })
    .with(Aspect.BlockRoleAspect(BlockRole.Inline(InlineSpacing.CONTENT)));
  declare(b, "badge", ["badge"], SyntaxDef.Macro([star, ArgDef.mandatory("text").layout(ArgLayout.Inline).docs("the text")]), "macro", badge);
  const boxed = new ElementSpec("wc.boxed").with(Aspect.Expand(BOXED)).with({ BlockRoleAspect: BlockRole.Block });
  const body = new BodyDef().layout(BodyLayout.Inline).blocks().mode(Mode.Text);
  declare(b, "boxed", ["boxed"], SyntaxDef.Environment([ArgDef.optional("title"), ArgDef.mandatory("note").blocks()], body), "environment", boxed);
  const at = new ElementSpec("wc.at").with(Aspect.Expand(AT)).with(Aspect.BlockRoleAspect(BlockRole.BlockDecorator(Placement.Free)));
  declare(b, "@@", [], SyntaxDef.Specials("@@", []), "specials", at);
  b.table("wc.probe", "what the free functions answer over each tree");
  b.stage({
    decl: () => new StageDecl("wc.probe", Effect.Compute, Scope.PerTree).writes("wc.probe"),
    run(cx, out) {
      const table = out.table("wc.probe");
      const attempt = f => { try { return f(); } catch (error) { return { error: error.kind, node: error.data.data.node }; } };
      // The tree the stage reads is the one after the expansion of `always`
      // definitions: the starred badge, left as it was, is still there.
      for (const node of cx.my_tree().descendants()) {
        if (!node.is_callable()) continue;
        let nodes;
        if (node.element_name() === "wc.badge") nodes = node.argument_content_nodes_named("text");
        else if (node.name() === "emph") nodes = node.argument_content_nodes(0);
        else continue;
        table.push({
          name: node.name(),
          read_chars: attempt(() => read_chars(nodes)),
          chars_at_lowering: attempt(() => chars_at_lowering(nodes)),
          spelling_at_lowering: attempt(() => spelling_at_lowering(nodes)),
          holds_blocks: argument_holds_blocks(node, 0),
        });
      }
      // The definitions as Rust reads them back.
      const greet = Definition.macro_("greet", [ArgDef.optional("who").docs("who is greeted"), ArgDef.mandatory("what")])
        .text("Hello #1")
        .default("who", "you")
        .argument_number_offset(1)
        .expand(ExpandPolicy.Always)
        .docs("a greeting");
      const ket = Definition.macro_("ket", "{").math("|#1>").block_role(BlockRole.Inline(InlineSpacing.CONTENT));
      const tilde = Definition.specials("@!", []).content("x").modes(DefinitionModes.Both).overriding();
      const env = Definition.environment("pane", []).content("#{body}").block_role(BlockRole.Block);
      table.push({ definitions: [greet, ket, tilde, env].map(d => new UserDefinition(d).definition()) });
      table.push({ star: arg_def_from_code("*") });
    },
  });
}
"##;

/// The document: a badge expanded, a starred badge left as it is, a badge
/// over a command, a box, and the specials.
const DOCUMENT: &str = "A \\badge{hey} and \\badge*{a \\emph{b}}.\n\n\\begin{boxed}[Tip]{one\n\ntwo}inside\\end{boxed}\n\nThen @@ here.";

/// The package of `MAIN`, loaded with a default host.
fn package() -> PackageEntry {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), MAIN);
    factory.build(&PackageSettings::new()).expect("the package builds")
}

/// The run of the text workflow, demanding the table `wc.probe` too, over
/// `DOCUMENT`, sequential (see `tests/quickjs_binding.rs`'s `run_demanding`).
fn run_text(registry: &Arc<FlmRegistry>) -> Run {
    let source = Arc::new(FlmSource::new(DOCUMENT).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = workflow_request(registry, text::WORKFLOW, &Overrides::none()).expect("the workflow").demand("wc.probe");
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The rendered text, the diagnostics and the probe's items of `run`.
fn outcome(run: &Run) -> (String, Vec<String>, Vec<Value>) {
    let text = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    let items = run
        .table(TableHandle::<ExtTable>::new("wc.probe"))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default();
    (text, common::diagnostics(run), items)
}

#[test]
fn the_api_modules_items_run_from_a_script() {
    let registry = registry_with([package()]);
    let here = outcome(&run_text(&registry));
    // On a marked worker thread the registry was not built on: the feature is
    // rehearsed there, its data-form hooks lent again.
    let shared = Arc::clone(&registry);
    let worker = flm_core::lang::guard::run_with_worker_stack(move || outcome(&run_text(&shared)))
        .expect("the worker thread starts");
    assert_eq!(worker, here);
    let (text, diagnostics, items) = here;
    // `Expansion.Default` left the starred badge as it was: the stage meets
    // it, and the render reports it as it would for a hook of Rust.
    assert_eq!(
        diagnostics,
        [
            "flm.blocks.contract-violated: the document tree breaks the structural rule UseSiteSurvived: a use of the user definition ‘wc.badge’ remains after expansion",
            "flm.lowering.rule-failed: the lowering rule `wc.badge.lower.expand-in-tree` failed at a node of the element `wc.badge` for the backend `text`: the content does not have the structure that lowering it requires: a use site of `badge` reached lowering unexpanded, although the expansion the rule requires runs over every tree lowered",
        ]
    );
    assert_eq!(text, "A HEY and [unsupported: wc.badge].\n\nBOX Tip false true\n\nThen AT here.\n");
    assert_eq!(items.len(), 4, "{items:?}");
    // Over `a \emph{b}`: the `\emph` node is not characters; the error's
    // data names it by its index in the tree.
    let node = &items[0]["read_chars"]["node"];
    assert!(node.is_u64(), "{items:?}");
    assert_eq!(
        items[0],
        json!({
            "name": "badge",
            "read_chars": { "error": "core", "node": node },
            "chars_at_lowering": { "error": "core", "node": node },
            "spelling_at_lowering": null,
            "holds_blocks": false,
        })
    );
    assert_eq!(
        items[1],
        json!({
            "name": "emph",
            "read_chars": { "text": "b", "has_placeholder": false },
            "chars_at_lowering": "b",
            "spelling_at_lowering": null,
            "holds_blocks": false,
        })
    );
    let content = json!({ "inline": { "absorb_before": false, "absorb_after": false } });
    let arg = |code: &str, name: Value, docs: &str| {
        json!({ "code": code, "name": name, "layout": "inline", "opaque_wrapper": false, "docs": docs, "mode": null, "scoped": [], "role": null })
    };
    let definition = |kind: &str, name: &str, args: Vec<Value>, content_: &str, modes: &str| {
        json!({
            "kind": kind, "name": name, "args": args, "content": content_, "modes": modes, "defaults": {},
            "argument_number_offset": 0, "block_role": content, "expand": "per-plan", "overrides": false, "docs": "",
            "extra_placeholders": [],
        })
    };
    let mut greet = definition("macro", "greet", vec![arg("[", json!("who"), "who is greeted"), arg("{", json!("what"), "")], "Hello #1", "text");
    greet["defaults"] = json!({ "who": "you" });
    greet["argument_number_offset"] = json!(1);
    greet["expand"] = json!("always");
    greet["docs"] = json!("a greeting");
    let ket = definition("macro", "ket", vec![arg("{", Value::Null, "")], "|#1>", "math");
    let mut at = definition("specials", "@!", Vec::new(), "x", "both");
    at["overrides"] = json!(true);
    let mut pane = definition("environment", "pane", Vec::new(), "#{body}", "both");
    pane["block_role"] = json!("block");
    assert_eq!(items[2], json!({ "definitions": [greet, ket, at, pane] }));
    // WP-4a: an argument writes its mode and its scoped packages too, and its
    // role.
    assert_eq!(
        items[3],
        json!({ "star": { "code": "*", "name": null, "layout": "inline", "opaque_wrapper": false, "docs": "", "mode": null, "scoped": [], "role": null } })
    );
}
