import logging
logger = logging.getLogger(__name__)

from .llmrendercontext import LLMRenderContext


class LLMDocumentRenderContext(LLMRenderContext):
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
        #self._delayed_id_counter = 1 # only used if we have delayed content markers
        self._delayed_render_nodes = {} # key => node
        self._delayed_render_content = {} # key => string-content

    def supports_feature(self, feature_name):
        return ( feature_name in self.feature_render_managers_by_name )

    def feature_render_manager(self, feature_name):
        return self.feature_render_managers_by_name[feature_name]

    def register_delayed_render(self, node, fragment_renderer):
        # register the node for delayed render, generate a key for it, and
        # return the key
        key = node.node_id #self._delayed_id_counter
        #self._delayed_id_counter += 1
        self._delayed_render_nodes[key] = node
        #node.llm_delayed_render_key = key # DON'T SET RENDER-TIME INFORMATION ON NODE OBJECT
        return key

    def get_delayed_render_content(self, node):
        return self._delayed_render_content[node.llm_delayed_render_key]



class LLMDocument:

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
        if enable_features is None:
            self.features = list( environment.features )
        else:
            self.features = [
                environment.feature(feature_name)
                for feature_name in enable_features
            ]

        #logger.debug("LLMDocument constructor, features = %r", self.features)

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

        #logger.debug("LLMDocument constructor, instantiated feature document managers = %r",
        #             self.feature_document_managers)

    def initialize(self):
        #logger.debug("LLMDocument's initialize() called")
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
        render_context = LLMDocumentRenderContext(
            self,
            fragment_renderer,
            self.feature_document_managers,
        )
        # and initialize our feature render managers
        if feature_render_options is None:
            feature_render_options = {}
        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_options = feature_render_options.get(feature_name, {})
                feature_render_manager.initialize(**feature_options)
        return render_context

    def render(self, fragment_renderer, feature_render_options=None):
        r"""
        ...........
        """
        #logger.debug("document render()")

        render_context = self.make_render_context(
            fragment_renderer,
            feature_render_options=feature_render_options
        )

        # first pass render or render w/o any delayed content
        value = self.render_callback(render_context)
        if value is None:
            logger.warning("The LLM document render callback function returned `None`! Did "
                           "you forget a ‘return ...’ instruction?")

        #logger.debug("first pass -> value = %r", value)

        # do any necessary processing required by the feature managers, in the
        # order they were specified

        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_render_manager.process(value)

        # now render all the delayed nodes

        for key, node in render_context._delayed_render_nodes.items():
            # render the content of these delayed-render nodes now.  We know
            # that the node's llm_specinfo must have a render() method because
            # it's a delayed render node.
            render_context._delayed_render_content[key] = \
                node.llm_specinfo.render(node, render_context)

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

        return value, render_context



