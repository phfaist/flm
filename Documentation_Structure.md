# Project documentation organization

## Documentation pillars - public-facing and internal-developer-facing

The documentation for this project is sorted into two pillars:

- The *public-facing* pillar. (Also known as the *user-facing* pillar.) The
  target audience is an outside user who would like to use FLM or FLM's
  libraries. The user will want (i) precise and useful information about the
  library's public API, what it does, how it can do what the user desires, how
  to extend it, etc.; and (ii) how to use FLM: writing FLM documents, using the
  command-line `flm` binary, configuring the FLM registry, etc.

- *internal-developer-facing* pillar: (Previously referred to as
  *developer-facing*.) This is documentation that describes the internal
  structure and functioning of the FLM libraries. It includes the design
  decisions and their rationale. This information is precious for agents and
  humans who want to contribute to the FLM libraries themselves.

The documentation is divided into four parts: *guides* (public-facing), *rustdoc
API documentation* (public-facing), *ARCHITECTURE* (internal-facing), and
*DESIGN_RATIONALE* (internal-facing).

### Guides (public-facing)

Guides are public-facing documentation.  See important information on
cross-referencing below, especially regarding attempts to cross-reference
information in dev-docs.  Documentation clarity rules (see below) apply to
guides in full.

Guides explain the overall architecture and philosophy of the
library and how to use it.  They collect global information about the library
that is poorly attached to a specific code location like rustdoc is.

Special guides ("AI guides") are optimized for loading in an AI agent's
context.  They are `docs/ai-guide.md` (entry point) and `docs/ai-guide-*.md`
(more detailed information for different parts of the library).

### API documentation directly in code (public-facing)

Code rustdoc API documentation is public-facing documentation.  See important
information on cross-referencing below, especially regarding attempts to
cross-reference information in dev-docs.

This pillar covers all workspace crates built into the documentation bundle:
`flm-core`, `flm-main`, `flm-cli`, as well as any extension crates (currently
`flm-phfpack`).

The rustdoc API documentation documents call contracts, prerequisites, best use
practices, and pointers to how specific functions/structs can solve a particular
problem when relevant and appropriate. See below for more specific instructions
on writing documentation and for documentation clarity rules.

The `docs/panics.md` guide contains the exhaustive public-facing list of the
public API's panicking items, each named specifically with an intra-doc link.
Any change to the public API's panicking behavior (adding, removing, or
changing a documented panic) must update that guide — it is kept exhaustive
by maintenance discipline.

### ARCHITECTURE documents (internal-developer-facing)

The ARCHITECTURE documents consist of `dev-docs/ARCHITECTURE.md`,
`dev-docs/ARCHITECTURE-backends.md`, `dev-docs/ARCHITECTURE-stdcore.md`,
`dev-docs/ARCHITECTURE-main.md`, and
`dev-docs/ARCHITECTURE-extra.md`. ARCHITECTURE documents are
internal-developer-facing.

The ARCHITECTURE files explain the present-day structure of the library;
high-level design principles; specific outcomes of design decisions.  These
documents do NOT contain history of how decisions were reached or rejected
alternatives. No dates. No "Assessment of where things stand" other than a
marker "Designed/Planned; not yet implemented". Code snippets are to be avoided;
use only when absolutely necessary and when not directly visible by looking up
the relevant source files.

ARCHITECTURE documents do NOT contain explanations of implementation phases.
These are a historical detail of how the library was implemented and must be
dropped without trace. This information will remain accessible in the git commit
history even if `dev-docs/*` plans and reports are removed.

The ARCHITECTURE documents are organized into logical sections that cover the
different aspects of the library.  They consist of the following documents:

- `dev-docs/ARCHITECTURE.md`: This document describes the architecture
  of the core library structure that FLM runs on. This includes the registry,
  the pipeline, how elements and tables are structured and run; how plans are
  built, etc.  This document does NOT include any design decisions related to
  FLM's standard library packages (stdcore features), nor any design decisions
  that are related to the command-line interface and/or the standard host
  (`flm-main`).
  
- `dev-docs/ARCHITECTURE-backends.md`: This document describes the architecture
  of the standard backends provided by the core library. This includes the
  latex-recomposed, html, and text backends (as well as backends we might add in
  `flm-core` in the future, such as `latex-rendered` or `markdown`). This also
  includes all items in the core library that primarily serve the backends, such
  as the structure of presentation elements or helper modules for backends
  (e.g. the `latex` module).
  
- `dev-docs/ARCHITECTURE-stdcore.md`: This document describes the architecture
  of the standard features and packages provided by the core library. This
  includes headings, href, textstyles, userdefs, etc.  This also includes helper
  modules that primarily serve stdcore features and packages, such as
  numbering. Exception: the standard backends (latex-recomposed, html, etc.) are
  not included here, they belong in `ARCHITECTURE-backends.md`.
  
- `dev-docs/ARCHITECTURE-main.md`: Here, we describe the architecture of the
  part of the project related to the host (`flm-main`) and the command-line tool
  (`flm-cli`).

- `dev-docs/ARCHITECTURE-extra.md`: Here, we describe the architecture of any
  remaining crates that are not part of the main `flm-core`, `flm-main`,
  `flm-cli` crates.  This includes the `flm-phfpack` crate.  (In the future, it
  may include other "extra" crates.)

Design decision rationales are documented instead in DESIGN_RATIONALE document
(see below).  See important note on cross-referencing below; in short:
*ARCHITECTURE documents may never reference DESIGN_RATIONALE entries*;
ARCHITECTURE may furthermore never reference temporary/build plans in
`dev-docs/*/`.

ARCHITECTURE documents are structured as follows.  Each section has a markdown
header (leading #, ##, ###, ####) with a relevant title and a label
`§flm-dd-arch:<name>` (e.g. "§flm-dd-arch:arguments-slots") for easy
cross-reference with `dd` standing for "developer-docs".  Labels enable easy
reorganization of the documentation without invalidating section numbers.
*The label must be unique across all ARCHITECTURE files.*

The `dev-docs/ARCHITECTURE.md` document should be kept at around max. ~100KB
size.  The other `dev-docs/ARCHITECTURE-*.md` files should avoid exceeding
max. ~200KB.  These figures are **soft** limits; never summarize important
information for the sole purpose of remaining under the stated threshold. At the
same time, remove entirely and without trace any explanations and descriptions
that have no significant impact on the design of the library.

The `dev-docs/ARCHITECTURE.md` document is structured as follows:

```md
# How to use and maintain this document [§flm-dd-arch:self-meta]

...

# Library design principles [§flm-dd-arch:lib-design-principles]

...

# Split between ARCHITECTURE files [§flm-dd-arch:arch-split]

... (short bullet-point list of guidelines that specify in
which ARCHITECTURE* file to include specific content, whenever
the guidelines in Documentation_Structure.md are not
sufficiently specific.)

# Overview of the library architecture [§flm-dd-arch:arch]

...

(organize into subheadings as appropriate)

```

The other `dev-docs/ARCHITECTURE-*.md` documents are structured as follows
(replace '*' by 'stdcore', 'backends', 'main', 'extra'):

```md
# How to use and maintain this document [§flm-dd-arch:self-meta-*]

...

# Design principles [§flm-dd-arch:lib-design-principles-*]

...

# Architecture overview [§flm-dd-arch:arch-*]

...

(organize into subheadings as appropriate)

```


### DESIGN_RATIONALE.md (internal-developer-facing)

The file `dev-docs/DESIGN_RATIONALE.md` is internal-developer-facing.

The DESIGN_RATIONALE tracks the reasons behind
individual design decisions, along with rejected alternatives, for future
reference and to ensure overall consistency of design decisions with our
fundamental design principles.

Where ARCHITECTURE documents explain *how* the library is structured and how it
behaves, the DESIGN_RATIONALE explains *why* the library is structured in that
way.  ARCHITECTURE records the design principles for public-facing APIs, which
are interpreted as "how the user should expect this library to behave" (*how*
the library behaves).  DESIGN_RATIONALE records the design principles we follow
to implement this library - the *why* this library is implemented in this way
and *why* this particular design was singled out.
  
History of design decisions may be useful if a decision was made and was later
reversed, since this information can serve to highlight a conscious choice was
made with options carefully considered.  But otherwise history of decisions
should be omitted.

A `Status:` line records the status and the who/context that led to the decision
(e.g. `Status: DECIDED (user-led).`). Status lines never mention a date. Dates
may only appear inside explicitly recorded reversal notes, where preserving the
history of a reversed decision is the point.

Each section has a markdown header (leading #, ##, ###, ####) with
a relevant title and a label `§flm-dd-dr:<name>`
for easy cross-reference with `dd` standing for "developer-docs" and `dr` standing
for the "design rationale" document.  Labels enable easy reorganization of the
documentation without invalidating section numbers, and the `flm-dd-dr:` enables
cross-referencing across documents.

Every labeled header carries, on the line immediately below it, an
`Architecture:` line naming the ARCHITECTURE sections the header relates to.
That line is the one mandatory cross-reference between the two developer
documents; its exact shape, and the command that reads it backwards to find the
decisions behind a given ARCHITECTURE section, are specified under 'Within
developer docs' below.  This line does not specify which of the specific
ARCHITECTURE documents the label is to be found in; it doesn't need to, since
the label is unique and is easily found with grep.
  
This document is carefully organized as follows:

```md
# How to use and maintain this document [§flm-dd-dr:self-meta]

...

# Implementation design principles [§flm-dd-dr:impl-design-principles]

...

# Decision register [§flm-dd-dr:decisions]

...

(organize into subheadings as appropriate)

```

## Documentation guidelines

These documentation guidelines apply to *all public-facing documentation*, with
the exception of the *AI guides*.

Internal developer-facing documentation should be succinct, clear, and provide
sufficient context for clarity, but the documentation clarity rules need not be
strictly enforced.


### Documentation clarity rules

Explanations must be succinct and clear. They must provide sufficient context to
understand the individual parts of the explanation without intimate knowledge of
all of the library's features and internals. Explain concepts and ideas one by one.
Disentangle explanations: explain ideas and concepts clearly, one at a time.
Zoom out and make sure that the documentation globally says important things first,
with sufficient context, and does not unnecessarily focus on small details.

- State the point you want to make directly. No convoluted sentences.

- No mannered prose. No invented jargon. No metaphors.

- Begin a documentation block with a clear explanation of what the object being documented does, what is purpose is, what its intended use is, how it behaves, and what information or context it requires (if applicable).  Do not begin an explanation with a disclaimer or a contrast with something else.  Example: techy's documentation should not begin with "techy is not a TeX engine" or "techy does not know of any specific commands like `\'` and `\#`" but should instead first describe what techy *is* and *does*, and only later clarify that it is not a TeX engine or that it abstracts out the command syntax.

- Disentangle explanations into individual steps which you explain individually and succinctly. Example: BAD:
```
What register can declare. The `builder`'s main verbs are these. `element` declares something an author can write, with its name, its argument syntax and the modes it is visible in. `lowering_rule` declares how an element becomes output on a backend.
```
(--> What register? which builder? what's a verb? something an author can write where?) Much better would  be:
```
FLM's *registry* contains all definitions that can appear in a document parsed by FLM. The register contains *elements*, which represent constructs (such as macros and environments) that an author can write in an FLM document. Each element has a *name*, an *argument syntax* (e.g., the macro or environment arguments), and the mode (e.g., text mode or math mode) the element is available in. A *backend* transforms elements into output of a specific format (e.g., HTML) by applying a set of *lowering rules*. Each lowering rule specifies how an element translates into the backend's output format.

A *FLM feature* can declare elements, backends, [tables](...), and lowering rules to be included in FLM's registry. These elements are declared via a *builder interface*: A builder is an object instance handed over to the feature's `register()` method and which exposes relevant methods for the feature to contribute elements, backends, tables, lowering rules, etc. to the registry.
```

- Every item or object referred to in a sentence must either be clearly obvious to the intended reader (e.g.: for developer-facing documentation, there's no need to define what a rust trait is), or be defined or have its definition clearly referenced in the immediate preceding context. The immediate preceding context must be very easy to find. The sentences in the same paragraph count as immediate context if they precede the sentence in question, or if the relevant information is specified immediately thereafter with a colon. (As an exception, certain definitions, perhaps that of a specific table name for instance, may be concentrated at the top of the same page if they are frequently referred to on the same page, provided they are made easy to find and a reference is included e.g. as "(see top of page)".)  A golden rule: Never write "the X" without it being unambiguously clear about what X you're talking about even to a reader who has no knowledge of the rest of the API, who only reads that sentence and briefly skims the preceding sentences.  Example: "The feature declares the table in the registry" is most likely bad, unless the preceding sentences clearly define which feature, table, and registry are being talked about; instead, prefer `Feature [MyFeature](...) declares the ['flm.my.table' table](...) into [FLM's registry](...)`, with links to the relevant parts of the documentation that define these objects.

- Prefer unambiguous terms instead of words that could mean several things out of context ("feature X names dependency Y" is bad; prefer "feature X declares Y as a dependency" since "names" could mean any of "declares/specifies", "simply lists", or "finds a suitable name for"). Avoid especially "to name", "to live", "the verb", "visible". Then, be careful with words are only unambiguous with an appropriate complement; e.g. "[the backend] writes [the element]" -> writes where? e.g. "[the backend] writes [the element] to the output file".

- Be succinct; in the sense of avoiding repeating yourself. But do NOT shorten any explanations or documentation that serve the clarity rules outlined here.

- Prefer an active voice and a full sentence, rather than a convoluted sentence making implicit reference to its surroundings. Bad: "The macros a document declares." Better: "This struct collects all macros declared in an FLM document."

- Headings and sections: Reflect the *substance/content*, not the "manner" or "role" the content play. Example: do not use "One caveat" as a heading, prefer "Limitation with unicode input strings".

- Do NOT use acronyms other than extremely widely-used and widely-understood ones.  (Prefer "WebAssembly" to "WASM"; use "Design Rationale" instead of "DR" (internal-facing docs only); exceptions only okay if reader is highly likely to know the acronym AND is easily guessed from context, e.g. "JPEG image", "PNG file".)

When writing documentation: ALWAYS VERIFY ANY DOCUMENTED STATEMENTS FOR CORRECTNESS. NEVER ASSUME BEHAVIOR. Apply strictly all the above rules.

After writing documentation: Always test the written documentation for quality and enforce thorougly the above rules.  Never summarize an explanation if it risks reducing its clarity according to the above rules.  Check for completeness, by asking yourself the following questions.  Is the context of the documented item clear? Will a user find the important features they are looking for (imagine different scenarios of users that might need to use this item)? Are there pointers to related methods and functions the user would want to be aware of?

### Audience: Both Humans and AI

Documentation should be designed and optimized both for HUMAN and for AI consumption.
Cross-references should be easy for an AI to follow (cf scheme above).  Explanations
should be concise, logical, and self-contained so that humans can clearly connect
the explanation with their current mental model and expectations.

The AI guides obey special rules: They are optimized for use by AI. The AI guide documentation writer decides the best content structure and level of detail for these guides.

### Document cross-referencing

#### Within user docs

Public-facing documents are generated with rustdoc using `cargo docs` (an alias for
`cargo doc --workspace --no-deps`).  Use internal standard rustdoc cross-references,
use inline doctests for examples, and follow rustdoc best practices for documentation.
The guides and API documentation are compiled together and may freely
cross-reference each other using rustdoc references that are checked by the compiler.
See technical information on cross-referencing within rustdoc in the
'technical considerations' section below.

Guides and API documentation may link each other freely in both directions;
they form one documentation bundle. Where a single API item canonically embodies
a concept, link the item; use the `concepts-overview.md` headings for
cross-cutting concepts that no single item represents.

#### Never reference internal-developer-docs from public-facing docs

Public-facing documentation MUST NOT reference developer docs.  In public-facing
docs, internal developer docs may NOT be referred to to document library
behavior or API structure.  If a user is likely to be confused about why an API
call is of a particular form, or about design choices in the API, a
self-contained explanation is to be provided in the public-facing documentation
itself.

Here's how to handle a misplaced reference from public-facing pages (e.g. code API) to
dev-docs.  This applies to existing such references, or if an agent or human is tempted
to include such a reference.  Do this instead:

- One guide is called `concepts-overview.md`.  It lists all the main concepts of the
  library.  A concept is referenced with a standard rustdoc link to the page's heading,
  e.g. `[myconcept](crate::guide::concepts_overview#myconcept)`; heading slugs are
  immutable once published (see 'Technical considerations' below).  Where a single API
  item canonically embodies the concept, prefer linking that item directly.
- For each reference in the user API docs to some dev-doc section, or in case of
  being tempted to do so: (A) Is this information about a core, public-facing
  feature of the library?  If so, do not reference ARCHITECTURE or
  DESIGN_RATIONALE, but reference instead the relevant concept in
  `concepts-overview.md` using a standard rustdoc link.  (B) Does this
  information explain the reason for a detail or quirk of the public API
  (e.g. why a function call has a particular shape or struct has particular form
  etc.)?  If so, explain the rationale behind this design decision.  This
  information may be repeated multiple times at various locations (say <~ 5
  times); if the same rationale is to be repeated more than that many times at
  different locations, identify the key location in the user API where this
  information is most relevant, and include a rustdoc cross-reference to that
  location at all the other locations inviting the user to consult that location
  for details about this design decision.  (C) Is this information purely about
  an internal implementation detail, with a high likelihood that this
  information is useless to anyone other than library maintainers?  Then include
  the information as a regular comment (not rustdoc) at the relevant locations,
  and in this case, include an explanation in the regular code comment and
  include a reference to the dev-docs as (for instance) `See decision rationale
  at [§flm-dd-arch:sources-and-spans]`.  (D) Does this situation not clearly fit
  in either case (A), (B), (C)?  Then, ask the user to categorize your case.

#### Within developer docs

Developer docs may cross-reference other developer docs using
the dedicated `§flm-dd-*:*` label structure.  Such a cross-reference is simply typed
with the bare label in brackets, for instance as "cf. [§flm-dd-arch:document-model]."
in DESIGN_RATIONALE, or "cf. [§flm-dd-arch:rendering]." inside ARCHITECTURE.

General rule: EVERY labeled header of DESIGN_RATIONALE — every decision entry
regardless of status (DECIDED, PROPOSED, OPEN, or DEFERRED), and every header
that groups entries — states the ARCHITECTURE sections it relates to, at least
one, on an `Architecture:` line of its own.  No exceptions: an entry attached to
no part of the architecture would otherwise get lost in the masses.  The
mandatory cross-reference runs in that one direction only: ARCHITECTURE
documents contain no `§flm-dd-dr` reference at all, and never points at
DESIGN_RATIONALE.

The `Architecture:` line has a fixed place and a fixed shape, so that a grep finds it
and its header together:

- it is the VERY NEXT line after the header, with no blank line between the two; the
  blank line and the `Status:` line follow it as before;
- it begins with the word `Architecture:` at column one;
- it names each ARCHITECTURE section as a bare bracketed label, separated by ", ",
  all on that one line however long it becomes (this line is exempt from the usual
  line wrapping, so that one line of grep context always reaches the header).

This line does not specify which of the ARCHITECTURE documents the label is
defined in. This information can be found easily with a simple grep through
`dev-docs/ARCHITECTURE*.md`.

For example:

```md
#### <Short decision title> [§flm-dd-dr:<short-label>]
Architecture: [§flm-dd-arch:rendering], [§flm-dd-arch:stdcore]

Status: <DECIDED|PROPOSED|OPEN|DEFERRED> (<who/context — no date>).
```

Finding the decisions behind an ARCHITECTURE section is then one command: grep the
`Architecture:` lines that name the section, with one line of previous context, which
is always the header of the entry (replace `X` with the section's label name):

```sh
grep -B1 '^Architecture:.*\[§flm-dd-arch:X\]' dev-docs/DESIGN_RATIONALE.md | grep -o '§flm-dd-dr:[a-z0-9-]*'
```

The rule is enforced by manual discipline: an entry and its `Architecture:` line are
written in the same change, and when a labeled header is added, moved, or removed in
either document, grep the repository for the label and keep the `Architecture:` lines
consistent (the procedure is recorded in DESIGN_RATIONALE's "How to use and maintain
this document" section).

#### Never reference archival docs from anywhere

Public-facing docs (API rustdoc, guides) and internal-developer-facing docs (ARCHITECTURE,
DESIGN_RATIONALE) are living documentation that must be kept up to date.
ALL OTHER DOCUMENTATION in `dev-docs/*/` ARE TO BE TREATED AS ARCHIVED HISTORICAL
RECORDS THAT CAN BE REMOVED AT ANY POINT. THEY CANNOT BE REFERENCED IN FLM'S
DOCUMENTATION.

## Related files

- `README.md`, `CLAUDE.md`: live independently and are not considered to be part
  of the project documentation covered here.
- `docs/*.md`, rustdoc in `**/*.rs` — the public-facing docs (see above).
- `dev-docs/ARCHITECTURE*.md`, `dev-docs/DESIGN_RATIONALE.md` — the internal
  developer docs (see above).
- `Documentation_Structure.md` (this file) - the standing specification of the project
  documentation system; lives at the repository root.
- `dev-docs/*/` - Temporary work files, or phase-specific design and
  implementation plans.  Treat as temporary or as obsolete historical design and
  implementation records. These documents MUST NOT be referenced neither from
  public-facing docs NOR internal developer docs.

## Related: Output strings and diagnostic messages

Output strings and diagnostic messages must obey the same documentation clarity
rules as all public-facing documentation. (Output strings and diagnostics are
not considered part of the public or internal-developer-facing documentation
API. These instructions are listed here to make sure agents find them, since
agents that write documentation are likely to also process output strings and
diagnostic texts.)

Diagnostic messages, in particular, must obey the following rules:
(I) state the violated condition directly ("`\text` must stand immediately within the
    quotation environment's body");
(II) do not mention the specific recovery applied to continue processing;
(III) offer possible fixes only if they are not obvious from the violated condition.
    But if you do list one nontrivial fix, then also mention other important fixes
    even if those are trivial;
(IV) avoid FLM-specific jargon.

## Technical considerations

### Guides included in rustdoc

Guide chapters are plain Markdown files living in `docs/` at the repository root.
They are compiled into the rustdoc output as *documentation-only modules*:
`flm-core/src/lib.rs` pulls them in through a custom `guide` module with relevant
`#[cfg(doc)] #[doc=include_str!(...)]` directives.  Each chapter renders as a
page under `flm_core::guide`, with `docs/guide.md` as the
landing page.  The `#[cfg(doc)]` gate keeps these modules out of compiled code
entirely; rustdoc sets `--cfg doc` both when rendering documentation and when
collecting doctests, so the chapters exist only for documentation purposes, yet
their fenced `rust` code blocks are compiled and executed by `cargo test --doc`
like any other doctest. Every example in a guide is therefore compile-checked by
default; opt out deliberately with a `text` or `ignore` fence tag.

Markdown pulled in through `#[doc = include_str!(...)]` is processed exactly as if
it had been written as a documentation comment at that spot in `lib.rs`: standard
rustdoc intra-doc links, e.g., `[`ClassName`](crate::submodule::ClassName)`.
The `rustdoc::broken_intra_doc_links` lint is set to `deny` so link rot fails the
build. Caveat: only the item-path part of a link is verified; a `#fragment`
anchor pointing at a Markdown heading is not validated (careful with links
to the concepts_overview page). Where an API item canonically represents a
concept, link to the item itself.

The documentation sidebar shows the chapters in a pinned "Guide" section through
a customized behavior in `docs/rustdoc-header.html`.  This file contains a
hand-maintained list of guide pages (`GUIDE_PAGES`).

Adding a chapter requires the following steps:

1. create `docs/<chapter>.md`;
2. declare its submodule in the `guide` block of `flm-core/src/lib.rs`;
3. add it to `GUIDE_PAGES` in `docs/rustdoc-header.html` (otherwise the chapter is
   missing from the sidebar);
4. list it in the chapter index of `docs/guide.md`.

Build with `cargo docs` (alias for `cargo doc --workspace --no-deps`); run all guide
examples with `cargo test --doc`. When verifying links, delete `target/doc` first —
rustdoc merges over stale output and can mask rot.

### Rustdoc cross-referencing style (guides and code API docs)

Available link forms (all standard rustdoc):

- API items: `[`ParsingState`]` or `[`ParsingState`](crate::state::ParsingState)`,
  including path-qualified forms.  Resolved and checked by the compiler; with the
  `rustdoc::broken_intra_doc_links` lint set to `deny`, a broken link fails the
  documentation build.
- Guide pages: link the chapter's module path, e.g.
  `[the parsing model](crate::guide::parsing_model)`.  Compiler-checked as well.
- Headings within a page: append the heading's anchor slug (lowercase, hyphens),
  e.g. `crate::guide::concepts_overview#parsing-state`.  The module-path part is
  checked; the `#fragment` part is not — treat published heading slugs as
  immutable, exactly like the `§flm-dd-*` labels.

### Dev-Docs labels

- Exact placement syntax, so greps are uniform: label last on the heading line, e.g.
  `## Sources and spans [§flm-dd-arch:source].`

- Uniqueness: (prefix, label) name pairs must be unique over all
  ARCHITECTURE/DESIGN_RATIONALE documents.  The `§flm-dd-arch:`/`§flm-dd-dr:`
  prefix identifies the type of document (ARCHITECTURE or
  DESIGN_RATIONALE). (The bare label therefore needs no file path and no section
  number.)  This path independence ensures that references survive file moves
  and reorganization (this is especially relevant for the case-(C) references to
  these labels from simple comments in rust modules).

- Immutability: once assigned, never renamed or reused; if a section splits,
  the label stays with the primary successor; removal only after a grep shows
  zero references.

- Granularity: every #/##/###/#### heading carries a label for
  cross-referencing.  Keep labels as short as possible but sufficiently
  expressive that they can easily be kept immutable without clashing with other
  labels.
