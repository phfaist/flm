import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
from pylatexenc import macrospec
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc import latexwalker


from .llmenvironment import (
    LLMEnvironment,
    LLMParsingState,
    make_arg_spec
)
from .llmspecinfo import (
    LLMMacroSpec, LLMEnvironmentSpec, LLMSpecialsSpec,
    TextFormat, HrefHyperlink, Verbatim, MathEnvironment, MathEqref, Error,
    ParagraphBreak
)
from .llmdocument import LLMDocument

from .enumeration import Enumeration

from .feature_endnotes import FeatureEndnotes, EndnoteCategory
from .feature_cite import FeatureExternalPrefixedCitations
from .feature_refs import FeatureRefs


# ------------------------------------------------------------------------------


class LLMLatexWalkerParsingStateEventHandler(
        latexnodes.LatexWalkerParsingStateEventHandler
):

    def enter_math_mode(self, math_mode_delimiter=None, trigger_token=None):
        logger.debug("LLMWalkerEventsParsingStateDeltasProvider.enter_math_mode !")
        return macrospec.ParsingStateDeltaExtendLatexContextDb(
            set_attributes=dict(
                in_math_mode=True,
                math_mode_delimiter=math_mode_delimiter,
            ),
            extend_latex_context=dict(
                unknown_macro_spec=LLMMacroSpec(''),
                unknown_environment_spec=LLMEnvironmentSpec(''),
                unknown_specials_spec=LLMSpecialsSpec(''),
            )
        )

    def leave_math_mode(self, trigger_token=None):
        logger.debug("LLMWalkerEventsParsingStateDeltasProvider.leave_math_mode !")
        return macrospec.ParsingStateDeltaExtendLatexContextDb(
            set_attributes=dict(
                in_math_mode=False,
                math_mode_delimiter=None
            ),
            extend_latex_context=dict(
                unknown_macro_spec=None,
                unknown_environment_spec=None,
                unknown_specials_spec=None,
            )
        )



_parsing_state_event_handler = LLMLatexWalkerParsingStateEventHandler()


# ------------------------------------------------------------------------------

_single_text_arg = [
    make_arg_spec('{', argname='text',)
]



def standard_latex_context_db():
    r"""
    ............

    The returned instance is not frozen, so you can continue adding new
    definition categories etc.
    """

    lw_context = macrospec.LatexContextDb()

    lw_context.add_context_category(
        'base-formatting',
        macros=[
            LLMMacroSpec('textbackslash', '', llm_specinfo='\\'),
            LLMMacroSpec('%', '', llm_specinfo='%'),
            LLMMacroSpec('#', '', llm_specinfo='#'),
            LLMMacroSpec('&', '', llm_specinfo='&'), # will do &amp; automatically for HTML
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
                llm_specinfo=ParagraphBreak()
            ),
        ]
    )
    lw_context.add_context_category(
        'math-environments',
        macros=[
            LLMMacroSpec(
                'eqref',
                arguments_spec_list=[
                    make_arg_spec(
                        latexnodes_parsers.LatexCharsGroupParser(),
                        argname='ref_target',
                    ),
                ],
                llm_specinfo=MathEqref(),
            ),
        ],
        environments=[
            LLMEnvironmentSpec(
                math_environment_name,
                '',
                llm_specinfo=MathEnvironment(),
                body_parsing_state_delta=latexnodes.ParsingStateDeltaEnterMathMode(),
            )
            for math_environment_name in (
                    'equation',
                    'equation*',
                    'align',
                    'align*',
                    'gather',
                    'gather*',
                    'split',
                    'split*',
            )
        ],
    )
    lw_context.add_context_category(
        'enumeration',
        environments=[
            LLMEnvironmentSpec(
                'itemize',
                arguments_spec_list=[
                    make_arg_spec(
                        latexnodes_parsers.LatexCharsGroupParser(
                            delimiters=('[',']'),
                            optional=True
                        ),
                        argname='tag_template',
                    )
                ],
                llm_specinfo=Enumeration(annotations=['itemize']),
            ),
            LLMEnvironmentSpec(
                'enumerate',
                arguments_spec_list=[
                    make_arg_spec(
                        latexnodes_parsers.LatexCharsGroupParser(
                            delimiters=('[',']'),
                            optional=True
                        ),
                        argname='tag_template',
                    )
                ],
                llm_specinfo=Enumeration(annotations=['enumerate']),
            ),
        ]
    )
    lw_context.add_context_category(
        'href',
        macros=[
            LLMMacroSpec(
                'href',
                arguments_spec_list=[
                    make_arg_spec(
                        latexnodes_parsers.LatexDelimitedVerbatimParser( ('{','}') ),
                        argname='target_href',
                    ),
                    make_arg_spec(
                        '{',
                        argname='display_text',
                    )
                ],
                llm_specinfo=HrefHyperlink(),
            ),
            LLMMacroSpec(
                'url',
                arguments_spec_list=[
                    make_arg_spec(
                        latexnodes_parsers.LatexDelimitedVerbatimParser( ('{','}') ),
                        argname='target_href',
                    )
                ],
                llm_specinfo=HrefHyperlink(command_arguments=('target_href',)),
            ),
        ]
    )
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

    return lw_context



def standard_parsing_state(*,
                           force_block_level=None,
                           enable_comments=False,
                           dollar_inline_math_mode=False):
    r"""
    Return a `ParsingState` configured in a standard way for parsing LLM
    content.  E.g., we typically disable commands and $-math mode, unless you
    specify keyword arguments to override this behavior.

    .. note:

       The `latex_context` field of the returned object is `None`.  You should
       set it yourself to a suitable `LatexContextDb` instance.  See
       `standard_latex_context_db()` for sensible defaults.
    """

    forbidden_characters = ''
    if not dollar_inline_math_mode:
        forbidden_characters += '$'
    if not enable_comments:
        forbidden_characters += '%'

    latex_inline_math_delimiters = [ (r'\(', r'\)'), ]

    if dollar_inline_math_mode:
        latex_inline_math_delimiters.append( ('$', '$') )

    return LLMParsingState(
        is_block_level=force_block_level,
        latex_context=None,
        enable_comments=enable_comments,
        latex_inline_math_delimiters=latex_inline_math_delimiters,
        latex_display_math_delimiters=[ (r'\[', r'\]') ],
        forbidden_characters=forbidden_characters,
    )





def standard_features(
        *,
        external_citations_provider=None,
        external_ref_resolver=None,
):
    features = [
        FeatureEndnotes(categories=[
            EndnoteCategory('footnote', 'alph', 'footnote'),
            EndnoteCategory('citation', lambda n: '[{:d}]'.format(n), None),
        ])
    ]
    if external_citations_provider is not None:
        features.append(
            FeatureExternalPrefixedCitations(
                external_citations_provider=external_citations_provider
            )
        )
    features.append(
        FeatureRefs(
            external_ref_resolver=external_ref_resolver,
        )
    )
    return features






class LLMStandardEnvironment(LLMEnvironment):
    def __init__(self,
                 latex_context=None,
                 parsing_state=None,
                 features=None,
                 *,
                 enable_comments=None,
                 external_citations_provider=None,
                 external_ref_resolver=None,
                 **kwargs):

        if latex_context is None:
            latex_context = standard_latex_context_db()
        if parsing_state is None:
            parsing_state = standard_parsing_state(
                enable_comments=enable_comments,
            )
        if features is None:
            features = standard_features(
                external_citations_provider=external_citations_provider,
                external_ref_resolver=external_ref_resolver
            )

        super().__init__(
            latex_context=latex_context,
            parsing_state=parsing_state,
            features=features,
            **kwargs
        )


    parsing_state_event_handler = LLMLatexWalkerParsingStateEventHandler()

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
