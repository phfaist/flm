// tests/js/smartdash/main.js: a rewrite stage, `smartdash.dash`, replacing `--` and
// `---` inside characters by an en dash and an em dash, outside mathematics and
// verbatim, and rewriting `\sdquote{text}` into a tree built from parts: an opening
// quotation mark (characters), the call itself with its argument restaged (a macro
// call with arguments, its children kept), a closing quotation mark. The stage
// publishes `smartdash.dashed`; the workflow `smartdash` demands it.
//
// The argument orders of the prelude this module uses:
//   new RewriteDecl(name, publishes).docs(text)
//   new Needs().after(capability).before(capability)
//   b.rewrite_stage({ decl(), needs(), restage(node, cx, rs) })
//   rs.stage_chars(text, like)                      a characters node, placed like `like`
//   rs.restage_subtree(node)                        → [staged, …]
//   rs.restage_argument_with_content(node, index, [staged, …])
//   rs.restage_invocation(node, [argument, …], [slot, …])
//   Restage.Descend, Restage.Emit([staged, …])
//   new ElementDef(owner, names, syntax, element).docs(text)
//   new LoweringRule(name, element, decl, body, mark).docs(text)
//   lw.pres(cx, element, payload, children)
import { Needs, RewriteDecl } from "flm:core/stage";
import { Restage } from "flm:techy/transform";
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, Mode, SyntaxDef } from "flm:lang/mint";
import { ElementSpec } from "flm:model/element";
import { SPAN } from "flm:stdcore/backends/pres";

const STAGE = "smartdash.dash";
const DASHED = "smartdash.dashed";
const QUOTE = "smartdash.quote";
const VERBATIM = ["flm.verbatim.macro", "flm.verbatim.environment"];

// Whether `node` stands inside mathematics or inside verbatim content.
function opaque(node) {
  if (node.mode() === Mode.Math) return true;
  for (let up = node.parent(); up !== null; up = up.parent())
    if (VERBATIM.includes(up.element_name())) return true;
  return false;
}

function dashed(text) { return text.replace(/---/g, "—").replace(/--/g, "–"); }

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    flm: { workflows: { smartdash: { extends: "text", demands: [DASHED], docs: "The text output, with the dashes replaced." } } },
    features: {
      dashes: { title: "Smart dashes", docs: "Replaces `--` and `---` by dashes before the trees are frozen.", register },
    },
  });
}

function register(b) {
  b.element(new ElementDef(
    b.feature(), ["sdquote"], SyntaxDef.Macro([ArgDef.mandatory("text")]), new ElementSpec(QUOTE),
  ).docs("A quotation, set between quotation marks by the rewrite."));
  b.rewrite_stage({
    decl: () => new RewriteDecl(STAGE, DASHED).docs("Replaces `--` and `---` by dashes and sets `\\sdquote` between quotation marks; publishes the trees as `smartdash.dashed`."),
    needs: () => new Needs().after("flm.userdefs").before("flm.blocks.structured"),
    restage(node, cx, rs) {
      if (node.is_chars()) {
        const text = node.chars();
        if (!text.includes("--") || opaque(node)) return Restage.Descend;
        return Restage.Emit([rs.stage_chars(dashed(text), node)]);          // shape 1: characters replaced
      }
      if (node.element_name() === QUOTE) {                                   // shape 2: a tree built from parts
        const content = (node.argument_content_nodes_named("text") ?? []).flatMap(child => rs.restage_subtree(child));
        const argument = rs.restage_argument_with_content(node, 0, content);
        return Restage.Emit([
          rs.stage_chars("“", node),
          rs.restage_invocation(node, [argument], []),
          rs.stage_chars("”", node),
        ]);
      }
      return Restage.Descend;
    },
  });
  b.lowering_rule(new LoweringRule(
    "smartdash.quote.lower.default", QUOTE, new LoweringDecl().produces(SPAN),
    { lower(node, cx, lw) { return Lowering.Emit([lw.pres(cx, SPAN, { kinds: ["smartdash-quote"] }, lw.lower_arg_named(cx, node, "text"))]); } },
    DefaultMark.Everywhere,
  ).docs("A quotation as a span of kind `smartdash-quote`."));
}
