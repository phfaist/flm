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
            what='(unknown)',
            silent=False,
    ):

        self.llm_text = llm_text
        self.what = what

        self.environment = environment

        self.silent = silent

        try:
            self.latex_walker, self.nodes = \
                LLMFragment.parse(self.llm_text,
                                  self.environment,)
        except latexnodes.LatexWalkerParseError as e:
            if not self.silent:
                error_message = self.environment.get_parse_error_message(e)
                logger.error(f"Parse error in latex-like markup ‘{self.what}’: {error_message}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise
        except Exception as e:
            if not self.silent:
                logger.error(f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise


    def render(self, doc, fragment_renderer, **kwargs):
        return doc.render_fragment(self, fragment_renderer, **kwargs)


    @classmethod
    def parse(cls, llm_text, environment):

        tolerant_parsing = environment.tolerant_parsing

        latex_walker = environment.make_latex_walker(llm_text)

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
        return self.environment.make_fragment(
            llm_text=nodelists_paragraphs[0].latex_verbatim(),
            what=f"{self.what}:first-paragraph",
            silent=self.silent
        )

