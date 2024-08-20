import os
import os.path
import string # string.Template
import shutil

from urllib.parse import urlparse
import urllib.request

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerError
from pylatexenc.latexnodes.nodes import LatexNodesVisitor
from flm.feature._base import Feature
from flm.feature.graphics import GraphicsResource


from ._inspectimagefile import get_image_file_info


# ------------------------------------------------------------------------------



class ResourcesScanner(LatexNodesVisitor):
    def __init__(self):
        super().__init__()
        self.encountered_resources = []

    def get_encountered_resources(self):
        return self.encountered_resources

    # ---

    def visit(self, node, **kwargs):
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




# ------------------------------------------------------------------------------




class FeatureGraphicsCollection(Feature):

    feature_name = 'graphics_resource_provider'
    feature_title = 'Process a collection of graphics that can be included in FLM content'

    class DocumentManager(Feature.DocumentManager):

        def initialize(self):
            self.document_graphics_by_source_key = {}

            self.flm_run_info = self.doc.metadata['_flm_run_info']
            self.resource_accessor = self.flm_run_info['resource_accessor']

        def flm_main_scan_fragment(self, fragment, document_parts_fragments=None, **kwargs):
            
            logger.debug('Scanning fragment for graphics resources')

            scanner = ResourcesScanner()

            fragment.start_node_visitor(scanner)
            if document_parts_fragments:
                for frag in document_parts_fragments:
                    frag.start_node_visitor(scanner)

            for resource in scanner.get_encountered_resources():
                if resource.get('resource_type', None) == 'graphics_path':
                    self.inspect_add_graphics_resource(resource)


        def get_source_info(self, graphics_path, resource_info):

            # Let's see if it's a local file (rather than a remote URL)

            urlp = urlparse(graphics_path)
            if urlp.scheme == '' or urlp.scheme == 'file':

                source_path = os.path.join(
                    resource_info.get_source_directory(),
                    urlp.path
                )

                return ('file', source_path, f"file:{source_path}")

            src_url = graphics_path

            return ('url', src_url, src_url)


        def inspect_add_graphics_resource(self, resource):

            logger.debug('Inspect graphics resource? %r', resource)

            if resource['resource_source_type'] != 'file':
                logger.warning("I don't know how to handle resource sources of type %r",
                               resource['resource_source_type'])
                return

            src_url = resource['resource_source']
            resource_info = resource['encountered_in']['resource_info']
                
            source_type, source_url, source_key = self.get_source_info(src_url, resource_info)

            if source_key not in self.document_graphics_by_source_key:
                self.document_graphics_by_source_key[source_key] = \
                    source_type, source_url, source_key

            if source_key in self.feature.graphics_collection:
                # info already there, no need to fetch anything or create
                # graphics resource.
                return

            # look up the file etc.
            if source_type == 'file':

                source_path = source_url

                # fetch the info and store the graphics

                file_path, file_name = self.resource_accessor.find_in_search_paths(
                    self.feature.graphics_search_path,
                    source_path,
                    ftype='graphics',
                    flm_run_info=self.flm_run_info
                )

                with self.resource_accessor.open_file_object_context(
                        fpath=file_path, fname=file_name,
                        ftype='graphics',
                        flm_run_info=self.flm_run_info, binary=True
                ) as fp:
                    info = self.feature.inspect_graphics_file(source_path, fp)

                if info is None:
                    info = {}

                logger.debug("Inspected graphics ‘%s’, found info %r", source_path, info)

                graphics_resource = GraphicsResource(
                    src_url=source_path,
                    ** info,
                )

                self.feature.add_graphics(source_key, graphics_resource)

            elif source_type == 'url':

                # won't be able to inspect meta-information
                logger.warning("Can't inspect meta-information for remote resouce ‘%s’",
                               source_url)

                graphics_resource = GraphicsResource(
                    src_url=source_url,
                )

                self.feature.add_graphics(source_key, graphics_resource)

            else:
                raise LatexWalkerError(
                    "Unknown resource source type: " + repr(source_type)
                )


    class RenderManager(Feature.RenderManager):

        def initialize(
                self,
                src_url_resolver_fn=None,
                allow_unknown_graphics=None,
                collect_graphics_to_output_folder=None,
                collect_graphics_relative_output_folder=None,
                collect_graphics_filename_template=None,
        ):
            self.src_url_resolver_fn = src_url_resolver_fn

            if allow_unknown_graphics is not None:
                self.allow_unknown_graphics = allow_unknown_graphics
            else:
                self.allow_unknown_graphics = self.feature.allow_unknown_graphics

            if collect_graphics_to_output_folder is not None:
                self.collect_graphics_to_output_folder = collect_graphics_to_output_folder
            else:
                self.collect_graphics_to_output_folder = \
                    self.feature.collect_graphics_to_output_folder

            if collect_graphics_relative_output_folder is not None:
                self.collect_graphics_relative_output_folder = \
                    collect_graphics_relative_output_folder
            else:
                self.collect_graphics_relative_output_folder = \
                    self.feature.collect_graphics_relative_output_folder

            if self.collect_graphics_relative_output_folder is None:
                self.collect_graphics_relative_output_folder = \
                    self.collect_graphics_to_output_folder

            if collect_graphics_filename_template is not None:
                self.collect_graphics_filename_template = collect_graphics_filename_template
            else:
                self.collect_graphics_filename_template = \
                    self.feature.collect_graphics_filename_template

            self.collect_graphics_filename_template_obj = string.Template(
                self.collect_graphics_filename_template
            )

            self.graphics_to_collect = {}

            if self.collect_graphics_to_output_folder:
                counter = 0
                for source_key, source_info in \
                    self.feature_document_manager.document_graphics_by_source_key.items():

                    source_type, source_url, source_key = source_info

                    # src_url is resolved according to graphics_source_paths
                    src_url = self.feature.graphics_collection[source_key].src_url

                    counter += 1
                    basename = os.path.basename(src_url)
                    basenoext, ext = os.path.splitext(basename)
                    target_fname = self.collect_graphics_filename_template_obj.substitute({
                        'basename': os.path.basename(src_url),
                        'basenoext': basenoext,
                        'ext': ext,
                        'counter': counter,
                    })
                    target_path = os.path.join(
                        self.collect_graphics_to_output_folder,
                        target_fname
                    )
                    target_relative_path = os.path.join(
                        self.collect_graphics_relative_output_folder,
                        target_fname
                    )
                    _, target_ext = os.path.splitext(target_path)

                    # prepare which files we have to collect
                    self.graphics_to_collect[source_key] = {
                        'source_type': source_type,
                        'source_url': source_url,
                        'src_url_resolved': src_url,
                        'target_path': target_path,
                        'target_relative_path': target_relative_path,
                        'from_ext': ext,
                        'to_ext': target_ext,
                    }


        def get_graphics_resource(self, graphics_path, resource_info):

            render_context = self.render_context
            src_url_resolver_fn = self.src_url_resolver_fn

            source_type, source_url, source_key = \
                self.feature_document_manager.get_source_info(graphics_path, resource_info)

            if source_key in self.feature.graphics_collection:
                graphics_resource = self.feature.graphics_collection[source_key]
            else:
                graphics_resource = self.get_unknown_graphics_resource(
                    graphics_path, source_path, resource_info
                )

            grkwargs = None
            collect_info = None

            # if we're collecting the graphics to some output folder, set the
            # path to the collected one
            if source_key in self.graphics_to_collect:
                collect_info = self.graphics_to_collect[source_key]
                grkwargs = {
                    'src_url': collect_info['target_relative_path'],
                }

            if src_url_resolver_fn is not None:
                src_url_result = src_url_resolver_fn(
                    graphics_resource, render_context, source_path,
                    collect_info=collect_info
                )
                if 'src_url' not in src_url_result:
                    raise ValueError(
                        "src_url_resolver_fn() did not return a dict with key src_url: "
                        + repr(src_url_result)
                    )

                grkwargs = {
                    'src_url': src_url_result['src_url'],
                    'srcset': src_url_result['srcset'],
                }

            if grkwargs:
                fullgrkwargs = dict(graphics_resource.asdict())
                fullgrkwargs.update(grkwargs)
                return GraphicsResource(**fullgrkwargs)

            return graphics_resource


        def get_unknown_graphics_resource(
                self, graphics_path, source_path, resource_info
        ):
            if not self.allow_unknown_graphics:
                raise LatexWalkerError(
                    f"Graphics ‘{graphics_path}’ (from ‘{source_path}’) was not "
                    f"added to collection"
                )
            return GraphicsResource(src_url=graphics_path)


        def postprocess(self, value):
            # collect graphics to output folder, if applicable
            if self.collect_graphics_to_output_folder:
                # make sure output folder exists
                os.makedirs(
                    os.path.realpath(self.collect_graphics_to_output_folder),
                    exist_ok=True
                )

                for source_key, collect_info in self.graphics_to_collect.items():
                    source_type = collect_info['source_type']
                    src_url = collect_info['src_url_resolved']
                    target_path = collect_info['target_path']

                    logger.info('Collecting ‘%s’ to ‘%s’ as %s', src_url, target_path,
                                source_type)

                    if os.path.exists(target_path):
                        logger.error("Cowardly refusing to overwrite %s", target_path)
                        continue

                    if source_type == 'file':
                        shutil.copyfile(src_url, target_path)
                    else:
                        with urllib.request.urlopen(src_url) as fr:
                            with open(target_path, 'wb') as fw:
                                shutil.copyfileobj(fr, fw)


    def __init__(
            self,
            allow_unknown_graphics=False,
            collect_graphics_to_output_folder=False,
            collect_graphics_relative_output_folder=None,
            collect_graphics_filename_template="gr${counter}${ext}",
            graphics_search_path=None,
    ):
        super().__init__()

        self.graphics_collection = {}

        # allow get_graphics_resource() calls to graphics given by source paths
        # that weren't explicitly added to the collection.
        self.allow_unknown_graphics = allow_unknown_graphics
        self.collect_graphics_to_output_folder = collect_graphics_to_output_folder
        self.collect_graphics_relative_output_folder = collect_graphics_relative_output_folder
        self.collect_graphics_filename_template = collect_graphics_filename_template

        self.graphics_search_path = list(graphics_search_path or ['.'])


    def inspect_graphics_file(self, file_path, fp):
        return get_image_file_info(file_path, fp)


    def add_graphics(self, source_path, graphics_resource):
        if source_path in self.graphics_collection:
            raise LatexWalkerError(
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
        



FeatureClass = FeatureGraphicsCollection
