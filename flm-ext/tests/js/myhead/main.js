// tests/js/myhead/main.js: the headings sketch reduced to one kind: `\myhead*{title}\label{…}`,
// a numbered heading of level 1 listed in the table of contents, whose number is written
// through a title template compiled against the built language, and whose recomposed
// LaTeX is `\section{title}` with its labels.
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole, RefPoint, NumberSource, AnchorPolicy, ContentRef, AcceptedRefTypes } from "flm:model/aspects";
import { TemplateLanguage, TemplateSpec, ValueBindings, compile_template_with, instantiate }
  from "flm:lang/rewrites/userdefs";
import * as refs from "flm:stdcore/features/refs";
import { COUNTERS, COUNTERS_SLOT, STYLES_SLOT, CounterDecl, CounterStyleDecl, FormatOptions }
  from "flm:stdcore/features/numbering";
import * as latex from "flm:stdcore/backends/latex";
import { BLOCK_HEADING, SPAN } from "flm:stdcore/backends/pres";

const HEADING = "myhead.heading";
const COUNTER = "myhead.heading";
const STYLE = "myhead.style.heading";
const SETTINGS = {
  config: {
    docs: "How a heading's number joins its title.",
    schema: { type: "object", properties: { format: { type: "string", description: "The title template; `#{number}` is the number." } } },
  },
};

// The title template's specification: one extra placeholder, `number`.
function spec(format) {
  return new TemplateSpec({
    text: format, mode: "text",
    vocabulary: { arity: 0, extras: [{ name: "number", substitution: false }] },
    definition: "the heading title of myhead", label: "myhead.format",
    origin_label: "the setting `format` of myhead", user_definition: false,
  });
}

// The compiled title template: compiled once per registry (the adapter keeps it; a
// second call with the same specification answers the same object), against the
// language the rule or the hook was lent.
function title_template(lang, format) {
  return compile_template_with(spec(format), lang, new TemplateLanguage(lang));
}

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: { myhead: { title: "My heading", settings: SETTINGS, register } },
  });
}

function register(b, settings) {
  const format = settings.format ?? "#{number}. ";
  const owner = b.feature();
  const numbered = node => !node.argument_is_provided("star");
  b.element(new ElementDef(owner, ["myhead"],
    SyntaxDef.Macro([ArgDef.named("star", "*"), ArgDef.mandatory("title"), refs.label_fields_arg()]),
    new ElementSpec(HEADING)
      .with(Aspect.BlockRoleAspect(BlockRole.Block))
      .with(Aspect.Sectioning({
        level(node) { return 1; },
        in_toc(node) { return numbered(node); },
        toc_title(node) { return ContentRef.ArgumentNamed("title"); },
      }))
      .with(Aspect.Referenceable({
        points(node) {
          const [labels, malformed] = refs.point_labels(refs.label_uses(node));
          return [new RefPoint({
            kind: "myhead",
            labels, malformed, computed: 0,
            anchor: labels.length ? AnchorPolicy.FromFirstLabel
              : AnchorPolicy.Slug({ prefix: "myhead", content: ContentRef.ArgumentNamed("title") }),
            number: numbered(node) ? new NumberSource({ counter: COUNTER, style: STYLE, advance: [0] }) : null,
            content: ContentRef.ArgumentNamed("title"),
            accepts: AcceptedRefTypes.these(["sec"]),
          })];
        },
      })),
  ).docs("`\\myhead*{title}\\label{sec:…}`: a heading of level 1; the star leaves it unnumbered and unlisted."));
  b.peer_slot(STYLES_SLOT, new CounterStyleDecl(STYLE));
  b.peer_slot(COUNTERS_SLOT, new CounterDecl(COUNTER, STYLE).docs("The headings of myhead."));
  b.peer_table(COUNTERS, "The counter values, read for a heading's number.");
  b.peer_table(refs.ANCHORS, "The anchors, read for a heading's anchor.");
  b.peer_table(refs.SAFE_LABELS, "The safe labels, read by the LaTeX rule.");
  b.peer_backend(latex.RECOMPOSED_BACKEND, "The recomposed LaTeX.");
  b.peer_element(latex.MACRO, "The LaTeX family's macro construct.");
  b.peer_element(latex.RAW, "The LaTeX family's raw construct.");
  // The check against the built language: a format that does not compile is reported
  // once per build, and the rule then writes the title alone.
  b.late_init({
    late_init(lang, diags) {
      try { title_template(lang, format); }
      catch (error) {
        diags.push({ id: "myhead.bad-format", severity: "error",
          message: `the setting \`format\` of myhead must be a template of the number: ${error.message}` });
      }
    },
  });
  // The default rule: the number, written through the title template, then the title.
  b.lowering_rule(new LoweringRule("myhead.heading.lower.default", HEADING,
    new LoweringDecl().reads_opt(refs.ANCHORS).reads_opt(COUNTERS).produces(BLOCK_HEADING).produces(SPAN),
    { lower(node, cx, lw) {
        const point = { addr: lw.addr(), index: 0 };
        const anchor = lw.get(refs.ANCHORS)?.anchors.id(point) ?? null;
        const counters = lw.get(COUNTERS)?.counters ?? null;
        const content = [];
        if (counters?.record(point)) {
          const segments = counters.format(point, new FormatOptions().without_word().without_delimiters());
          let template = null;
          try { template = title_template(lw.lang(), format); } catch (error) { template = null; }
          if (template !== null) {
            const bindings = new ValueBindings().with("number", segments[0].content);
            content.push(lw.pres(cx, SPAN, { role: "myhead.number" }, lw.splice(cx, instantiate(node, template, bindings))));
          }
        }
        content.push(...lw.lower_arg_named(cx, node, "title"));
        const payload = { name: "myhead", level: 1, in_toc: numbered(node), anchor };
        return Lowering.Emit([lw.pres(cx, BLOCK_HEADING, payload, content)]);
    } },
    DefaultMark.OnBackends(["html", "text"]),
  ).docs("A heading lowers to a block heading: its number through the title template, then its title."));
  // The recomposed LaTeX: `\section{title}` (`\section*` when starred), then the labels.
  b.lowering_rule(new LoweringRule("myhead.heading.lower.latex-recomposed", HEADING,
    new LoweringDecl().reads_opt(refs.SAFE_LABELS).option("LatexRecomposedRenderOptions").produces(latex.MACRO).produces(latex.RAW),
    { lower(node, cx, lw) {
        const shapes = numbered(node) ? [latex.LatexArgShape.Mandatory] : [latex.LatexArgShape.Star, latex.LatexArgShape.Mandatory];
        const heading = lw.pres_slots(cx, latex.MACRO, new latex.LatexMacroPayload("section", shapes),
          [[latex.ARG_SLOT, lw.lower_arg_named(cx, node, "title")]]);
        const labels = refs.label_uses(node).map(use => refs.stage_label(cx, lw, use.text, use.label));
        return Lowering.Emit([heading, ...refs.stage_label_group(cx, lw, labels, false)]);
    } },
    DefaultMark.OnBackends([latex.RECOMPOSED_BACKEND]),
  ).docs("A heading is written as `\\section{title}` with its labels."));
}
