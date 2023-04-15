import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerLocatedError, ParsedArgumentsInfo
from pylatexenc.latexnodes import parsers as latexnodes_parsers

from ..flmspecinfo import FLMMacroSpecBase
from ..flmenvironment import FLMArgumentSpec
from ._base import Feature


class GraphicsResource:
    def __init__(
            self,
            src_url, # a string, e.g., path or full URL to image location
            *,
            srcset=None, # possible alternative images for <img srcset=...>
            graphics_type=None, # 'raster' or 'vector'
            dpi=None,
            pixel_dimensions=None, # (width_px, height_px) # in pixels
            physical_dimensions=None, # (width_pt, height_pt) #  1 pt = 1/72 inch
    ):
        super().__init__()
        self.src_url = src_url
        self.srcset = srcset
        self.graphics_type = graphics_type
        self.dpi = dpi
        self.pixel_dimensions = pixel_dimensions
        self.physical_dimensions = physical_dimensions
        self._fields = ('src_url', 'srcset', 'graphics_type', 'dpi',
                        'pixel_dimensions', 'physical_dimensions',)

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )





# ------------------------------------------------------------------------------


class SimpleIncludeGraphicsMacro(FLMMacroSpecBase):

    is_block_level = True

    allowed_in_standalone_mode = False
    r"""
    Can't allow this macro in standalone mode; rendering this macro requires a
    graphics resource provider, which in turn must be provided by a document.
    """

    def __init__(self, macroname, **kwargs):
        super().__init__(
            macroname='includegraphics',
            arguments_spec_list=[
                FLMArgumentSpec(
                    parser=latexnodes_parsers.LatexCharsGroupParser(
                        delimiters=('[',']'),
                        optional=True
                    ),
                    argname='graphics_options',
                ),
                FLMArgumentSpec(
                    parser=latexnodes_parsers.LatexCharsGroupParser(
                        delimiters=('{','}'),
                    ),
                    argname='graphics_path',
                ),
            ],
            **kwargs
        )
        
    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('graphics_options', 'graphics_path',),
        )
        node.flmarg_graphics_options_value = \
            node_args['graphics_options'].get_content_as_chars()
        node.flmarg_graphics_path = \
            node_args['graphics_path'].get_content_as_chars()

        node.flm_resources = [
            { 'resource_type': 'graphics_path',
              'resource_source_type': 'file',
              'resource_source': node.flmarg_graphics_path },
        ]

        return node

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        graphics_options_value = node.flmarg_graphics_options_value
        graphics_path = node.flmarg_graphics_path
        
        if graphics_options_value:
            raise LatexWalkerLocatedError(
                f"Graphics options are not supported here: ‘{graphics_options_value}’",
                pos=node_args['graphics_options'].nodelist.pos,
            )

        if not render_context.supports_feature('graphics_resource_provider'):
            raise RuntimeError(
                "FLM's ‘SimpleIncludeGraphicsSpecInfo’ (‘\\includegraphics’) requires a "
                "‘graphics_resource_provider’ feature to be installed in the render context"
            )
        
        resource_info = node.latex_walker.resource_info

        graphics_resource_provider_mgr = \
            render_context.feature_render_manager('graphics_resource_provider')
        graphics_resource = \
            graphics_resource_provider_mgr.get_graphics_resource(graphics_path, resource_info)

        return fragment_renderer.render_graphics_block( graphics_resource, render_context )








# ------------------------------------------------------------------------------


class FeatureSimplePathGraphicsResourceProvider(Feature):

    feature_name = 'graphics_resource_provider'
    feature_title = 'Include external graphics'

    class RenderManager(Feature.RenderManager):

        def get_graphics_resource(self, graphics_path, resource_info):
            # return
            return GraphicsResource(src_url=graphics_path)
    

# ------------------------------------------------

FeatureClass = FeatureSimplePathGraphicsResourceProvider
