import sys
import fileinput
import json

import logging
logger = logging.getLogger(__name__)

from copy import copy
from collections import namedtuple

import importlib

import yaml
import frontmatter

from pylatexenc.latexnodes import LatexWalkerParseError

from . import llmstd
from . import fmthelpers

from .fragmentrenderer.text import TextFragmentRenderer
from .fragmentrenderer.html import HtmlFragmentRenderer
from .fragmentrenderer.latex import LatexFragmentRenderer

# from .feature.endnotes import FeatureEndnotes, EndnoteCategory
from .feature.enumeration import default_enumeration_environments
# from .feature.cite import FeatureExternalPrefixedCitations
# from .feature.refs import FeatureRefs
# from .feature.headings import FeatureHeadings
# from .feature.floats import FeatureFloatsIncludeGraphicsOnly #, FeatureFloats
# from .feature.graphics import FeatureSimplePathGraphicsResourceProvider
# from .feature.defterm import FeatureDefTerm



LLMMainArguments = namedtuple('LLMMainArguments',
                              ['llm_content', 'files', 'config', 'format', 'output',
                               'suppress_final_newline', 'verbose'],
                              defaults=[None, None, None, None, None,
                                        False, False],
                              )



class _PresetDefaults:
    def process_list_item(self, obj1, k, j, obj2k):
        logger.debug("defaults!")
        obj1[k][j:j+1] = obj2k
        return j+len(obj2k)

class _PresetFeatureConfig:
    def process_list_item(self, obj1, k, j, obj2k):
        logger.debug("feature-config, process_list_item, {obj1!r} {k!r} {j!r} {obj2k!r}")
        featurename = obj1[k][j].get('name', None)
        if featurename is None:
            raise ValueError(
                "feature-config $preset requires ‘name: <fully qualified feature class name>’"
            )
        newconfig = dict(obj1[k][j].get('config', {}))
        featurespecj0 = next(
            (j0 for j0 in range(j)
             if obj1[k][j0].get('name','') == featurename) ,
            None
        )
        if featurespecj0 is None:
            raise ValueError(
                f"feature-config $preset -- could not find feature named ‘{featurename}’"
            )
        recursive_assign_defaults(newconfig, obj1[k][featurespecj0]['config'])
        obj1[k][featurespecj0]['config'] = newconfig # overwrite the one we had
        del obj1[k][j] # remove this instruction from our original list
        return j

_presets = {
    'defaults': _PresetDefaults(),
    'feature-config': _PresetFeatureConfig(),
};

def recursive_assign_defaults(obj1, obj2):
    for k in obj2:
        if k not in obj1:
            logger.debug(f"Setting obj1's ‘{k}’ property to {obj2[k]!r}")
            obj1[k] = copy(obj2[k])

        if isinstance(obj2[k], dict):
            # process any preset
            if '$preset' in obj2[k]:
                preset_name = obj2[k]['preset']
                _presets[preset_name].process_property(obj2, k, obj1)

        if isinstance(obj1[k], list):
            j = 0
            while j < len(obj1[k]):
                if isinstance(obj1[k][j], dict) and '$preset' in obj1[k][j]:
                    preset_name = obj1[k][j]['$preset']
                    j = _presets[preset_name].process_list_item(obj1, k, j, obj2[k])
                else:
                    j += 1
        
        if isinstance(obj2[k], dict):
            # recurse into sub-properties
            recursive_assign_defaults(obj1[k], obj2[k])



# #footnote_counter_formatter = lambda n: f"[{fmthelpers.alphacounter(n)}]"
# #footnote_counter_formatter = 'fnsymbol'
# #footnote_counter_formatter = lambda n: f"[{fmthelpers.fnsymbolcounter(n)}]"
# footnote_counter_formatter = 'unicodesuperscript'
# #footnote_counter_formatter = lambda n: f"⁽{fmthelpers.unicodesuperscriptcounter(n)}⁾"


default_config = dict(
    _base=dict(
        parsing=dict(
            enable_comments=False,
            dollar_inline_math_mode=False,
            force_block_level=None,
        ),
        features=[
            dict(
                name='llm.feature.headings.FeatureHeadings',
                config=dict(
                    section_commands_by_level=None,
                )
            ),
            dict(
                name='llm.feature.enumeration.FeatureEnumeration',
                config=dict(
                    enumeration_environments=default_enumeration_environments,
                )
            ),
            dict(
                name='llm.feature.refs.FeatureRefs',
                config=dict()
            ),
            dict(
                name='llm.feature.endnotes.FeatureEndnotes',
                config=dict(
                    categories=[
                        dict(
                            category_name='footnote',
                            counter_formatter='alph',
                            heading_title='Footnotes',
                            endnote_command='footnote',
                        )
                    ],
                    render_options=dict(
                        include_headings_at_level=1,
                        set_headings_target_ids=True,
                        endnotes_heading_title=None,
                        endnotes_heading_level=1,
                    )
                )
            ),
            dict(
                name='llm.feature.floats.FeatureFloatsIncludeGraphicsOnly',
                config=dict(
                    float_types=[
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
                )
            ),
            dict(
                name='llm.feature.defterm.FeatureDefTerm',
                config=dict(),
            ),
            dict(
                name='llm.feature.graphics.FeatureSimplePathGraphicsResourceProvider',
                config=dict(),
            )
        ]
    ),
    html=dict(
        fragment_renderer=dict(
            use_link_target_blank=False,
            html_blocks_joiner="",
            heading_tags_by_level=HtmlFragmentRenderer.heading_tags_by_level,
            inline_heading_add_space=True
        ),
    ),
    text=dict(
        fragment_renderer=dict(
            display_href_urls=True,
        ),
        features=[
            {
                '$preset': 'defaults'
            },
            {
                '$preset': 'feature-config',
                'name': 'llm.feature.endnotes.FeatureEndnotes',
                'config': dict(
                    categories=[
                        dict(
                            category_name='footnote',
                            counter_formatter='unicodesuperscript',
                            heading_title='Footnotes',
                            endnote_command='footnote',
                        )
                    ]
                ),
            },
        ],
    ),
    latex=dict(
    ),
)


def importclass(fullname):
    modname, classname = fullname.rsplit('.', maxsplit=1)
    mod = importlib.import_module(modname)
    return getattr(mod, classname)

    
_preset_fragment_renderer_classes = {
    'html': HtmlFragmentRenderer,
    'text': TextFragmentRenderer,
    'latex': LatexFragmentRenderer,
}


def get_fragment_renderer(argformat, args):
    r"""
    Should return {'fragment_renderer': <instance>, 'doc_pre': ..., 'doc_post': ... }
    """

    if argformat in _preset_fragment_renderer_classes:
        fragment_renderer = _preset_fragment_renderer_classes[argformat] ()
    elif '.' in argformat:
        FragmentRendererClass = importclass(argformat)
        fragment_renderer = FragmentRendererClass()
    else:
        raise ValueError(f"Unknown format: ‘{argformat}’")

    doc_pre, doc_post = ('','')

    if argformat == 'html' and args.html_minimal_document:
        doc_pre = _html_minimal_document_pre
        doc_post = _html_minimal_document_post

    get_doc_pre_post = getattr(fragment_renderer, 'get_doc_pre_post', None)
    if get_doc_pre_post is not None:
        doc_pre, doc_post = get_doc_pre_post(args)

    return {
        'fragment_renderer': fragment_renderer,
        'doc_pre': doc_pre,
        'doc_post': doc_post,
    }


def setup_features(features_config):

    features = []

    for featurespec in features_config:
        
        FeatureClass = importclass(featurespec['name'])
        features.append( FeatureClass(**featurespec.get('config', {})) )

    return features


def runmain(args):

    # set up logging
    level = logging.INFO
    if args.verbose:
        level = logging.DEBUG
    logging.basicConfig(level=level)
    if args.verbose != 2:
        logging.getLogger('pylatexenc').setLevel(level=logging.INFO)

    logger = logging.getLogger(__name__)


    # Get the LLM content

    input_content = ''
    if args.llm_content:
        if args.files:
            raise ValueError(
                "You cannot specify both FILEs and --llm-content options. "
                "Type `llm --help` for more information."
            )
        input_content = args.llm_content
    else:
        if len(args.files) >= 2:
            logger.warning("When multiple files are given, only the YAML front matter "
                           "for the first specified file is inspected.")
        for line in fileinput.input(files=args.files, encoding='utf-8'):
            input_content += line

    metadata, llm_content = frontmatter.parse(input_content)

    # load config & defaults

    orig_config = args.config
    if isinstance(orig_config, str):
        # parse a YAML file
        with open(orig_config) as f:
            orig_config = yaml.safe_load(f)
    if not orig_config:
        orig_config = {}

    logger.debug(f"Input metadata is\n{json.dumps(metadata,indent=4)}")

    config_chain = [
        metadata.get('llm', {}),
        orig_config,
        default_config.get(args.format, {}),
        default_config['_base'],
    ]

    config = {}
    for configdefaults in config_chain:
        recursive_assign_defaults(config, configdefaults)

    logger.debug(f"Using config:\n{json.dumps(config,indent=4)}")


    # Set up the format & formatters

    x = get_fragment_renderer(args.format, args)
    fragment_renderer = x['fragment_renderer']
    doc_pre = x['doc_pre']
    doc_post = x['doc_post']


    # Set up any fragment_renderer properties from config

    for k, v in config['fragment_renderer'].items():
        setattr(fragment_renderer, k, v)

    # Set up the environment

    std_parsing_state = llmstd.standard_parsing_state(**config['parsing'])
    std_features = setup_features(config['features'])

    environ = llmstd.LLMStandardEnvironment(
        parsing_state=std_parsing_state,
        features=std_features,
    )

    fragment = environ.make_fragment(
        llm_content,
        is_block_level=args.force_block_level,
        silent=True, # we'll report errors ourselves
    )
    
    doc = environ.make_document(fragment.render)
    
    # give access to metadata to render functions -- e.g., we might want to put
    # keys like "title:", "date:", "author:", etc. in the YAML meta-data to be
    # used in the LLM content.  Or a bibliography manager might want a bibfile
    # where to look for references, etc.
    doc.metadata = metadata

    #
    # Render the main document
    #
    result, render_context = doc.render(fragment_renderer)

    #
    # Render endnotes
    #
    endnotes_mgr = render_context.feature_render_manager('endnotes')
    if endnotes_mgr is not None:
        # find endnotes feature config
        endnotes_feature_spec = next(
            spec for spec in config['features']
            if spec['name'] == 'llm.feature.endnotes.FeatureEndnotes'
        )
        
        endnotes_result = endnotes_mgr.render_endnotes()
        result = fragment_renderer.render_join_blocks([
            result,
            endnotes_result,
        ])

    #
    # Write to output
    #
    def open_context_fout():
        if not args.output or args.output == '-':
            return _TrivialContextManager(sys.stdout)
        else:
            return open(args.output, 'w')

    with open_context_fout() as fout:

        if doc_pre:
            fout.write(doc_pre)

        fout.write(result)

        if doc_post:
            fout.write(doc_post)

        if not args.suppress_final_newline:
            fout.write("\n")

    return


class _TrivialContextManager:
    def __init__(self, value):
        super().__init__()
        self.value = value

    def __enter__(self):
        return self.value

    def __exit__(*args):
        pass


_html_minimal_document_pre = r"""
<!doctype html>
<html>
<head>
  <title>LLM Document</title>
  <style type="text/css">
/* ------------------ */
html, body {
  font-size: 16px;
  line-height: 1.3em;
}

article {
  max-width: 640px;
  margin: 0px auto;
}

p, ul, ol {
  margin: 1em 0px;
}
p:first-child, ul:first-child, ol:first-child {
  margin-top: 0px;
}
p:last-child, ul:last-child, ol:last-child {
  margin-bottom: 0px;
}

a, a:link, a:hover, a:active, a:visited {
  color: #3232c8;
  text-decoration: none;
}
a:hover {
  color: #22228a;
}

.emph, .textit {
  font-style: italic;
}
.textbf {
  font-weight: bold;
}

h1 {
  font-size: 1.6rem;
  font-weight: bold;
  margin: 1em 0px;
}
h2 {
  font-size: 1.3rem;
  font-weight: bold;
  margin: 1em 0px;
}
h3 {
  font-size: 1rem;
  font-weight: bold;
  margin: 1em 0px;
}

.heading-level-4 {
  font-style: italic;
  display: inline;
}
.heading-level-4::after {
  display: inline-block;
  margin: 0px .12em;
  content: '—';
}

.heading-level-5 {
  font-style: italic;
  font-size: .9em;
  display: inline;
}
.heading-level-5::after {
  display: inline-block;
  margin-right: .12em;
  content: '';
}

.heading-level-6 {
  font-style: italic;
  font-size: .8em;
  display: inline;
}
.heading-level-6::after {
  display: inline-block;
  margin-right: .06em;
  content: '';
}

dl.enumeration {
  display: grid;
  grid-template-columns: 0fr 1fr;
  gap: 0.5em;
}
dl.enumeration > dt {
  grid-column-start: 1;
  grid-column-end: 2;
  text-align: right;
  margin: 0px;
}
dl.enumeration > dd {
  grid-column-start: 2;
  grid-column-end: 3;
  margin: 0px;
}

figure.float {
  width: 100%;
  border-width: 1px 0px 1px 0px;
  border-style: solid none solid none;
  border-color: rgba(120, 120, 140, 0.15);
  margin: 0.5rem 0px;
  padding: 0.5rem 0px;
}

figure.float .float-contents {
  width: 100%;
  max-width: 100%;
  overflow-x: auto;
}

figure.float .float-contents img {
  display: block;
  margin: 0pt;
  padding: 0pt;
  border: 0pt;
  margin: 0px auto;
}

figure.float figcaption {
  display: block;
  margin-top: 0.5em;
  margin: 0.75em 2em 0px;
  text-align: center;
}

figure.float figcaption > span {
  display: inline-block;
  font-style: italic;
  text-align: left;
}

.defterm {
  font-style: italic;
}

.defterm .defterm-term {
  font-style: italic;
  font-weight: bold;
}

.display-math {
  width: 100%;
  max-width: 100%;
  display: block;
  overflow-x: auto;
}

.citation {
  font-size: 0.8em;
  display: inline-block;
  vertical-align: 0.3em;
  margin-top: -0.3em;
}
.footnote {
  font-size: 0.8em;
  display: inline-block;
  vertical-align: 0.3em;
  margin-top: -0.3em;
}
dl.citation-list > dt, dl.footnote-list > dt {
  font-size: 0.8em;
  display: inline-block;
  vertical-align: 0.3em;
  margin-top: -0.3em;
}
/* ------------------ */
  </style>
  <script>
MathJax = {
    tex: {
        inlineMath: [['\\(', '\\)']],
        displayMath: [['\\[', '\\]']],
        processEnvironments: true,
        processRefs: true,

        // equation numbering on
        tags: 'ams'
    },
    options: {
        // all MathJax content is marked with CSS classes
        // skipHtmlTags: 'body',
        // processHtmlClass: 'display-math|inline-math',
    },
    startup: {
        pageReady: function() {
            // override the default "typeset everything on the page" behavior to
            // only typeset whatever we have explicitly marked as math
            return typesetPageMathPromise();
        }
    }
};
function typesetPageMathPromise()
{
    var elements = document.querySelectorAll('.display-math, .inline-math');
    return MathJax.typesetPromise(elements);
}
  </script>
  <script src="https://polyfill.io/v3/polyfill.min.js?features=es6"></script>
  <script id="MathJax-script" async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>
</head>
<body>
<article>
""".strip()

_html_minimal_document_post = r"""
</article>
</body>
</html>
""".strip()
