import os.path

import logging
logger = logging.getLogger(__name__)

from ..configmerger import ConfigMerger
from ..template import DocumentTemplate

from ._base import RenderWorkflow

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

        template_info = (
            self.main_config['llm'].get('template', {})
            .get(self.llm_run_info['fragment_renderer_name'], None)
        )

        if not template_info:
            return rendered_content

        template_name = template_info.get('name', None)
        template_config = template_info.get('config', {})

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
                'style': fr_style_information,
            }
        ])

        logger.debug(f"About to load template ‘%s’ (prefix ‘%s’), config is = %r",
                     template_name, template_prefix, template_config_wdefaults)

        template = DocumentTemplate(template_name,
                                    template_prefix,
                                    template_config_wdefaults,
                                    self.llm_run_info)


        metadata = document.metadata
        if metadata is None:
            metadata = {}
        else:
            metadata = {k: v for (k, v) in metadata.items() if k != "_config_llm"}

        rendered_template = template.render_template([
            {
                'content': rendered_content,
                'metadata': metadata,
            },
        ])

        return rendered_template


# ------------------------------------------------


RenderWorkflowClass = TemplateBasedRenderWorkflow
