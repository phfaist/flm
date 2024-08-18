
from ..flmspecinfo import (
    ConstantValueMacro, TextFormatMacro, ConstantValueSpecials, ParagraphBreakSpecials,
    FLMMacroSpecBase
)

from ._base import SimpleLatexDefinitionsFeature


class NoExtraSpaceAfterDotMacro(FLMMacroSpecBase):
    r"""
    Spec info class for the ``\@`` macro.  This macro should be placed
    immediately after a period that does not terminate a sentence, such as in
    initials or abbreviations, to avoid awkward spacing.  For instance, type
    ``Well, well, Mr.\@ Bond.  You look surprised to see me.``.
    """
    def render(self, node, render_context):
        if hasattr(render_context.fragment_renderer, 'latex_macro_no_extra_space_after_dot'):
            return render_context.fragment_renderer.latex_macro_no_extra_space_after_dot
        # otherwise, just ignore this macro.
        return ''


class FeatureBaseFormatting(SimpleLatexDefinitionsFeature):

    feature_name = 'baseformatting'
    feature_title = 'Basic formatting'

    feature_flm_doc = r"""
    You can produce basic formatting, including emphasis/italics and
    boldface using the following macros.  Several macros also provide a way to
    typeset literal characters that would otherwise have a special meaning in
    your FLM environment.

    Input accents, special characters, etc., directly as Unicode:
    \verbcode+éàààé😅Á+. Note that source files should always be encoded using
    the UTF-8 encoding. You can use pretty quotes \verbcode+‘+ \verbcode+’+
    \verbcode+“+ \verbcode+”+; dashes \verbcode|—| (em dash), \verbcode|–| (en
    dash, for ranges); spaces \verbcode| | (non-breaking space), \verbcode| |
    (em space), \verbcode| | (thin space), etc.
    """

    latex_definitions = {
        'macros': [
            ConstantValueMacro('textbackslash', value='\\'),
            ConstantValueMacro('%', value='%'),
            ConstantValueMacro('#', value='#'),
            ConstantValueMacro('&', value='&'), # will do &amp; automatically for HTML
            ConstantValueMacro('$', value='$'),
            ConstantValueMacro(' ', value=' '),
            ConstantValueMacro('{', value='{'),
            ConstantValueMacro('}', value='}'),

            TextFormatMacro('emph', text_formats=('textit',)),

            TextFormatMacro(
                'textit',
                text_formats=('textit',),
            ),
            TextFormatMacro(
                'textbf',
                text_formats=('textbf',),
            ),

            NoExtraSpaceAfterDotMacro('@'), # \@ macro
        ],
        'specials': [
            ConstantValueSpecials(
                '~',
                value=' '
            ),
            # new paragraph
            ParagraphBreakSpecials(
                '\n\n',
            ),
        ]
    }




FeatureClass = FeatureBaseFormatting
