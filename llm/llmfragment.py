import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes


class LLMFragment:
    def __init__(
            self,
            llm_text,
            environment,
            *,
            is_block_level=None,
            what='(unknown)',
            silent=False,
    ):

        self.llm_text = llm_text
        self.what = what

        self.is_block_level = is_block_level

        self.environment = environment

        self.silent = silent

        try:
            self.latex_walker, self.nodes = \
                LLMFragment.parse(self.llm_text,
                                  self.environment,
                                  is_block_level=self.is_block_level)
        except latexnodes.LatexWalkerParseError as e:
            if not self.silent:
                error_message = self.environment.get_parse_error_message(e)
                logger.error(
                    f"Parse error in latex-like markup ‘{self.what}’: {error_message}\n"
                    f"Given text was:\n‘{self.llm_text}’\n\n"
                )
            raise
        except Exception as e:
            if not self.silent:
                logger.error(f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise


    def render(self, render_context, **kwargs):
        return render_context.fragment_renderer.render_fragment(self, render_context, **kwargs)


    @classmethod
    def parse(cls, llm_text, environment, *, is_block_level=None):

        latex_walker = environment.make_latex_walker(llm_text)

        parsing_state = latex_walker.make_parsing_state(is_block_level=is_block_level)

        nodes, _ = latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser(),
            parsing_state=parsing_state,
        )

        return latex_walker, nodes



    def get_first_paragraph(self):
        r"""
        Returns a new :py:class:`LLMFragment` object that contains all material
        comprising the first paragraph in the present fragment.
        """
        nodelists_paragraphs = self.nodes.split_at_node(
            lambda n: (n.isNodeType(latexnodes_nodes.LatexSpecialsNode)
                       and n.specials_chars == '\n\n'),
            max_split=1
        )
        return self.environment.make_fragment(
            llm_text=nodelists_paragraphs[0].latex_verbatim(),
            what=f"{self.what}:first-paragraph",
            silent=self.silent
        )



    def start_node_visitor(self, node_visitor):
        node_visitor.start(self.nodes)
