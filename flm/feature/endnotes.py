import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import ParsedArgumentsInfo
#from pylatexenc import macrospec

from ..flmspecinfo import FLMMacroSpecBase
from ..flmenvironment import FLMArgumentSpec
from ..flmfragment import FLMFragment

from ._base import Feature
from ..counter import build_counter_formatter, Counter



_default_endnote_counter_formatter_spec = {
    'format_num': { 'template': '${roman}' },
    'prefix_display': None,
    'delimiters': ('',''),
    'join_spec': 'compact',
}



class EndnoteCategory:
    r"""
    The `counter_formatter` can be one of the keys in
    `counter.standard_counter_formatters` for instance.  Or it can be a
    callable.  It should return FLM text to use to represent the value of the
    counter.

    The `endnote_command` provides a simple way of defining a macro that adds an
    endnote of this category.  If non-None, then it should be a macro name (no
    backslash) that will be defined and whose behavior is to add an endnote of
    the given content in this endnote category.  The macro will take a single
    mandatory argument, the contents of the endnote, think like
    `\footnote{...}`.  Leave this to `None` to not define such a macro.
    """
    def __init__(self, category_name, counter_formatter, heading_title,
                 endnote_command=None):
        super().__init__()
        self.category_name = category_name
        counter_formatter = build_counter_formatter(
            counter_formatter,
            _default_endnote_counter_formatter_spec,
            counter_formatter_id='endnote',
        )
        self.counter_formatter = counter_formatter
        self.heading_title = heading_title
        self.endnote_command = endnote_command



class EndnoteMacro(FLMMacroSpecBase):

    allowed_in_standalone_mode = False

    def __init__(self, macroname, endnote_category_name, **kwargs):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                FLMArgumentSpec(
                    parser='{',
                    argname='endnote_content',
                    flm_doc="The content of the endnote to place (e.g., the text of a footnote)",
                ),
            ],
            **kwargs
        )
        self.endnote_category_name = endnote_category_name
        
    def get_flm_doc(self):
        return (f"Place an end note in the category ‘{self.endnote_category_name}’ with"
                f"the given content.")

    def render(self, node, render_context):
        
        mgr = render_context.feature_render_manager('endnotes')
        if mgr is None:
            raise RuntimeError(
                "You did not set up the feature 'endnotes' in your FLM environment"
            )

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('endnote_content',) ,
        )

        content_nodelist = node_args['endnote_content'].get_content_nodelist()

        #logger.debug("Endnote command, content_nodelist = %r", content_nodelist)

        # register & render the end note
        endnote = mgr.add_endnote(
            category_name=self.endnote_category_name,
            content_nodelist=content_nodelist,
            node=node,
        )

        rendered_endnote_mark = mgr.render_endnote_mark(endnote)
        return rendered_endnote_mark




class EndnoteInstance:
    def __init__(self, category_name, number, formatted_counter_value_flm,
                 content_nodelist, ref_label_prefix, ref_label):
        super().__init__()
        self.category_name = category_name
        self.number = number
        self.formatted_counter_value_flm = formatted_counter_value_flm
        self.content_nodelist = content_nodelist
        self.ref_label_prefix = ref_label_prefix
        self.ref_label = ref_label
        self._fields = ('category_name', 'number', 'formatted_counter_value_flm',
                        'content_nodelist', 'ref_label_prefix', 'ref_label',)

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )




class FeatureEndnotes(Feature):

    feature_name = 'endnotes'
    feature_title = 'Endnotes: footnotes, references, etc.'

    def feature_flm_doc(self):
        return (
            r"""Add footnotes and support for other endnotes (e.g., citations)
            at the bottom of your pages or your document.  This environment
            supports the base category(ies): """
            + ','.join([f"‘{cat.category_name}’" for cat in self.base_categories])
        )

    def __init__(self, categories, render_options=None):
        r"""
        .....

        Here, `categories` is a list
        """
        super().__init__()

        def _mkcatobj(x):
            if isinstance(x, EndnoteCategory):
                return x
            return EndnoteCategory(**x)

        if not categories:
            categories = []

        self.base_categories = [
            _mkcatobj(x)
            for x in categories
        ]
        
        self.default_render_options = render_options if render_options else {}

    def add_latex_context_definitions(self):

        macros = []
        for encat in self.base_categories:
            if encat.endnote_command:
                macros.append(
                    EndnoteMacro(
                        encat.endnote_command,
                        endnote_category_name=encat.category_name,
                    )
                )
        #logger.debug("Adding macros: %r", macros)
        return dict(macros=macros)

    class DocumentManager(Feature.DocumentManager):
        def initialize(self):
            self.categories = list(self.feature.base_categories)
            self.categories_by_name = { c.category_name : c
                                        for c in self.categories }
            #logger.debug("Initialized document endnote categories -- %r", self.categories)
            
        def add_endnote_category(self, endnote_category):
            if endnote_category.category_name in self.categories_by_name:
                raise ValueError(
                    f"Endnote category ‘{endnote_category.category_name}’ is "
                    f"already a registered endnote category"
                )
            self.categories.append(endnote_category)
            self.categories_by_name[endnote_category.category_name] = endnote_category

    class RenderManager(Feature.RenderManager):

        def initialize(self):
            self.endnotes = {
                c.category_name: []
                for c in self.feature_document_manager.categories
            }
            self.endnote_counters = {
                c.category_name: Counter(c.counter_formatter)
                for c in self.feature_document_manager.categories
            }
            self.endnote_instances = {} # node_id -> endnote instance

        def add_endnote(self, category_name, content_nodelist, *,
                        node, ref_label_prefix=None, ref_label=None):

            node_id = self.get_node_id(node)

            if node_id in self.endnote_instances:
                # this happens on second pass when rendering in two passes.
                return self.endnote_instances[node_id]

            # endnote_category_info = \
            #     self.feature_document_manager.categories_by_name[category_name]

            number, fmtvalue_flm_text = \
                self.endnote_counters[category_name].step_and_format_flm()

            fmtvalue_flm = self.render_context.doc.environment.make_fragment(
                fmtvalue_flm_text,
                is_block_level=False,
                what=f"{category_name} counter",
            )

            endnote = EndnoteInstance(
                category_name=category_name,
                number=number,
                formatted_counter_value_flm=fmtvalue_flm,
                content_nodelist=content_nodelist,
                ref_label_prefix=ref_label_prefix,
                ref_label=ref_label,
            )
            self.endnotes[category_name].append( endnote )

            if node_id is not None:
                self.endnote_instances[node_id] = endnote

            return endnote

        def render_endnote_mark(self, endnote, display_flm=None,
                                wrap_with_semantic_span='endnotes'):
            r"""
            Render the endnote mark for the given `endnote`.  You can
            replace the mark's displayed content by specifying the `display_flm`
            argument.  The latter must be a `FLMFragment` instance or a
            `pylatexenc.latexnodes.nodes.LatexNodesList` instance.
            """
            endnote_link_href = f"#{endnote.category_name}-{endnote.number}"

            if display_flm is None:
                fmtvalue_flm = endnote.formatted_counter_value_flm
            else:
                fmtvalue_flm = display_flm

            if isinstance(fmtvalue_flm, FLMFragment):
                fmtvalue_nodelist = fmtvalue_flm.nodes
            else:
                fmtvalue_nodelist = fmtvalue_flm

            annotations = ['endnote', endnote.category_name]
            if wrap_with_semantic_span:
                annotations.append(wrap_with_semantic_span)

            contents = self.render_context.fragment_renderer.render_link(
                'endnote',
                endnote_link_href,
                display_nodelist=fmtvalue_nodelist,
                render_context=self.render_context,
                annotations=annotations,
            )
            # ### Already added as annotation to the link; should save DOM size
            # ### etc. and shouldn't really be needed for anything else than
            # ### HTML output
            # ... render_semantic_span( ... )
            return contents

        def render_endnote_mark_many(self, endnote_list, *,
                                     counter_prefix_variant=None,
                                     counter_with_delimiters=True,
                                     counter_with_prefix=False,
                                     wrap_with_semantic_span='endnotes'):

            render_context = self.render_context
            fragment_renderer = render_context.fragment_renderer

            endnotes_by_category = {}
            for endnote in endnote_list:
                if endnote.category_name not in endnotes_by_category:
                    endnotes_by_category[endnote.category_name] = []
                endnotes_by_category[endnote.category_name].append(endnote)

            s_final_blocks = []

            for category_name, endnote_list in endnotes_by_category.items():

                endnote_category_info = \
                    self.feature_document_manager.categories_by_name[endnote.category_name]

                counter_formatter = endnote_category_info.counter_formatter

                s_items = counter_formatter.format_many_flm(
                    [ e.number for e in endnote_list ],
                    prefix_variant=counter_prefix_variant,
                    with_delimiters=counter_with_delimiters,
                    with_prefix=counter_with_prefix,
                    get_raw_s_items=True,
                )
                s = ''
                for sit in s_items:
                    s_frag = render_context.doc.environment.make_fragment(
                        sit['s'],
                        is_block_level=False,
                        standalone_mode=True,
                        what=f"Rendered endnote mark(s) bit {repr(sit)}",
                    )
                    if sit['n'] is None or sit['n'] is False:
                        s += fragment_renderer.render_fragment(s_frag, render_context)
                    else:
                        endnote_link_href = f"#{category_name}-{sit['n']}"

                        s += fragment_renderer.render_link(
                            'endnote',
                            endnote_link_href,
                            s_frag.nodes,
                            render_context=render_context,
                            # TODO: add annotation for external links etc. ??
                            annotations=['endnote', category_name,],
                        )

                s_final_blocks.append( s )

            contents = fragment_renderer.render_join(s_final_blocks, render_context)

            if wrap_with_semantic_span:
                return fragment_renderer.render_semantic_span(
                    contents,
                    wrap_with_semantic_span,
                    render_context,
                )
            return contents


        def render_endnotes_category(self, category_name):

            render_context = self.render_context
            fragment_renderer = render_context.fragment_renderer

            if hasattr(category_name, 'category_name'):
                encat = category_name
                category_name = encat.category_name

            def the_endnotes_enumeration_counter_formatter(n):
                endnote = self.endnotes[category_name][n-1]
                fmtvalue_flm = endnote.formatted_counter_value_flm
                return fmtvalue_flm.nodes

            def the_target_id_generator_fn(n):
                return f"{category_name}-{n}"

            #logger.debug("Endnotes are = %r", self.endnotes)

            # I have no idea why transcrypt seems to want a list here and not an
            # iterable (will render incorrectly otherwise???)
            iterable_of_content_endnotes = [
                en.content_nodelist
                for en in self.endnotes[category_name]
            ]

            return fragment_renderer.render_enumeration(
                iterable_of_content_endnotes,
                counter_formatter=the_endnotes_enumeration_counter_formatter,
                target_id_generator=the_target_id_generator_fn,
                render_context=self.render_context,
                annotations=[category_name+'-list'], # "footnote" -> "footnote-list"
            )


        def render_endnotes(self,
                            target_id='endnotes',
                            annotations=None,
                            include_headings_at_level=None,
                            set_headings_target_ids=None, #False,
                            endnotes_heading_title=None,
                            endnotes_heading_level=None, #1,
                            ):

            if include_headings_at_level is None:
                include_headings_at_level = \
                    self.feature.default_render_options.get('include_headings_at_level', None)
            if set_headings_target_ids is None:
                set_headings_target_ids = \
                    self.feature.default_render_options.get('set_headings_target_ids', False)
            if endnotes_heading_title is None:
                endnotes_heading_title = \
                    self.feature.default_render_options.get('endnotes_heading_title', None)
            if endnotes_heading_level is None:
                endnotes_heading_level = \
                    self.feature.default_render_options.get('endnotes_heading_level', 1)


            render_context = self.render_context
            fragment_renderer = render_context.fragment_renderer

            has_endnotes = False

            blocks = []
            for encat in self.feature_document_manager.categories:
                if not len(self.endnotes[encat.category_name]):
                    # skip this category rendering, no endnotes
                    continue

                has_endnotes = True

                if include_headings_at_level is not None \
                   and include_headings_at_level is not False:
                    heading_nodelist = self.render_context.doc.environment.make_fragment(
                        encat.heading_title,
                        is_block_level=False,
                        what=f"{encat.category_name} heading title",
                    )
                    heading_target_id = None
                    if set_headings_target_ids:
                        heading_target_id = f"{target_id}-{encat.category_name}"
                    blocks.append(
                        fragment_renderer.render_heading(
                            heading_nodelist.nodes,
                            render_context=self.render_context,
                            heading_level=include_headings_at_level,
                            target_id=heading_target_id,
                        )
                    )
                blocks.append(
                    self.render_endnotes_category(encat)
                )

            if not has_endnotes:
                return fragment_renderer.render_nothing(
                    annotations=['no-endnotes'],
                    render_context=render_context,
                )

            if endnotes_heading_title is not None:
                heading_title_nodelist = \
                    self.render_context.doc.environment.make_fragment(
                        endnotes_heading_title,
                        is_block_level=False,
                        what=f"endnotes heading title",
                    )
                blocks.insert(
                    0,
                    fragment_renderer.render_heading(
                        heading_title_nodelist.nodes,
                        render_context=self.render_context,
                        heading_level=endnotes_heading_level,
                    )
                )
                

            return fragment_renderer.render_semantic_block(
                fragment_renderer.render_join_blocks( blocks, render_context ),
                role='endnotes',
                render_context=self.render_context,
                annotations=annotations,
                target_id=target_id,
            )




# ------------------------------------------------

FeatureClass = FeatureEndnotes
