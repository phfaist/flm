
from pylatexenc import macrospec

from .llmspecinfo import LLMSpecInfo, LLMMacroSpec, Heading
from .llmenvironment import make_arg_spec

from .feature import Feature


class FeatureHeadings(Feature):
    r"""
    Add support for headings via LaTeX commands, including ``\section``,
    ``\subsection``, ``\subsubsection``, ``\paragraph``, etc.
    """

    feature_name = 'headings'
    # no managers needed
    DocumentManager = None
    RenderManager = None

    def __init__(self, section_commands_by_level=None):
        super().__init__()
        if section_commands_by_level is None:
            section_commands_by_level = [
                r"section",
                r"subsection",
                r"subsubsection",
                r"paragraph",
                r"subparagraph",
                r"subsubparagraph",
            ]
        self.section_commands_by_level = section_commands_by_level

    def add_latex_context_definitions(self):
        return dict(
            macros=[
                LLMMacroSpec(
                    sectioncmdname,
                    [
                        make_arg_spec('{', argname='text'),
                    ],
                    llm_specinfo=Heading(heading_level=1+j)
                )
                for j, sectioncmdname in enumerate(self.section_commands_by_level)
                if sectioncmdname is not None
            ]
        )

