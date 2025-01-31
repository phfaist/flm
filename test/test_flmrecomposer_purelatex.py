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


    def test_simple_macro(self):
        
        env = mk_flm_environ()

        s = r'''Single-char macro.\@;
\begin{align}
  x &= 0; \\[1ex]
  z &= 1.
\end{align}'''

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

        self.assertEqual(
            result["latex"],
            s
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
\begin{align}A + B = C\ .\label{eq:MySecondFancyEquation}\end{align}

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
            .replace( 'eq:MySecondFancyEquation', 'ref1' ) \
            .replace( 'eq:MyFancyEquation', 'ref2' ) \
            .replace( r'\ref{sec:mySubSection}',
                      r'\NoCaseChange{\protect\cref{sec:mySubSection}}' )

        self.assertEqual(
            result["latex"],
            result_latex_correct
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
