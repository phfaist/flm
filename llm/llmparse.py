
from pylatexenc import latexwalker
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes


class LLMEnvironmentBase:
    def __init__(self, latex_context_db):
        super().__init__()
        self.latex_context_db = latex_context_db

    def llm(self, llm_text, **kwargs):
        return LLM(llm_text, llm_environment=self, **kwargs)



class LLM:
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
                LLM.parse(self.llm_text, self.llm_environment.latex_context_db,
                          tolerant_parsing=self.tolerant_parsing)
        except Exception as e:
            if not self.silent:
                logger.error(f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                             f"Given text was:\n‘{self.llm_text}’\n\n")
            raise


    @classmethod
    def parse(cls, llm_text, latex_context_db, *, tolerant_parsing=False):

        latex_walker = latexwalker.LatexWalker(
            minilatex,
            latex_context=latex_context_db,
            tolerant_parsing=tolerant_parsing
        )

        nodes, _ = latex_walker.parse_content( latexnodes_parsers.LatexGeneralNodesParser() )

        return latex_walker, nodes


    def get_first_paragraph(self):
        r"""
        Returns a new LLM object
        """
        nodelists_paragraphs = self.nodes.split_at_node(
            lambda n: (n.isNodeType(latexnodes_nodes.LatexSpecialsNode)
                       and n.specials_chars == '\n\n'),
            max_split=1
        )
        return self.llm_environment.llm(
            llm_text=nodelists_paragraphs[0].latex_verbatim(),
            what=f"{self.what}:first-paragraph",
            silent=self.silent
        )

