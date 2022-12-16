import os.path
import sys
import re
import fileinput
import json
import string

import io

from dataclasses import dataclass

import logging
logger = logging.getLogger(__name__)

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

# --------------------------------------

from typing import Union, Optional

@dataclass
class LLMMainArguments:

    llm_content : Optional[str]

    force_block_level : Optional[bool] = None

    config : Union[str,dict,None] = None

    output : Union[str,io.TextIOBase,None] = None

    format : Optional[str] = None

    minimal_document : bool = True

    suppress_final_newline : bool = False

    verbose : Union[bool,int] = False

    files : Optional[list] = None


# --------------------------------------

from .configmerger import ConfigMerger, PresetDefaults


configmerger = ConfigMerger()


# #footnote_counter_formatter = lambda n: f"[{fmthelpers.alphacounter(n)}]"
# #footnote_counter_formatter = 'fnsymbol'
# #footnote_counter_formatter = lambda n: f"[{fmthelpers.fnsymbolcounter(n)}]"
# footnote_counter_formatter = 'unicodesuperscript'
# #footnote_counter_formatter = lambda n: f"⁽{fmthelpers.unicodesuperscriptcounter(n)}⁾"


default_config = dict(
    _base=dict(
        llm=dict(
            parsing=dict(
                enable_comments=True,
                comment_start='%%',
                dollar_inline_math_mode=False,
                force_block_level=None,
            ),
            fragment_renderer=dict(
            ),
            features=[
                dict(
                    name='llm.feature.math.FeatureMath',
                    config=dict()
                ),
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
                            ),
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
                    name='llm.feature.floats.FeatureFloats',
                    config=dict(
                        float_types=[
                            dict(
                                float_type='figure',
                                float_caption_name='Fig.',
                                counter_formatter='Roman',
                                content_handlers=['includegraphics'],
                            ),
                            dict(
                                float_type='table',
                                float_caption_name='Tab.',
                                counter_formatter='Roman',
                                content_handlers=['cells', 'includegraphics'],
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
            ],
            workflow=dict(),
        ),
    ),
    html=dict(
        llm=dict(
            fragment_renderer=dict(
                html=dict(
                    use_link_target_blank=False,
                    html_blocks_joiner="",
                    heading_tags_by_level=HtmlFragmentRenderer.heading_tags_by_level,
                    inline_heading_add_space=True,
                    render_nothing_as_comment_with_annotations=False,
                )
            ),
        )
    ),
    text=dict(
        llm=dict(
            fragment_renderer=dict(
                text=dict(
                    display_href_urls=True,
                )
            ),
            features=[
                {
                    '$defaults': True # value is not used for $defaults
                },
                {
                    '$merge-config': {
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
                },
            ],
        ),
    ),
    latex=dict(
        llm=dict(
            fragment_renderer=dict(
                latex=dict(
                    heading_commands_by_level = {
                        1: "section",
                        2: "subsection",
                        3: "subsubsection",
                        4: "paragraph",
                        5: "subparagraph",
                        6: None,
                    }
                ),
            ),
            features=[
                {
                    '$defaults': True
                },
                {
                    '$merge-config': {
                        'name': 'llm.feature.endnotes.FeatureEndnotes',
                        'config': dict(
                            categories=[
                                dict(
                                    category_name='footnote',
                                    counter_formatter={'template': "\\({}^{${arabic}}\\)"},
                                    heading_title='Footnotes',
                                    endnote_command='footnote',
                                )
                            ]
                        ),
                    },
                },
            ],
        ),
    ),
)


def importclass(fullname):
    modname, classname = fullname.rsplit('.', maxsplit=1)
    mod = importlib.import_module(modname)
    return getattr(mod, classname)

    
preset_fragment_renderer_classes = {
    'html': HtmlFragmentRenderer,
    'text': TextFragmentRenderer,
    'latex': LatexFragmentRenderer,
}


class RenderWorkflow:
    binary_output = False
    def __init__(self, args, config, fragment_renderer_class=None):
        self.args = args
        self.config = config if config else {}
        self._fragment_renderer_class = fragment_renderer_class

        for k, v in self.config.items():
            setattr(self, k, v)

    def get_fragment_renderer_class(self):
        return self._fragment_renderer_class

    def render_document(self, document, fragment_renderer):
        rendered_content, render_context = \
            self.render_document_fragments(document, fragment_renderer)
        final_content = self.postprocess_rendered_document(
            rendered_content, document, render_context
        )
        return final_content

    def render_document_fragments(self, document, fragment_renderer):
        # Render the main document
        rendered_result, render_context = document.render(fragment_renderer)

        # Render endnotes
        if render_context.supports_feature('endnotes'):
            endnotes_mgr = render_context.feature_render_manager('endnotes')
            endnotes_result = endnotes_mgr.render_endnotes()
            rendered_result = fragment_renderer.render_join_blocks([
                rendered_result,
                endnotes_result,
            ])

        return rendered_result, render_context

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        return rendered_content


class MinimalDocumentPostprocessor:
    def __init__(self, document, render_context, config):
        super().__init__()
        self.document = document
        self.render_context = render_context
        self.config = config if config else {}

    def postprocess(self, rendered_content, **kwargs):
        raise RuntimeError("Reimplement me")


class StandardTextBasedRenderWorkflow(RenderWorkflow):

    def __init__(self, args, format, config):
        super().__init__(args, config, preset_fragment_renderer_classes[format])
        self.format = format

    def postprocess_rendered_document(self, rendered_content, document, render_context):
        if not self.args.minimal_document:
            return rendered_content
        pp = self.get_minimal_document_postprocessor(document, render_context)
        if pp is None:
            # e.g., 'text' format has no minimal-document mode
            return rendered_content
        return pp.postprocess(rendered_content)

    def get_minimal_document_postprocessor(self, document, render_context):
        ppcls = _minimal_document_postprocessors.get(self.format, None)
        if ppcls is None:
            return None
        return ppcls(document, render_context, self.config)


def get_render_workflow(argformat, args, config):
    r"""
    Should return {'fragment_renderer': <instance>, 'doc_pre': ..., 'doc_post': ... }
    """

    workflow_config = config['llm']['workflow'].get(argformat, {})

    logger.debug("Using workflow config = %r", workflow_config)

    if argformat in preset_fragment_renderer_classes:
        return StandardTextBasedRenderWorkflow(args, argformat, workflow_config)
    elif '.' in argformat:
        WorkflowClass = importclass(argformat)
        return WorkflowClass(args, workflow_config)
    else:
        raise ValueError(f"Unknown format: ‘{argformat}’")



def setup_features(features_config):

    features = []

    for featurespec in features_config:
        
        FeatureClass = importclass(featurespec['name'])

        featureconfig = featurespec.get('config', {})
        if hasattr(FeatureClass, 'default_config'):
            defaultconfig = FeatureClass.default_config
            featureconfig = configmerger.recursive_assign_defaults(
                [featureconfig, defaultconfig]
            )

        features.append( FeatureClass(**featureconfig) )

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


    if args.format is None:
        raise ValueError(
            "No output format specified!"
        )

    logger.debug("Format is %r", args.format)

    # Get the LLM content

    input_content = ''
    dirname = None
    basename = None
    jobname = 'unknown-jobname'
    jobnameext = None
    if args.llm_content is not None:
        if args.files is not None and len(args.files):
            raise ValueError(
                "You cannot specify both FILEs and --llm-content options. "
                "Type `llm --help` for more information."
            )
        input_content = args.llm_content
    elif args.files is None:
        # doesn't happen on the command line because args.files is always a
        # list, possibly an empty one.  This trap is only useful for
        # programmatic invocation of runmain()
        raise ValueError(
            r"No input specified. Please use llm_content or specify input files."
        )
    else:
        if len(args.files) >= 1 and args.files[0] != '-':
            dirname, basename = os.path.split(args.files[0])
            jobname, jobnameext = os.path.splitext(basename)
        if len(args.files) >= 2:
            logger.warning("When multiple files are given, only the YAML front matter "
                           "for the first specified file is inspected.  The jobname is "
                           "set to the name of the first file.")
        for line in fileinput.input(files=args.files, encoding='utf-8'):
            input_content += line

    frontmatter_metadata, llm_content = frontmatter.parse(input_content)

    # compute line number offset (it doesn't look like I can grab this from the
    # `frontmatter` module's result :/
    rx_frontmatter = re.compile(r"^-{3,}\s*$\s*", re.MULTILINE) # \s also matches newline
    m = rx_frontmatter.search(input_content) # top separator
    if m is not None:
        m = rx_frontmatter.search(input_content, m.end()) # below the front matter
    line_number_offset = 0
    if m is not None:
        line_number_offset = input_content[:m.end()].count('\n') + 1

    # load config & defaults

    config_file = args.config
    orig_config = {}
    if isinstance(config_file, str) and config_file:
        # parse a YAML file
        with open(config_file) as f:
            orig_config = yaml.safe_load(f)
    elif isinstance(config_file, dict):
        orig_config = config_file
    else:
        # see if there's a llmconfig.(yaml|yml) in the current directory, and
        # load that one if applicable.
        if os.path.exists('llmconfig.yaml'):
            with open('llmconfig.yaml') as f:
                orig_config = yaml.safe_load(f)
        elif os.path.exists('llmconfig.yml'):
            with open('llmconfig.yml') as f:
                orig_config = yaml.safe_load(f)
        

    logger.debug(f"Input frontmatter_metadata is\n{json.dumps(frontmatter_metadata,indent=4)}")


    config = configmerger.recursive_assign_defaults([
        frontmatter_metadata,
        orig_config,
        default_config.get(args.format, {}),
        default_config['_base'],
    ])


    logger.debug(f"Using config:\n{json.dumps(config,indent=4)}")


    # Set up the format & formatters

    workflow = get_render_workflow(args.format, args, config)

    # fragment_renderer properties from config
    fragment_renderer_config = config['llm']['fragment_renderer'].get(args.format, {})

    FragmentRendererClass = workflow.get_fragment_renderer_class()
    fragment_renderer = FragmentRendererClass(config=fragment_renderer_config)


    # Set up the environment

    std_parsing_state = llmstd.standard_parsing_state(**config['llm']['parsing'])
    std_features = setup_features(config['llm']['features'])

    logger.debug(f"{std_parsing_state=}")

    environ = llmstd.LLMStandardEnvironment(
        parsing_state=std_parsing_state,
        features=std_features,
    )

    fragment = environ.make_fragment(
        llm_content,
        is_block_level=args.force_block_level,
        silent=True, # we'll report errors ourselves
        input_lineno_colno_offsets={
            'line_number_offset': line_number_offset,
        }
    )

    # give access to metadata to render functions -- e.g., we might want to put
    # keys like "title:", "date:", "author:", etc. in the YAML meta-data to be
    # used in the LLM content.  Or a bibliography manager might want a bibfile
    # where to look for references, etc.
    doc_metadata = {
        'filepath': {
            'dirname': dirname,
            'basename': basename,
            'jobnameext': jobnameext,
        },
        'jobname': jobname,
    }
    
    for k, v in config.items():
        if k not in ('llm', ):
            doc_metadata[k] = v

    doc_metadata['_config'] = config

    doc = environ.make_document(fragment.render, metadata=doc_metadata)
    
    #
    # Allow features prime access to the document and the fragment, in case they
    # want to scan stuff (e.g., for citations)
    #

    for feature_name, feature_document_manager in doc.feature_document_managers:
        if hasattr(feature_document_manager, 'llm_main_scan_fragment'):
            feature_document_manager.llm_main_scan_fragment(fragment)


    #
    # Render the document according to the workflow
    #

    result = workflow.render_document(doc, fragment_renderer)


    #
    # Write to output
    #
    def open_context_fout():
        if not args.output or args.output == '-':
            stream = sys.stdout
            if workflow.binary_output:
                stream = sys.stdout.buffer
            return _TrivialContextManager(stream)
        elif hasattr(args.output, 'write'):
            # it's a file-like object, use it directly
            return _TrivialContextManager(args.output)
        else:
            return open(args.output, 'w' + ('b' if workflow.binary_output else ''))

    with open_context_fout() as fout:

        fout.write(result)

        if not workflow.binary_output and not args.suppress_final_newline:
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


# ------------------------------------------------------------------------------

def _flatten_dict(d, joiner='.'):
    r = {}
    _flatten_dict_impl(r, d, [], joiner)
    return r

def _flatten_dict_impl(r, d, prefix, joiner):
    for k, v in d.items():
        p = prefix + [str(k)]
        if isinstance(v, dict):
            _flatten_dict_impl(r, v, p, joiner)
        else:
            r[ joiner.join(p) ] = v
            

class _Template(string.Template):
    braceidpattern = r'(?a:[_.a-z0-9-]+)'


class HtmlMinimalDocumentPostprocessor(MinimalDocumentPostprocessor):

    def postprocess(self, rendered_content):
        doc_pre, doc_post = self.get_pre_post()
        return ''.join([doc_pre, rendered_content, doc_post])

    def get_pre_post(self):

        config = self.config

        logger.debug("html minimal document post-processor, config is %r", config)

        metadata = self.document.metadata
        if metadata is None:
            metadata = {}
        else:
            metadata = {k: v for (k, v) in metadata.items() if k != "config"}

        logger.debug("html minimal document post-processor, metadata is %r", metadata)

        full_config = ConfigMerger().recursive_assign_defaults([
            config,
            {
                'metadata': metadata,
            },
            {
                'render_header': True,
                'metadata': { 'title': "", 'author': "", 'date': "" },
                'html': { 'extra_css': '', 'extra_js': '' },
                'style': {
                    'font_family': "Source Serif Pro",
                    'font_size': "16px",
                    #'default_font_families': "'Times New Roman', serif",
                    'default_font_families': "serif",
                }
            },
        ])

        logger.debug("html minimal document post-processor, full_config is %r", full_config)

        html_fragment_renderer = self.render_context.fragment_renderer

        full_config_style = full_config.get('style', {})

        css_global_page = _Template(
            _html_minimal_document_css_global_page_template 
        ).substitute(full_config_style)

        css = (
            '/* ======== */\n'
            + css_global_page
            + html_fragment_renderer.get_html_css_global()
            + html_fragment_renderer.get_html_css_content()
            + '/* ======== */\n'
        )
        if full_config['html']['extra_css']:
            css += full_config['html']['extra_css'] + '\n/* ======== */\n'

        js = html_fragment_renderer.get_html_js()
        if full_config['html']['extra_js']:
            js += '\n/* ======== */\n' + full_config['html']['extra_js'] + '\n/* ======== */\n'

        body_start_content = ""
        if full_config['render_header']:
            body_start_content_items = []
            if full_config['metadata']['title']:
                body_start_content_items.append(
                    f"<h1 class=\"header-title\">{full_config['metadata']['title']}</h1>"
                )
            if full_config['metadata']['author']:
                body_start_content_items.append(
                    f"<div role=\"doc-subtitle\" class=\"header-author\">"
                    f"{full_config['metadata']['author']}"
                    f"</div>"
                )
            if full_config['metadata']['date']:
                body_start_content_items.append(
                    f"<div role=\"doc-subtitle\" class=\"header-date\">"
                    f"{full_config['metadata']['date']}"
                    f"</div>"
                )
            if body_start_content_items:
                body_start_content += (
                    "<header>" + "".join(body_start_content_items) + "</header>"
                )

        full_config_w_htmltemplate = ConfigMerger().recursive_assign_defaults([
            full_config,
            {
                'html_template': {
                    'css': css,
                    'js': js,
                    'body_start_content': body_start_content,
                    'body_end_content': html_fragment_renderer.get_html_body_end_js_scripts(),
                },
            },
        ])

        flat_config = _flatten_dict(full_config_w_htmltemplate)

        html_pre = _Template(_html_minimal_document_pre_template).substitute(
            flat_config
        )

        html_post = _Template(_html_minimal_document_post_template).substitute(
            flat_config
        )

        return html_pre, html_post


_html_minimal_document_css_global_page_template = r"""
html, body {
  font-family: '${font_family}', ${default_font_families};
  font-size: ${font_size};
  line-height: 1.3em;
}

header, article {
  max-width: 640px;
  margin: 0px auto;
}
header {
  padding-bottom: 1em;
  border-bottom: 1px solid black;
  margin-bottom: 2em;
}
header div[role="doc-subtitle"] {
  margin-left: 2em;
  margin-top: 0.5em;
  font-size: 1.1rem;
  font-style: italic;
}
"""

_html_minimal_document_pre_template = r"""
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>${metadata.title}</title>
<style type="text/css">
/* ------------------ */
${html_template.css}
/* ------------------ */
</style>
<script type="text/javascript">
${html_template.js}
</script>
</head>
<body>
${html_template.body_start_content}
  <article>
""".strip()

_html_minimal_document_post_template = r"""
  </article>
${html_template.body_end_content}
</body>
</html>
""".strip()



# ------------------------------------------------------------------------------

_latex_minimal_document_pre = r"""\documentclass[11pt]{article}
\usepackage{phfnote}
\newenvironment{defterm}{%
  \par\begingroup\itshape
}{%
  \endgroup\par
}
\newcommand{\displayterm}[1]{\textbf{#1}}
\begin{document}
"""

_latex_minimal_document_post = r"""%
\end{document}
"""

class LatexMinimalDocumentPostprocessor(MinimalDocumentPostprocessor):

    doc_pre_post = (_latex_minimal_document_pre, _latex_minimal_document_post)

    def postprocess(self, rendered_content):
        doc_pre, doc_post = self.doc_pre_post
        return ''.join([doc_pre, rendered_content, doc_post])


# ------------------------------------------------------------------------------

_minimal_document_postprocessors = {
    'html': HtmlMinimalDocumentPostprocessor,
    'latex': LatexMinimalDocumentPostprocessor,
}
