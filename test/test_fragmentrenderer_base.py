import unittest

from flm.fragmentrenderer import FragmentRenderer

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



if __name__ == '__main__':
    unittest.main()
