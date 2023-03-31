
from pylatexenc import macrospec
from pylatexenc.latexnodes import ParsedArgumentsInfo
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import nodes as latexnodes_nodes

from ..llmspecinfo import LLMSpecInfo
from ..llmenvironment import LLMArgumentSpec

from ._base import SimpleLatexDefinitionsFeature




class VerbatimSpecInfo(LLMSpecInfo):

    allowed_in_standalone_mode = True

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

        elif node.isNodeType(latexnodes_nodes.LatexMacroNode):

            node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('verbatim_content',)
            )

            verbatim_contents = node_args['verbatim_content'].get_content_as_chars()

        else:

            verbatim_contents = node.latex_verbatim()
        
        annotations = self.annotations or []
        if environment_node_name is not None:
            annotations.append(environment_node_name)

        return render_context.fragment_renderer.render_verbatim(
            verbatim_contents,
            annotations=annotations,
        )



# transcrypt doesn't seem to like super().__init__() (or the default
# constructor) with multiple inheritance
### BEGINPATCH_MULTIPLE_BASE_CONSTRUCTORS
def _dobaseconstructors2argslast(Me, self, args, kwargs, kwargs_to_1=None):
    super(Me, self).__init__(*args, **kwargs)
### ENDPATCH_MULTIPLE_BASE_CONSTRUCTORS


class VerbatimMacro(VerbatimSpecInfo, macrospec.MacroSpec):
    def __init__(self, macroname,
                 verbatim_delimiters=None,
                 **kwargs):
        newkwargs = dict(
            macroname=macroname,
            arguments_spec_list=[
                LLMArgumentSpec(
                    parser=latexnodes_parsers.LatexDelimitedVerbatimParser(
                        delimiters=verbatim_delimiters,
                    ),
                    argname='verbatim_content'
                )
            ],
            **kwargs
        )
        _dobaseconstructors2argslast(VerbatimMacro, self, [], newkwargs)


class VerbatimEnvironment(VerbatimSpecInfo, macrospec.EnvironmentSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(VerbatimEnvironment, self, args, kwargs)





class FeatureVerbatim(SimpleLatexDefinitionsFeature):

    feature_name = 'verbatim'

    latex_definitions = {
        'macros': [
            VerbatimMacro(macroname='verb')
        ],
        'environments': [
            VerbatimEnvironment(environmentname='verbatimtext'),
        ]
    }


FeatureClass = FeatureVerbatim
