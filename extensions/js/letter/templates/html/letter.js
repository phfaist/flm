import { path } from "flm:host";
import { DEFAULTS, options, fill, escapeHtml, logoFile, readCustomization } from "../options.js";

export default {
  name: "letter",
  describe() { return { accepts: "all", config: DEFAULTS.html }; },
  files(data, cx) {
    const c = options(data.config, "html");
    const logo = logoFile(c, "html");
    const custom = readCustomization(cx, c.custom_css, c.custom_css_file, "CSS");
    return [...(logo ? [logo] : []), ...(custom.trim() ? [{
      name: "letter-custom-style", extension: "css", media_type: "text/css", text: custom,
    }] : [])];
  },
  render(data, cx) {
    const c = options(data.config, "html");
    const regions = Object.fromEntries(data.regions);
    const artifacts = Object.fromEntries(data.artifacts);
    // The composer owns the image URL and its accessible description. The
    // lowering-owned visual box is decorative; document HTML passes through
    // untouched, and the replaced image keeps its intrinsic dimensions.
    const cssString = text => `"${text.replace(/["\\\x00-\x1f<]/g, char => `\\${char.codePointAt(0).toString(16)} `)}"`;
    // A CSS point is 1/72in; a TeX point is 1/72.27in. Use physical dimensions
    // consistently for both outputs, including margins and body-relative em.
    const cssValues = Object.fromEntries(Object.entries(c).map(([key, value]) => [key,
      typeof value === "string" && /^(?:\d+(?:\.\d+)?|\.\d+)pt$/.test(value) ? `${parseFloat(value) * 72 / 72.27}pt` : value]));
    cssValues.address_font_pt = c.address_font_pt * 72 / 72.27;
    // Preserve CSS font-list syntax without allowing an HTML closing style tag.
    cssValues.font_family = c.font_family.replace(/</g, "\\3c ");
    const css = fill(cx.host("files").read_text(path(import.meta, "./style.css")), cssValues);
    const logoCss = c.show_logo ? `.flm-letter-logo { content: url(${cssString(artifacts["letter-logo-html"])}) / ""; }` : "";
    const pieces = place => data.pieces.filter(piece => piece.place === place && !piece.name.startsWith("flm.metadata.")).map(piece => piece.text).join("\n");
    return `<!DOCTYPE html>
<html lang="${escapeHtml(regions.lang ?? "en")}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${regions.title || "Letter"}</title>
${regions.styles ?? ""}
<style>${css}\n${logoCss}</style>
${artifacts["letter-custom-style"] ? `<link rel="stylesheet" href="${escapeHtml(artifacts["letter-custom-style"])}">` : ""}
</head>
<body class="flm-letter-${c.style} flm-letter-${c.letterhead}${c.show_logo ? "" : " flm-letter-no-logo"}${c.show_page_breaks ? " flm-letter-page-breaks" : ""}">
<main class="flm-letter-sheet" id="flm-content">
${pieces("before-content")}
${c.show_logo && c.logo_alt ? `<span class="flm-letter-logo-description" role="img" aria-label="${escapeHtml(c.logo_alt)}"></span>` : ""}
${regions.content}
${pieces("after-content")}
</main>
${regions.scripts ?? ""}
</body>
</html>
`;
  },
};
