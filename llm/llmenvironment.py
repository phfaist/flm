import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
from pylatexenc import macrospec
from pylatexenc.latexnodes import LatexWalkerParseError, LatexWalkerParseErrorFormatter
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc import latexwalker

from .llmfragment import LLMFragment
from .llmdocument import LLMDocument



### BEGINPATCH_UNIQUE_OBJECT_ID
fn_unique_object_id = id
### ENDPATCH_UNIQUE_OBJECT_ID


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


# ------------------------------------------------------------------------------


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



# ------------------------------------------------------------------------------


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

        is_head = True
        tail_char_node_info = None
        first_node = None
        char_nodes = []
        for j, node in enumerate(paragraph_nodes):

            if len(char_nodes) == 0 and first_node is not None \
               and getattr(first_node, 'llm_is_block_heading', False):
                # second item, but the first one was actually the paragraph
                # run-in header -- this still counts as head
                is_head = True

            if node.isNodeType(latexnodes_nodes.LatexCharsNode):
                info = {'is_head': is_head, 'is_tail': False}
                char_nodes.append( (node, info ) )
                is_head = False
                tail_char_node_info = info
            elif not node.isNodeType(latexnodes_nodes.LatexCommentNode):
                if first_node is None:
                    first_node = node
                is_head = False
                tail_char_node_info = None

        # find last char_node and mark it is_tail:
        if tail_char_node_info is not None:
            tail_char_node_info['is_tail'] = True

        for (char_node, info) in char_nodes:
            char_node.llm_chars_value = self.simplify_whitespace_chars(
                char_node.chars,
                is_head=info['is_head'],
                is_tail=info['is_tail'],
            )
            logger.debug(
                "simplifying whitespace for chars node, info['is_head']=%r char_node=%r "
                "--> char_node.llm_chars_value=%r",
                info['is_head'], char_node, char_node.llm_chars_value
            )

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



# ----------------------------



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




# ----------------------------



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

    def make_node(self, node_class, **kwargs):
        node = super().make_node(node_class, **kwargs)
        # attach a node ID, given by the object ID of the node
        node.node_id = fn_unique_object_id(node)
        return node

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
    



# ------------------------------------------------------------------------------

def features_sorted_by_dependencies(features):
    r"""
    This function returns the given list of features, but sorted such that
    features always appear after any of their dependencies.

    The order is deterministic, and does not depend on the initial ordering.
    Any independent features are sorted by their name (to ensure a deterministic
    order, even if it is arbitrary).
    
    This function raises an error if:

    - A feature was specified twice;

    - The feature dependency graph has a cycle.
    """

    # list() both for Transcrypt as well as to avoid modifying any original iterable/list
    features_to_sort = list(features)

    # build the features-by-name dictionary manually, so that we detect
    # and report duplicates.
    features_by_name = {}
    for feature in features_to_sort:
        if feature.feature_name in features_by_name:
            raise ValueError(
                f"Duplicate feature detected: feature {repr(feature)} has the same name "
                f"(‘{feature.feature_name}’) as the as already-included feature "
                f"{features_by_name[feature.feature_name]}"
            )
        features_by_name[feature.feature_name] = feature

    # Start by sorting all feature instances alphabetically by name.  This step
    # ensures that the order we get at the end does not depend on the order of
    # the features specified in the initial list.
    features_to_sort.sort(key=lambda f: f.feature_name)

    # check that all dependencies are met!
    for feature in features_to_sort:
        if feature.feature_dependencies is None:
            continue
        for fdepname in feature.feature_dependencies:
            if fdepname not in features_by_name:
                raise ValueError(
                    f"Feature ‘{feature.feature_name}’ ({repr(feature)}) has unmet "
                    f"dependency ‘{fdepname}’"
                )

    def get_feature_dependencies(f):
        deps = set()
        if f.feature_dependencies is not None:
            for fdepname in f.feature_dependencies:
                deps.add(fdepname)
        if f.feature_optional_dependencies is not None:
            for foptdepname in f.feature_optional_dependencies:
                if foptdepname in features_by_name:
                    deps.add(foptdepname)
        return sorted(list(deps))

    # This is the collection of all our graph edges.  The edge direction is
    # opposite to the relevant ordering in the sorting algorithm implemented
    # below.  Alternatively, think of the edge direction for the algorithm's
    # purposes stored as {(edge target): [list of edge sources]} in this dictionary.
    #
    # This object will be modified in the course of the algorithm below.
    all_feature_dependencies = dict([
        (fname, get_feature_dependencies(f))
        for fname, f in features_by_name.items()
    ])

    def get_feature_dependents(fparentname, all_feature_dependencies):
        dependents = set()
        for fname, fdepnames in all_feature_dependencies.items():
            for fdepname in fdepnames:
                if fdepname == fparentname:
                    dependents.add(fname)
        return sorted(list(dependents))

    # # This representation of all the graph edges is reversed, so that we have
    # # {(feature A): [list of feature names depending on feature A]}
    # #
    # # This object will be modified in the course of the algorithm below.
    # all_feature_dependents = dict([
    #     (fname, get_feature_dependents(fname, all_feature_dependencies))
    #     for fname, f in features_by_name.items()
    # ])

    # https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm

    sorted_features = [] # --> L
    root_features = [ # --> S
        f
        # reverse the list so that the list's pop() method gets the first element
        for f in reversed(features_to_sort)
        if len(all_feature_dependencies[f.feature_name]) == 0
    ]

    # # --> the graph's edges
    # dependency_pairs = []
    # for feature_name, fdeplist in all_feature_dependencies.items():
    #     for fdepname in fdeplist:
    #         dependency_pairs.append( (feature_name, fdepname) )

    # start the main loop
    while len(root_features) > 0:
        n = root_features.pop()
        sorted_features.append(n)

        n_name = n.feature_name

        iter_dependents = get_feature_dependents(n_name, all_feature_dependencies)
        for fdependentname in iter_dependents:
            # "m" has name fdependentname

            all_feature_dependencies[fdependentname].remove(n_name)

            # does 'm' have any other incoming edges?  I.e., does m have any
            # other dependencies?
            if len(all_feature_dependencies[fdependentname]) == 0:
                # no other dependencies
                root_features.insert(0, features_by_name[fdependentname])
        
                
    problematic_features = []
    for fsrcname, featuredeps in all_feature_dependencies.items():
        if len(featuredeps) > 0:
            problematic_features.append(
                f"‘{fsrcname}’ → " + ", ".join([f"‘{fdepname}’" for fdepname in featuredeps])
            )
    if len(problematic_features) > 0:
        raise ValueError(
            f"The feature dependency graph has a cycle!  Problematic dependencies:"
            + "; ".join(problematic_features)
        )

    if len(sorted_features) != len(features_to_sort):
        raise RuntimeError(
            "Internal error, we didn't get all the features right when ordering them!"
        )

    return sorted_features, features_by_name

    # ### Remember, we need to reverse the graph edge directions because in our
    # ### graph we point to "dependencies", so "parent" nodes that need to come
    # ### before the node that has those dependencies
    #
    # L ← Empty list that will contain the sorted elements
    # S ← Set of all nodes with no incoming edge
    #
    # while S is not empty do
    #     remove a node n from S
    #     add n to L
    #     for each node m with an edge e from n to m do
    #         remove edge e from the graph
    #         if m has no other incoming edges then
    #             insert m into S
    #
    # if graph has edges then
    #     return error   (graph has at least one cycle)
    # else 
    #     return L   (a topologically sorted order)

    

            

# ------------------------------------------------------------------------------


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
            features,
            parsing_state,
            latex_context,
            *,
            tolerant_parsing=False,
            parsing_mode_deltas=None,
    ):
        super().__init__()

        logger.debug("LLMEnvironment constructor")

        self.latex_context = latex_context
        self.parsing_state = parsing_state

        self.parsing_mode_deltas = dict(parsing_mode_deltas) if parsing_mode_deltas else {}

        self.features, self.features_by_name = features_sorted_by_dependencies(features)

        logger.debug("Creating environment; features: %r", self.features);

        self.tolerant_parsing = tolerant_parsing

        self._node_list_finalizer = NodeListFinalizer()

        if self.parsing_state.latex_context is None:

            # set the parsing_state's latex_context appropriately.
            for f in self.features:
                moredefs = f.add_latex_context_definitions()
                logger.debug(f"add_latex_context_definitions of “{f.feature_name}” -> {repr(moredefs)}")
                if moredefs is not None:
                    moredefs = dict(moredefs)
                    if len(moredefs):
                        logger.debug(f"Adding definitions for “{f.feature_name}”")
                        moredefs.update(prepend=True)
                        self.latex_context.add_context_category(
                            f'feature--{f.feature_name}',
                            **moredefs
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
    r"""
    There is no parsing state event handler by default.  If you want to
    allow unknown macros etc. in math mode, set this property to a
    LLMLatexWalkerParsingStateEventHandler() instance.
    """

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

        kwargs = {}
        if is_block_level is not None:
            kwargs['is_block_level'] = is_block_level

        return default_parsing_state.sub_context(**kwargs)


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



    environment_get_parse_error_message = None

    def get_parse_error_message(self, exception_object):
        if self.environment_get_parse_error_message is not None:
            return self.environment_get_parse_error_message(exception_object)
        return LatexWalkerParseErrorFormatter(exception_object).to_display_string()




# ------------------------------------------------------------------------------



def standard_parsing_state(*,
                           force_block_level=None,
                           enable_comments=True,
                           comment_start='%%',
                           extra_forbidden_characters='',
                           dollar_inline_math_mode=False):
    r"""
    Return a `ParsingState` configured in a standard way for parsing LLM
    content.  E.g., we typically disable commands and $-math mode, unless you
    specify keyword arguments to override this behavior.

    The `latex_context` field of the returned object is `None`, and this value
    will be accepted if you use this parsing state as an argument to
    :py:func:`standard_features()`.
    """

    forbidden_characters = str(extra_forbidden_characters)
    if not dollar_inline_math_mode and '$' not in forbidden_characters:
        forbidden_characters += '$'
    if (not enable_comments or comment_start != '%') and '%' not in forbidden_characters:
        # if comments are disabled entirely, we forbid the '%' sign completely.
        forbidden_characters += '%'

    latex_inline_math_delimiters = [ (r'\(', r'\)'), ]

    if dollar_inline_math_mode:
        latex_inline_math_delimiters.append( ('$', '$') )

    return LLMParsingState(
        is_block_level=force_block_level,
        latex_context=None,
        enable_comments=enable_comments,
        comment_start=comment_start,
        latex_inline_math_delimiters=latex_inline_math_delimiters,
        latex_display_math_delimiters=[ (r'\[', r'\]') ],
        forbidden_characters=forbidden_characters,
    )

# ------------------------------------------------------------------------------

class LLMLatexWalkerMathContextParsingStateEventHandler(
        latexnodes.LatexWalkerParsingStateEventHandler
):
    math_mode_extend_context = {
        'unknown_macro_spec': macrospec.MacroSpec(''),
        'unknown_environment_spec': macrospec.EnvironmentSpec(''),
        'unknown_specials_spec': macrospec.SpecialsSpec(''),
    }

    def enter_math_mode(self, math_mode_delimiter=None, trigger_token=None):
        set_attributes = dict(
            in_math_mode=True,
            math_mode_delimiter=math_mode_delimiter,
        )
        logger.debug("LLMWalkerEventsParsingStateDeltasProvider.enter_math_mode ! "
                     "math_mode_delimiter=%r, trigger_token=%r, set_attributes=%r",
                     math_mode_delimiter, trigger_token, set_attributes)
        return macrospec.ParsingStateDeltaExtendLatexContextDb(
            set_attributes=set_attributes,
            extend_latex_context=self.math_mode_extend_context
        )

    def leave_math_mode(self, trigger_token=None):
        #logger.debug("LLMWalkerEventsParsingStateDeltasProvider.leave_math_mode !")
        return macrospec.ParsingStateDeltaExtendLatexContextDb(
            set_attributes=dict(
                in_math_mode=False,
                math_mode_delimiter=None
            ),
            extend_latex_context=dict(
                unknown_macro_spec=None,
                unknown_environment_spec=None,
                unknown_specials_spec=None,
            )
        )


# ------------------------------------------------------------------------------


def standard_environment_get_parse_error_message(exception_object):
    msg = None
    error_type_info = exception_object.error_type_info
    if error_type_info:
        what = error_type_info['what']
        if what == 'token_forbidden_character':
            if error_type_info['forbidden_character'] == '%':
                msg = (
                    r"LaTeX comments are not allowed here. Use ‘\%’ to typeset a "
                    r"literal percent sign."
                )
            elif error_type_info['forbidden_character'] == '$':
                msg = (
                    r"You can't use ‘$’ here. LaTeX math should be typeset using "
                    r"\(...\) for inline math and \[...\] for unnumbered display "
                    r"equations. Use ‘\$’ for a literal dollar sign."
                )
    if not msg:
        msg = exception_object.msg

    errfmt = latexnodes.LatexWalkerParseErrorFormatter(exception_object)

    msg += errfmt.format_full_traceback()

    return msg



# ------------------------------------------------------------------------------


def make_standard_environment(features, parsing_state=None, latex_context=None,
                              llm_environment_options=None,
                              parsing_state_options=None):

    if latex_context is None:
        latex_context = macrospec.LatexContextDb()

    if parsing_state is None:
        parsing_state_options_2 = {}
        if parsing_state_options is not None:
            parsing_state_options_2 = parsing_state_options

        parsing_state = standard_parsing_state(**parsing_state_options_2)

    parsing_state_event_handler = LLMLatexWalkerMathContextParsingStateEventHandler()

    llm_environment_options_2 = {}
    if llm_environment_options is not None:
        llm_environment_options_2 = llm_environment_options

    environment = LLMEnvironment(
        features,
        parsing_state,
        latex_context,
        **llm_environment_options_2
    )

    environment.parsing_state_event_handler = parsing_state_event_handler

    environment.environment_get_parse_error_message = \
        standard_environment_get_parse_error_message

    return environment



