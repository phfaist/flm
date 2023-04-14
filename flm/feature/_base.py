

class Feature:

    # ---

    feature_name = None
    r"""
    A name that should uniquely identify this feature.
    """

    feature_dependencies = None
    r"""
    If non-`None`, then this is a list (or set) of feature names that must
    also be used in a given :py:class:`~flm.flmenvironment.FLMEnvironment`
    instance for the present feature to function as intended.  These dependency
    features will be initialized prior to the present feature.
    """

    feature_optional_dependencies = None
    r"""
    If non-`None`, then this is a list (or set) of feature names that may
    enhance the functionality of the present feature.  If these features are
    activated, they will be initialized prior to the present feature.
    """

    feature_default_config = {}
    r"""
    The default configuration tree for this feature.  The
    :py:mod:`~flm.run` module uses these defaults when no configuration is
    specified for a given feature.  Recall that the configuration is a
    dictionary of key/value pairs that will be specified by :py:mod:`~flm.run`
    as keyword arguments to the constructor of the feature instance.  If you
    create feature instances yourself, then you're responsible anyways for the
    arguments you specify to the constructor, and you are responsible for
    honoring or ignoring the values in `feature_default_config`.
    """


    # ---

    class DocumentManager:
        def __init__(self, feature, doc, **kwargs):
            super().__init__(**kwargs)
            self.feature = feature
            self.feature_name = self.feature.feature_name
            self.doc = doc
            self.RenderManager = self.feature.RenderManager

        def initialize(self):
            pass

        def get_node_id(self, node):
            return self.feature.get_node_id(node)

    # ---

    class RenderManager:
        def __init__(self, feature_document_manager, render_context, **kwargs):
            super().__init__(**kwargs)
            self.feature_document_manager = feature_document_manager
            self.feature = self.feature_document_manager.feature
            self.feature_name = self.feature.feature_name
            self.render_context = render_context

        def initialize(self):
            r"""
            Initialize the render manager.  You should subclass this
            method, and avoid subclassing the constructor.  You'll get all the
            `feature_render_options` for this feature (which you provided to the
            document's render() method) as keyword arguments to this method.
            """
            pass

        def process(self, first_pass_value):
            pass

        def postprocess(self, final_value):
            pass

        def get_node_id(self, node):
            return self.feature.get_node_id(node)


    # -----

    def add_latex_context_definitions(self):
        r"""
        Reimplement to add additional definitions to the latex context
        database.
        """
        return {}
        

    # ---

    def get_node_id(self, node):
        r"""
        Helper method to get a unique hashable identifier key (integer or tuple)
        associated with the object `node`.  The result can be used for instance
        as a dictionary key to store data that needs to be associated with a
        given object instance or with given unique identifying information.

        The argument `node` is assumed to be either an object instance (e.g., a
        `LatexNode` instance) or a tuple of hashable data.  In the first case,
        this method returns the object's `id(node)`, providing a unique key
        associated with that object instance, and in the latter case, the tuple
        is returned as is.
        """
        if isinstance(node, tuple):
            # In this case, the tuple directly provides a unique identifying
            # data; return it as is
            return node
        return node.node_id



class SimpleLatexDefinitionsFeature(Feature):
    
    DocumentManager = None
    RenderManager = None

    latex_definitions = {}
    r"""
    Set to a dictionary with one or more of the keys ('macros',
    'environments', 'specials'), whose corresponding values are lists of
    FLMMacroSpec, FLMEnvironmentSpec, and FLMSpecialsSpec instances.
    """

    def add_latex_context_definitions(self):
        return self.latex_definitions



