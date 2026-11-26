// The hand-written part of `flm:host`, spliced into the generated stub
// `api/host.js` by `tests/api_surface.rs`: the function `path`, which takes a
// module's `import.meta` where Rust takes the module's path.

// @hand export path
// No `flm-core` counterpart: the qualified path of the file at `relative`
// beside the module whose `import.meta` is `meta`, as Rust's `path` with
// `meta.path` (the module's path inside the extension, which the engine sets):
// `path(import.meta, "./style.css")` is `"self:templates/html/style.css"` in
// the module `templates/html/letter.js`.
export function path(meta, relative) { return call(MODULE, "path", [meta.path, relative]); }
