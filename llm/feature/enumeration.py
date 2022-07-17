
import logging
logger = logging.getLogger(__name__)

import re

from pylatexenc.latexnodes import ParsedArgumentsInfo
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes
from pylatexenc.macrospec import (
    MacroSpec,
    LatexEnvironmentBodyContentsParser,
    ParsingStateDeltaExtendLatexContextDb,
)

from ..llmspecinfo import LLMEnvironmentSpecBase
from ..llmenvironment import (
    LLMParsingStateDeltaSetBlockLevel,
    LLMArgumentSpec,
)

from .. import fmthelpers

from ._base import Feature



_count_initials_formatters = {
    'a': fmthelpers.alphacounter,
    'A': fmthelpers.Alphacounter,
    'i': fmthelpers.romancounter,
    'I': fmthelpers.Romancounter,
    '1': str,
}
_rx_count_initial = re.compile(r'[aA1iI]')
def _get_counter_formatter_from_tag_template(tag_template):
    m = _rx_count_initial.search(tag_template)
    if m is not None:
        # substitute a counter
        left = tag_template[:m.start()]
        right = tag_template[m.end():]
        counter_formatter = _count_initials_formatters[m.group()]
        return lambda n: (left + counter_formatter(n) + right)

    # no counter. E.g., a bullet symbol
    return tag_template


# "1.", "2.", ...
_default_enumeration_counter_formatter = [
    lambda n: f"{n}.",
    lambda n: f"({fmthelpers.romancounter(n)})",
    lambda n: f"{fmthelpers.alphacounter(n)}-",
]


class Enumeration(LLMEnvironmentSpecBase):
    r"""
    ..............

    `counter_formatter` can be either:
    
    - A fixed string -- this string will be used as the item tag

    - A callable -- the callable should accept an integer (starting at one) and
      return the string to display as item tag

    - A list, with elements being either fixed strings or callables; specify the
      item tags or how to generate them for nested lists.  The first element is
      used for the root list, the second is used for the first nested list, etc.
    """


    is_block_level = True

    body_contents_is_block_level = True

    allowed_in_standalone_mode = True

    def __init__(self,
                 environmentname,
                 *,
                 counter_formatter=None,
                 annotations=None,
                 **kwargs):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=[
                LLMArgumentSpec(
                    latexnodes_parsers.LatexCharsGroupParser(
                        delimiters=('[',']'),
                        optional=True
                    ),
                    argname='tag_template',
                )
            ],
            **kwargs
        )
        if counter_formatter is None:
            counter_formatter = _default_enumeration_counter_formatter
        self.counter_formatter = counter_formatter
        self.annotations = annotations

        self.body_parsing_state_delta = \
            LLMParsingStateDeltaSetBlockLevel(is_block_level=self.is_block_level)
        

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        MacroSpec('item', arguments_spec_list=[
                            LLMArgumentSpec('[', argname='custom_tag'),
                        ])
                    ]
                )
            )
        )

    def postprocess_parsed_node(self, node):
        # parse the node structure right away when finializing then ode
        logger.debug("finalizing node: node = %r", node)
        item_nodelists = node.nodelist.split_at_node(
            lambda n: (n.isNodeType(latexnodes_nodes.LatexMacroNode)
                       and n.macroname == 'item'),
            keep_separators=True,
        )
        enumeration_items = []
        for j, item_nodelist in enumerate(item_nodelists):
            if not item_nodelist:
                continue # ?
            item_macro = item_nodelist[0]
            if item_macro is None:
                continue # ?
            if (j == 0
                and item_macro.isNodeType(latexnodes_nodes.LatexCharsNode)
                and item_macro.chars.strip() == ''):
                # skip leading whitespace
                continue
            if (not item_macro.isNodeType(latexnodes_nodes.LatexMacroNode)
                or item_macro.macroname != 'item'):
                raise LatexWalkerParseError(
                    msg=f"Expected ‘\\item’ in enumeration environment: {item_macro!r}",
                    pos=item_macro.pos,
                )
            item_content_nodelist = item_macro.latex_walker.make_nodelist(
                item_nodelist.nodelist[1:],
                parsing_state=item_macro.parsing_state,
            )
            logger.debug("item_content_nodelist = %r  (blocks = %r)",
                         item_content_nodelist,
                         getattr(item_content_nodelist, 'llm_blocks', None))
            
            enumeration_items.append(
                (item_macro, item_content_nodelist)
            )

        node.enumeration_items = enumeration_items
        return node


    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('tag_template',),
        )

        state = render_context.get_logical_state('enumeration')
        nested_depth = state.get('nested_depth', 0)

        # determine the base counter formatter to use depending on nested depth
        counter_formatter = self.counter_formatter
        if not isinstance(counter_formatter, str) and not callable(counter_formatter):
            if nested_depth >= len(counter_formatter):
                counter_formatter = counter_formatter[len(counter_formatter)-1]
            else:
                counter_formatter = counter_formatter[nested_depth]

        if 'tag_template' in node_args and node_args['tag_template'].was_provided():
            tag_template_chars = node_args['tag_template'].get_content_as_chars()

            counter_formatter = _get_counter_formatter_from_tag_template(tag_template_chars)

        items_custom_tags = {}
        items_nodelists = []
        for j, iteminfo in enumerate(node.enumeration_items):
            item_macro, item_content_nodelist = iteminfo
            items_nodelists.append( item_content_nodelist )

            item_node_args = ParsedArgumentsInfo(node=item_macro).get_all_arguments_info(
                ('custom_tag',),
            )

            if 'custom_tag' in item_node_args and item_node_args['custom_tag'].was_provided():
                items_custom_tags[1+j] = item_node_args['custom_tag'].get_content_nodelist()

        def the_counter_formatter(n):
            if n in items_custom_tags:
                return items_custom_tags[n]
            if callable(counter_formatter):
                return counter_formatter(n)
            return counter_formatter

        with render_context.push_logical_state('enumeration', 'nested_depth', nested_depth+1):
            # for transcrypt -- don't return from within a with statement or the
            # __exit__ won't be called ...?
            result = fragment_renderer.render_enumeration(
                items_nodelists,
                the_counter_formatter,
                render_context=render_context,
                annotations=self.annotations,
                nested_depth=nested_depth
            )
        return result



default_enumeration_environments = {

    'itemize': {'counter_formatter':['•','-','▸'],},

    # uses default formatters 1., 2., ... incl. nested (i) etc.
    'enumerate': {'counter_formatter': None},
}


class FeatureEnumeration(Feature):
    r"""
    Add support for enumeration and itemization lists, e.g., via LaTeX commands
    ``\begin{enumerate} ... \end{enumerate}`` and ``\begin{itemize}
    ... \end{itemize}``.
    """

    feature_name = 'enumeration'
    # no managers needed
    DocumentManager = None
    RenderManager = None

    def __init__(self, enumeration_environments=None):
        super().__init__()
        if enumeration_environments is None:
            enumeration_environments = default_enumeration_environments
        self.enumeration_environments = enumeration_environments

    def add_latex_context_definitions(self):
        return dict(
            environments=[
                Enumeration(
                    environmentname=envname,
                    counter_formatter=envinfo['counter_formatter'],
                    annotations=[envname],
                )
                for envname, envinfo in self.enumeration_environments.items()
            ],
        )
