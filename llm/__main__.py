import sys
import argparse
import fileinput
import logging

from . import llmstd
from . import fmthelpers
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

    # set up logging
    level = logging.INFO
    if args.verbose:
        level = logging.DEBUG
    logging.basicConfig(level=level)
    if args.verbose != 2:
        logging.getLogger('pylatexenc').setLevel(level=logging.INFO)

    # Set up the format & formatters

    if args.format == 'text':

        fragment_renderer = TextFragmentRenderer()
        #footnote_counter_formatter = lambda n: f"[{fmthelpers.alphacounter(n)}]"
        #footnote_counter_formatter = 'fnsymbol'
        footnote_counter_formatter = lambda n: f"[{fmthelpers.fnsymbolcounter(n)}]"

    elif args.format == 'html':

        fragment_renderer = HtmlFragmentRenderer()
        footnote_counter_formatter = None # use default

    else:
        raise ValueError(f"Unknown format: ‘{args.format}’")



    # Set up the environment

    environ = llmstd.LLMStandardEnvironment(
        footnote_counter_formatter=footnote_counter_formatter,
    )

    # Get the LLM content

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

    #
    # Render the main document
    #
    result, render_context = doc.render(fragment_renderer)

    #
    # Render endnotes
    #
    endnotes_mgr = render_context.feature_render_manager('endnotes')
    if endnotes_mgr is not None:
        for category in endnotes_mgr.feature_document_manager.categories:
            category_name = category.category_name
            endnotes_this_category = endnotes_mgr.endnotes[category_name]
            if endnotes_this_category:
                endnote_heading_llm = environ.make_fragment(
                    (category_name + 's').capitalize(), # footnote -> Footnotes
                    is_block_level=False,
                )
                result = fragment_renderer.render_join_blocks([
                    result,
                    fragment_renderer.render_heading(
                        endnote_heading_llm.nodes,
                        heading_level=1,
                        render_context=render_context,
                    ),
                    endnotes_mgr.render_endnotes_category(category_name),
                ])

    sys.stdout.write(result)
    if not args.suppress_final_newline:
        sys.stdout.write("\n")
    return


if __name__ == '__main__':
    main()
