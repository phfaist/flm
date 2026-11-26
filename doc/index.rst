.. FLM documentation master file, created by
   sphinx-quickstart on Mon Jul 17 10:37:45 2023.
   You can adapt this file completely to your liking, but it should at least
   contain the root `toctree` directive.

Welcome to FLM's documentation!
===============================

----

⚠️⚠️⚠️ NOTICE: This is the documentation for the Python version of FLM.  The python
version is no longer maintained and will receive no further updates.  ⚠️⚠️⚠️

FLM has been rewritten in Rust.  FLM's API documentation is now on `docs.rs`:
https://docs.rs/flm-core/latest/flm_core/guide/index.html .

All future development happens on the same repository, but on the `main-rs`
branch: https://github.com/phfaist/flm .

Both python and rust versions install a command named 'flm'.  To remove the
python one, run ``pip uninstall flm-core`` (or uninstall it the same way you
installed it).

----

**Flexible Latex-Like Markup (FLM)** is a simple language strongly inspired
by LaTeX.  Parsed as a markup language, it is suited for structured
technical documents, for content that needs to be automatically postprocessed,
and can be rendered in various formats (html, text, latex, markdown).

It was developed to typeset content in the
`Error Correction Zoo <https://errorcorrectionzoo.org/>`_, but I've since been
using it for typesetting short scientific documents, as well as not-so-short
documents like a set of `lecture notes <https://phfaist.com/QEC>`_.


.. toctree::
   :maxdepth: 2
   :caption: Contents:

   overview
   standard-syntax
   command-line
   workflows
   configuration
   features
   model
   lib
   lib-js
   selfdocenv
   api


Indices and tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
