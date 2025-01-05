import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    ParsedArgumentsInfo,
    SingleParsedArgumentInfo,
    LatexWalkerLocatedError,
    ParsingStateDelta,
)
from pylatexenc.latexnodes.parsers import LatexParserBase
from pylatexenc.latexnodes.nodes import LatexNodeList, LatexGroupNode, LatexEnvironmentNode
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import ParsingStateDeltaExtendLatexContextDb

from ..flmspecinfo import (
    FLMArgumentSpec, FLMSpecInfo, FLMMacroSpecBase, FLMSpecialsSpecBase
)

from ._base import Feature



# ---

class NothingParser(LatexParserBase):
    def parse(self, latex_walker, token_reader, parsing_state, **kwargs):
        # parse nothing - always return None
        return latex_walker.make_nodelist([], parsing_state=parsing_state), None


SetArgumentNumberOffset = FLMArgumentSpec(
    parser=NothingParser(),
    argname='_SetArgumentNumberOffset',
)



# ---



_macroarg_placeholder_arguments_spec_list = [
    FLMArgumentSpec(
        parser='[',
        argname='substitution_arg',
        flm_doc=(
            'When defining custom substitution handlers, you can use this argument '
            'internally to specify custom substitution strings'
        )
    ),
    FLMArgumentSpec(
        parser='{',
        argname='placeholder_ref',
        flm_doc=(
            'Argument number (e.g. #2 for second argument, counting from 1 like LaTeX) or '
            'argument name (e.g. #{label} for argument named "label")'
        )
    ),
]



class SimpleMacroArgumentPlaceholder(FLMSpecialsSpecBase):

    allowed_in_standalone_mode = True

    def __init__(self,
                 specials_chars='#',
                 *,
                 macro_content_substitutor,
                 parse_arg_information_only=False,
                 ):
        super().__init__(specials_chars=specials_chars,
                         arguments_spec_list=_macroarg_placeholder_arguments_spec_list)
        self.macro_content_substitutor = macro_content_substitutor
        self.parse_arg_information_only = parse_arg_information_only

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('substitution_arg', 'placeholder_ref',) ,
        )
        
        placeholder_ref = node_args['placeholder_ref'].get_content_as_chars().strip()

        node.flmarg_placeholder_ref = placeholder_ref
        node.flmarg_substitution_arg_info = node_args['substitution_arg']

        if self.parse_arg_information_only:
            # stop here
            return

        value = self.macro_content_substitutor.get_placeholder_value(
            placeholder_ref,
            placeholder_node=node,
            substitution_arg_info=node_args['substitution_arg'],
        )
        if isinstance(value, str):
            nodelist = self.macro_content_substitutor.compile_flm_text(
                value,
                add_what=f"placeholder ‘{placeholder_ref}’ value",
                is_block_level=node.parsing_state.is_block_level,
            )
        else:
            # value should be a node list
            nodelist = value

        logger.debug("Got placeholder substitution node list = %r", nodelist)

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

        node.flmarg_placeholder_ref = placeholder_ref
        node.flm_placeholder_content_nodelist = nodelist

        nodelist_parsing_state = nodelist.parsing_state

        logger.debug("specinfo postprocessing substitution macro node %r", node)

        substitute_node = node.latex_walker.make_node(
            LatexGroupNode,
            parsing_state=nodelist_parsing_state,
            delimiters=('',''),
            nodelist=nodelist,
            pos=node.pos,
            pos_end=node.pos_end,
        )

        logger.debug("setting substitute node = %r", substitute_node)
        node.flm_SUBSTITUTE_NODE = substitute_node


    #
    # ### Not needed nor called, it's the substitute node that's rendered...
    #
    def render(self, node, render_context):
        raise RuntimeError("Shouldn't be called")
    




def _make_ifarg_argument_argspec(macro_content_substitutor):
    return FLMArgumentSpec(
        parser='{',
        parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
            extend_latex_context={
                'specials': [
                    SimpleMacroArgumentPlaceholder(
                        '#',
                        macro_content_substitutor=macro_content_substitutor,
                        parse_arg_information_only=True,
                    ),
                ],
            },
            set_attributes=dict(is_block_level=False),
        ),
        argname='arg_ref',
    )




def _ifargcmd_condition_wasprovided(argument_info):
    return argument_info.was_provided()

def _ifargcmd_condition_wasnotprovided(argument_info):
    return not argument_info.was_provided()

def _ifargcmd_condition_isempty(argument_info):
    arg_content_nodes = argument_info.get_content_nodelist().filter(
        skip_none=True, skip_comments=True,
    )
    return len(arg_content_nodes) == 0

def _ifargcmd_condition_notempty(argument_info):
    arg_content_nodes = argument_info.get_content_nodelist().filter(
        skip_none=True, skip_comments=True,
    )
    return len(arg_content_nodes) != 0


_ifargcmd_types = {
    'IfNoValueTF': (
        _ifargcmd_condition_wasnotprovided,
        ('value_true', 'value_false',),
    ),
    'IfNoValueT': (
        _ifargcmd_condition_wasnotprovided,
        ('value_true',),
    ),
    'IfNoValueF': (
        _ifargcmd_condition_wasnotprovided,
        ('value_false',),
    ),
    'IfBooleanTF': (
        _ifargcmd_condition_wasprovided,
        ('value_true', 'value_false',),
    ),
    'IfBooleanT': (
        _ifargcmd_condition_wasprovided,
        ('value_true',),
    ),
    'IfBooleanF': (
        _ifargcmd_condition_wasprovided,
        ('value_false',),
    ),
    'IfValueTF': (
        _ifargcmd_condition_wasprovided,
        ('value_true', 'value_false',),
    ),
    'IfValueT': (
        _ifargcmd_condition_wasprovided,
        ('value_true',),
    ),
    'IfValueF': (
        _ifargcmd_condition_wasprovided,
        ('value_false',),
    ),
    'ifblank': (
        _ifargcmd_condition_isempty,
        ('value_true', 'value_false',)
    ),
    'notblank': (
        _ifargcmd_condition_notempty,
        ('value_true', 'value_false',)
    ),
}



def _make_ifarg_arguments_spec_list(macroname, macro_content_substitutor):

    if macroname not in _ifargcmd_types:
        raise ValueError(f"Invalid/unknown macro name for ifarg-type macro: {macroname}")

    args = [
        _make_ifarg_argument_argspec(macro_content_substitutor),
    ]
    for argname in _ifargcmd_types[macroname][1]:
        args.append(
            FLMArgumentSpec(
                parser='{',
                argname=argname,
            )
        )
    return args


def _make_patched_callables(environment):
    patched_callables = {
        'macros': [],
        'environments': [],
        'specials': [],
    }
    if environment.supports_feature('href'):
        # Use versions of these macros that enable specials replacements in the
        # URL.  Otherwise #N placeholders will be kept verbatim!
        href_feature = environment.feature('href')
        HrefHyperlinkMacroClass = href_feature.HrefHyperlinkMacroClass
        patched_callables['macros'] += [
            HrefHyperlinkMacroClass(
                macroname='href',
                command_arguments=('target_Xhref', 'display_text',),
            ),
            HrefHyperlinkMacroClass(
                macroname='url',
                command_arguments=('target_Xhref',),
            ),
            HrefHyperlinkMacroClass(
                macroname='email',
                command_arguments=('target_Xemail',),
            ),
        ]

    if environment.supports_feature('refs'):
        RefMacroCls = environment.feature('refs').RefMacroCls
        patched_callables['macros'] += [
            RefMacroCls(
                macroname='ref',
                command_arguments=('Xref_label',)
            ),
            RefMacroCls(
                macroname='hyperref',
                command_arguments=('[]Xref_label','display_text',)
            ),
        ]
    return patched_callables


class SimpleMacroContentIfArgCondition(FLMMacroSpecBase):

    allowed_in_standalone_mode = True

    def __init__(self,
                 macroname,
                 *,
                 macro_content_substitutor,
                 ):
        arguments_spec_list = _make_ifarg_arguments_spec_list(
            macroname, macro_content_substitutor
        )
        super().__init__(macroname=macroname,
                         arguments_spec_list=arguments_spec_list)
        self.macro_content_substitutor = macro_content_substitutor
        
    def postprocess_parsed_node(self, node):
        
        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('arg_ref', 'value_true', 'value_false') ,
            skip_nonexistent_arguments=True,
        )
        
        arg_ref_nodelist = node_args['arg_ref'].get_content_nodelist().filter(
            skip_none=True, skip_comments=True, skip_whitespace_char_nodes=True
        )
        
        if len(arg_ref_nodelist) != 1 \
           or not hasattr(arg_ref_nodelist[0], 'flmarg_placeholder_ref'):
            raise LatexWalkerLocatedError(
                f"First argument of \\{self.macroname} must be a argument reference. "
                f"Got = {repr(arg_ref_nodelist)}",
                pos=node.pos,
            )

        arg_ref_node = arg_ref_nodelist[0]

        placeholder_ref = arg_ref_node.flmarg_placeholder_ref

        if arg_ref_node.flmarg_substitution_arg_info.was_provided():
            raise LatexWalkerLocatedError(
                f"Cannot provide substitution placeholder optional arguments in argument "
                f"of \\{self.macroname}; got = "
                f"{repr(arg_ref_node.flmarg_substitution_arg_info)}.",
                pos=node.pos,
            )

        # figure out if condition is true or false and do substitution accordingly.

        argument_info = self.macro_content_substitutor.get_parsed_argument_info(
            placeholder_ref,
            placeholder_node=arg_ref_node,
        )

        result_nodes = None

        condition_fn, _ = _ifargcmd_types[self.macroname]

        if condition_fn(argument_info):
            if 'value_true' in node_args:
                result_nodes = node_args['value_true'].get_content_nodelist()
        else:
            if 'value_false' in node_args:
                result_nodes = node_args['value_false'].get_content_nodelist()

        if result_nodes is None:
            result_nodes = node.latex_walker.make_nodelist(
                [],
                parsing_state=node.parsing_state,
                pos=node.pos,
            )

        substitute_node = node.latex_walker.make_node(
            LatexGroupNode,
            parsing_state=node.parsing_state,
            delimiters=('',''),
            nodelist=result_nodes,
            pos=result_nodes.pos,
            pos_end=result_nodes.pos_end,
        )

        node.flm_SUBSTITUTE_NODE = substitute_node



# ------------------------------------------------------------------------------



def _get_arg_spec(argspec):
    parser_val = None
    try:
        parser_val = argspec['parser']
        # argspec is a dictionary and it has a 'parser' key.  (Don't use
        # isinstance(..., dict) so that this works with Transcrypt...)
    except (TypeError, KeyError):
        # not a dict, okay, use default constructor (may still be a string)
        pass
    if parser_val is not None:
        if argspec == 'SetArgumentNumberOffset':
            return SetArgumentNumberOffset
        if isinstance(argspec, str):
            return FLMArgumentSpec(parser=argspec, argname=None)
        argspecargs = dict(argspec)
        if 'argname' not in argspecargs:
            argspecargs['argname'] = None
        return FLMArgumentSpec(**argspecargs)
    return argspec


# --------------------------------------



class MacroContentSubstitutor:
    def __init__(self,
                 substitutor_manager,
                 callable_node,
                 parsed_arguments_infos,
                 argument_number_offset,
                 default_argument_values):

        if default_argument_values is None:
            default_argument_values = {}

        self.substitutor_manager = substitutor_manager
        self.callable_node = callable_node
        self.parsed_arguments_infos = parsed_arguments_infos
        self.default_argument_values = default_argument_values
        self.argument_number_offset = argument_number_offset

        self.num_arguments = 0
        self.argument_names = []
        for arg in self.parsed_arguments_infos.keys():
            if isinstance(arg, int) or arg.isdigit(): # .isdigit() implies len>=1
                if arg >= self.num_arguments:
                    self.num_arguments = int(arg) + 1
            else:
                self.argument_names.append(arg)

        ifmacros = [
            SimpleMacroContentIfArgCondition(
                macroname=ifmacroname,
                macro_content_substitutor=self,
            )
            for ifmacroname in _ifargcmd_types.keys()
        ]

        self.macro_content_parsing_state_delta = ParsingStateDeltaExtendLatexContextDb(
            extend_latex_context={
                'specials': [
                    SimpleMacroArgumentPlaceholder(
                        '#',
                        macro_content_substitutor=self,
                    ),
                ],
                'macros': ifmacros
            },
            set_attributes={
                'is_block_level': None,
            }
        )

        # set the argument number offset, if applicable:
        if self.callable_node.nodeargd is not None:
            arguments_spec_list = self.callable_node.nodeargd.arguments_spec_list
            if arguments_spec_list:
                for j, arg_spec in enumerate(arguments_spec_list):
                    if arg_spec.argname == '_SetArgumentNumberOffset':
                        logger.debug("Found _SetArgumentNumberOffset in %r, "
                                     "setting offset = %d",
                                     repr(self.callable_node), j)
                        self.argument_number_offset = j + 1
                        break



    def initialize(self):
        # can be reimplemented to set up computed values after base class is
        # constructed without having to worry about passing arguments up to base
        # class constructor
        pass

    # ---

    def get_placeholder_value(self, placeholder_ref, placeholder_node, substitution_arg_info):

        value = self.substitutor_manager.get_placeholder_value(
            placeholder_ref,
            placeholder_node=placeholder_node,
            substitution_arg_info=substitution_arg_info,
            callable_node=self.callable_node,
            macro_content_substitutor=self,
        )
        if value is not None:
            return value

        if placeholder_ref == 'body':
            # return environment body
            return self.callable_node.nodelist

        arg_value = self.get_argument_placeholder_value(placeholder_ref, placeholder_node)

        if arg_value is not None:
            return arg_value

        raise ValueError("Invalid callable argument placeholder reference: ‘{}’"
                         .format(placeholder_ref))


    # ---

    def get_argument_key(self, placeholder_ref, placeholder_node=None):
        
        argument_key = None
        if placeholder_ref and placeholder_ref.isdigit():
            # numerical argument reference. Store index, starting from zero.
            argument_key = int(placeholder_ref) - 1
            if self.argument_number_offset is not None:
                argument_key += self.argument_number_offset

            if argument_key < 0 or argument_key >= self.num_arguments:
                expected_what = None
                if self.num_arguments == 0:
                    expected_what = "The callable accepts no numbered arguments"
                else:
                    expected_what = \
                        f"Expected a number between 1 and {self.num_arguments} (incl.)"
                e = LatexWalkerLocatedError(
                    f"Invalid argument number: ‘{placeholder_ref}’.  {expected_what}",
                    pos=(placeholder_node.pos if placeholder_node is not None else None)
                )
                e.set_pos_or_add_open_context_from_node(node=self.callable_node)
                raise e
        else:
            argument_key = placeholder_ref

        # logger.debug(f"Got argument replacement placeholder ref={repr(placeholder_ref)} "
        #              f"[→ type {type(placeholder_ref)} → key={argument_key}]")
        # logger.debug("Available argument values are %r", self.parsed_arguments_infos)

        if argument_key not in self.parsed_arguments_infos:
            lastnum = self.num_arguments
            if self.argument_number_offset is not None:
                lastnum -= self.argument_number_offset
            valid_arg_specs = f"numbers 1–{self.num_arguments-self.argument_number_offset}"
            if len(self.argument_names):
                valid_arg_specs = (
                    ",".join([f"‘{argname}’" for argname in self.argument_names ])
                    + " and " + valid_arg_specs
                )
            e = LatexWalkerLocatedError(
                f"Invalid argument name or index: ‘{placeholder_ref}’.  Valid argument "
                + f"specifiers are {valid_arg_specs}",
                pos=(placeholder_node.pos if placeholder_node is not None else None)
            )
            e.set_pos_or_add_open_context_from_node(node=self.callable_node)
            raise e

        return argument_key


    def get_parsed_argument_info(self, placeholder_ref, placeholder_node=None):
        
        argument_key = self.get_argument_key(placeholder_ref, placeholder_node)

        return self.parsed_arguments_infos[argument_key]


    def get_argument_placeholder_value(self, placeholder_ref, placeholder_node):
        
        argument_key = self.get_argument_key(placeholder_ref, placeholder_node)

        nodelist = self.parsed_arguments_infos[argument_key].get_content_nodelist()

        use_default = False
        if nodelist is None or (len(nodelist) == 1 and nodelist[0] is None):
            nodelist = []
            use_default = True

        if use_default:
            nodelist = self.get_default_argument_value_nodelist(
                argument_key,
                placeholder_node=placeholder_node,
            )
            if nodelist is None or (len(nodelist) == 1 and nodelist[0] is None):
                nodelist = []

        return nodelist


    def get_default_argument_value_flm_text(self, argument_key):

        argument_ref_user = None

        node = self.callable_node

        if isinstance(argument_key, int):
            # 0-th argument is "#1" so use "1" as argument key
            argument_ref_user = argument_key + 1
            if self.argument_number_offset is not None:
                argument_key -= self.argument_number_offset

            if argument_ref_user in self.default_argument_values:
                # all ok, found our requested default value
                return self.default_argument_values[argument_ref_user]

            # try to find default value by argument name instead.
            if node.nodeargd is None or node.nodeargd.arguments_spec_list is None \
               or argument_key < 0 \
               or argument_key >= len(node.nodeargd.arguments_spec_list):
                raise ValueError(
                    "Unexpected invalid argument_key={} for node={}".format(
                        repr(argument_key), repr(node)
                    )
                )

            argname = node.nodeargd.arguments_spec_list[argument_key].argname
            if not argname or argname not in self.default_argument_values:
                # no default value provided.
                return None

            # all ok, use ref by name
            argument_ref_user = argname
            return self.default_argument_values[argument_ref_user]

        # argument_key is an argname.

        if argument_key in self.default_argument_values:
            # all ok, found our requested default value
            argument_ref_user = argument_key
            return self.default_argument_values[argument_ref_user]

        # try to find by index instead
        if node.nodeargd is None or node.nodeargd.arguments_spec_list is None:
            raise ValueError("Unexpected invalid node arguments for node={}"
                             .format(repr(node)))
        for arg_j, arg_spec in enumerate(node.nodeargd.arguments_spec_list):
            if arg_spec.argname == argument_key:
                break
        else:
            raise ValueError("Unexpected invalid argument argname={} for node={}"
                             .format(repr(argument_key), repr(node)))
        # found arg_j
        argument_ref_user = arg_j + 1
        if self.argument_number_offset is not None:
            argument_ref_user -= self.argument_number_offset
        if argument_ref_user not in self.default_argument_values:
            # no default value provided.
            return None

        # all ok, found our requested default value
        return self.default_argument_values[argument_ref_user]

    def get_default_argument_value_nodelist(self, argument_key, placeholder_node):

        is_block_level = placeholder_node.parsing_state.is_block_level

        default_arg_flm_text = self.get_default_argument_value_flm_text(argument_key)

        if default_arg_flm_text is None:
            return []

        return self.compile_flm_text(
            default_arg_flm_text,
            add_what=f"default ‘{argument_key}’",
            is_block_level=is_block_level,
        )


    # ---

    def compile_flm_text(self, flm_text, add_what=None, is_block_level=None, 
                         parsing_state_delta=None):

        mc_parsing_state_delta = self.macro_content_parsing_state_delta
        callable_node = self.callable_node
        base_latex_walker = callable_node.latex_walker
        flm_environment = base_latex_walker.flm_environment

        what = f"{base_latex_walker.what}→{self.substitutor_manager.spec_object.get_what()}"
        if add_what:
            what += f"[{add_what}]"

        patched_callables = _make_patched_callables(flm_environment)

        if 'macros' in patched_callables:
            if 'macros' not in mc_parsing_state_delta.extend_latex_context:
                mc_parsing_state_delta.extend_latex_context['macros'] = []
            mc_parsing_state_delta.extend_latex_context['macros'] \
                += patched_callables['macros']

        if 'environments' in patched_callables:
            if 'environments' not in mc_parsing_state_delta.extend_latex_context:
                mc_parsing_state_delta.extend_latex_context['environments'] = []
            mc_parsing_state_delta.extend_latex_context['environments'] \
                += patched_callables['environments']

        if 'specials' in patched_callables:
            if 'specials' not in mc_parsing_state_delta.extend_latex_context:
                mc_parsing_state_delta.extend_latex_context['specials'] = []
            mc_parsing_state_delta.extend_latex_context['specials'] \
                += patched_callables['specials']

        # No, don't parse macro content with *outer* parsing state block mode.  The
        # way the macro content is parsed shouldn't depend on where it is inserted.
        #
        # if is_block_level is None:
        #     is_block_level = callable_node.parsing_state.is_block_level

        content_latex_walker = flm_environment.make_latex_walker(
            flm_text,
            is_block_level=is_block_level,
            parsing_mode=base_latex_walker.parsing_mode,
            resource_info=base_latex_walker.resource_info,
            standalone_mode=base_latex_walker.standalone_mode,
            tolerant_parsing=base_latex_walker.tolerant_parsing,
            what=what,
            input_lineno_colno_offsets=None,
        )

        content_parsing_state = mc_parsing_state_delta.get_updated_parsing_state(
            callable_node.parsing_state,
            content_latex_walker
        )
        if parsing_state_delta is not None:
            content_parsing_state = parsing_state_delta.get_updated_parsing_state(
                content_parsing_state,
                content_latex_walker
            )

        nodes, newpsdelta = content_latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser(),
            parsing_state=content_parsing_state
        )
        if newpsdelta is not None:
            logger.warning(
                f"Ignoring parsing state delta from compiling substitution nodes {what}"
            )

        return nodes





class MacroContentSubstitutorManager:

    MacroContentSubstitutorClass = MacroContentSubstitutor

    def __init__(self,
                 spec_object,
                 argument_number_offset=None,
                 default_argument_values=None,
                 ):
        super().__init__()
        self.spec_object = spec_object
        self.argument_number_offset = argument_number_offset
        self.default_argument_values = default_argument_values

    def make_macro_content_substitutor(self, callable_node):

        parsed_arguments_infos = \
            ParsedArgumentsInfo(node=callable_node).get_all_arguments_info()

        parsed_arguments_infos = self.filter_parsed_arguments_infos(
            parsed_arguments_infos,
            callable_node
        )

        return self.MacroContentSubstitutorClass(
            substitutor_manager=self,
            callable_node=callable_node,
            parsed_arguments_infos=parsed_arguments_infos,
            argument_number_offset=self.argument_number_offset,
            default_argument_values=self.default_argument_values,
        )

    def filter_parsed_arguments_infos(self, parsed_arguments_infos, callable_node):
        return parsed_arguments_infos

    def get_placeholder_value(
            self,
            placeholder_ref,
            placeholder_node,
            *,
            substitution_arg_info,
            callable_node,
            macro_content_substitutor
        ):
        # by default, try the corresponding method on the spec object.
        return self.spec_object.get_placeholder_value(
            placeholder_ref=placeholder_ref,
            placeholder_node=placeholder_node,
            substitution_arg_info=substitution_arg_info,
            callable_node=callable_node,
            macro_content_substitutor=macro_content_substitutor,
        )



# ------------------------------------------------------------------------------



class SubstitutionCallableSpecInfo(FLMSpecInfo):
    r"""
    A callable spec that describes a simple substitution of the
    macro/environment/specials (possibly with arguments) by user-defined custom
    FLM content (possibly with argument placeholders).

    - `arguments_spec_list` is the list of arguments specification objects
      describing the arguments accepted by the custom macro.  This parameter is
      as you'd give to :py:class:`FLMSpecInfo` or :py:class:`FLMMacroSpecBase`.
      As an exception, each element of the list can also be a dictionary, in
      which case the corresponding argument specification is constructed by
      passing the dictionary's contents as keyword arguments to the constructor
      of `LatexArgumentSpec()`.  E.g. `arguments_spec_list = [ {'parser': '[',
      'argname': 'argone'}, {'parser': '{', 'argname': 'argtwo'} ]`.

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

    MacroContentSubstitutorManagerClass = MacroContentSubstitutorManager

    def __init__(self,
                 spec_node_parser_type,
                 arguments_spec_list=None,
                 default_argument_values=None,
                 argument_number_offset=None,
                 content=None,
                 is_block_level=None,
                 macro_content_substitutor_manager=None,
                 render_time_substitution=False,
                 **kwargs,
                 ):
        
        # allow user to specify a latex argument spec as a dict
        if arguments_spec_list is not None and len(arguments_spec_list):
            arguments_spec_list = [ _get_arg_spec(arg) for arg in arguments_spec_list ]

        super().__init__(
            spec_node_parser_type=spec_node_parser_type,
            arguments_spec_list=arguments_spec_list,
            **kwargs
        )
        
        if macro_content_substitutor_manager is None:
            macro_content_substitutor_manager = self.MacroContentSubstitutorManagerClass(
                spec_object=self,
                default_argument_values=default_argument_values,
                argument_number_offset=argument_number_offset,
            )
        else:
            if default_argument_values is not None:
                logger.warning(
                    "Ignoring `default_argument_values` in SubstitutionCallableSpecInfo "
                    "constructor because you already provided a "
                    "macro_content_substitutor_manager instance."
                )
            if argument_number_offset is not None:
                logger.warning(
                    "Ignoring `argument_number_offset` in SubstitutionCallableSpecInfo "
                    "constructor because you already provided a "
                    "macro_content_substitutor_manager instance."
                )

        self.macro_content_substitutor_manager = macro_content_substitutor_manager

        if content is None:
            content = ''

        self.content_textmode = None
        self.content_mathmode = None
        if isinstance(content, str):
            self.content_textmode = content
            self.content_mathmode = content
        else:
            if 'textmode' in content:
                self.content_textmode = content['textmode']
            if 'mathmode' in content:
                self.content_mathmode = content['mathmode']

        self.render_time_substitution = render_time_substitution

        logger.debug("Constructing SimpleSubstitutionMacro, arguments_spec_list = %r; "
                     "content_textmode=%r, content_mathmode=%r, render_time_substitution=%r; "
                     "kwargs=%r",
                     arguments_spec_list, self.content_textmode, self.content_mathmode,
                     self.render_time_substitution, kwargs)

    def get_what(self):
        if hasattr(self, 'macroname'):
            return '“\\' + str(self.macroname) + '”'
        elif hasattr(self, 'environmentname'):
            return r'“\begin{' + str(self.environmentname) + r'}...\end{..}”'
        elif hasattr(self, 'specials_chars'):
            return r'“' + str(self.specials_chars) + r'”'
        return '(?callable?)'


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

        substitute_nodelist = self.compile_subst_nodes(node)

        node.flm_macro_replacement_flm_nodes = substitute_nodelist

        substitute_nodelist_parsing_state = substitute_nodelist.parsing_state

        substitute_node = node.latex_walker.make_node(
            LatexGroupNode,
            parsing_state=substitute_nodelist_parsing_state,
            delimiters=('',''),
            nodelist=substitute_nodelist,
            pos=node.pos,
            pos_end=node.pos_end,
        )

        node.flm_macro_replacement_flm_group_node = substitute_node

        if self.render_time_substitution:
            logger.debug("Node is to be substituted only at render time: %r", node)
            return

        logger.debug("setting substitute node = %r", substitute_node)
        node.flm_SUBSTITUTE_NODE = substitute_node


    def compile_subst_nodes(self, node):

        # compose & compile the flm text into nodes, including argument
        # placeholder nodes.

        #base_latex_walker = node.latex_walker
        #flm_environment = node.latex_walker.flm_environment

        macro_replacement_flm_text = node.flm_macro_replacement_flm_text

        macro_content_substitutor = \
            self.macro_content_substitutor_manager.make_macro_content_substitutor(
                callable_node=node
            )

        macro_content_substitutor.initialize()

        nodes = macro_content_substitutor.compile_flm_text(
            macro_replacement_flm_text,
        )

        return nodes


    def get_placeholder_value(
            self,
            placeholder_ref,
            placeholder_node,
            *,
            substitution_arg_info,
            callable_node,
            macro_content_substitutor
        ):
        # nothing by default.
        return None


    def render(self, node, render_context):

        if not self.render_time_substitution:
            logger.error(
                "Rendering substitution macro node that should have been "
                "substituted in node tree!"
            )

        logger.debug("Rendering substitution node %r", node)

        return render_context.fragment_renderer.render_nodelist(
            node.flm_macro_replacement_flm_nodes,
            render_context,
        )



class SubstitutionMacro(SubstitutionCallableSpecInfo):
    def __init__(self, macroname, **kwargs):
        super().__init__(
            macroname=macroname,
            spec_node_parser_type='macro',
            **kwargs
        )


class SubstitutionEnvironment(SubstitutionCallableSpecInfo):
    def __init__(self, environmentname, **kwargs):
        super().__init__(
            environmentname=environmentname,
            spec_node_parser_type='environment',
            **kwargs
        )



class SubstitutionSpecials(SubstitutionCallableSpecInfo):
    def __init__(self, specials_chars, **kwargs):
        super().__init__(
            specials_chars=specials_chars,
            spec_node_parser_type='specials',
            **kwargs
        )






class FeatureSubstMacros(Feature):

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
        if 'environments' not in definitions:
            definitions['environments'] = {}
        if 'specials' not in definitions:
            definitions['specials'] = {}

        self.definitions = definitions
        

    def add_latex_context_definitions(self):
        r"""
        Reimplement to add additional definitions to the latex context
        database.
        """
        return {
            'macros': [
                SubstitutionMacro(macroname=macroname, **specdef)
                for macroname, specdef in self.definitions['macros'].items()
            ],
            'environments': [
                SubstitutionEnvironment(environmentname=environmentname, **specdef)
                for environmentname, specdef in self.definitions['environments'].items()
            ],
            'specials': [
                SubstitutionSpecials(specials_chars=specials_chars, **specdef)
                for specials_chars, specdef in self.definitions['specials'].items()
            ],
        }
        

FeatureClass = FeatureSubstMacros
