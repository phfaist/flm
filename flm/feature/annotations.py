r"""
Annotation macros for highlighting text and adding inline comments.

Each annotation macro supports two forms: highlighted text (``\mac{...}`` or
``\mac ... \endmac``) and inline comments (``\mac[...]``).  Multiple
annotators can be defined, each with their own initials and color index.
When ``hide_all_annotations`` is set, highlights show only the underlying
text and comments are suppressed.
"""

from pylatexenc.latexnodes import (
    # LatexWalkerParseError,
    ParsedArgumentsInfo,
)
from pylatexenc.latexnodes.nodes import LatexGroupNode
from pylatexenc.latexnodes.parsers import (
    LatexParserBase,
    LatexGeneralNodesParser,
    LatexDelimitedGroupParser,
)

from .._typing_helpers import (
    TypeCallableSpecBase, Sequence, TypeDictWithLatexContextDefinitions,
    Mapping
)

from ..flmenvironment import (
    FLMArgumentSpec,
    FLMParsingStateDeltaSetBlockLevel,
)
from ..flmspecinfo import (
    FLMMacroSpecBase,
)

from ._base import Feature, FeatureRenderManagerBase


class AnnotationArgumentParser(LatexParserBase):
    r"""Custom argument parser for annotation macros.

    Dispatches based on the opening delimiter:

    * ``{...}`` -- highlighted text
    * ``[...]`` -- inline comment
    * bare text until ``\end<macroname>`` -- highlighted text (alternative form)

    :param macroname: The annotation macro name, used to derive the
        ``\end<macroname>`` stop condition.
    """
    def __init__(self, macroname):
        super().__init__()
        
        self.macroname = macroname
        self.endmacroname = 'end'+self.macroname

        self.parser_arg_highlight = LatexDelimitedGroupParser(
            delimiters=('{','}'),
        )
        self.parser_arg_comment = LatexDelimitedGroupParser(
            delimiters=('[',']'),
        )
        self.parser_endmacro_highlight = LatexGeneralNodesParser(
            stop_token_condition= lambda tok: (
                tok.tok == 'macro' and tok.arg == self.endmacroname
            ),
            require_stop_condition_met=True,
        )

    def parse(self, latex_walker, token_reader, parsing_state, **kwargs):

        parsing_state_w_brackets = parsing_state.sub_context(
            latex_group_delimiters=[('{','}'), ('[', ']')],
        )
        tok = token_reader.peek_token( parsing_state=parsing_state_w_brackets )

        if tok.tok == 'brace_open':
            if tok.arg == '{':
                # standard highlighted text, read a standard delimited group
                return self.parser_arg_highlight.parse(
                    latex_walker, token_reader, parsing_state_w_brackets, **kwargs
                )
            if tok.arg == '[':
                # comment text, read a group delimited by [...]
                return self.parser_arg_comment.parse(
                    latex_walker, token_reader, parsing_state_w_brackets, **kwargs
                )
            raise ValueError(f"Unexpected {tok.arg=}!")

        nodelist, parsing_state_delta = self.parser_endmacro_highlight.parse(
            latex_walker, token_reader, parsing_state, **kwargs
        )
        # consume the final token "\endmacro"
        token_reader.next_token(parsing_state)
        # always return a LatexGroupNode
        groupnode = latex_walker.make_node(
            LatexGroupNode,
            nodelist=nodelist,
            delimiters=('',''),
            pos=nodelist.pos,
            pos_end=nodelist.pos_end,
            parsing_state=parsing_state,
        )
        return groupnode, parsing_state_delta



class AnnotationMacro(FLMMacroSpecBase):
    r"""Spec info for a single annotation macro (e.g. ``\abc``).

    Supports two syntactic forms: ``\abc{highlighted text}`` (or the
    ``\abc ... \endabc`` variant) for highlighting, and ``\abc[comment]``
    for inline comments.  Rendering delegates to
    ``render_annotation_highlight`` or ``render_annotation_comment`` on the
    fragment renderer.
    """

    allowed_in_standalone_mode = True

    # internal; used when truncating fragments to a certain number of characters
    # to determine where to look for text to truncate within formatting commands
    # (see fragment.truncate_to())
    _flm_main_text_argument = 'text'

    def __init__(self, macroname : str, initials : str|None = None, color_index : int = 0):
        r"""
        :param macroname: The macro name to register (e.g. ``'abc'``).
        :param initials: Optional author initials displayed alongside the
            annotation.
        :param color_index: Zero-based index selecting the annotation color,
            incremented automatically for each annotator defined in
            :py:class:`FeatureAnnotations`.
        """
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                FLMArgumentSpec(
                    argname='text',
                    parser=AnnotationArgumentParser(macroname=macroname),
                    # don't use is_block_level=None, use parsing_state_delta=,
                    # otherwise the parsing state delta isn't registered.
                    parsing_state_delta=FLMParsingStateDeltaSetBlockLevel(
                        is_block_level=None, # both block or inline are good
                    )
                )
            ],
        )
        self.initials = initials
        self.color_index = color_index

    _fields = ('macroname',)

    def get_flm_doc(self):
        return (
            f"Annotation macro \\verbcode+\\{self.macroname}+"
        )

    def render(self, node, render_context):

        rdr_mgr = render_context.feature_render_manager('annotations')
        if rdr_mgr.hide_all_annotations:
            if node.flmarg_text_delimiters[0] == '[':
                return render_context.fragment_renderer.render_nothing(render_context)
            return render_context.fragment_renderer.render_nodelist(
                node.flmarg_text_nodelist,
                render_context,
                is_block_level=node.flm_is_block_level,
            )

        if node.flmarg_text_delimiters[0] == '[':
            return render_context.fragment_renderer.render_annotation_comment(
                node.flmarg_text_nodelist,
                render_context=render_context,
                color_index=self.color_index,
                initials=self.initials,
                is_block_level=node.flm_is_block_level,
            )

        return render_context.fragment_renderer.render_annotation_highlight(
            node.flmarg_text_nodelist,
            render_context=render_context,
            color_index=self.color_index,
            initials=self.initials,
            is_block_level=node.flm_is_block_level,
        )


    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('text',) ,
        )

        text_arg = node_args['text']

        # note, text_arg should in all cases have a LatexGroupNode as argument
        # node.  We should simply check its delimiters to find out whether it's
        # a higlight or a comment.

        node.flmarg_text_delimiters = text_arg.argument_node_object.delimiters
        text_nodelist = text_arg.get_content_nodelist()
        node.flmarg_text_nodelist = text_nodelist

        nodes_finalizer = node.latex_walker.flm_environment.nodes_finalizer
        node.flm_is_block_level = nodes_finalizer.infer_is_block_level_nodelist(
            text_nodelist
        )


# Transcrypt does not need the type definition because it strips type
# annotations.  Provide it in python.
### BEGIN_FLM_PYTHON_TYPING
from typing import TypedDict
class TypeAnnotationMacroDef(TypedDict, total=False):
    initials : str|None
### END_FLM_PYTHON_TYPING



class FeatureAnnotations(Feature):
    r"""
    Feature that registers custom annotation macros for multiple authors.
    Each macro supports text highlighting and inline comment syntax, with
    an optional ``hide_all_annotations`` mode to strip annotations from output.
    """

    feature_name = 'annotations'
    feature_title = 'Support for simple annotations in text (comments and highlighted text)'

    feature_flm_doc = r"""
    Supports the definition of custom macros to mark annotations from multiple
    authors.

    Say we define an annotation macro '\abc'.  There are two types of
    annotations each macro such as '\abc' supports:

       \abc{This piece of text is highlighted}
       \abc This piece of text is highlighted, alternative syntax \endabc

       \abc[This is an inline comment.]

    Highlighted text refers to pieces of the document that are highlighted in a
    particular color; inline comments are any remarks that concern the
    surrounding text and which would typically be deleted before completing the
    document.
    """

    def __init__(
            self,
            macrodefs : Mapping[str, TypeAnnotationMacroDef]|None = None,
            hide_all_annotations : bool = False,
    ):
        r"""
        :param macrodefs: Mapping of macro names to annotation definition
            dicts.  Each dict may contain an ``'initials'`` key with the
            annotator's initials.  A unique ``color_index`` is assigned
            automatically in iteration order.
        :param hide_all_annotations: When *True*, highlights render only the
            underlying text and inline comments are suppressed entirely.
        """
        super().__init__()
        self.macrodefs = dict(macrodefs or {})
        self.hide_all_annotations = hide_all_annotations
        

    def add_latex_context_definitions(self) -> TypeDictWithLatexContextDefinitions:
        macros : Sequence[TypeCallableSpecBase] = []
        
        color_index = 0

        for macroname, annotdef in self.macrodefs.items():
            macros.append(
                AnnotationMacro(
                    macroname=macroname,
                    color_index=color_index,
                    **annotdef,
                )
            )
            color_index += 1

        return {
            'macros': macros
        }

    class RenderManager(FeatureRenderManagerBase):
        def initialize(self, hide_all_annotations : bool|None = None):
            if hide_all_annotations is not None:
                self.hide_all_annotations = hide_all_annotations
            else:
                self.hide_all_annotations = self.feature.hide_all_annotations



FeatureClass = FeatureAnnotations
