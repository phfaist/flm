import logging
logger = logging.getLogger(__name__)

from pylatexenc import macrospec

from .llmspecinfo import LLMMacroSpec, LLMSpecInfo
from .llmenvironment import make_arg_spec

from .feature import Feature
from . import fmthelpers



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
        
        if hasattr(node, 'llm_endnotes_rendered_endnote_mark'):
            # for two-pass rendering, don't add a second endnote!
            return node.llm_endnotes_rendered_endnote_mark

        fragment_renderer = render_context.fragment_renderer
        mgr = render_context.feature_render_manager('endnotes')
        if mgr is None:
            raise RuntimeError(
                "You did not set up the feature 'endnotes' in your LLM environment"
            )

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('endnote_content',) ,
            all=True
        )

        content_nodelist = node_args['endnote_content'].nodelist

        logger.debug("Endnote command, content_nodelist = %r", content_nodelist)

        # register & render the end note
        endnote = mgr.add_endnote(
            category_name=self.endnote_category_name,
            content_nodelist=content_nodelist,
        )
        rendered_endnote_mark = mgr.render_endnote_mark(endnote)
        node.llm_endnotes_rendered_endnote_mark = rendered_endnote_mark
        return rendered_endnote_mark




class EndnoteInstance:
    def __init__(self, category_name, number, formatted_counter_value_llm_text,
                 content_nodelist, label):
        super().__init__()
        self.category_name = category_name
        self.number = number
        self.formatted_counter_value_llm_text = formatted_counter_value_llm_text
        self.content_nodelist = content_nodelist
        self.label = label

    def __repr__(self):
        return (
            f"{self.__class__.__name__}(category_name={self.category_name!r}, "
            f"number={self.number!r}, "
            f"formatted_counter_value_llm_text={self.formatted_counter_value_llm_text!r}, "
            f"content_nodelist={self.content_nodelist!r}, "
            f"label={self.label!r})"
        )

class FeatureEndnotesRenderManager(Feature.RenderManager):

    def initialize(self):
        self.endnotes = {
            c.category_name: []
            for c in self.feature.categories
        }
        self.endnote_counters = {
            c.category_name: 1
            for c in self.feature.categories
        }

    def add_endnote(self, category_name, content_nodelist, label=None):
        fmtcounter = self.feature.categories_by_name[category_name].counter_formatter
        number = self.endnote_counters[category_name]
        self.endnote_counters[category_name] += 1

        fmtvalue_llm_text = fmtcounter(number)

        endnote = EndnoteInstance(
            category_name=category_name,
            number=number,
            formatted_counter_value_llm_text=fmtvalue_llm_text,
            content_nodelist=content_nodelist,
            label=label,
        )
        self.endnotes[category_name].append( endnote )
        return endnote

    def render_endnote_mark(self, endnote):
        endnote_link_href = f"#{endnote.category_name}-{endnote.number}"
        fmtvalue_llm = self.render_context.doc.environment.make_fragment(
            endnote.formatted_counter_value_llm_text,
            is_block_level=False,
            what=f"Endnote counter ({endnote.category_name})",
        )
        return self.render_context.fragment_renderer.render_link(
            'endnote',
            endnote_link_href,
            display_nodelist=fmtvalue_llm.nodes,
            render_context=self.render_context,
            annotations=['endnote', endnote.category_name],
        )


    def render_endnote_category(self, category_name):

        render_context = self.render_context
        fragment_renderer = render_context.fragment_renderer

        if hasattr(category_name, 'category_name'):
            encat = category_name
            category_name = encat.category_name
        else:
            encat = self.feature.categories_by_name[category_name]


        def the_endnotes_enumeration_counter_formatter(n):
            endnote = self.endnotes[category_name][n-1]
            fmtvalue_llm = self.render_context.doc.environment.make_fragment(
                endnote.formatted_counter_value_llm_text,
                is_block_level=False,
                what=f"Endnote counter ({endnote.category_name})",
            )
            return fmtvalue_llm.nodes

        logger.debug("Endnotes are = %r", self.endnotes)

        return fragment_renderer.render_enumeration(
            ( en.content_nodelist for en in self.endnotes[category_name] ),
            counter_formatter=the_endnotes_enumeration_counter_formatter,
            render_context=self.render_context,
            annotations=[category_name+'-list'], # "footnote" -> "footnote-list"
        )


    def render_endnotes(self):

        render_context = self.render_context
        fragment_renderer = render_context.fragment_renderer

        blocks = [
            self.render_endnote_category(encat)
            for encat in self.feature.categories
        ]

        return fragment_renderer.render_semantic_block(
            fragment_renderer.render_join_blocks( blocks ),
            role='endnotes',
        )




class FeatureEndnotes(Feature):

    feature_name = 'endnotes'
    RenderManager = FeatureEndnotesRenderManager

    def __init__(self, categories):
        r"""
        .....

        Here, `categories` is a list
        """
        super().__init__()
        self.categories = categories
        self.categories_by_name = { c.category_name : c
                                    for c in self.categories }

    def add_latex_context_definitions(self):

        macros = []
        for encat in self.categories:
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
