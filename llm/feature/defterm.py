import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    LatexArgumentSpec, ParsedArguments, ParsedArgumentsInfo
)
from pylatexenc.latexnodes import nodes as latexnodes_nodes

from .. import llmspecinfo
from ..llmspecinfo import (
    LLMMacroSpecBase, LLMEnvironmentSpecBase, TextFormatMacro,
)
from ..llmenvironment import (
    LLMParsingStateDeltaSetBlockLevel,
    LLMArgumentSpec,
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
# def get_term_safe_target_id(term_ref_label_verbatim):
#     return re.sub(r'[^a-zA-Z0-9-]', lambda m: f'_{ord(m.group()):x}X',
#                   term_ref_label_verbatim)


# ------------------------------------------------------------------------------

class DefineTermEnvironment(LLMEnvironmentSpecBase):

    is_block_level = True

    allowed_in_standalone_mode = False

    allowed_ref_label_prefixes = ('topic', )
    
    defterm_ref_type = 'defterm'

    def __init__(self, environmentname, render_with_term=True, render_with_term_suffix=': ',
                 **kwargs):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=[
                LLMArgumentSpec('{', argname='term'),
                llmspecinfo.label_arg,
            ],
            **kwargs
        )
        self.render_with_term = render_with_term
        self.render_with_term_suffix = render_with_term_suffix
        if self.render_with_term:
            mspec = TextFormatMacro('', text_formats=['defterm-term'])
            mspec.is_block_heading = True
            self.render_term_text_format_spec = mspec

        self.body_parsing_state_delta = \
            LLMParsingStateDeltaSetBlockLevel(is_block_level=self.is_block_level)
        

    def postprocess_parsed_node(self, node):
        node_args = \
            ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('term','label'),
            )

        term_llm_ref_label_verbatim = \
            get_term_ref_label_verbatim(node_args['term'].get_content_nodelist())
        node.llmarg_term_llm_ref_label_verbatim = term_llm_ref_label_verbatim
        #node.llmarg_term_safe_target_id = get_term_safe_target_id(term_llm_ref_label_verbatim)

        # pick out \label{}'s, if any
        node.llmarg_labels = llmspecinfo.helper_collect_labels(
            node_args['label'],
            self.allowed_ref_label_prefixes
        )

        node.llm_referenceable_infos = [
            refs.ReferenceableInfo(
                formatted_ref_llm_text=node.llmarg_term_llm_ref_label_verbatim,
                labels=(
                    [(self.defterm_ref_type, node.llmarg_term_llm_ref_label_verbatim)]
                    + list(node.llmarg_labels)
                ),
            )
        ]

        return node

    def render(self, node, render_context):

        formatted_ref_llm_text = node.llmarg_term_llm_ref_label_verbatim

        # register the term
        if render_context.supports_feature('refs'):
            refs_mgr = render_context.feature_render_manager('refs')
            for referenceable_info in node.llm_referenceable_infos:
                refs_mgr.register_reference_referenceable(
                    node=node,
                    referenceable_info=referenceable_info,
                    #target_href=target_href,
                )

        thenodelist = node.nodelist

        if self.render_with_term:
            environ = render_context.doc.environment
            term_fragment = environ.make_fragment(
                formatted_ref_llm_text + self.render_with_term_suffix,
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
        return render_context.fragment_renderer.render_semantic_block(
            content=render_context.fragment_renderer.render_nodelist(
                thenodelist,
                render_context=render_context,
                is_block_level=True,
            ),
            role='defterm',
            target_id=node.llm_referenceable_infos[0].get_target_id(),
        )


class RefTermMacro(LLMMacroSpecBase):

    allowed_in_standalone_mode = False

    defterm_ref_type = 'defterm'

    def __init__(self, macroname, **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                LLMArgumentSpec('[', argname='ref_term'),
                LLMArgumentSpec('{', argname='term'),
            ],
            **kwargs
        )

    def postprocess_parsed_node(self, node):
        node_args = \
            ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('ref_term', 'term',),
            )
        
        if node_args['ref_term'].was_provided():
            node.llm_term_llm_show_term_nodelist = \
                node_args['term'].get_content_nodelist()
            node.llm_term_llm_ref_label_verbatim = \
                get_term_ref_label_verbatim(node_args['ref_term'].get_content_nodelist())
        else:
            node.llm_term_llm_show_term_nodelist = node_args['term'].get_content_nodelist()
            node.llm_term_llm_ref_label_verbatim = \
                get_term_ref_label_verbatim(node.llm_term_llm_show_term_nodelist)
        
        node.llmarg_ref = (self.defterm_ref_type, node.llm_term_llm_ref_label_verbatim)


    def render(self, node, render_context):

        term_llm_show_term_nodelist = node.llm_term_llm_show_term_nodelist
        term_llm_ref_label_verbatim = node.llm_term_llm_ref_label_verbatim
        
        if not render_context.supports_feature('refs'):
            # no support for 'refs' -- simply render the term, no reference
            return render_context.fragment_renderer.render_nodelist(
                term_llm_show_term_nodelist,
                render_context=render_context,
                is_block_level=False,
            )
        
        resource_info = node.latex_walker.resource_info

        # grab the reference
        refs_mgr = render_context.feature_render_manager('refs')
        ref_instance = refs_mgr.get_ref(
            self.defterm_ref_type,
            term_llm_ref_label_verbatim,
            resource_info,
        )

        return render_context.fragment_renderer.render_link(
            'term',
            href=ref_instance.target_href,
            display_nodelist=term_llm_show_term_nodelist,
            render_context=render_context,
        )



# --------------------------------------

class FeatureDefTerm(Feature):

    feature_name = 'defterm'

    FeatureDocumentManager = None
    FeatureRenderManager = None

    render_defterm_with_term = True
    render_defterm_with_term_suffix = ': '

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
