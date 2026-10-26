// tests/js/typst/main.js: a backend of its own, `typst`: the library's lowering
// stage, a scripted emit stage over the lowered trees (`emit.js`), a scripted
// final stage writing the one artifact, the output `typst` (`.typ`), a workflow of
// the same name, and one presentation element of the fixture's own, with a data
// payload, which `\tcallout[kind]{text}` lowers to on this backend alone.
//
// The argument orders of the prelude this module uses:
//   b.presentation_element(name, docs)
//   new ElementDef(owner, names, syntax, element).docs(text)
//   SyntaxDef.Macro([ArgDef.optional(name), ArgDef.mandatory(name)])
//   new LoweringRule(name, element, decl, body, mark).docs(text)
//   lw.pres(cx, element, payload, children); lw.lower_arg_named(cx, node, name)
//   b.table_named(name, StandardTable.X, docs); b.table(name, docs)
//   new StageDecl(name, effect, scope).writes(table).docs(text); new Needs().reads(table)
//   new BackendDef(name, native, lowering, emit, final_stage).docs(text)
//   new LoweringStage(backend); lowered_table_name(backend)
//   new OutputDef(name, capability, table, media_type, extension).docs(text)
//   Artifact.bytes(new ArtifactId(owner, name), media_type, text).as_entry()
import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
import { BackendDef, OutputDef, StandardTable } from "flm:core/registry";
import { Artifact, ArtifactId } from "flm:core/artifact";
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { LoweringStage, lowered_table_name } from "flm:stdcore/backends/lowering";
import { ALL, BLOCK, UNSUPPORTED } from "flm:stdcore/backends/pres";
import { TEXT, GROUP, OPAQUE_WRAPPER, COMMENT, CONTAINER, PARBREAK } from "flm:lang/structural";
import { ArgDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { chars_text } from "flm:lang/text";
import { ElementSpec, Aspect } from "flm:model/element";
import { BlockRole } from "flm:model/aspects";
import { emit } from "./emit.js";

const BACKEND = "typst";
const OUTPUT = "typst.output";                     // the emitted text, one item
const DOCUMENT = "typst.document";                 // the artifact set
const MEDIA_TYPE = "application/x-typst";
const CALLOUT = "typst.pres.callout";              // the presentation element of the fixture's own
const CALLOUT_ELEMENT = "typst.callout";           // \tcallout[kind]{text}

// The elements the emitter writes natively: the whole presentation vocabulary (the
// rest as the `[unsupported: …]` stand-in), the six structural elements the text
// backend writes, and the fixture's own element.
const NATIVE = [...ALL, TEXT, GROUP, OPAQUE_WRAPPER, COMMENT, CONTAINER, PARBREAK, CALLOUT];

// The one element of the standard package whose library rule is registered for the
// library's backends alone, so that a backend of an extension lowers it by a rule
// of its own: here, to the stand-in.
const IMAGE_ELEMENT = "flm.graphics.image";

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    flm: { workflows: { typst: { outputs: [BACKEND], demands: ["flm.validated"], docs: "The document as Typst markup, running every check." } } },
    features: {
      typst: { title: "Typst backend", docs: "Typst markup over the presentation vocabulary.", register },
    },
  });
}

function register(b) {
  // The library's name of the backend's lowered trees, "flm.lowered.typst" (a
  // function of the extension API is called from a handler, never at a module's
  // top level).
  const LOWERED = lowered_table_name(BACKEND);
  // The presentation element with a data payload, `{ kind }`.
  b.presentation_element(CALLOUT, "a callout and its kind, written natively by the typst backend");
  // `\tcallout[kind]{text}`, a block.
  b.element(new ElementDef(
    b.feature(), ["tcallout"], SyntaxDef.Macro([ArgDef.optional("kind"), ArgDef.mandatory("text")]),
    new ElementSpec(CALLOUT_ELEMENT).with(Aspect.BlockRoleAspect(BlockRole.Block)),
  ).docs("A callout of a kind (`note` unless given), around its text."));
  // On this backend, the fixture's own element with its payload; elsewhere, a block.
  b.lowering_rule(new LoweringRule(
    "typst.callout.lower.typst", CALLOUT_ELEMENT, new LoweringDecl().produces(CALLOUT),
    { lower(node, cx, lw) {
        const kind = node.argument_is_provided("kind") ? chars_text(node.argument_content_nodes_named("kind")) ?? "note" : "note";
        return Lowering.Emit([lw.pres(cx, CALLOUT, { kind }, lw.lower_arg_named(cx, node, "text"))]);
    } },
    DefaultMark.OnBackends([BACKEND]),
  ).docs("A callout as the typst backend writes it."));
  b.lowering_rule(new LoweringRule(
    "typst.callout.lower.default", CALLOUT_ELEMENT, new LoweringDecl().produces(BLOCK),
    { lower(node, cx, lw) { return Lowering.Emit([lw.pres(cx, BLOCK, { role: CALLOUT_ELEMENT }, lw.lower_arg_named(cx, node, "text"))]); } },
    DefaultMark.Everywhere,
  ).docs("A callout as a block, on the other backends."));
  // An image, which this backend does not write: the stand-in.
  b.lowering_rule(new LoweringRule(
    "typst.image.lower.typst", IMAGE_ELEMENT, new LoweringDecl().produces(UNSUPPORTED),
    { lower(node, cx, lw) {
        return Lowering.Emit([lw.pres(cx, UNSUPPORTED, { element: IMAGE_ELEMENT, message: "the typst backend writes no image" }, [])]);
    } },
    DefaultMark.OnBackends([BACKEND]),
  ).docs("An image as the stand-in, on the typst backend."));
  // The backend's tables: the lowered trees (the library's name and type), the
  // emitted text, the artifact set.
  b.table_named(LOWERED, StandardTable.LoweredTrees, "the typst backend's lowered trees, one per document tree");
  b.table(OUTPUT, "the document as Typst markup, one item");
  b.table_named(DOCUMENT, StandardTable.ArtifactSet, "the Typst document as a one-artifact set");
  // The emit stage: the lowered trees, in document order, as one text.
  const emitStage = {
    decl: () => new StageDecl("typst.emit", Effect.Compute, Scope.Document).writes(OUTPUT).docs("writes the lowered trees as Typst markup"),
    needs: () => new Needs().reads(LOWERED),
    run(cx, out) { out.table(OUTPUT).push(emit(cx, cx.get(LOWERED).lowered)); },
  };
  // The final stage: the text as the entry artifact `typst:document`.
  const finalStage = {
    decl: () => new StageDecl("typst.final", Effect.Compute, Scope.Document).writes(DOCUMENT).docs("writes the markup as the one-artifact set"),
    needs: () => new Needs().reads(OUTPUT),
    run(cx, out) {
      const [text] = cx.get(OUTPUT)?.ext.items() ?? [];
      if (text === undefined) return;
      out.table(DOCUMENT).push(Artifact.bytes(new ArtifactId(BACKEND, "document"), MEDIA_TYPE, text).as_entry());
    },
  };
  b.backend(new BackendDef(BACKEND, NATIVE, new LoweringStage(BACKEND), emitStage, finalStage).docs("Typst markup"));
  b.output(new OutputDef(BACKEND, DOCUMENT, DOCUMENT, MEDIA_TYPE, "typ").docs("the document as Typst markup"));
}
