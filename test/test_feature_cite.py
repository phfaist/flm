import unittest


from llm.llmenvironment import make_standard_environment
from llm.stdfeatures import standard_features
from llm.fragmentrenderer.html import HtmlFragmentRenderer

from llm.feature import cite as feature_cite


def mk_llm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class MyCitationsProvider:
    def get_citation_full_text_llm(self, cite_prefix, cite_key, resource_info):
        if cite_prefix == 'arxiv':
            # can return LLM text as a string
            return r'\textit{arXiv} paper ' + f'arXiv:{cite_key}'
        if cite_prefix == 'manual':
            # can return a compiled fragment
            return environ.make_fragment(
                cite_key,
                is_block_level=False,
                standalone_mode=True,
                what=f"Manual citation text {repr(cite_key)}",
            )
        raise ValueError(f"Invalid citation prefix: {repr(cite_prefix)}")



class TestFeatureCite(unittest.TestCase):

    maxDiff = None

    def test_citation_1(self):

        environ = mk_llm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(r"Citation \cite{arxiv:1234.56789}.",
                                      is_block_level=False)

        def rdr(render_context):
            return {
                'text': frag1.render(render_context),
                'endnotes': render_context.feature_render_manager('endnotes')
                            .render_endnotes(target_id=None, include_headings_at_level=None,
                                             endnotes_heading_level=None)
            }
        doc = environ.make_document(rdr)

        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr)
        print(result['text'])
        self.assertEqual(
            result['text'],
            r"""
Citation <span class="citations"><a href="#citation-1" class="href-endnote endnote citation">[1]</a></span>.
""".strip()
        )

        print(result['endnotes'])
        self.assertEqual(
            result['endnotes'],
            r"""
<div class="endnotes"><dl class="enumeration citation-list"><dt id="citation-1">[1]</dt><dd><span class="textit">arXiv</span> paper arXiv:1234.56789</dd></dl></div>
            """.strip()
)

    def test_citation_2(self):

        environ = mk_llm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(r"Citation \cite{arxiv:1234.56789,arxiv:9876.07654}.",
                                      is_block_level=False)

        def rdr(render_context):
            return {
                'text': frag1.render(render_context),
                'endnotes': render_context.feature_render_manager('endnotes')
                            .render_endnotes(target_id=None, include_headings_at_level=None,
                                             endnotes_heading_level=None)
            }
        doc = environ.make_document(rdr)

        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr)
        print(result['text'])
        self.assertEqual(
            result['text'],
            r"""
Citation <span class="citations">[<a href="#citation-1" class="href-endnote endnote citation">1</a>,<a href="#citation-2" class="href-endnote endnote citation">2</a>]</span>.
""".strip()
        )

        print(result['endnotes'])
        self.assertEqual(
            result['endnotes'],
            r"""
<div class="endnotes"><dl class="enumeration citation-list"><dt id="citation-1">[1]</dt><dd><span class="textit">arXiv</span> paper arXiv:1234.56789</dd><dt id="citation-2">[2]</dt><dd><span class="textit">arXiv</span> paper arXiv:9876.07654</dd></dl></div>
            """.strip()
)


    def test_citation_multirng(self):

        environ = mk_llm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(
            r"""
Citation \cite{arxiv:1111.11111,arxiv:2222.22222}.
Cite \cite{arxiv:3333.33333,arxiv:4444.44444}.
Cite \cite{arxiv:3333.33333,arxiv:2222.22222,arxiv:5555.55555,arxiv:1111.11111,arxiv:6666.66666}.
            """ .strip(),
            is_block_level=False
        )

        def rdr(render_context):
            return {
                'text': frag1.render(render_context),
                'endnotes': render_context.feature_render_manager('endnotes')
                            .render_endnotes(target_id=None, include_headings_at_level=None,
                                             endnotes_heading_level=None)
            }
        doc = environ.make_document(rdr)

        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr)
        print(result['text'])
        self.assertEqual(
            result['text'],
            r"""
Citation <span class="citations">[<a href="#citation-1" class="href-endnote endnote citation">1</a>,<a href="#citation-2" class="href-endnote endnote citation">2</a>]</span>.
Cite <span class="citations">[<a href="#citation-3" class="href-endnote endnote citation">3</a>,<a href="#citation-4" class="href-endnote endnote citation">4</a>]</span>.
Cite <span class="citations">[<a href="#citation-1" class="href-endnote endnote citation">1</a>–<a href="#citation-3" class="href-endnote endnote citation">3</a>,<a href="#citation-5" class="href-endnote endnote citation">5</a>,<a href="#citation-6" class="href-endnote endnote citation">6</a>]</span>.
""".strip() .replace('\n', ' ')
        )

        print(result['endnotes'])
        self.assertEqual(
            result['endnotes'],
            r"""
<div class="endnotes"><dl class="enumeration citation-list"><dt id="citation-1">[1]</dt><dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd><dt id="citation-2">[2]</dt><dd><span class="textit">arXiv</span> paper arXiv:2222.22222</dd><dt id="citation-3">[3]</dt><dd><span class="textit">arXiv</span> paper arXiv:3333.33333</dd><dt id="citation-4">[4]</dt><dd><span class="textit">arXiv</span> paper arXiv:4444.44444</dd><dt id="citation-5">[5]</dt><dd><span class="textit">arXiv</span> paper arXiv:5555.55555</dd><dt id="citation-6">[6]</dt><dd><span class="textit">arXiv</span> paper arXiv:6666.66666</dd></dl></div>
            """.strip()
)



    def test_citation_w_extra(self):

        environ = mk_llm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(
            r"""
Citation \cite[Theorem~45]{arxiv:1111.11111}.
            """ .strip(),
            is_block_level=False
        )

        def rdr(render_context):
            return {
                'text': frag1.render(render_context),
                'endnotes': render_context.feature_render_manager('endnotes')
                            .render_endnotes(target_id=None, include_headings_at_level=None,
                                             endnotes_heading_level=None)
            }
        doc = environ.make_document(rdr)

        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr)
        print(result['text'])
        self.assertEqual(
            result['text'],
            r"""
Citation <span class="citations">[<a href="#citation-1" class="href-endnote endnote citation">1; Theorem&nbsp;45</a>]</span>.
""".strip() .replace('\n', ' ')
        )

        print(result['endnotes'])
        self.assertEqual(
            result['endnotes'],
            r"""
<div class="endnotes"><dl class="enumeration citation-list"><dt id="citation-1">[1]</dt><dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd></dl></div>
            """.strip()
)


    def test_citation_multi_w_extra_error(self):

        environ = mk_llm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        with self.assertRaises(Exception):
            frag1 = environ.make_fragment(
                r"""
Citation \cite[Theorem~45]{arxiv:1111.11111,arxiv:2222.22222}.
                """ .strip(),
                is_block_level=False
            )

