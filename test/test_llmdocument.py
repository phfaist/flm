import unittest

from llm.llmdocument import LLMDocument
from llm.fragmentrenderer import TextFragmentRenderer
from llm.htmlfragmentrenderer import HtmlFragmentRenderer
from llm.llmstd import LLMStandardEnvironment
from llm import llmstd

# ------------------

import pylatexenc.latexnodes.nodes as latexnodes_nodes

from llm.llmspecinfo import LLMMacroSpec, LLMSpecInfo
from llm.feature import DocumentFeatureBase

class _MyFeatureManager(DocumentFeatureBase):
    feature_name = 'my-test-feature'

    def initialize(self, doc):
        self.doc = doc
        self.anchors = {}
        self.document_size = None
        self.final_document_size = None

    def process(self, doc, fragment_renderer, first_pass_value):
        print("Document after first pass:\n********\n"+first_pass_value+"\n********")
        self.document_size = len(first_pass_value)

    def postprocess(self, doc, fragment_renderer, final_value):
        self.final_document_size = len(final_value)

    def register_anchor(self, name, node, label):
        self.anchors[name] = {
            'node': node,
            'label': label,
            'number': 1+len(self.anchors)
        }

    def get_anchor_number(self, name):
        return self.anchors[name]['number']

    @classmethod
    def latex_context_definitions(cls):
        return dict(
            macros=[
                LLMMacroSpec('myAnchor', '',
                             llm_specinfo=_MyDocumentSizeMacroSpecInfo()),
                LLMMacroSpec('anotherAnchor', '',
                             llm_specinfo=_MyDocumentSizeMacroSpecInfo()),
                LLMMacroSpec('linkMyAnchor', '',
                             llm_specinfo=_MyDocumentSizeMacroSpecInfo()),
                LLMMacroSpec('linkAnotherAnchor', '',
                             llm_specinfo=_MyDocumentSizeMacroSpecInfo()),
                LLMMacroSpec('printDocumentSize', '',
                             llm_specinfo=_MyDocumentSizeMacroSpecInfo()),
            ],
            environments=[],
            specials=[],
        )

class _MyDocumentSizeMacroSpecInfo(LLMSpecInfo):

    delayed_render = True

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    def prepare_delayed_render(self, node, doc, fragment_renderer):
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'myAnchor':
            doc.feature_manager('my-test-feature').register_anchor('myAnchor', node, 'myAnchor')
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'anotherAnchor':
            doc.feature_manager('my-test-feature').register_anchor('anotherAnchor', node,
                                                                   'anotherAnchor')

    def render(self, node, doc, fragment_renderer):

        mgr = doc.feature_manager('my-test-feature')

        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'myAnchor':
            return fragment_renderer.render_nothing('anchor myAnchor')
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'anotherAnchor':
            return fragment_renderer.render_nothing('anchor anotherAnchor')
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'linkMyAnchor':
            return fragment_renderer.render_link(
                'anchor', '#myAnchor',
                'link to Anchor #{}'.format(mgr.get_anchor_number('myAnchor'))
            )
        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
           and node.macroname == 'linkAnotherAnchor':
            return fragment_renderer.render_link(
                'anchor', '#anotherAnchor',
                'link to Anchor #{}'.format(mgr.get_anchor_number('anotherAnchor'))
            )

        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
           and node.macroname == 'printDocumentSize':
            return fragment_renderer.render_value(
                str(mgr.document_size) + ' characters'
            )

        raise ValueError("I don't know what to print: " + repr(node))



# ------------------




class TestLLMDocument(unittest.TestCase):

    maxDiff = None

    def test_simple_html(self):
    
        environ = LLMStandardEnvironment()

        frag1 = environ.make_fragment(r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).")
        frag2 = environ.make_fragment(
            r"""
We can also split text across multiple paragraphs, like this
block of text here.

we can also have an equation, like this:
\begin{align}
    1 + 3 - 5 = -1
\end{align}
            """.strip()
        )

        def render_fn(docobj, frobj):
            return (
                "<main>\n"
                "<div>" + frag1.render(docobj, frobj) + "</div>\n"
                "<div>" + frag2.render(docobj, frobj) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()
        doc = LLMDocument(render_fn, environ, fr)

        result = doc.render()
        print(result)

        self.assertEqual(result, r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.</p></div>
<div><p>We can also split text across multiple paragraphs, like this
block of text here.</p>
<p>we can also have an equation, like this:
<span class="display-math env-align">\begin{align}
    1 + 3 - 5 = -1
\end{align}</span></p></div>
</main>
        """.strip())
        

    def test_simple_text(self):
    
        environ = LLMStandardEnvironment()

        frag1 = environ.make_fragment(r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).")
        frag2 = environ.make_fragment(
            r"""
We can also split text across multiple paragraphs, like this
block of text here.

we can also have an equation, like this:
\begin{align}
    1 + 3 - 5 = -1
\end{align}
            """.strip()
        )

        def render_fn(docobj, frobj):
            return (
                "|||\n" + frag1.render(docobj, frobj) + "\n|||\n"
                + frag2.render(docobj, frobj) + "\n|||"
            )

        fr = TextFragmentRenderer()
        doc = LLMDocument(render_fn, environ, fr)

        result = doc.render()
        print(result)

        self.assertEqual(result, r"""
|||
Hello world, we know that \(a+b=c\).
|||
We can also split text across multiple paragraphs, like this
block of text here.

we can also have an equation, like this:
\begin{align}
    1 + 3 - 5 = -1
\end{align}
|||
        """.strip())
        




    def test_delayed_render(self):

        my_feature_manager = _MyFeatureManager()

        latex_context = llmstd.standard_latex_context_db()
        latex_context.add_context_category(
            'my-test-feature-macros',
            **my_feature_manager.latex_context_definitions()
        )
        latex_context.freeze()
        parsing_state = llmstd.standard_parsing_state(latex_context=latex_context)

        environ = LLMStandardEnvironment(parsing_state=parsing_state)

        frag1 = environ.make_fragment(
            r"\anotherAnchor\textbf{Hello} \textit{world}. Here is a \linkMyAnchor."
        )
        frag2 = environ.make_fragment(
            r"\myAnchor We meet \textbf{again}. Here is a \linkAnotherAnchor. Total document size is (approx.) = \printDocumentSize."
        )

        def render_fn(docobj, frobj, flip_order=False):
            if flip_order:
                (f1, f2) = (frag2, frag1)
            else:
                (f1, f2) = (frag1 ,frag2)
            return (
                "<main>\n"
                "<div>" + f1.render(docobj, frobj) + "</div>\n"
                "<div>" + f2.render(docobj, frobj) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()
        doc = LLMDocument(
            render_fn,
            environ,
            fr,
            feature_managers=[
                my_feature_manager
            ]
        )

        result = doc.render()
        print(result)

        predict_docsize = len(r"""
<main>
<div><p><LLM:DLYD:1/><span class="textbf">Hello</span> <span class="textit">world</span>. Here is a <LLM:DLYD:2/>.</p></div>
<div><p><LLM:DLYD:3/>We meet <span class="textbf">again</span>. Here is a <LLM:DLYD:4/>. Total document size is (approx.) = <LLM:DLYD:5/>.</p></div>
</main>
""".strip())

        self.assertEqual(result, r"""
<main>
<div><p><!-- anchor anotherAnchor --><span class="textbf">Hello</span> <span class="textit">world</span>. Here is a <a href="#myAnchor" class="href-anchor">link to Anchor #2</a>.</p></div>
<div><p><!-- anchor myAnchor -->We meet <span class="textbf">again</span>. Here is a <a href="#anotherAnchor" class="href-anchor">link to Anchor #1</a>. Total document size is (approx.) = {docsize} characters.</p></div>
</main>
        """.strip().format(docsize=predict_docsize))





if __name__ == '__main__':
    unittest.main()
