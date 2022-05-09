from pylatexenc import macrospec

from .llmspecinfo import LLMMacroSpec, LLMSpecInfo
from .feature import Feature, FeatureDocumentManager
from . import fmthelpers



class EndnoteCategory:
    r"""
    The `counter_formatter` can be one of the keys in
    `fmthelpers.standard_counter_formatters` for instance.  Or it can be a
    callable.

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
        
    def render(self, node, doc, fragment_renderer):

        mgr = doc.feature_manager('endnotes')
        if mgr is None:
            raise RuntimeError(
                "You did not set up the feature 'endnotes' in your LLM environment"
            )

        node_args = fragment_renderer.get_arguments_nodelists(
            node,
            ('endnote_content',) ,
            all=True
        )

        content = fragment_renderer.render_nodelist(
            node_args['endnote_content'].nodelist,
            doc,
            use_paragraphs=False
        )

        # register & render the end note
        endnote = mgr.add_endnote(
            self.endnote_category_name,
            content
        )
        return mgr.render_endnote_mark(endnote, fragment_renderer)




class EndnoteInstance:
    def __init__(self, category_name, number, formatted_counter_value, content, label):
        super().__init__()
        self.category_name = category_name
        self.number = number
        self.formatted_counter_value = formatted_counter_value
        self.content = content
        self.label = label


class FeatureEndnotesDocumentManager(FeatureDocumentManager):

    def initialize(self):
        self.endnotes = {
            c.category_name: []
            for c in self.feature.categories
        }
        self.endnote_counters = {
            c.category_name: 1
            for c in self.feature.categories
        }


    def add_endnote(self, category_name, content, label=None):
        fmtcounter = self.feature.categories_by_name[category_name].counter_formatter
        number = self.endnote_counters[category_name]
        self.endnote_counters[category_name] += 1
        endnote = EndnoteInstance(
            category_name=category_name,
            number=number,
            formatted_counter_value=fmtcounter(number),
            content=content,
            label=label,
        )
        self.endnotes[category_name].append( endnote )
        return endnote

    def render_endnote_mark(self, endnote, fragment_renderer):
        endnote_link_href = f"#{endnote.category_name}-{endnote.number}"
        return fragment_renderer.render_link(
            'endnote',
            endnote_link_href,
            endnote.formatted_counter_value,
            ['endnote', endnote.category_name],
        )


    def render_endnote_category(self, category_name, fragment_renderer):

        if hasattr(category_name, 'category_name'):
            encat = category_name
            category_name = encat.category_name
        else:
            encat = self.feature.categories_by_name[category_name]

        return fragment_renderer.render_enumeration(
            ( en.content for en in self.endnotes[category_name] ),
            counter_formatter=\
                lambda n: self.endnotes[category_name][n].formatted_counter_value,
            annotations=[category_name+'-list'], # "footnote" -> "footnote-list"
        )


    def render_endnotes(self, fragment_renderer):

        blocks = [
            self.render_endnote_category(encat, fragment_renderer)
            for encat in self.feature.categories
        ]

        return fragment_renderer.render_semantic_block(
            fragment_renderer.render_join_blocks( blocks ),
            role='endnotes',
        )




class FeatureEndnotes(Feature):

    feature_name = 'endnotes'
    feature_manager_class = FeatureEndnotesDocumentManager

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
                            macrospec.LatexArgumentSpec('{', argname='endnote_content'),
                        ],
                        llm_specinfo=EndnoteSpecInfo(encat.category_name)
                    )
                )
        print("Adding macros: ", macros)
        return dict(macros=macros)
