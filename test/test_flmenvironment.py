import unittest
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes.nodes import *
from pylatexenc.latexnodes import LatexWalkerParseError
from pylatexenc.macrospec import LatexContextDb

from flm import flmenvironment
from flm.flmenvironment import (
    FLMParsingState,
    FLMParsingStateDeltaSetBlockLevel,
    FLMArgumentSpec,
    NodesFinalizer,
    FLMEnvironment,
    standard_parsing_state,
    make_standard_environment,
    features_ensure_dependencies_are_met,
    standard_environment_get_located_error_message,
)
from flm.flmspecinfo import (
    ConstantValueMacro, TextFormatMacro, ParagraphBreakSpecials,
)
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
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

        self.assertEqual(len(blocks), 3)

        nn1 = blocks[0].nodelist[0]
        nn3 = blocks[0].nodelist[2]
        nn5 = blocks[2].nodelist[0]

        self.assertEqual(nn1.flm_chars_value, 'Hello world. ')
        self.assertEqual(nn3.flm_chars_value, '. That\'s it!')
        self.assertEqual(nn5.flm_chars_value, 'More text content.')


    def test_handles_white_space_correctly_disabledsimplifywhitespace(self):
        n1 = LatexCharsNode(chars='\n  Hello  \tworld. \n ')
        n2 = LatexMacroNode(macroname='somemacro')
        n2.flm_is_block_level = False
        n3 = LatexCharsNode(chars=' .  That\'s it!  ')
        n4 = LatexEnvironmentNode(environmentname='enumerate', nodelist=None)
        n4.flm_is_block_level = True
        n5 = LatexCharsNode(chars='\r\tMore  text content.  ')

        bb = flmenvironment.BlocksBuilder(
            LatexNodeList([ n1, n2, n3, n4, n5 ]),
            simplify_whitespace=False
        )

        blocks = bb.build_blocks()

        self.assertEqual(len(blocks), 3)

        nn1 = blocks[0].nodelist[0]
        nn3 = blocks[0].nodelist[2]
        nn5 = blocks[2].nodelist[0]

        self.assertEqual(nn1.flm_chars_value, 'Hello  \tworld. \n ')
        self.assertEqual(nn3.flm_chars_value, ' .  That\'s it!')
        self.assertEqual(nn5.flm_chars_value, 'More  text content.')


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

        

    def test_chars_options_no_auto(self):

        latex_context = make_simple_context()
        environ = flmenvironment.FLMEnvironment(
            latex_context=latex_context,
            parsing_state=flmenvironment.FLMParsingState(),
            features=[],
            text_processing_options={'auto': False},
        )

        frag1 = environ.make_fragment(r"""
Hello 'world'. Does this "work" or does it ``function?''

It does---doesn't it? Maybe...
""")

        self.assertEqual(frag1.nodes.flm_blocks[0][0].flm_chars_value, r"""
Hello 'world'. Does this "work" or does it ``function?''
""".strip())
        self.assertEqual(frag1.nodes.flm_blocks[1][0].flm_chars_value, r"""
It does---doesn't it? Maybe...
""".strip())

    def test_autounichars(self):
        from flm.flmenvironment import _autounichars
        self.assertEqual(
            _autounichars.convert_auto_quotes(
                '''Hello 'world'. Does this "work" or does it "function?"'''
            ),
            """Hello ‘world’. Does this “work” or does it “function?”"""
        )


    def test_chars_options_auto(self):

        latex_context = make_simple_context()
        environ = flmenvironment.FLMEnvironment(
            latex_context=latex_context,
            parsing_state=flmenvironment.FLMParsingState(),
            features=[],
            text_processing_options={'auto': True},
        )

        frag1 = environ.make_fragment(r"""
Hello 'world'. Does this "work" or does it ``function?''

It does---doesn't it? Maybe...
""")

        self.assertEqual(frag1.nodes.flm_blocks[0][0].flm_chars_value, r"""
Hello ‘world’. Does this “work” or does it “function?”
""".strip())
        self.assertEqual(frag1.nodes.flm_blocks[1][0].flm_chars_value, r"""
It does—doesn’t it? Maybe…
""".strip())

    def test_chars_options_ligonly(self):

        latex_context = make_simple_context()
        environ = flmenvironment.FLMEnvironment(
            latex_context=latex_context,
            parsing_state=flmenvironment.FLMParsingState(),
            features=[],
            text_processing_options={
                'auto': False,
                'ligature_unicode_quotes': True,
                'ligature_unicode_dashes': True,
                'ligature_unicode_ellipses': True,
            },
        )

        frag1 = environ.make_fragment(r"""
Hello `world'. Does this "work" or does it ``function?''

It does---doesn't it? Maybe...
""")

        self.assertEqual(frag1.nodes.flm_blocks[0][0].flm_chars_value, r"""
Hello ‘world’. Does this "work" or does it “function?”
""".strip())
        self.assertEqual(frag1.nodes.flm_blocks[1][0].flm_chars_value, r"""
It does—doesn’t it? Maybe…
""".strip())







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


    def test_duplicate_feature_raises(self):
        with self.assertRaises(ValueError):
            flmenvironment.features_sorted_by_dependencies([Feat1(), Feat1()])

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





def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


# --- FLMParsingState ---

class TestFLMParsingState(unittest.TestCase):

    def test_is_block_level_true(self):
        ps = FLMParsingState(is_block_level=True)
        self.assertTrue(ps.is_block_level)

    def test_is_block_level_default_none(self):
        ps = FLMParsingState()
        self.assertIsNone(ps.is_block_level)

    def test_is_block_level_false(self):
        ps = FLMParsingState(is_block_level=False)
        self.assertFalse(ps.is_block_level)


# --- FLMParsingStateDeltaSetBlockLevel ---

class TestFLMParsingStateDeltaSetBlockLevel(unittest.TestCase):

    def test_set_block_level_true(self):
        delta = FLMParsingStateDeltaSetBlockLevel(is_block_level=True)
        self.assertTrue(delta.is_block_level)
        self.assertEqual(delta.set_attributes, {'is_block_level': True})

    def test_set_block_level_false(self):
        delta = FLMParsingStateDeltaSetBlockLevel(is_block_level=False)
        self.assertFalse(delta.is_block_level)

    def test_apply_to_parsing_state(self):
        ps = FLMParsingState(is_block_level=None, latex_context=None)
        delta = FLMParsingStateDeltaSetBlockLevel(is_block_level=True)
        ps2 = delta.get_updated_parsing_state(ps, latex_walker=None)
        self.assertTrue(ps2.is_block_level)


# --- FLMArgumentSpec ---

class TestFLMArgumentSpec(unittest.TestCase):

    def test_basic(self):
        arg = FLMArgumentSpec('{', 'text')
        self.assertEqual(arg.argname, 'text')
        self.assertEqual(arg.parser, '{')
        self.assertFalse(arg.parsing_state_delta.is_block_level)

    def test_block_level_true(self):
        arg = FLMArgumentSpec('{', 'body', is_block_level=True)
        self.assertTrue(arg.parsing_state_delta.is_block_level)

    def test_block_level_none_no_delta(self):
        arg = FLMArgumentSpec('{', 'content', is_block_level=None)
        self.assertIsNone(arg.parsing_state_delta)

    def test_incompatible_delta_raises(self):
        delta = FLMParsingStateDeltaSetBlockLevel(is_block_level=True)
        with self.assertRaises(ValueError):
            FLMArgumentSpec('{', 'x', is_block_level=False, parsing_state_delta=delta)


# --- standard_parsing_state ---

class TestStandardParsingState(unittest.TestCase):

    def test_defaults(self):
        sps = standard_parsing_state()
        self.assertIsNone(sps.is_block_level)
        self.assertTrue(sps.enable_comments)
        self.assertEqual(sps.comment_start, '%%')
        self.assertEqual(sps.forbidden_characters, '$%')
        self.assertEqual(sps.latex_inline_math_delimiters, [('\\(', '\\)')])
        self.assertEqual(sps.latex_display_math_delimiters, [('\\[', '\\]')])

    def test_dollar_inline_math_mode(self):
        sps = standard_parsing_state(dollar_inline_math_mode=True)
        self.assertEqual(sps.forbidden_characters, '%')
        self.assertEqual(sps.latex_inline_math_delimiters, [('\\(', '\\)'), ('$', '$')])

    def test_comments_disabled(self):
        sps = standard_parsing_state(enable_comments=False)
        self.assertFalse(sps.enable_comments)
        self.assertEqual(sps.forbidden_characters, '$%')

    def test_force_block_level(self):
        sps = standard_parsing_state(force_block_level=True)
        self.assertTrue(sps.is_block_level)

    def test_extra_forbidden_characters(self):
        sps = standard_parsing_state(extra_forbidden_characters='#')
        self.assertEqual(sps.forbidden_characters, '#$%')

    def test_custom_comment_start(self):
        sps = standard_parsing_state(comment_start='#')
        self.assertEqual(sps.forbidden_characters, '$%')


# --- NodesFinalizer ---

class TestNodesFinalizer(unittest.TestCase):

    def test_defaults(self):
        nf = NodesFinalizer()
        self.assertTrue(nf.simplify_whitespace)
        self.assertTrue(nf.auto_unicode_quotes)
        self.assertTrue(nf.ligature_unicode_quotes)
        self.assertTrue(nf.ligature_unicode_dashes)
        self.assertTrue(nf.ligature_unicode_ellipses)

    def test_auto_false(self):
        nf = NodesFinalizer(text_processing_options={'auto': False})
        self.assertFalse(nf.auto_unicode_quotes)
        self.assertFalse(nf.ligature_unicode_quotes)
        self.assertFalse(nf.ligature_unicode_dashes)
        self.assertFalse(nf.ligature_unicode_ellipses)

    def test_auto_quotes(self):
        nf = NodesFinalizer(text_processing_options={'auto': 'quotes'})
        self.assertTrue(nf.auto_unicode_quotes)
        self.assertFalse(nf.ligature_unicode_dashes)

    def test_auto_ligatures(self):
        nf = NodesFinalizer(text_processing_options={'auto': 'ligatures'})
        self.assertFalse(nf.auto_unicode_quotes)
        self.assertTrue(nf.ligature_unicode_quotes)

    def test_invalid_auto_raises(self):
        with self.assertRaises(ValueError):
            NodesFinalizer(text_processing_options={'auto': 'bad'})

    def test_invalid_option_raises(self):
        with self.assertRaises(ValueError):
            NodesFinalizer(text_processing_options={'bad_option': True})

    def test_process_text_whitespace(self):
        nf = NodesFinalizer()
        self.assertEqual(nf.process_text('Hello  world'), 'Hello world')

    def test_process_text_dashes(self):
        nf = NodesFinalizer()
        self.assertEqual(nf.process_text('a---b'), 'a\u2014b')

    def test_process_text_ellipsis(self):
        nf = NodesFinalizer()
        self.assertEqual(nf.process_text('wait...'), 'wait\u2026')


# --- make_standard_environment ---

class TestMakeStandardEnvironment(unittest.TestCase):

    def test_basic(self):
        env = make_standard_environment(standard_features())
        self.assertTrue(isinstance(env, FLMEnvironment))
        self.assertFalse(env.tolerant_parsing)

    def test_with_parsing_state_options(self):
        env = make_standard_environment(
            standard_features(),
            parsing_state_options={'dollar_inline_math_mode': True}
        )
        self.assertEqual(
            env.parsing_state.latex_inline_math_delimiters,
            [('\\(', '\\)'), ('$', '$')]
        )

    def test_with_flm_environment_options(self):
        env = make_standard_environment(
            standard_features(),
            flm_environment_options={'tolerant_parsing': True}
        )
        self.assertTrue(env.tolerant_parsing)


# --- FLMEnvironment methods ---

class TestFLMEnvironmentMethods(unittest.TestCase):

    def test_supports_feature(self):
        env = mk_flm_environ()
        self.assertTrue(env.supports_feature('refs'))
        self.assertFalse(env.supports_feature('nonexistent'))

    def test_feature(self):
        env = mk_flm_environ()
        self.assertEqual(env.feature('refs').feature_name, 'refs')

    def test_get_features_selection_none(self):
        env = mk_flm_environ()
        sel = env.get_features_selection(None)
        self.assertEqual(len(sel), 11)

    def test_get_features_selection_subset(self):
        env = mk_flm_environ()
        sel = env.get_features_selection(['refs', 'baseformatting'])
        self.assertEqual(len(sel), 2)
        names = [f.feature_name for f in sel]
        self.assertTrue('refs' in names)
        self.assertTrue('baseformatting' in names)

    def test_define_parsing_mode(self):
        env = mk_flm_environ()
        delta = FLMParsingStateDeltaSetBlockLevel(is_block_level=True)
        env.define_parsing_mode('mymode', delta)
        self.assertTrue('mymode' in env.parsing_mode_deltas)

    def test_define_parsing_mode_duplicate_raises(self):
        env = mk_flm_environ()
        delta = FLMParsingStateDeltaSetBlockLevel(is_block_level=True)
        env.define_parsing_mode('mymode', delta)
        with self.assertRaises(ValueError):
            env.define_parsing_mode('mymode', delta)

    def test_make_parsing_state_invalid_mode_raises(self):
        env = mk_flm_environ()
        with self.assertRaises(ValueError):
            env.make_parsing_state(is_block_level=None, parsing_mode='nonexistent')

    def test_make_fragment_returns_existing_fragment(self):
        env = mk_flm_environ()
        frag = env.make_fragment('hello', standalone_mode=True)
        frag2 = env.make_fragment(frag)
        self.assertIs(frag, frag2)

    def test_make_fragment_mismatch_raises(self):
        env = mk_flm_environ()
        frag = env.make_fragment('hello', standalone_mode=True)
        with self.assertRaises(ValueError):
            env.make_fragment(frag, standalone_mode=False)

    def test_make_document(self):
        env = mk_flm_environ()
        frag = env.make_fragment('Hello world')
        doc = env.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        self.assertEqual(result, 'Hello world')


# --- features_ensure_dependencies_are_met ---

class TestFeaturesEnsureDependencies(unittest.TestCase):

    def test_unmet_dependency_raises(self):
        class FakeFeature:
            def __init__(self, name, deps=None):
                self.feature_name = name
                self.feature_dependencies = deps
                self.feature_optional_dependencies = None
        with self.assertRaises(ValueError):
            features_ensure_dependencies_are_met([
                FakeFeature('a', deps=['b']),
            ])

    def test_met_dependencies_ok(self):
        class FakeFeature:
            def __init__(self, name, deps=None):
                self.feature_name = name
                self.feature_dependencies = deps
                self.feature_optional_dependencies = None
        # Should not raise
        features_ensure_dependencies_are_met([
            FakeFeature('a'),
            FakeFeature('b', deps=['a']),
        ])


# --- standard_environment_get_located_error_message ---

class TestStandardEnvironmentGetLocatedErrorMessage(unittest.TestCase):

    def test_percent_forbidden(self):
        env = mk_flm_environ()
        class FakeExc:
            error_type_info = {'what': 'token_forbidden_character', 'forbidden_character': '%'}
        msg = standard_environment_get_located_error_message(env, FakeExc())
        self.assertEqual(
            msg,
            'Single percent signs are not allowed here. Use \u2018\\%\u2019 to typeset a '
            'literal percent sign, and \u2018%%\u2019 to start a comment.'
        )

    def test_dollar_forbidden(self):
        env = mk_flm_environ()
        class FakeExc:
            error_type_info = {'what': 'token_forbidden_character', 'forbidden_character': '$'}
        msg = standard_environment_get_located_error_message(env, FakeExc())
        self.assertEqual(
            msg,
            "You can't use \u2018$\u2019 here. LaTeX math should be typeset using "
            "\\(...\\) for inline math and \\[...\\] for unnumbered display "
            "equations. Use \u2018\\$\u2019 for a literal dollar sign."
        )

    def test_other_error_returns_none(self):
        env = mk_flm_environ()
        class FakeExc:
            error_type_info = {'what': 'some_other_error'}
        msg = standard_environment_get_located_error_message(env, FakeExc())
        self.assertIsNone(msg)

    def test_no_error_type_info_returns_none(self):
        env = mk_flm_environ()
        class FakeExc:
            error_type_info = None
        msg = standard_environment_get_located_error_message(env, FakeExc())
        self.assertIsNone(msg)


# --- Rendering via full environment ---

class TestFLMEnvironmentRendering(unittest.TestCase):

    maxDiff = None

    def test_simple_render_standalone(self):
        env = mk_flm_environ()
        frag = env.make_fragment(r'Hello \textbf{world}', standalone_mode=True)
        result = frag.render_standalone(HtmlFragmentRenderer())
        self.assertEqual(result, 'Hello <span class="textbf">world</span>')

    def test_filter_whitespace_comments_nodes(self):
        env = mk_flm_environ()
        frag = env.make_fragment('Hello\n\nworld')
        lw = frag.nodes.latex_walker
        filtered = lw.filter_whitespace_comments_nodes(frag.nodes)
        self.assertEqual(len(filtered), 2)
        self.assertEqual(filtered[0].chars, 'Hello')
        self.assertEqual(filtered[1].chars, 'world')

    def test_forbidden_dollar_raises(self):
        env = mk_flm_environ()
        with self.assertRaises(LatexWalkerParseError):
            env.make_fragment('$x$', standalone_mode=True)

    def test_dollar_math_mode_enabled(self):
        env = make_standard_environment(
            standard_features(),
            parsing_state_options={'dollar_inline_math_mode': True}
        )
        frag = env.make_fragment('$x$', standalone_mode=True)
        result = frag.render_standalone(HtmlFragmentRenderer())
        self.assertEqual(
            result,
            '<span class="inline-math">\\(x\\)</span>'
        )


if __name__ == '__main__':
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
