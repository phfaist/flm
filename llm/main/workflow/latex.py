from ._base import RenderWorkflow, MinimalDocumentPostprocessor

from llm.fragmentrenderer.html import HtmlFragmentRenderer


# ------------------------------------------------------------------------------

_latex_minimal_document_pre = r"""\documentclass[11pt]{article}
\usepackage{phfnote}
\newenvironment{defterm}{%
  \par\begingroup\itshape
}{%
  \endgroup\par
}
\newcommand{\displayterm}[1]{\textbf{#1}}
\begin{document}
"""

_latex_minimal_document_post = r"""%
\end{document}
"""

class LatexMinimalDocumentPostprocessor(MinimalDocumentPostprocessor):

    doc_pre_post = (_latex_minimal_document_pre, _latex_minimal_document_post)

    def postprocess(self, rendered_content):
        doc_pre, doc_post = self.doc_pre_post
        return ''.join([doc_pre, rendered_content, doc_post])


# ------------------------------------------------------------------------------


class LatexRenderWorkflow(RenderWorkflow):

    fragment_renderer_class = LatexFragmentRenderer

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        if not self.minimal_document:
            return rendered_content

        pp = LatexMinimalDocumentPostprocessor(document, render_context, self.config)
        return pp.postprocess(rendered_content)


RenderWorkflowClass = LatexRenderWorkflow
