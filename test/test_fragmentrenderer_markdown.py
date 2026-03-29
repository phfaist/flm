import unittest

from flm.fragmentrenderer.markdown import (
    MarkdownFragmentRenderer,
    FragmentRendererInformation,
    rx_mdspecials,
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
        fr = MarkdownFragmentRenderer()
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    result, _ = doc.render(fr)
    return result


# ---- Init / Config ----

class TestMarkdownFragmentRendererInit(unittest.TestCase):

    def test_default_init(self):
        fr = MarkdownFragmentRenderer()
        self.assertTrue(fr.supports_delayed_render_markers)
        self.assertEqual(fr.use_target_ids, 'anchor')
        # self.assertEqual(fr.graphics_raster_magnification, 1)
        # self.assertEqual(fr.graphics_vector_magnification, 1)

    def test_custom_config(self):
        fr = MarkdownFragmentRenderer(config={
            'use_target_ids': 'pandoc',
            'graphics_raster_magnification': 2,
        })
        self.assertEqual(fr.use_target_ids, 'pandoc')
        # self.assertEqual(fr.graphics_raster_magnification, 2)

    def test_heading_level_formatter_keys(self):
        fr = MarkdownFragmentRenderer()
        fmt = dict(fr.heading_level_formatter)
        self.assertTrue(1 in fmt)
        self.assertTrue(2 in fmt)
        self.assertTrue(3 in fmt)
        self.assertTrue(4 in fmt)
        self.assertTrue(5 in fmt)
        self.assertTrue(6 in fmt)
        self.assertTrue('theorem' in fmt)


# ---- Heading Level Formatter ----

class TestHeadingLevelFormatter(unittest.TestCase):

    def test_level_1(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter[1]('Title'), '# Title')

    def test_level_2(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter[2]('Sub'), '## Sub')

    def test_level_3(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter[3]('SubSub'), '### SubSub')

    def test_level_4(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter[4]('Para'), '#### Para')

    def test_level_5(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter[5]('SubPara'), '##### SubPara')

    def test_level_6(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter[6]('Deep'), '###### Deep')

    def test_theorem_level(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.heading_level_formatter['theorem']('Theorem 1'), 'Theorem 1.  ')


# ---- render_value (markdown escaping) ----

class TestMarkdownRenderValue(unittest.TestCase):

    def test_plain_text(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('hello', None), 'hello')

    def test_empty(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('', None), '')

    def test_asterisk(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a*b', None), 'a\\*b')

    def test_underscore(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('x_y', None), 'x\\_y')

    def test_brackets_parens(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a[b](c)', None), 'a\\[b\\]\\(c\\)')

    def test_backtick(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('code`here`', None), 'code\\`here\\`')

    def test_angle_brackets(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a<b>c', None), 'a\\<b\\>c')

    def test_hash(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('#heading', None), '\\#heading')

    def test_braces(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a{b}c', None), 'a\\{b\\}c')

    def test_tilde(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a~b', None), 'a\\~b')

    def test_plus(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a+b', None), 'a\\+b')

    def test_period(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a.b', None), 'a\\.b')

    def test_pipe(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a|b', None), 'a\\|b')

    def test_dash(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a-b', None), 'a\\-b')

    def test_backslash(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_value('a\\b', None), 'a\\\\b')


# ---- Basic render methods ----

class TestMarkdownBasicMethods(unittest.TestCase):

    def test_render_empty_error_placeholder(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_empty_error_placeholder('debug', None), '')

    def test_render_nothing(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_nothing(None), '')

    def test_render_nothing_with_annotations(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_nothing(None, annotations=['a']), '')

    def test_render_verbatim_inline(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_verbatim('x+y', None), '`` x\\+y ``')

    def test_render_verbatim_double_backtick(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(
            fr.render_verbatim('has `` backticks', None),
            '`` has \\` \\`  backticks ``'
        )

    def test_render_verbatim_with_target_id(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(
            fr.render_verbatim('code', None, target_id='my-id'),
            '<a name="my-id"></a> `` code ``'
        )

    def test_render_delayed_marker(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_delayed_marker(None, 42, None), '<FLM:DLYD:42/>')

    def test_render_delayed_dummy_placeholder(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(
            fr.render_delayed_dummy_placeholder(None, 'key1', None),
            '<!-- delayed:key1 -->'
        )

    def test_replace_delayed_markers(self):
        fr = MarkdownFragmentRenderer()
        result = fr.replace_delayed_markers_with_final_values(
            'before <FLM:DLYD:0/> after <FLM:DLYD:1/> end',
            ['FIRST', 'SECOND']
        )
        self.assertEqual(result, 'before FIRST after SECOND end')


# ---- _get_target_id_md_code ----

class TestGetTargetIdMdCode(unittest.TestCase):

    def test_none_target_id(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr._get_target_id_md_code(None), '')

    def test_anchor_mode(self):
        fr = MarkdownFragmentRenderer()  # default 'anchor'
        self.assertEqual(fr._get_target_id_md_code('myid'), '<a name="myid"></a> ')

    def test_pandoc_mode(self):
        fr = MarkdownFragmentRenderer(config={'use_target_ids': 'pandoc'})
        self.assertEqual(fr._get_target_id_md_code('myid'), '[]{#myid} ')

    def test_github_mode(self):
        fr = MarkdownFragmentRenderer(config={'use_target_ids': 'github'})
        self.assertEqual(fr._get_target_id_md_code('myid'), '[](#myid) ')

    def test_disabled_mode(self):
        fr = MarkdownFragmentRenderer(config={'use_target_ids': None})
        self.assertEqual(fr._get_target_id_md_code('myid'), '')


# ---- render_join / render_join_blocks ----

class TestMarkdownJoinMethods(unittest.TestCase):

    def test_render_join(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_join(['a', 'b', 'c'], None), 'abc')

    def test_render_join_empty(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_join([], None), '')

    def test_render_join_blocks(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_join_blocks(['a', 'b', 'c'], None), 'a\n\nb\n\nc')

    def test_render_join_blocks_skips_empty(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_join_blocks(['a', '', 'c'], None), 'a\n\nc')

    def test_render_join_blocks_skips_none(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_join_blocks(['a', None, 'c'], None), 'a\n\nc')

    def test_render_join_blocks_empty_list(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(fr.render_join_blocks([], None), '')

    def test_render_join_blocks_collapses_newlines(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(
            fr.render_join_blocks(['a\n\n\n', '\n\nb'], None),
            'a\n\nb'
        )


# ---- render_semantic_block ----

class TestMarkdownSemanticBlock(unittest.TestCase):

    def test_with_target_id(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(
            fr.render_semantic_block('some content', 'section', None, target_id='my-target'),
            '<a name="my-target"></a>\nsome content'
        )

    def test_without_target_id(self):
        fr = MarkdownFragmentRenderer()
        self.assertEqual(
            fr.render_semantic_block('some content', 'section', None),
            '\nsome content'
        )


# ---- render_graphics_block ----

class TestMarkdownGraphics(unittest.TestCase):

    def test_render_graphics_block(self):
        fr = MarkdownFragmentRenderer()
        gr = GraphicsResource(src_url='img/test.png')
        self.assertEqual(fr.render_graphics_block(gr, None), '![](img/test.png)')

    def test_render_graphics_block_different_url(self):
        fr = MarkdownFragmentRenderer()
        gr = GraphicsResource(src_url='https://example.com/pic.jpg')
        self.assertEqual(fr.render_graphics_block(gr, None), '![](https://example.com/pic.jpg)')


# ---- Integration: text format ----

class TestMarkdownTextFormat(unittest.TestCase):

    maxDiff = None

    def test_textbf(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\textbf{Hello}', standalone_mode=True)
        result = frag.render_standalone(MarkdownFragmentRenderer())
        self.assertEqual(result, '**Hello**')

    def test_textit(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\textit{World}', standalone_mode=True)
        result = frag.render_standalone(MarkdownFragmentRenderer())
        self.assertEqual(result, '*World*')

    def test_emph(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\emph{emphasis}', standalone_mode=True)
        result = frag.render_standalone(MarkdownFragmentRenderer())
        self.assertEqual(result, '*emphasis*')

    def test_nested_bold_italic(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\textbf{\textit{Nested}}', standalone_mode=True)
        result = frag.render_standalone(MarkdownFragmentRenderer())
        self.assertEqual(result, '***Nested***')


# ---- Integration: headings ----

class TestMarkdownHeadings(unittest.TestCase):

    maxDiff = None

    def test_section_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\section{My Section}')
        self.assertEqual(
            result,
            '# <a name="sec--My-Section"></a> My Section'
        )

    def test_subsection_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\subsection{My Sub}')
        self.assertEqual(
            result,
            '## <a name="sec--My-Sub"></a> My Sub'
        )

    def test_subsubsection_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\subsubsection{My SubSub}')
        self.assertEqual(
            result,
            '### <a name="sec--My-SubSub"></a> My SubSub'
        )

    def test_paragraph_heading(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\paragraph{My Para}')
        self.assertEqual(
            result,
            '#### <a name="sec--My-Para"></a> My Para'
        )

    def test_heading_pandoc_target_ids(self):
        environ = mk_flm_environ()
        fr = MarkdownFragmentRenderer(config={'use_target_ids': 'pandoc'})
        result = render_doc(environ, r'\section{My Section}', fr=fr)
        self.assertEqual(result, '# []{#sec--My-Section} My Section')

    def test_heading_github_target_ids(self):
        environ = mk_flm_environ()
        fr = MarkdownFragmentRenderer(config={'use_target_ids': 'github'})
        result = render_doc(environ, r'\section{My Section}', fr=fr)
        self.assertEqual(result, '# [](#sec--My-Section) My Section')

    def test_heading_no_target_ids(self):
        environ = mk_flm_environ()
        fr = MarkdownFragmentRenderer(config={'use_target_ids': None})
        result = render_doc(environ, r'\section{My Section}', fr=fr)
        self.assertEqual(result, '# My Section')


# ---- Integration: paragraphs ----

class TestMarkdownParagraphs(unittest.TestCase):

    maxDiff = None

    def test_two_paragraphs(self):
        environ = mk_flm_environ()
        result = render_doc(environ, 'First paragraph.\n\nSecond paragraph.')
        self.assertEqual(result, 'First paragraph\\.\n\nSecond paragraph\\.')

    def test_single_paragraph(self):
        environ = mk_flm_environ()
        result = render_doc(environ, 'Just some text.')
        self.assertEqual(result, 'Just some text\\.')


# ---- Integration: links ----

class TestMarkdownLinks(unittest.TestCase):

    maxDiff = None

    def test_href(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\href{https://example.com}{Click here}')
        self.assertEqual(result, '[Click here](https://example.com)')

    def test_url(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\url{https://example.com}')
        self.assertEqual(result, '[example\\.com](https://example.com)')

    def test_email(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\email{test@example.com}')
        self.assertEqual(result, '[test@example\\.com](mailto:test@example.com)')


# ---- Integration: math ----

class TestMarkdownMath(unittest.TestCase):

    maxDiff = None

    def test_inline_math(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'We have \( x^2 \) here', standalone_mode=True)
        result = frag.render_standalone(MarkdownFragmentRenderer())
        self.assertEqual(result, 'We have \\\\\\( x^2 \\\\\\) here')

    def test_equation_environment(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\begin{equation}E = mc^2\end{equation}')
        self.assertEqual(
            result,
            '<a name="equation-1"></a> E = mc^2\\\\tag\\*\\{\\(1\\)\\}'
        )

    def test_equation_with_label_and_eqref(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{equation}\label{eq:test}E = mc^2\end{equation}'
            '\n\n'
            r'See \eqref{eq:test}.'
        )
        self.assertEqual(
            result,
            '<a name="equation-1"></a> \\\\label\\{eq:test\\}E = mc^2\\\\tag\\*\\{\\(1\\)\\}'
            '\n\n'
            'See [\\(1\\)](#equation-1)\\.'
        )


# ---- Integration: verbatim ----

class TestMarkdownVerbatim(unittest.TestCase):

    maxDiff = None

    def test_verbcode_inline(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'\verbcode{x+y}', standalone_mode=True)
        result = frag.render_standalone(MarkdownFragmentRenderer())
        self.assertEqual(result, '`` x\\+y ``')

    def test_verbatimcode_environment(self):
        environ = mk_flm_environ()
        result = render_doc(environ, 'Some text.\n\n'
                            '\\begin{verbatimcode}\nx = 1\ny = 2\n\\end{verbatimcode}')
        self.assertEqual(result, 'Some text\\.\n\n`` x = 1\ny = 2\n ``')


# ---- Integration: enumeration ----

class TestMarkdownEnumeration(unittest.TestCase):

    maxDiff = None

    def test_itemize(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{itemize}
\item First item
\item Second item
\end{itemize}''')
        self.assertEqual(result, '- \u2022 First item\n\n- \u2022 Second item')

    def test_enumerate(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{enumerate}
\item Alpha
\item Beta
\item Gamma
\end{enumerate}''')
        self.assertEqual(result, '- 1\\. Alpha\n\n- 2\\. Beta\n\n- 3\\. Gamma')

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
            '- 1\\. First\n\n- 2\\. Second\n  \n  - \\- Sub A\n  \n  - \\- Sub B'
        )


# ---- Integration: floats ----

class TestMarkdownFloats(unittest.TestCase):

    maxDiff = None

    def test_figure_with_caption_and_label(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}'
            r'\caption{A nice figure.}\label{figure:test}\end{figure}',
        )
        self.assertEqual(
            result,
            '---\n\n'
            '<a name="figure-1"></a> ![](img/test.png)\n\n'
            'Figure\xa01: A nice figure\\.\n\n'
            '---'
        )

    def test_figure_bare(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}',
        )
        self.assertEqual(
            result,
            '---\n\n![](img/test.png)\n\n---'
        )

    def test_figure_caption_only(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}'
            r'\caption{Just caption}\end{figure}',
        )
        self.assertEqual(
            result,
            '---\n\n![](img/test.png)\n\nFigure: Just caption\n\n---'
        )


# ---- Integration: footnote ----

class TestMarkdownFootnote(unittest.TestCase):

    maxDiff = None

    def test_footnote_inline_marker(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'Some text\footnote{A note.} here.')
        self.assertEqual(result, 'Some text[a](#footnote-1) here\\.')


# ---- Integration: defterm ----

class TestMarkdownDefterm(unittest.TestCase):

    maxDiff = None

    def test_defterm_simple(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{defterm}{Entropy}[label={term:entropy}]
A measure of disorder.
\end{defterm}''')
        self.assertEqual(
            result,
            '<a name="defterm-Entropy"></a>\n'
            'Entropy: \\[label=term:entropy\\] A measure of disorder\\.'
        )


# ---- Integration: theorem ----

class TestMarkdownTheorem(unittest.TestCase):

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
            '<a name="theorem-1"></a> '
            'Theorem\xa01 \\(title=Important result\\).  \n'
            'Some theorem content\\.'
        )


# ---- Integration: cells (delegates to HTML) ----

class TestMarkdownCells(unittest.TestCase):

    maxDiff = None

    def test_table_float_with_cells(self):
        features = standard_features()
        features.append(FeatureCells())
        environ = make_standard_environment(features)
        result = render_doc(environ, r'''\begin{table}
\begin{cells}
\celldata<H>{Name & Age}
\celldata{Alice & 30}
\end{cells}
\caption{A table.}\label{table:test}
\end{table}''')
        self.assertEqual(
            result,
            '---\n\n'
            '<a name="table-1"></a> '
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>Name</p></th>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>Age</p></th>'
            '</tr><tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>Alice</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>30</p></td>'
            '</tr></table>\n\n'
            'Table\xa01: A table\\.\n\n'
            '---'
        )


# ---- rx_mdspecials regex ----

class TestRxMdSpecials(unittest.TestCase):

    def test_matches_backslash(self):
        self.assertTrue(rx_mdspecials.search('a\\b') is not None)

    def test_matches_asterisk(self):
        self.assertTrue(rx_mdspecials.search('a*b') is not None)

    def test_matches_hash(self):
        self.assertTrue(rx_mdspecials.search('#x') is not None)

    def test_no_match_plain(self):
        self.assertIsNone(rx_mdspecials.search('hello world'))


# ---- FragmentRendererInformation ----

class TestFragmentRendererInformation(unittest.TestCase):

    def test_class(self):
        self.assertIs(
            FragmentRendererInformation.FragmentRendererClass,
            MarkdownFragmentRenderer,
        )

    def test_format_name(self):
        self.assertEqual(FragmentRendererInformation.format_name, 'markdown')


if __name__ == '__main__':
    unittest.main()
