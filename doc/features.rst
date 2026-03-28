.. _flm-features:

FLM Features
============

FLM organizes its functionality into *features* --- pluggable modules that
define macros, environments, and specials.  Features are the mechanism by which
FLM provides (and lets you customize) the available markup.

FLM is a framework, not a fixed language.  The macros and environments available
in your documents depend entirely on which features are loaded.  You can enable
or disable any feature, configure its behavior, or write your own custom
features.


How Features Work
-----------------

Each feature is a Python class inheriting from
:py:class:`~flm.feature.Feature`.  A feature:

- Defines macros, environments, and/or specials via its
  ``add_latex_context_definitions()`` method.
- May provide a ``DocumentManager`` for per-document state (e.g., managing
  numbering or collecting footnotes).
- May provide a ``RenderManager`` for per-render state.
- Can declare dependencies on other features.


Default Features
----------------

The following features are enabled by default when using the standard
configuration or the ``standard_features()`` helper:

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Feature
     - Description
     - Syntax Reference
   * - ``baseformatting``
     - Text emphasis, bold, character escapes, paragraph breaks, non-breaking
       spaces.
     - :ref:`standard-syntax-formatting`
   * - ``href``
     - Hyperlinks: ``\href``, ``\url``, ``\email``.
     - :ref:`standard-syntax-links`
   * - ``verbatim``
     - Verbatim and code typesetting: ``\verbcode``, ``\verbtext``,
       ``verbatimcode``, ``verbatimtext`` environments.
     - :ref:`standard-syntax-verbatim`
   * - ``math``
     - Mathematics: inline ``\( ... \)``, display environments (``equation``,
       ``align``, ``gather``), ``\eqref``.
     - :ref:`standard-syntax-math`
   * - ``headings``
     - Sectioning commands: ``\section``, ``\subsection``, etc.
     - :ref:`standard-syntax-headings`
   * - ``enumeration``
     - Lists: ``enumerate``, ``itemize``, ``description`` environments.
     - :ref:`standard-syntax-lists`
   * - ``refs``
     - Cross-reference management: ``\ref``, ``\hyperref``.
     - :ref:`standard-syntax-refs`
   * - ``endnotes``
     - Endnotes and footnotes: ``\footnote``.
     - :ref:`standard-syntax-footnotes`
   * - ``floats``
     - Figures and tables: ``figure``, ``table`` environments with captions and
       numbering.
     - :ref:`standard-syntax-figures`
   * - ``graphics``
     - Graphics inclusion: ``\includegraphics``.
     - :ref:`standard-syntax-figures`
   * - ``defterm``
     - Definition terms: ``defterm`` environment, ``\term`` macro.
     - :ref:`standard-syntax-defterm`
   * - ``theorems``
     - Theorems, lemmas, proofs, and related environments.
     - :ref:`standard-syntax-theorems`


Opt-In Features
---------------

The following features are available but must be explicitly enabled in the
configuration:

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Feature
     - Description
     - Syntax Reference
   * - ``substmacros``
     - Custom substitution macros defined via configuration.
     - :ref:`standard-syntax-substmacros`
   * - ``quote``
     - Block quote environments with attribution.
     - :ref:`standard-syntax-quotes`
   * - ``cells``
     - Table-like cell layout.
     - :ref:`standard-syntax-cells`
   * - ``citations``
     - Citations (via the ``flm-citations`` extension package).
     - :ref:`standard-syntax-citations`
   * - ``annotations``
     - Inline annotations and comments.
     - See :py:mod:`flm.feature.annotations`
   * - ``markup``
     - Custom text formatting macros and semantic block environments.
     - See :py:mod:`flm.feature.markup`


Enabling and Configuring Features
---------------------------------

Features are configured in your ``flmconfig.yaml`` or in the document's YAML
front matter.  See :ref:`config-features` for the full details.

**Enable a feature** with default options:

.. code-block:: yaml

    flm:
      features:
        quote: {}

**Disable a feature:**

.. code-block:: yaml

    flm:
      features:
        theorems: false

**Configure a feature** with custom options:

.. code-block:: yaml

    flm:
      features:
        endnotes:
          categories:
            - category_name: footnote
              counter_formatter: roman
              heading_title: 'Footnotes'
              endnote_command: 'footnote'


Using Features from Extension Packages
---------------------------------------

External features can be loaded by using their fully qualified Python module path
as the feature name in the configuration:

.. code-block:: yaml

    flm:
      features:
        'my_extension_package.my_feature': {}

Or by importing the package's default configuration with ``$import``:

.. code-block:: yaml

    $import:
      - pkg:my_extension_package


Writing Custom Features
-----------------------

You can write your own features by creating a Python class that inherits from
:py:class:`~flm.feature.Feature`.  The key method to implement is
``add_latex_context_definitions()``, which returns a dictionary of macros,
environments, and specials to register.

See the :ref:`flm-api-features` API documentation and the source code of the
built-in features in the ``flm/feature/`` directory for examples.
