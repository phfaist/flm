r"""
FLM document classes for multi-fragment rendering.

A :py:class:`FLMDocument` collects multiple FLM fragments and renders them
together, enabling cross-references, consistent numbering, footnote
collection, and other features that require a global document context.

The rendering pipeline works as follows:

1. The document creates a :py:class:`FLMDocumentRenderContext` with the
   active features and fragment renderer.
2. The user-provided render callback is called, which renders fragments
   within the render context.
3. Feature render managers process the first-pass output.
4. Delayed-render nodes (e.g., ``\\ref``) are resolved.
5. The final output is produced by replacing delayed markers or by
   performing a second rendering pass.
"""

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerLocatedError

from .flmrendercontext import FLMRenderContext


class FLMDocumentRenderContext(FLMRenderContext):
    r"""
    A render context created for document-mode rendering.  Extends
    :py:class:`~flm.flmrendercontext.FLMRenderContext` with support for
    feature managers and delayed rendering.
    """
    def __init__(self, doc, fragment_renderer, feature_document_managers, **kwargs):
        super().__init__(doc=doc, fragment_renderer=fragment_renderer, **kwargs)
        self.feature_document_managers = feature_document_managers
        self.feature_render_managers = [
            ( (feature_name, fdm.RenderManager(fdm, self))
              if fdm is not None and fdm.RenderManager is not None
              else (feature_name, None) )
            for feature_name, fdm in self.feature_document_managers
        ]
        self.feature_render_managers_by_name = dict(self.feature_render_managers)

        # flags and internal counters for delayed content

        self.is_first_pass = True
        self._delayed_render_nodes = {} # key => node
        self._delayed_render_content = {} # key => string-content

        # data can be set by fragment renderers if they need to store per-render
        # event data, object instances, or else.  This should NOT be used to
        # track a logical state in the render (e.g., do NOT use .data to store
        # an indent level!)  The document renderer method will call the fragment
        # renderer's document_render_start(render_context) and
        # document_render_finish(render_context) to give it the opportunity to
        # set and/or clean up stuff in here.
        self.data = {}


    def supports_feature(self, feature_name):
        return ( feature_name in self.feature_render_managers_by_name )

    def feature_render_manager(self, feature_name):
        return self.feature_render_managers_by_name[feature_name]

    def register_delayed_render(self, node, fragment_renderer):
        # register the node for delayed render
        key = node._flm_node_id
        self._delayed_render_nodes[key] = node
        return key

    def get_delayed_render_content(self, node):
        key = node._flm_node_id
        return self._delayed_render_content[key]

    def make_standalone_fragment(self, flm_text, **kwargs):
        return self.doc.environment.make_fragment(flm_text, standalone_mode=True, **kwargs)



# ------------------------------------------------------------------------------




class FLMDocument:
    r"""
    An FLM document that collects fragments for rendering together.

    Usually you should create documents via
    :py:meth:`~flm.flmenvironment.FLMEnvironment.make_document` rather
    than constructing this class directly.

    :param render_callback: A callable ``render_callback(render_context)``
        that composes the output by calling
        :py:meth:`~flm.flmfragment.FLMFragment.render` on each fragment.
        Must return the composed output (string, dict, or list).
    :param environment: The :py:class:`~flm.flmenvironment.FLMEnvironment`
        instance.
    :param enable_features: An optional list of feature names to enable
        (subset of the environment's features).  ``None`` means all.
    :param feature_document_options: A dictionary mapping feature names to
        dictionaries of options passed to each feature's
        ``DocumentManager.initialize()``.
    :param metadata: Arbitrary user-defined metadata (e.g., document title).
    """

    def __init__(
            self,
            render_callback,
            environment,
            enable_features=None,
            feature_document_options=None,
            metadata=None,
    ):
        super().__init__()

        # set up environment, callback function, fragment_renderer
        self.environment = environment
        self.render_callback = render_callback

        # custom user-defined meta-data
        self.metadata = metadata

        # set up features & feature document managers
        self.features = self.environment.get_features_selection(enable_features)

        #logger.debug("FLMDocument constructor, features = %r", self.features)

        if feature_document_options is None:
            feature_document_options = {}
        self.feature_document_options = dict(feature_document_options)

        self.feature_document_managers = [
            ( (f.feature_name, f.DocumentManager(f, self))
              if f.DocumentManager is not None
              else (f.feature_name, None) )
            for f in self.features
        ]
        self.feature_document_managers_by_name = dict(self.feature_document_managers)

        #logger.debug("FLMDocument constructor, instantiated feature document managers = %r",
        #             self.feature_document_managers)

    def initialize(self):
        #logger.debug("FLMDocument's initialize() called")
        # initialize our feature document managers
        for feature_name, feature_document_manager in self.feature_document_managers:
            if feature_document_manager is not None:
                feature_options = self.feature_document_options.get(feature_name, {})
                feature_document_manager.initialize( **feature_options )

    def supports_feature(self, feature_name):
        return ( feature_name in self.feature_document_managers_by_name )

    def feature_document_manager(self, feature_name):
        return self.feature_document_managers_by_name[feature_name]

    def make_render_context(self, fragment_renderer, feature_render_options=None):
        # create the render context
        render_context = FLMDocumentRenderContext(
            self,
            fragment_renderer,
            self.feature_document_managers,
        )
        # and initialize our feature render managers
        if feature_render_options is None:
            feature_render_options = {}

        # want full dict() object for Transcrypt
        feature_render_options = dict(feature_render_options)

        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_options = feature_render_options.get(feature_name, dict())
                feature_render_manager.initialize(**feature_options)
        return render_context

    def render(self, fragment_renderer, feature_render_options=None):
        r"""
        Render the document and return the result.

        This method performs the full rendering pipeline:

        1. Creates a render context with feature render managers.
        2. Calls the render callback (first pass).
        3. Lets feature render managers process the first-pass output.
        4. Renders delayed-render nodes (e.g., ``\ref``).
        5. Produces the final output by replacing delayed markers (or by
           performing a second pass if the fragment renderer does not
           support delayed render markers).
        6. Calls feature render managers' ``postprocess()`` methods.

        :param fragment_renderer: A
            :py:class:`~flm.fragmentrenderer.FragmentRenderer` instance
            (e.g., :py:class:`~flm.fragmentrenderer.html.HtmlFragmentRenderer`).
        :param feature_render_options: An optional dictionary mapping
            feature names to dictionaries of options passed to each
            feature's ``RenderManager.initialize()``.
        :returns: A tuple ``(result, render_context)`` where *result*
            is the rendered output (string, dict, or list) and
            *render_context* is the
            :py:class:`FLMDocumentRenderContext` used for rendering.
        """
        #logger.debug("document render()")

        render_context = self.make_render_context(
            fragment_renderer,
            feature_render_options=feature_render_options
        )

        fragment_renderer.document_render_start(render_context)

        # first pass render or render w/o any delayed content
        value = self.render_callback(render_context)
        if value is None:
            logger.warning("The FLM document render callback function returned `None`! Did "
                           "you forget a ‘return ...’ instruction?")

        logger.debug("flm document render first pass done, will render delayed values")

        # do any necessary processing required by the feature managers, in the
        # order they were specified

        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_render_manager.process(value)

        # now render all the delayed nodes

        for key, node in render_context._delayed_render_nodes.items():
            # render the content of these delayed-render nodes now.  We know
            # that the node's flm_specinfo must have a render() method because
            # it's a delayed render node.
            try:
                render_context._delayed_render_content[key] = \
                    node.flm_specinfo.render(node, render_context)
            except LatexWalkerLocatedError as e:
                e.set_pos_or_add_open_context_from_node(
                    node,
                    what=f"{node.display_str()} (delayed render)"
                )
                if node.latex_walker.what:
                    e.set_pos_or_add_open_context_from_node(
                        node,
                        what=node.latex_walker.what
                    )
                raise e
            except ValueError as e:
                raise LatexWalkerLocatedError(str(e), pos=node.pos)

        # now produce the final, rendered result

        if fragment_renderer.supports_delayed_render_markers:

            # Fix the resulting value, whether it is a dictionary, list, or a
            # single string.  We allow general values like dict or list in case
            # the renderer actually wants to render separate parts of a page and
            # keep them separate for future use.

            fix_string_fn = lambda s: \
                fragment_renderer.replace_delayed_markers_with_final_values(
                    s,
                    render_context._delayed_render_content
                )

            if isinstance(value, dict):
                # dictionary, fix it
                value = {
                    k: fix_string_fn(s)
                    for k, s in value.items()
                }
            elif isinstance(value, list):
                value = [ fix_string_fn(x) for x in value ]
            else:
                value = fix_string_fn( value )

        else:

            # need a second pass to re-render everything with the correct values
            render_context.set_render_pass('second-pass')
            value = self.render_callback(render_context)

        #logger.debug("document render final_value = %r", value)

        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_render_manager.postprocess(value)

        fragment_renderer.document_render_finish(render_context)

        logger.debug("flm document render done")

        return value, render_context



