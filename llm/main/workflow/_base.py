
class RenderWorkflow:

    binary_output = False

    def __init__(self, config, llm_run_info, fragment_renderer_class=None):

        self.config = config
        self.llm_run_info = llm_run_info

        if fragment_renderer_class is not None:
            self.fragment_renderer_class = fragment_renderer_class

        for k, v in self.config.items():
            setattr(self, k, v)

    def get_fragment_renderer_class(self):
        return self.fragment_renderer_class

    def render_document(self, document, fragment_renderer):

        rendered_content, render_context = \
            self.render_document_fragments(document, fragment_renderer)

        final_content = self.postprocess_rendered_document(
            rendered_content, document, render_context
        )

        return final_content

    def render_document_fragments(self, document, fragment_renderer):

        # Render the main document
        rendered_result, render_context = document.render(fragment_renderer)

        # # Render endnotes
        # if render_context.supports_feature('endnotes'):
        #     endnotes_mgr = render_context.feature_render_manager('endnotes')
        #     endnotes_result = endnotes_mgr.render_endnotes()
        #     rendered_result = fragment_renderer.render_join_blocks([
        #         rendered_result,
        #         endnotes_result,
        #     ])

        return rendered_result, render_context

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        return rendered_content


