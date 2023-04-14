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

from .feature.theorems import FeatureTheorems


def standard_features(
        *,
        baseformatting=True,
        href=True,
        verbatim=True,
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
        theorems=False,
):

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

    if theorems:
        # The interface to define new theorems here is not great, if you really
        # need this then it's recommended to simply create the FeatureTheorems
        # instance directly
        features.append(
            FeatureTheorems(**(theorems if isinstance(theorems, dict) else {}))
        )

    return features



