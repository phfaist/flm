import os.path
import json
import subprocess
from urllib.parse import urlparse

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes.nodes import LatexNodesVisitor
from flm.feature._base import Feature
from flm.feature.graphics import GraphicsResource

from ._find_exe import find_std_exe

magick_exe = find_std_exe('magick')


# ------------------------------------------------------------------------------



class ResourcesScanner(LatexNodesVisitor):
    def __init__(self):
        super().__init__()
        self.encountered_resources = []

    def get_encountered_resources(self):
        return self.encountered_resources

    # ---

    def visit(self, node):
        logger.debug('Scanning for graphics resources - visiting node %s', node)
        if hasattr(node, 'flm_resources'):
            # it's a node that requires access to an external resource.
            for resource in node.flm_resources:
                rdata = dict(resource)
                rdata['encountered_in'] = {
                    "resource_info": node.latex_walker.resource_info,
                    "what": node.latex_walker.what,
                }
                self.encountered_resources.append(rdata)





def _inspect_graphics_file(file_path):

    # use magick to get the resolution information.
    result = subprocess.check_output([
        magick_exe,
        'identify',
        '-format',
        '{"w":%[width],"h":%[height],"rx":%[resolution.x],"ry":%[resolution.y],"rU":"%[units]"}',
        file_path
    ])
    gfdata = json.loads(result)

    width_px = float(gfdata['w'])
    height_px = float(gfdata['h'])

    dpi_x = float(gfdata['rx'])
    dpi_y = float(gfdata['ry'])
    if gfdata['rU'].lower() == "PixelsPerCentimeter".lower():
        dpi_x = dpi_x*2.54
        dpi_y = dpi_y*2.54

    if abs(dpi_x - dpi_y) > 1e-2:
        raise ValueError(
            "Your image seems to have different DPI values for the X and Y dimensions: "
            f"({dpi_x!r}, {dpi_y!r}).  I don't know how to handle this.  Please fix "
            "your image so that it has a fixed DPI setting."
        )

    # round up DPI setting a bit (two decimal places' equivalent in binary)
    dpi = int(dpi_x * 128 + 0.5) / 128

    # set the relevant data in our graphics_resource data structure

    # There are 72 pts in an inch. Don't use 96 here, it's the DPI value that
    # should reflect the value 96 that your googling might have alerted you to.
    width_pt = (width_px / dpi) * 72
    height_pt = (height_px / dpi) * 72

    if file_path.endswith( ('svg', 'pdf') ):
        # vector graphics
        return {
            'graphics_type': 'vector',
            'dpi': dpi,
            'pixel_dimensions': (width_px, height_px),
            'physical_dimensions': (width_pt, height_pt),
        }
    else:
        width_px = int(width_px + 0.5)
        height_px = int(height_px + 0.5)
        # raster graphics
        return {
            'graphics_type': 'raster',
            'dpi': dpi,
            'pixel_dimensions': (width_px, height_px),
            'physical_dimensions': (width_pt, height_pt),
        }



class FeatureSimpleGraphicsCollection(Feature):

    feature_name = 'graphics_resource_provider'
    feature_title = 'Process a collection of graphics that can be included in FLM content'

    class DocumentManager(Feature.DocumentManager):

        def flm_main_scan_fragment(self, fragment):
            
            logger.debug('Scanning fragment for graphics resources')

            scanner = ResourcesScanner()

            fragment.start_node_visitor(scanner)

            for resource in scanner.get_encountered_resources():
                if resource.get('resource_type', None) == 'graphics_path':
                    self.inspect_add_graphics_resource(resource)

        def inspect_add_graphics_resource(self, resource):

            logger.debug('Inspect graphics resource? %r', resource)

            # look up the file etc.
            if resource['resource_source_type'] == 'file':

                src_url = resource['resource_source']

                # Let's see if it's actually a URL.  If not, we won't be able to
                # retrieve its meta info.
                urlp = urlparse(src_url)
                if urlp.scheme == '' or urlp.scheme == 'file':
                    src_url_basepath = None
                    try:
                        src_url_basepath = self.doc.metadata['filepath']['dirname']
                    except KeyError:
                        # filepath or dirname not provided in metadata, we don't know
                        # what the document's base dir is.
                        pass

                    file_path = os.path.join(src_url_basepath, src_url)

                    graphics_resource = GraphicsResource(
                        src_url=src_url,
                        ** self.feature.inspect_graphics_file(file_path),
                    )

                    self.feature.add_graphics(src_url, graphics_resource)
            else:
                raise ValueError("Unknown resource source type: " + repr(resource_source_type))


    class RenderManager(Feature.RenderManager):

        def initialize(self, src_url_resolver_fn=None):
            self.src_url_resolver_fn = src_url_resolver_fn

        def get_graphics_resource(self, graphics_path, resource_info):
            return self.feature.get_graphics_resource_base(
                graphics_path, resource_info, self.render_context,
                self.src_url_resolver_fn
            )

    def __init__(self,
                 allow_unknown_graphics=True,
                 export_graphics_resource_url_fn=None):
        super().__init__()

        self.graphics_collection = {}

        # allow get_graphics_resource() calls to graphics given by source paths
        # that weren't explicitly added to the collection.  In such cases, the
        # method self.get_unknown_graphics_resource() is called to form the
        # GraphicsResource object.
        self.allow_unknown_graphics = allow_unknown_graphics

        self.export_graphics_resource_url_fn = export_graphics_resource_url_fn



    def inspect_graphics_file(self, file_path):
        return _inspect_graphics_file(file_path)



    def add_graphics(self, source_path, graphics_resource):
        if source_path in self.graphics_collection:
            raise ValueError(
                f"Graphics collection already has a graphics resource registered "
                f"for path ‘{source_path}’ (registered target "
                f"‘{self.graphics_collection[source_path].src_url}’, new target "
                f"‘{graphics_resource.src_url}’"
            )
        self.graphics_collection[source_path] = graphics_resource
        info = ''
        if graphics_resource.physical_dimensions:
            w_pt, h_pt = graphics_resource.physical_dimensions
            info = f'{w_pt:.6f}pt x {h_pt:.6f}pt'
        logger.info(f"Graphics: ‘{source_path}’ {info}")

    def set_collection(self, collection):
        for source_path, graphics_resource in collection.items():
            self.add_graphics(source_path, graphics_resource)

    def has_graphics_for(self, source_path):
        return (source_path in self.graphics_collection)
        

    def get_unknown_graphics_resource(
            self, graphics_path, source_path, resource_info, render_context,
            src_url_resolver_fn
    ):
        # Alternatively, we could raise an error here. (You can reimplement in subclass.)
        return GraphicsResource(src_url=graphics_path)

    def get_graphics_resource_base(self, graphics_path, resource_info,
                                   render_context,
                                   src_url_resolver_fn=None):

        # Note: the `render_context` argument here is only needed so that we can
        # provide it as parameter to the custom callback `src_url_resolver_fn()`

        #
        # [FUTURE: in case of multiple input files] Compose full source path 
        #
        # source_path = os.path.join(
        #     resource_info.get_source_directory(),
        #     graphics_path
        # )
        source_path = graphics_path

        if source_path in self.graphics_collection:
            graphics_resource = self.graphics_collection[source_path]
        else:
            graphics_resource = self.get_unknown_graphics_resource(
                graphics_path, source_path, resource_info, render_context,
                src_url_resolver_fn
            )

        if src_url_resolver_fn is not None:
            src_url_result = src_url_resolver_fn(
                graphics_resource, render_context, source_path
            )
            if 'src_url' not in src_url_result:
                raise ValueError(
                    "src_url_resolver_fn() did not return a dict with key src_url: "
                    + repr(src_url_result)
                )

            grkwargs = dict(graphics_resource.asdict())
            grkwargs['src_url'] = src_url_result['src_url']
            grkwargs['srcset'] = src_url_result['srcset']

            graphics_resource = GraphicsResource(**grkwargs)

        return graphics_resource



FeatureClass = FeatureSimpleGraphicsCollection
