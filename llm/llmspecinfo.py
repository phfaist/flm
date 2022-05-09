from pylatexenc import macrospec
from pylatexenc.latexnodes import nodes as latexnodes_nodes



class LLMSpecInfo:

    delayed_render = False

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    # def finalize_parsed_node(self, node):
    #     return node

    def scan(self, node, scanner):
        r"""
        ...
        """
        pass

    def prepare_delayed_render(self, node, doc, fragment_renderer):
        r"""
        For items with `delayed_render=True`, this method is called instead of
        render() on the first pass, so that this document item has the
        opportunity to register itself in document feature managers, etc.

        This method is never called if `delayed_render=False`.
        """
        raise RuntimeError("Reimplement me!")

    def render(self, node, doc, fragment_renderer):
        raise RuntimeError(
            f"Element ‘{node}’ cannot be placed here, render() not reimplemented."
        )



class LLMSpecInfoSpecClass:
    def __init__(self, llm_specinfo, **kwargs):
        super().__init__(**kwargs)
        self.llm_specinfo = llm_specinfo
        if hasattr(llm_specinfo, 'render'):
            self.llm_specinfo_string = None
        else:
            self.llm_specinfo_string = llm_specinfo

    def finalize_node(self, node):
        node.llm_specinfo = self.llm_specinfo
        return node

    

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

    def render(self, node, doc, fragment_renderer):

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('text',) ,
            all=True
        )

        content = fragment_renderer.render_nodelist(
            node_args['text'].nodelist, doc,
            use_paragraphs=False
        )

        return fragment_renderer.render_text_format(self.text_formats, content)



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

    def render(self, node, doc, fragment_renderer):

        if node.isNodeType(latexnodes_nodes.LatexEnvironmentNode):
            if self.include_environment_begin_end:
                verbatim_contents = node.latex_verbatim()
            else:
                # it's an environment node, and we only want to render the contents of
                # the environment.
                verbatim_contents = node.nodelist.latex_verbatim()
        else:
            verbatim_contents = node.latex_verbatim()
        
        return fragment_renderer.render_verbatim(
            verbatim_contents,
            self.annotation
        )

class MathEnvironment(LLMSpecInfo):
    def render(self, node, doc, fragment_renderer):
        environmentname = node.environmentname
        return fragment_renderer.render_math_content(
            (f"\\begin{{{environmentname}}}", f"\\end{{{environmentname}}}",),
            node.nodelist,
            doc,
            'display',
            environmentname
        )

class MathEqref(LLMSpecInfo):
    def render(self, node, doc, fragment_renderer):
        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('ref_target',),
            all=True
        )
        
        ref_type = None
        ref_target = fragment_renderer.get_nodelist_as_chars(
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

        return fragment_renderer.render_math_content(
            (r"\(", r"\)"),
            latexnodes_nodes.LatexNodeList([node]),
            doc,
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

    def render(self, node, doc, fragment_renderer):

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            self.command_arguments,
            all=True
        )
        
        target_href = None
        display_text = None

        if 'target_href' in node_args:
            target_href = fragment_renderer.get_nodelist_as_chars(
                node_args['target_href'].nodelist
            )
        if 'display_text' in node_args:
            display_text = fragment_renderer.render_nodelist(
                node_args['display_text'].nodelist,
                doc=doc,
                use_paragraphs=False,
            )

        # show URL by default
        if display_text is None:
            display_text = self.pretty_url(target_href)

        return fragment_renderer.render_link(
            self.ref_type,
            target_href,
            display_text,
        )



class Error(LLMSpecInfo):
    def __init__(self, error_msg=None):
        super().__init__()
        self.error_msg = error_msg
    
    def render(self, node, doc, fragment_renderer):
        if self.error_msg:
            raise ValueError(self.error_msg)
        else:
            raise ValueError(f"The node ‘{node}’ cannot be placed here.")


