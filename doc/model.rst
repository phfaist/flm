The FLM Document Model
======================

This section explains the core concepts and processing pipeline behind FLM.


Parsing
-------

FLM text is parsed into a tree of *nodes* using `pylatexenc
<https://github.com/phfaist/pylatexenc>`_ (version 3).  The parser recognizes:

- **Macro nodes** --- e.g., ``\emph{text}`` produces a macro node for
  ``\emph`` with a child group node containing ``text``.
- **Environment nodes** --- e.g., ``\begin{enumerate}...\end{enumerate}``
  produces an environment node.
- **Specials nodes** --- e.g., ``~`` or a paragraph break (double newline).
- **Character nodes** --- plain text.
- **Group nodes** --- content within braces ``{...}``.
- **Math nodes** --- inline ``\(...\)`` or display math environments.

The parsing is controlled by a *latex context*, which defines which macros,
environments, and specials are recognized and what argument structure each
expects.  This is where features come in: each feature contributes definitions to
the latex context.


FLMSpecInfo: Defining Macros and Environments
---------------------------------------------

Each "active" or "callable" node (a macro, environment, or special) is described
by an :py:class:`~flm.flmspecinfo.FLMSpecInfo` instance.  The spec info object
provides both:

- The **argument structure** --- inherited from pylatexenc's macro spec classes
  (e.g., ``MacroSpec``, ``EnvironmentSpec``).  This tells the parser what
  arguments to expect.

- The **rendering behavior** --- the ``render()`` method, which produces the
  final output for the node with the help of a fragment renderer.

The main base classes are:

- :py:class:`~flm.flmspecinfo.FLMMacroSpecBase` --- for macros.
- :py:class:`~flm.flmspecinfo.FLMEnvironmentSpecBase` --- for environments.
- :py:class:`~flm.flmspecinfo.FLMSpecialsSpecBase` --- for specials.

Key properties of spec info objects:

``is_block_level``
    Whether this construct is a block-level element (like a heading or a list)
    or an inline element (like emphasis or a link).  Block-level elements cause
    paragraph breaks around them.

``allowed_in_standalone_mode``
    Whether this construct can be used in a standalone fragment (without a
    document context).  Constructs that require document-wide state (e.g.,
    cross-references, footnotes) typically set this to ``False``.

``delayed_render``
    Whether this construct uses two-pass rendering.  See
    :ref:`model-multipass` below.


The Environment
---------------

An :py:class:`~flm.flmenvironment.FLMEnvironment` collects the spec info
definitions contributed by all enabled features into a single latex context.  It
also holds the parsing state configuration.

The environment provides the key entry points:

- ``make_fragment(flm_text, ...)`` --- parse FLM text into a
  :py:class:`~flm.flmfragment.FLMFragment`.
- ``make_document(render_callback, ...)`` --- create a
  :py:class:`~flm.flmdocument.FLMDocument` for multi-fragment rendering.

Use :py:func:`~flm.flmenvironment.make_standard_environment` to create an
environment with standard settings and a given set of features.


Fragments
---------

An :py:class:`~flm.flmfragment.FLMFragment` is a piece of FLM text that has
been parsed with respect to a given environment.  It is represented internally
as a node tree.

A fragment can be rendered in two modes:

**Standalone mode** (``standalone_mode=True``)
    The fragment is rendered on its own, without a document context.  Some
    features (like cross-references and footnotes) are not available in this
    mode.  Use ``fragment.render_standalone(fragment_renderer)``.

**Document mode** (default)
    The fragment is rendered within a document, which enables cross-references,
    consistent numbering, and footnote collection.  Use
    ``fragment.render(render_context)`` inside a document's render callback.

A fragment carries optional ``resource_info`` metadata that can help locate
external resources (e.g., the filesystem directory containing image files
referenced by ``\includegraphics``).


Documents
---------

An :py:class:`~flm.flmdocument.FLMDocument` collects one or more fragments for
rendering as a coherent unit.  The concept of a document is important for:

- Consistent **numbering** of equations, sections, figures, etc.
- Resolving **cross-references** between fragments.
- Collecting **footnotes** and other endnotes.

A document is created from a *render callback* --- a function that receives a
render context and returns the composed output.  The callback typically calls
``fragment.render(render_context)`` on each fragment and assembles the results.


The Render Context
------------------

The :py:class:`~flm.flmdocument.FLMDocumentRenderContext` carries state during
the rendering process:

- **Feature document managers** --- per-document state for each feature (e.g.,
  the endnotes manager collects footnotes).
- **Feature render managers** --- per-render state for each feature (e.g.,
  mapping nodes to their assigned numbers).
- **Delayed render tracking** --- for multi-pass rendering.


.. _model-multipass:

Multi-Pass Rendering
--------------------

Some constructs need information that is not available until the entire document
has been processed.  For example, a ``\ref`` to a section that appears later in
the document needs to know the section number, which is only assigned when that
section is rendered.

FLM handles this with *delayed rendering*:

1. **First pass**: The document is rendered.  Constructs with
   ``delayed_render=True`` (like ``\ref``) register themselves and produce a
   placeholder.

2. **Second pass**: After the first pass is complete and all numbering and
   labels are assigned, the delayed nodes are rendered with the now-available
   information, and the placeholders are replaced with the final content.

This mechanism is transparent to feature authors: simply set
``delayed_render=True`` on spec info classes that need it, and the rendering
pipeline handles the rest.


Block-Level vs. Inline
----------------------

FLM distinguishes between *block-level* and *inline* content:

- **Block-level** content forms paragraphs and structural elements: headings,
  lists, figures, tables, display equations.  Paragraph breaks (double
  newlines) are only meaningful in block-level mode.

- **Inline** content flows within a paragraph: emphasis, bold, links, inline
  math, footnote marks.

The parsing state tracks whether we are in block-level mode
(:py:class:`~flm.flmenvironment.FLMParsingState`).  When
``is_block_level=None``, the system auto-detects: if the content contains any
block-level constructs, it is treated as block-level; otherwise it is inline.

The nodes finalizer
(:py:class:`~flm.flmenvironment.NodesFinalizer`) post-processes node lists to
set the ``flm_is_block_level`` flag and handle whitespace normalization
(removing spaces between block-level elements, preserving them between inline
elements).


Counters and Numbering
----------------------

Features that produce numbered items (equations, sections, figures, theorems)
use a counter system.  Counters are managed by feature document managers and
support configurable formatters:

- ``arabic`` --- 1, 2, 3, ...
- ``alph`` --- a, b, c, ...
- ``Alph`` --- A, B, C, ...
- ``roman`` --- i, ii, iii, ...
- ``Roman`` --- I, II, III, ...
- ``unicodesuperscript`` --- superscript numerals
- Custom templates via the ``template`` formatter spec

Counter formatters can be specified in the configuration for each feature that
supports numbering.


Referenceable Items
-------------------

Many features produce *referenceable items* --- entities that can be given a
label (via ``\label``) and referenced elsewhere (via ``\ref``).  These include
sections, equations, figures, tables, theorems, and definition terms.

Each referenceable item has:

- A **ref type** prefix (e.g., ``sec:``, ``eq:``, ``figure:``, ``thm:``)
- A **label** (user-chosen identifier)
- A **formatted ref text** (the text displayed by ``\ref``, e.g., "Theorem 3")

The ``refs`` feature manages the resolution of references, and the
``target_href`` / ``target_id`` system provides location identifiers for linking
(e.g., ``id="..."`` attributes in HTML output).
