import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.feature.verbatim import (
    FeatureVerbatim, VerbatimSpecInfo, VerbatimMacro, VerbatimEnvironment,
)
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer


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

class TestFeatureVerbatimInit(unittest.TestCase):

    def test_feature_name(self):
        f = FeatureVerbatim()
        self.assertEqual(f.feature_name, 'verbatim')

    def test_feature_title(self):
        f = FeatureVerbatim()
        self.assertEqual(f.feature_title, 'Verbatim content typesetting')

    def test_default_include_types(self):
        f = FeatureVerbatim()
        self.assertEqual(f.verbatim_include_types, ('text', 'code', 'a'))

    def test_macro_names(self):
        f = FeatureVerbatim()
        defs = f.add_latex_context_definitions()
        names = [m.macroname for m in defs['macros']]
        self.assertEqual(names, ['verbtext', 'verbcode', 'verba'])

    def test_environment_names(self):
        f = FeatureVerbatim()
        defs = f.add_latex_context_definitions()
        names = [e.environmentname for e in defs['environments']]
        self.assertEqual(names, ['verbatimtext', 'verbatimcode'])

    def test_all_macros_standalone(self):
        f = FeatureVerbatim()
        defs = f.add_latex_context_definitions()
        for m in defs['macros']:
            self.assertTrue(m.allowed_in_standalone_mode)

    def test_verbatimcode_env_is_block_level(self):
        f = FeatureVerbatim()
        defs = f.add_latex_context_definitions()
        envs_by_name = {e.environmentname: e for e in defs['environments']}
        self.assertTrue(envs_by_name['verbatimcode'].is_block_level)
        self.assertFalse(envs_by_name['verbatimtext'].is_block_level)


# --- HTML rendering: standalone macros ---

class TestVerbatimHtmlStandalone(unittest.TestCase):

    maxDiff = None

    def test_verbtext(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verbtext+hello world+'),
            '<span class="verbatimtext verbatimtext-inline">hello world</span>'
        )

    def test_verbcode(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verbcode+x = 1+'),
            '<span class="verbatimcode verbatimcode-inline">x = 1</span>'
        )

    def test_verbcode_with_lang(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verbcode[python]+def f(): pass+'),
            '<span class="verbatimcode verbatimcode-inline verbatim-lang-python">'
            'def f(): pass</span>'
        )

    def test_verba(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verba+keyword+'),
            '<span class="verbatima verbatima-inline">keyword</span>'
        )

    def test_verba_with_lang(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verba[rust]+struct+'),
            '<span class="verbatima verbatima-inline verbatim-lang-rust">struct</span>'
        )

    def test_verbcode_braces(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verbcode+a{b}c+'),
            '<span class="verbatimcode verbatimcode-inline">a{b}c</span>'
        )

    def test_verbtext_html_escaping(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_standalone(env, r'\verbtext+<tag>&amp;+'),
            '<span class="verbatimtext verbatimtext-inline">&lt;tag&gt;&amp;amp;</span>'
        )


# --- HTML rendering: environments ---

class TestVerbatimHtmlEnvironment(unittest.TestCase):

    maxDiff = None

    def test_verbatimtext_env(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_doc(env, r'\begin{verbatimtext}some text here\end{verbatimtext}'),
            '<span class="verbatimtext verbatimtext-environment">some text here</span>'
        )

    def test_verbatimcode_env(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_doc(env, r'\begin{verbatimcode}x = 1\end{verbatimcode}'),
            '<div class="verbatimcode verbatimcode-environment">x = 1</div>'
        )

    def test_verbatimcode_env_with_lang(self):
        env = mk_flm_environ()
        self.assertEqual(
            render_doc(env, r'\begin{verbatimcode}[python]def f(): pass\end{verbatimcode}'),
            '<div class="verbatimcode verbatimcode-environment verbatim-lang-python">'
            'def f(): pass</div>'
        )


# --- Text renderer ---

class TestVerbatimText(unittest.TestCase):

    maxDiff = None

    def _rs(self, environ, s):
        frag = environ.make_fragment(s, standalone_mode=True)
        return frag.render_standalone(TextFragmentRenderer())

    def _rd(self, environ, s):
        frag = environ.make_fragment(s.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(TextFragmentRenderer())
        return result

    def test_verbtext(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbtext+hello world+'), 'hello world')

    def test_verbcode(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode+x = 1+'), 'x = 1')

    def test_verbcode_with_lang(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode[python]+def f(): pass+'), 'def f(): pass')

    def test_verba(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verba+keyword+'), 'keyword')

    def test_verbcode_braces(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode+a{b}c+'), 'a{b}c')

    def test_verbatimtext_env(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rd(e, r'\begin{verbatimtext}some text here\end{verbatimtext}'),
            'some text here'
        )

    def test_verbatimcode_env(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rd(e, r'\begin{verbatimcode}x = 1\end{verbatimcode}'),
            'x = 1'
        )


# --- LaTeX renderer ---

class TestVerbatimLatex(unittest.TestCase):

    maxDiff = None

    def _rs(self, environ, s):
        frag = environ.make_fragment(s, standalone_mode=True)
        return frag.render_standalone(LatexFragmentRenderer())

    def _rd(self, environ, s):
        frag = environ.make_fragment(s.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(LatexFragmentRenderer())
        return result

    def test_verbtext(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbtext+hello world+'), 'hello world')

    def test_verbcode(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode+x = 1+'), 'x = 1')

    def test_verbcode_with_lang(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode[python]+def f(): pass+'), 'def f(): pass')

    def test_verba(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verba+keyword+'), 'keyword')

    def test_verbcode_braces(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode+a{b}c+'), 'a\\{b\\}c')

    def test_verbtext_html_chars(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rs(e, r'\verbtext+<tag>&amp;+'),
            '\\ensuremath{<}tag\\ensuremath{>}\\&amp;'
        )

    def test_verbatimcode_env_block(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rd(e, r'\begin{verbatimcode}x = 1\end{verbatimcode}'),
            '\\begin{verbatim}\nx = 1\\end{verbatim}\n'
        )

    def test_verbatimcode_env_with_lang(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rd(e, r'\begin{verbatimcode}[python]def f(): pass\end{verbatimcode}'),
            '\\begin{verbatim}\ndef f(): pass\\end{verbatim}\n'
        )


# --- Markdown renderer ---

class TestVerbatimMarkdown(unittest.TestCase):

    maxDiff = None

    def _rs(self, environ, s):
        frag = environ.make_fragment(s, standalone_mode=True)
        return frag.render_standalone(MarkdownFragmentRenderer())

    def _rd(self, environ, s):
        frag = environ.make_fragment(s.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(MarkdownFragmentRenderer())
        return result

    def test_verbtext(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbtext+hello world+'), '`` hello world ``')

    def test_verbcode(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode+x = 1+'), '`` x = 1 ``')

    def test_verbcode_with_lang(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rs(e, r'\verbcode[python]+def f(): pass+'),
            '`` def f\\(\\): pass ``'
        )

    def test_verba(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verba+keyword+'), '`` keyword ``')

    def test_verbcode_braces(self):
        e = mk_flm_environ()
        self.assertEqual(self._rs(e, r'\verbcode+a{b}c+'), '`` a\\{b\\}c ``')

    def test_verbatimcode_env(self):
        e = mk_flm_environ()
        self.assertEqual(
            self._rd(e, r'\begin{verbatimcode}x = 1\end{verbatimcode}'),
            '`` x = 1 ``'
        )


# --- Recomposer ---

class TestVerbatimRecompose(unittest.TestCase):

    maxDiff = None

    def _recompose(self, flm_input, opts=None):
        env = mk_flm_environ()
        frag = env.make_fragment(flm_input.strip(), standalone_mode=True)
        recomposer = FLMPureLatexRecomposer(opts if opts is not None else {})
        return recomposer.recompose_pure_latex(frag.nodes)

    def test_default_passthrough_verbcode(self):
        result = self._recompose(r'\verbcode+x = 1+')
        self.assertEqual(result['latex'], r'\verbcode+x = 1+')

    def test_default_passthrough_verbtext(self):
        result = self._recompose(r'\verbtext+hello+')
        self.assertEqual(result['latex'], r'\verbtext+hello+')

    def test_default_passthrough_verba(self):
        result = self._recompose(r'\verba+keyword+')
        self.assertEqual(result['latex'], r'\verba+keyword+')

    def test_fvextra_verbcode(self):
        result = self._recompose(
            r'\verbcode+x = 1+',
            {'verbatim': {'use_fvextra': True}}
        )
        self.assertEqual(
            result['latex'],
            r'\EscVerb[formatcom=\flmFmtVRB{verbcode}]{x\ \=\ 1}'
        )
        self.assertTrue('fvextra' in result['packages'])

    def test_fvextra_verbtext(self):
        result = self._recompose(
            r'\verbtext+hello+',
            {'verbatim': {'use_fvextra': True}}
        )
        self.assertEqual(
            result['latex'],
            r'\EscVerb[formatcom=\flmFmtVRB{verbtext}]{hello}'
        )

    def test_fvextra_verba(self):
        result = self._recompose(
            r'\verba+keyword+',
            {'verbatim': {'use_fvextra': True}}
        )
        self.assertEqual(
            result['latex'],
            r'\EscVerb[formatcom=\flmFmtVRB{verba}]{keyword}'
        )


if __name__ == '__main__':
    unittest.main()
