import unittest

from llm.llmdocument import LLMDocument
from llm.fragmentrenderer import TextFragmentRenderer
from llm.htmlfragmentrenderer import HtmlFragmentRenderer
from llm.llmstd import LLMStandardEnvironment
from llm import llmstd

# ------------------

import pylatexenc.latexnodes.nodes as latexnodes_nodes

from llm.llmspecinfo import LLMMacroSpec, LLMSpecInfo
from llm.feature import Feature, FeatureDocumentManager


class _MyFeature(Feature):
    feature_name = 'my-test-feature'

    def spawn_document_manager(self, doc):
        return _MyFeatureDocManager(self, doc)

    def add_latex_context_definitions(self):
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

class _MyFeatureDocManager(FeatureDocumentManager):

    def __init__(self, feature, doc):
        super().__init__(feature, doc)
        self.anchors = {}
        self.document_size = None
        self.final_document_size = None

    def process(self, fragment_renderer, first_pass_value):
        print("Document after first pass:\n********\n"+first_pass_value+"\n********")
        self.document_size = len(first_pass_value)

    def postprocess(self, fragment_renderer, final_value):
        self.final_document_size = len(final_value)

    def register_anchor(self, name, node, label):
        self.anchors[name] = {
            'node': node,
            'label': label,
            'number': 1+len(self.anchors)
        }

    def get_anchor_number(self, name):
        return self.anchors[name]['number']


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
            return fragment_renderer.render_nothing(['anchor', 'myAnchor'])
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'anotherAnchor':
            return fragment_renderer.render_nothing(['anchor', 'anotherAnchor'])
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
        doc = LLMDocument(render_fn, environ)

        result = doc.render(fr)
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
        doc = LLMDocument(render_fn, environ)

        result = doc.render(fr)
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

        my_feature = _MyFeature()

        # we need to add the definitions manually here because I'm not using
        # latex_context = llmstd.standard_latex_context_db()
        # latex_context.add_context_category(
        #     'my-test-feature-macros',
        #     **my_feature_manager.add_latex_context_definitions()
        # )
        # latex_context.freeze()
        parsing_state = llmstd.standard_parsing_state()

        environ = LLMStandardEnvironment(
            latex_context=None, #llmstd.standard_latex_context_db(), #latex_context,
            parsing_state=parsing_state,
            features=[
                my_feature
            ]
        )

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
        # could be environ.make_document(...) instead, avoids having to repeat
        # feature_managers
        doc = LLMDocument(
            render_fn,
            environ,
            environ.features,
        )

        result = doc.render(fr)
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



    # ------------------

    def test_more_basic_features_html(self):
    
        environ = LLMStandardEnvironment()

        # -- unknown macros in math --
        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(\alpha+\beta=\gamma\)."
        )
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
        doc = LLMDocument(render_fn, environ)

        result = doc.render(fr)
        print(result)

        self.assertEqual(result, r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(\alpha+\beta=\gamma\)</span>.</p></div>
<div><p>We can also split text across multiple paragraphs, like this
block of text here.</p>
<p>we can also have an equation, like this:
<span class="display-math env-align">\begin{align}
    1 + 3 - 5 = -1
\end{align}</span></p></div>
</main>
        """.strip())




if __name__ == '__main__':
    unittest.main()
