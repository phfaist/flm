// tests/js/exercises/main.js: a numbered, referenceable environment written in
// JavaScript: `\begin{exercise}[title]\label{ex:…} … \end{exercise}`, numbered by a
// counter of its own, with `\hint{…}` legal inside the environment alone, a heading
// line "Exercise 3 (title)", and the recomposed LaTeX written as the theorems feature
// writes its kinds. The package `intoc` lists exercises in the table of contents.
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole, RefPoint, NumberSource, AnchorPolicy, ContentRef, AcceptedRefTypes } from "flm:model/aspects";
import * as refs from "flm:stdcore/features/refs";
import { COUNTERS, COUNTERS_SLOT, STYLES_SLOT, CounterDecl, CounterStyleDecl, FormatOptions, PrefixWords }
  from "flm:stdcore/features/numbering";
import * as latex from "flm:stdcore/backends/latex";
import { BLOCK, SPAN, BODY_SLOT } from "flm:stdcore/backends/pres";

const EXERCISE = "exercises.exercise";          // the element
const HINT = "exercises.hint";                  // the element of \hint
const HINTS = "exercises.hints";                // the scoped package holding \hint
const COUNTER = "exercises.exercise";           // the counter
const STYLE = "exercises.style.exercise";       // its style
const PIECE = "exercises.env";                  // the preamble piece's name
const PIECE_ID = latex.preamble.ID_PREFIX + PIECE;   // "flm.latex.def.exercises.env"

// The reference point of an exercise: its labels, an anchor from the first label or an
// automatic one, and the number source naming the counter and the style.
function points(node) {
  const [labels, malformed] = refs.point_labels(refs.label_uses(node));
  return [new RefPoint({
    kind: "exercise",
    labels, malformed, computed: 0,
    anchor: labels.length ? AnchorPolicy.FromFirstLabel : AnchorPolicy.Auto({ prefix: "exercise" }),
    number: new NumberSource({ counter: COUNTER, style: STYLE, advance: [0] }),
    content: null,
    accepts: AcceptedRefTypes.these(["ex"]),
  })];
}

// The heading line: the number written by the counters table (the word left to the
// text here, the delimiters left out), then the title in parentheses when there is one.
function heading(node, cx, lw) {
  const point = { addr: lw.addr(), index: 0 };
  const children = [lw.text(cx, "Exercise ")];
  const counters = lw.get(COUNTERS)?.counters ?? null;
  const segments = counters?.format(point, new FormatOptions().without_word().without_delimiters()) ?? [];
  for (const segment of segments) children.push(...lw.splice(cx, segment.content));
  if (node.argument_is_provided("title")) {
    children.push(lw.text(cx, " ("), ...lw.lower_arg_named(cx, node, "title"), lw.text(cx, ")"));
  }
  return lw.pres(cx, SPAN, { role: "exercises.heading" }, children);
}

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      exercises: {
        title: "Exercises",
        register(b) {
          const owner = b.feature();
          // The environment: an optional title, the `\label` fields, a body of blocks in
          // which the scoped package `exercises.hints` is available.
          b.element(new ElementDef(owner, ["exercise"],
            SyntaxDef.Environment([ArgDef.optional("title"), refs.label_fields_arg()], new BodyDef().blocks().scoped(HINTS)),
            new ElementSpec(EXERCISE)
              .with(Aspect.BlockRoleAspect(BlockRole.Block))
              .with(Aspect.Referenceable({ points })),
          ).docs("`\\begin{exercise}[title]\\label{ex:…}…\\end{exercise}`: a numbered exercise."));
          // `\hint{…}`, legal inside an exercise alone.
          b.scoped_package(new ScopedPackageDef(HINTS, [
            new ElementDef(owner, ["hint"], SyntaxDef.Macro([ArgDef.mandatory("text")]),
              new ElementSpec(HINT).with(Aspect.BlockRoleAspect(BlockRole.DEFAULT)),
            ).docs("`\\hint{…}`: a hint, written inside an exercise."),
          ], "The commands available inside an exercise: `\\hint`."));
          // The counter and its style, contributed to the numbering feature as data; the
          // style's words are what `\ref` writes before the number.
          const style = new CounterStyleDecl(STYLE);
          style.prefix_words = new PrefixWords("Exercise~", "Exercises~");
          b.peer_slot(STYLES_SLOT, style);
          b.peer_slot(COUNTERS_SLOT, new CounterDecl(COUNTER, STYLE).docs("The exercises, numbered through the document."));
          // The preamble piece of the recomposed LaTeX, contributed to the LaTeX family's
          // slot as data; the LaTeX rule requires it.
          b.peer_slot(latex.preamble.SLOT, new latex.LatexPreambleDef(PIECE,
            "\\newtheorem{exercise}{Exercise}\n", "The `exercise` environment."));
          // The tables and the backend's constructs the rules refer to.
          b.peer_table(COUNTERS, "The counter values, read for an exercise's number.");
          b.peer_table(refs.ANCHORS, "The anchors, read for an exercise's anchor.");
          b.peer_table(refs.SAFE_LABELS, "The safe labels, read by the LaTeX rule.");
          b.peer_backend(latex.RECOMPOSED_BACKEND, "The recomposed LaTeX, on which the LaTeX rule is the default.");
          b.peer_element(latex.ENVIRONMENT, "The LaTeX family's environment construct.");
          b.peer_element(latex.MACRO, "The LaTeX family's macro construct, in which `\\label` is written.");
          b.peer_element(latex.RAW, "The LaTeX family's raw construct, which the label helpers write.");
          // The general rule, on HTML and text: a block holding the heading line and the body.
          b.lowering_rule(new LoweringRule("exercises.exercise.lower.default", EXERCISE,
            new LoweringDecl().reads_opt(COUNTERS).reads_opt(refs.ANCHORS).produces(BLOCK).produces(SPAN),
            { lower(node, cx, lw) {
                const anchor = lw.get(refs.ANCHORS)?.anchors.id({ addr: lw.addr(), index: 0 }) ?? null;
                const content = [heading(node, cx, lw), ...lw.lower_body(cx, node)];
                return Lowering.Emit([lw.pres(cx, BLOCK, { role: EXERCISE, anchor }, content)]);
            } },
            DefaultMark.OnBackends(["html", "text"]),
          ).docs("An exercise lowers to a block: its heading line, then its body."));
          // The LaTeX rule: `\begin{exercise}[title]\label{…} … \end{exercise}`, the labels
          // staged through the references feature's helpers, the piece required.
          b.lowering_rule(new LoweringRule("exercises.exercise.lower.latex-recomposed", EXERCISE,
            new LoweringDecl().reads_opt(refs.SAFE_LABELS).option("LatexRecomposedRenderOptions")
              .produces(latex.ENVIRONMENT).produces(latex.MACRO).produces(latex.RAW),
            { lower(node, cx, lw) {
                lw.require(PIECE_ID);
                const shapes = [], slots = [];
                if (node.argument_is_provided("title")) {
                  shapes.push(latex.LatexArgShape.Optional);
                  slots.push([latex.ARG_SLOT, lw.lower_arg_named(cx, node, "title")]);
                }
                const labels = refs.label_uses(node).map(use => refs.stage_label(cx, lw, use.text, use.label));
                slots.push([BODY_SLOT, [...refs.stage_label_group(cx, lw, labels, false), ...lw.lower_body(cx, node)]]);
                return Lowering.Emit([lw.pres_slots(cx, latex.ENVIRONMENT, new latex.LatexEnvironmentPayload("exercise", shapes), slots)]);
            } },
            DefaultMark.OnBackends([latex.RECOMPOSED_BACKEND]),
          ).docs("An exercise is written as the LaTeX environment `exercise`."));
          // `\hint`: a span on every backend.
          b.lowering_rule(new LoweringRule("exercises.hint.lower.default", HINT,
            new LoweringDecl().produces(SPAN),
            { lower(node, cx, lw) {
                return Lowering.Emit([lw.pres(cx, SPAN, { role: HINT }, [lw.text(cx, "Hint: "), ...lw.lower_arg_named(cx, node, "text")])]);
            } },
            DefaultMark.Everywhere,
          ).docs("A hint lowers to a span: the word, then its text."));
        },
      },
    },
  });
  // The titled exercises listed in the table of contents, at level 3, under their titles: the
  // `Sectioning` aspect attached from another package onto the first one's element.
  ext.package("intoc", {
    uses: [".#_"],
    features: {
      intoc: {
        register(b) {
          b.attach(EXERCISE, Aspect.Sectioning({
            level(node) { return 3; },
            in_toc(node) { return node.argument_is_provided("title"); },
            toc_title(node) { return ContentRef.ArgumentNamed("title"); },
          }));
        },
      },
    },
  });
}
