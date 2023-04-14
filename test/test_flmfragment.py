import unittest

from flm.flmfragment import FLMFragment
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features

import pylatexenc.latexnodes.nodes as latexnodes_nodes
from pylatexenc.latexnodes import LatexWalkerParseError



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)




class TestFLMFragment(unittest.TestCase):

    def test_simple(self):

        env = mk_flm_environ()

        s = r'Hello, this is a FLM fragment of text with only simple characters.'

        frag = FLMFragment(
            s,
            env,
            what='example text fragment'
        )

        self.assertEqual(len(frag.nodes), 1)
        self.assertTrue(frag.nodes[0].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(frag.nodes[0].chars, s)

    def test_failure(self):

        env = mk_flm_environ()

        s = r'Hello, this is a FLM fragment of text with an \UnknownMacroThatRaisesAnError.'

        with self.assertRaises(LatexWalkerParseError):
            frag = FLMFragment(
                s,
                env,
                what='example text fragment'
            )

    def test_get_first_paragraph(self):

        env = mk_flm_environ()

        s = r'''
Hello, this is a \textbf{FLM} fragment
of text with multiple paragraphs.

Here is the \textit{second}
paragraph. It is separated from the
first with two newline characters.
'''.strip()

        frag_full = FLMFragment(
            s,
            env,
            what='example text fragment'
        )

        frag = frag_full.get_first_paragraph()

        self.assertTrue(isinstance(frag, FLMFragment))
        self.assertEqual(len(frag.nodes), 3)
        self.assertTrue(frag.nodes[0].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(frag.nodes[0].chars, 'Hello, this is a ')
        self.assertEqual(frag.nodes[2].chars, ' fragment\nof text with multiple paragraphs.')

    def test_whitespace_stripped(self):

        env = mk_flm_environ()

        s = r'''
  Hello, this has
whitespace   

'''

        frag_full = FLMFragment(
            s,
            env,
            what='example text fragment'
        )

        frag = frag_full.whitespace_stripped()

        self.assertTrue(isinstance(frag, FLMFragment))
        self.assertEqual(len(frag.nodes), 1)
        self.assertTrue(frag.nodes[0].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(frag.nodes[0].chars, 'Hello, this has\nwhitespace')


    def test_truncate_content(self):

        env = mk_flm_environ()

        s = r'''
Here is \emph{an example of a \textbf{FLM} fragment}
of text with multiple paragraphs.

\begin{align}
    A = \int_0^\infty f(x)\,\Bigl[ \frac{a}{b} - \frac12\bigl(z_1 - z_2\bigr) \Bigr]
        + z_0[S, T] + t_0
\end{align}

Here is another \textit{paragraph}.
 It is separated from the previous one
with two newline characters.
'''

        frag_full = FLMFragment(
            s,
            env,
            what='example text fragment'
        )

        frag_1 = frag_full.truncate_to(chars=6)
        self.assertEqual(len(frag_1.nodes), 1)
        self.assertEqual(frag_1.nodes[0].chars, '\nHere …')

        frag_1 = frag_full.truncate_to(chars=16)
        self.assertEqual(len(frag_1.nodes), 2)
        self.assertEqual(frag_1.nodes[0].chars, '\nHere is ')
        self.assertEqual(frag_1.nodes[1].macroname, 'emph')
        self.assertEqual(frag_1.nodes[1].nodeargd.argnlist[0].nodelist[0].chars, 'an …')

        frag_1 = frag_full.truncate_to(chars=16, truncation_marker=' ...')
        self.assertEqual(len(frag_1.nodes), 2)
        self.assertEqual(frag_1.nodes[0].chars, '\nHere is ')
        self.assertEqual(frag_1.nodes[1].macroname, 'emph')
        self.assertEqual(frag_1.nodes[1].nodeargd.argnlist[0].nodelist[0].chars, 'an ...')




if __name__ == '__main__':
    unittest.main()
