import unittest

from pylatexenc.latexnodes import LatexWalkerParseError
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

        print(frag1.nodes.llm_blocks)

        self.assertTrue(False)

        

