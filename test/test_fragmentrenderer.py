import unittest

from llm.fragmentrenderer import FragmentRenderer

from llm.llmstd import LLMStandardEnvironment
from llm.llmfragment import LLMFragment


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

    def render_join(self, content_list):
        _register_call(self.store, 'render_join', (content_list,))
        return self.pieces_joiner_string.join(content_list)

    def render_build_paragraph(self, content_list):
        print("content_list = ", content_list)
        _register_call(self.store, 'render_build_paragraph', (list(content_list),))
        if self.mark_paragraphs:
            return "<P>" + self.pieces_joiner_string.join(content_list) + "</P>"
        return self.pieces_joiner_string.join(content_list)

    def render_join_blocks(self, content_list):
        _register_call(self.store, 'render_join_blocks', (content_list,))
        return self.blocks_joiner_string.join(content_list)

    # --

    def render_value(self, value):
        _register_call(self.store, 'render_value', (value,))
        return value

    def render_delayed_marker(self, node, delayed_key, doc):
        _register_call(self.store, 'render_delayed_marker', (node, delayed_key, doc))
        return ''

    def render_empty_error_placeholder(self, debug_str):
        _register_call(self.store, 'render_empty_error_placeholder', (debug_str,))
        return ''

    def render_text_format(self, text_formats, content):
        _register_call(self.store, 'render_text_format', (text_formats, content,))
        return '['+content+']'
    
    def render_verbatim(self, value, annotation):
        _register_call(self.store, 'render_verbatim', (value, annotation,))
        return value

    def render_link(self, ref_type, href, display_content):
        _register_call(self.store, 'render_link', (ref_type, href, display_content,))
        return display_content + ' <' + href + '>'





class TestFragmentRenderer(unittest.TestCase):

    def test_renders_text_format(self):

        env = LLMStandardEnvironment()
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
                ('render_value', ('Hello ',)),
                ('render_value', ('world',)),
                ('render_join', (['world'],)),
                ('render_text_format', (('textbf',), 'world',)),
                ('render_value', ('.',)),
                ('render_join', (['Hello ', '[world]', '.'],)),
            ]
        )


    def test_render_nodelist_with_block_level(self):

        env = LLMStandardEnvironment()
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
                ('render_value', ('Hello.',)),
                ('render_build_paragraph', (['Hello.'],)),
                ('render_value', ('New paragraph.',)),
                ('render_build_paragraph', (['New paragraph.'],)),
                ('render_join_blocks', (['Hello.', 'New paragraph.'],)),
            ]
        )


    def test_render_nodelist_no_block_level(self):

        env = LLMStandardEnvironment()
        s = r'Hello \textbf{world}.'
        frag = env.make_fragment(s, what='example text fragment')

        store = {'calls': []}
        fr = _MyTestFragmentRenderer(store)

        render_result = fr.render_fragment(frag, None, is_block_level=False)

        self.assertEqual(render_result, r"Hello [world].")

        self.assertEqual(
            store['calls'],
            [
                ('render_value', ('Hello ',)),
                ('render_value', ('world',)),
                ('render_join', (['world'],)),
                ('render_text_format', (('textbf',), 'world',)),
                ('render_value', ('.',)),
                ('render_join', (['Hello ', '[world]', '.'],)),
            ]
        )


    def test_render_nodelist_autodetect_block_level(self):

        env = LLMStandardEnvironment()
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
                ('render_value', ('Hello.',)),
                ('render_build_paragraph', (['Hello.'],)),
                ('render_value', ('New paragraph.',)),
                ('render_build_paragraph', (['New paragraph.'],)),
                ('render_join_blocks', (['Hello.', 'New paragraph.'],)),
            ]
        )


    def test_render_nodelist_with_paragraphs(self):

        env = LLMStandardEnvironment()
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
            "<P>Here is a new\n paragraph.</P>\n\n"
            "<P>And here comes\nanother one.</P>"
        )

    def test_render_nodelist_with_paragraphs_single_paragraph(self):

        env = LLMStandardEnvironment()
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
<P>Hello |[world]|. There are no paragraphs here, but some nodes are
not only chars nodes (such as math nodes: |\(1+2=3\)|). The different node
types will have to be joined together with our |[render nodelist]|
function.</P>
""".strip()
        )

    def test_render_nodelist_no_paragraphs(self):

        env = LLMStandardEnvironment()
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
Hello |[world]|. There are no paragraphs here, but some nodes are
not only chars nodes (such as math nodes: |\(1+2=3\)|). The different node
types will have to be joined together with our |[render nodelist]|
function.
""".strip()
        )



if __name__ == '__main__':
    unittest.main()
