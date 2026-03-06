# FLM — Claude Notes

## Project

**flm-core** is a Python library implementing *Flexible Latex-like Markup* (FLM): an approximate subset of LaTeX syntax parsed via `pylatexenc` (v3 pre-release) and rendered to multiple output formats (HTML, LaTeX, plain text, Markdown).

The codebase is also transpiled to JavaScript via **Transcrypt** (`flm-js/`). This constrains what Python idioms are usable.

Packages are managed via `poetry`.  Prefix test and python-specific calls appropriately, e.g. `poetry run python -m pytest ...`

## Structure

- `flm/` — main Python package
  - `flmenvironment.py` — `make_standard_environment()`, `FLMArgumentSpec`
  - `stdfeatures.py` — `standard_features()` helper
  - `flmspecinfo.py` — `FLMMacroSpecBase`, `FLMEnvironmentSpecBase`, `text_arg`
  - `flmfragment.py`, `flmdocument.py` — fragment/document lifecycle
  - `feature/` — pluggable feature modules (`href.py`, `quote.py`, `math.py`, …)
  - `fragmentrenderer/` — output backends (`html.py`, `latex.py`, `text.py`, …)
  - `flmrecomposer/` — pure-LaTeX recomposer (`purelatex.py`)
- `test/` — unittest files, one per module/feature

## Key Patterns

**Features** are plugins added to `standard_features()`. Each implements `add_latex_context_definitions()` returning `{'macros': [...], 'environments': [...]}`.

**Rendering**: environments inheriting `FLMEnvironmentSpecBase` have `allowed_in_standalone_mode = False`, so tests must use the document rendering pattern:
```python
frag = environ.make_fragment(src)
doc = environ.make_document(frag.render)
result, _ = doc.render(HtmlFragmentRenderer())
```

**Nodelists** must be created via `node.latex_walker.make_nodelist(nodes, parsing_state=...)`, not plain Python lists, so that certain meta fields (parsing state, position, `flm_is_block_level`) are set correctly.

**Recomposer options**: `FLMPureLatexRecomposer(options)` takes a flat dict; per-feature options are retrieved inside `recompose_pure_latex` via `recomposer.get_options('feature_key')`.

## Transcrypt Compatibility

- Do not refactor existing code needlessly.
- In unit tests, stick to `self.assertTrue`, `self.assertFalse`, `self.assertEqual`, `self.assertIs`, `self.assertIsNot`, `self.assertIsNone`, `self.assertIsNotNone`, and `self.assertRaises`. Do not use other assertion methods such as `assertIn` / `assertNotIn` / `assertIsInstance`; use `self.assertTrue(x in items)` if necessary.  
- Avoid negative array/string index lookups or slices.
- Dictionaries received as arguments in the public-facing API might be assigned to pure JS objects instead of Transcrypt's JS `dict` object.  So ensure any such objects are explicitly cast to `dict(x)` before accessing dict-specific methods on them (e.g. `for k,v in x.items(): ...` -> `x = dict(x); for k,v in x.items(): ...`).


## Testing

Run with `pytest` (configured in `pyproject.toml`). Test files live in `test/` and are named `test_<module>.py`.
