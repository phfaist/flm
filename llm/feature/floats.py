


class SimpleIncludeGraphicsSpecInfo(LLMSpecInfo):

    is_block_level = True

    def scan(self, node, scanner):

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer
        
        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('graphics_options', 'graphics_path',),
            all=True
        )
        graphics_options_value = fragment_renderer.get_nodelist_as_chars(
            node_args['graphics_options'].nodelist,
        )
        graphics_path = fragment_renderer.get_nodelist_as_chars(
            node_args['graphics_path'].nodelist,
        )

        if graphics_options_value:
            raise LatexWalkerParseError(
                f"Graphics options are not supported here: ‘{graphics_options_value}’",
                pos=node_args['graphics_options'].nodelist.pos,
            )
        
        return fragment_renderer.render_graphics_image(
            graphics_path
        )




class FloatEnvironmentSpecInfo(LLMSpecInfo):

    def render(self, node, render_context):
        environmentname = node.environmentname
        floats_mgr = render_context.feature_render_manager('floats')

        .........

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        MacroSpec('label', arguments_spec_list=[
                            make_arg_spec(
                                parser=latexnodes_parsers.LatexCharsGroupParser(
                                    delimiters=('{','}'),
                                ),
                                argname='label',
                            ),
                        ])
                    ]
                )
            )
        )

    def finalize_parsed_node(self, node):
        # parse the node structure right away when finializing the node to try
        # to find any \label{} instruction.
        logger.debug("finalizing math environment node: node = %r", node)

        # find and register child nodes
        node.llm_equation_label_node = None
        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'label':
                # this is the equation's \label command -- register it
                if node.llm_equation_label_node is not None:
                    raise LatexWalkerParseError(
                        "You cannot use multiple \\label's in an equation",
                        pos=n.pos
                    )
                node.llm_equation_label_node = n
                logger.debug("Found label node: %r", n)

        return node





class FeatureFloats(LLMSpecInfo):
    def __init__(self, float_type='figure', float_caption_name='Figure'):
        super().__init__()
        self.float_type = float_type
        self.float_caption_name = float_caption_name
        
    def finalize_parsed_node(self, node):
        ...

    def render(self, node, doc, fragment_renderer):
        
