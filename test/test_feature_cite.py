import unittest


from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.feature import cite as feature_cite
from flm.feature.cite import FeatureExternalPrefixedCitations


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_cite_doc(environ, flm_input, fragment_renderer_class=HtmlFragmentRenderer):
    frag1 = environ.make_fragment(flm_input.strip(), is_block_level=False)

    def rdr(render_context):
        return {
            'text': frag1.render(render_context),
            'endnotes': render_context.feature_render_manager('endnotes')
                        .render_endnotes(target_id=None,
                                         include_headings_at_level=None,
                                         endnotes_heading_level=None)
        }
    doc = environ.make_document(rdr)
    fr = fragment_renderer_class()
    result, render_context = doc.render(fr)
    return result


class MyCitationsProvider:
    def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
        if cite_prefix == 'arxiv':
            # can return FLM text as a string
            return r'\textit{arXiv} paper ' + f'arXiv:{cite_key}'
        if cite_prefix == 'manual':
            # can return a compiled fragment — needs environ from caller
            return self._environ.make_fragment(
                cite_key,
                is_block_level=False,
                standalone_mode=True,
                what=f"Manual citation text {repr(cite_key)}",
            )
        if cite_prefix is None:
            return f'Bare citation: {cite_key}'
        raise ValueError(f"Invalid citation prefix: {repr(cite_prefix)}")


class ReturnsNoneProvider:
    """Provider that returns None for everything."""
    def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
        return None


# ----------------------------------------------------------------
# Existing tests — kept as-is
# ----------------------------------------------------------------

class TestFeatureCite(unittest.TestCase):

    maxDiff = None

    def test_citation_1(self):

        environ = mk_flm_environ(
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
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>.'
        )

        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1234.56789</dd>'
            '</dl></div>'
        )

    def test_citation_2(self):

        environ = mk_flm_environ(
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
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '[<a href="#citation-1" class="href-endnote endnote citation">1</a>,'
            '<a href="#citation-2" class="href-endnote endnote citation">2</a>]'
            '</span>.'
        )

        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1234.56789</dd>'
            '<dt id="citation-2">[2]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:9876.07654</dd>'
            '</dl></div>'
        )


    def test_citation_multirng(self):

        environ = mk_flm_environ(
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
        self.assertEqual(
            result['text'],
            r"""
Citation <span class="citation-marks">[<a href="#citation-1" class="href-endnote endnote citation">1</a>,<a href="#citation-2" class="href-endnote endnote citation">2</a>]</span>.
Cite <span class="citation-marks">[<a href="#citation-3" class="href-endnote endnote citation">3</a>,<a href="#citation-4" class="href-endnote endnote citation">4</a>]</span>.
Cite <span class="citation-marks">[<a href="#citation-1" class="href-endnote endnote citation">1</a>–<a href="#citation-3" class="href-endnote endnote citation">3</a>,<a href="#citation-5" class="href-endnote endnote citation">5</a>,<a href="#citation-6" class="href-endnote endnote citation">6</a>]</span>.
""".strip() .replace('\n', ' ')
        )

        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd>'
            '<dt id="citation-2">[2]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:2222.22222</dd>'
            '<dt id="citation-3">[3]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:3333.33333</dd>'
            '<dt id="citation-4">[4]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:4444.44444</dd>'
            '<dt id="citation-5">[5]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:5555.55555</dd>'
            '<dt id="citation-6">[6]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:6666.66666</dd>'
            '</dl></div>'
        )



    def test_citation_w_extra(self):

        environ = mk_flm_environ(
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
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">'
            '[1; Theorem&nbsp;45]</a></span>.'
        )

        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd>'
            '</dl></div>'
        )


    def test_citation_multi_w_extra_error(self):

        environ = mk_flm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        with self.assertRaises(Exception):
            frag1 = environ.make_fragment(
                r"""
Citation \cite[Theorem~45]{arxiv:1111.11111,arxiv:2222.22222}.
                """ .strip(),
                is_block_level=False
            )


    def test_citation_chain_macros(self):

        environ = mk_flm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(
            r"""
Citation \cite{arxiv:1111.11111}\cite{arxiv:2222.22222}.
Citation \cite[Theorem~45]{arxiv:1111.11111}\cite{arxiv:2222.22222}.
Citation \cite{arxiv:1111.11111}\cite[Theorem~45]{arxiv:2222.22222}.
Citation \cite{arxiv:1111.11111,arxiv:3333.33333}\cite[Theorem~45]{arxiv:2222.22222}.
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
        self.assertEqual(
            result['text'],
            r"""
Citation <span class="citation-marks">[<a href="#citation-1" class="href-endnote endnote citation">1</a>,<a href="#citation-2" class="href-endnote endnote citation">2</a>]</span>.
Citation <span class="citation-marks"><a href="#citation-2" class="href-endnote endnote citation">[2]</a><a href="#citation-1" class="href-endnote endnote citation">[1; Theorem&nbsp;45]</a></span>.
Citation <span class="citation-marks"><a href="#citation-1" class="href-endnote endnote citation">[1]</a><a href="#citation-2" class="href-endnote endnote citation">[2; Theorem&nbsp;45]</a></span>.
Citation <span class="citation-marks">[<a href="#citation-1" class="href-endnote endnote citation">1</a>,<a href="#citation-3" class="href-endnote endnote citation">3</a>][<a href="#citation-2" class="href-endnote endnote citation">2; Theorem&nbsp;45</a>]</span>.
""".strip() .replace('\n', ' ')
        )

        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd>'
            '<dt id="citation-2">[2]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:2222.22222</dd>'
            '<dt id="citation-3">[3]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:3333.33333</dd>'
            '</dl></div>'
        )


# ----------------------------------------------------------------
# New tests — initialization & configuration
# ----------------------------------------------------------------

class TestFeatureCiteInit(unittest.TestCase):

    def test_feature_name(self):
        feat = FeatureExternalPrefixedCitations(external_citations_providers=None)
        self.assertEqual(feat.feature_name, 'citations')
        self.assertEqual(feat.feature_title, 'Citations')

    def test_default_counter_formatter(self):
        feat = FeatureExternalPrefixedCitations(external_citations_providers=None)
        self.assertEqual(feat.counter_formatter.delimiters, ('[', ']'))

    def test_default_options(self):
        feat = FeatureExternalPrefixedCitations(external_citations_providers=None)
        self.assertEqual(feat.citation_optional_text_separator, '; ')
        self.assertEqual(feat.references_heading_title, 'References')
        self.assertTrue(feat.sort_and_compress)
        self.assertTrue(feat.use_endnotes)

    def test_custom_delimiters(self):
        feat = FeatureExternalPrefixedCitations(
            external_citations_providers=None,
            citation_delimiters=('(', ')'),
        )
        self.assertEqual(feat.counter_formatter.delimiters, ('(', ')'))

    def test_custom_options(self):
        feat = FeatureExternalPrefixedCitations(
            external_citations_providers=None,
            citation_optional_text_separator=' -- ',
            references_heading_title='Bibliography',
            sort_and_compress=False,
        )
        self.assertEqual(feat.citation_optional_text_separator, ' -- ')
        self.assertEqual(feat.references_heading_title, 'Bibliography')
        self.assertFalse(feat.sort_and_compress)

    def test_add_latex_context_definitions(self):
        feat = FeatureExternalPrefixedCitations(external_citations_providers=None)
        defs = feat.add_latex_context_definitions()
        macro_names = [m.macroname for m in defs['macros']]
        self.assertEqual(macro_names, ['cite'])

    def test_set_external_citations_providers(self):
        feat = FeatureExternalPrefixedCitations(external_citations_providers=None)
        self.assertIsNone(feat.external_citations_providers)

        class P:
            pass

        feat.set_external_citations_providers([P()])
        self.assertEqual(len(feat.external_citations_providers), 1)

    def test_add_external_citations_provider_on_none(self):
        feat = FeatureExternalPrefixedCitations(external_citations_providers=None)

        class P:
            pass

        feat.add_external_citations_provider(P())
        self.assertEqual(len(feat.external_citations_providers), 1)

    def test_add_external_citations_provider_on_existing(self):
        class P:
            pass

        feat = FeatureExternalPrefixedCitations(external_citations_providers=[P()])
        feat.add_external_citations_provider(P())
        self.assertEqual(len(feat.external_citations_providers), 2)

    def test_feature_optional_dependencies(self):
        self.assertEqual(
            FeatureExternalPrefixedCitations.feature_optional_dependencies,
            ['endnotes']
        )


# ----------------------------------------------------------------
# New tests — bare keys, FLMFragment provider, no-endnotes mode
# ----------------------------------------------------------------

class TestFeatureCiteVariants(unittest.TestCase):

    maxDiff = None

    def test_bare_key_no_prefix(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'Citation \cite{mykey}.')
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>.'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd>Bare citation: mykey</dd>'
            '</dl></div>'
        )

    def test_flm_fragment_returning_provider(self):
        provider = MyCitationsProvider()
        features = standard_features(
            external_citations_providers=[provider]
        )
        environ = make_standard_environment(features)
        provider._environ = environ

        result = render_cite_doc(environ, r'Citation \cite{manual:Hello world}.')
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>.'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd>Hello world</dd>'
            '</dl></div>'
        )

    def test_no_endnotes_inline_mode(self):
        features = standard_features(citations=False)
        feat = FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
        )
        feat.use_endnotes = False
        features.append(feat)
        environ = make_standard_environment(features)

        frag1 = environ.make_fragment(
            r'Citation \cite{arxiv:1234.56789}.', is_block_level=False
        )

        def rdr(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(rdr)
        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        self.assertEqual(
            result,
            'Citation <span class="citation-marks">'
            '[<span class="textit">arXiv</span> paper arXiv:1234.56789]'
            '</span>.'
        )

    def test_sort_and_compress_false(self):
        features = standard_features(citations=False)
        features.append(FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
            sort_and_compress=False,
        ))
        environ = make_standard_environment(features)

        result = render_cite_doc(
            environ,
            r'Citation \cite{arxiv:1111.11111,arxiv:2222.22222}.'
        )
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '<a href="#citation-2" class="href-endnote endnote citation">[2]</a>'
            '</span>.'
        )

    def test_multiple_bare_keys(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'\cite{key1,key2,key3}')
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '[<a href="#citation-1" class="href-endnote endnote citation">1</a>'
            '\u2013'
            '<a href="#citation-3" class="href-endnote endnote citation">3</a>]'
            '</span>'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt><dd>Bare citation: key1</dd>'
            '<dt id="citation-2">[2]</dt><dd>Bare citation: key2</dd>'
            '<dt id="citation-3">[3]</dt><dd>Bare citation: key3</dd>'
            '</dl></div>'
        )

    def test_mixed_prefix_and_bare_keys(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'\cite{arxiv:1111.11111,key1}')
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '[<a href="#citation-1" class="href-endnote endnote citation">1</a>,'
            '<a href="#citation-2" class="href-endnote endnote citation">2</a>]'
            '</span>'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd>'
            '<dt id="citation-2">[2]</dt><dd>Bare citation: key1</dd>'
            '</dl></div>'
        )

    def test_case_insensitive_prefix(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'\cite{ArXiv:1111.11111}')
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd>'
            '</dl></div>'
        )

    def test_prefix_with_spaces_stripped(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'\cite{ arxiv : 1111.11111}')
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>'
        )
        # Note: key retains the leading space after ":"
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv: 1111.11111</dd>'
            '</dl></div>'
        )

    def test_same_citation_referenced_twice(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'First \cite{arxiv:1111.11111}. Second \cite{arxiv:1111.11111}.'
        )
        self.assertEqual(
            result['text'],
            'First <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>. Second <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>.'
        )
        # Only one endnote entry
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1111.11111</dd>'
            '</dl></div>'
        )

    def test_two_separate_cite_commands(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'See \cite{arxiv:1111.11111} and \cite{arxiv:2222.22222}.'
        )
        self.assertEqual(
            result['text'],
            'See <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span> and <span class="citation-marks">'
            '<a href="#citation-2" class="href-endnote endnote citation">[2]</a>'
            '</span>.'
        )

    def test_triple_chained_cite(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'\cite{arxiv:1111.11111}\cite{arxiv:2222.22222}\cite{arxiv:3333.33333}'
        )
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '[<a href="#citation-1" class="href-endnote endnote citation">1</a>'
            '\u2013'
            '<a href="#citation-3" class="href-endnote endnote citation">3</a>]'
            '</span>'
        )

    def test_chain_extra_on_first(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'\cite[p.~5]{arxiv:1111.11111}\cite{arxiv:2222.22222}'
        )
        # With extra on first, bare items come before extra items in rendering
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '<a href="#citation-2" class="href-endnote endnote citation">[2]</a>'
            '<a href="#citation-1" class="href-endnote endnote citation">'
            '[1; p.&nbsp;5]</a>'
            '</span>'
        )

    def test_chain_both_with_extras(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'\cite[Thm~1]{arxiv:1111.11111}\cite[Cor~2]{arxiv:2222.22222}'
        )
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">'
            '[1; Thm&nbsp;1]</a>'
            '<a href="#citation-2" class="href-endnote endnote citation">'
            '[2; Cor&nbsp;2]</a>'
            '</span>'
        )

    def test_no_endnotes_inline_multiple_keys(self):
        features = standard_features(citations=False)
        feat = FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
        )
        feat.use_endnotes = False
        features.append(feat)
        environ = make_standard_environment(features)

        frag = environ.make_fragment(
            r'See \cite{arxiv:1111.11111,arxiv:2222.22222}.', is_block_level=False
        )

        def rdr(render_context):
            return frag.render(render_context)

        doc = environ.make_document(rdr)
        result, _ = doc.render(HtmlFragmentRenderer())
        self.assertEqual(
            result,
            'See <span class="citation-marks">'
            '[<span class="textit">arXiv</span> paper arXiv:1111.11111]'
            '[<span class="textit">arXiv</span> paper arXiv:2222.22222]'
            '</span>.'
        )

    def test_no_endnotes_inline_with_extra(self):
        features = standard_features(citations=False)
        feat = FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
        )
        feat.use_endnotes = False
        features.append(feat)
        environ = make_standard_environment(features)

        frag = environ.make_fragment(
            r'See \cite[Thm~1]{arxiv:1111.11111}.', is_block_level=False
        )

        def rdr(render_context):
            return frag.render(render_context)

        doc = environ.make_document(rdr)
        result, _ = doc.render(HtmlFragmentRenderer())
        self.assertEqual(
            result,
            'See <span class="citation-marks">'
            '[<span class="textit">arXiv</span> paper arXiv:1111.11111; Thm&nbsp;1]'
            '</span>.'
        )

    def test_sort_and_compress_false_three_keys(self):
        features = standard_features(citations=False)
        features.append(FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
            sort_and_compress=False,
        ))
        environ = make_standard_environment(features)

        result = render_cite_doc(
            environ,
            r'See \cite{arxiv:1111.11111,arxiv:2222.22222,arxiv:3333.33333}.'
        )
        self.assertEqual(
            result['text'],
            'See <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '<a href="#citation-2" class="href-endnote endnote citation">[2]</a>'
            '<a href="#citation-3" class="href-endnote endnote citation">[3]</a>'
            '</span>.'
        )

    def test_sort_and_compress_false_chain_with_extra(self):
        features = standard_features(citations=False)
        features.append(FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
            sort_and_compress=False,
        ))
        environ = make_standard_environment(features)

        result = render_cite_doc(
            environ,
            r'\cite{arxiv:1111.11111}\cite[Thm~1]{arxiv:2222.22222}'
        )
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '<a href="#citation-2" class="href-endnote endnote citation">[2; Thm&nbsp;1]</a>'
            '</span>'
        )

    def test_custom_optional_text_separator(self):
        features = standard_features(citations=False)
        features.append(FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
            citation_optional_text_separator=' -- ',
        ))
        environ = make_standard_environment(features)

        result = render_cite_doc(environ, r'\cite[Thm~1]{arxiv:1111.11111}')
        self.assertEqual(
            result['text'],
            '<span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">'
            '[1 \u2013 Thm&nbsp;1]</a>'
            '</span>'
        )

    def test_custom_delimiters_rendering(self):
        features = standard_features(citations=False)
        features.append(FeatureExternalPrefixedCitations(
            external_citations_providers=[MyCitationsProvider()],
            citation_delimiters=('(', ')'),
        ))
        environ = make_standard_environment(features)

        result = render_cite_doc(environ, r'Citation \cite{arxiv:1234.56789}.')
        self.assertEqual(
            result['text'],
            'Citation <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">(1)</a>'
            '</span>.'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">(1)</dt>'
            '<dd><span class="textit">arXiv</span> paper arXiv:1234.56789</dd>'
            '</dl></div>'
        )


# ----------------------------------------------------------------
# New tests — error cases
# ----------------------------------------------------------------

class TestFeatureCiteErrors(unittest.TestCase):

    maxDiff = None

    def test_citation_not_found_raises(self):
        environ = mk_flm_environ(
            external_citations_providers=[ReturnsNoneProvider()]
        )
        with self.assertRaises(Exception):
            render_cite_doc(environ, r'Citation \cite{arxiv:nonexistent}.')

    def test_no_providers_raises(self):
        features = standard_features(citations=False)
        feat = FeatureExternalPrefixedCitations(
            external_citations_providers=None,
        )
        features.append(feat)
        environ = make_standard_environment(features)

        with self.assertRaises(Exception):
            render_cite_doc(environ, r'Citation \cite{arxiv:1234.56789}.')


# ----------------------------------------------------------------
# New tests — alternative renderers
# ----------------------------------------------------------------

class TestFeatureCiteMultipleProviders(unittest.TestCase):

    maxDiff = None

    def test_fallback_to_second_provider(self):
        class Provider1:
            def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
                if cite_prefix == 'arxiv':
                    return r'\textit{arXiv} paper ' + f'arXiv:{cite_key}'
                return None

        class Provider2:
            def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
                if cite_prefix == 'doi':
                    return f'DOI paper {cite_key}'
                return None

        environ = mk_flm_environ(
            external_citations_providers=[Provider1(), Provider2()]
        )
        result = render_cite_doc(environ, r'See \cite{doi:10.1234/foo}.')
        self.assertEqual(
            result['text'],
            'See <span class="citation-marks">'
            '<a href="#citation-1" class="href-endnote endnote citation">[1]</a>'
            '</span>.'
        )
        self.assertEqual(
            result['endnotes'],
            '<div class="endnotes"><dl class="enumeration citation-list">'
            '<dt id="citation-1">[1]</dt><dd>DOI paper 10.1234/foo</dd>'
            '</dl></div>'
        )


class TestFeatureCiteTextRenderer(unittest.TestCase):

    maxDiff = None

    def test_single_citation_text(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'Citation \cite{arxiv:1234.56789}.',
                                 TextFragmentRenderer)
        self.assertEqual(result['text'], 'Citation [1].')
        self.assertEqual(
            result['endnotes'],
            '  [1] arXiv paper arXiv:1234.56789'
        )

    def test_multiple_citations_text(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ, r'See \cite{arxiv:1111.11111,arxiv:2222.22222}.',
            TextFragmentRenderer
        )
        self.assertEqual(result['text'], 'See [1,2].')
        self.assertEqual(
            result['endnotes'],
            '  [1] arXiv paper arXiv:1111.11111\n\n  [2] arXiv paper arXiv:2222.22222'
        )

    def test_citation_with_extra_text(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ, r'See \cite[Thm~1]{arxiv:1111.11111}.',
            TextFragmentRenderer
        )
        self.assertEqual(result['text'], 'See [1; Thm\xa01].')

    def test_same_citation_twice_text(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'First \cite{arxiv:1111.11111}. Second \cite{arxiv:1111.11111}.',
            TextFragmentRenderer
        )
        self.assertEqual(result['text'], 'First [1]. Second [1].')

    def test_triple_chain_text(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'\cite{arxiv:1111.11111}\cite{arxiv:2222.22222}\cite{arxiv:3333.33333}',
            TextFragmentRenderer
        )
        self.assertEqual(result['text'], '[1\u20133]')


class TestFeatureCiteLatexRenderer(unittest.TestCase):

    maxDiff = None

    def test_single_citation_latex(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'Citation \cite{arxiv:1234.56789}.',
                                 LatexFragmentRenderer)
        self.assertEqual(
            result['text'],
            'Citation \\hyperref[{x:citation-1}]{[1]}%\n.'
        )
        self.assertTrue(
            result['endnotes'].startswith('% --- begin  ---')
        )

    def test_multiple_citations_latex(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ, r'See \cite{arxiv:1111.11111,arxiv:2222.22222}.',
            LatexFragmentRenderer
        )
        self.assertEqual(
            result['text'],
            'See [\\hyperref[{x:citation-1}]{1}%\n'
            ',\\hyperref[{x:citation-2}]{2}%\n].'
        )

    def test_citation_with_extra_latex(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ, r'See \cite[Thm~1]{arxiv:1111.11111}.',
            LatexFragmentRenderer
        )
        self.assertEqual(
            result['text'],
            'See \\hyperref[{x:citation-1}]{[1; Thm~1]}%\n.'
        )

    def test_triple_chain_latex(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'\cite{arxiv:1111.11111}\cite{arxiv:2222.22222}\cite{arxiv:3333.33333}',
            LatexFragmentRenderer
        )
        self.assertEqual(
            result['text'],
            '[\\hyperref[{x:citation-1}]{1}%\n'
            '{\\textendash}\\hyperref[{x:citation-3}]{3}%\n]'
        )


class TestFeatureCiteMarkdownRenderer(unittest.TestCase):

    maxDiff = None

    def test_single_citation_markdown(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(environ, r'Citation \cite{arxiv:1234.56789}.',
                                 MarkdownFragmentRenderer)
        self.assertEqual(
            result['text'],
            'Citation [\\[1\\]](#citation-1)\\.'
        )
        self.assertEqual(
            result['endnotes'],
            '\n- \\[1\\] <a name="citation-1"></a> *arXiv* paper arXiv:1234\\.56789'
        )

    def test_multiple_citations_markdown(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ, r'See \cite{arxiv:1111.11111,arxiv:2222.22222}.',
            MarkdownFragmentRenderer
        )
        self.assertEqual(
            result['text'],
            'See \\[[1](#citation-1),[2](#citation-2)\\]\\.'
        )
        self.assertEqual(
            result['endnotes'],
            '\n- \\[1\\] <a name="citation-1"></a> *arXiv* paper arXiv:1111\\.11111'
            '\n\n- \\[2\\] <a name="citation-2"></a> *arXiv* paper arXiv:2222\\.22222'
        )

    def test_citation_with_extra_markdown(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ, r'See \cite[Thm~1]{arxiv:1111.11111}.',
            MarkdownFragmentRenderer
        )
        self.assertEqual(
            result['text'],
            'See [\\[1; Thm\xa01\\]](#citation-1)\\.'
        )

    def test_triple_chain_markdown(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        result = render_cite_doc(
            environ,
            r'\cite{arxiv:1111.11111}\cite{arxiv:2222.22222}\cite{arxiv:3333.33333}',
            MarkdownFragmentRenderer
        )
        self.assertEqual(
            result['text'],
            '\\[[1](#citation-1)\u2013[3](#citation-3)\\]'
        )


# ----------------------------------------------------------------
# New tests — recomposer
# ----------------------------------------------------------------

class TestFeatureCiteRecomposer(unittest.TestCase):

    maxDiff = None

    def test_recompose_single_key(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(r'\cite{arxiv:1234.56789}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{arxiv:1234.56789}}'
        )

    def test_recompose_multiple_keys(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(r'\cite{arxiv:1111.11111,arxiv:2222.22222}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{arxiv:1111.11111,arxiv:2222.22222}}'
        )

    def test_recompose_with_extra(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(r'\cite[Theorem~45]{arxiv:1111.11111}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite[{Theorem~45}]{arxiv:1111.11111}}'
        )

    def test_recompose_bare_key(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(r'\cite{mykey}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{None:mykey}}'
        )

    def test_recompose_chained_cite(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(
            r'\cite{arxiv:1111.11111}\cite[Thm~2]{arxiv:2222.22222}'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{arxiv:1111.11111}'
            r'\protect\cite[{Thm~2}]{arxiv:2222.22222}}'
        )

    def test_recompose_three_keys(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(
            r'\cite{arxiv:1111.11111,arxiv:2222.22222,arxiv:3333.33333}'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{'
            r'arxiv:1111.11111,arxiv:2222.22222,arxiv:3333.33333}}'
        )

    def test_recompose_mixed_prefix_and_bare(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(r'\cite{arxiv:1111.11111,key1}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{arxiv:1111.11111,None:key1}}'
        )

    def test_recompose_chained_both_extras(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(
            r'\cite[Thm~1]{arxiv:1111.11111}\cite[Cor~2]{arxiv:2222.22222}'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite[{Thm~1}]{arxiv:1111.11111}'
            r'\protect\cite[{Cor~2}]{arxiv:2222.22222}}'
        )

    def test_recompose_chain_multikeys_plus_extra(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(
            r'\cite{arxiv:1111.11111,arxiv:3333.33333}\cite[p.~5]{arxiv:2222.22222}'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{arxiv:1111.11111,arxiv:3333.33333}'
            r'\protect\cite[{p.~5}]{arxiv:2222.22222}}'
        )

    def test_recompose_safe_labels(self):
        environ = mk_flm_environ(
            external_citations_providers=[MyCitationsProvider()]
        )
        frag = environ.make_fragment(r'\cite{arxiv:1234.56789}')
        recomposer = FLMPureLatexRecomposer({'cite': {'safe_labels': True}})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\NoCaseChange{\protect\cite{cite1}}'
        )


if __name__ == '__main__':
    import logging
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
