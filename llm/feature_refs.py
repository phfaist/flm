
from pylatexenc import macrospec

from .llmspecinfo import LLMSpecInfo, LLMMacroSpec

from .feature import Feature, FeatureDocumentManager



class RefInstance:
    def __init__(self, ref_type, ref_target, formatted_ref_text, target_href):
        super().__init__()
        self.ref_type = ref_type
        self.ref_target = ref_target
        self.formatted_ref_text = formatted_ref_text
        self.target_href = target_href


class FeatureRefsDocumentManager(FeatureDocumentManager):

    def initialize(self):
        self.ref_labels = {}
        
    def register_reference(self, ref_type, ref_target, formatted_ref_text, target_href):
        self.ref_labels[(ref_type, ref_target)] = RefInstance(
            ref_type=ref_type,
            ref_target=ref_target,
            formatted_ref_text=formatted_ref_text,
            target_href=target_href,
        )

    def get_ref(self, ref_type, ref_target):
        if (ref_type, ref_target) in self.ref_labels:
            return self.ref_labels[(ref_type, ref_target)]

        if self.feature.external_ref_resolver is not None:
            ref = self.feature.external_ref_resolver.get_ref(ref_type, ref_target)
            if ref is not None:
                return ref

        raise ValueError(f"Ref target not found: ‘{ref_type}:{ref_target}’")



class FeatureRefs(Feature):
    r"""
    Manager for internal references, such as ``\ref{...}``, ``\hyperref{...}``,
    etc.
    """

    feature_name = 'refs'
    feature_manager_class = FeatureRefsDocumentManager

    def __init__(self, external_ref_resolver=None):
        super().__init__()
        # e.g., a different code page in the EC zoo!
        self.external_ref_resolver = external_ref_resolver

    def add_latex_context_definitions(self):
        return dict(
            macros=[
                LLMMacroSpec(
                    'ref',
                    [
                        macrospec.LatexArgumentSpec('{', argname='ref_target'),
                    ],
                    llm_specinfo=RefSpecInfo('ref', command_arguments=('ref_target',))
                ),
                LLMMacroSpec(
                    'hyperref',
                    [
                        macrospec.LatexArgumentSpec('[', argname='ref_target'),
                        macrospec.LatexArgumentSpec('{', argname='display_text'),
                    ],
                    llm_specinfo=RefSpecInfo('ref',
                                             command_arguments=('ref_target','display_text',))
                ),
            ]
        )


class RefSpecInfo(LLMSpecInfo):

    delayed_render = True

    def __init__(
            self,
            ref_type,
            command_arguments=('ref_target', 'display_text',)
    ):
        super().__init__()
        self.ref_type = ref_type
        self.command_arguments = command_arguments
        
    def prepare_delayed_render(self, node, doc, fragment_renderer):
        pass

    def render(self, node, doc, fragment_renderer):

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            self.command_arguments,
            all=True,
            skip_nonexistent=True,
        )

        ref_type = None
        ref_target = fragment_renderer.get_nodelist_as_chars(
            node_args['ref_target'].nodelist
        )
        if ':' in ref_target:
            ref_type, ref_target = ref_target.split(':', 1)

        if 'display_text' in node_args:
            display_content = fragment_renderer.render_nodelist(
                node_args['display_text'],
                doc=doc,
            )
        else:
            display_content = None

        mgr = doc.feature_manager('refs')

        ref_instance = mgr.get_ref(ref_type, ref_target)

        if display_content is None:
            display_content = ref_instance.formatted_ref_text

        return fragment_renderer.render_link(
            'ref',
            ref_instance.target_href,
            display_content,
            annotations=[f'ref-{ref_type}',], # TODO: add annotation for external links etc.
        )

