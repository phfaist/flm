// tests/js/twopack/main.js: case 3, an extension of two packages. The
// package `_` defines `\tpmark` and a table of how many times `./shared.js`
// was evaluated in the engine instance; the package `proofs` uses it
// (`.#_`) and defines `\tpqed` and a table of its own.
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { Definition, UserDefinition } from "flm:lang/rewrites/userdefs";
import { element_name_under, expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";
import { mark } from "./shared.js";

// Declares `definition`, its element under the stem, and its lowering rule.
function declare(b, definition) {
  const element = element_name_under("twopack", definition);
  b.element(new UserDefinition(definition).element_def(b.feature(), element));
  b.lowering_rule(expand_in_tree_rule_for(element, definition));
}

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      twopack: {
        title: "Marks",
        register(b) {
          declare(b, Definition.macro_("tpmark", []).content(mark("mark")));
          b.table("twopack.loads", "how many times the shared module was evaluated in the engine instance");
          b.stage({
            decl: () => new StageDecl("twopack.loads", Effect.Compute, Scope.Document).writes("twopack.loads"),
            run(cx, out) { out.table("twopack.loads").push({ loads: globalThis.__twopack_loads }); },
          });
        },
      },
    },
  });
  ext.package("proofs", {
    uses: [".#_"],
    features: {
      proofs: {
        title: "Proofs",
        register(b) {
          declare(b, Definition.macro_("tpqed", []).content(mark("qed")));
          b.table("twopack.proofs", "the proofs");
        },
      },
    },
  });
}
