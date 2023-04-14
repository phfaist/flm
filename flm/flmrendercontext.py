
import logging
logger = logging.getLogger(__name__)



class FLMRenderContext:

    is_standalone_mode = False

    is_first_pass = True

    def __init__(self, fragment_renderer, *, doc=None, **kwargs):
        super().__init__(**kwargs)
        self.doc = doc
        self.fragment_renderer = fragment_renderer
        self.pass_name = None
        self.is_first_pass = True
        self._logical_state = {}
        
        self._nodes_determined_as_delayed = {}

    def supports_feature(self, feature_name):
        return False

    def feature_render_manager(self, feature_name):
        return None

    def register_delayed_render(self, node, fragment_renderer):
        raise RuntimeError("This render context does not support delayed rendering")

    def get_delayed_render_content(self, node):
        raise RuntimeError("This render context does not support delayed rendering")

    def get_is_delayed_render(self, node):
        if node.node_id in self._nodes_determined_as_delayed:
            return self._nodes_determined_as_delayed[node.node_id]

        yn = node.flm_specinfo.delayed_render
        if callable(yn):
            yn = yn(node, render_context)
            self._nodes_determined_as_delayed[node.node_id] = yn

        return yn

    def set_render_pass(self, pass_name):
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


