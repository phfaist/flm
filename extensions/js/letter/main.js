import { ArgDef, ElementDef, SyntaxDef, BodyDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole } from "flm:model/aspects";
import { LoweringRule, LoweringDecl, Lowering, DefaultMark } from "flm:core/lowering";
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { TAG } from "flm:stdlib/backends/html";
import * as latex from "flm:stdlib/backends/latex";
import { BLOCK, PAR, BODY_SLOT } from "flm:stdlib/backends/pres";
import { TEMPLATES_SLOT } from "flm:stdlib/features/compose";
import htmlTemplate from "./templates/html/letter.js";
import latexTemplate from "./templates/latex-rendered/letter.js";
import { SETTINGS } from "./templates/options.js";

const LETTER = "letter.letter";
const FIELDS = ["fromaddress", "toaddress", "lettersubject", "letterdate", "salutation", "closing", "signature", "enclosed"];
const ENVIRONMENTS = ["fromaddress", "toaddress", "signature", "enclosed"];
const COMMANDS = ["lettersubject", "letterdate", "salutation", "closing", "enclosed"];
const FIELD_ELEMENTS = ["flm.quote", ...COMMANDS.map(name => `letter.${name}`)];
const isField = node => (node.element_name() === "flm.quote" && ENVIRONMENTS.includes(node.name()))
  || COMMANDS.some(name => node.element_name() === `letter.${name}`);
const fieldContent = node => [...(node.element_name() === "flm.quote" ? node.body() ?? [] : node.argument_content_nodes_named("text") ?? [])];
const lowerField = (node, cx, lw) => node.element_name() === "flm.quote" ? lw.lower_nodes(cx, [node]) : lw.lower_arg_named(cx, node, "text");

function meaningful(node) {
  if (node.is_comment()) return false;
  if (node.is_chars()) return node.chars().trim() !== "";
  // A callable can have visible effects without textual children (a note
  // marker, reference or image). Only wrappers are inspected recursively.
  if (node.is_callable()) return node.name() !== "\\";
  for (const child of node.children()) if (meaningful(child)) return true;
  return false;
}

// Only direct fields are collected. Their actual document nodes are lowered once,
// so links, mathematics, references and footnotes keep their normal FLM behavior.
function parts(node) {
  const fields = {};
  const body = [];
  for (const child of node.body() ?? []) {
    if (isField(child)) {
      const name = child.name();
      if (fieldContent(child).some(meaningful) && fields[name] === undefined) fields[name] = child;
    } else {
      body.push(child);
    }
  }
  return { fields, body };
}

function tag(cx, lw, name, className, children, attributes = []) {
  return lw.pres(cx, TAG, { tag: name, attributes: [["class", className], ...attributes], block: true }, children);
}

function htmlLetter(node, cx, lw) {
  const { fields, body } = parts(node);
  const field = name => fields[name] === undefined ? [] : [tag(cx, lw, "div", `flm-letter-${name}`, lowerField(fields[name], cx, lw))];
  const header = tag(cx, lw, "header", "flm-letter-header", [
    tag(cx, lw, "div", "flm-letter-logo", [], [["aria-hidden", "true"]]),
    ...field("fromaddress"), ...field("toaddress"), ...field("letterdate"),
  ]);
  const closing = fields.closing || fields.signature
    ? [tag(cx, lw, "div", "flm-letter-signoff", [...field("closing"), ...field("signature")])] : [];
  return Lowering.Emit([tag(cx, lw, "article", "flm-letter", [
    header, ...field("lettersubject"), ...field("salutation"),
    tag(cx, lw, "div", "flm-letter-body", lw.lower_nodes(cx, body)),
    ...closing, ...field("enclosed"),
  ])]);
}

function texMacro(cx, lw, name, args) {
  return lw.pres_slots(cx, latex.MACRO, new latex.LatexMacroPayload(name, args.map(() => latex.LatexArgShape.Mandatory)),
    args.map(content => [latex.ARG_SLOT, content]));
}

function latexLetter(node, cx, lw) {
  const { fields, body } = parts(node);
  const field = name => fields[name] === undefined ? [] : lowerField(fields[name], cx, lw);
  const optional = (fieldName, macro) => fields[fieldName] === undefined ? [] : [texMacro(cx, lw, macro, [field(fieldName)])];
  const content = [texMacro(cx, lw, "flmxLetterHeader", [field("fromaddress"), field("toaddress"), field("letterdate")]),
    ...optional("lettersubject", "flmxLetterSubject"), ...optional("salutation", "flmxLetterSalutation"),
    lw.pres_slots(cx, latex.ENVIRONMENT, new latex.LatexEnvironmentPayload("flmxLetterBody", []),
      [[BODY_SLOT, lw.lower_nodes(cx, body)]]),
  ];
  if (fields.closing || fields.signature) content.push(texMacro(cx, lw, "flmxLetterSignoff", [field("closing"), field("signature")]));
  content.push(...optional("enclosed", "flmxLetterEnclosed"));
  return Lowering.Emit(content);
}

function register(b) {
  const owner = b.feature();
  const block = name => new ElementSpec(name).with(Aspect.BlockRoleAspect(BlockRole.Block));
  b.element(new ElementDef(owner, ["letter"],
    SyntaxDef.Environment([], new BodyDef().blocks()), block(LETTER))
    .docs("One letter: optional fields in any order, with ordinary FLM paragraphs as its body."));
  for (const name of COMMANDS) {
    const element = `letter.${name}`;
    b.element(new ElementDef(owner, [name], SyntaxDef.Macro([ArgDef.mandatory("text")]), block(element))
      .docs(`The optional ${name} field, as inline FLM.`));
    // Fields are consumed by the letter rule; declare their inline rendering
    // as well so the plan can establish every element's renderability.
    b.lowering_rule(new LoweringRule(`${element}.lower`, element, new LoweringDecl(),
      { lower(node, cx, lw) { return Lowering.Emit(lw.lower_arg_named(cx, node, "text")); } }, DefaultMark.Everywhere));
  }

  b.capability("letter.checked", "The letter's structure and unique fields have been checked.", null);
  b.collection_member("flm.validated", "letter.checked");
  b.stage({
    decl() { return new StageDecl("letter.check", Effect.Check, Scope.Document).provides("letter.checked"); },
    run(cx, out) {
      let letters = 0;
      // The letters, and the elements a field can be, wherever they stand.
      for (const { node } of cx.walk().elements([LETTER, ...FIELD_ELEMENTS])) {
        if (node.element_name() === LETTER) {
          letters++;
          let parent = node.parent();
          while (parent !== null && parent.is_list()) parent = parent.parent();
          if (parent !== null) out.diags({ id: "letter.misplaced-letter", severity: "error", at: node,
            message: "The ‘letter’ environment must be at the document's top level" });
          const seen = new Set();
          for (const child of node.body() ?? []) {
            const name = child.name();
            if (!isField(child)) continue;
            if (seen.has(name)) out.diags({ id: "letter.duplicate-field", severity: "error", at: child,
              message: `Duplicate letter field ‘${child.name()}’: each field may occur only once` });
            seen.add(name);
          }
        }
        if (isField(node)) {
          let parent = node.parent();
          while (parent !== null && parent.is_list()) parent = parent.parent();
          if (parent === null || parent.element_name() !== LETTER) out.diags({
            id: "letter.misplaced-field", severity: "error", at: node,
            message: `Misplaced letter field ‘${node.name()}’: fields must be directly inside ‘letter’`,
          });
        }
      }
      if (letters !== 1) out.diags({ id: "letter.letter-count", severity: "error",
        message: `Expected exactly one ‘letter’ environment (found ${letters})` });
    },
  });
  b.peer_backend("html", "The HTML letter.");
  b.peer_element(TAG, "HTML containers for the letter's fields.");
  b.peer_backend("latex-rendered", "The typeset LaTeX letter.");
  b.peer_element(latex.MACRO, "The letter template's LaTeX commands.");
  b.peer_element(latex.ENVIRONMENT, "The letter body's LaTeX environment.");
  b.lowering_rule(new LoweringRule("letter.lower.html", LETTER, new LoweringDecl().produces(TAG),
    { lower: htmlLetter }, DefaultMark.OnBackends(["html"])));
  b.lowering_rule(new LoweringRule("letter.lower.latex", LETTER,
    new LoweringDecl().produces(latex.MACRO).produces(latex.ENVIRONMENT),
    { lower: latexLetter }, DefaultMark.OnBackends(["latex-rendered"])));
  // A useful plain-text rendering for inspection, without template commands.
  b.lowering_rule(new LoweringRule("letter.lower.text", LETTER, new LoweringDecl().produces(BLOCK).produces(PAR), {
    lower(node, cx, lw) {
      const { fields, body } = parts(node);
      const field = name => !fields[name] ? [] : fields[name].element_name() === "flm.quote" ? lowerField(fields[name], cx, lw)
        : [lw.pres(cx, PAR, {}, lowerField(fields[name], cx, lw))];
      return Lowering.Emit([lw.pres(cx, BLOCK, {}, [
        ...["fromaddress", "toaddress", "letterdate", "lettersubject", "salutation"].flatMap(field),
        lw.lower_nodes(cx, body), ...["closing", "signature", "enclosed"].flatMap(field),
      ])]);
    },
  }, DefaultMark.OnBackends(["text"])));
}

export default function (ext) {
  ext.package("_", {
    uses: [".#html", ".#latex"],
    flm: { features: {
      compose: { template: { html: "letter", "latex-rendered": "letter" } },
      quote: { environments: Object.fromEntries(ENVIRONMENTS.map(name => [name, { sections: [], bare: "lines" }])) },
    } },
    features: { letter: { title: "Letter", register } },
  });
  // Module engines are identified by package and template name. Separate
  // packages let both outputs use the public template name `letter`.
  for (const [name, output, template] of [["html", "html", htmlTemplate], ["latex", "latex-rendered", latexTemplate]]) {
    ext.package(name, { host: ["files"], features: {
      [`letter.${name}-template`]: {
        title: `Letter template (${name})`,
        templates: { letter: { output, settings: SETTINGS[output] } },
        register(b) { b.peer_slot(TEMPLATES_SLOT, template); },
      },
    } });
  }
}
