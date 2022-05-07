
from .llmfragment import LLMFragment

# ------------------------------------------------------------------------------


class LLMEnvironment:
    def __init__(self, latex_context=None, tolerant_parsing=False):
        super().__init__()
        self.latex_context = latex_context
        self.tolerant_parsing = tolerant_parsing

    def make_latex_walker(self, llm_text):
        return latexwalker.LatexWalker(
            llm_text,
            latex_context=self.latex_context,
            tolerant_parsing=self.tolerant_parsing
        )

    def make_fragment(self, llm_text, **kwargs):
        return LLMFragment(llm_text, llm_environment=self, **kwargs)

    def get_parse_error_message(self, exception_object):
        return str( exception_object )


# ------------------------------------------------------------------------------

