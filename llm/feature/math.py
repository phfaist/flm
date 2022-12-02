import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    LatexWalkerParseError,
    ParsedArgumentsInfo,
    ParsingStateDeltaEnterMathMode,
)
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import (
    MacroSpec,
    LatexEnvironmentBodyContentsParser,
    ParsingStateDeltaExtendLatexContextDb,
)

from ..llmspecinfo import LLMMacroSpecBase, LLMEnvironmentSpecBase
from ..llmenvironment import LLMArgumentSpec

from ._base import Feature

from .. import fmthelpers



def sanitize_for_id(x):
    return re.sub(r'[^a-zA-Z0-9_-]', '-', x)



_default_math_environment_names = (
    'equation',
    'equation*',
    'align',
    'align*',
    'gather',
    'gather*',
)


class FeatureMath(Feature):

    feature_name = 'math'

    def __init__(
            self,
            eq_counter_formatter=None,
            math_environment_names=None,
            eqref_macro_name='eqref',
            eqref_ref_type='eq',
    ):
        super().__init__()

        if eq_counter_formatter is None:
            eq_counter_formatter = {'template': '(${arabic})'}
        self.eq_counter_formatter = \
            fmthelpers.parse_counter_formatter(eq_counter_formatter)

        if math_environment_names is None:
            math_environment_names = _default_math_environment_names
        self.math_environment_names = math_environment_names

        self.eqref_macro_name = eqref_macro_name
        self.eqref_ref_type = eqref_ref_type

    class DocumentManager(Feature.DocumentManager):
        def initialize(self):
            pass
            
    class RenderManager(Feature.RenderManager):
        def initialize(self):
            self.equation_counter = 1
            self.equation_info_by_node = {}

        def new_numbered_display_math(self, node, lineno, custom_tag_llm_text=None):

            key = (self.get_node_id(node), lineno)
            if key in self.equation_info_by_node:
                return self.equation_info_by_node[key]

            if custom_tag_llm_text is not None:
                formatted_ref_llm_text = custom_tag_llm_text
                eq_id = f"_{key[0]}"
                if lineno:
                    eq_id += f"-{lineno}"
            else:
                eq_id = self.equation_counter
                formatted_ref_llm_text = self.feature.eq_counter_formatter(eq_id)
                self.equation_counter += 1

            info = (eq_id, formatted_ref_llm_text)
            self.equation_info_by_node[key] = info
            return info


    def add_latex_context_definitions(self):
        environments = [
            MathEnvironment(
                math_environment_name,
            )
            for math_environment_name in self.math_environment_names
        ]
        if self.eqref_macro_name is not None:
            macros = [
                MathEqrefMacro(
                    macroname=self.eqref_macro_name,
                    ref_type=self.eqref_ref_type
                ),
            ]

        return dict(macros=macros, environments=environments)



# ---

class _ProxyNodeWithLatexVerbatim:

    pos = None
    pos_end = None

    def __init__(self, verbatim):
        self._verbatim = verbatim

    def latex_verbatim(self):
        return self._verbatim


class MathEnvironment(LLMEnvironmentSpecBase):

    allowed_in_standalone_mode = True

    def __init__(self, environmentname, is_numbered=None):
        super().__init__(environmentname=environmentname)
        if is_numbered is not None:
            self.is_numbered = is_numbered
        else:
            self.is_numbered = (environmentname[-1:] != '*') # align*, gather*, etc. 

    def make_body_parsing_state_delta(self, token, nodeargd, arg_parsing_state_delta,
                                      latex_walker, **kwargs):
        return ParsingStateDeltaEnterMathMode()

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        MacroSpec('label', arguments_spec_list=[
                            LLMArgumentSpec(
                                parser=latexnodes_parsers.LatexCharsGroupParser(
                                    delimiters=('{','}'),
                                ),
                                argname='label',
                            ),
                        ]),
                        MacroSpec('tag', arguments_spec_list=[
                            LLMArgumentSpec(
                                parser='*',
                                argname='tag_star',
                            ),
                            LLMArgumentSpec(
                                parser=latexnodes_parsers.LatexCharsGroupParser(
                                    delimiters=('{','}'),
                                ),
                                argname='text',
                            ),
                        ]),
                        MacroSpec('\\', arguments_spec_list=[]),
                    ],
                )
            )
        )

    def postprocess_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        #logger.debug("finalizing math environment node: node = %r", node)

        # find and register and \label nodes
        node.llm_equation_lines_labels_infos = []

        if not self.is_numbered:
            return node

        last_line_info = {
            'labels_info': [],
            'custom_tag_llm_text': None,
        }
        def _flush_last_equation_line_labels_infos(newline_node=None):
            node.llm_equation_lines_labels_infos.append({
                'labels': list(last_line_info['labels_info']),
                'custom_tag_llm_text': last_line_info['custom_tag_llm_text'],
                'newline_node': newline_node,
            })
            last_line_info.update({
                'labels_info': [],
                'custom_tag_llm_text': None,
            })

        last_node_is_newline = False
        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'label':
                # this is a \label command -- register it for this equation line

                # extract ref_type, ref_label and store these values
                ref_label_node_args = \
                    ParsedArgumentsInfo(node=n).get_all_arguments_info(
                        ('label',),
                    )
                ref_label_full = ref_label_node_args['label'].get_content_as_chars()

                if ':' in ref_label_full:
                    ref_type, ref_label = ref_label_full.split(':', 1)
                else:
                    ref_type, ref_label = None, ref_label_full

                info = {
                    'node': n,
                    'label': (ref_type, ref_label),
                }

                last_line_info['labels_info'].append(info)
                break

            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'tag':
                # custom tag for this line
                tag_node_args = \
                    ParsedArgumentsInfo(node=n).get_all_arguments_info(
                        ('tag_star', 'text'),
                    )

                custom_tag_llm_text = \
                    tag_node_args['text'].get_content_nodelist().latex_verbatim()
                if not tag_node_args['tag_star'].was_provided():
                    custom_tag_llm_text = f'({custom_tag_llm_text})'

                last_line_info['custom_tag_llm_text'] = custom_tag_llm_text

            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == '\\':
                _flush_last_equation_line_labels_infos(n)
                last_node_is_newline = True
                continue
            
            if (not n.isNodeType(latexnodes_nodes.LatexCommentNode)
                and not (n.isNodeType(latexnodes_nodes.LatexCharsNode) and not n.chars.strip())
                ):
                last_node_is_newline = False
                
        if not last_node_is_newline:
            _flush_last_equation_line_labels_infos()

        return node


    def render(self, node, render_context):
        r"""
        """
        environmentname = node.environmentname

        if not render_context.supports_feature('math'):
            raise ValueError("Feature 'math' is not enabled, cannot render math environment")

        math_mgr = render_context.feature_render_manager('math')

        refs_mgr = None
        if render_context.supports_feature('refs'):
            refs_mgr = render_context.feature_render_manager('refs')

        # create an alternative node list
        nodelist = list(node.nodelist)

        target_id = None

        for lineno, line_infos in enumerate(node.llm_equation_lines_labels_infos):

            custom_tag_llm_text = line_infos['custom_tag_llm_text']

            # add equation instance
            eq_id, formatted_ref_llm_text = \
                math_mgr.new_numbered_display_math(node, lineno, custom_tag_llm_text)

            this_target_id = f'equation-{eq_id}'
            if target_id is None:
                # target_id refers to the first equation in an equation list
                target_id = this_target_id

            # insert the tag at appropriate location in nodelist
            newline_node = line_infos['newline_node']
            if newline_node is not None:
                i = nodelist.index(newline_node)
            else:
                i = len(nodelist)
            nodelist.insert(
                i,
                _ProxyNodeWithLatexVerbatim(
                    r'\tag*{' + formatted_ref_llm_text + r'}'
                )
            )

            # register the reference
            if refs_mgr is not None and render_context.is_first_pass:
                for label_info in line_infos['labels']:
                    (ref_type, ref_label) = label_info['label']
                    refs_mgr.register_reference(
                        ref_type, ref_label,
                        node=node, formatted_ref_llm_text=formatted_ref_llm_text,
                        target_href=f'#{this_target_id}'
                    )

        return render_context.fragment_renderer.render_math_content(
            (f"\\begin{'{'}{environmentname}{'}'}", f"\\end{'{'}{environmentname}{'}'}",),
            latexnodes_nodes.LatexNodeList(nodelist),
            render_context,
            'display',
            environmentname=environmentname,
            target_id=target_id,
        )




class MathEqrefMacro(LLMMacroSpecBase):

    delayed_render = True

    allowed_in_standalone_mode = False
    r"""
    Reference commands are definitly not allowed in standalone mode
    """

    def __init__(self, macroname='eqref', ref_type='eq', **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                LLMArgumentSpec(
                    latexnodes_parsers.LatexCharsGroupParser(),
                    argname='ref_label',
                )
            ],
        )
        self.ref_type = ref_type

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('ref_label',),
        )
        
        ref_type = None
        ref_label = node_args['ref_label'].get_content_as_chars()
        if ':' in ref_label:
            ref_type, ref_label = ref_label.split(':', 1)

        if ref_type != self.ref_type:
            raise LatexWalkerParseError(
                f"Equation labels must begin with “{self.ref_type}:” "
                f"(error in ‘\\{node.macroname}’)",
                pos=node.pos
            )

        node.llmarg_ref = (ref_type, ref_label)


    def prepare_delayed_render(self, node, render_context):
        pass

    def render(self, node, render_context):

        ref_type, ref_label = node.llmarg_ref

        refs_mgr = render_context.feature_render_manager('refs')

        resource_info = node.latex_walker.resource_info

        try:
            return refs_mgr.render_ref(
                ref_type, ref_label,
                None,
                resource_info,
                render_context
            )
        except Exception as e:
            logger.error(f"Failed to resolve reference to ‘{ref_type}:{ref_label}’: {e} "
                         f"in ‘{node.latex_verbatim()}’ @ {node.format_pos()}")
            raise LatexWalkerParseError(
                f"Unable to resolve reference to ‘{ref_type}:{ref_label}’: {e}",
                pos=node.pos,
            )


