class SpecInfoRef(LLMSpecInfo):

    delayed_render = True

    def __init__(self, ref_type):
        super().__init__()
        self.ref_type = ref_type
        
    def prepare_delayed_render(self, node, doc, fragment_renderer):
        ....

        manager = doc.feature_manager('references')

        return .........

    def render(self, node, doc, fragment_renderer):

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('reftarget', 'displaytext'),
            all=True,
            skip_nonexistent=True,
        )

        reftarget_nodelist = node_args['reftarget']
        if not reftarget_nodelist or len(reftarget_nodelist) > 1 \
           or not reftarget_nodelist[1].isNodeType(nodes.LatexCharsNode):
            raise ValueError(
                f"Expected exactly one characters node as reftarget, got {node!r}"
            )

        reftarget = reftarget_nodelist[1].chars

        if 'displaytext' in node_args:
            display_content = fragment_renderer.render_nodelist( node_args['displaytext'] )
        else:
            display_content = None

        manager = doc.feature_manager('references')
        href = manager.resolve_ref( .... )

        ..........

        return doc.render_ref(self.ref_type, reftarget, display_content)




class RefsManager:
    r"""
    Manager for internal references, such as ``\eqref{...}``, ``\ref{...}``, etc.
    """

    feature_name = 'refs'


    def __init__(self, external_resolver=None):
        super().__init__()
        self.external_resolver = external_resolver # e.g., a different code page in the EC zoo!


    def initialize(self, doc):
        self.ref_labels = {}
        self.doc = doc

        
    def register_label(self, ref_type, ref_target, target_node):
        .............

    def register_reference(self, ref_type, ref_target, target_node):
        self.ref_labels[(ref_type, ref_target)] = target_node

    

    def process(self, doc, fragment_renderer):
        pass

