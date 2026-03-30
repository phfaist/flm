Self-Documenting Environments
=============================

The ``flm.docgen`` module provides a self-documenting environment system that
lets feature authors write structured documentation for macros, environments,
and specials directly in FLM markup.

The system defines special-purpose constructs (``\flmDocArg``,
``{flmDocArguments}``, etc.) that produce formatted argument documentation when
rendered.  I'm planning to use this internally to generate the
:doc:`standard-syntax` reference from the ``get_flm_doc()`` methods on each spec
info class.

.. automodule:: flm.docgen.docgen
   :members:
   :undoc-members:
