The FLM Document Model
======================


- parsing in nodes

- "active" or "callable" nodes (e.g., a macro) are described by a
  :py:class:`~flm.FLMSpecInfo` instance.  The instance provides both
  the argument structure (via 
  :py:class:`pylatexenc.macrospec.MacroSpec` inheritance) as well as
  information on how to render the node with the help of a fragment
  renderer.

- rendering through callbacks



The render process
------------------

- multi-pass rendering, etc.



Some concepts
-------------

- environment ..............................

- document ..............................

- fragment ..............................

- `render_context` .............................. (and FIXME: sub_render_context
  with logical state.....)

- `target_href`, `target_id` ..............................
  --> A location target identifier (id="..." in HTML), suitable for usage in
  links

  **FIXME**: Distinguish internal id's (automatically generated to get internal
  links working) from public-facing id's (designed to be linkable from another
  page and ensure some persistency via e.g. a user-chosen label or section
  heading title)

- TODO: COUNTERS etc.


Feature-specific concepts:

- "referenceable" items - with pinned labels & refs

- 
