from collections.abc import Mapping

import logging
logger = logging.getLogger(__name__)

from ..configmerger import ConfigMerger
from ..template import DocumentTemplate
from .._util import abbrev_value_str

from ._base import RenderWorkflow

# ------------------------------------------------------------------------------

_default_config = {
    'template': None,
    'template_config': {}
}


class TemplateBasedRenderWorkflow(RenderWorkflow):
    r"""
    This workflow works by simply reimplementing
    postprocess_rendered_document() to include the document contents in a
    suitable template (defined by the run info, output format, and relevant
    config items).

    This class offers the method `render_templated_document()`, for the use of
    any other workflows or scripts can render documents based on templates in a
    similar way as a subroutine (e.g. to render PDF based on templated HTML
    output).
    """

    @staticmethod
    def get_workflow_default_config(flm_run_info, config):
        return _default_config

    @staticmethod
    def get_fragment_renderer_name(outputformat, flm_run_info, run_config):

        if outputformat == 'pdf':
            raise ValueError(
                "The default workflow ‘templatebasedworkflow’ cannot generate PDF. "
                "Try instead the options '-w runlatexpdf -f pdf'."
            )

        return outputformat or 'html'

    # ---

    use_output_format_name = None

    template_config_workflow_defaults = {}

    def render_templated_document(self, rendered_content, document, render_context):
        r"""
        Take the raw rendered document content `rendered_content` and place
        it in a document based on a suitable template.  The argument `document`
        is the `FLMDocument` instance that was rendered with the
        render context `render_context`.
        """

        use_output_format_name = \
            self.use_output_format_name or self.flm_run_info['fragment_renderer_name']

        use_template_name = None
        if self.flm_run_info.get('template', None) is not None:
            use_template_name = self.flm_run_info['template']
        if use_template_name is None:
            use_template_name = self.main_config['flm'].get('template', {})
            if isinstance(use_template_name, Mapping):
                use_template_name = use_template_name.get(use_output_format_name, None)

        logger.debug("Template: using output format ‘%s’ and template name ‘%s’",
                     use_output_format_name, use_template_name)

        if use_template_name is None:
            # no template specified
            logger.debug("No template specified, returning raw content")
            return rendered_content

        template_config = self.main_config['flm'].get('template_config', {}) \
            .get(use_output_format_name, {}).get(use_template_name, {})

        if not use_template_name:
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
            self.template_config_workflow_defaults,
            {
                'style': fr_style_information,
            }
        ])

        logger.debug(f"About to load template ‘%s’ (prefix ‘%s’), config is = %s",
                     use_template_name, template_prefix,
                     abbrev_value_str(template_config_wdefaults))

        template = DocumentTemplate(use_template_name,
                                    template_prefix,
                                    template_config_wdefaults,
                                    self.flm_run_info)

        metadata = document.metadata
        if metadata is None:
            metadata = {}
        else:
            metadata = {k: v for (k, v) in metadata.items() if k != "_flm_config"}

        rendered_template = template.render_template([
            {
                'content': rendered_content,
                'metadata': metadata,
            },
        ])

        return rendered_template


    # ---

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        return self.render_templated_document(rendered_content, document, render_context)



# ------------------------------------------------


RenderWorkflowClass = TemplateBasedRenderWorkflow
