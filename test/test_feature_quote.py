import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer

from flm.feature.quote import (
    FeatureQuote, QuoteEnvironment, nodelist_strip_surrounding_whitespace
)


def mk_flm_environ(**quote_kwargs):
    features = standard_features()
    features.append(FeatureQuote(**quote_kwargs))
    return make_standard_environment(features)


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result



class TestFeatureQuoteInit(unittest.TestCase):

    def test_default_environments(self):
        feature = FeatureQuote()
        self.assertTrue('quote' in feature.quote_environments)
        self.assertTrue('address' in feature.quote_environments)
        self.assertTrue('blockquote' in feature.quote_environments)

    def test_custom_environments(self):
        feature = FeatureQuote(quote_environments={
            'myquote': {
                'enabled_quote_sections': ['text', 'attributed'],
            },
        })
        self.assertTrue('myquote' in feature.quote_environments)
        self.assertTrue('quote' not in feature.quote_environments)

    def test_latex_context_definitions(self):
        feature = FeatureQuote()
        defs = feature.add_latex_context_definitions()
        self.assertTrue('environments' in defs)
        env_names = [e.environmentname for e in defs['environments']]
        self.assertTrue('quote' in env_names)
        self.assertTrue('address' in env_names)
        self.assertTrue('blockquote' in env_names)

    def test_mk_quote_environment_spec_from_dict(self):
        feature = FeatureQuote()
        spec = feature._mk_quote_environment_spec('myenv', {
            'enabled_quote_sections': ['text'],
        })
        self.assertTrue(isinstance(spec, QuoteEnvironment))
        self.assertEqual(spec.environmentname, 'myenv')
        self.assertEqual(spec.enabled_quote_sections, ['text'])

    def test_mk_quote_environment_spec_from_instance(self):
        feature = FeatureQuote()
        qenv = QuoteEnvironment('myenv', enabled_quote_sections=['text'])
        spec = feature._mk_quote_environment_spec('myenv', qenv)
        self.assertIs(spec, qenv)

    def test_mk_quote_environment_spec_name_mismatch(self):
        feature = FeatureQuote()
        qenv = QuoteEnvironment('other', enabled_quote_sections=['text'])
        with self.assertRaises(ValueError):
            feature._mk_quote_environment_spec('myenv', qenv)


class TestFeatureQuoteBlockquote(unittest.TestCase):
    """Tests for the blockquote environment (auto_quote_section_bare_content='block')."""

    maxDiff = None

    def test_simple_blockquote(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{blockquote}
  Hello world.
\end{blockquote}
""")
        self.assertEqual(
            result,
            '<div class="blockquote">'
            '<div class="quote-block"><p>Hello world.</p></div>'
            '</div>'
        )

    def test_blockquote_multiline(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{blockquote}
First paragraph.

Second paragraph.
\end{blockquote}
""")
        self.assertEqual(
            result,
            '<div class="blockquote">'
            '<div class="quote-block">'
            '<p>First paragraph.</p>\n<p>Second paragraph.</p>'
            '</div>'
            '</div>'
        )

    def test_blockquote_with_formatting(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{blockquote}Some \emph{emphasized} text.\end{blockquote}
""")
        self.assertEqual(
            result,
            '<div class="blockquote">'
            '<div class="quote-block">'
            '<p>Some <span class="textit">emphasized</span> text.</p>'
            '</div>'
            '</div>'
        )


class TestFeatureQuoteAddress(unittest.TestCase):
    """Tests for the address environment (auto_quote_section_bare_content='lines')."""

    maxDiff = None

    def test_simple_address(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{address}
1200 E California Blvd\\
Pasadena, CA 91125\\
U.S.A.
\end{address}
""")
        self.assertEqual(
            result,
            '<div class="address">'
            '<p class="lines quote-lines">'
            '<span>1200 E California Blvd</span><br>'
            '<span>Pasadena, CA 91125</span><br>'
            '<span>U.S.A.</span>'
            '</p>'
            '</div>'
        )


class TestFeatureQuoteQuote(unittest.TestCase):
    """Tests for the quote environment with explicit quote sections."""

    maxDiff = None

    def test_quote_text_section(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\text{To be or not to be.}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<div class="quote-text">To be or not to be.</div>'
            '</div>'
        )

    def test_quote_attributed_section(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\text{To be or not to be.}\attributed{William Shakespeare}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<div class="quote-text">To be or not to be.</div>\n'
            '<div class="quote-attributed">William Shakespeare</div>'
            '</div>'
        )

    def test_quote_block_section(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\block{This is a block quote paragraph.}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<div class="quote-block">This is a block quote paragraph.</div>'
            '</div>'
        )

    def test_quote_lines_section(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}
  \lines{Roses are red\\
    Violets are blue
}
\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>Roses are red</span><br><span>Violets are blue</span>'
            '</p>'
            '</div>'
        )

    def test_quote_lines_single_line(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\lines{A single line of poetry}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>A single line of poetry</span>'
            '</p>'
            '</div>'
        )

    def test_quote_text_and_attributed(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}
\text{The only thing we have to fear is fear itself.}
\attributed{Franklin D. Roosevelt, 1933}
\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<div class="quote-text">The only thing we have to fear is fear itself.</div>\n'
            '<div class="quote-attributed">Franklin D. Roosevelt, 1933</div>'
            '</div>'
        )

    def test_quote_lines_and_attributed(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}
\lines{Shall I compare thee to a summer's day?\\Thou art more lovely and more temperate.}
\attributed{William Shakespeare, Sonnet 18}
\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            "<span>Shall I compare thee to a summer\u2019s day?</span><br>"
            "<span>Thou art more lovely and more temperate.</span>"
            '</p>\n'
            '<div class="quote-attributed">William Shakespeare, Sonnet 18</div>'
            '</div>'
        )

    def test_quote_multiple_sections(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}
\text{First quote.}
\text{Second quote.}
\attributed{An author}
\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<div class="quote-text">First quote.</div>\n'
            '<div class="quote-text">Second quote.</div>\n'
            '<div class="quote-attributed">An author</div>'
            '</div>'
        )


class TestFeatureQuoteErrors(unittest.TestCase):
    """Tests for error handling in quote environments."""

    maxDiff = None

    def test_bare_content_in_quote_raises(self):
        """Bare content in a quote environment (no auto_quote_section_bare_content)
        should raise an error."""
        environ = mk_flm_environ()
        with self.assertRaises(LatexWalkerLocatedError):
            render_doc(environ, r"""
\begin{quote}Some bare content without a section macro.\end{quote}
""")


class TestFeatureQuoteCustomEnvironments(unittest.TestCase):
    """Tests for custom quote environment configurations."""

    maxDiff = None

    def test_custom_env_text_only(self):
        environ = mk_flm_environ(quote_environments={
            'myquote': {
                'enabled_quote_sections': ['text'],
            },
        })
        result = render_doc(environ, r"""
\begin{myquote}\text{Custom quote text.}\end{myquote}
""")
        self.assertEqual(
            result,
            '<div class="myquote">'
            '<div class="quote-text">Custom quote text.</div>'
            '</div>'
        )

    def test_custom_env_auto_block(self):
        environ = mk_flm_environ(quote_environments={
            'aside': {
                'enabled_quote_sections': [],
                'auto_quote_section_bare_content': 'block',
            },
        })
        result = render_doc(environ, r"""
\begin{aside}An aside remark.\end{aside}
""")
        self.assertEqual(
            result,
            '<div class="aside">'
            '<div class="quote-block"><p>An aside remark.</p></div>'
            '</div>'
        )


class TestNodelistStripSurroundingWhitespace(unittest.TestCase):
    """Tests for nodelist_strip_surrounding_whitespace, exercised via \\lines{}
    which calls it on each line after splitting on \\\\."""

    maxDiff = None

    def test_strips_whitespace_around_line_breaks(self):
        """Whitespace around \\\\ should be stripped from each line."""
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}
\lines{  first line  \\  second line  }
\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>first line</span><br><span>second line</span>'
            '</p>'
            '</div>'
        )

    def test_no_whitespace_to_strip(self):
        """Content without surrounding whitespace passes through unchanged."""
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\lines{no spaces\\also none}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>no spaces</span><br><span>also none</span>'
            '</p>'
            '</div>'
        )

    def test_strips_leading_whitespace_with_formatting(self):
        """Leading whitespace before a formatting macro (pre_space) is stripped."""
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\lines{plain text\\ \emph{emphasized line}}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>plain text</span><br>'
            '<span><span class="textit">emphasized line</span></span>'
            '</p>'
            '</div>'
        )

    def test_strips_trailing_whitespace_before_line_break(self):
        """Trailing whitespace before \\\\ is stripped from the preceding line."""
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\lines{line one   \\line two}\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>line one</span><br><span>line two</span>'
            '</p>'
            '</div>'
        )

    def test_three_lines(self):
        """Three lines: br between each pair, whitespace stripped."""
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{quote}\lines{ one \\  two  \\  three }\end{quote}
""")
        self.assertEqual(
            result,
            '<div class="quote">'
            '<p class="lines quote-lines">'
            '<span>one</span><br><span>two</span><br><span>three</span>'
            '</p>'
            '</div>'
        )

    def test_direct_empty_nodelist(self):
        """nodelist_strip_surrounding_whitespace on an empty nodelist returns it as-is."""
        environ = mk_flm_environ()
        lw = environ.make_fragment(r'hello', standalone_mode=True).nodes.latex_walker
        empty_nl = lw.make_nodelist([], parsing_state=lw.make_parsing_state())
        result = nodelist_strip_surrounding_whitespace(empty_nl)
        self.assertIs(result, empty_nl)

    def test_direct_no_whitespace(self):
        """nodelist_strip_surrounding_whitespace on content without surrounding
        whitespace returns an equivalent nodelist."""
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'hello world', standalone_mode=True)
        result = nodelist_strip_surrounding_whitespace(frag.nodes)
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.render_nodelist(result, fr.ensure_render_context(None), is_block_level=False),
            'hello world'
        )

    def test_direct_leading_whitespace(self):
        """nodelist_strip_surrounding_whitespace strips leading whitespace."""
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'  hello', standalone_mode=True)
        result = nodelist_strip_surrounding_whitespace(frag.nodes)
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.render_nodelist(result, fr.ensure_render_context(None), is_block_level=False),
            'hello'
        )

    def test_direct_trailing_whitespace(self):
        """nodelist_strip_surrounding_whitespace strips trailing whitespace."""
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'hello  ', standalone_mode=True)
        result = nodelist_strip_surrounding_whitespace(frag.nodes)
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.render_nodelist(result, fr.ensure_render_context(None), is_block_level=False),
            'hello'
        )

    def test_direct_both_sides_whitespace(self):
        """nodelist_strip_surrounding_whitespace strips both leading and trailing."""
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'  hello world  ', standalone_mode=True)
        result = nodelist_strip_surrounding_whitespace(frag.nodes)
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.render_nodelist(result, fr.ensure_render_context(None), is_block_level=False),
            'hello world'
        )


if __name__ == '__main__':
    import logging
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
