import unittest

from flm.flmdocument import FLMDocument
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.flmenvironment import make_standard_environment, standard_parsing_state
from flm.feature.baseformatting import FeatureBaseFormatting
from flm.stdfeatures import standard_features

# ------------------

import pylatexenc.latexnodes.nodes as latexnodes_nodes

from flm.flmspecinfo import FLMMacroSpecBase
from flm.feature import Feature



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class _EmptyDefaultDict:
    def __getitem__(self, k):
        return ''


class _MyFeatureRenderManager(Feature.RenderManager):

    def initialize(self):
        self.anchors = {}
        self.document_size = None
        self.final_document_size = None

    def process(self, first_pass_value):
        print("Document after first pass:\n********\n"+first_pass_value+"\n********")
        # compute length w/o the delayed render markers.
        first_pass_value_nodelayedmarkers = \
            self.render_context.fragment_renderer.replace_delayed_markers_with_final_values(
                first_pass_value,
                _EmptyDefaultDict()
            )
        self.document_size = len(first_pass_value_nodelayedmarkers)

    def postprocess(self, final_value):
        self.final_document_size = len(final_value)

    def register_anchor(self, name, node, label):
        self.anchors[name] = {
            'node': node,
            'label': label,
            'number': 1+len(self.anchors)
        }

    def get_anchor_number(self, name):
        return self.anchors[name]['number']

class _MyFeature(Feature):
    feature_name = 'my-test-feature'

    RenderManager = _MyFeatureRenderManager

    def add_latex_context_definitions(self):
        return dict(
            macros=[
                _MyDocumentSizeMacro('myAnchor'),
                _MyDocumentSizeMacro('anotherAnchor'),
                _MyDocumentSizeMacro('linkMyAnchor'),
                _MyDocumentSizeMacro('linkAnotherAnchor'),
                _MyDocumentSizeMacro('printDocumentSize'),
            ],
            environments=[],
            specials=[],
        )


class _MyDocumentSizeMacro(FLMMacroSpecBase):

    delayed_render = True

    def __init__(self, macroname, **kwargs):
        super().__init__(macroname=macroname, arguments_spec_list='', **kwargs)

    def prepare_delayed_render(self, node, render_context):
        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
	   and node.macroname == 'myAnchor':
            render_context.feature_render_manager('my-test-feature') \
                          .register_anchor('myAnchor', node, 'myAnchor')
        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
	   and node.macroname == 'anotherAnchor':
            render_context.feature_render_manager('my-test-feature') \
                          .register_anchor('anotherAnchor', node, 'anotherAnchor')

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer
        mgr = render_context.feature_render_manager('my-test-feature')

        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'myAnchor':
            return fragment_renderer.render_nothing(render_context, ['anchor', 'myAnchor'])
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'anotherAnchor':
            return fragment_renderer.render_nothing(render_context, ['anchor', 'anotherAnchor'])
        if node.isNodeType(latexnodes_nodes.LatexMacroNode)\
	   and node.macroname == 'linkMyAnchor':
            content_nl = node.latex_walker.make_nodelist(
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars='link to Anchor #{}'.format(mgr.get_anchor_number('myAnchor')),
                        parsing_state=node.parsing_state,
                        pos=node.pos,
                        pos_end=node.pos,
                    )
                ],
                parsing_state=node.parsing_state,
            )
            return fragment_renderer.render_link(
                'anchor', '#myAnchor',
                content_nl, render_context
            )
        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
           and node.macroname == 'linkAnotherAnchor':
            content_nl = node.latex_walker.make_nodelist(
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars='link to Anchor #{}'
                            .format(mgr.get_anchor_number('anotherAnchor')),
                        parsing_state=node.parsing_state,
                        pos=node.pos,
                        pos_end=node.pos,
                    )
                ],
                parsing_state=node.parsing_state,
            )
            return fragment_renderer.render_link(
                'anchor', '#anotherAnchor',
                content_nl, render_context
            )

        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
           and node.macroname == 'printDocumentSize':
            return fragment_renderer.render_value(
                str(mgr.document_size) + ' characters',
                render_context,
            )

        raise ValueError("I don't know what to print: " + repr(node))






# ------------------




class TestFLMDocument(unittest.TestCase):

    maxDiff = None

    def test_simple_html(self):
    
        environ = mk_flm_environ()

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

        def render_fn(render_context):
            return (
                "<main>\n"
                "<div>" + frag1.render(render_context, is_block_level=True) + "</div>\n"
                "<div>" + frag2.render(render_context, is_block_level=True) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()
        doc = FLMDocument(render_fn, environ, enable_features=['math'])
        doc.initialize()

        result, _ = doc.render(fr)
        print(result)

        self.assertEqual(result, r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.</p></div>
<div><p>We can also split text across multiple paragraphs, like this block of text here.</p>
<p>we can also have an equation, like this: <span id="equation-1" class="display-math env-align">\begin{align}
    1 + 3 - 5 = -1
\tag*{(1)}\end{align}</span></p></div>
</main>
        """.strip())
    

    def test_simple_text(self):
    
        environ = mk_flm_environ()

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

        def render_fn(render_context):
            return (
                "|||\n" + frag1.render(render_context) + "\n|||\n"
                + frag2.render(render_context) + "\n|||"
            )

        fr = TextFragmentRenderer()
        doc = FLMDocument(render_fn, environ)
        doc.initialize()

        result, _ = doc.render(fr)
        print(result)

        self.assertEqual(result, r"""
|||
Hello world, we know that \(a+b=c\).
|||
We can also split text across multiple paragraphs, like this block of text here.

we can also have an equation, like this: \begin{align}
    1 + 3 - 5 = -1
\tag*{(1)}\end{align}
|||
        """.strip())
        


    def test_delayed_render(self):

        my_feature = _MyFeature()

        parsing_state = standard_parsing_state()

        environ = make_standard_environment(
            latex_context=None,
            parsing_state=parsing_state,
            features=[
                FeatureBaseFormatting(),
                my_feature
            ]
        )

        frag1 = environ.make_fragment(
            r"\anotherAnchor\textbf{Hello} \textit{world}. Here is a \linkMyAnchor."
        )
        frag2 = environ.make_fragment(
            r"\myAnchor We meet \textbf{again}. Here is a \linkAnotherAnchor. Total document size is (approx.) = \printDocumentSize."
        )

        def render_fn(render_context, flip_order=False):
            if flip_order:
                (f1, f2) = (frag2, frag1)
            else:
                (f1, f2) = (frag1 ,frag2)
            return (
                "<main>\n"
                "<div>" + f1.render(render_context, is_block_level=True) + "</div>\n"
                "<div>" + f2.render(render_context, is_block_level=True) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()
        # could be environ.make_document(...) instead, avoids having to repeat
        # feature_managers
        doc = FLMDocument(
            render_fn,
            environ,
        )
        doc.initialize()

        result, _ = doc.render(fr)
        print(result)

        predict_docsize = len(r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>. Here is a .</p></div>
<div><p>We meet <span class="textbf">again</span>. Here is a . Total document size is (approx.) = .</p></div>
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
    
        environ = mk_flm_environ()

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

        def render_fn(render_context):
            return (
                "<main>\n"
                "<div>" + frag1.render(render_context) + "</div>\n"
                "<div>" + frag2.render(render_context) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()
        doc = FLMDocument(render_fn, environ)
        doc.initialize()

        result, _ = doc.render(fr)
        print(result)

        self.assertEqual(result, r"""
<main>
<div><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(\alpha+\beta=\gamma\)</span>.</div>
<div><p>We can also split text across multiple paragraphs, like this block of text here.</p>
<p>we can also have an equation, like this: <span id="equation-1" class="display-math env-align">\begin{align}
    1 + 3 - 5 = -1
\tag*{(1)}\end{align}</span></p></div>
</main>
        """.strip())




if __name__ == '__main__':
    unittest.main()
