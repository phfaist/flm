import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.feature.headings import FeatureHeadings, HeadingMacro
from flm.feature.numbering import FeatureNumbering
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer


def mk_environ(headings_feature=None, with_numbering=False):
    features = standard_features(headings=False)
    if headings_feature is None:
        headings_feature = FeatureHeadings()
    features.insert(0, headings_feature)
    if with_numbering:
        features.append(FeatureNumbering())
    return make_standard_environment(features)


def render(environ, src, fr_cls=HtmlFragmentRenderer):
    frag = environ.make_fragment(src)
    doc = environ.make_document(frag.render)
    fr = fr_cls()
    result, _ = doc.render(fr)
    return result


# -------------------------------------------------------------------
# HTML rendering tests
# -------------------------------------------------------------------

class TestFeatureHeadings(unittest.TestCase):

    maxDiff = None

    def test_section(self):
        environ = mk_environ()
        result = render(environ, r'\section{Introduction}')
        self.assertEqual(
            result,
            '<h1 id="sec--Introduction" class="heading-level-1">Introduction</h1>'
        )

    def test_subsection(self):
        environ = mk_environ()
        result = render(environ, r'\subsection{Background}')
        self.assertEqual(
            result,
            '<h2 id="sec--Background" class="heading-level-2">Background</h2>'
        )

    def test_subsubsection(self):
        environ = mk_environ()
        result = render(environ, r'\subsubsection{Details}')
        self.assertEqual(
            result,
            '<h3 id="sec--Details" class="heading-level-3">Details</h3>'
        )

    def test_paragraph_inline(self):
        environ = mk_environ()
        result = render(environ, r'\paragraph{Note} Some content.')
        self.assertEqual(
            result,
            '<p><span id="sec--Note" class="heading-level-4 heading-inline">'
            'Note</span> Some content.</p>'
        )

    def test_subparagraph_inline(self):
        environ = mk_environ()
        result = render(environ, r'\subparagraph{Point} Content.')
        self.assertEqual(
            result,
            '<p><span id="sec--Point" class="heading-level-5 heading-inline">'
            'Point</span> Content.</p>'
        )

    def test_subsubparagraph_inline(self):
        environ = mk_environ()
        result = render(environ, r'\subsubparagraph{Deep} Content.')
        self.assertEqual(
            result,
            '<p><span id="sec--Deep" class="heading-level-6 heading-inline">'
            'Deep</span> Content.</p>'
        )

    def test_section_starred_no_number(self):
        # \section* should produce a heading without a section number
        environ = mk_environ(FeatureHeadings(numbering_section_depth=3), with_numbering=True)
        result_numbered = render(environ, r'\section{Introduction}')
        result_starred = render(environ, r'\section*{Introduction}')
        self.assertEqual(
            result_numbered,
            '<h1 id="sec--Introduction" class="heading-level-1">1. Introduction</h1>'
        )
        self.assertEqual(
            result_starred,
            '<h1 id="sec--Introduction" class="heading-level-1">Introduction</h1>'
        )

    def test_section_with_numbering(self):
        environ = mk_environ(FeatureHeadings(numbering_section_depth=2), with_numbering=True)
        result = render(
            environ,
            r'\section{First} Some text. \subsection{Sub}'
        )
        self.assertEqual(
            result,
            '<h1 id="sec--First" class="heading-level-1">1. First</h1>\n'
            '<p>Some text.</p>\n'
            '<h2 id="sec--Sub" class="heading-level-2">1.1. Sub</h2>'
        )

    def test_section_target_id(self):
        environ = mk_environ()
        result = render(environ, r'\section{My Heading}')
        self.assertEqual(
            result,
            '<h1 id="sec--My-Heading" class="heading-level-1">My Heading</h1>'
        )

    def test_section_rich_text(self):
        # inline markup inside heading title
        environ = mk_environ()
        result = render(environ, r'\section{Introduction to \emph{FLM}}')
        self.assertEqual(
            result,
            '<h1 id="sec--Introduction-to-emph-FLM-" class="heading-level-1">'
            'Introduction to <span class="textit">FLM</span></h1>'
        )

    def test_section_sequential_numbering(self):
        # section counter increments across multiple headings
        environ = mk_environ(FeatureHeadings(numbering_section_depth=1), with_numbering=True)
        result = render(environ, r'\section{Alpha}\section{Beta}')
        self.assertEqual(
            result,
            '<h1 id="sec--Alpha" class="heading-level-1">1. Alpha</h1>\n'
            '<h1 id="sec--Beta" class="heading-level-1">2. Beta</h1>'
        )

    def test_subsection_numbering_resets(self):
        # subsection counter resets when a new section begins
        environ = mk_environ(FeatureHeadings(numbering_section_depth=2), with_numbering=True)
        result = render(
            environ,
            r'\section{A}\subsection{A1}\subsection{A2}\section{B}\subsection{B1}'
        )
        self.assertEqual(
            result,
            '<h1 id="sec--A" class="heading-level-1">1. A</h1>\n'
            '<h2 id="sec--A1" class="heading-level-2">1.1. A1</h2>\n'
            '<h2 id="sec--A2" class="heading-level-2">1.2. A2</h2>\n'
            '<h1 id="sec--B" class="heading-level-1">2. B</h1>\n'
            '<h2 id="sec--B1" class="heading-level-2">2.1. B1</h2>'
        )

    def test_three_level_numbering(self):
        environ = mk_environ(FeatureHeadings(numbering_section_depth=3), with_numbering=True)
        result = render(
            environ,
            r'\section{A}\subsection{A1}\subsubsection{A1a}'
        )
        self.assertEqual(
            result,
            '<h1 id="sec--A" class="heading-level-1">1. A</h1>\n'
            '<h2 id="sec--A1" class="heading-level-2">1.1. A1</h2>\n'
            '<h3 id="sec--A1a" class="heading-level-3">1.1.1. A1a</h3>'
        )

    def test_section_with_label(self):
        # \label{sec:...} sets the anchor id from the label
        environ = mk_environ()
        result = render(environ, r'\section{Methodology}\label{sec:method}')
        self.assertEqual(
            result,
            '<h1 id="sec-method" class="heading-level-1">Methodology</h1>'
        )

    def test_custom_section_commands(self):
        # custom section_commands_by_level replaces default commands
        environ = mk_environ(FeatureHeadings(section_commands_by_level={1: dict(cmdname='chapter')}))
        result = render(environ, r'\chapter{My Chapter}')
        self.assertEqual(
            result,
            '<h1 id="sec--My-Chapter" class="heading-level-1">My Chapter</h1>'
        )

    def test_duplicate_heading_ids(self):
        # same heading text gets incremented target ids
        environ = mk_environ()
        result = render(environ, r'\section{Intro}\section{Intro}')
        self.assertEqual(
            result,
            '<h1 id="sec--Intro" class="heading-level-1">Intro</h1>\n'
            '<h1 id="sec--Intro-2" class="heading-level-1">Intro</h1>'
        )

    def test_long_heading_truncates_target_id(self):
        environ = mk_environ()
        result = render(
            environ,
            r'\section{A Very Long Heading That Exceeds Thirty Two Characters}'
        )
        self.assertEqual(
            result,
            '<h1 id="sec--A-Very-Long-Heading-That-Ex" class="heading-level-1">'
            'A Very Long Heading That Exceeds Thirty Two Characters</h1>'
        )

    def test_empty_heading(self):
        environ = mk_environ()
        result = render(environ, r'\section{}')
        self.assertEqual(
            result,
            '<h1 id="sec--" class="heading-level-1"></h1>'
        )

    def test_special_characters_in_heading(self):
        environ = mk_environ()
        result = render(environ, r'\section{Hello \& World}')
        self.assertEqual(
            result,
            '<h1 id="sec--Hello-World" class="heading-level-1">'
            'Hello &amp; World</h1>'
        )


# -------------------------------------------------------------------
# Ref tests
# -------------------------------------------------------------------

class TestFeatureHeadingsRefs(unittest.TestCase):

    maxDiff = None

    def test_ref_to_unnumbered_section(self):
        # \ref to an unnumbered section renders the heading text as link
        environ = mk_environ()
        result = render(
            environ,
            r'\section{Methods}\label{sec:methods} See Section~\ref{sec:methods}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-methods" class="heading-level-1">Methods</h1>\n'
            '<p>See Section&nbsp;'
            '<a href="#sec-methods" class="href-ref ref-sec">Methods</a>.</p>'
        )

    def test_ref_to_numbered_section(self):
        # \ref to a numbered section renders the section number as link
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=2), with_numbering=True
        )
        result = render(
            environ,
            r'\section{Intro}\label{sec:intro} See Section~\ref{sec:intro}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-intro" class="heading-level-1">1. Intro</h1>\n'
            '<p>See Section&nbsp;'
            '<a href="#sec-intro" class="href-ref ref-sec">'
            '§&thinsp;1</a>.</p>'
        )

    def test_ref_cross_sections(self):
        # \ref to an earlier section from a later one
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{Alpha}\label{sec:alpha}'
            r'\section{Beta}\label{sec:beta} Back to~\ref{sec:alpha}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-alpha" class="heading-level-1">1. Alpha</h1>\n'
            '<h1 id="sec-beta" class="heading-level-1">2. Beta</h1>\n'
            '<p>Back to&nbsp;'
            '<a href="#sec-alpha" class="href-ref ref-sec">'
            '§&thinsp;1</a>.</p>'
        )

    def test_ref_cross_sections_2(self):
        # \ref to multiple numbered sections in a single \ref
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{Alpha}\label{sec:alpha}'
            r'\section{Beta}\label{sec:beta}'
            r' Back to~\ref{sec:alpha,sec:beta}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-alpha" class="heading-level-1">1. Alpha</h1>\n'
            '<h1 id="sec-beta" class="heading-level-1">2. Beta</h1>\n'
            '<p>Back to&nbsp;'
            '<a href="#sec-alpha" class="href-ref refcnt-section">§&thinsp;</a>'
            '<a href="#sec-alpha" class="href-ref refcnt-section">1</a>,'
            '<a href="#sec-beta" class="href-ref refcnt-section">2</a>.</p>'
        )


# -------------------------------------------------------------------
# Alternative renderers
# -------------------------------------------------------------------

class TestFeatureHeadingsTextRenderer(unittest.TestCase):
    """Tests using TextFragmentRenderer which triggers two-pass rendering."""

    maxDiff = None

    def test_section_text(self):
        environ = mk_environ()
        result = render(environ, r'\section{Introduction}', TextFragmentRenderer)
        self.assertEqual(result, 'Introduction\n============')

    def test_paragraph_text(self):
        environ = mk_environ()
        result = render(
            environ, r'\paragraph{Note} Some content.', TextFragmentRenderer
        )
        self.assertEqual(result, 'Note:  Some content.')

    def test_numbered_section_text(self):
        """Single numbered section gets number 1, not 2 (two-pass fix)."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(environ, r'\section{Intro}', TextFragmentRenderer)
        self.assertEqual(result, '1. Intro\n========')

    def test_numbered_sequential_sections_text(self):
        """Two numbered sections get 1, 2 — not 3, 4 (two-pass fix)."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ, r'\section{Alpha}\section{Beta}', TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. Alpha\n========\n\n2. Beta\n======='
        )

    def test_numbered_subsection_resets_text(self):
        """Subsection counters reset under new sections (two-pass fix)."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=2), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{A1}\subsection{A2}'
            r'\section{B}\subsection{B1}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\n'
            '1.1. A1\n-------\n\n'
            '1.2. A2\n-------\n\n'
            '2. B\n====\n\n'
            '2.1. B1\n-------'
        )

    def test_three_level_numbering_text(self):
        """Three-level numbering is correct through two-pass rendering."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=3), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{A1}\subsubsection{A1a}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\n'
            '1.1. A1\n-------\n\n'
            '1.1.1. A1a\n~~~~~~~~~~'
        )

    def test_starred_mixed_with_numbered_text(self):
        """Starred sections don't consume a number (two-pass fix)."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=2), with_numbering=True
        )
        result = render(
            environ,
            r'\section{X}\section*{Y}\section{Z}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. X\n====\n\nY\n=\n\n2. Z\n===='
        )

    def test_numbered_section_with_ref_text(self):
        """Ref to a numbered section works correctly through two-pass rendering."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{Intro}\label{sec:intro} See Section~\ref{sec:intro}.',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. Intro\n========\n\nSee Section\xa0\u00a7\u20091.'
        )

    def test_numbered_paragraph_text(self):
        """Inline heading with numbering through two-pass rendering."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=4), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\paragraph{P} Content.',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\na. P:  Content.'
        )

    def test_subsection_text(self):
        environ = mk_environ()
        result = render(environ, r'\subsection{Sub}', TextFragmentRenderer)
        self.assertEqual(result, 'Sub\n---')

    def test_subsubsection_text(self):
        environ = mk_environ()
        result = render(environ, r'\subsubsection{Deep}', TextFragmentRenderer)
        self.assertEqual(result, 'Deep\n~~~~')

    def test_five_numbered_sections_text(self):
        """Many sections number correctly through two-pass rendering."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\section{B}\section{C}\section{D}\section{E}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\n2. B\n====\n\n3. C\n====\n\n'
            '4. D\n====\n\n5. E\n===='
        )

    def test_duplicate_heading_text_numbered_text(self):
        """Duplicate heading text still gets sequential numbers."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{Intro}\section{Intro}\section{Intro}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. Intro\n========\n\n2. Intro\n========\n\n3. Intro\n========'
        )

    def test_duplicate_heading_text_unnumbered_text(self):
        environ = mk_environ()
        result = render(
            environ,
            r'\section{Same}\section{Same}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            'Same\n====\n\nSame\n===='
        )

    def test_depth_boundary_subsection_unnumbered_text(self):
        """With depth=1, subsections remain unnumbered."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{Sub}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\nSub\n---'
        )

    def test_numbering_section_depth_true_text(self):
        """numbering_section_depth=True numbers all levels."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=True), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{B}\subsubsection{C}\paragraph{D} text',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\n1.1. B\n------\n\n'
            '1.1.1. C\n~~~~~~~~\n\na. D:  text'
        )

    def test_three_level_numbering_full_reset_text(self):
        """Three-level numbering resets correctly across sections."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=3), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{A1}\subsubsection{A1a}\subsubsection{A1b}'
            r'\subsection{A2}\subsubsection{A2a}'
            r'\section{B}\subsection{B1}\subsubsection{B1a}',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\n'
            '1.1. A1\n-------\n\n'
            '1.1.1. A1a\n~~~~~~~~~~\n\n'
            '1.1.2. A1b\n~~~~~~~~~~\n\n'
            '1.2. A2\n-------\n\n'
            '1.2.1. A2a\n~~~~~~~~~~\n\n'
            '2. B\n====\n\n'
            '2.1. B1\n-------\n\n'
            '2.1.1. B1a\n~~~~~~~~~~'
        )

    def test_ref_to_numbered_subsection_text(self):
        """Ref to a numbered subsection works through two-pass rendering."""
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=2), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{Sub}\label{sec:sub}'
            r' Ref to~\ref{sec:sub}.',
            TextFragmentRenderer
        )
        self.assertEqual(
            result,
            '1. A\n====\n\n'
            '1.1. Sub\n--------\n\n'
            'Ref to\xa0\u00a7\u20091.1.'
        )


class TestFeatureHeadingsLatexRenderer(unittest.TestCase):

    maxDiff = None

    def test_section_latex(self):
        environ = mk_environ()
        result = render(environ, r'\section{Introduction}', LatexFragmentRenderer)
        self.assertEqual(
            result,
            '\\section{Introduction}%\n\\label{x:sec--Introduction}%\n'
        )

    def test_subsection_latex(self):
        environ = mk_environ()
        result = render(environ, r'\subsection{Details}', LatexFragmentRenderer)
        self.assertEqual(
            result,
            '\\subsection{Details}%\n\\label{x:sec--Details}%\n'
        )

    def test_paragraph_latex(self):
        environ = mk_environ()
        result = render(
            environ, r'\paragraph{Note} Some content.', LatexFragmentRenderer
        )
        self.assertEqual(
            result,
            '\\paragraph{Note}%\n\\label{x:sec--Note}%\nSome content.\n'
        )

    def test_section_numbered_latex(self):
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=2), with_numbering=True
        )
        result = render(
            environ,
            r'\section{First}\subsection{Sub}',
            LatexFragmentRenderer
        )
        self.assertEqual(
            result,
            '\\section{1. First}%\n\\label{x:sec--First}%\n\n'
            '\\subsection{1.1. Sub}%\n\\label{x:sec--Sub}%\n'
        )


    def test_starred_mixed_with_numbered_latex(self):
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\section*{B}\section{C}',
            LatexFragmentRenderer
        )
        self.assertEqual(
            result,
            '\\section{1. A}%\n\\label{x:sec--A}%\n\n'
            '\\section{B}%\n\\label{x:sec--B}%\n\n'
            '\\section{2. C}%\n\\label{x:sec--C}%\n'
        )


class TestFeatureHeadingsMarkdownRenderer(unittest.TestCase):

    maxDiff = None

    def test_section_markdown(self):
        environ = mk_environ()
        result = render(environ, r'\section{Introduction}', MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '# <a name="sec--Introduction"></a> Introduction'
        )

    def test_subsection_markdown(self):
        environ = mk_environ()
        result = render(environ, r'\subsection{Details}', MarkdownFragmentRenderer)
        self.assertEqual(
            result,
            '## <a name="sec--Details"></a> Details'
        )

    def test_paragraph_markdown(self):
        environ = mk_environ()
        result = render(
            environ, r'\paragraph{Note} Content here.', MarkdownFragmentRenderer
        )
        self.assertEqual(
            result,
            '#### <a name="sec--Note"></a> Note\nContent here\\.'
        )

    def test_section_numbered_markdown(self):
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=1), with_numbering=True
        )
        result = render(
            environ, r'\section{Intro}', MarkdownFragmentRenderer
        )
        self.assertEqual(
            result,
            '# <a name="sec--Intro"></a> 1\\. Intro'
        )

    def test_numbered_section_and_subsection_markdown(self):
        environ = mk_environ(
            FeatureHeadings(numbering_section_depth=2), with_numbering=True
        )
        result = render(
            environ,
            r'\section{A}\subsection{A1}\section{B}',
            MarkdownFragmentRenderer
        )
        self.assertEqual(
            result,
            '# <a name="sec--A"></a> 1\\. A\n\n'
            '## <a name="sec--A1"></a> 1\\.1\\. A1\n\n'
            '# <a name="sec--B"></a> 2\\. B'
        )


# -------------------------------------------------------------------
# Recomposer tests
# -------------------------------------------------------------------

class TestFeatureHeadingsRecomposer(unittest.TestCase):

    maxDiff = None

    def test_recompose_section(self):
        environ = mk_environ()
        frag = environ.make_fragment(r'\section{Introduction}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(result['latex'], '\\section{Introduction}')

    def test_recompose_subsection(self):
        environ = mk_environ()
        frag = environ.make_fragment(r'\subsection{Sub}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(result['latex'], '\\subsection{Sub}')

    def test_recompose_starred(self):
        environ = mk_environ()
        frag = environ.make_fragment(r'\section*{Appendix}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(result['latex'], '\\section*{Appendix}')

    def test_recompose_with_label(self):
        environ = mk_environ()
        frag = environ.make_fragment(r'\section{Methods}\label{sec:methods}')
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(result['latex'], '\\section{Methods}\\label{ref1}')

    def test_recompose_macroname_mapping(self):
        environ = mk_environ()
        frag = environ.make_fragment(r'\section{Intro}')
        recomposer = FLMPureLatexRecomposer({
            'headings': {'macroname_mapping': {'section': 'chapter'}}
        })
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(result['latex'], '\\chapter{Intro}')


# -------------------------------------------------------------------
# Initialization tests
# -------------------------------------------------------------------

class TestFeatureHeadingsInit(unittest.TestCase):

    def test_default_construction(self):
        fh = FeatureHeadings()
        self.assertEqual(fh.feature_name, 'headings')
        self.assertEqual(fh.numbering_section_depth, False)

    def test_numbering_section_depth_none_becomes_false(self):
        fh = FeatureHeadings(numbering_section_depth=None)
        self.assertEqual(fh.numbering_section_depth, False)

    def test_section_commands_by_level(self):
        fh = FeatureHeadings()
        self.assertTrue(1 in fh.section_commands_by_level)
        self.assertEqual(fh.section_commands_by_level[1].cmdname, 'section')
        self.assertEqual(fh.section_commands_by_level[1].inline, False)
        self.assertEqual(fh.section_commands_by_level[4].cmdname, 'paragraph')
        self.assertEqual(fh.section_commands_by_level[4].inline, True)

    def test_custom_section_commands_from_str(self):
        fh = FeatureHeadings(section_commands_by_level={1: 'chapter'})
        self.assertEqual(fh.section_commands_by_level[1].cmdname, 'chapter')
        self.assertEqual(fh.section_commands_by_level[1].inline, False)

    def test_add_latex_context_definitions(self):
        fh = FeatureHeadings()
        defs = fh.add_latex_context_definitions()
        self.assertTrue('macros' in defs)
        macro_names = [m.macroname for m in defs['macros']]
        self.assertTrue('section' in macro_names)
        self.assertTrue('subsection' in macro_names)
        self.assertTrue('paragraph' in macro_names)


class TestHeadingMacroInit(unittest.TestCase):

    def test_defaults(self):
        hm = HeadingMacro('section')
        self.assertEqual(hm.macroname, 'section')
        self.assertEqual(hm.heading_level, 1)
        self.assertEqual(hm.inline_heading, False)
        self.assertEqual(hm.unregistered_heading, False)
        self.assertIsNone(hm.target_id)
        self.assertTrue(hm.is_block_level)
        self.assertTrue(hm.allowed_in_standalone_mode)

    def test_inline_heading(self):
        hm = HeadingMacro('paragraph', heading_level=4, inline_heading=True)
        self.assertEqual(hm.heading_level, 4)
        self.assertTrue(hm.inline_heading)

    def test_unregistered_heading(self):
        hm = HeadingMacro('section', unregistered_heading=True, target_id='my-target')
        self.assertTrue(hm.unregistered_heading)
        self.assertEqual(hm.target_id, 'my-target')
        self.assertEqual(hm.heading_level, 1)
        self.assertEqual(hm.inline_heading, False)

    def test_unregistered_heading_defaults(self):
        hm = HeadingMacro('section')
        self.assertFalse(hm.unregistered_heading)
        self.assertIsNone(hm.target_id)

    def test_fields(self):
        hm = HeadingMacro('section', heading_level=1, inline_heading=False)
        self.assertEqual(
            hm._fields,
            ('macroname', 'heading_level', 'inline_heading')
        )


if __name__ == '__main__':
    unittest.main()
