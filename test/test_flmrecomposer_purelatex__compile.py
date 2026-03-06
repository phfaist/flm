"""
Compile-tests for ``default_purelatex_defs_makeatletter``.

Each test converts a small FLM document to pure LaTeX (via
``FLMPureLatexRecomposer``), wraps it in a minimal article document that
includes the FLM preamble definitions, and compiles it with ``pdflatex``.

If ``pdflatex`` is not found on PATH every test in this module is **skipped**
automatically.

Purpose
-------
Regression suite for ``default_purelatex_defs_makeatletter``.
"""

import os
import shutil
import subprocess
import tempfile
import textwrap
import unittest

from flm.flmrecomposer.purelatex import (
    FLMPureLatexRecomposer,
    default_purelatex_defs_makeatletter,
)
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features


# ---------------------------------------------------------------------------
# Skip guard – skip everything when pdflatex is not installed
# ---------------------------------------------------------------------------

PDFLATEX = shutil.which("pdflatex")
requires_pdflatex = unittest.skipUnless(
    PDFLATEX,
    "pdflatex not found on PATH – skipping LaTeX compile tests",
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _mk_env(**kwargs):
    return make_standard_environment(standard_features(**kwargs))


def _flm_to_latex(flm_source, recomposer_options=None, *, env=None):
    """Parse *flm_source* and recompose to pure LaTeX.  Returns the result dict
    ``{"latex": str, "packages": dict}``."""
    if env is None:
        env = _mk_env()
    frag = env.make_fragment(flm_source, what="compile-test")
    recomposer = FLMPureLatexRecomposer(recomposer_options or {})
    return recomposer.recompose_pure_latex(frag.nodes)


# The FLM preamble block (makeatletter + defs + makeatother).
# Packages are loaded *before* this block so that the defs can test for their
# presence (e.g. the conditional cleveref setup in \flmFinalPreambleSetup).
_FLM_PREAMBLE = (
    r"\makeatletter"
    + "\n"
    + default_purelatex_defs_makeatletter
    + "\n"
    + r"\makeatother"
)


def _make_document(body, *, extra_packages="", extra_preamble=""):
    r"""Wrap *body* in a minimal compilable article document.

    Package loading order: ``amsmath`` → ``hyperref`` → *extra_packages*
    → FLM defs (``\makeatletter`` … ``\makeatother``) → *extra_preamble*.

    ``hyperref`` is always loaded because several FLM macros (``\phantomsection``,
    ``\hyperref``, …) rely on it.
    """
    parts = [
        r"\documentclass{article}",
        r"\usepackage{amsmath}",
        r"\usepackage{hyperref}",
    ]
    if extra_packages:
        parts.append(extra_packages)
    parts.append(_FLM_PREAMBLE)
    parts.append(r"\flmFinalPreambleSetup")
    if extra_preamble:
        parts.append(extra_preamble)
    parts += [
        r"\begin{document}",
        body,
        r"\end{document}",
        "",
    ]
    return "\n".join(parts)


def _compile(tex_source, *, runs=1, timeout=60):
    """Write *tex_source* to a temp dir and compile with ``pdflatex``.

    Parameters
    ----------
    runs:
        Number of compilation passes (use ``runs=2`` when cross-references
        need a second pass to resolve).
    timeout:
        Per-run timeout in seconds.

    Returns
    -------
    (success: bool, log: str, aux: str)
        *log* is the content of the ``test.log`` file written by pdflatex
        (falling back to stdout+stderr if the file does not exist).  It
        includes all LaTeX warnings such as "There were undefined references."
        *aux* is the content of the ``test.aux`` file (empty string if absent),
        containing ``\\newlabel`` and ``\\zref@newlabel`` entries that record
        which labels were registered during compilation.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        tex_path = os.path.join(tmpdir, "test.tex")
        with open(tex_path, "w", encoding="utf-8") as fh:
            fh.write(tex_source)
        cmd = [PDFLATEX, "-interaction=nonstopmode", "-halt-on-error", "test.tex"]
        proc = None
        for _ in range(runs):
            proc = subprocess.run(
                cmd,
                cwd=tmpdir,
                capture_output=True,
                text=True,
                timeout=timeout,
            )
        assert proc is not None
        log_path = os.path.join(tmpdir, "test.log")
        if os.path.exists(log_path):
            with open(log_path, encoding="latin-1") as fh:
                log = fh.read()
        else:
            log = proc.stdout + proc.stderr
        aux_path = os.path.join(tmpdir, "test.aux")
        if os.path.exists(aux_path):
            with open(aux_path, encoding="latin-1") as fh:
                aux = fh.read()
        else:
            aux = ""
        return proc.returncode == 0, log, aux


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

class TestPureLatexDefsCompile(unittest.TestCase):
    """Compile-tests for ``default_purelatex_defs_makeatletter``."""

    maxDiff = None

    # --- structural ---------------------------------------------------------

    @requires_pdflatex
    def test_empty_body(self):
        """FLM preamble defs load without error on an empty document body."""
        tex = _make_document("")
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    @requires_pdflatex
    def test_plain_text(self):
        """Plain text paragraph compiles correctly."""
        result = _flm_to_latex("Hello, world!  This is a simple paragraph.")
        tex = _make_document(result["latex"])
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    # --- math ---------------------------------------------------------------

    @requires_pdflatex
    def test_display_math(self):
        r"""Display math ``\[ … \]`` compiles correctly."""
        result = _flm_to_latex(r"\[ A + B = C. \]")
        tex = _make_document(result["latex"])
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    @requires_pdflatex
    def test_align_environment(self):
        r"""``align`` environment without labels compiles correctly."""
        flm = textwrap.dedent(r"""
            \begin{align}
              x &= 0, \\
              y &= 1.
            \end{align}
        """).strip()
        result = _flm_to_latex(flm)
        tex = _make_document(result["latex"])
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    @requires_pdflatex
    def test_align_with_label_and_eqref(self):
        r"""``\label`` inside ``align`` + ``\eqref`` cross-reference.

        Uses only ``amsmath`` (no cleveref required).
        """
        flm = textwrap.dedent(r"""
            An equation:
            \begin{align}
              A + B = C.  \label{eq:one}
            \end{align}
            See~\eqref{eq:one}.
        """).strip()
        result = _flm_to_latex(
            flm,
            {"recomposer": {"safe_label_ref_types": {"ref": {"eq": True}}}},
        )
        tex = _make_document(result["latex"])
        ok, log, aux = _compile(tex, runs=2)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)
        self.assertNotIn("There were undefined references", log)
        self.assertIn(r"\newlabel{eq:one}", aux)

    # --- refs (zref-clever) -------------------------------------------------
    # The refactored defs use \zcref (zref-clever) instead of \cref (cleveref).
    # \NoCaseChange{\protect\zcref{…}} is the generated form; \NoCaseChange
    # is still provided by cleveref (which refs.py continues to request).

    @requires_pdflatex
    def test_ref_generates_zcref(self):
        r"""``\ref{…}`` produces ``\NoCaseChange{\protect\zcref{…}}``.

        Requires ``zref-clever`` (for ``\zcref``) and ``cleveref`` (for
        ``\NoCaseChange``).
        """
        result = _flm_to_latex(r"\ref{figure:one}")
        tex = _make_document(
            result["latex"],
            extra_packages="\n".join([
                r"\usepackage{zref-clever}",
                r"\usepackage[nameinlink]{cleveref}",
            ]),
        )
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    @requires_pdflatex
    def test_section_heading_with_ref(self):
        r"""``\section`` heading + ``\ref`` cross-reference."""
        flm = textwrap.dedent(r"""
            \section{Introduction}\label{sec:intro}

            See Section~\ref{sec:intro}.
        """).strip()
        result = _flm_to_latex(
            flm,
            {"recomposer": {"safe_label_ref_types": {"ref": {"sec": True}}}},
        )
        tex = _make_document(
            result["latex"],
            extra_packages="\n".join([
                r"\usepackage{zref-clever}",
                r"\usepackage[nameinlink]{cleveref}",
            ]),
        )
        ok, log, aux = _compile(tex, runs=2)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)
        self.assertNotIn("There were undefined references", log)
        self.assertIn(r"\newlabel{sec:intro}", aux)

    @requires_pdflatex
    def test_defterm_with_label(self):
        r"""``defterm`` environment – ``\flmLDefLabelText`` uses ``\zcsetup``
        (zref-clever) to attach a custom ref type to the label."""
        flm = textwrap.dedent(r"""
            \begin{defterm}{Hamiltonian}
            The \term{Hamiltonian} is an operator.
            \end{defterm}
            Refer to the \term{Hamiltonian}.
        """).strip()
        result = _flm_to_latex(flm)
        # Provide stub defs for presentation commands not in purelatex_defs
        extra_preamble = textwrap.dedent(r"""
            \providecommand\flmDeftermFormat{}
            \providecommand\flmDisplayTerm[1]{#1}
        """)
        tex = _make_document(
            result["latex"],
            extra_packages=r"\usepackage{zref-clever}",
            extra_preamble=extra_preamble,
        )
        ok, log, aux = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)
        self.assertIn("flm--internal--ref", aux)

    # --- verbatim -----------------------------------------------------------

    @requires_pdflatex
    def test_verbatimcode_environment(self):
        r"""``verbatimcode`` block environment (backed by the ``verbatim``
        package loaded via ``\flmRequirePackage{verbatim}``) compiles."""
        flm = textwrap.dedent(r"""
            Some code listing:
            \begin{verbatimcode}
            def hello():
                print("Hello, world!")
            \end{verbatimcode}
        """).strip()
        result = _flm_to_latex(flm)
        tex = _make_document(result["latex"])
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    @requires_pdflatex
    def test_inline_verbatim(self):
        r"""Inline ``\verbcode|…|`` uses ``\flmInlineVerb`` defined in the
        purelatex defs."""
        result = _flm_to_latex(r"Call \verbcode|my_func(x)| to compute the result.")
        tex = _make_document(result["latex"])
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)

    # --- quotes / environments ----------------------------------------------

    @requires_pdflatex
    def test_blockquote_environment(self):
        r"""``blockquote`` environment defined in the purelatex defs compiles.

        ``blockquote`` is part of ``FeatureQuote`` (not enabled by default),
        so the environment is explicitly enabled for this test.
        """
        flm = textwrap.dedent(r"""
            Some introductory text.

            \begin{blockquote}
            A famous quotation.
            \end{blockquote}

            Concluding text.
        """).strip()
        result = _flm_to_latex(flm, env=_mk_env(quote_environments=True))
        tex = _make_document(result["latex"])
        ok, log, _ = _compile(tex)
        self.assertTrue(ok, msg="pdflatex failed:\n" + log)


if __name__ == "__main__":
    unittest.main()
