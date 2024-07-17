import re

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexNodesLatexRecomposer


_default_rx_escape_chars_text = re.compile('[$&#^_%]')



class FLMNodesFlmRecomposer(LatexNodesLatexRecomposer):
    r"""
    Class capable of recomposing FLM code that corresponds to the
    information stored in a node and its descendants.

    This class can also be used whenever we want to traverse the node tree and
    produce some string representation of the nodes.
    """

    recompose_specinfo_method = 'flm_text_recomposed'

    def flm_text_recompose(self, node):
        r"""
        Attempt to recompose FLM code corresponding to the given node or
        node list.  Returns a string.
        """
        return self.start(node)


    rx_escape_chars_text = _default_rx_escape_chars_text

    def escape_chars(self, chars, parsing_state):
        if parsing_state.in_math_mode:
            return chars
        return self.rx_escape_chars_text.sub(lambda m: '\\'+m.group(), chars)


    # ---

    def _attempt_node_specinfo_recompose(self, node, **kwargs):
        if hasattr(node, 'flm_specinfo') and \
           hasattr(node.flm_specinfo, self.recompose_specinfo_method):
            return getattr(node.flm_specinfo, self.recompose_specinfo_method)(
                recomposer=self,
                **kwargs
            )
        return False

    def recompose_chars(self, chars, n):
        if not chars:
            chars = '' # not None or other stuff
        chars = str(chars)
        return self.escape_chars(chars, n.parsing_state)


    def visit_macro_node(self, node, **kwargs):
        recomposed = self._attempt_node_specinfo_recompose(node, **kwargs)
        if recomposed is not False:
            return recomposed
        return super().visit_macro_node(node, **kwargs)

    def visit_environment_node(self, node, **kwargs):
        recomposed = self._attempt_node_specinfo_recompose(node, **kwargs)
        if recomposed is not False:
            return recomposed
        return super().visit_environment_node(node, **kwargs)

    def visit_specials_node(self, node, **kwargs):
        recomposed = self._attempt_node_specinfo_recompose(node, **kwargs)
        if recomposed is not False:
            return recomposed
        return super().visit_environment_node(node, **kwargs)

    def visit_unknown_node(self, node, **kwargs):
        recomposed = self._attempt_node_specinfo_recompose(node, **kwargs)
        if recomposed is not False:
            return recomposed
        return super().visit_unknown_node(node, **kwargs)

