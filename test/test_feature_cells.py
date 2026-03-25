import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.feature.cells import (
    CellIndexRangeModel,
    CellPlacementModel,
    CellPlacementsMappingModel,
    CellsModel,
    CellsEnvironment,
    FeatureCells,
    CellMacro,
    CelldataMacroSpec,
    MergeMacroSpec,
    LatexTabularRowSeparatorSpec,
    LatexTabularColumnSeparatorSpec,
    _splfysidews,
)
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    features.append(FeatureCells())
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr_cls=HtmlFragmentRenderer):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


# ---------------------------------------------------------------
# Data model unit tests
# ---------------------------------------------------------------


class TestCellIndexRangeModel(unittest.TestCase):

    def test_single_cell_repr(self):
        r = CellIndexRangeModel(0, 1)
        self.assertEqual(repr(r), '1')

    def test_multi_cell_repr(self):
        r = CellIndexRangeModel(0, 3)
        self.assertEqual(repr(r), '[1-3]')

    def test_fields(self):
        r = CellIndexRangeModel(2, 5)
        self.assertEqual(r.start, 2)
        self.assertEqual(r.end, 5)
        self.assertEqual(r._fields, ('start', 'end',))


class TestCellPlacementModel(unittest.TestCase):

    def test_repr(self):
        row = CellIndexRangeModel(0, 1)
        col = CellIndexRangeModel(0, 3)
        p = CellPlacementModel(row, col)
        self.assertEqual(repr(p), '1,[1-3]')

    def test_fields(self):
        row = CellIndexRangeModel(1, 2)
        col = CellIndexRangeModel(3, 4)
        p = CellPlacementModel(row, col)
        self.assertEqual(p.row_range.start, 1)
        self.assertEqual(p.col_range.start, 3)
        self.assertEqual(p._fields, ('row_range', 'col_range',))


class TestCellPlacementsMappingModel(unittest.TestCase):

    def test_empty_placements_row(self):
        pm = CellPlacementsMappingModel([], [])
        ri = pm.get_row_index_range(0, current_row=2)
        self.assertEqual(ri.start, 2)
        self.assertEqual(ri.end, 3)

    def test_empty_placements_col(self):
        pm = CellPlacementsMappingModel([], [])
        ci = pm.get_col_index_range(0, current_col=5)
        self.assertEqual(ci.start, 5)
        self.assertEqual(ci.end, 6)

    def test_start_row_col(self):
        pm = CellPlacementsMappingModel([], [])
        sr, sc = pm.start_row_col(current_row=3, current_col=7)
        self.assertEqual(sr, 3)
        self.assertEqual(sc, 7)

    def test_with_placements_row(self):
        pm = CellPlacementsMappingModel(
            [CellIndexRangeModel(0, 1), CellIndexRangeModel(2, 3)],
            []
        )
        r0 = pm.get_row_index_range(0)
        self.assertEqual(r0.start, 0)
        self.assertEqual(r0.end, 1)
        r1 = pm.get_row_index_range(1)
        self.assertEqual(r1.start, 2)
        self.assertEqual(r1.end, 3)

    def test_open_ended_col_range(self):
        pm = CellPlacementsMappingModel(
            [],
            [CellIndexRangeModel(0, 1), CellIndexRangeModel(1, 2),
             CellIndexRangeModel(2, None)]
        )
        c2 = pm.get_col_index_range(2)
        self.assertEqual(c2.start, 2)
        self.assertEqual(c2.end, 3)
        c3 = pm.get_col_index_range(3)
        self.assertEqual(c3.start, 3)
        self.assertEqual(c3.end, 4)
        c4 = pm.get_col_index_range(4)
        self.assertEqual(c4.start, 4)
        self.assertEqual(c4.end, 5)

    def test_repr(self):
        pm = CellPlacementsMappingModel([], [])
        r = repr(pm)
        self.assertTrue('CellPlacementsMappingModel(' in r)
        self.assertTrue('row_placements=' in r)
        self.assertTrue('col_placements=' in r)


class TestSplfysidews(unittest.TestCase):

    def test_strips_side_whitespace(self):
        self.assertEqual(_splfysidews('  hello  '), ' hello ')

    def test_no_side_whitespace(self):
        self.assertEqual(_splfysidews('hello'), 'hello')

    def test_preserves_inner_whitespace(self):
        self.assertEqual(_splfysidews('  a b c  '), ' a b c ')


# ---------------------------------------------------------------
# CellsModel parsing tests (index, range, placement specs)
# ---------------------------------------------------------------


class TestCellsModelParsing(unittest.TestCase):

    def test_parse_cell_index_integer(self):
        m = CellsModel()
        self.assertEqual(m.parse_cell_index_spec('5', is_row=True), 4)
        self.assertEqual(m.parse_cell_index_spec('1', is_col=True), 0)

    def test_parse_cell_index_empty_returns_current(self):
        m = CellsModel()
        m.current_row = 2
        m.current_col = 3
        self.assertEqual(m.parse_cell_index_spec('', is_row=True), 2)
        self.assertEqual(m.parse_cell_index_spec('.', is_row=True), 2)
        self.assertEqual(m.parse_cell_index_spec('', is_col=True), 3)
        self.assertEqual(m.parse_cell_index_spec('.', is_col=True), 3)

    def test_parse_cell_index_with_default(self):
        m = CellsModel()
        self.assertEqual(m.parse_cell_index_spec('', is_row=True, default=99), 99)

    def test_parse_cell_index_invalid(self):
        m = CellsModel()
        with self.assertRaises(ValueError):
            m.parse_cell_index_spec('nonexistent', is_row=True)

    def test_parse_range_single(self):
        m = CellsModel()
        self.assertEqual(m.parse_cell_index_range_spec('3', is_row=True), (2, 3))

    def test_parse_range_dash(self):
        m = CellsModel()
        self.assertEqual(
            m.parse_cell_index_range_spec('2-4', is_row=True), (1, 4)
        )

    def test_parse_range_plus(self):
        m = CellsModel()
        self.assertEqual(
            m.parse_cell_index_range_spec('2+3', is_row=True), (1, 4)
        )

    def test_parse_range_comma_contiguous(self):
        m = CellsModel()
        self.assertEqual(
            m.parse_cell_index_range_spec('1,2', is_row=True), (0, 2)
        )

    def test_parse_range_comma_non_contiguous_raises(self):
        m = CellsModel()
        with self.assertRaises(ValueError):
            m.parse_cell_index_range_spec('1,3', is_row=True)

    def test_parse_range_plus_invalid_number(self):
        m = CellsModel()
        with self.assertRaises(ValueError):
            m.parse_cell_index_range_spec('.+abc', is_row=True)

    def test_parse_range_dash_defaults(self):
        m = CellsModel()
        self.assertEqual(
            m.parse_cell_index_range_spec(
                '-', is_row=True, default_start=0, default_end=5
            ),
            (0, 5)
        )


class TestCellsModelOperations(unittest.TestCase):

    def test_init(self):
        m = CellsModel()
        self.assertEqual(m.current_row, 0)
        self.assertEqual(m.current_col, 0)
        self.assertEqual(m.cells_size, [None, None])
        self.assertEqual(m.cells_data, [])
        self.assertIsNone(m.grid_data)

    def test_move_next_row(self):
        m = CellsModel()
        m.current_col = 3
        m.move_next_row()
        self.assertEqual(m.current_row, 1)
        self.assertEqual(m.current_col, 0)

    def test_move_to_col(self):
        m = CellsModel()
        m.move_to_col(5)
        self.assertEqual(m.current_col, 5)

    def test_repr(self):
        m = CellsModel()
        r = repr(m)
        self.assertTrue('CellsModel(' in r)
        self.assertTrue('cells_size=' in r)
        self.assertTrue('cells_data=' in r)


# ---------------------------------------------------------------
# Spec classes
# ---------------------------------------------------------------


class TestSpecClasses(unittest.TestCase):

    def test_cells_environment_attrs(self):
        ce = CellsEnvironment()
        self.assertTrue(ce.is_block_level)
        self.assertTrue(ce.allowed_in_standalone_mode)
        self.assertTrue(ce.body_contents_is_block_level)
        self.assertEqual(ce.environmentname, 'cells')

    def test_cells_environment_custom_name(self):
        ce = CellsEnvironment(environmentname='mytable')
        self.assertEqual(ce.environmentname, 'mytable')

    def test_cell_macro(self):
        cm = CellMacro()
        self.assertEqual(cm.macroname, 'cell')

    def test_celldata_macro_spec(self):
        cd = CelldataMacroSpec()
        self.assertEqual(cd.macroname, 'celldata')

    def test_merge_macro_spec(self):
        ms = MergeMacroSpec()
        self.assertEqual(ms.macroname, 'merge')

    def test_row_separator(self):
        rs = LatexTabularRowSeparatorSpec()
        self.assertEqual(rs.macroname, '\\')

    def test_col_separator(self):
        cs = LatexTabularColumnSeparatorSpec()
        self.assertEqual(cs.specials_chars, '&')

    def test_feature_cells_init(self):
        fc = FeatureCells()
        self.assertEqual(fc.feature_name, 'cells')
        self.assertEqual(fc.feature_title, 'Typesetting data tables')
        self.assertIsNone(fc.DocumentManager)
        self.assertIsNone(fc.RenderManager)

    def test_feature_cells_latex_context_defs(self):
        fc = FeatureCells()
        defs = fc.add_latex_context_definitions()
        self.assertEqual(len(defs['environments']), 1)
        self.assertEqual(defs['environments'][0].environmentname, 'cells')


# ---------------------------------------------------------------
# HTML rendering tests
# ---------------------------------------------------------------


class TestCellsHtmlRendering(unittest.TestCase):

    maxDiff = None

    def test_basic_2x2_cell(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\cell{A} \cell{B}
\\
\cell{C} \cell{D}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<td class="cell celltbledge-top celltbledge-left"><p>A</p></td>'
            '<td class="cell celltbledge-top celltbledge-right"><p>B</p></td>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>C</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>D</p></td>'
            '</tr>'
            '</table>'
        )

    def test_celldata_2x3(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata{
A & B & C \\
D & E & F
}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<td class="cell celltbledge-top celltbledge-left"><p>A</p></td>'
            '<td class="cell celltbledge-top"><p>B</p></td>'
            '<td class="cell celltbledge-top celltbledge-right"><p>C</p></td>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>D</p></td>'
            '<td class="cell celltbledge-bottom"><p>E</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>F</p></td>'
            '</tr>'
            '</table>'
        )

    def test_header_and_data(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata<H>{Name & City & Height}
\celldata{John & Berlin & 1m80}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>Name</p></th>'
            '<th class="cell cellstyle-H celltbledge-top"><p>City</p></th>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>Height</p></th>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>John</p></td>'
            '<td class="cell celltbledge-bottom"><p>Berlin</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>1m80</p></td>'
            '</tr>'
            '</table>'
        )

    def test_celldata_with_merge(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata{
A & B & C \\
\cell[\merge{1-2}]{DE} & F
}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<td class="cell celltbledge-top celltbledge-left"><p>A</p></td>'
            '<td class="cell celltbledge-top"><p>B</p></td>'
            '<td class="cell celltbledge-top celltbledge-right"><p>C</p></td>'
            '</tr>'
            '<tr>'
            '<td colspan="2" class="cell celltbledge-left celltbledge-bottom"><p>DE</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>F</p></td>'
            '</tr>'
            '</table>'
        )

    def test_styles_on_cells(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\cell<H>{Name} \cell<H>{City}
\\
\cell{John} \cell<c>{Berlin}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>Name</p></th>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>City</p></th>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>John</p></td>'
            '<td class="cell cellstyle-c celltbledge-bottom celltbledge-right"><p>Berlin</p></td>'
            '</tr>'
            '</table>'
        )

    def test_celldata_with_placement_mapping(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata<H>{A & B & C}
\celldata[-]{
D & E & F \\
G & H & I
}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>A</p></th>'
            '<th class="cell cellstyle-H celltbledge-top"><p>B</p></th>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>C</p></th>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left"><p>D</p></td>'
            '<td class="cell"><p>E</p></td>'
            '<td class="cell celltbledge-right"><p>F</p></td>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>G</p></td>'
            '<td class="cell celltbledge-bottom"><p>H</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>I</p></td>'
            '</tr>'
            '</table>'
        )

    def test_celldata_multiple_merges(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata[-]{
A & B & C \\
\cell[\merge{1-2}]{DE} & F \\
\cell[\merge{+2}]{XY} & Z
}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<td class="cell celltbledge-top celltbledge-left"><p>A</p></td>'
            '<td class="cell celltbledge-top"><p>B</p></td>'
            '<td class="cell celltbledge-top celltbledge-right"><p>C</p></td>'
            '</tr>'
            '<tr>'
            '<td colspan="2" class="cell celltbledge-left"><p>DE</p></td>'
            '<td class="cell celltbledge-right"><p>F</p></td>'
            '</tr>'
            '<tr>'
            '<td colspan="2" class="cell celltbledge-left celltbledge-bottom"><p>XY</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>Z</p></td>'
            '</tr>'
            '</table>'
        )

    def test_3x1_table(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\cell{One}
\\
\cell{Two}
\\
\cell{Three}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<td class="cell celltbledge-top celltbledge-left celltbledge-right"><p>One</p></td>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-right"><p>Two</p></td>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom celltbledge-right"><p>Three</p></td>'
            '</tr>'
            '</table>'
        )

    def test_complex_header_styles_merge(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata<H>{One & Two & Three}
\celldata<c>[-]{
A & B & C \\
\cell<red>[\merge{1-2}]{DE} & F
}
\cell<green>{X}
\cell<yellow>[\merge{+2}]{YZ}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>One</p></th>'
            '<th class="cell cellstyle-H celltbledge-top"><p>Two</p></th>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>Three</p></th>'
            '</tr>'
            '<tr>'
            '<td class="cell cellstyle-c celltbledge-left"><p>A</p></td>'
            '<td class="cell cellstyle-c"><p>B</p></td>'
            '<td class="cell cellstyle-c celltbledge-right"><p>C</p></td>'
            '</tr>'
            '<tr>'
            '<td colspan="2" class="cell cellstyle-red cellstyle-c celltbledge-left"><p>DE</p></td>'
            '<td class="cell cellstyle-c celltbledge-right"><p>F</p></td>'
            '</tr>'
            '<tr>'
            '<td class="cell cellstyle-green celltbledge-left celltbledge-bottom"><p>X</p></td>'
            '<td colspan="2" class="cell cellstyle-yellow celltbledge-bottom celltbledge-right"><p>YZ</p></td>'
            '</tr>'
            '</table>'
        )

    def test_multi_style_celldata(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata<H c>{One & Two}
\celldata{A & B}
\end{cells}''')
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H cellstyle-c celltbledge-top celltbledge-left"><p>One</p></th>'
            '<th class="cell cellstyle-H cellstyle-c celltbledge-top celltbledge-right"><p>Two</p></th>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>A</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>B</p></td>'
            '</tr>'
            '</table>'
        )


# ---------------------------------------------------------------
# LaTeX renderer
# ---------------------------------------------------------------


class TestCellsLatexRendering(unittest.TestCase):

    maxDiff = None

    def test_simple_table(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata<H>{Name & City}
\celldata{John & Berlin}
\end{cells}''', fr_cls=LatexFragmentRenderer)
        self.assertEqual(
            result,
            '\\flmCellsBeginCenter\n'
            '\\long\\def\\flmTempTypesetThisTable#1{%\n'
            '\\begin{tblr}{#1,\n'
            '  hspan=minimal,\n'
            '  cell{1}{1}={}{m, font={\\flmCellsHeaderFont}},\n'
            '  cell{1}{2}={}{m, font={\\flmCellsHeaderFont}},\n'
            '  hline{2}={1}{.4pt,solid},\n'
            '  hline{2}={2}{.4pt,solid}}%\n'
            '\\toprule\n'
            'Name\n'
            '&City\n'
            '\\\\\n'
            'John\n'
            '&Berlin\n'
            '\\\\\n'
            '\\bottomrule\n'
            '\\end{tblr}%\n'
            '}%\n'
            '\\def\\flmTmpMaxW{\\dimexpr 0.96\\linewidth\\relax}%\n'
            '\\setbox0=\\hbox{\\flmTempTypesetThisTable{colspec={cc}}}%\n'
            '\\ifdim\\wd0<\\flmTmpMaxW\\relax\n'
            '  \\leavevmode\\box0 \n'
            '\\else\n'
            '  \\flmTempTypesetThisTable{width=\\flmTmpMaxW,colspec={X[-1]X[-1]}}\n'
            '\\fi\n'
            '\\flmCellsEndCenter\n'
        )


# ---------------------------------------------------------------
# Markdown renderer (falls back to HTML for cells)
# ---------------------------------------------------------------


class TestCellsMarkdownRendering(unittest.TestCase):

    maxDiff = None

    def test_simple_table(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'''\begin{cells}
\celldata<H>{Name & City}
\celldata{John & Berlin}
\end{cells}''', fr_cls=MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '<table class="cells">'
            '<tr>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>Name</p></th>'
            '<th class="cell cellstyle-H celltbledge-top celltbledge-right"><p>City</p></th>'
            '</tr>'
            '<tr>'
            '<td class="cell celltbledge-left celltbledge-bottom"><p>John</p></td>'
            '<td class="cell celltbledge-bottom celltbledge-right"><p>Berlin</p></td>'
            '</tr>'
            '</table>'
        )


# ---------------------------------------------------------------
# Error cases
# ---------------------------------------------------------------


class TestCellsErrors(unittest.TestCase):

    def test_overlapping_cells_raises(self):
        environ = mk_flm_environ()
        with self.assertRaises(Exception):
            render_doc(environ, r'''\begin{cells}
\cell{A}
\cell[1]{B}
\end{cells}''')

    def test_invalid_content_in_cells_raises(self):
        environ = mk_flm_environ()
        with self.assertRaises(Exception):
            render_doc(environ, r'''\begin{cells}
Hello world
\end{cells}''')


# ---------------------------------------------------------------
# Recomposer tests
# ---------------------------------------------------------------


class TestCellsRecomposer(unittest.TestCase):

    maxDiff = None

    def test_keep_as_is(self):
        environ = mk_flm_environ()
        src = (r'\begin{cells}' '\n'
               r'\celldata<H>{Name & City}' '\n'
               r'\celldata{John & Berlin}' '\n'
               r'\end{cells}')
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({'cells': {'keep_as_is': True}})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            '\\begin{cells}\n'
            '\\celldata<H>{Name & City}\n'
            '\\celldata{John & Berlin}\n'
            '\\end{cells}'
        )

    def test_no_render_context_raises(self):
        environ = mk_flm_environ()
        src = (r'\begin{cells}' '\n'
               r'\celldata{A & B}' '\n'
               r'\end{cells}')
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        with self.assertRaises(ValueError):
            recomposer.recompose_pure_latex(frag.nodes)

    def test_with_latex_renderer(self):
        environ = mk_flm_environ()
        src = (r'\begin{cells}' '\n'
               r'\celldata<H>{Name & City}' '\n'
               r'\celldata{John & Berlin}' '\n'
               r'\end{cells}')
        frag = environ.make_fragment(src)
        doc = environ.make_document(frag.render)
        fr = LatexFragmentRenderer()
        _rendered, render_context = doc.render(fr)
        recomposer = FLMPureLatexRecomposer({})
        recomposer.render_context = render_context
        result = recomposer.recompose_pure_latex(frag.nodes)
        # The recomposer with a LatexFragmentRenderer produces tblr output
        self.assertTrue('\\begin{tblr}' in result['latex'])
        self.assertTrue('Name' in result['latex'])
        self.assertTrue('City' in result['latex'])


if __name__ == '__main__':
    unittest.main()
