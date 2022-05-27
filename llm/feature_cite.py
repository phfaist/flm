import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc import macrospec

from .llmspecinfo import LLMSpecInfo, LLMMacroSpec
from .llmenvironment import make_arg_spec

from .feature import Feature


class FeatureExternalPrefixedCitationsRenderManager(Feature.RenderManager):

    def initialize(self):
        self.citation_endnotes = {}

    def get_citation_endnote(self, cite_prefix, cite_key):
        endnotes_mgr = self.render_context.feature_render_manager('endnotes')
        if endnotes_mgr is None:
            raise RuntimeError("No 'endnotes' feature manager found to add citations")

        if (cite_prefix, cite_key) in self.citation_endnotes:
            return self.citation_endnotes[(cite_prefix, cite_key)]

        citation_llm = self.render_context.doc.environment.make_fragment(
            self.feature.external_citations_provider.get_citation_full_text_llm(
                cite_prefix, cite_key
            ),
            is_block_level=False,
            what=f"Citation text for {cite_prefix}:{cite_key}",
        )
        
        logger.debug("Got citation content LLM nodelist = %r", citation_llm.nodes)

        endnote = endnotes_mgr.add_endnote(
            category_name='citation', 
            content_nodelist=citation_llm.nodes,
            label=f"{cite_prefix}:{cite_key}"
        )

        self.citation_endnotes[(cite_prefix, cite_key)] = endnote

        return endnote


class FeatureExternalPrefixedCitations(Feature):

    feature_name = 'citations'
    RenderManager = FeatureExternalPrefixedCitationsRenderManager

    def __init__(self, external_citations_provider):
        super().__init__()
        self.external_citations_provider = external_citations_provider

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

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('cite_pre_text', 'citekey') ,
            all=True
        )

        optional_cite_extra_content = None
        if node_args['cite_pre_text'].provided:
            #
            optional_cite_extra_content = fragment_renderer.render_nodelist(
                node_args['cite_pre_text'].nodelist,
                render_context,
                is_block_level=False
            )

        citekeylist_nodelist = node_args['citekey'].nodelist

        # citekeylist_nodelist is a list of groups, each group is delimited by
        # ('', ',') and represents a citation key.  It was parsed using
        # pylatexenc3's LatexCharsCommaSeparatedListParser.

        #logger.debug(f"Citation key nodes: {citekeylist_nodelist=}")

        cite_mgr = render_context.feature_render_manager('citations')
        endnotes_mgr = render_context.feature_render_manager('endnotes')

        s_items = []
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

            if cite_mgr is None:
                s_items.append(
                    fragment_renderer.render_text_format(
                        text_formats=['cite'],
                        content=fragment_renderer.render_value(f'[{citekey_verbatim}]'),
                    )
                )
                continue

            #logger.debug(f"Parsing citation {citekey_verbatim=}")

            if ':' in citekey_verbatim:
                citation_key_prefix, citation_key = citekey_verbatim.split(':', 1)
                # normalize citation_key_prefix to allow for surrounding spaces as well
                # as case tolerance e.g. 'arxiv' vs 'arXiv' etc.
                citation_key_prefix = citation_key_prefix.strip().lower()
            else:
                citation_key_prefix, citation_key = None, citekey_verbatim
            
            endnote = cite_mgr.get_citation_endnote(
                citation_key_prefix,
                citation_key,
            )
            s_items.append(
                endnotes_mgr.render_endnote_mark(endnote)
            )

        return fragment_renderer.render_join(s_items)
