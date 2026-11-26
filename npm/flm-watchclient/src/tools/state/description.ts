// The description of the rows of the Metadata and Settings tabs: the root keys that the
// Metadata tab shows in rows of their own; the topics of the common settings of the Settings
// tab in their order, each with its rows; and the curated settings that the advanced list
// always shows under their feature. This is a choice of the tools page's own,
// made knowing the features of the host's package `std`; the program holds nothing of it.
// It is the one module of the tools page that names a feature or one of its keys: changing
// the choice changes this module alone. A row whose path the schema of the document's
// environment does not have is left out (`settings-panel.ts`, `describedRows`). It also
// holds which packages are not added by hand (`loadedAutomatically`), and the code that
// refers to a label (`referenceCode`).

import { FEATURES_PATH, FLM_KEY } from '../../shared/settings';
import type { Option } from './settings';

/** A segment of a described path that stands for the output that the build type shown is made from. */
export const SHOWN_OUTPUT = { shownOutput: true } as const;

/** A segment of a described path: a key, or `SHOWN_OUTPUT`. */
export type Segment = string | typeof SHOWN_OUTPUT;

/** A path from the root of the front matter, as a description gives it. */
export type DescribedPath = readonly Segment[];

/** A checkbox of a group: the entry of the map that it stands for, its label, and a note after the label. */
export interface DescribedBox {
  entry: string;
  label: string;
  note?: string;
}

/**
 * A row of the description, by the kind of its control. `label` is the row's label, where
 * a name between backquotes is code; a curated row of the advanced list has none, as its
 * label is its path from the feature's entry. `path` is the value that the row writes.
 *
 * - `templates`: one row per output, a choice of the compose templates registered for it,
 *   over the key at `path`, which gives one template's name or a name per output;
 * - `template-settings`: one row per setting of the compose template in use, the one that
 *   the key at `template` gives the output that the build type shown is made from. The key
 *   at `path` holds every template's settings, those of a template under its output's name
 *   and then its own; the settings and their defaults are those that the template declares;
 * - `choice`: the `options`, then those of the document's environment named by
 *   `environment`; `custom`, the option "Custom…", which opens the YAML editor of the value
 *   at that path; `settingsOf`, the beginning of the name of the feature that holds the
 *   settings of the option chosen, `settingsOf` followed by the option's value: when that
 *   feature has settings, the link *options…* after the choice shows it in the advanced list;
 * - `switch`, `number`: a switch, a number field;
 * - `checkboxes`: a checkbox per entry of the map at `path` (a feature's entry, whose
 *   entries are its keys, or a key that holds a map); unchecking one writes `false` for its
 *   entry;
 * - `texts`: a list of texts; the row is greyed out while the value at `unless` is `false`;
 * - `links`: a link to each of `features` in the advanced list;
 * - `packages`: the packages in use, in order, and those that can be added; those that
 *   are loaded automatically (`loadedAutomatically`) are listed last, and are not added.
 */
export type Described =
  | { kind: 'templates'; path: DescribedPath }
  | { kind: 'template-settings'; path: DescribedPath; template: DescribedPath }
  | {
    kind: 'choice';
    label?: string;
    path: DescribedPath;
    options: readonly Option[];
    environment?: 'profiles' | 'math_processors';
    custom?: DescribedPath;
    settingsOf?: string;
  }
  | { kind: 'switch'; label?: string; path: DescribedPath }
  | { kind: 'number'; label?: string; path: DescribedPath }
  | { kind: 'checkboxes'; label: string; path: DescribedPath; boxes: readonly DescribedBox[] }
  | { kind: 'texts'; label: string; path: DescribedPath; unless?: DescribedPath }
  | { kind: 'links'; label: string; features: readonly string[] }
  | { kind: 'packages'; path: DescribedPath };

/** A topic of the common settings: its heading and its rows. */
export interface Topic {
  title: string;
  rows: readonly Described[];
}

/** The path of the key `keys` of the feature `name`'s entry. */
function feature(name: string, ...keys: Segment[]): DescribedPath {
  return [...FEATURES_PATH, name, ...keys];
}

/** The path of the key that chooses the compose template: one name, or a name per output. */
const TEMPLATE_PATH: DescribedPath = feature('compose', 'template');

/** The path of the math processor: the row that the note of the packages list leads to. */
export const MATH_PROCESSOR_PATH: DescribedPath = [FLM_KEY, 'math'];

/** The beginning of the name of a math processor's package: the processor `NAME` is the package `math-NAME`. */
export const MATH_PACKAGE_PREFIX = 'math-';

/** The beginning of the name of a build type's package: the build type `NAME` uses the package `profile-NAME`. */
export const PROFILE_PACKAGE_PREFIX = 'profile-';

/**
 * The beginnings of the names of the packages that are not added by hand: the selected math
 * processor and build type load them. The program treats them as any other package.
 */
const AUTOMATIC_PREFIXES: readonly string[] = [MATH_PACKAGE_PREFIX, PROFILE_PACKAGE_PREFIX];

/** Whether the package `name` is one that the math processor or the build type loads, not added by hand. */
export function loadedAutomatically(name: string): boolean {
  return AUTOMATIC_PREFIXES.some((prefix) => name.startsWith(prefix));
}

/**
 * The label type of the labels that a term definition computes from its term and its
 * aliases, whose label proper is the term as written: a term is referred to by `\term`.
 */
const TERM_LABEL_TYPE = 'defterm';

/**
 * The code that refers to the label `label`, as written (`type:label`): `\term{…}` with the
 * label proper for a label of a term (the type `defterm`), `\ref{…}` with the whole label
 * for every other.
 */
export function referenceCode(label: string): string {
  const colon = label.indexOf(':');
  if (colon >= 0 && label.slice(0, colon) === TERM_LABEL_TYPE) return `\\term{${label.slice(colon + 1)}}`;
  return `\\ref{${label}}`;
}

/** The common settings, by topic, in their order. */
export const TOPICS: readonly Topic[] = [
  { title: 'Template', rows: [{ kind: 'templates', path: TEMPLATE_PATH }] },
  {
    title: 'Build',
    rows: [{ kind: 'choice', label: 'Default build type', path: [FLM_KEY, 'default_profile'], options: [], environment: 'profiles' }],
  },
  {
    title: 'Mathematics',
    rows: [
      {
        kind: 'choice',
        label: 'Processor',
        path: MATH_PROCESSOR_PATH,
        options: [{ label: 'none', value: false }],
        environment: 'math_processors',
        // The processor `NAME` is the package `math-NAME`, whose feature has its name.
        settingsOf: MATH_PACKAGE_PREFIX,
      },
      { kind: 'switch', label: 'Allow `$…$` and `$$…$$` for mathematics', path: [FLM_KEY, 'parsing', 'dollar_math'] },
    ],
  },
  {
    title: 'Headings',
    rows: [
      {
        kind: 'choice',
        label: 'Numbered',
        path: feature('headings', 'numbering'),
        options: [
          { label: 'None', value: 'nothing-numbered' },
          { label: 'All', value: 'all-numbered' },
          { label: 'Sections only', value: { 'up-to-depth': 1 } },
          { label: 'Sections and subsections', value: { 'up-to-depth': 2 } },
        ],
      },
      { kind: 'number', label: 'Contents depth', path: feature('toc', 'depth') },
    ],
  },
  {
    title: 'Typography',
    rows: [
      {
        // Each family of the typographic substitutions of the feature `blocks`, shown as what
        // the source spells (in the monospace stack) and what it becomes (in the note).
        kind: 'checkboxes',
        label: 'Substitutions',
        path: feature('blocks'),
        boxes: [
          { entry: 'auto_unicode_quotes', label: '"a"', note: '→ “a”' },
          { entry: 'ligature_unicode_quotes', label: "``a''", note: '→ “a”' },
          { entry: 'ligature_unicode_dashes', label: '-- ---', note: '→ – —' },
          { entry: 'ligature_unicode_ellipses', label: '...', note: '→ …' },
        ],
      },
    ],
  },
  {
    title: 'Bibliography',
    rows: [
      {
        kind: 'checkboxes',
        label: 'Citation keys',
        path: feature('citations-standard', 'sources'),
        boxes: [
          { entry: 'bib', label: 'bib:', note: 'local files' },
          { entry: 'arxiv', label: 'arxiv:ID' },
          { entry: 'doi', label: 'doi:DOI' },
          { entry: 'manual', label: 'manual:{citation}' },
        ],
      },
      {
        kind: 'texts',
        label: 'Files',
        path: feature('citations-standard', 'bibliography'),
        unless: feature('citations-standard', 'sources', 'bib'),
      },
      {
        kind: 'choice',
        label: 'Citation marks',
        path: feature('citations', 'mark'),
        options: [
          { label: '[1]', value: 'bracketed' },
          { label: '¹', value: 'superscript' },
        ],
        custom: feature('citations', 'style'),
      },
      { kind: 'links', label: 'Advanced:', features: ['citations', 'citations-standard'] },
    ],
  },
  {
    title: 'Current template settings',
    rows: [{ kind: 'template-settings', path: feature('compose', 'template_config'), template: TEMPLATE_PATH }],
  },
  { title: 'Packages', rows: [{ kind: 'packages', path: [FLM_KEY, 'use'] }] },
];

/**
 * A row of its own of the Metadata tab, over the root key `key`: a text field (`text`),
 * whose emptying removes the key, with `placeholder` while it is empty; or a list of texts
 * shown as chips (`texts`), with the button `add`.
 */
export type MetadataRow =
  | { kind: 'text'; label: string; key: string; placeholder?: string }
  | { kind: 'texts'; label: string; key: string; add: string };

/** The root keys that the Metadata tab shows in rows of their own, in their order. */
export const METADATA: readonly MetadataRow[] = [
  { kind: 'text', label: 'Title', key: 'title' },
  { kind: 'texts', label: 'Authors', key: 'author', add: 'Add author' },
  { kind: 'text', label: 'Date', key: 'date', placeholder: 'Not set' },
];

/** The curated settings of the advanced list, by feature: always shown, set or not. */
export const CURATED: Readonly<Record<string, readonly Described[]>> = {
  floats: [
    {
      kind: 'choice',
      path: feature('floats', 'types', 'figure', 'numbering'),
      options: [
        { label: 'auto-numbered', value: 'auto-numbered' },
        { label: 'numbered', value: 'numbered' },
        { label: 'unnumbered', value: 'unnumbered' },
      ],
    },
    {
      kind: 'choice',
      path: feature('floats', 'types', 'figure', 'number_format'),
      options: [
        { label: '1, 2, 3', value: { formats: '#{arabic}' } },
        { label: 'a, b, c', value: { formats: '#{alph}' } },
        { label: 'i, ii, iii', value: { formats: '#{roman}' } },
      ],
      custom: feature('floats', 'types', 'figure', 'number_format'),
    },
  ],
  graphics: [
    { kind: 'number', path: feature('graphics', 'magnification', SHOWN_OUTPUT, 'raster') },
    { kind: 'number', path: feature('graphics', 'magnification', SHOWN_OUTPUT, 'vector') },
  ],
};
