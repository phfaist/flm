import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.feature.enumeration import FeatureEnumeration, Enumeration
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer
from pylatexenc.latexnodes import LatexWalkerLocatedError


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


def render_doc_with(environ, flm_input, renderer_cls):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    result, _ = doc.render(renderer_cls())
    return result


# --- Feature attributes ---

class TestFeatureEnumerationInit(unittest.TestCase):

    def test_feature_name(self):
        f = FeatureEnumeration()
        self.assertEqual(f.feature_name, 'enumeration')

    def test_feature_title(self):
        f = FeatureEnumeration()
        self.assertEqual(f.feature_title, 'Enumeration and itemization lists')

    def test_environment_names(self):
        f = FeatureEnumeration()
        defs = f.add_latex_context_definitions()
        env_names = sorted([e.environmentname for e in defs['environments']])
        self.assertEqual(env_names, ['description', 'enumerate', 'itemize'])

    def test_enumeration_is_block_level(self):
        e = Enumeration('itemize')
        self.assertTrue(e.is_block_level)
        self.assertTrue(e.allowed_in_standalone_mode)

    def test_no_managers(self):
        f = FeatureEnumeration()
        self.assertIsNone(f.DocumentManager)
        self.assertIsNone(f.RenderManager)


# --- HTML rendering ---

class TestEnumerationHtml(unittest.TestCase):

    maxDiff = None

    def test_itemize(self):
        env = mk_flm_environ()
        result = render_doc(env, r'\begin{itemize}\item First\item Second\end{itemize}')
        self.assertEqual(
            result,
            '<dl class="enumeration itemize">'
            '<dt>\u2022</dt><dd><p>First</p></dd>'
            '<dt>\u2022</dt><dd><p>Second</p></dd>'
            '</dl>'
        )

    def test_enumerate(self):
        env = mk_flm_environ()
        result = render_doc(env, r'\begin{enumerate}\item Alpha\item Beta\end{enumerate}')
        self.assertEqual(
            result,
            '<dl class="enumeration enumerate">'
            '<dt>1.</dt><dd><p>Alpha</p></dd>'
            '<dt>2.</dt><dd><p>Beta</p></dd>'
            '</dl>'
        )

    def test_nested_itemize(self):
        env = mk_flm_environ()
        result = render_doc(env,
            r'\begin{itemize}\item Outer\begin{itemize}\item Inner\end{itemize}\end{itemize}')
        self.assertEqual(
            result,
            '<dl class="enumeration itemize">'
            '<dt>\u2022</dt><dd><p>Outer</p>\n'
            '<dl class="enumeration itemize">'
            '<dt>-</dt><dd><p>Inner</p></dd>'
            '</dl></dd></dl>'
        )

    def test_enumerate_custom_template(self):
        env = mk_flm_environ()
        result = render_doc(env,
            r'\begin{enumerate}[i.]\item First\item Second\end{enumerate}')
        self.assertEqual(
            result,
            '<dl class="enumeration enumerate">'
            '<dt>i.</dt><dd><p>First</p></dd>'
            '<dt>ii.</dt><dd><p>Second</p></dd>'
            '</dl>'
        )

    def test_item_custom_tag(self):
        env = mk_flm_environ()
        result = render_doc(env,
            r'\begin{enumerate}\item[*] Custom tag\end{enumerate}')
        self.assertEqual(
            result,
            '<dl class="enumeration enumerate">'
            '<dt>*</dt><dd><p>Custom tag</p></dd>'
            '</dl>'
        )


# --- Text renderer ---

class TestEnumerationText(unittest.TestCase):

    maxDiff = None

    def test_itemize(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{itemize}\item First\item Second\end{itemize}',
            TextFragmentRenderer)
        self.assertEqual(
            result,
            '  \u2022 First\n\n  \u2022 Second'
        )

    def test_enumerate(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{enumerate}\item Alpha\item Beta\end{enumerate}',
            TextFragmentRenderer)
        self.assertEqual(
            result,
            '  1. Alpha\n\n  2. Beta'
        )

    def test_enumerate_roman(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{enumerate}[i.]\item First\item Second\end{enumerate}',
            TextFragmentRenderer)
        self.assertEqual(
            result,
            '   i. First\n\n  ii. Second'
        )


# --- LaTeX renderer ---

class TestEnumerationLatex(unittest.TestCase):

    maxDiff = None

    def test_itemize(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{itemize}\item First\item Second\end{itemize}',
            LatexFragmentRenderer)
        self.assertEqual(
            result,
            '\\begin{itemize}% enumeration,itemize\n'
            '%\n'
            '\\item[{{\\textbullet}}]First\n'
            '%\n'
            '\\item[{{\\textbullet}}]Second%\n'
            '\\end{itemize}\n'
        )

    def test_enumerate(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{enumerate}\item Alpha\item Beta\end{enumerate}',
            LatexFragmentRenderer)
        self.assertEqual(
            result,
            '\\begin{enumerate}% enumeration,enumerate\n'
            '%\n'
            '\\item[{1.}]Alpha\n'
            '%\n'
            '\\item[{2.}]Beta%\n'
            '\\end{enumerate}\n'
        )


# --- Markdown renderer ---

class TestEnumerationMarkdown(unittest.TestCase):

    maxDiff = None

    def test_itemize(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{itemize}\item First\item Second\end{itemize}',
            MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '- \u2022 First\n\n- \u2022 Second'
        )

    def test_enumerate(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{enumerate}\item Alpha\item Beta\end{enumerate}',
            MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '- 1\\. Alpha\n\n- 2\\. Beta'
        )

    def test_custom_tag(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{enumerate}\item[*] Custom tag\end{enumerate}',
            MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '- \\* Custom tag'
        )


# --- Recomposer ---

class TestEnumerationRecompose(unittest.TestCase):

    maxDiff = None

    def _recompose(self, flm_input, opts=None):
        env = mk_flm_environ()
        frag = env.make_fragment(flm_input.strip())
        recomposer = FLMPureLatexRecomposer(opts if opts is not None else {})
        return recomposer.recompose_pure_latex(frag.nodes)

    def test_itemize(self):
        result = self._recompose(
            r'\begin{itemize}\item First\item Second\end{itemize}')
        self.assertEqual(
            result['latex'],
            r'\begin{itemize}\item First\item Second\end{itemize}'
        )

    def test_enumerate(self):
        result = self._recompose(
            r'\begin{enumerate}\item Alpha\item Beta\end{enumerate}')
        self.assertEqual(
            result['latex'],
            r'\begin{enumerate}\item Alpha\item Beta\end{enumerate}'
        )


# --- Error cases ---

class TestEnumerationErrors(unittest.TestCase):

    def test_missing_item_raises(self):
        env = mk_flm_environ()
        with self.assertRaises(LatexWalkerLocatedError):
            env.make_fragment(
                r'\begin{itemize}no item here\end{itemize}'
            )


if __name__ == '__main__':
    unittest.main()
