import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import ParsedArgumentsInfo

from ..flmenvironment import FLMArgumentSpec
from ..flmspecinfo import FLMMacroSpecBase

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
    # special, for macro expansions.  Allow specials (like the "#N"
    # placeholders!) to be expanded while processing the target URL or Email.
    'target_Xhref': FLMArgumentSpec(
        parser='{',
        argname='target_href',
    ),
    'target_Xemail': FLMArgumentSpec(
        parser='{',
        argname='target_email',
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
        arguments_spec_list = self._get_arguments_spec_list(command_arguments)
        super().__init__(
            macroname=macroname,
            arguments_spec_list=arguments_spec_list,
        )
        self.command_arguments = command_arguments
        self.ref_type = ref_type

        self.command_argnames = [ c.argname for c in arguments_spec_list ]

        if 'display_text' in self.command_argnames:
            # internal, used when truncating fragments to a certain number of
            # characters (see fragment.truncate_to())
             self._flm_main_text_argument = 'display_text'


    _fields = ('macroname', 'command_arguments', 'ref_type', )


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
            self.command_argnames,
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
            ps = node.parsing_state.sub_context(is_block_level=False)
            display_text_nodelist = node.latex_walker.make_nodelist(
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        parsing_state=ps,
                        chars=self.pretty_url(target_href),
                        pos=node.pos,
                        pos_end=node.pos,
                    )
                ],
                parsing_state=ps,
            )

        return render_context.fragment_renderer.render_link(
            self.ref_type,
            target_href,
            display_text_nodelist,
            render_context,
        )


    def recompose_pure_latex(self, node, recomposer, visited_results_arguments, **kwargs):

        s = '\\' + node.macroname

        # Fix URL arguments to make sure we escape '#' and '%'.  This might be
        # necessary in case the \href/\url command is used within an argument to
        # some outer macro and the catcodes of these characters are already
        # fixed...

        s_macro_args = []

        rx = re.compile(r'[\\#%{}]')

        for (argname, recomposed_arg_value) in \
            zip(self.command_argnames, visited_results_arguments):

            if argname in ('target_href', 'target_email'):
                slenm1 = len(recomposed_arg_value) - 1
                s_macro_args.append(
                    recomposed_arg_value[0]
                    + rx.sub(lambda m: '\\'+m.group(0), recomposed_arg_value[1:slenm1])
                    + recomposed_arg_value[slenm1]
                )
            else:
                s_macro_args.append(recomposed_arg_value)

        return s + "".join(s_macro_args)



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

    HrefHyperlinkMacroClass = HrefHyperlinkMacro


FeatureClass = FeatureHref
