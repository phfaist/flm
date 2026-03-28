.. _configuration:

Configuration
=============

FLM has a flexible configuration system.  Configuration controls parser
settings, renderer options, feature selection and options, and workflow behavior.
Configuration can come from multiple sources that are merged together in a
well-defined order.


.. _config-hierarchy:

Configuration Hierarchy
-----------------------

Configuration is merged from several sources.  When the same setting appears in
multiple sources, the *highest priority* source wins.  From highest to lowest
priority:

1. **CLI flags** (``-f``, ``-w``, etc.) and ``--inline-config``
2. **YAML front matter** in the input file
3. **Configuration file** (``flmconfig.yaml`` or specified with ``-C``)
4. **Inline default config** (``--inline-default-config``)
5. **Built-in defaults**


.. _config-file-discovery:

Configuration File Discovery
-----------------------------

When no explicit configuration file is given with ``-C``, FLM looks for a
configuration file in the current directory.  It tries the following names in
order, stopping at the first match:

- ``flmconfig+WORKFLOW.FORMAT.yaml`` (or ``.yml``)
- ``flmconfig+WORKFLOW.yaml`` (or ``.yml``)
- ``flmconfig.FORMAT.yaml`` (or ``.yml``)
- ``flmconfig.yaml`` (or ``.yml``)

Here, ``WORKFLOW`` and ``FORMAT`` are replaced by the selected workflow and
output format names.  This allows you to maintain separate configurations for
different output formats or workflows.


.. _config-structure:

Configuration Structure
-----------------------

The top-level structure of an FLM configuration file is:

.. code-block:: yaml

    flm:
      parsing:
        # Parser settings
        ...
      features:
        # Feature selection and per-feature configuration
        ...
      renderer:
        # Per-format renderer options
        ...
      workflow:
        # Workflow-specific configuration
        ...

You can also include top-level metadata such as a document title:

.. code-block:: yaml

    title: 'My Document Title'
    flm:
      ...


.. _config-frontmatter:

YAML Front Matter
-----------------

FLM documents can include YAML front matter at the top of the file, delimited
by ``---`` lines.  The front matter is merged into the configuration with higher
priority than the ``flmconfig.yaml`` file:

.. code-block:: latex

    ---
    title: 'My FLM Document'
    flm:
      parsing:
        dollar_inline_math_mode: true
      features:
        endnotes:
          categories:
            - category_name: footnote
              counter_formatter: unicodesuperscript
              heading_title: 'Footnotes'
              endnote_command: 'footnote'
    ---

    \section{Greeting}
    Hello, \emph{world}. Inline math can now also be typeset
    as $a$ and $b$.


.. _config-import:

The ``$import`` Directive
-------------------------

Use the ``$import`` directive to import configuration from external files, URLs,
or Python extension packages:

.. code-block:: yaml

    $import:
      - my-flm-config.yaml
    flm:
      ...

The ``$import`` target can be:

- A **relative or absolute file path**: ``my-flm-config.yaml`` or
  ``/path/to/my/flm-config.yaml``
- A **URL**: ``https://example.com/my/flm-config.yaml``
- A **Python package** prefixed with ``pkg:``: ``pkg:flm_citations``

When a package name is specified, the package is loaded and its default FLM
configuration is extracted from its ``flm_default_import_config`` attribute (a
dictionary or a callable that returns a dictionary).

You can optionally follow the package name with a path to specify
submodules/attributes to read instead of ``flm_default_import_config``.  For
example, ``pkg:mypackage/foo/bar`` will import the module ``mypackage`` and read
the configuration dictionary stored in ``mypackage.foo.bar``.

Multiple imports are specified as a list; they are processed in order.
Configuration specified alongside the import is merged on top.


.. _config-parser:

Parser Configuration
--------------------

.. code-block:: yaml

    flm:
      parsing:
        # Enable/disable comments as in LaTeX, led by '%%'
        enable_comments: true

        # This string initiates a comment up to the end of
        # the line, if comments are enabled.  By default we
        # require TWO '%' signs to avoid accidental comments
        # (e.g., "... is up 10% from ...")
        comment_start: '%%'

        # Enable/disable math mode with $ signs as in LaTeX
        # in addition to \( ...\)
        dollar_inline_math_mode: false

        # Force parsing of the content in block-level mode
        # (paragraphs).  Here 'null' means to auto-detect
        force_block_level: null


.. _config-renderer:

Renderer Configuration
----------------------

Renderer options are specified under the format name within the ``renderer``
key.  Only the options for the selected output format are used.

HTML renderer
^^^^^^^^^^^^^

.. code-block:: yaml

    flm:
      renderer:
        html:
          use_link_target_blank: false
          html_blocks_joiner: ''
          # use 'heading_tags_by_level: null' for defaults
          heading_tags_by_level:
            1: h1
            2: h2
            3: h3
            4: span
            5: span
            6: span
          inline_heading_add_space: true

Text renderer
^^^^^^^^^^^^^

.. code-block:: yaml

    flm:
      renderer:
        text:
          display_href_urls: true

LaTeX renderer
^^^^^^^^^^^^^^

.. code-block:: yaml

    flm:
      renderer:
        latex:
          heading_commands_by_level:
            1: "section"
            2: "subsection"
            3: "subsubsection"
            4: "paragraph"
            5: "subparagraph"
            6: null

Markdown renderer
^^^^^^^^^^^^^^^^^

.. code-block:: yaml

    flm:
      renderer:
        markdown:
          use_target_ids: 'github'  # or 'anchor' or 'pandoc' or null


.. _config-features:

Feature Configuration
---------------------

Features are selected and configured under the ``features`` key.  Each feature
is identified by its name.

**Enable a feature** with default options (use an empty dict):

.. code-block:: yaml

    flm:
      features:
        headings: {}
        refs: {}

**Disable a feature:**

.. code-block:: yaml

    flm:
      features:
        theorems: false

**Configure a feature** by providing a dict of options:

.. code-block:: yaml

    flm:
      features:
        endnotes:
          categories:
            - category_name: footnote
              counter_formatter: alph
              heading_title: 'Footnotes'
              endnote_command: 'footnote'
          render_options:
            include_headings_at_level: 1
            set_headings_target_ids: true
            endnotes_heading_title: null
            endnotes_heading_level: 1

Here is a more comprehensive example showing several features with their
configuration:

.. code-block:: yaml

    flm:
      features:
        enumeration:
          enumeration_environments:
            enumerate:
              counter_formatter: null  # use defaults
            itemize:
              counter_formatter:
                - "\u2022"
                - '-'
                - "\u25B8"
        refs: {}
        endnotes:
          categories:
            - category_name: footnote
              counter_formatter: alph
              endnote_command: footnote
              heading_title: Footnotes
          render_options:
            include_headings_at_level: 1
            set_headings_target_ids: true
        floats:
          float_types:
            - counter_formatter: Roman
              float_caption_name: Fig.
              float_type: figure
              content_handlers: ['any', 'includegraphics', 'cells']
            - counter_formatter: Roman
              float_caption_name: Tab.
              float_type: table
              content_handlers: ['cells']
        defterm: {}
        graphics: {}

You can also load features from external packages by using their fully qualified
Python module path as the feature name:

.. code-block:: yaml

    flm:
      features:
        'flm.main.feature_graphics_collection': {}


.. _config-formats:

Per-Format Configuration
------------------------

The built-in defaults include per-format overrides for renderer options and
feature settings.  For example, when rendering to text output, the endnotes
counter formatter is automatically set to ``unicodesuperscript`` instead of
``alph``.

You do not usually need to worry about per-format configuration, as the
defaults handle common cases.  If you need to customize per-format behavior, you
can use format-specific config files (e.g., ``flmconfig.html.yaml``) or specify
settings in the renderer section for each format.


.. _config-extensions:

Extension Packages
------------------

FLM can be extended with external Python packages that provide additional
features, templates, and workflows.

flm-citations
^^^^^^^^^^^^^

Automatically fetch citations from arXiv, DOI cross-references, or bibliography
files.  Install and import:

.. code-block:: bash

    pip install flm-citations

.. code-block:: yaml

    $import:
      - pkg:flm_citations
    bibliography:
      - mybibliography.yaml
      - anotherbib.json

Citations are organized by prefix:

- ``arxiv:<arXiv ID>`` --- retrieved from the arXiv API
- ``doi:<DOI>`` --- retrieved from doi.org
- ``manual:{Manual citation}`` --- uses the text directly
- ``bib:key`` --- looks up a key in the specified bibliography files
  (CSL-JSON or CSL-YAML format)

flm-templates
^^^^^^^^^^^^^

Additional HTML templates and template engines.  Install and use:

.. code-block:: bash

    pip install flm-templates flm-htmlplus

.. code-block:: bash

    flm mydocument.flm -o output.html -w flm_htmlplus \
        -P 'pkg:flm_templates' -t sunset

Try also the template ``-t oldtextbook``.

Writing your own extension packages
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Extension packages are Python packages that can expose:

- A ``flm_default_import_config`` attribute (or callable) for use with the
  ``$import: pkg:...`` directive
- A ``get_template_path()`` function for template discovery
- Custom feature classes for use in the ``features`` configuration

See the `flm-citations source <https://github.com/phfaist/flm-citations>`_ for
an example of how to structure an extension package.


.. _config-special-directives:

Advanced: Config Merger Directives
----------------------------------

The configuration merger supports several special directives for advanced use
cases:

``$import``
    Import configuration from a file, URL, or package.  See :ref:`config-import`.

``$defaults``
    Pull in default values from lower-priority configuration sources.  This is
    used internally by the default configuration to ensure that user
    configuration is merged with sensible defaults.

``$merge-config``
    Merge configuration into a named item within a list.  Useful for modifying a
    specific item (identified by its ``name`` field) without replacing the entire
    list:

    .. code-block:: yaml

        - $merge-config:
            name: footnote
            config:
              counter_formatter: roman

``$remove-item``
    Remove a named item from a list:

    .. code-block:: yaml

        - $remove-item: footnote

``$no-merge``
    Mark a dictionary as non-mergeable.  When this directive is present, the
    dictionary replaces any lower-priority value entirely instead of being
    recursively merged:

    .. code-block:: yaml

        heading_tags_by_level:
          $no-merge: true
          1: h2
          2: h3
