
class FragmentRenderer:

    use_paragraphs = True


    # ---


    def render_nodelist(self, nodelist, *, use_paragraphs=None):
        r"""
        Render a nodelist, splitting the contents into paragraphs if applicable.
        """
        if use_paragraphs is None:
            use_paragraphs = self.use_paragraphs

        if not use_paragraphs:
            return self.render_join_pieces(
                [
                    self.render_node(n)
                    for n in nodelist
                ]
            )

        nodelists_paragraphs = nodelist.split_at_node(
            lambda n: ( n.isNodeType(nodes.LatexSpecialsNode)
                        and n.specials_chars == '\n\n' )
        )

        # conver to HTML per-paragraph
        rendered_paragraphs_content = [
            self.render_join_pieces([
                self.render_node(node)
                for node in para_nodelist
            ])
            for para_nodelist in nodelists_paragraphs
        ]

        return self.render_join_as_paragraphs(rendered_paragraphs_content)

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


    def render_node(self, node):
        if node.isNodeType(nodes.LatexCharsNode):
            return self.render_node_chars(node)
        if node.isNodeType(nodes.LatexCommentNode):
            return self.render_node_comment(node)
        if node.isNodeType(nodes.LatexGroupNode):
            return self.render_node_group(node)
        if node.isNodeType(nodes.LatexMacroNode):
            return self.render_node_macro(node)
        if node.isNodeType(nodes.LatexEnvironmentNode):
            return self.render_node_environment(node)
        if node.isNodeType(nodes.LatexSpecialsNode):
            return self.render_node_specials(node)
        if node.isNodeType(nodes.LatexMathNode):
            return self.render_node_math(node)

        raise ValueError(f"Invalid node type: {node!r}")
        

    def render_node_chars(self, node):
        self.render_constant( node.chars )

    def render_node_comment(self, node):
        return ''

    def render_node_group(self, node):
        return self.render_nodelist( node.nodelist )

    def render_node_macro(self, node):
        return self.render_invocable_node(node)

    def render_node_environment(self, node):
        return self.render_invocable_node(node)

    def render_node_specials(self, node):
        return self.render_invocable_node(node)

    def render_invocable_node(self, node):
        if hasattr(node.spec.rd, 'render'):
            return node.spec.rd.render(node, self)
        # simple value stored as the 'rd' attribute
        return self.render_value( node.spec.rd )

    def render_node_math(self, node):
        # use verbatim to render math, at least for now
        return self.render_verbatim(node.latex_verbatim(),
                                    annotation=f'{node.displaytype}-math')
    

    # ---

    #
    # these methods are specifically to be overridden !!
    #
    
    # this base implementation simply renders everything to plain text

    def render_value(self, value):
        return value

    def render_empty_error_placeholder(self):
        return ''

    def render_verbatim(self, value, *, annotation=None):
        return value

    def render_text_format(self, text_format, value):
        return value
    
    
    def render_ref(self, ref_type, ref_target, display_content):
        if display_content is not None:
            return display_content
        return '??'


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
            argnode = node.nodeargd.argnlist[arg_i]
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
        
