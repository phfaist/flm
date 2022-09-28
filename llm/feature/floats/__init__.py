import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import ParsedArgumentsInfo, LatexWalkerParseError
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import (
    LatexEnvironmentBodyContentsParser,
    MacroSpec,
    ParsingStateDeltaExtendLatexContextDb,
)

from ...llmenvironment import LLMArgumentSpec
from ...llmspecinfo import LLMEnvironmentSpecBase
from ... import fmthelpers

from .._base import Feature
from ..graphics import SimpleIncludeGraphicsMacro
from ..cells import CellsEnvironment



# ------------------------------------------------------------------------------

class FloatContentHandlerBase:

    def float_content_set_extra_definitions(self, extend_latex_context):
        pass

    def float_handle_content_nodes(self, float_node, content_nodes):
        # subclasses can choose to verify the float's content nodes to ensure
        # that it only contains prescribed content nodes (e.g., only a single
        # \includegraphics node)
        #
        # Return a (possibly post-processed) node list to use in place of
        # content_nodes if the nodes were accepted; raise a
        # LatexWalkerParseError if the nodes were not accepted (unacceptable
        # macros, etc.)
        raise RuntimeError(
            f"This method needs to be reimplemented in subclasses!"
        )


class FloatContentAnyContent(FloatContentHandlerBase):

    def float_handle_content_nodes(self, float_node, content_nodes):
        # allow all content --
        return content_nodes


class FloatContentIncludeGraphics(FloatContentHandlerBase):

    def float_content_set_extra_definitions(self, extend_latex_context):
        extend_latex_context['macros'].append( 
            SimpleIncludeGraphicsMacro(macroname='includegraphics')
        )

    def float_handle_content_nodes(self, float_node, content_nodes):

        content_nodes = content_nodes.latex_walker.filter_whitespace_comments_nodes(
            content_nodes
        )

        if len(content_nodes) == 1:
            node = content_nodes[0]
            if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
               and node.macroname == 'includegraphics':
                # all good!
                return content_nodes

        raise LatexWalkerParseError(
            f"expected exactly one \\includegraphics command",
            pos=content_nodes.pos
        )


class FloatContentCells(FloatContentHandlerBase):

    def float_content_set_extra_definitions(self, extend_latex_context):
        extend_latex_context['environments'].append( 
            CellsEnvironment()
        )

    def float_handle_content_nodes(self, float_node, content_nodes):

        content_nodes = content_nodes.latex_walker.filter_whitespace_comments_nodes(
            content_nodes
        )

        if len(content_nodes) == 1:
            node = content_nodes[0]
            if node.isNodeType(latexnodes_nodes.LatexEnvironmentNode) \
               and node.environmentname == 'cells':
                # all good!
                return content_nodes

        raise LatexWalkerParseError(
            f"expected exactly one "
            f"\\begin{'{'}cells{'}'}...\\end{'{'}cells{'}'} environment",
            pos=content_nodes.pos
        )


available_content_handlers = {
    'any': FloatContentAnyContent,
    'includegraphics': FloatContentIncludeGraphics,
    'cells': FloatContentCells,
}



def _make_content_handler(c):
    if isinstance(c, FloatContentHandlerBase):
        return c
    if isinstance(c, str):
        c = { 'name': c }
    if c['name'] in available_content_handlers:
        return available_content_handlers[c['name']](**c.get('config', {}))
    raise ValueError(f"Invalid float handler specification: {repr(c)}")

# ------------------------------------------------------------------------------

class FloatEnvironment(LLMEnvironmentSpecBase):

    is_block_level = True

    float_content_is_block_level = True

    float_content_render_at_environment_node_location = True
    r"""
    By default (`True`), the float will be rendered exactly where the float
    environment was positioned (making it actually not a float).  If you set
    `False` here, the float is not rendered, and it is up to you (a custom float
    feature manager, for instance), to actually render the float somewhere.
    """

    allowed_in_standalone_mode = False

    def __init__(self, float_type, content_handlers=None):
        super().__init__(
            environmentname=float_type,
            arguments_spec_list=[],
        )
        if content_handlers is None:
            content_handlers = ['includegraphics', 'cells']
        content_handlers = [
            _make_content_handler(c)
            for c in content_handlers
        ]
        self.float_type = float_type
        self.content_handlers = content_handlers

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        extend_latex_context = dict(
            macros=[
                MacroSpec('label', arguments_spec_list=[
                    LLMArgumentSpec(
                        parser=latexnodes_parsers.LatexCharsGroupParser(
                            delimiters=('{','}'),
                        ),
                        argname='label',
                    ),
                ]),
                MacroSpec('caption', arguments_spec_list=[
                    LLMArgumentSpec(
                        '{',
                        argname='captiontext',
                    ),
                ])
            ],
            environments=[],
            specials=[]
        )

        for content_handler in self.content_handlers:
            content_handler.float_content_set_extra_definitions(extend_latex_context)

        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=extend_latex_context,
                set_attributes=dict(is_block_level=self.float_content_is_block_level),
            )
        )

    def postprocess_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        #logger.debug("finalizing float node: node = %r", node)

        # find and register child nodes
        node.llm_float_label = dict(ref_label_prefix=None, ref_label=None, label_node=None)
        node.llm_float_caption = dict(caption_nodelist=None, caption_node=None)

        float_content_nodes = []

        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode):

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

                    if ref_label_prefix != self.float_type:
                        raise LatexWalkerParseError(
                            f"Float's \\label{'{'}...{'}'} must have the "
                            f"prefix ‘{self.float_type}:’",
                            pos=n.pos,
                        )

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

            # keep this node as part of the float's content
            float_content_nodes.append(n)

        float_content_nodes = node.latex_walker.make_nodelist(
            float_content_nodes,
            parsing_state=node.nodelist.parsing_state,
        )

        # call handlers for the float's content
        errors = []
        final_content_nodes = None
        for content_handler in self.content_handlers:
            try:
                final_content_nodes = content_handler.float_handle_content_nodes(
                    node, float_content_nodes
                )
            except LatexWalkerParseError as e:
                errors.append(f"*** {content_handler.__class__.__name__} error: {str(e)}")
                pass # ignore error

        if final_content_nodes is None:
            # no content handler accepted this float's content node list
            raise LatexWalkerParseError(
                f"Invalid {self.float_type} contents! The following content handler(s) "
                f"were unable to parse the float's content [other than possible "
                f"\\caption and \\label commands]:\n"
                + "\n".join(errors),
                pos=node.pos
            )

        node.llm_float_content_nodelist = final_content_nodes

        #logger.debug("llm_float_content_nodelist = %r", node.llm_float_content_nodelist)

        return node


    def render(self, node, render_context):

        floats_mgr = render_context.feature_render_manager('floats')

        logger.debug(f"Rendering float: {node=}")

        ref_label_prefix = node.llm_float_label['ref_label_prefix']
        ref_label = node.llm_float_label['ref_label']

        numbered = True
        if ref_label_prefix is None and ref_label is None:
            # \label{} omitted -> no numbering
            numbered = False

        float_instance = floats_mgr.register_float(
            node=node,
            float_type=self.float_type,
            numbered=numbered,
            ref_label_prefix=ref_label_prefix,
            ref_label=ref_label,
            caption_nodelist=node.llm_float_caption['caption_nodelist'],
            content_nodelist=node.llm_float_content_nodelist,
        )

        # note: do NOT store the float instance onto the `node` object, because
        # the node object might be shared and re-used between multiple documents!

        #logger.debug("Registered float: %r", float_instance)

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

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )


# ------------------------------------------------


class FloatType:
    def __init__(self, float_type, float_caption_name=None, counter_formatter=None,
                 content_handlers=None):
        r"""


        ..... float_caption_name can be a string ("Figure") or a callable
        (`float_caption_name(number, fmt_value_llm_text)`) .......
        """
        super().__init__()

        self.float_type = float_type

        if float_caption_name is None:
            float_caption_name = float_type

        self.float_caption_name = float_caption_name

        if counter_formatter is None:
            counter_formatter = 'arabic'
        if not callable(counter_formatter):
            counter_formatter = fmthelpers.standard_counter_formatters[counter_formatter]
        self.counter_formatter = counter_formatter

        self.content_handlers = content_handlers

        self._fields = ('float_type', 'float_caption_name',
                        'counter_formatter', 'content_handlers')

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )


# ------------------------------------------------------------------------------


class FeatureFloats(Feature):

    feature_name = 'floats'

    def __init__(self, float_types=None):
        super().__init__()
        if float_types is None:
            float_types = [
                FloatType('figure', 'Figure', 'arabic', ['includegraphics']),
                FloatType('table', 'Table', 'arabic',  ['cells', 'includegraphics']),
            ]
        def _mkfloattypeobj(x):
            if isinstance(x, FloatType):
                return x
            return FloatType(**x)
        self.float_types_list = [ _mkfloattypeobj(x) for x in float_types ]
        self.float_types = {
            ft.float_type: ft
            for ft in self.float_types_list
        }

    def make_float_environment_spec(self, float_type):
        return FloatEnvironment(
            float_type,
            content_handlers=self.float_types[float_type].content_handlers
        )

    def add_latex_context_definitions(self):
        environments = []
        for float_type, ftinfo in self.float_types.items():
            environments.append(
                self.make_float_environment_spec(float_type)
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
            self.float_instances = {} # node_id -> float_instance
        
        def register_float(
                self,
                float_type,
                *,
                node,
                numbered=True,
                ref_label_prefix=None,
                ref_label=None,
                caption_nodelist=None,
                content_nodelist=None,
        ):

            node_id = self.get_node_id(node)

            if node_id in self.float_instances:
                # this happens on second pass when rendering in two passes.
                return self.float_instances[node_id]

            float_type_info = self.feature.float_types[float_type]

            logger.debug("registering float ... ")

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

            #logger.debug("registering float ... float instance is = %r", float_instance)

            self.floats[float_type].append( float_instance )

            # register also the reference in the 'refs' manager, if applicable
            if number is not None and self.render_context.supports_feature('refs'):

                #logger.debug("maybe registering numbered float in refs manager")

                refs_mgr = self.render_context.feature_render_manager('refs')

                if ref_label_prefix is not None and ref_label is not None:

                    assert( ref_label_prefix == float_type )

                    formatted_ref_llm_text = self.get_formatted_ref_llm_text(float_instance)

                    #logger.debug(f"registering float, {number=} {ref_label_prefix=} "
                    #             f"{ref_label=}, {formatted_ref_llm_text=}")

                    refs_mgr.register_reference(
                        ref_label_prefix,
                        ref_label,
                        node=node,
                        formatted_ref_llm_text=formatted_ref_llm_text,
                        target_href=f'#{target_id}',
                    )

            self.float_instances[node_id] = float_instance
            return float_instance

        def get_formatted_ref_llm_text(self, float_instance):
            return (
                float_instance.float_type_info.float_caption_name + '~' +
                float_instance.formatted_counter_value_llm.llm_text
            )





# ------------------------------------------------------------------------------



# class FloatEnvironmentWithTabularDataOrIncludeGraphics(FloatEnvironment):

#     def float_content_set_extra_definitions(self, extend_latex_context):
#         extend_latex_context['macros'].append( 
#             SimpleIncludeGraphicsMacro(macroname='includegraphics'),
#             TabularEnvironment(environmentname='tabular')
#         )


#     def _check_single_main_content_node(self, float_node, content_node):
#         if hasattr(float_node, 'llm_main_content_node') and \
#            float_node.llm_main_content_node is not None:
#             raise LatexWalkerParseError(
#                 f"{self.float_type} should contain exactly one "
#                 f"\\includegraphics command OR "
#                 f"\\begin{'{'}tabular{'}'}..\\end{'{'}tabular{'}'} environment "
#                 f"apart from possible "
#                 f"\\caption and \\label commands",
#                 pos=content_node.pos
#             )

#     def finalize_handle_content_node(self, float_node, content_node):
#         if content_node.isNodeType(latexnodes_nodes.LatexMacroNode) \
#            and content_node.macroname == 'includegraphics':
#             # \includegraphics command -- okay!
#             self._check_single_main_content_node(float_node, content_node)
#             float_node.llm_main_content_node = content_node
#             return True

#         if content_node.isNodeType(latexnodes_nodes.LatexEnvironmentNode) \
#            and content_node.environmentname == 'tabular':
#             # main table content -- okay!
#             self._check_single_main_content_node(float_node, content_node)
#             float_node.llm_main_content_node = content_node
#             return True

#         if content_node.isNodeType(latexnodes_nodes.LatexCharsNode) \
#            and len(content_node.chars.strip()) == 0:
#             # skip whitespace without errors
#             return False

#         if content_node.isNodeType(latexnodes_nodes.LatexCommentNode):
#             # skip comments without errors
#             return False

#         raise LatexWalkerParseError(
#             f"{self.float_type} cannot contain content other than "
#             f"\\includegraphics, \\caption and \\label commands",
#             pos=content_node.pos
#         )






# ------------------

# class FeatureFloatsIncludeGraphicsOnly(FeatureFloats):

#     make_float_environment_spec = FloatEnvironmentIncludeGraphicsOnly