import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import (
    LatexWalkerLocatedError,
    ParsedArgumentsInfo
)
#from pylatexenc import macrospec

from ..flmfragment import FLMFragment
from ..flmspecinfo import FLMMacroSpecBase
from ..flmenvironment import FLMArgumentSpec

from ._base import Feature




class ReferenceableInfo:
    def __init__(self, formatted_ref_flm_text, labels):
        super().__init__()
        self.formatted_ref_flm_text = formatted_ref_flm_text
        self.labels = labels # list of (ref_type, ref_label)

        self._fields = ('formatted_ref_flm_text', 'labels',)

    def get_target_id(self):

        if not self.labels:
            return None

        lbl_ref_type, lbl_ref_label = self.labels[0]
        return get_safe_target_id(lbl_ref_type, lbl_ref_label)

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ", ".join(f"{k}={getattr(self,k)!r}" for k in self._fields)
            + ")"
        )



class RefInstance:
    def __init__(self, ref_type, ref_label, formatted_ref_flm_text, target_href,
                 counter_value, counter_formatter_id):
        super().__init__()
        self.ref_type = ref_type
        self.ref_label = ref_label
        self.formatted_ref_flm_text = formatted_ref_flm_text
        self.target_href = target_href
        self.counter_value = counter_value
        self.counter_formatter_id = counter_formatter_id

        self._fields = ('ref_type', 'ref_label', 'formatted_ref_flm_text', 'target_href',
                        'counter_value', 'counter_formatter_id',)

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ", ".join(f"{k}={repr(getattr(self,k))}" for k in self._fields)
            + ")"
        )





# Apparently, Transcrypt does not support hexadecimal formatting, neither
# through a string format like '{:x}'.format(...), nor f'{...:x}', neither via
# hex(...) ... so we provide our own JS implementation.

# Feed some raw JS to transcrypt directly
#__pragma__('ecom')
"""?
__pragma__('js', 'var hexstr = (v) => (+v).toString(16);');
?"""

#__pragma__('skip')
def hexstr(value):
    return f'{value:x}'
#__pragma__('noskip')


_rx_unsafe_char = re.compile(r'[^a-zA-Z0-9-]')
def _rx_match_safechar(m):
    return f'_{hexstr(ord(m.group()))}X'

def get_safe_target_id(ref_type, ref_label):
    ref_type_safe = _rx_unsafe_char.sub(_rx_match_safechar, ref_type)
    ref_label_safe = _rx_unsafe_char.sub(_rx_match_safechar, ref_label)
    return f"{ref_type_safe}-{ref_label_safe}"




class FeatureRefsRenderManager(Feature.RenderManager):

    def initialize(self, add_external_ref_resolvers=None):
        self.ref_labels = {}
        self.registered_references = {}
        self.external_ref_resolvers = []
        if add_external_ref_resolvers:
            self.external_ref_resolvers.extend(add_external_ref_resolvers)
        self.external_ref_resolvers.extend(self.feature.external_ref_resolvers)

        self.registered_counter_formatters = {}
        
    def register_reference_referenceable(self, *, node, referenceable_info):

        if not referenceable_info.labels:
            return

        target_href = '#' + referenceable_info.get_target_id()

        for ref_type, ref_label in referenceable_info.labels:
            self.register_reference(
                ref_type, ref_label,
                formatted_ref_flm_text=referenceable_info.formatted_ref_flm_text,
                node=node,
                target_href=target_href,
            )

    def register_counter_formatter(self, *, counter_formatter_id=None, counter_formatter):
        if counter_formatter_id is None:
            counter_formatter_id = counter_formatter.counter_formatter_id
        if counter_formatter_id in self.registered_counter_formatters:
            raise ValueError(
                f"Counter formatter width ID ‘{counter_formatter_id}’ is already registered!"
            )
        self.registered_counter_formatters[counter_formatter_id] = counter_formatter

    def register_reference_step_counter(
            self, ref_type=None, ref_label=None, *,
            node, counter, target_href_fn=None,
            counter_with_prefix=True,
            counter_prefix_variant=None, counter_with_delimiters=True):
        
        node_id = self.get_node_id(node)
        kk = (node_id, ref_type, ref_label)
        if kk in self.registered_references:
            return self.registered_references[kk]

        counter.step()

        formatted_ref_flm_text = counter.format_flm(
            with_prefix=counter_with_prefix,
            prefix_variant=counter_prefix_variant,
            with_delimiters=counter_with_delimiters,
        )

        if target_href_fn is not None:
            target_href = target_href_fn(counter.value)
        else:
            target_href = None

        return self.register_reference(
            ref_type, ref_label,
            node=node,
            formatted_ref_flm_text=formatted_ref_flm_text,
            target_href=target_href,
            counter_value=counter.value,
            counter_formatter_id=counter.formatter.counter_formatter_id
        )


    def register_reference(self, ref_type, ref_label, *,
                           node, formatted_ref_flm_text, target_href,
                           counter_value=None, counter_formatter_id=None):
        r"""
        ........
        
        If you call this method a second time on the same render context with
        the same `node` instance and the same `(ref_type, ref_label)`, then the
        additional arguments are ignored and the earlier registered reference
        refinstance is returned instead.

        `formatted_ref_flm_text` is FLM code given as a string or as a
        FLMFragment instance.
        """

        node_id = self.get_node_id(node)
        kk = (node_id, ref_type, ref_label)
        if kk in self.registered_references:
            return self.registered_references[kk]

        if (ref_type is not None and ref_label is not None
            and (ref_type, ref_label) in self.ref_labels):
            raise ValueError(
                f"Duplicate reference label ‘{ref_type}:{ref_label}’ in the same document!"
            )

        refinstance = RefInstance(
            ref_type=ref_type,
            ref_label=ref_label,
            formatted_ref_flm_text=formatted_ref_flm_text,
            target_href=target_href,
            counter_value=counter_value,
            counter_formatter_id=counter_formatter_id,
        )

        self.registered_references[ kk ] = refinstance
        if ref_type is not None and ref_label is not None:
            self.ref_labels[ (ref_type, ref_label) ] = refinstance
        logger.debug("Registered reference: %r", refinstance)
        return refinstance


    def get_ref(self, ref_type, ref_label, resource_info):
        if (ref_type, ref_label) in self.ref_labels:
            return self.ref_labels[(ref_type, ref_label)]

        logger.debug(f"Couldn't find {(ref_type, ref_label)} in current document "
                     f"labels; will query external ref resolvers.  self.ref_labels=%r",
                     self.ref_labels)

        logger.debug("external ref resolvers are %r", self.external_ref_resolvers)

        for resolver in self.external_ref_resolvers:
            logger.debug("Trying external ref resolver ... %r", resolver)
            ref = resolver.get_ref(
                ref_type,
                ref_label,
                resource_info,
                self.render_context,
            )
            logger.debug("Tried external ref resolver, ref=%r", ref)
            if ref is not None:
                return ref

        raise ValueError(f"Ref target ‘{ref_type}:{ref_label}’ found neither within "
                         f"database nor with any set external resolvers")


    def render_ref(self, ref_type, ref_label, display_content_flm,
                   resource_info, render_context,
                   counter_prefix_variant=None,
                   counter_with_prefix=True, counter_with_delimiters=True):

        ref_instance = self._get_ref_instance(ref_type, ref_label, resource_info)

        return self.render_ref_instance(
            ref_instance, display_content_flm, render_context,
            counter_prefix_variant=counter_prefix_variant,
            counter_with_prefix=counter_with_prefix,
            counter_with_delimiters=counter_with_delimiters,
        )

    def render_ref_instance(self, ref_instance, display_content_flm, render_context,
                            counter_prefix_variant=None,
                            counter_with_prefix=True, counter_with_delimiters=True):

        if display_content_flm is None:
            display_content_flm = ref_instance.formatted_ref_flm_text

        if not isinstance(display_content_flm, FLMFragment):
            display_content_flm = render_context.doc.environment.make_fragment(
                display_content_flm,
                standalone_mode=True
            )

        display_content_nodelist = display_content_flm.nodes

        fragment_renderer = render_context.fragment_renderer

        return fragment_renderer.render_link(
            'ref',
            ref_instance.target_href,
            display_content_nodelist,
            render_context=render_context,
            # TODO: add annotation for external links etc. ??
            annotations=[f'ref-{ref_instance.ref_type}',],
        )


    ref_many_use_flm_hyperref = True

    def render_ref_many(self, ref_type_label_list, resource_info, render_context, *,
                        counter_prefix_variant=None, counter_with_delimiters=True,
                        counter_with_prefix=True):

        fragment_renderer = render_context.fragment_renderer

        ref_instances = [
            self._get_ref_instance(ref_type, ref_label, resource_info)
            for (ref_type, ref_label) in ref_type_label_list
        ]
        ref_instances_by_counter_formatter_id = {}
        ref_instances_nocounter = []
        for ri in ref_instances:
            if (ri.counter_value is None or ri.counter_formatter_id is None):
                ref_instances_nocounter.append(ri)
                continue
            if ri.counter_formatter_id not in ref_instances_by_counter_formatter_id:
                ref_instances_by_counter_formatter_id[ri.counter_formatter_id] = {}

            ref_instances_by_counter_formatter_id[ri.counter_formatter_id][ri.counter_value] = ri

        s_final_blocks = []

        for counter_formatter_id, rcdict in ref_instances_by_counter_formatter_id.items():
            #
            counter_formatter = self.registered_counter_formatters[counter_formatter_id]
            #
            s_items = counter_formatter.format_many_flm(
                rcdict.keys(),
                prefix_variant=counter_prefix_variant,
                with_delimiters=counter_with_delimiters,
                with_prefix=counter_with_prefix,
                get_raw_s_items=True,
            )
            s = ''
            for sit in s_items:
                s_frag = render_context.make_standalone_fragment(
                    sit['s'],
                    is_block_level=False,
                    what=f"Rendered counter ref bit {repr(sit)}",
                )
                if sit['n'] is None or sit['n'] is False:
                    s += fragment_renderer.render_fragment(s_frag, render_context)
                else:
                    rinst = rcdict[sit['n']]
                    s += fragment_renderer.render_link(
                        'ref',
                        rinst.target_href,
                        s_frag.nodes,
                        render_context=render_context,
                        # TODO: add annotation for external links etc. ??
                        annotations=[f'refcnt-{counter_formatter_id}',],
                    )
            s_final_blocks.append( s )

        if len(ref_instances_nocounter):
            for ri in ref_instances_nocounter:
                s_final_blocks += [ self.render_ref_instance(ri, None, render_context) ]

        return ', '.join(s_final_blocks)



    def _get_ref_instance(self, ref_type, ref_label, resource_info):

        try:
            return self.get_ref(ref_type, ref_label, resource_info)
        except Exception as e:
            logger.debug(f"render_ref({ref_type}, {ref_label}): self.get_ref() failed: {e}",
                         exc_info=True)
            raise ValueError(
                f"Unable to resolve reference to ‘{ref_type}:{ref_label}’: {e} "
                f"[in {repr(resource_info)}]"
            )






class FeatureRefs(Feature):
    r"""
    Manager for internal references, such as ``\ref{...}``, ``\hyperref{...}``,
    etc.
    """

    feature_name = 'refs'
    feature_title = 'Labels and cross-references'

    RenderManager = FeatureRefsRenderManager

    def __init__(self, external_ref_resolvers=None):
        super().__init__()
        # e.g., resolve a reference to a different code page in the EC zoo!
        if external_ref_resolvers is not None:
            self.external_ref_resolvers = list(external_ref_resolvers)
        else:
            self.external_ref_resolvers = []
        logger.debug(f"Created FeatureRefs with external_ref_resolvers = "
                     f"{repr(external_ref_resolvers)}")

    def set_external_ref_resolvers(self, external_ref_resolvers):
        if self.external_ref_resolvers is not None and len(self.external_ref_resolvers):
            logger.warning("FeatureRefs.set_external_ref_resolvers(): There were already "
                           "external refs resolvers set.  They will be replaced.")
        self.external_ref_resolvers = external_ref_resolvers

    def add_external_ref_resolver(self, external_ref_resolver):
        self.external_ref_resolvers.append( external_ref_resolver )

    def add_latex_context_definitions(self):
        return dict(
            macros=[
                RefMacro(
                    macroname='ref',
                    command_arguments=('ref_label',)
                ),
                RefMacro(
                    macroname='hyperref',
                    command_arguments=('[]ref_label','display_text',)
                ),
            ]
        )


    # mainly for debug messages
    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            f"external_ref_resolvers={repr(self.external_ref_resolvers)})"
        )


_ref_arg_specs = {
    'ref_label': FLMArgumentSpec(latexnodes_parsers.LatexCharsGroupParser(),
                                  argname='ref_label'),
    '[]ref_label': FLMArgumentSpec(
        latexnodes_parsers.LatexCharsGroupParser(
            delimiters=('[', ']'),
        ),
        argname='ref_label'
    ),
    'display_text': FLMArgumentSpec('{', argname='display_text',),
}


class RefMacro(FLMMacroSpecBase):

    delayed_render = True

    def __init__(
            self,
            macroname,
            *,
            ref_type='ref',
            command_arguments=('ref_label', 'display_text',),
            counter_prefix_variant=None,
    ):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=self._get_arguments_spec_list(command_arguments),
        )
        self.ref_type = ref_type
        self.command_arguments = [ c.replace('[]','') for c in command_arguments ]
        self.counter_prefix_variant = counter_prefix_variant
        
    @classmethod
    def _get_arguments_spec_list(cls, command_arguments):
        return [ _ref_arg_specs[argname]
                 for argname in command_arguments ]

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            self.command_arguments,
        )

        ref_spec = node_args['ref_label'].get_content_as_chars()
        ref_pair_list = []
        for ref_spec_one in ref_spec.split(','):
            ref_type, ref_label = None, ref_spec_one
            if ':' in ref_label:
                ref_type, ref_label = ref_label.split(':', 1)
            ref_pair_list.append( (ref_type, ref_label) )

        if 'display_text' in node_args:
            display_content_nodelist = node_args['display_text'].get_content_nodelist()
        else:
            display_content_nodelist = None

        node.flmarg_ref_list = ref_pair_list
        if len(ref_pair_list) == 1:
            node.flmarg_ref = ref_pair_list[0]
        node.flm_ref_info = {
            'ref_list': ref_pair_list,
            'display_content_nodelist': display_content_nodelist,
        }
        

    def prepare_delayed_render(self, node, render_context):
        pass

    def render(self, node, render_context):

        ref_list = node.flm_ref_info['ref_list']
        display_content_nodelist = node.flm_ref_info['display_content_nodelist']

        mgr = render_context.feature_render_manager('refs')
        resource_info = node.latex_walker.resource_info

        if len(ref_list) == 1:
            ref_type, ref_label = ref_list[0]
            try:
                return mgr.render_ref(ref_type, ref_label,
                                      display_content_nodelist,
                                      resource_info,
                                      render_context,
                                      counter_prefix_variant=self.counter_prefix_variant)
            except Exception as e:
                logger.error(f"Failed to resolve reference to ‘{ref_type}:{ref_label}’: {e} "
                             f"in ‘{node.latex_verbatim()}’ @ {node.format_pos()}")
                raise LatexWalkerLocatedError(
                    f"Unable to resolve reference to ‘{ref_type}:{ref_label}’: {e}",
                    pos=node.pos,
                )

        # we have multiple ref targets

        # can only have one ref target if we have a display string

        if display_content_nodelist is not None:
            raise ValueError("Reference with custom display string cannot "
                             "have multiple ref targets: " + repr(ref_list))

        return mgr.render_ref_many(ref_list, resource_info, render_context)


# ------------------------------------------------

FeatureClass = FeatureRefs
