The FLM Python Library
======================

FLM can be used as a Python library to parse FLM-formatted text and render it
to various output formats.  This is useful for integrating FLM into web
applications, static site generators, template engines, or any tool that needs
to process LaTeX-like markup.


Installation
------------

::

    pip install flm-core


Basic Usage
-----------

The simplest way to use FLM is to create an environment with the standard
features, parse a fragment of FLM text, and render it:

.. code-block:: python

    from flm.flmenvironment import make_standard_environment
    from flm.stdfeatures import standard_features
    from flm.fragmentrenderer.html import HtmlFragmentRenderer

    # Create an environment with the standard set of features
    environ = make_standard_environment(features=standard_features())

    # Parse a fragment of FLM text in standalone mode
    fragment = environ.make_fragment(r'Hello, \emph{world}.', standalone_mode=True)

    # Render it as a standalone fragment (no document context)
    result = fragment.render_standalone(HtmlFragmentRenderer())
    print(result)
    # Output: Hello, <span class="textit">world</span>.


Document-Mode Rendering
------------------------

For documents with multiple fragments that need to see each other (e.g., for
cross-references, consistent numbering, footnotes), use document-mode rendering.
You define a *render callback* that receives a render context and composes the
output from multiple fragments:

.. code-block:: python

    from flm.flmenvironment import make_standard_environment
    from flm.stdfeatures import standard_features
    from flm.fragmentrenderer.html import HtmlFragmentRenderer

    environ = make_standard_environment(features=standard_features())

    # Parse multiple fragments
    fragment_1 = environ.make_fragment(r'Hello, \emph{world}.')
    fragment_2 = environ.make_fragment(
        r'''Here's a question: \(1+2=?\)
    \begin{enumerate}[(a)]
    \item 1
    \item 2
    \item 3
    \end{enumerate}
    '''
    )

    # Define a render callback that composes the output
    def render_fn(render_context):
        return (
            "<main>\n"
            + "<div>" + fragment_1.render(render_context) + "</div>\n"
            + fragment_2.render(render_context) + "\n"
            + "</main>"
        )

    # Create a document from the render callback
    doc = environ.make_document(render_fn)

    # Render the document
    fragment_renderer = HtmlFragmentRenderer()
    result_html, render_context = doc.render(fragment_renderer)

    print(result_html)
    # Output:
    # <main>
    # <div>Hello, <span class="textit">world</span>.</div>
    # <p>Here's a question: <span class="inline-math">\(1+2=?\)</span></p>
    # <dl class="enumeration enumerate">
    #   <dt>(a)</dt><dd><p>1</p></dd>
    #   <dt>(b)</dt><dd><p>2</p></dd>
    #   <dt>(c)</dt><dd><p>3</p></dd>
    # </dl>
    # </main>

The advantage of document-mode rendering is that different fragments can "see"
each other --- for instance, a ``\ref`` in one fragment can reference a
``\label`` in another.  This is particularly useful in combination with template
engines, where different parts of a page are rendered from separate FLM
fragments.

Note that math content is marked up with ``<span>`` tags for use with
`MathJax <https://www.mathjax.org/>`_.


Fragment Renderers
------------------

FLM provides several built-in fragment renderers:

- :py:class:`~flm.fragmentrenderer.html.HtmlFragmentRenderer` --- HTML output
- :py:class:`~flm.fragmentrenderer.text.TextFragmentRenderer` --- plain text output
- :py:class:`~flm.fragmentrenderer.latex.LatexFragmentRenderer` --- LaTeX source output
- :py:class:`~flm.fragmentrenderer.markdown.MarkdownFragmentRenderer` --- Markdown output


Configuring Features
--------------------

You can customize which features are available and how they behave by passing
options to ``standard_features()``:

.. code-block:: python

    from flm.stdfeatures import standard_features

    features = standard_features(
        # Disable theorems
        theorems=False,
        # Use roman numerals for footnotes
        footnote_counter_formatter='roman',
        # Use custom equation counter formatter
        eq_counter_formatter='Roman',
    )

For more fine-grained control, instantiate feature classes directly:

.. code-block:: python

    from flm.feature.baseformatting import FeatureBaseFormatting
    from flm.feature.math import FeatureMath
    from flm.feature.headings import FeatureHeadings

    features = [
        FeatureBaseFormatting(),
        FeatureMath(counter_formatter='roman'),
        FeatureHeadings(
            section_commands_by_level={
                1: {'cmdname': 'chapter'},
                2: {'cmdname': 'section'},
                3: {'cmdname': 'subsection'},
            },
        ),
        # ... add more features as needed
    ]

Then pass the feature list to ``make_standard_environment()``:

.. code-block:: python

    from flm.flmenvironment import make_standard_environment

    environ = make_standard_environment(features=features)


Key Concepts
------------

- **Environment** (:py:class:`~flm.flmenvironment.FLMEnvironment`) --- collects
  feature definitions and parsing settings.  Provides ``make_fragment()`` and
  ``make_document()`` methods.

- **Fragment** (:py:class:`~flm.flmfragment.FLMFragment`) --- a piece of parsed
  FLM text, represented as a node tree.  Can be rendered standalone or within a
  document.

- **Document** (:py:class:`~flm.flmdocument.FLMDocument`) --- collects
  fragments for rendering together, enabling cross-references, consistent
  numbering, and footnote collection.

- **Render Context** (:py:class:`~flm.flmrendercontext.FLMRenderContext`) ---
  carries state during rendering, including feature managers and delayed render
  data.

- **Fragment Renderer** (:py:class:`~flm.fragmentrenderer.FragmentRenderer`) ---
  produces the final output in a specific format (HTML, text, etc.).

See :doc:`model` for a deeper discussion of the FLM document model and
rendering pipeline, and the :doc:`api` for full API documentation.
