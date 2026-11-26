import { path } from "flm:host";
import { DEFAULTS, options, fill, readCustomization, logoFile } from "../options.js";

export default {
  name: "letter",
  describe() { return { accepts: "all", config: DEFAULTS["latex-rendered"] }; },
  files(data) {
    const logo = logoFile(options(data.config, "latex-rendered"), "latex-rendered");
    return logo === null ? [] : [logo];
  },
  render(data, cx) {
    const c = options(data.config, "latex-rendered");
    const regions = Object.fromEntries(data.regions);
    const artifacts = Object.fromEntries(data.artifacts);
    const preamble = fill(cx.host("files").read_text(path(import.meta, "./preamble.tex")), c);
    const custom = readCustomization(cx, c.latex_preamble, c.latex_preamble_file, "LaTeX preamble");
    const pieces = place => data.pieces.filter(piece => piece.place === place && !piece.name.startsWith("flm.metadata.")).map(piece => piece.text).join("\n");
    // FLM supplies the artifact path; detokenize preserves spaces and underscores.
    const logoWidth = c.letterhead === "columns" ? c.logo_width : c.letterhead === "page-width" ? "\\paperwidth" : "\\textwidth";
    const logo = c.show_logo
      ? `\\includegraphics[width=${logoWidth},height=${c.logo_max_height},keepaspectratio]{\\detokenize{${artifacts["letter-logo-latex"]}}}` : "";
    return `\\documentclass[${c.paper_tex}]{article}
\\usepackage{${regions["style-package"]}}
${preamble}
\\newcommand{\\flmxLetterLogo}{${logo}}
${custom}
\\begin{document}
\\fontsize{${c.font_pt}}{${c.baseline_pt}}\\selectfont
${c.letterhead_shift_pt === 0 ? "" : `\\vspace*{${c.letterhead_shift_pt}pt}`}
${pieces("before-content")}
${regions.content}
${pieces("after-content")}
\\end{document}
`;
  },
};
