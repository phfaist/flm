FLM Recomposing
===============

The recomposer modules convert a parsed FLM node tree back into textual
markup without performing any render-level processing (no equation
numbering, float placement, etc.).  Two recomposer classes serve
different use cases:

- :py:class:`~flm.flmrecomposer.FLMNodesFlmRecomposer` -- produces
  round-trip **FLM markup** that closely matches the original source.
  Use this when you need to re-serialize an edited or inspected node
  tree back to FLM text.

- :py:class:`~flm.flmrecomposer.purelatex.FLMPureLatexRecomposer` --
  converts FLM constructs into **standard LaTeX** code suitable for
  direct compilation.  Use this when you want to export FLM content to
  a ``.tex`` file.  It also tracks required LaTeX packages and
  generates safe label mappings.

.. toctree::
   :maxdepth: 2
   :caption: FLM Recomposing Modules:

   flm.flmrecomposer
