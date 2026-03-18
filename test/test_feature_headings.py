import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.feature.headings import FeatureHeadings
from flm.feature.numbering import FeatureNumbering


def mk_environ(headings_feature=None, with_numbering=False):
    features = standard_features(headings=False)
    if headings_feature is None:
        headings_feature = FeatureHeadings()
    features.insert(0, headings_feature)
    if with_numbering:
        features.append(FeatureNumbering())
    return make_standard_environment(features)


def render(environ, src):
    frag = environ.make_fragment(src)
    doc = environ.make_document(frag.render)
    result, _ = doc.render(HtmlFragmentRenderer())
    return result


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
            '<p><span id="sec--Note" class="heading-level-4 heading-inline">Note</span> Some content.</p>'
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

    def test_subparagraph_inline(self):
        environ = mk_environ()
        result = render(environ, r'\subparagraph{Point} Content.')
        self.assertEqual(
            result,
            '<p><span id="sec--Point" class="heading-level-5 heading-inline">Point</span>'
            ' Content.</p>'
        )


    def test_ref_to_unnumbered_section(self):
        # \ref to an unnumbered section renders the heading text as link
        environ = mk_environ()
        result = render(environ, r'\section{Methods}\label{sec:methods} See Section~\ref{sec:methods}.')
        self.assertEqual(
            result,
            '<h1 id="sec-methods" class="heading-level-1">Methods</h1>\n'
            '<p>See Section&nbsp;<a href="#sec-methods" class="href-ref ref-sec">Methods</a>.</p>'
        )

    def test_ref_to_numbered_section(self):
        # \ref to a numbered section renders the section number as link
        environ = mk_environ(FeatureHeadings(numbering_section_depth=2), with_numbering=True)
        result = render(environ, r'\section{Intro}\label{sec:intro} See Section~\ref{sec:intro}.')
        self.assertEqual(
            result,
            '<h1 id="sec-intro" class="heading-level-1">1. Intro</h1>\n'
            '<p>See Section&nbsp;<a href="#sec-intro" class="href-ref ref-sec">§&thinsp;1</a>.</p>'
        )

    def test_ref_cross_sections(self):
        # \ref to an earlier section from a later one
        environ = mk_environ(FeatureHeadings(numbering_section_depth=1), with_numbering=True)
        result = render(
            environ,
            r'\section{Alpha}\label{sec:alpha}\section{Beta}\label{sec:beta} Back to~\ref{sec:alpha}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-alpha" class="heading-level-1">1. Alpha</h1>\n'
            '<h1 id="sec-beta" class="heading-level-1">2. Beta</h1>\n'
            '<p>Back to&nbsp;<a href="#sec-alpha" class="href-ref ref-sec">§&thinsp;1</a>.</p>'
        )

    def test_ref_cross_sections_2(self):
        # \ref to an earlier section from a later one
        environ = mk_environ(FeatureHeadings(numbering_section_depth=1), with_numbering=True)
        result = render(
            environ,
            r'\section{Alpha}\label{sec:alpha}\section{Beta}\label{sec:beta} Back to~\ref{sec:alpha,sec:beta}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-alpha" class="heading-level-1">1. Alpha</h1>\n'
            '<h1 id="sec-beta" class="heading-level-1">2. Beta</h1>\n'
            '<p>Back to&nbsp;<a href="#sec-alpha" class="href-ref refcnt-section">§&thinsp;</a><a href="#sec-alpha" class="href-ref refcnt-section">1</a>,<a href="#sec-beta" class="href-ref refcnt-section">2</a>.</p>'
        )


if __name__ == '__main__':
    unittest.main()
