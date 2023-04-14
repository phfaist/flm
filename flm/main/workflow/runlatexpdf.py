import os
import os.path
import tempfile
import logging
logger = logging.getLogger(__name__)

import urllib
import urllib.request
import glob
import subprocess
import shutil

from flm.main.configmerger import ConfigMerger
from flm.fragmentrenderer.latex import (
    LatexFragmentRenderer,
    FragmentRendererInformation as LatexFragmentRendererInformation
)

from ._base import RenderWorkflow
from .templatebasedworkflow import TemplateBasedRenderWorkflow



magick_patterns = [
    '/usr/local/bin/magick',
    '/opt/homebrew/bin/magick',
    r"C:\Programs Files*\Image Magick*\**\magick.exe",
]
latexmk_patterns = [
    '/usr/local/texlive/*/bin/*/latexmk',
    '/usr/local/bin/latexmk',
    r'C:\texlive\*\bin\*\latexmk.exe',
    r'C:\Program Files*\MikTeX*\miktex\bin\latexmk.exe'
]

def _find_exe(exe_name, std_patterns, var_name):
    if var_name in os.environ:
        return os.environ[var_name]
    for p in std_patterns:
        result = glob.glob(p, recursive=True)
        if len(result):
            return result[0]
    rexe = shutil.which(exe_name)
    if rexe:
        return rexe
    raise ValueError(f"Cannot find executable ‘{exe_name}’ on your system! "
                     f"Please set {var_name} to its full path.")

# magick_exe = '/opt/homebrew/bin/magick'
# latexmk_exe = '/usr/local/texlive/2020/bin/x86_64-darwin/latexmk'

magick_exe = _find_exe('magick', magick_patterns, 'MAGICK')
latexmk_exe = _find_exe('latexmk', latexmk_patterns, 'LATEXMK')


class CollectGraphicsLatexFragmentRenderer(LatexFragmentRenderer):

    graphics_resource_counter = 0
    graphics_resource_data_name_prefix = 'gr'

    use_endnote_latex_command = 'flmEndnoteMark'
    use_citation_latex_command = 'flmCitationMark'


    def collect_graphics_resource(self, graphics_resource, render_context):
        # can be reimplemented to collect the given graphics resource somewhere
        # relevant etc.

        if 'graphics_resource_data' not in render_context.data:
            render_context.data['graphics_resource_data'] = {}
        if 'latex_preamble' not in render_context.data:
            render_context.data['latex_preamble'] = {}

        self.graphics_resource_counter = (self.graphics_resource_counter + 1)
        grname = f"{self.graphics_resource_data_name_prefix}{self.graphics_resource_counter}"

        with urllib.request.urlopen(graphics_resource.src_url) as fimg:
            grdata = fimg.read()

        _, grext = graphics_resource.src_url.rsplit('.', maxsplit=1)

        if grext in ('jpg', 'jpeg', 'png', 'pdf'):
            # all ok
            pass
        elif grext in ('svg', 'gif', 'mng',):
            # needs conversion
            with tempfile.TemporaryDirectory() as tempdirname:
                ins = '-'
                if grext in ('gif', 'mng'):
                    ins = '-[0]'
                subprocess.run([magick_exe, 'convert', ins, 'out.png'],
                               input=grdata, cwd=tempdirname)
                with open(os.path.join(tempdirname, 'out.png'), 'rb') as f:
                    grdata = f.read()
                    grext = 'png'

        gr_fname = f"{grname}.{grext}"

        render_context.data['graphics_resource_data'][gr_fname] = grdata

        if 'adjustbox' not in render_context.data['latex_preamble']:
            # 'export' allows adjustbox keys in \includegraphics
            render_context.data['latex_preamble']['adjustbox'] = \
                r"\usepackage[export]{adjustbox}"

        include_graphics_options = r"max width=\linewidth"

        return gr_fname, include_graphics_options



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
                shutil.copyfile(latexfn, loctexfn)
                logger.warning("latexmk exited with error code.  Copying tex file to %s "
                               "and log file to %s",
                               loctexfn, logfn, exc_info=True)

            with open(os.path.join(tempdirname, 'main.pdf'), 'rb') as f:
                result_pdf = f.read()

            return result_pdf

            
    
# ------------------------------------------------

RenderWorkflowClass = RunPdfLatexRenderWorkflow
