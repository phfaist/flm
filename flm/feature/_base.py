from .._typing_helpers import (
    Sequence, Mapping, Set, Type, TypeNodeId, TypeDictWithLatexContextDefinitions
)


class _DocumentManager:
    r"""
    The feature instance runs globally for the environment.  A document
    manager is created for each new document.  It can be used to store
    document-related information.

    Subclasses should reimplement `initialize()` instead of providing a
    constructor.
    """

    def __init__(self, feature, doc, **kwargs):
        super().__init__(**kwargs)
        self.feature = feature
        self.feature_name = self.feature.feature_name
        self.doc = doc
        self.RenderManager = self.feature.RenderManager

    def initialize(self) -> None:
        r"""
        Called to initialize the object.  Better use this method
        instead of providing a constructor in subclasses.
        """
        pass

    def get_node_id(self, node) -> TypeNodeId:
        r"""
        Helper to obtain a unique ID associated with a node instance.
        """
        return self.feature.get_node_id(node)


class _RenderManager:
    r"""
    The feature instance runs globally for the environment.  A document
    manager is created for each new document.  A render manager is created
    for each rendering instance of the document.  It can be used to store
    render-related information (for instance, an assignment of node objects
    to equation/section/theorem numbers).

    Subclasses should reimplement `initialize()` instead of providing a
    constructor.
    """

    def __init__(self, feature_document_manager, render_context, **kwargs):
        super().__init__(**kwargs)
        self.feature_document_manager = feature_document_manager
        self.feature = self.feature_document_manager.feature
        self.feature_name = self.feature.feature_name
        self.render_context = render_context

    def initialize(self, **kwargs) -> None:
        r"""
        Initialize the render manager.  You should subclass this
        method, and avoid subclassing the constructor.

        You'll get all the `feature_render_options` for this feature (which
        you provided to the document's render() method) as keyword arguments
        to this method.
        """
        pass

    def process(self, first_pass_value) -> None:
        r"""
        Called after the first rendering pass, before delayed-render nodes
        are resolved.  Override to process the first-pass output (e.g., to
        assign numbers to items).

        :param first_pass_value: The output of the first rendering pass.
        """
        pass

    def postprocess(self, final_value) -> None:
        r"""
        Called after the final output is produced, after all delayed-render
        nodes have been resolved.  Override for any final processing.

        :param final_value: The final rendered output.
        """
        pass

    def get_node_id(self, node) -> TypeNodeId:
        r"""
        Helper to obtain a unique ID associated with a node instance.
        """
        return self.feature.get_node_id(node)



class Feature:
    r"""
    Base class to implement a FLM feature (extension).
    """

    feature_name : str|None = None
    r"""
    A name that should uniquely identify this feature.
    """

    feature_dependencies : Set[str]|Sequence[str]|None = None
    r"""
    If non-`None`, then this is a list (or set) of feature names that must
    also be used in a given :py:class:`~flm.flmenvironment.FLMEnvironment`
    instance for the present feature to function as intended.  These dependency
    features will be initialized prior to the present feature.
    """

    feature_optional_dependencies : Set[str]|Sequence[str]|None = None
    r"""
    If non-`None`, then this is a list (or set) of feature names that may
    enhance the functionality of the present feature.  If these features are
    activated, they will be initialized prior to the present feature.
    """

    feature_default_config : Mapping = {}
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

    DocumentManager : Type[_DocumentManager]|None = _DocumentManager
    r"""
    The document manager class to use for this feature.  Instances of this class
    will automatically be created when a new :py:class:`flmdocument.FLMDocument` is
    instantiated.  This class is expected to be a subclass of
    :py:class:`flm.feature.Feature.DocumentManager`.
    Alternatively, set this class attribute to `None` in your feature subclass
    to indicate that this feature does not need any document manager instance.
    """

    # ---

    RenderManager : Type[_RenderManager]|None = _RenderManager
    r"""
    The render manager class to use for this feature.  Instances of this class
    will automatically be created when rendering a :py:class:`flmdocument.FLMDocument`
    (see :py:meth:`flm.flmdocument.FLMDocument.render()`, and more specifically
    :py:meth:`flm.flmdocument.FLMDocument.make_render_context()`).
    This class is expected to be a subclass of
    :py:class:`flm.feature.Feature.RenderManager`.
    Alternatively, set this class attribute to `None` in your feature subclass
    to indicate that this feature does not need any document manager instance.
    """

    # -----

    def add_latex_context_definitions(self) -> None|TypeDictWithLatexContextDefinitions:
        r"""
        Override to provide macro, environment, and specials definitions for
        this feature.

        :returns: A dictionary with optional keys ``'macros'``,
            ``'environments'``, and ``'specials'``, whose values are lists of
            :py:class:`~flm.flmspecinfo.FLMMacroSpecBase`,
            :py:class:`~flm.flmspecinfo.FLMEnvironmentSpecBase`, and
            :py:class:`~flm.flmspecinfo.FLMSpecialsSpecBase` instances
            respectively.  Return an empty dict or ``None`` if no
            definitions are needed.
        """
        return {}
        

    # ---

    @classmethod
    def get_node_id(cls, node) -> TypeNodeId:
        r"""
        Helper method to get a unique hashable identifier key (integer or tuple)
        associated with the object `node`.  The result can be used for instance
        as a dictionary key to store data that needs to be associated with a
        given object instance or with given unique identifying information.

        The argument `node` is assumed to be either an object instance (e.g., a
        `LatexNode` instance) or a tuple of hashable data.  In the first case,
        this method essentially returns the object's `id(node)`, providing a
        unique key associated with that object instance, and in the latter case,
        the tuple is returned as is.
        """
        if isinstance(node, tuple):
            # In this case, the tuple directly provides a unique identifying
            # data; return it as is
            return node
        return node._flm_node_id





class SimpleLatexDefinitionsFeature(Feature):
    r"""
    A simple feature base class whose only purpose is to provide additional
    LaTeX definitions to the latex context of the parser, without any document
    or render managers.
    """
    
    # no need for "manager" instances - nothing to keep track of at document
    # processing or rendering time.
    DocumentManager = None
    RenderManager = None

    latex_definitions : TypeDictWithLatexContextDefinitions = {}
    r"""
    Set to a dictionary with one or more of the keys ('macros',
    'environments', 'specials'), whose corresponding values are lists of
    FLMMacroSpec, FLMEnvironmentSpec, and FLMSpecialsSpec instances.
    """

    def add_latex_context_definitions(self) -> None|TypeDictWithLatexContextDefinitions:
        return self.latex_definitions



