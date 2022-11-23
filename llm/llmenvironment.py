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

    # transcrypt seems to behave funny with tuple([*list, new_item]) ...
    _fields = tuple(list(latexnodes.ParsingState._fields)+['is_block_level'])

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
        parsing_state_delta = LLMParsingStateDeltaSetBlockLevel(
            is_block_level=is_block_level
        )
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
        #logger.debug("Flushing paragraph: %r", paragraph_nodes)
        self.blocks.append(
            latexnodes_nodes.LatexNodeList(
                paragraph_nodes,
                parsing_state=self.latexnodelist.parsing_state,
                latex_walker=self.latexnodelist.latex_walker
            )
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

            is_head = (j == 0)
            if j == 1 and getattr(paragraph_nodes[0], 'llm_is_block_heading', False):
                # second item, but the first one was actually the paragraph
                # run-in header -- this still counts as head
                is_head = True

            if node.isNodeType(latexnodes_nodes.LatexCharsNode):
                node.llm_chars_value = self.simplify_whitespace_chars(
                    node.chars,
                    is_head=is_head,
                    is_tail=(j==lastj)
                )
                logger.debug(f"simplifying whitespace for chars node, {is_head=} {j=} {node=} --> {node.llm_chars_value=}")

        return paragraph_nodes

    def build_blocks(self):
        latexnodelist = self.latexnodelist

        #logger.debug("Decomposing node list into blocks -- %r", latexnodelist)

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
                    #logger.debug("New block heading node: %r", n)
                    self.pending_paragraph_nodes.append(n)
                    continue

                # add the node as its own block
                #logger.debug("New node block: %r", n)
                self.blocks.append(n)
                continue

            paragraph_started_yet = True
            if not self.pending_paragraph_nodes:
                paragraph_started_yet = False
            if len(self.pending_paragraph_nodes) == 1:
                if getattr(self.pending_paragraph_nodes[0], 'llm_is_block_heading', False):
                    # we've only seen it's a block lead-in heading so far
                    paragraph_started_yet = False

            if ( not paragraph_started_yet
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
                 default_parsing_state,
                 llm_environment,
                 parsing_state_event_handler=None,
                 standalone_mode=False,
                 resource_info=None,
                 parsing_mode=None,
                 what=None,
                 **kwargs):

        super().__init__(
            s=llm_text,
            default_parsing_state=default_parsing_state,
            **kwargs
        )

        self.llm_environment = llm_environment

        self.standalone_mode = standalone_mode

        # user custom additional information that can be useful to locate
        # additional resources.
        self.resource_info = resource_info
        
        self.what = what

        # stored just for information / for user messages ...
        self.parsing_mode = parsing_mode

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

    # ---

    def filter_whitespace_comments_nodes(self, nodelist):
        r"""
        Utility to filter out nodes from nodelist that are pure whitespace,
        comments, `None`, or paragraph break markers (`'\n\n'`).
        """
        return nodelist.filter(
            node_predicate_fn=self._filter_whitespace_comments_nodes_predicate,
            skip_none=True,
            skip_comments=True,
            skip_whitespace_char_nodes=True,
        )

    def _filter_whitespace_comments_nodes_predicate(self, node):
        if getattr(node, 'llm_is_paragraph_break_marker', False):
            return False
        return True
    


class LLMEnvironment:
    r"""
    ....

    - `parsing_state`: please provide a `LLMParsingState` object instance to
      serve as the default parsing state.  You should keep the `latex_context`
      field of the `parsing_state` object to `None`.  Only then will we add the
      relevant features' definitions etc.  You can still specify a base latex
      context with the `latex_context=` argument.

    - `parsing_mode_deltas` — a dictionary of parsing_mode names (strings) to
      :py:class:`ParsingStateDelta` instances.  When a nontrivial
      `parsing_mode=` argument is specified to `make_latex_walker`, this delta
      is applied onto the default parsing state.  Note that the parsing state
      delta objects must not need the `latex_context` object in their updates.
      For instance, you cannot use
      :py:class:`pylatexenc.latexnodes.ParsingStateDeltaWalkerEvent` or
      subclasses like :py:class:`ParsingStateDeltaEnterMathMode`.
    """
    def __init__(
            self,
            *,
            latex_context,
            parsing_state,
            features,
            parsing_mode_deltas=None,
            tolerant_parsing=False,
    ):
        super().__init__()

        logger.debug("LLMEnvironment constructor")

        logger.debug(f"{features=}")

        self.latex_context = latex_context
        self.parsing_state = parsing_state

        self.parsing_mode_deltas = dict(parsing_mode_deltas) if parsing_mode_deltas else {}

        self.features = list(features) # maybe list() for Transcrypt ?

        # build dict manually to ensure features are unique & for better error
        # messages
        #self.features_by_name = {f.feature_name: f for f in self.features}
        self.features_by_name = {}
        for feature in self.features:
            if feature.feature_name in self.features_by_name:
                raise ValueError(
                    f"Duplicate feature detected: feature {feature} has same name/role "
                    f"as the as already-included feature "
                    f"{self.features_by_name[feature.feature_name]}"
                )
            self.features_by_name[feature.feature_name] = feature

        self.tolerant_parsing = tolerant_parsing

        self._node_list_finalizer = NodeListFinalizer()

        if self.parsing_state.latex_context is None:

            # set the parsing_state's latex_context appropriately.
            for f in self.features:
                moredefs = f.add_latex_context_definitions()
                if moredefs:
                    logger.debug(f"Adding definitions for “{f.feature_name}”")
                    moredefs2 = dict(moredefs)
                    moredefs2.update(prepend=True)
                    self.latex_context.add_context_category(
                        f'feature--{f.feature_name}',
                        **moredefs2
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


    def feature(self, feature_name):
        return self.features_by_name[feature_name]


    parsing_state_event_handler = None

    def make_latex_walker(self, llm_text, *,
                          standalone_mode,
                          is_block_level,
                          parsing_mode=None,
                          resource_info=None,
                          tolerant_parsing=None,
                          what=None,
                          input_lineno_colno_offsets=None,
                          ):

        # logger.debug("Parsing state walker event handler = %r",
        #              self.parsing_state_event_handler,)
        
        default_parsing_state = self.make_parsing_state(
            is_block_level=is_block_level,
            parsing_mode=parsing_mode,
        )

        if tolerant_parsing is None:
            tolerant_parsing = self.tolerant_parsing

        if input_lineno_colno_offsets is None:
            input_lineno_colno_offsets = {}

        latex_walker = LLMLatexWalker(
            llm_text=llm_text,
            default_parsing_state=default_parsing_state,
            tolerant_parsing=tolerant_parsing,
            # custom additions -- 
            llm_environment=self,
            standalone_mode=standalone_mode,
            resource_info=resource_info,
            what=what,
            parsing_state_event_handler=self.parsing_state_event_handler,
            #
            line_number_offset=input_lineno_colno_offsets.get('line_number_offset', None),
            first_line_column_offset=
                input_lineno_colno_offsets.get('first_line_column_offset', None),
            column_offset=input_lineno_colno_offsets.get('column_offset', None),
        )

        return latex_walker

    def make_parsing_state(self, is_block_level, parsing_mode=None):
        # subclasses might do something interesting with parsing_mode, we ignore
        # it here

        default_parsing_state = self.parsing_state

        if parsing_mode is not None:
            try:
                parsing_state_delta = self.parsing_mode_deltas[parsing_mode]
            except KeyError as e:
                raise ValueError(f"Invalid parsing_mode ‘{parsing_mode!r}’")

            if parsing_state_delta is not None:
                default_parsing_state = parsing_state_delta.get_updated_parsing_state(
                    default_parsing_state,
                    latex_walker=None
                )

        return default_parsing_state.sub_context(is_block_level=is_block_level)


    def make_fragment(self, llm_text, **kwargs):
        try:
            fragment = LLMFragment(llm_text, environment=self, **kwargs)
            return fragment
        except: # Exception as e: --- catch anything in JS (for Transcrypt)
            if not kwargs.get('silent', False):
                logger.error(
                    "Error compiling fragment for {}\nContent was:\n‘{}’\n"
                    .format( kwargs.get('what','(unknown)'), llm_text ),
                    exc_info=True
                )
            raise

    def node_list_finalizer(self):
        return self._node_list_finalizer

    # ---

    def make_document(self, render_callback, **kwargs):
        r"""
        Instantiates a :py:class:`LLMDocument` object with the relevant arguments
        (environment instance, feature objects).  This method also calls the
        document's `initialize()` method.

        Returns the instantiated document object.
        """
        doc = LLMDocument(
            render_callback,
            environment=self,
            **kwargs
        )
        doc.initialize()
        return doc


    def get_parse_error_message(self, exception_object):
        return LatexWalkerParseErrorFormatter(exception_object).to_display_string()



# ------------------------------------------------------------------------------
