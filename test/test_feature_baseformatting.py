import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.feature.baseformatting import FeatureBaseFormatting, NoExtraSpaceAfterDotMacro
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from pylatexenc.latexnodes import LatexWalkerParseError


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_standalone(environ, flm_input):
    frag = environ.make_fragment(flm_input, standalone_mode=True)
    return frag.render_standalone(HtmlFragmentRenderer())


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


# --- Feature attributes ---

class TestFeatureBaseFormattingAttributes(unittest.TestCase):

    def test_feature_name(self):
        f = FeatureBaseFormatting()
        self.assertEqual(f.feature_name, 'baseformatting')

    def test_feature_title(self):
        f = FeatureBaseFormatting()
        self.assertEqual(f.feature_title, 'Basic formatting')

    def test_no_dependencies(self):
        f = FeatureBaseFormatting()
        self.assertIsNone(f.feature_dependencies)


# --- Constant value macros ---

class TestConstantValueMacros(unittest.TestCase):

    maxDiff = None

    def test_textbackslash(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\textbackslash{}'), '\\')

    def test_percent(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\%'), '%')

    def test_hash(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\#'), '#')

    def test_ampersand(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\&'), '&amp;')

    def test_dollar(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\$'), '$')

    def test_backslash_space(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'a\ b'), 'a b')

    def test_lbrace(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\{'), '{')

    def test_rbrace(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'\}'), '}')


# --- Text format macros ---

class TestTextFormatMacros(unittest.TestCase):

    maxDiff = None

    def test_emph(self):
        environ = mk_flm_environ()
        self.assertEqual(
            render_standalone(environ, r'\emph{hello}'),
            '<span class="textit">hello</span>'
        )

    def test_textit(self):
        environ = mk_flm_environ()
        self.assertEqual(
            render_standalone(environ, r'\textit{hello}'),
            '<span class="textit">hello</span>'
        )

    def test_textbf(self):
        environ = mk_flm_environ()
        self.assertEqual(
            render_standalone(environ, r'\textbf{hello}'),
            '<span class="textbf">hello</span>'
        )

    def test_nested_bold_italic(self):
        environ = mk_flm_environ()
        self.assertEqual(
            render_standalone(environ, r'\textbf{\textit{bold italic}}'),
            '<span class="textbf"><span class="textit">bold italic</span></span>'
        )

    def test_emph_in_bold(self):
        environ = mk_flm_environ()
        self.assertEqual(
            render_standalone(environ, r'\textbf{\emph{emphasized}}'),
            '<span class="textbf"><span class="textit">emphasized</span></span>'
        )


# --- Specials ---

class TestBaseFormattingSpecials(unittest.TestCase):

    maxDiff = None

    def test_tilde_nbsp(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'a~b'), 'a&nbsp;b')

    def test_paragraph_break(self):
        environ = mk_flm_environ()
        self.assertEqual(
            render_doc(environ, 'First.\n\nSecond.'),
            '<p>First.</p>\n<p>Second.</p>'
        )


# --- NoExtraSpaceAfterDotMacro (\@) ---

class TestNoExtraSpaceAfterDotMacro(unittest.TestCase):

    maxDiff = None

    def test_at_renders_empty_html_standalone(self):
        environ = mk_flm_environ()
        self.assertEqual(render_standalone(environ, r'Mr.\@ Bond.'), 'Mr. Bond.')

    def test_at_renders_empty_html_doc(self):
        environ = mk_flm_environ()
        self.assertEqual(render_doc(environ, r'Mr.\@ Bond.'), 'Mr. Bond.')

    def test_at_renders_latex(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'Mr.\@ Bond.')
        doc = environ.make_document(frag.render)
        lfr = LatexFragmentRenderer()
        result, _ = doc.render(lfr)
        self.assertEqual(result, r'Mr.\@ Bond.')

    def test_attributes(self):
        macro = NoExtraSpaceAfterDotMacro('@')
        self.assertEqual(macro.macroname, '@')
        self.assertTrue(macro.allowed_in_standalone_mode)


# --- Text renderer ---

class TestBaseFormattingText(unittest.TestCase):

    maxDiff = None

    def _rs(self, environ, s):
        frag = environ.make_fragment(s, standalone_mode=True)
        return frag.render_standalone(TextFragmentRenderer())

    def _rd(self, environ, s):
        frag = environ.make_fragment(s.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(TextFragmentRenderer())
        return result

    def test_textbackslash(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textbackslash{}'), '\\')

    def test_percent(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\%'), '%')

    def test_hash(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\#'), '#')

    def test_ampersand(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\&'), '&')

    def test_dollar(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\$'), '$')

    def test_space(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'a\ b'), 'a b')

    def test_lbrace(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\{'), '{')

    def test_rbrace(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\}'), '}')

    def test_emph(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\emph{hello}'), 'hello')

    def test_textit(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textit{hello}'), 'hello')

    def test_textbf(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textbf{hello}'), 'hello')

    def test_tilde(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'a~b'), 'a\xa0b')

    def test_paragraph_break(self):
        e = mk_flm_environ()
        self.assertEqual(self._rd(e, 'First.\n\nSecond.'), 'First.\n\nSecond.')

    def test_at_macro(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'Mr.\@ Bond.'), 'Mr. Bond.')


# --- LaTeX renderer ---

class TestBaseFormattingLatex(unittest.TestCase):

    maxDiff = None

    def _rs(self, environ, s):
        frag = environ.make_fragment(s, standalone_mode=True)
        return frag.render_standalone(LatexFragmentRenderer())

    def _rd(self, environ, s):
        frag = environ.make_fragment(s.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(LatexFragmentRenderer())
        return result

    def test_textbackslash(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textbackslash{}'), '{\\textbackslash}')

    def test_percent(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\%'), '\\%')

    def test_hash(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\#'), '\\#')

    def test_ampersand(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\&'), '\\&')

    def test_dollar(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\$'), '\\$')

    def test_space(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'a\ b'), 'a b')

    def test_lbrace(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\{'), '\\{')

    def test_rbrace(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\}'), '\\}')

    def test_emph(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\emph{hello}'), '\\textit{hello}')

    def test_textit(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textit{hello}'), '\\textit{hello}')

    def test_textbf(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textbf{hello}'), '\\textbf{hello}')

    def test_tilde(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'a~b'), 'a~b')

    def test_paragraph_break(self):
        e = mk_flm_environ()
        self.assertEqual(self._rd(e, 'First.\n\nSecond.'), 'First.\n\nSecond.\n')

    def test_at_macro(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'Mr.\@ Bond.'), r'Mr.\@ Bond.')


# --- Markdown renderer ---

class TestBaseFormattingMarkdown(unittest.TestCase):

    maxDiff = None

    def _rs(self, environ, s):
        frag = environ.make_fragment(s, standalone_mode=True)
        return frag.render_standalone(MarkdownFragmentRenderer())

    def _rd(self, environ, s):
        frag = environ.make_fragment(s.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(MarkdownFragmentRenderer())
        return result

    def test_textbackslash(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textbackslash{}'), '\\\\')

    def test_percent(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\%'), '%')

    def test_hash(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\#'), '\\#')

    def test_ampersand(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\&'), '&')

    def test_dollar(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\$'), '$')

    def test_space(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'a\ b'), 'a b')

    def test_lbrace(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\{'), '\\{')

    def test_rbrace(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\}'), '\\}')

    def test_emph(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\emph{hello}'), '*hello*')

    def test_textit(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textit{hello}'), '*hello*')

    def test_textbf(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\textbf{hello}'), '**hello**')

    def test_tilde(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'a~b'), 'a\xa0b')

    def test_paragraph_break(self):
        e = mk_flm_environ()
        self.assertEqual(self._rd(e, 'First.\n\nSecond.'), 'First\\.\n\nSecond\\.')

    def test_at_macro(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'Mr.\@ Bond.'), 'Mr\\. Bond\\.')


if __name__ == '__main__':
    unittest.main()
