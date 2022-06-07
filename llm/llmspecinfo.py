
import logging
logger = logging.getLogger(__name__)


from pylatexenc import macrospec
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import ParsedArgumentsInfo, LatexWalkerParseError

from .llmenvironment import LLMArgumentSpec


# ------------------------------------------------------------------------------


class LLMSpecInfo:
    r"""
    Class that specifies how to finalize a given parsed node and how to process
    it to render into primitives understood by
    :py:class:`~llm.fragmentrenderer.FragmentRenderer` objects
    """

    delayed_render = False
    r"""
    Whether this node needs to be rendered at the delayed rendering stage, i.e.,
    after a first pass through the document.  This is the case, for instance,
    for ``\ref`` commands etc. for which the entire document needs to have been
    traversed at least once beforehand.  See the delayed render mechanism in the
    documentation for the :py:class:`llmdocument.LLMDocument` class.
    """

    is_block_level = False
    r"""
    If this flag is set to `True`, then elements of this type are always parsed
    as separate block-level elements (e.g., a section heading, an enumeration
    list, etc.)
    """

    is_block_heading = False
    r"""
    If `is_block_level=True` and this flag is also set to `True`, then this
    element *introduces* a new paragraph.  I.e., a block-level/paragraph break
    is introduced immediately before this item.  The present item is itself
    included along with the non-block-level content that follows to form a new
    paragraph.
    """

    is_paragraph_break_marker = False
    r"""
    True if this node's sole purpose is to split paragraphs.  Use this for
    ``\n\n`` breaks or maybe if the user would like to introduce support for
    ``\par`` macros.
    """
    
    allowed_in_restricted_mode = False
    r"""
    Whether or not this node is allowed in *restricted mode*, i.e., whether or
    not this node can be rendered independently of any document object.
    """


    def postprocess_parsed_node(self, node):
        r"""
        Can be overridden to add additional information to node objects.

        You shouldn't change the node object.  You don't have to return it,
        either.
        """
        pass

    def prepare_delayed_render(self, node, render_context):
        r"""
        For items with `delayed_render=True`, this method is called instead of
        render() on the first pass, so that this document item has the
        opportunity to register itself in document feature managers, etc.

        This method is never called if `delayed_render=False`.
        """
        raise RuntimeError("Reimplement me!")

    def render(self, node, render_context):
        r"""
        Produce a final representation of the node, using the given
        `render_context`.
        """
        raise RuntimeError(
            f"Element ‘{node}’ cannot be placed here, render() not reimplemented."
        )


    # ---

    # the following method(s) are not meant to be overridden

    def finalize_node(self, node):
        r"""
        Override this method only if you know what you're doing!
        """
        node.llm_specinfo = self
        self.postprocess_parsed_node(node)
        node.llm_is_block_level = self.is_block_level
        node.llm_is_block_heading = self.is_block_heading
        node.llm_is_paragraph_break_marker = self.is_paragraph_break_marker
        return node
    


# ------------------------------------------------------------------------------


class LLMMacroSpecBase(LLMSpecInfo, macrospec.MacroSpec):
    pass

class LLMEnvironmentSpecBase(LLMSpecInfo, macrospec.EnvironmentSpec):
    pass

class LLMSpecialsSpecBase(LLMSpecInfo, macrospec.SpecialsSpec):
    pass




# ------------------------------------------------------------------------------


class LLMSpecInfoConstantValue(LLMSpecInfo):

    allowed_in_restricted_mode = True

    def __init__(self, *args, value, **kwargs):
        super().__init__(*args, **kwargs)
        self.value = value

    def render(self, node, render_context):
        return render_context.fragment_renderer.render_value(self.value)


class ConstantValueMacro(LLMSpecInfoConstantValue, macrospec.MacroSpec):
    pass
class ConstantValueSpecials(LLMSpecInfoConstantValue, macrospec.SpecialsSpec):
    pass


_text_arg = LLMArgumentSpec('{', argname='text',)


class TextFormatMacro(LLMMacroSpecBase):
    r"""
    The argument `text_formats` is a list of strings, each string is a format
    name to apply.  Format names are inspired from the corresponding canonical
    LaTeX macros that apply them.  They are:

    - `textit`

    - `textbf`

    - (possibly more in the future)
    """

    allowed_in_restricted_mode = True

    def __init__(self, macroname, *, text_formats):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[_text_arg],
        )
        self.text_formats = text_formats

    def render(self, node, render_context):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('text',) ,
        )

        return render_context.fragment_renderer.render_text_format(
            self.text_formats,
            node_args['text'].get_content_nodelist(),
            render_context,
        )


class LLMSpecInfoParagraphBreak(LLMSpecInfo):

    is_block_level = True

    is_paragraph_break_marker = True

    allowed_in_restricted_mode = True
    
    def render(self, node, render_context):
        raise LatexWalkerParseError('Paragraph break is not allowed here', pos=node.pos)


class ParagraphBreakSpecials(LLMSpecInfoParagraphBreak, macrospec.SpecialsSpec):
    pass
class ParagraphBreakMacro(LLMSpecInfoParagraphBreak, macrospec.MacroSpec):
    pass




class LLMSpecInfoError(LLMSpecInfo):

    allowed_in_restricted_mode = True

    def __init__(self, *args, error_msg=None, **kwargs):
        super().__init__(*args, **kwargs)
        self.error_msg = error_msg
    
    def render(self, node, render_context):
        if self.error_msg:
            msg = self.error_msg
        else:
            msg = f"The node ‘{node}’ cannot be placed here."

        raise LatexWalkerParseError(msg, pos=node.pos)


class LLMMacroSpecError(LLMSpecInfoError, macrospec.MacroSpec):
    pass

class LLMEnvironmentSpecError(LLMSpecInfoError, macrospec.EnvironmentSpec):
    pass

class LLMSpecialsSpecError(LLMSpecInfoError, macrospec.SpecialsSpec):
    pass




class HeadingMacro(LLMMacroSpecBase):

    is_block_level = True

    allowed_in_restricted_mode = True

    def __init__(self, macroname, *, heading_level=1, inline_heading=False):
        r"""
        Heading level is to be coordinated with fragment renderer and LLM
        environment/context commands; for example `heading_level=1..6` with
        commands ``\section`` ... ``\subsubparagraph``
        """
        super().__init__(
            macroname,
            arguments_spec_list=[ _text_arg ],
        )
        self.heading_level = heading_level
        self.inline_heading = inline_heading
        # llmspec API -
        self.is_block_heading = self.inline_heading

    def render(self, node, render_context):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('text',) ,
        )

        return render_context.fragment_renderer.render_heading(
            node_args['text'].get_content_nodelist(),
            render_context=render_context,
            heading_level=self.heading_level,
        )


_href_arg_specs = {
    'target_href': LLMArgumentSpec(
        parser=latexnodes_parsers.LatexDelimitedVerbatimParser( ('{','}') ),
        argname='target_href'
    ),
    'display_text': LLMArgumentSpec('{', argname='display_text',),
}


class HrefHyperlinkMacro(LLMMacroSpecBase):

    allowed_in_restricted_mode = True

    def __init__(
            self,
            macroname,
            *,
            command_arguments=('target_href', 'display_text',),
            ref_type='href',
    ):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=self._get_arguments_spec_list(command_arguments)
        )
        self.command_arguments = [ c.strip('[]') for c in command_arguments ]
        self.ref_type = ref_type

    @classmethod
    def _get_arguments_spec_list(cls, command_arguments):
        return [
            _href_arg_specs[cmdarg]
            for cmdarg in command_arguments
        ]

    @classmethod
    def pretty_url(cls, target_href):
        url_display = str(target_href)
        for prefix in ('http://', 'https://'):
            if url_display.startswith(prefix):
                url_display = url_display[len(prefix):]
                break
        url_display = url_display.rstrip('/#?')
        return url_display


    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            self.command_arguments,
        )
        
        target_href = None
        display_text_nodelist = None

        if 'target_href' in node_args:
            target_href = node_args['target_href'].get_content_as_chars()
        if 'display_text' in node_args:
            display_text_nodelist = node_args['display_text'].get_content_nodelist()

        node.llm_href_info = {
            'target_href': target_href,
            'display_text_nodelist': display_text_nodelist
        }

    def render(self, node, render_context):

        target_href = node.llm_href_info['target_href']
        display_text_nodelist = node.llm_href_info['display_text_nodelist']

        # show URL by default
        if display_text_nodelist is None:
            display_text_nodelist = node.latex_walker.make_nodelist(
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        parsing_state=node.parsing_state,
                        chars=self.pretty_url(target_href),
                        pos=node.pos,
                        pos_end=node.pos,
                    )
                ],
                parsing_state=node.parsing_state
            )

        return render_context.fragment_renderer.render_link(
            self.ref_type,
            target_href,
            display_text_nodelist,
            render_context,
        )


class VerbatimSpecInfo(LLMSpecInfo):

    allowed_in_restricted_mode = True

    r"""
    Wraps an argument, or an environment body, as verbatim content.

    The `annotation` is basically a HTML class name to apply to the block of
    content.  Use this for instance to separate out math content, etc.
    """
    def __init__(self, *args,
                 annotations=None,
                 include_environment_begin_end=False,
                 **kwargs):
        super().__init__(*args, **kwargs)
        self.annotations = annotations
        self.include_environment_begin_end = include_environment_begin_end

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        r"""
        Used for environments only.
        """
        assert( token.tok == 'begin_environment' )
        environment_name = token.arg
        return latexnodes_parsers.LatexVerbatimEnvironmentContentsParser(
            environment_name=environment_name
        )


    def render(self, node, render_context):

        environment_node_name = None

        if node.isNodeType(latexnodes_nodes.LatexEnvironmentNode):

            environment_node_name = node.environmentname

            if self.include_environment_begin_end:
                verbatim_contents = node.latex_verbatim()
            else:
                # it's an environment node, and we only want to render the contents of
                # the environment.
                verbatim_contents = node.nodelist.latex_verbatim()
        else:
            verbatim_contents = node.latex_verbatim()
        
        annotations = self.annotations or []
        if environment_node_name is not None:
            annotations.append(environment_node_name)

        return render_context.fragment_renderer.render_verbatim(
            verbatim_contents,
            annotations=annotations,
        )

class VerbatimMacro(VerbatimSpecInfo, macrospec.MacroSpec):
    def __init__(self, macroname,
                 verbatim_delimiters=None,
                 **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                latexnodes_parsers.LatexDelimitedVerbatimParser(
                    delimiters=verbatim_delimiters,
                ),
            ],
            **kwargs
        )

class VerbatimEnvironment(VerbatimSpecInfo, macrospec.EnvironmentSpec):
    pass



