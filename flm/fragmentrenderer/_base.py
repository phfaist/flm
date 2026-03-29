r"""
Base class for FLM fragment renderers.

A fragment renderer is responsible for producing the final output string in
a specific format (HTML, plain text, LaTeX, Markdown).  Each output format
has its own subclass of :py:class:`FragmentRenderer`.

The built-in renderers are:

- :py:class:`~flm.fragmentrenderer.html.HtmlFragmentRenderer`
- :py:class:`~flm.fragmentrenderer.text.TextFragmentRenderer`
- :py:class:`~flm.fragmentrenderer.latex.LatexFragmentRenderer`
- :py:class:`~flm.fragmentrenderer.markdown.MarkdownFragmentRenderer`

To implement a custom output format, subclass :py:class:`FragmentRenderer`
and implement the abstract ``render_*`` methods.
"""

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    LatexWalkerLocatedError
)
from pylatexenc.latexnodes import nodes

from ..flmrendercontext import FLMRenderContext
from ..flmrecomposer import FLMNodesFlmRecomposer



class FragmentRenderer:
    r"""
    Base class for defining how to render FLM content in a given output format.

    Subclasses must implement the abstract ``render_*`` methods such as
    :py:meth:`render_value`, :py:meth:`render_text_format`,
    :py:meth:`render_heading`, :py:meth:`render_enumeration`,
    :py:meth:`render_verbatim`, :py:meth:`render_link`, etc.

    The base class provides the rendering pipeline: it traverses the node
    tree, handles block/paragraph decomposition, manages delayed rendering,
    and dispatches to the appropriate ``render_*`` methods.

    :param config: Optional dictionary of configuration options.  Keys are
        set as attributes on the renderer instance (e.g.,
        ``use_link_target_blank`` for HTML).
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
        r"""Called by :py:meth:`FLMDocument.render()
        <flm.flmdocument.FLMDocument.render>` at the start of the rendering
        pipeline, before the render callback is invoked.  Override to
        initialise renderer-specific state in *render_context*.

        :param render_context: The document render context for this render
            pass.
        """
        pass

    def document_render_finish(self, render_context):
        r"""Called by :py:meth:`FLMDocument.render()
        <flm.flmdocument.FLMDocument.render>` after the rendering pipeline
        is complete (including delayed-render resolution).  Override to
        tear down renderer-specific state.

        :param render_context: The document render context for this render
            pass.
        """
        pass


    def render_fragment(self, flm_fragment, render_context, is_block_level=None):
        r"""
        Render an :py:class:`~flm.flmfragment.FLMFragment` into the output
        format.

        :param flm_fragment: The fragment to render.
        :param render_context: The current render context.
        :param is_block_level: Override the fragment's block-level setting.
        :returns: The rendered output string.
        """
        try:
            return self.render_nodelist(flm_fragment.nodes,
                                        self.ensure_render_context(render_context),
                                        is_block_level=is_block_level)
        except LatexWalkerLocatedError as e:
            logger.debug(
                f"Error in rendering fragment ‘{flm_fragment.what}’: {e}",
                exc_info=True
            )
            # add open LaTeX context for this fragment (show document file name)
            e.set_pos_or_add_open_context_from_node(
                flm_fragment.nodes, what=flm_fragment.what
            )
            raise e
        except Exception as e:
            logger.debug(
                f"Exception while rendering fragment ‘{flm_fragment.what}’: {e}",
                exc_info=True
            )
            raise

    def render_nodelist(self, nodelist, render_context, is_block_level=None):
        r"""Render a node list, handling block-level decomposition.

        If the node list contains block-level content (paragraphs, lists,
        figures), it is split into blocks and rendered via
        :py:meth:`render_blocks`.  Otherwise it is rendered as inline
        content via :py:meth:`render_inline_content`.

        :param nodelist: A :py:class:`~pylatexenc.latexnodes.nodes.LatexNodeList`
            with ``flm_is_block_level`` and (if block-level) ``flm_blocks``
            attributes set by the FLM finalizer.
        :param render_context: The current render context.
        :param is_block_level: Override block-level detection.  If ``None``
            (the default), the node list's own ``flm_is_block_level`` flag
            is used.  If ``False`` and the node list contains block-level
            content, a :py:exc:`ValueError` is raised.
        :returns: The rendered output string.
        """

        if nodelist is None:
            raise ValueError("render_nodelist(): nodelist should not be None")

        if not hasattr(nodelist, 'flm_is_block_level'):
            logger.debug("The given node list was not parsed & produced by FLM; "
                         "missing .flm_is_block_level attribute:\n"
                         f"{nodelist=}")
            raise LatexWalkerLocatedError(
                f"The given node list was not parsed & produced by FLM; "
                f"missing .flm_is_block_level attribute: {repr(nodelist)[0:100]}",
                pos=(
                    #getattr(nodelist, 'pos', None)  # Transcrypt doesn't support 3-arg getattr
                    getattr(nodelist, 'pos') if hasattr(nodelist, 'pos') else None
                ),
            )

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
        r"""Render a single parsed node by dispatching to the appropriate
        ``render_node_*`` method based on the node type (chars, group,
        macro, environment, specials, math, or comment).  If the node has
        an ``flm_replace_by_node`` attribute, that replacement node is
        rendered instead.

        :param node: A pylatexenc node.
        :param render_context: The current render context.
        :returns: The rendered output string for this node.
        :raises ValueError: If the node type is not recognised.
        """
        render_context = self.ensure_render_context(render_context)

        try:

            if hasattr(node, 'flm_replace_by_node') and node.flm_replace_by_node is not None:
                # implement a form of pre-processing at render time.  Useful for
                # custom macros, etc.
                return self.render_node(node.flm_replace_by_node, render_context)

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

        except LatexWalkerLocatedError as e:
            # add open LaTeX context!
            e.set_pos_or_add_open_context_from_node(node)
            raise e

        except Exception as e:
            err = LatexWalkerLocatedError(str(e))
            err.set_pos_or_add_open_context_from_node(node)
            raise err
        

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

        begin_delim, end_delim = delimiters
        if environmentname:
            begin_delim = f"\\begin{'{'}{environmentname}{'}'}"
            end_delim = f"\\end{'{'}{environmentname}{'}'}"

        rendered = self.render_verbatim(
            begin_delim + self.recompose_latex(nodelist) + end_delim,
            render_context=render_context,
            annotations=[f'{displaytype}-math'],
            target_id=target_id,
            is_block_level=(displaytype == 'display')
        )
        return rendered
    
    def recompose_latex(self, node):
        r"""Recompose a node or nodelist back into its FLM/LaTeX source text.

        Used internally when rendering math content verbatim.

        :param node: A pylatexenc node or nodelist.
        :returns: The reconstructed FLM source string.
        """
        flm = FLMNodesFlmRecomposer().recompose_flm_text(node)
        logger.debug("recomposed flm ‘%s’ for node: %r", flm, node)
        return flm


    # ---


    def render_blocks(self, node_blocks, render_context):
        r"""Render a list of block-level items (paragraphs and standalone
        block nodes) and join them together.

        Each block is either a :py:class:`~pylatexenc.latexnodes.nodes.LatexNodeList`
        (rendered as a paragraph via :py:meth:`render_build_paragraph`) or a
        single block-level node (rendered via :py:meth:`render_node`).
        The results are joined with :py:meth:`render_join_blocks`.

        :param node_blocks: A list of node lists or single block-level nodes.
        :param render_context: The current render context.
        :returns: The rendered output string with all blocks joined.
        """

        rendered_blocks = []

        for block in node_blocks:

            if isinstance(block, nodes.LatexNodeList):
                para = self.render_build_paragraph(block, render_context)
            else:
                para = self.render_node(block, render_context)

            rendered_blocks.append( para )

        return self.render_join_blocks( rendered_blocks, render_context )


    def render_build_paragraph(self, nodelist, render_context):
        r"""Render a node list as a single paragraph.

        By default this delegates to :py:meth:`render_inline_content`.
        Subclasses may override to wrap the result in paragraph markup
        (e.g., ``<p>...</p>`` in HTML).

        :param nodelist: The node list forming one paragraph.
        :param render_context: The current render context.
        :returns: The rendered paragraph string.
        """
        return self.render_inline_content(nodelist, render_context)

    def render_inline_content(self, nodelist, render_context):
        r"""Render all nodes in *nodelist* as inline content and join them.

        Each node is rendered individually via :py:meth:`render_node`, then
        the results are concatenated with :py:meth:`render_join`.

        :param nodelist: The node list to render inline.
        :param render_context: The current render context.
        :returns: The rendered inline content string.
        """
        return self.render_join([ self.render_node(n, render_context)
                                  for n in nodelist ], render_context)


    def render_join(self, content_list, render_context):
        r"""Join a list of already-rendered inline content strings.

        The default implementation concatenates the strings with no
        separator.

        :param content_list: List of rendered content strings.
        :param render_context: The current render context.
        :returns: The concatenated result.
        """
        return "".join(content_list)


    def render_join_blocks(self, content_list, render_context):
        r"""Join a list of already-rendered block-level content strings.

        Each element is a paragraph or standalone block.  The default
        implementation joins non-empty items with a double newline.

        :param content_list: List of rendered block strings (may contain
            ``None`` or empty strings, which are skipped).
        :param render_context: The current render context.
        :returns: The joined result.
        """
        return "\n\n".join([c for c in content_list if c is not None and len(c)])


    # ---


    def render_semantic_span(self, content, role, render_context, *,
                             annotations=None, target_id=None):
        r"""Wrap inline content in a semantic span.

        Marks the given already-rendered inline text as belonging to a
        single construct (e.g., a sequence of citations or endnotes).
        In HTML, this corresponds to a ``<span>`` tag.

        The default implementation returns *content* unchanged.

        :param content: The already-rendered inline content string.
        :param role: A string identifying the semantic role (e.g.,
            ``'citations'``, ``'footnotes'``).
        :param render_context: The current render context.
        :param annotations: Optional list of additional annotation strings.
        :param target_id: Optional anchor ID for the element.
        :returns: The wrapped content string.
        """
        return content

    def render_semantic_block(self, content, role, render_context, *,
                              annotations=None, target_id=None):
        r"""Wrap block content in a semantic container.

        Encloses the given already-rendered block in a semantic element
        (e.g., an HTML ``<section>`` or ``<div>``) that conveys document
        structure without necessarily affecting visual layout.

        The default implementation returns *content* unchanged.

        :param content: The already-rendered block content string.
        :param role: A string identifying the semantic role (e.g.,
            ``'section'``, ``'enumeration'``).
        :param render_context: The current render context.
        :param annotations: Optional list of additional annotation strings.
        :param target_id: Optional anchor ID for the element.
        :returns: The wrapped content string.
        """
        return content

    # ---


    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        r"""Replace delayed-render markers in the output with final values.

        Only used when :py:attr:`supports_delayed_render_markers` is
        ``True``.  After the first rendering pass, this method substitutes
        the placeholder markers (produced by :py:meth:`render_delayed_marker`)
        with the actual rendered content.

        :param content: The first-pass output string containing markers.
        :param delayed_values: A dictionary mapping delayed keys to their
            final rendered content strings.
        :returns: The output string with all markers replaced.
        """
        raise RuntimeError("Reimplement me!")


    # --- to be reimplemented ---

    def render_value(self, value, render_context):
        r"""Render a plain text value, applying any format-specific escaping.

        For example, the HTML renderer escapes ``<``, ``>``, and ``&``
        characters.  The plain text renderer returns *value* unchanged.

        :param value: The raw text string to render.
        :param render_context: The current render context.
        :returns: The escaped/formatted output string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_delayed_marker(self, node, delayed_key, render_context):
        r"""Return a placeholder marker for a delayed-render node.

        Used when :py:attr:`supports_delayed_render_markers` is ``True``.
        The marker is later replaced with the real content by
        :py:meth:`replace_delayed_markers_with_final_values`.

        :param node: The delayed-render node.
        :param delayed_key: The unique key identifying this delayed node.
        :param render_context: The current render context.
        :returns: A marker string that will be substituted later.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        r"""Return a disposable placeholder for a delayed-render node.

        Used on the first pass of a two-pass rendering scheme (when
        :py:attr:`supports_delayed_render_markers` is ``False``).  The
        returned value is discarded after the first pass and never appears
        in the final output.

        :param node: The delayed-render node.
        :param delayed_key: The unique key identifying this delayed node.
        :param render_context: The current render context.
        :returns: A dummy placeholder string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_nothing(self, render_context, annotations=None):
        r"""Render an empty or invisible element.

        May produce an empty string, an HTML comment, or another
        format-appropriate representation of "nothing".

        :param render_context: The current render context.
        :param annotations: Optional list of annotation strings that
            describe the empty element (e.g., for debugging).
        :returns: A string representing empty content in the output format.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_empty_error_placeholder(self, debug_str, render_context):
        r"""Render a placeholder indicating a rendering error.

        Used when a construct could not be rendered properly.  The output
        should be minimal and may include *debug_str* for diagnostics.

        :param debug_str: A short description of the error for debugging.
        :param render_context: The current render context.
        :returns: A placeholder string in the output format.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_text_format(self, text_formats, nodelist, render_context):
        r"""Render text with formatting such as bold, italic, or code.

        :param text_formats: A list of format name strings (e.g.,
            ``['textbf']``, ``['textit']``, ``['defterm-term']``).
        :param nodelist: The node list containing the formatted content.
        :param render_context: The current render context.
        :returns: The rendered and formatted output string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")
    
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None):
        r"""Render an enumeration (ordered or unordered list).

        :param iter_items_nodelists: An iterable of node lists, one per
            list item.
        :param counter_formatter: A callable ``counter_formatter(n)``
            returning the formatted tag for item number *n* (1-based).
            Returns a string or a node list, or ``None`` for bullet lists.
        :param render_context: The current render context.
        :param target_id_generator: Optional callable
            ``target_id_generator(n)`` returning a target anchor ID for
            item *n*, or ``None``.
        :param annotations: Optional list of annotation strings.
        :returns: The rendered enumeration as a block-level string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_lines(self, lines_info_list, render_context,
                     *, role=None, annotations=None, target_id=None):
        r"""Render a sequence of lines (e.g., from a ``lines`` environment).

        :param lines_info_list: A list of line info objects, each having
            at least a ``nodelist`` attribute and optional ``indent_left``,
            ``indent_right``, and ``align`` attributes.
        :param render_context: The current render context.
        :param role: Optional semantic role string (e.g., ``'lines'``).
        :param annotations: Optional list of annotation strings.
        :param target_id: Optional anchor ID for the block.
        :returns: The rendered lines as a string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1,
                       #heading_formatted_number=None,
                       inline_heading=False,
                       target_id=None,
                       annotations=None):
        r"""Render a section heading.

        :param heading_nodelist: The node list containing the heading text.
        :param render_context: The current render context.
        :param heading_level: Integer level (1--6) or the string
            ``'theorem'`` for theorem-style headings.
        :param inline_heading: If ``True``, the heading is a run-in
            heading (e.g., ``\paragraph``) that does not start a new block.
        :param target_id: Optional anchor ID for the heading.
        :param annotations: Optional list of annotation strings.
        :returns: The rendered heading string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
        r"""Render verbatim (pre-formatted) text, such as code or math source.

        :param value: The raw verbatim text string.
        :param render_context: The current render context.
        :param is_block_level: ``True`` if the verbatim block should be
            rendered as a block-level element.
        :param annotations: Optional list of annotation strings (e.g.,
            ``['verbatimcode']``, ``['inline-math']``).
        :param target_id: Optional anchor ID for the element.
        :returns: The rendered verbatim content string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):
        r"""Render a hyperlink.

        :param ref_type: The type of reference (e.g., ``'href'``, ``'url'``,
            ``'ref'``).
        :param href: The link target.  Can be an external URL or an
            anchor fragment (``'#fragment-name'``) for internal links.
        :param display_nodelist: The node list for the displayed link text.
        :param render_context: The current render context.
        :param annotations: Optional list of annotation strings.
        :returns: The rendered hyperlink string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_annotation_comment(
            self,
            display_nodelist,
            render_context,
            is_block_level=False,
            color_index=0,
            initials=None,
    ):
        r"""Render an annotation comment (e.g., from ``\annot``).

        :param display_nodelist: The node list for the comment text.
        :param render_context: The current render context.
        :param is_block_level: Whether to render as a block element.
        :param color_index: Integer selecting a color for the annotation.
        :param initials: Optional string with the annotator's initials.
        :returns: The rendered annotation comment string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")

    def render_annotation_highlight(
            self,
            display_nodelist,
            render_context,
            is_block_level=False,
            color_index=0,
            initials=None
    ):
        r"""Render an annotation highlight (e.g., from ``\annot``).

        :param display_nodelist: The node list for the highlighted text.
        :param render_context: The current render context.
        :param is_block_level: Whether to render as a block element.
        :param color_index: Integer selecting a color for the highlight.
        :param initials: Optional string with the annotator's initials.
        :returns: The rendered annotation highlight string.
        """
        raise RuntimeError("Subclasses need to reimplement this method")


    # extras:


    def render_float(self, float_instance, render_context):
        r"""Render a float (figure, table, or other captioned block).

        :param float_instance: A float instance object providing
            ``float_type_info``, ``content_nodelist``, ``caption_nodelist``,
            ``counter_value``, and ``formatted_counter_value_flm``.
        :param render_context: The current render context.
        :returns: The rendered float as a block-level string.
        """
        raise RuntimeError("Feature is not implemented by subclass")

    def render_graphics_block(self, graphics_resource, render_context):
        r"""Render a graphics/image block.

        :param graphics_resource: A graphics resource object with a
            ``src_url`` attribute pointing to the image source.
        :param render_context: The current render context.
        :returns: The rendered graphics block string.
        """
        raise RuntimeError("Feature is not implemented by subclass")

    def render_cells(self, cells_model, render_context, target_id=None):
        r"""Render a cells/table structure.

        :param cells_model: A cells model object with ``cells_data`` (list
            of cell objects, each having ``content_nodes``, ``styles``, and
            ``placement``) and ``cells_size``.
        :param render_context: The current render context.
        :param target_id: Optional anchor ID for the table element.
        :returns: The rendered table/cells string.
        """
        raise RuntimeError("Feature is not implemented by subclass")


    # ---
    
    # helpers

    def ensure_render_context(self, render_context):
        r"""Return *render_context* if it is not ``None``, otherwise create
        a minimal standalone :py:class:`~flm.flmrendercontext.FLMRenderContext`.

        :param render_context: An existing render context, or ``None``.
        :returns: A valid :py:class:`~flm.flmrendercontext.FLMRenderContext`.
        """
        return render_context or FLMRenderContext(fragment_renderer=self)


