r"""
Render context classes for FLM rendering.

The render context carries state during the rendering of an FLM document or
standalone fragment.  It tracks which features are available, manages
delayed rendering for forward references, and provides a logical-state
mechanism for context-dependent rendering (e.g., nested enumerations).

See also :py:class:`~flm.flmdocument.FLMDocumentRenderContext` for the
document-mode subclass that supports full feature managers and delayed
rendering.
"""

import logging
logger = logging.getLogger(__name__)

from ._typing_helpers import Hashable, Mapping, TypeNodeId, TypeFLMDocument, TypeFragmentRenderer
from .feature._base import FeatureRenderManagerBase


class FLMRenderContext:
    r"""
    Base class for render contexts.

    A render context is passed to every node's :py:meth:`render()` method
    and carries:

    - The :py:attr:`fragment_renderer` producing the output.
    - Feature support queries (:py:meth:`supports_feature`,
      :py:meth:`feature_render_manager`).
    - Delayed rendering registration (for forward references).
    - A logical state mechanism for context-dependent rendering.

    The class attributes ``is_standalone_mode`` and ``is_first_pass``
    indicate whether we are in standalone mode and whether this is the
    first rendering pass, respectively.
    """

    is_standalone_mode : bool = False

    is_first_pass : bool = True

    def __init__(
            self,
            fragment_renderer : TypeFragmentRenderer,
            *,
            doc : TypeFLMDocument|None = None,
            **kwargs
        ):
        r"""
        Initialize the render context.

        :param fragment_renderer: The
            :py:class:`~flm.fragmentrenderer.FragmentRenderer` instance
            that produces the output in a specific format (HTML, text, etc.).
        :param doc: The :py:class:`~flm.flmdocument.FLMDocument` instance
            this render context belongs to, or ``None`` for standalone mode.
        """
        super().__init__(**kwargs)
        self.doc = doc
        self.fragment_renderer = fragment_renderer
        self.pass_name = None
        self.is_first_pass = True
        self._logical_state = {}

        self._nodes_determined_as_delayed = {}

    # for python typing hints
    _flmtyping_is = 'FLMRenderContext'

    doc : TypeFLMDocument|None = None
    fragment_renderer : TypeFragmentRenderer = None
    pass_name : str|None = None
    _nodes_determined_as_delayed : dict[TypeNodeId,bool] = {}

    def supports_feature(self, feature_name) -> bool:
        r"""Return ``True`` if the given feature is active in this render context."""
        return False

    def feature_render_manager(self, feature_name) -> FeatureRenderManagerBase|None:
        r"""Return the render manager for the given feature, or ``None``."""
        return None

    def register_delayed_render(self, node, fragment_renderer) -> Hashable:
        r"""Register a node for delayed rendering.  Returns a key for later retrieval."""
        raise RuntimeError("This render context does not support delayed rendering")

    def get_delayed_render_content(self, node) -> str:
        r"""Retrieve the rendered content for a delayed-render node."""
        raise RuntimeError("This render context does not support delayed rendering")

    def get_is_delayed_render(self, node) -> bool:
        r"""
        Determine whether *node* should use delayed rendering.

        If the node's :py:attr:`~flm.flmspecinfo.FLMSpecInfo.delayed_render`
        attribute is a callable, it is invoked with ``(node, self)`` and the
        boolean result is cached by node ID so the callable is called at most
        once per node.  If the attribute is a plain boolean, it is returned
        directly.

        :param node: A parsed LaTeX node with ``flm_specinfo`` set.
        :returns: ``True`` if the node requires delayed rendering.
        """
        if node._flm_node_id in self._nodes_determined_as_delayed:
            return self._nodes_determined_as_delayed[node._flm_node_id]

        yn = node.flm_specinfo.delayed_render
        if callable(yn):
            yn = bool( yn(node, self) )
            self._nodes_determined_as_delayed[node._flm_node_id] = yn
        else:
            yn = bool(yn)

        return yn

    def set_render_pass(self, pass_name : str|None) -> None:
        r"""
        Set the current rendering pass.

        The rendering pipeline may perform multiple passes (e.g., a first pass
        followed by a ``'second-pass'`` to resolve delayed content).  This
        method updates :py:attr:`pass_name` and sets :py:attr:`is_first_pass`
        to ``True`` when *pass_name* is ``None``, or ``False`` otherwise.

        :param pass_name: The name of the rendering pass (e.g.,
            ``'second-pass'``), or ``None`` to reset to the first pass.
        """
        self.pass_name = pass_name
        if pass_name is None:
            self.is_first_pass = True
        else:
            self.is_first_pass = False

    def get_logical_state(self, domainname):
        r"""
        Certain LaTeX nodes might render differently depending on where they are
        placed in.  Render contexts provide a minimal 'state function' mechanism
        that enables nodes to track how to render.  E.g., an enumeration
        environment will update the state when rendering content nodes to
        correctly render nested enumerations.

        The full logical state is organized into 'domains', e.g., enumeration
        environments will store any state in a domain called
        'feature.enumeration' to avoid interference with other possible features
        etc.

        In each domain, the logical state is a dictionary.  The render methods
        of nodes can set/remove keys as they see fit.
        """

        if domainname not in self._logical_state:
            self._logical_state[domainname] = {}

        return self._logical_state[domainname]
        
    def push_logical_state(self, domainname, propname, value):
        r"""
        Use as context manager::

            with render_context.push_logical_state('myfeature', 'depth', depth+1):
               # ...

        FIXME: SHOULDN'T THIS BE IMPLEMENTED BY A DIFFERENT "SUB-RENDER-CONTEXT"
        OBJECT WHICH IS THEN PASSED TO OTHER CALLS TO RENDER METHODS?
        ..............................
        """
        return _RenderContextPushLogicalState(self, domainname, propname, value)

        
class _RenderContextPushLogicalState:
    def __init__(self, render_context, domainname, propname, value):
        super().__init__()
        self.render_context = render_context
        self.domainname = domainname
        self.propname = propname
        self.value = value
        
    def __enter__(self):
        state = self.render_context.get_logical_state(self.domainname)
        if self.propname in state:
            self.old_value_exists = True
            self.old_value = state[self.propname]
        else:
            self.old_value_exists = False
            self.old_value = None

        # change the state
        state[self.propname] = self.value

    def __exit__(self, exc_type, exc_value, traceback):
        state = self.render_context.get_logical_state(self.domainname)
        if not self.old_value_exists:
            state.pop(self.propname)
        else:
            state[self.propname] = self.old_value

        
        


class FLMStandaloneModeRenderContext(FLMRenderContext):
    r"""
    A render context for standalone-mode rendering.

    This context does not support features, delayed rendering, or document
    managers.  It is used by
    :py:meth:`~flm.flmfragment.FLMFragment.render_standalone`.
    """

    is_standalone_mode = True

    def __init__(self, fragment_renderer):
        super().__init__(fragment_renderer=fragment_renderer)

    def supports_feature(self, feature_name):
        return False

    def feature_render_manager(self, feature_name):
        raise ValueError(
            f"There are no document features when rendering FLM text in "
            f"standalone mode (reqested ‘{feature_name}’)"
        )

    def register_delayed_render(self, node, fragment_renderer):
        raise ValueError(
            f"Cannot render nodes with delayed content in standalone mode"
        )

    def get_delayed_render_content(self, node):
        raise ValueError(
            f"There's no delayed render content in standalone mode"
        )


