// The hand-written part of `flm:model/element`, spliced into the generated
// stub `api/model/element.js` by `tests/api_surface.rs`: `ElementSpec.with`,
// which lends the handler object written in an `{ Expand: hook }`,
// `{ Referenceable: object }` or `{ Sectioning: object }` aspect (the
// protocol for handler objects).
import { aspect_for } from "flm:_runtime";

// @hand method ElementSpec.with
  // Attaches an aspect. A handler object (an expansion hook, a plain object
  // with `expand` and an optional `policy`; a `Referenceable`, with
  // `points`; a `Sectioning`, with `level` and the optional `in_toc` and
  // `toc_title`) is recorded under the element's name and lent.
  with(aspect) {
    this.aspects_.push(aspect_for(this.name, aspect, true));
    return this;
  }
