import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
from pylatexenc.latexnodes import LatexWalkerParseError, LatexWalkerParseErrorFormatter
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc import latexwalker

from .llmfragment import LLMFragment
from .llmdocument import LLMDocument

# ------------------------------------------------------------------------------


class LLMParsingState(latexnodes.ParsingState):

    _fields = tuple([*latexnodes.ParsingState._fields, 'is_block_level'])

    def set_fields(self, *, is_block_level=None, **kwargs):
        super().set_fields(**kwargs)
        self.is_block_level = is_block_level


class LLMParsingStateDeltaSetBlockLevel(latexnodes.ParsingStateDelta):
    def __init__(self, is_block_level=None):
        super().__init__(
            set_attributes=dict(is_block_level=is_block_level)
        )


def LLMArgumentSpec(parser, argname, is_block_level=False):
    r"""
    ..........

    I might turn this function into a proper subclass of `LatexArgumentSpec` in
    the future.
    """
    parsing_state_delta = None
    if is_block_level is not None:
        parsing_state_delta = LLMParsingStateDeltaSetBlockLevel(is_block_level)
    return latexnodes.LatexArgumentSpec(
        parser=parser,
        argname=argname,
        parsing_state_delta=parsing_state_delta,
    )



class BlocksBuilder:

    rx_space = re.compile(r'[ \t\n\r]+')
    rx_only_space = re.compile(r'^[ \t\n\r]+$')

    def __init__(self, latexnodelist):
        super().__init__()
        self.latexnodelist = latexnodelist
        self.pending_paragraph_nodes = []
        self.blocks = []

    def flush_paragraph(self):
        if not self.pending_paragraph_nodes:
            return
        paragraph_nodes = self.pending_paragraph_nodes
        paragraph_nodes = self.finalize_paragraph(paragraph_nodes)
        logger.debug("Flushing paragraph: %r", paragraph_nodes)
        self.blocks.append(
            latexnodes_nodes.LatexNodeList(paragraph_nodes)
        )
        self.pending_paragraph_nodes = []

    def simplify_whitespace_chars(self, chars, is_head=False, is_tail=False):
        newchars = self.rx_space.sub(' ', chars)
        if is_head:
            newchars = newchars.lstrip()
        if is_tail:
            newchars = newchars.rstrip()
        return newchars

    def finalize_paragraph(self, paragraph_nodes):
        if not paragraph_nodes:
            return paragraph_nodes

        # simplify white space correctly.
        lastj = len(paragraph_nodes) - 1
        for j, node in enumerate(paragraph_nodes):
            if node.isNodeType(latexnodes_nodes.LatexCharsNode):
                node.llm_chars_value = self.simplify_whitespace_chars(
                    node.chars,
                    is_head=(j==0),
                    is_tail=(j==lastj)
                )

        return paragraph_nodes

    def build_blocks(self):
        latexnodelist = self.latexnodelist

        logger.debug("Decomposing node list into blocks -- %r", latexnodelist)

        assert( len(self.blocks) == 0 )

        for n in latexnodelist:
            n_is_block_level = getattr(n, 'llm_is_block_level', None)
            n_is_block_heading = getattr(n, 'llm_is_block_heading', False)
            if n_is_block_level:
                # new block-level item -- causes paragraph break
                self.flush_paragraph()

                if getattr(n, 'llm_is_paragraph_break_marker', False):
                    # it's only a paragraph break marker '\n\n' -- don't include
                    # it as a block
                    continue

                if n_is_block_heading:
                    # block break, but add the item to be included in a new
                    # paragraph instead of on its own
                    logger.debug("New block heading node: %r", n)
                    self.pending_paragraph_nodes.append(n)
                    continue

                # add the node as its own block
                logger.debug("New node block: %r", n)
                self.blocks.append(n)
                continue

            if (not self.pending_paragraph_nodes
                and n.isNodeType(latexnodes_nodes.LatexCharsNode)
                and self.rx_only_space.match(n.chars)):
                # white space characters, and we haven't started a new paragraph
                # yet -- ignore them.
                continue

            # add the item to be included in the current paragraph.
            self.pending_paragraph_nodes.append(n)

        # finalize the last paragraph with any pending items
        self.flush_paragraph()

        return self.blocks



class NodeListFinalizer:
    r"""
    Responsible for adding additional meta-information to nodes to tell whether
    nodes and node lists are block-level or inline text.
    """
    def finalize_nodelist(self, latexnodelist):
        r"""
        Inspect the node list and set information about whether or not it is block level.
        
        * If the parsing state does not specify the block level, infer whether
          or not the node list is block-level by inspecting the nodes in the node list.

        * If the parsing state does specify that the node list is not block
          level (i.e. it is inline level), then make sure that all nodes in the
          node list are allowed to appear there.
        
        * In all cases, inspect how the node list is split into blocks, and
          store the relevant information in a property `llm_blocks_info`
        """
        is_block_level = latexnodelist.parsing_state.is_block_level
        if is_block_level is None:
            # need to infer block level
            is_block_level = self.infer_is_block_level_nodelist(latexnodelist)

        latexnodelist.llm_is_block_level = is_block_level

        # consistency checks
        if not is_block_level:
            # make sure there are no block-level nodes in the list
            for n in latexnodelist:
                if getattr(n, 'llm_is_block_level', None):
                    raise LatexWalkerParseError(
                        msg=
                          f"Content is not allowed in inline text "
                          f"(not block level): ‘{n.latex_verbatim()}’",
                        pos=n.pos,
                    )
                # simplify any white space!
                if n.isNodeType(latexnodes_nodes.LatexCharsNode):
                    n.llm_chars_value = self.simplify_whitespace_chars_inline(
                        n.chars
                    )

            # all set -- return the node list
            return latexnodelist

        # prepare the node list into blocks (e.g., paragraphs or other
        # block-level items like enumeration lists)
        if is_block_level:
            blocks_builder = self.make_blocks_builder(latexnodelist)
            llm_blocks = blocks_builder.build_blocks()
            latexnodelist.llm_blocks = llm_blocks

        return latexnodelist

    def infer_is_block_level_nodelist(self, latexnodelist):
        for n in latexnodelist:
            n_is_block_level = getattr(n, 'llm_is_block_level', None)
            if n_is_block_level:
                return True
        return False

    def simplify_whitespace_chars_inline(self, chars):
        return self.rx_inline_space.sub(' ', chars)

    make_blocks_builder = BlocksBuilder
                    
    rx_inline_space = BlocksBuilder.rx_space



class LLMLatexWalker(latexwalker.LatexWalker):
    r"""
    A LatexWalker class that is meant to parse LLM code.

    This walker class takes care to add additional information to node lists
    that is then needed by the code that renders LLM fragments into output
    formats (e.g. HTML).  For instance, node lists need to be split into
    "blocks" (paragraphs or block-level content) as they are parsed (see
    :py:meth:`make_nodelist()`).

    This class also accepts a custom parsing state event handler instance.  See
    :py:mod:`llm.llmstd` for how it is set in the standard environment.
    """
    def __init__(self,
                 *,
                 llm_text,
                 parsing_state,
                 llm_environment,
                 parsing_state_event_handler=None,
                 standalone_mode=False,
                 resource_info=None,
                 **kwargs):

        super().__init__(
            s=llm_text,
            # the latex_context will be overwritten anyway; don't specify `None`
            # here because that will cause pylatexenc to load its big default
            # database:
            latex_context=parsing_state.latex_context,
            **kwargs
        )

        # set the latex walker's default parsing state here:
        self.default_parsing_state = parsing_state

        self.llm_environment = llm_environment

        self.standalone_mode = standalone_mode

        # user custom additional information that can be useful to locate
        # additional resources.
        self.resource_info = resource_info

        self._parsing_state_event_handler = parsing_state_event_handler

    def parsing_state_event_handler(self):
        if self._parsing_state_event_handler:
            return self._parsing_state_event_handler
        return super().parsing_state_event_handler()

    def make_nodelist(self, nodelist, parsing_state, **kwargs):
        nl = super().make_nodelist(nodelist=nodelist, parsing_state=parsing_state, **kwargs)
        # check & see if the block level is consistent
        nl = self.llm_environment.node_list_finalizer().finalize_nodelist(nl)
        return nl



class LLMEnvironment:
    def __init__(self,
                 *,
                 latex_context,
                 parsing_state,
                 features,
                 tolerant_parsing=False):
        super().__init__()

        logger.debug("LLMEnvironment constructor")

        logger.debug(f"{features=}")

        self.latex_context = latex_context
        self.parsing_state = parsing_state
        self.features = features
        self.features_by_name = {f.feature_name: f for f in self.features}
        self.tolerant_parsing = tolerant_parsing

        self._node_list_finalizer = NodeListFinalizer()

        if self.parsing_state.latex_context is None:

            # set the parsing_state's latex_context appropriately.
            for f in features:
                moredefs = f.add_latex_context_definitions()
                if moredefs:
                    logger.debug(f"Adding definitions for “{f.feature_name}”")
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



    parsing_state_event_handler = None

    def make_latex_walker(self, llm_text, *, standalone_mode, resource_info, ):

        logger.debug("Parsing state walker event handler = %r",
                     self.parsing_state_event_handler,)

        latex_walker = LLMLatexWalker(
            llm_text=llm_text,
            parsing_state=self.parsing_state,
            tolerant_parsing=self.tolerant_parsing,
            # custom additions -- 
            llm_environment=self,
            standalone_mode=standalone_mode,
            resource_info=resource_info,
            parsing_state_event_handler=self.parsing_state_event_handler,
        )

        return latex_walker

    def make_fragment(self, llm_text, **kwargs):
        fragment = LLMFragment(llm_text, environment=self, **kwargs)
        return fragment


    def node_list_finalizer(self):
        return self._node_list_finalizer

    # ---

    def make_document(self, render_callback):
        r"""
        Instantiates a :py:class:`LLMDocument` object with the relevant arguments
        (environment instance, feature objects).  This method also calls the
        document's `initialize()` method.

        Returns the instantiated document object.
        """
        doc = LLMDocument(
            render_callback,
            environment=self,
            features=self.features,
        )
        doc.initialize()
        return doc


    def get_parse_error_message(self, exception_object):
        return LatexWalkerParseErrorFormatter(exception_object).to_display_string()



# ------------------------------------------------------------------------------
