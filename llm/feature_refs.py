
from pylatexenc import macrospec

from .llmspecinfo import LLMSpecInfo, LLMMacroSpec
from .llmenvironment import make_arg_spec

from .feature import Feature



class RefInstance:
    def __init__(self, ref_type, ref_target, formatted_ref_llm_text, target_href):
        super().__init__()
        self.ref_type = ref_type
        self.ref_target = ref_target
        self.formatted_ref_llm_text = formatted_ref_llm_text
        self.target_href = target_href


class FeatureRefsRenderManager(Feature.RenderManager):

    def initialize(self):
        self.ref_labels = {}
        
    def register_reference(self, ref_type, ref_target, formatted_ref_llm_text, target_href):
        r"""
        `formatted_ref_llm_text` is LLM code.
        """
        self.ref_labels[(ref_type, ref_target)] = RefInstance(
            ref_type=ref_type,
            ref_target=ref_target,
            formatted_ref_llm_text=formatted_ref_llm_text,
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
    RenderManager = FeatureRefsRenderManager

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
                        make_arg_spec('{', argname='ref_target'),
                    ],
                    llm_specinfo=RefSpecInfo('ref', command_arguments=('ref_target',))
                ),
                LLMMacroSpec(
                    'hyperref',
                    [
                        make_arg_spec('[', argname='ref_target'),
                        make_arg_spec('{', argname='display_text'),
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
        
    def prepare_delayed_render(self, node, render_context):
        pass

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

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
            display_content_nodelist = node_args['display_text'].nodelist
        else:
            display_content_nodelist = None

        mgr = render_context.feature_render_manager('refs')

        ref_instance = mgr.get_ref(ref_type, ref_target)

        if display_content_nodelist is None:
            display_content_llm = render_context.doc.environment.make_fragment(
                ref_instance.formatted_ref_llm_text
            )
            display_content_nodelist = display_content_llm.nodes


        return fragment_renderer.render_link(
            'ref',
            ref_instance.target_href,
            display_content_nodelist,
            render_context=render_context,
            annotations=[f'ref-{ref_type}',], # TODO: add annotation for external links etc. ??
        )

