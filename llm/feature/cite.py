import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import ParsedArgumentsInfo

from ..llmspecinfo import LLMMacroSpecBase
from ..llmfragment import LLMFragment
from ..llmenvironment import LLMArgumentSpec

from ..counter import build_counter_formatter

from ._base import Feature

from .endnotes import EndnoteCategory


_cite_default_counter_formatter_spec = {
    'format_num': 'arabic',
    'prefix_display': None,
    'delimiters': ('[',']'),
    'join_spec': 'compact',
}


# class CitationEndnoteCategory(EndnoteCategory):
#     def __init__(self, counter_formatter='arabic'): #, citation_delimiters=(None,None)):

#         self.counter_formatter = build_counter_formatter(
#             counter_formatter,
#             _cite_default_counter_formatter_spec,
#         )
#         # self.inner_counter_formatter_fn = counter_formatter
#         # if self.inner_counter_formatter_fn in counter.standard_counter_formatters:
#         #     self.inner_counter_formatter_fn = \
#         #         counter.standard_counter_formatters[self.inner_counter_formatter_fn]

#         # self.citation_delimiters = citation_delimiters

#         # full_counter_formatter = lambda x: (
#         #     self.citation_delimiters[0]
#         #     + self.inner_counter_formatter_fn(x)
#         #     + self.citation_delimiters[1]
#         # )

#         super().__init__(
#             'citation',
#             counter_formatter=self.counter_formatter, #lambda n: self.counter_formatter.format_llm(n),
#             #                =full_counter_formatter,
#             heading_title='References',
#         )



class FeatureExternalPrefixedCitations(Feature):

    feature_name = 'citations'
    feature_title = 'Citations'

    feature_optional_dependencies = [ 'endnotes' ]

    use_endnotes = True

    class DocumentManager(Feature.DocumentManager):
        def initialize(self, use_endnotes=None):
            if use_endnotes is not None:
                self.use_endnotes = use_endnotes
            else:
                self.use_endnotes = self.feature.use_endnotes

            if self.use_endnotes:
                endnotes_mgr = self.doc.feature_document_manager('endnotes')
                self.endnote_category = EndnoteCategory(
                    'citation',
                    counter_formatter=self.feature.counter_formatter,
                    #citation_delimiters=self.feature.citation_delimiters,
                    heading_title=self.feature.references_heading_title,
                )
                endnotes_mgr.add_endnote_category( self.endnote_category )

    class RenderManager(Feature.RenderManager):

        def initialize(self):
            self.citation_endnotes = {}
            self.use_endnotes = self.feature_document_manager.use_endnotes
            self.external_citations_providers = self.feature.external_citations_providers

        def get_citation_content_llm(self, cite_prefix, cite_key, resource_info):

            if self.external_citations_providers is None:
                raise ValueError("No external citations providers are set!")

            # retrieve citation from citations provider --
            citation_llm_text = None
            for external_citations_provider in self.external_citations_providers:
                citation_llm_text = \
                    external_citations_provider.get_citation_full_text_llm(
                        cite_prefix, cite_key,
                        resource_info
                    )
                if citation_llm_text:
                    break
            
            if citation_llm_text is None:
                raise ValueError(f"Citation not found: ‘{cite_prefix}:{cite_key}’")

            if isinstance(citation_llm_text, LLMFragment):
                citation_llm = citation_llm_text
            else:
                citation_llm = self.render_context.doc.environment.make_fragment(
                    citation_llm_text,
                    is_block_level=False,
                    standalone_mode=True,
                    what=f"Citation text for {cite_prefix}:{cite_key}",
                )

            #logger.debug("Got citation content LLM nodelist = %r", citation_llm.nodes)

            return citation_llm
            

        def get_citation_endnote(self, cite_prefix, cite_key, resource_info):
            endnotes_mgr = None
            if self.use_endnotes:
                endnotes_mgr = self.render_context.feature_render_manager('endnotes')

            if (cite_prefix, cite_key) in self.citation_endnotes:
                return self.citation_endnotes[(cite_prefix, cite_key)]

            citation_llm = self.get_citation_content_llm(cite_prefix, cite_key,
                                                         resource_info)

            endnote = endnotes_mgr.add_endnote(
                category_name='citation', 
                content_nodelist=citation_llm.nodes,
                ref_label_prefix=cite_prefix,
                ref_label=cite_key,
                node=(cite_prefix,cite_key),
            )

            # also add a custom field, the formatted inner counter text (e.g.,
            # "1" for citation "[1]").  It'll be useful for combining a citation
            # number with an optional text as in [31; Theorem 4].
            endnote.formatted_inner_counter_value_llm = \
                self.render_context.doc.environment.make_fragment(
                    self.feature_document_manager.endnote_category.counter_formatter.format_llm(
                        endnote.number,
                        with_delimiters=False
                    ),
                    is_block_level=False,
                    standalone_mode=True,
                    what=f"citation counter (inner)",
                )

            self.citation_endnotes[(cite_prefix, cite_key)] = endnote

            return endnote



    def __init__(self,
                 external_citations_providers,
                 counter_formatter='arabic',
                 citation_delimiters=None,
                 citation_optional_text_separator="; ",
                 references_heading_title='References',
                 ):
        super().__init__()
        self.external_citations_providers = external_citations_providers
        dflt = dict(_cite_default_counter_formatter_spec)
        if citation_delimiters is not None:
            dflt['delimiters'] = citation_delimiters
        self.counter_formatter = build_counter_formatter(
            counter_formatter,
            dflt,
        )
        #self.citation_delimiters = citation_delimiters
        self.citation_optional_text_separator = citation_optional_text_separator
        self.references_heading_title = references_heading_title

    def set_external_citations_providers(self, external_citations_providers):
        if self.external_citations_providers is not None:
            logger.warning(
                "FeatureExternalPrefixedCitations.set_external_citations_providers(): "
                "There are already external citation providers set.  They will be replaced."
            )
        self.external_citations_providers = external_citations_providers

    def add_external_citations_provider(self, external_citations_provider):
        if self.external_citations_providers is None:
            logger.warning(
                "FeatureExternalPrefixedCitations.add_external_citations_provider(): "
                "External citations provider list was not initialized, creating an empty list."
            )
            self.external_citations_providers = []

        self.external_citations_providers.append( external_citations_provider )

    def add_latex_context_definitions(self):
        return {
            'macros': [
                CiteMacro('cite',),
            ]
        }



class CiteMacro(LLMMacroSpecBase):

    allowed_in_standalone_mode = False

    def __init__(self, macroname):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                LLMArgumentSpec(
                    '[',
                    argname='cite_pre_text',
                ),
                LLMArgumentSpec(
                    latexnodes_parsers.LatexCharsCommaSeparatedListParser(
                        enable_comments=False
                    ),
                    argname='citekey'
                ),
            ]
        )

    def postprocess_parsed_node(self, node):
        
        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('cite_pre_text', 'citekey') ,
        )

        optional_cite_extra_nodelist = None
        if node_args['cite_pre_text'].was_provided():
            #
            optional_cite_extra_nodelist = node_args['cite_pre_text'].get_content_nodelist()

        citekeylist_nodelist = node_args['citekey'].get_content_nodelist()

        node.llmarg_optional_cite_extra_nodelist = optional_cite_extra_nodelist
        node.llmarg_citekeylist_nodelist = citekeylist_nodelist

        # citekeylist_nodelist is a list of groups, each group is delimited by
        # ('', ',') and represents a citation key.  It was parsed using
        # pylatexenc3's LatexCharsCommaSeparatedListParser.

        #logger.debug(f"Citation key nodes: {citekeylist_nodelist=}")

        cite_items = []
        for citekeygroupnode in citekeylist_nodelist:

            if not citekeygroupnode:
                continue

            citekey_verbatim = citekeygroupnode.latex_verbatim()
            if citekeygroupnode.delimiters[0]:
                citekey_verbatim = citekey_verbatim[
                    len(citekeygroupnode.delimiters[0]) :
                ]
            if citekeygroupnode.delimiters[1]:
                citekey_verbatim = citekey_verbatim[
                    : -len(citekeygroupnode.delimiters[1])
                ]

            #logger.debug(f"Parsing citation {citekey_verbatim=}")

            if ':' in citekey_verbatim:
                citation_key_prefix, citation_key = citekey_verbatim.split(':', 1)
                # normalize citation_key_prefix to allow for surrounding spaces as well
                # as case tolerance e.g. 'arxiv' vs 'arXiv' etc.
                citation_key_prefix = citation_key_prefix.strip().lower()
            else:
                citation_key_prefix, citation_key = None, citekey_verbatim
            
            cite_items.append( (citation_key_prefix, citation_key) )

        node.llmarg_cite_items = cite_items

        return node


    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        optional_cite_extra_nodelist = node.llmarg_optional_cite_extra_nodelist

        cite_mgr = render_context.feature_render_manager('citations')
        citation_delimiters = cite_mgr.feature.counter_formatter.delimiters

        resource_info = node.latex_walker.resource_info

        #
        # TODO: SORT & MERGE CITATIONS -- use CounterFormatter's cool new methods
        #

        s_items = []
        for cite_item in node.llmarg_cite_items:

            citation_key_prefix, citation_key = cite_item

            if cite_mgr is None:
                s_items.append(
                    fragment_renderer.render_text_format(
                        text_formats=['cite'],
                        content=fragment_renderer.render_value(
                            f'[{citation_key_prefix}:{citation_key}]',
                            render_context=render_context,
                        ),
                    )
                )
                continue

            endnote = None
            citation_content_llm = None
            show_inline_content_llm = None
            if cite_mgr.use_endnotes:
                endnote = cite_mgr.get_citation_endnote(
                    citation_key_prefix,
                    citation_key,
                    resource_info
                )
                show_inline_content_llm = endnote.formatted_inner_counter_value_llm
            else:
                citation_content_llm = cite_mgr.get_citation_content_llm(
                    citation_key_prefix,
                    citation_key,
                    resource_info
                )
                show_inline_content_llm = citation_content_llm

            # don't use endnotes_mgr.render_endnote_mark(endnote) because it
            # can't render the optional citation text.  Form the citation mark
            # ourselves, using the citation delimiters etc.
            cite_content_list_of_nodes = []
            if citation_delimiters[0] is not None:
                cite_content_list_of_nodes.append(
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars=citation_delimiters[0],
                        pos=node.pos,
                        pos_end=node.pos_end,
                        parsing_state=node.parsing_state,
                    )
                )
            # list() needed for transcrypt ... :/ -->
            cite_content_list_of_nodes.extend( list(show_inline_content_llm.nodes) )
            if optional_cite_extra_nodelist is not None:
                cite_content_list_of_nodes.append(
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars=cite_mgr.feature.citation_optional_text_separator,
                        pos=node.pos,
                        pos_end=node.pos_end,
                        parsing_state=node.parsing_state,
                    )
                )
                # list() needed for transcrypt ... :/ -->
                cite_content_list_of_nodes.extend( list(optional_cite_extra_nodelist) )
            if citation_delimiters[1] is not None:
                cite_content_list_of_nodes.append(
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars=citation_delimiters[1],
                        pos=node.pos,
                        pos_end=node.pos_end,
                        parsing_state=node.parsing_state,
                    )
                )

            citation_nodes_parsing_state = node.parsing_state.sub_context(
                is_block_level=False
            )

            if cite_mgr.use_endnotes:
                endnote_link_href = f"#{endnote.category_name}-{endnote.number}"
                full_cite_mark = render_context.fragment_renderer.render_link(
                    'endnote',
                    endnote_link_href,
                    display_nodelist=node.latex_walker.make_nodelist(
                        cite_content_list_of_nodes,
                        parsing_state=citation_nodes_parsing_state,
                    ),
                    render_context=render_context,
                    annotations=['endnote', endnote.category_name],
                )

                s_items.append(
                    full_cite_mark
                )
            else:
                full_inline_citation = render_context.fragment_renderer.render_nodelist(
                    node.latex_walker.make_nodelist(
                        cite_content_list_of_nodes,
                        parsing_state=citation_nodes_parsing_state,
                    ),
                    render_context
                )

                s_items.append(
                    full_inline_citation
                )

        return fragment_renderer.render_join(s_items, render_context)
