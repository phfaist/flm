import unittest

from flm.fragmentrenderer.text import (
    TextFragmentRenderer,
    FragmentRendererInformation,
    _add_punct,
)
from flm.feature.graphics import GraphicsResource
from flm.feature.cells import FeatureCells
from flm.feature.theorems import FeatureTheorems
from flm.stdfeatures import standard_features
from flm.flmenvironment import make_standard_environment


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr=None):
    if fr is None:
        fr = TextFragmentRenderer()
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    result, _ = doc.render(fr)
    return result


# ---- Init / Config ----

class TestTextFragmentRendererInit(unittest.TestCase):

    def test_default_init(self):
        fr = TextFragmentRenderer()
        self.assertFalse(fr.supports_delayed_render_markers)
        self.assertTrue(fr.display_href_urls)
        self.assertEqual(fr.float_separator_top, '\u00b7' * 80)
        self.assertEqual(fr.float_separator_bottom, '\u00b7' * 80)
        self.assertEqual(fr.float_caption_title_separator, ': ')
        self.assertEqual(fr.cells_column_sep, '   ')

    def test_custom_config(self):
        fr = TextFragmentRenderer(config={
            'display_href_urls': False,
            'float_separator_top': '---',
            'float_separator_bottom': '===',
            'float_caption_title_separator': '. ',
            'cells_column_sep': ' | ',
        })
        self.assertFalse(fr.display_href_urls)
        self.assertEqual(fr.float_separator_top, '---')
        self.assertEqual(fr.float_separator_bottom, '===')
        self.assertEqual(fr.float_caption_title_separator, '. ')
        self.assertEqual(fr.cells_column_sep, ' | ')

    def test_heading_level_formatter_keys(self):
        fr = TextFragmentRenderer()
        fmt = dict(fr.heading_level_formatter)
        self.assertTrue(1 in fmt)
        self.assertTrue(2 in fmt)
        self.assertTrue(3 in fmt)
        self.assertTrue(4 in fmt)
        self.assertTrue(5 in fmt)
        self.assertTrue(6 in fmt)
        self.assertTrue('theorem' in fmt)


# ---- _add_punct helper ----

class TestAddPunct(unittest.TestCase):

    def test_adds_colon_when_no_punct(self):
        self.assertEqual(_add_punct('Hello', ':'), 'Hello:')

    def test_no_add_when_ends_with_period(self):
        self.assertEqual(_add_punct('Hello.', ':'), 'Hello.')

    def test_no_add_when_ends_with_colon(self):
        self.assertEqual(_add_punct('Hello:', ':'), 'Hello:')

    def test_no_add_when_ends_with_comma(self):
        self.assertEqual(_add_punct('Hello,', ':'), 'Hello,')

    def test_no_add_when_ends_with_semicolon(self):
        self.assertEqual(_add_punct('Hello;', ':'), 'Hello;')

    def test_no_add_when_ends_with_question(self):
        self.assertEqual(_add_punct('Hello?', ':'), 'Hello?')

    def test_no_add_when_ends_with_exclamation(self):
        self.assertEqual(_add_punct('Hello!', ':'), 'Hello!')

    def test_strips_trailing_whitespace_for_check(self):
        self.assertEqual(_add_punct('Hello.  ', ':'), 'Hello.  ')

    def test_converts_to_string(self):
        self.assertEqual(_add_punct(42, ':'), '42:')

    def test_empty_string(self):
        # '' in '.,:;?!' is True in Python (empty string is always a substring),
        # so _add_punct returns '' unchanged
        self.assertEqual(_add_punct('', ':'), '')


# ---- Heading Level Formatter ----

class TestHeadingLevelFormatter(unittest.TestCase):

    def test_level_1_underline_equals(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[1]('Title')
        self.assertEqual(result, 'Title\n=====')

    def test_level_2_underline_dashes(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[2]('Subtitle')
        self.assertEqual(result, 'Subtitle\n--------')

    def test_level_3_underline_tildes(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[3]('Sub-subtitle')
        self.assertEqual(result, 'Sub-subtitle\n~~~~~~~~~~~~')

    def test_level_4_inline_colon(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[4]('My Paragraph')
        self.assertEqual(result, 'My Paragraph:  ')

    def test_level_5_indent_colon(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[5]('My Subparagraph')
        self.assertEqual(result, '    My Subparagraph:  ')

    def test_level_6_double_indent_colon(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[6]('Deep Level')
        self.assertEqual(result, '        Deep Level:  ')

    def test_theorem_level(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter['theorem']('Theorem 1')
        self.assertEqual(result, 'Theorem 1.  ')

    def test_level_4_already_punctuated(self):
        fr = TextFragmentRenderer()
        result = fr.heading_level_formatter[4]('Already?')
        self.assertEqual(result, 'Already?  ')


# ---- Basic render methods ----

class TestTextFragmentRendererBasicMethods(unittest.TestCase):

    def test_render_value(self):
        fr = TextFragmentRenderer()
        self.assertEqual(fr.render_value('hello world', None), 'hello world')

    def test_render_nothing(self):
        fr = TextFragmentRenderer()
        self.assertEqual(fr.render_nothing(None), '')

    def test_render_nothing_with_annotations(self):
        fr = TextFragmentRenderer()
        self.assertEqual(fr.render_nothing(None, annotations=['ann1']), '')

    def test_render_empty_error_placeholder(self):
        fr = TextFragmentRenderer()
        self.assertEqual(fr.render_empty_error_placeholder('debug info', None), '')

    def test_render_delayed_marker(self):
        fr = TextFragmentRenderer()
        self.assertEqual(fr.render_delayed_marker(None, 'key1', None), '')

    def test_render_delayed_dummy_placeholder(self):
        fr = TextFragmentRenderer()
        self.assertEqual(
            fr.render_delayed_dummy_placeholder(None, 'key1', None),
            '#DELAYED#'
        )

    def test_render_verbatim_inline(self):
        fr = TextFragmentRenderer()
        self.assertEqual(fr.render_verbatim('x+y', None, is_block_level=False), 'x+y')

    def test_render_verbatim_block(self):
        fr = TextFragmentRenderer()
        self.assertEqual(
            fr.render_verbatim('some code', None, is_block_level=True),
            'some code'
        )


# ---- Graphics ----

class TestTextFragmentRendererGraphics(unittest.TestCase):

    def test_render_graphics_block_centered(self):
        fr = TextFragmentRenderer()
        gr = GraphicsResource(src_url='myimage.png')
        result = fr.render_graphics_block(gr, None)
        self.assertEqual(
            result,
            '                                 [myimage.png]                                  '
        )
        self.assertEqual(len(result), 80)

    def test_render_graphics_block_short_name(self):
        fr = TextFragmentRenderer()
        gr = GraphicsResource(src_url='a.png')
        result = fr.render_graphics_block(gr, None)
        self.assertEqual(
            result,
            '                                    [a.png]                                     '
        )
        self.assertEqual(len(result), 80)


# ---- Integration: text format (no-op) ----

class TestTextFragmentRendererTextFormat(unittest.TestCase):

    maxDiff = None

    def test_textbf_passthrough(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\textbf{Hello}', standalone_mode=True)
        result = frag.render_standalone(TextFragmentRenderer())
        self.assertEqual(result, 'Hello')

    def test_textit_passthrough(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\textit{World}', standalone_mode=True)
        result = frag.render_standalone(TextFragmentRenderer())
        self.assertEqual(result, 'World')

    def test_nested_format_passthrough(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\textbf{\textit{Nested}}', standalone_mode=True)
        result = frag.render_standalone(TextFragmentRenderer())
        self.assertEqual(result, 'Nested')


# ---- Integration: headings ----

class TestTextFragmentRendererHeadings(unittest.TestCase):

    maxDiff = None

    def test_section_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\section{My Section}')
        self.assertEqual(result, 'My Section\n==========')

    def test_subsection_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\subsection{My Subsection}')
        self.assertEqual(result, 'My Subsection\n-------------')

    def test_subsubsection_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\subsubsection{My Subsubsection}')
        self.assertEqual(result, 'My Subsubsection\n~~~~~~~~~~~~~~~~')

    def test_paragraph_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\paragraph{My Paragraph}')
        self.assertEqual(result, 'My Paragraph:  ')

    def test_subparagraph_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\subparagraph{Level Five}')
        self.assertEqual(result, '    Level Five:  ')

    def test_numbered_sections(self):
        environ = mk_flm_environ(
            headings={'section': True, 'subsection': True}
        )
        result = render_doc(environ, r'''\section{Introduction}

Some text here.

\section{Methods}

More text.''')
        self.assertEqual(
            result,
            'Introduction\n============\n\nSome text here.\n\n'
            'Methods\n=======\n\nMore text.'
        )


# ---- Integration: links ----

class TestTextFragmentRendererLinks(unittest.TestCase):

    maxDiff = None

    def test_href_shows_url(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\href{https://example.com}{Click here}')
        self.assertEqual(result, 'Click here <https://example.com>')

    def test_url_shows_url(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\url{https://example.com}')
        self.assertEqual(result, 'example.com <https://example.com>')

    def test_href_no_display_urls(self):
        environ = mk_flm_environ()
        fr = TextFragmentRenderer(config={'display_href_urls': False})
        result = render_doc(environ, r'\href{https://example.com}{Click here}', fr=fr)
        self.assertEqual(result, 'Click here')

    def test_email_link(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\email{test@example.com}')
        self.assertEqual(result, 'test@example.com <mailto:test@example.com>')


# ---- Integration: enumeration ----

class TestTextFragmentRendererEnumeration(unittest.TestCase):

    maxDiff = None

    def test_itemize(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{itemize}
\item First item
\item Second item
\end{itemize}''')
        self.assertEqual(result, '  \u2022 First item\n\n  \u2022 Second item')

    def test_enumerate(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{enumerate}
\item Alpha
\item Beta
\item Gamma
\end{enumerate}''')
        self.assertEqual(result, '  1. Alpha\n\n  2. Beta\n\n  3. Gamma')

    def test_nested_enumeration(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{enumerate}
\item First
\item Second
  \begin{itemize}
  \item Sub A
  \item Sub B
  \end{itemize}
\end{enumerate}''')
        self.assertEqual(
            result,
            '  1. First\n\n  2. Second\n\n      - Sub A\n\n      - Sub B'
        )


# ---- Integration: math ----

class TestTextFragmentRendererMath(unittest.TestCase):

    maxDiff = None

    def test_inline_math(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'We have \( x^2 \) here', standalone_mode=True)
        result = frag.render_standalone(TextFragmentRenderer())
        self.assertEqual(result, 'We have \\( x^2 \\) here')

    def test_equation_environment(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{equation}
E = mc^2
\end{equation}''')
        self.assertEqual(
            result,
            '\\begin{equation}\nE = mc^2\n\\tag*{(1)}\\end{equation}'
        )


# ---- Integration: verbatim ----

class TestTextFragmentRendererVerbatim(unittest.TestCase):

    maxDiff = None

    def test_verbcode_inline(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\verbcode{x+y}', standalone_mode=True)
        result = frag.render_standalone(TextFragmentRenderer())
        self.assertEqual(result, 'x+y')

    def test_verbatimcode_environment(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''Some text.

\begin{verbatimcode}
x = 1
y = 2
\end{verbatimcode}''')
        self.assertEqual(result, 'Some text.\n\nx = 1\ny = 2\n')


# ---- Integration: paragraphs ----

class TestTextFragmentRendererParagraphs(unittest.TestCase):

    maxDiff = None

    def test_two_paragraphs(self):
        environ = mk_flm_environ()
        result = render_doc(environ, 'First paragraph.\n\nSecond paragraph.')
        self.assertEqual(result, 'First paragraph.\n\nSecond paragraph.')

    def test_single_paragraph(self):
        environ = mk_flm_environ()
        result = render_doc(environ, 'Just some simple text.')
        self.assertEqual(result, 'Just some simple text.')

    def test_unicode_text(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment('Hello \u00e9l\u00e8ve', standalone_mode=True)
        result = frag.render_standalone(TextFragmentRenderer())
        self.assertEqual(result, 'Hello \u00e9l\u00e8ve')


# ---- Integration: footnotes ----

class TestTextFragmentRendererFootnotes(unittest.TestCase):

    maxDiff = None

    def test_footnote_inline_marker(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'Some text\footnote{A note.} here.')
        self.assertEqual(result, 'Some texta here.')


# ---- Integration: defterm ----

class TestTextFragmentRendererDefterm(unittest.TestCase):

    maxDiff = None

    def test_defterm_simple(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{defterm}{Entropy}[label={term:entropy}]
A measure of disorder.
\end{defterm}''')
        self.assertEqual(
            result,
            'Entropy: [label=term:entropy] A measure of disorder.'
        )

    def test_defterm_with_ref_hides_internal_link(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{defterm}{Term}[label={term:myterm}]
A definition.
\end{defterm}

See \term{Term}.''')
        self.assertEqual(
            result,
            'Term: [label=term:myterm] A definition.\n\nSee Term.'
        )


# ---- Integration: floats ----

class TestTextFragmentRendererFloats(unittest.TestCase):

    maxDiff = None

    def test_figure_with_caption_and_label(self):
        environ = mk_flm_environ()
        fr = TextFragmentRenderer()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}'
            r'\caption{A nice figure.}\label{figure:test}\end{figure}',
            fr=fr,
        )
        sep = '\u00b7' * 80
        centered_img = fr.render_graphics_block(
            GraphicsResource(src_url='img/test.png'), None
        )
        self.assertEqual(
            result,
            sep + '\n' + centered_img + '\n\n'
            'Figure\u00a01: A nice figure.\n'
            + sep
        )

    def test_figure_bare(self):
        environ = mk_flm_environ()
        fr = TextFragmentRenderer()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}',
            fr=fr,
        )
        sep = '\u00b7' * 80
        centered_img = fr.render_graphics_block(
            GraphicsResource(src_url='img/test.png'), None
        )
        self.assertEqual(
            result,
            sep + '\n' + centered_img + '\n' + sep
        )

    def test_figure_caption_only_no_label(self):
        environ = mk_flm_environ()
        fr = TextFragmentRenderer()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}'
            r'\caption{Just caption}\end{figure}',
            fr=fr,
        )
        sep = '\u00b7' * 80
        centered_img = fr.render_graphics_block(
            GraphicsResource(src_url='img/test.png'), None
        )
        self.assertEqual(
            result,
            sep + '\n' + centered_img + '\n\n'
            'Figure: Just caption\n'
            + sep
        )

    def test_figure_custom_separators(self):
        environ = mk_flm_environ()
        fr = TextFragmentRenderer(config={
            'float_separator_top': '---',
            'float_separator_bottom': '===',
        })
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}',
            fr=fr,
        )
        centered_img = fr.render_graphics_block(
            GraphicsResource(src_url='img/test.png'), None
        )
        self.assertEqual(
            result,
            '---\n' + centered_img + '\n==='
        )


# ---- Integration: theorem ----

class TestTextFragmentRendererTheorem(unittest.TestCase):

    maxDiff = None

    def test_theorem_with_title(self):
        features = standard_features()
        features.append(FeatureTheorems())
        environ = make_standard_environment(features)
        result = render_doc(environ, r'''\begin{theorem}[title={Important result}]
Some theorem content.
\end{theorem}''')
        self.assertEqual(
            result,
            'Theorem\u00a01 (title=Important result).  Some theorem content.'
        )


# ---- Integration: cells (smoke test) ----

class TestTextFragmentRendererCells(unittest.TestCase):

    maxDiff = None

    def test_celldata_does_not_error(self):
        features = standard_features()
        features.append(FeatureCells())
        environ = make_standard_environment(features)
        src = (
            r'\begin{cells}'
            r'\celldata<H>{Name & City}'
            '\n'
            r'\celldata{John & Berlin}'
            '\n'
            r'\end{cells}'
        )
        result = render_doc(environ, src)
        self.assertEqual(result, '    Name\n    City\n    John\n    Berlin')


# ---- FragmentRendererInformation ----

class TestFragmentRendererInformation(unittest.TestCase):

    def test_class(self):
        self.assertIs(
            FragmentRendererInformation.FragmentRendererClass,
            TextFragmentRenderer,
        )

    def test_format_name(self):
        self.assertEqual(FragmentRendererInformation.format_name, 'text')


if __name__ == '__main__':
    unittest.main()
