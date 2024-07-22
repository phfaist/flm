
from ...fragmentrenderer._base import FragmentRenderer
from ...flmrecomposer.purelatex import FLMPureLatexRecomposer

from ._base import RenderWorkflow



_default_config = {
    'recomposer_options': {},
}


class PureLatexDummyFragmentRenderer(FragmentRenderer):
    pass

class PureLatexDummyFragmentRendererInfo:
    FragmentRendererClass = PureLatexDummyFragmentRenderer


class FlmLatexWorkflow(RenderWorkflow):

    @staticmethod
    def get_workflow_default_config(flm_run_info, config):
        return _default_config

    @staticmethod
    def get_fragment_renderer_name(outputformat, flm_run_info, run_config):

        if not outputformat or outputformat == 'latex':
            return 'flm.main.workflow.flmlatex.PureLatexDummyFragmentRendererInfo'

        raise ValueError(f"Unsupported format: {outputformat}")

    def render_templated_document(self, rendered_content, document, render_context):
        return "<<< render_templated_document() is unused but needs to be present >>>"


    def render_document(self, document):

        render_context = document.make_render_context(fragment_renderer=None)

        recomposer = FLMPureLatexRecomposer(
            dict(self.recomposer_options,
                 render_context=render_context)
        )

        recomposed_result = \
            recomposer.recompose_pure_latex(document.document_fragments[0].nodes)

        latex = recomposed_result['latex']
        packages = recomposed_result['packages']

        # need some template stuff at this point
        s = r'\documentclass{article}'
        for pname, pinfo in packages.items():
            s += r'\usepackage'
            if pinfo['options']:
                s += '[' + pinfo['options'] + ']'
            s += r'{' + pname + r'}'
        
        s += '\n'
        s += r'\begin{document}\n'
        has_title = False
        if 'title' in document.metadata:
            has_title = True
            s += r'\title{' + document.metadata['title'] + '}\n'
        if 'author' in document.metadata:
            has_title = True
            s += r'\author{' + document.metadata['author'] + '}\n'
        if 'date' in document.metadata:
            has_title = True
            s += r'\date{' + document.metadata['date'] + '}\n'
        if has_title:
            s += r'\maketitle' + '\n'

        s += latex

        s += r'\end{document}' + '\n'

        return s




RenderWorkflowClass = FlmLatexWorkflow
