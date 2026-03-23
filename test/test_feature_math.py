import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.feature.math import FeatureMath, MathEnvironment, MathEqrefMacro
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

class TestFeatureMathInit(unittest.TestCase):

    def test_feature_name(self):
        f = FeatureMath()
        self.assertEqual(f.feature_name, 'math')

    def test_feature_title(self):
        f = FeatureMath()
        self.assertEqual(
            f.feature_title,
            'Mathematical typesetting: equations and equation references'
        )

    def test_default_eqref_macro_name(self):
        f = FeatureMath()
        self.assertEqual(f.eqref_macro_name, 'eqref')

    def test_default_eqref_ref_type(self):
        f = FeatureMath()
        self.assertEqual(f.eqref_ref_type, 'eq')

    def test_default_math_environment_names(self):
        f = FeatureMath()
        self.assertEqual(
            f.math_environment_names,
            ('equation', 'equation*', 'align', 'align*', 'gather', 'gather*')
        )

    def test_environment_definitions(self):
        f = FeatureMath()
        defs = f.add_latex_context_definitions()
        env_names = [e.environmentname for e in defs['environments']]
        self.assertEqual(
            env_names,
            ['equation', 'equation*', 'align', 'align*', 'gather', 'gather*']
        )

    def test_macro_definitions(self):
        f = FeatureMath()
        defs = f.add_latex_context_definitions()
        macro_names = [m.macroname for m in defs['macros']]
        self.assertEqual(macro_names, ['eqref'])

    def test_starred_envs_unnumbered(self):
        f = FeatureMath()
        defs = f.add_latex_context_definitions()
        envs_by_name = {e.environmentname: e for e in defs['environments']}
        self.assertTrue(envs_by_name['equation'].is_numbered)
        self.assertFalse(envs_by_name['equation*'].is_numbered)
        self.assertTrue(envs_by_name['align'].is_numbered)
        self.assertFalse(envs_by_name['align*'].is_numbered)

    def test_starred_envs_standalone(self):
        f = FeatureMath()
        defs = f.add_latex_context_definitions()
        envs_by_name = {e.environmentname: e for e in defs['environments']}
        self.assertFalse(envs_by_name['equation'].allowed_in_standalone_mode)
        self.assertTrue(envs_by_name['equation*'].allowed_in_standalone_mode)

    def test_eqref_not_standalone(self):
        f = FeatureMath()
        defs = f.add_latex_context_definitions()
        self.assertFalse(defs['macros'][0].allowed_in_standalone_mode)

    def test_optional_dependencies(self):
        f = FeatureMath()
        self.assertEqual(f.feature_optional_dependencies, ['refs', 'numbering'])


# --- HTML rendering ---

class TestFeatureMathHtml(unittest.TestCase):

    maxDiff = None

    def test_equation_star(self):
        env = mk_flm_environ()
        result = render_doc(env, r'\begin{equation*}E = mc^2\end{equation*}')
        self.assertEqual(
            result,
            '<span id="equation-1" class="display-math env-equation-star">'
            r'\begin{equation*}E = mc^2\tag*{(1)}\end{equation*}</span>'
        )

    def test_align_star(self):
        env = mk_flm_environ()
        result = render_doc(env, r'\begin{align*}a &= b \\ c &= d\end{align*}')
        self.assertEqual(
            result,
            '<span id="equation-1" class="display-math env-align-star">'
            r'\begin{align*}a &amp;= b \tag*{(1)}\\ c &amp;= d\tag*{(2)}\end{align*}</span>'
        )

    def test_gather_star(self):
        env = mk_flm_environ()
        result = render_doc(env, r'\begin{gather*}x + y = z\end{gather*}')
        self.assertEqual(
            result,
            '<span id="equation-1" class="display-math env-gather-star">'
            r'\begin{gather*}x + y = z\tag*{(1)}\end{gather*}</span>'
        )

    def test_equation_with_label(self):
        env = mk_flm_environ()
        result = render_doc(env,
            r'\begin{equation}\label{eq:euler}e^{i\pi} + 1 = 0\end{equation}')
        self.assertEqual(
            result,
            '<span id="equation-1" class="display-math env-equation">'
            r'\begin{equation}\label{eq:euler}e^{i\pi} + 1 = 0\tag*{(1)}\end{equation}</span>'
        )

    def test_eqref(self):
        env = mk_flm_environ()
        src = r'\begin{equation}\label{eq:test}a = b\end{equation} See \eqref{eq:test}.'
        result = render_doc(env, src)
        self.assertEqual(
            result,
            '<span id="equation-1" class="display-math env-equation">'
            r'\begin{equation}\label{eq:test}a = b\tag*{(1)}\end{equation}</span>'
            ' See <a href="#equation-1" class="href-ref ref-eq">(1)</a>.'
        )


# --- Text renderer ---

class TestFeatureMathText(unittest.TestCase):

    maxDiff = None

    def test_equation_star(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{equation*}E = mc^2\end{equation*}',
            TextFragmentRenderer)
        self.assertEqual(
            result,
            r'\begin{equation*}E = mc^2\tag*{(1)}\end{equation*}'
        )

    def test_eqref(self):
        env = mk_flm_environ()
        src = r'\begin{equation}\label{eq:test}a = b\end{equation} See \eqref{eq:test}.'
        result = render_doc_with(env, src, TextFragmentRenderer)
        self.assertEqual(
            result,
            r'\begin{equation}\label{eq:test}a = b\tag*{(1)}\end{equation} See (1).'
        )


# --- LaTeX renderer ---

class TestFeatureMathLatex(unittest.TestCase):

    maxDiff = None

    def test_equation_star(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{equation*}E = mc^2\end{equation*}',
            LatexFragmentRenderer)
        self.assertEqual(
            result,
            r'\begin{equation*}E = mc^2\tag*{(1)}\end{equation*}'
        )

    def test_eqref(self):
        env = mk_flm_environ()
        src = r'\begin{equation}\label{eq:test}a = b\end{equation} See \eqref{eq:test}.'
        result = render_doc_with(env, src, LatexFragmentRenderer)
        self.assertEqual(
            result,
            r'\begin{equation}\label{eq:test}a = b\tag*{(1)}\end{equation}'
            ' See \\hyperref[{x:equation-1}]{(1)}%\n.'
        )


# --- Markdown renderer ---

class TestFeatureMathMarkdown(unittest.TestCase):

    maxDiff = None

    def test_equation_star(self):
        env = mk_flm_environ()
        result = render_doc_with(env,
            r'\begin{equation*}E = mc^2\end{equation*}',
            MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '<a name="equation-1"></a> E = mc^2\\\\tag\\*\\{\\(1\\)\\}'
        )

    def test_eqref(self):
        env = mk_flm_environ()
        src = r'\begin{equation}\label{eq:test}a = b\end{equation} See \eqref{eq:test}.'
        result = render_doc_with(env, src, MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '<a name="equation-1"></a>'
            ' \\\\label\\{eq:test\\}a = b\\\\tag\\*\\{\\(1\\)\\}'
            ' See [\\(1\\)](#equation-1)\\.'
        )


# --- Recomposer ---

class TestFeatureMathRecompose(unittest.TestCase):

    maxDiff = None

    def _recompose(self, flm_input, opts=None):
        env = mk_flm_environ()
        frag = env.make_fragment(flm_input.strip())
        recomposer = FLMPureLatexRecomposer(opts if opts is not None else {})
        return recomposer.recompose_pure_latex(frag.nodes)

    def test_equation_star(self):
        result = self._recompose(r'\begin{equation*}E = mc^2\end{equation*}')
        self.assertEqual(
            result['latex'],
            r'\begin{equation*}E = mc^2\end{equation*}'
        )

    def test_equation_with_label(self):
        result = self._recompose(
            r'\begin{equation}\label{eq:euler}e^{i\pi} + 1 = 0\end{equation}'
        )
        self.assertEqual(
            result['latex'],
            r'\begin{equation}e^{i\pi} + 1 = 0\label{ref1}\end{equation}'
        )

    def test_align_star(self):
        result = self._recompose(r'\begin{align*}a &= b \\ c &= d\end{align*}')
        self.assertEqual(
            result['latex'],
            r'\begin{align*}a &= b \\ c &= d\end{align*}'
        )

    def test_eqref(self):
        result = self._recompose(
            r'\begin{equation}\label{eq:test}a = b\end{equation} See \eqref{eq:test}.'
        )
        self.assertEqual(
            result['latex'],
            r'\begin{equation}a = b\label{ref1}\end{equation} See \eqref{ref1}.'
        )


# --- Error cases ---

class TestFeatureMathErrors(unittest.TestCase):

    def test_eqref_wrong_prefix_raises(self):
        env = mk_flm_environ()
        with self.assertRaises(LatexWalkerLocatedError):
            env.make_fragment(
                r'\begin{equation}\label{eq:test}a=b\end{equation} \eqref{wrong:test}'
            )


if __name__ == '__main__':
    unittest.main()
