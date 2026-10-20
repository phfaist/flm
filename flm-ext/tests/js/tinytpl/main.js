// tests/js/tinytpl/main.js: one package shipping the template module `tiny`,
// contributed with the feature's configuration (`heading`).
import { TEMPLATES_SLOT } from "flm:stdcore/features/compose";
import tiny from "./templates/html/tiny.js";

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    host: ["files"],
    features: {
      tinytpl: {
        title: "Tiny page",
        settings: { config: { docs: "The page's heading.", schema: { type: "object", properties: { heading: { type: "string" } } } } },
        register(b, settings) { b.peer_slot(TEMPLATES_SLOT, tiny); },
      },
    },
  });
}
