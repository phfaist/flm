import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer

from flm.feature import annotations as feature_annotations
from flm.feature.annotations import (
    FeatureAnnotations,
    AnnotationMacro,
    AnnotationArgumentParser,
)


def mk_flm_environ(annotation_feature):
    features = standard_features()
    features.append(annotation_feature)
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr_cls=HtmlFragmentRenderer):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


def render_doc_block(environ, flm_input, fr_cls=HtmlFragmentRenderer):
    frag = environ.make_fragment(flm_input.strip(), is_block_level=True)
    doc = environ.make_document(frag.render)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


# ----------------------------------------------------------------
# Init / config
# ----------------------------------------------------------------

class TestFeatureAnnotationsInit(unittest.TestCase):

    def test_feature_name(self):
        feat = FeatureAnnotations()
        self.assertEqual(feat.feature_name, 'annotations')

    def test_default_macrodefs_empty(self):
        feat = FeatureAnnotations()
        self.assertEqual(feat.macrodefs, {})

    def test_default_hide_all_false(self):
        feat = FeatureAnnotations()
        self.assertFalse(feat.hide_all_annotations)

    def test_macrodefs_stored(self):
        feat = FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        )
        self.assertEqual(feat.macrodefs, {'phf': {'initials': 'PhF'}, 'abc': {'initials': 'A.B.C.'}})

    def test_hide_all_annotations_true(self):
        feat = FeatureAnnotations(
            macrodefs=[('x', {'initials': 'X'})],
            hide_all_annotations=True,
        )
        self.assertTrue(feat.hide_all_annotations)

    def test_add_latex_context_definitions_empty(self):
        feat = FeatureAnnotations()
        defs = feat.add_latex_context_definitions()
        self.assertEqual(defs['macros'], [])

    def test_add_latex_context_definitions_two_macros(self):
        feat = FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        )
        defs = feat.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 2)

        m0 = defs['macros'][0]
        self.assertEqual(m0.macroname, 'phf')
        self.assertEqual(m0.initials, 'PhF')
        self.assertEqual(m0.color_index, 0)

        m1 = defs['macros'][1]
        self.assertEqual(m1.macroname, 'abc')
        self.assertEqual(m1.initials, 'A.B.C.')
        self.assertEqual(m1.color_index, 1)

    def test_annotation_macro_allowed_standalone(self):
        self.assertTrue(AnnotationMacro.allowed_in_standalone_mode)

    def test_annotation_macro_fields(self):
        m = AnnotationMacro(macroname='test', initials='T')
        self.assertEqual(m._fields, ('macroname',))

    def test_annotation_argument_parser_endmacroname(self):
        p = AnnotationArgumentParser(macroname='phf')
        self.assertEqual(p.endmacroname, 'endphf')


# ----------------------------------------------------------------
# HTML rendering
# ----------------------------------------------------------------

class TestFeatureAnnotationsHTML(unittest.TestCase):

    maxDiff = None

    def test_highlight_brace_syntax(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\phf{Test highlighted text}')
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-0">'
            '<span class="annotation-initials">PhF</span>'
            'Test highlighted text</span>'
        )

    def test_highlight_endmacro_syntax(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\phf Test highlighted text\endphf')
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-0">'
            '<span class="annotation-initials">PhF</span>'
            'Test highlighted text</span>'
        )

    def test_comment_bracket_syntax(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\phf[Test comment here]')
        self.assertEqual(
            result,
            '<span class="annotation annotation-comment annotation-0">'
            '<span class="annotation-initials">PhF</span>'
            'Test comment here</span>'
        )

    def test_second_annotator_highlight(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\abc{Hello world}')
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-1">'
            '<span class="annotation-initials">A.B.C.</span>'
            'Hello world</span>'
        )

    def test_second_annotator_comment(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\abc[A comment]')
        self.assertEqual(
            result,
            '<span class="annotation annotation-comment annotation-1">'
            '<span class="annotation-initials">A.B.C.</span>'
            'A comment</span>'
        )

    def test_second_annotator_endmacro(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\abc Highlighted text\endabc')
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-1">'
            '<span class="annotation-initials">A.B.C.</span>'
            'Highlighted text</span>'
        )

    def test_inline_with_surrounding_text(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})]
        ))
        result = render_doc(environ, r'Before \phf{highlighted} after.')
        self.assertEqual(
            result,
            'Before <span class="annotation annotation-highlight annotation-0">'
            '<span class="annotation-initials">PhF</span>'
            'highlighted</span> after.'
        )

    def test_inline_comment_with_surrounding_text(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})]
        ))
        result = render_doc(environ, r'Before \phf[comment here] after.')
        self.assertEqual(
            result,
            'Before <span class="annotation annotation-comment annotation-0">'
            '<span class="annotation-initials">PhF</span>'
            'comment here</span> after.'
        )

    def test_no_initials_highlight(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {})]
        ))
        result = render_doc(environ, r'\phf{No initials}')
        self.assertEqual(
            result,
            '<span class="annotation annotation-highlight annotation-0">'
            'No initials</span>'
        )

    def test_no_initials_comment(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {})]
        ))
        result = render_doc(environ, r'\phf[No initials comment]')
        self.assertEqual(
            result,
            '<span class="annotation annotation-comment annotation-0">'
            'No initials comment</span>'
        )

    def test_block_level_highlight(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc_block(environ, r"""\phf
Test highlight
\begin{itemize}
\item one
\end{itemize}
\endphf""")
        self.assertEqual(
            result.replace('\n', ''),
            '<div class="annotation annotation-highlight annotation-0">'
            '<span class="annotation-initials">PhF</span>'
            '<p>Test highlight</p>'
            '<dl class="enumeration itemize"><dt>\u2022</dt><dd><p>one</p></dd></dl>'
            '</div>'
        )

    def test_block_level_comment(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc_block(environ, r'\abc[Test comment here]')
        self.assertEqual(
            result,
            '<p><span class="annotation annotation-comment annotation-1">'
            '<span class="annotation-initials">A.B.C.</span>'
            'Test comment here</span></p>'
        )


# ----------------------------------------------------------------
# hide_all_annotations
# ----------------------------------------------------------------

class TestFeatureAnnotationsHideAll(unittest.TestCase):

    maxDiff = None

    def test_hide_highlight_shows_text_only(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})],
            hide_all_annotations=True,
        ))
        result = render_doc(environ, r'\phf{Hidden highlight}')
        self.assertEqual(result, 'Hidden highlight')

    def test_hide_comment_renders_nothing(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})],
            hide_all_annotations=True,
        ))
        result = render_doc(environ, r'\phf[Hidden comment]')
        self.assertEqual(result, '<!--  -->')

    def test_hide_highlight_latex(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})],
            hide_all_annotations=True,
        ))
        result = render_doc(environ, r'\phf{Hidden}', LatexFragmentRenderer)
        self.assertEqual(result, 'Hidden')

    def test_hide_comment_latex(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})],
            hide_all_annotations=True,
        ))
        result = render_doc(environ, r'\phf[Hidden]', LatexFragmentRenderer)
        self.assertEqual(result, '% \n')


# ----------------------------------------------------------------
# LaTeX renderer
# ----------------------------------------------------------------

class TestFeatureAnnotationsLatex(unittest.TestCase):

    maxDiff = None

    def test_highlight(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})]
        ))
        result = render_doc(environ, r'\phf{Test text}', LatexFragmentRenderer)
        self.assertEqual(result, r'\flmAnnotationHighlight{0}{PhF}{Test text}')

    def test_comment(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})]
        ))
        result = render_doc(environ, r'\phf[Test comment]', LatexFragmentRenderer)
        self.assertEqual(result, r'\flmAnnotationComment{0}{PhF}{Test comment}')

    def test_second_annotator_color_index(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'}), ('abc', {'initials': 'A.B.C.'})]
        ))
        result = render_doc(environ, r'\abc{Abc text}', LatexFragmentRenderer)
        self.assertEqual(result, r'\flmAnnotationHighlight{1}{A.B.C.}{Abc text}')

    def test_no_initials(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {})]
        ))
        result = render_doc(environ, r'\phf{No initials}', LatexFragmentRenderer)
        self.assertEqual(result, r'\flmAnnotationHighlight{0}{No initials}')

    def test_no_initials_comment(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {})]
        ))
        result = render_doc(environ, r'\phf[No initials]', LatexFragmentRenderer)
        self.assertEqual(result, r'\flmAnnotationComment{0}{No initials}')

    def test_endmacro_syntax(self):
        environ = mk_flm_environ(FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})]
        ))
        result = render_doc(environ, r'\phf Highlighted text\endphf', LatexFragmentRenderer)
        self.assertEqual(result, r'\flmAnnotationHighlight{0}{PhF}{Highlighted text}')


# ----------------------------------------------------------------
# RenderManager
# ----------------------------------------------------------------

class TestFeatureAnnotationsRenderManager(unittest.TestCase):

    def test_render_manager_inherits_hide_all_false(self):
        feat = FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})],
            hide_all_annotations=False,
        )
        environ = mk_flm_environ(feat)
        frag = environ.make_fragment(r'\phf{text}'.strip())
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr)
        rdr_mgr = render_context.feature_render_manager('annotations')
        self.assertFalse(rdr_mgr.hide_all_annotations)

    def test_render_manager_inherits_hide_all_true(self):
        feat = FeatureAnnotations(
            macrodefs=[('phf', {'initials': 'PhF'})],
            hide_all_annotations=True,
        )
        environ = mk_flm_environ(feat)
        frag = environ.make_fragment(r'\phf{text}'.strip())
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr)
        rdr_mgr = render_context.feature_render_manager('annotations')
        self.assertTrue(rdr_mgr.hide_all_annotations)


if __name__ == '__main__':
    import logging
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
