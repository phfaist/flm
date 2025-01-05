import os
import os.path
import string # string.Template
import tempfile
import shutil
import subprocess

from urllib.parse import urlparse
import urllib.request

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexWalkerError
from pylatexenc.latexnodes.nodes import LatexNodesVisitor
from flm.feature._base import Feature
from flm.feature.graphics import GraphicsResource

from ._inspectimagefile import get_image_file_info

from ._find_exe import find_std_exe


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

class GraphicsFormatConversionRule:
    def __init__(self, rule):
        super().__init__()
        self.rule = rule
        self._parse_set_rule(rule)

    def _parse_set_rule(self, rule):

        if isinstance(rule, str):
            rule_from, rule_to = rule.split(':', maxsplit=1)
            rule_via = []
            rule_options = None
        else:
            rule_from = rule.get('from', [])
            rule_to = rule.get('to', [])
            rule_via = rule.get('via', [])
            rule_options = rule.get('options', None)
            
        if isinstance(rule_from, str):
            rule_from = rule_from.split(',')
        if isinstance(rule_to, str):
            rule_to = rule_to.split(',')
        if isinstance(rule_via, str):
            rule_via = rule_via.split(',')

        self.from_ext_list = [x for x in rule_from if x]
        self.to_ext_list = [x for x in rule_to if x]
        self.via_list = [x for x in rule_via if x]
        self.options = rule_options

        if len(self.to_ext_list) >= 2:
            logger.warning("Rules with multiple output formats are not supported yet.  "
                           "First output format specified will be used.")

    def get_converter_or_none(self, ext):

        if len(self.from_ext_list) and ext not in self.from_ext_list:
            return None

        to_ext = self.to_ext_list[0]

        for converter in _graphics_converters:

            if len(self.via_list) and converter.name not in self.via_list:
                continue

            if not converter.can_convert(ext, to_ext):
                continue

            return {
                'target_ext': to_ext,
                'instance': converter.get_instance(),
                'options': self.options
            }

        logger.warning(
            f"Could not find a graphics converter that could convert ‘{ext}’ to ‘{to_ext}’ "
            f"as requested by rule"
        )
        return None


# ---


class GraphicsConverter:
    name = None

    @classmethod
    def get_instance(cls):
        if not hasattr(cls, '_instance') or cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def read_input(self, source_type, src_url, binary=True):
        if source_type == 'file':
            if binary:
                with open(src_url, 'rb') as f:
                    data = f.read()
            else:
                with open(src_url, 'r', encoding='utf-8') as f:
                    data = f.read()
            return data

        with urllib.request.urlopen(src_url) as f:
            data = f.read()
        if binary:
            return data
        return data.decode('utf-8')



class CairoSvgConverter(GraphicsConverter):
    
    name = 'cairosvg'

    @staticmethod
    def can_convert(ext, to_ext):
        if ext == '.svg' and to_ext in ('.pdf', '.png', '.ps', '.eps'):
            return True
        return False
        
    def convert(self, source_type, src_url, target_path, converter_info, options=None):

        if options is None:
            options = {}

        svgkwargs = {}
        if 'dpi' in options:
            svgkwargs['dpi'] = options['dpi']
        if 'scale' in options:
            svgkwargs['scale'] = options['scale']
        if 'parent_width' in options:
            svgkwargs['parent_width'] = options['parent_width']
        if 'parent_height' in options:
            svgkwargs['parent_height'] = options['parent_height']

        import cairosvg

        target_ext = converter_info['target_ext']
        if target_ext == '.pdf':
            svgconvert = cairosvg.svg2pdf
        elif target_ext == '.png':
            svgconvert = cairosvg.svg2png
        elif target_ext in ('.ps', '.eps'):
            svgconvert = cairosvg.svg2ps
        else:
            raise ValueError(f"Invalid target ext {target_ext}, shouldn't be here")

        svgconvert(url=src_url, write_to=target_path, **svgkwargs)



class GhostscriptConverter(GraphicsConverter):
    
    name = 'gs'

    @staticmethod
    def can_convert(ext, to_ext):
        if ext in ('.pdf', '.ps', '.eps') and to_ext in ('.png', '.jpg', '.jpeg', '.pdf'):
            return True
        return False
        
    def __init__(self):
        self.gs_exe = find_std_exe('gs')

    def convert(self, source_type, src_url, target_path, converter_info, options=None):

        target_ext = converter_info['target_ext']

        if options is None:
            options = {}

        dpi = 300
        if 'dpi' in options:
            dpi = options['dpi']

        gs_device = None
        gs_device_options = None
        if target_ext == '.png':
            gs_device = 'png16m'
            gs_device_options = ['-dTextAlphaBits=4', '-dGraphicsAlphaBits=4',]
        elif target_ext in ('.jpg', '.jpeg'):
            gs_device = 'jpeg'
            gs_device_options = ['-dTextAlphaBits=4', '-dGraphicsAlphaBits=4',]
        elif target_ext == '.pdf':
            gs_device = 'pdfwrite'
            gs_device_options = []
        else:
            raise ValueError(
                f"Invalid target extension for GhostscriptConverter: ‘{target_ext}’"
            )

        input_data = self.read_input(source_type, src_url, binary=True)

        cmdargs = [
            self.gs_exe,
            '-dSAFER',
            '-dBATCH',
            '-dNOPAUSE',
            '-q',
            f"-sDEVICE={gs_device}",
            f"-r{dpi}",
            *gs_device_options,
            f"-sOutputFile={target_path}",
            "-", # use STDIN as input
        ]

        logger.debug('Running: %r', cmdargs)
        result = subprocess.run(
            cmdargs,
            input=input_data,
            #cwd=tempdirname
        )
        if result.returncode != 0:
            logger.error(f"Command failed: {repr(cmdargs)}")






class PdfToCairoCmdlConverter(GraphicsConverter):
    
    name = 'pdftocairo'

    @classmethod
    def can_convert(cls, ext, to_ext):
        if ext == '.pdf' and to_ext in cls._fmts_to_opts:
            return True
        return False

    _fmts_to_opts = {
        '.ps': [ '-ps', '-origpagesizes', '-level3' ],
        '.eps': [ '-eps', '-origpagesizes', '-level3' ],
        '.svg': [ '-svg', '-origpagesizes' ],
        '.png': [ '-png', '-singlefile' ],
        '.jpg': [ '-jpeg', '-singlefile' ],
        '.jpeg': [ '-jpeg', '-singlefile' ],
        '.tiff': [ '-tiff', '-singlefile' ],
    }

    def __init__(self):
        self.pdftocairo_exe = find_std_exe('pdftocairo')

    def convert(self, source_type, src_url, target_path, converter_info, options=None):

        if options is None:
            options = {}

        target_ext = converter_info['target_ext']

        xtracmdargs = []
        if 'dpi' in options:
            xtracmdargs += [ '-r', str(options['dpi']) ]
        if options.get('transparent_bg', False) and target_ext == '.png':
            xtracmdargs += [ '-transp' ]

        input_data = self.read_input(source_type, src_url, binary=True)
        
        cmdargs = [
            self.pdftocairo_exe,
            '-',
            *self._fmts_to_opts[target_ext],
            *xtracmdargs,
            '-', # we'll write to target file ourselves, otherwise pdftocairo
                 # tries to add an extension automatically ... :/
        ]

        logger.debug('Running: %r', cmdargs)
        with open(target_path, 'wb') as fw:
            result = subprocess.run(
                cmdargs,
                input=input_data,
                stdout=fw,
                #cwd=tempdirname
            )
            if result.returncode != 0:
                logger.error(f"Command failed: {repr(cmdargs)}")



class MagickConverter(GraphicsConverter):

    name = 'magick'

    @staticmethod
    def can_convert(ext, to_ext):
        return True

    def __init__(self):
        self.magick_exe = find_std_exe('magick')

    def convert(self, source_type, src_url, target_path, converter_info, options=None):

        if options is None:
            options = {}

        transparent_bg = options.get('transparent_bg', False)
        dpi = options.get('dpi', None)

        input_data = self.read_input(source_type, src_url, binary=True)
        
        ins = '-'
        if src_url.endswith( ('.gif', '.mng') ):
            ins = '-[0]'

        extra_args = []
        if transparent_bg:
            extra_args = extra_args + [ '-background', 'none', ]
        if dpi is not None:
            extra_args = extra_args + [
                '-density', str(dpi),
            ]

        cmdargs = [
            self.magick_exe,
            'convert',
            *extra_args,
            ins,
            target_path
        ]

        logger.debug('Running: %r', cmdargs)
        result = subprocess.run(
            cmdargs,
            input=input_data,
            #cwd=tempdirname
        )
        if result.returncode != 0:
            logger.error(f"Command failed: {repr(cmdargs)}")




_graphics_converters = [
    CairoSvgConverter,
    GhostscriptConverter,
    PdfToCairoCmdlConverter,
    MagickConverter,
]


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
                collect_format_conversion_rules=None,
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

            if collect_format_conversion_rules is not None:
                self.collect_format_conversion_rules = collect_format_conversion_rules
            else:
                self.collect_format_conversion_rules = \
                    self.feature.collect_format_conversion_rules

            # prepare output target template object, if applicable
            self.collect_graphics_filename_template_obj = string.Template(
                self.collect_graphics_filename_template
            )

            # prepare conversion rules, if applicable
            if self.collect_format_conversion_rules:
                self.collect_format_conversion_rules = [
                    GraphicsFormatConversionRule(rule)
                    for rule in self.collect_format_conversion_rules
                ]
            else:
                self.collect_format_conversion_rules = []

            self.graphics_to_collect = {}

            if self.collect_graphics_to_output_folder:
                counter = 0
                for source_key, source_info in \
                    self.feature_document_manager.document_graphics_by_source_key.items():

                    counter += 1
                    # src_url is resolved according to graphics_source_paths
                    src_url = self.feature.graphics_collection[source_key].src_url

                    self.prepare_collect_graphics(
                        source_info,
                        resolved_src_url=src_url,
                        counter=counter,
                    )


        def prepare_collect_graphics_get_converter(self, source_info, resolved_src_url, ext):
            # see if we should convert this

            converter_info = None

            for rule in self.collect_format_conversion_rules:
                converter_info = rule.get_converter_or_none(ext)
                if converter_info:
                    return converter_info

            # by default, same output extension as input extension
            return { 'target_ext': ext, 'instance': None }

        def prepare_collect_graphics(self, source_info, resolved_src_url, counter):

            source_type, source_url, source_key = source_info

            basename = os.path.basename(resolved_src_url)
            basenoext, ext = os.path.splitext(basename)

            converter_info = self.prepare_collect_graphics_get_converter(
                source_info, resolved_src_url, ext
            )
            target_ext = converter_info['target_ext']

            target_fname = self.collect_graphics_filename_template_obj.substitute({
                'basename': os.path.basename(resolved_src_url),
                'basenoext': basenoext,
                'ext': target_ext,
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

            # prepare which files we have to collect
            self.graphics_to_collect[source_key] = {
                'source_type': source_type,
                'source_url': source_url,
                'src_url_resolved': resolved_src_url,
                'converter_info': converter_info,
                'target_path': target_path,
                'target_relative_path': target_relative_path,
                'from_ext': ext,
                'to_ext': target_ext,
            }

            
        def collect_graphics(self, source_key, collect_info):

            source_type = collect_info['source_type']
            src_url = collect_info['src_url_resolved']
            converter_info = collect_info['converter_info']
            converter = converter_info['instance']
            target_path = collect_info['target_path']

            logger.info('Collecting ‘%s’ to ‘%s’ as %s (via %s)',
                        src_url, target_path, source_type, converter.name)

            logger.debug('converter_info = %r', converter_info)

            if os.path.exists(target_path):
                logger.error("Cowardly refusing to overwrite %s", target_path)
                return

            if converter is not None:

                converter_options = converter_info['options']

                converter.convert(
                    source_type, src_url, target_path,
                    converter_info=converter_info,
                    options=converter_options,
                )

            else:

                if source_type == 'file':
                    shutil.copyfile(src_url, target_path)
                else:
                    with urllib.request.urlopen(src_url) as fr:
                        with open(target_path, 'wb') as fw:
                            shutil.copyfileobj(fr, fw)


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
            self.collect_all_graphics()

        def collect_all_graphics(self):

            # collect graphics to output folder, if applicable
            if self.collect_graphics_to_output_folder:
                # make sure output folder exists
                os.makedirs(
                    os.path.realpath(self.collect_graphics_to_output_folder),
                    exist_ok=True
                )

                for source_key, collect_info in self.graphics_to_collect.items():

                    self.collect_graphics(source_key, collect_info)



    def __init__(
            self,
            allow_unknown_graphics=False,
            collect_graphics_to_output_folder=False,
            collect_graphics_relative_output_folder=None,
            collect_graphics_filename_template="gr${counter}${ext}",
            collect_format_conversion_rules=None,
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
        self.collect_format_conversion_rules = collect_format_conversion_rules

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
