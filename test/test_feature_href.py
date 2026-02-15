import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.feature.href import FeatureHref, HrefHyperlinkMacro


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_standalone(environ, flm_input):
    frag = environ.make_fragment(flm_input, standalone_mode=True)
    fr = HtmlFragmentRenderer()
    return frag.render_standalone(fr)



class TestFeatureHrefInit(unittest.TestCase):

    def test_feature_name(self):
        feature = FeatureHref()
        self.assertEqual(feature.feature_name, 'href')

    def test_latex_definitions_macros(self):
        feature = FeatureHref()
        defs = feature.add_latex_context_definitions()
        macro_names = [m.macroname for m in defs['macros']]
        self.assertTrue('href' in macro_names)
        self.assertTrue('url' in macro_names)
        self.assertTrue('email' in macro_names)

    def test_href_macro_command_arguments(self):
        feature = FeatureHref()
        defs = feature.add_latex_context_definitions()
        macros_by_name = {m.macroname: m for m in defs['macros']}
        self.assertEqual(
            macros_by_name['href'].command_arguments,
            ('target_href', 'display_text',)
        )
        self.assertEqual(
            macros_by_name['url'].command_arguments,
            ('target_href',)
        )
        self.assertEqual(
            macros_by_name['email'].command_arguments,
            ('target_email',)
        )


class TestHrefHyperlinkMacroPrettyUrl(unittest.TestCase):

    def test_strips_https(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('https://example.com/page'),
            'example.com/page'
        )

    def test_strips_http(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('http://example.com'),
            'example.com'
        )

    def test_strips_mailto(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('mailto:user@example.com'),
            'user@example.com'
        )

    def test_strips_trailing_slash(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('https://example.com/'),
            'example.com'
        )

    def test_strips_trailing_hash(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('https://example.com/#'),
            'example.com'
        )

    def test_strips_trailing_question_mark(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('https://example.com/?'),
            'example.com'
        )

    def test_no_prefix(self):
        self.assertEqual(
            HrefHyperlinkMacro.pretty_url('ftp://files.example.com'),
            'ftp://files.example.com'
        )


class TestFeatureHrefRendering(unittest.TestCase):

    maxDiff = None

    def test_href_with_display_text(self):
        env = mk_flm_environ()
        result = render_standalone(env, r'\href{https://example.com}{Click here}')
        self.assertEqual(
            result,
            '<a href="https://example.com" class="href-href">Click here</a>'
        )

    def test_href_with_formatted_display_text(self):
        env = mk_flm_environ()
        result = render_standalone(env, r'\href{https://example.com}{\textbf{Bold link}}')
        self.assertEqual(
            result,
            '<a href="https://example.com" class="href-href">'
            '<span class="textbf">Bold link</span></a>'
        )

    def test_url_displays_pretty_url(self):
        env = mk_flm_environ()
        result = render_standalone(env, r'\url{https://example.com/page}')
        self.assertEqual(
            result,
            '<a href="https://example.com/page" class="href-href">example.com/page</a>'
        )

    def test_url_strips_trailing_slash(self):
        env = mk_flm_environ()
        result = render_standalone(env, r'\url{https://example.com/}')
        self.assertEqual(
            result,
            '<a href="https://example.com/" class="href-href">example.com</a>'
        )

    def test_email_generates_mailto(self):
        env = mk_flm_environ()
        result = render_standalone(env, r'\email{user@example.com}')
        self.assertEqual(
            result,
            '<a href="mailto:user@example.com" class="href-href">user@example.com</a>'
        )

    def test_href_inline_in_text(self):
        env = mk_flm_environ()
        result = render_standalone(env,
            r'Visit \href{https://example.com}{our site} for more info.')
        self.assertEqual(
            result,
            'Visit <a href="https://example.com" class="href-href">our site</a> for more info.'
        )

    def test_url_with_special_chars(self):
        env = mk_flm_environ()
        result = render_standalone(env, r'\url{https://example.com/path?q=1&x=2}')
        self.assertEqual(
            result,
            '<a href="https://example.com/path?q=1&x=2" class="href-href">'
            'example.com/path?q=1&amp;x=2</a>'
        )


class TestFeatureHrefRecompose(unittest.TestCase):

    maxDiff = None

    def _recompose(self, flm_input, recomposer_opts=None):
        env = mk_flm_environ()
        frag = env.make_fragment(flm_input.strip())
        recomposer = FLMPureLatexRecomposer(
            recomposer_opts if recomposer_opts is not None else {}
        )
        return recomposer.recompose_pure_latex(frag.nodes)

    def test_recompose_href(self):
        result = self._recompose(r'\href{https://example.com}{Click here}')
        self.assertEqual(
            result['latex'],
            r'\href{https://example.com}{Click here}'
        )

    def test_recompose_url(self):
        result = self._recompose(r'\url{https://example.com/page}')
        self.assertEqual(
            result['latex'],
            r'\url{https://example.com/page}'
        )

    def test_recompose_email(self):
        result = self._recompose(r'\email{user@example.com}')
        self.assertEqual(
            result['latex'],
            r'\email{user@example.com}'
        )

    def test_recompose_escapes_hash_in_url(self):
        result = self._recompose(r'\url{https://example.com/page#section}')
        self.assertEqual(
            result['latex'],
            r'\url{https://example.com/page\#section}'
        )

    def test_recompose_escapes_percent_in_url(self):
        result = self._recompose(r'\url{https://example.com/100%done}')
        self.assertEqual(
            result['latex'],
            r'\url{https://example.com/100\%done}'
        )

    def test_recompose_map_macros(self):
        result = self._recompose(
            r'\href{https://example.com}{Link}',
            {'href': {'map_macros': {'href': 'myHref'}}}
        )
        self.assertEqual(
            result['latex'],
            r'\myHref{https://example.com}{Link}'
        )

    def test_recompose_map_macros_url(self):
        result = self._recompose(
            r'\url{https://example.com}',
            {'href': {'map_macros': {'url': 'myUrl'}}}
        )
        self.assertEqual(
            result['latex'],
            r'\myUrl{https://example.com}'
        )

    def test_recompose_map_macros_no_match(self):
        """Unmapped macros keep their original name."""
        result = self._recompose(
            r'\url{https://example.com}',
            {'href': {'map_macros': {'href': 'myHref'}}}
        )
        self.assertEqual(
            result['latex'],
            r'\url{https://example.com}'
        )


if __name__ == '__main__':
    unittest.main()
