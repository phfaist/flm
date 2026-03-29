.. _workflows:

Workflows
=========

A *workflow* controls how FLM processes and renders your document.  While the
output *format* (``--format``) determines the target language (HTML, LaTeX,
text, Markdown), the *workflow* (``--workflow``) determines the overall
rendering pipeline --- for instance, whether to apply a template, compile LaTeX
to PDF, or recompose FLM into standard LaTeX.

Select a workflow with the ``-w`` / ``--workflow`` flag::

    flm mydocument.flm -w <workflow_name> ...

If no workflow is specified, FLM uses ``templatebasedworkflow``.  You can also
set a default workflow in your configuration:

.. code-block:: yaml

    flm:
      default_workflow: 'templatebasedworkflow'

Three built-in workflows are available:

- **templatebasedworkflow** --- Render and wrap in a template (default).
- **runlatexpdf** --- Render to LaTeX and compile to PDF.
- **flmlatex** --- Recompose FLM into pure standard LaTeX.


Choosing a Workflow
-------------------

.. list-table::
   :header-rows: 1
   :widths: 30 18 52

   * - Goal
     - Workflow
     - Command
   * - HTML page
     - (default)
     - ``flm doc.flm -o doc.html -f html``
   * - Plain text
     - (default)
     - ``flm doc.flm -o doc.txt -f text``
   * - Markdown
     - (default)
     - ``flm doc.flm -o doc.md -f markdown``
   * - Rendered LaTeX (for PDF)
     - (default)
     - ``flm doc.flm -o doc.tex -f latex``
   * - PDF
     - ``runlatexpdf``
     - ``flm doc.flm -o doc.pdf -w runlatexpdf -f pdf``
   * - Recomposed LaTeX (for inclusion in a larger LaTeX project)
     - ``flmlatex``
     - ``flm doc.flm -o doc.tex -w flmlatex``


.. _workflow-templatebasedworkflow:

``templatebasedworkflow`` --- Render and Template
-------------------------------------------------

This is the default workflow.  It renders your FLM document using the selected
output format's fragment renderer and wraps the result in a template.

Use this workflow for HTML, text, Markdown, and rendered LaTeX output.

Usage examples::

    # HTML output with the built-in simple template (the default)
    flm mydocument.flm -o mydocument.html -f html -t simple

    # HTML output with no template wrapping (raw rendered fragment)
    flm mydocument.flm -f html -t ''

    # Plain text output
    flm mydocument.flm -o mydocument.txt -f text

    # Markdown output
    flm mydocument.flm -o mydocument.md -f markdown

    # Rendered LaTeX output (FLM resolves all references and numbering)
    flm mydocument.flm -o mydocument.tex -f latex -t simple

.. note::

   This workflow does not support ``--format=pdf``.  Use the ``runlatexpdf``
   workflow instead.


Templates
^^^^^^^^^

The ``-t`` / ``--template`` flag selects a template for wrapping the rendered
content.  Templates provide the surrounding structure (e.g.,
``<html>...</html>`` for HTML, or
``\documentclass...\begin{document}...\end{document}`` for LaTeX).

Built-in templates:

- **HTML:** ``simple`` --- a clean page with configurable fonts and layout.
- **LaTeX:** ``simple`` --- an ``article``-class document with customizable
  fonts and preamble; ``revtex`` --- a `RevTeX
  <https://journals.aps.org/revtex>`_-based document for Physical Review-style
  papers.
- **Text:** ``simple`` --- a minimal wrapper with a title header.
- **Markdown:** ``simple`` --- a minimal wrapper with a title header.

Use ``-t ''`` (empty string) to suppress template wrapping and output only the
raw rendered content.


Template search paths
"""""""""""""""""""""

Use ``-P`` / ``--template-path`` to add directories or Python packages to the
template search path::

    flm mydocument.flm -P ./my-templates -P pkg:flm_templates -t mytemplate

Templates are searched in order: custom paths first, then the current directory,
then the built-in templates directory.

The extension package `flm-templates
<https://github.com/phfaist/flm-templates>`_ (and possibly others in the future)
can provide additional templates and template engines::

    pip install flm-templates
    flm mydocument.flm -o output.html -P 'pkg:flm_templates' -t sunset


Template configuration
""""""""""""""""""""""

Template selection and per-template settings are controlled via configuration.
The default template for each format can be set under ``flm.template``, and
per-template options go under ``flm.template_config``:

.. code-block:: yaml

    flm:
      template:
        html: 'simple'      # default template for HTML output
        latex: 'simple'      # default template for LaTeX output
      template_config:
        html:
          simple:
            font:
              family: 'Source Serif Pro'
              size: '16px'
            layout:
              content_width: '640px'
        latex:
          simple:
            documentclass:
              fontsize: '12pt'
            preamble:
              fonts: |
                \usepackage{newtxtext}
                \usepackage{newtxmath}
            render_toc: true


Currently, the most reliable way to identify all detailed configuration options
for a given template is to inspect its template definition.  You can explore the
`builtin template definitions on github here
<https://github.com/phfaist/flm/tree/main/flm/main/templates>`_.  Find the
`templatename`\ ``.yaml`` file for the appropriate format and inspect its
``default_config:`` section.


.. _workflow-runlatexpdf:

``runlatexpdf`` --- Compile to PDF
----------------------------------

This workflow renders your FLM document to LaTeX, wraps it in a LaTeX template,
and compiles the result to PDF using ``latexmk``.  It requires a LaTeX
distribution (such as `TeX Live <https://tug.org/texlive/>`_) to be installed on
your system.

Usage::

    flm mydocument.flm -o mydocument.pdf -w runlatexpdf -f pdf

You can also request the intermediate LaTeX source instead of the final PDF by
using ``-f latex``::

    flm mydocument.flm -o mydocument.tex -w runlatexpdf -f latex


How it works
^^^^^^^^^^^^

1. FLM renders your document using the LaTeX fragment renderer.  All
   cross-references, equation numbers, and endnotes are resolved by FLM.
2. The rendered LaTeX is wrapped in a template (``simple`` or ``revtex``).
   Any LaTeX preamble definitions collected during rendering are included
   automatically.
3. Graphics referenced via ``\includegraphics`` are collected and placed in a
   temporary build directory.
4. ``latexmk`` is invoked with ``lualatex`` to compile the document to PDF.


Template selection
^^^^^^^^^^^^^^^^^^

Select the LaTeX template with ``-t``::

    # Default article-class document
    flm mydocument.flm -o doc.pdf -w runlatexpdf -f pdf -t simple

    # RevTeX (Physical Review) style
    flm mydocument.flm -o doc.pdf -w runlatexpdf -f pdf -t revtex

The ``simple`` template uses the ``article`` document class with
LuaLaTeX-compatible fonts (Source Serif Pro and Libertinus Math by default).
The ``revtex`` template uses the RevTeX document class with options suitable for
Physical Review journals.

Template configuration for LaTeX templates can customize the document class
options, font packages, and preamble:

.. code-block:: yaml

    flm:
      template_config:
        latex:
          simple:
            documentclass:
              fontsize: '12pt'
            preamble:
              fonts: |
                \usepackage{newtxtext}
                \usepackage{newtxmath}
            render_title: true
            render_toc: true
          revtex:
            documentclass_options: 'reprint,pra,superscriptaddress'


.. _workflow-flmlatex:

``flmlatex`` --- Recompose to Standard LaTeX
---------------------------------------------

This workflow converts your FLM document into pure standard LaTeX code using the
FLM recomposer.  Unlike the rendered LaTeX output (``--format=latex`` with the
default workflow), the recomposed output leaves labels and cross-references for
the LaTeX engine to resolve via ``\label`` and ``\ref``.  This makes the output
suitable for inclusion in a larger LaTeX project.

Usage::

    flm mydocument.flm -o mydocument.tex -w flmlatex


Rendered LaTeX vs. recomposed LaTeX
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

The distinction between these two kinds of LaTeX output is important:

**Rendered LaTeX** (``-f latex``, default workflow)
    FLM resolves all cross-references, section numbers, equation numbers, and
    endnotes itself.  The output LaTeX is essentially pre-processed: its primary
    purpose is to be compiled directly to a typeset PDF.

**Recomposed LaTeX** (``-w flmlatex``)
    FLM converts its constructs into standard LaTeX equivalents (``\label``,
    ``\ref``, ``\section``, etc.) and lets the LaTeX engine handle numbering
    and cross-references.  The recomposer also tracks which LaTeX packages are
    required and emits the corresponding ``\usepackage`` commands.

Use recomposed LaTeX when you want to:

- Include FLM content as a chapter or section in a larger LaTeX document or
  book.
- Let LaTeX handle cross-references and numbering (e.g., for compatibility with
  other LaTeX tools like ``bibtex`` or ``makeindex``).
- Produce LaTeX source that is closer to hand-written LaTeX and is
  human-readable and editable.


Workflow configuration
^^^^^^^^^^^^^^^^^^^^^^

The ``flmlatex`` workflow accepts the following configuration options under
``flm.workflow_config.flmlatex``:

.. code-block:: yaml

    flm:
      workflow_config:
        flmlatex:
          # Per-feature recomposer options
          recomposer_options: {}
          # LaTeX packages to omit from the output preamble
          skip_packages: []
          # Path to a bibliography file, or false
          add_bibliography: false

``recomposer_options``
    A dictionary of per-feature options passed to the recomposer.  Keys are
    feature names; values are dictionaries of options consumed by each feature's
    ``recompose_pure_latex()`` method.

``skip_packages``
    A list of LaTeX package names to exclude from the generated
    ``\usepackage{...}`` lines.  This is useful when the surrounding LaTeX
    document already loads certain packages.

``add_bibliography`` Set to additional LaTeX content to produce at the end of
    the document, where the bibliography should typically be placed.  Set to
    `false` (the default) to omit it.  This only works if the selected
    template supports this feature.


Custom Workflows
----------------

You can also specify a fully qualified Python module or class name as the
workflow.  FLM will import the module and look for a ``RenderWorkflowClass``
attribute that should be a subclass of ``RenderWorkflow``::

    flm mydocument.flm -w my_package.my_workflow_module

See :doc:`command-line` for the full list of CLI flags, and :doc:`configuration`
for details on the configuration system.  See :doc:`api-module-workflow` for API
documentation of the relevant classes.
