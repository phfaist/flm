
from ...fragmentrenderer._base import FragmentRenderer
from ...flmrecomposer.purelatex import FLMPureLatexRecomposer

from ._base import RenderWorkflow
from .templatebasedworkflow import TemplateBasedRenderWorkflow



_default_config = {
    'recomposer_options': {},
}



class FlmLatexWorkflow(TemplateBasedRenderWorkflow):

    @staticmethod
    def get_workflow_default_config(flm_run_info, config):
        return _default_config

    @staticmethod
    def get_fragment_renderer_name(outputformat, flm_run_info, run_config):

        if not outputformat or outputformat == 'latex':
            # Use 'latex' fragment renderer.  It will provide some useful
            # methods for rendering parts of the latex, for instance things to
            # do with graphics or maybe even cells
            return 'latex'

        raise ValueError(f"Unsupported format: {outputformat}")


    def get_wstyle_information(self):
        return {
            'preamble_suggested_defs': _latex_wstyle_suggested_preamble_defs,
        }

    def render_document(self, document, content_parts_infos, **kwargs):

        render_context = document.make_render_context(
            fragment_renderer=self.fragment_renderer # a LatexFragmentRenderer
        )

        recomposer = FLMPureLatexRecomposer(
            dict(self.recomposer_options,
                 render_context=render_context)
        )

        recomposed_result = \
            recomposer.recompose_pure_latex(document.document_fragments[0].nodes)

        latex = recomposed_result['latex']

        # render individual parts, if applicable:

        doc_parts = content_parts_infos.get('parts', None)
        if not doc_parts: doc_parts = []
        for doc_part_info in doc_parts:

            fragment_part = doc_part_info['fragment']

            recomposed_result_part = recomposer.recompose_pure_latex(fragment_part.nodes)

            latex_part = recomposed_result_part['latex']
            latex += latex_part

        packages = recomposed_result['packages']

        # Call resource manager process/postprocess methods in case there is any
        # post-processing that is needed to be done (e.g., collect graphics)

        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_render_manager.process(latex)

        for feature_name, feature_render_manager in render_context.feature_render_managers:
            if feature_render_manager is not None:
                feature_render_manager.postprocess(latex)

        # Render the template.

        rendered_content = latex
        
        flmlatex_preamble_packages = ''
        for pname, pinfo in packages.items():
            flmlatex_preamble_packages += r'\usepackage'
            if pinfo['options']:
                flmlatex_preamble_packages += '[' + pinfo['options'] + ']'
            flmlatex_preamble_packages += r'{' + pname + r'}' + '\n'

        final_content = self.render_templated_document(
            rendered_content, document, render_context,
            add_context={
                'flmlatex_preamble_packages': flmlatex_preamble_packages
            }
        )

        return final_content


# --------

_latex_wstyle_suggested_preamble_defs = r"""
\makeatletter
\newif\ifdeftermShowTerm
\deftermShowTermfalse
\def\defterm#1{%
  \begingroup
  \edef\flmL@cur@defterm{\detokenize{#1}}%
  \par\vspace{\abovedisplayskip}%
  \flmDeftermFormat
  \phantomsection
  \hypertarget{term:\flmL@cur@defterm}{}\relax
  \ifdeftermShowTerm \flmDisplayTerm{#1: }\fi
}
\def\enddefterm{%
  \par
  \vspace{\belowdisplayskip}%
  \endgroup
}
\def\term{\@ifnextchar[\term@o\term@a}%]
\def\term@a#1{\term@o[{#1}]{#1}}
\def\term@o[#1]#2{%
  \edef\flmL@tmp@a{\detokenize{#1}}%
  \ifx\flmL@tmp@a\flmL@cur@defterm%
    \termDisplayInDefterm{#2}%
  \else
    \hyperlink{term:\flmL@tmp@a}{%
      \termDisplay{#2}%
    }%
  \fi
}
\def\termDisplayInDefterm#1{%
  \textbf{\textit{#1}}%
}
\def\termDisplay#1{%
  #1%
}
\def\flmFloat#1#2{%
  \edef\flmFloat@curfloatenv{#1}%
  \edef\x{%
    \noexpand\begin{#1}\csname flmFloatPlacementArgs#2\endcsname}%
  \x
  \centering
}
\def\endflmFloat{%
  \expandafter\end\expandafter{\flmFloat@curfloatenv}%
}
\def\flmFloatPlacementArgsNumCap{[tbph]}
\def\flmFloatPlacementArgsNumOnly{[tbph]}
\def\flmFloatPlacementArgsCapOnly{[h]}
\def\flmFloatPlacementArgsBare{[h]}
\makeatother
"""


# --------


RenderWorkflowClass = FlmLatexWorkflow
