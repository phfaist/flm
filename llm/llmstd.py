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

from .enumeration import Enumeration
from .math import MathEnvironment, MathEqrefViaMathContent

from .feature.endnotes import FeatureEndnotes, EndnoteCategory
from .feature.cite import FeatureExternalPrefixedCitations
from .feature.refs import FeatureRefs
from .feature.headings import FeatureHeadings
from .feature.floats import FeatureFloatsIncludeGraphicsOnly #, FeatureFloats
from .feature.graphics import FeatureSimplePathGraphicsResourceProvider
from .feature.defterm import FeatureDefTerm

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
                unknown_macro_spec=macrospec.MacroSpec(''),
                unknown_environment_spec=macrospec.EnvironmentSpec(''),
                unknown_specials_spec=macrospec.SpecialsSpec(''),
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
        'math-environments',
        environments=[
            MathEnvironment(
                math_environment_name,
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
        'math-eqref-via-math-content', # e.g., for use with MathJax
        macros=[
            MathEqrefViaMathContent(
                macroname='eqref',
            ),
        ],
    )
    lw_context.add_context_category(
        'enumeration',
        environments=[
            Enumeration(
                environmentname='itemize',
                counter_formatter='•',
                annotations=['itemize'],
            ),
            Enumeration(
                environmentname='enumerate',
                annotations=['enumerate'],
            ),
        ],
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
        headings=True,
        heading_section_commands_by_level=None,
        refs=True,
        external_ref_resolver=None,
        endnotes=True,
        citations=True,
        external_citations_provider=None,
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
    if headings:
        features.append(
            FeatureHeadings(
                section_commands_by_level=heading_section_commands_by_level,
            )
        )
    if refs:
        features.append(
            FeatureRefs(
                external_ref_resolver=external_ref_resolver,
            )
        )
    if endnotes:
        endnote_categories = [
            EndnoteCategory('footnote', footnote_counter_formatter, 'footnote'),
        ]
        features.append(
            FeatureEndnotes(categories=endnote_categories)
        )
    if citations and external_citations_provider is not None:
        features.append(
            FeatureExternalPrefixedCitations(
                external_citations_provider=external_citations_provider,
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
            FeatureFloatsIncludeGraphicsOnly(float_types=float_types)
        )
    if defterm:
        features.append(
            FeatureDefTerm()
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
                 footnote_counter_formatter=None,
                 citation_counter_formatter=None,
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
                external_ref_resolver=external_ref_resolver,
                footnote_counter_formatter=footnote_counter_formatter,
                citation_counter_formatter=citation_counter_formatter,
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
