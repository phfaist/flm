// tests/js/twopack/shared.js: imported by the main module for both packages;
// it counts its own evaluations in the engine instance.
globalThis.__twopack_loads = (globalThis.__twopack_loads ?? 0) + 1;

// `text` in brackets.
export function mark(text) { return `[${text}]`; }
