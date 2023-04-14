
from ..flmspecinfo import (
    ConstantValueMacro, TextFormatMacro, ConstantValueSpecials, ParagraphBreakSpecials
)

from ._base import SimpleLatexDefinitionsFeature



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
