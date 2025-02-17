import unittest
import json

import logging
logger = logging.getLogger(__name__)

from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


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
