import unittest
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerParseError
from pylatexenc.latexnodes.nodes import *
from pylatexenc.macrospec import LatexContextDb

from llm import llmenvironment
from llm.llmspecinfo import (
    LLMMacroSpec, LLMEnvironmentSpec, LLMSpecialsSpec,
    TextFormat, ParagraphBreak, MathEnvironment,
)
from llm.enumeration import Enumeration


def make_simple_context():
    latex_context = LatexContextDb()
    latex_context.add_context_category(
        'my-stuff',
        macros=[
            LLMMacroSpec('textbf', '{',
                         llm_specinfo=TextFormat(['textbf'])),
            LLMMacroSpec('item', '', llm_specinfo='- ')
        ],
        environments=[
            LLMEnvironmentSpec('equation', '',
                               llm_specinfo=MathEnvironment(),
                               is_math_mode=True,),
            LLMEnvironmentSpec('enumerate', '',
                               llm_specinfo=Enumeration()),
        ],
        specials=[
            LLMSpecialsSpec(
                '\n\n',
                llm_specinfo=ParagraphBreak()
            ),
        ],
    )
    return latex_context



class TestBlocksBuilder(unittest.TestCase):

    def test_builds_blocks_single_para(self):
        n1 = LatexCharsNode(chars='\n  Hello  \tworld. \n ')
        n2 = LatexMacroNode(macroname='somemacro')
        n2.llm_is_block_level = False

        bb = llmenvironment.BlocksBuilder([ n1, n2 ])

        blocks = bb.build_blocks()

        logger.debug("blocks = %r", blocks)

        self.assertEqual(
            blocks,
            [
                LatexNodeList([n1, n2]),
            ]
        )

    def test_builds_blocks_multiple_paras(self):
        n1 = LatexCharsNode(chars='\n  Hello  \tworld. \n ')
        n2 = LatexMacroNode(macroname='somemacro')
        n2.llm_is_block_level = False
        n3 = LatexCharsNode(chars='.  That\'s it ')
        n4 = LatexSpecialsNode(specials_chars='\n\n')
        n4.llm_is_block_level = True
        n4.llm_is_paragraph_break_marker = True
        n5 = LatexCharsNode(chars='More  text content.  ')

        bb = llmenvironment.BlocksBuilder([ n1, n2, n3, n4, n5 ])

        blocks = bb.build_blocks()

        logger.debug("blocks = %r", blocks)

        self.assertEqual(
            blocks,
            [
                LatexNodeList([n1, n2, n3,]),
                LatexNodeList([n5,]),
            ]
        )

    def test_builds_blocks_multiple_paras_with_block_elements(self):
        n1 = LatexCharsNode(chars='\n  Hello  \tworld. \n ')
        n2 = LatexMacroNode(macroname='somemacro')
        n2.llm_is_block_level = False
        n3 = LatexCharsNode(chars='.  That\'s it ')
        n4 = LatexEnvironmentNode(environmentname='enumerate', nodelist=None)
        n4.llm_is_block_level = True
        n5 = LatexCharsNode(chars='More  text content.  ')

        bb = llmenvironment.BlocksBuilder([ n1, n2, n3, n4, n5 ])

        blocks = bb.build_blocks()

        logger.debug("blocks = %r", blocks)

        self.assertEqual(
            blocks,
            [
                LatexNodeList([n1, n2, n3,]),
                n4,
                LatexNodeList([n5,]),
            ]
        )


    def test_handles_white_space_correctly(self):
        n1 = LatexCharsNode(chars='\n  Hello  \tworld. \n ')
        n2 = LatexMacroNode(macroname='somemacro')
        n2.llm_is_block_level = False
        n3 = LatexCharsNode(chars='.  That\'s it!  ')
        n4 = LatexEnvironmentNode(environmentname='enumerate', nodelist=None)
        n4.llm_is_block_level = True
        n5 = LatexCharsNode(chars='\r\tMore  text content.  ')

        bb = llmenvironment.BlocksBuilder([ n1, n2, n3, n4, n5 ])

        blocks = bb.build_blocks()

        self.assertEqual(n1.llm_chars_value, 'Hello world. ')
        self.assertEqual(n3.llm_chars_value, '. That\'s it!')
        self.assertEqual(n5.llm_chars_value, 'More text content.')


class TestLLMEnvironment(unittest.TestCase):

    def test_blocks_paragraphs(self):

        latex_context = make_simple_context()
        environ = llmenvironment.LLMEnvironment(
            latex_context=latex_context,
            parsing_state=llmenvironment.LLMParsingState(),
            features=[],
        )

        frag1 = environ.make_fragment(r"""
     

Test
some
input \textbf{string} here. Does this
\begin{equation}
 a = b
\end{equation}
work?

\textbf{A   new} paragraph is started    here.

\begin{enumerate}
\item one
\item two
\end{enumerate}

      .
""")

        self.assertEqual(len(frag1.nodes.llm_blocks), 4)

        

