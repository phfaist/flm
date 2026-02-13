import unittest
# import json

import logging
logger = logging.getLogger(__name__)

from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features

from flm.flmrendercontext import FLMRenderContext
from flm.fragmentrenderer.html import HtmlFragmentRenderer


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


class TestFLMPureLatexRecomposerInit(unittest.TestCase):

    maxDiff = None

    def test_init_empty_options(self):
        r = FLMPureLatexRecomposer({})
        self.assertEqual(r.options, {})
        self.assertEqual(r.options_recomposer, {})
        self.assertIsNone(r.render_context)
        self.assertEqual(r.packages, {})
        self.assertEqual(r.safe_to_label, {})
        self.assertEqual(r.label_to_safe, {})
        self.assertEqual(r.safe_label_counter, 1)
        self.assertEqual(r.safe_ref_types, {})

    def test_init_none_options(self):
        r = FLMPureLatexRecomposer(None)
        self.assertEqual(r.options, {})
        self.assertEqual(r.options_recomposer, {})

    def test_init_preserves_top_level_options(self):
        opts = {
            'math': {'surround_display_math': ['\\before ', '\\after ']},
            'refs': {'emit_flm_macro': True},
            'graphics': {'set_max_width': '10cm'},
            'quote': {'setup_macro': None},
        }
        r = FLMPureLatexRecomposer(opts)
        self.assertEqual(r.options['math'],
                         {'surround_display_math': ['\\before ', '\\after ']})
        self.assertEqual(r.options['refs'], {'emit_flm_macro': True})
        self.assertEqual(r.options['graphics'], {'set_max_width': '10cm'})
        self.assertEqual(r.options['quote'], {'setup_macro': None})

    def test_init_recomposer_options(self):
        opts = {
            'recomposer': {
                'safe_label_ref_types': {
                    'ref': {'sec': True},
                },
            },
        }
        r = FLMPureLatexRecomposer(opts)
        self.assertEqual(r.options_recomposer, opts['recomposer'])
        self.assertEqual(r.safe_ref_types, {'ref': {'sec': True}})

    def test_init_render_context(self):
        render_context = FLMRenderContext(HtmlFragmentRenderer())
        r = FLMPureLatexRecomposer({'render_context': render_context})
        self.assertTrue(r.render_context is render_context)

    # -- get_options ---

    def test_get_options_existing_key(self):
        r = FLMPureLatexRecomposer({
            'quote': {'setup_macro': 'myMacro', 'keep_as_is': False},
        })
        result = r.get_options('quote')
        self.assertEqual(result, {'setup_macro': 'myMacro', 'keep_as_is': False})

    def test_get_options_missing_key(self):
        r = FLMPureLatexRecomposer({})
        result = r.get_options('nonexistent')
        self.assertEqual(result, {})

    def test_get_options_returns_copy(self):
        r = FLMPureLatexRecomposer({'quote': {'setup_macro': 'X'}})
        result = r.get_options('quote')
        result['setup_macro'] = 'CHANGED'
        self.assertEqual(r.get_options('quote'), {'setup_macro': 'X'})

    # -- ensure_latex_package ---

    def test_ensure_latex_package_basic(self):
        r = FLMPureLatexRecomposer({})
        r.ensure_latex_package('amsmath')
        self.assertEqual(r.packages, {'amsmath': {'options': None}})

    def test_ensure_latex_package_with_options(self):
        r = FLMPureLatexRecomposer({})
        r.ensure_latex_package('geometry', 'margin=1in')
        self.assertEqual(r.packages['geometry'], {'options': 'margin=1in'})

    def test_ensure_latex_package_idempotent(self):
        r = FLMPureLatexRecomposer({})
        r.ensure_latex_package('amsmath')
        r.ensure_latex_package('amsmath')
        self.assertEqual(r.packages, {'amsmath': {'options': None}})

    def test_ensure_latex_package_add_options_to_existing(self):
        r = FLMPureLatexRecomposer({})
        r.ensure_latex_package('hyperref')
        r.ensure_latex_package('hyperref', 'colorlinks')
        self.assertEqual(r.packages['hyperref'], {'options': 'colorlinks'})

    def test_ensure_latex_package_same_options_ok(self):
        r = FLMPureLatexRecomposer({})
        r.ensure_latex_package('geometry', 'margin=1in')
        r.ensure_latex_package('geometry', 'margin=1in')
        self.assertEqual(r.packages['geometry'], {'options': 'margin=1in'})

    def test_ensure_latex_package_conflicting_options_raises(self):
        r = FLMPureLatexRecomposer({})
        r.ensure_latex_package('geometry', 'margin=1in')
        with self.assertRaises(ValueError):
            r.ensure_latex_package('geometry', 'margin=2cm')

    # -- make_safe_label ---

    def test_make_safe_label_increments_counter(self):
        r = FLMPureLatexRecomposer({})
        l1 = r.make_safe_label('ref', 'eq', 'MyEq', None)
        l2 = r.make_safe_label('ref', 'fig', 'MyFig', None)
        self.assertEqual(l1['safe_label'], 'ref1')
        self.assertEqual(l2['safe_label'], 'ref2')
        self.assertEqual(r.safe_label_counter, 3)

    def test_make_safe_label_same_label_returns_same(self):
        r = FLMPureLatexRecomposer({})
        l1 = r.make_safe_label('ref', 'eq', 'MyEq', None)
        l2 = r.make_safe_label('ref', 'eq', 'MyEq', None)
        self.assertEqual(l1['safe_label'], l2['safe_label'])
        self.assertEqual(r.safe_label_counter, 2)

    def test_make_safe_label_raw_when_safe_ref_type(self):
        r = FLMPureLatexRecomposer({
            'recomposer': {
                'safe_label_ref_types': {
                    'ref': {'sec': True},
                },
            },
        })
        result = r.make_safe_label('ref', 'sec', 'intro', None)
        self.assertEqual(result['safe_label'], 'sec:intro')
        # Counter should not have been incremented
        self.assertEqual(r.safe_label_counter, 1)

    def test_make_safe_label_not_raw_when_false(self):
        r = FLMPureLatexRecomposer({
            'recomposer': {
                'safe_label_ref_types': {
                    'ref': {'eq': False},
                },
            },
        })
        result = r.make_safe_label('ref', 'eq', 'MyEq', None)
        self.assertEqual(result['safe_label'], 'ref1')

    def test_make_safe_label_tracks_bidirectional_mapping(self):
        r = FLMPureLatexRecomposer({})
        r.make_safe_label('ref', 'eq', 'MyEq', None)
        self.assertEqual(
            r.label_to_safe['ref']['eq:MyEq']['safe_label'],
            'ref1'
        )
        self.assertEqual(r.safe_to_label['ref']['ref1']['ref_type'], 'eq')
        self.assertEqual(r.safe_to_label['ref']['ref1']['ref_label'], 'MyEq')

    def test_make_safe_label_different_domains(self):
        r = FLMPureLatexRecomposer({})
        l1 = r.make_safe_label('ref', 'eq', 'eq1', None)
        l2 = r.make_safe_label('cite', 'arxiv', 'paper1', None)
        self.assertEqual(l1['safe_label'], 'ref1')
        self.assertEqual(l2['safe_label'], 'cite2')

    # -- escape_chars ---

    def test_escape_chars_all_specials(self):
        env = mk_flm_environ()
        r = FLMPureLatexRecomposer({})
        ps = env.make_parsing_state(is_block_level=False)
        result = r.escape_chars('$&#^_%', ps)
        self.assertEqual(result, r'\$\&\#\^\_\%')


class TestFLMPureLatexRecomposer(unittest.TestCase):

    maxDiff = None

    def test_simple(self):
        
        env = mk_flm_environ()

        s = r'''\textbf{Hello}, \emph{world}: \[ A + B = C. \]'''

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            s
        )


    def test_simple_math(self):
        
        env = mk_flm_environ()

        s = r'''Single-char macro.\@;
\begin{align}
  x &= 0; \\[1ex]
  z &= 1.
\end{align}
'''

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            s
        )


    def test_simple_math_1a(self):
        
        env = mk_flm_environ()

        s = r'''Single-char macro.\@;
\begin{align*}
  x &= 0; \\[1ex]
  z &= 1.
\end{align*}
'''

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            s
        )

    def test_simple_math_1b(self):
        
        env = mk_flm_environ()

        s = r'''\begin{align}ABC\end{align}'''

        frag = env.make_fragment( s, what='example text fragment' )

        recomposer = FLMPureLatexRecomposer({
            "math": {
                "surround_display_math": [r'\mathbefore ', r'\mathafter '],
            },
        })

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r'\mathbefore ' + s + r'\mathafter '
        )

    def test_simple_math_1c(self):
        
        env = mk_flm_environ()

        s = r'''\begin{align*} ABC\end{align*}'''

        frag = env.make_fragment( s, what='example text fragment' )

        recomposer = FLMPureLatexRecomposer({
            "math": {
                "emit_flm_math_environment_macro": True,
            },
        })

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r'\flmMathEnvironment{align*}{}{ ABC}'
        )

    def test_simple_math_2(self):
        
        env = mk_flm_environ()

        s = r'''
\begin{align}
   x &= 0;
   \label{eq:one}\\
   z &= 1;
   \label{eq:two}
\end{align}
'''.strip()

        result_expected = r'''
\begin{align}
   x &= 0;\label{ref1}\\
   z &= 1;\label{ref2}
\end{align}
'''.strip()

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        print("*** s=")
        print(s)
        print("*** result=")
        print(result['latex'])
        print("*** result_expected=")
        print(result_expected)

        self.assertEqual(
            result["latex"],
            result_expected
        )

    def test_simple_math_3(self):
        
        env = mk_flm_environ()

        s = r'''
\begin{align}
   \label{eq:MyEquation}
   x &= 0.
\end{align}
'''.strip()

        result_expected = r'''
\begin{align}
   x &= 0.\label{ref1}
\end{align}
'''.strip()

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        print("*** s=")
        print(s)
        print("*** result=")
        print(result['latex'])
        print("*** result_expected=")
        print(result_expected)

        self.assertEqual(
            result["latex"],
            result_expected
        )



    def test_escape_chars(self):

        env = mk_flm_environ()

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.escape_chars(
            r"""
You & me want _lots_ of $$$, right ^^
""",
            env.make_parsing_state(is_block_level=False)
        )

        self.assertEqual(
            result,
            r"""
You \& me want \_lots\_ of \$\$\$, right \^\^
"""
        )


    def test_environment_with_newlines(self):

        env = mk_flm_environ()

        s = r'''
\begin{align}A + B = C\ .\end{align}
'''

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        result_latex_correct = s

        self.assertEqual(
            result["latex"],
            result_latex_correct
        )



    def test_safe_labels(self):

        env = mk_flm_environ()

        s = r'''
\section{Introduction}\label{sec:intro}

Anticipating equation~\eqref{eq:MySecondFancyEquation}, along with \ref{sec:mySubSection}.

An equation:
\begin{align}A + B = C\ .\label{eq:MyFancyEquation}\end{align}

Another equation:
\begin{align}
\label{eq:MySecondFancyEquation}
  A + B = C\ .
\end{align}

Test reference to~\eqref{eq:MyFancyEquation}.

\subsection{Some subsection}\label{sec:mySubSection}

More text.
'''.strip()

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({
            'recomposer': {
                'safe_label_ref_types': {
                    'ref': {
                        'sec': True,
                    },
                },
            }
        })

        result = recomposer.recompose_pure_latex(frag.nodes)

        result_latex_correct = s \
            .replace(
                r"""\begin{align}
\label{eq:MySecondFancyEquation}
  A + B = C\ .
\end{align}""",
                r"""\begin{align}
  A + B = C\ .\label{ref1}
\end{align}""",
            ) \
            .replace(
                'eq:MySecondFancyEquation', 'ref1'
            ) \
            .replace( 'eq:MyFancyEquation', 'ref2' ) \
            .replace( r'\ref{sec:mySubSection}',
                      r'\NoCaseChange{\protect\cref{sec:mySubSection}}' )

        self.assertEqual(
            result["latex"],
            result_latex_correct
        )


    def test_refs_1(self):

        env = mk_flm_environ()

        s = r'''\ref{figure:one}'''

        frag = env.make_fragment( s, what='example text fragment' )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r"""\NoCaseChange{\protect\cref{ref1}}"""
        )

    def test_refs_1b(self):

        env = mk_flm_environ()

        s = r'''\hyperref[figure:one]{Figure One}'''

        frag = env.make_fragment( s, what='example text fragment' )

        recomposer = FLMPureLatexRecomposer({})

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r"""\NoCaseChange{\protect\hyperref[{ref1}]{Figure One}}"""
        )

    def test_refs_2a(self):

        env = mk_flm_environ()

        s = r'''\ref{figure:one} | \hyperref[figure:two]{Figure Two}'''

        frag = env.make_fragment( s, what='example text fragment' )

        recomposer = FLMPureLatexRecomposer({
            'refs': {
                'protect_surround': ('', ''),
            }
        })

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r"""\cref{ref1} | \hyperref[{ref2}]{Figure Two}"""
        )

    def test_refs_3(self):

        env = mk_flm_environ()

        s = r'''\ref{figure:one} | \hyperref[figure:two]{Figure Two}'''

        frag = env.make_fragment( s, what='example text fragment' )

        recomposer = FLMPureLatexRecomposer({
            'refs': {
                'emit_flm_macro': True,
            }
        })

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r"""\flmRefsCref{ref1} | \flmRefsHyperref{ref2}{Figure Two}"""
        )




    def test_float(self):

        env = mk_flm_environ()

        s = r'''\begin{figure}
\includegraphics{myfigure}
\caption{My favorite figure}
\label{figure:myfig}
\end{figure}'''

        frag = env.make_fragment(
            s,
            what='example text fragment'
        )

        recomposer = FLMPureLatexRecomposer({
            'graphics': {
                'set_max_width': '10cm'
            }
        })

        result = recomposer.recompose_pure_latex(frag.nodes)

        self.assertEqual(
            result["latex"],
            r"""\begin{flmFloat}{figure}{NumCap}\includegraphics[max width=10cm]{myfigure}\caption{My favorite figure}\label{ref1}\end{flmFloat}"""
        )





if __name__ == '__main__':
    unittest.main()
