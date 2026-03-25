import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.feature.defterm import (
    FeatureDefTerm,
    DefineTermEnvironment,
    RefTermMacro,
    simplify_trim_whitespace,
)
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def mk_flm_environ_custom_defterm(**kwargs):
    features = standard_features(defterm=False)
    features.append(FeatureDefTerm(**kwargs))
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr=None):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    if fr is None:
        fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


def render_doc_nojoin(environ, flm_input, fr=None):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    if fr is None:
        fr = HtmlFragmentRenderer(config=dict(html_blocks_joiner=""))
    result, _ = doc.render(fr)
    return result


# ----------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------

class TestHelpers(unittest.TestCase):

    def test_simplify_trim_whitespace_basic(self):
        self.assertEqual(simplify_trim_whitespace('  hello   world  '), 'hello world')

    def test_simplify_trim_whitespace_tabs(self):
        self.assertEqual(simplify_trim_whitespace('tabs\there'), 'tabs here')

    def test_simplify_trim_whitespace_empty(self):
        self.assertEqual(simplify_trim_whitespace(''), '')

    def test_simplify_trim_whitespace_only_spaces(self):
        self.assertEqual(simplify_trim_whitespace('  '), '')


# ----------------------------------------------------------------
# Init / Config
# ----------------------------------------------------------------

class TestFeatureDefTermInit(unittest.TestCase):

    def test_defaults(self):
        f = FeatureDefTerm()
        self.assertEqual(f.feature_name, 'defterm')
        self.assertTrue(f.render_defterm_with_term)
        self.assertEqual(f.render_defterm_with_term_suffix, ': ')

    def test_custom_config(self):
        f = FeatureDefTerm(render_defterm_with_term=False,
                           render_defterm_with_term_suffix=' -- ')
        self.assertFalse(f.render_defterm_with_term)
        self.assertEqual(f.render_defterm_with_term_suffix, ' -- ')

    def test_latex_context_definitions(self):
        f = FeatureDefTerm()
        defs = f.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 1)
        self.assertEqual(defs['macros'][0].macroname, 'term')
        self.assertEqual(len(defs['environments']), 1)
        self.assertEqual(defs['environments'][0].environmentname, 'defterm')

    def test_define_term_environment_fields(self):
        env = DefineTermEnvironment('defterm')
        self.assertTrue(env.is_block_level)
        self.assertFalse(env.allowed_in_standalone_mode)
        self.assertEqual(env.defterm_ref_type, 'defterm')

    def test_ref_term_macro_fields(self):
        m = RefTermMacro('term')
        self.assertFalse(m.allowed_in_standalone_mode)
        self.assertTrue(m.delayed_render)
        self.assertEqual(m.defterm_ref_type, 'defterm')


# ----------------------------------------------------------------
# HTML Rendering
# ----------------------------------------------------------------

class TestFeatureDefTermHtml(unittest.TestCase):

    maxDiff = None

    def test_simple_defterm(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{defterm}{stabilizer code}A code is called a '
            r'\term{stabilizer code} if it '
            r'is defined as the subspace stabilizerd by a Pauli stabilizer '
            r'group.\end{defterm}'
        )
        self.assertEqual(
            result,
            '<div id="defterm-stabilizer_20Xcode" class="defterm p-block">'
            '<p><span class="defterm-term">stabilizer code: </span>'
            'A code is called a '
            '<span class="defterm-term term-in-defining-defterm">'
            'stabilizer code</span> if it '
            'is defined as the subspace stabilizerd by a Pauli stabilizer '
            'group.</p></div>'
        )

    def test_term_ref_before_defterm(self):
        environ = mk_flm_environ()
        result = render_doc_nojoin(
            environ,
            r'Ref: \term{stabilizer code}.'
            '\n\n'
            r'\begin{defterm}{stabilizer code}A code is called a '
            r'\term{stabilizer code} if it '
            r'is defined as the subspace stabilizerd by a Pauli stabilizer '
            r'group.\end{defterm}'
        )
        self.assertEqual(
            result,
            '<p>Ref: <a href="#defterm-stabilizer_20Xcode" '
            'class="href-term">stabilizer code</a>.</p>'
            '<div id="defterm-stabilizer_20Xcode" class="defterm p-block">'
            '<p><span class="defterm-term">stabilizer code: </span>'
            'A code is called a '
            '<span class="defterm-term term-in-defining-defterm">'
            'stabilizer code</span> if it '
            'is defined as the subspace stabilizerd by a Pauli stabilizer '
            'group.</p></div>'
        )

    def test_cross_ref_between_defterms(self):
        environ = mk_flm_environ(render_defterm_with_term=False)
        result = render_doc_nojoin(
            environ,
            r'\begin{defterm}{a}Definition of \term{a}, related to \term{b}.\end{defterm}'
            '\n'
            r'\begin{defterm}{b}Definition of \term{b}, related to \term{a}.\end{defterm}'
        )
        self.assertEqual(
            result,
            '<div id="defterm-a" class="defterm p-block">'
            '<p>Definition of '
            '<span class="defterm-term term-in-defining-defterm">a</span>'
            ', related to <a href="#defterm-b" class="href-term">b</a>.</p></div>'
            '<div id="defterm-b" class="defterm p-block">'
            '<p>Definition of '
            '<span class="defterm-term term-in-defining-defterm">b</span>'
            ', related to <a href="#defterm-a" class="href-term">a</a>.</p></div>'
        )

    def test_term_with_ref_term_optional_arg(self):
        environ = mk_flm_environ()
        result = render_doc_nojoin(
            environ,
            r'\begin{defterm}{Markov chain}'
            r'A Markov chain is a stochastic process.\end{defterm}'
            '\n\n'
            r'These are \term[Markov chain]{Markov chains}.'
        )
        self.assertEqual(
            result,
            '<div id="defterm-Markov_20Xchain" class="defterm p-block">'
            '<p><span class="defterm-term">Markov chain: </span>'
            'A Markov chain is a stochastic process.</p></div>'
            '<p>These are '
            '<a href="#defterm-Markov_20Xchain" class="href-term">'
            'Markov chains</a>.</p>'
        )

    def test_defterm_with_label_and_ref(self):
        environ = mk_flm_environ()
        result = render_doc_nojoin(
            environ,
            r'\begin{defterm}{stabilizer code}\label{topic:stab}'
            r'A stabilizer code is nice.\end{defterm}'
            '\n\n'
            r'See \ref{topic:stab}.'
        )
        self.assertEqual(
            result,
            '<div id="defterm-stabilizer_20Xcode" class="defterm p-block">'
            '<p><span class="defterm-term">stabilizer code: </span>'
            'A stabilizer code is nice.</p></div>'
            '<p>See <a href="#defterm-stabilizer_20Xcode" '
            'class="href-ref ref-topic">stabilizer code</a>.</p>'
        )

    def test_custom_suffix(self):
        environ = mk_flm_environ_custom_defterm(
            render_defterm_with_term_suffix=' --- '
        )
        result = render_doc(
            environ,
            r'\begin{defterm}{quantum code}'
            r'A quantum code encodes information.\end{defterm}'
        )
        self.assertEqual(
            result,
            '<div id="defterm-quantum_20Xcode" class="defterm p-block">'
            '<p><span class="defterm-term">quantum code \N{EM DASH} </span>'
            'A quantum code encodes information.</p></div>'
        )

    def test_render_without_term(self):
        environ = mk_flm_environ(render_defterm_with_term=False)
        result = render_doc(
            environ,
            r'\begin{defterm}{quantum code}'
            r'A quantum code encodes information.\end{defterm}'
        )
        self.assertEqual(
            result,
            '<div id="defterm-quantum_20Xcode" class="defterm p-block">'
            '<p>A quantum code encodes information.</p></div>'
        )


# ----------------------------------------------------------------
# Text Renderer
# ----------------------------------------------------------------

class TestFeatureDefTermText(unittest.TestCase):

    maxDiff = None

    def test_defterm_and_term_ref(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{defterm}{stabilizer code}'
            r'A code is called a \term{stabilizer code} if it works.'
            r'\end{defterm}'
            '\n\n'
            r'See \term{stabilizer code}.',
            fr=TextFragmentRenderer()
        )
        self.assertEqual(
            result,
            'stabilizer code: A code is called a stabilizer code'
            ' if it works.\n\nSee stabilizer code.'
        )

    def test_term_with_ref_term(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{defterm}{Markov chain}'
            r'A Markov chain is a process.\end{defterm}'
            '\n\n'
            r'These are \term[Markov chain]{Markov chains}.',
            fr=TextFragmentRenderer()
        )
        self.assertEqual(
            result,
            'Markov chain: A Markov chain is a process.'
            '\n\nThese are Markov chains.'
        )


# ----------------------------------------------------------------
# LaTeX Renderer
# ----------------------------------------------------------------

class TestFeatureDefTermLatex(unittest.TestCase):

    maxDiff = None

    def test_defterm_and_term_ref(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{defterm}{stabilizer code}'
            r'A code is called a \term{stabilizer code} if it works.'
            r'\end{defterm}'
            '\n\n'
            r'See \term{stabilizer code}.',
            fr=LatexFragmentRenderer()
        )
        self.assertEqual(
            result,
            r'\begin{flmDefterm}\phantomsection '
            r'\flmPinLabelHereWithDisplayText{x:defterm-stabilizer_20Xcode}'
            r'{<block>}'
            r'\flmDisplayTerm{stabilizer code: }'
            r'A code is called a \flmDisplayTerm{stabilizer code}'
            ' if it works.\n'
            r'\end{flmDefterm}%'
            '\n\n'
            r'See \hyperref[{x:defterm-stabilizer_20Xcode}]{stabilizer code}%'
            '\n.\n'
        )


# ----------------------------------------------------------------
# Markdown Renderer
# ----------------------------------------------------------------

class TestFeatureDefTermMarkdown(unittest.TestCase):

    maxDiff = None

    def test_defterm_and_term_ref(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{defterm}{stabilizer code}'
            r'A code is called a \term{stabilizer code} if it works.'
            r'\end{defterm}'
            '\n\n'
            r'See \term{stabilizer code}.',
            fr=MarkdownFragmentRenderer()
        )
        self.assertEqual(
            result,
            '<a name="defterm-stabilizer_20Xcode"></a>\n'
            'stabilizer code: A code is called a stabilizer code'
            ' if it works\\.\n\n'
            'See [stabilizer code](#defterm-stabilizer_20Xcode)\\.'
        )

    def test_term_with_ref_term(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{defterm}{Markov chain}'
            r'A Markov chain is a process.\end{defterm}'
            '\n\n'
            r'These are \term[Markov chain]{Markov chains}.',
            fr=MarkdownFragmentRenderer()
        )
        self.assertEqual(
            result,
            '<a name="defterm-Markov_20Xchain"></a>\n'
            'Markov chain: A Markov chain is a process\\.\n\n'
            'These are [Markov chains](#defterm-Markov_20Xchain)\\.'
        )


# ----------------------------------------------------------------
# Recomposer
# ----------------------------------------------------------------

class TestFeatureDefTermRecomposer(unittest.TestCase):

    maxDiff = None

    def test_basic_defterm(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(
            r'\begin{defterm}{stabilizer code}'
            r'A \term{stabilizer code} is nice.\end{defterm}'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{defterm}{stabilizer code}\label{ref1}'
            r'A \flmTerm{term}{ref1}{}{stabilizer code} is nice.'
            r'\end{defterm}'
        )

    def test_defterm_with_label(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(
            r'\begin{defterm}{stabilizer code}\label{topic:stab}'
            r'A stabilizer code.\end{defterm}'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{defterm}{stabilizer code}\label{ref1}\label{ref2}'
            r'A stabilizer code.\end{defterm}'
        )

    def test_term_macros(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(
            r'A \term{stabilizer code} and \term[Markov chain]{Markov chains}.'
        )
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'A \flmTerm{term}{ref1}{}{stabilizer code}'
            r' and \flmTerm{term}{ref2}{Markov chain}{Markov chains}.'
        )


if __name__ == '__main__':
    unittest.main()
