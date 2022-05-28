
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import nodes



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


    def render_fragment(self, llm_fragment, render_context, is_block_level=None):
        return self.render_nodelist(llm_fragment.nodes,
                                    self._ensure_render_context(render_context),
                                    is_block_level=is_block_level)

    def render_nodelist(self, nodelist, render_context, is_block_level=None):
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

        if is_block_level is None:
            is_block_level = nodelist.llm_is_block_level

        if not is_block_level and nodelist.llm_is_block_level:
            raise ValueError(
                f"Cannot render node list ‘{nodelist!r}’ in inline mode (not block "
                f"level mode) as it contains block-level elements."
            )

        if is_block_level:

            # it could be that nodelist doesn't have an llm_blocks attribute;
            # e.g., if it's actually a node list without any block-level items
            # that was seen as inline content but which we're now forcing to be
            # rendered as a paragraph in block mode.
            node_blocks = getattr(nodelist, 'llm_blocks', [nodelist])

            return self.render_blocks(node_blocks, render_context)

        return self.render_inline_content(nodelist, render_context)


    def render_node(self, node, render_context):
        render_context = self._ensure_render_context(render_context)
        if node.isNodeType(nodes.LatexCharsNode):
            return self.render_node_chars(node, render_context)
        if node.isNodeType(nodes.LatexCommentNode):
            return self.render_node_comment(node, render_context)
        if node.isNodeType(nodes.LatexGroupNode):
            return self.render_node_group(node, render_context)
        if node.isNodeType(nodes.LatexMacroNode):
            return self.render_node_macro(node, render_context)
        if node.isNodeType(nodes.LatexEnvironmentNode):
            return self.render_node_environment(node, render_context)
        if node.isNodeType(nodes.LatexSpecialsNode):
            return self.render_node_specials(node, render_context)
        if node.isNodeType(nodes.LatexMathNode):
            return self.render_node_math(node, render_context)

        raise ValueError(f"Invalid node type: {node!r}")
        

    def render_node_chars(self, node, render_context):
        chars_value = getattr(node, 'llm_chars_value', None)
        if chars_value is None:
            # might happen if the chars is not specifically in a node list
            chars_value = node.chars
        return self.render_value( chars_value )

    def render_node_comment(self, node, render_context):
        return ''

    def render_node_group(self, node, render_context):
        return self.render_nodelist( node.nodelist,
                                     self._ensure_render_context(render_context) )

    def render_node_macro(self, node, render_context):
        return self.render_invocable_node(node,
                                          self._ensure_render_context(render_context))

    def render_node_environment(self, node, render_context):
        return self.render_invocable_node(node,
                                          self._ensure_render_context(render_context))

    def render_node_specials(self, node, render_context):
        return self.render_invocable_node(node,
                                          self._ensure_render_context(render_context))

    def render_invocable_node(self, node, render_context):
        if node.spec.llm_specinfo_string is not None:
            # simple pre-set string
            return self.render_value( node.spec.llm_specinfo_string )

        #
        # Rendering result will be obtained by calling render() on the
        # specinfo object
        #
        return self.render_invocable_node_call_render(
            node,
            node.spec.llm_specinfo,
            self._ensure_render_context(render_context)
        )



    def render_invocable_node_call_render(self, node, llm_specinfo, render_context):

        # render_context is not None because of internal call
        assert( render_context is not None )

        if llm_specinfo is None:
            raise ValueError(f"Cannot render {node=!r} because specinfo is None!")

        if llm_specinfo.delayed_render:
            # requested a delayed rendering -- 

            is_first_pass = (self.supports_delayed_render_markers
                             or not render_context.two_pass_mode_is_second_pass)
            delayed_key = None

            if is_first_pass:
                llm_specinfo.prepare_delayed_render(node, render_context)
                delayed_key = render_context.register_delayed_render(node, self)

            if self.supports_delayed_render_markers:
                # first pass, there's only one pass anyways; we're generating
                # the marker for the delayed content now -->
                return self.render_delayed_marker(node, delayed_key, render_context)
            elif is_first_pass:
                # first pass of a two-pass scheme
                llm_specinfo.prepare_delayed_render(node, render_context)
                # dummy placeholder, you'll never see it unless there's a bug:
                return self.render_delayed_dummy_placeholder(node, delayed_key, render_context)
            else:
                # second pass of the two-pass scheme
                assert( render_context.two_pass_mode_is_second_pass )
                # can return content that has been rendered by now
                return render_context.get_delayed_render_content(node)


        # simply call render() to get the rendered value

        value = llm_specinfo.render(node, render_context)
        return value


    def render_node_math(self, node, render_context):
        return self.render_math_content(
            node.delimiters,
            node.nodelist,
            self._ensure_render_context(render_context),
            node.displaytype,
            None
        )


    def render_math_content(self, delimiters, nodelist, render_context, displaytype,
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
            return rendered
        return rendered
    


    # ---


    def render_blocks(self, node_blocks, render_context):

        rendered_blocks = []

        for block in node_blocks:
            if isinstance(block, nodes.LatexNodeList):
                rendered_blocks.append( self.render_build_paragraph(block, render_context) )
            else:
                rendered_blocks.append( self.render_node(block, render_context) )

        return self.render_join_blocks( rendered_blocks )


    def render_build_paragraph(self, nodelist, render_context):
        r"""
        Render and join the given content together into one paragraph.
        """
        return self.render_inline_content(nodelist, render_context)

    def render_inline_content(self, nodelist, render_context):
        return self.render_join([ self.render_node(n, render_context)
                                  for n in nodelist ])


    def render_join(self, content_list):
        r"""
        For inline content; content_list contains already rendered items.
        """
        return "".join(content_list)


    def render_join_blocks(self, content_list):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return "\n\n".join(content_list)


    # ---


    def render_semantic_block(self, content, role, *, annotations=None, target_id=None):
        r"""
        Enclose the given content in a block (say, a DOM <section> or such) that is
        meant to convey semantic information about the document's structure, but
        with no necessary impact on the layout or appearance of the text itself.
        """
        return content


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


    # --- to be reimplemented ---

    def render_value(self, value):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_delayed_marker(self, node, delayed_key, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_nothing(self, annotations=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_empty_error_placeholder(self, debug_str):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_text_format(self, text_formats, nodelist, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")
    
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_verbatim(self, value, annotations=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):
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
        raise RuntimeError("Subclasses need to reimplement this method")


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

    # --

    def _ensure_render_context(self, render_context):
        return render_context or _OnlyFragmentRendererRenderContext(self)





class _OnlyFragmentRendererRenderContext:
    def __init__(self, fragment_renderer):
        self.doc = None
        self.fragment_renderer = fragment_renderer

    def supports_feature(self, feature_name):
        return False

    def feature_render_manager(self, feature_name):
        return None


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

