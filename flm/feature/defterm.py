import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    ParsedArguments, ParsedArgumentsInfo
)
from pylatexenc.latexnodes import nodes as latexnodes_nodes

from .. import flmspecinfo
from ..flmspecinfo import (
    FLMMacroSpecBase, FLMEnvironmentSpecBase, TextFormatMacro,
)
from ..flmenvironment import (
    FLMParsingStateDeltaSetBlockLevel,
    FLMArgumentSpec,
)
from ._base import Feature
from . import refs


def simplify_trim_whitespace(x):
    # any type of space (tab, etc.) -> single space.  Also remove entirely
    # leading/trailing whitespace.
    return re.sub(r'\s+', ' ', x.strip())

def get_term_ref_label_verbatim(node_term_arg_nodelist):
    return simplify_trim_whitespace(
        node_term_arg_nodelist.latex_verbatim()
    )


# ------------------------------------------------------------------------------

class DefineTermEnvironment(FLMEnvironmentSpecBase):

    is_block_level = True

    allowed_in_standalone_mode = False

    allowed_ref_label_prefixes = ('topic', )
    
    defterm_ref_type = 'defterm'

    def __init__(self, environmentname, render_with_term=True, render_with_term_suffix=': ',
                 **kwargs):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=[
                FLMArgumentSpec('{', argname='term'),
                flmspecinfo.label_arg,
            ],
            body_parsing_state_delta=
                FLMParsingStateDeltaSetBlockLevel(is_block_level=self.is_block_level),
            **kwargs
        )
        self.render_with_term = render_with_term
        self.render_with_term_suffix = render_with_term_suffix
        if self.render_with_term:
            mspec = TextFormatMacro('', text_formats=['defterm-term'])
            mspec.is_block_heading = True
            self.render_term_text_format_spec = mspec
        
    _fields = (
        'environmentname', 'render_with_term', 'render_with_term_suffix',
    )

    def postprocess_parsed_node(self, node):
        node_args = \
            ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('term','label'),
            )

        term_flm_ref_label_verbatim = \
            get_term_ref_label_verbatim(node_args['term'].get_content_nodelist())
        node.flmarg_term_flm_ref_label_verbatim = term_flm_ref_label_verbatim
        #node.flmarg_term_safe_target_id = get_term_safe_target_id(term_flm_ref_label_verbatim)

        # pick out \label{}'s, if any
        node.flmarg_labels = flmspecinfo.helper_collect_labels(
            node_args['label'],
            self.allowed_ref_label_prefixes
        )

        node.flm_referenceable_infos = [
            refs.ReferenceableInfo(
                kind='defterm',
                formatted_ref_flm_text=node.flmarg_term_flm_ref_label_verbatim,
                labels=(
                    [(self.defterm_ref_type, node.flmarg_term_flm_ref_label_verbatim)]
                    + list(node.flmarg_labels)
                ),
            )
        ]

        return node

    def render(self, node, render_context):

        formatted_ref_flm_text = node.flmarg_term_flm_ref_label_verbatim

        logical_state = render_context.get_logical_state('feature.defterm')
        if 'currently_defining_terms' not in logical_state:
            logical_state['currently_defining_terms'] = []

        cur_def_terms = list(logical_state['currently_defining_terms'])
        cur_def_terms.append( formatted_ref_flm_text )

        with render_context.push_logical_state('feature.defterm',
                                               'currently_defining_terms', cur_def_terms):

            logger.debug("Pushed logical state, cur_def_terms = %r", cur_def_terms)

            # register the term
            if render_context.supports_feature('refs'):
                refs_mgr = render_context.feature_render_manager('refs')
                for referenceable_info in node.flm_referenceable_infos:
                    refs_mgr.register_reference_referenceable(
                        node=node,
                        referenceable_info=referenceable_info,
                        #target_href=target_href,
                    )

            thenodelist = node.nodelist

            if self.render_with_term:
                environ = render_context.doc.environment
                term_fragment = environ.make_fragment(
                    formatted_ref_flm_text + self.render_with_term_suffix,
                    standalone_mode=True
                )
                intro_node = term_fragment.latex_walker.make_node(
                    latexnodes_nodes.LatexMacroNode,
                    macroname='',
                    spec=self.render_term_text_format_spec,
                    macro_post_space='',
                    parsing_state=term_fragment.nodes.parsing_state,
                    nodeargd=ParsedArguments(
                        arguments_spec_list=self.render_term_text_format_spec.arguments_spec_list,
                        argnlist=[
                            term_fragment.nodes,
                        ]
                    ),
                    pos=node.pos,
                    pos_end=node.pos_end
                )
                self.render_term_text_format_spec.finalize_node( intro_node )

                thenodelist = term_fragment.latex_walker.make_nodelist(
                    [ intro_node ] + list(thenodelist),
                    parsing_state=node.nodelist.parsing_state,
                )

            # A call to render_semantic_block() is needed around the rendered
            # nodelist so that we can attach a target_id anchor to the content.
            result = render_context.fragment_renderer.render_semantic_block(
                content=render_context.fragment_renderer.render_nodelist(
                    thenodelist,
                    render_context=render_context,
                    is_block_level=True,
                ),
                role='defterm',
                render_context=render_context,
                target_id=node.flm_referenceable_infos[0].get_target_id(),
            )

            logger.debug("End of logical state push/")

        return result


    def recompose_pure_latex(self, node, recomposer, visited_results_arguments,
                             visited_results_body, **kwargs):

        # produce \begin{defterm}{The Term}\label{one}\label{two}
        # ... \end{defterm}, as per the original FLM code, but make sure that
        # the labels are safe!

        if node.flm_referenceable_infos is None:
            raise LatexWalkerLocatedError(
                "Recomposing pure LaTeX: Invalid referenceable info in defterm node "
                + repr(node),
                pos=node.pos
            )
        
        # flm_environment = node.latex_walker.flm_environment

        s = r'\begin{' + str(node.environmentname) + '}'
        s += str(visited_results_arguments[0]) # first mandatory argument, the term itself

        # term label itself is already in flm_referenceable_infos and will be
        # turned into a safe label --
        for referenceable_info in node.flm_referenceable_infos:
            for ref_type, ref_label in referenceable_info.labels:
                safe_label_info = recomposer.make_safe_label('ref', ref_type, ref_label)
                safe_label = safe_label_info['safe_label']
                s += r'\label{' + str(safe_label) + '}'
            
        s += recomposer.recompose_nodelist(visited_results_body, node)
        s += r'\end{' + str(node.environmentname) + '}'

        return s

        




class RefTermMacro(FLMMacroSpecBase):

    allowed_in_standalone_mode = False
    delayed_render = True

    defterm_ref_type = 'defterm'

    def __init__(self, macroname, **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                FLMArgumentSpec('[', argname='ref_term',
                                flm_doc=r'The term that is being referenced.  '
                                r'May be omitted if it coincides with \verba{term}'),
                FLMArgumentSpec('{', argname='term',
                                flm_doc=r'The term the exact way it should '
                                r'be typeset at this point'),
            ],
            **kwargs
        )

    _fields = ( 'macroname', )

    def get_flm_doc(self):
        return r""" Reference a term defined somewhere else in a
        \verbcode+\begin{defterm}…\end{defterm}+ environment.  This will
        typically produce a link in HTML output for instance to the location
        where this term is defined.  In case you need to typeset the term
        differently than when you defined it (e.g., because of a plural,
        capital/lower case, other declination), use the optional argument to
        give the term exactly as defined and specify the term as it should
        appear in the main argument.  E.g.
        \verbcode+These are \term[Markov chain]{Markov chains}+"""

    def postprocess_parsed_node(self, node):
        node_args = \
            ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('ref_term', 'term',),
            )
        
        if node_args['ref_term'].was_provided():
            node.flm_term_flm_show_term_nodelist = \
                node_args['term'].get_content_nodelist()
            node.flm_term_flm_ref_label_verbatim = \
                get_term_ref_label_verbatim(node_args['ref_term'].get_content_nodelist())
        else:
            node.flm_term_flm_show_term_nodelist = node_args['term'].get_content_nodelist()
            node.flm_term_flm_ref_label_verbatim = \
                get_term_ref_label_verbatim(node.flm_term_flm_show_term_nodelist)
        
        node.flmarg_ref = (self.defterm_ref_type, node.flm_term_flm_ref_label_verbatim)
        node.flm_ref_info = {
            'ref_list': [ node.flmarg_ref ],
            'display_content_nodelist': node.flm_term_flm_show_term_nodelist,
        }


    def prepare_delayed_render(self, node, render_context):

        logger.debug("Prepare delayed render - node = %r", node)

        # flag to set a special style to \term{...} calls *within the defterm
        # environment that is defining that precise term*
        is_currently_defining_term = False
        logical_state = render_context.get_logical_state('feature.defterm')
        if 'currently_defining_terms' in logical_state:
            if (node.flm_term_flm_ref_label_verbatim
                in logical_state['currently_defining_terms']):
                is_currently_defining_term = True

        logger.debug("Queried logical state, cur_def_terms = %r; currently defining term? %s",
                     logical_state.get('currently_defining_terms'),
                     is_currently_defining_term)

        mgr = render_context.feature_render_manager('defterm')
        mgr.register_term_node_info(
            node,
            is_currently_defining_term=is_currently_defining_term
        )


    def render(self, node, render_context):

        term_flm_show_term_nodelist = node.flm_term_flm_show_term_nodelist
        term_flm_ref_label_verbatim = node.flm_term_flm_ref_label_verbatim
        
        if not render_context.supports_feature('refs'):
            # no support for 'refs' -- simply render the term, no reference
            return render_context.fragment_renderer.render_nodelist(
                term_flm_show_term_nodelist,
                render_context=render_context,
                is_block_level=False,
            )
        
        resource_info = node.latex_walker.resource_info

        # grab the reference
        refs_mgr = render_context.feature_render_manager('refs')
        ref_instance = refs_mgr.get_ref(
            self.defterm_ref_type,
            term_flm_ref_label_verbatim,
            resource_info,
        )

        mgr = render_context.feature_render_manager('defterm')
        term_info = mgr.get_term_node_info(node)

        is_currently_defining_term = term_info['is_currently_defining_term']

        if is_currently_defining_term:

            return render_context.fragment_renderer.render_text_format(
                ['defterm-term', 'term-in-defining-defterm'],
                nodelist=term_flm_show_term_nodelist,
                render_context=render_context,
            )

        return render_context.fragment_renderer.render_link(
            'term',
            href=ref_instance.target_href,
            display_nodelist=term_flm_show_term_nodelist,
            render_context=render_context,
            annotations=[],
        )

    def recompose_pure_latex(self, node, recomposer, visited_results_arguments, **kwargs):

        ref_type = self.defterm_ref_type
        ref_label = node.flm_term_flm_ref_label_verbatim

        safe_label_info = recomposer.make_safe_label('ref', ref_type, ref_label)
        safe_label = safe_label_info['safe_label']

        return (
            '\\flmTerm{' + node.macroname + '}{'+safe_label+'}'
            + ('{' + visited_results_arguments[0] + '}'
               if visited_results_arguments[0] else '{}')
            + visited_results_arguments[1]
        )



# --------------------------------------

class FeatureDefTerm(Feature):

    feature_name = 'defterm'
    feature_title = 'Definition terms'

    class RenderManager(Feature.RenderManager):
        def initialize(self):
            self.registered_term_node_infos = {}
        def register_term_node_info(self, node, **kwargs):
            self.registered_term_node_infos[self.get_node_id(node)] = kwargs
        def get_term_node_info(self, node):
            return self.registered_term_node_infos[self.get_node_id(node)]


    render_defterm_with_term = True
    render_defterm_with_term_suffix = ': '

    def __init__(self, render_defterm_with_term=None, render_defterm_with_term_suffix=None):
        super().__init__()
        if render_defterm_with_term is not None:
            self.render_defterm_with_term = render_defterm_with_term
        if render_defterm_with_term_suffix is not None:
            self.render_defterm_with_term_suffix = render_defterm_with_term_suffix


    def add_latex_context_definitions(self):
        return dict(
            macros=[
                RefTermMacro('term',)
            ],
            environments=[
                DefineTermEnvironment(
                    'defterm',
                    render_with_term=self.render_defterm_with_term,
                    render_with_term_suffix=self.render_defterm_with_term_suffix,
                )
            ]
        )

# ------------------------------------------------

FeatureClass = FeatureDefTerm
