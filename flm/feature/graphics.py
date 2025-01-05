import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerLocatedError, ParsedArgumentsInfo
from pylatexenc.latexnodes import parsers as latexnodes_parsers

from ..flmspecinfo import FLMMacroSpecBase
from ..flmenvironment import FLMArgumentSpec
from ._base import Feature


class GraphicsResource:
    r"""
    Collects information about a graphics resource, i.e., an external image.

    Attributes:

    .. py:attribute:: src_url

       A string containing the path or full URL at which the graphics resource
       should be found.  This is the path or URL that should be included in the
       rendered output.

       This path might differ from the path the original graphics source was
       found, if you're using the FLM routines as part of a processing pipeline
       that produces FLM output along with e.g. optimized graphics (e.g., if
       you're producing a page on a website).

    .. py:attribute:: graphics_type

       One of 'raster' or 'vector'.

    .. py:attribute:: dpi

       The dots per inch (or pixels per inch) resolution of the source image.

       This property is only used if the `graphics_type` is 'raster'.

    .. py:attribute:: pixel_dimensions

       A tuple `(width_px, height_px)` storing the pixel dimensions of the
       raster source image.

       This property is only used if the `graphics_type` is 'raster'.

    .. py:attribute:: physical_dimensions

       A tuple `(width_pt, height_pt)` storing the dimensions at which the image
       is meant to be produced on a physical display.  A dimension of `1 pt` is
       defined to be `1/72 in`, i.e., `72pt = 1in`.

       This property is used both for 'raster' and 'vector' graphics types.  For
       'raster' graphics types, this property can normally be deduced from the
       'pixel_dimensions' and the 'dpi' attributes.

       In case the image actually has different dpi resolutions along the `X`
       and `Y` directions, the `physical_dimensions` can incorporate this
       difference while the 'dpi' field might be unreliable or incomplete.  In
       such a case, the relation is ``physical_dimensions = (
       pixel_dimensions[0]*72/x_dpi, pixel_dimensions[1]*72/y_dpi )``

    .. py:attribute:: srcset

       Possible alternative source URL to retrieve the final image resource
       (URLs to be included in rendered result), meant for use in <img
       srcset=... >.

       This attribute is a LIST of DICTs of the form ``[ { 'source':
       <source-url-1>, 'pixel_density': <pixel_density-1> }, { 'source':
       <source-url-2>, 'pixel_density': <pixel-density-2> }, ... ]``.  The
       `<pixel-density>` should be a number (integer or floating point), e.g.,
       ``2`` for a pixel density of `2x`.  For a source item, you may omit the
       'pixel_density', in which case browsers will interpret this the same way
       as `'pixel_density': 1`.
    
       See also:
       `The <srcset> HTML tag <https://developer.mozilla.org/en-US/docs/Web/API/HTMLImageElement/srcset>`_.

    .. py:attribute:: source_info

       This attribute can be set to a dictionary to store any additional
       information about where this graphics resource was resolved/found.

       This attribute is not set or used by the core FLM routines.  By default,
       this attribute is `None`.
    """
    def __init__(
            self,
            src_url, # 
            *,
            srcset=None,
            graphics_type=None,
            dpi=None,
            pixel_dimensions=None, # (width_px, height_px) # in pixels
            physical_dimensions=None, # (width_pt, height_pt) #  1 pt = 1/72 inch
            source_info=None,
    ):
        super().__init__()
        self.src_url = src_url
        self.srcset = srcset
        self.graphics_type = graphics_type
        self.dpi = dpi
        self.pixel_dimensions = pixel_dimensions
        self.physical_dimensions = physical_dimensions
        self.source_info = source_info
        self._fields = ('src_url', 'srcset', 'graphics_type', 'dpi',
                        'pixel_dimensions', 'physical_dimensions',
                        'source_info',)

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

    _fields = ('macroname', )
        
    def get_flm_doc(self):
        return r"""Insert an external graphics object."""

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('graphics_options', 'graphics_path',),
        )
        node.flmarg_graphics_options_value = \
            node_args['graphics_options'].get_content_as_chars()
        node.flmarg_graphics_path = \
            node_args['graphics_path'].get_content_as_chars()

        if node.flmarg_graphics_options_value:
            raise LatexWalkerLocatedError(
                (f"Graphics options are not supported for now: "
                 + f"‘{node.flmarg_graphics_options_value}’"),
                pos=node_args['graphics_options'].nodelist.pos,
            )

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

    #
    # recompose pure latex
    #

    def recompose_pure_latex(self, node, recomposer, **kwargs):

        magnification = None

        if recomposer.render_context is not None:
            graphics_resource_provider_mgr = \
                recomposer.render_context.feature_render_manager('graphics_resource_provider')
            graphics_resource = \
                graphics_resource_provider_mgr.get_graphics_resource(
                    node.flmarg_graphics_path,
                    resource_info=node.latex_walker.resource_info
                )
            fragment_renderer = recomposer.render_context.fragment_renderer
            if fragment_renderer is not None:
                if graphics_resource.graphics_type == 'raster' \
                   and hasattr(fragment_renderer, 'graphics_raster_magnification'):
                    magnification = fragment_renderer.graphics_raster_magnification
                elif hasattr(fragment_renderer, 'graphics_vector_magnification'):
                    # in particular, use the "vector" magnificiation if graphics
                    # type is unknown
                    magnification = fragment_renderer.graphics_vector_magnification
                    
        else:
            logger.warning(
                f"Recomposing pure latex: we have no access to a graphics resource provider "
                f"(need a render_context in the pure latex recomposer for that). "
                f"Graphics resource information will not be available."
            )
            graphics_resource = GraphicsResource(
                src_url=node.flmarg_graphics_path,
            )
        
        recopt_graphics = recomposer.get_options('graphics')
        logger.debug(f"recomposer graphics options = %r", recopt_graphics)
        width_scale = recopt_graphics.get('width_scale', None)
        set_max_width = recopt_graphics.get('set_max_width', r'\linewidth')

        if magnification is not None and width_scale is not None:
            logger.warning(
                "Applying graphics magnification both from magnification=%f from "
                "fragment_renderer option as well as from width_scale recomposer option."
            )
            magnification = magnification * width_scale
        elif width_scale is not None:
            # magnification is None, width_scale is not None
            magnification = width_scale
        elif magnification is None:
            # magnification is None, width_scale is None
            magnification = 1
        # else:
        #     pass # magnification is not None, width_scale is None --> already ok

        graphics_options_list = []
        if graphics_resource.physical_dimensions is not None:
            width_pt, _ = graphics_resource.physical_dimensions
            if width_pt is not None:
                width_pt = float(width_pt) * magnification
                graphics_options_list.append(f'width={width_pt:.6f}bp')
        if set_max_width is not None and set_max_width is not False:
            recomposer.ensure_latex_package('adjustbox', options='export')
            graphics_options_list.append(f'max width={set_max_width}')

        return (
            r'\includegraphics' 
            + ("["+",".join([opt for opt in graphics_options_list ])+"]"
               if graphics_options_list else "")
            + "{" + str(graphics_resource.src_url) + "}"
        )




class FeatureSimplePathGraphicsResourceProvider(Feature):

    feature_name = 'graphics_resource_provider'
    feature_title = 'Include external graphics'

    class RenderManager(Feature.RenderManager):

        def get_graphics_resource(self, graphics_path, resource_info):
            # return
            return GraphicsResource(src_url=graphics_path)
    

# ------------------------------------------------

FeatureClass = FeatureSimplePathGraphicsResourceProvider


