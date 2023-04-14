import logging
logger = logging.getLogger(__name__)

from .._util import abbrev_value_str


class RenderWorkflow:

    binary_output = False

    @staticmethod
    def get_workflow_default_config(flm_run_info, config):
        return {}


    @staticmethod
    def get_fragment_renderer_name(outputformat, flm_run_info, run_config):
        r"""
        The workflow has a say about which fragment renderer class will be
        used.  Return the fragment render name here, where the fragment renderer
        information will be queried as if it were specified by the output
        format.  Normally outputformat should be sufficient.
        """
        return None

    @staticmethod
    def get_default_main_config(flm_run_info, run_config):
        return None



    # ---


    def __init__(self, workflow_config, flm_run_info,
                 fragment_renderer_information, fragment_renderer):

        self.config = workflow_config
        self.flm_run_info = flm_run_info
        self.fragment_renderer_information = fragment_renderer_information
        self.fragment_renderer = fragment_renderer

        self.main_config = self.flm_run_info['main_config']

        for k, v in self.config.items():
            setattr(self, k, v)

        logger.debug("Initialized workflow ‘%s’ with config %s", self.__class__.__name__,
                     abbrev_value_str(workflow_config, maxstrlen=512))


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
            ], render_context)

        return rendered_result


    def postprocess_rendered_document(self, rendered_content, document, render_context):
        return rendered_content


