# Plan: handle `data:` (and remote) URLs in graphics collection

In broad strokes: Cache downloaded URL bytes in temp files, not in memory.
Inspect image dimensions for all URLs, including `data:` as well as `https:` and
others.  Each URL is downloaded once, written to a temp file, and that path is
reused everywhere downstream — so URL graphics flow through the existing
`source_type == 'file'` code paths (inspect, hash, convert, copy) with almost no
special-casing.

## Caching scope: the **Feature** instance (not the DocumentManager)

The download cache (bytes-on-disk + inspected info) lives on the
`FeatureGraphicsCollection` **instance**, not on the per-document
`DocumentManager`. The `DocumentManager` already references its owning feature
(`self.feature`, set in `FeatureDocumentManagerBase.__init__`), and one Feature
instance spawns every DocumentManager, so the cache is reachable from the scan
code via `self.feature` with no plumbing.

### Lifetime analysis (read before relying on cross-render reuse)

- A single `main.run()` builds one environment, one set of Feature instances,
  and renders exactly **one** document (`run.py:1122/1164`; document-part
  fragments share that same document). For an ordinary CLI invocation, Feature
  scope and DocumentManager scope are therefore *identical in effect* — there is
  only ever one DocumentManager per Feature.
- **Watch mode** rebuilds the environment and all Feature instances on every
  recompile (`watch.py:158` → `main.main()` → `run.py:372`; the `watch.py:145`
  comment deliberately avoids object reuse). A Feature-level cache does **not**
  survive across recompiles — each save re-downloads.
- Feature scope therefore only beats DocumentManager scope when one environment
  is deliberately reused across several `make_document`/render passes (custom
  batch driver, multi-format render off one env). It is a strict superset of
  per-document dedup and is otherwise harmless.
- **Follow-up (out of scope here):** to cache downloads across watch-mode
  recompiles, persistence must move to an even broader scope (a module-level
  dict keyed by URL, or persisting the environment across recompiles). Not done
  in this plan.

Because the Feature can outlive any single DocumentManager, the temp directory
and the temp-file paths it hands out must remain valid for the whole Feature
lifetime — which they do (see "temp-file lifetime" below).

## Plan

**0. Feature-level cache state (in `FeatureGraphicsCollection.__init__`).**
Add, alongside the existing config attributes:
- `self._url_cache = {}` — keyed by the URL string (`source_url`); value is a
  dict `{ 'temp_file_path', 'mimetype', 'detected_ext', 'info', 'input_hash' }`
  (or a failure marker, see step 3).
- `self._url_tempdir = None` — created lazily on first URL download via
  `tempfile.TemporaryDirectory()` and stored on the Feature, so the temp files'
  lifetime is tied to the Feature lifetime.
- `self._url_download_counter = 0` — monotonic, Feature-scoped, used to name temp
  files uniquely within the shared temp dir (per-document counters could collide
  in a shared dir).

These are plain attributes set in `__init__`; Transcrypt-safe. Mixing runtime
cache state onto the (otherwise config-only) Feature object is intentional and
required by the chosen scope.

**1. New Feature method `fetch_inspect_url(source_url)` — the memoizing core.**
Lives on the Feature (it owns the cache + temp dir + already owns
`inspect_graphics_file`). Behavior:
- If `source_url in self._url_cache`, return the cached entry immediately (this is
  the dedup / single-download guarantee, now Feature-wide).
- Otherwise download + inspect once:
  - `with urllib.request.urlopen(source_url) as r: content = r.read(); mimetype = r.headers.get_content_type()` — the default opener already has `DataHandler`, so `data:` is handled offline (the same call `collect_graphics` already uses).
  - `detected_ext` from a small override dict (`image/png`→`.png`, `image/jpeg`→`.jpg`, `image/svg+xml`→`.svg`, `application/pdf`→`.pdf`, `image/gif`→`.gif`), falling back to `mimetypes.guess_extension(mimetype)` (default `''`).
  - Lazily create `self._url_tempdir` if `None`; bump `self._url_download_counter`; write `content` to a temp file named `inline{counter}{detected_ext}` inside the temp dir, so the synthetic name carries the right extension for the inspector and converters.
  - Inspect by opening that temp file: `with open(tmp_path, 'rb') as fp: info = self.inspect_graphics_file(tmp_path, fp)`.
  - Compute `input_hash` once here from `content` (`hashlib.sha256(content).hexdigest()`), so it's cached with the entry and reused for `${hash}`/cache-skip.
  - Store and return `{ 'temp_file_path': tmp_path, 'mimetype', 'detected_ext', 'info': info or {}, 'input_hash' }`.
- Wrap fetch+inspect in try/except: on failure, **warn and cache a failure marker**
  `{ 'temp_file_path': None, 'mimetype': None, 'detected_ext': '', 'info': {}, 'input_hash': None }` (preserves today's behavior for unreachable/odd URLs — no hard regression, and we don't retry a known-bad URL on every document).

**2. `get_source_info` — document the multi-call contract, keep it cheap.**
Add a comment that it runs once per resource at scan *and* once per
`\includegraphics` at render (`:1207`), only to classify the source and compute a
stable `source_key`. No network / decode / disk I/O here. Return tuple and
`source_resolved` unchanged; `data:` URLs keep classifying as `'url'`.

**3. Scan branch uses the Feature cache (the `# TODO`, `:839`).**
Rewrite the `elif source_type == 'url':` branch in
`inspect_add_graphics_resource` to delegate:
- `entry = self.feature.fetch_inspect_url(source_url)` (downloads at most once
  per URL per Feature lifetime; later documents/renders reuse it).
- `graphics_resource = GraphicsResource(src_url=source_url, **entry['info'])`.
- `self.add_graphics(source_key, source_info, graphics_resource, url_entry=entry)`.

**4. Cache resolved paths in the per-document collection entry.**
Have `add_graphics` (`:858`) accept an optional `url_entry` and store, in
`graphics_collection[source_key]`, the extra fields `temp_file_path` (str or
`None`), `mimetype`, `detected_ext`, and `input_hash`, copied from `url_entry`.
For `file` sources (`url_entry=None`), set
`detected_ext = splitext(full_file_path)[1]`, `temp_file_path=None`,
`mimetype=None`, `input_hash=None`, so the fields are uniformly present. This
per-document dict is internal, never serialized — `GraphicsResource` stays clean.
The `temp_file_path` it stores points into the Feature's temp dir, which outlives
the DocumentManager, so it is valid throughout scan→render→collect.

**5. `prepare_collect_graphics` — use `detected_ext` and the cached hash (the `# TODO (2)`).**
For `source_type == 'url'`, take `ext` from the entry's `detected_ext` (not
`splitext` of the data URL, which yields garbage), and take `input_hash` from the
entry (already computed by the Feature in step 1 — no second read). For `file`
sources keep the existing `hashlib.file_digest(f, 'sha256')` path. So URL graphics
get cache-skip and `${hash}`/`${hash6}` template keys exactly like files.

**6. `collect_graphics` — copy/convert from the temp file.**
In the url branch (`:1170`), if a `temp_file_path` is available (looked up via
`source_key`, or threaded into `collect_info` in step 5), treat it like a local
file: `shutil.copyfile(temp_file_path, target_path)` for the no-converter path,
and pass the temp path (with `source_type='file'`) to `converter.convert(...)` so
converters read it off disk with no second download. Fall back to the existing
`urllib.request.urlopen` copy only when there is no temp file (earlier fetch
failed). Converters stay completely untouched.

**7. Guard the non-collecting `relpath` (`:1228`).**
`os.path.relpath` only when `source_type == 'file'`; otherwise return `src_url`
unchanged so inline `data:` / remote URLs survive verbatim in non-collecting
output.

**8. Imports:** add `tempfile` and `mimetypes` at module top (alongside existing
`urllib.request`). (`io` not needed — inspection opens the temp file directly.)

**9. Tests.** Small base64 PNG `data:` URL through both a collecting render
(assert the collected file ends `.png`, exists on disk, and the
`GraphicsResource` has `physical_dimensions`) and a non-collecting render (assert
the data URL passes through intact). Add one test that reuses a single
`FeatureGraphicsCollection` instance across two documents and asserts the URL is
fetched only once (e.g. by checking `len(feature._url_cache) == 1`, or by
monkeypatching/counting `urlopen`) — this is what the Feature-scoped cache buys.
Use a temp output dir; allowed assertion methods only
(`assertTrue`/`assertEqual`/etc.).

## Notes on temp-file lifetime

- The `TemporaryDirectory` is created lazily (only if at least one URL is
  downloaded) and held on the **Feature**, so it persists across every document
  and render/collect pass that Feature serves, and is removed automatically when
  the Feature is garbage-collected. No unbounded in-memory bytes are retained
  (only the bytes-on-disk cache, bounded by the number of distinct URLs).
- Deduplication is per URL across the whole Feature lifetime: a repeated identical
  URL — within one document *or across documents that share the Feature* — is
  downloaded to a temp file once.
- Caveat (restated): in standard single-document runs and in watch mode the
  Feature is not reused across renders, so the cache does not persist across them.
  See the lifetime analysis above.

Shall I implement this?
