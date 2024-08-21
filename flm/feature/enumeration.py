
import logging
logger = logging.getLogger(__name__)


from pylatexenc.latexnodes import ParsedArgumentsInfo, LatexWalkerLocatedError
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes
from pylatexenc.macrospec import (
    MacroSpec,
    LatexEnvironmentBodyContentsParser,
    ParsingStateDeltaExtendLatexContextDb,
)

from ..flmspecinfo import (
    FLMEnvironmentSpecBase,
    label_arg,
    helper_collect_labels,
    # FLMSpecInfoParagraphBreak
)
from ..flmenvironment import (
    FLMArgumentSpec,
)

from .. import counter

from ._base import Feature

from .refs import get_safe_target_id



# "1.", "2.", ...
_default_enumeration_counter_formatter = [
    {'template': "${arabic}."},
    {'template': "(${roman})"},
    {'template': "${alph}-"},
]


class Enumeration(FLMEnvironmentSpecBase):
    r"""
    Doc ..............

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


    # allow these prefixes in \label's pinned to individual \item's
    allowed_item_label_prefixes = ('item', )


    def __init__(self,
                 environmentname,
                 *,
                 counter_formatter=None,
                 annotations=None,
                 **kwargs):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=[
                FLMArgumentSpec(
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
        
    _fields = ('environmentname', 'counter_formatter', 'annotations', )

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        MacroSpec('item', arguments_spec_list=[
                            FLMArgumentSpec('[', argname='custom_tag'),
                            label_arg, # allow \label{}'s to be attached to \item's
                        ])
                    ]
                )
            )
        )

    def postprocess_parsed_node(self, node):
        # don't postprocess the node twice (e.g. after dump/load cycle)
        if hasattr(node, 'flm_enumeration_items'):
            logger.debug("enumeration: postprocess_parsed_node() on already "
                         "postprocessed node, skipping")
            return node

        # parse the node structure right away when finializing then ode
        #logger.debug("finalizing node: node = %r", node)
        item_nodelists = node.nodelist.split_at_node(
            lambda n: (n.isNodeType(latexnodes_nodes.LatexMacroNode)
                       and n.macroname == 'item'),
            keep_separators=True,
            call_make_nodelist=False,
        )
        enumeration_items = []
        for j, item_nodelist in enumerate(item_nodelists):
            if not item_nodelist:
                continue # ?
            item_macro = item_nodelist[0]
            if item_macro is None:
                continue # ?
            if (j == 0
                and (
                    (item_macro.isNodeType(latexnodes_nodes.LatexCharsNode)
                     and item_macro.chars.strip() == '')
                    or (
                        item_macro.isNodeType(latexnodes_nodes.LatexSpecialsNode)
                        #and isinstance(item_macro.spec, FLMSpecInfoParagraphBreak)
                        and item_macro.spec.is_paragraph_break_marker
                    )
                )):
                # skip leading whitespace
                continue
            if (not item_macro.isNodeType(latexnodes_nodes.LatexMacroNode)
                or item_macro.macroname != 'item'):
                raise LatexWalkerLocatedError(
                    msg=f"Expected ‘\\item’ in enumeration environment: {item_macro!r}",
                    pos=item_macro.pos,
                )
            item_content_nodelist = item_macro.latex_walker.make_nodelist(
                item_nodelist.nodelist[1:],
                parsing_state=item_macro.parsing_state,
            )
            logger.debug("item_content_nodelist = %r  (blocks = %r)",
                         item_content_nodelist,
                         getattr(item_content_nodelist, 'flm_blocks', None))
            
            enumeration_items.append(
                (item_macro, item_content_nodelist)
            )

        node.flm_enumeration_items = enumeration_items
        return node


    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('tag_template', ),
        )

        state = render_context.get_logical_state('enumeration')
        nested_depth = state.get('nested_depth', 0)

        # determine the base counter formatter to use depending on nested depth
        counter_formatter_spec = self.counter_formatter
        if isinstance(counter_formatter_spec, (list,tuple)):
            if nested_depth >= len(counter_formatter_spec):
                counter_formatter_spec = counter_formatter_spec[len(counter_formatter_spec)-1]
            else:
                counter_formatter_spec = counter_formatter_spec[nested_depth]

        if 'tag_template' in node_args and node_args['tag_template'].was_provided():
            tag_template_chars = node_args['tag_template'].get_content_as_chars()
            counter_formatter_spec = tag_template_chars

        counter_formatter = counter.parse_counter_formatter(
            counter_formatter_spec,
            str_use_tag_template=True,
        )

        items_custom_tags = {}
        items_custom_labels = {}
        items_nodelists = []
        for j, iteminfo in enumerate(node.flm_enumeration_items):
            item_macro, item_content_nodelist = iteminfo
            items_nodelists.append( item_content_nodelist )

            item_node_args = ParsedArgumentsInfo(node=item_macro).get_all_arguments_info(
                ('custom_tag', 'label', ),
            )

            if 'custom_tag' in item_node_args and item_node_args['custom_tag'].was_provided():
                items_custom_tags[1+j] = item_node_args['custom_tag'].get_content_nodelist()
            if 'label' in item_node_args and item_node_args['label'].was_provided():
                items_custom_labels[1+j] = helper_collect_labels(
                    item_node_args['label'],
                    self.allowed_item_label_prefixes,
                )

        def the_counter_formatter(n):
            if n in items_custom_tags:
                return items_custom_tags[n]
            return counter_formatter(n)

        enum_items_target_ids = {}
        if len(items_custom_labels) and render_context.supports_feature('refs'):
            # register any label refs
            mgr_enum = render_context.feature_render_manager('enumeration')
            mgr_refs = render_context.feature_render_manager('refs')

            # register labels to the full enumeration
            for n, labels in items_custom_labels.items():

                if len(labels) == 0:
                    continue

                if n in items_custom_tags:
                    item_tag_flm_text = item_custom_tags[n]
                else:
                    # This function will be called twice, oh well...
                    item_tag_flm_text = the_counter_formatter(n)

                ref_type_0, ref_label_0 = labels[0]

                enum_item_tgtid = get_safe_target_id(ref_type_0, ref_label_0)
                target_href = '#' + enum_item_tgtid
                enum_items_target_ids[n] = enum_item_tgtid

                for (ref_type, ref_label) in labels:
                    mgr_refs.register_reference(
                        ref_type, ref_label,
                        formatted_ref_flm_text=item_tag_flm_text,
                        node=node,
                        target_href=target_href,
                    )

        def item_target_id_generator(n):
            if n in enum_items_target_ids:
                return enum_items_target_ids[n]
            return None

        with render_context.push_logical_state('enumeration', 'nested_depth', nested_depth+1):
            # for transcrypt -- don't return from within a with statement or the
            # __exit__ won't be called ...?
            result = fragment_renderer.render_enumeration(
                items_nodelists,
                the_counter_formatter,
                render_context=render_context,
                annotations=self.annotations,
                nested_depth=nested_depth,
                target_id_generator=item_target_id_generator,
            )
        return result


    def recompose_pure_latex(self, node, recomposer, visited_results_arguments,
                             visited_results_body, **kwargs):

        # we'll have to recompose body again, oh well

        s = r'\begin{' + node.environmentname + '}'
        for arg in visited_results_arguments:
            if arg is not None:
                s += arg

        for (item_macro, item_content_nodelist) in node.flm_enumeration_items:
            
            item_node_args = ParsedArgumentsInfo(node=item_macro).get_all_arguments_info(
                ('custom_tag', 'label', ),
            )

            s += r'\item '

            if 'custom_tag' in item_node_args and item_node_args['custom_tag'].was_provided():
                s += recomposer.recompose_pure_latex(
                    item_node_args['custom_tag'].get_content_nodelist()
                )["latex"]
            if 'label' in item_node_args and item_node_args['label'].was_provided():
                items_custom_labels = helper_collect_labels(
                    item_node_args['label'],
                    self.allowed_item_label_prefixes,
                )
                for ref_type, ref_label in items_custom_labels:
                    safe_label_info = recomposer.make_safe_label('ref', ref_type, ref_label)
                    s += r'\label{' + safe_label_info['safe_label'] + '}'

            s += recomposer.recompose_pure_latex( item_content_nodelist )["latex"]

        s += r'\end{' + node.environmentname + '}'
        return s



class FeatureEnumeration(Feature):
    r"""
    Add support for enumeration and itemization lists, e.g., via LaTeX commands
    ``\begin{enumerate} ... \end{enumerate}`` and ``\begin{itemize}
    ... \end{itemize}``.
    """

    feature_name = 'enumeration'
    feature_title = 'Enumeration and itemization lists'

    feature_default_config = {
        'enumeration_environments': {
            'itemize': {'counter_formatter': ['•','-','▸'],},
            # uses default formatters 1., 2., ... incl. nested (i) etc.
            'enumerate': {'counter_formatter': None},
        }
    }

    # no managers needed

    DocumentManager = None
    RenderManager = None


    def __init__(self, enumeration_environments=None):
        super().__init__()
        if enumeration_environments is None:
            # This can normally only happen when the feature is instantiated
            # directly.  When the feature is instantiated via config, the value
            # should already be merged in by the config merger.
            enumeration_environments = \
                self.feature_default_config.get('enumeration_environments', {})
        self.enumeration_environments = enumeration_environments

    def add_latex_context_definitions(self):
        return dict(
            environments=[
                Enumeration(
                    environmentname=envname,
                    counter_formatter=envinfo['counter_formatter'],
                    annotations=[envname],
                )
                for envname, envinfo in dict(self.enumeration_environments).items()
            ],
        )


# ------------------------------------------------

FeatureClass = FeatureEnumeration

