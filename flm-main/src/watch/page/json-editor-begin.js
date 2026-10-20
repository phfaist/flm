
  // `@json-editor/json-editor` hands its classes over through `module.exports` too, given
  // these two names; it is given fresh ones, `morphdom` having been taken (`regions.js`),
  // and `settings.js` takes it from there.
  module = { exports: {} };
  exports = module.exports;

