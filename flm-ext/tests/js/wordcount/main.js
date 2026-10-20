// tests/js/wordcount/main.js: case 1's main module. Fixed replacements, a
// marker a stage collects, a hook computing its replacement, and a per-tree
// word count; three settings, one per binding: `shout_suffix` (the
// configuration, read by `register`), `min_length` (the option
// `wordcount.counting`, set by a render) and `report_counts` (the option
// `wordcount.reporting`, set by a document).
import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
import { ArgDef } from "flm:lang/mint";
import { chars_text } from "flm:lang/text";
import { Definition, UserDefinition, Expansion } from "flm:lang/rewrites/userdefs";
import { element_name_under, expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";
import { count_words } from "./helpers.js";

// The settings the feature declares.
const SETTINGS = {
  config: {
    docs: "What the macros write.",
    schema: { type: "object", properties: { shout_suffix: { type: "string", description: "Appended to the capitals of \\shout." } } },
  },
  options: {
    "wordcount.counting": {
      bind: "render",
      docs: "How words are counted.",
      schema: { type: "object", properties: { min_length: { type: "integer", description: "The characters a word must have to count." } } },
    },
    "wordcount.reporting": {
      bind: "document",
      docs: "What the count reports.",
      schema: { type: "object", properties: { report_counts: { type: "boolean", description: "Whether each tree's count is reported as a note." } } },
    },
  },
};

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      wordcount: { title: "Word count", docs: "Counts words, collects to-do markers, shouts.", settings: SETTINGS, register },
    },
  });
}

const WORDS = "wordcount.words";
const TODOS = "wordcount.todos";
const COUNTED = "wordcount.counted";

// Declares `definition`: its element under the helper's name (with `hook`
// when one is given) and its lowering rule. Answers the element's name.
function declare(b, owner, definition, hook) {
  const ud = new UserDefinition(definition);
  const element = element_name_under("wordcount", definition);
  b.element(hook ? ud.element_def_with(owner, element, hook) : ud.element_def(owner, element));
  b.lowering_rule(expand_in_tree_rule_for(element, definition));
  return element;
}

function register(b, settings) {
  const suffix = settings.shout_suffix ?? "";
  const owner = b.feature();
  // Fixed replacements.
  declare(b, owner, Definition.macro_("flmname", []).content("FLM").docs("the name of the language"));
  declare(b, owner, Definition.macro_("sayhi", [ArgDef.mandatory("name")]).content("Hello, #1!"));
  // The marker: expanded to nothing; a stage collects it.
  const todo = Definition.macro_("todo", [ArgDef.mandatory("text")]).content("").docs("a to-do note, kept out of the output");
  const TODO_ELEMENT = declare(b, owner, todo);
  // The hook: the argument's characters in capitals, staged as one chars node.
  declare(b, owner, Definition.macro_("shout", [ArgDef.mandatory("text")]).docs("the text in capitals"), {
    expand(node, cx, xp) {
      const text = chars_text(node.argument_content_nodes_named("text"));
      if (text === null)
        return Expansion.Emit(xp.recover(cx, node, { id: "wordcount.shout.not-characters", severity: "error",
          message: "the argument of \\shout must be plain characters" }));
      return Expansion.Emit([cx.stage_chars(text.toUpperCase() + suffix)]);
    },
  });
  // The word count, per tree: the chars nodes of the stage's own tree. Its
  // capability joins the collection `flm.validated`, so that the count runs
  // with every standard workflow.
  b.table(WORDS, "the number of words of each tree");
  b.capability(COUNTED, "the words of each tree counted", null);
  b.collection_member("flm.validated", COUNTED);
  b.stage({
    decl: () => new StageDecl("wordcount.count", Effect.Compute, Scope.PerTree).writes(WORDS).provides(COUNTED).docs("counts the words of the tree"),
    needs: () => new Needs().option("wordcount.counting").option("wordcount.reporting"),
    run(cx, out) {
      const min = cx.option("wordcount.counting").min_length ?? 1;
      let words = 0;
      for (const node of cx.my_tree().descendants()) if (node.is_chars()) words += count_words(node.chars(), min);
      out.table(WORDS).push({ tree: cx.tree_id(), words });
      if (cx.option("wordcount.reporting").report_counts === true)
        out.diags({ id: "wordcount.count", severity: "note",
          message: `the tree \`${cx.tree_id()}\` holds ${words} words of at least ${min} characters` });
    },
  });
  // The markers, for the document, from the trees in which only the
  // definitions of the policy `always` are expanded.
  b.table(TODOS, "every \\todo of the document, with its tree");
  b.stage({
    decl: () => new StageDecl("wordcount.todos", Effect.Compute, Scope.Document).writes(TODOS),
    needs: () => new Needs().reads("flm.userdefs.always-expanded"),
    run(cx, out) {
      const trees = cx.get("flm.userdefs.always-expanded").trees;
      for (const [tree, root] of trees.trees())
        for (const node of root.descendants())
          if (node.is_callable() && node.element_name() === TODO_ELEMENT)
            out.table(TODOS).push({ tree, text: chars_text(node.argument_content_nodes_named("text")) });
    },
  });
}
