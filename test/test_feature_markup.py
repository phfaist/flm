import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.feature.markup import FeatureMarkup


def mk_flm_environ(markup_feature):
    features = standard_features()
    features.append(markup_feature)
    return make_standard_environment(features)


# -------------------------------------------------------------------
# Init / config
# -------------------------------------------------------------------

class TestFeatureMarkupInit(unittest.TestCase):

    def test_defaults(self):
        m = FeatureMarkup()
        self.assertEqual(m.text_macros, {})
        self.assertEqual(m.semantic_environments, {})
        self.assertEqual(m.feature_name, 'markup')

    def test_custom_text_macros(self):
        m = FeatureMarkup(text_macros={'a': {'text_formats': ['textit']}})
        self.assertEqual(m.text_macros, {'a': {'text_formats': ['textit']}})
        self.assertEqual(m.semantic_environments, {})

    def test_custom_semantic_environments(self):
        m = FeatureMarkup(semantic_environments={'myenv': {'role': 'note'}})
        self.assertEqual(m.text_macros, {})
        self.assertEqual(m.semantic_environments, {'myenv': {'role': 'note'}})

    def test_no_managers(self):
        self.assertIsNone(FeatureMarkup.DocumentManager)
        self.assertIsNone(FeatureMarkup.RenderManager)


# -------------------------------------------------------------------
# add_latex_context_definitions
# -------------------------------------------------------------------

class TestFeatureMarkupLatexContextDefs(unittest.TestCase):

    def test_macro_defs(self):
        m = FeatureMarkup(text_macros={'a': {'text_formats': ['textit']}})
        defs = m.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 1)
        self.assertEqual(len(defs['environments']), 0)
        self.assertEqual(defs['macros'][0].macroname, 'a')
        self.assertEqual(defs['macros'][0].text_formats, ['textit'])

    def test_env_defs(self):
        m = FeatureMarkup(semantic_environments={'myenv': {'role': 'note'}})
        defs = m.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 0)
        self.assertEqual(len(defs['environments']), 1)
        self.assertEqual(defs['environments'][0].environmentname, 'myenv')
        self.assertEqual(defs['environments'][0].role, 'note')

    def test_empty_defs(self):
        m = FeatureMarkup()
        defs = m.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 0)
        self.assertEqual(len(defs['environments']), 0)

    def test_env_with_annotations(self):
        m = FeatureMarkup(
            semantic_environments={'myenv': {'role': 'note', 'annotations': ['a', 'b']}}
        )
        defs = m.add_latex_context_definitions()
        self.assertEqual(defs['environments'][0].annotations, ['a', 'b'])

    def test_multiple_macros_and_envs(self):
        m = FeatureMarkup(
            text_macros={
                'mybold': {'text_formats': ['textbf']},
                'myit': {'text_formats': ['textit']},
            },
            semantic_environments={
                'myblk': {'role': 'block'},
                'mybox': {'role': 'box'},
            },
        )
        defs = m.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 2)
        self.assertEqual(len(defs['environments']), 2)


# -------------------------------------------------------------------
# HTML rendering
# -------------------------------------------------------------------

class TestFeatureMarkupHtml(unittest.TestCase):

    maxDiff = None

    def test_simple_macro_textbf(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{my macro}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Test <span class="textbf">my macro</span>.'
        )

    def test_simple_macro_textit(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'myit': {'text_formats': ['textit']}
        }))
        frag = environ.make_fragment(
            r'Test \myit{italic}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Test <span class="textit">italic</span>.'
        )

    def test_multiple_text_formats(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf', 'textit']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold italic}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Test <span class="textbf textit">bold italic</span>.'
        )

    def test_simple_environment(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenviron': {'role': 'my-environ-role'}
        }))
        frag = environ.make_fragment(
            "Test \\begin{myenviron}\nenvironment content\n\\end{myenviron}",
            standalone_mode=True,
        )
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            '<p>Test</p>\n<div class="my-environ-role"> environment content </div>'
        )

    def test_environment_with_annotations(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenv': {'role': 'note', 'annotations': ['important', 'highlighted']}
        }))
        frag = environ.make_fragment(
            r'\begin{myenv}Some note.\end{myenv}', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            '<div class="note important highlighted">Some note.</div>'
        )

    def test_both_macros_and_envs(self):
        environ = mk_flm_environ(FeatureMarkup(
            text_macros={'mybold': {'text_formats': ['textbf']}},
            semantic_environments={'myblock': {'role': 'special'}},
        ))
        frag = environ.make_fragment(
            "Text with \\mybold{bold} and\n"
            "\\begin{myblock}\nblock content\n\\end{myblock}",
            standalone_mode=True,
        )
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            '<p>Text with <span class="textbf">bold</span> and</p>\n'
            '<div class="special"> block content </div>'
        )

    def test_empty_feature_plain_text(self):
        environ = mk_flm_environ(FeatureMarkup())
        frag = environ.make_fragment(r'Plain text.', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Plain text.'
        )


# -------------------------------------------------------------------
# Text renderer
# -------------------------------------------------------------------

class TestFeatureMarkupText(unittest.TestCase):

    maxDiff = None

    def test_macro_textbf(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold text}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Test bold text.'
        )

    def test_macro_multi_formats(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf', 'textit']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold italic}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Test bold italic.'
        )

    def test_environment(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenviron': {'role': 'my-environ-role'}
        }))
        frag = environ.make_fragment(
            "Test \\begin{myenviron}\nenvironment content\n\\end{myenviron}",
            standalone_mode=True,
        )
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Test\n\n environment content '
        )

    def test_env_with_annotations(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenv': {'role': 'note', 'annotations': ['important', 'highlighted']}
        }))
        frag = environ.make_fragment(
            r'\begin{myenv}Some note.\end{myenv}', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Some note.'
        )


# -------------------------------------------------------------------
# LaTeX renderer
# -------------------------------------------------------------------

class TestFeatureMarkupLatex(unittest.TestCase):

    maxDiff = None

    def test_macro_textbf(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold text}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(LatexFragmentRenderer()),
            r'Test \textbf{bold text}.'
        )

    def test_macro_multi_formats(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf', 'textit']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold italic}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(LatexFragmentRenderer()),
            r'Test \textbf{\textit{bold italic}}.'
        )

    def test_environment_quotation(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'quotation': {'role': 'quotation'}
        }))
        frag = environ.make_fragment(
            "Test \\begin{quotation}\nenvironment content\n\\end{quotation}",
            standalone_mode=True,
        )
        self.assertEqual(
            frag.render_standalone(LatexFragmentRenderer()),
            "Test\n\n"
            r"\begin{quotation} environment content \end{quotation}%" + "\n"
        )

    def test_env_with_annotations(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenv': {'role': 'note', 'annotations': ['important', 'highlighted']}
        }))
        frag = environ.make_fragment(
            r'\begin{myenv}Some note.\end{myenv}', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(LatexFragmentRenderer()),
            '% --- begin important,highlighted ---\n'
            'Some note.'
            '% --- end important,highlighted ---\n'
        )


# -------------------------------------------------------------------
# Markdown renderer
# -------------------------------------------------------------------

class TestFeatureMarkupMarkdown(unittest.TestCase):

    maxDiff = None

    def test_macro_textbf(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold text}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(MarkdownFragmentRenderer()),
            'Test **bold text**\\.'
        )

    def test_macro_multi_formats(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf', 'textit']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{bold italic}.', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(MarkdownFragmentRenderer()),
            'Test *bold italic*\\.'
        )

    def test_environment(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenviron': {'role': 'my-environ-role'}
        }))
        frag = environ.make_fragment(
            "Test \\begin{myenviron}\nenvironment content\n\\end{myenviron}",
            standalone_mode=True,
        )
        self.assertEqual(
            frag.render_standalone(MarkdownFragmentRenderer()),
            'Test\n\n environment content'
        )

    def test_env_with_annotations(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenv': {'role': 'note', 'annotations': ['important', 'highlighted']}
        }))
        frag = environ.make_fragment(
            r'\begin{myenv}Some note.\end{myenv}', standalone_mode=True
        )
        self.assertEqual(
            frag.render_standalone(MarkdownFragmentRenderer()),
            'Some note\\.'
        )


# -------------------------------------------------------------------
# Recomposer
# -------------------------------------------------------------------

class TestFeatureMarkupRecomposer(unittest.TestCase):

    maxDiff = None

    def test_macro_recompose(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {'text_formats': ['textbf']}
        }))
        frag = environ.make_fragment(
            r'Test \mymacro{content}.', standalone_mode=True
        )
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'Test \mymacro{content}.')

    def test_env_recompose(self):
        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenv': {'role': 'note'}
        }))
        frag = environ.make_fragment(
            r'\begin{myenv}block content\end{myenv}', standalone_mode=True
        )
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'\begin{myenv}block content\end{myenv}')

    def test_macro_multi_formats_recompose(self):
        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'myit': {'text_formats': ['textbf', 'textit']}
        }))
        frag = environ.make_fragment(
            r'Test \myit{styled}.', standalone_mode=True
        )
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'Test \myit{styled}.')


if __name__ == '__main__':
    unittest.main()
