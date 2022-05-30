def sanitize_for_id(x):
    return re.sub(r'[^a-zA-Z0-9_-]', '-', x)



class MathEnvironmentRegisterRef(MathEnvironment):

    .............

    def render(self, node, render_context):

        refs_mgr = None
        if self.register_refs_reference \
           and render_context.supports_feature('refs') \
           and hasattr(node, 'llm_equation_label_node'):

            # register reference

            refs_mgr = render_context.feature_render_manager('refs')

            ref_label_node = node.llm_equation_label_node
            logging.debug("Equation has label: %r", ref_label_node)
            ref_label_node_args = \
                ParsedArgumentsInfo(node=ref_label_node).get_all_arguments_info(
                ('label',),
            )
            ref_label_value = ref_label_node_args['label'].get_content_as_chars()

            if ':' in ref_label_value:
                ref_label_prefix, ref_target = ref_label_value.split(':', 1)
            else:
                ref_label_prefix, ref_target = None, ref_label_value

            target_id = f"equation--{sanitize_for_id(ref_label_value)}"
            self.register_equation_reference(
                'eq', ref_target, node, ref_label_node,
                target_id, render_context, refs_mgr
            )

        return 

    def register_equation_reference(self, ref_prefix, ref_target, node, ref_label_node,
                                    target_id, render_context, refs_mgr):
        if ref_prefix != 'eq':
            raise LatexWalkerParseError(
                f"Equation labels must be of the form ‘eq:...’: ‘{ref_prefix}:{ref_target}’",
                pos=node.llm_label_node.pos
            )

        refs_mgr.register_reference(
            'eq',
            ref_target,
            "(*)",
            f"#{target_id}"
        )



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

        # find and register \label node
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


