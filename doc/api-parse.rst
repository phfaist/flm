FLM Code Parsing
================

Here, we document the modules related to parsing FLM content into an internal
node structure representation.

They contain:

- The *FLM specifications classes*, which provide a way to define LaTeX macros,
  LaTeX environments, and LaTeX specials to process in FLM content, along with
  generic code to render them.

- The *FLM environment* collects specification classes to define what we'd like
  to parse in FLM content, i.e., what will define the FLM constructs that are in
  use.

  The specification classes are organized in ‘FLM features,’ which can be
  thought of as the equivalent of what a LaTeX package provides.  See
  :ref:`flm-features` and :ref:`flm-api-features`.

- A *FLM document* collects pieces (“fragments”) of FLM content that are to be
  rendered together in one output unit.  For instance, a web page might be
  composed directly with HTML code, including multiple FLM fragments at various
  points in the page; the full web page (or its main contents) is a FLM
  document.

  The concept of document is important in order to enable consistent equation
  and section numbering, find relevant cross-references, and provide consistent
  citation and footnotes numbering.

- A *FLM fragment* is a segment of FLM code that has been compiled with respect
  to a given FLM environment, and is represented internally as a node tree.



.. toctree::
   :maxdepth: 2
   :caption: FLM Parsing Modules:
   
   flm.flmspecinfo
   flm.flmenvironment
   flm.flmdocument
   flm.flmfragment

