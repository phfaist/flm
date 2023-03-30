
class RenderWorkflow:

    binary_output = False

    @staticmethod
    def get_workflow_default_config(llm_run_info, config):
        return {}


    @staticmethod
    def get_default_fragment_renderer(llm_run_info, run_config):
        return None


    def __init__(self, workflow_config, main_config, llm_run_info,
                 fragment_renderer_information, fragment_renderer):

        self.config = workflow_config
        self.main_config = main_config
        self.llm_run_info = llm_run_info
        self.fragment_renderer_information = fragment_renderer_information
        self.fragment_renderer = fragment_renderer

        for k, v in self.config.items():
            setattr(self, k, v)


    def render_document(self, document):

        rendered_content, render_context = self.render_document_fragments(document)

        final_content = self.postprocess_rendered_document(
            rendered_content, document, render_context
        )

        return final_content


    def render_document_fragments(self, document):

        # Render the main document
        rendered_result, render_context = document.render(self.fragment_renderer)

        return rendered_result, render_context


    def render_document_fragment_callback(self, fragment, render_context):

        rendered_result = fragment.render(render_context)

        # Render endnotes
        if ( getattr(self, 'render_endnotes', True)
             and render_context.supports_feature('endnotes') ):
            endnotes_mgr = render_context.feature_render_manager('endnotes')
            endnotes_result = endnotes_mgr.render_endnotes()
            rendered_result = render_context.fragment_renderer.render_join_blocks([
                rendered_result,
                endnotes_result,
            ])

        return rendered_result


    def postprocess_rendered_document(self, rendered_content, document, render_context):
        return rendered_content


