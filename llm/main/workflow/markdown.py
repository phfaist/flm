from ._base import RenderWorkflow

from llm.fragmentrenderer.markdown import MarkdownFragmentRenderer

class MarkdownRenderWorkflow(RenderWorkflow):
    fragment_renderer_class = MarkdownFragmentRenderer

RenderWorkflowClass = MarkdownRenderWorkflow
