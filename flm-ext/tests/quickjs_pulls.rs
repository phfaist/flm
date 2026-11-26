//! What a scripted handler pulls from the host: the traversals of nodes
//! restricted by element names and by node kinds, the rows of a table
//! another stage of the extension wrote, and the sequences of nodes and of
//! staged nodes, read and passed back joined.

#![cfg(feature = "quickjs")]

mod common;

use flm_core::core::settings::PackageSettings;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::json;

use common::{ext_items, registry_with, run_demanding, text_of};

/// The extension `wc`: the stage `wc.filters` writes into `wc.filters`, for
/// each restriction of `cx.walk()` and of the descendants of the one tree's
/// root, the addresses of the nodes the restricted traversal returns, beside
/// those of the nodes of the unrestricted traversal that match, in order.
const FILTERS: &str = r#"
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { NodeKindName } from "flm:techy/node";

export default function (ext) {
  ext.package("_", { features: { filters: { register(b) {
    b.table("wc.filters", "the restricted traversals and what they must return");
    b.stage({
      decl: () => new StageDecl("wc.filters", Effect.Compute, Scope.Document).writes("wc.filters"),
      run(cx, out) {
        const key = (node) => JSON.stringify(cx.addr_of(node));
        const keys = (nodes) => nodes.map(key);
        const nodes = (walk) => [...walk].map(({ node }) => node);
        const walked = nodes(cx.walk());
        const [[, tree]] = [...cx.trees()];
        const descended = [...tree.root().descendants()];
        const named = (names) => (node) => names.includes(node.element_name());
        const of_kind = (kinds) => (node) => kinds.includes(node.kind());
        const chars_name = walked.find(of_kind([NodeKindName.Chars])).element_name();
        const names = [walked.find((node) => of_kind([NodeKindName.Callable])(node) && node.name() === "emph").element_name()];
        const kinds = [NodeKindName.Chars];
        const both = [...names, chars_name];
        const both_kinds = [NodeKindName.Chars, NodeKindName.Comment];
        const matching_both = (node) => named(both)(node) && of_kind(both_kinds)(node);
        out.table("wc.filters").push({
          walk_elements: [keys(nodes(cx.walk().elements(names))), keys(walked.filter(named(names)))],
          descendants_elements: [keys([...tree.root().descendants().elements(names)]), keys(descended.filter(named(names)))],
          walk_kinds: [keys(nodes(cx.walk().kinds(kinds))), keys(walked.filter(of_kind(kinds)))],
          descendants_kinds: [keys([...tree.root().descendants().kinds(kinds)]), keys(descended.filter(of_kind(kinds)))],
          walk_both: [keys(nodes(cx.walk().elements(both).kinds(both_kinds))), keys(walked.filter(matching_both))],
          descendants_both: [keys([...tree.root().descendants().kinds(both_kinds).elements(both)]), keys(descended.filter(matching_both))],
          unknown: [nodes(cx.walk().elements(["no-such-element"])).length, [...tree.root().descendants().elements(["no-such-element"])].length],
          emph: walked.filter(named(names)).length,
          both_narrows: [walked.filter(named(both)).length, walked.filter(of_kind(both_kinds)).length, walked.filter(matching_both).length],
        });
      },
    });
  } } } });
}
"#;

/// On a document where `\emph` stands inside another `\emph`, beside a
/// comment, each restricted traversal returns the nodes of the unrestricted
/// traversal that match, in order (restricted with both methods, those that
/// match both, fewer than either method alone returns), and a name no
/// element has returns no node.
#[test]
fn a_restricted_traversal_returns_the_matching_nodes_in_order() {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), FILTERS);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let run = run_demanding(&registry, &["wc.filters"], &["a \\emph{b \\emph{c} d} e %% x"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let items = ext_items(&run, "wc.filters");
    let [item] = items.as_slice() else { panic!("one item: {items:?}") };
    assert_eq!(item["emph"], json!(2), "{item}");
    assert_eq!(item["unknown"], json!([0, 0]), "{item}");
    let [named, of_kind, both] = [0, 1, 2].map(|i| item["both_narrows"][i].as_u64().expect("a count"));
    assert!(both < named && both < of_kind, "{item}");
    for name in ["walk_elements", "descendants_elements", "walk_kinds", "descendants_kinds", "walk_both", "descendants_both"] {
        let pair = item[name].as_array().expect("a pair");
        assert_eq!(pair[0], pair[1], "{name}");
        assert!(pair[0].as_array().is_some_and(|keys| !keys.is_empty()), "{name}: {item}");
    }
    assert_eq!(item["walk_elements"][0].as_array().map(Vec::len), Some(2), "{item}");
}

/// The extension `wc`: the stage `wc.write` pushes five rows into
/// `wc.rows`, the last larger than a group's bound in bytes; the stage
/// `wc.read` reads them back through `items()` and writes what `len()`,
/// `is_empty()`, `get(i)` and two iterations return into `wc.read`.
const ROWS: &str = r#"
import { StageDecl, Effect, Scope, Needs } from "flm:core/stage";

export default function (ext) {
  ext.package("_", { features: { rows: { register(b) {
    b.table("wc.rows", "the rows the first stage wrote");
    b.table("wc.read", "what the second stage read");
    b.stage({
      decl: () => new StageDecl("wc.write", Effect.Compute, Scope.Document).writes("wc.rows"),
      run(cx, out) {
        const rows = out.table("wc.rows");
        for (const n of [3, 1, 4, 1]) rows.push({ n });
        rows.push({ n: 5, text: "x".repeat(100000) });
      },
    });
    b.stage({
      decl: () => new StageDecl("wc.read", Effect.Compute, Scope.Document).writes("wc.read"),
      needs: () => new Needs().reads("wc.rows"),
      run(cx, out) {
        const items = cx.get("wc.rows").ext.items();
        const n = (row) => row.n;
        const got = [];
        for (let i = 0; i < items.len(); i++) got.push(n(items.get(i)));
        out.table("wc.read").push({
          len: items.len(),
          is_empty: items.is_empty(),
          got,
          past_the_end: items.get(items.len()),
          first: [...items].map(n),
          second: [...items].map(n),
          text: items.get(4).text.length,
        });
      },
    });
  } } } });
}
"#;

/// A stage reads a table of several rows that another stage wrote: `len()`,
/// `get(i)` and iteration, repeated, return the rows in the order they were
/// pushed, and a row past a group's bound in bytes arrives whole.
#[test]
fn a_stage_reads_the_rows_of_a_table_by_index_and_by_iteration() {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), ROWS);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let run = run_demanding(&registry, &["wc.read"], &["Text."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        ext_items(&run, "wc.read"),
        [json!({
            "len": 5,
            "is_empty": false,
            "got": [3, 1, 4, 1, 5],
            "past_the_end": null,
            "first": [3, 1, 4, 1, 5],
            "second": [3, 1, 4, 1, 5],
            "text": 100000,
        })]
    );
}

/// The extension `wc`: the stage `wc.children` writes into `wc.children`, for
/// the paragraph of the one tree, what the sequence `children()` answers
/// beside what `child_count()` and `child(i)` answer, as the nodes'
/// addresses.
const CHILDREN: &str = r#"
import { StageDecl, Effect, Scope } from "flm:core/stage";

export default function (ext) {
  ext.package("_", { features: { children: { register(b) {
    b.table("wc.children", "a node's children, by each method");
    b.stage({
      decl: () => new StageDecl("wc.children", Effect.Compute, Scope.Document).writes("wc.children"),
      run(cx, out) {
        const key = (node) => node === null ? null : JSON.stringify(cx.addr_of(node));
        const [[, tree]] = [...cx.trees()];
        let paragraph = tree.root();
        while (paragraph.child_count() < 300) paragraph = paragraph.child(0);
        const children = paragraph.children();
        const by_index = [];
        for (let i = 0; i < children.len(); i++) by_index.push(key(children.get(i)));
        const by_child = [];
        for (let i = 0; i < paragraph.child_count(); i++) by_child.push(key(paragraph.child(i)));
        out.table("wc.children").push({
          len: children.len(),
          count: paragraph.child_count(),
          by_index,
          by_child,
          past_the_end: children.get(children.len()),
          first: [...children].map(key),
          second: [...children].map(key),
        });
      },
    });
  } } } });
}
"#;

/// For a node with more children than one group holds, `children()` has the
/// `len()` of `child_count()`, `get(i)` returns the node `child(i)` returns
/// for every index, `get(len())` returns `null`, and two iterations return
/// the same nodes.
#[test]
fn a_node_s_children_are_a_sequence_with_its_child_count_and_its_children() {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), CHILDREN);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let source = (0..200).map(|i| format!("w{i} \\emph{{e}}")).collect::<Vec<_>>().join(" ");
    let run = run_demanding(&registry, &["wc.children"], &[source.as_str()]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let items = ext_items(&run, "wc.children");
    let [item] = items.as_slice() else { panic!("one item: {items:?}") };
    let count = item["count"].as_u64().expect("a count");
    assert!(count > 300, "{item}");
    assert_eq!(item["len"], json!(count));
    assert_eq!(item["by_index"], item["by_child"]);
    assert_eq!(item["by_index"].as_array().map(Vec::len), usize::try_from(count).ok());
    assert_eq!(item["past_the_end"], json!(null));
    assert_eq!(item["first"], item["by_child"]);
    assert_eq!(item["second"], item["by_child"]);
}

/// The extension `wc`: the macro `\join{a}{b}`, whose rule writes the
/// characters of two joined lists of nodes: the sequence of the first
/// argument's content then the one node of the second's, and the other way
/// round.
const JOIN: &str = r#"
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, Mode, SyntaxDef } from "flm:lang/mint";
import { chars_text } from "flm:lang/text";
import { ElementSpec } from "flm:model/element";
import { SPAN } from "flm:stdlib/backends/pres";

const JOIN = "wc.join";

export default function (ext) {
  ext.package("_", { features: { join: { register(b) {
    b.element(new ElementDef(
      b.feature(), ["join"], SyntaxDef.Macro([ArgDef.mandatory("a"), ArgDef.mandatory("b")]), new ElementSpec(JOIN),
    ).modes([Mode.Text]));
    b.lowering_rule(new LoweringRule("wc.join.lower", JOIN, new LoweringDecl().produces(SPAN), {
      lower(node, cx, lw) {
        const a = node.argument_content_nodes_named("a");
        const [b] = node.argument_content_nodes_named("b");
        const text = `${chars_text([a, b])}|${chars_text([b, a])}`;
        return Lowering.Emit([lw.pres(cx, SPAN, {}, [lw.text(cx, text)])]);
      },
    }, DefaultMark.Everywhere));
  } } } });
}
"#;

/// A rule passes a joined list of nodes, a sequence and a single node, to a
/// function that takes nodes: the parts arrive in order.
#[test]
fn a_joined_list_of_nodes_reaches_a_function_in_order() {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), JOIN);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let (text, diagnostics) = text_of(&registry, "\\join{ab}{c}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text.trim(), "abc|cab");
}

/// The extension `wc`: the macro `\pair{a}{b}`, whose rule passes `pres` a
/// joined list of staged nodes: the sequence of the lowered second argument,
/// a staged text, then the sequence of the lowered first argument.
const PAIR: &str = r#"
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, Mode, SyntaxDef } from "flm:lang/mint";
import { ElementSpec } from "flm:model/element";
import { SPAN } from "flm:stdlib/backends/pres";

const PAIR = "wc.pair";

export default function (ext) {
  ext.package("_", { features: { pair: { register(b) {
    b.element(new ElementDef(
      b.feature(), ["pair"], SyntaxDef.Macro([ArgDef.mandatory("a"), ArgDef.mandatory("b")]), new ElementSpec(PAIR),
    ).modes([Mode.Text]));
    b.lowering_rule(new LoweringRule("wc.pair.lower", PAIR, new LoweringDecl().produces(SPAN), {
      lower(node, cx, lw) {
        const a = lw.lower_arg_named(cx, node, "a");
        const b = lw.lower_arg_named(cx, node, "b");
        return Lowering.Emit([lw.pres(cx, SPAN, {}, [b, lw.text(cx, "|"), a])]);
      },
    }, DefaultMark.Everywhere));
  } } } });
}
"#;

/// A rule passes a joined list of staged nodes, a sequence, a single staged
/// node and another sequence, to `pres`: the parts arrive in order.
#[test]
fn a_joined_list_of_staged_nodes_reaches_pres_in_order() {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), PAIR);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let (text, diagnostics) = text_of(&registry, "\\pair{a \\emph{b}}{c}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text.trim(), "c|a b");
}
