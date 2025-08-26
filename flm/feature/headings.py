import re
import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    LatexArgumentSpec, ParsedArgumentsInfo,
)

from .. import flmspecinfo
from ..counter import build_counter_formatter

from ._base import Feature

from . import refs
from . import numbering




class HeadingMacro(flmspecinfo.FLMMacroSpecBase):

    is_block_level = True

    allowed_in_standalone_mode = True

    # internal, used when truncating fragments to a certain number of characters
    # (see fragment.truncate_to())
    _flm_main_text_argument = 'text'

    allowed_ref_label_prefixes = ('sec', 'topic',)

    def __init__(self, macroname, *, heading_level=1, inline_heading=False):
        r"""
        Heading level is to be coordinated with fragment renderer and FLM
        environment/context commands; for example `heading_level=1..6` with
        commands ``\section`` ... ``\subsubparagraph``
        """
        super().__init__(
            macroname,
            arguments_spec_list=[
                LatexArgumentSpec('*', argname='star'),
                flmspecinfo.text_arg,
                flmspecinfo.label_arg
            ],
        )
        self.heading_level = heading_level
        self.inline_heading = inline_heading
        # reimplemented from flmspecinfo -
        self.is_block_heading = self.inline_heading

    _fields = ('macroname', 'heading_level', 'inline_heading', )

    def get_flm_doc(self):
        return (
            f"Create a{ 'n inline' if self.inline_heading else '' } heading at "
            f"level {self.heading_level}"
        )

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('star', 'text', 'label') ,
        )

        node.flmarg_heading_content_nodelist = node_args['text'].get_content_nodelist()

        node.flmarg_labels = flmspecinfo.helper_collect_labels(
            node_args['label'],
            self.allowed_ref_label_prefixes
        )

        heading_flm_text = node.flmarg_heading_content_nodelist.latex_verbatim()

        node.flm_referenceable_infos = [
            refs.ReferenceableInfo(
                kind='heading',
                formatted_ref_flm_text=heading_flm_text,
                labels=node.flmarg_labels,
            )
        ]


    def render(self, node, render_context):

        headings_mgr = render_context.feature_render_manager('headings')

        heading_info = headings_mgr.new_heading(
            node=node,
            heading_level=self.heading_level,
            labels=node.flmarg_labels,
            heading_content_nodelist=node.flmarg_heading_content_nodelist,
        )

        return render_context.fragment_renderer.render_heading(
            heading_info['content_nodelist'],
            render_context=render_context,
            heading_level=self.heading_level,
            inline_heading=self.inline_heading,
            target_id=heading_info['target_id'],
        )


    def recompose_pure_latex(self, node, recomposer):

        heading_macroname = node.macroname

        recopt_cells = recomposer.get_options('headings')
        heading_macroname_mapping = recopt_cells.get('macroname_mapping', None)
        if heading_macroname_mapping:
            if heading_macroname in heading_macroname_mapping:
                heading_macroname = heading_macroname_mapping[heading_macroname]

        s = '\\' + heading_macroname

        # arguments_spec_list=[
        #     LatexArgumentSpec('*', argname='star'),
        #     flmspecinfo.text_arg,
        #     flmspecinfo.label_arg
        # ],
        
        if node.nodeargd is not None:

            if node.nodeargd.argnlist[0] is not None:
                s += recomposer.subrecompose( node.nodeargd.argnlist[0] ) # star

            if node.nodeargd.argnlist[1] is not None:
                s += recomposer.subrecompose( node.nodeargd.argnlist[1] ) # text argument

        # add labels
        for ref_type, ref_label in node.flmarg_labels:
            safe_label_info = recomposer.make_safe_label(
                'ref', ref_type, ref_label, node.latex_walker.resource_info
            )
            s += r'\label{' + safe_label_info['safe_label'] + '}'

        return s


sec_default_counter_formatter_spec = {
    'format_num': { 'template': '${arabic}' },
    'prefix_display': {
        'singular': '§ ',
        'plural': '§ ',
    },
    'delimiters': ('',''),
    'join_spec': 'compact',
}


class FeatureHeadings(Feature):
    r"""
    Add support for headings via LaTeX commands, including ``\section``,
    ``\subsection``, ``\subsubsection``, ``\paragraph``, etc.
    """

    feature_name = 'headings'
    feature_title = 'Headings: sections, paragraphs'

    feature_optional_dependencies = [ 'refs', 'numbering', ]

    class RenderManager(Feature.RenderManager):
        # the render manager will take care of generating render-context-unique
        # target id's for headers
        def initialize(
                self,
                numbering_section_depth=None,
                section_numbering_by_level=None,
                counter_formatter=None,
        ):
            self.target_id_counters = {}
            self.target_ids = {}

            if numbering_section_depth is not None:
                self.numbering_section_depth = numbering_section_depth
            else:
                self.numbering_section_depth = self.feature.numbering_section_depth

            if section_numbering_by_level is not None:
                self.section_numbering_by_level = {
                    level: self.feature._make_section_numbering_info(x)
                    for level, x in dict(section_numbering_by_level).items()
                }
            else:
                self.section_numbering_by_level = self.feature.section_numbering_by_level

            if counter_formatter is not None:
                self.counter_formatter = build_counter_formatter(
                    counter_formatter,
                    sec_default_counter_formatter_spec,
                    counter_formatter_id='section',
                )
            else:
                self.counter_formatter = self.feature.counter_formatter

            logger.debug(
                "Initialize FeatureHeadings.RenderManager; using:\n"
                "    numbering_section_depth=%r\n"
                "    section_commands_by_level=%r\n"
                "    section_numbering_by_level=%r\n"
                "    counter_formatter=%r",
                self.numbering_section_depth, self.feature.section_commands_by_level,
                self.section_numbering_by_level, self.counter_formatter)

            self.section_counter_ifaces = {}
            last_counter_name = None
            if self.numbering_section_depth is not False:
                for j in sorted(self.section_numbering_by_level.keys(), key=int):
                    if (self.numbering_section_depth is not True
                        and j > self.numbering_section_depth):
                        break
                    counter_name = self.feature.section_commands_by_level[j].cmdname
                    numbering_info = self.section_numbering_by_level[j]
                    if not numbering_info:
                        # non-numbered part/chapter/section. Can still contain
                        # numbered sub-headings which will be numbered
                        # throughout the current section level.  (E.g. this can
                        # be a nonnumbered \part with \chapters numbered
                        # throughout.)
                        continue
                    always_number_within = None
                    number_within_reset_at = numbering_info.number_within_reset_at
                    if number_within_reset_at:
                        if number_within_reset_at is True:
                            if last_counter_name is not None:
                                number_within_reset_at = last_counter_name
                            else:
                                number_within_reset_at = None
                        always_number_within = {
                            'reset_at': number_within_reset_at,
                            'numprefix': numbering_info.numprefix,
                        }
                    counter_iface = numbering.get_document_render_counter(
                        self.render_context, counter_name, self.counter_formatter,
                        always_number_within=always_number_within
                    )
                    self.section_counter_ifaces[j] = counter_iface
                    last_counter_name = counter_name
            
                logger.debug(
                    "Set up counter interfaces: %r",
                    self.section_counter_ifaces
                )

        def new_heading(self, node, heading_level,
                        labels, heading_content_nodelist, target_id=None):

            if target_id is None:
                if hasattr(node, 'flm_heading_target_id'):
                    # used to set the target_id when an internally generated heading is
                    # needed and a HeadingMacro macro instance is internally created
                    # (e.g., for theorems)
                    target_id = node.flm_heading_target_id
                elif len(node.flm_referenceable_infos):
                    target_id = node.flm_referenceable_infos[0].get_target_id()

            if target_id is None:
                target_id = self.get_default_target_id(
                    labels,
                    heading_content_nodelist,
                    node=node,
                )

            refs_mgr = None
            if self.render_context.supports_feature('refs') \
               and self.render_context.is_first_pass:
                refs_mgr = self.render_context.feature_render_manager('refs')

            # get section number, if applicable.
            sec_num_info = None
            if heading_level in self.section_counter_ifaces:
                counter_iface = self.section_counter_ifaces[heading_level]
                numbering_info = self.section_numbering_by_level[heading_level]

                sec_num_info = counter_iface.register_item()

                heading_joiner = numbering_info.heading_joiner

                heading_number_fragment = self.render_context.doc.environment.make_fragment(
                    sec_num_info['formatted_value'] + heading_joiner,
                    is_block_level=False,
                    what=f"section-{heading_level} counter",
                )

                full_heading_nodelist = (
                    []
                    + heading_number_fragment.nodes.nodelist
                    + heading_content_nodelist.nodelist
                )

                if refs_mgr is not None:
                    for label_info in labels:
                        (ref_type, ref_label) = label_info
                        counter_formatter_id = self.counter_formatter.counter_formatter_id
                        refs_mgr.register_reference(
                            ref_type, ref_label,
                            node=node,
                            formatted_ref_flm_text=self.counter_formatter.format_flm(
                                sec_num_info['value'].get_num(),
                                subnums=sec_num_info['value'].get_subnums(),
                                numprefix=sec_num_info['numprefix'],
                                with_prefix=True,
                            ),
                            target_href=f'#{target_id}',
                            counter_value=sec_num_info['value'],
                            counter_numprefix=sec_num_info['numprefix'],
                            counter_formatter_id=counter_formatter_id
                        )
            else:
                full_heading_nodelist = heading_content_nodelist

                if refs_mgr is not None:
                    for flm_referenceable_info in node.flm_referenceable_infos:
                        refs_mgr.register_reference_referenceable(
                            node=node,
                            referenceable_info=flm_referenceable_info,
                        )

            return {
                'target_id': target_id,
                'content_nodelist': full_heading_nodelist,
                'sec_num_info': sec_num_info,
            }

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


    class SectionCommandInfo:
        def __init__(self, cmdname, inline=False):
            super().__init__()
            self.cmdname = cmdname
            self.inline = inline

        def __repr__(self):
            return (
                f"{self.__class__.__name__}(cmdname={repr(self.cmdname)}, "
                f"inline={repr(self.inline)})"
            )

    class SectionNumberingInfo:
        r"""
        Doc...

        The `number_within_reset_at` sets the parent counter to number this
        heading level within.  If set to `None`, there will be no parent counter
        and this section level is numbered throughout.  If set to `True`, then
        the parent counter is determined automatically (no parent counter for
        top-level heading and last numbered heading level for sub-headings).
        """
        def __init__(self, format_num, numprefix=None, heading_joiner=' ',
                     number_within_reset_at=True):
            super().__init__()
            self.format_num = format_num
            self.numprefix = numprefix
            self.heading_joiner = heading_joiner
            self.number_within_reset_at = number_within_reset_at

        def __repr__(self):
            return (
                f"{self.__class__.__name__}(format_num={repr(self.format_num)}, "
                f"numprefix={repr(self.numprefix)}, "
                f"heading_joiner={repr(self.heading_joiner)}, "
                f"number_within_reset_at={repr(self.number_within_reset_at)}"
            )


    feature_default_config = {
        'counter_formatter': sec_default_counter_formatter_spec,
        'section_commands_by_level': {
            1: dict(cmdname=r"section"),
            2: dict(cmdname=r"subsection"),
            3: dict(cmdname=r"subsubsection"),
            4: dict(cmdname=r"paragraph", inline=True),
            5: dict(cmdname=r"subparagraph", inline=True),
            6: dict(cmdname=r"subsubparagraph", inline=True),
        },
        'section_numbering_by_level': {
            1: dict(
                format_num={'template': '${arabic}'},
                numprefix=None,
                heading_joiner='. '
            ),
            2: dict(
                format_num={'template': '${arabic}'},
                numprefix='${section}.',
                heading_joiner='. '
            ),
            3: dict(
                format_num={'template': '${arabic}'},
                numprefix='${subsection}.',
                heading_joiner='. '
            ),
            4: dict(
                format_num={'template': '${alph}'},
                numprefix=None,
                heading_joiner='. '
            ),
            5: dict(
                format_num={'template': '${alph}'},
                numprefix='${paragraph}.',
                heading_joiner='. '
            ),
            6: dict(
                format_num={'template': '${alph}'},
                numprefix='${subparagraph}.',
                heading_joiner='. '
            ),
        }
    }

    def __init__(
            self,
            section_commands_by_level=None,
            numbering_section_depth=False,
            counter_formatter=None,
            section_numbering_by_level=None,
    ):
        super().__init__()

        if section_commands_by_level is None:
            section_commands_by_level = self.feature_default_config['section_commands_by_level']
        if section_numbering_by_level is None:
            section_numbering_by_level = \
                self.feature_default_config['section_numbering_by_level']
        if counter_formatter is None:
            counter_formatter = self.feature_default_config['counter_formatter']
        counter_formatter = build_counter_formatter(
            counter_formatter,
            sec_default_counter_formatter_spec,
            counter_formatter_id='section',
        )
        self.counter_formatter = counter_formatter

        # below, dict(...) seems to be needed to force a python-style dict
        # object when using Transcrypt.
        self.section_commands_by_level = {
            level: self._make_section_command_info(x)
            for level, x in dict(section_commands_by_level).items()
            if x is not None
        }
        self.section_numbering_by_level = {
            level: self._make_section_numbering_info(x)
            for level, x in dict(section_numbering_by_level).items()
            if x is not None
        }

        # all section headings with level <= numbering_section_depth will be
        # numbered.
        self.numbering_section_depth = numbering_section_depth
        if self.numbering_section_depth is None:
            self.numbering_section_depth = False


    def _make_section_command_info(self, x):
        r"""
        Ensure the macro spec info object for a sectioning command.
        
        The object `x` might already be a `SectionCommandInfo` instance.
        """
        if isinstance(x, self.SectionCommandInfo):
            return x
        if isinstance(x, str):
            return self.SectionCommandInfo(x)
        return self.SectionCommandInfo(**x)

    def _make_section_numbering_info(self, x):
        if isinstance(x, self.SectionNumberingInfo):
            return x
        return self.SectionNumberingInfo(**x)

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



# ------------------------------------------------

FeatureClass = FeatureHeadings
