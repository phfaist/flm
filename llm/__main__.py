import sys
import argparse
import logging

from pylatexenc.latexnodes import LatexWalkerParseError

from .runmain import runmain, preset_fragment_renderer_classes
from llm import __version__ as llm_version


def main(cmdargs=None):
    
    args_parser = argparse.ArgumentParser(
        prog='llm',
        description='Latex-Like Markup parser and formatter - https://github.com/phfaist/llm',
        epilog='Have a lot of llm fun!',
    )

    args_parser.add_argument('-c', '--llm-content', action='store',
                             help="LLM content to parse and convert")

    args_parser.add_argument('-B', '--force-block-level', action='store_true',
                             default=None,
                             help="Parse input as block-level (paragraph) content")


    args_parser.add_argument('-C', '--config', action='store',
                             default=None,
                             help="YAML Configuration file for LLM parse settings and "
                             "features.  By default, ‘llmconfig.yaml’ will used in the "
                             "current directory if it exists.  In all cases the input "
                             "YAML front matter takes precedence over this config.")


    args_parser.add_argument('-o', '--output', action='store',
                             default=None,
                             help="Output file name (stdout by default or with ‘--output=-’)")

    args_parser.add_argument('-f', '--format', action='store',
                             default='html',
                             help=f"LLM content to parse and convert.  One of "
                             f"{', '.join(preset_fragment_renderer_classes.keys())} or a "
                             "fully specified class name for a FragmentRenderer class.")

    args_parser.add_argument('-D', '--minimal-document', action='store_true',
                             default=True,
                             help="Produce a minimal document preamble/postambule to form "
                             "a self-contained document.  Only applicable to specific "
                             "formats such as --format=html and --format=latex.  "
                             "On by default.  Use the --fragment (-F) option to disable.")

    args_parser.add_argument('-F', '--fragment', dest='minimal_document',
                             action='store_false',
                             help="The opposite of --minimal-document. Only applicable "
                             "to specific formats such as --format=html and --format=latex. "
                             "Use this option to generate fragments of (e.g. HTML) code "
                             "that can be inserted in other documents.")


    args_parser.add_argument('-n', '--suppress-final-newline', action='store_true',
                             default=False,
                             help="Do not add a newline at the end of the output")

    args_parser.add_argument('-v', '--verbose', action='store_true',
                             default=False,
                             help="Enable verbose debugging output")
    args_parser.add_argument('-w', '--very-verbose', action='store_const',
                             dest='verbose',
                             const=2,
                             help="Enable very long verbose debugging output "
                             "(include very elaborate pylatexenc debug messages)")

    args_parser.add_argument('--version', action='version', version=llm_version)

    args_parser.add_argument('files', metavar="FILE", nargs='*',
                             help='Input files (if none specified, read from stdandard input)')

    # --

    args = args_parser.parse_args(args=cmdargs)

    return runmain(args)


if __name__ == '__main__':
    try:
        main()
    except LatexWalkerParseError as e:
        logging.getLogger('llm').critical(
            f"Parse Error\n{e}"
        )
    except Exception as e:
        logging.getLogger('llm').critical('Error.', exc_info=e)
        import pdb; pdb.post_mortem()
