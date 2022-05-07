import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
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
    ):

        self.llm_text = llm_text
        self.what = what

        self.llm_environment = llm_environment

        self.silent = silent

        try:
            self.latex_walker, self.nodes = \
                LLMFragment.parse(self.llm_text,
                                  self.llm_environment.get_parsing_state(),
                                  tolerant_parsing=self.llm_environment.tolerant_parsing)
        except latexnodes.LatexWalkerParseError as e:
            if not self.silent:
                error_message = self.llm_environment.get_parse_error_message(e)
                logger.error(f"Parse error in latex-like markup ‘{self.what}’: {error_message}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise
        except Exception as e:
            if not self.silent:
                logger.error(f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise


    def render(self, doc, fragment_renderer):
        return doc.render_fragment(self, fragment_renderer)


    @classmethod
    def parse(cls, llm_text, parsing_state, *, tolerant_parsing=False):

        latex_walker = latexwalker.LatexWalker(
            llm_text,
            # the latex_context will be overwritten anyway; don't specify `None`
            # here because that will cause pylatexenc to load its big default
            # database:
            latex_context=parsing_state.latex_context,
            tolerant_parsing=tolerant_parsing
        )

        # Set the default_parsing_state directly.
        latex_walker.default_parsing_state = parsing_state

        nodes, _ = latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser(),
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
        return self.llm_environment.make_fragment(
            llm_text=nodelists_paragraphs[0].latex_verbatim(),
            what=f"{self.what}:first-paragraph",
            silent=self.silent
        )

