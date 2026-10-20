// The page of the `load-in-browser` mode: the script definition
// `math-mathjax.script` that loads MathJax into the reader's browser, and the
// document-scope stage `math-mathjax.packages` that writes its configuration.
//
// The script definition has two parts, in the order the HTML backend's own
// MathJax script has them: the configuration (a `table` part, the text the
// stage writes into `CONFIGURATION`), then the loader (a `url` part, the
// setting `loader_url`). The stage runs only where the definition is resolved
// on a page that requires it: the formula rules require the table's
// capability and require the definition at every formula.
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { ScriptDef, DefContent } from "flm:core/registry";
import { ENVIRONMENT as MATH_ENVIRONMENT } from "flm:stdcore/features/math";
import { MACRO, ENVIRONMENT, OWN } from "./definitions.js";

// The script definition the formulas require.
export const SCRIPT = "math-mathjax.script";
// The table holding the configuration's text.
export const CONFIGURATION = "math-mathjax.configuration";
// The stage writing the configuration.
export const PACKAGES_STAGE = "math-mathjax.packages";
// The class of a formula's element in the page, which MathJax typesets.
export const FORMULA_CLASS = "math-mathjax-formula";
// The address MathJax is loaded from by default: the HTML backend's
// `MATHJAX_LOADER` (`flm_core::stdcore::backends::html::mathjax`), the same
// pinned major version.
export const DEFAULT_LOADER_URL = "https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js";

// The click listener and the in-place update code, copied from the HTML
// backend's MathJax configuration
// (`flm-core/src/stdcore/backends/html/mathjax-config.js`, the two parts after
// the configuration object), unchanged.
const LISTENERS = `/* A reference written in text inside a formula is a link that the HTML
   backend writes into the HTML that the formula carries.  The renderer
   removes the address from every link in its output, keeping it in
   \`data-mjx-href\`, and follows only the links that it made itself, so a
   click on such a link would otherwise do nothing.  This listener follows
   the link, and acts only on a plain primary click (no modifier key held) on
   one of the backend's links inside a typeset formula. */
document.addEventListener('click', function (event) {
  if (event.defaultPrevented || event.button !== 0 ||
      event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) { return; }
  var from = event.target;
  var link = from && from.closest &&
    from.closest('mjx-container mjx-html a.flm-pres-link[data-mjx-href]');
  if (!link) { return; }
  event.preventDefault();
  window.location.href = link.getAttribute('data-mjx-href');
});
/* A page updated in place.  A script that updates an output region of the
   page (a named part of the page, such as its content) without loading the
   page again follows two conventions: it dispatches the event
   \`flm:region-updated\` on the region's element once the region's content is
   new, and it leaves unchanged every element that carries the attribute
   \`data-flm-original-html\` when the new HTML inside that element equals the
   attribute's value.  This code writes that attribute on each formula,
   holding the HTML inside the formula's element, just before the renderer
   typesets the formula; and on the event, it typesets the formulas of the
   region that carry no such attribute: the new formulas, and those whose
   source changed.  Before the renderer has started, there is nothing to do,
   because the renderer typesets every formula of the page when it starts. */
(function () {
  var ORIGINAL = 'data-flm-original-html';
  var formulas = window.MathJax.startup.elements.join(', ');
  function untypeset(root) {
    return Array.prototype.filter.call(root.querySelectorAll(formulas), function (element) {
      return !element.hasAttribute(ORIGINAL);
    });
  }
  function record(elements) {
    elements.forEach(function (element) {
      element.setAttribute(ORIGINAL, element.innerHTML);
    });
    return elements;
  }
  window.MathJax.startup.pageReady = function () {
    record(untypeset(document));
    return window.MathJax.startup.defaultPageReady();
  };
  document.addEventListener('flm:region-updated', function (event) {
    var renderer = window.MathJax;
    if (!renderer.startup || !renderer.startup.promise) { return; }
    renderer.startup.promise = renderer.startup.promise.then(function () {
      var fresh = record(untypeset(event.target));
      if (fresh.length > 0) { return renderer.typesetPromise(fresh); }
    });
  });
})();
`;

// The text of the configuration for a document using the MathJax packages
// `used` (in the bundle's order; `base`, which every other package builds on,
// is always listed); `lazy` adds the component `ui/lazy`, which
// typesets each formula when it scrolls into view. `base` is part of
// MathJax's TeX input and has no component of its own to load.
//
// `own` says whether the page lists the extension's own TeX package (`OWN`,
// `definitions.js`). The package is then listed last and defined before
// MathJax creates its TeX input: the function `startup.ready` defines it
// through MathJax's own classes (`MathJax._.input.tex`), as the bundle does
// (`MathJax.configure`), then lets MathJax go on (`defaultReady`). The package
// has no component to load.
//
// `preamble` is the feature's setting: when it is not empty, the page's
// MathJax processes it once, after MathJax has created its TeX input and
// before it typesets the page (the function `startup.pageReady`, wrapped
// around the one the listeners below set), and discards the MathML. The
// preamble is written as a JavaScript string, with `<` escaped, so that
// nothing in it closes the script element.
export function configuration_text(used, lazy, own, preamble) {
  const load = [...used, "texhtml"].filter((name) => name !== "base").map((name) => `[tex]/${name}`);
  if (lazy) load.push("ui/lazy");
  const packages = own ? [...used, "texhtml", OWN.name] : [...used, "texhtml"];
  const quoted = (list) => list.map((item) => `'${item}'`).join(", ");
  const startup = own ? `{
    elements: ['.${FORMULA_CLASS}'],
    ready: function () {
      var tex = window.MathJax._.input.tex;
      new tex.TokenMap.DelimiterMap('${OWN.map}', tex.ParseMethods.default.delimiter, ${own_entries()});
      tex.Configuration.Configuration.create('${OWN.name}',
        {handler: {delimiter: ['${OWN.map}'], macro: ['${OWN.map}']}});
      window.MathJax.startup.defaultReady();
    }
  }` : `{elements: ['.${FORMULA_CLASS}']}`;
  return `/* math-mathjax.script */
/* The configuration of MathJax, which typesets in the reader's browser the
   formulas the math-mathjax extension writes into the page: the TeX packages
   the document uses, HTML inside a formula, MathJax's own numbering off (the
   numbers and tags are the document's), and the formulas the extension
   marked${own ? `; when MathJax is ready, the extension's own TeX package
   is defined before MathJax creates its TeX input: the delimiters \\llangle
   and \\rrangle` : ""}.  After the configuration object come a listener that follows a
   link inside a formula when it is clicked, and the code that keeps the
   formulas typeset while a script of the page updates parts of the page in
   place, both copied from the HTML backend's configuration${preamble === "" ? "" : `; then the
   preamble of the feature math-mathjax, which MathJax processes once, before
   it typesets the page, and for which every TeX package the feature's
   settings allow is listed`}. */
window.MathJax = {
  loader: {load: [${quoted(load)}]},
  tex: {
    packages: [${quoted(packages)}],
    allowTexHTML: true,
    inlineMath: [['\\\\(', '\\\\)']],
    displayMath: [['\\\\[', '\\\\]']],
    processEnvironments: true,
    processRefs: false,
    tags: 'none'
  },
  chtml: {mtextInheritFont: true},
  startup: ${startup}
};
${LISTENERS}${preamble === "" ? "" : preamble_text(preamble)}`;
}

// The part of the configuration that processes the preamble `preamble`.
function preamble_text(preamble) {
  const literal = JSON.stringify(preamble).replace(/</g, "\\u003c");
  return `(function () {
  var pageReady = window.MathJax.startup.pageReady;
  window.MathJax.startup.pageReady = function () {
    window.MathJax.tex2mml(${literal}, {display: false});
    return pageReady();
  };
})();
`;
}

// The entries of the extension's own delimiter map, as the text of a
// JavaScript object: each command with its backslash, each character written
// as a `\u` escape.
function own_entries() {
  const escaped = (text) => text.replace(/[^\x20-\x7e]/g,
    (character) => `\\u${character.charCodeAt(0).toString(16).toUpperCase().padStart(4, "0")}`);
  const entries = Object.entries(OWN.delimiters)
    .map(([command, character]) => `'\\\\${command}': '${escaped(character)}'`);
  return `{${entries.join(", ")}}`;
}

// Registers, on the builder `b`, the script definition, its table and the
// stage writing it. `commands` are the commands as `read_commands` answers
// them: its `defined` maps give the MathJax package of a display-math
// environment and of `\tag` and `\notag`, which the extension does not
// declare. `bundled` is the bundle's list of packages, `settings` the
// feature's configuration as read.
//
// A page whose MathJax processes a preamble lists every package of the setting
// `packages`, and the extension's own, whatever the document uses: the TeX
// input that accepted the preamble when the feature registered had them all,
// and the extension does not read the preamble to know which it uses.
export function register_page(b, settings, commands, bundled) {
  b.definition_text_table(CONFIGURATION, "The text of the MathJax configuration of the page.");
  b.script_def(new ScriptDef(SCRIPT, [DefContent.Table(CONFIGURATION), DefContent.Url(settings.loader_url)],
    "MathJax, loaded into the page to typeset the formulas: its configuration, then its program."));
  b.stage({
    decl() {
      return new StageDecl(PACKAGES_STAGE, Effect.Compute, Scope.Document).writes(CONFIGURATION).docs(
        "Writes the MathJax configuration of the page, listing the TeX packages whose commands and "
        + "environments the document uses.");
    },
    run(cx, out) {
      const used = new Set(["base"]);
      const add = (pkg) => { if (pkg !== undefined && pkg !== null) used.add(pkg); };
      for (const { node } of cx.walk()) {
        const element = node.element_name();
        if (element === MACRO || element === ENVIRONMENT) {
          add(node.element_value()?.package);
        } else if (element === MATH_ENVIRONMENT) {
          // The environment and the line commands its TeX is written with.
          add(commands.defined.environments.get(node.name()));
          add(commands.defined.macros.get("tag"));
          add(commands.defined.macros.get("notag"));
        }
      }
      if (settings.preamble !== "") {
        for (const pkg of settings.packages) used.add(pkg);
        used.add(OWN.name);
      }
      const ordered = bundled.filter((name) => used.has(name));
      out.table(CONFIGURATION).push(configuration_text(ordered, settings.lazy, used.has(OWN.name), settings.preamble));
    },
  });
}
