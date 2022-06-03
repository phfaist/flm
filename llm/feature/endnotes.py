import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import ParsedArgumentsInfo
#from pylatexenc import macrospec

from ..llmspecinfo import LLMMacroSpec, LLMSpecInfo
from ..llmenvironment import make_arg_spec

from ._base import Feature
from .. import fmthelpers



class EndnoteCategory:
    r"""
    The `counter_formatter` can be one of the keys in
    `fmthelpers.standard_counter_formatters` for instance.  Or it can be a
    callable.  It should return LLM text to use to represent the value of the
    counter.

    The `endnote_command` provides a simple way of defining a macro that adds an
    endnote of this category.  If non-None, then it should be a macro name (no
    backslash) that will be defined and whose behavior is to add an endnote of
    the given content in this endnote category.  The macro will take a single
    mandatory argument, the contents of the endnote, think like
    `\footnote{...}`.  Leave this to `None` to not define such a macro.
    """
    def __init__(self, category_name, counter_formatter, endnote_command=None):
        super().__init__()
        self.category_name = category_name
        if not callable(counter_formatter):
            counter_formatter = fmthelpers.standard_counter_formatters[counter_formatter]
        self.counter_formatter = counter_formatter
        self.endnote_command = endnote_command


class EndnoteSpecInfo(LLMSpecInfo):

    def __init__(self, endnote_category_name, **kwargs):
        super().__init__(**kwargs)
        self.endnote_category_name = endnote_category_name
        
    def render(self, node, render_context):
        
        mgr = render_context.feature_render_manager('endnotes')
        if mgr is None:
            raise RuntimeError(
                "You did not set up the feature 'endnotes' in your LLM environment"
            )

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('endnote_content',) ,
        )

        content_nodelist = node_args['endnote_content'].get_content_nodelist()

        logger.debug("Endnote command, content_nodelist = %r", content_nodelist)

        if hasattr(node, 'llm_endnote_instance'):
            # for two-pass rendering, don't add a second endnote!
            endnote = node.llm_endnote_instance
        else:
            # register & render the end note
            endnote = mgr.add_endnote(
                category_name=self.endnote_category_name,
                content_nodelist=content_nodelist,
            )
            node.llm_endnote_instance = endnote

        rendered_endnote_mark = mgr.render_endnote_mark(endnote)
        return rendered_endnote_mark




class EndnoteInstance:
    def __init__(self, category_name, number, formatted_counter_value_llm,
                 content_nodelist, ref_label_prefix, ref_label):
        super().__init__()
        self.category_name = category_name
        self.number = number
        self.formatted_counter_value_llm = formatted_counter_value_llm
        self.content_nodelist = content_nodelist
        self.ref_label_prefix = ref_label_prefix
        self.ref_label = ref_label
        self._fields = ('category_name', 'number', 'formatted_counter_value_llm',
                        'content_nodelist', 'ref_label_prefix', 'ref_label',)

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )




class FeatureEndnotes(Feature):

    feature_name = 'endnotes'

    def __init__(self, categories):
        r"""
        .....

        Here, `categories` is a list
        """
        super().__init__()
        self.base_categories = categories

    def add_latex_context_definitions(self):

        macros = []
        for encat in self.base_categories:
            if encat.endnote_command:
                macros.append(
                    LLMMacroSpec(
                        encat.endnote_command,
                        [
                            make_arg_spec('{', argname='endnote_content'),
                        ],
                        llm_specinfo=EndnoteSpecInfo(encat.category_name,)
                    )
                )
        logger.debug("Adding macros: %r", macros)
        return dict(macros=macros)

    class DocumentManager(Feature.DocumentManager):
        def initialize(self):
            self.categories = list(self.feature.base_categories)
            self.categories_by_name = { c.category_name : c
                                        for c in self.categories }
            
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
                c.category_name: 1
                for c in self.feature_document_manager.categories
            }

        def add_endnote(self, category_name, content_nodelist, *,
                        ref_label_prefix=None, ref_label=None):
            endnote_category_info = \
                self.feature_document_manager.categories_by_name[category_name]
            fmtcounter = endnote_category_info.counter_formatter
            number = self.endnote_counters[category_name]
            self.endnote_counters[category_name] += 1

            fmtvalue_llm_text = fmtcounter(number)
            fmtvalue_llm = self.render_context.doc.environment.make_fragment(
                fmtvalue_llm_text,
                is_block_level=False,
                what=f"{category_name} counter",
            )

            endnote = EndnoteInstance(
                category_name=category_name,
                number=number,
                formatted_counter_value_llm=fmtvalue_llm,
                content_nodelist=content_nodelist,
                ref_label_prefix=ref_label_prefix,
                ref_label=ref_label,
            )
            self.endnotes[category_name].append( endnote )
            return endnote

        def render_endnote_mark(self, endnote):
            endnote_link_href = f"#{endnote.category_name}-{endnote.number}"
            fmtvalue_llm = endnote.formatted_counter_value_llm
            return self.render_context.fragment_renderer.render_link(
                'endnote',
                endnote_link_href,
                display_nodelist=fmtvalue_llm.nodes,
                render_context=self.render_context,
                annotations=['endnote', endnote.category_name],
            )


        def render_endnotes_category(self, category_name):

            render_context = self.render_context
            fragment_renderer = render_context.fragment_renderer

            if hasattr(category_name, 'category_name'):
                encat = category_name
                category_name = encat.category_name
            # else:
            #     encat = self.feature_document_manager.categories_by_name[category_name]

            def the_endnotes_enumeration_counter_formatter(n):
                endnote = self.endnotes[category_name][n-1]
                fmtvalue_llm = endnote.formatted_counter_value_llm
                return fmtvalue_llm.nodes

            def the_target_id_generator_fn(n):
                return f"{category_name}-{n}"

            logger.debug("Endnotes are = %r", self.endnotes)

            return fragment_renderer.render_enumeration(
                ( en.content_nodelist for en in self.endnotes[category_name] ),
                counter_formatter=the_endnotes_enumeration_counter_formatter,
                target_id_generator=the_target_id_generator_fn,
                render_context=self.render_context,
                annotations=[category_name+'-list'], # "footnote" -> "footnote-list"
            )


        def render_endnotes(self):

            render_context = self.render_context
            fragment_renderer = render_context.fragment_renderer

            blocks = [
                self.render_endnotes_category(encat)
                for encat in self.feature_document_manager.categories
            ]

            return fragment_renderer.render_semantic_block(
                fragment_renderer.render_join_blocks( blocks ),
                role='endnotes',
            )


