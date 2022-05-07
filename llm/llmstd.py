from pylatexenc import latexnodes
from pylatexenc import macrospec


from .llmenvironment import (
    LLMEnvironment, LLMMacroSpec, LLMEnvironmentSpec, LLMSpecialsSpec,
    TextFormat, Verbatim, MathEnvironment, Error
)
from .llmdocument import LLMDocument


_single_text_arg = [
    macrospec.LatexArgumentSpec('{', argname='text')
]

def standard_latex_context_db():

    lw_context = macrospec.LatexContextDb()

    lw_context.add_context_category(
        'base-formatting',
        macros=[
            LLMMacroSpec('textbackslash', '', llm_specinfo='\\'),
            LLMMacroSpec('%', '', llm_specinfo='%'),
            LLMMacroSpec('#', '', llm_specinfo='#'),
            LLMMacroSpec('&', '', llm_specinfo='&'), # escaped as &amp; automatically
            LLMMacroSpec('$', '', llm_specinfo='$'),
            LLMMacroSpec(' ', '', llm_specinfo=' '),
            LLMMacroSpec('{', '', llm_specinfo='{'),
            LLMMacroSpec('}', '', llm_specinfo='}'),

            LLMMacroSpec(
                'emph',
                _single_text_arg,
                llm_specinfo=TextFormat(text_formats=('textit',))
            ),
            LLMMacroSpec(
                'textit',
                _single_text_arg,
                llm_specinfo=TextFormat(text_formats=('textit',))
            ),
            LLMMacroSpec(
                'textbf',
                _single_text_arg,
                llm_specinfo=TextFormat(text_formats=('textbf',))
            ),
        ],
        specials=[
            LLMSpecialsSpec(
                '~',
                llm_specinfo=' '
            ),
            # new paragraph
            LLMSpecialsSpec(
                '\n\n',
                llm_specinfo=Error('Paragraph break is not allowed here')
            ),
        ]
    )
    lw_context.add_context_category(
        'math-environments',
        environments=[
            LLMEnvironmentSpec(
                math_environment_name,
                '',
                llm_specinfo=MathEnvironment()
            )
            for math_environment_name in (
                    'align',
                    'align*',
                    'gather',
                    'gather*',
                    'split',
                    'split*',
            )
        ]
    )
    # lw_context.add_context_category(
    #     'x-refs',
    #     macros=[
    #         LLMMacroSpec(
    #             'ref',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(LatexCharsGroupParser(), argname='reftarget'),
    #             ],
    #             item_to_html=ItemToHtmlRef()
    #         ),
    #         LLMMacroSpec(
    #             'eqref',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(LatexCharsGroupParser(), argname='reftarget'),
    #             ],
    #             item_to_html=ItemToHtmlEqRef()
    #         ),
    #         # \label{...} for equations
    #         LLMMacroSpec(
    #             'label',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(LatexCharsGroupParser(), argname='reftarget'),
    #             ],
    #             item_to_html=ItemToHtmlError()
    #         ),
    #         LLMMacroSpec(
    #             'cite',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(
    #                     '[',
    #                     argname='cite_pre_text'
    #                 ),
    #                 LatexArgumentSpec(
    #                     LatexCharsCommaSeparatedListParser(enable_comments=False),
    #                     argname='citekey'
    #                 ),
    #             ],
    #             item_to_html=ItemToHtmlCite()
    #         ),
    #         LLMMacroSpec(
    #             'footnote',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(
    #                     '{',
    #                     argname='footnotetext'
    #                 ),
    #             ],
    #             item_to_html=ItemToHtmlFootnote()
    #         ),
    #         LLMMacroSpec(
    #             'href',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(
    #                     LatexDelimitedVerbatimParser( ('{','}') ),
    #                     argname='url',
    #                 ),
    #                 LatexArgumentSpec(
    #                     '{',
    #                     argname='displaytext',
    #                 )
    #             ],
    #             item_to_html=ItemToHtmlHref()
    #         ),
    #         LLMMacroSpec(
    #             'hyperref',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(
    #                     '[',
    #                     argname='target'
    #                 ),
    #                 LatexArgumentSpec(
    #                     '{',
    #                     argname='displaytext'
    #                 ),
    #             ],
    #             item_to_html=ItemToHtmlHyperref()
    #         ),
    #         LLMMacroSpec(
    #             'url',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec(
    #                     LatexDelimitedVerbatimParser( ('{','}') ),
    #                     argname='url',
    #                 )
    #             ],
    #             item_to_html=ItemToHtmlUrl()
    #         ),
    #     ]
    # )
    # lw_context.add_context_category(
    #     'floats',
    #     macros=[
    #         LLMMacroSpec(
    #             'includegraphics',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec('[', 'options'),
    #                 LatexArgumentSpec(LatexCharsGroupParser(), 'filename'),
    #             ]
    #         ),
    #         LLMMacroSpec(
    #             'caption',
    #             arguments_spec_list=[
    #                 LatexArgumentSpec('[', 'shortcaptiontext'),
    #                 LatexArgumentSpec('{', 'captiontext'),
    #             ]
    #         ),
    #         # ### \label is already defined above (e.g. for equations)
    #         # LLMMacroSpec(
    #         #     'label',
    #         #     arguments_spec_list=[
    #         #         LatexArgumentSpec(LatexCharsGroupParser(), 'reftarget'),
    #         #     ]
    #         # ),
    #     ],
    #     environments=[
    #         LLMEnvironmentSpec(
    #             'figure',
    #             item_to_html=ItemToHtmlFloat('figure', 'Figure'),
    #         ),
    #         LLMEnvironmentSpec(
    #             'table',
    #             item_to_html=ItemToHtmlFloat('table', 'Table'),
    #         ),
    #     ],
    # )
    # lw_context.add_context_category(
    #     'verbatim-input',
    #     environments={
    #         LLMEnvironmentSpec(
    #             'verbatiminput',
    #             arguments_spec_list=[],
    #             body_parser=LatexVerbatimEnvironmentContentsParser(
    #                 environment_name='verbatiminput'
    #             ),
    #             item_to_html=ItemToHtmlVerbatimContentsWrapTag(
    #                 class_="verbatiminput",
    #                 is_environment=True,
    #             ),
    #         ),
    #     }
    # )

    # # ignore unknown macros -- TODO !! only ignore in math mode !!  (note
    # # unknown macro instances are still caught and reported at to-html time)
    # lw_context.set_unknown_macro_spec(LLMMacroSpec(''))
    # lw_context.set_unknown_environment_spec(LLMEnvironmentSpec(''))

    return lw_context



def standard_parsing_state(latex_context,
                           *,
                           enable_comments=False,
                           dollar_inline_math_mode=False):

    forbidden_characters = ''
    if not dollar_inline_math_mode:
        forbidden_characters += '$'
    if not enable_comments:
        forbidden_characters += '%'

    latex_inline_math_delimiters = [ (r'\(', r'\)'), ]

    if dollar_inline_math_mode:
        latex_inline_math_delimiters.append( ('$', '$') )

    return latexnodes.ParsingState(
        latex_context=latex_context,
        enable_comments=enable_comments,
        latex_inline_math_delimiters=latex_inline_math_delimiters,
        latex_display_math_delimiters=[ (r'\[', r'\]') ],
        forbidden_characters=forbidden_characters,
    )


class LLMStandardEnvironment(LLMEnvironment):
    def __init__(self, parsing_state=None, **kwargs):
        super().__init__(**kwargs)

        self.latex_context = None

        if parsing_state is None:
            latex_context = standard_latex_context_db()
            parsing_state = standard_parsing_state(latex_context)

        self.parsing_state = parsing_state

    def get_parsing_state(self):
        return self.parsing_state


    def make_document(self, render_callback, fragment_renderer):
        return LLMDocument(render_callback, self, fragment_renderer)


    def get_parse_error_message(self, exception_object):
        error_type_info = exception_object.error_type_info
        if error_type_info:
            what = error_type_info['what']
            if what == 'token_forbidden_character':
                if error_type_info['forbidden_character'] == '%':
                    return (
                        r"LaTeX comments are not allowed here. Use ‘\%’ to typeset a "
                        r"literal percent sign."
                    )
                if error_type_info['forbidden_character'] == '$':
                    return (
                        r"You can't use ‘$’ here. LaTeX math should be typeset using "
                        r"\(...\) for inline math and \[...\] for unnumbered display "
                        r"equations. Use ‘\$’ for a literal dollar sign."
                    )
        return str(exception_object)
