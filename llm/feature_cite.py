
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc import macrospec

from .llmspecinfo import LLMSpecInfo, LLMMacroSpec

from .feature import Feature, FeatureDocumentManager


class FeatureExternalPrefixedCitationsDocumentManager(FeatureDocumentManager):

    def initialize(self):
        self.citation_endnotes = {}

    def get_citation_endnote(self, cite_prefix, cite_key, doc, fragment_renderer):
        endnotes_mgr = self.doc.feature_manager('endnotes')
        if endnotes_mgr is None:
            raise RuntimeError("No 'endnotes' feature manager found to add citations")

        if (cite_prefix, cite_key) in self.citation_endnotes:
            return self.citation_endnotes[(cite_prefix, cite_key)]

        citation_llm = doc.environment.make_fragment(
            self.feature.external_citations_provider.get_citation_full_text_llm(
                cite_prefix, cite_key
            )
        )

        formatted_citation = fragment_renderer.render_fragment(
            citation_llm, 
            doc=None
        )

        endnote = endnotes_mgr.add_endnote(
            'citation', 
            formatted_citation,
            label=f"{cite_prefix}:{cite_key}"
        )

        self.citation_endnotes[(cite_prefix, cite_key)] = endnote

        return endnote


class FeatureExternalPrefixedCitations(Feature):

    feature_name = 'citations'
    feature_manager_class = FeatureExternalPrefixedCitationsDocumentManager

    def __init__(self, external_citations_provider):
        super().__init__()
        self.external_citations_provider = external_citations_provider

    def add_latex_context_definitions(self):
        return {
            'macros': [
                LLMMacroSpec(
                    'cite',
                    [
                        macrospec.LatexArgumentSpec(
                            '[',
                            argname='cite_pre_text'
                        ),
                        macrospec.LatexArgumentSpec(
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

    def render(self, node, doc, fragment_renderer):

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('cite_pre_text', 'citekey') ,
            all=True
        )

        optional_cite_extra_content = None
        if node_args['cite_pre_text'] \
           and node_args['cite_pre_text'] != latexnodes_nodes.LatexNodeList([None]):
            #
            optional_cite_extra_content = fragment_renderer.render_nodelist(
                node_args['cite_pre_text'],
                doc,
                use_paragraphs=False
            )

        citekeylist_nodelist = node_args['citekey']

        # citekeylist_nodelist is a list of groups, each group is delimited by
        # ('', ',') and represents a citation key.  It was parsed using
        # pylatexenc3's LatexCharsCommaSeparatedListParser.

        #logger.debug(f"Citation key nodes: {citekeylist_nodelist=}")

        cite_mgr = None
        endnotes_mgr = None
        if doc is not None:
            cite_mgr = doc.feature_manager('citations')
            endnotes_mgr = doc.feature_manager('endnotes')

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
                doc,
                fragment_renderer,
            )
            s_items.append(
                endnotes_mgr.render_endnote_mark(endnote, fragment_renderer)
            )

        return fragment_renderer.render_join(s_items)
