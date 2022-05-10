import logging
logger = logging.getLogger(__name__)


class LLMDocument:

    def __init__(self,
                 render_callback,
                 environment,
                 features=None):
        super().__init__()

        # set up environment, callback function, fragment_renderer
        self.environment = environment
        self.render_callback = render_callback

        # set up feature managers
        if not features:
            features = []
        else:
            features = list(features)

        self.features = features

        self.feature_managers_list = [
            f.spawn_document_manager(self)
            for f in self.features
        ]
        self.feature_managers = {
            fm.feature_name: fm
            for fm in self.feature_managers_list
        }

        # more flags, etc.

        self.two_pass_mode_is_second_pass = False

        self._delayed_id_counter = 1 # only used if we have delayed content markers

        self._delayed_render_nodes = {} # key => node
        self._delayed_render_content = {} # key => string-content


    def supports_feature(self, feature_name):
        return ( feature_name in self.feature_managers )

    def feature_manager(self, feature_name):
        return self.feature_managers[feature_name]


    def render(self, fragment_renderer):
        r"""
        ...........

        must be called only once on a given instance!
        """
        logger.debug("document render()")

        # first, initialize our feature managers
        for feature_manager in self.feature_managers_list:
            feature_manager.initialize()

        # assumes no delayed rendering occurs
        value = self.render_callback(self, fragment_renderer)
        if value is None:
            logger.warning("The LLM document render callback function returned `None`! Did "
                           "you forget a ‘return ...’ instruction?")

        logger.debug("first pass -> value = %r", value)

        # do any necessary processing required by the feature managers, in the
        # order they were specified

        for feature_manager in self.feature_managers_list:
            feature_manager.process(fragment_renderer, value)

        # now render all the delayed nodes

        for key, node in self._delayed_render_nodes.items():
            # render the content of these delayed-render nodes now.  We know
            # that the node's llm_specinfo must have a render() method because
            # it's a delayed render node.
            self._delayed_render_content[key] = \
                node.llm_specinfo.render(node, self, fragment_renderer)

        # now produce the final, rendered result

        if fragment_renderer.supports_delayed_render_markers:

            # Fix the resulting value, whether it is a dictionary, list, or a
            # single string.  We allow general values like dict or list in case
            # the renderer actually wants to render separate parts of a page and
            # keep them separate for future use.

            fix_string_fn = lambda s: \
                fragment_renderer.replace_delayed_markers_with_final_values(
                    s,
                    self._delayed_render_content
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
            self.two_pass_mode_is_second_pass = True
            value = self.render_callback(self, fragment_renderer)

        logger.debug("ok, got final_value = %r", value)

        for feature_manager in self.feature_managers_list:
            feature_manager.postprocess(fragment_renderer, value)

        return value


    def render_fragment(self, fragment, fragment_renderer, **kwargs):
        return fragment_renderer.render_fragment(fragment, self, **kwargs)


    def register_delayed_render(self, node, fragment_renderer):
        # register the node for delayed render, generate a key for it, and
        # return the key
        key = self._delayed_id_counter
        self._delayed_id_counter += 1
        self._delayed_render_nodes[key] = node
        node.llm_delayed_render_key = key
        return key

    def get_delayed_render_content(self, node):
        return self._delayed_render_content[node.llm_delayed_render_key]


