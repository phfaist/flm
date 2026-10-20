// The hand-written part of `flm:stdcore/backends/lowering`, spliced into the
// generated stub `api/stdcore/backends/lowering.js` by `tests/api_surface.rs`:
// the class `LoweringStage`, whose constructor answers a `LibraryStage`.
// An import line here is copied into the stub.
import { LibraryStage } from "flm:core/registry";

// @hand export LoweringStage
// The library's lowering stage for a backend: `new LoweringStage(backend)`
// answers it as an object of the class `LibraryStage` (as Rust's
// `LoweringStage::new` answers a `LibraryStage`), the value a
// `BackendDef`'s `lowering` takes; so it is not `instanceof LoweringStage`.
export class LoweringStage extends LibraryStage {
  constructor(backend) { return call(MODULE, "LoweringStage::new", [backend]); }
}
