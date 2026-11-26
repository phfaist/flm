// The settings state as the server gives it (`GET /.flm-preview/settings`), and the change
// of the front matter that the tools page sends (`POST /.flm-preview/front-matter`).
// `PROTOCOL.md` specifies both; this module reads the state's fields that the tools page
// uses and leaves the others as they are.

import { isObject } from './protocol';

/** The request that reads the settings state. */
export const SETTINGS = '/.flm-preview/settings';

/** The request that reads the value at a path of the front matter as YAML text. */
export const SETTINGS_YAML = '/.flm-preview/settings/yaml';

/** The request that reads what each settings source writes at a path of the front matter. */
export const SETTINGS_SOURCES = '/.flm-preview/settings/sources';

/** The request that makes the changes of one control's change to the front matter, together. */
export const FRONT_MATTER = '/.flm-preview/front-matter';

/** The root key of the front matter that holds the settings of FLM itself. */
export const FLM_KEY = 'flm';

/** The path of the features' settings in the front matter: a map from a feature's name to its entry. */
export const FEATURES_PATH: readonly string[] = [FLM_KEY, 'features'];

/** The key of a feature's entry that switches the feature on or off. */
export const SWITCH_KEY = 'enabled';

/** The kinds of source a value can come from, the lowest rank first. */
export const SOURCES = ['defaults', 'package', 'front_matter', 'workflow', 'command_line'] as const;

export type SourceKind = (typeof SOURCES)[number];

/** Where a value comes from. */
export interface Origin {
  source: SourceKind;
  /** The program's text for the origin, as "the option `-O toc.depth=2`". */
  text: string;
  /** For a package and a workflow: the package's name, `IDENTITY#PACKAGE`. */
  package: string | null;
  /** For a package and a workflow: the extension's stem, the name that `use` gives it. */
  extension: string | null;
  /** For a workflow: its name. */
  workflow: string | null;
}

/**
 * The origins below a value combined key by key: for each key of the value that a source
 * writes, the origin of the write that gives it, and, when that key's value is itself
 * combined key by key, the origins below it (`entries`, else `null`).
 */
export type EntryOrigins = Record<string, { origin: Origin; entries: EntryOrigins | null }>;

/** A value in effect, and where it comes from. */
export interface InEffect {
  value: unknown;
  origin: Origin;
  /** For a key combined key by key: the origins below it; `null` for a value decided whole. */
  entries: EntryOrigins | null;
}

/** A feature in effect: the origin of its switch when it is off, and its keys in effect. */
export interface FeatureInEffect {
  switchedOff: Origin | null;
  keys: Record<string, InEffect>;
}

/** The settings state, as far as the tools page reads it. */
export interface SettingsState {
  /** The revision of the front matter, or `null` when it cannot be read. */
  revision: string | null;
  /** Why the front matter cannot be read, one line of text; `null` when it can. */
  unreadable: string | null;
  /** What the front matter writes; `null` for a file without one, and when it cannot be read. */
  frontMatter: Record<string, unknown> | null;
  /** The JSON Schema of a front matter. */
  schema: Record<string, unknown> | null;
  /** The features in effect, by name; empty before the first successful preparation. */
  features: Record<string, FeatureInEffect>;
  /** The keys of `flm` itself in effect, by name; empty before it. */
  flm: Record<string, unknown>;
  /** The choices that the document's environment offers, as the server gives them; or `null`. */
  choices: Record<string, unknown> | null;
  /** Why the last preparation failed, one line per problem; `null` after one that succeeded. */
  preparationFailed: string | null;
  /** Whether `schema` is that of the document's packages as the last build found them. */
  schemaCurrent: boolean;
}

/** One change of the front matter: a value set (as data or as YAML text), or a removal. */
export type Change = { value: unknown } | { yaml: string } | { remove: true };

/**
 * The body of `POST /.flm-preview/front-matter`: the changes, made in the order given
 * against the one revision, each at its path.
 */
export type FrontMatterWrite = { revision: string; changes: ({ path: string[] } & Change)[] };

function textOrNull(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

/** The origin of `value`, or `null` when it is not one. */
export function readOrigin(value: unknown): Origin | null {
  if (!isObject(value) || typeof value.text !== 'string') return null;
  const source = SOURCES.find((kind) => kind === value.source);
  if (source === undefined) return null;
  return {
    source,
    text: value.text,
    package: textOrNull(value.package),
    extension: textOrNull(value.extension),
    workflow: textOrNull(value.workflow),
  };
}

/** The origins below a value of the field `entries`, `{KEY: {origin, entries?}}`, or `null` when it is not a map. */
function readEntries(value: unknown): EntryOrigins | null {
  if (!isObject(value)) return null;
  const entries: EntryOrigins = {};
  for (const [name, each] of Object.entries(value)) {
    const origin = isObject(each) ? readOrigin(each.origin) : null;
    if (origin !== null && isObject(each)) entries[name] = { origin, entries: readEntries(each.entries) };
  }
  return entries;
}

/** The value in effect of `value`, `{value, origin, entries?}`, or `null` when it is not one. */
export function readInEffect(value: unknown): InEffect | null {
  if (!isObject(value)) return null;
  const origin = readOrigin(value.origin);
  if (origin === null || !('value' in value)) return null;
  return { value: value.value, origin, entries: readEntries(value.entries) };
}

function readFeatures(value: unknown): Record<string, FeatureInEffect> {
  const features: Record<string, FeatureInEffect> = {};
  if (!isObject(value)) return features;
  for (const [name, feature] of Object.entries(value)) {
    if (!isObject(feature)) continue;
    const keys: Record<string, InEffect> = {};
    if (isObject(feature.keys)) {
      for (const [key, each] of Object.entries(feature.keys)) {
        const read = readInEffect(each);
        if (read !== null) keys[key] = read;
      }
    }
    features[name] = { switchedOff: readOrigin(feature.switched_off), keys };
  }
  return features;
}

/** The settings state of the response's JSON `value`, or `null` when it is not one. */
export function readSettings(value: unknown): SettingsState | null {
  if (!isObject(value)) return null;
  const revision = textOrNull(value.revision);
  const unreadable = textOrNull(value.unreadable);
  if (revision === null && unreadable === null) return null;
  const inEffect = isObject(value.in_effect) ? value.in_effect : {};
  return {
    revision,
    unreadable,
    frontMatter: isObject(value.front_matter) ? value.front_matter : null,
    schema: isObject(value.schema) ? value.schema : null,
    features: readFeatures(inEffect.features),
    flm: isObject(inEffect.flm) ? inEffect.flm : {},
    choices: isObject(value.choices) ? value.choices : null,
    preparationFailed: textOrNull(value.preparation_failed),
    schemaCurrent: value.schema_current !== false,
  };
}

/**
 * The JSON Pointer (RFC 6901) of `path`, the keys from the root of the front matter, as
 * `GET /.flm-preview/settings/yaml` takes it: each key after a `/`, with `~` written `~0`
 * and `/` written `~1`.
 */
export function pointerOf(path: readonly string[]): string {
  return path.map((key) => `/${key.replace(/~/g, '~0').replace(/\//g, '~1')}`).join('');
}

/** One source's write at a path: the value as the source writes it, its origin, and whether it decides the value there. */
export interface SourceWrite {
  value: unknown;
  origin: Origin;
  inEffect: boolean;
}

/** A value that a write copies into the front matter: its path from the root of the front matter, and the value. */
export interface Copied {
  path: string[];
  value: unknown;
}

/** What the settings give at one path (`GET /.flm-preview/settings/sources`), as far as the tools page reads it. */
export interface Sources {
  /** Each source's write, lowest rank first; `null` for a path under no feature's key, and before the first preparation. */
  sources: SourceWrite[] | null;
}

/** The copies of the list `value`, `[{path, value}, …]`, leaving out an entry that is not one. */
export function readCopied(value: unknown): Copied[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((each) => (isObject(each) && Array.isArray(each.path) && each.path.every((key) => typeof key === 'string') && 'value' in each
    ? [{ path: each.path as string[], value: each.value }]
    : []));
}

/** The sources at a path of the response's JSON `value`, or `null` when it is not an answer of that request. */
export function readSources(value: unknown): Sources | null {
  if (!isObject(value) || !('sources' in value)) return null;
  let sources: SourceWrite[] | null = null;
  if (Array.isArray(value.sources)) {
    sources = value.sources.flatMap((each) => {
      const origin = isObject(each) ? readOrigin(each.origin) : null;
      return origin !== null && isObject(each) ? [{ value: each.value, origin, inEffect: each.in_effect === true }] : [];
    });
  }
  return { sources };
}
