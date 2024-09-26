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
