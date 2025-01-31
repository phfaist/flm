
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexNodesLatexRecomposer




class FLMNodesFlmRecomposer(LatexNodesLatexRecomposer):
    r"""
    Class capable of recomposing FLM code that corresponds to the
    information stored in a node and its descendants.

    This class can also be used whenever we want to traverse the node tree and
    produce some string representation of the nodes.
    """

    recompose_specinfo_method = 'recompose_flm_text'

    def recompose_flm_text(self, node):
        r"""
        Attempt to recompose FLM code corresponding to the given node or
        node list.  Returns a string.
        """
        return self.start(node)


    rx_escape_chars_text = None

    recompose_escape_chars_if_specials_disabled = False

    def escape_chars(self, chars, parsing_state):
        if self.rx_escape_chars_text is None:
            return chars
        if not parsing_state.enable_specials \
           and not self.recompose_escape_chars_if_specials_disabled:
            return chars
        if parsing_state.in_math_mode:
            return chars
        return self.rx_escape_chars_text.sub(lambda m: '\\'+m.group(), chars)


    def subrecompose(self, node):
        r"""
        Helper method for calling within FLMSpecInfo `recompose_***()` functions.

        [In the future, we should add possibilities to create a sub-recomposer
        object/visitor where new sub-options can be set just for recomposing
        this specific node and its children.]
        """
        return node.accept_node_visitor(self)


    # ---

    def _attempt_node_specinfo_recompose(self, node, **kwargs):
        if hasattr(node, 'flm_specinfo') and \
           hasattr(node.flm_specinfo, self.recompose_specinfo_method):
            return getattr(node.flm_specinfo, self.recompose_specinfo_method)(
                node=node,
                recomposer=self,
                **kwargs
            )
        return False


    def recompose_chars(self, chars, n):
        if not chars:
            chars = '' # not None or other stuff
        chars = str(chars)
        return self.escape_chars(chars, n.parsing_state)


    def node_standard_process_macro(self, node):
        recomposed = self._attempt_node_specinfo_recompose(node)
        if recomposed is not False:
            return recomposed
        return super().node_standard_process_macro(node)

    def node_standard_process_environment(self, node):
        recomposed = self._attempt_node_specinfo_recompose(node)
        if recomposed is not False:
            return recomposed
        return super().node_standard_process_environment(node)

    def node_standard_process_specials(self, node):
        recomposed = self._attempt_node_specinfo_recompose(node)
        if recomposed is not False:
            return recomposed
        return super().node_standard_process_specials(node)

    def visit_unknown_node(self, node, **kwargs):
        recomposed = self._attempt_node_specinfo_recompose(node, **kwargs)
        if recomposed is not False:
            return recomposed
        return super().visit_unknown_node(node, **kwargs)

