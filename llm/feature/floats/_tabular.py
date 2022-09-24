import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import ParsedArgumentsInfo, LatexWalkerParseError
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
# from pylatexenc.macrospec import (
#     LatexEnvironmentBodyContentsParser,
#     MacroSpec,
#     ParsingStateDeltaExtendLatexContextDb,
# )

from ...llmenvironment import LLMArgumentSpec
from ...llmspecinfo import LLMEnvironmentSpecBase
from ... import fmthelpers



.........................




class TabularEnvironment(LLMEnvironmentSpecBase):

    is_block_level = True

    allowed_in_standalone_mode = True



    def postprocess_parsed_node(self, node):

        # build an internal representation of the table here!
        ...............


    
    def render(self, node, render_context):
        r"""
        Produce a final representation of the node, using the given
        `render_context`.
        """
        raise RuntimeError(
            f"Element ‘{node}’ cannot be placed here, render() not reimplemented."
        )

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        ........??
        assert( token.tok == 'begin_environment' )
        environment_name = token.arg
        return latexnodes_parsers.LatexVerbatimEnvironmentContentsParser(
            environment_name=environment_name
        )

