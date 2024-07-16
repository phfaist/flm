import os
import os.path
import tempfile
import logging
logger = logging.getLogger(__name__)

import urllib
import urllib.request
from urllib.parse import urlparse
import subprocess
import shutil

from flm.main.configmerger import ConfigMerger
from flm.fragmentrenderer.latex import (
    LatexFragmentRenderer,
    FragmentRendererInformation as LatexFragmentRendererInformation
)

from ._base import RenderWorkflow
from .templatebasedworkflow import TemplateBasedRenderWorkflow

from .._find_exe import find_std_exe

magick_exe = find_std_exe('magick')
latexmk_exe = find_std_exe('latexmk')


class CollectGraphicsLatexFragmentRenderer(LatexFragmentRenderer):

    graphics_resource_counter = 0
    graphics_resource_data_name_prefix = 'gr'

    graphics_resource_magick_transparent_bg = False
    graphics_resource_magick_raster_dpi = 300 # None

    use_endnote_latex_command = 'flmEndnoteMark'
    use_citation_latex_command = 'flmCitationMark'

    # these attributes are already in LatexFragmentRenderer
    graphics_raster_magnification = 1
    graphics_vector_magnification = 1


    def collect_graphics_resource(self, graphics_resource, render_context):
        # can be reimplemented to collect the given graphics resource somewhere
        # relevant etc.

        if 'graphics_resource_data' not in render_context.data:
            render_context.data['graphics_resource_data'] = {}
        if 'latex_preamble' not in render_context.data:
            render_context.data['latex_preamble'] = {}

        self.graphics_resource_counter = (self.graphics_resource_counter + 1)
        grname = f"{self.graphics_resource_data_name_prefix}{self.graphics_resource_counter}"

        src_url = graphics_resource.src_url

        urlp = urlparse(src_url)
        if urlp.scheme == '':
            src_url_basepath = None
            try:
                src_url_basepath = render_context.doc.metadata['filepath']['dirname']
            except KeyError:
                # filepath or dirname not provided in metadata, we don't know
                # what the document's base dir is.
                pass
            src_url = 'file:' + os.path.join(src_url_basepath, src_url)

        with urllib.request.urlopen(src_url) as fimg:
            grdata = fimg.read()

        _, grext = src_url.rsplit('.', maxsplit=1)

        includegraphics_option_list = []

        if grext in ('jpg', 'jpeg', 'png', 'pdf'):
            # all ok
            pass
        elif grext in ('svg', 'gif', 'mng',):
            # needs conversion
            with tempfile.TemporaryDirectory() as tempdirname:
                ins = '-'
                if grext in ('gif', 'mng'):
                    ins = '-[0]'

                extra_args = []
                if self.graphics_resource_magick_transparent_bg:
                    extra_args = extra_args + [ '-background', 'none', ]
                if self.graphics_resource_magick_raster_dpi is not None:
                    extra_args = extra_args + [
                        '-density', str(self.graphics_resource_magick_raster_dpi),
                    ]

                subprocess.run([magick_exe,
                                'convert',
                                *extra_args,
                                ins,
                                'out.png'],
                               input=grdata, cwd=tempdirname)
                # scale_factor = 1.0 #72.0 / self.graphics_resource_magick_raster_dpi
                # includegraphics_option_list.append(
                #     f'scale={scale_factor:.6g}'
                # )
                with open(os.path.join(tempdirname, 'out.png'), 'rb') as f:
                    grdata = f.read()
                    grext = 'png'

        gr_fname = f"{grname}.{grext}"

        render_context.data['graphics_resource_data'][gr_fname] = grdata

        if 'adjustbox' not in render_context.data['latex_preamble']:
            # 'export' allows adjustbox keys in \includegraphics
            render_context.data['latex_preamble']['adjustbox'] = \
                r"\usepackage[export]{adjustbox}"

        if graphics_resource.physical_dimensions:
            width_pt, height_pt = graphics_resource.physical_dimensions

            # use magnification, if applicable
            if graphics_resource.graphics_type == 'raster':
                width_pt *= self.graphics_raster_magnification
                height_pt *= self.graphics_raster_magnification
            if graphics_resource.graphics_type == 'vector':
                width_pt *= self.graphics_vector_magnification
                height_pt *= self.graphics_vector_magnification

            # remember: standard 1pt = 1bp in LaTeX = 1/72.0 in; 1pt in LaTeX = 1/72.27 in

            includegraphics_option_list.append(
                f"width={width_pt:.6f}bp"
            )
            includegraphics_option_list.append(
                f"height={height_pt:.6f}bp"
            )

        includegraphics_option_list.append(r"max width=\linewidth")

        includegraphics_options = None
        if includegraphics_option_list:
            includegraphics_options = ','.join(includegraphics_option_list)

        return gr_fname, includegraphics_options



class CollectGraphicsLatexFragmentRendererInformation:
    FragmentRendererClass = CollectGraphicsLatexFragmentRenderer

    format_name = 'latex'

    @staticmethod
    def get_style_information(fragment_renderer):
        return LatexFragmentRendererInformation.get_style_information(fragment_renderer)



# ------------------------------------------------



class RunPdfLatexRenderWorkflow(RenderWorkflow):

    binary_output = True

    @staticmethod
    def get_fragment_renderer_name(req_outputformat, flm_run_info, run_config):

        if req_outputformat and req_outputformat not in ('latex', 'pdf', ):
            raise ValueError(
                f"The `runpdflatex` workflow only supports output formats 'latex' and 'pdf'"
            )

        # Always use our own 'latex'-derived fragment renderer.
        return 'flm.main.workflow.runlatexpdf.CollectGraphicsLatexFragmentRendererInformation'

    @staticmethod
    def get_default_main_config(flm_run_info, run_config):
        return {
            'flm': {
                'template': {
                    'latex': 'simple',
                },
            },
        }

    # ---

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    def postprocess_rendered_document(self, rendered_content, document, render_context):

        latex_preamble_collected = render_context.data.get('latex_preamble', {})

        # post-process LaTeX document contents

        latex_template_workflow_config = ConfigMerger().recursive_assign_defaults([
            {
                'use_output_format_name': 'latex',
                'template_config_workflow_defaults': {
                    'style': {
                        'extra_preamble': (
                            '\n'.join([
                                p
                                for what, p in latex_preamble_collected.items()
                            ]) + '\n'
                            + r"\providecommand\flmEndnoteMark{\textsuperscript}" + "\n"
                            + r"\providecommand\flmCitationMark{}" + "\n"
                        ),
                    },
                }
            },
            self.config,
        ])

        latex_template_workflow = TemplateBasedRenderWorkflow(
            latex_template_workflow_config,
            self.flm_run_info,
            self.fragment_renderer_information,
            self.fragment_renderer,
        )

        result_latex = latex_template_workflow.render_templated_document(
            rendered_content, document, render_context,
        )

        # logger.debug('Full LaTeX:\n\n%s\n\n', result_latex)

        if self.flm_run_info['requested_outputformat'] == 'latex':
            return result_latex.encode('utf-8')

        #
        # convert result to PDF using latexmk
        #

        with tempfile.TemporaryDirectory() as tempdirname:
            latexfname = os.path.join(tempdirname, 'main.tex')
            with open(latexfname, 'w') as fw:
                fw.write(result_latex)

            # write any graphics resource files
            if 'graphics_resource_data' in render_context.data:
                for grfname, grdata in render_context.data['graphics_resource_data'].items():
                    with open(os.path.join(tempdirname, grfname), 'wb') as fw:
                        fw.write(grdata)

            try:
                subprocess.run([latexmk_exe, '-f', '-silent', '-lualatex', 'main.tex'],
                               cwd=tempdirname, check=True)
            except subprocess.CalledProcessError as e:
                logfn = '_flm_runlatexpdf_compile_log.latex.log'
                shutil.copyfile(os.path.join(tempdirname, 'main.log'), logfn)
                loctexfn = '_flm_runlatexpdf_compile_log.latex.tex'
                shutil.copyfile(latexfname, loctexfn)
                logger.warning("latexmk exited with error code.  Copying tex file to %s "
                               "and log file to %s",
                               loctexfn, logfn, exc_info=True)

            with open(os.path.join(tempdirname, 'main.pdf'), 'rb') as f:
                result_pdf = f.read()

            return result_pdf

            
    
# ------------------------------------------------

RenderWorkflowClass = RunPdfLatexRenderWorkflow
