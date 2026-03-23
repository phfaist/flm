import unittest

from pylatexenc.latexnodes import LatexWalkerParseError

from flm import flmspecinfo
from flm.flmspecinfo import (
    FLMSpecInfo,
    FLMMacroSpecBase,
    FLMEnvironmentSpecBase,
    FLMSpecialsSpecBase,
    FLMSpecInfoConstantValue,
    ConstantValueMacro,
    ConstantValueSpecials,
    TextFormatMacro,
    SemanticBlockEnvironment,
    FLMSpecInfoParagraphBreak,
    ParagraphBreakSpecials,
    ParagraphBreakMacro,
    FLMSpecInfoError,
    FLMMacroSpecError,
    FLMEnvironmentSpecError,
    FLMSpecialsSpecError,
    make_verb_argument,
    text_arg,
    label_arg,
)
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_standalone(environ, flm_input):
    frag = environ.make_fragment(flm_input, standalone_mode=True)
    return frag.render_standalone(HtmlFragmentRenderer())


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


# --- FLMSpecInfo class attributes ---

class TestFLMSpecInfo(unittest.TestCase):

    def test_no_delayed_render_by_default(self):
        macrospecinfo = FLMSpecInfo(spec_node_parser_type='macro')
        self.assertFalse(macrospecinfo.delayed_render)

    def test_is_block_level_default(self):
        si = FLMSpecInfo(spec_node_parser_type='macro')
        self.assertFalse(si.is_block_level)

    def test_is_block_heading_default(self):
        si = FLMSpecInfo(spec_node_parser_type='macro')
        self.assertFalse(si.is_block_heading)

    def test_is_paragraph_break_marker_default(self):
        si = FLMSpecInfo(spec_node_parser_type='macro')
        self.assertFalse(si.is_paragraph_break_marker)

    def test_allowed_in_standalone_mode_default(self):
        si = FLMSpecInfo(spec_node_parser_type='macro')
        self.assertFalse(si.allowed_in_standalone_mode)

    def test_body_contents_is_block_level_default(self):
        si = FLMSpecInfo(spec_node_parser_type='macro')
        self.assertIsNone(si.body_contents_is_block_level)




# --- Convenience base classes ---

class TestFLMMacroSpecBase(unittest.TestCase):

    def test_macroname(self):
        ms = FLMMacroSpecBase('textbf')
        self.assertEqual(ms.macroname, 'textbf')

    def test_with_arguments(self):
        ms = FLMMacroSpecBase('mymacro', arguments_spec_list=[text_arg])
        self.assertEqual(ms.macroname, 'mymacro')


class TestFLMEnvironmentSpecBase(unittest.TestCase):

    def test_environmentname(self):
        es = FLMEnvironmentSpecBase('itemize')
        self.assertEqual(es.environmentname, 'itemize')


class TestFLMSpecialsSpecBase(unittest.TestCase):

    def test_specials_chars(self):
        ss = FLMSpecialsSpecBase('~')
        self.assertEqual(ss.specials_chars, '~')


# --- make_verb_argument ---

class TestMakeVerbArgument(unittest.TestCase):

    def test_simple(self):
        self.assertEqual(make_verb_argument('hello'), '+hello+')

    def test_value_contains_plus(self):
        self.assertEqual(make_verb_argument('a+b'), '|a+b|')

    def test_value_contains_plus_and_pipe(self):
        self.assertEqual(make_verb_argument('a+b|c'), '=a+b|c=')

    def test_all_delimiters_used_raises(self):
        with self.assertRaises(ValueError):
            make_verb_argument('+|=.-!~,;:')


# --- ConstantValueMacro ---

class TestConstantValueMacro(unittest.TestCase):

    def test_value(self):
        cvm = ConstantValueMacro(macroname='textasciitilde', value='~')
        self.assertEqual(cvm.value, '~')

    def test_allowed_in_standalone_mode(self):
        cvm = ConstantValueMacro(macroname='textasciitilde', value='~')
        self.assertTrue(cvm.allowed_in_standalone_mode)


# --- ConstantValueSpecials ---

class TestConstantValueSpecials(unittest.TestCase):

    def test_value(self):
        cvs = ConstantValueSpecials(specials_chars='~', value='\xa0')
        self.assertEqual(cvs.value, '\xa0')


# --- text_arg and label_arg constants ---

class TestArgConstants(unittest.TestCase):

    def test_text_arg(self):
        self.assertEqual(text_arg.argname, 'text')
        self.assertEqual(text_arg.parser, '{')

    def test_label_arg(self):
        self.assertEqual(label_arg.argname, 'label')


# --- TextFormatMacro ---

class TestTextFormatMacro(unittest.TestCase):

    maxDiff = None

    def test_attributes(self):
        tfm = TextFormatMacro('textbf', text_formats=['textbf'])
        self.assertTrue(tfm.allowed_in_standalone_mode)
        self.assertEqual(tfm.macroname, 'textbf')
        self.assertEqual(tfm.text_formats, ['textbf'])

    def test_render_textbf(self):
        environ = mk_flm_environ()
        result = render_standalone(environ, r'\textbf{Hello}')
        self.assertEqual(result, '<span class="textbf">Hello</span>')

    def test_render_textit(self):
        environ = mk_flm_environ()
        result = render_standalone(environ, r'\textit{world}')
        self.assertEqual(result, '<span class="textit">world</span>')

    def test_render_multiple_formats(self):
        environ = mk_flm_environ()
        result = render_standalone(environ, r'\textbf{Hello} and \textit{world}')
        self.assertEqual(
            result,
            '<span class="textbf">Hello</span> and <span class="textit">world</span>'
        )


# --- SemanticBlockEnvironment ---

class TestSemanticBlockEnvironment(unittest.TestCase):

    def test_attributes(self):
        sbe = SemanticBlockEnvironment('center', role='body_text', annotations=['center'])
        self.assertEqual(sbe.role, 'body_text')
        self.assertEqual(sbe.annotations, ['center'])
        self.assertTrue(sbe.is_block_level)
        self.assertTrue(sbe.allowed_in_standalone_mode)


# --- FLMSpecInfoParagraphBreak ---

class TestParagraphBreak(unittest.TestCase):

    maxDiff = None

    def test_paragraph_break_specials_attributes(self):
        pbs = ParagraphBreakSpecials(specials_chars='\n\n')
        self.assertTrue(pbs.is_block_level)
        self.assertTrue(pbs.is_paragraph_break_marker)
        self.assertTrue(pbs.allowed_in_standalone_mode)

    def test_paragraph_break_macro_attributes(self):
        pbm = ParagraphBreakMacro(macroname='par')
        self.assertEqual(pbm.macroname, 'par')
        self.assertTrue(pbm.is_block_level)
        self.assertTrue(pbm.is_paragraph_break_marker)

    def test_paragraph_rendering_in_document(self):
        environ = mk_flm_environ()
        result = render_doc(environ, 'First para.\n\nSecond para.')
        self.assertEqual(result, '<p>First para.</p>\n<p>Second para.</p>')


# --- FLMSpecInfoError and subclasses ---

class TestFLMSpecInfoError(unittest.TestCase):

    def test_macro_spec_error_attributes(self):
        mse = FLMMacroSpecError(macroname='badmacro', error_msg='Not allowed here')
        self.assertEqual(mse.macroname, 'badmacro')
        self.assertEqual(mse.error_msg, 'Not allowed here')
        self.assertTrue(mse.allowed_in_standalone_mode)

    def test_environment_spec_error_attributes(self):
        ese = FLMEnvironmentSpecError(environmentname='badenv')
        self.assertEqual(ese.environmentname, 'badenv')
        self.assertIsNone(ese.error_msg)

    def test_specials_spec_error_attributes(self):
        sse = FLMSpecialsSpecError(specials_chars='@')
        self.assertEqual(sse.specials_chars, '@')
        self.assertIsNone(sse.error_msg)


# --- Rendering of ConstantValue via full environment ---

class TestConstantValueRendering(unittest.TestCase):

    maxDiff = None

    def test_tilde_renders_nbsp(self):
        environ = mk_flm_environ()
        result = render_standalone(environ, r'a~b')
        self.assertEqual(result, 'a&nbsp;b')

    def test_textbackslash(self):
        environ = mk_flm_environ()
        result = render_standalone(environ, r'\textbackslash{}')
        self.assertEqual(result, '\\')


if __name__ == '__main__':
    unittest.main()
