r"""
Helper module to gather a collection of standard features, without enabling
all the on-demand module loading mechanism.  (E.g., this module is
Transcrypt-friendly, but `flm.run` is probably not.)
"""

from .feature.baseformatting import FeatureBaseFormatting
from .feature.href import FeatureHref
from .feature.verbatim import FeatureVerbatim
from .feature.math import FeatureMath
from .feature.endnotes import FeatureEndnotes, EndnoteCategory
from .feature.enumeration import FeatureEnumeration
from .feature.cite import FeatureExternalPrefixedCitations
from .feature.refs import FeatureRefs
from .feature.headings import FeatureHeadings
from .feature.floats import FeatureFloats
from .feature.graphics import FeatureSimplePathGraphicsResourceProvider
from .feature.defterm import FeatureDefTerm
from .feature.substmacros import FeatureSubstMacros
from .feature.quote import FeatureQuote

from .feature.theorems import FeatureTheorems


def standard_features(
        *,
        baseformatting=True,
        href=True,
        verbatim=True,
        math=True,
        headings=True,
        heading_section_commands_by_level=None,
        heading_numbering_section_depth=None,
        heading_section_numbering_by_level=None,
        heading_counter_formatter=None,
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
        render_defterm_with_term=True,
        theorems=False,
        substmacros_definitions=None,
        quote_environments=False,
):
    r"""
    Build a standard set of features with reasonable defaults.

    By default, the following features are enabled:
    ``baseformatting``, ``href``, ``verbatim``, ``math``, ``headings``,
    ``refs``, ``enumeration``, ``endnotes``, ``floats``, ``graphics``,
    ``defterm``.

    The following are disabled by default but can be enabled:
    ``theorems`` (pass ``theorems=True`` or a dict of options),
    ``substmacros`` (pass ``substmacros_definitions={...}``),
    ``quote`` (pass ``quote_environments=True`` or a dict of options),
    ``citations`` (pass ``citations=True`` along with
    ``external_citations_providers``).

    Each feature can be disabled by setting its parameter to ``False``
    (e.g., ``headings=False``).

    :param baseformatting: Enable text formatting (``\emph``, ``\textbf``,
        etc.).
    :param href: Enable hyperlinks (``\href``, ``\url``).
    :param verbatim: Enable verbatim/code (``\verbcode``).
    :param math: Enable math mode (``\(...\)``, ``equation``, etc.).
    :param headings: Enable section headings.
    :param refs: Enable cross-references (``\ref``, ``\label``).
    :param enumeration_environments: Enable lists (``enumerate``,
        ``itemize``).
    :param endnotes: Enable footnotes.
    :param floats: Enable figures and tables.
    :param defterm: Enable definition terms.
    :param theorems: Enable theorem environments.
    :param eq_counter_formatter: Counter formatter for equations.
    :param footnote_counter_formatter: Counter formatter for footnotes
        (default: ``'alph'``).
    :returns: A list of :py:class:`~flm.feature.Feature` instances.
    """

    if footnote_counter_formatter is None:
        footnote_counter_formatter = 'alph'
    if citation_counter_formatter is None:
        citation_counter_formatter = 'arabic'

    features = []

    if baseformatting:
        features.append(
            FeatureBaseFormatting()
        )

    if href:
        features.append(
            FeatureHref()
        )

    if verbatim:
        features.append(
            FeatureVerbatim()
        )

    if math:
        features.append(
            FeatureMath(
                counter_formatter=eq_counter_formatter,
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
                numbering_section_depth=heading_numbering_section_depth,
                section_numbering_by_level=heading_section_numbering_by_level,
                counter_formatter=heading_counter_formatter,
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
            FeatureDefTerm(render_defterm_with_term=render_defterm_with_term)
        )

    if theorems:
        # The interface to define new theorems here is not great, if you really
        # need this then it's recommended to simply create the FeatureTheorems
        # instance directly
        features.append(
            FeatureTheorems(**(theorems if isinstance(theorems, dict) else {}))
        )

    if substmacros_definitions:
        features.append(
            FeatureSubstMacros(substmacros_definitions)
        )

    if quote_environments is not False:
        dargs = {}
        if isinstance(quote_environments, dict):
            dargs.update(quote_environments)
        features.append(
            FeatureQuote(**dargs)
        )

    return features



