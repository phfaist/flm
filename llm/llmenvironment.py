
from pylatexenc import latexwalker

from .llmfragment import LLMFragment
from .llmdocument import LLMDocument

# ------------------------------------------------------------------------------


class LLMEnvironment:
    def __init__(self,
                 *,
                 latex_context,
                 parsing_state,
                 features,
                 tolerant_parsing=False):
        super().__init__()

        print("**** LLMEnvironment constructor ****")

        print(f"{features=}")

        self.latex_context = latex_context
        self.parsing_state = parsing_state
        self.features = features
        self.tolerant_parsing = tolerant_parsing

        if self.parsing_state.latex_context is None:

            # set the parsing_state's latex_context appropriately.
            for f in features:
                moredefs = f.add_latex_context_definitions()
                if moredefs:
                    print(f"*** Adding definitions for “{f.feature_name}”")
                    self.latex_context.add_context_category(
                        f'feature--{f.feature_name}',
                        **moredefs,
                        prepend=True,
                    )

            # prevent further changes to latex context
            self.latex_context.freeze()

            # set the parsing state's latex_context
            self.parsing_state.latex_context = self.latex_context
            
        elif self.latex_context is not None:
            # parsing_state might have `latex_context=None` if we provide a
            # specific latex_context instance to use
            raise RuntimeError(
                "The specified `parsing_state` instance already has a latex_context set"
            )



    def make_latex_walker(self, llm_text):

        latex_walker = latexwalker.LatexWalker(
            llm_text,
            # the latex_context will be overwritten anyway; don't specify `None`
            # here because that will cause pylatexenc to load its big default
            # database:
            latex_context=self.latex_context,
            tolerant_parsing=self.tolerant_parsing
        )

        # Set the default_parsing_state directly.
        latex_walker.default_parsing_state = self.parsing_state

        return latex_walker


    def make_fragment(self, llm_text, **kwargs):
        return LLMFragment(llm_text, environment=self, **kwargs)


    def make_document(self, render_callback):
        doc = LLMDocument(
            render_callback,
            environment=self,
            features=self.features,
        )
        doc.initialize()
        return doc


    def get_parse_error_message(self, exception_object):
        return str( exception_object )


# ------------------------------------------------------------------------------

