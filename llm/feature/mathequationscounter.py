
...........................



class MathEnvironmentRegisterRef(MathEnvironment):

    .............

    def render(self, node, render_context):

        refs_mgr = None
        if self.register_refs_reference \
           and render_context.supports_feature('refs') \
           and hasattr(node, 'llm_equation_label_node'):

            # register reference

            refs_mgr = render_context.feature_render_manager('refs')

            .............. cut out -->
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
            ....... <--

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


