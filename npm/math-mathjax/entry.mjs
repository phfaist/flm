// The source of `vendor/mathjax.js`: the part of MathJax 4 the math-mathjax
// extension uses, behind one object. `bundle.mjs` bundles this module with
// esbuild into one ES module.
//
// Reduced from the experiment's recipe (branch `experiment-quickjs`,
// `prototypes/experiment-quickjs/js/common.mjs`): the TeX input with every
// package the experiment bundled except `newcommand` (no definition is made
// inside a formula) and `bussproofs` (it needs an output jax), the MathML
// serialization, the DOM-free lite adaptor and its parser, and a synchronous
// loader that does nothing; no output jax and no font data.
//
// The exported object `MathJax`:
//
// - `configure(packages, own)`: a fresh TeX input configured with `packages`
//   (a list of names of `PACKAGES`) and the TeX package `own` (below),
//   MathJax's own numbering off (`tags: 'none'`) and an error in a formula
//   thrown (`formatError`);
// - `names(tex)`: the command maps the TeX input `tex` holds, kind by kind
//   (`macro`, `environment`, `delimiter`), in MathJax's order of lookup:
//   `[{ kind, map, package, names: [name, …] }, …]`; the `character` kind,
//   whose maps are patterns rather than names, is left out;
// - `convert(tex, string, display)`: the MathML of the formula `string` as a
//   tree `{ tag, attributes: [[name, value], …], children: [tree | text] }`,
//   through a fresh document of `tex`, with TeX's spacing written in (below);
//   a TeX error is thrown as `TexError`;
// - `TexError`: the class of MathJax's TeX errors;
// - `PACKAGES`, `VERSION`.
//
// The TeX package `own` is the caller's, defined through MathJax's own
// mechanism for a TeX package: `{ name, map, delimiters }`, a package named
// `name` whose one map, a delimiter map named `map`, holds `delimiters` (a
// command's name without its backslash -> its character). The map is listed
// for delimiters and for macros, as the `base` package lists its map of
// delimiters (`\langle`), so that each command is read alone, after a size
// command (`\bigl`) and after `\left` or `\right`. The package is defined
// anew at each call, and the TeX input lists it after `packages`.
//
// TeX's spacing. The MathML `convert` answers carries the spacing between the
// parts of the formula that MathJax computes for its own typesetting (TeX's
// rules, by the TeX class of each part and the script level), and leaves the
// reader no spacing of its own to add: `convert` computes the TeX classes of
// the converted tree (`setTeXclass`, as MathJax's output does), and its
// serialization
//
// - writes, before each item of a row (a child of an `mrow`, or of the row a
//   `math`, `mstyle`, `mtd` or other element holds without an `mrow`), the
//   space MathJax puts before it (`texSpacing()`), as an `mspace` of that
//   width in em, unless the item has spacing attributes of its own
//   (`hasSpacingAttributes()`: its core operator sets `lspace` or `rspace`);
// - writes `lspace` and `rspace` on every `mo`: a value the TeX input set on
//   it is kept, and an unset one is written as `0`.
//
// Nothing else in the serialization changes.
import { mathjax } from '@mathjax/src/js/mathjax.js';
import { TeX } from '@mathjax/src/js/input/tex.js';
import { Configuration, ConfigurationHandler } from '@mathjax/src/js/input/tex/Configuration.js';
import { ConfigurationType, HandlerType } from '@mathjax/src/js/input/tex/HandlerTypes.js';
import { DelimiterMap } from '@mathjax/src/js/input/tex/TokenMap.js';
import ParseMethods from '@mathjax/src/js/input/tex/ParseMethods.js';
import TexError from '@mathjax/src/js/input/tex/TexError.js';
import { liteAdaptor } from '@mathjax/src/js/adaptors/liteAdaptor.js';
import { RegisterHTMLHandler } from '@mathjax/src/js/handlers/html.js';
import { STATE } from '@mathjax/src/js/core/MathItem.js';
import { SerializedMmlVisitor } from '@mathjax/src/js/core/MmlTree/SerializedMmlVisitor.js';
import { MATHSPACE, em } from '@mathjax/src/js/util/lengths.js';

import '@mathjax/src/js/input/tex/base/BaseConfiguration.js';
import '@mathjax/src/js/input/tex/ams/AmsConfiguration.js';
import '@mathjax/src/js/input/tex/amscd/AmsCdConfiguration.js';
import '@mathjax/src/js/input/tex/boldsymbol/BoldsymbolConfiguration.js';
import '@mathjax/src/js/input/tex/braket/BraketConfiguration.js';
import '@mathjax/src/js/input/tex/bbox/BboxConfiguration.js';
import '@mathjax/src/js/input/tex/cancel/CancelConfiguration.js';
import '@mathjax/src/js/input/tex/cases/CasesConfiguration.js';
import '@mathjax/src/js/input/tex/centernot/CenternotConfiguration.js';
import '@mathjax/src/js/input/tex/color/ColorConfiguration.js';
import '@mathjax/src/js/input/tex/empheq/EmpheqConfiguration.js';
import '@mathjax/src/js/input/tex/enclose/EncloseConfiguration.js';
import '@mathjax/src/js/input/tex/extpfeil/ExtpfeilConfiguration.js';
import '@mathjax/src/js/input/tex/gensymb/GensymbConfiguration.js';
import '@mathjax/src/js/input/tex/mathtools/MathtoolsConfiguration.js';
import '@mathjax/src/js/input/tex/mhchem/MhchemConfiguration.js';
import '@mathjax/src/js/input/tex/physics/PhysicsConfiguration.js';
import '@mathjax/src/js/input/tex/textcomp/TextcompConfiguration.js';
import '@mathjax/src/js/input/tex/textmacros/TextMacrosConfiguration.js';
import '@mathjax/src/js/input/tex/unicode/UnicodeConfiguration.js';
import '@mathjax/src/js/input/tex/upgreek/UpgreekConfiguration.js';
import '@mathjax/src/js/input/tex/units/UnitsConfiguration.js';
import '@mathjax/src/js/input/tex/verb/VerbConfiguration.js';
import '@mathjax/src/js/input/tex/dsfont/DsfontConfiguration.js';

// The packages a document may use: the experiment's list without
// `newcommand` and `bussproofs`.
const PACKAGES = Object.freeze([
  'base', 'ams', 'amscd', 'boldsymbol', 'braket', 'bbox', 'cancel',
  'cases', 'centernot', 'color', 'empheq', 'enclose', 'extpfeil', 'gensymb',
  'mathtools', 'mhchem', 'physics', 'textcomp', 'textmacros', 'unicode',
  'upgreek', 'units', 'verb', 'dsfont',
]);

// The synchronous loader that does nothing: every package is bundled.
mathjax.asyncLoad = () => {};
mathjax.asyncIsSynchronous = true;
const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);

// MathJax's MathML serialization with TeX's spacing written in (see the
// module's comment). The serialization visits the children of an element in
// `childNodeMml`, but those of an implicit row in `visitInferredMrowNode`.
class TeXSpacingVisitor extends SerializedMmlVisitor {
  // The `mspace` before the item `child` of a row (indented by `space`), or
  // the empty string.
  space_before(child, space) {
    const name = child.hasSpacingAttributes() ? '' : child.texSpacing();
    return name ? `${space}<mspace width="${em(MATHSPACE[name])}"></mspace>` : '';
  }

  // Each child of `row` preceded by its space.
  row_children(row, space, nl) {
    return row.childNodes.map((child) => {
      const before = this.space_before(child, space);
      return (before ? before + nl : '') + this.visitNode(child, space);
    });
  }

  childNodeMml(node, space, nl) {
    if (!node.isKind('mrow')) return super.childNodeMml(node, space, nl);
    return this.row_children(node, space, nl).map((mml) => mml + nl).join('');
  }

  visitInferredMrowNode(node, space) {
    return this.row_children(node, space, '\n').join('\n');
  }

  getAttributeList(node) {
    const list = super.getAttributeList(node);
    if (node.isKind('mo')) {
      for (const name of ['lspace', 'rspace']) {
        if (!node.attributes.isSet(name)) list[name] = '0';
      }
    }
    return list;
  }
}

const visitor = new TeXSpacingVisitor();

// The kinds of command maps whose entries are names.
const KINDS = ['macro', 'environment', 'delimiter'];

// Defines the TeX package `own` (see the module's comment).
function define_own({ name, map, delimiters }) {
  const entries = {};
  for (const [command, character] of Object.entries(delimiters)) entries[`\\${command}`] = character;
  new DelimiterMap(map, ParseMethods.delimiter, entries);
  Configuration.create(name, {
    [ConfigurationType.HANDLER]: { [HandlerType.DELIMITER]: [map], [HandlerType.MACRO]: [map] },
  });
}

function configure(packages, own) {
  for (const name of packages) {
    if (!PACKAGES.includes(name)) throw new Error(`the MathJax package \`${name}\` is not bundled`);
  }
  define_own(own);
  return new TeX({ packages: [...packages, own.name], tags: 'none', formatError: (jax, error) => { throw error; } });
}

// The package whose configuration lists the map `map` for the kind `kind`,
// among `packages` (the first that does), or `null`.
function package_of(packages, kind, map) {
  for (const name of packages) {
    const handler = ConfigurationHandler.get(name)?.handler?.[kind] ?? [];
    if (handler.includes(map)) return name;
  }
  return null;
}

function names(tex) {
  const packages = tex.parseOptions.options.packages ?? [];
  const maps = [];
  for (const kind of KINDS) {
    for (const { item } of tex.parseOptions.handlers.get(kind)?._configuration.items ?? []) {
      const map = item._name;
      const table = item.map;
      const keys = table instanceof Map ? [...table.keys()] : [];
      maps.push({ kind, map, package: package_of(packages, kind, map), names: keys });
    }
  }
  return maps;
}

// The lite adaptor's tree of the serialized MathML as data.
function tree_of(node) {
  if (adaptor.kind(node) === '#text') return adaptor.value(node);
  return {
    tag: adaptor.kind(node),
    attributes: adaptor.allAttributes(node).map(({ name, value }) => [name, value]),
    children: adaptor.childNodes(node).map(tree_of),
  };
}

function convert(tex, string, display) {
  const doc = mathjax.document('', { InputJax: tex });
  const root = doc.convert(string, { display, end: STATE.CONVERT });
  root.setTeXclass(null);
  const mml = visitor.visitTree(root, doc);
  const parsed = adaptor.parse(mml, 'text/html');
  const math = adaptor.childNodes(adaptor.body(parsed)).find((node) => adaptor.kind(node) === 'math');
  return tree_of(math);
}

export const MathJax = Object.freeze({
  configure, names, convert, TexError, PACKAGES, VERSION: mathjax.version,
});
