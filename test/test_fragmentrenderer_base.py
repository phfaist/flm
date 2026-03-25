import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.fragmentrenderer import FragmentRenderer
from flm.flmrendercontext import FLMRenderContext

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features

def mk_flm_environ():
    return make_standard_environment(features=standard_features())


def _register_call(store, what, args):
    store['calls'].append( (what, args) )


class _MyTestFragmentRenderer(FragmentRenderer):

    mark_paragraphs = False
    pieces_joiner_string = ""
    blocks_joiner_string = "\n\n"

    def __init__(self, store):
        super().__init__()
        self.store = store

    # --

    def render_blocks(self, node_blocks, render_context):
        _register_call(self.store, 'render_blocks', tuple())
        return super().render_blocks(node_blocks, render_context)

    def render_build_paragraph(self, nodelist, render_context):
        _register_call(self.store, 'render_build_paragraph', tuple())
        joined = self.render_inline_content(nodelist, render_context)
        if self.mark_paragraphs:
            return "<P>" + joined + "</P>"
        return joined

    def render_inline_content(self, nodelist, render_context):
        _register_call(self.store, 'render_inline_content', tuple())
        return self.render_join([
            self.render_node(n, render_context)
            for n in nodelist
        ], render_context)

    def render_join(self, content_list, render_context):
        _register_call(self.store, 'render_join', (content_list,))
        return self.pieces_joiner_string.join(content_list)

    def render_join_blocks(self, content_list, render_context):
        _register_call(self.store, 'render_join_blocks', (content_list,))
        return self.blocks_joiner_string.join(content_list)

    # --

    def render_value(self, value, render_context):
        _register_call(self.store, 'render_value', (value,))
        return value

    def render_delayed_marker(self, node, delayed_key, doc):
        _register_call(self.store, 'render_delayed_marker', (node, delayed_key, doc))
        return ''

    def render_empty_error_placeholder(self, debug_str, render_context):
        _register_call(self.store, 'render_empty_error_placeholder', (debug_str,))
        return ''

    def render_text_format(self, text_formats, nodelist, render_context):
        _register_call(self.store, 'render_text_format', (text_formats,))
        content = self.render_nodelist(nodelist, render_context, is_block_level=False)
        return '['+content+']'
    
    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
        _register_call(self.store, 'render_verbatim',
                       (value, is_block_level, annotations, target_id,))
        return value

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):
        _register_call(self.store, 'render_link', (ref_type, href,))
        return display_content + ' <' + href + '>'





class TestFragmentRenderer(unittest.TestCase):

    def test_renders_text_format(self):

        env = mk_flm_environ()
        s = r'Hello \textbf{world}.'
        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        # auto-detect block level, here auto-detects no block level.
        render_result = fr.render_fragment(frag, None)

        self.assertEqual(render_result, r"Hello [world].")

        self.assertEqual(
            store['calls'],
            [
                ('render_inline_content', tuple()),
                ('render_value', ('Hello ',)),
                ('render_text_format', (('textbf',),)),
                ('render_inline_content', tuple()),
                ('render_value', ('world',)),
                ('render_join', (['world'],)),
                ('render_value', ('.',)),
                ('render_join', (['Hello ', '[world]', '.'],)),
            ]
        )


    def test_render_nodelist_with_block_level(self):

        env = mk_flm_environ()
        s = r'''Hello.

New paragraph.'''
        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        render_result = fr.render_fragment(frag, None, is_block_level=True)

        self.assertEqual(render_result, r"""Hello.

New paragraph.""")

        self.assertEqual(
            store['calls'],
            [
                ('render_blocks', tuple()),
                ('render_build_paragraph', tuple()),
                ('render_inline_content', tuple()),
                ('render_value', ('Hello.',)),
                ('render_join', (['Hello.'],)),
                ('render_build_paragraph', tuple()),
                ('render_inline_content', tuple()),
                ('render_value', ('New paragraph.',)),
                ('render_join', (['New paragraph.'],)),
                #
                ('render_join_blocks', (['Hello.', 'New paragraph.'],)),
            ]
        )


    def test_render_nodelist_no_block_level(self):

        env = mk_flm_environ()
        s = r'Hello \textbf{world}.'
        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        render_result = fr.render_fragment(frag, None, is_block_level=False)

        self.assertEqual(render_result, r"Hello [world].")

        self.assertEqual(
            store['calls'],
            [
                ('render_inline_content', tuple()),
                ('render_value', ('Hello ',)),
                ('render_text_format', (('textbf',),)),
                ('render_inline_content', tuple()),
                ('render_value', ('world',)),
                ('render_join', (['world'],)),
                ('render_value', ('.',)),
                ('render_join', (['Hello ', '[world]', '.'],)),
            ]
        )


    def test_render_nodelist_autodetect_block_level(self):

        env = mk_flm_environ()
        s = r'''Hello.

New paragraph.'''
        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        # auto-detect block level, here auto-detects no block level.
        render_result = fr.render_fragment(frag, None)

        self.assertEqual(render_result, r"""Hello.

New paragraph.""")

        self.assertEqual(
            store['calls'],
            [
                ('render_blocks', tuple()),
                ('render_build_paragraph', tuple()),
                ('render_inline_content', tuple()),
                ('render_value', ('Hello.',)),
                ('render_join', (['Hello.'],)),
                ('render_build_paragraph', tuple()),
                ('render_inline_content', tuple()),
                ('render_value', ('New paragraph.',)),
                ('render_join', (['New paragraph.'],)),
                ('render_join_blocks', (['Hello.', 'New paragraph.'],)),
            ]
        )


    def test_render_nodelist_with_paragraphs(self):

        env = mk_flm_environ()
        s = r'''

Hello world.

Here is a new
 paragraph.

And here comes
another one.

'''.strip()

        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)
        fr.mark_paragraphs = True

        render_result = fr.render_nodelist(frag.nodes, None, is_block_level=True)

        self.assertEqual(
            render_result,
            "<P>Hello world.</P>\n\n"
            "<P>Here is a new paragraph.</P>\n\n"
            "<P>And here comes another one.</P>"
        )

    def test_render_nodelist_with_paragraphs_single_paragraph(self):

        env = mk_flm_environ()
        s = r'''

Hello \textbf{world}. There are no paragraphs here, but some nodes are
not only chars nodes (such as math nodes: \(1+2=3\)). The different node
types will have to be joined together with our \textit{render nodelist}
function.

'''.strip()

        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)
        
        fr.pieces_joiner_string = "|"
        fr.mark_paragraphs = True

        render_result = fr.render_nodelist(frag.nodes, None, is_block_level=True)

        self.assertEqual(
            render_result,
            r"""
<P>Hello |[world]|. There are no paragraphs here, but some nodes are not only chars nodes (such as math nodes: |\(1+2=3\)|). The different node types will have to be joined together with our |[render nodelist]| function.</P>
""".strip()
        )

    def test_render_nodelist_no_paragraphs(self):

        env = mk_flm_environ()
        s = r'''

Hello \textbf{world}. There are no paragraphs here, but some nodes are
not only chars nodes (such as math nodes: \(1+2=3\)). The different node
types will have to be joined together with our \textit{render nodelist}
function.

'''.strip()

        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)
        
        fr.pieces_joiner_string = "|"
        fr.mark_paragraphs = True

        render_result = fr.render_nodelist(frag.nodes, None, is_block_level=False)

        self.assertEqual(
            render_result,
            r"""
Hello |[world]|. There are no paragraphs here, but some nodes are not only chars nodes (such as math nodes: |\(1+2=3\)|). The different node types will have to be joined together with our |[render nodelist]| function.
""".strip()
        )



class TestFragmentRendererInit(unittest.TestCase):

    def test_default_init(self):
        fr = FragmentRenderer()
        self.assertFalse(fr.supports_delayed_render_markers)

    def test_init_with_config(self):
        fr = FragmentRenderer(config={
            'supports_delayed_render_markers': True,
            'custom_attr': 42,
        })
        self.assertTrue(fr.supports_delayed_render_markers)
        self.assertEqual(fr.custom_attr, 42)

    def test_init_with_none_config(self):
        fr = FragmentRenderer(config=None)
        self.assertFalse(fr.supports_delayed_render_markers)

    def test_init_with_empty_config(self):
        fr = FragmentRenderer(config={})
        self.assertFalse(fr.supports_delayed_render_markers)


class TestEnsureRenderContext(unittest.TestCase):

    def test_ensure_render_context_none(self):
        fr = FragmentRenderer()
        rc = fr.ensure_render_context(None)
        self.assertTrue(isinstance(rc, FLMRenderContext))
        self.assertIs(rc.fragment_renderer, fr)

    def test_ensure_render_context_existing(self):
        fr = FragmentRenderer()
        existing_rc = FLMRenderContext(fragment_renderer=fr)
        rc = fr.ensure_render_context(existing_rc)
        self.assertIs(rc, existing_rc)


class TestSemanticPassthrough(unittest.TestCase):

    def test_semantic_span_returns_content(self):
        fr = FragmentRenderer()
        self.assertEqual(
            fr.render_semantic_span('hello', 'role', None),
            'hello'
        )

    def test_semantic_block_returns_content(self):
        fr = FragmentRenderer()
        self.assertEqual(
            fr.render_semantic_block('content', 'role', None),
            'content'
        )


class TestRenderJoinMethods(unittest.TestCase):

    def test_render_join(self):
        fr = FragmentRenderer()
        self.assertEqual(fr.render_join(['x', 'y', 'z'], None), 'xyz')

    def test_render_join_empty(self):
        fr = FragmentRenderer()
        self.assertEqual(fr.render_join([], None), '')

    def test_render_join_blocks(self):
        fr = FragmentRenderer()
        self.assertEqual(
            fr.render_join_blocks(['a', 'b', 'c'], None),
            'a\n\nb\n\nc'
        )

    def test_render_join_blocks_filters_none_and_empty(self):
        fr = FragmentRenderer()
        self.assertEqual(
            fr.render_join_blocks(['a', None, '', 'b', None, 'c'], None),
            'a\n\nb\n\nc'
        )

    def test_render_join_blocks_all_empty(self):
        fr = FragmentRenderer()
        self.assertEqual(
            fr.render_join_blocks([None, '', None], None),
            ''
        )


class TestRenderNodelistErrors(unittest.TestCase):

    def test_none_nodelist_raises_value_error(self):
        fr = FragmentRenderer()
        with self.assertRaises(ValueError):
            fr.render_nodelist(None, None)

    def test_missing_flm_is_block_level_raises(self):
        fr = FragmentRenderer()
        with self.assertRaises(LatexWalkerLocatedError):
            fr.render_nodelist([1, 2, 3], None)

    def test_block_content_in_inline_mode_raises(self):
        env = mk_flm_environ()
        s = 'Hello.\n\nNew paragraph.'
        frag = env.make_fragment(s, what='test')
        fr = FragmentRenderer()
        with self.assertRaises(ValueError):
            fr.render_nodelist(frag.nodes, None, is_block_level=False)


class TestAbstractMethodsRaise(unittest.TestCase):

    # Use Exception instead of RuntimeError for Transcrypt compatibility

    def test_render_value(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_value('val', None)

    def test_render_delayed_marker(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_delayed_marker(None, 'key', None)

    def test_render_delayed_dummy_placeholder(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_delayed_dummy_placeholder(None, 'key', None)

    def test_render_nothing(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_nothing(None)

    def test_render_empty_error_placeholder(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_empty_error_placeholder('debug', None)

    def test_render_text_format(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_text_format(['bold'], None, None)

    def test_render_enumeration(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_enumeration([], None, None)

    def test_render_lines(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_lines([], None)

    def test_render_heading(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_heading(None, None)

    def test_render_verbatim(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_verbatim('val', None)

    def test_render_link(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_link('ref', 'href', None, None)

    def test_render_annotation_comment(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_annotation_comment(None, None)

    def test_render_annotation_highlight(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_annotation_highlight(None, None)

    def test_render_float(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_float(None, None)

    def test_render_graphics_block(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_graphics_block(None, None)

    def test_render_cells(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.render_cells(None, None)

    def test_replace_delayed_markers_with_final_values(self):
        fr = FragmentRenderer()
        with self.assertRaises(Exception):
            fr.replace_delayed_markers_with_final_values('content', {})


class TestRenderComment(unittest.TestCase):

    maxDiff = None

    def test_comment_stripped(self):
        env = mk_flm_environ()
        s = 'Hello%% comment\n world'
        frag = env.make_fragment(s, what='test')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        result = fr.render_fragment(frag, None, is_block_level=False)
        self.assertEqual(result, 'Helloworld')


class TestRenderMathContent(unittest.TestCase):

    maxDiff = None

    def test_inline_math(self):
        env = mk_flm_environ()
        s = r'x = \(a+b\)'
        frag = env.make_fragment(s, what='test')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        result = fr.render_fragment(frag, None, is_block_level=False)
        self.assertEqual(result, r'x = \(a+b\)')

    def test_inline_math_calls(self):
        env = mk_flm_environ()
        s = r'\(a+b\)'
        frag = env.make_fragment(s, what='test')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        result = fr.render_fragment(frag, None, is_block_level=False)
        self.assertEqual(result, r'\(a+b\)')

        # Check that render_verbatim was called with correct annotations
        verbatim_calls = [c for c in store['calls'] if c[0] == 'render_verbatim']
        self.assertEqual(len(verbatim_calls), 1)
        self.assertEqual(verbatim_calls[0][1][0], r'\(a+b\)')
        self.assertEqual(verbatim_calls[0][1][1], False)  # is_block_level
        self.assertEqual(verbatim_calls[0][1][2], ['inline-math'])  # annotations


class TestRenderNodelistInlineForced(unittest.TestCase):

    maxDiff = None

    def test_inline_no_block_content(self):
        env = mk_flm_environ()
        s = r'Hello \textbf{world}.'
        frag = env.make_fragment(s, what='test')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        result = fr.render_nodelist(frag.nodes, None, is_block_level=False)
        self.assertEqual(result, 'Hello [world].')

    def test_force_block_on_inline_content(self):
        env = mk_flm_environ()
        s = r'Hello world.'
        frag = env.make_fragment(s, what='test')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)
        fr.mark_paragraphs = True

        # Force block level on content that has no block-level items
        result = fr.render_nodelist(frag.nodes, None, is_block_level=True)
        self.assertEqual(result, '<P>Hello world.</P>')


if __name__ == '__main__':
    unittest.main()
