
from pylatexenc import macrospec
from pylatexenc.latexnodes import ParsedArgumentsInfo
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import nodes as latexnodes_nodes

from ..flmspecinfo import FLMSpecInfo
from ..flmenvironment import FLMArgumentSpec

from ._base import SimpleLatexDefinitionsFeature




class VerbatimSpecInfo(FLMSpecInfo):

    allowed_in_standalone_mode = True

    r"""
    Wraps an argument, or an environment body, as verbatim content.

    The `annotation` is basically a HTML class name to apply to the block of
    content.  Use this for instance to separate out math content, etc.
    """
    def __init__(self, *args,
                 annotations=None,
                 verbatimtype='text',
                 is_block_level=False,
                 include_environment_begin_end=False,
                 **kwargs):
        super().__init__(*args, **kwargs)
        self.annotations = annotations
        self.verbatimtype = verbatimtype

        self.include_environment_begin_end = include_environment_begin_end

        # recognized FLMSpecInfo property
        self.is_block_level = is_block_level

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

        # environment_node_name = None

        verbatim_lang = None

        is_inline = False
        is_environment = False

        if node.isNodeType(latexnodes_nodes.LatexEnvironmentNode):

            is_environment = True

            # environment_node_name = node.environmentname

            if self.include_environment_begin_end:
                verbatim_contents = node.latex_verbatim()
            else:
                # it's an environment node, and we only want to render the contents of
                # the environment.
                verbatim_contents = node.nodelist.latex_verbatim()

            # check for verbatim_lang
            node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('verbatim_lang'),
                skip_nonexistent_arguments=True,
            )
            if 'verbatim_lang' in node_args:
                verbatim_lang = node_args['verbatim_lang'].get_content_as_chars()

        else: # macro or specials

            is_inline = True

            node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('verbatim_content', 'verbatim_lang'),
                skip_nonexistent_arguments=True,
            )

            verbatim_contents = node_args['verbatim_content'].get_content_as_chars()

            if 'verbatim_lang' in node_args:
                verbatim_lang = node_args['verbatim_lang'].get_content_as_chars()

        annotations = self.annotations or []

        if self.verbatimtype is not None:
            annotations.append(f'verbatim{self.verbatimtype}')
            if is_inline:
                annotations.append(f'verbatim{self.verbatimtype}-inline')
            if is_environment:
                annotations.append(f'verbatim{self.verbatimtype}-environment')

        if verbatim_lang:
            annotations.append(f'verbatim-lang-{verbatim_lang}')

        return render_context.fragment_renderer.render_verbatim(
            verbatim_contents,
            render_context,
            is_block_level=self.is_block_level,
            annotations=annotations,
        )

    # FLM-doc

    def get_flm_doc(self):
        return f"""Typeset verbatim content of type ‘{self.verbatimtype}’."""


# transcrypt doesn't seem to like super().__init__() (or the default
# constructor) with multiple inheritance
### BEGINPATCH_MULTIPLE_BASE_CONSTRUCTORS
def _dobaseconstructors2argslast(Me, self, args, kwargs, kwargs_to_1=None):
    super(Me, self).__init__(*args, **kwargs)
### ENDPATCH_MULTIPLE_BASE_CONSTRUCTORS


def make_verbatim_args_spec_list(ismacro, verbatim_delimiters, optional_lang_arg):
    a = []
    if optional_lang_arg:
        a.append(
            FLMArgumentSpec(
                parser=latexnodes_parsers.LatexCharsGroupParser(
                    delimiters=('[', ']'),
                    optional=True,
                    allow_pre_space=False,
                    enable_comments=False,
                    enable_groups=False
                ),
                argname='verbatim_lang',
                flm_doc="Programming language in which to interpret the verbatim content, "
                "if applicable.  Some formatters might support syntax highlighting in the "
                "relevant language."
            )
        )
    if ismacro:
        a.append(
            FLMArgumentSpec(
                parser=latexnodes_parsers.LatexDelimitedVerbatimParser(
                    delimiters=verbatim_delimiters,
                ),
                argname='verbatim_content',
                flm_doc=r"The raw, verbatim content to typeset.  Any special characters, "
                r"including \verbcode+\+, \verbcode+{+, and \verbcode+}+, will be typeset "
                r"as is."
            )
        )
    return a

class VerbatimMacro(VerbatimSpecInfo, macrospec.MacroSpec):
    def __init__(self, macroname,
                 verbatim_delimiters=None,
                 *,
                 optional_lang_arg=False,
                 **kwargs):
        newkwargs = dict(
            macroname=macroname,
            arguments_spec_list=make_verbatim_args_spec_list(
                True, verbatim_delimiters, optional_lang_arg
            ),
            **kwargs
        )
        _dobaseconstructors2argslast(VerbatimMacro, self, [], newkwargs)


class VerbatimEnvironment(VerbatimSpecInfo, macrospec.EnvironmentSpec):
    def __init__(self, environmentname, *, optional_lang_arg=False, **kwargs):
        newkwargs = dict(
            environmentname=environmentname,
            arguments_spec_list=make_verbatim_args_spec_list(
                False, None, optional_lang_arg
            ),
            **kwargs
        )
        _dobaseconstructors2argslast(VerbatimEnvironment, self, [], newkwargs)





class FeatureVerbatim(SimpleLatexDefinitionsFeature):

    feature_name = 'verbatim'
    feature_title = 'Verbatim content typesetting'
    
    # which verbatim types to include.
    # E.g. 'code' ->  \verbcode+...+ and \begin{verbatimcode}[lang]...\end{verbatimcode}
    verbatim_include_types = ('text', 'code', 'a')

    def feature_flm_doc(self):
        s = r"""
        You can typeset verbatim content using the \verbcode+\verb...+ family of
        macros and environments.  When typesetting verbatim content, any special
        meaning of characters in FLM is ignored.  E.g., the characters
        ‘\verbcode+\+’, ‘\verbcode+{+’, ‘\verbcode+}+’ are typeset as is, and
        braces don't have to be matched.

        The following verbatim types are available:
        \begin{itemize}
        """
        if 'text' in self.verbatim_include_types:
            s += r"""
            \item \verba{text} — the verbatim characters are typeset as is, with no
                special formatting applied to them.  This verbatim type is intended
                for inputing special characters into normal text.
            """
        if 'code' in self.verbatim_include_types:
            s += r"""
            \item \verba{code} — the verbatim characters are typeset as a block of
                code, possibly to be interpreted as a given programming language.
                Renderers should represent the content as code, e.g., with a monospaced
                font.
            """
        if 'a' in self.verbatim_include_types:
            s += r"""
            \item \verba{a} — the verbatim characters are typeset as an identifier,
                typically in italic font.  This command is intended to typeset e.g.
                keywords or other content that isn't necessarily to be highlighted as
                computer code.
            """
        s += r"\end{itemize}"
        return s

    def add_latex_context_definitions(self):
        macros = []
        environments = []
        
        if 'text' in self.verbatim_include_types:
            macros.append(
                VerbatimMacro(macroname='verbtext'),
            )
            environments.append(
                VerbatimEnvironment(environmentname='verbatimtext'),
            )
        if 'code' in self.verbatim_include_types:
            macros.append(
                VerbatimMacro(
                    macroname='verbcode',
                    optional_lang_arg=True,
                    verbatimtype='code',
                ),
            )
            environments.append(
                VerbatimEnvironment(environmentname='verbatimcode',
                                    optional_lang_arg=True,
                                    verbatimtype='code',
                                    is_block_level=True,
                                    )
            )
        if 'a' in self.verbatim_include_types:
            macros.append(
                VerbatimMacro(
                    macroname='verba',
                    optional_lang_arg=True,
                    verbatimtype='a',
                ),
            )

        return {
            'macros': macros,
            'environments': environments,
        }


FeatureClass = FeatureVerbatim
