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

    A FLM fragment is intended to later be inserted in a document so that it can
    be rendered into the desired output format (HTML, plain text).  If the
    fragment is *standalone* (`standalone_mode=True`), then some FLM features
    are disabled (typically, for instance, cross-references) and the fragment
    can be rendered directly on its own without inserting it in a document, see
    :py:meth:`render_standalone()`.

    Doc .....................

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
            if _flm_text_if_loading_nodes:
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
            if not self.silent:
                error_message = self.environment.get_located_error_message(e)
                logger.error(
                    f"Parse error in latex-like markup ‘{self.what}’: {error_message}\n"
                    f"Given text was:\n‘{self.flm_text}’\n\n"
                )
            raise
        except Exception as e:
            if not self.silent:
                logger.error(f"Error parsing latex-like markup ‘{self.what}’: {e}\n"
                             f"Given text was:\n‘{self.flm_text}’\n\n")
            raise


    _attribute_fields = _flmfragment_attribute_fields

    _fields = tuple(['nodes'] + list(_flmfragment_attribute_fields))



    def _attributes(self, **kwargs):
        d = {
            k: getattr(self, k)
            for k in self._attribute_fields
        }
        # dict(
        #     is_block_level=self.is_block_level,
        #     resource_info=self.resource_info,
        #     standalone_mode=self.standalone_mode,
        #     silent=self.silent,
        #     what=self.what,
        #     parsing_mode=self.parsing_mode,
        # )
        d.update(kwargs)
        return d


    def render(self, render_context, **kwargs):
        return render_context.fragment_renderer.render_fragment(
            self, render_context,
            **kwargs
        )

    def render_standalone(self, fragment_renderer):
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
        node_visitor.start(self.nodes)


    def is_empty(self):
        return len(self.flm_text.strip()) == 0

    def __bool__(self):
        return not self.is_empty()

    def __repr__(self):
        theflmtext = self.flm_text
        if len(theflmtext) > 50:
            theflmtext = theflmtext[:49]+'…'
        return f"<{self.__class__.__name__} {theflmtext!r}>"


    def whitespace_stripped(self):
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
        if self.count < self.min_chars:
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
