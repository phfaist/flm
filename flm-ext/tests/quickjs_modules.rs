//! The prelude's stubs of step E4 reached from a script (WP-4a; that every
//! export of every module is reachable is `tests/quickjs_exports.rs`'s): the
//! new classes and functions of `flm:model/aspects`,
//! `flm:stdcore/features/refs`, `flm:stdcore/features/numbering`,
//! `flm:stdcore/backends/latex` (its namespaces `latex.preamble` and `latex.packages`),
//! `flm:core/registry`, `flm:model/element`, `flm:lang/mint` and
//! `flm:lang/rewrites/userdefs`, one smoke call per new method forwarding to
//! its arm: an
//! extension `sm` whose feature declares a numbered, referenceable,
//! sectioning box with a scoped package, contributes to the five data-valued
//! slots, joins a collection, registers a late-initialization hook and a
//! finalizer, and lowers the box on the default backends (reading the
//! counters and the anchors) and on the recomposed LaTeX (the label helpers
//! and the four payloads). A second extension `rl` declares an argument role
//! and a peer argument role, and sets each on an argument.

#![cfg(feature = "quickjs")]

mod common;

use flm_core::core::registry::PackageResolver;
use flm_core::core::settings::PackageSettings;
use flm_ext::quickjs::{JsHost, JsSettings};

/// The extension's main module.
const MAIN: &str = r##"
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, BodyDef, ElementDef, Mode, ScopedPackageDef, SyntaxDef } from "flm:lang/mint";
import { Definition, UserDefinition } from "flm:lang/rewrites/userdefs";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole, RefPoint, NumberSource, AnchorPolicy, ContentRef, AcceptedRefTypes } from "flm:model/aspects";
import * as refs from "flm:stdcore/features/refs";
import {
  COUNTERS, COUNTERS_SLOT, STYLES_SLOT, PARENT_SETS_SLOT, CounterDecl, CounterStyleDecl, ParentSetDecl, Within,
  PrefixWords, PluralWords, Delimiters, NumberTemplate, FormatOptions,
} from "flm:stdcore/features/numbering";
import * as latex from "flm:stdcore/backends/latex";
import { BODY_SLOT } from "flm:stdcore/backends/pres";
import { StageDecl, Effect, Scope } from "flm:core/stage";

const BOX = "sm.box";

// The box's reference point: numbered in the counter `sm.box`, anchored at
// its first label, else after a slug of its content.
function points(node) {
  const [labels, malformed] = refs.point_labels(refs.label_uses(node));
  return [new RefPoint({
    kind: "smbox", labels, malformed,
    anchor: labels.length ? AnchorPolicy.FromFirstLabel : AnchorPolicy.Slug({ prefix: "sm", content: ContentRef.ArgumentNamed("content") }),
    number: new NumberSource({ counter: BOX, style: BOX, advance: [0] }),
    content: ContentRef.Argument(0),
    accepts: AcceptedRefTypes.these(["smbox"]),
  })];
}

// The default rule: the box's content, then what the counters and the
// anchors answer for its point.
function lower_default(node, cx, lw) {
  const point = { addr: lw.addr(), index: 0 };
  const counters = lw.get(COUNTERS).counters;
  const anchors = lw.get(refs.ANCHORS).anchors;
  const record = counters.record(point);
  const segments = counters.format(point, new FormatOptions().without_word().without_delimiters().variant("short"));
  const summary = ` [${record.counter} ${record.value.join(".")} ${record.style}; ${segments.length} segment; ${anchors.id(point)}]`;
  return Lowering.Emit([...lw.lower_arg_named(cx, node, "content"), lw.text(cx, summary)]);
}

// The recomposed LaTeX rule: the labels, and the four payloads.
function lower_latex(node, cx, lw) {
  lw.require(latex.preamble.piece_id("sm.piece"));
  const labels = refs.label_uses(node).map(use => refs.stage_label(cx, lw, use.text, use.label));
  const group = refs.stage_label_group(cx, lw, labels, false);
  const bold = lw.pres_slots(cx, latex.MACRO, new latex.LatexMacroPayload("textbf", [latex.LatexArgShape.Mandatory]),
    [[latex.ARG_SLOT, lw.lower_arg_named(cx, node, "content")]]);
  const raw = lw.pres(cx, latex.RAW, new latex.LatexRawPayload("\\smx{}"), []);
  const definition = lw.pres_slots(cx, latex.DEFINITION,
    new latex.LatexDefinitionPayload(latex.LatexDefinitionKind.Macro, "smdef", 0).of_feature("sm.f"),
    [[BODY_SLOT, [lw.text(cx, "D")]]]);
  const env = lw.pres_slots(cx, latex.ENVIRONMENT, new latex.LatexEnvironmentPayload("smenv", []), [[BODY_SLOT, [bold, ...group]]]);
  return Lowering.Emit([raw, definition, env]);
}

let READBACK = null;

export default function (ext) {
  ext.package("_", { features: { f: { register(b) {
    const owner = b.feature();
    // The box: a scoped argument in text mode, its labels, its two aspects.
    b.element(new ElementDef(owner, ["smbox"],
      SyntaxDef.Macro([ArgDef.mandatory("content").scoped("sm.hints").mode(Mode.Text), refs.label_fields_arg()]),
      new ElementSpec(BOX).with(Aspect.BlockRoleAspect(BlockRole.DEFAULT)).with(Aspect.Referenceable({ points }))));
    b.element(new ElementDef(owner, ["smenv"], SyntaxDef.Environment([], new BodyDef().scoped("sm.hints")), new ElementSpec("sm.env")));
    b.scoped_package(new ScopedPackageDef("sm.hints", [
      new ElementDef(owner, ["smhint"], SyntaxDef.Macro([ArgDef.mandatory("text")]), new ElementSpec("sm.hint")),
    ], "The hints, legal inside a box's content."));
    b.attach(BOX, Aspect.Sectioning({ level(node) { return 2; } }));
    // The five data-valued slots, and `contribute_to`.
    const style = new CounterStyleDecl(BOX);
    style.prefix_words = new PrefixWords("Box~", "Boxes~").with_variant("short", new PluralWords("B~", "Bs~"));
    style.delimiters = new Delimiters("(", ")");
    style.number_format = NumberTemplate.formats("#{arabic}");
    b.peer_slot(STYLES_SLOT, style);
    b.peer_slot(COUNTERS_SLOT, new CounterDecl(BOX, BOX).docs("The boxes."));
    b.contribute_to(COUNTERS_SLOT, new CounterDecl("sm.sub", BOX).within(Within.parent(BOX).joined_by("-")));
    b.peer_slot(PARENT_SETS_SLOT, new ParentSetDecl("sm.parents", [BOX]));
    b.peer_slot(latex.preamble.SLOT, new latex.LatexPreambleDef("sm.piece", "\\newcommand{\\smx}{x}", "A piece.").requires([latex.packages.AMSTHM]));
    b.peer_slot(latex.packages.SLOT, new latex.LatexPackageDef("xcolor", "A package.").id("sm.xcolor").options(["dvipsnames"]));
    // A capability joining a collection.
    b.capability("sm.checked", "A check of the boxes.");
    b.collection_member("flm.validated", "sm.checked");
    b.stage({ decl() { return new StageDecl("sm.check", Effect.Check, Scope.Document).provides("sm.checked"); }, run(cx, out) {} });
    // The hooks.
    READBACK = new UserDefinition(Definition.macro_("smstamp", "").content("[#{stamp}]").extra_placeholder("stamp", false)).definition();
    b.late_init({ late_init(lang, diags) {
      diags.push({ id: "sm.readback", severity: "note", message: JSON.stringify(READBACK.extra_placeholders ?? null) });
    } });
    b.finalizer({ finalize(b) { b.table("sm.second", "A table of the second round."); } });
    // The rules.
    b.lowering_rule(new LoweringRule("sm.box.lower.default", BOX, new LoweringDecl().reads(COUNTERS).reads(refs.ANCHORS),
      { lower: lower_default }, DefaultMark.Everywhere));
    b.lowering_rule(new LoweringRule("sm.box.lower.latex-recomposed", BOX,
      new LoweringDecl().reads_opt(refs.SAFE_LABELS).option("LatexRecomposedRenderOptions")
        .produces(latex.MACRO).produces(latex.ENVIRONMENT).produces(latex.RAW).produces(latex.DEFINITION),
      { lower: lower_latex }, DefaultMark.OnBackends([latex.RECOMPOSED_BACKEND])));
    b.lowering_rule(new LoweringRule("sm.hint.lower.default", "sm.hint", new LoweringDecl(),
      { lower(node, cx, lw) { return Lowering.Emit(lw.lower_arg_named(cx, node, "text")); } }, DefaultMark.Everywhere));
    b.lowering_rule(new LoweringRule("sm.env.lower.default", "sm.env", new LoweringDecl(),
      { lower(node, cx, lw) { return Lowering.Emit(lw.lower_body(cx, node)); } }, DefaultMark.Everywhere));
  } } } });
}
"##;

/// The registry of the standard package and the extension `sm`.
fn registry() -> std::sync::Arc<flm_core::core::registry::FlmRegistry> {
    let host = JsHost::new(JsSettings::default());
    let loader = common::js_loader(&host, common::js_extension("sm", &[("main.js", MAIN)], common::js_manifest("main.js")));
    let factory = loader.resolve("sm").expect("the extension loads").expect("found");
    common::registry_with([factory.build(&PackageSettings::new()).expect("the package builds")])
}

/// The document.
const DOCUMENT: &str = "\\smbox{Alpha \\smhint{h}}\\label{smbox:one} and \\smbox{Beta}. See \\ref{smbox:one}. \\begin{smenv}\\smhint{in}\\end{smenv}";

#[test]
fn every_new_stub_forwards_to_its_arm() {
    let registry = registry();
    let found: Vec<String> = registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    let readback: Vec<&String> = found.iter().filter(|text| text.starts_with("sm.readback")).collect();
    assert_eq!(readback, [r#"sm.readback: [{"name":"stamp","substitution":false}]"#], "{found:?}");
    let (text, diagnostics) = common::text_of(&registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The text wraps its lines; the words are compared.
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        text,
        "Alpha h [sm.box 1 sm.box; 1 segment; smbox-one] and Beta [sm.box 2 sm.box; 1 segment; sm--Beta]. See Box (1). in"
    );
    let (latex, diagnostics) = common::latex_of(&registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        latex.contains("\\smx{}\\begin{smenv}\\textbf{Alpha h}\\flmLabels{\\label{ref1}}\\end{smenv} and \\smx{}\\begin{smenv}\\textbf{Beta}\\end{smenv}. See \\flmRef{ref1}."),
        "{latex}"
    );
}

#[test]
fn a_command_of_the_scoped_package_outside_its_scope_is_unknown() {
    let (_, diagnostics) = common::text_of(&registry(), "Outside: \\smhint{x}.");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].contains("smhint"), "{diagnostics:?}");
}

/// The main module of the extension `rl`: its feature declares the argument
/// role `rl.key`, declares the references feature's role `flm.refs.target`
/// as a peer, and sets each on the argument of a command of its own.
const ROLES: &str = r##"
import { ArgDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec } from "flm:model/element";

export default function (ext) {
  ext.package("_", { features: { f: { register(b) {
    const owner = b.feature();
    b.argument_role("rl.key", "A key that a lookup finds.");
    b.peer_argument_role("flm.refs.target");
    b.element(new ElementDef(owner, ["rlkey"], SyntaxDef.Macro([ArgDef.mandatory("key").role("rl.key")]), new ElementSpec("rl.lookup")));
    b.element(new ElementDef(owner, ["rlsee"], SyntaxDef.Macro([ArgDef.mandatory("target").role("flm.refs.target")]), new ElementSpec("rl.see")));
  } } } });
}
"##;

#[test]
fn a_script_declares_an_argument_role_and_a_peer_role_and_sets_them() {
    let host = JsHost::new(JsSettings::default());
    let loader = common::js_loader(&host, common::js_extension("rl", &[("main.js", ROLES)], common::js_manifest("main.js")));
    let factory = loader.resolve("rl").expect("the extension loads").expect("found");
    let registry = common::registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let role = registry.argument_role_named("rl.key").expect("declared");
    assert_eq!(role.docs, "A key that a lookup finds.");
    let uses = registry.declarations().role_uses("rl.key");
    let written: Vec<(&str, Option<&str>)> =
        uses.iter().map(|argument| (argument.declaration.names[0].as_str(), argument.arg.name.as_deref())).collect();
    assert_eq!(written, [("rlkey", Some("key"))]);
    assert_eq!(uses[0].declaration.owner, role.owner);
    let peers: Vec<(&str, bool)> = registry
        .peer_argument_roles()
        .iter()
        .filter(|peer| peer.owner == role.owner)
        .map(|peer| (peer.name.as_str(), peer.resolved))
        .collect();
    assert_eq!(peers, [("flm.refs.target", true)]);
    let targets = registry.declarations().role_uses("flm.refs.target");
    assert!(targets.iter().any(|argument| argument.declaration.names[0] == "rlsee"), "{targets:?}");
}
