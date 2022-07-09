import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes

from .llmrendercontext import LLMStandaloneModeRenderContext


class LLMFragment:
    r"""
    A fragment of LLM-formatted code.

    A LLM fragment is intended to later be inserted in a document so that it can
    be rendered into the desired output format (HTML, plain text).  If the
    fragment is *standalone* (`standalone_mode=True`), then some LLM features
    are disabled (typically, for instance, cross-references) and the fragment
    can be rendered directly on its own without inserting it in a document, see
    :py:meth:`render_standalone()`.

    .....................

    The argument `resource_info` can be set to any custom object that can help
    locate resources called by LLM text.  For instance, a `\includegraphics{}`
    call might wish to look for graphics in the same filesystem folder as a file
    that contained the LLM code; the `resource_info` object can be used to store
    the filesystem folder of the LLM code forming this fragment.
    """

    def __init__(
            self,
            llm_text,
            environment,
            *,
            is_block_level=None,
            resource_info=None,
            standalone_mode=False,
            what='(unknown)',
            silent=False,
    ):

        self.llm_text = llm_text
        self.environment = environment

        self.is_block_level = is_block_level
        self.resource_info = resource_info
        self.standalone_mode = standalone_mode
        self.what = what
        self.silent = silent

        if isinstance(llm_text, latexnodes_nodes.LatexNodeList):
            # We want to initialize a fragment with already-parsed node lists.
            # This is for internal use only!
            self.nodes = self.llm_text
            self.latex_walker = self.nodes.latex_walker
            self.llm_text = self.nodes.latex_verbatim()
            return

        try:
            self.latex_walker, self.nodes = \
                LLMFragment.parse(
                    self.llm_text,
                    self.environment,
                    standalone_mode=self.standalone_mode,
                    is_block_level=self.is_block_level,
                    what=self.what,
                    resource_info=self.resource_info,
                )
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


    def _attributes(self, **kwargs):
        d = dict(
            is_block_level=self.is_block_level,
            resource_info=self.resource_info,
            standalone_mode=self.standalone_mode,
            silent=self.silent,
            what=self.what,
        )
        d.update(kwargs)
        return d


    def render(self, render_context, **kwargs):
        return render_context.fragment_renderer.render_fragment(self, render_context, **kwargs)

    def render_standalone(self, fragment_renderer):
        if not self.standalone_mode:
            raise ValueError(
                "You can only use render_standalone() on a fragment that "
                "was parsed in standalone mode (use `standalone_mode=True` "
                "in the LLMFragment constructor)"
            )
        render_context = LLMStandaloneModeRenderContext(fragment_renderer=fragment_renderer)
        return self.render(render_context)

    @classmethod
    def parse(cls, llm_text, environment, *,
              standalone_mode=False, resource_info=None, is_block_level=None, what=None):

        latex_walker = environment.make_latex_walker(
            llm_text,
            resource_info=resource_info,
            standalone_mode=standalone_mode,
            what=what,
        )

        parsing_state = latex_walker.make_parsing_state(is_block_level=is_block_level)

        nodes, _ = latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser(),
            parsing_state=parsing_state,
        )

        return latex_walker, nodes


    def whitespace_stripped(self):
        new_fragment = self.environment.make_fragment(
            self.llm_text.strip(),
            **self._attributes(what=f"{self.what}:whitespace-stripped")
        )
        return new_fragment

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

        nodelists_paragraphs = [
            nls_p
            for nls_p in nodelists_paragraphs
            if len(nls_p) > 0
        ]

        if not nodelists_paragraphs:
            return self

        logger.debug(f"{nodelists_paragraphs[0]=}")

        thenodes = nodelists_paragraphs[0]

        logger.debug(f"First paragraph -> {thenodes=}")
        return self.environment.make_fragment(
            llm_text=thenodes,
            **self._attributes(what=f"{self.what}:first-paragraph")
        )



    def start_node_visitor(self, node_visitor):
        node_visitor.start(self.nodes)
