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

# magick_exe = find_std_exe('magick')
latexmk_exe = find_std_exe('latexmk')




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
        #'flm.main.workflow.runlatexpdf.CollectGraphicsLatexFragmentRendererInformation'

        # use the standard latex fragment renderer.
        return 'latex'

    @staticmethod
    def get_default_main_config(flm_run_info, run_config):
        return {
            'flm': {
                'template': {
                    'latex': 'simple',
                },
            },
        }

    @staticmethod
    def requires_temporary_directory_output(flm_run_info, run_config):
        return True

    # ---

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    def postprocess_rendered_document(self, rendered_content, document, render_context):

        #
        # The main run() routine is kind enough to provide a temporary directory
        # for us (as we request via requires_temporary_directory_output()
        # above).  The name of the temporary directory is stored in the
        # 'output_cwd' field of the flm_run_info.
        #
        tempdirname = self.flm_run_info['output_cwd']

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
