/* flm.preview.script */
// The live preview's page script: it keeps the open page in step with the document while
// `flm watch` runs, and lays a toolbar over the page. It is made of several files, joined
// in this order into one script that runs inside one function, so that it defines no
// global name of its own (the copy of `core-js` that the form library
// `@json-editor/json-editor` bundles adds `__core-js_shared__` to the window and makes three
// other changes to the page's built-ins, listed in `vendor/json-editor/README.md`):
//
// - this beginning;
// - `vendor/morphdom/morphdom-umd.js`, the library `morphdom`, unmodified;
// - `regions.js`, the update of the page: a changed output region updated in place;
// - `problems.js`, the last build's problems: the marks on the elements they lie in, and
//   their list, which needs no server;
// - `toolbar.js`, the toolbar: the state of the build, the tools and the menu of editors;
// - `json-editor-begin.js`, then `vendor/json-editor/jsoneditor.js`, the library
//   `@json-editor/json-editor`, unmodified, which makes a form from a JSON Schema;
// - `settings.js`, the toolbar's tool *Settings*, which shows the document's features in
//   such forms and saves them;
// - `gestures.js`, the clicks with modifier keys that open an element's place in the
//   editor or copy a reference to it, and *Open in editor* on a problem;
// - `connection.js`, the connection to the preview's server, which drives the others;
// - `end.js`, the end of the function.
(function () {
  'use strict';

  // `morphdom` is written for several ways of loading a script. Given these two names it
  // hands its function over through `module.exports` instead of defining a global name;
  // `regions.js` takes it from there.
  var module = { exports: {} };
  var exports = module.exports;

