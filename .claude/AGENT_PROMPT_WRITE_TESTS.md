# Agent Prompt: Write Comprehensive Tests for FLM

## Task

Write high-quality, comprehensive unit tests for the **flm** Python library, going module by module. Each module should get a dedicated test file (or have its existing test file improved). Tests must assert **exact full outputs** using `assertEqual`, not loose substring checks with `assertTrue(x in result)`.

Tests for the `flm.main` module and its submodules go in `test/main/`. All other tests go in `test/`.

## Project Overview

**flm-core** is a Python library implementing *Flexible Latex-like Markup* (FLM): an approximate subset of LaTeX syntax parsed via `pylatexenc` (v3 pre-release) and rendered to multiple output formats (HTML, LaTeX, plain text, Markdown). The codebase is also transpiled to JavaScript via Transcrypt, which constrains Python idioms.

Packages are managed via `poetry`. Run tests with `poetry run python -m pytest test/ -v`.

## Critical Rules

### Transcrypt-Compatible Assertions Only

In unit tests, stick to these assertion methods ONLY:
- `self.assertTrue`, `self.assertFalse`
- `self.assertEqual`
- `self.assertIs`, `self.assertIsNot`
- `self.assertIsNone`, `self.assertIsNotNone`
- `self.assertRaises`

**Do NOT use**: `assertIn`, `assertNotIn`, `assertIsInstance`, `assertAlmostEqual`, `assertGreater`, `assertLess`, `assertRegex`, `assertCountEqual`, or any other assertion method.

If you need an "in" check, write `self.assertTrue(x in items)`.

### Transcrypt-Compatible Python

- Avoid negative array/string index lookups or slices (no `x[-1]`, no `x[-2:]`).
- Cast dictionaries from public API to `dict(x)` before calling `.items()`, `.keys()`, `.values()`.
- Only use `import` statements at the top of a module, not inside a function or method.

### Test Quality Requirements

1. **Exact output matching**: Use `self.assertEqual(result, expected)` with the full expected output. Do NOT use `self.assertTrue(substring in result)` for testing rendered output.
2. **To get exact outputs**: render the FLM input first by running the code, capture the actual output, then verify it is correct, then use that as the assertEqual expectation. You can run snippets via `poetry run python -c "..."` to see actual outputs before writing assertions.
3. **Test file naming**: `test/test_<module>.py` for core and feature modules, `test/main/test_main_<submodule>.py` for main submodules.
4. **One test class per logical group** (e.g. `TestFeatureQuoteRendering`, `TestFeatureQuoteInit`, `TestFeatureQuoteRecomposer`).
5. **Set `maxDiff = None`** on test classes that compare rendered output.
6. **Error cases**: Test that invalid input raises appropriate exceptions using `self.assertRaises`.
7. **Do not refactor or change any source code** — only write/modify test files.
8. **Flag suspicious behavior** — do NOT change a test to make it pass if you suspect a bug in the code.  Mark the source code for review with a comment "# REVIEW: <NOTE>".
9. **Flag Transcrypt-incompatible patterns in source code** — If you encounter source code that uses Python features unsupported by Transcrypt (e.g. `RuntimeError`, calling `.keys()`/`.items()`/`.values()` on dicts received from external input without `dict()` wrapping, or other patterns that would break in JS), flag them for review. Add a `# REVIEW: <NOTE>` comment in the source and mention it when reporting results. Do NOT remove tests that fail in JS. In this case, stop and ask for user input.
10. **Flag incorrect `allowed_in_standalone_mode`** — Simple macros/specials that take no block-level content and produce no block-level output should generally have `allowed_in_standalone_mode = True`. If you find a macro or specials class where this seems wrong (e.g. a simple no-argument macro that defaults to `False`), flag it for review.

### Rendering Pattern

Features with `allowed_in_standalone_mode = False` (most environments) require the document rendering pattern:

```python
def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result
```

Standalone-capable macros can use the simpler pattern:

```python
frag = environ.make_fragment(src, standalone_mode=True)
result = frag.render_standalone(HtmlFragmentRenderer())
```

### Recomposer Testing Pattern

```python
environ = mk_flm_environ()
frag = environ.make_fragment(flm_input.strip())
recomposer = FLMPureLatexRecomposer(options if options else {})
result = recomposer.recompose_pure_latex(frag.nodes)
latex_output = result['latex']
```

### Environment Setup Pattern

```python
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features

def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)
```

For features not in `standard_features()` or needing custom configuration:

```python
def mk_flm_environ(**kwargs):
    features = standard_features(some_feature=False)  # disable default
    features.append(CustomFeature(**custom_args))      # add custom
    return make_standard_environment(features)
```

## Modules to Test

Work through these modules in order. For each module, read the source, read any existing tests, then write or improve the test file. If a test file already exists and is high quality, skip it. If it exists but uses loose assertions or has poor coverage, rewrite it.

### Priority 1 — Core Modules

| Module | Source | Test File |
|--------|--------|-----------|
| counter | `flm/counter.py` | `test/test_counter.py` |
| flmspecinfo | `flm/flmspecinfo.py` | `test/test_flmspecinfo.py` |
| flmfragment | `flm/flmfragment.py` | `test/test_flmfragment.py` |
| flmdocument | `flm/flmdocument.py` | `test/test_flmdocument.py` |
| flmenvironment | `flm/flmenvironment.py` | `test/test_flmenvironment.py` |
| flmdump | `flm/flmdump.py` | `test/test_flmdump.py` |

### Priority 2 — Feature Modules

| Module | Source | Test File |
|--------|--------|-----------|
| baseformatting | `flm/feature/baseformatting.py` | `test/test_feature_baseformatting.py` |
| href | `flm/feature/href.py` | `test/test_feature_href.py` |
| verbatim | `flm/feature/verbatim.py` | `test/test_feature_verbatim.py` |
| math | `flm/feature/math.py` | `test/test_feature_math.py` |
| enumeration | `flm/feature/enumeration.py` | `test/test_feature_enumeration.py` |
| headings | `flm/feature/headings.py` | `test/test_feature_headings.py` |
| refs | `flm/feature/refs.py` | `test/test_feature_refs.py` |
| endnotes | `flm/feature/endnotes.py` | `test/test_feature_endnotes.py` |
| cite | `flm/feature/cite.py` | `test/test_feature_cite.py` |
| floats | `flm/feature/floats.py` | `test/test_feature_floats.py` |
| defterm | `flm/feature/defterm.py` | `test/test_feature_defterm.py` |
| theorems | `flm/feature/theorems.py` | `test/test_feature_theorems.py` |
| substmacros | `flm/feature/substmacros.py` | `test/test_feature_substmacros.py` |
| quote | `flm/feature/quote.py` | `test/test_feature_quote.py` |
| markup | `flm/feature/markup.py` | `test/test_feature_markup.py` |
| annotations | `flm/feature/annotations.py` | `test/test_feature_annotations.py` |
| numbering | `flm/feature/numbering.py` | `test/test_feature_numbering.py` |
| cells | `flm/feature/cells.py` | `test/test_feature_cells.py` |
| graphics | `flm/feature/graphics.py` | `test/test_feature_graphics.py` |

### Priority 3 — Renderers and Recomposer

| Module | Source | Test File |
|--------|--------|-----------|
| fragmentrenderer base | `flm/fragmentrenderer/_base.py` | `test/test_fragmentrenderer_base.py` |
| fragmentrenderer html | `flm/fragmentrenderer/html.py` | `test/test_fragmentrenderer_html.py` |
| fragmentrenderer latex | `flm/fragmentrenderer/latex.py` | `test/test_fragmentrenderer_latex.py` |
| fragmentrenderer text | `flm/fragmentrenderer/text.py` | `test/test_fragmentrenderer_text.py` |
| fragmentrenderer markdown | `flm/fragmentrenderer/markdown.py` | `test/test_fragmentrenderer_markdown.py` |
| recomposer purelatex | `flm/flmrecomposer/purelatex.py` | `test/test_flmrecomposer_purelatex.py` |

### Priority 4 — Main Module (lightweight)

Tests for main module should be **lightweight** — test only essential functionality, not exhaustive edge cases. These tests go in `test/main/`.

| Module | Source | Test File |
|--------|--------|-----------|
| configmerger | `flm/main/configmerger.py` | `test/main/test_main_configmerger.py` |
| main | `flm/main/main.py` | `test/main/test_main.py` |
| importclass | `flm/main/importclass.py` | `test/main/test_main_importclass.py` |
| template | `flm/main/template.py` | `test/main/test_main_template.py` |

Skip these main submodules (they involve filesystem/process operations not suitable for unit tests): `watch.py`, `watch_hotreload.py`, `watch_util.py`, `oshelper.py`, `run.py`, `_find_exe.py`, `_inspectimagefile.py`.

## Workflow Per Module

1. **Read the source module** to understand its public API, classes, and methods.
2. **Read the existing test file** (if any) to understand current coverage and quality.
3. **Decide**: if existing tests are already high quality with exact assertions and good coverage, skip to the next module. If they need improvement, **keep the existing tests** and add any additional test cases you deem necessary.  If existing tests are incomplete, add more tests to test other inputs or edge cases that were missed.
4. **Capture actual outputs** by running the code first:
   ```bash
   poetry run python -c "
   from flm.flmenvironment import make_standard_environment
   from flm.stdfeatures import standard_features
   from flm.fragmentrenderer.html import HtmlFragmentRenderer
   environ = make_standard_environment(standard_features())
   frag = environ.make_fragment(r'\textbf{Hello}', standalone_mode=True)
   print(repr(frag.render_standalone(HtmlFragmentRenderer())))
   "
   ```
5. **CHECK THE OUTPUTS**: Make sure that the outputs are indeed expected for the test code you designed.  Flag any suspicious behavior or suspected bug with a comment `# REVIEW: <note>` in the source code.  Only fix the source code itself if the error is a typo or a missing import.  In all other cases, keep the failing test and mark it to be skipped pending code review.
6. **Write the test file** with exact assertEqual assertions using the captured outputs, for test cases whose outputs match the expected output.
7. **Run the tests** for that module: `poetry run python -m pytest test/test_<module>.py -v`
8. **Fix any failures** — if the expected output doesn't match, re-capture and correct.  Flag suspicious behavior (see point 5.) instead of adapting the test itself to the suspicious behavior.
9. **Run the full test suite** periodically: `poetry run python -m pytest test/ -v` to check for regressions.
10. **When you're finished with the module** — read the module again and identify classes, functions, methods, or attributes that are not sufficiently well tested according to the standards specified here.  If you find any, complete the tests.
11. **Check also the JS-transpiled tests** — by running `(cd flm-js && poetry run python ./generate_flm_js.py --pylatexenc-src-dir=../../pylatexenc --delete-target-dir --compile-tests && node test-flm-js/runtests.js)`.  If tests fail, identify the failure location and identify its cause. Do not change the test logic. Remove the test only if it is clearly unnecessary. Ask for the user's input if fixing the test involves anything else than minor refactoring to remove a feature not supported by transcrypt. JS output and Python output must be the same (except for string-formatted floating-point numbers).


## What to Test Per Module

For each module, cover these categories as applicable:

- **Construction / initialization**: Feature/class instantiation with default and custom arguments
- **Core rendering**: Each macro and environment the feature provides, rendered to HTML
- **Formatting variants**: Different options, flags, starred variants
- **Cross-feature interaction**: Refs to equations, headings with numbering, etc.
- **Error handling**: Invalid input, missing labels, duplicate labels
- **Recomposer output**: Pure LaTeX recomposition where applicable
- **Edge cases**: Empty input, special characters, Unicode
- **Self-documentation attributes and methods**: Do not write tests for `get_flm_doc()` and similar self-documentation methods.


## Example Test File Structure

```python
import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


class TestFeatureXxxRendering(unittest.TestCase):

    maxDiff = None

    def test_basic_usage(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r'\somemacro{content}')
        self.assertEqual(
            result,
            '<p>expected full HTML output here</p>'
        )

    def test_error_case(self):
        environ = mk_flm_environ()
        with self.assertRaises(SomeException):
            render_doc(environ, r'\invalid input')


if __name__ == '__main__':
    unittest.main()
```

## Completed Modules

The following modules have already been processed by an agent following the present instructions.  Do not process them again.  Skip to the next module to be processed.  After completing tests for a module, add it to the following table.

| Module | Test File | Tests (before → after) | Notes |
|--------|-----------|------------------------|-------|
| counter | `test/test_counter.py` | 32 → 68 | Added alpha, roman, fnsymbol, custom digits, unicode counters, formatter tests |
| flmspecinfo | `test/test_flmspecinfo.py` | 1 → 31 | Full rewrite; covers all public classes and helpers |
| flmfragment | `test/test_flmfragment.py` | 6 → 29 | Added parse, rendering, is_empty, repr, attributes, tolerant parsing, node visitor tests |
| flmdocument | `test/test_flmdocument.py` | 4 → 19 | Added attributes, render context, dict/list render, enable_features tests |
| flmenvironment | `test/test_flmenvironment.py` | 15 → 64 | Added FLMParsingState, FLMArgumentSpec, NodesFinalizer, standard_parsing_state, error messages, rendering tests |
| flmdump | `test/test_flmdump.py` | 7 → 16 | Added clear, get_keys, version mismatch, roundtrip HTML, helpers tests |
| baseformatting | `test/test_feature_baseformatting.py` | 0 → 64 | New file; all constructs tested with HTML, text, LaTeX, and markdown renderers |
| href | `test/test_feature_href.py` | 25 → 46 | Added text, LaTeX, markdown renderer tests for \href, \url, \email |
| verbatim | `test/test_feature_verbatim.py` | 0 → 44 | New file; macros, environments, all 4 renderers, recomposer with fvextra |
| math | `test/test_feature_math.py` | 0 → 27 | New file; init, equation/align/gather envs, eqref, all 4 renderers, recomposer, error cases |
| enumeration | `test/test_feature_enumeration.py` | 0 → 21 | New file; itemize/enumerate/nested, custom tags/templates, all 4 renderers, recomposer, errors |
| headings | `test/test_feature_headings.py` | 20 → 64 | Added all 6 levels, 3-level numbering, duplicate IDs, long heading truncation, empty heading, special chars, text/LaTeX/markdown renderers, recomposer (section/starred/label/mapping), init tests. Added two-pass rendering fix (heading_infos cache) and comprehensive TextFragmentRenderer tests: single/sequential/5-section numbering, subsection resets, 3-level full reset, starred+numbered mix, depth boundary, depth=True, duplicate titles, refs to subsections, unnumbered sub/subsubsection. Also added starred+numbered LaTeX test, multi-level numbered Markdown test. Flagged `isinstance(heading_level, int)` in `markdown.py:258` as Transcrypt-incompatible (causes JS heading level fallback to `###`). |
| refs | `test/test_feature_refs.py` | 45 → 45 | Already high quality — skipped |
| endnotes | `test/test_feature_endnotes.py` | 0 → 20 | New file; EndnoteCategory/EndnoteInstance data classes, FeatureEndnotes init, footnote mark HTML, endnotes with category headings, no-endnotes case, custom category, text/LaTeX/markdown renderers, recomposer |
| cite | `test/test_feature_cite.py` | 6 → 61 | Added init/config (10), variants (19): bare/mixed/case-insensitive keys, space-stripped prefix, same-key dedup, two separate cites, triple chain, chain w/ extras (first/both), no-endnotes inline (single/multi/extra), sort_and_compress=False (2/3 keys, chain+extra), custom separator, custom delimiters, FLMFragment provider, multi-provider fallback. Errors (2). All 4 renderers (text/LaTeX/markdown: single/multi/extra/chain). Recomposer (10): single/multi/3-keys/extra/bare/mixed/chained/both-extras/chain+multikeys/safe_labels. |
| floats | `test/test_feature_floats.py` | 4 → 51 | Added _make_content_handler (6), available_content_handlers, _float_default_counter_formatter_spec (5), FloatType (4), FloatInstance (3), FeatureFloats init/config (6), FloatEnvironment (3), HTML figure (bare/caption-only/label-only/label+caption), HTML table, HTML refs (single/multi), any-content handler, text renderer (fig/table), LaTeX renderer (fig/table), markdown renderer (fig/table), recomposer (7: label+cap/bare/cap-only/label-only/table/keep_as_is/custom captioncmd), errors (wrong prefix/invalid content). Fixed class-level dict pollution in test_fragmentrenderer_latex.py. |
| defterm | `test/test_feature_defterm.py` | 3 → 24 | Full rewrite (fixed class name). Helpers (4), init/config (5), HTML (7: simple, ref-before-def, cross-ref, optional ref_term, label+ref, custom suffix, no-term), text (2), LaTeX (1), markdown (2), recomposer (3: basic, with label, term macros) |
| theorems | `test/test_feature_theorems.py` | 7 → 40 | Added init/config (13), HTML (10 new: definition, remark, conjecture, proof, proof+title, proof+*ref, proof+**ref, noproofref, shared counter across types), text/LaTeX/markdown renderers, recomposer (7: simple, title, label, proof, *ref, **ref, noproofref), error case. Added missing LatexWalkerLocatedError/LatexWalkerParseError imports to theorems.py. Flagged `dict(a, **b)` Transcrypt incompatibility for defaultset/richset construction (conjecture test fails in JS). |
| fragmentrenderer html | `test/test_fragmentrenderer_html.py` | 140 → 154 | Tightened ~30 loose `assertTrue(x in result)` to exact `assertEqual`. Added: render_math_content (6: target_id, env name, starred env, invalid displaytype, non-standard delims, display target_id), render_float (2: with/without caption), render_cells (1: simple table via FeatureCells), include_node_data_attrs_fn (3: with/without/returns-none), style info (2: css link styles, display-math styles). Graphics tests use full regex matching for Python/JS float format compatibility. |
| substmacros | `test/test_feature_substmacros.py` | 21 → 49 | Added init/config (4), get_what (3), text/LaTeX/markdown renderers (8: simple/args/specials/env), content dict textmode/mathmode (3), empty content (1), string argspec shorthand (1), error: invalid arg number (1), recomposer (7: macro+args/default/math/env/specials/IfNoValueTF both branches). Unicode curly quotes in get_what assertions. |
| quote | `test/test_feature_quote.py` | 50 → 67 | Added text renderer (5: text+attributed, blockquote, address, lines+indent, lines+attributed), LaTeX renderer (4: text+attributed, blockquote, address, lines+attributed), markdown renderer (4: text+attributed, blockquote, address, lines+attributed), recomposer extras (4: lines+indent, address+indent, custom attributed_macro, custom block_macro) |
| markup | `test/test_feature_markup.py` | 3 → 31 | Full rewrite; init/config (4), latex context defs (5), HTML (7: textbf/textit/multi-format/env/env+annotations/both/empty), text (4), LaTeX (4), markdown (4), recomposer (3: macro/env/multi-format). All pass Python + JS. |
| annotations | `test/test_feature_annotations.py` | 5 → 34 | Added init/config (10), HTML (12: brace/endmacro/comment/2nd annotator/inline context/no initials/block-level), hide_all (4: highlight+comment × HTML+LaTeX), LaTeX renderer (6: highlight/comment/color_index/no_initials/endmacro), RenderManager (2). Text/markdown renderers not supported (raise RuntimeError). No recomposer support. All pass Python + JS. |
| numbering | `test/test_feature_numbering.py` | 0 → 52 | New file. Counter (11: init/step/set_value/format/step_and_format/reset), CounterAlias (6: value/step/format/step_and_format/formatter), _CounterIface (5: register_item/custom_label/name/formatter), get_document_render_counter (4: basic/increments/always_number_within_raises/alias), FeatureNumbering init (3), RenderManager (10: register/errors/items/custom_label/formatted_value/alias/invalid_config), doc state (3: set/overwrite/clear_dependants), parent counters (5: none/one/chain/compute_keys), repr (2), integration (2: equations_within_sections/without_numbering). Fixed Counter.reset() bug (missing self.initial_value). All pass Python + JS. |
| cells | `test/test_feature_cells.py` | 0 → 55 | New file. Data models (14: CellIndexRangeModel repr/fields, CellPlacementModel repr/fields, CellPlacementsMappingModel empty/placements/open-ended/repr, _splfysidews). CellsModel parsing (10: index int/empty/default/invalid, range single/dash/plus/comma/non-contiguous/defaults). CellsModel ops (4: init/move_next_row/move_to_col/repr). Spec classes (9: env attrs/custom name, macros, feature init/defs). HTML (10: 2x2 cell, celldata 2x3, header+data, merge, styles, placement mapping, multiple merges, 3x1, complex header+styles+merge, multi-style celldata). LaTeX (1), Markdown (1). Errors (2: overlap, invalid content). Recomposer (3: keep_as_is, no render_context raises, with latex renderer). Fixed Transcrypt `''.split()` bug in `add_celldata_node` causing spurious `cellstyle-` class in JS. All pass Python + JS. |
| graphics | `test/test_feature_graphics.py` | 0 → 32 | New file. GraphicsResource (8: init minimal/full, srcset, fields, asdict minimal/full, repr minimal/full). SimpleIncludeGraphicsMacro (4: macroname/block_level/standalone/fields). FeatureSimplePathGraphicsResourceProvider (5: name/title/defs/render_manager/get_graphics_resource). HTML (4: basic figure, caption, srcset provider, physical dimensions). Text (1), LaTeX (1), Markdown (1). Recomposer (7: default, set_max_width=False, physical dims+render_context, width_scale, no max_width+phys dims, bare figure, default provider). Errors (1: options raises). Flagged `graphics.py:176` bug: `node_args['graphics_options'].nodelist.pos` — `SingleParsedArgumentInfo` has no `nodelist` attribute, causes AttributeError instead of LatexWalkerLocatedError. Uses regex for float format compatibility in physical dimensions tests (Python/JS). All pass Python + JS. |
| fragmentrenderer base | `test/test_fragmentrenderer_base.py` | 7 → 45 | Added init/config (4), ensure_render_context (2), semantic passthrough (2), render_join methods (5: join/empty/blocks/filter-none-empty/all-empty), nodelist errors (3: None/missing attr/block-in-inline), abstract methods raise (17: all unimplemented methods), comment rendering (1), math content (2: inline math + call verification), inline/block forcing (2). Used Exception instead of RuntimeError for Transcrypt compatibility. All pass Python + JS. |
| fragmentrenderer latex | `test/test_fragmentrenderer_latex.py` | 5 → 91 | Full rewrite. Init/config (4), latexescape (9: braces/amp/percent/hash/underscore-caret/tilde/plain/empty/unicode), wrap_in_text_format_macro (4: single/nested/none/empty), pin_label_here (5: default/no-phantom/without-flm-macro/disabled/no-phantom-config), _latex_join (6: plain/comment/named-macro/multiline-comment/multiline-no-comment/empty), render basic methods (7: value/value-escapes/error-placeholder/error-newlines/nothing-no-ann/nothing-with-ann/nothing-multi-ann), render_verbatim (4: inline/block-verbatimcode/escapes/wrap-macro), render_join_blocks (2), render_semantic_span (4: plain/endnote/citation/non-matching), render_semantic_block (4: known-role/unknown/annotations/target-id), links (5: hyperref/disabled/href-simple/href-hash/href-percent), delayed markers (3: marker/dummy/replace), graphics (7: collect-no-dims/with-dims/width-only/raster-mag/vector-mag/render-no-dims/render-with-dims), integration (22: text-format/bold-italic/inline-math/equation/align/paragraphs/heading-section/heading-label/heading-subsection/heading-invalid/href/url/delayed-section-ref/equation-ref/enumerate/enumerate-direct/itemize/verbatim-env/custom-text-format/defterm/footnote/float-caption/float-no-caption/link-endnote), FragmentRendererInformation (3). Uses regex for float format compatibility (Python/JS). All pass Python + JS. |
| fragmentrenderer text | `test/test_fragmentrenderer_text.py` | 55 → 65 | Added floats (4: caption+label/bare/caption-only/custom separators), theorem (1), cells smoke test (1), defterm with ref (1: internal link hides URL), _center_text_sqbrkt_line removed (Transcrypt skip pragma). Float tests use render_graphics_block() for centered image string to handle Python/JS centering differences. All pass Python + JS. |
| fragmentrenderer markdown | `test/test_fragmentrenderer_markdown.py` | 0 → 87 | New file. Init/config (3), heading_level_formatter (7), render_value escaping (15: all md specials), basic methods (9: error/nothing/verbatim/delayed/replace), _get_target_id_md_code (5: none/anchor/pandoc/github/disabled), join methods (7: join/blocks/empty/none/collapse), semantic_block (2), graphics (2), rx_mdspecials (4), FragmentRendererInformation (2). Integration: text_format (4: bold/italic/emph/nested), headings (7: section/sub/subsub/para/pandoc/github/none), paragraphs (2), links (3: href/url/email), math (3: inline/equation/eqref), verbatim (2), enumeration (3: itemize/enumerate/nested), floats (3: caption+label/bare/caption-only), footnote (1), defterm (1), theorem (1), cells table (1). All pass Python + JS. |
| recomposer purelatex | `test/test_flmrecomposer_purelatex.py` | 25 → 69 | Added init (3: shallow copy, multiple safe_ref_type domains, options dict copy). ensure_latex_package (1: multiple packages). make_safe_label (4: use_raw config dict, custom ref_to_global_key, resource_info tracked, unknown ref_type in safe domain). escape_chars (4: math mode no-escape, specials disabled no-escape, plain text, empty string). Recompose integration (22: bold/italic, nested formatting, inline/display math, eqref, gather, heading section/starred/optarg, enumerate, itemize, footnote, href, url, email, verbcode inline, verbatimcode block, defterm, theorem, paragraphs, returns dict, specinfo method). All pass Python + JS. |
| configmerger | `test/main/test_main_configmerger.py` | 12 → 36 | Added helpers (6: _get_preset_keyvals variants, ListProperty, get_default_presets), init (3: default/custom/additional_sources), edge cases (8: empty dict/list, None filter, scalar/list precedence, non-mapping ignored, single obj), PresetKeepMarker $_cwd (1), errors (6: $no-merge invalid, $remove-item none/missing, $merge-config none/missing, multiple presets), merge-config overwrite + defaults-then-append (2). All lightweight, no FLM rendering. |
| main | `test/main/test_main.py` | 3 → 38 | Added _process_arg_inline_configs (9), _TrivialContextManager (2), ResourceAccessor (4), load_external_configs (1), Main init (9: attrs/kwargs/errors/frontmatter/inline configs/doc metadata/flm_run_info), Main.run (2: make_run_object/skip_write), output formats (9: text/html/frontmatter/markdown/latex/suppress_newline/inline_config/inline_default/dollar_math), main_print_merged_config (2). Flagged `load_external_configs` line 100 f-string bug (crashes when arg_config is dict). |
| importclass | `test/main/test_main_importclass.py` | 0 → 12 | New file. Dotted fullname (5: basic/with default_classnames/first wins/wrong class/nonexistent module), no-dot (5: no defaults raises/empty raises/with prefix/without prefix/prefix fallback), return values (2: module+class/usable instance). Lightweight, no FLM rendering. |
| template | `test/main/test_main_template.py` | 0 → 35 | New file. _ProxyDictVarConfig (12: basic/nested/missing/none/deep missing raises/none_as_empty_string=False/if true/false/missing/else/endif), _StrTemplate (4: basic/dotted/colon/dash), replace_ifmarks (8: iftrue/iffalse ×with/without else, nested, no marks, empty, unmatched raises), TemplateEngineBase (2), OnlyContentTemplate (1), SimpleStringTemplate (8: basic/if true/false/missing, filename default/custom ext/custom file, combined). Flagged `get_config_value` AttributeError on deep traversal past non-dict value. |
