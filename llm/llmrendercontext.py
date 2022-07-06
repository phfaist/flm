
import logging
logger = logging.getLogger(__name__)



class LLMRenderContext:

    is_standalone_mode = False

    def __init__(self, fragment_renderer, *, doc=None, **kwargs):
        super().__init__(**kwargs)
        self.doc = doc
        self.fragment_renderer = fragment_renderer

    def supports_feature(self, feature_name):
        return False

    def feature_render_manager(self, feature_name):
        return None

    def register_delayed_render(self, node, fragment_renderer):
        raise RuntimeError("This render context does not support delayed rendering")

    def get_delayed_render_content(self, node):
        raise RuntimeError("This render context does not support delayed rendering")



class LLMStandaloneModeRenderContext(LLMRenderContext):

    is_standalone_mode = True

    def __init__(self, fragment_renderer):
        super().__init__(fragment_renderer=fragment_renderer)

    def supports_feature(self, feature_name):
        return False

    def feature_render_manager(self, feature_name):
        raise ValueError(
            f"There are no document features when rendering LLM text in "
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


