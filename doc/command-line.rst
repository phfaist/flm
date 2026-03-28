Command-Line Usage
==================

The ``flm`` command-line tool compiles FLM documents into various output
formats.


Basic Usage
-----------

Compile an FLM file to standard output::

    flm mydocument.flm

Write output to a file::

    flm mydocument.flm -o mydocument.html

Specify the output format::

    flm mydocument.flm -o mydocument.html --format=html

Read from standard input (when no files are given)::

    echo '\emph{Hello, world.}' | flm --format=html


Output Formats
--------------

Use the ``-f`` / ``--format`` flag to select the output format:

``html``
    HTML output.  Math is marked up with ``<span>`` tags for use with
    `MathJax <https://www.mathjax.org/>`_.

``text``
    Plain text output.

``latex``
    LaTeX source output.

``markdown``
    Markdown output.

``pdf``
    PDF output (requires a LaTeX distribution; use with
    ``--workflow=runlatexpdf``).

You can also specify a fully qualified Python module or class name defining a
custom ``FragmentRenderer`` subclass.


Templates
---------

Use the ``-t`` / ``--template`` flag to select a template for wrapping the
rendered content::

    flm mydocument.flm -o mydocument.html -f html -t simple

Specifying an empty template outputs the raw rendered fragment content without
any surrounding template::

    flm mydocument.flm -f html -t ''

Additional HTML templates
^^^^^^^^^^^^^^^^^^^^^^^^^

The `flm-templates <https://github.com/phfaist/flm-templates>`_ and
`flm-htmlplus <https://github.com/phfaist/flm-htmlplus>`_ extension packages
provide additional templates and template engines.  Install them with::

    pip install flm-templates flm-htmlplus

Then use them::

    flm mydocument.flm -o output.html -w flm_htmlplus -P 'pkg:flm_templates' -t sunset

Try also the template ``-t oldtextbook``.

Template search paths
^^^^^^^^^^^^^^^^^^^^^

Use ``-P`` / ``--template-path`` to add directories or packages to the template
search path.  You can specify this option multiple times.  Each path is either a
relative or absolute directory, or of the form ``pkg:flm_pkg_name`` to load
template paths from an FLM Python extension package::

    flm mydocument.flm -P ./my-templates -P pkg:flm_templates -t mytemplate


Workflows
---------

A *workflow* controls how the FLM document is processed and rendered.  Use
``-w`` / ``--workflow`` to select a workflow:

``templatebasedworkflow`` (default)
    Renders the document and wraps it in a template.

``runlatexpdf``
    Renders to LaTeX and compiles to PDF.  Requires a LaTeX distribution (e.g.,
    TeX Live) to be installed.  Use with ``--format=pdf``::

        flm mydocument.flm -o mydocument.pdf -w runlatexpdf -f pdf

``flmlatex``
    Renders FLM to pure LaTeX source using the FLM recomposer.  This differs
    from the default workflow's ``--format=latex`` in that it uses the
    ``FLMPureLatexRecomposer`` to produce LaTeX output.

You can also specify a fully qualified Python module or class name for a custom
workflow.


Configuration
-------------

Use ``-C`` / ``--config`` to specify a configuration file::

    flm mydocument.flm -C myconfig.yaml

By default, ``flmconfig.yaml`` in the current directory is used if it exists.

Use ``--inline-config`` to pass additional configuration as inline YAML or
JSON.  This is merged into the document's front-matter configuration.  You can
specify this option multiple times; earlier occurrences take precedence::

    flm mydocument.flm --inline-config '{"flm": {"parsing": {"dollar_inline_math_mode": true}}}'

Use ``--print-merged-config`` to debug your configuration by printing the fully
merged config and exiting.

See :doc:`configuration` for full details on the configuration system.


Watch Mode
----------

Use ``-W`` / ``--watch`` to continuously monitor the input file and
automatically update the output file when it changes::

    flm mydocument.flm -o mydocument.html -f html -t simple -W


Opening the Output
------------------

Use ``-V`` / ``--view`` to open the output file with your browser or default
desktop application after compilation.  Requires ``-o`` / ``--output`` to be
specified::

    flm mydocument.flm -o mydocument.html -f html -t simple -V


Other Options
-------------

``-o FILE`` / ``--output FILE``
    Write output to the given file instead of standard output.  Use
    ``--output=-`` to explicitly write to standard output.

``-c CONTENT`` / ``--flm-content CONTENT``
    Parse the given FLM content string instead of reading from a file.

``-B`` / ``--force-block-level``
    Force parsing of the input as block-level (paragraph) content.

``-n`` / ``--suppress-final-newline``
    Do not add a trailing newline at the end of the output.

``-v`` / ``--verbose``
    Enable verbose debugging output.

``--very-verbose``
    Enable very detailed debugging output, including pylatexenc debug messages.

``--version``
    Print the FLM version and exit.
