// The template module `tiny`: an HTML page with a heading from the feature's
// configuration, a stylesheet it computes, a partial it reads from the
// extension's folder, and, when the template's settings name one, a banner
// image shipped from the document's folder.
export default {
  name: "tiny",
  describe(settings) {
    return {
      output: "html",
      accepts: "all",
      requires: [],
      config: { heading: settings.heading ?? "Tiny", color: "teal", banner: null },
    };
  },
  files(data, cx) {
    const files = [{ name: "style", extension: "css", media_type: "text/css", text: `h1 { color: ${data.config.color}; }\n` }];
    if (data.config.banner !== null) files.push({ name: "banner", file: data.config.banner, scope: "document" });
    return files;
  },
  render(data, cx) {
    const regions = Object.fromEntries(data.regions);
    const artifacts = Object.fromEntries(data.artifacts);
    const head = cx.host("files").read_text("extension", "templates/html/head.html") ?? "";
    const banner = artifacts.banner === undefined ? "" : `<img src="${artifacts.banner}" alt="">\n`;
    const after = data.pieces.filter((piece) => piece.place === "after-content").map((piece) => piece.text).join("\n");
    return `<!doctype html>\n<html>\n<head>\n${head}<title>${regions.title ?? ""}</title>\n` +
      `<link rel="stylesheet" href="${artifacts.style}">\n${regions.styles ?? ""}\n</head>\n<body>\n` +
      `<h1>${data.config.heading}</h1>\n${banner}${regions.content}\n${after}\n${regions.scripts ?? ""}\n</body>\n</html>\n`;
  },
};
