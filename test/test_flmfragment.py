import unittest

from flm.flmfragment import FLMFragment
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer

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



    def test_truncate_no_modif_original(self):

        env = mk_flm_environ()

        s = r'''
A nonlinear single-asymmetric-error code that generalize VT codes and that is constructed from an abelian group.

A CR code for group \(G\) and fixed group element \(g\) consists of all binary strings \(c=c_1c_2\cdots c_n\) that satisfy \(\sum_i c_i g_i = g\) for some elements \(h_i\) a.  Here, addition is the group operation, the multiplication \(1 g_i = g_i\), and \(0 g_i = 0_G\) where \(0\).  BUG!!
'''.strip()

        frag_full = FLMFragment(
            s,
            env,
            what='example text fragment'
        )

        frag_1 = frag_full.truncate_to(chars=360)

        self.assertEqual(frag_1.nodes.nodelist[-1].chars, ' where ')
        self.assertEqual(frag_1.nodes.nodelist[-1].chars, ' where ')
        self.assertEqual(frag_1.nodes.flm_blocks[1].nodelist[-1].chars, ' where ')
        # When building the node list, the last chars node gets a rstripped
        # flm_chars_value because it's the last node.
        self.assertEqual(frag_1.nodes.flm_blocks[1].nodelist[-1].flm_chars_value, ' where')

        # But, we want to make sure that the same node is not stripped in the
        # original fragment!
        the_charsnode_idx = len(frag_1.nodes.flm_blocks[1].nodelist) - 1
        self.assertEqual(
            frag_full.nodes.flm_blocks[1].nodelist[the_charsnode_idx].chars,
            ' where '
        )
        self.assertEqual(
            frag_full.nodes.flm_blocks[1].nodelist[the_charsnode_idx].flm_chars_value,
            ' where '
        )






class TestFLMFragmentParse(unittest.TestCase):

    def test_parse_classmethod_returns_walker_and_nodes(self):
        env = mk_flm_environ()
        latex_walker, nodes = FLMFragment.parse(
            r'Hello \textbf{world}!',
            env,
            standalone_mode=True,
            what='test-parse',
        )
        self.assertEqual(len(nodes), 3)
        self.assertTrue(nodes[0].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(nodes[0].chars, 'Hello ')
        self.assertTrue(nodes[1].isNodeType(latexnodes_nodes.LatexMacroNode))
        self.assertEqual(nodes[1].macroname, 'textbf')
        self.assertTrue(nodes[2].isNodeType(latexnodes_nodes.LatexCharsNode))
        self.assertEqual(nodes[2].chars, '!')
        self.assertTrue(latex_walker.standalone_mode)

    def test_parse_classmethod_invalid_input_raises(self):
        env = mk_flm_environ()
        with self.assertRaises(LatexWalkerParseError):
            FLMFragment.parse(
                r'\UnknownMacro',
                env,
                standalone_mode=True,
            )


class TestFLMFragmentRendering(unittest.TestCase):

    maxDiff = None

    def test_render_standalone(self):
        env = mk_flm_environ()
        frag = env.make_fragment(r'Hello \textbf{world}', standalone_mode=True)
        result = frag.render_standalone(HtmlFragmentRenderer())
        self.assertEqual(result, 'Hello <span class="textbf">world</span>')

    def test_render_standalone_on_non_standalone_raises(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test')
        with self.assertRaises(ValueError):
            frag.render_standalone(HtmlFragmentRenderer())


class TestFLMFragmentIsEmpty(unittest.TestCase):

    def test_empty_string(self):
        env = mk_flm_environ()
        frag = env.make_fragment('', standalone_mode=True)
        self.assertTrue(frag.is_empty())
        self.assertFalse(bool(frag))

    def test_whitespace_only(self):
        env = mk_flm_environ()
        frag = env.make_fragment('   ', standalone_mode=True)
        self.assertTrue(frag.is_empty())
        self.assertFalse(bool(frag))

    def test_has_content(self):
        env = mk_flm_environ()
        frag = env.make_fragment('hello', standalone_mode=True)
        self.assertFalse(frag.is_empty())
        self.assertTrue(bool(frag))


class TestFLMFragmentRepr(unittest.TestCase):

    def test_short_text(self):
        env = mk_flm_environ()
        frag = env.make_fragment('hello', standalone_mode=True)
        self.assertEqual(repr(frag), "<FLMFragment 'hello'>")

    def test_long_text_truncated(self):
        env = mk_flm_environ()
        long_text = 'A' * 60
        frag = env.make_fragment(long_text, standalone_mode=True)
        self.assertEqual(
            repr(frag),
            "<FLMFragment '" + 'A' * 49 + "\u2026'>"
        )


class TestFLMFragmentAttributes(unittest.TestCase):

    def test_what(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test', what='my-test')
        self.assertEqual(frag.what, 'my-test')

    def test_resource_info(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test', resource_info='/some/path')
        self.assertEqual(frag.resource_info, '/some/path')

    def test_standalone_mode(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test', standalone_mode=True)
        self.assertTrue(frag.standalone_mode)

    def test_standalone_mode_default_false(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test')
        self.assertFalse(frag.standalone_mode)


class TestFLMFragmentGetFirstParagraphNoParagraphs(unittest.TestCase):

    def test_no_paragraph_breaks_returns_same_text(self):
        env = mk_flm_environ()
        frag = env.make_fragment('just one paragraph', standalone_mode=True)
        fp = frag.get_first_paragraph()
        self.assertEqual(fp.flm_text, 'just one paragraph')


class TestFLMFragmentInitWithNodeList(unittest.TestCase):

    maxDiff = None

    def test_init_with_preparsed_nodes(self):
        env = mk_flm_environ()
        frag1 = env.make_fragment(r'Hello \textbf{world}', standalone_mode=True)
        # Re-create fragment from the already-parsed node list
        frag2 = FLMFragment(frag1.nodes, env, standalone_mode=True)
        self.assertEqual(frag2.flm_text, r'Hello \textbf{world}')
        self.assertEqual(len(frag2.nodes), 2)
        result = frag2.render_standalone(HtmlFragmentRenderer())
        self.assertEqual(result, 'Hello <span class="textbf">world</span>')

    def test_init_with_preparsed_nodes_custom_flm_text(self):
        env = mk_flm_environ()
        frag1 = env.make_fragment(r'Hello \textbf{world}', standalone_mode=True)
        frag2 = FLMFragment(
            frag1.nodes, env, standalone_mode=True,
            _flm_text_if_loading_nodes='custom text'
        )
        self.assertEqual(frag2.flm_text, 'custom text')


class TestFLMFragmentTolerantParsing(unittest.TestCase):

    maxDiff = None

    def test_tolerant_parsing_skips_unknown_macros(self):
        env = mk_flm_environ()
        frag = env.make_fragment(
            r'Hello \UnknownMacro world',
            tolerant_parsing=True,
            standalone_mode=True,
        )
        result = frag.render_standalone(HtmlFragmentRenderer())
        self.assertEqual(result, 'Hello world')


class TestFLMFragmentSilent(unittest.TestCase):

    def test_silent_still_raises_on_error(self):
        env = mk_flm_environ()
        with self.assertRaises(Exception):
            env.make_fragment(r'\BadMacro', silent=True)


class TestFLMFragmentIsBlockLevel(unittest.TestCase):

    def test_is_block_level_set(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test', is_block_level=True)
        self.assertTrue(frag.is_block_level)

    def test_is_block_level_default_none(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test')
        self.assertIsNone(frag.is_block_level)


class TestFLMFragmentStartNodeVisitor(unittest.TestCase):

    def test_start_node_visitor(self):
        env = mk_flm_environ()
        frag = env.make_fragment('hello', standalone_mode=True)

        class SimpleVisitor:
            def __init__(self):
                self.started = False
                self.start_nodes = None
            def start(self, nodes):
                self.started = True
                self.start_nodes = nodes

        visitor = SimpleVisitor()
        frag.start_node_visitor(visitor)
        self.assertTrue(visitor.started)
        self.assertIs(visitor.start_nodes, frag.nodes)


class TestFLMFragmentInternalAttributes(unittest.TestCase):

    def test_attributes_returns_dict(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test', standalone_mode=True, what='mytest',
                                 resource_info='/path')
        attrs = frag._attributes()
        self.assertEqual(attrs['what'], 'mytest')
        self.assertEqual(attrs['resource_info'], '/path')
        self.assertTrue(attrs['standalone_mode'])

    def test_attributes_kwargs_override(self):
        env = mk_flm_environ()
        frag = env.make_fragment('test', what='original')
        attrs = frag._attributes(what='overridden')
        self.assertEqual(attrs['what'], 'overridden')


if __name__ == '__main__':
    unittest.main()
