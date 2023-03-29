
from ..llmspecinfo import (
    ConstantValueMacro, TextFormatMacro, ConstantValueSpecials, ParagraphBreakSpecials
)

from ._base import SimpleLatexDefinitionsFeature



class FeatureBaseFormatting(SimpleLatexDefinitionsFeature):

    feature_name = 'baseformatting'

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
