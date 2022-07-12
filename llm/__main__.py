import sys
import argparse
import logging

from pylatexenc.latexnodes import LatexWalkerParseError

from .runmain import runmain


def main(cmdargs=None):
    
    args_parser = argparse.ArgumentParser(
        prog='llm',
        epilog='Have a lot of llm fun!'
    )

    args_parser.add_argument('-c', '--llm-content', action='store',
                             help="LLM content to parse and convert")
    args_parser.add_argument('-B', '--force-block-level', action='store_true',
                             default=None,
                             help="Parse input as block-level (paragraph) content")

    args_parser.add_argument('-f', '--format', action='store',
                             default='html',
                             help="LLM content to parse and convert")
    args_parser.add_argument('-n', '--suppress-final-newline', action='store_true',
                             default=False,
                             help="Do not add a newline at the end of the output")
    args_parser.add_argument('-v', '--verbose', action='store_true',
                             default=False,
                             help="Enable verbose/debug output")
    args_parser.add_argument('-w', '--very-verbose', action='store_const',
                             dest='verbose',
                             const=2,
                             help="Enable long verbose/debug output (include pylatexenc debug)")

    args_parser.add_argument('files', metavar="FILE", nargs='*',
                             help='Input files (if none specified, read from stdandard input)')

    # --

    args = args_parser.parse_args(args=cmdargs)

    args.config = None

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
