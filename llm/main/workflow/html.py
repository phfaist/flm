
import os.path

from ._base import RenderWorkflow
from ..configmerger import ConfigMerger

from llm.fragmentrenderer.html import (
    HtmlFragmentRenderer,
    get_html_css_global, get_html_css_content, get_html_js_local,
    get_html_body_end_js_scripts
)



# ------------------------------------------------------------------------------



# class HtmlMinimalDocumentPostprocessor(MinimalDocumentPostprocessor):

#     def postprocess(self, rendered_content):

#         config = self.config

#         logger.debug("html minimal document post-processor, config is %r", config)

#         metadata = self.document.metadata
#         if metadata is None:
#             metadata = {}
#         else:
#             metadata = {k: v for (k, v) in metadata.items() if k != "config"}

#         logger.debug("html minimal document post-processor, metadata is %r", metadata)

#         full_config = ConfigMerger().recursive_assign_defaults([
#             config,
#             {
#                 'metadata': metadata,
#             },
#             {
#                 'render_header': True,
#                 'metadata': { 'title': "", 'author': "", 'date': "" },
#                 'html': { 'extra_css': '', 'extra_js': '' },
#                 'style': {
#                     'font_family': "Source Serif Pro",
#                     'font_size': "16px",
#                     #'default_font_families': "'Times New Roman', serif",
#                     'default_font_families': "serif",
#                 }
#             },
#         ])

#         logger.debug("html minimal document post-processor, full_config is %r", full_config)

#         full_config_style = full_config.get('style', {})

#         css_global_page = _Template(
#             _html_minimal_document_css_global_page_template 
#         ).substitute(full_config_style)

#         css = (
#             '/* ======== */\n'
#             + css_global_page
#             + html_fragment_renderer.get_html_css_global()
#             + html_fragment_renderer.get_html_css_content()
#             + '/* ======== */\n'
#         )
#         if full_config['html']['extra_css']:
#             css += full_config['html']['extra_css'] + '\n/* ======== */\n'

#         js = html_fragment_renderer.get_html_js()
#         if full_config['html']['extra_js']:
#             js += '\n/* ======== */\n' + full_config['html']['extra_js'] + '\n/* ======== */\n'

#         body_start_content = ""
#         if full_config['render_header']:
#             body_start_content_items = []
#             if full_config['metadata']['title']:
#                 body_start_content_items.append(
#                     f"<h1 class=\"header-title\">{full_config['metadata']['title']}</h1>"
#                 )
#             if full_config['metadata']['author']:
#                 body_start_content_items.append(
#                     f"<div role=\"doc-subtitle\" class=\"header-author\">"
#                     f"{full_config['metadata']['author']}"
#                     f"</div>"
#                 )
#             if full_config['metadata']['date']:
#                 body_start_content_items.append(
#                     f"<div role=\"doc-subtitle\" class=\"header-date\">"
#                     f"{full_config['metadata']['date']}"
#                     f"</div>"
#                 )
#             if body_start_content_items:
#                 body_start_content += (
#                     "<header>" + "".join(body_start_content_items) + "</header>"
#                 )

#         full_config_w_htmltemplate = ConfigMerger().recursive_assign_defaults([
#             full_config,
#             {
#                 'html_template': {
#                     'css': css,
#                     'js': js,
#                     'body_start_content': body_start_content,
#                     'body_end_content': html_fragment_renderer.get_html_body_end_js_scripts(),
#                 },
#             },
#         ])

#         full_config_w_htmltemplate['html_content'] = rendered_content

#         flat_config = _flatten_dict(full_config_w_htmltemplate)

#         return = StrTemplate(_html_minimal_document_template).substitute(
#             flat_config
#         )


# _html_minimal_document_css_global_page_template = r"""
# html, body {
#   font-family: '${font_family}', ${default_font_families};
#   font-size: ${font_size};
#   line-height: 1.3em;
# }

# header, article {
#   max-width: 640px;
#   margin: 0px auto;
# }
# header {
#   padding-bottom: 1em;
#   border-bottom: 1px solid black;
#   margin-bottom: 2em;
# }
# header div[role="doc-subtitle"] {
#   margin-left: 2em;
#   margin-top: 0.5em;
#   font-size: 1.1rem;
#   font-style: italic;
# }
# """

# _html_minimal_document_pre_template = r"""
# <!doctype html>
# <html>
# <head>
# <meta charset="utf-8">
# <title>${metadata.title}</title>
# <style type="text/css">
# /* ------------------ */
# ${html_template.css}
# /* ------------------ */
# </style>
# <script type="text/javascript">
# ${html_template.js}
# </script>
# </head>
# <body>
# ${html_template.body_start_content}
#   <article>
# """.strip()

# _html_minimal_document_post_template = r"""
#   </article>
# ${html_template.body_end_content}
# </body>
# </html>
# """.strip()


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
