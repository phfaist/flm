import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    ParsedArgumentsInfo,
    LatexWalkerParseError,
)
from pylatexenc.latexnodes import parsers as latexnodes_parsers

from .. import llmspecinfo

from ._base import Feature

from . import refs





class HeadingMacro(llmspecinfo.LLMMacroSpecBase):

    is_block_level = True

    allowed_in_standalone_mode = True

    # internal, used when truncating fragments to a certain number of characters
    # (see fragment.truncate_to())
    _llm_main_text_argument = 'text'

    allowed_ref_label_prefixes = ('sec', 'topic',)

    def __init__(self, macroname, *, heading_level=1, inline_heading=False):
        r"""
        Heading level is to be coordinated with fragment renderer and LLM
        environment/context commands; for example `heading_level=1..6` with
        commands ``\section`` ... ``\subsubparagraph``
        """
        super().__init__(
            macroname,
            arguments_spec_list=[ llmspecinfo.text_arg, llmspecinfo.label_arg ],
        )
        self.heading_level = heading_level
        self.inline_heading = inline_heading
        # reimplemented from llmspecinfo -
        self.is_block_heading = self.inline_heading

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('text', 'label') ,
        )

        node.llmarg_heading_content_nodelist = node_args['text'].get_content_nodelist()

        node.llmarg_labels = llmspecinfo.helper_collect_labels(
            node_args['label'],
            self.allowed_ref_label_prefixes
        )

        node.llm_referenceable_infos = [
            refs.ReferenceableInfo(
                formatted_ref_llm_text=node.llmarg_heading_content_nodelist,
                labels=node.llmarg_labels,
            )
        ]


    def render(self, node, render_context):

        headings_mgr = render_context.feature_render_manager('headings')
        target_id = node.llm_referenceable_infos[0].get_target_id()

        if target_id is None:
            target_id = headings_mgr.get_default_target_id(
                node.llmarg_labels,
                node.llmarg_heading_content_nodelist,
                node=node,
            )

        if render_context.supports_feature('refs') and render_context.is_first_pass:
            refs_mgr = render_context.feature_render_manager('refs')
            for llm_referenceable_info in node.llm_referenceable_infos:
                refs_mgr.register_reference_referenceable(
                    node=node,
                    referenceable_info=llm_referenceable_info,
                )

        return render_context.fragment_renderer.render_heading(
            node.llmarg_heading_content_nodelist,
            render_context=render_context,
            heading_level=self.heading_level,
            inline_heading=self.inline_heading,
            target_id=target_id
        )






class FeatureHeadings(Feature):
    r"""
    Add support for headings via LaTeX commands, including ``\section``,
    ``\subsection``, ``\subsubsection``, ``\paragraph``, etc.
    """

    feature_name = 'headings'

    feature_optional_dependencies = [ 'refs' ]

    class RenderManager(Feature.RenderManager):
        # the render manager will take care of generating render-context-unique
        # target id's for headers
        def initialize(self):
            self.target_id_counters = {}
            self.target_ids = {}

        def get_default_target_id(self, heading_labels, heading_content_nodelist, *, node):

            node_id = self.get_node_id(node)

            if node_id in self.target_ids:
                return self.target_ids[node_id]

            tgtid = self._generate_default_target_id(heading_labels, heading_content_nodelist)
            self.target_ids[node_id] = tgtid
            return tgtid

        def _generate_default_target_id(self, heading_labels, heading_content_nodelist):

            # "slugify" the heading nodelist
            tgtid = heading_content_nodelist.latex_verbatim().strip()
            tgtid = re.sub(r'[^A-Za-z0-9_-]+', '-', tgtid)
            tgtid = f"sec--{tgtid}"
            tgtid = tgtid[:32] # truncate label to 32 chars
            if tgtid in self.target_id_counters:
                self.target_id_counters[tgtid] += 1
                return f"{tgtid}-{self.target_id_counters[tgtid]}"

            self.target_id_counters[tgtid] = 1
            return tgtid


    class SectionCommandSpec:
        def __init__(self, cmdname, inline=False):
            super().__init__()
            self.cmdname = cmdname
            self.inline = inline
        def __repr__(self):
            return (
                f"{self.__class__.__name__}(cmdname={self.cmdname!r}, "
                f"inline={self.inline!r})"
            )

    def __init__(self, section_commands_by_level=None):
        super().__init__()
        if section_commands_by_level is None:
            section_commands_by_level = {
                1: self.SectionCommandSpec(r"section"),
                2: self.SectionCommandSpec(r"subsection"),
                3: self.SectionCommandSpec(r"subsubsection"),
                4: self.SectionCommandSpec(r"paragraph", inline=True),
                5: self.SectionCommandSpec(r"subparagraph", inline=True),
                6: self.SectionCommandSpec(r"subsubparagraph", inline=True),
            }

        def _mkspecobj(x):
            if isinstance(x, self.SectionCommandSpec):
                return x
            if isinstance(x, str):
                return self.SectionCommandSpec(x)
            return self.SectionCommandSpec(**x)

        # below, dict(...) seems to be needed to force a python-style dict
        # object when using Transcrypt.
        self.section_commands_by_level = {
            level: _mkspecobj(x)
            for level, x in dict(section_commands_by_level).items()
        }

    def add_latex_context_definitions(self):
        return dict(
            macros=[
                HeadingMacro(
                    macroname=sectioncmdspec.cmdname,
                    heading_level=level,
                    inline_heading=sectioncmdspec.inline,
                )
                for level, sectioncmdspec in self.section_commands_by_level.items()
            ]
        )



