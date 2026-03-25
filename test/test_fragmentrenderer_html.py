import unittest
import re

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import (
    HtmlFragmentRenderer,
    get_html_css_global,
    get_html_css_content,
    get_html_js,
    get_html_body_end_js_scripts,
    FragmentRendererInformation,
)
from flm.feature.cells import FeatureCells


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_environ():
    return make_standard_environment(standard_features())


def _make_nodelist(text, is_block_level=False):
    environ = _make_environ()
    frag = environ.make_fragment(text, is_block_level=is_block_level, standalone_mode=True)
    return frag.nodes


def _render_standalone(text, fr=None):
    if fr is None:
        fr = HtmlFragmentRenderer()
    environ = _make_environ()
    frag = environ.make_fragment(text, is_block_level=False, standalone_mode=True)
    return frag.render_standalone(fr)


def _render_block(text, fr=None):
    if fr is None:
        fr = HtmlFragmentRenderer()
    environ = _make_environ()
    frag = environ.make_fragment(text)
    doc = environ.make_document(frag.render)
    result, _ = doc.render(fr)
    return result


class _MockLineInfo:
    def __init__(self, nodelist, indent_left=None, indent_right=None, align=None):
        self.nodelist = nodelist
        self.indent_left = indent_left
        self.indent_right = indent_right
        self.align = align


class _MockGraphicsResource:
    src_url = 'image.png'
    physical_dimensions = None
    graphics_type = 'raster'
    srcset = None


# ---------------------------------------------------------------------------
# Escape helpers
# ---------------------------------------------------------------------------

class TestHtmlEscape(unittest.TestCase):

    def test_htmlescape_basic_entities(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.htmlescape('<b>bold</b> & "quoted"'),
            '&lt;b&gt;bold&lt;/b&gt; &amp; &quot;quoted&quot;'
        )

    def test_htmlescape_plain_text_unchanged(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.htmlescape('Hello world'), 'Hello world')

    def test_htmlescape_nonbreaking_space(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.htmlescape('\u00a0'), '&nbsp;')

    def test_htmlescape_hair_space(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.htmlescape('\u200a'), '&hairsp;')

    def test_htmlescape_thin_space(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.htmlescape('\u2009'), '&thinsp;')

    def test_htmlescape_en_space(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.htmlescape('\u2002'), '&ensp;')

    def test_htmlescape_em_space(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.htmlescape('\u2003'), '&emsp;')

    def test_htmlescape_attribute_plain_ampersand_unchanged(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1&b=2'),
            'https://example.com/page?a=1&b=2'
        )

    def test_htmlescape_attribute_entity_escaped(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1&b;2'),
            'https://example.com/page?a=1&amp;b;2'
        )

    def test_htmlescape_attribute_double_quote_escaped(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1"&b=2'),
            'https://example.com/page?a=1&quot;&b=2'
        )

    def test_htmlescape_attribute_aggressive_mode(self):
        fr = HtmlFragmentRenderer()
        fr.aggressively_escape_html_attributes = True
        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1&b=2'),
            'https://example.com/page?a=1&amp;b=2'
        )

    def test_htmlescape_attribute_named_entity_escaped(self):
        fr = HtmlFragmentRenderer()
        # &amp; looks like an HTML entity, the & should be escaped
        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('a=1&amp;b=2'),
            'a=1&amp;amp;b=2'
        )


# ---------------------------------------------------------------------------
# Tag-building helpers
# ---------------------------------------------------------------------------

class TestTagHelpers(unittest.TestCase):

    def test_generate_open_tag_bare(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.generate_open_tag('div'), '<div>')

    def test_generate_open_tag_with_attrs(self):
        fr = HtmlFragmentRenderer()
        result = fr.generate_open_tag('a', attrs={'href': 'https://example.com'})
        self.assertEqual(result, '<a href="https://example.com">')

    def test_generate_open_tag_with_class_names(self):
        fr = HtmlFragmentRenderer()
        result = fr.generate_open_tag('span', class_names=['foo', 'bar'])
        self.assertEqual(result, '<span class="foo bar">')

    def test_generate_open_tag_attrs_and_class_names(self):
        fr = HtmlFragmentRenderer()
        result = fr.generate_open_tag('a', attrs={'href': '#'}, class_names=['active'])
        # class is added to attrs dict after the existing attrs, so comes last
        self.assertEqual(result, '<a href="#" class="active">')

    def test_generate_open_tag_self_close(self):
        fr = HtmlFragmentRenderer()
        result = fr.generate_open_tag('img', attrs={'src': 'a.png'}, self_close_tag=True)
        self.assertEqual(result, '<img src="a.png"/>')

    def test_generate_open_tag_class_via_attrs_raises(self):
        fr = HtmlFragmentRenderer()
        self.assertRaises(
            ValueError,
            fr.generate_open_tag, 'span', attrs={'class': 'foo'}
        )

    def test_generate_open_tag_attrs_as_list_of_tuples(self):
        fr = HtmlFragmentRenderer()
        result = fr.generate_open_tag('a', attrs=[('href', 'http://example.com'), ('id', 'lnk')])
        self.assertEqual(result, '<a href="http://example.com" id="lnk">')

    def test_wrap_in_tag_simple(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.wrap_in_tag('p', 'Hello'), '<p>Hello</p>')

    def test_wrap_in_tag_with_class_names(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.wrap_in_tag('span', 'text', class_names=['bold']),
            '<span class="bold">text</span>'
        )

    def test_wrap_in_tag_with_attrs(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.wrap_in_tag('a', 'click', attrs={'href': '#top'}),
            '<a href="#top">click</a>'
        )

    def test_wrap_in_tag_empty_content(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.wrap_in_tag('div', ''), '<div></div>')

    def test_wrap_in_link_basic(self):
        fr = HtmlFragmentRenderer()
        result = fr.wrap_in_link('Click me', 'https://example.com')
        self.assertEqual(result, '<a href="https://example.com">Click me</a>')

    def test_wrap_in_link_none_href_becomes_hash(self):
        fr = HtmlFragmentRenderer()
        result = fr.wrap_in_link('Click', None)
        self.assertEqual(result, '<a href="#">Click</a>')

    def test_wrap_in_link_no_target_blank_by_default(self):
        fr = HtmlFragmentRenderer()
        result = fr.wrap_in_link('Ext', 'https://example.com')
        self.assertEqual(result, '<a href="https://example.com">Ext</a>')

    def test_wrap_in_link_target_blank_true_external(self):
        fr = HtmlFragmentRenderer()
        fr.use_link_target_blank = True
        result = fr.wrap_in_link('Ext', 'https://example.com')
        self.assertEqual(
            result,
            '<a href="https://example.com" target="_blank">Ext</a>'
        )

    def test_wrap_in_link_target_blank_not_for_anchor(self):
        fr = HtmlFragmentRenderer()
        fr.use_link_target_blank = True
        result = fr.wrap_in_link('Top', '#top')
        self.assertEqual(result, '<a href="#top">Top</a>')

    def test_wrap_in_link_target_blank_callable_true(self):
        fr = HtmlFragmentRenderer()
        fr.use_link_target_blank = lambda url: url.startswith('https://')
        result = fr.wrap_in_link('Ext', 'https://example.com')
        self.assertEqual(
            result,
            '<a href="https://example.com" target="_blank">Ext</a>'
        )

    def test_wrap_in_link_target_blank_callable_false(self):
        fr = HtmlFragmentRenderer()
        fr.use_link_target_blank = lambda url: url.startswith('https://')
        result = fr.wrap_in_link('Int', 'http://example.com')
        self.assertEqual(result, '<a href="http://example.com">Int</a>')

    def test_wrap_in_link_class_names(self):
        fr = HtmlFragmentRenderer()
        result = fr.wrap_in_link('Link', '#sec', class_names=['myclass'])
        self.assertEqual(result, '<a href="#sec" class="myclass">Link</a>')


# ---------------------------------------------------------------------------
# Core render methods (no FLM environment needed)
# ---------------------------------------------------------------------------

class TestRenderMethodsDirect(unittest.TestCase):

    maxDiff = None

    def test_render_value_plain(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_value('Hello world', None), 'Hello world')

    def test_render_value_escapes_html(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(
            fr.render_value('<b>bold</b>', None),
            '&lt;b&gt;bold&lt;/b&gt;'
        )

    def test_render_empty_error_placeholder_structure(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_empty_error_placeholder('some error', None)
        self.assertEqual(
            result,
            '<span class="empty-error-placeholder"><!-- some error -->(?)</span>'
        )

    def test_render_empty_error_placeholder_sanitizes_double_dash(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_empty_error_placeholder('a--b', None)
        self.assertEqual(
            result,
            '<span class="empty-error-placeholder"><!-- a- - b -->(?)</span>'
        )

    def test_render_nothing_default_comment(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_nothing(None)
        # empty annotations list joined by space gives empty string, so two spaces
        self.assertEqual(result, '<!--  -->')

    def test_render_nothing_with_annotations(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_nothing(None, annotations=['foo', 'bar'])
        self.assertEqual(result, '<!-- foo bar -->')

    def test_render_nothing_empty_string_mode(self):
        fr = HtmlFragmentRenderer()
        fr.render_nothing_as_comment_with_annotations = False
        result = fr.render_nothing(None, annotations=['foo'])
        self.assertEqual(result, '')

    def test_render_nothing_sanitizes_double_dash_in_annotations(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_nothing(None, annotations=['a--b'])
        self.assertEqual(result, '<!-- a- - b -->')

    def test_render_verbatim_default_class(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        result = fr.render_verbatim('Hello', None, annotations=[])
        self.assertEqual(result, '<span class="verbatimtext">Hello</span>')

    def test_render_verbatim_with_annotation(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        result = fr.render_verbatim('code', None, annotations=['verbatimcode'])
        self.assertEqual(result, '<span class="verbatimcode">code</span>')

    def test_render_verbatim_block_level_uses_div(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        result = fr.render_verbatim(
            'code block', None,
            annotations=['verbatimcode-environment'],
            is_block_level=True
        )
        self.assertEqual(result, '<div class="verbatimcode-environment">code block</div>')

    def test_render_verbatim_with_target_id(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        result = fr.render_verbatim('x', None, target_id='myid', annotations=['verbatimcode'])
        self.assertEqual(result, '<span id="myid" class="verbatimcode">x</span>')

    def test_render_verbatim_backslash_protection_on(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_verbatim(r'\alpha', None, annotations=['verbatimcode'])
        self.assertEqual(
            result,
            '<span class="verbatimcode"><span>\\</span>alpha</span>'
        )

    def test_render_verbatim_backslash_protection_off(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        result = fr.render_verbatim(r'\alpha', None, annotations=['verbatimcode'])
        self.assertEqual(result, '<span class="verbatimcode">\\alpha</span>')

    def test_render_verbatim_highlight_spaces(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        fr.verbatim_highlight_spaces = True
        result = fr.render_verbatim('a b', None, annotations=['verbatimcode'])
        self.assertEqual(
            result,
            '<span class="verbatimcode">'
            'a<span class="verbatimspace">&nbsp;</span>b</span>'
        )

    def test_render_verbatim_escapes_html_chars(self):
        fr = HtmlFragmentRenderer()
        fr.verbatim_protect_backslashes = False
        result = fr.render_verbatim('<script>', None, annotations=['verbatimcode'])
        self.assertEqual(result, '<span class="verbatimcode">&lt;script&gt;</span>')

    def test_render_delayed_marker(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_delayed_marker(None, 42, None), '<FLM:DLYD:42/>')

    def test_render_delayed_marker_zero(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_delayed_marker(None, 0, None), '<FLM:DLYD:0/>')

    def test_render_delayed_dummy_placeholder(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_delayed_dummy_placeholder(None, 7, None), '<!-- delayed:7 -->')

    def test_replace_delayed_markers_basic(self):
        fr = HtmlFragmentRenderer()
        content = 'before <FLM:DLYD:0/> and <FLM:DLYD:1/> after'
        result = fr.replace_delayed_markers_with_final_values(
            content, {0: 'FIRST', 1: 'SECOND'}
        )
        self.assertEqual(result, 'before FIRST and SECOND after')

    def test_replace_delayed_markers_no_markers(self):
        fr = HtmlFragmentRenderer()
        content = '<p>no markers here</p>'
        result = fr.replace_delayed_markers_with_final_values(content, {})
        self.assertEqual(result, '<p>no markers here</p>')

    def test_replace_delayed_markers_html_content(self):
        fr = HtmlFragmentRenderer()
        content = '<p><FLM:DLYD:5/></p>'
        result = fr.replace_delayed_markers_with_final_values(
            content, {5: '<a href="#x">ref</a>'}
        )
        self.assertEqual(result, '<p><a href="#x">ref</a></p>')

    def test_render_join(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_join(['a', 'b', 'c'], None), 'abc')

    def test_render_join_empty(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_join([], None), '')

    def test_render_join_coerces_to_str(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.render_join([1, 2, 3], None), '123')

    def test_render_join_blocks_default_joiner(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_join_blocks(['<p>A</p>', '<p>B</p>'], None)
        self.assertEqual(result, '<p>A</p>\n<p>B</p>')

    def test_render_join_blocks_skips_empty_and_none(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_join_blocks(['<p>A</p>', '', None, '<p>B</p>'], None)
        self.assertEqual(result, '<p>A</p>\n<p>B</p>')

    def test_render_join_blocks_custom_joiner(self):
        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''
        result = fr.render_join_blocks(['X', 'Y'], None)
        self.assertEqual(result, 'XY')

    def test_render_semantic_span_basic(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_span('content', 'citation', None)
        self.assertEqual(result, '<span class="citation">content</span>')

    def test_render_semantic_span_role_not_duplicated_in_annotations(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_span(
            'content', 'citation', None, annotations=['citation', 'note']
        )
        self.assertEqual(result, '<span class="citation note">content</span>')

    def test_render_semantic_span_extra_annotations(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_span('x', 'ref', None, annotations=['extra'])
        self.assertEqual(result, '<span class="ref extra">x</span>')

    def test_render_semantic_span_with_target_id(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_span('content', 'ref', None, target_id='ref-1')
        self.assertEqual(result, '<span id="ref-1" class="ref">content</span>')

    def test_render_semantic_block_default_div(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_block('content', 'theorem', None)
        self.assertEqual(result, '<div class="theorem">content</div>')

    def test_render_semantic_block_section_tag(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_block('content', 'section', None)
        self.assertEqual(result, '<section>content</section>')

    def test_render_semantic_block_main_tag(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_block('content', 'main', None)
        self.assertEqual(result, '<main>content</main>')

    def test_render_semantic_block_article_tag(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_block('content', 'article', None)
        self.assertEqual(result, '<article>content</article>')

    def test_render_semantic_block_role_not_duplicated_in_annotations(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_block(
            'content', 'proof', None, annotations=['proof', 'special']
        )
        self.assertEqual(result, '<div class="proof special">content</div>')

    def test_render_semantic_block_with_target_id(self):
        fr = HtmlFragmentRenderer()
        result = fr.render_semantic_block('content', 'proof', None, target_id='thm-1')
        self.assertEqual(result, '<div id="thm-1" class="proof">content</div>')


# ---------------------------------------------------------------------------
# Graphics rendering
# ---------------------------------------------------------------------------

class TestRenderGraphicsBlock(unittest.TestCase):

    def test_simple_src_url(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        result = fr.render_graphics_block(gr, None)
        # generate_open_tag without self_close_tag=True does not add self-close slash
        self.assertEqual(result, '<img src="image.png">')

    # Use of _fpt is necessary for tests with Transcrypt — image dimension exports as
    # "width:300pt" (JS) or "width:300.000000pt" (Py) are both acceptable.
    def _fpt(self, value):
        """Regex pattern matching a floating-point number equal to *value*."""
        int_part = int(value)
        if value == int_part:
            return str(int_part) + r'(\.0*)?'
        return re.escape(str(value))

    def test_with_raster_dimensions(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        gr.physical_dimensions = (100.0, 50.0)
        gr.graphics_type = 'raster'
        result = fr.render_graphics_block(gr, None)
        self.assertIsNotNone(re.search('^' +
            r'<img style="width:' + self._fpt(100) + r'pt;height:' + self._fpt(50) + r'pt" src="image\.png">' + '$',
            result
        ))

    def test_with_vector_dimensions(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        gr.physical_dimensions = (80.0, 40.0)
        gr.graphics_type = 'vector'
        result = fr.render_graphics_block(gr, None)
        self.assertIsNotNone(re.search('^' +
            r'<img style="width:' + self._fpt(80) + r'pt;height:' + self._fpt(40) + r'pt" src="image\.png">' + '$',
            result
        ))

    def test_raster_magnification(self):
        fr = HtmlFragmentRenderer()
        fr.graphics_raster_magnification = 2
        gr = _MockGraphicsResource()
        gr.physical_dimensions = (100.0, 50.0)
        gr.graphics_type = 'raster'
        result = fr.render_graphics_block(gr, None)
        self.assertIsNotNone(re.search('^' +
            r'<img style="width:' + self._fpt(200) + r'pt;height:' + self._fpt(100) + r'pt" src="image\.png">' + '$',
            result
        ))

    def test_vector_magnification(self):
        fr = HtmlFragmentRenderer()
        fr.graphics_vector_magnification = 0.5
        gr = _MockGraphicsResource()
        gr.physical_dimensions = (100.0, 50.0)
        gr.graphics_type = 'vector'
        result = fr.render_graphics_block(gr, None)
        self.assertIsNotNone(re.search('^' +
            r'<img style="width:' + self._fpt(50) + r'pt;height:' + self._fpt(25) + r'pt" src="image\.png">' + '$',
            result
        ))

    def test_only_width_no_height(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        gr.physical_dimensions = (200.0, None)
        gr.graphics_type = 'raster'
        result = fr.render_graphics_block(gr, None)
        self.assertIsNotNone(re.search('^' +
            r'<img style="width:' + self._fpt(200) + r'pt" src="image\.png">' + '$',
            result
        ))

    def test_srcset(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        gr.srcset = [
            {'source': 'image.png', 'pixel_density': 1},
            {'source': 'image@2x.png', 'pixel_density': 2},
        ]
        result = fr.render_graphics_block(gr, None)
        self.assertEqual(
            result,
            '<img src="image.png" srcset="image.png 1x, image@2x.png 2x">'
        )

    def test_srcset_source_only(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        gr.srcset = [{'source': 'image.png'}]
        result = fr.render_graphics_block(gr, None)
        self.assertEqual(result, '<img src="image.png" srcset="image.png">')

    def test_no_dimensions_no_style(self):
        fr = HtmlFragmentRenderer()
        gr = _MockGraphicsResource()
        gr.physical_dimensions = None
        result = fr.render_graphics_block(gr, None)
        self.assertEqual(result, '<img src="image.png">')


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

class TestConfiguration(unittest.TestCase):

    def test_default_attributes(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.use_link_target_blank, False)
        self.assertEqual(fr.html_blocks_joiner, '\n')
        self.assertEqual(fr.supports_delayed_render_markers, True)
        self.assertEqual(fr.use_mathjax, True)
        self.assertEqual(fr.use_standard_math_delimiters, True)
        self.assertEqual(fr.aggressively_escape_html_attributes, False)
        self.assertEqual(fr.render_nothing_as_comment_with_annotations, True)
        self.assertEqual(fr.render_links_with_empty_href, False)
        self.assertEqual(fr.verbatim_protect_backslashes, True)
        self.assertEqual(fr.verbatim_highlight_spaces, False)
        self.assertEqual(fr.inline_heading_add_space, True)

    def test_config_via_constructor(self):
        fr = HtmlFragmentRenderer(config={
            'use_link_target_blank': True,
            'html_blocks_joiner': '',
            'use_mathjax': False,
        })
        self.assertEqual(fr.use_link_target_blank, True)
        self.assertEqual(fr.html_blocks_joiner, '')
        self.assertEqual(fr.use_mathjax, False)

    def test_heading_tags_by_level(self):
        fr = HtmlFragmentRenderer()
        self.assertEqual(fr.heading_tags_by_level[1], 'h1')
        self.assertEqual(fr.heading_tags_by_level[2], 'h2')
        self.assertEqual(fr.heading_tags_by_level[3], 'h3')
        self.assertEqual(fr.heading_tags_by_level[4], 'span')
        self.assertEqual(fr.heading_tags_by_level['theorem'], 'span')


# ---------------------------------------------------------------------------
# Tests requiring a FLM environment
# ---------------------------------------------------------------------------

class TestRenderWithEnviron(unittest.TestCase):

    maxDiff = None

    # --- basic text ---

    def test_render_plain_text(self):
        self.assertEqual(_render_standalone('Hello world'), 'Hello world')

    def test_render_html_chars_escaped(self):
        self.assertEqual(
            _render_standalone('a < b & c > d'),
            'a &lt; b &amp; c &gt; d'
        )

    def test_render_two_paragraphs(self):
        result = _render_block('First paragraph.\n\nSecond paragraph.')
        self.assertEqual(result, '<p>First paragraph.</p>\n<p>Second paragraph.</p>')

    def test_render_single_paragraph(self):
        # Single paragraph without explicit breaks renders inline (no <p> wrapper)
        result = _render_block('Only one paragraph.')
        self.assertEqual(result, 'Only one paragraph.')

    # --- text formatting ---

    def test_textbf(self):
        self.assertEqual(
            _render_standalone(r'\textbf{bold text}'),
            '<span class="textbf">bold text</span>'
        )

    def test_textit(self):
        self.assertEqual(
            _render_standalone(r'\textit{italic text}'),
            '<span class="textit">italic text</span>'
        )

    def test_emph(self):
        self.assertEqual(
            _render_standalone(r'\emph{emphasized}'),
            '<span class="textit">emphasized</span>'
        )

    # --- math ---

    def test_inline_math(self):
        # FLM uses \(...\) for inline math (dollar signs are forbidden)
        result = _render_standalone(r'\(x^2\)')
        self.assertEqual(result, r'<span class="inline-math">\(x^2\)</span>')

    def test_display_math(self):
        result = _render_block(r'\[x^2\]')
        self.assertEqual(result, r'<span class="display-math">\[x^2\]</span>')

    def test_math_non_standard_delimiters(self):
        # Call render_math_content directly to test non-standard delimiter passthrough
        fr = HtmlFragmentRenderer()
        fr.use_standard_math_delimiters = False
        nodelist = _make_nodelist('x^2')
        result = fr.render_math_content(
            (r'$$', r'$$'), nodelist, None, 'inline'
        )
        # Should keep the provided delimiters instead of overriding with \( \)
        self.assertEqual(result, r'<span class="inline-math">$$x^2$$</span>')

    def test_display_math_environment(self):
        result = _render_block(r'\begin{align}x &= y\end{align}')
        self.assertEqual(
            result,
            r'<span id="equation-1" class="display-math env-align">'
            r'\begin{align}x &amp;= y\tag*{(1)}\end{align}</span>'
        )

    # --- headings ---

    def test_render_heading_h1(self):
        result = _render_block(r'\section{Introduction}')
        self.assertEqual(
            result,
            '<h1 id="sec--Introduction" class="heading-level-1">Introduction</h1>'
        )

    def test_render_heading_h2(self):
        result = _render_block(r'\subsection{Background}')
        self.assertEqual(
            result,
            '<h2 id="sec--Background" class="heading-level-2">Background</h2>'
        )

    def test_render_heading_h3(self):
        result = _render_block(r'\subsubsection{Details}')
        self.assertEqual(
            result,
            '<h3 id="sec--Details" class="heading-level-3">Details</h3>'
        )

    def test_render_heading_paragraph_inline(self):
        result = _render_block(r'\paragraph{Note} Some content.')
        self.assertEqual(
            result,
            '<p><span id="sec--Note" class="heading-level-4 heading-inline">Note</span>'
            ' Some content.</p>'
        )

    def test_render_heading_invalid_level_raises(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('text')
        self.assertRaises(
            ValueError,
            fr.render_heading, nodelist, None, heading_level=99
        )

    def test_render_heading_inline_adds_space(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('Note')
        result = fr.render_heading(nodelist, None, heading_level=4, inline_heading=True)
        self.assertEqual(result, '<span class="heading-level-4 heading-inline">Note</span> ')

    def test_render_heading_inline_no_space(self):
        fr = HtmlFragmentRenderer()
        fr.inline_heading_add_space = False
        nodelist = _make_nodelist('Note')
        result = fr.render_heading(nodelist, None, heading_level=4, inline_heading=True)
        self.assertEqual(result, '<span class="heading-level-4 heading-inline">Note</span>')

    def test_render_heading_with_target_id(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('My Section')
        result = fr.render_heading(nodelist, None, heading_level=1, target_id='my-sec')
        self.assertEqual(result, '<h1 id="my-sec" class="heading-level-1">My Section</h1>')

    def test_render_heading_theorem_level(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('Theorem')
        result = fr.render_heading(nodelist, None, heading_level='theorem')
        self.assertEqual(result, '<span class="heading-level-theorem">Theorem</span>')

    def test_render_heading_with_annotations(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('Title')
        result = fr.render_heading(
            nodelist, None, heading_level=1, annotations=['numbered']
        )
        self.assertEqual(result, '<h1 class="numbered heading-level-1">Title</h1>')

    # --- links ---

    def test_render_link_external(self):
        result = _render_standalone(r'\href{https://example.com}{click here}')
        self.assertEqual(result, '<a href="https://example.com" class="href-href">click here</a>')

    def test_render_link_empty_href_no_anchor(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('display text')
        result = fr.render_link('href', '', nodelist, None)
        # With render_links_with_empty_href=False, no link wrapping
        self.assertEqual(result, 'display text')

    def test_render_link_empty_href_with_flag(self):
        fr = HtmlFragmentRenderer()
        fr.render_links_with_empty_href = True
        nodelist = _make_nodelist('display text')
        result = fr.render_link('href', '', nodelist, None)
        self.assertEqual(result, '<a href="#" class="href-href">display text</a>')

    def test_render_link_ref_type_in_class(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('text')
        result = fr.render_link('href', 'https://x.com', nodelist, None)
        self.assertEqual(result, '<a href="https://x.com" class="href-href">text</a>')

    def test_render_link_with_annotations(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('text')
        result = fr.render_link('href', '#x', nodelist, None, annotations=['myannot'])
        self.assertEqual(result, '<a href="#x" class="href-href myannot">text</a>')

    # --- annotations ---

    def test_render_annotation_comment_inline(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('a note')
        result = fr.render_annotation_comment(nodelist, None, is_block_level=False)
        self.assertEqual(
            result,
            '<span class="annotation annotation-comment annotation-0">a note</span>'
        )

    def test_render_annotation_comment_block(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('a note')
        result = fr.render_annotation_comment(nodelist, None, is_block_level=True)
        self.assertEqual(
            result,
            '<div class="annotation annotation-comment annotation-0">a note</div>'
        )

    def test_render_annotation_comment_with_initials(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('a note')
        result = fr.render_annotation_comment(nodelist, None, initials='AB')
        self.assertEqual(
            result,
            '<span class="annotation annotation-comment annotation-0">'
            '<span class="annotation-initials">AB</span>a note</span>'
        )

    def test_render_annotation_comment_color_index(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('a note')
        result = fr.render_annotation_comment(nodelist, None, color_index=3)
        self.assertEqual(
            result,
            '<span class="annotation annotation-comment annotation-3">a note</span>'
        )

    def test_render_annotation_highlight_inline(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('highlighted text')
        result = fr.render_annotation_highlight(nodelist, None, is_block_level=False)
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-0">highlighted text</span>'
        )

    def test_render_annotation_highlight_block(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('highlighted text')
        result = fr.render_annotation_highlight(nodelist, None, is_block_level=True)
        self.assertEqual(
            result,
            '<div class="annotation annotation-highlight annotation-0">highlighted text</div>'
        )

    def test_render_annotation_highlight_with_initials(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('highlighted text')
        result = fr.render_annotation_highlight(nodelist, None, initials='XY')
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-0">'
            '<span class="annotation-initials">XY</span>highlighted text</span>'
        )

    def test_render_annotation_highlight_color_index(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('highlighted')
        result = fr.render_annotation_highlight(nodelist, None, color_index=1)
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-1">highlighted</span>'
        )

    # --- text_format via renderer directly ---

    def test_render_text_format_single(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('bold text')
        result = fr.render_text_format(['textbf'], nodelist, None)
        self.assertEqual(result, '<span class="textbf">bold text</span>')

    def test_render_text_format_multiple(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('text')
        result = fr.render_text_format(['textbf', 'textit'], nodelist, None)
        self.assertEqual(result, '<span class="textbf textit">text</span>')

    # --- lines ---

    def test_render_lines_basic(self):
        fr = HtmlFragmentRenderer()
        nl1 = _make_nodelist('Line one')
        nl2 = _make_nodelist('Line two')
        lines = [_MockLineInfo(nl1), _MockLineInfo(nl2)]
        result = fr.render_lines(lines, None)
        self.assertEqual(
            result,
            '<p class="lines"><span>Line one</span><br><span>Line two</span></p>'
        )

    def test_render_lines_single_no_br(self):
        fr = HtmlFragmentRenderer()
        nl = _make_nodelist('Only line')
        lines = [_MockLineInfo(nl)]
        result = fr.render_lines(lines, None)
        self.assertEqual(result, '<p class="lines"><span>Only line</span></p>')

    def test_render_lines_with_indent_left(self):
        fr = HtmlFragmentRenderer()
        nl = _make_nodelist('Indented')
        lines = [_MockLineInfo(nl, indent_left=2)]
        result = fr.render_lines(lines, None)
        self.assertEqual(
            result,
            '<p class="lines"><span>'
            '<span class="quote-lines-indent"></span>'
            '<span class="quote-lines-indent"></span>'
            'Indented</span></p>'
        )

    def test_render_lines_with_role(self):
        fr = HtmlFragmentRenderer()
        nl = _make_nodelist('Line')
        lines = [_MockLineInfo(nl)]
        result = fr.render_lines(lines, None, role='quote-lines')
        self.assertEqual(result, '<p class="lines quote-lines"><span>Line</span></p>')

    def test_render_lines_with_target_id(self):
        fr = HtmlFragmentRenderer()
        nl = _make_nodelist('Line')
        lines = [_MockLineInfo(nl)]
        result = fr.render_lines(lines, None, target_id='poem-1')
        self.assertEqual(result, '<p id="poem-1" class="lines"><span>Line</span></p>')

    def test_render_lines_no_br_flag(self):
        fr = HtmlFragmentRenderer()
        fr.lines_use_br = False
        nl1 = _make_nodelist('Line one')
        nl2 = _make_nodelist('Line two')
        lines = [_MockLineInfo(nl1), _MockLineInfo(nl2)]
        result = fr.render_lines(lines, None)
        self.assertEqual(
            result,
            '<p class="lines"><span>Line one</span><span>Line two</span></p>'
        )

    def test_render_lines_no_span_flag(self):
        fr = HtmlFragmentRenderer()
        fr.lines_use_line_span = False
        fr.lines_use_br = False
        nl = _make_nodelist('Line')
        lines = [_MockLineInfo(nl)]
        result = fr.render_lines(lines, None)
        self.assertEqual(result, '<p class="lines">Line</p>')

    def test_render_lines_role_annotation_not_duplicated(self):
        fr = HtmlFragmentRenderer()
        nl = _make_nodelist('Line')
        lines = [_MockLineInfo(nl)]
        result = fr.render_lines(lines, None, role='quote-lines',
                                 annotations=['quote-lines'])
        # 'quote-lines' should appear once, not twice, since it's both role and annotation
        self.assertEqual(result, '<p class="lines quote-lines"><span>Line</span></p>')

    # --- enumeration ---

    def test_render_enumeration(self):
        result = _render_block(
            r'\begin{enumerate}\item First\item Second\end{enumerate}'
        )
        self.assertEqual(
            result,
            '<dl class="enumeration enumerate">'
            '<dt>1.</dt><dd><p>First</p></dd>'
            '<dt>2.</dt><dd><p>Second</p></dd>'
            '</dl>'
        )

    def test_render_itemize(self):
        result = _render_block(
            r'\begin{itemize}\item Alpha\item Beta\end{itemize}'
        )
        self.assertEqual(
            result,
            '<dl class="enumeration itemize">'
            '<dt>\u2022</dt><dd><p>Alpha</p></dd>'
            '<dt>\u2022</dt><dd><p>Beta</p></dd>'
            '</dl>'
        )

    # --- paragraph building ---

    def test_render_build_paragraph(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('Some content')
        result = fr.render_build_paragraph(nodelist, None)
        self.assertEqual(result, '<p>Some content</p>')

    # --- verbatim via FLM ---

    def test_verbatim_code_macro(self):
        # FLM uses \verbcode{...} (not \verb|...|)
        result = _render_standalone(r'\verbcode{hello world}')
        self.assertEqual(
            result,
            '<span class="verbatimcode verbatimcode-inline">hello world</span>'
        )


# ---------------------------------------------------------------------------
# Math rendering (direct method calls)
# ---------------------------------------------------------------------------

class TestRenderMathContent(unittest.TestCase):

    maxDiff = None

    def test_inline_math_with_target_id(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('x^2')
        result = fr.render_math_content(
            (r'\(', r'\)'), nodelist, None, 'inline', target_id='eq-1'
        )
        self.assertEqual(
            result,
            '<span id="eq-1" class="inline-math">\\(x^2\\)</span>'
        )

    def test_display_math_with_environment_name(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('x^2')
        result = fr.render_math_content(
            (r'\[', r'\]'), nodelist, None, 'display', environmentname='align'
        )
        self.assertEqual(
            result,
            '<span class="display-math env-align">'
            r'\begin{align}x^2\end{align}</span>'
        )

    def test_display_math_with_target_id(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('x^2')
        result = fr.render_math_content(
            (r'\[', r'\]'), nodelist, None, 'display', target_id='eq-2'
        )
        self.assertEqual(
            result,
            '<span id="eq-2" class="display-math">\\[x^2\\]</span>'
        )

    def test_display_math_starred_environment(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('a = b')
        result = fr.render_math_content(
            (r'\[', r'\]'), nodelist, None, 'display', environmentname='align*'
        )
        self.assertEqual(
            result,
            '<span class="display-math env-align-star">'
            r'\begin{align*}a = b\end{align*}</span>'
        )

    def test_invalid_displaytype_raises(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('x')
        self.assertRaises(
            ValueError,
            fr.render_math_content,
            (r'\(', r'\)'), nodelist, None, 'block'
        )

    def test_non_standard_delimiters_display(self):
        fr = HtmlFragmentRenderer()
        fr.use_standard_math_delimiters = False
        nodelist = _make_nodelist('x^2')
        result = fr.render_math_content(
            ('$$', '$$'), nodelist, None, 'display'
        )
        self.assertEqual(
            result,
            '<span class="display-math">$$x^2$$</span>'
        )


# ---------------------------------------------------------------------------
# Float rendering via FLM
# ---------------------------------------------------------------------------

class TestRenderFloat(unittest.TestCase):

    maxDiff = None

    def test_float_with_caption(self):
        result = _render_block(
            r'\begin{figure}\includegraphics{image.png}\caption{A figure}\end{figure}'
        )
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents"><img src="image.png"></div>\n'
            '<figcaption class="float-caption-content"><span>'
            '<span class="float-no-number">Figure</span>: A figure'
            '</span></figcaption></figure>'
        )

    def test_float_no_caption(self):
        result = _render_block(
            r'\begin{figure}\includegraphics{image.png}\end{figure}'
        )
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents"><img src="image.png"></div>'
            '</figure>'
        )


# ---------------------------------------------------------------------------
# Cells rendering via FLM
# ---------------------------------------------------------------------------

class TestRenderCells(unittest.TestCase):

    maxDiff = None

    def test_simple_cells_table(self):
        features = standard_features()
        features.append(FeatureCells())
        environ = make_standard_environment(features)
        frag = environ.make_fragment(
            r'\begin{cells}\celldata<H>{A & B}\celldata{1 & 2}\end{cells}'
        )
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        # Non-header cells may or may not include an empty "cellstyle-" class
        # depending on Python vs JS transpilation, so match with regex.
        self.assertIsNotNone(re.search('^' +
            r'<table class="cells">'
            r'<tr>'
            r'<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>A</p></th>'
            r'<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>B</p></th>'
            r'</tr>'
            r'<tr>'
            r'<td class="cell (cellstyle- )?celltbledge-left celltbledge-bottom"><p>1</p></td>'
            r'<td class="cell (cellstyle- )?celltbledge-bottom celltbledge-right"><p>2</p></td>'
            r'</tr>'
            r'</table>' + '$',
            result
        ))


# ---------------------------------------------------------------------------
# include_node_data_attrs_fn
# ---------------------------------------------------------------------------

class TestNodeDataAttrs(unittest.TestCase):

    maxDiff = None

    def test_build_paragraph_with_data_attrs(self):
        fr = HtmlFragmentRenderer()
        fr.include_node_data_attrs_fn = lambda node, **kw: {'test': 'val'}
        nodelist = _make_nodelist('Hello world')
        result = fr.render_build_paragraph(nodelist, None)
        self.assertEqual(result, '<p data-test="val">Hello world</p>')

    def test_build_paragraph_without_data_attrs_fn(self):
        fr = HtmlFragmentRenderer()
        nodelist = _make_nodelist('Hello world')
        result = fr.render_build_paragraph(nodelist, None)
        self.assertEqual(result, '<p>Hello world</p>')

    def test_data_attrs_fn_returns_none(self):
        fr = HtmlFragmentRenderer()
        fr.include_node_data_attrs_fn = lambda node, **kw: None
        nodelist = _make_nodelist('Hello')
        result = fr.render_build_paragraph(nodelist, None)
        self.assertEqual(result, '<p>Hello</p>')


# ---------------------------------------------------------------------------
# Style info functions and FragmentRendererInformation
# ---------------------------------------------------------------------------

class TestStyleInfo(unittest.TestCase):

    def test_get_html_css_global_is_nonempty_string(self):
        fr = HtmlFragmentRenderer()
        result = get_html_css_global(fr)
        self.assertTrue(len(result) > 0)

    def test_get_html_css_content_is_nonempty_string(self):
        fr = HtmlFragmentRenderer()
        result = get_html_css_content(fr)
        self.assertTrue(len(result) > 0)

    def test_get_html_js_with_mathjax(self):
        fr = HtmlFragmentRenderer()
        fr.use_mathjax = True
        result = get_html_js(fr)
        self.assertTrue('MathJax' in result)

    def test_get_html_js_without_mathjax(self):
        fr = HtmlFragmentRenderer()
        fr.use_mathjax = False
        result = get_html_js(fr)
        self.assertEqual(result, '')

    def test_get_html_body_end_js_with_mathjax(self):
        fr = HtmlFragmentRenderer()
        fr.use_mathjax = True
        result = get_html_body_end_js_scripts(fr)
        self.assertTrue('script' in result)

    def test_get_html_body_end_js_without_mathjax(self):
        fr = HtmlFragmentRenderer()
        fr.use_mathjax = False
        result = get_html_body_end_js_scripts(fr)
        self.assertEqual(result, '')

    def test_fragment_renderer_information_class(self):
        self.assertIs(
            FragmentRendererInformation.FragmentRendererClass, HtmlFragmentRenderer
        )

    def test_fragment_renderer_information_format_name(self):
        self.assertEqual(FragmentRendererInformation.format_name, 'html')

    def test_fragment_renderer_information_get_style_information_keys(self):
        fr = HtmlFragmentRenderer()
        info = FragmentRendererInformation.get_style_information(fr)
        self.assertEqual(
            sorted(info.keys()),
            ['body_end_js_scripts', 'css_content', 'css_global', 'js']
        )

    def test_css_content_contains_enumeration_styles(self):
        fr = HtmlFragmentRenderer()
        css = get_html_css_content(fr)
        self.assertTrue('enumeration' in css)

    def test_css_content_contains_heading_styles(self):
        fr = HtmlFragmentRenderer()
        css = get_html_css_content(fr)
        self.assertTrue('heading-level' in css)

    def test_css_global_contains_link_styles(self):
        fr = HtmlFragmentRenderer()
        css = get_html_css_global(fr)
        self.assertTrue('a:link' in css)

    def test_css_content_contains_display_math_styles(self):
        fr = HtmlFragmentRenderer()
        css = get_html_css_content(fr)
        self.assertTrue('.display-math' in css)


if __name__ == '__main__':
    unittest.main()
