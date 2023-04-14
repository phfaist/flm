import unittest
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes.nodes import *
from pylatexenc.macrospec import LatexContextDb

from flm import flmenvironment
from flm.flmspecinfo import (
    ConstantValueMacro, TextFormatMacro, ParagraphBreakSpecials,
)
from flm.feature.math import MathEnvironment
from flm.feature.enumeration import Enumeration


def make_simple_context():
    latex_context = LatexContextDb()
    latex_context.add_context_category(
        'my-stuff',
        macros=[
            TextFormatMacro('textbf', text_formats=['textbf']),
            ConstantValueMacro('item', value='- ')
        ],
        environments=[
            MathEnvironment('equation',),
            Enumeration('enumerate'),
        ],
        specials=[
            ParagraphBreakSpecials(
                '\n\n',
            ),
        ],
    )
    return latex_context



class TestBlocksBuilder(unittest.TestCase):

    def test_builds_blocks_single_para(self):
        n1 = LatexCharsNode(chars='\n  Hello  \tworld. \n ')
        n2 = LatexMacroNode(macroname='somemacro')
        n2.flm_is_block_level = False

        bb = flmenvironment.BlocksBuilder(LatexNodeList([ n1, n2 ]))

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
        n2.flm_is_block_level = False
        n3 = LatexCharsNode(chars='.  That\'s it ')
        n4 = LatexSpecialsNode(specials_chars='\n\n')
        n4.flm_is_block_level = True
        n4.flm_is_paragraph_break_marker = True
        n5 = LatexCharsNode(chars='More  text content.  ')

        bb = flmenvironment.BlocksBuilder(LatexNodeList([ n1, n2, n3, n4, n5 ]))

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
        n2.flm_is_block_level = False
        n3 = LatexCharsNode(chars='.  That\'s it ')
        n4 = LatexEnvironmentNode(environmentname='enumerate', nodelist=None)
        n4.flm_is_block_level = True
        n5 = LatexCharsNode(chars='More  text content.  ')

        bb = flmenvironment.BlocksBuilder(LatexNodeList([ n1, n2, n3, n4, n5 ]))

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
        n2.flm_is_block_level = False
        n3 = LatexCharsNode(chars='.  That\'s it!  ')
        n4 = LatexEnvironmentNode(environmentname='enumerate', nodelist=None)
        n4.flm_is_block_level = True
        n5 = LatexCharsNode(chars='\r\tMore  text content.  ')

        bb = flmenvironment.BlocksBuilder(LatexNodeList([ n1, n2, n3, n4, n5 ]))

        blocks = bb.build_blocks()

        self.assertEqual(n1.flm_chars_value, 'Hello world. ')
        self.assertEqual(n3.flm_chars_value, '. That\'s it!')
        self.assertEqual(n5.flm_chars_value, 'More text content.')


class TestFLMEnvironment(unittest.TestCase):

    def test_blocks_paragraphs_correct_number(self):

        latex_context = make_simple_context()
        environ = flmenvironment.FLMEnvironment(
            latex_context=latex_context,
            parsing_state=flmenvironment.FLMParsingState(),
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

        self.assertEqual(len(frag1.nodes.flm_blocks), 4)

        



class Feat1:
    feature_name = 'feature 1'
    feature_dependencies = None
    feature_optional_dependencies = None

class Feat2:
    feature_name = 'feature 2'
    feature_dependencies = ['feature 1']
    feature_optional_dependencies = []

class Feat3:
    feature_name = 'feature 3'
    feature_dependencies = []
    feature_optional_dependencies = None

class Feat4:
    feature_name = 'feature 4'
    feature_dependencies = ['feature 3']
    feature_optional_dependencies = ['feature 1']

class Feat5:
    feature_name = 'feature 5'
    feature_dependencies = ['feature 3', 'feature 2']
    feature_optional_dependencies = ['feature 1', 'feature 4']
    

class FeatCycle1:
    feature_name = 'feature c1'
    feature_dependencies = ['feature 2', 'feature c2']
    feature_optional_dependencies = None

class FeatCycle2:
    feature_name = 'feature c2'
    feature_dependencies = ['feature 2', 'feature c3']
    feature_optional_dependencies = None

class FeatCycle3:
    feature_name = 'feature c2'
    feature_dependencies = ['feature c2', 'feature c1']
    feature_optional_dependencies = None

    

class Test_features_sorted_by_dependencies(unittest.TestCase):

    def test_get_sorted(self):

        sorted_features, features_by_name = flmenvironment.features_sorted_by_dependencies([
            Feat3(),
            Feat2(),
            Feat5(),
            Feat1(),
            Feat4(),
        ])

        self.assertEqual(
            [ f.feature_name for f in sorted_features ],
            [ 'feature 1', 'feature 3', 'feature 2', 'feature 4', 'feature 5' ]
        )

    def test_get_sorted_2(self):

        sorted_features, features_by_name = flmenvironment.features_sorted_by_dependencies([
            Feat1(),
            Feat2(),
            Feat3(),
            Feat4(),
            Feat5(),
        ])

        self.assertEqual(
            [ f.feature_name for f in sorted_features ],
            [ 'feature 1', 'feature 3', 'feature 2', 'feature 4', 'feature 5' ]
        )


    def test_get_sorted_woptional(self):

        sorted_features, features_by_name = flmenvironment.features_sorted_by_dependencies([
            Feat3(),
            Feat4(),
        ])

        self.assertEqual(
            [ f.feature_name for f in sorted_features ],
            [ 'feature 3', 'feature 4' ]
        )


    def test_get_sorted_woptional_2(self):

        sorted_features, features_by_name = flmenvironment.features_sorted_by_dependencies([
            Feat4(),
            Feat3(),
        ])

        self.assertEqual(
            [ f.feature_name for f in sorted_features ],
            [ 'feature 3', 'feature 4' ]
        )


    def test_get_sorted_wcycles(self):

        with self.assertRaises(ValueError):
            sorted_features, features_by_name = flmenvironment.features_sorted_by_dependencies([
                Feat1(),
                Feat2(),
                Feat3(),
                Feat4(),
                Feat5(),
                FeatCycle1(),
                FeatCycle2(),
                FeatCycle3(),
            ])





if __name__ == '__main__':
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
