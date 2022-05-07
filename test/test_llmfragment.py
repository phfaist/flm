import unittest

from llm.llmfragment import LLMFragment
from llm.llmstd import LLMStandardEnvironment

import pylatexenc.latexnodes.nodes as latexnodes_nodes
from pylatexenc.latexnodes import LatexWalkerParseError


class TestLLMFragment(unittest.TestCase):

    def test_simple(self):

        env = LLMStandardEnvironment()

        s = r'Hello, this is a LLM fragment of text with only simple characters.'

        frag = LLMFragment(
            s,
            env,
            what='example text fragment'
        )

        self.assertEqual(len(frag.nodes), 1)
        self.assertTrue(frag.nodes[0].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(frag.nodes[0].chars, s)

    def test_failure(self):

        env = LLMStandardEnvironment()

        s = r'Hello, this is a LLM fragment of text with an \UnknownMacroThatRaisesAnError.'

        with self.assertRaises(LatexWalkerParseError):
            frag = LLMFragment(
                s,
                env,
                what='example text fragment'
            )

    def test_get_first_paragraph(self):

        env = LLMStandardEnvironment()

        s = r'''
Hello, this is a \textbf{LLM} fragment
of text with multiple paragraphs.

Here is the \textit{second}
paragraph. It is separated from the
first with two newline characters.
'''.strip()

        frag_full = LLMFragment(
            s,
            env,
            what='example text fragment'
        )

        frag = frag_full.get_first_paragraph()

        self.assertTrue(isinstance(frag, LLMFragment))
        self.assertEqual(len(frag.nodes), 3)
        self.assertTrue(frag.nodes[0].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(frag.nodes[0].chars, 'Hello, this is a ')
        self.assertEqual(frag.nodes[2].chars, ' fragment\nof text with multiple paragraphs.')




if __name__ == '__main__':
    unittest.main()
