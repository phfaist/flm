API Reference
=============

This section provides the full Python API reference for the FLM library.

The API is organized into the following areas:

- :doc:`api-parse` --- core classes for defining FLM constructs, setting up
  environments, and compiling FLM text into fragments and documents.

- :doc:`api-render` --- the render context and fragment renderer classes that
  produce output in specific formats (HTML, text, LaTeX, Markdown).

- :doc:`api-features` --- the pluggable feature system and all built-in feature
  modules.

- :doc:`api-dump` --- serialization and deserialization of compiled FLM
  fragments.

- :doc:`api-util` --- utility modules (counters, etc.).

- :doc:`api-main` --- the command-line tool internals (config merging,
  templates, workflows).

- :doc:`api-recomposer` --- tools for recomposing parsed FLM nodes back into
  FLM or LaTeX source code.

See :doc:`lib` for a narrative introduction to using FLM as a Python library.


.. toctree::
   :maxdepth: 2

   api-parse
   api-render
   api-features
   api-dump
   api-util
   api-main
   api-recomposer
   api-docgen
