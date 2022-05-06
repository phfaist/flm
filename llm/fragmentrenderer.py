from pylatexenc.latexnodes import nodes

class FragmentRenderer:
    r"""
    .................
    """

    use_paragraphs = True
    r"""
    If True, then LaTeX paragraphs generate separate paragraphs in the output
    format.
    """

    supports_delayed_render_markers = False
    r"""
    If True, then the render methods generate special-syntax placeholders for
    content whose rendering must be delayed (e.g., references to figures,
    etc.). If False, then the whole content must be rendered in two passes.
    """


    def render_fragment(self, llm_fragment, doc):
        return self.render_nodelist(llm_fragment.nodes, doc)

    def render_nodelist(self, nodelist, doc, use_paragraphs=None):
        r"""
        Render a nodelist, splitting the contents into paragraphs if applicable.
        """
        if use_paragraphs is None:
            use_paragraphs = self.use_paragraphs

        if not use_paragraphs:
            return self.render_join_pieces(
                [
                    self.render_node(n, doc)
                    for n in nodelist
                ],
            )

        nodelists_paragraphs = nodelist.split_at_node(
            lambda n: ( n.isNodeType(nodes.LatexSpecialsNode)
                        and n.specials_chars == '\n\n' )
        )

        # convert to HTML per-paragraph
        rendered_paragraphs_content = [
            self.render_join_pieces([
                self.render_node(node, doc)
                for node in para_nodelist
            ])
            for para_nodelist in nodelists_paragraphs
        ]

        return self.render_join_as_paragraphs(rendered_paragraphs_content)

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

        if llm_specinfo.delayed_render:
            # requested a delayed rendering -- 

            if self.supports_delayed_render_markers:

                delayed_key = doc.register_delayed_render(node)
                return self.render_delayed_marker(node, delayed_key, doc)

            else:
                # otherwise, we're in a two-pass scheme

                if doc.two_pass_mode_is_second_pass:
                    # can produce rendered content now
                    return doc.get_delayed_render_content(node)

                # register the delayed node, we don't need the delayed_key
                _ = doc.register_delayed_render(node)

                # dummy placeholder, you'll never see it unless there's a bug:
                return '<#>'

        # simply call render() to get the rendered value

        value = llm_specinfo.render(node, doc, self)
        return value


    def render_node_math(self, node, doc):
        return self.render_math_content( node.delimiters, node.nodelist, doc,
                                         node.displaytype, None )


    def render_math_content(self, delimiters, nodelist, doc, displaytype, environmentname):
        # Use verbatim to render math in the base implementation. It will work
        # for our HTML implementation as well since we'll rely on MathJax.
        # Other implementations that don't want to render math in this type of
        # way will have to reimplement render_node_math().
        return self.render_verbatim(
            delimiters[0] + nodelist.latex_verbatim() + delimiters[1],
            f'{displaytype}-math'
        )
    


    # ---
    

    def render_join_pieces(self, pieces):
        return "".join(pieces)

    def render_join_as_paragraphs(self, paragraphs_content):
        r"""
        Render a sequence of paragraphs.  The argument `paragraphs_content` is a
        sequence (list) of the rendered contents of each paragraph.  This method
        must make sure they are treated as individual paragraphs (e.g., wrap the
        contents in ``<p>...</p>`` tags, or render the contents separated by
        ``\n\n`` or etc.).
        """
        return "\n\n".join(paragraphs_content)


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
            if argnode is None:
                argnodelist = nodes.LatexNodeList([None])
            elif argnode.isNodeType(nodes.LatexGroupNode):
                argnodelist = argnode.nodelist
            else:
                argnodelist = nodes.LatexNodeList([argnode])

            args_nodelists[arg_spec.argname] = argnodelist

        if not skip_nonexistent:
            # if there's an argument in argnames that wasn't seen, that's an
            # error
            for argname in argnames:
                if argname not in argnames_seen:
                    raise ValueError(f"Missing argument ‘{argname}’ to {node}")
                
        return args_nodelists
        




class TextFragmentRenderer(FragmentRenderer):

    display_href_urls = True

    #supports_delayed_render_markers = False # -- inherited alreay

    def render_value(self, value):
        return value

    def render_delayed_marker(self, node, delayed_key, doc):
        return ''

    def render_empty_error_placeholder(self, debug_str):
        return ''

    def render_verbatim(self, value, annotation):
        return value

    def render_text_format(self, text_format, value):
        return value
    
    def render_link(self, ref_type, href, display_content):
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


