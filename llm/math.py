import re

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerParseError, ParsedArgumentsInfo
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import (
    MacroSpec,
    LatexEnvironmentBodyContentsParser,
    ParsingStateDeltaExtendLatexContextDb,
)

from .llmspecinfo import LLMSpecInfo
from .llmenvironment import (
    LLMParsingStateDeltaSetBlockLevel,
    make_arg_spec,
)

def sanitize_for_id(x):
    return re.sub(r'[^a-zA-Z0-9_-]', '-', x)


class MathEnvironment(LLMSpecInfo):

    def render(self, node, render_context):
        environmentname = node.environmentname

        target_id = None

        refs_mgr = None
        if render_context.supports_feature('refs'):
            refs_mgr = render_context.feature_render_manager('refs')
        if refs_mgr is not None and hasattr(node, 'llm_equation_label_node'):
            ref_label_node = node.llm_equation_label_node
            logging.debug("Equation has label: %r", ref_label_node)
            ref_label_node_args = \
                ParsedArgumentsInfo(node=ref_label_node).get_all_arguments_info(
                ('label',),
            )
            ref_label_value = ref_label_node_args['label'].get_content_as_chars()
            if not ref_label_value.startswith('eq:'):
                raise LatexWalkerParseError(
                    f"Equation labels must be of the form ‘eq:...’: ‘{ref_label}’",
                    pos=node.llm_label_node.pos
                )
            ref_target = ref_label_value[3:] # remove prefix 'eq:'
            target_id = f"equation--{sanitize_for_id(ref_target)}"
            refs_mgr.register_reference(
                'eq',
                ref_target,
                "(*)",
                f"#{target_id}"
            )

        return render_context.fragment_renderer.render_math_content(
            (f"\\begin{{{environmentname}}}", f"\\end{{{environmentname}}}",),
            node.nodelist,
            render_context,
            'display',
            environmentname=environmentname,
            target_id=target_id
        )

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        MacroSpec('label', arguments_spec_list=[
                            make_arg_spec(
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

    def finalize_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        logger.debug("finalizing math environment node: node = %r", node)

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
                node.llm_equation_label_node = n
                logger.debug("Found label node: %r", n)

        return node



class MathEqref(LLMSpecInfo):
    def render(self, node, render_context):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('ref_target',),
        )
        
        ref_type = None
        ref_target = node_args['ref_target'].get_content_as_chars()
        if ':' in ref_target:
            ref_type, ref_target = ref_target.split(':', 1)

        if ref_type != 'eq':
            raise ValueError(
                f"Equation labels must begin with “eq:” (error in ‘\\{node.macroname}’)"
            )

        # simply emit the \eqref{...} call as we got it directly, and let
        # MathJax handle the referencing

        return render_context.fragment_renderer.render_math_content(
            (r"\(", r"\)"),
            latexnodes_nodes.LatexNodeList([node]),
            render_context,
            'inline',
        )

