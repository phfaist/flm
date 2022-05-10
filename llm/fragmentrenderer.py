import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import nodes




class BlockLevelContent:
    r"""
    Used to mark content that is block-level, i.e., that is not part of a
    paragraph and that should not wrapped to be part of a paragraph.
    """
    def __init__(self, content):
        self.content = content

    def __str__(self):
        return self.content
    
    def __repr__(self):
        return f"{self.__class__.__name__}({self.content!r})"



class FragmentRenderer:
    r"""
    .................
    """

    supports_delayed_render_markers = False
    r"""
    If True, then the render methods generate special-syntax placeholders for
    content whose rendering must be delayed (e.g., references to figures,
    etc.). If False, then the whole content must be rendered in two passes.
    """


    def render_fragment(self, llm_fragment, doc, is_block_level=None):
        return str(
            self.render_nodelist(llm_fragment.nodes, doc, is_block_level=is_block_level)
        )

    def render_nodelist(self, nodelist, doc, is_block_level=None):
        r"""
        Render a nodelist, splitting the contents into paragraphs if applicable.

        If `is_block_level=False`, then paragraph breaks and other block-level
        content (e.g., enumeration lists or figures) are reported as syntax
        errors.
        
        If `is_block_level=None`, then we'll try to auto-detect if we are in
        block level or not.  If we encounter any paragraph breaks or block-level
        items (enumerations, etc.), then we'll render paragraphs, and otherwise,
        we'll render inline content.  Useful for rendering fragments where we
        don't know if the content is block-level or not.
        """

        rendered_block_items = []

        # used also in non-block-level mode for the list of items
        building_paragraph_rendered_items = []

        def flush_para():
            if not building_paragraph_rendered_items:
                return
            rendered_block_items.append(
                self.render_build_paragraph( building_paragraph_rendered_items )
            )
            building_paragraph_rendered_items.clear()

        for n in nodelist:
            if n.isNodeType(nodes.LatexSpecialsNode) and n.specials_chars == '\n\n':
                if is_block_level is None:
                    is_block_level = True # saw paragraph break -- autodetected block level to True
                # paragraph break
                if is_block_level:
                    flush_para()
                else:
                    posfmt = n.latex_walker.format_pos(n.pos)
                    raise ValueError(
                        f"You cannot use a paragraph break in inline text {posfmt}: "
                        f" ‘{nodelist.latex_verbatim}’"
                    )
                continue

            if (not building_paragraph_rendered_items
                and n.isNodeType(nodes.LatexCharsNode)
                and not n.chars.strip()):
                # only white space, and we haven't started a new paragraph yet -- ignore it.
                continue

            rendered = self.render_node(n, doc)
            if isinstance(rendered, BlockLevelContent):
                if is_block_level is None:
                    is_block_level = True # saw block-level content -- autodetected block level

                if not is_block_level:
                    posfmt = n.latex_walker.format_pos(n.pos)
                    raise ValueError(
                        f"Block-level content cannot be used in inline text {posfmt}: "
                        f" ‘{nodelist.latex_verbatim}’"
                    )
                # make sure this block-level item is not included in a paragraph.
                flush_para()
                rendered_block_items.append(rendered)
                continue

            # add to last paragraph being built (which is simply the list of
            # nodes if we're not in block-level mode)
            building_paragraph_rendered_items.append(rendered)

        if is_block_level:

            # finalize the last paragraph with any pending items
            flush_para()

            rendered = self.render_join_blocks(
                rendered_block_items
            )

        else:

            assert( len(rendered_block_items) == 0 )

            rendered = self.render_join( building_paragraph_rendered_items )

        logger.debug("render_nodelist: rendered -> %r", rendered)
        return rendered

    def render_node(self, node, doc):
        if node.isNodeType(nodes.LatexCharsNode):
            return self.render_node_chars(node, doc)
        if node.isNodeType(nodes.LatexCommentNode):
            return self.render_node_comment(node, doc)
        if node.isNodeType(nodes.LatexGroupNode):
            return self.render_node_group(node, doc)
        if node.isNodeType(nodes.LatexMacroNode):
            return self.render_node_macro(node, doc)
        if node.isNodeType(nodes.LatexEnvironmentNode):
            return self.render_node_environment(node, doc)
        if node.isNodeType(nodes.LatexSpecialsNode):
            return self.render_node_specials(node, doc)
        if node.isNodeType(nodes.LatexMathNode):
            return self.render_node_math(node, doc)

        raise ValueError(f"Invalid node type: {node!r}")
        

    def render_node_chars(self, node, doc):
        return self.render_value( node.chars )

    def render_node_comment(self, node, doc):
        return ''

    def render_node_group(self, node, doc):
        return self.render_nodelist( node.nodelist, doc )

    def render_node_macro(self, node, doc):
        return self.render_invocable_node(node, doc)

    def render_node_environment(self, node, doc):
        return self.render_invocable_node(node, doc)

    def render_node_specials(self, node, doc):
        return self.render_invocable_node(node, doc)

    def render_invocable_node(self, node, doc):
        if node.spec.llm_specinfo_string is not None:
            # simple pre-set string
            return self.render_value( node.spec.llm_specinfo_string )

        #
        # Rendering result will be obtained by calling render() on the
        # specinfo object
        #
        return self.render_invocable_node_call_render(node, node.spec.llm_specinfo, doc)



    def render_invocable_node_call_render(self, node, llm_specinfo, doc):

        if llm_specinfo is None:
            raise ValueError(f"Cannot render {node=!r} because specinfo is None!")

        if llm_specinfo.delayed_render:
            # requested a delayed rendering -- 

            is_first_pass = (self.supports_delayed_render_markers
                             or not doc.two_pass_mode_is_second_pass)
            delayed_key = None

            if is_first_pass:
                llm_specinfo.prepare_delayed_render(node, doc, self)
                delayed_key = doc.register_delayed_render(node, self)

            if self.supports_delayed_render_markers:
                # first pass, there's only one pass anyways; we're generating
                # the marker for the delayed content now -->
                return self.render_delayed_marker(node, delayed_key, doc)
            elif is_first_pass:
                # first pass of a two-pass scheme
                llm_specinfo.prepare_delayed_render(node, doc, self)
                # dummy placeholder, you'll never see it unless there's a bug:
                return self.render_delayed_dummy_placeholder(node, delayed_key, doc)
            else:
                # second pass of the two-pass scheme
                assert( doc.two_pass_mode_is_second_pass )
                # can return content that has been rendered by now
                return doc.get_delayed_render_content(node)


        # simply call render() to get the rendered value

        value = llm_specinfo.render(node, doc, self)
        return value


    def render_node_math(self, node, doc):
        return self.render_math_content( node.delimiters, node.nodelist, doc,
                                         node.displaytype, None )


    def render_math_content(self, delimiters, nodelist, doc, displaytype,
                            environmentname=None):
        # Use verbatim to render math in the base implementation. It will work
        # for our HTML implementation as well since we'll rely on MathJax.
        # Other implementations that don't want to render math in this type of
        # way will have to reimplement render_node_math().
        rendered = self.render_verbatim(
            delimiters[0] + nodelist.latex_verbatim() + delimiters[1],
            f'{displaytype}-math'
        )
        if displaytype == 'display':
            return BlockLevelContent(rendered)
        return rendered
    


    # ---


    def render_join(self, content_list):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Usually you'd want to simply join the strings together with
        no joiner, which is what the default implementation does.
        """
        return "".join(content_list)

    def render_build_paragraph(self, content_list):
        r"""
        Join the given content together into one paragraph.
        """
        return BlockLevelContent("".join(content_list).strip())

    # def render_join_as_paragraphs(self, paragraphs_content):
    #     r"""
    #     Render a sequence of paragraphs.  The argument `paragraphs_content` is a
    #     sequence (list) of the rendered contents of each paragraph.  This method
    #     must make sure they are treated as individual paragraphs (e.g., wrap the
    #     contents in ``<p>...</p>`` tags, or render the contents separated by
    #     ``\n\n`` or etc.).
    #     """
    #     return "\n\n".join(paragraphs_content)

    def render_join_blocks(self, content_list):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return BlockLevelContent("\n\n".join([str(s) for s in content_list]))

    # --

    def render_semantic_block(self, content, role, annotations=None):
        r"""
        Enclose the given content in a block (say, a DOM <section> or such) that is
        meant to convey semantic information about the document's structure, but
        with no necessary impact on the layout or appearance of the text itself.
        """
        return BlockLevelContent(str(content))


    # ---

    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        r"""
        Only used if the fragment renderer supports `markers' for items whose
        rendering is delayed.

        - `content` is the string result of the first pass rendering, which
          contains the markers generated on that pass

        - `delayed_values` is a dictionary of `delayed_key` mapping to the final
          rendered string values associated with the items with that key.
        """
        raise RuntimeError("Reimplement me!")




    # ---
    
    # helpers

    def get_arguments_nodelists(self, node, argnames, *, all=True, skip_nonexistent=False):
        args_nodelists = {}
        if node.nodeargd is None:
            # no arguments at all -- return all args empty
            return {k: nodes.LatexNodeList([])
                    for k in argnames}
        # find the correct argument number
        argnames_seen = set()
        for j, arg_spec in enumerate(node.nodeargd.arguments_spec_list):
            if arg_spec.argname not in argnames:
                if all:
                    raise ValueError(f"Got unexpected argument {arg_spec.argname} to {node}")
                continue
            argnode = node.nodeargd.argnlist[j]
            argnames_seen.add(arg_spec.argname)
            main_arg_node = None
            if argnode is None:
                argnodelist = nodes.LatexNodeList([None])
                main_arg_node = None
            elif argnode.isNodeType(nodes.LatexGroupNode):
                argnodelist = argnode.nodelist
                main_arg_node = argnode
            else:
                argnodelist = nodes.LatexNodeList([argnode])
                main_arg_node = argnode

            args_nodelists[arg_spec.argname] = _NodeArgInfo(
                nodelist=argnodelist,
                main_arg_node=main_arg_node,
                provided=(True if main_arg_node is not None else False),
            )

        if not skip_nonexistent:
            # if there's an argument in argnames that wasn't seen, that's an
            # error
            for argname in argnames:
                if argname not in argnames_seen:
                    raise ValueError(f"Missing argument ‘{argname}’ to {node}")
                
        return args_nodelists
        
    def get_nodelist_as_chars(self, nodelist):
        charslist = []
        if len(nodelist) == 1 and nodelist[0].isNodeType(nodes.LatexGroupNode):
            # allow enclosing group, e.g., to protect a square closing brace
            # char as in " \item[{]}] "
            nodelist = nodelist[0].nodelist
        for n in nodelist:
            if n is None:
                continue
            if not n.isNodeType(nodes.LatexCharsNode):
                raise ValueError(
                    f"Expected chars-only nodes, got "
                    f"‘{n.latex_verbatim()}<{n.__class__.__name__}>’ in "
                    f"‘{nodelist.latex_verbatim()}’"
                )
            charslist.append(n.chars)
        return "".join(charslist)


class _NodeArgInfo:
    def __init__(self, nodelist, main_arg_node, provided):
        super().__init__()
        self.nodelist = nodelist
        self.main_arg_node = main_arg_node
        self.provided = provided

    def __repr__(self):
        return (
            f"{self.__class__.__name__}(nodelist={self.nodelist!r}, "
            f"main_arg_node={self.main_arg_node!r})"
        )




class TextFragmentRenderer(FragmentRenderer):

    display_href_urls = True

    #supports_delayed_render_markers = False # -- inherited alreay

    def render_value(self, value):
        return value

    def render_delayed_marker(self, node, delayed_key, doc):
        return ''

    def render_delayed_dummy_placeholder(self, node, delayed_key, doc):
        return '#DELAYED#'

    def render_nothing(self, annotations=None):
        return ''

    def render_empty_error_placeholder(self, debug_str):
        return ''

    def render_text_format(self, text_formats, content):
        return content
    
    def render_enumeration(self, iter_items_content, counter_formatter, annotations=None):
        all_items = [
            (counter_formatter(1+j), item_content)
            for j, item_content in enumerate(iter_items_content)
        ]
        max_item_width = max([ len(fmtcnt) for fmtcnt, item_content in all_items ])
        return self.render_join_blocks([
            self.render_semantic_block(
                self.render_join([
                    self.render_value(fmtcnt.rjust(max_item_width+1, ' ')),
                    item_content,
                    "\n"
                ])
            )
            for fmtcnt, item_content in all_items
        ])

    def render_verbatim(self, value, annotations=None):
        return value

    def render_link(self, ref_type, href, display_content, annotations=None):
        r"""
        .....

        `href` can be:

        - a URL (external link)
        
        - an anchor fragment only (`#fragment-name`), for links within the
          document; note that we use #fragment-name universally, even if the
          output format is not HTML.  It's up to the output format's
          DocumentContext implementation to translate the linking scheme
          correctly.
        """
        if self.display_href_urls:
            return f"{display_content} <{href}>"
        return display_content

