from .._typing_helpers import (
    Sequence, Mapping, Set, Type, TypeNodeId, TypeDictWithLatexContextDefinitions
)


class FeatureDocumentManagerBase:
    r"""
    Per-document manager for a feature.

    While the :py:class:`Feature` instance is shared across the entire
    environment, a new :py:class:`FeatureDocumentManagerBase` is created for
    each :py:class:`~flm.flmdocument.FLMDocument`.  It stores any state that
    is scoped to a single document.

    **Lifecycle** (driven by :py:class:`~flm.flmdocument.FLMDocument`):

    1. The constructor is called automatically; subclasses should **not**
       override it.
    2. :py:meth:`initialize` is called immediately after construction.
       Override this method for custom setup.

    Attributes set automatically:

    * :py:attr:`feature` -- the owning :py:class:`Feature` instance.
    * :py:attr:`feature_name` -- shortcut for ``feature.feature_name``.
    * :py:attr:`doc` -- the :py:class:`~flm.flmdocument.FLMDocument` this
      manager belongs to.
    * :py:attr:`RenderManager` -- the render-manager class taken from the
      feature.
    """

    def __init__(self, feature, doc, **kwargs):
        super().__init__(**kwargs)
        self.feature = feature
        self.feature_name = self.feature.feature_name
        self.doc = doc
        self.RenderManager = self.feature.RenderManager

    def initialize(self) -> None:
        r"""
        Called to initialize the document manager after construction.

        Subclasses should override this method instead of ``__init__`` to
        perform any per-document setup.
        """
        pass

    def get_node_id(self, node) -> TypeNodeId:
        r"""
        Return a unique hashable identifier for *node*.

        Delegates to :py:meth:`Feature.get_node_id`.  The returned value
        can be used as a dictionary key to associate data with a specific
        node instance.

        :param node: A :py:class:`~pylatexenc.latexnodes.nodes.LatexNode`
            instance, or a tuple of hashable data used as an explicit key.
        :returns: An integer (``id(node)``) or the tuple itself.
        """
        return self.feature.get_node_id(node)


class FeatureRenderManagerBase:
    r"""
    Per-render manager for a feature.

    A new :py:class:`FeatureRenderManagerBase` is created each time a
    :py:class:`~flm.flmdocument.FLMDocument` is rendered.  It stores state
    that is specific to a single rendering pass (for example, the mapping
    from nodes to assigned equation or section numbers).

    **Lifecycle** (driven by :py:meth:`~flm.flmdocument.FLMDocument.render`):

    1. The constructor is called automatically; subclasses should **not**
       override it.
    2. :py:meth:`initialize` is called.  Any ``feature_render_options``
       supplied to :py:meth:`~flm.flmdocument.FLMDocument.render` for this
       feature are forwarded as keyword arguments.
    3. The first rendering pass runs (the document render callback executes,
       nodes call ``render()`` or ``prepare_delayed_render()``).
    4. :py:meth:`process` is called with the first-pass output, before
       delayed-render nodes are resolved.
    5. Delayed-render nodes are rendered with full document context.
    6. :py:meth:`postprocess` is called with the final rendered output.

    Attributes set automatically:

    * :py:attr:`feature_document_manager` -- the owning
      :py:class:`FeatureDocumentManagerBase`.
    * :py:attr:`feature` -- shortcut for
      ``feature_document_manager.feature``.
    * :py:attr:`feature_name` -- shortcut for ``feature.feature_name``.
    * :py:attr:`render_context` -- the active
      :py:class:`~flm.flmrendercontext.FLMRenderContext`.
    """

    def __init__(self, feature_document_manager, render_context, **kwargs):
        super().__init__(**kwargs)
        self.feature_document_manager = feature_document_manager
        self.feature = self.feature_document_manager.feature
        self.feature_name = self.feature.feature_name
        self.render_context = render_context

    def initialize(self) -> None:
        r"""
        Initialize the render manager after construction.

        Subclasses should override this method instead of ``__init__``.
        Any ``feature_render_options`` for this feature that were passed to
        :py:meth:`~flm.flmdocument.FLMDocument.render` are forwarded here
        as keyword arguments.
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
        Return a unique hashable identifier for *node*.

        Delegates to :py:meth:`Feature.get_node_id`.  The returned value
        can be used as a dictionary key to associate data with a specific
        node instance.

        :param node: A :py:class:`~pylatexenc.latexnodes.nodes.LatexNode`
            instance, or a tuple of hashable data used as an explicit key.
        :returns: An integer (``id(node)``) or the tuple itself.
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

    DocumentManager : Type[FeatureDocumentManagerBase]|None = FeatureDocumentManagerBase
    r"""
    The document manager class to use for this feature.  Instances of this class
    will automatically be created when a new :py:class:`flmdocument.FLMDocument` is
    instantiated.  This class is expected to be a subclass of
    :py:class:`flm.feature.FeatureDocumentManagerBase`.
    Alternatively, set this class attribute to `None` in your feature subclass
    to indicate that this feature does not need any document manager instance.
    """

    # ---

    RenderManager : Type[FeatureRenderManagerBase]|None = FeatureRenderManagerBase
    r"""
    The render manager class to use for this feature.  Instances of this class
    will automatically be created when rendering a :py:class:`flmdocument.FLMDocument`
    (see :py:meth:`flm.flmdocument.FLMDocument.render()`, and more specifically
    :py:meth:`flm.flmdocument.FLMDocument.make_render_context()`).
    This class is expected to be a subclass of
    :py:class:`flm.feature.FeatureRenderManagerBase`.
    Alternatively, set this class attribute to `None` in your feature subclass
    to indicate that this feature does not need any document manager instance.
    """

    # ---

    feature_title : str|None = None
    r"""
    Descriptive name or title for this feature.
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



