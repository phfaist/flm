import unittest

### BEGIN_TEST_LLM_SKIP

import io

from llm.main.main import main


class TestRunMain(unittest.TestCase):
    
    maxDiff = None

    def test_simple_text(self):
        sout = io.StringIO()
        main(
            output=sout,
            llm_content=r"Hello \emph{world}!  Looking great today.",
            format='text'
        )
        self.assertEqual(sout.getvalue(), "Hello world! Looking great today.\n")

    def test_simple_html(self):
        sout = io.StringIO()
        main(
            output=sout,
            llm_content=r"Hello \emph{world}!  Looking great today.",
            format='html',
            minimal_document=False,
        )
        self.assertEqual(
            sout.getvalue(),
            """Hello <span class="textit">world</span>! Looking great today.\n"""
        )


    def test_simple_frontmatter(self):
        sout = io.StringIO()
        main(
            output=sout,
            llm_content=r"""---
llm:
  parsing:
    comment_start: '##'
    enable_comments: true
    dollar_inline_math_mode: true
    force_block_level: true
---
Hello \emph{world}!  Let $x$ and $y$ be real numbers. ## comments configured like this!
""",
            format='html',
            minimal_document=False,
            suppress_final_newline=True,
        )
        self.assertEqual(
            sout.getvalue(),
            """<p>Hello <span class="textit">world</span>! Let <span class="inline-math">$x$</span> and <span class="inline-math">$y$</span> be real numbers.</p>"""
        )




if __name__ == '__main__':
    unittest.main()

### END_TEST_LLM_SKIP
