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
  - `main/` — CLI entry point, config merging, workflows, JSON schema generation (`run.py`, `main.py`, `configmerger.py`, `_flm_args_schema.py`)
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
- Only use `import` statements at the top of a module, not inside a function or method.
- Do not use the `dict(a, **b)` construct to merge dicts.

## Testing

Run with `pytest` (configured in `pyproject.toml`). Test files live in `test/` and are named `test_<module>.py` or `test_<module>_<submodule>.py`. Test files for the `flm.main` module and its submodules live in the `test/main/` folder.

## Architecture & Terminology

### Rendering pipeline

The full rendering pipeline is: **Environment → Fragment → Document → RenderContext → FragmentRenderer → output string**.

1. **`FLMEnvironment`** (`flmenvironment.py`) — the central object. Holds all registered features, the parsing state, and the latex context database. Created via `make_standard_environment(features=standard_features())`. Provides `make_fragment()` and `make_document()`.

2. **`FLMFragment`** (`flmfragment.py`) — a piece of FLM source text parsed into a pylatexenc node tree. Created via `environ.make_fragment(text)`. Two modes:
   - **Standalone mode** (`standalone_mode=True`): can be rendered independently via `fragment.render_standalone(renderer)`. Only features with `allowed_in_standalone_mode=True` are available.
   - **Document mode** (default): rendered within a document via `fragment.render(render_context)` inside a render callback.

3. **`FLMDocument`** (`flmdocument.py`) — wraps a render callback that composes output from multiple fragments. Created via `environ.make_document(render_fn)`. Calling `doc.render(fragment_renderer)` returns `(result, render_context)`.

4. **`FLMRenderContext`** (`flmrendercontext.py`) — carries state during rendering. `FLMDocumentRenderContext` (in `flmdocument.py`) is the document-mode subclass with full feature manager support. `FLMStandaloneModeRenderContext` is the minimal standalone variant. The render context manages:
   - Feature render managers (`supports_feature()`, `feature_render_manager()`)
   - Delayed rendering for forward references (`register_delayed_render()`, `get_delayed_render_content()`)
   - Logical state for context-dependent rendering (e.g., nested enumeration depth via `push_logical_state()`)

5. **`FragmentRenderer`** (`fragmentrenderer/_base.py`) — abstract base class producing output in a specific format. Subclasses implement `render_value()`, `render_text_format()`, `render_heading()`, `render_enumeration()`, `render_verbatim()`, `render_link()`, `render_float()`, `render_cells()`, etc. Built-in renderers: `HtmlFragmentRenderer`, `TextFragmentRenderer`, `LatexFragmentRenderer`, `MarkdownFragmentRenderer`.

### Spec info classes

**`FLMSpecInfo`** (`flmspecinfo.py`, inherits pylatexenc's `CallableSpec`) — defines how a parsed construct (macro/environment/specials) is finalized and rendered. Key attributes:
- `delayed_render` — if `True`, rendering is deferred to a second pass (used by `\ref`, `\eqref`, etc.)
- `is_block_level` — `True` for block elements (headings, lists, figures), `False` for inline, `None` for auto-detect
- `allowed_in_standalone_mode` — whether usable without a document context
- `body_contents_is_block_level` — for environments, whether body is parsed as block-level

Convenience subclasses: `FLMMacroSpecBase`, `FLMEnvironmentSpecBase`, `FLMSpecialsSpecBase`.

### Feature system

**`Feature`** (`feature/_base.py`) — base class for all FLM extensions. Each feature:
- Sets `feature_name` (unique string identifier)
- Overrides `add_latex_context_definitions()` → returns `{'macros': [...], 'environments': [...], 'specials': [...]}`
- Optionally provides inner classes `DocumentManager` (per-document state) and `RenderManager` (per-render state)
- Can declare `feature_dependencies` and `feature_optional_dependencies` (features are topologically sorted)

**`SimpleLatexDefinitionsFeature`** — lightweight subclass with no managers; just sets `latex_definitions` dict.

The **render manager lifecycle** during `FLMDocument.render()`:
1. `RenderManager.__init__()` + `initialize()` — setup
2. First-pass render (render callback runs, nodes call `prepare_delayed_render()` or `render()`)
3. `RenderManager.process(first_pass_value)` — e.g., assign numbers
4. Delayed nodes are rendered
5. Final output produced (marker replacement or second pass)
6. `RenderManager.postprocess(final_value)` — cleanup

### Block-level decomposition

The `NodesFinalizer` class (`flmenvironment.py`) and `BlocksBuilder` handle decomposing node lists into blocks (paragraphs). After finalization, a node list has:
- `flm_is_block_level` — whether the list contains block-level content
- `flm_blocks` — list of blocks (each a `LatexNodeList` paragraph or a standalone block-level node)

Nodes have `flm_is_block_level`, `flm_is_block_heading` (run-in headings like `\paragraph`), and `flm_is_paragraph_break_marker` (for `\n\n`).

### Delayed rendering

Used for forward references (`\ref`, `\eqref`, `\cite`). The mechanism:
1. On first pass, `prepare_delayed_render()` is called (registers the node with feature managers)
2. A placeholder marker or dummy is inserted
3. After the first pass, delayed nodes are rendered with full document context
4. Either markers are replaced in the output string (`supports_delayed_render_markers=True`, used by HTML renderer) or a full second pass is performed

### Counter formatting

`flm/counter.py` provides `alphacounter`, `romancounter`, `unicodesuperscriptcounter`, `fnsymbolcounter`, and `build_counter_formatter()` which accepts format strings like `'alph'`, `'Roman'`, `'arabic'`, or pattern strings like `'(a)'`.

## Documentation

Sphinx docs live in `doc/`. Build with `poetry run sphinx-build -b html doc doc/_build/html`. API docs use `autodoc` directives pulling from source docstrings. User guide pages (overview, standard-syntax, configuration, features, model, lib) are hand-written RST. The build generates `flm-config-json-schema.json` in the HTML output root via a `build-finished` hook in `conf.py`. CLI flags `--validate-config-only` and `--print-config-json-schema` expose schema validation and generation.
