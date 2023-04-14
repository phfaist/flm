import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import ParsedArgumentsInfo

from ..flmspecinfo import FLMArgumentSpec, FLMMacroSpecBase

from ._base import SimpleLatexDefinitionsFeature


_href_arg_specs = {
    'target_href': FLMArgumentSpec(
        parser=latexnodes_parsers.LatexDelimitedVerbatimParser( ('{','}') ),
        argname='target_href',
    ),
    'target_email': FLMArgumentSpec(
        parser=latexnodes_parsers.LatexDelimitedVerbatimParser( ('{','}') ),
        argname='target_email',
    ),
    'display_text': FLMArgumentSpec(
        parser='{',
        argname='display_text',
    ),
}


class HrefHyperlinkMacro(FLMMacroSpecBase):

    allowed_in_standalone_mode = True

    def __init__(
            self,
            macroname,
            *,
            command_arguments=('target_href', 'target_email', 'display_text',),
            ref_type='href',
    ):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=self._get_arguments_spec_list(command_arguments)
        )
        self.command_arguments = [ c.strip('[]') for c in command_arguments ]
        self.ref_type = ref_type

        if 'display_text' in command_arguments:
            # internal, used when truncating fragments to a certain number of
            # characters (see fragment.truncate_to())
             self._flm_main_text_argument = 'display_text'

    @classmethod
    def _get_arguments_spec_list(cls, command_arguments):
        return [
            _href_arg_specs[cmdarg]
            for cmdarg in command_arguments
        ]

    @classmethod
    def pretty_url(cls, target_href):
        url_display = str(target_href)
        for prefix in ('http://', 'https://', 'mailto:'):
            if url_display.startswith(prefix):
                url_display = url_display[len(prefix):]
                break
        url_display = url_display.rstrip('/#?')
        return url_display


    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            self.command_arguments,
        )
        
        target_href = None
        display_text_nodelist = None

        if 'target_href' in node_args:
            target_href = node_args['target_href'].get_content_as_chars()
        elif 'target_email' in node_args:
            target_href = "mailto:" + node_args['target_email'].get_content_as_chars()
        else:
            raise ValueError(
                "FLM Internal Error: no URL or hyper reference provided to href-like macro"
            )

        if 'display_text' in node_args:
            display_text_nodelist = node_args['display_text'].get_content_nodelist()

        node.flm_href_info = {
            'target_href': target_href,
            'display_text_nodelist': display_text_nodelist
        }

    def render(self, node, render_context):

        target_href = node.flm_href_info['target_href']
        display_text_nodelist = node.flm_href_info['display_text_nodelist']

        # show URL by default
        if display_text_nodelist is None:
            display_text_nodelist = node.latex_walker.make_nodelist(
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        parsing_state=node.parsing_state,
                        chars=self.pretty_url(target_href),
                        pos=node.pos,
                        pos_end=node.pos,
                    )
                ],
                parsing_state=node.parsing_state
            )

        return render_context.fragment_renderer.render_link(
            self.ref_type,
            target_href,
            display_text_nodelist,
            render_context,
        )




class FeatureHref(SimpleLatexDefinitionsFeature):

    feature_name = 'href'
    feature_title = 'Hyperlinks'

    latex_definitions = {
        'macros': [
            HrefHyperlinkMacro(
                macroname='href',
                command_arguments=('target_href', 'display_text',),
            ),
            HrefHyperlinkMacro(
                macroname='url',
                command_arguments=('target_href',),
            ),
            HrefHyperlinkMacro(
                macroname='email',
                command_arguments=('target_email',),
            ),
        ]
    }


FeatureClass = FeatureHref
