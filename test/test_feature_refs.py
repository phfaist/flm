import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.feature.refs import (
    RefInstance
)


def mk_flm_environ(**quote_kwargs):
    features = standard_features()
    return make_standard_environment(features)


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result



class TestFeatureRefsSimple(unittest.TestCase):

    maxDiff = None

    def test_simple1(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:a}
\end{align}

See \ref{eq:a}.

See \ref[S]{eq:a}.

See~\eqref{eq:a}.
""")
        self.assertEqual(
            result, r"""
<p><span id="equation-1" class="display-math env-align">\begin{align}
  x=1
  \label{eq:a}
\tag*{(1)}\end{align}</span></p>
<p>See <a href="#equation-1" class="href-ref ref-eq">Eq.&nbsp;(1)</a>.</p>
<p>See <a href="#equation-1" class="href-ref ref-eq">Equation&nbsp;(1)</a>.</p>
<p>See&nbsp;<a href="#equation-1" class="href-ref ref-eq">(1)</a>.</p>
""".strip())

