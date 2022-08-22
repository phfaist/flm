import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import (
    LatexWalkerParseError,
    ParsedArgumentsInfo
)
#from pylatexenc import macrospec

from ..llmfragment import LLMFragment
from ..llmspecinfo import LLMMacroSpecBase
from ..llmenvironment import LLMArgumentSpec

from ._base import Feature




class ReferenceableInfo:
    def __init__(self, formatted_ref_llm_text, labels):
        super().__init__()
        self.formatted_ref_llm_text = formatted_ref_llm_text
        self.labels = labels # list of (ref_type, ref_label)

        self._fields = ('formatted_ref_llm_text', 'labels',)

    def get_target_id(self):

        if not self.labels:
            return None

        lbl_ref_type, lbl_ref_label = self.labels[0]
        return get_safe_target_id(lbl_ref_type, lbl_ref_label)


    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ", ".join(f"{k}={getattr(self,k)!r}" for k in self._fields)
            + ")"
        )



class RefInstance:
    def __init__(self, ref_type, ref_target, formatted_ref_llm_text, target_href):
        super().__init__()
        self.ref_type = ref_type
        self.ref_target = ref_target
        self.formatted_ref_llm_text = formatted_ref_llm_text
        self.target_href = target_href

        self._fields = ('ref_type', 'ref_target', 'formatted_ref_llm_text', 'target_href',)

    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ", ".join(f"{k}={getattr(self,k)!r}" for k in self._fields)
            + ")"
        )





_rx_unsafe_char = re.compile(r'[^a-zA-Z0-9-]')
def _rx_match_safechar(m):
    return f'_{ord(m.group()):x}X'

def get_safe_target_id(ref_type, ref_label):
    ref_type_safe = _rx_unsafe_char.sub(_rx_match_safechar, ref_type)
    ref_label_safe = _rx_unsafe_char.sub(_rx_match_safechar, ref_label)
    return f"{ref_type_safe}-{ref_label_safe}"




class FeatureRefsRenderManager(Feature.RenderManager):

    def initialize(self):
        self.ref_labels = {}
        self.registered_references = {}
        self.external_ref_resolvers = self.feature.external_ref_resolvers
        
    def register_reference_referenceable(self, *, node, referenceable_info):

        if not referenceable_info.labels:
            return

        target_href = '#' + referenceable_info.get_target_id()

        for ref_type, ref_label in referenceable_info.labels:
            self.register_reference(
                ref_type, ref_label,
                formatted_ref_llm_text=referenceable_info.formatted_ref_llm_text,
                node=node,
                target_href=target_href,
            )


    def register_reference(self, ref_type, ref_target, *,
                           node, formatted_ref_llm_text, target_href):
        r"""
        ........
        
        If you call this method a second time on the same render context with
        the same `node` instance and the same `(ref_type, ref_target)`, then the
        additional arguments are ignored and the earlier registered reference
        refinstance is returned instead.

        `formatted_ref_llm_text` is LLM code given as a string or as a
        LLMFragment instance.
        """

        node_id = self.get_node_id(node)
        kk = (node_id, ref_type, ref_target)
        if kk in self.registered_references:
            return self.registered_references

        if (ref_type, ref_target) in self.ref_labels:
            raise ValueError(
                f"Duplicate reference label ‘{ref_type}:{ref_target}’ in the same document!"
            )

        refinstance = RefInstance(
            ref_type=ref_type,
            ref_target=ref_target,
            formatted_ref_llm_text=formatted_ref_llm_text,
            target_href=target_href,
        )
        self.registered_references[ kk ] = refinstance
        self.ref_labels[ (ref_type, ref_target) ] = refinstance
        logger.debug("Registered reference: %r", refinstance)
        return refinstance


    def get_ref(self, ref_type, ref_target, resource_info):
        if (ref_type, ref_target) in self.ref_labels:
            return self.ref_labels[(ref_type, ref_target)]

        logger.debug(f"Couldn't find {(ref_type, ref_target)} in current document "
                     f"labels; will query external ref resolver.  {self.ref_labels=}")

        for resolver in self.external_ref_resolvers:
            ref = resolver.get_ref(
                ref_type,
                ref_target,
                resource_info,
            )
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

    def __init__(self, external_ref_resolvers=None):
        super().__init__()
        # e.g., resolve a reference to a different code page in the EC zoo!
        self.external_ref_resolvers = external_ref_resolvers

    def set_external_ref_resolvers(self, external_ref_resolvers):
        if self.external_ref_resolvers is not None:
            logger.warning("FeatureRefs.set_external_ref_resolvers(): There were already "
                           "external refs resolvers set.  They will be replaced.")
        self.external_ref_resolvers = external_ref_resolvers

    def add_external_ref_resolver(self, external_ref_resolver):
        self.external_ref_resolvers.append( external_ref_resolver )

    def add_latex_context_definitions(self):
        return dict(
            macros=[
                RefMacro(macroname='ref', command_arguments=('ref_target',)),
                RefMacro(
                    macroname='hyperref',
                    command_arguments=('[]ref_target','display_text',)
                ),
            ]
        )


_ref_arg_specs = {
    'ref_target': LLMArgumentSpec(latexnodes_parsers.LatexCharsGroupParser(),
                                  argname='ref_target'),
    '[]ref_target': LLMArgumentSpec(
        latexnodes_parsers.LatexCharsGroupParser(
            delimiters=('[', ']'),
        ),
        argname='ref_target'
    ),
    'display_text': LLMArgumentSpec('{', argname='display_text',),
}


class RefMacro(LLMMacroSpecBase):

    delayed_render = True

    def __init__(
            self,
            macroname,
            *,
            ref_type='ref',
            command_arguments=('ref_target', 'display_text',)
    ):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=self._get_arguments_spec_list(command_arguments),
        )
        self.ref_type = ref_type
        self.command_arguments = [ c.replace('[]','') for c in command_arguments ]
        
    @classmethod
    def _get_arguments_spec_list(self, command_arguments):
        return [ _ref_arg_specs[argname]
                 for argname in command_arguments ]

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            self.command_arguments,
        )

        ref_type = None
        ref_target = node_args['ref_target'].get_content_as_chars()
        if ':' in ref_target:
            ref_type, ref_target = ref_target.split(':', 1)

        if 'display_text' in node_args:
            display_content_nodelist = node_args['display_text'].get_content_nodelist()
        else:
            display_content_nodelist = None

        node.llm_ref_info = {
            'ref_type_and_target': (ref_type, ref_target),
            'display_content_nodelist': display_content_nodelist,
        }
        

    def prepare_delayed_render(self, node, render_context):
        pass

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        ref_type, ref_target = node.llm_ref_info['ref_type_and_target']
        display_content_nodelist = node.llm_ref_info['display_content_nodelist']

        mgr = render_context.feature_render_manager('refs')

        resource_info = node.latex_walker.resource_info

        try:
            ref_instance = mgr.get_ref(ref_type, ref_target, resource_info)
        except Exception as e:
            raise LatexWalkerParseError(
                f"Unable to resolve reference to ‘{ref_type}:{ref_target}’: {e}",
                pos=node.pos,
            )

        if display_content_nodelist is None:
            if isinstance(ref_instance.formatted_ref_llm_text, LLMFragment):
                display_content_llm = ref_instance.formatted_ref_llm_text
            else:
                display_content_llm = render_context.doc.environment.make_fragment(
                    ref_instance.formatted_ref_llm_text,
                    standalone_mode=True
                )
            display_content_nodelist = display_content_llm.nodes


        return fragment_renderer.render_link(
            'ref',
            ref_instance.target_href,
            display_content_nodelist,
            render_context=render_context,
            annotations=[f'ref-{ref_type}',], # TODO: add annotation for external links etc. ??
        )

