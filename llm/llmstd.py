import logging
logger = logging.getLogger(__name__)

from pylatexenc import latexnodes
from pylatexenc import macrospec

from .llmenvironment import (
    LLMEnvironment,
    LLMParsingState,
)
from .llmspecinfo import (
    ConstantValueMacro,
    ConstantValueSpecials,
    ParagraphBreakSpecials,
    TextFormatMacro,
    HrefHyperlinkMacro,
    VerbatimEnvironment,
)

from .feature.math import FeatureMath
from .feature.endnotes import FeatureEndnotes, EndnoteCategory
from .feature.enumeration import FeatureEnumeration
from .feature.cite import FeatureExternalPrefixedCitations
from .feature.refs import FeatureRefs
from .feature.headings import FeatureHeadings
from .feature.floats import FeatureFloats
from .feature.graphics import FeatureSimplePathGraphicsResourceProvider
from .feature.defterm import FeatureDefTerm

# ------------------------------------------------------------------------------


class LLMLatexWalkerParsingStateEventHandler(
        latexnodes.LatexWalkerParsingStateEventHandler
):

    def enter_math_mode(self, math_mode_delimiter=None, trigger_token=None):
        set_attributes = dict(
            in_math_mode=True,
            math_mode_delimiter=math_mode_delimiter,
        )
        logger.debug("LLMWalkerEventsParsingStateDeltasProvider.enter_math_mode ! "
                     "math_mode_delimiter=%r, trigger_token=%r, set_attributes=%r",
                     math_mode_delimiter, trigger_token, set_attributes)
        return macrospec.ParsingStateDeltaExtendLatexContextDb(
            set_attributes=set_attributes,
            extend_latex_context=dict(
                unknown_macro_spec=macrospec.MacroSpec(''),
                unknown_environment_spec=macrospec.EnvironmentSpec(''),
                unknown_specials_spec=macrospec.SpecialsSpec(''),
            )
        )

    def leave_math_mode(self, trigger_token=None):
        #logger.debug("LLMWalkerEventsParsingStateDeltasProvider.leave_math_mode !")
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



# _parsing_state_event_handler = LLMLatexWalkerParsingStateEventHandler()


# ------------------------------------------------------------------------------



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
        specials=[
            ConstantValueSpecials(
                '~',
                value=' '
            ),
            # new paragraph
            ParagraphBreakSpecials(
                '\n\n',
            ),
        ]
    )
    lw_context.add_context_category(
        'href',
        macros=[
            HrefHyperlinkMacro(
                macroname='href',
                command_arguments=('target_href', 'display_text',),
            ),
            HrefHyperlinkMacro(
                macroname='url',
                command_arguments=('target_href',),
            ),
        ]
    )
    lw_context.add_context_category(
        'verbatimtext',
        environments={
            VerbatimEnvironment(environmentname='verbatimtext'),
        }
    )

    return lw_context



def standard_parsing_state(*,
                           force_block_level=None,
                           enable_comments=True,
                           comment_start='%%',
                           extra_forbidden_characters='',
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

    forbidden_characters = str(extra_forbidden_characters)
    if not dollar_inline_math_mode and '$' not in forbidden_characters:
        forbidden_characters += '$'
    if (not enable_comments or comment_start != '%') and '%' not in forbidden_characters:
        # if comments are disabled entirely, we forbid the '%' sign completely.
        forbidden_characters += '%'

    latex_inline_math_delimiters = [ (r'\(', r'\)'), ]

    if dollar_inline_math_mode:
        latex_inline_math_delimiters.append( ('$', '$') )

    return LLMParsingState(
        is_block_level=force_block_level,
        latex_context=None,
        enable_comments=enable_comments,
        comment_start=comment_start,
        latex_inline_math_delimiters=latex_inline_math_delimiters,
        latex_display_math_delimiters=[ (r'\[', r'\]') ],
        forbidden_characters=forbidden_characters,
    )





def standard_features(
        *,
        math=True,
        headings=True,
        heading_section_commands_by_level=None,
        refs=True,
        external_ref_resolvers=None,
        enumeration_environments=True,
        enumeration_environments_dict=None,
        endnotes=True,
        citations=True,
        external_citations_providers=None,
        eq_counter_formatter=None,
        footnote_counter_formatter=None,
        citation_counter_formatter=None,
        use_simple_path_graphics_resource_provider=True,
        floats=True,
        float_types=None,
        defterm=True,
):

    if footnote_counter_formatter is None:
        footnote_counter_formatter = 'alph'
    if citation_counter_formatter is None:
        citation_counter_formatter = 'arabic'

    features = []

    if math:
        features.append(
            FeatureMath(
                eq_counter_formatter=eq_counter_formatter,
            )
        )

    if enumeration_environments:
        features.append(
            FeatureEnumeration(
                enumeration_environments=enumeration_environments_dict,
            )
        )

    if headings:
        features.append(
            FeatureHeadings(
                section_commands_by_level=heading_section_commands_by_level,
            )
        )

    if refs:
        features.append(
            FeatureRefs(
                external_ref_resolvers=external_ref_resolvers,
            )
        )

    if endnotes:
        endnote_categories = [
            EndnoteCategory(category_name='footnote',
                            counter_formatter=footnote_counter_formatter,
                            heading_title='Footnotes',
                            endnote_command='footnote'),
        ]
        features.append(
            FeatureEndnotes(categories=endnote_categories)
        )

    if citations and external_citations_providers is not None:
        features.append(
            FeatureExternalPrefixedCitations(
                external_citations_providers=external_citations_providers,
                counter_formatter=citation_counter_formatter,
                citation_delimiters=('[', ']'),
            )
        )

    if use_simple_path_graphics_resource_provider:
        features.append(
            FeatureSimplePathGraphicsResourceProvider()
        )


    if floats:
        features.append(
            FeatureFloats(float_types=float_types)
        )

    if defterm:
        features.append(
            FeatureDefTerm()
        )

    return features






class LLMStandardEnvironment(LLMEnvironment):
    def __init__(self,
                 features=None,
                 parsing_state=None,
                 latex_context=None,
                 **kwargs):

        if latex_context is None:
            latex_context = standard_latex_context_db()
        if parsing_state is None:
            parsing_state = standard_parsing_state(
                **{ k: kwargs.pop(k)
                    for k in ('enable_comments', 'comment_start',)
                    if k in kwargs }
            )
        if features is None:
            features = standard_features(
                **{ k: kwargs.pop(k)
                    for k in ('external_citations_providers',
                              'external_ref_resolvers',
                              'footnote_counter_formatter',
                              'citation_counter_formatter')
                    if k in kwargs }
            )

        super().__init__(
            features,
            parsing_state,
            latex_context,
            **kwargs
        )


    parsing_state_event_handler = LLMLatexWalkerParsingStateEventHandler()

    def get_parse_error_message(self, exception_object):
        msg = None
        error_type_info = exception_object.error_type_info
        if error_type_info:
            what = error_type_info['what']
            if what == 'token_forbidden_character':
                if error_type_info['forbidden_character'] == '%':
                    msg = (
                        r"LaTeX comments are not allowed here. Use ‘\%’ to typeset a "
                        r"literal percent sign."
                    )
                elif error_type_info['forbidden_character'] == '$':
                    msg = (
                        r"You can't use ‘$’ here. LaTeX math should be typeset using "
                        r"\(...\) for inline math and \[...\] for unnumbered display "
                        r"equations. Use ‘\$’ for a literal dollar sign."
                    )
        if not msg:
            msg = exception_object.msg

        errfmt = latexnodes.LatexWalkerParseErrorFormatter(exception_object)

        msg += errfmt.format_full_traceback()

        return msg
