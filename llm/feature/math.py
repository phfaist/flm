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
from .refs import (
    ReferenceableInfo,
)

from .. import fmthelpers




# replaced by referenceable
# def sanitize_for_id(x):
#     return re.sub(r'[^a-zA-Z0-9_-]', '-', x)




class FeatureMath(Feature):

    feature_name = 'math'

    def __init__(self):
        super().__init__()

    class DocumentManager(Feature.DocumentManager):
        def initialize(self):
            pass
            
    class RenderManager(Feature.RenderManager):
        def initialize(self):
            self.equation_counter = 0

        def add_numbered_display_math(self):

            referenceable_info = ReferenceableInfo(
                self........
            )

            if self.render_context.supports_feature('refs') \
               and self.render_context.is_first_pass:
                refs_mgr = render_context.feature_render_manager('refs')
                refs_mgr.register_reference_referenceable(
                    node=node,
                    referenceable_info=node.llm_referenceable_info,
                )


                pass


# ---

class MathEnvironment(LLMEnvironmentSpecBase):

    allowed_in_standalone_mode = True

    def __init__(self, environmentname):
        super().__init__(environmentname=environmentname)

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

        last_equation_line_labels_info = []
        def _flush_last_equation_line_labels_infos(newline_node=None):
            node.llm_equation_lines_labels_infos.append({
                'labels': list(last_equation_line_labels_info),
                'newline_node': newline_node,
            })
            last_equation_line_labels_info[:] = []

        last_node_is_newline = False
        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'label':
                # this is a \label command -- register it for this equation line

                # extract ref_label_prefix, ref_label and store these values
                ref_label_node_args = \
                    ParsedArgumentsInfo(node=n).get_all_arguments_info(
                        ('label',),
                    )
                ref_label_full = ref_label_node_args['label'].get_content_as_chars()

                if ':' in ref_label_full:
                    ref_label_prefix, ref_label = ref_label_full.split(':', 1)
                else:
                    ref_label_prefix, ref_label = None, ref_label_full

                info = {
                    'node': n,
                    'label': (ref_label_prefix, ref_label),
                }

                last_equation_line_labels_info.append(info)
                break

            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == '\\':
                _flush_last_equation_line_labels_infos(n)
                last_node_is_newline = True
                continue
            
            if (not n.isNodeType(latexnodes_nodes.LatexCommentNode)
                and not (n.isNodeType(latexnodes_nodes.LatexCharsNode) and not n.chars.strip())
                ):
                last_node_is_newline = False
                
        if not last_node_is_bb:
            _flush_last_equation_line_labels_infos()

        return node


    def render(self, node, render_context):
        r"""
        """
        environmentname = node.environmentname

        

        # transcrypt doesn't like getattr with default argument
        ref_label_prefix = None
        ref_label = None
        if hasattr(node, 'llm_equation_ref_label_prefix'):
            ref_label_prefix = node.llm_equation_ref_label_prefix
        if hasattr(node, 'llm_equation_ref_label'):
            ref_label = node.llm_equation_ref_label

        if ref_label_prefix is not None and ref_label is not None:
            target_id = f"equation--{sanitize_for_id(ref_label_prefix+':'+ref_label)}"
        else:
            target_id = None

        return render_context.fragment_renderer.render_math_content(
            (f"\\begin{'{'}{environmentname}{'}'}", f"\\end{'{'}{environmentname}{'}'}",),
            node.nodelist,
            render_context,
            'display',
            environmentname=environmentname,
            target_id=target_id,
        )




class MathEqrefViaMathContent(LLMMacroSpecBase):

    allowed_in_standalone_mode = False
    r"""
    Reference commands are definitly not allowed in standalone mode
    """

    def __init__(self, macroname='eqref', **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                LLMArgumentSpec(
                    latexnodes_parsers.LatexCharsGroupParser(),
                    argname='ref_label',
                )
            ],
        )

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('ref_label',),
        )
        
        ref_type = None
        ref_label = node_args['ref_label'].get_content_as_chars()
        if ':' in ref_label:
            ref_type, ref_label = ref_label.split(':', 1)

        if ref_type != 'eq':
            raise LatexWalkerParseError(
                f"Equation labels must begin with “eq:” (error in ‘\\{node.macroname}’)",
                pos=node.pos
            )

        node.llmarg_ref = (ref_type, ref_label)


    def render(self, node, render_context):

        # simply emit the \eqref{...} call as we got it directly, and let
        # MathJax handle the referencing

        return render_context.fragment_renderer.render_math_content(
            (r"\(", r"\)"),
            latexnodes_nodes.LatexNodeList([node]),
            render_context,
            'inline',
        )

