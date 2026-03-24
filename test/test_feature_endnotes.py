import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer
from flm.feature.endnotes import (
    EndnoteCategory,
    EndnoteInstance,
    FeatureEndnotes,
)


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr_cls=HtmlFragmentRenderer):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


def render_doc_with_endnotes(environ, flm_input, fr_cls=HtmlFragmentRenderer,
                              endnotes_kwargs=None):
    if endnotes_kwargs is None:
        endnotes_kwargs = {}
    frag = environ.make_fragment(flm_input.strip())

    def render_fn(render_context):
        result = frag.render(render_context)
        endnotes_mgr = render_context.feature_render_manager('endnotes')
        endnotes_result = endnotes_mgr.render_endnotes(
            target_id='endnotes', **endnotes_kwargs
        )
        fr = render_context.fragment_renderer
        return result + fr.render_join_blocks([endnotes_result], render_context)

    doc = environ.make_document(render_fn)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


# -------------------------------------------------------------------
# Data classes
# -------------------------------------------------------------------

class TestEndnoteCategory(unittest.TestCase):

    def test_basic_construction(self):
        cat = EndnoteCategory(
            'footnote',
            {'format_num': {'template': '${fnsymbol}'}},
            'Footnotes',
            'footnote'
        )
        self.assertEqual(cat.category_name, 'footnote')
        self.assertEqual(cat.heading_title, 'Footnotes')
        self.assertEqual(cat.endnote_command, 'footnote')

    def test_no_endnote_command(self):
        cat = EndnoteCategory(
            'note',
            {'format_num': {'template': '${arabic}'}},
            'Notes',
        )
        self.assertIsNone(cat.endnote_command)


class TestEndnoteInstance(unittest.TestCase):

    def test_basic_construction(self):
        inst = EndnoteInstance('footnote', 1, 'a', None, None, None)
        self.assertEqual(inst.category_name, 'footnote')
        self.assertEqual(inst.number, 1)
        self.assertEqual(inst.formatted_counter_value_flm, 'a')

    def test_asdict(self):
        inst = EndnoteInstance('footnote', 1, 'a', None, None, None)
        d = inst.asdict()
        self.assertEqual(d['category_name'], 'footnote')
        self.assertEqual(d['number'], 1)
        self.assertEqual(d['formatted_counter_value_flm'], 'a')

    def test_repr(self):
        inst = EndnoteInstance('footnote', 1, 'a', None, None, None)
        r = repr(inst)
        self.assertTrue('EndnoteInstance' in r)
        self.assertTrue('footnote' in r)


# -------------------------------------------------------------------
# FeatureEndnotes init
# -------------------------------------------------------------------

class TestFeatureEndnotesInit(unittest.TestCase):

    def test_feature_name(self):
        fe = FeatureEndnotes(categories=[])
        self.assertEqual(fe.feature_name, 'endnotes')

    def test_with_categories(self):
        cat = EndnoteCategory('footnote', {'format_num': {'template': '${fnsymbol}'}},
                              'Footnotes', 'footnote')
        fe = FeatureEndnotes(categories=[cat])
        self.assertEqual(len(fe.base_categories), 1)
        self.assertEqual(fe.base_categories[0].category_name, 'footnote')

    def test_categories_from_dict(self):
        fe = FeatureEndnotes(categories=[{
            'category_name': 'remark',
            'counter_formatter': {'format_num': {'template': '${arabic}'}},
            'heading_title': 'Remarks',
            'endnote_command': 'remark',
        }])
        self.assertEqual(fe.base_categories[0].category_name, 'remark')

    def test_add_latex_context_definitions(self):
        cat = EndnoteCategory('footnote', {'format_num': {'template': '${fnsymbol}'}},
                              'Footnotes', 'footnote')
        fe = FeatureEndnotes(categories=[cat])
        defs = fe.add_latex_context_definitions()
        self.assertTrue('macros' in defs)
        self.assertEqual(len(defs['macros']), 1)
        self.assertEqual(defs['macros'][0].macroname, 'footnote')

    def test_no_command_no_macro_defined(self):
        cat = EndnoteCategory('note', {'format_num': {'template': '${arabic}'}},
                              'Notes')
        fe = FeatureEndnotes(categories=[cat])
        defs = fe.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 0)


# -------------------------------------------------------------------
# Footnote rendering (HTML)
# -------------------------------------------------------------------

class TestFootnoteRenderingHtml(unittest.TestCase):

    maxDiff = None

    def test_single_footnote(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'Hello\footnote{A note.} world.')
        self.assertEqual(
            result,
            'Hello'
            '<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a>'
            ' world.'
        )

    def test_multiple_footnotes(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'A\footnote{One.} B\footnote{Two.} C\footnote{Three.}'
        )
        self.assertEqual(
            result,
            'A<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a>'
            ' B<a href="#footnote-2" class="href-endnote endnote footnote endnote-marks">b</a>'
            ' C<a href="#footnote-3" class="href-endnote endnote footnote endnote-marks">c</a>'
        )

    def test_footnote_with_endnotes_rendered(self):
        environ = mk_flm_environ()
        result = render_doc_with_endnotes(
            environ, r'Hello\footnote{A note.} world.'
        )
        self.assertEqual(
            result,
            'Hello'
            '<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a>'
            ' world.'
            '<div id="endnotes" class="endnotes">'
            '<dl class="enumeration footnote-list">'
            '<dt id="footnote-1">a</dt><dd>A note.</dd>'
            '</dl></div>'
        )

    def test_endnotes_with_category_heading(self):
        environ = mk_flm_environ()
        result = render_doc_with_endnotes(
            environ,
            r'Hello\footnote{Note.} world.',
            endnotes_kwargs={'include_headings_at_level': 2}
        )
        self.assertEqual(
            result,
            'Hello'
            '<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a>'
            ' world.'
            '<div id="endnotes" class="endnotes">'
            '<h2 class="heading-level-2">Footnotes</h2>\n'
            '<dl class="enumeration footnote-list">'
            '<dt id="footnote-1">a</dt><dd>Note.</dd>'
            '</dl></div>'
        )

    def test_no_endnotes_renders_nothing(self):
        environ = mk_flm_environ()
        result = render_doc_with_endnotes(environ, r'Hello world.')
        self.assertEqual(result, 'Hello world.<!-- no-endnotes -->')


# -------------------------------------------------------------------
# Custom endnote category
# -------------------------------------------------------------------

class TestCustomEndnoteCategory(unittest.TestCase):

    maxDiff = None

    def test_custom_category_arabic_counter(self):
        features = standard_features(endnotes=False)
        custom_cat = EndnoteCategory(
            category_name='remark',
            counter_formatter={'format_num': {'template': '${arabic}'}},
            heading_title='Remarks',
            endnote_command='remark',
        )
        features.append(FeatureEndnotes(categories=[custom_cat]))
        environ = make_standard_environment(features)
        result = render_doc(environ, r'Hello\remark{Custom note.} world.')
        self.assertEqual(
            result,
            'Hello'
            '<a href="#remark-1" class="href-endnote endnote remark endnote-marks">1</a>'
            ' world.'
        )


# -------------------------------------------------------------------
# Other renderers
# -------------------------------------------------------------------

class TestFootnoteTextRenderer(unittest.TestCase):

    maxDiff = None

    def test_footnote_text(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'Hello\footnote{A note.} world.', TextFragmentRenderer)
        self.assertEqual(result, 'Helloa world.')


class TestFootnoteLatexRenderer(unittest.TestCase):

    maxDiff = None

    def test_footnote_latex(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ, r'Hello\footnote{A note.} world.', LatexFragmentRenderer
        )
        self.assertEqual(
            result,
            'Hello\\hyperref[{x:footnote-1}]{a}%\n world.'
        )


class TestFootnoteMarkdownRenderer(unittest.TestCase):

    maxDiff = None

    def test_footnote_markdown(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ, r'Hello\footnote{A note.} world.', MarkdownFragmentRenderer
        )
        self.assertEqual(result, 'Hello[a](#footnote-1) world\\.')


# -------------------------------------------------------------------
# Recomposer
# -------------------------------------------------------------------

class TestEndnotesRecomposer(unittest.TestCase):

    maxDiff = None

    def test_recompose_footnote(self):
        environ = mk_flm_environ()
        frag = environ.make_fragment(r'Hello\footnote{A note.} world.')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(result['latex'], 'Hello\\footnote{A note.} world.')


if __name__ == '__main__':
    unittest.main()
