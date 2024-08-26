
from ...fragmentrenderer._base import FragmentRenderer
from ...flmrecomposer.purelatex import FLMPureLatexRecomposer

from ._base import RenderWorkflow
from .templatebasedworkflow import TemplateBasedRenderWorkflow



_default_config = {
    'recomposer_options': {},
    'skip_packages': [],
    'add_bibliography': False,
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

    def recompose_fragment(self, recomposer, fragment):
        
        recomposed_result = recomposer.recompose_pure_latex(fragment.nodes)

        recomposed_result["latex"] += "%%\n"

        return recomposed_result

    def render_document(self, document, content_parts_infos, **kwargs):

        render_context = document.make_render_context(
            fragment_renderer=self.fragment_renderer # a LatexFragmentRenderer
        )

        recomposer = FLMPureLatexRecomposer(
            dict(self.recomposer_options,
                 render_context=render_context)
        )

        recomposed_result = self.recompose_fragment(
            recomposer,
            document.document_fragments[0]
        )

        latex = recomposed_result['latex']

        # render individual parts, if applicable:

        doc_parts = content_parts_infos.get('parts', None)
        if not doc_parts: doc_parts = []
        for doc_part_info in doc_parts:

            fragment_part = doc_part_info['fragment']

            recomposed_result_part = self.recompose_fragment(
                recomposer,
                fragment_part
            )

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
            if pname in self.skip_packages:
                continue
            flmlatex_preamble_packages += r'\usepackage'
            if pinfo['options']:
                flmlatex_preamble_packages += '[' + pinfo['options'] + ']'
            flmlatex_preamble_packages += r'{' + pname + r'}' + '\n'

        flmlatex_add_bibliography = None
        if self.add_bibliography:
            flmlatex_add_bibliography = self.add_bibliography

        final_content = self.render_templated_document(
            rendered_content, document, render_context,
            add_context={
                'flmlatex_preamble_packages': flmlatex_preamble_packages,
                'flmlatex_add_bibliography': flmlatex_add_bibliography,
            }
        )

        return final_content


# --------

_latex_wstyle_suggested_preamble_defs = r"""
\makeatletter

\providecommand\flmFinalPreambleSetup{}

% For references -- pin down custom labels wherever we want for \cref
\NewDocumentCommand{\flmL@crefCustomLabel}{O{flmLCustomLabel}m+m}{%
  \begingroup
    \cref@constructprefix{#1}{\cref@result}%
    \protected@edef\@currentlabel{%
      #3}%
    \protected@edef\@currentlabelname{#3}%
    \protected@edef\cref@currentlabel{%
      [#1][][\cref@result]%
      #3%
    }%
    \flmL@cref@label[#1]{#2}%
  \endgroup
}
\g@addto@macro\flmFinalPreambleSetup{%
  \ifcsname crefname\noexpand\endcsname
    \crefname{flmLCustomLabel}{}{}%
    \Crefname{flmLCustomLabel}{}{}%
  \fi}
\newcommand\flmLDefLabelText[2]{%
  \begingroup
    \let\flmL@cref@label\label
    \def\label##1{%
      \flmL@crefCustomLabel{##1}{#1}%
    }%
    #2%
  \endgroup
}


% For defterms -- 

\newif\ifdeftermShowTerm
\deftermShowTermfalse
\def\defterm#1\label#2{%
  \begingroup
  \par\vspace{\abovedisplayskip}%
  \flmDeftermFormat
  \phantomsection
  \label{#2}%
  \edef\flmL@cur@defterm@label{\flmL@cur@defterm@label,#2,}%
  \ifdeftermShowTerm \flmDisplayTerm{#1: }\fi
}
\def\flmL@cur@defterm@label{}
\def\enddefterm{%
  \par
  \vspace{\belowdisplayskip}%
  \endgroup
}
\def\flmTerm#1#2#3#4{%
  \edef\x{\noexpand\in@{,#2,}{\flmL@cur@defterm@label}}%
  \x\ifin@
    \termDisplayInDefterm{#4}%
  \else
    \flmDisplayTermRef{#2}{#4}%
  \fi
}
\robustify\flmTerm
\def\flmDisplayTermRef#1#2{%
  \hyperref[#1]{\termDisplay{#2}}%
}
\def\termDisplayInDefterm#1{%
  \textbf{\textit{#1}}%
}
\def\termDisplay#1{%
  #1%
}
\def\flmFloat#1#2{%
  \edef\flmFloat@curfloatenv{#1}%
  \edef\flmFloat@usefloatenv{#1}%
  \edef\flmFloat@useenvargs{\csname flmFloatPlacementArgs#2\endcsname}%
  \ifcsname flmFloatSetUseEnv#1\endcsname
    \csname flmFloatSetUseEnv#1\endcsname
  \fi
  \ifcsname flmFloatSetUseConfig#2\endcsname
    \csname flmFloatSetUseConfig#2\endcsname
  \fi
  \edef\x{%
    \noexpand\begin{\flmFloat@usefloatenv}\expandonce\flmFloat@useenvargs}%
  \x
  \centering
}
\def\endflmFloat{%
  \expandafter\end\expandafter{\flmFloat@usefloatenv}%
}
\def\flmFloatPlacementArgsNumCap{[tbph]}
\def\flmFloatPlacementArgsNumOnly{[tbph]}
\def\flmFloatPlacementArgsCapOnly{[h]}
\def\flmFloatPlacementArgsBare{[h]}
\def\flmFloatSetUseConfigBare{%
  \edef\flmFloat@usefloatenv{center}%
  \def\flmFloat@useenvargs{%
    \edef\@captype{\flmFloat@curfloatenv}%
  }%
}
\let\flmFloatSetUseConfigCapOnly\flmFloatSetUseConfigBare
\makeatother
"""


# --------


RenderWorkflowClass = FlmLatexWorkflow
