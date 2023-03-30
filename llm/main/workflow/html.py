

raise DontUseThis!!!!!!!!!!



import os.path

from ._base import RenderWorkflow
from ..configmerger import ConfigMerger

from llm.fragmentrenderer.html import (
    HtmlFragmentRenderer,
    get_html_css_global, get_html_css_content, get_html_js_local,
    get_html_body_end_js_scripts
)


# ------------------------------------------------------------------------------

_default_config = {
    'template': None,
    'template_config': {}
}


class HtmlRenderWorkflow(RenderWorkflow):

    workflow_default_config = _default_config

    fragment_renderer_class = HtmlFragmentRenderer

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        if not self.config['template']:
            return rendered_content

        template_name = self.config['template']
        template_config = self.config['template_config']

        template_config_wdefaults = ConfigMerger().recursive_assign_defaults([
            template_config,
            {
                'html': {
                    'css_global': get_html_css_global(render_context.fragment_renderer),
                    'css_content': get_html_css_content(render_context.fragment_renderer),
                    'js_local': get_html_js_local(render_context.fragment_renderer),
                    'body_end_js_scripts':
                        get_html_body_end_js_scripts(render_context.fragment_renderer),
                },
                'html_content': 'MISSING CONTENT!'
            }
        ])

        template = DocumentTemplate(template_name,
                                    template_config_wdefaults,
                                    self.llm_run_info)

        return template.render_template(rendered_content, document)




RenderWorkflowClass = HtmlRenderWorkflow
