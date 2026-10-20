// _flmextensions/loud/main.js: a feature whose documentation texts are not valid FLM (each
// holds a `%` and a brace that is not closed), which `loud.flm` uses: the feature's text and
// that of its macro `\shout`, whose first paragraph fails too. `flm info` shows each as
// plain text, with a warning.
import { ArgDef, ElementDef, Mode, SyntaxDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole } from "flm:model/aspects";
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { SPAN } from "flm:stdcore/backends/pres";

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      loud: {
        title: "Loud",
        docs: "Loud text, 50% {louder.",
        register(b) {
          b.element(
            new ElementDef(
              b.feature(),
              ["shout"],
              SyntaxDef.Macro([ArgDef.mandatory("what").docs("The text to shout.")]),
              new ElementSpec("loud.shout").with(Aspect.BlockRoleAspect(BlockRole.Block)),
            )
              .modes([Mode.Text])
              .docs("Like `\\textbf`, but 100% {louder.\n\nIts argument is the text."),
          );
          b.lowering_rule(
            new LoweringRule(
              "loud.shout.lower",
              "loud.shout",
              new LoweringDecl().produces(SPAN),
              {
                lower(node, cx, lw) {
                  return Lowering.Emit([lw.pres(cx, SPAN, {}, lw.lower_arg_named(cx, node, "what"))]);
                },
              },
              DefaultMark.OnBackends(["html", "text", "latex-recomposed"]),
            ).docs("Lowers a shout to a span of text."),
          );
        },
      },
    },
  });
}
