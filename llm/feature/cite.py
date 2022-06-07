import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import ParsedArgumentsInfo

from ..llmspecinfo import LLMSpecInfo, LLMMacroSpec
from ..llmenvironment import make_arg_spec

from .. import fmthelpers

from ._base import Feature

from .endnotes import EndnoteCategory


class CitationEndnoteCategory(EndnoteCategory):
    def __init__(self, counter_formatter='arabic', citation_delimiters=(None,None)):

        self.inner_counter_formatter_fn = counter_formatter
        if self.inner_counter_formatter_fn in fmthelpers.standard_counter_formatters:
            self.inner_counter_formatter_fn = \
                fmthelpers.standard_counter_formatters[self.inner_counter_formatter_fn]

        self.citation_delimiters = citation_delimiters

        full_counter_formatter = lambda x: (
            self.citation_delimiters[0]
            + self.inner_counter_formatter_fn(x)
            + self.citation_delimiters[1]
        )

        super().__init__(
            'citation',
            counter_formatter=full_counter_formatter,
        )



class FeatureExternalPrefixedCitations(Feature):

    feature_name = 'citations'

    class DocumentManager(Feature.DocumentManager):
        def initialize(self):
            endnotes_mgr = self.doc.feature_document_manager('endnotes')
            self.endnote_category = CitationEndnoteCategory(
                counter_formatter=self.feature.counter_formatter,
                citation_delimiters=self.feature.citation_delimiters,
            )
            endnotes_mgr.add_endnote_category( self.endnote_category )

    class RenderManager(Feature.RenderManager):

        def initialize(self):
            self.citation_endnotes = {}

        def get_citation_endnote(self, cite_prefix, cite_key, *, resource_info):
            endnotes_mgr = self.render_context.feature_render_manager('endnotes')

            if (cite_prefix, cite_key) in self.citation_endnotes:
                return self.citation_endnotes[(cite_prefix, cite_key)]

            # retrieve citation from citations provider --
            citation_llm_text = \
                self.feature.external_citations_provider.get_citation_full_text_llm(
                    cite_prefix, cite_key,
                    resource_info=resource_info
                )

            citation_llm = self.render_context.doc.environment.make_fragment(
                citation_llm_text,
                is_block_level=False,
                what=f"Citation text for {cite_prefix}:{cite_key}",
            )

            logger.debug("Got citation content LLM nodelist = %r", citation_llm.nodes)

            endnote = endnotes_mgr.add_endnote(
                category_name='citation', 
                content_nodelist=citation_llm.nodes,
                ref_label_prefix=cite_prefix,
                ref_label=cite_key,
            )

            # also add formatted inner counter text (e.g., "1" for citaiton "[1]")
            endnote.formatted_inner_counter_value_llm = \
                self.render_context.doc.environment.make_fragment(
                    self.feature_document_manager.endnote_category.inner_counter_formatter_fn(
                        endnote.number
                    ),
                    is_block_level=False,
                    what=f"citation counter (inner)",
                )

            self.citation_endnotes[(cite_prefix, cite_key)] = endnote

            return endnote



    def __init__(self,
                 external_citations_provider,
                 counter_formatter='arabic',
                 citation_delimiters=('[',']'),
                 citation_optional_text_separator="; ",
                 ):
        super().__init__()
        self.external_citations_provider = external_citations_provider
        self.counter_formatter = counter_formatter
        self.citation_delimiters = citation_delimiters
        self.citation_optional_text_separator = citation_optional_text_separator

    def add_latex_context_definitions(self):
        return {
            'macros': [
                LLMMacroSpec(
                    'cite',
                    [
                        make_arg_spec(
                            '[',
                            argname='cite_pre_text'
                        ),
                        make_arg_spec(
                            latexnodes_parsers.LatexCharsCommaSeparatedListParser(
                                enable_comments=False
                            ),
                            argname='citekey'
                        ),
                    ],
                    llm_specinfo=CiteSpecInfo(),
                ),
            ]
        }



class CiteSpecInfo(LLMSpecInfo):

    allowed_in_restricted_mode = False

    def finalize_parsed_node(self, node):
        
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
        citation_delimiters = cite_mgr.feature.citation_delimiters

        resource_info = node.latex_walker.resource_info

        s_items = []
        for cite_item in node.llmarg_cite_items:

            citation_key_prefix, citation_key = cite_item

            if cite_mgr is None:
                s_items.append(
                    fragment_renderer.render_text_format(
                        text_formats=['cite'],
                        content=fragment_renderer.render_value(
                            f'[{citation_key_prefix}:{citation_key}]'
                        ),
                    )
                )
                continue

            endnote = cite_mgr.get_citation_endnote(
                citation_key_prefix,
                citation_key,
                resource_info=resource_info
            )

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
            cite_content_list_of_nodes += list(
                endnote.formatted_inner_counter_value_llm.nodes
            )
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
                cite_content_list_of_nodes += list(
                    optional_cite_extra_nodelist
                )
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

            endnote_link_href = f"#{endnote.category_name}-{endnote.number}"
            full_cite_mark = render_context.fragment_renderer.render_link(
                'endnote',
                endnote_link_href,
                display_nodelist=node.latex_walker.make_nodelist(
                    cite_content_list_of_nodes,
                    parsing_state=node.parsing_state,
                ),
                render_context=render_context,
                annotations=['endnote', endnote.category_name],
            )

            s_items.append(
                full_cite_mark
            )

        return fragment_renderer.render_join(s_items)
