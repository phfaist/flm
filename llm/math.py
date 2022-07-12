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

from .llmspecinfo import LLMMacroSpecBase, LLMEnvironmentSpecBase
from .llmenvironment import LLMArgumentSpec



def sanitize_for_id(x):
    return re.sub(r'[^a-zA-Z0-9_-]', '-', x)



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
                        ])
                    ]
                )
            )
        )

    def postprocess_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        #logger.debug("finalizing math environment node: node = %r", node)

        # find and register \label node
        node.llm_equation_label_node = None
        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'label':
                # this is the equation's \label command -- register it
                if node.llm_equation_label_node is not None:
                    raise LatexWalkerParseError(
                        "You cannot use multiple \\label's in an equation",
                        pos=n.pos
                    )

                #logger.debug("Found label node: %r", n)
                node.llm_equation_label_node = n
                
                # extract ref_label_prefix, ref_label and store these values

                ref_label_node_args = \
                    ParsedArgumentsInfo(node=node.llm_equation_label_node) \
                    .get_all_arguments_info(
                        ('label',),
                    )
                ref_label_full = ref_label_node_args['label'].get_content_as_chars()

                if ':' in ref_label_full:
                    ref_label_prefix, ref_label = ref_label_full.split(':', 1)
                else:
                    ref_label_prefix, ref_label = None, ref_label_full

                node.llm_equation_ref_label_prefix = ref_label_prefix
                node.llm_equation_ref_label = ref_label

                break

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
                    argname='ref_target',
                )
            ],
        )

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('ref_target',),
        )
        
        ref_type = None
        ref_target = node_args['ref_target'].get_content_as_chars()
        if ':' in ref_target:
            ref_type, ref_target = ref_target.split(':', 1)

        if ref_type != 'eq':
            raise LatexWalkerParseError(
                f"Equation labels must begin with “eq:” (error in ‘\\{node.macroname}’)",
                pos=node.pos
            )

        node.llmarg_ref_type = ref_type
        node.llmarg_ref_target = ref_target


    def render(self, node, render_context):

        # simply emit the \eqref{...} call as we got it directly, and let
        # MathJax handle the referencing

        return render_context.fragment_renderer.render_math_content(
            (r"\(", r"\)"),
            latexnodes_nodes.LatexNodeList([node]),
            render_context,
            'inline',
        )

