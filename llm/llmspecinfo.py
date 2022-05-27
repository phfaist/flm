
import logging
logger = logging.getLogger(__name__)


from pylatexenc import macrospec
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import LatexWalkerParseError


# ------------------------------------------------------------------------------


class LLMSpecInfo:

    delayed_render = False

    is_block_level = False

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    def finalize_parsed_node(self, node):
        return node

    def scan(self, node, scanner):
        r"""
        ...
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
        raise RuntimeError(
            f"Element ‘{node}’ cannot be placed here, render() not reimplemented."
        )


# ------------------------------------------------------------------------------


class LLMSpecInfoSpecClass:
    def __init__(self, llm_specinfo, **kwargs):
        super().__init__(**kwargs)
        self.llm_specinfo = llm_specinfo
        if hasattr(llm_specinfo, 'render'):
            self.llm_specinfo_string = None
        else:
            self.llm_specinfo_string = llm_specinfo

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        if hasattr(self.llm_specinfo, 'make_body_parser'):
            return self.llm_specinfo.make_body_parser(token, nodeargd, arg_parsing_state_delta)
        return super().make_body_parser(token, nodeargd, arg_parsing_state_delta)

    def make_body_parsing_state_delta(self, token, nodeargd, arg_parsing_state_delta,
                                      latex_walker):
        logger.debug("LLM make_body_parsing_state_delta was called.")
        if hasattr(self.llm_specinfo, 'body_parsing_state_delta'):
            return getattr(self.llm_specinfo, 'body_parsing_state_delta')
        return super().make_body_parsing_state_delta(
            token=token,
            nodeargd=nodeargd,
            arg_parsing_state_delta=arg_parsing_state_delta,
            latex_walker=latex_walker,
        )

    def finalize_node(self, node):
        node.llm_specinfo = self.llm_specinfo
        if hasattr(self.llm_specinfo, 'finalize_parsed_node'):
            node = self.llm_specinfo.finalize_parsed_node(node)
        if hasattr(self.llm_specinfo, 'is_block_level'):
            node.llm_is_block_level = self.llm_specinfo.is_block_level
        if hasattr(self.llm_specinfo, 'is_paragraph_break_marker'):
            node.llm_is_paragraph_break_marker = self.llm_specinfo.is_paragraph_break_marker
        return node

    
# ------------------------------------------------------------------------------


class LLMMacroSpec(LLMSpecInfoSpecClass, macrospec.MacroSpec):
    def __init__(self, macroname, arguments_spec_list=None, *,
                 llm_specinfo=None, **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=arguments_spec_list,
            llm_specinfo=llm_specinfo,
            **kwargs
        )


class LLMEnvironmentSpec(LLMSpecInfoSpecClass, macrospec.EnvironmentSpec):
    def __init__(self, environmentname, arguments_spec_list=None, *,
                 llm_specinfo=None, **kwargs):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=arguments_spec_list,
            llm_specinfo=llm_specinfo,
            **kwargs
        )

class LLMSpecialsSpec(LLMSpecInfoSpecClass, macrospec.SpecialsSpec):
    def __init__(self, specials_chars, arguments_spec_list=None, *,
                 llm_specinfo=None, **kwargs):
        super().__init__(
            specials_chars=specials_chars,
            arguments_spec_list=arguments_spec_list,
            llm_specinfo=llm_specinfo,
            **kwargs
        )




# ------------------------------------------------------------------------------



class TextFormat(LLMSpecInfo):
    # any additional 
    def __init__(self, text_formats):
        r"""
        The argument `text_formats` is a list of strings, each string is a format
        name to apply.  Format names are inspired from the corresponding
        canonical LaTeX macros that apply them.  They are:

        - `textit`

        - `textbf`

        - (possibly more in the future)
        """
        super().__init__()
        self.text_formats = text_formats

    def render(self, node, render_context):

        node_args = render_context.fragment_renderer.get_arguments_nodelists(
            node,
            ('text',) ,
            all=True
        )

        return render_context.fragment_renderer.render_text_format(
            self.text_formats,
            node_args['text'].nodelist,
            render_context,
        )


class MathEnvironment(LLMSpecInfo):
    def render(self, node, render_context):
        environmentname = node.environmentname
        return render_context.fragment_renderer.render_math_content(
            (f"\\begin{{{environmentname}}}", f"\\end{{{environmentname}}}",),
            node.nodelist,
            render_context,
            'display',
            environmentname
        )

class MathEqref(LLMSpecInfo):
    def render(self, node, render_context):
        node_args = render_context.fragment_renderer.get_arguments_nodelists(
            node,
            ('ref_target',),
            all=True
        )
        
        ref_type = None
        ref_target = render_context.fragment_renderer.get_nodelist_as_chars(
            node_args['ref_target'].nodelist
        )
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


class HrefHyperlink(LLMSpecInfo):
    def __init__(
            self,
            command_arguments=('target_href', 'display_text',),
            ref_type='href',
    ):
        super().__init__()
        self.command_arguments = command_arguments
        self.ref_type = ref_type

    @classmethod
    def pretty_url(self, target_href):
        url_display = str(target_href)
        for prefix in ('http://', 'https://'):
            if url_display.startswith(prefix):
                url_display = url_display[len(prefix):]
                break
        url_display = url_display.rstrip('/#?')
        # for suffix in ('/', '#', '?',):
        #     if url_display.endswith(suffix):
        #         url_display = url_display[:-len(suffix)]
        return url_display

    def render(self, node, render_context):

        node_args = render_context.fragment_renderer.get_arguments_nodelists(
            node,
            self.command_arguments,
            all=True
        )
        
        target_href = None
        display_text_nodelist = None

        if 'target_href' in node_args:
            target_href = render_context.fragment_renderer.get_nodelist_as_chars(
                node_args['target_href'].nodelist
            )
        if 'display_text' in node_args:
            display_text_nodelist = node_args['display_text'].nodelist

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


class Verbatim(LLMSpecInfo):
    r"""
    Wraps an argument, or an environment body, as verbatim content.

    The `annotation` is basically a HTML class name to apply to the block of
    content.  Use this for instance to separate out math content, etc.
    """
    def __init__(self, annotation=None, include_environment_begin_end=False):
        super().__init__()
        self.annotation = annotation
        self.include_environment_begin_end = include_environment_begin_end

    def render(self, node, render_context):

        if node.isNodeType(latexnodes_nodes.LatexEnvironmentNode):
            if self.include_environment_begin_end:
                verbatim_contents = node.latex_verbatim()
            else:
                # it's an environment node, and we only want to render the contents of
                # the environment.
                verbatim_contents = node.nodelist.latex_verbatim()
        else:
            verbatim_contents = node.latex_verbatim()
        
        return render_context.fragment_renderer.render_verbatim(
            verbatim_contents,
            self.annotation
        )



class ParagraphBreak(LLMSpecInfo):

    is_block_level = True

    is_paragraph_break_marker = True
    
    def __init__(self):
        super().__init__()

    def render(self, node, render_context):
        raise LatexWalkerParseError('Paragraph break is not allowed here', pos=node.pos)



class Error(LLMSpecInfo):
    def __init__(self, error_msg=None):
        super().__init__()
        self.error_msg = error_msg
    
    def render(self, node, render_context):
        if self.error_msg:
            msg = self.error_msg
        else:
            msg = f"The node ‘{node}’ cannot be placed here."

        raise LatexWalkerParseError(msg, pos=node.pos)
