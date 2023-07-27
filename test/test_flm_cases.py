import unittest
import re

from pylatexenc.latexnodes import LatexWalkerParseError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer

from flm.feature import refs as feature_refs



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


from .cases import (
    frag1, frag2,
)


class TestFlmCases(unittest.TestCase):

    maxDiff = None

    def test_case_frag1(self):
        run_test_case(self, frag1.case_info)

    def test_case_frag2(self):
        run_test_case(self, frag2.case_info)





def run_test_case(test_case_obj, case_info):

    environ = mk_flm_environ( **dict( case_info.get('standard_features', {}) ) )

    fragment = environ.make_fragment( case_info['source'] () )

    result = render_fragment(
        environ,
        fragment,
        **{ k: v
            for k,v in case_info.items()
            if k in ('render_to', 'endnotes',) }
    )

    print(result)
    test_case_obj.assertEqual(
        result,
        case_info['render_result'] ()
    )


def render_fragment(environment, fragment, *, render_to='html', endnotes=False):

    def render_fn(render_context):
        return fragment.render(render_context, is_block_level=True)

    doc = environment.make_document(render_fn)

    if render_to == 'html':

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = "\n"
        result, render_context = doc.render(fr)

    else:

        raise ValueError("Invalid render_to: " + repr(render_to))


    if endnotes:

        # add footnotes
        endnotes_mgr = render_context.feature_render_manager('endnotes')
        result = render_context.fragment_renderer.render_join_blocks([
            result,
            render_context.fragment_renderer.render_heading(
                environment.make_fragment('Footnotes').nodes,
                heading_level=1,
                render_context=render_context,
            ),
            endnotes_mgr.render_endnotes_category('footnote'),
        ], render_context)


    return result
