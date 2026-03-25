import re
import unittest

from flm.fragmentrenderer.latex import LatexFragmentRenderer, FragmentRendererInformation
from flm.feature.graphics import GraphicsResource
from flm.stdfeatures import standard_features
from flm.flmenvironment import make_standard_environment
from flm.flmrendercontext import FLMStandaloneModeRenderContext


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr=None):
    if fr is None:
        fr = LatexFragmentRenderer()
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    result, _ = doc.render(fr)
    return result


# ---- Init / Config ----

class TestLatexFragmentRendererInit(unittest.TestCase):

    def test_default_init(self):
        fr = LatexFragmentRenderer()
        self.assertTrue(fr.supports_delayed_render_markers)
        self.assertIsNone(fr.latex_wrap_verbatim_macro)
        self.assertIsNone(fr.use_endnote_latex_command)
        self.assertIsNone(fr.use_citation_latex_command)
        self.assertTrue(fr.use_phantom_section)
        self.assertEqual(fr.latex_label_prefix, 'x:')
        self.assertFalse(fr.debug_disable_pin_labels)
        self.assertTrue(fr.use_flm_macro_for_pinning_labels)

    def test_custom_config(self):
        fr = LatexFragmentRenderer(config={
            'latex_wrap_verbatim_macro': 'myverb',
            'use_endnote_latex_command': 'textsuperscript',
            'use_citation_latex_command': 'textsc',
            'debug_disable_pin_labels': True,
            'latex_label_prefix': 'y:',
        })
        self.assertEqual(fr.latex_wrap_verbatim_macro, 'myverb')
        self.assertEqual(fr.use_endnote_latex_command, 'textsuperscript')
        self.assertEqual(fr.use_citation_latex_command, 'textsc')
        self.assertTrue(fr.debug_disable_pin_labels)
        self.assertEqual(fr.latex_label_prefix, 'y:')

    def test_heading_commands_by_level(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.heading_commands_by_level[1], 'section')
        self.assertEqual(fr.heading_commands_by_level[2], 'subsection')
        self.assertEqual(fr.heading_commands_by_level[3], 'subsubsection')
        self.assertEqual(fr.heading_commands_by_level[4], 'paragraph')
        self.assertEqual(fr.heading_commands_by_level['theorem'], 'flmTheoremHeading')

    def test_text_format_cmds(self):
        fr = LatexFragmentRenderer()
        cmds = dict(fr.text_format_cmds)
        self.assertEqual(cmds['textit'], 'textit')
        self.assertEqual(cmds['textbf'], 'textbf')
        self.assertEqual(cmds['defterm-term'], 'flmDisplayTerm')
        self.assertIsNone(cmds['term-in-defining-defterm'])


# ---- latexescape ----

class TestLatexEscape(unittest.TestCase):

    def test_braces(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('{x}'), r'\{x\}')

    def test_ampersand(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('a & b'), r'a \& b')

    def test_percent(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('100% done'), r'100\% done')

    def test_hash(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('#1'), r'\#1')

    def test_underscore_caret(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('a_b^c'), r'a\_b{\textasciicircum}c')

    def test_tilde(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('~tilde'), r'{\textasciitilde}tilde')

    def test_plain(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('hello'), 'hello')

    def test_empty(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape(''), '')

    def test_unicode(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr.latexescape('\u00c5ngstr\u00f6m'), '\\r{A}ngstr\\"om')


# ---- wrap_in_text_format_macro ----

class TestWrapInTextFormatMacro(unittest.TestCase):

    def test_single_format(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.wrap_in_text_format_macro('hello', ['textbf'], rc),
            r'\textbf{hello}'
        )

    def test_nested_formats(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.wrap_in_text_format_macro('hello', ['textit', 'textbf'], rc),
            r'\textit{\textbf{hello}}'
        )

    def test_none_format_passthrough(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.wrap_in_text_format_macro('hello', ['term-in-defining-defterm'], rc),
            'hello'
        )

    def test_empty_formats(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.wrap_in_text_format_macro('hello', [], rc),
            'hello'
        )


# ---- pin_label_here ----

class TestPinLabelHere(unittest.TestCase):

    def test_default(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.pin_label_here('sec-intro', 'Section 1'),
            r'\phantomsection '
            r'\flmPinLabelHereWithDisplayText{x:sec-intro}{Section 1}'
        )

    def test_no_phantom_section(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.pin_label_here('sec-intro', 'Section 1', insert_phantom_section=False),
            r'\flmPinLabelHereWithDisplayText{x:sec-intro}{Section 1}'
        )

    def test_without_flm_macro(self):
        fr = LatexFragmentRenderer(config={'use_flm_macro_for_pinning_labels': False})
        self.assertEqual(
            fr.pin_label_here('sec-intro', 'Section 1'),
            r'\phantomsection '
            r'\expandafter\def\csname @currentlabel\endcsname{Section 1}'
            r'\label{x:sec-intro}'
        )

    def test_disabled(self):
        fr = LatexFragmentRenderer(config={'debug_disable_pin_labels': True})
        self.assertEqual(fr.pin_label_here('sec-intro', 'Section 1'), '')

    def test_no_phantom_section_config(self):
        fr = LatexFragmentRenderer(config={'use_phantom_section': False})
        self.assertEqual(
            fr.pin_label_here('sec-intro', 'Section 1'),
            r'\flmPinLabelHereWithDisplayText{x:sec-intro}{Section 1}'
        )


# ---- _latex_join ----

class TestLatexJoin(unittest.TestCase):

    def test_plain_join(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr._latex_join('hello', 'world'), 'helloworld')

    def test_comment_adds_newline(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr._latex_join('hello% comment', 'world'), 'hello% comment\nworld')

    def test_named_macro_adds_space(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr._latex_join(r'\textbf', 'x'), r'\textbf x')

    def test_multiline_comment_in_last_line(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr._latex_join('abc\ndef% comm', 'more'), 'abc\ndef% comm\nmore')

    def test_multiline_no_comment(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr._latex_join('abc\ndef', 'more'), 'abc\ndefmore')

    def test_empty_first(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(fr._latex_join('', 'x'), 'x')


# ---- render_value / render_empty_error_placeholder / render_nothing ----

class TestRenderBasicMethods(unittest.TestCase):

    def test_render_value(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(fr.render_value('hello', rc), 'hello')

    def test_render_value_escapes(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(fr.render_value('{x}', rc), r'\{x\}')

    def test_render_empty_error_placeholder(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_empty_error_placeholder('debug info', rc),
            '% debug info\n'
        )

    def test_render_empty_error_placeholder_newlines(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_empty_error_placeholder('line1\nline2', rc),
            '% line1 line2\n'
        )

    def test_render_nothing_no_annotations(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(fr.render_nothing(rc), '% \n')

    def test_render_nothing_with_annotations(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(fr.render_nothing(rc, annotations=['test']), '% test\n')

    def test_render_nothing_multiple_annotations(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(fr.render_nothing(rc, annotations=['a', 'b']), '% a b\n')


# ---- render_verbatim ----

class TestRenderVerbatim(unittest.TestCase):

    def test_inline_verbatim(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_verbatim('code here', rc,
                               is_block_level=False, annotations=['inline-math']),
            'code here'
        )

    def test_block_verbatimcode(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_verbatim('x = 1', rc,
                               is_block_level=True, annotations=['verbatimcode']),
            '\\begin{verbatim}\nx = 1\\end{verbatim}'
        )

    def test_inline_escapes_special_chars(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_verbatim('special {char}', rc,
                               is_block_level=False, annotations=[]),
            r'special \{char\}'
        )

    def test_wrap_verbatim_macro(self):
        fr = LatexFragmentRenderer(config={'latex_wrap_verbatim_macro': 'myverb'})
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_verbatim('x+y', rc, is_block_level=False, annotations=[]),
            r'\myverb{x+y}'
        )


# ---- render_join_blocks ----

class TestRenderJoinBlocks(unittest.TestCase):

    def test_join_blocks(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_join_blocks(['a', 'b'], rc),
            'a\n\nb\n'
        )

    def test_strips_whitespace(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_join_blocks(['  a  ', '  b  '], rc),
            'a\n\nb\n'
        )


# ---- render_semantic_span ----

class TestRenderSemanticSpan(unittest.TestCase):

    def test_plain_content(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(fr.render_semantic_span('content', 'other', rc), 'content')

    def test_endnote_command(self):
        fr = LatexFragmentRenderer(config={'use_endnote_latex_command': 'textsuperscript'})
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_span('content', 'endnotes', rc),
            r'\textsuperscript{content}'
        )

    def test_citation_command(self):
        fr = LatexFragmentRenderer(config={'use_citation_latex_command': 'textsuperscript'})
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_span('content', 'citations', rc),
            r'\textsuperscript{content}'
        )

    def test_non_matching_role_with_endnote_command(self):
        fr = LatexFragmentRenderer(config={'use_endnote_latex_command': 'textsuperscript'})
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_span('content', 'other', rc),
            'content'
        )


# ---- render_semantic_block ----

class TestRenderSemanticBlock(unittest.TestCase):

    def test_known_role(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_block('content', 'defterm', rc),
            '\\begin{flmDefterm}content\\end{flmDefterm}%\n'
        )

    def test_unknown_role(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_block('content', 'unknown_role', rc),
            '% --- begin  ---\ncontent% --- end  ---\n'
        )

    def test_with_annotations(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_block('content', None, rc, annotations=['ann1']),
            '% --- begin ann1 ---\ncontent% --- end ann1 ---\n'
        )

    def test_with_target_id(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_semantic_block('content', 'defterm', rc, target_id='test-id'),
            '\\begin{flmDefterm}'
            '\\phantomsection \\flmPinLabelHereWithDisplayText{x:test-id}{<block>}'
            'content\\end{flmDefterm}%\n'
        )


# ---- render_latex_link_hyperref / render_latex_link_href ----

class TestRenderLinks(unittest.TestCase):

    def test_hyperref(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.render_latex_link_hyperref('display', 'sec-intro'),
            r'\hyperref[{x:sec-intro}]{display}' + '%\n'
        )

    def test_hyperref_disabled(self):
        fr = LatexFragmentRenderer(config={'debug_disable_link_hyperref': True})
        self.assertEqual(
            fr.render_latex_link_hyperref('display', 'sec-intro'),
            'display'
        )

    def test_href_simple(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.render_latex_link_href('click me', 'https://example.com'),
            r'\href{https://example.com}{click me}'
        )

    def test_href_escapes_hash(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.render_latex_link_href('click', 'https://example.com/path#frag'),
            r'\href{https://example.com/path\#frag}{click}'
        )

    def test_href_escapes_percent(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.render_latex_link_href('click', 'https://example.com/100%25'),
            r'\href{https://example.com/100\%25}{click}'
        )


# ---- delayed markers ----

class TestDelayedMarkers(unittest.TestCase):

    def test_render_delayed_marker(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_delayed_marker(None, 42, rc),
            r'\FLMDLYD{42}'
        )

    def test_render_delayed_dummy_placeholder(self):
        fr = LatexFragmentRenderer()
        rc = FLMStandaloneModeRenderContext(fr)
        self.assertEqual(
            fr.render_delayed_dummy_placeholder(None, 99, rc),
            '% delayed:99\n'
        )

    def test_replace_delayed_markers(self):
        fr = LatexFragmentRenderer()
        self.assertEqual(
            fr.replace_delayed_markers_with_final_values(
                r'before \FLMDLYD{1} middle \FLMDLYD{2} after',
                {1: 'FIRST', 2: 'SECOND'}
            ),
            'before FIRST middle SECOND after'
        )


# ---- collect_graphics_resource / render_graphics_block ----

class TestGraphics(unittest.TestCase):

    def test_collect_no_dimensions(self):
        fr = LatexFragmentRenderer()
        gr = GraphicsResource(src_url='test.png')
        src, opts = fr.collect_graphics_resource(gr, None)
        self.assertEqual(src, 'test.png')
        self.assertIsNone(opts)

    def test_collect_with_dimensions(self):
        fr = LatexFragmentRenderer()
        gr = GraphicsResource(src_url='test.png', physical_dimensions=(100, 50))
        src, opts = fr.collect_graphics_resource(gr, None)
        self.assertEqual(src, 'test.png')
        # Use regex for float format compatibility (Python vs JS)
        self.assertTrue(
            re.fullmatch(r'width=100(\.0+)?pt,height=50(\.0+)?pt,', opts) is not None
        )

    def test_collect_width_only(self):
        fr = LatexFragmentRenderer()
        gr = GraphicsResource(src_url='test.png', physical_dimensions=(100, None))
        src, opts = fr.collect_graphics_resource(gr, None)
        self.assertTrue(
            re.fullmatch(r'width=100(\.0+)?pt,', opts) is not None
        )

    def test_collect_raster_magnification(self):
        fr = LatexFragmentRenderer(config={'graphics_raster_magnification': 2})
        gr = GraphicsResource(src_url='test.png',
                              physical_dimensions=(100, 50),
                              graphics_type='raster')
        src, opts = fr.collect_graphics_resource(gr, None)
        self.assertTrue(
            re.fullmatch(r'width=200(\.0+)?pt,height=100(\.0+)?pt,', opts) is not None
        )

    def test_collect_vector_magnification(self):
        fr = LatexFragmentRenderer(config={'graphics_vector_magnification': 0.5})
        gr = GraphicsResource(src_url='test.svg',
                              physical_dimensions=(200, 100),
                              graphics_type='vector')
        src, opts = fr.collect_graphics_resource(gr, None)
        self.assertTrue(
            re.fullmatch(r'width=100(\.0+)?pt,height=50(\.0+)?pt,', opts) is not None
        )

    def test_render_graphics_block_no_dims(self):
        fr = LatexFragmentRenderer()
        gr = GraphicsResource(src_url='test.png')
        self.assertEqual(
            fr.render_graphics_block(gr, None),
            r'\includegraphics{test.png}'
        )

    def test_render_graphics_block_with_dims(self):
        fr = LatexFragmentRenderer()
        gr = GraphicsResource(src_url='test.png', physical_dimensions=(100, 50))
        result = fr.render_graphics_block(gr, None)
        # Use regex for float format compatibility (Python vs JS)
        self.assertTrue(
            re.fullmatch(
                r'\\includegraphics\[width=100(\.0+)?pt,height=50(\.0+)?pt,\]\{test\.png\}',
                result
            ) is not None
        )


# ---- Integration: full rendering through FLM pipeline ----

class TestLatexRendererIntegration(unittest.TestCase):

    maxDiff = None

    def test_text_format(self):
        fr = LatexFragmentRenderer()
        environ = mk_flm_environ()
        frag = environ.make_fragment(r"Text content")
        render_result = fr.render_text_format(
            ['textbf'],
            frag.nodes,
            FLMStandaloneModeRenderContext(fr),
        )
        self.assertEqual(
            render_result.replace('%\n', '').replace(r'\relax ', '').strip(),
            r'\textbf{Text content}'
        )

    def test_bold_and_italic(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        frag = environ.make_fragment(
            r'\textbf{bold} and \textit{italic}', standalone_mode=True
        )
        result = frag.render_standalone(fr)
        self.assertEqual(result, r'\textbf{bold} and \textit{italic}')

    def test_inline_math(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        frag = environ.make_fragment(r'Inline \(x+y\) math', standalone_mode=True)
        result = frag.render_standalone(fr)
        self.assertEqual(result, r'Inline \(x+y\) math')

    def test_equation_environment(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        result = render_doc(environ, r'\begin{equation}E=mc^2\end{equation}', fr)
        self.assertEqual(
            result,
            r'\begin{equation}E=mc^2\tag*{(1)}\end{equation}'
        )

    def test_align_environment(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        # In FLM, \\\\ (two backslashes) means LaTeX line break \\
        frag = environ.make_fragment(
            '\\begin{align}\na &= b \\\\\nc &= d\n\\end{align}'
        )
        doc = environ.make_document(frag.render)
        result, _ = doc.render(fr)
        self.assertEqual(
            result,
            '\\begin{align}\na &= b \\tag*{(1)}\\\\\nc &= d\n\\tag*{(2)}\\end{align}'
        )

    def test_paragraphs(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        result = render_doc(environ, 'Hello world.\n\nSecond paragraph.', fr)
        self.assertEqual(result, 'Hello world.\n\nSecond paragraph.\n')

    def test_heading_section(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        result = render_doc(environ, r'\section{Intro}', fr)
        self.assertEqual(result, '\\section{Intro}%\n\\label{x:sec--Intro}%\n')

    def test_heading_with_label(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        result = render_doc(environ, r'\section{Intro}\label{sec:intro}', fr)
        self.assertEqual(result, '\\section{Intro}%\n\\label{x:sec-intro}%\n')

    def test_heading_subsection(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        result = render_doc(environ, r'\subsection{Details}', fr)
        self.assertEqual(result, '\\subsection{Details}%\n\\label{x:sec--Details}%\n')

    def test_heading_invalid_level_raises(self):
        fr = LatexFragmentRenderer()
        environ = mk_flm_environ()
        frag = environ.make_fragment('Title', standalone_mode=True)
        rc = FLMStandaloneModeRenderContext(fr)
        with self.assertRaises(ValueError):
            fr.render_heading(frag.nodes, rc, heading_level=99)

    def test_href_link(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        frag = environ.make_fragment(
            r'\href{https://example.com}{click}', standalone_mode=True
        )
        result = frag.render_standalone(fr)
        self.assertEqual(result, r'\href{https://example.com}{click}')

    def test_url_link(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        frag = environ.make_fragment(
            r'\url{https://example.com}', standalone_mode=True
        )
        result = frag.render_standalone(fr)
        self.assertEqual(result, r'\href{https://example.com}{example.com}')

    def test_delayed_rendering_section_ref(self):
        lfr = LatexFragmentRenderer()
        environ = mk_flm_environ()

        frag = environ.make_fragment(r"""
\section{My Section}
\label{sec:mysec}
Hello.

See \ref{sec:mysec}.
""".strip())

        doc = environ.make_document(
            lambda render_context: {'content': frag.render(render_context)}
        )
        result, _ = doc.render(lfr)

        self.assertEqual(
            result['content'].replace('%\n', '').replace(r'\relax ', '').strip(),
            r'\section{My Section}\label{x:sec-mysec}'
            '\nHello.\n\n'
            r'See \hyperref[{x:sec-mysec}]{My Section}.'
        )

    def test_equation_with_ref(self):
        environ = mk_flm_environ()
        fr = LatexFragmentRenderer()
        result = render_doc(environ, r"""
See \eqref{eq:main}.
\begin{equation}\label{eq:main}
x = y + z
\end{equation}
""".strip(), fr)
        self.assertEqual(
            result,
            'See \\hyperref[{x:equation-1}]{(1)}%\n'
            '. \\begin{equation}\\label{eq:main}\n'
            'x = y + z\n'
            '\\tag*{(1)}\\end{equation}'
        )

    def test_enumerate(self):
        lfr = LatexFragmentRenderer()
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{enumerate}
\item One
\item Two
\item[c] Three
\end{enumerate}
""".strip(), lfr)
        self.assertEqual(
            result.replace('%\n', '').replace(r'\relax ', '').strip(),
            '\\begin{enumerate}% enumeration,enumerate\n'
            '\\item[{1.}]One\n'
            '\\item[{2.}]Two\n'
            '\\item[{c}]Three\\end{enumerate}'
        )

    def test_enumerate_direct(self):
        lfr = LatexFragmentRenderer()
        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"1.")
        frag2 = environ.make_fragment(r"Hello.")

        render_result = lfr.render_enumeration(
            [frag2.nodes],
            lambda n: frag1.nodes,
            FLMStandaloneModeRenderContext(lfr),
        )
        self.assertEqual(
            render_result.replace('%\n', '').replace(r'\relax ', ''),
            '\\begin{itemize}% enumeration\n'
            '\\item[{1.}]Hello.\\end{itemize}'
        )

    def test_itemize(self):
        environ = mk_flm_environ()
        lfr = LatexFragmentRenderer()
        result = render_doc(environ, r"""
\begin{itemize}
\item Alpha
\item Beta
\end{itemize}
""".strip(), lfr)
        self.assertEqual(
            result.replace('%\n', '').replace(r'\relax ', '').strip(),
            '\\begin{itemize}% enumeration,itemize\n'
            '\\item[{{\\textbullet}}]Alpha\n'
            '\\item[{{\\textbullet}}]Beta\\end{itemize}'
        )

    def test_verbatim_environment(self):
        environ = mk_flm_environ()
        lfr = LatexFragmentRenderer()
        result = render_doc(environ, r"""
\begin{verbatimcode}
x = 1
y = 2
\end{verbatimcode}
""".strip(), lfr)
        self.assertEqual(result, '\\begin{verbatim}\nx = 1\ny = 2\n\\end{verbatim}\n')

    def test_custom_text_format_cmd(self):
        text_format_cmds = dict(LatexFragmentRenderer.text_format_cmds)
        text_format_cmds['defterm-term'] = 'ecztermdef'

        fr = LatexFragmentRenderer(config={'text_format_cmds': text_format_cmds})
        environ = mk_flm_environ()

        frag = environ.make_fragment(r'\begin{enumerate}\item One\item Two\item[c] Three\end{enumerate}')
        doc = environ.make_document(frag.render)
        result, _ = doc.render(fr)

        self.assertEqual(
            result.replace('%\n', '').replace(r'\relax ', '').strip(),
            '\\begin{enumerate}% enumeration,enumerate\n'
            '\\item[{1.}]One\n'
            '\\item[{2.}]Two\n'
            '\\item[{c}]Three\\end{enumerate}'
        )

    def test_defterm_rendering(self):
        environ = mk_flm_environ()
        lfr = LatexFragmentRenderer()
        result = render_doc(environ, r"""
\begin{defterm}{Entropy}
\label{topic:entropy}
The measure of disorder.
\end{defterm}
""".strip(), lfr)
        self.assertEqual(
            result,
            '\\begin{flmDefterm}'
            '\\phantomsection '
            '\\flmPinLabelHereWithDisplayText{x:defterm-Entropy}{<block>}'
            '\\flmDisplayTerm{Entropy: }The measure of disorder.\n'
            '\\end{flmDefterm}%\n'
        )

    def test_footnote(self):
        environ = mk_flm_environ()
        lfr = LatexFragmentRenderer()
        frag = environ.make_fragment(r'Text\footnote{A note.}')
        doc = environ.make_document(
            lambda rc: {'content': frag.render(rc)},
            enable_features=['endnotes']
        )
        result, _ = doc.render(lfr)
        self.assertEqual(
            result['content'],
            'Text\\hyperref[{x:footnote-1}]{a}%\n'
        )

    def test_float_with_caption(self):
        environ = mk_flm_environ()
        lfr = LatexFragmentRenderer()
        result = render_doc(environ, r"""
\begin{figure}
\includegraphics{img.png}
\caption{My caption}\label{figure:test}
\end{figure}
""".strip(), lfr)
        self.assertEqual(
            result,
            '\\begin{figure}[hbtp]%\n'
            '\\centering{}\\includegraphics{img.png}\n\n'
            '\\flmFloatCaption{%'
            ' --- begin  ---\n'
            '\\phantomsection '
            '\\flmPinLabelHereWithDisplayText{x:figure-1}{Figure~1}'
            'Figure~1: My caption'
            '% --- end  ---\n'
            '}\n'
            '\\end{figure}\n'
        )

    def test_float_without_caption(self):
        environ = mk_flm_environ()
        lfr = LatexFragmentRenderer()
        result = render_doc(environ, r"""
\begin{figure}
\includegraphics{img.png}
\end{figure}
""".strip(), lfr)
        self.assertEqual(
            result,
            '\\begin{center}%\n\\includegraphics{img.png}\n\\end{center}\n'
        )

    def test_link_with_endnote_command(self):
        fr = LatexFragmentRenderer(config={'use_endnote_latex_command': 'textsuperscript'})
        environ = mk_flm_environ()
        frag = environ.make_fragment(
            r'\href{https://example.com}{click}', standalone_mode=True
        )
        result = frag.render_standalone(fr)
        self.assertEqual(result, r'\href{https://example.com}{click}')


# ---- FragmentRendererInformation ----

class TestFragmentRendererInformation(unittest.TestCase):

    def test_class(self):
        self.assertTrue(
            FragmentRendererInformation.FragmentRendererClass is LatexFragmentRenderer
        )

    def test_format_name(self):
        self.assertEqual(FragmentRendererInformation.format_name, 'latex')

    def test_style_information_keys(self):
        fr = LatexFragmentRenderer()
        info = dict(FragmentRendererInformation.get_style_information(fr))
        self.assertTrue('preamble_suggested_defs' in info)
        self.assertTrue('package_suggested_defs' in info)
        self.assertTrue(len(info['preamble_suggested_defs']) > 0)


if __name__ == '__main__':
    unittest.main()
