
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import nodes

from ..flmrendercontext import FLMRenderContext


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

    def __init__(self, config=None):
        super().__init__()
        # use kwargs to set properties on the class object.
        if config is not None:
            for k,v in config.items():
                setattr(self, k, v)


    def document_render_start(self, render_context):
        pass

    def document_render_finish(self, render_context):
        pass


    def render_fragment(self, flm_fragment, render_context, is_block_level=None):
        try:
            return self.render_nodelist(flm_fragment.nodes,
                                        self.ensure_render_context(render_context),
                                        is_block_level=is_block_level)
        except Exception as e:
            logger.debug(f"Exception while rendering fragment ‘{flm_fragment.what}’: {e}")
            raise

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

        if nodelist is None:
            raise ValueError("render_nodelist(): nodelist should not be None")

        if not hasattr(nodelist, 'flm_is_block_level'):
            logger.debug("The given node list was not parsed & produced by FLM; "
                         "missing .flm_is_block_level attribute:\n"
                         f"{nodelist=}")
            raise ValueError("The given node list was not parsed & produced by FLM; "
                             "missing .flm_is_block_level attribute")

        if is_block_level is None:
            is_block_level = nodelist.flm_is_block_level

        if not is_block_level and nodelist.flm_is_block_level:
            raise ValueError(
                f"Cannot render node list ‘{nodelist!r}’ in inline mode (not block "
                f"level mode) as it contains block-level elements."
            )

        if is_block_level:

            # it could be that nodelist doesn't have an flm_blocks attribute;
            # e.g., if it's actually a node list without any block-level items
            # that was seen as inline content but which we're now forcing to be
            # rendered as a paragraph in block mode.
            if hasattr(nodelist, 'flm_blocks'):
                node_blocks = nodelist.flm_blocks
            else:
                node_blocks = [nodelist]

            return self.render_blocks(node_blocks, render_context)

        return self.render_inline_content(nodelist, render_context)


    def render_node(self, node, render_context):
        render_context = self.ensure_render_context(render_context)

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
        if hasattr(node, 'flm_chars_value'): # transcrypt doesn't like getattr with default arg
            chars_value = node.flm_chars_value
        else:
            chars_value = None
        if chars_value is None:
            # might happen if the chars is not specifically in a node list
            chars_value = node.chars
        return self.render_value( chars_value, render_context )

    def render_node_comment(self, node, render_context):
        return ''

    def render_node_group(self, node, render_context):
        return self.render_nodelist( node.nodelist,
                                     self.ensure_render_context(render_context) )

    def render_node_macro(self, node, render_context):
        return self.render_invocable_node(node,
                                          self.ensure_render_context(render_context))

    def render_node_environment(self, node, render_context):
        return self.render_invocable_node(node,
                                          self.ensure_render_context(render_context))

    def render_node_specials(self, node, render_context):
        return self.render_invocable_node(node,
                                          self.ensure_render_context(render_context))

    def render_invocable_node(self, node, render_context):
        #
        # Rendering result will be obtained by calling render() on the
        # specinfo object
        #

        if not hasattr(node, 'flm_specinfo') or node.flm_specinfo is None:
            raise RuntimeError(f"Node {node} does not have the `flm_specinfo` attribute set")

        if render_context.is_standalone_mode:
            if not node.flm_specinfo.allowed_in_standalone_mode:
                raise ValueError(
                    f"Cannot render ‘{node.latex_verbatim()}’ in standalone mode."
                )

        return self.render_invocable_node_call_render(
            node,
            node.flm_specinfo,
            self.ensure_render_context(render_context)
        )



    def render_invocable_node_call_render(self, node, flm_specinfo, render_context):

        # render_context is not None because of internal call
        assert( render_context is not None )

        if flm_specinfo is None:
            raise ValueError(f"Cannot render {node=!r} because specinfo is None!")

        is_delayed_render = render_context.get_is_delayed_render(node)
        if is_delayed_render:
            # requested a delayed rendering -- 

            is_first_pass = render_context.is_first_pass
            delayed_key = None

            if is_first_pass:
                flm_specinfo.prepare_delayed_render(node, render_context)
                delayed_key = render_context.register_delayed_render(node, self)

            if self.supports_delayed_render_markers:
                # first pass, there's only one pass anyways; we're generating
                # the marker for the delayed content now -->
                return self.render_delayed_marker(node, delayed_key, render_context)
            elif is_first_pass:
                # first pass of a two-pass scheme
                flm_specinfo.prepare_delayed_render(node, render_context)
                # dummy placeholder, you'll never see it unless there's a bug:
                return self.render_delayed_dummy_placeholder(node, delayed_key, render_context)
            else:
                # second pass of the two-pass scheme
                assert( not render_context.is_first_pass )
                # can return content that has been rendered by now
                return render_context.get_delayed_render_content(node)

        # simply call render() to get the rendered value

        value = flm_specinfo.render(node, render_context)
        return value

    def render_node_math(self, node, render_context):
        return self.render_math_content(
            node.delimiters,
            node.nodelist,
            self.ensure_render_context(render_context),
            displaytype=node.displaytype,
            target_id=None
        )

    def render_math_content(self, delimiters, nodelist, render_context, displaytype, *,
                            environmentname=None, target_id=None):

        # Use verbatim to render math in the base implementation. It will work
        # for our HTML implementation as well since we'll rely on MathJax.
        # Other implementations that don't want to render math in this type of
        # way will have to reimplement render_node_math().
        rendered = self.render_verbatim(
            delimiters[0] + nodelist.latex_verbatim() + delimiters[1],
            render_context=render_context,
            annotations=[f'{displaytype}-math'],
            target_id=target_id,
            is_block_level=(displaytype == 'display')
        )
        return rendered
    


    # ---


    def render_blocks(self, node_blocks, render_context):

        rendered_blocks = []

        for block in node_blocks:

            if isinstance(block, nodes.LatexNodeList):
                para = self.render_build_paragraph(block, render_context)
            else:
                para = self.render_node(block, render_context)

            rendered_blocks.append( para )

        return self.render_join_blocks( rendered_blocks, render_context )


    def render_build_paragraph(self, nodelist, render_context):
        r"""
        Render and join the given content together into one paragraph.
        """
        return self.render_inline_content(nodelist, render_context)

    def render_inline_content(self, nodelist, render_context):
        return self.render_join([ self.render_node(n, render_context)
                                  for n in nodelist ], render_context)


    def render_join(self, content_list, render_context):
        r"""
        For inline content; content_list contains already rendered items.
        """
        return "".join(content_list)


    def render_join_blocks(self, content_list, render_context):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return "\n\n".join([c for c in content_list if c is not None and len(c)])


    # ---


    def render_semantic_span(self, content, role, render_context, *,
                             annotations=None, target_id=None):
        r"""
        Possibly mark the given inline text content as belonging to a
        single construct (e.g., a sequence of citations or endnotes).  This
        might correspond to a `<span>` tag in HTML.
        """
        return content

    def render_semantic_block(self, content, role, render_context, *,
                              annotations=None, target_id=None):
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

    def render_value(self, value, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_delayed_marker(self, node, delayed_key, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_nothing(self, render_context, annotations=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_empty_error_placeholder(self, debug_str, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_text_format(self, text_formats, nodelist, render_context):
        raise RuntimeError("Subclasses need to reimplement this method")
    
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, inline_heading=False, target_id=None):
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
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


    # extras:


    def render_float(self, float_instance, render_context):
        raise RuntimeError("Feature is not implemented by subclass")

    def render_graphics_block(self, graphics_resource, render_context):
        raise RuntimeError("Feature is not implemented by subclass")

    def render_cells(self, cells_model, render_context, target_id=None):
        raise RuntimeError("Feature is not implemented by subclass")


    # ---
    
    # helpers

    def ensure_render_context(self, render_context):
        return render_context or FLMRenderContext(fragment_renderer=self)


