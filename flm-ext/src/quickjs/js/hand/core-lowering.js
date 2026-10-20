// The hand-written part of `flm:core/lowering`, spliced into the generated
// stub `api/core/lowering.js` by `tests/api_surface.rs`: the constructor of
// `LoweringRule`, which records the body under the rule's name and lends it
// (the protocol for lowering bodies) before calling `LoweringRule::new`.

// @hand method LoweringRule.constructor
  // `new LoweringRule(name, element, decl, body, mark)`: `body` is a plain
  // object `{ lower(node, cx, lw) }`, recorded under `name` so that the engine
  // instance of the thread that lowers finds it again.
  constructor(name, element, decl, body, mark) {
    record_lower(name, body);
    return call(MODULE, "LoweringRule::new", [name, element, decl, lend_lower(name), mark]);
  }
