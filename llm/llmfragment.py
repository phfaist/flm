import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexwalker
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes


class LLMFragment:
    def __init__(
            self,
            llm_text,
            llm_environment,
            *,
            what='(unknown)',
            silent=False,
            tolerant_parsing=False,
    ):

        self.llm_text = llm_text
        self.what = what

        self.llm_environment = llm_environment

        self.silent = silent
        self.tolerant_parsing = tolerant_parsing

        try:
            self.latex_walker, self.nodes = \
                LLMFragment.parse(self.llm_text, self.llm_environment.latex_context_db,
                                  tolerant_parsing=self.tolerant_parsing)
        except Exception as e:
            if not self.silent:
                logger.error(f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise


    def render(self, doc, fragment_renderer):
        return doc.render_fragment(self, fragment_renderer)


    @classmethod
    def parse(cls, llm_text, latex_context_db, *, tolerant_parsing=False):

        latex_walker = latexwalker.LatexWalker(
            llm_text,
            latex_context=latex_context_db,
            tolerant_parsing=tolerant_parsing
        )

        nodes, _ = latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser()
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
        return self.llm_environment.make_llm_fragment(
            llm_text=nodelists_paragraphs[0].latex_verbatim(),
            what=f"{self.what}:first-paragraph",
            silent=self.silent
        )

