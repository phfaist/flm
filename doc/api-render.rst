FLM Rendering
=============

During rendering, two main objects work together:

- The **render context** (:py:class:`~flm.flmrendercontext.FLMRenderContext`)
  carries the state of the rendering process.  It tracks which features are
  active, manages delayed rendering (for forward references like ``\ref``), and
  provides a logical-state mechanism that nodes can use to adjust their output
  depending on nesting depth or other context.

- The **fragment renderer** (:py:class:`~flm.fragmentrenderer.FragmentRenderer`)
  is responsible for producing the final output string in a specific format.
  Each output format (HTML, text, LaTeX, Markdown) has its own fragment
  renderer subclass.  The fragment renderer provides methods for rendering
  text formatting, headings, lists, math, links, and other structural elements.

The rendering pipeline proceeds as follows:

1. A :py:class:`~flm.flmdocument.FLMDocument` creates a
   :py:class:`~flm.flmdocument.FLMDocumentRenderContext` (a subclass of
   ``FLMRenderContext``) with the active features and fragment renderer.

2. The document's render callback is invoked, which calls
   :py:meth:`~flm.flmfragment.FLMFragment.render()` on each fragment.

3. Each node in the fragment's node tree calls its spec info's
   :py:meth:`~flm.flmspecinfo.FLMSpecInfo.render()` method, which in turn
   calls methods on the fragment renderer to produce the output.

4. For nodes with delayed rendering (e.g., ``\ref``), a placeholder is
   inserted on the first pass, and the actual content is filled in on
   subsequent passes.


.. toctree::
   :maxdepth: 2
   :caption: FLM Rendering Modules:

   flm.flmrendercontext
   flm.fragmentrenderer
