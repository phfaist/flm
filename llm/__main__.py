import sys
import argparse
import fileinput

from . import llmstd
from .fragmentrenderer.text import TextFragmentRenderer
from .fragmentrenderer.html import HtmlFragmentRenderer


def main(cmdargs=None):
    
    args_parser = argparse.ArgumentParser(
        prog='llm',
        epilog='Have a lot of llm fun!'
    )

    args_parser.add_argument('-c', '--llm-content', action='store',
                             help="LLM content to parse and convert")
    args_parser.add_argument('-f', '--format', action='store',
                             default='html',
                             help="LLM content to parse and convert")
    args_parser.add_argument('-n', '--suppress-final-newline', action='store_true',
                             default=False,
                             help="Do not add a newline at the end of the output")

    args_parser.add_argument('files', metavar="FILE", nargs='*',
                             help='Input files (if none specified, read from stdandard input)')

    # --

    args = args_parser.parse_args(args=cmdargs)

    environ = llmstd.LLMStandardEnvironment()

    llm_content = ''
    if args.llm_content:
        if args.files:
            raise ValueError(
                "You cannot specify both FILEs and --llm-content options. "
                "Type `llm --help` for more information."
            )
        llm_content = args.llm_content
    else:
        for line in fileinput.input(files=args.files):
            llm_content += line


    fragment = environ.make_fragment(llm_content)
    
    doc = environ.make_document(fragment.render)

    if args.format == 'text':
        fragment_renderer = TextFragmentRenderer()
    elif args.format == 'html':
        fragment_renderer = HtmlFragmentRenderer()
    else:
        raise ValueError(f"Unknown format: ‘{args.format}’")

    result, render_context = doc.render(fragment_renderer)

    sys.stdout.write(result)
    if not args.suppress_final_newline:
        sys.stdout.write("\n")
    return


if __name__ == '__main__':
    main()
