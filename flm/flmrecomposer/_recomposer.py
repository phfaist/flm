
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import LatexNodesLatexRecomposer




class FLMNodesFlmRecomposer(LatexNodesLatexRecomposer):
    r"""
    Recompose FLM source text from a parsed node tree.

    Traverses a pylatexenc node tree and produces a string representation by
    visiting each node.  By default the output is round-trip FLM markup that
    closely matches the original source.  If a node's ``flm_specinfo`` object
    provides a method named by :py:attr:`recompose_specinfo_method` (default
    ``'recompose_flm_text'``), that method is called to produce the text for
    the node; otherwise the base-class default recomposition is used.

    Subclasses (e.g. :py:class:`~flm.flmrecomposer.purelatex.FLMPureLatexRecomposer`)
    can override :py:attr:`recompose_specinfo_method` and
    :py:attr:`rx_escape_chars_text` to target a different output dialect.
    """

    recompose_specinfo_method = 'recompose_flm_text'

    def recompose_flm_text(self, node):
        r"""
        Recompose FLM markup from the given node or node list.

        This is the main entry point.  Internally delegates to
        :py:meth:`start`, which walks the node tree via the visitor pattern.

        :param node: A pylatexenc node or node list.
        :return: The recomposed FLM source text.
        :rtype: str
        """
        return self.start(node)


    rx_escape_chars_text = None

    recompose_escape_chars_if_specials_disabled = False

    def escape_chars(self, chars, parsing_state):
        r"""
        Escape special characters in a text chunk according to the current
        parsing state.

        Characters matching :py:attr:`rx_escape_chars_text` are
        backslash-escaped, except when:

        - :py:attr:`rx_escape_chars_text` is ``None`` (no escaping configured),
        - the parsing state is in math mode, or
        - specials are disabled and
          :py:attr:`recompose_escape_chars_if_specials_disabled` is ``False``.

        :param str chars: Raw character content from a ``LatexCharsNode``.
        :param parsing_state: The node's parsing state.
        :return: The (possibly escaped) string.
        :rtype: str
        """
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
        Recompose a child node during a ``recompose_*`` callback.

        Call this from within an ``flm_specinfo.recompose_flm_text()`` (or
        analogous) method to recursively recompose a sub-node or node list
        using the same recomposer instance.

        :param node: A pylatexenc node or node list to recompose.
        :return: The recomposed string for the sub-tree.
        :rtype: str
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
        r"""
        Recompose a character-content node.

        Converts *chars* to a string (handling ``None``), then applies
        :py:meth:`escape_chars` using the node's parsing state.

        :param chars: The raw character content (may be ``None``).
        :param n: The ``LatexCharsNode`` that owns this content.
        :return: The escaped character string.
        :rtype: str
        """
        if not chars:
            chars = '' # not None or other stuff
        chars = str(chars)
        return self.escape_chars(chars, n.parsing_state)


    def node_standard_process_macro(self, node):
        r"""
        Process a macro node.  If the node's ``flm_specinfo`` provides a
        recompose method (named by :py:attr:`recompose_specinfo_method`),
        that method is used; otherwise falls back to the base-class
        recomposition.

        :param node: A ``LatexMacroNode``.
        :return: Recomposed string.
        :rtype: str
        """
        recomposed = self._attempt_node_specinfo_recompose(node)
        if recomposed is not False:
            return recomposed
        return super().node_standard_process_macro(node)

    def node_standard_process_environment(self, node):
        r"""
        Process an environment node.  If the node's ``flm_specinfo`` provides
        a recompose method (named by :py:attr:`recompose_specinfo_method`),
        that method is used; otherwise falls back to the base-class
        recomposition.

        :param node: A ``LatexEnvironmentNode``.
        :return: Recomposed string.
        :rtype: str
        """
        recomposed = self._attempt_node_specinfo_recompose(node)
        if recomposed is not False:
            return recomposed
        return super().node_standard_process_environment(node)

    def node_standard_process_specials(self, node):
        r"""
        Process a specials node.  If the node's ``flm_specinfo`` provides a
        recompose method (named by :py:attr:`recompose_specinfo_method`),
        that method is used; otherwise falls back to the base-class
        recomposition.

        :param node: A ``LatexSpecialsNode``.
        :return: Recomposed string.
        :rtype: str
        """
        recomposed = self._attempt_node_specinfo_recompose(node)
        if recomposed is not False:
            return recomposed
        return super().node_standard_process_specials(node)

    def visit_unknown_node(self, node, **kwargs):
        r"""
        Fallback visitor for node types not handled by the standard
        processing methods.  Attempts the ``flm_specinfo`` recompose method
        first; if unavailable, delegates to the base class.

        :param node: An unrecognized node.
        :return: Recomposed string.
        :rtype: str
        """
        recomposed = self._attempt_node_specinfo_recompose(node, **kwargs)
        if recomposed is not False:
            return recomposed
        return super().visit_unknown_node(node, **kwargs)

