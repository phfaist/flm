// tests/js/tinytpl/main.js: one package shipping the template module `tiny`,
// declared with its settings, contributed with the feature's configuration
// (`heading`).
import { TEMPLATES_SLOT } from "flm:stdlib/features/compose";
import tiny from "./templates/html/tiny.js";

// The settings of `tiny`; its `describe` gives their defaults.
const TINY_SETTINGS = {
  type: "object",
  properties: {
    heading: { type: "string", description: "The page's heading." },
    color: { type: "string", description: "The color of the heading." },
    banner: {
      type: ["string", "null"],
      description: "An image shown under the heading: its path, relative to the file in which it is written.",
      "x-flm-marks": ["path"],
    },
  },
};

export default function (ext) {
  ext.package("_", {
    host: ["files"],
    features: {
      tinytpl: {
        title: "Tiny page",
        settings: { config: { docs: "The page's heading.", schema: { type: "object", properties: { heading: { type: "string" } } } } },
        templates: { tiny: { output: "html", settings: TINY_SETTINGS } },
        register(b, settings) { b.peer_slot(TEMPLATES_SLOT, tiny); },
      },
    },
  });
}
