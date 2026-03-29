r"""
FLM fragment: a compiled piece of FLM-formatted text.

An :py:class:`FLMFragment` represents a piece of FLM source text that has
been parsed into a node tree.  Fragments can be rendered standalone (if
parsed with ``standalone_mode=True``) or within a document context for
cross-reference resolution, consistent numbering, and footnotes.

Create fragments via
:py:meth:`~flm.flmenvironment.FLMEnvironment.make_fragment`.
"""

import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
import pylatexenc.latexnodes.parsers as latexnodes_parsers
import pylatexenc.latexnodes.nodes as latexnodes_nodes

from .flmrendercontext import FLMStandaloneModeRenderContext



# needs to be outside of class definition for Transcrypt so that we can use this
# tuple to initialize both _attribute_fields as well as _fields
_flmfragment_attribute_fields = (
    'is_block_level',
    'resource_info',
    'standalone_mode',
    'silent',
    'what',
    'parsing_mode',
)


class FLMFragment:
    r"""
    A fragment of FLM-formatted code.

    Usually you should avoid manually creating `FLMFragment` instances.  Rather,
    use the environment objects's
    :py:meth:`flm.flmenvironment.FLMEnvironment.make_fragment()` method.

    A FLM fragment is intended to later be inserted in a document so that it can
    be rendered into the desired output format (HTML, plain text).  If the
    fragment is *standalone* (`standalone_mode=True`), then some FLM features
    are disabled (typically, for instance, cross-references) and the fragment
    can be rendered directly on its own without inserting it in a document, see
    :py:meth:`render_standalone()`.

    The `environment` argument should be a :py:class:`FLMEnvironment` instance
    used to parse this fragment.

    :param flm_text: The FLM source text to parse, or a pre-parsed
        :py:class:`~pylatexenc.latexnodes.nodes.LatexNodeList`.
    :param environment: The :py:class:`~flm.flmenvironment.FLMEnvironment`
        used to parse this fragment.
    :param is_block_level: Whether to parse as block-level (``True``),
        inline (``False``), or auto-detect (``None``, the default).
    :param standalone_mode: If ``True``, disables features that require a
        document context (e.g., cross-references).  Enables the use of
        :py:meth:`render_standalone`.
    :param what: A short description for error messages (e.g.,
        ``'abstract'``).

    The argument `resource_info` can be set to any custom object that can help
    locate resources called by FLM text.  For instance, a `\includegraphics{}`
    call might wish to look for graphics in the same filesystem folder as a file
    that contained the FLM code; the `resource_info` object can be used to store
    the filesystem folder of the FLM code forming this fragment.
    """

    def __init__(
            self,
            flm_text,
            environment,
            *,
            is_block_level=None,
            resource_info=None,
            standalone_mode=False,
            tolerant_parsing=False,
            what='(unknown)',
            silent=False,
            parsing_mode=None, # see FLMEnvironment.get_parsing_state(parsing_mode=)
            input_lineno_colno_offsets=None,
            _flm_text_if_loading_nodes=None,
    ):
        r"""
        :param flm_text: The FLM source text to parse, or a pre-parsed
            :py:class:`~pylatexenc.latexnodes.nodes.LatexNodeList`.
        :param environment: The :py:class:`~flm.flmenvironment.FLMEnvironment`
            used to parse this fragment.
        :param is_block_level: Whether to parse as block-level (``True``),
            inline (``False``), or auto-detect (``None``, the default).
        :param resource_info: An arbitrary object to help locate external
            resources referenced by the FLM text.
        :param standalone_mode: If ``True``, features requiring a document
            context are disabled and :py:meth:`render_standalone` can be used.
        :param tolerant_parsing: If ``True``, parsing errors are handled
            more leniently by the underlying pylatexenc walker.
        :param what: A short description for error messages (e.g.,
            ``'abstract'``).
        :param silent: If ``True``, suppress error logging on parse failure.
        :param parsing_mode: An optional string selecting a named parsing
            mode delta registered on the environment (see
            :py:meth:`~flm.flmenvironment.FLMEnvironment.make_parsing_state`).
        :param input_lineno_colno_offsets: A dictionary of line/column offset
            options forwarded to the latex walker for accurate position
            reporting.  Supported keys: ``'line_number_offset'``,
            ``'first_line_column_offset'``, ``'column_offset'``.
        """

        self.flm_text = flm_text
        self.environment = environment

        self.is_block_level = is_block_level
        self.resource_info = resource_info
        self.standalone_mode = standalone_mode
        self.tolerant_parsing = tolerant_parsing
        self.what = what
        self.silent = silent
        self.parsing_mode = parsing_mode

        if isinstance(flm_text, latexnodes_nodes.LatexNodeList):
            # We want to initialize a fragment with already-parsed node lists.
            # This is for internal use only!
            self.nodes = self.flm_text
            self.latex_walker = self.nodes.latex_walker
            if _flm_text_if_loading_nodes is not None:
                self.flm_text = _flm_text_if_loading_nodes
            else:
                self.flm_text = self.nodes.latex_verbatim()
            return

        try:
            self.latex_walker, self.nodes = \
                FLMFragment.parse(
                    self.flm_text,
                    self.environment,
                    standalone_mode=self.standalone_mode,
                    tolerant_parsing=self.tolerant_parsing,
                    is_block_level=self.is_block_level,
                    what=self.what,
                    resource_info=self.resource_info,
                    parsing_mode=self.parsing_mode,
                    input_lineno_colno_offsets=input_lineno_colno_offsets,
                )
        except latexnodes.LatexWalkerLocatedError as e:
            # tag on the information about resource_info and what
            e.flm_fragment_resource_info = self.resource_info
            e.flm_fragment_what = self.what
            # add an open context about parsing this fragment
            if not hasattr(e, 'open_contexts') or not e.open_contexts:
                e.open_contexts = []
            e.open_contexts.append(
                ('parsing '+str(self.what), None,
                 '',
                 None,
                 )
            )
            error_message = self.environment.get_located_error_message(e)
            if not self.silent:
                errfmt = latexnodes.LatexWalkerLocatedErrorFormatter(e)
                errmsg = errfmt.to_display_string()
                if error_message is not None:
                    errmsg = error_message + '\n' + errmsg
                logger.error(
                    f"Parse error in latex-like markup ‘{self.what}’: {errmsg}\n"
                    f"Given text was:\n‘{_abbrevtext(self.flm_text)}’\n\n"
                )
            if error_message is not None:
                e.msg = error_message
            raise
        except Exception as e:
            if not self.silent:
                logger.error(
                    f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                    f"Given text was:\n‘{_abbrevtext(self.flm_text)}’\n\n"
                )
            raise


    _attribute_fields = _flmfragment_attribute_fields

    _fields = tuple(['nodes'] + list(_flmfragment_attribute_fields))



    def _attributes(self, **kwargs):
        d = {
            k: getattr(self, k)
            for k in self._attribute_fields
        }
        d.update(kwargs)
        return d


    def render(self, render_context, **kwargs):
        r"""
        Render this fragment within a document render context.

        This method is typically called inside the render callback passed
        to :py:meth:`~flm.flmdocument.FLMDocument.render`.

        :param render_context: The
            :py:class:`~flm.flmrendercontext.FLMRenderContext` (typically a
            :py:class:`~flm.flmdocument.FLMDocumentRenderContext`).
        :returns: The rendered output string.
        """
        return render_context.fragment_renderer.render_fragment(
            self, render_context,
            **kwargs
        )

    def render_standalone(self, fragment_renderer):
        r"""
        Render this fragment in standalone mode (without a document context).

        The fragment must have been parsed with ``standalone_mode=True``.
        Features that require a document context (e.g., cross-references,
        footnotes) are not available in standalone mode.

        :param fragment_renderer: A
            :py:class:`~flm.fragmentrenderer.FragmentRenderer` instance.
        :returns: The rendered output string.
        :raises ValueError: If the fragment was not parsed in standalone
            mode.
        """
        if not self.standalone_mode:
            raise ValueError(
                "You can only use render_standalone() on a fragment that "
                "was parsed in standalone mode (use `standalone_mode=True` "
                "in the FLMFragment constructor)"
            )
        render_context = FLMStandaloneModeRenderContext(fragment_renderer=fragment_renderer)
        return self.render(render_context)

    @classmethod
    def parse(cls, flm_text, environment, *,
              standalone_mode=False, tolerant_parsing=None,
              is_block_level=None, parsing_mode=None,
              resource_info=None, what=None,
              input_lineno_colno_offsets=None,
              ):
        r"""
        Parse FLM source text into a walker and node list without creating
        an :py:class:`FLMFragment` instance.

        This low-level classmethod creates an
        :py:class:`~flm.flmenvironment.FLMLatexWalker` via the environment
        and parses the full input.  It is used internally by ``__init__``
        and can be called directly when only the raw parse result is needed.

        :param flm_text: The FLM source string.
        :param environment: The :py:class:`~flm.flmenvironment.FLMEnvironment`.
        :param standalone_mode: If ``True``, parse in standalone mode.
        :param tolerant_parsing: If ``True``, handle parse errors leniently.
        :param is_block_level: Block-level mode (``True``/``False``/``None``).
        :param parsing_mode: Optional named parsing mode string.
        :param resource_info: Resource locator object.
        :param what: Description for error messages.
        :param input_lineno_colno_offsets: Line/column offset dictionary.
        :returns: A tuple ``(latex_walker, nodes)`` where *latex_walker* is
            the :py:class:`~flm.flmenvironment.FLMLatexWalker` instance and
            *nodes* is the parsed
            :py:class:`~pylatexenc.latexnodes.nodes.LatexNodeList`.
        """

        logger.debug("Parsing FLM content %r", flm_text)

        latex_walker = environment.make_latex_walker(
            flm_text,
            is_block_level=is_block_level,
            parsing_mode=parsing_mode,
            resource_info=resource_info,
            standalone_mode=standalone_mode,
            tolerant_parsing=tolerant_parsing,
            what=what,
            input_lineno_colno_offsets=input_lineno_colno_offsets,
        )

        nodes, _ = latex_walker.parse_content(
            latexnodes_parsers.LatexGeneralNodesParser(),
        )

        return latex_walker, nodes


    def start_node_visitor(self, node_visitor):
        r"""
        Start a node visitor traversal on this fragment's parsed node tree.

        Calls ``node_visitor.start()`` with the fragment's root
        :py:class:`~pylatexenc.latexnodes.nodes.LatexNodeList`.

        :param node_visitor: A node visitor object implementing a
            ``start(nodes)`` method.
        """
        node_visitor.start(self.nodes)


    def is_empty(self):
        r"""
        Return ``True`` if the fragment's source text is empty or
        whitespace-only.

        :rtype: bool
        """
        return len(self.flm_text.strip()) == 0

    def __bool__(self):
        return not self.is_empty()

    def __repr__(self):
        theflmtext = self.flm_text
        if len(theflmtext) > 50:
            theflmtext = theflmtext[:49]+'…'
        return f"<{self.__class__.__name__} {repr(theflmtext)}>"


    def whitespace_stripped(self):
        r"""
        Return a new :py:class:`FLMFragment` with leading and trailing
        whitespace removed from the source text.
        """
        new_fragment = self.environment.make_fragment(
            self.flm_text.strip(),
            **self._attributes(what=f"{self.what}:whitespace-stripped")
        )
        return new_fragment

    def get_first_paragraph(self):
        r"""
        Returns a new :py:class:`FLMFragment` object that contains all material
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

        logger.debug("nodelists_paragraphs[0] = %r", nodelists_paragraphs[0])

        thenodes = nodelists_paragraphs[0]

        logger.debug("First paragraph -> %r", thenodes)
        return self.environment.make_fragment(
            flm_text=thenodes,
            **self._attributes(what=f"{self.what}:first-paragraph")
        )

    def truncate_to(self, chars, min_chars=None, truncation_marker=' …'):
        r"""
        Return a new :py:class:`FLMFragment` truncated to approximately
        *chars* characters.

        The truncation is performed at the node level, attempting to break
        at word boundaries.

        :param chars: Target maximum number of characters.
        :param min_chars: Minimum number of characters to include even if
            truncation would otherwise stop earlier.
        :param truncation_marker: String appended at the truncation point
            (default: ``' …'``).
        :returns: A new :py:class:`FLMFragment` with truncated content.
        """

        trunc = _NodeListTruncator(chars=chars, min_chars=min_chars,
                                   truncation_marker=truncation_marker)

        newnodes = trunc.truncate_node_list(self.nodes)

        return self.environment.make_fragment(
            flm_text=newnodes,
            **self._attributes(what=f"{self.what}:tr-{chars}")
        )



# ----------------------------




class _NodeListTruncator:
    def __init__(self, chars, min_chars=None, truncation_marker=None):
        super().__init__()
        self.chars = chars
        self.min_chars = min_chars
        self.truncation_marker = truncation_marker

        self.count = 0

    def truncate_node_list(self, nodes):
        self.count = 0
        newnodes = self.collect_nodes(nodes)
        if newnodes is None:
            return nodes # no truncation was necessary
        return newnodes

    def collect_nodes(self, nodes):
        for j, node in enumerate(nodes):
            newnode = self.collect_node(node)
            if newnode is not None:
                newnodes = nodes[:j]
                if newnode is not True: # True == "stop here but don't include this node"
                    newnodes.extend([newnode])
                return nodes.latex_walker.make_nodelist(
                    newnodes,
                    parsing_state=nodes.parsing_state,
                )
        # all ok
        return None

    def collect_node(self, node):
        if node.isNodeType(latexnodes_nodes.LatexGroupNode):
            return self.collect_nodes_groupnode(node)

        if node.isNodeType(latexnodes_nodes.LatexMacroNode):
            return self.collect_nodes_macronode(node)

        if node.isNodeType(latexnodes_nodes.LatexEnvironmentNode):
            return self.collect_nodes_environmentnode(node)

        if node.isNodeType(latexnodes_nodes.LatexSpecialsNode):
            return self.collect_nodes_specialsnode(node)

        return self.collect_nodes_simplenode(node)

    def collect_nodes_groupnode(self, node):
        groupnodelist = self.collect_nodes(node.nodelist)
        if groupnodelist is None:
            # everything collected ok, with room to spare
            return None
        # we had to stop at some point --> need new group node here.
        groupnode = node.latex_walker.make_node(
            latexnodes_nodes.LatexGroupNode,
            delimiters=node.delimiters,
            nodelist=groupnodelist,
            parsing_state=node.parsing_state,
            pos=node.pos,
            pos_end=node.pos_end
        )
        return groupnode


    def collect_node_argument(self, node):
        if isinstance(node, latexnodes_nodes.LatexNodeList):
            return self.collect_nodes(node)
        return self.collect_node(node)

    def collect_nodes_macronode(self, node):

        # let's try and recognize some known cases
        if hasattr(node.spec, '_flm_main_text_argument'):
            main_text_argname = node.spec._flm_main_text_argument
            # find which argument is the one that counts, by name
            arg_j = next(j for j, arg in enumerate(node.spec.arguments_spec_list)
                         if arg.argname == main_text_argname)
            # descend into the node's argument
            text_arg = node.nodeargd.argnlist[arg_j]
            text_arg_new = self.collect_node_argument(text_arg)
            if text_arg_new:
                new_argnlist = \
                    node.nodeargd.argnlist[:arg_j] + [text_arg_new] \
                    + node.nodeargd.argnlist[arg_j+1:]
                if text_arg_new is not None:
                    # new macro node with shortened argument
                    newmacronode = node.latex_walker.make_node(
                        latexnodes_nodes.LatexMacroNode,
                        macroname=node.macroname,
                        spec=node.spec,
                        nodeargd=latexnodes.ParsedArguments(
                            arguments_spec_list=node.nodeargd.arguments_spec_list,
                            argnlist=new_argnlist,
                        ),
                        macro_post_space=node.macro_post_space,
                        parsing_state=node.parsing_state,
                        pos=node.pos,
                        pos_end=node.pos_end
                    )
                    # we need to 'finalize' this node to regenerate all
                    # necessary meta-information (extra attributes etc.)
                    newmacronode = node.spec.finalize_node(newmacronode)
                    return newmacronode

        # all ok
        return None

    def collect_nodes_environmentnode(self, node):
        nodelist = self.collect_nodes(node.nodelist)
        if nodelist is None:
            # everything collected ok, with room to spare
            return None
        # we had to stop at some point --> need new group node here.
        newnode = node.latex_walker.make_node(
            latexnodes_nodes.LatexEnvironmentNode,
            environmentname=node.environmentname,
            nodeargd=node.nodeargd,
            nodelist=nodelist,
            parsing_state=node.parsing_state,
            pos=node.pos,
            pos_end=node.pos_end
        )
        newnode = node.spec.finalize_node(newnode)
        return newnode        

    def collect_nodes_specialsnode(self, node):
        # no idea how to deal with this in general---let's simply inspect the
        # entire source code for this specials including arguments
        my_length = len(node.latex_verbatim())
        if my_length < (self.chars - self.count):
            # enough room remaining -- keep going
            self.count += my_length
            return None

        return True # True == stop here, don't include this node

    def collect_nodes_simplenode(self, node):

        estimated_length = self.estimate_simple_node_char_count(node)

        if estimated_length < (self.chars - self.count):
            # enough room remaining -- keep going
            self.count += estimated_length
            return None
        
        # not enough room. Let's see if we can truncate this.
        if node.isNodeType(latexnodes_nodes.LatexCharsNode):
            # we can truncate the string?
            chars = node.chars
            last_break_pos = 0
            for j, c in enumerate(chars):
                if not c.isalpha():
                    last_break_pos = j
                if self.count + j > self.chars:
                    if self.min_chars is None \
                       or self.count + last_break_pos >= self.min_chars:
                        # stop here
                        break
                continue

            newchars = chars[:last_break_pos] + self.truncation_marker

            new_node = node.latex_walker.make_node(
                latexnodes_nodes.LatexCharsNode,
                chars=newchars,
                parsing_state=node.parsing_state,
                pos=node.pos,
                pos_end=node.pos_end,
            )
            return new_node

        # We can't split this node.  If we don't have enough chars left but
        # didn't make the minimum include this node and we're done.
        if self.min_chars is not None and self.count < self.min_chars:
            # include this node and stop here
            return node
            
        # stop here and don't include this node
        return True


    def estimate_simple_node_char_count(self, node):
        
        if node.isNodeType(latexnodes_nodes.LatexCharsNode):
            return len(node.chars)

        if node.isNodeType(latexnodes_nodes.LatexMathNode):
            # let's use a brutally terrible model that on average, we can
            # estimate the effective length of a math environment in effective
            # number of character widths as 2/3 of the number of chars in the
            # math's source
            return len(node.latex_verbatim()) * 2 // 3

        if node.isNodeType(latexnodes_nodes.LatexCommentNode):
            return 0

        return 0




def _abbrevtext(x, maxlen=100):
    x = str(x)
    return x[:maxlen] + ('…' if len(x) > maxlen else '')
