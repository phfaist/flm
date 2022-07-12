import sys
import fileinput
import logging
from collections import namedtuple

from pylatexenc.latexnodes import LatexWalkerParseError

from . import llmstd
from . import fmthelpers

from .fragmentrenderer.text import TextFragmentRenderer
from .fragmentrenderer.html import HtmlFragmentRenderer

from .feature.endnotes import FeatureEndnotes, EndnoteCategory
from .feature.enumeration import FeatureEnumeration, default_enumeration_environments
from .feature.cite import FeatureExternalPrefixedCitations
from .feature.refs import FeatureRefs
from .feature.headings import FeatureHeadings
from .feature.floats import FeatureFloatsIncludeGraphicsOnly #, FeatureFloats
from .feature.graphics import FeatureSimplePathGraphicsResourceProvider
from .feature.defterm import FeatureDefTerm



LLMMainArguments = namedtuple('LLMMainArguments',
                              ['llm_content', 'files', 'config', 'format',
                               'suppress_final_newline', 'verbose'],
                              defaults=[None, None, None, 'html',
                                        False, False],
                              )

parsing_defaults = dict(
    enable_comments=False,
    dollar_inline_math_mode=False,
    force_block_level=None,
)

endnote_render_options = dict(
    include_headings_at_level=1,
    set_headings_target_ids=True,
    endnotes_heading_title=None,
    endnotes_heading_level=1,
)

# #footnote_counter_formatter = lambda n: f"[{fmthelpers.alphacounter(n)}]"
# #footnote_counter_formatter = 'fnsymbol'
# #footnote_counter_formatter = lambda n: f"[{fmthelpers.fnsymbolcounter(n)}]"
# footnote_counter_formatter = 'unicodesuperscript'
# #footnote_counter_formatter = lambda n: f"⁽{fmthelpers.unicodesuperscriptcounter(n)}⁾"

default_float_types = [
    dict(
        float_type='figure',
        float_caption_name='Fig.',
        counter_formatter='Roman',
    ),
    dict(
        float_type='table',
        float_caption_name='Tab.',
        counter_formatter='Roman',
    ),
]


default_config = dict(
    html=dict(
        parsing=parsing_defaults,
        fragment_renderer=dict(
            use_link_target_blank=False,
            html_blocks_joiner="",
            heading_tags_by_level=HtmlFragmentRenderer.heading_tags_by_level,
            inline_heading_add_space=True
        ),
        features=dict(
            headings=dict(
                heading_section_commands_by_level=None,
            ),
            enumeration=dict(
                enumeration_environments=default_enumeration_environments,
            ),
            endnotes=dict(
                endnote_categories=[
                    dict(
                        category_name='footnote',
                        counter_formatter='alph',
                        heading_title='Footnotes',
                        endnote_command='footnote',
                    )
                ],
                render_options=endnote_render_options,
            ),
            floats=dict(
                float_types=default_float_types,
            ),
            defterm=dict(),
        ),
    ),
    text=dict(
        parsing=parsing_defaults,
        fragment_renderer=dict(
            display_href_urls=True,
        ),
        features=dict(
            # set any of these keys to None to disable feature
            headings=dict(
                heading_section_commands_by_level=None
            ),
            enumeration=dict(
                enumeration_environments=default_enumeration_environments,
            ),
            endnotes=dict(
                endnote_categories=[
                    dict(
                        category_name='footnote',
                        counter_formatter='unicodesuperscript',
                        heading_title='Footnotes',
                        endnote_command='footnote',
                    )
                ],
                render_options=endnote_render_options,
            ),
            floats=dict(
                float_types=default_float_types,
            ),
            defterm=dict(),
        ),
    ),
)


def setup_features(features_config):

    features = []

    if features_config.get('headings', {}) is not None:
        features.append(
            FeatureHeadings(
                section_commands_by_level=
                    features_config.get('headings',{})
                    .get('heading_section_commands_by_level', None)
            )
        )

    if features_config.get('refs', {}) is not None:
        features.append(
            FeatureRefs(
                external_ref_resolver=None,
            )
        )

    if features_config.get('enumeration', {}) is not None:
        features.append(
            FeatureEnumeration(
                enumeration_environments=
                    features_config.get('enumeration', {})
                    .get('enumeration_environments', None)
            )
        )

    if features_config.get('endnotes', {}) is not None:
        features.append(
            FeatureEndnotes(
                categories=
                    features_config.get('endnotes',{}).get('endnote_categories', None)
            )
        )
    
    # feature_citations = FeatureExternalPrefixedCitations(
    #         external_citations_provider=???????,
    #         counter_formatter=citation_counter_formatter,
    #         citation_delimiters=citation_delimiters,
    #     )

    if features_config.get('floats', {}) is not None:
        features.append(
            FeatureFloatsIncludeGraphicsOnly(
                float_types=
                    features_config.get('floats',{}).get('float_types', None)
            )
        )
        
    if features_config.get('defterm', {}) is not None:

        features.append( FeatureDefTerm() )

    if features_config.get('simple_path_graphics_resource_provider', {}) is not None:

        features.append(
            FeatureSimplePathGraphicsResourceProvider()
        )

    return features

    


def runmain(args):

    # set up logging
    level = logging.INFO
    if args.verbose:
        level = logging.DEBUG
    logging.basicConfig(level=level)
    if args.verbose != 2:
        logging.getLogger('pylatexenc').setLevel(level=logging.INFO)

    # Set up the format & formatters

    if args.format == 'text':

        fragment_renderer = TextFragmentRenderer()

    elif args.format == 'html':

        fragment_renderer = HtmlFragmentRenderer()

    else:
        raise ValueError(f"Unknown format: ‘{args.format}’")


    config = args.config
    if config is None:
        config = default_config[args.format]

    for k, v in config.get('fragment_renderer',{}).items():
        setattr(fragment_renderer, k, v)

    # Set up the environment

    std_parsing_state = llmstd.standard_parsing_state(**config.get('parsing',{}))
    std_features = setup_features(config.get('features',{}))

    environ = llmstd.LLMStandardEnvironment(
        parsing_state=std_parsing_state,
        features=std_features,
    )

    # Get the LLM content

    llm_content = ''
    if args.llm_content:
        if args.files:
            raise ValueError(
                "You cannot specify both FILEs and --llm-content options. "
                "Type `llm --help` for more information."
            )
        llm_content = args.llm_content
    else:
        for line in fileinput.input(files=args.files):
            llm_content += line

    fragment = environ.make_fragment(
        llm_content,
        is_block_level=args.force_block_level,
        silent=True, # we'll report errors ourselves
    )
    
    doc = environ.make_document(fragment.render)

    #
    # Render the main document
    #
    result, render_context = doc.render(fragment_renderer)

    #
    # Render endnotes
    #
    endnotes_mgr = render_context.feature_render_manager('endnotes')
    if endnotes_mgr is not None:
        endnotes_result = endnotes_mgr.render_endnotes(
            **config.get('features',{}).get('endnotes',{}).get('render_options',{})
        )
        result = fragment_renderer.render_join_blocks([
            result,
            endnotes_result,
        ])

    sys.stdout.write(result)
    if not args.suppress_final_newline:
        sys.stdout.write("\n")
    return

