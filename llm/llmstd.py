from pylatexenc import macrospec


from .llmenvironment import (
    LLMEnvironment, LLMMacroSpec, LLMEnvironmentSpec, LLMSpecialsSpec,
    TextFormat, Verbatim, MathEnvironment
)


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



class LLMStandardEnvironment(LLMEnvironment):
    def __init__(self):
        latex_context_db = standard_latex_context_db()
        super().__init__(latex_context_db=latex_context_db)
