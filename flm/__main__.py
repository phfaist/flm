import sys
import argparse
import logging

from pylatexenc.latexnodes import LatexWalkerError

import colorlog

from .main.main import main as _main
from .main import oshelper as flm_main_oshelper
from flm import __version__ as flm_version


def setup_logging(level):
    # You should use colorlog >= 6.0.0a4
    handler = colorlog.StreamHandler()
    handler.setFormatter( colorlog.LevelFormatter(
        log_colors={
            "DEBUG": "white",
            "INFO": "",
            "WARNING": "red",
            "ERROR": "bold_red",
            "CRITICAL": "bold_red",
        },
        fmt={
            # emojis we can use: 🐞 🐜 🚨 🚦 ⚙️ 🧨 🧹 ❗️❓‼️ ⁉️ ⚠️ ℹ️ ➡️ ✔️ 〰️
            # 🎶 💭 📣 🔔 ⏳ 🔧 🔩 ✨ 💥 🔥 🐢 👉
            "DEBUG":    "%(log_color)s〰️    %(message)s", #'  [%(name)s]'
            "INFO":     "%(log_color)s✨  %(message)s",
            "WARNING":  "%(log_color)s⚠️   %(message)s", # (%(module)s:%(lineno)d)",
            "ERROR":    "%(log_color)s🚨  %(message)s", # (%(module)s:%(lineno)d)",
            "CRITICAL": "%(log_color)s🚨  %(message)s", # (%(module)s:%(lineno)d)",
        },
        stream=sys.stderr
    ) )

    root = colorlog.getLogger()
    root.addHandler(handler)

    root.setLevel(level)



def run_main(cmdargs=None, enable_debug_pdb=False):
    try:
        _run_main_inner()
    except LatexWalkerError as e:
        logging.getLogger('flm').debug("Got LatexWalkerError, traceback = ", exc_info=True)
        logging.getLogger('flm').critical(
            f"FLM Error\n{e}",
        )
    except Exception as e:
        logging.getLogger('flm').critical('Error.', exc_info=e)
        if enable_debug_pdb:
            import pdb
            pdb.post_mortem()


def _run_main_inner(cmdargs=None):
    
    args_parser = argparse.ArgumentParser(
        prog='flm',
        description='Latex-Like Markup parser and formatter - https://github.com/phfaist/flm',
        epilog='Have a lot of flm fun!',
    )

    args_parser.add_argument('-c', '--flm-content', action='store',
                             help="FLM content to parse and convert")

    args_parser.add_argument('-B', '--force-block-level', action='store_true',
                             default=None,
                             help="Parse input as block-level (paragraph) content")


    args_parser.add_argument('-C', '--config', action='store',
                             default=None,
                             help="YAML Configuration file for FLM parse settings and "
                             "features.  By default, ‘flmconfig.yaml’ will used in the "
                             "current directory if it exists.  In all cases the input "
                             "YAML front matter takes precedence over this config.")


    args_parser.add_argument('-o', '--output', action='store',
                             default=None,
                             help="Output file name (stdout by default or with ‘--output=-’)")

    args_parser.add_argument('-f', '--format', action='store',
                             default=None,
                             help=f"FLM content to parse and convert.  One of "
                             f"html,text,markdown,latex or a "
                             "fully specified module or class name defining a "
                             "FragmentRenderer subclass.")

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
                             "a format (--format) other than 'flm'.")


    args_parser.add_argument('-n', '--suppress-final-newline', action='store_true',
                             default=False,
                             help="Do not add a newline at the end of the output")

    args_parser.add_argument('-v', '--verbose', action='store_true',
                             default=False,
                             help="Enable verbose debugging output")
    args_parser.add_argument('-W', '--very-verbose', action='store_const',
                             dest='verbose',
                             const=2,
                             help="Enable very long verbose debugging output "
                             "(include very elaborate pylatexenc debug messages)")

    args_parser.add_argument('--version', action='version', version=flm_version)

    args_parser.add_argument('files', metavar="FILE", nargs='*',
                             help='Input files (if none specified, read from stdandard input)')

    # --

    args = args_parser.parse_args(args=cmdargs)


    #
    # set up logging
    #
    level = logging.INFO
    if args.verbose:
        level = logging.DEBUG
    setup_logging(level=level)
    if args.verbose != 2:
        logging.getLogger('pylatexenc').setLevel(level=logging.INFO)

    #logger = logging.getLogger(__name__)

    #
    # Dispatch call to our main function
    #

    d = args.__dict__

    _main(**d)

    if args.view:
        if not args.output or args.output == '-':
            raise ValueError("You cannot use --view without --output")
        flm_main_oshelper.os_open_file(args.output)

    return



if __name__ == '__main__':
    run_main()
