
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    LatexWalkerLocatedError,
    ParsedArgumentsInfo,
    ParsingStateDeltaEnterMathMode,
)
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.macrospec import (
    MacroSpec,
    LatexEnvironmentBodyContentsParser,
    ParsingStateDeltaExtendLatexContextDb,
)

from ..flmspecinfo import FLMMacroSpecBase, FLMEnvironmentSpecBase
from ..flmenvironment import FLMArgumentSpec

from ._base import Feature

from ..counter import build_counter_formatter, Counter





_default_math_environment_names = (
    'equation',
    'equation*',
    'align',
    'align*',
    'gather',
    'gather*',
)


eq_default_counter_formatter_spec = {
    'format_num': { 'template': '(${arabic})' },
    'prefix_display': {
        'singular': 'Eq.~',
        'plural': 'Eqs.~',
        'capital': {
            'singular': 'Equation',
            'plural': 'Equations',
        },
    },
    'delimiters': ('',''),
    'join_spec': 'compact',
}


class FeatureMath(Feature):

    feature_name = 'math'
    feature_title = 'Mathematical typesetting: equations and equation references'

    def feature_flm_doc(self):
        return (
            r"""
            You can use the following environments and macros to typeset pretty display
            math equations, create labels, and refer to them at other places in your
            document.

            Note that all equation labels must begin with the prefix \verbcode{"""
            + self.eqref_ref_type + r""":}."""
        )

    feature_optional_dependencies = [ 'refs' ]

    feature_default_config = {
        'counter_formatter': eq_default_counter_formatter_spec,
    }

    def __init__(
            self,
            counter_formatter=None,
            math_environment_names=None,
            eqref_macro_name='eqref',
            eqref_ref_type='eq',
    ):
        super().__init__()

        if counter_formatter is None:
            counter_formatter = self.feature_default_config['counter_formatter']
        self.counter_formatter = build_counter_formatter(
            counter_formatter,
            eq_default_counter_formatter_spec,
            counter_formatter_id='eq',
        )

        if math_environment_names is None:
            math_environment_names = _default_math_environment_names
        self.math_environment_names = math_environment_names

        self.eqref_macro_name = eqref_macro_name
        self.eqref_ref_type = eqref_ref_type

    class DocumentManager(Feature.DocumentManager):
        def initialize(self):
            pass
            
    class RenderManager(Feature.RenderManager):
        def initialize(self):
            self.equation_counter = Counter(self.feature.counter_formatter)

            self.equation_info_by_node = {}

            if self.render_context.supports_feature('refs'):
                refs_mgr = self.render_context.feature_render_manager('refs')
                refs_mgr.register_counter_formatter(
                    counter_formatter=self.feature.counter_formatter
                )

        def new_numbered_display_math(self, node, lineno, custom_tag_flm_text=None):

            key = (self.get_node_id(node), lineno)
            if key in self.equation_info_by_node:
                return self.equation_info_by_node[key]

            if custom_tag_flm_text is not None:
                formatted_ref_flm_text = custom_tag_flm_text
                eq_id = f"_{key[0]}"
                if lineno:
                    eq_id += f"-{lineno}"
                eq_counter_number = None
            else:
                eq_id, formatted_ref_flm_text = \
                    self.equation_counter.step_and_format_flm()
                eq_counter_number = eq_id

            info = (eq_id, formatted_ref_flm_text, eq_counter_number)
            self.equation_info_by_node[key] = info
            return info


    def add_latex_context_definitions(self):
        environments = [
            MathEnvironment(
                math_environment_name,
            )
            for math_environment_name in self.math_environment_names
        ]
        macros = []
        if self.eqref_macro_name is not None:
            macros.append(
                MathEqrefMacro(
                    macroname=self.eqref_macro_name,
                    ref_type=self.eqref_ref_type
                ),
            )

        return dict(macros=macros, environments=environments)



# ---

class _EmptyClass:
    pass

class _ProxyNodeWithRecomposedLatex:

    pos = None
    pos_end = None

    def __init__(self, verbatim):
        self._verbatim = verbatim

        # fake specinfo for recomposer
        self.flm_specinfo = _EmptyClass()
        self.flm_specinfo.recompose_flm_text = \
            lambda node, recomposer, **kw: \
                self.recompose_flm_text(node, recomposer=recomposer, **kw)

    def accept_node_visitor(self, visitor):
        return visitor.visit_unknown_node(self)

    def recompose_flm_text(self, node, recomposer, **kwargs):
        return self._verbatim

    def latex_verbatim(self):
        return self._verbatim



class MathEnvironment(FLMEnvironmentSpecBase):

    # Nope! This environment adds reference labels in general
    #allowed_in_standalone_mode = True

    def get_flm_doc(self):
        return r"""Display equation typeset as the corresponding LaTeX environment."""

    def __init__(self, environmentname, is_numbered=None):
        super().__init__(
            environmentname=environmentname
        )
        if is_numbered is not None:
            self.is_numbered = is_numbered
        else:
            self.is_numbered = (environmentname[-1:] != '*') # align*, gather*, etc. 
        
        if not self.is_numbered:
            # Okay, equation is not numbered; it can appear in standalone mode.
            self.allowed_in_standalone_mode = True

    _fields = ('environmentname', 'is_numbered', )

    def make_body_parsing_state_delta(self, token, nodeargd, arg_parsing_state_delta,
                                      latex_walker, **kwargs):
        return ParsingStateDeltaEnterMathMode()

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        MacroSpec('label', arguments_spec_list=[
                            FLMArgumentSpec(
                                parser=latexnodes_parsers.LatexCharsGroupParser(
                                    delimiters=('{','}'),
                                ),
                                argname='label',
                            ),
                        ]),
                        MacroSpec('tag', arguments_spec_list=[
                            FLMArgumentSpec(
                                parser='*',
                                argname='tag_star',
                            ),
                            FLMArgumentSpec(
                                parser=latexnodes_parsers.LatexCharsGroupParser(
                                    delimiters=('{','}'),
                                ),
                                argname='text',
                            ),
                        ]),
                        MacroSpec('\\', arguments_spec_list=[]),
                    ],
                )
            )
        )

    def postprocess_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        #logger.debug("finalizing math environment node: node = %r", node)

        # find and register and \label nodes
        node.flm_equation_lines_labels_infos = []

        if not self.is_numbered:
            return node

        def init_last_line_info():
            return {
                'labels_info': [],
                'custom_tag_flm_text': None,
                'nonumber': False,
                'line_nodelist': [], # node list w/o \label or \tag content
            }
        last_line_info = dict(init_last_line_info())
        def _flush_last_equation_line_labels_infos(newline_node=None):
            node.flm_equation_lines_labels_infos.append({
                'labels': list(last_line_info['labels_info']),
                'custom_tag_flm_text': last_line_info['custom_tag_flm_text'],
                'nonumber': last_line_info['nonumber'],
                'line_nodelist': list(last_line_info['line_nodelist']),
                'newline_node': newline_node,
            })
            last_line_info.update(init_last_line_info())

        last_node_is_newline = False
        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'label':
                # this is a \label command -- register it for this equation line

                # extract ref_type, ref_label and store these values
                ref_label_node_args = \
                    ParsedArgumentsInfo(node=n).get_all_arguments_info(
                        ('label',),
                    )
                ref_label_full = ref_label_node_args['label'].get_content_as_chars()

                if ':' in ref_label_full:
                    ref_type, ref_label = ref_label_full.split(':', 1)
                else:
                    ref_type, ref_label = None, ref_label_full

                info = {
                    'node': n,
                    'label': (ref_type, ref_label),
                }

                last_line_info['labels_info'].append(info)

            elif n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'tag':
                # custom tag for this line
                tag_node_args = \
                    ParsedArgumentsInfo(node=n).get_all_arguments_info(
                        ('tag_star', 'text'),
                    )

                custom_tag_flm_text = \
                    tag_node_args['text'].get_content_nodelist().latex_verbatim()
                if not tag_node_args['tag_star'].was_provided():
                    custom_tag_flm_text = f'({custom_tag_flm_text})'

                last_line_info['custom_tag_flm_text'] = custom_tag_flm_text

            elif n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'nonumber':
                # omit equation number on this line
                last_line_info['nonumber'] = True

            elif n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == '\\':
                _flush_last_equation_line_labels_infos(n)
                last_node_is_newline = True

            else:
                last_line_info['line_nodelist'].append(n)
            
                if (not n.isNodeType(latexnodes_nodes.LatexCommentNode)
                    and not (n.isNodeType(latexnodes_nodes.LatexCharsNode)
                             and not n.chars.strip())
                    ):
                    last_node_is_newline = False
                
        if not last_node_is_newline:
            _flush_last_equation_line_labels_infos()

        # a consistency check -- check that we don't have \label and \nonumber
        # on the same line
        for linej, lineinfo in enumerate(node.flm_equation_lines_labels_infos):
            if lineinfo['nonumber'] and \
               (len(lineinfo['labels']) or lineinfo['custom_tag_flm_text']):
                found_stuff_list = []
                if len(lineinfo['labels']):
                    labels_text = [ lblinfo['label'] for lblinfo in lineinfo['labels'] ]
                    labels_text_joined = ', '.join(['‘'+ln+'’' for ln in labels_text])
                    found_stuff_list.append(
                        f"\\label label(s) {labels_text_joined}"
                    )
                if lineinfo['custom_tag_flm_text']:
                    found_stuff_list.append(
                        f"custom \\tag “{lineinfo['custom_tag_flm_text']}”"
                    )
                found_stuff = " and ".join(found_stuff_list)
                raise LatexWalkerParseError(
                    f"You can't have \\nonumber and \\label/\\tag on the same equation line, "
                    f"found {found_stuff}",
                    pos=(lineinfo['newline_node'] or node).pos
                )

        return node


    def render(self, node, render_context):
        r"""
        """
        environmentname = node.environmentname

        if not render_context.supports_feature('math'):
            raise ValueError("Feature 'math' is not enabled, cannot render math environment")

        math_mgr = render_context.feature_render_manager('math')

        refs_mgr = None
        if render_context.supports_feature('refs'):
            refs_mgr = render_context.feature_render_manager('refs')

        # create an alternative node list
        nodelist = list(node.nodelist)

        target_id = None

        lineno = 0
        for line_infos in node.flm_equation_lines_labels_infos:

            nonumber = line_infos['nonumber']

            if nonumber:
                continue

            custom_tag_flm_text = line_infos['custom_tag_flm_text']

            # add equation instance
            eq_id, formatted_ref_flm_text, eq_counter_number = \
                math_mgr.new_numbered_display_math(node, lineno, custom_tag_flm_text)

            this_target_id = f'equation-{eq_id}'
            if target_id is None:
                # target_id refers to the first equation in an equation list
                target_id = this_target_id

            if not custom_tag_flm_text:
                # insert the automatically generated tag at appropriate location
                # in nodelist
                newline_node = line_infos['newline_node']
                if newline_node is not None:
                    i = nodelist.index(newline_node)
                else:
                    i = len(nodelist)
                nodelist.insert(
                    i,
                    # quick & dirty....
                    _ProxyNodeWithRecomposedLatex(
                        r'\tag*{' + formatted_ref_flm_text + r'}'
                    )
                )

            # USE THE FIRST TARGET_ID ONLY.  THIS IS BECAUSE WE DON'T PIN DOWN
            # THE TARGET_IDs OF INTERMEDIATE EQUATION LINES IN THE HTML OUTPUT.
            this_target_id = target_id

            # register the reference
            if refs_mgr is not None and render_context.is_first_pass:
                for label_info in line_infos['labels']:
                    (ref_type, ref_label) = label_info['label']
                    counter_formatter_id = \
                        math_mgr.feature.counter_formatter.counter_formatter_id
                    refs_mgr.register_reference(
                        ref_type, ref_label,
                        node=node, formatted_ref_flm_text=formatted_ref_flm_text,
                        target_href=f'#{this_target_id}',
                        counter_value=eq_counter_number,
                        counter_formatter_id=counter_formatter_id
                    )

            lineno += 1


        return render_context.fragment_renderer.render_math_content(
            ('', ''),
            latexnodes_nodes.LatexNodeList(nodelist),
            render_context,
            'display',
            environmentname=environmentname,
            target_id=target_id,
        )


    def recompose_pure_latex(self, node, recomposer, visited_results_arguments, 
                             visited_results_body, **kwargs):

        # we'll have to recompose the body again, oh well ...

        s = r'\begin{' + node.environmentname + '}'
        s += "".join([ x for x in visited_results_arguments if x is not None ])

        # do the body line-by-line, correcting labels as necessary.
        s_lines = []
        for line_infos in node.flm_equation_lines_labels_infos:

            s_line = ''

            for n in line_infos['line_nodelist']:
                s_line += recomposer.recompose_pure_latex( n ) ["latex"]

            s_line = s_line.strip()  # ??

            if line_infos['custom_tag_flm_text'] is not None:
                s_line += r'\tag*{' + recomposer.recompose_pure_latex(
                    line_infos['custom_tag_flm_text']
                ) ["latex"] + '}'

            for label_info in line_infos['labels']:
                (ref_type, ref_label) = label_info['label']
                safe_label_info = recomposer.make_safe_label('ref', ref_type, ref_label)
                s_line += r'\label{' + safe_label_info['safe_label'] + '}'

            if line_infos['nonumber']:
                s_line += r'\nonumber '

            logger.debug("adding line = \n%r\n... from line_infos=%r", s_line, line_infos)

            s_lines.append(s_line)

        s += '\\\\\n'.join( s_lines )

        s += r'\end{' + node.environmentname + '}'

        return s


class MathEqrefMacro(FLMMacroSpecBase):

    delayed_render = True

    allowed_in_standalone_mode = False
    r"""
    Reference commands are definitly not allowed in standalone mode
    """

    def __init__(self, macroname='eqref', ref_type='eq', **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                FLMArgumentSpec(
                    latexnodes_parsers.LatexCharsGroupParser(),
                    argname='ref_label',
                    flm_doc=(r'Equation label.  Must begin with the prefix ‘\verbtext+'
                             + ref_type + ':+’'),
                )
            ],
        )
        self.ref_type = ref_type

    _fields = ('macroname', 'ref_type', )

    def get_flm_doc(self):
        return (
            r"""Refer to an equation by its label.  The argument must be
            a valid equation label which starts with \verbcode+""" + self.ref_type
            + r""":+.  You can place labels in display equations using the
            \verbcode+\label{…}+ macro."""
        )

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('ref_label',),
        )
        
        ref_type = None
        ref_label = node_args['ref_label'].get_content_as_chars()
        if ':' in ref_label:
            ref_type, ref_label = ref_label.split(':', 1)

        if ref_type != self.ref_type:
            raise LatexWalkerLocatedError(
                f"Equation labels must begin with “{self.ref_type}:” "
                f"(error in ‘\\{node.macroname}’)",
                pos=node.pos
            )

        node.flmarg_ref = (ref_type, ref_label)
        node.flm_ref_info = {
            'ref_list': [ node.flmarg_ref ],
            'display_content_nodelist': None,
        }


    def prepare_delayed_render(self, node, render_context):
        pass

    def render(self, node, render_context):

        ref_type, ref_label = node.flmarg_ref

        refs_mgr = render_context.feature_render_manager('refs')

        resource_info = node.latex_walker.resource_info

        try:
            return refs_mgr.render_ref(
                ref_type, ref_label,
                None,
                resource_info,
                counter_with_prefix=False, # no "Eq.~" prefix
            )
        except Exception as e:
            logger.error(f"Failed to resolve reference to ‘{ref_type}:{ref_label}’: {e} "
                         f"in ‘{node.latex_verbatim()}’ @ {node.format_pos()}")
            raise LatexWalkerLocatedError(
                f"Unable to resolve reference to ‘{ref_type}:{ref_label}’. {e}",
                pos=node.pos,
            )

    def recompose_pure_latex(self, node, recomposer, visited_results_arguments, **kwargs):
        
        # use raw label, not safe label for now.  FIXME: convert all to safe labels...

        safe_label_info = recomposer.make_safe_label(
            'ref', node.flmarg_ref[0], node.flmarg_ref[1]
        )

        return r'\eqref{' + safe_label_info['safe_label'] + '}'


# ------------------------------------------------

FeatureClass = FeatureMath
