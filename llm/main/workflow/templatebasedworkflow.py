import os.path

from ._base import RenderWorkflow
from ..configmerger import ConfigMerger


# ------------------------------------------------------------------------------

_default_config = {
    'template': None,
    'template_config': {}
}


class TemplateBasedRenderWorkflow(RenderWorkflow):

    @staticmethod
    def get_workflow_default_config(llm_run_info, config):
        return _default_config

    @staticmethod
    def get_default_fragment_renderer(llm_run_info, run_config):
        return 'html'

    # ---

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        template_info = template_name.get(llm_run_info['fragment_renderer_name'], None)

        if not template_info:
            return rendered_content

        template_name = template_info['name']
        template_config = template_info['config']

        if not template_name:
            return rendered_content

        # get any relevant style information for this fragment renderer & format
        frinfo = self.fragment_renderer_information
        fr_style_information = {}
        if hasattr(frinfo, 'get_style_information'):
            fr_style_information = frinfo.get_style_information(self.fragment_renderer)

        template_prefix = self.config.get('template_prefix', None)
        if template_prefix is None and hasattr(frinfo, 'format_name'):
            template_prefix = frinfo.format_name
        

        template_config_wdefaults = ConfigMerger().recursive_assign_defaults([
            template_config,
            {
                'template_prefix': template_prefix,
                'style': fr_style_information,
            }
        ])

        template = DocumentTemplate(template_name,
                                    template_config_wdefaults,
                                    self.llm_run_info)

        rendered_template = template.render_template(document, {'content': rendered_content})

        return rendered_template


# ------------------------------------------------


RenderWorkflowClass = TemplateBasedRenderWorkflow
