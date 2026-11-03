// The hand-written parts of `flm:stdcore/features/refs`, spliced into the
// generated stub `api/stdcore/features/refs.js` by `tests/api_surface.rs`:
// the function `label_fields_arg`, which reads its answer back as an
// `ArgDef`.

import { ArgDef } from "flm:lang/mint";

// @hand export label_fields_arg
// Mirrors `label_fields_arg`: the argument that reads the `\label{…}` fields
// written after a construct, named `FIELDS_ARG`, to place last in the
// construct's argument list.
export function label_fields_arg() { return ArgDef.read_(call(MODULE, "label_fields_arg", [])); }
