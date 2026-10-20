// _flmextensions/runsh/main.js: a document stage that finds `sh` through the
// host service `programs`, runs it in a temporary folder with one file
// written there, and reports what the run answered (its status, its standard
// output and the two files it collected, one text and one binary) as a note;
// and the workflow `ran`, which runs that stage.
import { StageDecl, Effect, Scope } from "flm:core/stage";

const RAN = "runsh.ran";

const SCRIPT = 'tr a-z A-Z < in.txt > out.txt; printf "\\001\\002\\377" > out.bin; echo "$1"';

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    host: ["programs"],
    flm: { workflows: { ran: { extends: "text", demands: ["flm.validated", RAN], docs: "The text output, with the run reported." } } },
    features: {
      runsh: {
        title: "Run sh",
        register(b) {
          b.table(RAN, "the run, one item");
          b.stage({
            decl: () => new StageDecl("runsh.run", Effect.Compute, Scope.Document).writes(RAN),
            run(cx, out) {
              const programs = cx.host("programs");
              const sh = programs.call("find", { names: ["flm-no-such-shell", "sh"] });
              const ran = programs.call("run", {
                program: sh,
                args: ["-c", SCRIPT, "sh", "an argument"],
                files: { "in.txt": { text: "written by the extension" } },
                collect: ["out.txt", "out.bin", "absent.txt"],
              });
              const files = Object.keys(ran.files).sort().map((path) => `${path} ${JSON.stringify(ran.files[path])}`);
              out.table(RAN).push({ status: ran.status });
              out.diags({ id: "runsh.ran", severity: "note",
                message: `sh answered the status ${ran.status}, the output ${JSON.stringify(ran.stdout)} and the files ${files.join(", ")}` });
            },
          });
        },
      },
    },
  });
}
