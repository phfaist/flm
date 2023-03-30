

raise DontUseThis!!!!!!!!!!




from ._base import RenderWorkflow

from llm.fragmentrenderer.text import TextFragmentRenderer

class TextRenderWorkflow(RenderWorkflow):
    fragment_renderer_class = TextFragmentRenderer

RenderWorkflowClass = TextRenderWorkflow
