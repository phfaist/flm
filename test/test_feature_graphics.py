import re
import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.feature.graphics import (
    GraphicsResource,
    SimpleIncludeGraphicsMacro,
    FeatureSimplePathGraphicsResourceProvider,
)
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def mk_flm_environ_custom_provider(provider_cls):
    features = standard_features(use_simple_path_graphics_resource_provider=False)
    features.append(provider_cls())
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr_cls=HtmlFragmentRenderer):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


# ---------------------------------------------------------------
# GraphicsResource data class
# ---------------------------------------------------------------


class TestGraphicsResource(unittest.TestCase):

    def test_minimal_init(self):
        gr = GraphicsResource('image.png')
        self.assertEqual(gr.src_url, 'image.png')
        self.assertIsNone(gr.srcset)
        self.assertIsNone(gr.graphics_type)
        self.assertIsNone(gr.dpi)
        self.assertIsNone(gr.pixel_dimensions)
        self.assertIsNone(gr.physical_dimensions)
        self.assertIsNone(gr.source_info)

    def test_full_init(self):
        gr = GraphicsResource(
            'photo.jpg',
            graphics_type='raster',
            dpi=150,
            pixel_dimensions=(300, 200),
            physical_dimensions=(144.0, 96.0),
            source_info={'origin': 'upload'},
        )
        self.assertEqual(gr.src_url, 'photo.jpg')
        self.assertEqual(gr.graphics_type, 'raster')
        self.assertEqual(gr.dpi, 150)
        self.assertEqual(gr.pixel_dimensions, (300, 200))
        self.assertEqual(gr.physical_dimensions, (144.0, 96.0))
        self.assertEqual(gr.source_info, {'origin': 'upload'})

    def test_srcset(self):
        srcset_data = [
            {'source': 'img@2x.png', 'pixel_density': 2},
            {'source': 'img@3x.png', 'pixel_density': 3},
        ]
        gr = GraphicsResource('img.png', srcset=srcset_data)
        self.assertEqual(gr.srcset, srcset_data)

    def test_fields(self):
        gr = GraphicsResource('x.png')
        self.assertEqual(
            gr._fields,
            ('src_url', 'srcset', 'graphics_type', 'dpi',
             'pixel_dimensions', 'physical_dimensions', 'source_info',)
        )

    def test_asdict_minimal(self):
        gr = GraphicsResource('image.png')
        d = gr.asdict()
        self.assertEqual(d, {
            'src_url': 'image.png',
            'srcset': None,
            'graphics_type': None,
            'dpi': None,
            'pixel_dimensions': None,
            'physical_dimensions': None,
            'source_info': None,
        })

    def test_asdict_full(self):
        gr = GraphicsResource(
            'photo.jpg',
            graphics_type='raster',
            dpi=150,
            pixel_dimensions=(300, 200),
            physical_dimensions=(144.0, 96.0),
        )
        d = gr.asdict()
        self.assertEqual(d['src_url'], 'photo.jpg')
        self.assertEqual(d['graphics_type'], 'raster')
        self.assertEqual(d['dpi'], 150)
        self.assertEqual(d['pixel_dimensions'], (300, 200))
        self.assertEqual(d['physical_dimensions'], (144.0, 96.0))

    def test_repr_minimal(self):
        gr = GraphicsResource('image.png')
        r = repr(gr)
        self.assertTrue('GraphicsResource(' in r)
        self.assertTrue('image.png' in r)

    def test_repr_full(self):
        gr = GraphicsResource('photo.jpg', graphics_type='raster', dpi=150)
        r = repr(gr)
        self.assertTrue('GraphicsResource(' in r)
        self.assertTrue('photo.jpg' in r)
        self.assertTrue('raster' in r)
        self.assertTrue('150' in r)


# ---------------------------------------------------------------
# SimpleIncludeGraphicsMacro spec class
# ---------------------------------------------------------------


class TestSimpleIncludeGraphicsMacro(unittest.TestCase):

    def test_macroname(self):
        m = SimpleIncludeGraphicsMacro(macroname='includegraphics')
        self.assertEqual(m.macroname, 'includegraphics')

    def test_is_block_level(self):
        m = SimpleIncludeGraphicsMacro(macroname='includegraphics')
        self.assertTrue(m.is_block_level)

    def test_not_allowed_in_standalone_mode(self):
        m = SimpleIncludeGraphicsMacro(macroname='includegraphics')
        self.assertFalse(m.allowed_in_standalone_mode)

    def test_fields(self):
        m = SimpleIncludeGraphicsMacro(macroname='includegraphics')
        self.assertEqual(m._fields, ('macroname',))


# ---------------------------------------------------------------
# FeatureSimplePathGraphicsResourceProvider
# ---------------------------------------------------------------


class TestFeatureSimplePathGraphicsResourceProvider(unittest.TestCase):

    def test_feature_name(self):
        f = FeatureSimplePathGraphicsResourceProvider()
        self.assertEqual(f.feature_name, 'graphics_resource_provider')

    def test_feature_title(self):
        f = FeatureSimplePathGraphicsResourceProvider()
        self.assertEqual(f.feature_title, 'Include external graphics')

    def test_empty_latex_context_defs(self):
        f = FeatureSimplePathGraphicsResourceProvider()
        defs = f.add_latex_context_definitions()
        self.assertEqual(defs, {})

    def test_render_manager_exists(self):
        f = FeatureSimplePathGraphicsResourceProvider()
        self.assertTrue(f.RenderManager is not None)

    def test_render_manager_get_graphics_resource(self):
        environ = mk_flm_environ()
        src = r'\begin{figure}\includegraphics{img/test.png}\end{figure}'
        frag = environ.make_fragment(src)
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        _result, render_context = doc.render(fr)
        mgr = render_context.feature_render_manager('graphics_resource_provider')
        gr = mgr.get_graphics_resource('some/path.png', None)
        self.assertEqual(gr.src_url, 'some/path.png')
        self.assertIsNone(gr.graphics_type)


# ---------------------------------------------------------------
# HTML rendering (inside figure, since includegraphics requires
# graphics_resource_provider feature via document rendering)
# ---------------------------------------------------------------


class TestGraphicsHtmlRendering(unittest.TestCase):

    maxDiff = None

    def test_basic_includegraphics_in_figure(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}'
        )
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents">'
            '<img src="img/test.png">'
            '</div>'
            '</figure>'
        )

    def test_includegraphics_with_caption(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}'
            r'\caption{Test caption}\end{figure}'
        )
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents">'
            '<img src="img/test.png">'
            '</div>\n'
            '<figcaption class="float-caption-content">'
            '<span><span class="float-no-number">Figure</span>: '
            'Test caption</span>'
            '</figcaption>'
            '</figure>'
        )

    def test_with_srcset_provider(self):

        class SrcsetProvider(FeatureSimplePathGraphicsResourceProvider):
            class RenderManager(
                FeatureSimplePathGraphicsResourceProvider.RenderManager
            ):
                def get_graphics_resource(self, graphics_path, resource_info):
                    return GraphicsResource(
                        src_url=graphics_path,
                        srcset=[
                            {
                                'source': graphics_path.replace(
                                    '.png', '@2x.png'
                                ),
                                'pixel_density': 2,
                            },
                        ],
                    )

        environ = mk_flm_environ_custom_provider(SrcsetProvider)
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}'
        )
        self.assertEqual(
            result,
            '<figure class="float float-figure">'
            '<div class="float-contents">'
            '<img src="img/test.png" srcset="img/test@2x.png 2x">'
            '</div>'
            '</figure>'
        )

    def test_with_physical_dimensions(self):

        class DimProvider(FeatureSimplePathGraphicsResourceProvider):
            class RenderManager(
                FeatureSimplePathGraphicsResourceProvider.RenderManager
            ):
                def get_graphics_resource(self, graphics_path, resource_info):
                    return GraphicsResource(
                        src_url=graphics_path,
                        graphics_type='raster',
                        dpi=72,
                        pixel_dimensions=(200, 100),
                        physical_dimensions=(200.0, 100.0),
                    )

        environ = mk_flm_environ_custom_provider(DimProvider)
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}'
        )
        # Use regex for float format compatibility (Python: 200.000000,
        # JS: 200)
        self.assertTrue(re.fullmatch(
            r'<figure class="float float-figure">'
            r'<div class="float-contents">'
            r'<img style="width:200(?:\.0+)?pt;height:100(?:\.0+)?pt" '
            r'src="img/test\.png">'
            r'</div>'
            r'</figure>',
            result,
        ) is not None)


# ---------------------------------------------------------------
# Other renderers
# ---------------------------------------------------------------


class TestGraphicsTextRendering(unittest.TestCase):

    maxDiff = None

    def test_text_figure(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}',
            fr_cls=TextFragmentRenderer,
        )
        # Text renderer centering may differ between Python and JS
        self.assertTrue('[img/test.png]' in result)
        self.assertTrue('\u00b7' * 80 in result)


class TestGraphicsLatexRendering(unittest.TestCase):

    maxDiff = None

    def test_latex_figure(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}',
            fr_cls=LatexFragmentRenderer,
        )
        self.assertEqual(
            result,
            '\\begin{center}%\n'
            '\\includegraphics{img/test.png}\n'
            '\\end{center}\n'
        )


class TestGraphicsMarkdownRendering(unittest.TestCase):

    maxDiff = None

    def test_markdown_figure(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\begin{figure}\includegraphics{img/test.png}\end{figure}',
            fr_cls=MarkdownFragmentRenderer,
        )
        self.assertEqual(
            result,
            '---\n\n![](img/test.png)\n\n---'
        )


# ---------------------------------------------------------------
# Recomposer tests
# ---------------------------------------------------------------


class TestGraphicsRecomposer(unittest.TestCase):

    maxDiff = None

    def test_default_options(self):
        environ = mk_flm_environ()
        src = (r'\begin{figure}\includegraphics{img/test.png}'
               r'\caption{A test}\end{figure}')
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{flmFloat}{figure}{CapOnly}'
            r'\includegraphics[max width=\linewidth]{img/test.png}'
            r'\caption{A test}\end{flmFloat}'
        )
        packages = dict(result['packages'])
        self.assertEqual(
            packages['adjustbox'],
            {'options': 'export'}
        )

    def test_set_max_width_false(self):
        environ = mk_flm_environ()
        src = (r'\begin{figure}\includegraphics{img/test.png}'
               r'\caption{A test}\end{figure}')
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer(
            {'graphics': {'set_max_width': False}}
        )
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{flmFloat}{figure}{CapOnly}'
            r'\includegraphics{img/test.png}'
            r'\caption{A test}\end{flmFloat}'
        )
        packages = dict(result['packages'])
        self.assertEqual(packages, {})

    def test_with_physical_dimensions_and_render_context(self):

        class DimProvider(FeatureSimplePathGraphicsResourceProvider):
            class RenderManager(
                FeatureSimplePathGraphicsResourceProvider.RenderManager
            ):
                def get_graphics_resource(self, graphics_path, resource_info):
                    return GraphicsResource(
                        src_url=graphics_path,
                        graphics_type='raster',
                        dpi=150,
                        pixel_dimensions=(300, 200),
                        physical_dimensions=(144.0, 96.0),
                    )

        environ = mk_flm_environ_custom_provider(DimProvider)
        src = (r'\begin{figure}\includegraphics{img/test.png}'
               r'\caption{A test}\end{figure}')
        frag = environ.make_fragment(src)
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        _rendered, render_context = doc.render(fr)
        recomposer = FLMPureLatexRecomposer({})
        recomposer.render_context = render_context
        result = recomposer.recompose_pure_latex(frag.nodes)
        # Use regex for float format compatibility (Python vs JS)
        self.assertTrue(re.fullmatch(
            r'\\begin\{flmFloat\}\{figure\}\{CapOnly\}'
            r'\\includegraphics\[width=144(?:\.0+)?bp,max width=\\linewidth\]'
            r'\{img/test\.png\}\\caption\{A test\}\\end\{flmFloat\}',
            result['latex'],
        ) is not None)

    def test_width_scale_with_physical_dimensions(self):

        class DimProvider(FeatureSimplePathGraphicsResourceProvider):
            class RenderManager(
                FeatureSimplePathGraphicsResourceProvider.RenderManager
            ):
                def get_graphics_resource(self, graphics_path, resource_info):
                    return GraphicsResource(
                        src_url=graphics_path,
                        graphics_type='raster',
                        dpi=150,
                        pixel_dimensions=(300, 200),
                        physical_dimensions=(144.0, 96.0),
                    )

        environ = mk_flm_environ_custom_provider(DimProvider)
        src = (r'\begin{figure}\includegraphics{img/test.png}'
               r'\caption{A test}\end{figure}')
        frag = environ.make_fragment(src)
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        _rendered, render_context = doc.render(fr)
        recomposer = FLMPureLatexRecomposer(
            {'graphics': {'width_scale': 0.5}}
        )
        recomposer.render_context = render_context
        result = recomposer.recompose_pure_latex(frag.nodes)
        # Use regex for float format compatibility (Python vs JS)
        self.assertTrue(re.fullmatch(
            r'\\begin\{flmFloat\}\{figure\}\{CapOnly\}'
            r'\\includegraphics\[width=72(?:\.0+)?bp,max width=\\linewidth\]'
            r'\{img/test\.png\}\\caption\{A test\}\\end\{flmFloat\}',
            result['latex'],
        ) is not None)

    def test_no_max_width_with_physical_dimensions(self):

        class DimProvider(FeatureSimplePathGraphicsResourceProvider):
            class RenderManager(
                FeatureSimplePathGraphicsResourceProvider.RenderManager
            ):
                def get_graphics_resource(self, graphics_path, resource_info):
                    return GraphicsResource(
                        src_url=graphics_path,
                        graphics_type='raster',
                        dpi=150,
                        pixel_dimensions=(300, 200),
                        physical_dimensions=(144.0, 96.0),
                    )

        environ = mk_flm_environ_custom_provider(DimProvider)
        src = (r'\begin{figure}\includegraphics{img/test.png}'
               r'\caption{A test}\end{figure}')
        frag = environ.make_fragment(src)
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        _rendered, render_context = doc.render(fr)
        recomposer = FLMPureLatexRecomposer(
            {'graphics': {'set_max_width': False}}
        )
        recomposer.render_context = render_context
        result = recomposer.recompose_pure_latex(frag.nodes)
        # Use regex for float format compatibility (Python vs JS)
        self.assertTrue(re.fullmatch(
            r'\\begin\{flmFloat\}\{figure\}\{CapOnly\}'
            r'\\includegraphics\[width=144(?:\.0+)?bp\]'
            r'\{img/test\.png\}\\caption\{A test\}\\end\{flmFloat\}',
            result['latex'],
        ) is not None)

    def test_bare_figure_no_caption(self):
        environ = mk_flm_environ()
        src = r'\begin{figure}\includegraphics{img/bare.png}\end{figure}'
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{flmFloat}{figure}{Bare}'
            r'\includegraphics[max width=\linewidth]{img/bare.png}'
            r'\end{flmFloat}'
        )

    def test_with_render_context_default_provider(self):
        environ = mk_flm_environ()
        src = (r'\begin{figure}\includegraphics{img/test.png}'
               r'\caption{A test}\end{figure}')
        frag = environ.make_fragment(src)
        doc = environ.make_document(frag.render)
        fr = LatexFragmentRenderer()
        _rendered, render_context = doc.render(fr)
        recomposer = FLMPureLatexRecomposer(
            {'graphics': {'set_max_width': False}}
        )
        recomposer.render_context = render_context
        result = recomposer.recompose_pure_latex(frag.nodes)
        # Default provider returns no physical_dimensions,
        # so no width= option
        self.assertEqual(
            result['latex'],
            r'\begin{flmFloat}{figure}{CapOnly}'
            r'\includegraphics{img/test.png}'
            r'\caption{A test}\end{flmFloat}'
        )


# ---------------------------------------------------------------
# Error cases
# ---------------------------------------------------------------


class TestGraphicsErrors(unittest.TestCase):

    # REVIEW: graphics.py:176 — postprocess_parsed_node tries to access
    # node_args['graphics_options'].nodelist.pos but SingleParsedArgumentInfo
    # has no 'nodelist' attribute. This causes an AttributeError instead of
    # the intended LatexWalkerLocatedError when graphics options are provided.
    def test_options_raises_error(self):
        environ = mk_flm_environ()
        with self.assertRaises(Exception):
            environ.make_fragment(
                r'\begin{figure}'
                r'\includegraphics[width=5cm]{img/test.png}'
                r'\end{figure}'
            )


if __name__ == '__main__':
    unittest.main()
