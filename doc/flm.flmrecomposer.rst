Recompose FLM text: ``flm.flmrecomposer``
=========================================

This module provides tools to “recompose” FLM text, i.e., to transform a list of
parsed FLM nodes back into FLM code.  You can even recompose the node tree into
standard LaTeX code.  The FLM or LaTeX code is put together without any
render-level processing (no equation numbering, etc.).


.. automodule:: flm.flmrecomposer
   :members:


.. autoclass:: FLMNodesFlmRecomposer
   :members:
   :undoc-members:




Recomposing “pure” (“standard”) LaTeX code
------------------------------------------

.. automodule:: flm.flmrecomposer.purelatex

.. autoclass:: FLMPureLatexRecomposer
   :members:
   :undoc-members:
