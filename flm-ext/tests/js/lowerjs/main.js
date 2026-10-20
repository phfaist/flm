// The lowering façade's test fixture: a feature declaring `\ljbox{text}` in
// text mode, with a scripted lowering rule staging a block on the HTML and
// text backends, and one staging the span alone on the recomposed LaTeX
// (whose backend writes no block); a stage numbering the boxes into a table,
// each row keyed by `cx.addr_of(node)`, which the rules read at each box with
// `lw.get_at`; a stylesheet whose one part a document-scope
// stage writes; a peer table read optionally. The package `dup` declares a
// second rule under the first one's name.
import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
import { StyleDef, DefContent } from "flm:core/registry";
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, Mode, SyntaxDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole } from "flm:model/aspects";
import { ANCHORS } from "flm:stdcore/features/math";
import { BLOCK, SPAN } from "flm:stdcore/backends/pres";

const BOX = "lowerjs.box";
const RULE = "lowerjs.box.lower.default";
const NUMBERS = "lowerjs.numbers";
const CSS = "lowerjs.css";
const STYLE = "lowerjs.style";

// A span holding the box's lowered argument and a text leaf with its number
// (and whether the anchors were read), staged in one call.
function inner(node, cx, lw) {
  const content = lw.lower_arg_named(cx, node, "text");
  const [row] = lw.get_at(NUMBERS, node) ?? [];
  const anchors = lw.get(ANCHORS) === null ? "" : ", anchors read";
  return lw.pres_tree(cx, {
    element: SPAN,
    payload: { kinds: ["lowerjs-inner"] },
    children: [...content.map(build => ({ build })), { text: ` (box ${row?.number ?? "?"}${anchors})` }],
  });
}

// The rule staging a block around the span, on the HTML and text backends;
// as both packages give it.
function box_rule(docs) {
  return new LoweringRule(
    RULE, BOX,
    new LoweringDecl().produces(BLOCK).produces(SPAN).reads(NUMBERS).reads_opt(ANCHORS)
      .requires(CSS).selection_requires(STYLE),
    { lower(node, cx, lw) { return Lowering.Emit([lw.pres(cx, BLOCK, { role: BOX }, [inner(node, cx, lw)])]); } },
    DefaultMark.OnBackends(["html", "text"]),
  ).docs(docs);
}

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      boxes: {
        title: "Boxes",
        register(b) {
          b.element(new ElementDef(
            b.feature(), ["ljbox"], SyntaxDef.Macro([ArgDef.mandatory("text")]),
            new ElementSpec(BOX).with(Aspect.BlockRoleAspect(BlockRole.Block)),
          ).modes([Mode.Text]).docs("A box around its text."));
          b.table(NUMBERS, "The number of each box, by its address.");
          b.stage({
            decl() { return new StageDecl("lowerjs.number", Effect.Compute, Scope.Document).writes(NUMBERS); },
            run(cx, out) {
              let number = 0;
              for (const { node } of cx.walk())
                if (node.element_name() === BOX) out.table(NUMBERS).push({ addr: cx.addr_of(node), number: ++number });
            },
          });
          b.definition_text_table(CSS, "The text of the boxes' stylesheet.");
          b.stage({
            decl() { return new StageDecl("lowerjs.write-css", Effect.Compute, Scope.Document).writes(CSS); },
            run(cx, out) { out.table(CSS).push(".lowerjs-box{border:1px solid}"); },
          });
          b.style_def(new StyleDef(STYLE, [DefContent.Table(CSS)], "The boxes' stylesheet."));
          b.peer_table(ANCHORS, "The anchors, read when the references feature is there.");
          b.lowering_rule(box_rule("Lowers a box to a block."));
          b.lowering_rule(new LoweringRule(
            "lowerjs.box.lower.latex-recomposed", BOX,
            new LoweringDecl().produces(SPAN).reads(NUMBERS).reads_opt(ANCHORS),
            { lower(node, cx, lw) { return Lowering.Emit([inner(node, cx, lw)]); } },
            DefaultMark.OnBackends(["latex-recomposed"]),
          ));
        },
      },
    },
  });
  ext.package("dup", {
    uses: [".#_"],
    features: {
      again: {
        register(b) {
          b.peer_table(ANCHORS, "The anchors.");
          b.lowering_rule(box_rule("The same rule's name, a second time."));
        },
      },
    },
  });
}
