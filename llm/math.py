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



class MathEnvironment(LLMSpecInfo):

    def render(self, node, render_context, *, target_id=None):
        r"""
        ....

        The `target_id=` parameter is in case subclasses want to call this base
        class implementation with a custom `target_id` to set to the rendered
        math content.
        """
        environmentname = node.environmentname

        return render_context.fragment_renderer.render_math_content(
            (f"\\begin{{{environmentname}}}", f"\\end{{{environmentname}}}",),
            node.nodelist,
            render_context,
            'display',
            environmentname=environmentname,
            target_id=target_id
        )



class MathEqrefViaMathContent(LLMSpecInfo):
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

