# Vendored: `@json-editor/json-editor`

The *Settings* tool of the live preview's toolbar makes the form of a feature's settings
from the feature's JSON Schema with `@json-editor/json-editor`, which renders a form from a
schema and gives the value the form holds back as JSON.

- **Package**: `@json-editor/json-editor` on the npm registry, version **2.17.2** (license
  MIT, in `LICENSE` beside this file, copied from the package; the licenses of the parts
  the package's file bundles are named in `jsoneditor.js.LICENSE.txt`, copied from the
  package as well).
- **File**: `jsoneditor.js` is the package's `dist/jsoneditor.js`, unmodified. Given a local
  `module` object, it hands its classes over through it. It bundles a copy of the polyfill
  library `core-js` (3.36.1), a library that adds or replaces built-in functions of the
  browser. When the page script loads, on every preview page whether or not *Settings* is
  opened, that copy makes exactly four changes to the page's built-ins, and the library
  makes no other:
  1. it adds `__core-js_shared__` to `window`, the store every copy of `core-js` in a page
     shares;
  2. it adds `useSetter` to `Symbol`;
  3. it adds `useSimple` to `Symbol`;
  4. it replaces `Function.prototype.toString` by a wrapper that answers the same text as
     the browser's own for every function that is not the library's own.

  The test of the page's global names (`flm-main/tests/watch_page.rs`, the test whose name
  ends `the_page_defines_no_global_name`) holds the first three; a refresh of the library
  is checked against this list.
- **Origin**: `https://registry.npmjs.org/@json-editor/json-editor/-/json-editor-2.17.2.tgz`,
  whose SHA-1 the registry states as `8a2993147f670b125e5594aeb5f644796240a941`.

To refresh it, from this folder (replace the version in the three places):

```sh
curl -sSL https://registry.npmjs.org/@json-editor/json-editor/-/json-editor-2.17.2.tgz | tar xz --strip-components=1 package/dist/jsoneditor.js package/dist/jsoneditor.js.LICENSE.txt package/LICENSE && mv dist/jsoneditor.js dist/jsoneditor.js.LICENSE.txt . && rmdir dist
```
