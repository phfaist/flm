// The thread and strict-mode probe: a per-tree stage whose module counts its
// runs. On one engine instance the count grows from run to run; a fresh
// instance starts again at 1.
import { StageDecl, Effect, Scope } from "flm:core/stage";

let calls = 0;

export default function (ext) {
  ext.package("_", {
    features: {
      counter: {
        title: "Counts the runs of its stage on this engine instance",
        register(b) {
          b.table("counter.calls", "How many times the stage ran on this engine instance, one item per run.");
          // No `needs`: the binding supplies an empty one.
          b.stage({
            decl() { return new StageDecl("counter.count", Effect.Compute, Scope.PerTree).writes("counter.calls"); },
            run(cx, out) { out.table("counter.calls").push({ calls: ++calls }); },
          });
        },
      },
    },
  });
}
