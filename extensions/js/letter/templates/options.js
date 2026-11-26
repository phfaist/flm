// These options deliberately cover common letter layouts. CSS and the LaTeX
// preamble are the extension points for institution-specific designs.
const COMMON_DEFAULTS = {
  style: "europe", paper: "a4", logo: null, letterhead: "columns",
  logo_width: "45mm", logo_max_height: "45mm",
  font_family: null, font_size: "11pt", line_spacing: "1.25",
  margin_top: "23mm", margin_bottom: "25mm", margin_left: "25mm", margin_right: "25mm", letterhead_margin_top: null,
  paragraph_spacing: "0.65em", paragraph_indent: null, header_gap: "8mm", signature_gap: "18mm",
};
// The defaults of each output's template: the common options and that output's own.
export const DEFAULTS = {
  html: { ...COMMON_DEFAULTS, logo_alt: "", show_page_breaks: false, custom_css: "", custom_css_file: null },
  "latex-rendered": { ...COMMON_DEFAULTS, latex_preamble: "\\raggedright", latex_preamble_file: null },
};

// The settings of each letter template, as the feature of each declares
// them; their defaults are `DEFAULTS`.
const text = (description) => ({ type: "string", description });
const length = (description) => ({ type: "string", description: `${description} (a length in mm, cm, pt or em).` });
// A file setting is marked as a path: a bare path reaches the module with the root of the
// place where it is written (`doc:` in the document, `ext+NAME:` in a package).
const file = (description) => ({ type: ["string", "null"], description, "x-flm-marks": ["path"] });
const COMMON_SETTINGS = {
  style: { type: "string", enum: ["europe", "us"], description: "The placement of the fields and the default indentation: ‘europe’ or ‘us’." },
  paper: { type: "string", enum: ["a4", "letter"], description: "The paper size: ‘a4’ or ‘letter’." },
  logo: file("The logo: null for none, ‘placeholder’ for the supplied artwork, or the path of an image, relative to the file in which it is written."),
  letterhead: { type: "string", enum: ["columns", "text-width", "page-width"], description: "The area of the logo: ‘columns’, ‘text-width’ or ‘page-width’." },
  logo_width: length("The largest width of a logo beside the sender"),
  logo_max_height: length("The largest height of the logo"),
  font_family: { type: ["string", "null"], description: "The text font: null for the default ones, a CSS font list in HTML, a font name in LaTeX." },
  font_size: { type: "string", description: "The body font size, from 6pt to 24pt (a length in pt, mm or cm)." },
  line_spacing: { type: ["string", "number"], description: "The distance of the body's baselines divided by the font size, from 1 to 3." },
  margin_top: length("The top margin of the paper"),
  margin_bottom: length("The bottom margin of the paper"),
  margin_left: length("The left margin of the paper"),
  margin_right: length("The right margin of the paper"),
  letterhead_margin_top: { type: ["string", "null"], description: "The top margin of the first page, above the letterhead: null for ‘margin_top’, or a length in mm, cm, pt or em." },
  paragraph_spacing: length("The space between the body's paragraphs"),
  paragraph_indent: { type: ["string", "null"], description: "The first-line indent of the body's paragraphs: null for 0pt in Europe and 2em in the US, or a length in mm, cm, pt or em." },
  header_gap: length("The space between the sections of the letterhead and the addresses"),
  signature_gap: length("The handwriting space between the closing and the signature"),
};
export const SETTINGS = {
  html: { type: "object", properties: {
    ...COMMON_SETTINGS,
    logo_alt: text("The alternative text of the logo."),
    show_page_breaks: { type: "boolean", description: "Whether the screen shows a thin line where each printed page approximately ends." },
    custom_css: text("CSS added after the default styles and ‘custom_css_file’."),
    custom_css_file: file("The path of a CSS file, relative to the file in which it is written, added after the default styles."),
  } },
  "latex-rendered": { type: "object", properties: {
    ...COMMON_SETTINGS,
    latex_preamble: text("LaTeX added to the preamble after the default one and ‘latex_preamble_file’."),
    latex_preamble_file: file("The path of a LaTeX file, relative to the file in which it is written, added to the preamble after the default one."),
  } },
};

const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const units = { pt: 1, mm: 72.27 / 25.4, cm: 72.27 / 2.54 };
const lengths = ["logo_width", "logo_max_height", "margin_top", "margin_bottom", "margin_left", "margin_right",
  "letterhead_margin_top", "paragraph_spacing", "paragraph_indent", "header_gap", "signature_gap"];

export function options(given, output) {
  const defaults = DEFAULTS[output];
  for (const key of Object.keys(given)) {
    if (!own(defaults, key)) throw new Error(`Unknown letter template option ‘${key}’`);
  }
  const c = { ...defaults, ...given };
  const choice = (key, values) => {
    if (!values.includes(c[key])) throw new Error(`Invalid letter option ‘${key}’: expected ${values.map(v => `‘${v}’`).join(" or ")}`);
  };
  choice("style", ["europe", "us"]);
  choice("paper", ["a4", "letter"]);
  choice("letterhead", ["columns", "text-width", "page-width"]);
  for (const key of ["logo_alt", "custom_css", "latex_preamble"].filter(key => own(defaults, key))) {
    if (typeof c[key] !== "string") throw new Error(`Invalid letter option ‘${key}’: expected text`);
  }
  if (own(defaults, "show_page_breaks") && typeof c.show_page_breaks !== "boolean") {
    throw new Error("Invalid letter option ‘show_page_breaks’: expected true or false");
  }
  for (const key of ["logo", "custom_css_file", "latex_preamble_file"].filter(key => own(defaults, key))) {
    if (c[key] !== null && (typeof c[key] !== "string" || !c[key].trim() || /[\x00-\x1f\x7f]/.test(c[key]))) {
      throw new Error(`Invalid letter option ‘${key}’: expected a file path or null`);
    }
  }
  if (c.font_family === null) {
    c.font_family = output === "html" ? '"Times New Roman", "TeX Gyre Termes", Times, serif' : "TeX Gyre Termes";
  }
  if (typeof c.font_family !== "string" || !c.font_family.trim() || /[\x00-\x1f\x7f]/.test(c.font_family)) {
    throw new Error("Invalid letter option ‘font_family’: expected nonempty text on one line or null");
  }
  c.font_family = c.font_family.trim();
  if (output === "latex-rendered" && /[\\{}%#$&^_~]/.test(c.font_family)) {
    throw new Error("Invalid letter option ‘font_family’: expected a font name without TeX special characters");
  }
  const font = typeof c.font_size === "string" && /^(\d+(?:\.\d+)?|\.\d+)(pt|mm|cm)$/.exec(c.font_size);
  c.font_pt = font ? Number(font[1]) * units[font[2]] : NaN;
  if (!Number.isFinite(c.font_pt) || c.font_pt < 6 || c.font_pt > 24) {
    throw new Error("Invalid letter option ‘font_size’: expected a length from 6pt to 24pt in pt, mm or cm");
  }
  if (!["string", "number"].includes(typeof c.line_spacing) || !/^(?:\d+(?:\.\d+)?|\.\d+)$/.test(String(c.line_spacing))
      || !Number.isFinite(Number(c.line_spacing)) || Number(c.line_spacing) < 1 || Number(c.line_spacing) > 3) {
    throw new Error("Invalid letter option ‘line_spacing’: expected a multiplier from 1 to 3 (quote decimals)");
  }
  if (c.paragraph_indent === null) c.paragraph_indent = c.style === "us" ? "2em" : "0pt";
  if (c.letterhead_margin_top === null) c.letterhead_margin_top = c.margin_top;
  const points = {};
  for (const key of lengths) {
    const match = typeof c[key] === "string" && /^(\d+(?:\.\d+)?|\.\d+)(pt|mm|cm|em)$/.exec(c[key]);
    points[key] = match ? Number(match[1]) * (match[2] === "em" ? c.font_pt : units[match[2]]) : NaN;
    if (!Number.isFinite(points[key]) || points[key] < 0 || points[key] > 16000) {
      throw new Error(`Invalid letter option ‘${key}’: expected a nonnegative length in pt, mm, cm or em (at most 16000pt)`);
    }
    // Normalize units once: em is relative to the body font, even in @page,
    // geometry, or a smaller sender address. CSS pt is 1/72in, TeX pt 1/72.27in.
    c[key] = `${points[key]}pt`;
  }
  const paperWidth = c.paper === "a4" ? 210 * units.mm : 8.5 * 72.27;
  const paperHeight = c.paper === "a4" ? 297 * units.mm : 11 * 72.27;
  const width = paperWidth - points.margin_left - points.margin_right;
  const height = paperHeight - points.margin_top - points.margin_bottom;
  c.page_text_height = `${height}pt`;
  // The first page has its own top margin, above the letterhead.
  const firstHeight = paperHeight - points.letterhead_margin_top - points.margin_bottom;
  // Unit conversions can round an exact boundary by a fraction of a point.
  const epsilon = 1e-7;
  if (width + epsilon < 40 * units.mm || Math.min(height, firstHeight) + epsilon < 40 * units.mm) {
    throw new Error("Invalid letter margins: at least 40mm of text width and height must remain");
  }
  if (points.paragraph_indent > width - 10 * units.mm + epsilon) {
    throw new Error("Invalid letter option ‘paragraph_indent’: at least 10mm of paragraph width must remain");
  }
  for (const key of ["paragraph_spacing", "header_gap", "signature_gap"]) {
    if (points[key] >= height) throw new Error(`Invalid letter option ‘${key}’: the gap must be smaller than the text height`);
  }
  c.show_logo = c.logo !== null;
  if (c.show_logo) {
    if (points.logo_max_height <= 0 || points.logo_max_height + points.header_gap >= firstHeight) {
      throw new Error("Invalid letter option ‘logo_max_height’: the logo and header gap must fit the text height");
    }
    const maxWidth = c.style === "us" ? (width - 8 * units.mm) / 2 : width - 8 * units.mm - 20 * units.mm;
    if (c.letterhead === "columns" && (points.logo_width <= 0 || points.logo_width > maxWidth + epsilon)) {
      throw new Error("Invalid letter option ‘logo_width’: the logo and sender columns must fit the text width");
    }
    logoFile(c, output); // Refuse unsupported image formats before composing.
  }
  c.baseline_pt = c.font_pt * Number(c.line_spacing);
  c.address_font_pt = c.style === "europe" ? Math.min(9, c.font_pt) : c.font_pt;
  c.address_baseline_pt = c.address_font_pt * 1.25;
  c.font_size = `${c.font_pt}pt`;
  c.paper_css = c.paper === "a4" ? "A4" : "letter";
  c.paper_tex = c.paper === "a4" ? "a4paper" : "letterpaper";
  c.us_tex = c.style === "us" ? "true" : "false";
  c.logo_mode_tex = c.letterhead;
  c.paper_width = c.paper === "a4" ? "210mm" : "8.5in";
  c.paper_height = c.paper === "a4" ? "297mm" : "11in";
  c.column_gap = "8mm";
  // The space added (or removed) at the top of the first page, in TeX points. Rounding
  // removes the residue of a unit conversion and keeps the number in decimal notation.
  c.letterhead_shift_pt = Math.round((points.letterhead_margin_top - points.margin_top) * 1e5) / 1e5;
  return c;
}

export function logoFile(c, output) {
  if (c.logo === null) return null;
  // `placeholder` reaches the module with the root of the place where it is written.
  const placeholder = c.logo.slice(c.logo.indexOf(":") + 1) === "placeholder";
  const file = placeholder ? `assets/placeholder.${output === "html" ? "svg" : "pdf"}` : c.logo;
  const extension = file.split(".").pop().toLowerCase();
  const formats = output === "html" ? ["svg", "png", "jpg", "jpeg", "gif", "webp"] : ["pdf", "png", "jpg", "jpeg"];
  if (!formats.includes(extension)) {
    throw new Error(`Unsupported letter logo ‘${file}’ for ‘${output}’: expected ${formats.join(", ")}`);
  }
  const types = { svg: "image/svg+xml", pdf: "application/pdf", png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp" };
  return { name: `letter-logo-${output === "html" ? "html" : "latex"}`,
    file: placeholder ? `self:${file}` : file, media_type: types[extension] };
}

export function readCustomization(cx, inline, filename, label) {
  let text = "";
  if (filename !== null) {
    text = cx.host("files").read_text(filename);
    if (text === null) throw new Error(`Missing letter ${label} file ‘${filename}’`);
  }
  return `${text}\n${inline}`;
}

export function fill(text, values) {
  return text.replace(/@@([a-z_]+)@@/g, (_, key) => {
    if (!own(values, key)) throw new Error(`Missing letter template value ‘${key}’`);
    return String(values[key]);
  });
}

export function escapeHtml(text) {
  return String(text).replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
}
