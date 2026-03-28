r"""
Fragment renderers for producing FLM output in various formats.

Each renderer subclasses :py:class:`FragmentRenderer` and implements the
format-specific rendering methods.

Available renderers:

- :py:class:`~flm.fragmentrenderer.html.HtmlFragmentRenderer` --- HTML
- :py:class:`~flm.fragmentrenderer.text.TextFragmentRenderer` --- plain text
- :py:class:`~flm.fragmentrenderer.latex.LatexFragmentRenderer` --- LaTeX
- :py:class:`~flm.fragmentrenderer.markdown.MarkdownFragmentRenderer` --- Markdown
"""

from ._base import (
    FragmentRenderer,
)
