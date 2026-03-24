import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer
from flm.feature.floats import (
    FloatContentAnyContent,
    FloatContentIncludeGraphics,
    FloatContentCells,
    FloatInstance,
    FloatType,
    FloatEnvironment,
    FeatureFloats,
    _make_content_handler,
    _float_default_counter_formatter_spec,
    available_content_handlers,
)


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr=None):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    if fr is None:
        fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


# -----------------------------------------------------------------------
# Content handler helpers
# -----------------------------------------------------------------------


class TestMakeContentHandler(unittest.TestCase):

    def test_string_any(self):
        h = _make_content_handler('any')
        self.assertEqual(type(h).__name__, 'FloatContentAnyContent')

    def test_string_includegraphics(self):
        h = _make_content_handler('includegraphics')
        self.assertEqual(type(h).__name__, 'FloatContentIncludeGraphics')

    def test_dict_cells(self):
        h = _make_content_handler({'name': 'cells'})
        self.assertEqual(type(h).__name__, 'FloatContentCells')

    def test_instance_passthrough(self):
        inst = FloatContentAnyContent()
        h = _make_content_handler(inst)
        self.assertIs(h, inst)

    def test_invalid_string(self):
        with self.assertRaises(ValueError):
            _make_content_handler('nonexistent')

    def test_invalid_dict(self):
        with self.assertRaises(ValueError):
            _make_content_handler({'name': 'nonexistent'})


class TestAvailableContentHandlers(unittest.TestCase):

    def test_keys(self):
        self.assertTrue('any' in available_content_handlers)
        self.assertTrue('includegraphics' in available_content_handlers)
        self.assertTrue('cells' in available_content_handlers)


# -----------------------------------------------------------------------
# _float_default_counter_formatter_spec
# -----------------------------------------------------------------------


class TestFloatDefaultCounterFormatterSpec(unittest.TestCase):

    def test_figure_prefix(self):
        spec = _float_default_counter_formatter_spec('figure')
        self.assertEqual(spec['prefix_display']['singular'], 'Fig.~')
        self.assertEqual(spec['prefix_display']['plural'], 'Figs.~')
        self.assertEqual(
            spec['prefix_display']['capital']['singular'], 'Figure~'
        )
        self.assertEqual(
            spec['prefix_display']['capital']['plural'], 'Figures~'
        )

    def test_table_prefix(self):
        spec = _float_default_counter_formatter_spec('table')
        self.assertEqual(spec['prefix_display']['singular'], 'Tab.~')
        self.assertEqual(spec['prefix_display']['plural'], 'Tabs.~')
        self.assertEqual(
            spec['prefix_display']['capital']['singular'], 'Table~'
        )

    def test_custom_type_prefix(self):
        spec = _float_default_counter_formatter_spec('widget')
        self.assertEqual(spec['prefix_display']['singular'], 'Widget~')
        self.assertEqual(spec['prefix_display']['plural'], 'Widgets~')

    def test_format_num_roman(self):
        spec = _float_default_counter_formatter_spec('figure')
        self.assertEqual(spec['format_num'], {'template': '${Roman}'})

    def test_delimiters(self):
        spec = _float_default_counter_formatter_spec('figure')
        self.assertEqual(spec['delimiters'], ('', ''))


# -----------------------------------------------------------------------
# FloatType
# -----------------------------------------------------------------------


class TestFloatType(unittest.TestCase):

    def test_basic_init(self):
        ft = FloatType('figure', 'Figure', {'format_num': 'arabic'},
                        ['includegraphics'])
        self.assertEqual(ft.float_type, 'figure')
        self.assertEqual(ft.float_caption_name, 'Figure')
        self.assertEqual(ft.content_handlers, ['includegraphics'])

    def test_default_caption_name(self):
        ft = FloatType('myfloat')
        self.assertEqual(ft.float_caption_name, 'myfloat')

    def test_repr(self):
        ft = FloatType('figure', 'Figure', {'format_num': 'arabic'},
                        ['includegraphics'])
        r = repr(ft)
        self.assertTrue(r.startswith('FloatType('))
        self.assertTrue('figure' in r)

    def test_asdict(self):
        ft = FloatType('figure', 'Figure', {'format_num': 'arabic'}, ['ig'])
        d = ft.asdict()
        self.assertEqual(d['float_type'], 'figure')
        self.assertEqual(d['float_caption_name'], 'Figure')
        self.assertEqual(d['content_handlers'], ['ig'])
        self.assertTrue('counter_formatter' in d)


# -----------------------------------------------------------------------
# FloatInstance
# -----------------------------------------------------------------------


class TestFloatInstance(unittest.TestCase):

    def test_basic_init(self):
        fi = FloatInstance(float_type='figure', ref_label='myfig',
                           target_id='figure-1')
        self.assertEqual(fi.float_type, 'figure')
        self.assertEqual(fi.ref_label, 'myfig')
        self.assertEqual(fi.target_id, 'figure-1')
        self.assertIsNone(fi.counter_value)
        self.assertIsNone(fi.caption_nodelist)

    def test_asdict(self):
        fi = FloatInstance(float_type='table', ref_label='t1',
                           target_id='table-1')
        d = fi.asdict()
        self.assertEqual(d['float_type'], 'table')
        self.assertEqual(d['ref_label'], 't1')
        self.assertEqual(len(d), 10)

    def test_repr(self):
        fi = FloatInstance(float_type='figure')
        r = repr(fi)
        self.assertTrue(r.startswith('FloatInstance('))
        self.assertTrue('figure' in r)


# -----------------------------------------------------------------------
# FeatureFloats init / config
# -----------------------------------------------------------------------


class TestFeatureFloatsInit(unittest.TestCase):

    def test_default_float_types(self):
        ff = FeatureFloats()
        self.assertEqual(ff.feature_name, 'floats')
        self.assertTrue('figure' in ff.float_types)
        self.assertTrue('table' in ff.float_types)
        self.assertEqual(len(ff.float_types_list), 2)

    def test_custom_float_types_from_dicts(self):
        ff = FeatureFloats(float_types=[
            {'float_type': 'diagram', 'float_caption_name': 'Diagram'}
        ])
        self.assertTrue('diagram' in ff.float_types)
        self.assertFalse('figure' in ff.float_types)

    def test_custom_float_types_from_objects(self):
        ft = FloatType('chart', 'Chart', {'format_num': 'arabic'}, ['any'])
        ff = FeatureFloats(float_types=[ft])
        self.assertTrue('chart' in ff.float_types)

    def test_add_latex_context_definitions(self):
        ff = FeatureFloats()
        defs = ff.add_latex_context_definitions()
        env_names = [e.environmentname for e in defs['environments']]
        self.assertTrue('figure' in env_names)
        self.assertTrue('table' in env_names)

    def test_make_float_environment_spec(self):
        ff = FeatureFloats()
        env = ff.make_float_environment_spec('figure')
        self.assertEqual(env.float_type, 'figure')
        self.assertTrue(env.is_block_level)
        self.assertFalse(env.allowed_in_standalone_mode)

    def test_optional_dependencies(self):
        ff = FeatureFloats()
        self.assertTrue('refs' in ff.feature_optional_dependencies)
        self.assertTrue('numbering' in ff.feature_optional_dependencies)


# -----------------------------------------------------------------------
# FloatEnvironment properties
# -----------------------------------------------------------------------


class TestFloatEnvironment(unittest.TestCase):

    def test_init(self):
        env = FloatEnvironment('figure')
        self.assertEqual(env.float_type, 'figure')
        self.assertTrue(env.is_block_level)
        self.assertFalse(env.allowed_in_standalone_mode)
        self.assertTrue(env.float_content_render_at_environment_node_location)

    def test_default_content_handlers(self):
        env = FloatEnvironment('figure')
        self.assertEqual(len(env.content_handlers), 2)
        self.assertEqual(
            type(env.content_handlers[0]).__name__,
            'FloatContentIncludeGraphics'
        )
        self.assertEqual(
            type(env.content_handlers[1]).__name__,
            'FloatContentCells'
        )

    def test_custom_content_handlers(self):
        env = FloatEnvironment('figure', content_handlers=['any'])
        self.assertEqual(len(env.content_handlers), 1)
        self.assertEqual(
            type(env.content_handlers[0]).__name__,
            'FloatContentAnyContent'
        )


# -----------------------------------------------------------------------
# HTML rendering — figures
# -----------------------------------------------------------------------


class TestFeatureFloatsHtmlFigure(unittest.TestCase):

    maxDiff = None

    def test_bare_figure(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{fig/test}
\end{figure}
''')
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents">'
            '<img src="fig/test">'
            '</div></figure>'
        )

    def test_caption_only(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{fig/test}
\caption{Here is my test figure}
\end{figure}
''')
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents">'
            '<img src="fig/test">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-no-number">Figure</span>'
            ': Here is my test figure</span></figcaption></figure>'
        )

    def test_label_only(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{fig/test}
\label{figure:numbered}
\end{figure}
''')
        self.assertEqual(
            result,
            '<figure id="figure-1" class="float float-figure">'
            '<div class="float-contents">'
            '<img src="fig/test">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Figure&nbsp;1</span>'
            '</span></figcaption></figure>'
        )

    def test_label_and_caption(self):
        environ = mk_flm_environ()
        fr = HtmlFragmentRenderer()
        fr.float_caption_title_separator = '. '
        frag = environ.make_fragment(r'''
\begin{figure}
\includegraphics{fig/test}
\label{figure:numbered}
\caption{My test figure}
\end{figure}
'''.strip())
        doc = environ.make_document(frag.render)
        result, _ = doc.render(fr)
        self.assertEqual(
            result,
            '<figure id="figure-1" class="float float-figure">'
            '<div class="float-contents">'
            '<img src="fig/test">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Figure&nbsp;1</span>'
            '. My test figure</span></figcaption></figure>'
        )


# -----------------------------------------------------------------------
# HTML rendering — tables
# -----------------------------------------------------------------------


class TestFeatureFloatsHtmlTable(unittest.TestCase):

    maxDiff = None

    def test_table_with_label_and_caption(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{table}
\includegraphics{data/tab}
\label{table:data}
\caption{Data table}
\end{table}
''')
        self.assertEqual(
            result,
            '<figure id="table-1" class="float float-table">'
            '<div class="float-contents">'
            '<img src="data/tab">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Table&nbsp;1</span>'
            ': Data table</span></figcaption></figure>'
        )


# -----------------------------------------------------------------------
# HTML rendering — figure with ref
# -----------------------------------------------------------------------


class TestFeatureFloatsHtmlRef(unittest.TestCase):

    maxDiff = None

    def test_figure_with_ref(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{img/photo}
\label{figure:photo}
\caption{A photo}
\end{figure}
See \ref{figure:photo}.
''')
        self.assertEqual(
            result,
            '<figure id="figure-1" class="float float-figure">'
            '<div class="float-contents">'
            '<img src="img/photo">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Figure&nbsp;1</span>'
            ': A photo</span></figcaption></figure>\n'
            '<p>See <a href="#figure-1" class="href-ref ref-figure">'
            'Fig.&nbsp;1</a>.</p>'
        )

    def test_multiple_figures_with_refs(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{img/a}
\label{figure:a}
\caption{First}
\end{figure}
\begin{figure}
\includegraphics{img/b}
\label{figure:b}
\caption{Second}
\end{figure}
See \ref{figure:a} and \ref{figure:b}.
''')
        self.assertEqual(
            result,
            '<figure id="figure-1" class="float float-figure">'
            '<div class="float-contents">'
            '<img src="img/a">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Figure&nbsp;1</span>'
            ': First</span></figcaption></figure>\n'
            '<figure id="figure-2" class="float float-figure">'
            '<div class="float-contents">'
            '<img src="img/b">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Figure&nbsp;2</span>'
            ': Second</span></figcaption></figure>\n'
            '<p>See <a href="#figure-1" class="href-ref ref-figure">'
            'Fig.&nbsp;1</a> and '
            '<a href="#figure-2" class="href-ref ref-figure">'
            'Fig.&nbsp;2</a>.</p>'
        )


# -----------------------------------------------------------------------
# HTML rendering — any content handler
# -----------------------------------------------------------------------


class TestFeatureFloatsAnyContent(unittest.TestCase):

    maxDiff = None

    def test_any_content_figure(self):
        features = standard_features(floats=False)
        features.append(FeatureFloats(float_types=[
            FloatType('figure', 'Figure', {'format_num': 'arabic'}, ['any']),
        ]))
        environ = make_standard_environment(features)
        result = render_doc(environ, r'''
\begin{figure}
Hello world
\caption{Text figure}
\label{figure:txt}
\end{figure}
''')
        self.assertEqual(
            result,
            '<figure id="figure-1" class="float float-figure">'
            '<div class="float-contents">'
            '<p>Hello world  </p>'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-number">Figure&nbsp;1</span>'
            ': Text figure</span></figcaption></figure>'
        )


# -----------------------------------------------------------------------
# Text renderer
# -----------------------------------------------------------------------


class TestFeatureFloatsTextRenderer(unittest.TestCase):

    maxDiff = None

    def test_figure_text(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{img/photo}
\label{figure:photo}
\caption{A photo}
\end{figure}
See \ref{figure:photo}.
''', fr=TextFragmentRenderer())
        lines = result.split('\n')
        self.assertEqual(len(lines), 7)
        self.assertEqual(lines[0], '\u00b7' * 80)
        self.assertEqual(lines[1].strip(), '[img/photo]')
        self.assertEqual(lines[2], '')
        self.assertEqual(lines[3], 'Figure\xa01: A photo')
        self.assertEqual(lines[4], '\u00b7' * 80)
        self.assertEqual(lines[5], '')
        self.assertEqual(lines[6], 'See Fig.\xa01.')

    def test_table_text(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{table}
\includegraphics{data/tab}
\label{table:data}
\caption{Data table}
\end{table}
''', fr=TextFragmentRenderer())
        lines = result.split('\n')
        self.assertEqual(len(lines), 5)
        self.assertEqual(lines[0], '\u00b7' * 80)
        self.assertEqual(lines[1].strip(), '[data/tab]')
        self.assertEqual(lines[2], '')
        self.assertEqual(lines[3], 'Table\xa01: Data table')
        self.assertEqual(lines[4], '\u00b7' * 80)


# -----------------------------------------------------------------------
# LaTeX renderer
# -----------------------------------------------------------------------


class TestFeatureFloatsLatexRenderer(unittest.TestCase):

    maxDiff = None

    def test_figure_latex(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{img/photo}
\label{figure:photo}
\caption{A photo}
\end{figure}
See \ref{figure:photo}.
''', fr=LatexFragmentRenderer())
        self.assertEqual(
            result,
            '\\begin{figure}[hbtp]%\n'
            '\\centering{}\\includegraphics{img/photo}\n\n'
            '\\flmFloatCaption{% --- begin  ---\n'
            '\\phantomsection '
            '\\flmPinLabelHereWithDisplayText{x:figure-1}{Figure~1}'
            'Figure~1: A photo'
            '% --- end  ---\n}\n'
            '\\end{figure}\n\n'
            'See \\hyperref[{x:figure-1}]{Fig.~1}%\n.\n'
        )

    def test_table_latex(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{table}
\includegraphics{data/tab}
\label{table:data}
\caption{Data table}
\end{table}
''', fr=LatexFragmentRenderer())
        self.assertEqual(
            result,
            '\\begin{table}[hbtp]%\n'
            '\\centering{}\\includegraphics{data/tab}\n\n'
            '\\flmFloatCaption{% --- begin  ---\n'
            '\\phantomsection '
            '\\flmPinLabelHereWithDisplayText{x:table-1}{Table~1}'
            'Table~1: Data table'
            '% --- end  ---\n}\n'
            '\\end{table}\n'
        )


# -----------------------------------------------------------------------
# Markdown renderer
# -----------------------------------------------------------------------


class TestFeatureFloatsMarkdownRenderer(unittest.TestCase):

    maxDiff = None

    def test_figure_markdown(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{figure}
\includegraphics{img/photo}
\label{figure:photo}
\caption{A photo}
\end{figure}
See \ref{figure:photo}.
''', fr=MarkdownFragmentRenderer())
        self.assertEqual(
            result,
            '---\n\n'
            '<a name="figure-1"></a> ![](img/photo)\n\n'
            'Figure\xa01: A photo\n\n'
            '---\n\n'
            'See [Fig\\.\xa01](#figure-1)\\.'
        )

    def test_table_markdown(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''
\begin{table}
\includegraphics{data/tab}
\label{table:data}
\caption{Data table}
\end{table}
''', fr=MarkdownFragmentRenderer())
        self.assertEqual(
            result,
            '---\n\n'
            '<a name="table-1"></a> ![](data/tab)\n\n'
            'Table\xa01: Data table\n\n'
            '---'
        )


# -----------------------------------------------------------------------
# Recomposer
# -----------------------------------------------------------------------


class TestFeatureFloatsRecomposer(unittest.TestCase):

    maxDiff = None

    def _recompose(self, flm_input, options=None):
        environ = mk_flm_environ()
        frag = environ.make_fragment(flm_input.strip())
        recomposer = FLMPureLatexRecomposer(options if options else {})
        result = recomposer.recompose_pure_latex(frag.nodes)
        return result['latex']

    def test_label_and_caption(self):
        latex = self._recompose(r'''
\begin{figure}
\includegraphics{img/photo}
\label{figure:photo}
\caption{A photo}
\end{figure}
''')
        self.assertEqual(
            latex,
            r'\begin{flmFloat}{figure}{NumCap}'
            r'\includegraphics[max width=\linewidth]{img/photo}'
            r'\caption{A photo}'
            r'\label{ref1}'
            r'\end{flmFloat}'
        )

    def test_bare(self):
        latex = self._recompose(r'''
\begin{figure}
\includegraphics{img/bare}
\end{figure}
''')
        self.assertEqual(
            latex,
            r'\begin{flmFloat}{figure}{Bare}'
            r'\includegraphics[max width=\linewidth]{img/bare}'
            r'\end{flmFloat}'
        )

    def test_caption_only(self):
        latex = self._recompose(r'''
\begin{figure}
\includegraphics{img/caponly}
\caption{Cap only}
\end{figure}
''')
        self.assertEqual(
            latex,
            r'\begin{flmFloat}{figure}{CapOnly}'
            r'\includegraphics[max width=\linewidth]{img/caponly}'
            r'\caption{Cap only}'
            r'\end{flmFloat}'
        )

    def test_label_only(self):
        latex = self._recompose(r'''
\begin{figure}
\includegraphics{img/numonly}
\label{figure:numonly}
\end{figure}
''')
        self.assertEqual(
            latex,
            r'\begin{flmFloat}{figure}{NumOnly}'
            r'\includegraphics[max width=\linewidth]{img/numonly}'
            r'\caption{}'
            r'\label{ref1}'
            r'\end{flmFloat}'
        )

    def test_table_recompose(self):
        latex = self._recompose(r'''
\begin{table}
\includegraphics{data/tab}
\label{table:data}
\caption{Data table}
\end{table}
''')
        self.assertEqual(
            latex,
            r'\begin{flmFloat}{table}{NumCap}'
            r'\includegraphics[max width=\linewidth]{data/tab}'
            r'\caption{Data table}'
            r'\label{ref1}'
            r'\end{flmFloat}'
        )

    def test_keep_as_is(self):
        latex = self._recompose(r'''
\begin{figure}
\includegraphics{img/photo}
\label{figure:photo}
\caption{A photo}
\end{figure}
''', options={'floats': {'keep_as_is': True}})
        self.assertEqual(
            latex,
            '\\begin{figure}\n'
            '\\includegraphics[max width=\\linewidth]{img/photo}\n'
            '\\label{figure:photo}\n'
            '\\caption{A photo}\n'
            '\\end{figure}'
        )

    def test_custom_captioncmd_for_num_only(self):
        latex = self._recompose(r'''
\begin{figure}
\includegraphics{img/numonly}
\label{figure:numonly}
\end{figure}
''', options={'floats': {'captioncmd_for_num_only': r'\mycaption{}'}})
        self.assertEqual(
            latex,
            r'\begin{flmFloat}{figure}{NumOnly}'
            r'\includegraphics[max width=\linewidth]{img/numonly}'
            r'\mycaption{}'
            r'\label{ref1}'
            r'\end{flmFloat}'
        )


# -----------------------------------------------------------------------
# Error cases
# -----------------------------------------------------------------------


class TestFeatureFloatsErrors(unittest.TestCase):

    def test_wrong_label_prefix(self):
        environ = mk_flm_environ()
        with self.assertRaises(LatexWalkerLocatedError):
            environ.make_fragment(r'''
\begin{figure}
\includegraphics{img/test}
\label{table:wrong}
\end{figure}
'''.strip())

    def test_invalid_content_in_figure(self):
        environ = mk_flm_environ()
        with self.assertRaises(LatexWalkerLocatedError):
            environ.make_fragment(r'''
\begin{figure}
Hello world
\end{figure}
'''.strip())


if __name__ == '__main__':
    unittest.main()
