import sys
import os
import os.path
import argparse
import json

import logging
logger = logging.getLogger(__name__)

import yaml
import frontmatter

import llm.main.run as llm_main_run
import llm.main.main as llm_main_main

from . import docgen




docgen_builtin_default_config = {
    'llm': {
        'features': {
            'cells': True,
        }
    }
}



def main_docgen():

    args_parser = argparse.ArgumentParser(
        prog='llm-doc-gen',
        description='Documentation generator for a Latex-Like Markup (LLM) environment',
        epilog='Have a lot of llm fun!',
    )

    args_parser.add_argument('-C', '--config', action='store',
                             default=None,
                             help="YAML Configuration file for LLM parse settings and "
                             "features.  By default, ‘llmconfig.yaml’ will used in the "
                             "current directory if it exists.  In all cases the input "
                             "YAML front matter takes precedence over this config.")

    args_parser.add_argument('--no-frontmatter', action='store_false', default=True,
                             dest='frontmatter',
                             help='Do not include front matter in the output LLM content')

    args_parser.add_argument('-o', '--output', action='store',
                             default=None,
                             help="Output to the given file (choose format with -f)")

    args_parser.add_argument('-f', '--format', action='store',
                             default='html',
                             help="Output format: either 'llm' or one format we our "
                             "LLM engine can compile to (e.g. html or text)")

    args_parser.add_argument('-V', '--view', action='store_true',
                             default=False,
                             help="If true, open your browser to the output file.  "
                             "Requires an output file to be specified (--output) as well as "
                             "a format (--format) other than 'llm'.")

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

        frontmatter_metadata, llm_content = frontmatter.parse(input_content)


    run_config = frontmatter_metadata or {}

    # load config & defaults
    orig_config = llm_main_main.load_external_config(args.config)

    logger.debug("Input frontmatter_metadata is\n%s",
                 json.dumps(frontmatter_metadata,indent=4))

    llm_run_info = {
        'resource_accessor': llm_main_main.ResourceAccessor(),
        'outputformat': None,
        'workflow': None,
        'template': None,
        'force_block_level': None,
        'cwd': os.path.dirname(args.file) if args.file else os.getcwd(),
        'input_source': args.file,
        'input_lineno_colno_offsets': None,
        'metadata': None,
    }

    #
    # load the environment
    #
    wenv = llm_main_run.load_workflow_environment(
        llm_run_info=llm_run_info,
        run_config=run_config,
        default_configs=[orig_config, docgen_builtin_default_config]
    )

    #
    # Now, generate the documentation for this environment
    #

    docgen_obj = docgen.LLMEnvironmentDocumentationGenerator()

    docgen_llm_content = ''

    if args.frontmatter:
        docgen_llm_content += '---\n'
        docgen_llm_content += yaml.dump({
            'llm': {
                'features': {
                    'llm.docgen.FeatureLLMDocumentation': True,
                    'enumeration': {
                        'enumeration_environments': {
                            'llmDocItemize': {'counter_formatter':['▸'],},
                        },
                    },
                },
                'template': {
                    'html': {
                        'name': 'docgen',
                    },
                },
                'renderer': {
                    'html': {
                        'verbatim_highlight_spaces': True,
                        'verbatim_protect_backslashes': True,
                    }
                },
            },
        }, default_flow_style=False).rstrip('\n') + '\n'
        docgen_llm_content += '---\n'

    docgen_llm_content += docgen_obj.document_environment(wenv.environment) + '\n'
    docgen_llm_content += docgen_obj.document_epilog() + '\n'

    if args.format == 'llm':
        if not args.output or args.output == '-':
            sys.stdout.write(docgen_llm_content)
            return

        with open(args.output, 'w', encoding='utf-8') as fw:
            fw.write(docgen_llm_content)
        return

    # It's another format, compile to that format.

    llm_main_main.main(
        llm_content=docgen_llm_content,
        workflow=None,
        format=args.format,
        output=args.output,
    )

    if args.view:
        if not args.output or args.output == '-':
            raise ValueError("You cannot use --view without --output")
        os_view_file(args.output)

    return


# thanks https://stackoverflow.com/a/435669/1694896
def os_view_file(filepath):

    import subprocess, os, platform

    if platform.system() == 'Darwin':       # macOS
        subprocess.call(('open', filepath))
    elif platform.system() == 'Windows':    # Windows
        os.startfile(filepath)
    else:                                   # linux variants
        subprocess.call(('xdg-open', filepath))



def run_main_docgen():
    try:
        main_docgen()
    except Exception as e:
        logger.critical(f"Exception--- {e}", exc_info=True)
        raise


if __name__ == '__main__':

    run_main_docgen()
