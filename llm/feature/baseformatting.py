
from ..llmspecinfo import (
    ConstantValueMacro, TextFormatMacro, ConstantValueSpecials, ParagraphBreakSpecials
)

from ._base import SimpleLatexDefinitionsFeature



class FeatureBaseFormatting(SimpleLatexDefinitionsFeature):

    feature_name = 'baseformatting'
    feature_title = 'Basic formatting'

    feature_llm_doc = r"""
    You can produce basic formatting, including emphasis/italics and
    boldface using the following macros.  Several macros also provide a way to
    typeset literal characters that would otherwise have a special meaning in
    your LLM environment.
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
