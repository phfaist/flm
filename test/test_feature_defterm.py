import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer

from flm.feature import defterm as feature_defterm


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class TestFeatureTheorems(unittest.TestCase):

    maxDiff = None

    def test_simple_1(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"""
\begin{defterm}{stabilizer code}A code is called a \term{stabilizer code} if it 
is defined as the subspace stabilizerd by a Pauli stabilizer group.\end{defterm}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div id="defterm-stabilizer_20Xcode" class="defterm">
<p><span class="defterm-term">stabilizer code: </span>A code is called a <span class="defterm-term term-in-defining-defterm">stabilizer code</span> if it is defined as the subspace stabilizerd by a Pauli stabilizer group.</p>
</div>
""".strip() .replace('\n', '')
        )


    def test_simple_2(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"""
Ref: \term{stabilizer code}.

\begin{defterm}{stabilizer code}A code is called a \term{stabilizer code} if it 
is defined as the subspace stabilizerd by a Pauli stabilizer group.\end{defterm}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer(config=dict(html_blocks_joiner=""))
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p>Ref: <a href="#defterm-stabilizer_20Xcode" class="href-term">stabilizer code</a>.</p>
<div id="defterm-stabilizer_20Xcode" class="defterm">
<p><span class="defterm-term">stabilizer code: </span>A code is called a <span class="defterm-term term-in-defining-defterm">stabilizer code</span> if it is defined as the subspace stabilizerd by a Pauli stabilizer group.</p>
</div>
""".strip() .replace('\n', '')
        )


    def test_simple_extfrominner(self):

        environ = mk_flm_environ(render_defterm_with_term=False)

        frag1 = environ.make_fragment(r"""
\begin{defterm}{a}Definition of \term{a}, related to \term{b}.\end{defterm}
\begin{defterm}{b}Definition of \term{b}, related to \term{a}.\end{defterm}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer(config=dict(html_blocks_joiner=""))
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div id="defterm-a" class="defterm">
<p>Definition of <span class="defterm-term term-in-defining-defterm">a</span>, related to <a href="#defterm-b" class="href-term">b</a>.</p>
</div>
<div id="defterm-b" class="defterm">
<p>Definition of <span class="defterm-term term-in-defining-defterm">b</span>, related to <a href="#defterm-a" class="href-term">a</a>.</p>
</div>
""".strip() .replace('\n', '')
        )





if __name__ == '__main__':
    unittest.main()
