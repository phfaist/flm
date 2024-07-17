import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    ParsedArgumentsInfo,
    LatexArgumentSpec,
    LatexWalkerLocatedError,
)
from pylatexenc.latexnodes.nodes import LatexNodeList, LatexGroupNode
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import ParsingStateDeltaExtendLatexContextDb

from ..flmspecinfo import (
    FLMArgumentSpec, FLMMacroSpecBase, FLMSpecialsSpecBase
)

from ._base import Feature


_macroarg_placeholder_arguments_spec_list = [
    FLMArgumentSpec(
        parser='{',
        argname='argument_ref',
        flm_doc=(
            'Argument number (e.g. #2 for second argument, counting from 1 like LaTeX) or '
            'argument name (e.g. #{label} for argument named "label")'
        )
    )
]

class SimpleMacroArgumentPlaceholder(FLMSpecialsSpecBase):

    allowed_in_standalone_mode = True

    def __init__(self, specials_chars='#', parsed_arguments_infos=None,
                 compile_default_argument_value=None):
        super().__init__(specials_chars=specials_chars,
                         arguments_spec_list=_macroarg_placeholder_arguments_spec_list)
        self.parsed_arguments_infos = parsed_arguments_infos
        self.compile_default_argument_value = compile_default_argument_value
        self.num_arguments = 0
        self.argument_names = []
        for arg in self.parsed_arguments_infos.keys():
            if isinstance(arg, int) or len(arg) == 1 and arg[0].isdigit():
                if arg >= self.num_arguments:
                    self.num_arguments = arg + 1
            else:
                self.argument_names.append(arg)

        
    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('argument_ref',) ,
        )
        
        argument_ref = node_args['argument_ref'].get_content_as_chars().strip()

        argument_ref_key = None
        if len(argument_ref) == 1 and argument_ref.isdigit():
            # numerical argument reference
            argument_ref_key = int(argument_ref) - 1 # store index, starting from zero

            if argument_ref_key < 0 or argument_ref_key >= self.num_arguments:
                raise LatexWalkerLocatedError(
                    f"Invalid argument number: ‘{argument_ref}’.  Expected a number between "
                    f"1 and {self.num_arguments} (incl.)",
                    pos=node.pos
                )
        else:
            argument_ref_key = argument_ref

        logger.debug(f"Got argument replacement placeholder ref={repr(argument_ref)} "
                     f"[→ type {type(argument_ref)} → key={argument_ref_key}]")
        logger.debug("Available argument values are %r", self.parsed_arguments_infos)

        node.flmarg_argument_ref = argument_ref
        node.flmarg_argument_ref_key = argument_ref_key

        # and figure out the replacement nodes!
        if argument_ref_key not in self.parsed_arguments_infos:
            valid_arg_specs = f"numbers 1–{self.num_arguments}"
            if len(self.argument_names):
                valid_arg_specs = (
                    ",".join([f"‘{argname}’" for argname in self.argument_names ])
                    + " and " + valid_arg_specs
                )
            raise LatexWalkerLocatedError(
                f"Invalid argument name or index: ‘{argument_ref}’.  Valid argument "
                + f"specifiers are {valid_arg_specs}",
                pos=node.pos
            )

        nodelist = self.parsed_arguments_infos[argument_ref_key].get_content_nodelist()

        use_default = False
        if nodelist is None or (len(nodelist) == 1 and nodelist[0] is None):
            nodelist = []
            use_default = True

        if use_default and self.compile_default_argument_value is not None:
            nodelist = self.compile_default_argument_value(argument_ref_key)
            if nodelist is None or (len(nodelist) == 1 and nodelist[0] is None):
                nodelist = []

        # call make_nodelist if necessary

        if isinstance(nodelist, LatexNodeList) and not hasattr(nodelist, "flm_is_block_level"):
            # the LatexNodeList was not created correctly.  Get the raw list
            # of nodes, we'll re-create the LatexNodeList with latex_walker.make_nodelist().
            nodelist = nodelist.nodelist

        if not isinstance(nodelist, LatexNodeList):
            nodelist = node.latex_walker.make_nodelist(
                nodelist,
                parsing_state=node.parsing_state
            )

        node.flm_placeholder_content_nodelist = nodelist

        logger.debug("specinfo postprocessing substitution macro node %r", node)

        substitute_node = node.latex_walker.make_node(
            LatexGroupNode,
            parsing_state=node.parsing_state,
            delimiters=('',''),
            nodelist=nodelist,
            pos=node.pos,
            pos_end=node.pos_end,
        )

        logger.debug("setting substitute node = %r", substitute_node)
        node.flm_SUBSTITUTE_NODE = substitute_node


    # ### Not needed nor called, it's the substitute node that's rendered...
    #
    # def render(self, node, render_context):
    #     return render_context.fragment_renderer.render_nodelist(
    #         node.flm_placeholder_content_nodelist,
    #         render_context,
    #     )


def _get_arg_spec(argspec):
    try:
        if argspec['parser']:
            # argspec is a dictionary and it has a 'parser' key.  (Don't use
            # isinstance(..., dict) so that this works with Transcrypt...)
            return LatexArgumentSpec(**argspec)
    except (TypeError, KeyError):
        # not a dict, okay, use default constructor (may still be a string)
        pass
    return argspec


class SimpleSubstitutionMacro(FLMMacroSpecBase):
    r"""
    A macro spec that describes a simple substitution of the macro
    (possibly with arguments) by user-defined custom FLM content (possibly with
    argument placeholders).

    - `macroname` is the name of the custom macro

    - `arguments_spec_list` is the list of arguments specification objects
      describing the arguments accepted by the custom macro.  This parameter is
      as you'd give to `FLMMacroSpecBase`.  As an exception, each element of the
      list can also be a dictionary, in which case the corresponding argument
      specification is constructed by passing the dictionary's contents as
      keyword arguments to the constructor of `LatexArgumentSpec()`.
      E.g. `arguments_spec_list = [ {'parser': '[', 'argname': 'argone'},
      {'parser': '{', 'argname': 'argtwo'} ]`.

    - `default_argument_values` is a dictionary with values that should be used
      for the arguments if the argument is not specified.  The values are FLM
      text that can include `#..` argument placeholders.

    - `content` specifies the replacement FLM string for the macro.  If
      `content` is a string, it is the replacement FLM string associated with
      the macro, which can contain argument placeholders `#1...#N` and
      `#{argname}`.  If it is a dictionary, it should have the form
      `{'textmode': ..., 'mathmode'...}` specifying the replacement FLM string
      values to be used in text mode and in math mode.

    - `is_block_level` can be used to specify whether the macro represents
      block-level content or inline content.
    """

    allowed_in_standalone_mode = True

    def __init__(self, macroname,
                 arguments_spec_list=None,
                 default_argument_values=None,
                 content=None,
                 is_block_level=None,
                 ):
        
        # allow user to specify a latex argument spec as a dict
        if arguments_spec_list is not None and len(arguments_spec_list):
            arguments_spec_list = [ _get_arg_spec(arg) for arg in arguments_spec_list ]

        super().__init__(macroname, arguments_spec_list)
        
        if default_argument_values is None:
            default_argument_values = {}
        self.default_argument_values = default_argument_values

        if content is None:
            content = ''

        if isinstance(content, str):
            self.content_textmode = content
            self.content_mathmode = content
        else:
            self.content_textmode = content['textmode']
            self.content_mathmode = content['mathmode']


    def postprocess_parsed_node(self, node):

        logger.debug("specinfo postprocessing substitution macro node %r", node)

        if node.parsing_state.in_math_mode:
            macro_replacement_flm_text = self.content_mathmode
        else:
            macro_replacement_flm_text = self.content_textmode

        if macro_replacement_flm_text is None:
            raise LatexWalkerLocatedError(
                f"Custom macro ‘\\{self.macroname}’ not allowed here (replacement is None in "
                + ('math' if node.parsing_state.in_math_mode else 'text') + " mode).",
                pos=node.pos
            )
    
        # the replacement fragment flm text
        node.flm_macro_replacement_flm_text = macro_replacement_flm_text

        substitute_nodelist = self._compile_nodes(node)

        node.flm_macro_replacement_flm_nodes = substitute_nodelist

        substitute_node = node.latex_walker.make_node(
            LatexGroupNode,
            parsing_state=node.parsing_state,
            delimiters=('',''),
            nodelist=substitute_nodelist,
            pos=node.pos,
            pos_end=node.pos_end,
        )

        logger.debug("setting substitute node = %r", substitute_node)
        node.flm_SUBSTITUTE_NODE = substitute_node



    def _compile_nodes(self, node):

        # compose & compile the flm text into nodes, including argument
        # placeholder nodes.

        base_latex_walker = node.latex_walker
        flm_environment = node.latex_walker.flm_environment

        macro_replacement_flm_text = node.flm_macro_replacement_flm_text

        parsed_arguments_infos = ParsedArgumentsInfo(node=node).get_all_arguments_info()

        logger.debug(
            "Parsing macro content %r with replacements %r",
            macro_replacement_flm_text,
            parsed_arguments_infos
        )

        parsing_state_delta = None

        def compile_default_argument_value(arg_ref):
            arg_ref_user = arg_ref
            if isinstance(arg_ref, int):
                # 0-th argument is "#1" so use "1" as argument key
                arg_ref_user = arg_ref + 1
            try:
                default_arg_flm_text = self.default_argument_values[arg_ref_user]
            except (TypeError, KeyError):
                # no such argument
                return []
            if default_arg_flm_text is None:
                return []

            defaultarg_latex_walker = flm_environment.make_latex_walker(
                default_arg_flm_text,
                is_block_level=node.parsing_state.is_block_level,
                parsing_mode=base_latex_walker.parsing_mode,
                resource_info=base_latex_walker.resource_info,
                standalone_mode=base_latex_walker.standalone_mode,
                tolerant_parsing=base_latex_walker.tolerant_parsing,
                what=f"{base_latex_walker.what}→\\{self.macroname}/default arg {arg_ref}",
                input_lineno_colno_offsets=None,
            )

            defaultarg_parsing_state = parsing_state_delta.get_updated_parsing_state(
                node.parsing_state,
                defaultarg_latex_walker
            )

            nodes, _ = defaultarg_latex_walker.parse_content(
                latexnodes_parsers.LatexGeneralNodesParser(),
                parsing_state=defaultarg_parsing_state
            )
            return nodes


        parsing_state_delta = ParsingStateDeltaExtendLatexContextDb(
            {
                'specials': [
                    SimpleMacroArgumentPlaceholder('#', parsed_arguments_infos,
                                                   compile_default_argument_value),
                ],
            },
        )

        content_latex_walker = flm_environment.make_latex_walker(
            macro_replacement_flm_text,
            is_block_level=node.parsing_state.is_block_level,
            parsing_mode=base_latex_walker.parsing_mode,
            resource_info=base_latex_walker.resource_info,
            standalone_mode=base_latex_walker.standalone_mode,
            tolerant_parsing=base_latex_walker.tolerant_parsing,
            what=f"{base_latex_walker.what}→\\{self.macroname}",
            input_lineno_colno_offsets=None,
        )

        content_parsing_state = parsing_state_delta.get_updated_parsing_state(
            node.parsing_state,
            content_latex_walker
        )

        nodes, _ = content_latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser(),
            parsing_state=content_parsing_state
        )

        return nodes



    def render(self, node, render_context):

        return render_context.fragment_renderer.render_nodelist(
            node.flm_macro_replacement_flm_nodes,
            render_context,
        )


class FeatureSimpleMacros(Feature):

    DocumentManager = None
    RenderManager = None

    feature_name = 'macros'
    feature_title = 'Custom macros definitions'
    

    def __init__(self, definitions):
        super().__init__()

        if definitions is None:
            definitions = {}
        if 'macros' not in definitions:
            definitions['macros'] = {}

        self.definitions = definitions
        

    def add_latex_context_definitions(self):
        r"""
        Reimplement to add additional definitions to the latex context
        database.
        """
        return {
            'macros': [
                SimpleSubstitutionMacro(macroname=macroname, **macrodef)
                for macroname, macrodef in self.definitions['macros'].items()
            ]
        }
        

FeatureClass = FeatureSimpleMacros
