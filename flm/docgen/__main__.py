import sys
import os
import os.path
import argparse
import json

import logging
logger = logging.getLogger(__name__)

import yaml
import frontmatter

import flm.main.run as flm_main_run
import flm.main.main as flm_main_main
import flm.main.oshelper as flm_main_oshelper

from . import docgen




docgen_builtin_default_config = {
    'flm': {
        'features': {
            'cells': True,
        }
    }
}



def main_docgen():

    args_parser = argparse.ArgumentParser(
        prog='flm-doc-gen',
        description='Documentation generator for a Latex-Like Markup (FLM) environment',
        epilog='Have a lot of flm fun!',
    )

    args_parser.add_argument('-C', '--config', action='store',
                             default=None,
                             help="YAML Configuration file for FLM parse settings and "
                             "features.  By default, ‘flmconfig.yaml’ will used in the "
                             "current directory if it exists.  In all cases the input "
                             "YAML front matter takes precedence over this config.")

    args_parser.add_argument('--no-frontmatter', action='store_false', default=True,
                             dest='frontmatter',
                             help='Do not include front matter in the output FLM content')

    args_parser.add_argument('-o', '--output', action='store',
                             default=None,
                             help="Output to the given file (choose format with -f)")

    args_parser.add_argument('-f', '--format', action='store',
                             default='html',
                             help="Output format: either 'flm' or one format we our "
                             "FLM engine can compile to (e.g. html or text)")

    args_parser.add_argument('-w', '--workflow', action='store',
                             default=None,
                             help="Use custom a workflow to compile the FLM document.")

    args_parser.add_argument('-t', '--template', action='store',
                             default=None,
                             help="Template to use to render the document.  Templates are "
                             "specific to output formats.  See documentation (TODO) "
                             "for more info.  Specify an empty argument to ouptut the "
                             "fragment content only without any surrounding template "
                             "content (“-t ''”). (Try e.g. “-t simple”.)")

    args_parser.add_argument('-P', '--template-path', action='append',
                             default=[],
                             help=f"Path to search for templates. You can specify this "
                             f"argument multiple times to give multiple paths.  "
                             f"Each path is either a relative or absolute "
                             f"folder, or of the form ‘pkg:flm_pkg_name’ to load the "
                             f"template paths relevant to that FLM python extention package.")

    args_parser.add_argument('-V', '--view', action='store_true',
                             default=False,
                             help="Open the output file with your browser or default "
                             "desktop application.  Requires an output file to be "
                             "specified (--output) as well as "
                             "a format (--format).")

    args_parser.add_argument('-v', '--verbose', action='store_true',
                             default=False,
                             help="Enable verbose debugging output")

    args_parser.add_argument('file', metavar="FILE", default=None, nargs='?',
                             help='If a file is specified, its YAML front matter is used')


    args = args_parser.parse_args()

    #
    # set up logging
    #
    level = logging.INFO
    if args.verbose:
        level = logging.DEBUG
    logging.basicConfig(level=level)

    #
    # Load front matter of the given file, if applicable
    #
    frontmatter_metadata = None
    if args.file is not None:
        with open(args.file, encoding='utf-8') as f:
            input_content = f.read()

        frontmatter_metadata, flm_content = frontmatter.parse(input_content)


    run_config = frontmatter_metadata or {}

    # load config & defaults
    orig_config = flm_main_main.load_external_config(args.config)

    logger.debug("Input frontmatter_metadata is\n%s",
                 json.dumps(frontmatter_metadata,indent=4))

    flm_run_info = {
        'resource_accessor': flm_main_main.ResourceAccessor(),
        'outputformat': None,
        'workflow': None,
        'template': None,
        'add_template_path': None,
        'force_block_level': None,
        'cwd': os.path.dirname(args.file) if args.file else os.getcwd(),
        'input_source': args.file,
        'input_lineno_colno_offsets': None,
        'metadata': None,
    }

    #
    # load the environment
    #
    wenv = flm_main_run.load_workflow_environment(
        flm_run_info=flm_run_info,
        run_config=run_config,
        default_configs=[orig_config, docgen_builtin_default_config]
    )

    #
    # Now, generate the documentation for this environment
    #

    docgen_obj = docgen.FLMEnvironmentDocumentationGenerator()

    docgen_flm_content = ''

    if args.frontmatter:
        docgen_flm_content += '---\n'
        docgen_flm_content += yaml.dump({
            'flm': {
                'features': {
                    'flm.docgen.FeatureFLMDocumentation': True,
                    'enumeration': {
                        'enumeration_environments': {
                            'flmDocItemize': {'counter_formatter':['▸'],},
                        },
                    },
                },
                'template': 'docgen',
                'renderer': {
                    'html': {
                        'verbatim_highlight_spaces': True,
                        'verbatim_protect_backslashes': True,
                    }
                },
            },
        }, default_flow_style=False).rstrip('\n') + '\n'
        docgen_flm_content += '---\n'

    docgen_flm_content += docgen_obj.document_environment(wenv.environment) + '\n'
    docgen_flm_content += docgen_obj.document_epilog() + '\n'

    if args.format == 'flm':
        if not args.output or args.output == '-':
            sys.stdout.write(docgen_flm_content)
            return

        with open(args.output, 'w', encoding='utf-8') as fw:
            fw.write(docgen_flm_content)
        return

    # It's another format, compile to that format.

    flm_main_main.main(
        flm_content=docgen_flm_content,
        format=args.format,
        output=args.output,
        workflow=args.workflow,
        template=args.template,
        template_path=args.template_path,
    )

    if args.view:
        if not args.output or args.output == '-':
            raise ValueError("You cannot use --view without --output")
        flm_main_oshelper.os_open_file(args.output)

    return



def run_main_docgen():
    try:
        main_docgen()
    except Exception as e:
        logger.critical(f"Exception--- {e}", exc_info=True)
        raise


if __name__ == '__main__':

    run_main_docgen()
