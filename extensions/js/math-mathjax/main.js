// The math-mathjax extension: mathematics processed by MathJax. One package,
// `_`, with one feature, `math-mathjax`, which declares the commands and
// environments of MathJax's TeX packages in math mode (read from MathJax when
// the feature registers) and the two delimiters `\llangle` and `\rrangle`
// (`definitions.js`), checks the formulas, and lowers them on the HTML,
// text, recomposed-LaTeX and latex-rendered backends. It requires the `math`
// feature.
//
// The feature's settings (its configuration):
//
// - `mode`: `load-in-browser` (the default) or `mathml`;
// - `packages`: the MathJax TeX packages a document may use (default: every
//   package of the bundle, `vendor/bundle.js`);
// - `math_font_url`: in `mathml` mode, the address of a math font;
// - `loader_url`: in `load-in-browser` mode, the address the page loads
//   MathJax from;
// - `lazy`: in `load-in-browser` mode, whether MathJax typesets each formula
//   only when it scrolls into view (default `false`);
// - `preamble`: TeX that MathJax processes once, before any formula, in both
//   modes (default empty);
// - `export_commands`: the commands the preamble defines that a formula may
//   use, each name without its backslash (default none).
import { MathJax } from "./vendor/mathjax.js";
import { PACKAGES as BUNDLED } from "./vendor/bundle.js";
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { CONTENT, ENVIRONMENT, equation_lines } from "flm:stdcore/features/math";
import { read_commands, declare_commands, OWN } from "./definitions.js";
import { register_rules } from "./lower.js";
import { register_page, DEFAULT_LOADER_URL } from "./page.js";
import { register_render } from "./render.js";

// The modes.
const MODES = ["mathml", "load-in-browser"];

// The check stage, the capability it provides beside `flm.math.content`, and
// its diagnostic.
const CHECK = "math-mathjax.check";
const EMPTY_LINE = "math-mathjax.empty-line";

// The feature's configuration as read, with its defaults. A package name the
// bundle does not hold is the registration's failure.
function read_settings(settings) {
  const packages = settings.packages ?? BUNDLED;
  for (const name of packages) {
    if (!BUNDLED.includes(name)) {
      throw new Error(`the MathJax package \`${name}\` is not in the extension's bundle; the bundled packages are `
        + BUNDLED.map((bundled) => `\`${bundled}\``).join(", "));
    }
  }
  return {
    mode: settings.mode ?? "load-in-browser",
    packages,
    math_font_url: settings.math_font_url ?? null,
    loader_url: settings.loader_url ?? DEFAULT_LOADER_URL,
    lazy: settings.lazy ?? false,
    preamble: settings.preamble ?? "",
    export_commands: settings.export_commands ?? [],
  };
}

// Processes the preamble `preamble` (a setting) on the TeX input `tex`, once,
// before any formula: what it defines or sets stays on the TeX input for every
// formula, and what MathJax outputs for it is discarded. The extension hands
// it to MathJax as written. A preamble MathJax rejects is the registration's
// failure.
function run_preamble(tex, preamble) {
  if (preamble === "") return;
  try {
    MathJax.convert(tex, preamble, false);
  } catch (error) {
    if (!(error instanceof MathJax.TexError)) throw error;
    throw new Error("the `preamble` of the feature `math-mathjax` must be TeX that MathJax accepts; "
      + `MathJax reports: ${error.message}`);
  }
}

// Whether a node of an equation line's content is mathematics: anything but
// blank characters and comments.
function is_content(node) {
  if (node.is_comment()) return false;
  return !(node.is_chars() && node.chars().trim() === "");
}

// The check: every equation line of every display-math environment of the
// tree holds mathematics; everything else was decided by the parse. It
// provides `flm.math.content` and the feature's own capability `CHECK`, which
// the feature adds to the collection `flm.validated`.
const check = {
  decl() {
    return new StageDecl(CHECK, Effect.Check, Scope.PerTree).provides(CONTENT).provides(CHECK).docs(
      "The check of the formulas processed by MathJax: it reports every equation line that holds no "
      + "mathematics, and provides `flm.math.content` and `math-mathjax.check`, a member of the "
      + "collection `flm.validated`.");
  },
  run(cx, out) {
    const tree = cx.my_tree();
    if (tree === null) return;
    for (const node of tree.descendants()) {
      if (node.element_name() !== ENVIRONMENT) continue;
      equation_lines(node).forEach((line, index) => {
        if (line.content.some(is_content)) return;
        out.diags({
          id: EMPTY_LINE,
          severity: "error",
          message: `line ${index} (counting from 0) of the \`${node.name()}\` environment must hold mathematics; `
            + "a `\\label`, `\\tag` or `\\nonumber` alone is not mathematics",
          at: line.content[0] ?? node,
        });
      });
    }
  },
};

export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      "math-mathjax": {
        title: "Mathematics by MathJax",
        docs: "The commands and environments of MathJax's TeX packages inside mathematics, and the "
          + "formulas typeset by MathJax: as MathML written into the page (`mathml`), or as TeX that "
          + "MathJax typesets in the reader's browser (`load-in-browser`). Requires the `math` feature.",
        settings: {
          config: {
            docs: "How the formulas are processed, which MathJax packages a document may use, and the TeX "
              + "MathJax processes once before any formula, with the commands it defines that a formula may use.",
            schema: {
              type: "object",
              properties: {
                mode: { enum: MODES },
                packages: { type: "array", items: { type: "string" } },
                math_font_url: { type: "string" },
                loader_url: { type: "string" },
                lazy: { type: "boolean" },
                preamble: { type: "string" },
                export_commands: { type: "array", items: { type: "string" } },
              },
            },
          },
        },
        register(b, settings) {
          const config = read_settings(settings);
          // The settings change what processing produces (the preamble's
          // definitions, the mode, the font, the loader) in ways the build
          // cannot see from the declarations, so cached results are not
          // reused after they change.
          b.digest_input(JSON.stringify(config));
          // One TeX input, made here, converts every formula, rather than one
          // TeX input per formula: MathJax's `mathtools` package records every
          // TeX input it configures in a module-level map (its `MathtoolsTags`
          // classes), so a TeX input per formula leaks about 100 KiB each and a
          // long document meets the engine's memory limit. No command a
          // formula may hold defines anything or changes a setting for later
          // formulas (`definitions.js`): what the TeX input holds beyond its
          // packages is what the preamble defines and sets, processed here in
          // both modes, before the commands are read, so that the commands it
          // defines are among them.
          const tex = MathJax.configure(config.packages, OWN);
          run_preamble(tex, config.preamble);
          const commands = read_commands(MathJax, tex, config.export_commands);
          declare_commands(b, commands);
          // Every workflow that validates a document runs the check, since this
          // feature processes its mathematics. An extension adds to a
          // collection only a capability named under its own stem, so the
          // check's own capability joins `flm.validated`, not `flm.math.content`.
          b.capability(CHECK, "The mathematics of the document has been checked by the feature "
            + "`math-mathjax`.", null);
          b.collection_member("flm.validated", CHECK);
          b.stage(check);
          register_rules(b, config, commands);
          if (config.mode === "load-in-browser") register_page(b, config, commands, BUNDLED);
          else register_render(b, MathJax, tex);
        },
      },
    },
  });
}
