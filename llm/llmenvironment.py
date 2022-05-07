from pylatexenc import macrospec

from .llmfragment import LLMFragment


# ------------------------------------------------------------------------------


class LLMEnvironment:
    def __init__(self, latex_context_db):
        super().__init__()
        self.latex_context_db = latex_context_db

    def make_llm_fragment(self, llm_text, **kwargs):
        return LLMFragment(llm_text, llm_environment=self, **kwargs)



# ------------------------------------------------------------------------------

class LLMSpecInfo:

    delayed_render = False

    def __init__(self):
        super().__init__()

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
        raise RuntimeError(f"Element ‘{node}’ cannot be placed here, render() not reimplemented.")



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
    def __init__(self, macroname, arguments_spec_list=None, llm_specinfo=None):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=arguments_spec_list,
            llm_specinfo=llm_specinfo,
        )


class LLMEnvironmentSpec(LLMSpecInfoSpecClass, macrospec.EnvironmentSpec):
    def __init__(self, environmentname, arguments_spec_list=None, body_parser=None,
                 llm_specinfo=None):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=arguments_spec_list,
            body_parser=body_parser,
            llm_specinfo=llm_specinfo,
        )

class LLMSpecialsSpec(LLMSpecInfoSpecClass, macrospec.SpecialsSpec):
    def __init__(self, specials_chars, arguments_spec_list=None, llm_specinfo=None):
        super().__init__(
            specials_chars=specials_chars,
            arguments_spec_list=arguments_spec_list,
            llm_specinfo=llm_specinfo,
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
            node_args['text'], doc,
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

        if node.isNodeType(nodes.LatexEnvironmentNode):
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



class Error(LLMSpecInfo):
    def render(self, node, doc, fragment_renderer):
        raise ValueError(
            f"The node ‘{node}’ cannot be placed here."
        )


