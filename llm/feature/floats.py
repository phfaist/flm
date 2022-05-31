import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import ParsedArgumentsInfo, LatexWalkerParseError
from pylatexenc.latexnodes.nodes import *
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import (
    LatexEnvironmentBodyContentsParser,
    MacroSpec,
    ParsingStateDeltaExtendLatexContextDb,
)

from ..llmenvironment import make_arg_spec
from ..llmspecinfo import LLMSpecInfo, LLMMacroSpec, LLMEnvironmentSpec
from .. import fmthelpers

from ._base import Feature
from .graphics import LLMIncludeGraphicsMacroSpec


# ------------------------------------------------------------------------------


class FloatEnvironmentSpecInfo(LLMSpecInfo):

    is_block_level = True

    float_content_is_block_level = True

    float_content_render_at_environment_node_location = True
    r"""
    By default (`True`), the float will be rendered exactly where the float
    environment was positioned (making it actually not a float).  If you set
    `False` here, the float is not rendered, and it is up to you (a custom float
    feature manager, for instance), to actually render the float somewhere.
    """

    def __init__(self, float_type):
        super().__init__()
        self.float_type = float_type

    def float_content_set_extra_definitions(self, extend_latex_context):
        pass

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        extend_latex_context = dict(
            macros=[
                MacroSpec('label', arguments_spec_list=[
                    make_arg_spec(
                        parser=latexnodes_parsers.LatexCharsGroupParser(
                            delimiters=('{','}'),
                        ),
                        argname='label',
                    ),
                ]),
                MacroSpec('caption', arguments_spec_list=[
                    make_arg_spec(
                        parser=latexnodes_parsers.LatexCharsGroupParser(
                            delimiters=('{','}'),
                        ),
                        argname='captiontext',
                    ),
                ])
            ],
            environments=[],
            specials=[]
        )
        self.float_content_set_extra_definitions(extend_latex_context)
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=extend_latex_context,
                set_attributes=dict(is_block_level=self.float_content_is_block_level),
            )
        )

    def finalize_handle_content_node(self, float_node, content_node):
        # subclasses can choose to verify the float's content to ensure that it
        # only contains prescribed content nodes (e.g., only a single
        # \includegraphics node)
        #
        # Return True if the node should be added to the main float contents;
        # False if the node is to be ignored, or raise a LatexWalkerParseError
        # if the node shouldn't appear here.
        return True

    def finalize_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        logger.debug("finalizing math environment node: node = %r", node)

        # find and register child nodes
        node.llm_float_label = dict(ref_label_prefix=None, ref_label=None, label_node=None)
        node.llm_float_caption = dict(caption_nodelist=None, caption_node=None)
        float_content_items = []
        for n in node.nodelist:
            if n.isNodeType(LatexMacroNode):

                if n.macroname == 'label':
                    # this is the figure label.
                    node.llm_float_label['label_node'] = n
                    ref_label_node_args = \
                        ParsedArgumentsInfo(node=n).get_all_arguments_info(
                            ('label',),
                        )
                    ref_label_full = ref_label_node_args['label'].get_content_as_chars()
                    if ':' in ref_label_full:
                        ref_label_prefix, ref_label = ref_label_full.split(':', 1)
                    else:
                        ref_label_prefix, ref_label = None, ref_label_full

                    if ref_label_prefix != self.float_type:
                        raise LatexWalkerParseError(
                            f"{self.float_type} label must start with the prefix "
                            f"‘{self.float_type}:...’ (got ‘{ref_label_full}’)",
                            pos=n.pos,
                        )

                    node.llm_float_label['ref_label_prefix'] = ref_label_prefix
                    node.llm_float_label['ref_label'] = ref_label
                    continue

                if n.macroname == 'caption':
                    # this is the main figure caption
                    node.llm_float_caption['caption_node'] = n
                    ref_caption_node_args = \
                        ParsedArgumentsInfo(node=n).get_all_arguments_info(
                            ('captiontext',),
                        )
                    ref_caption_nodelist = \
                        ref_caption_node_args['captiontext'].get_content_nodelist()
                    node.llm_float_caption['caption_nodelist'] = ref_caption_nodelist
                    continue
                
            # handle this node
            include_in_content = self.finalize_handle_content_node(node, n)
            if include_in_content:
                float_content_items.append(n)

        node.llm_float_content_nodelist = \
            node.latex_walker.make_nodelist(
                float_content_items,
                parsing_state=node.nodelist.parsing_state,
            )

        logger.debug("llm_float_content_nodelist = %r", node.llm_float_content_nodelist)

        return node


    def render(self, node, render_context):

        floats_mgr = render_context.feature_render_manager('floats')

        if hasattr(node, 'llm_float_instance'):
            # happens in two-pass rendering schemes. Don't register the float a
            # second time!
            float_instance = node.llm_float_instance
        else:
            float_instance = floats_mgr.register_float(
                float_type=self.float_type,
                ref_label_prefix=node.llm_float_label['ref_label_prefix'],
                ref_label=node.llm_float_label['ref_label'],
                caption_nodelist=node.llm_float_caption['caption_nodelist'],
                content_nodelist=node.llm_float_content_nodelist,
            )
            node.llm_float_instance = float_instance

        logger.debug("Rendering float: %r", float_instance)

        if self.float_content_render_at_environment_node_location:
            return self.render_float(float_instance, node, render_context)

        return render_context.fragment_renderer.render_nothing(
            annotations=['float-placeholder',
                         f"float-{self.float_type}-placeholder"]
        )


    def render_float(self, float_instance, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        return fragment_renderer.render_float( float_instance, render_context )



# ------------------------------------------------------------------------------


class FloatInstance:
    def __init__(self,
                 *,
                 float_type=None,
                 float_type_info=None,
                 number=None,
                 formatted_counter_value_llm=None,
                 ref_label_prefix=None,
                 ref_label=None,
                 target_id=None,
                 caption_nodelist=None,
                 content_nodelist=None,
                 ):
        super().__init__()
        self.float_type = float_type
        self.float_type_info = float_type_info
        self.number = number
        self.formatted_counter_value_llm = formatted_counter_value_llm
        self.ref_label_prefix = ref_label_prefix
        self.ref_label = ref_label
        self.target_id = target_id
        self.caption_nodelist = caption_nodelist
        self.content_nodelist = content_nodelist
        self._fields = ('float_type', 'float_type_info', 'number',
                        'formatted_counter_value_llm',
                        'ref_label_prefix', 'ref_label', 'target_id',
                        'caption_nodelist', 'content_nodelist',)

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )


# ------------------------------------------------


class FloatType:
    def __init__(self, float_type, float_caption_name, counter_formatter,):
        r"""


        ..... float_caption_name can be a string ("Figure") or a callable
        (`float_caption_name(number, fmt_value_llm_text)`) .......
        """
        super().__init__()

        self.float_type = float_type
        self.float_caption_name = float_caption_name

        if not callable(counter_formatter):
            counter_formatter = fmthelpers.standard_counter_formatters[counter_formatter]
        self.counter_formatter = counter_formatter

        self._fields = ('float_type', 'float_caption_name', 'counter_formatter',)

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )

# -----

class FeatureFloats(Feature):

    feature_name = 'floats'

    def __init__(self, float_types=None):
        super().__init__()
        if float_types is None:
            float_types = [
                FloatType('figure', 'Figure', 'arabic'),
                FloatType('table', 'Table', 'arabic'),
            ]
        self.float_types_list = float_types
        self.float_types = {
            ft.float_type: ft
            for ft in self.float_types_list
        }

    make_float_environment_spec_info = FloatEnvironmentSpecInfo

    def add_latex_context_definitions(self):
        environments = []
        for float_type, ftinfo in self.float_types.items():
            environments.append(
                LLMEnvironmentSpec(
                    float_type, # the environment name is identical to float_type
                    arguments_spec_list=[],
                    llm_specinfo=self.make_float_environment_spec_info(float_type),
                )
            )
        return dict(environments=environments)


    class RenderManager(Feature.RenderManager):

        def initialize(self):
            self.floats = {
                float_type: []
                for float_type, ftinfo in self.feature.float_types.items()
            }
            self.float_counters = {
                float_type: 1
                for float_type, ftinfo in self.feature.float_types.items()
            }
        
        def register_float(
                self,
                float_type,
                *,
                numbered=True,
                ref_label_prefix=None,
                ref_label=None,
                caption_nodelist=None,
                content_nodelist=None,
        ):

            float_type_info = self.feature.float_types[float_type]

            if numbered:
                fmtcounter = float_type_info.counter_formatter
                number = self.float_counters[float_type]
                self.float_counters[float_type] += 1

                fmtvalue_llm_text = fmtcounter(number)
                fmtvalue_llm = self.render_context.doc.environment.make_fragment(
                    fmtvalue_llm_text,
                    is_block_level=False,
                    what=f"{float_type} {number} counter value",
                )
            else:
                number = None
                fmtvalue_llm = None

            target_id = None
            if number is not None:
                target_id = f"{float_type}-{number}"

            float_instance = FloatInstance(
                float_type=float_type,
                float_type_info=float_type_info,
                number=number,
                formatted_counter_value_llm=fmtvalue_llm,
                ref_label_prefix=ref_label_prefix,
                ref_label=ref_label,
                target_id=target_id,
                caption_nodelist=caption_nodelist,
                content_nodelist=content_nodelist,
            )

            self.floats[float_type].append( float_instance )

            # register also the reference in the 'refs' manager, if applicable
            if number is not None and self.render_context.supports_feature('refs'):

                refs_mgr = self.render_context.feature_render_manager('refs')

                assert( ref_label_prefix == float_type )

                formatted_ref_llm_text = self.get_formatted_ref_llm_text(float_instance)

                refs_mgr.register_reference(
                    ref_label_prefix,
                    ref_label,
                    formatted_ref_llm_text=formatted_ref_llm_text,
                    target_href=f'#{target_id}',
                )

            return float_instance

        def get_formatted_ref_llm_text(self, float_instance):
            return (
                float_instance.float_type_info.float_caption_name + '~' +
                float_instance.formatted_counter_value_llm.llm_text
            )



# ------------------------------------------------


class FloatEnvironmentIncludeGraphicsOnlySpecInfo(FloatEnvironmentSpecInfo):

    def float_content_set_extra_definitions(self, extend_latex_context):
        extend_latex_context['macros'].append( LLMIncludeGraphicsMacroSpec() )

    def finalize_handle_content_node(self, float_node, content_node):
        if content_node.isNodeType(LatexMacroNode) \
           and content_node.macroname == 'includegraphics':
            # \includegraphics command
            if getattr(float_node, 'llm_includegraphics_node', None) is not None:
                raise LatexWalkerParseError(
                    f"{self.float_type} should contain exactly one "
                    f"\\includegraphics command apart from possible "
                    f"\\caption and \\label commands",
                    pos=content_node.pos
                )
            float_node.llm_includegraphics_node = content_node
            return True

        if content_node.isNodeType(LatexCharsNode) \
           and len(content_node.chars.strip()) == 0:
            # skip whitespace without errors
            return False

        if content_node.isNodeType(LatexCommentNode):
            # skip comments without errors
            return False

        raise LatexWalkerParseError(
            f"{self.float_type} cannot contain content other than "
            f"\\includegraphics, \\caption and \\label commands",
            pos=content_node.pos
        )




# ------------------

class FeatureFloatsIncludeGraphicsOnly(FeatureFloats):

    make_float_environment_spec_info = FloatEnvironmentIncludeGraphicsOnlySpecInfo