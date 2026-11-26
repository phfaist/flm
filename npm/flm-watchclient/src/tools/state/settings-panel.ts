// What the Settings tab makes of the settings state and of the description of the common
// and curated settings (`description.ts`): the rows that the document's environment has,
// the value each row shows and where it comes from, the control that its schema gives, the
// words of its marks, the writes of the template rows, of the switch of a feature and of a
// group of checkboxes, the settings of the compose template in use, and the order of the
// advanced list.
//
// After a preparation that failed, what the front matter writes is shown from the front
// matter, current and enabled as always: a key's value, a feature's switch, and the
// features and keys it writes, which the lists hold even when the schema lacks them. What
// comes from the last successful preparation (a value the front matter does not write, with
// its source) is marked as outdated; a list of texts, whose write would copy such a value
// into the front matter, is disabled. The server refuses any other change for which it would
// copy such values.
//
// Nothing here names a feature or a key, and nothing touches the page, so that the tests
// can run each case.

import type { InEffect, Origin, SettingsState } from '../../shared/settings';
import { FEATURES_PATH, SWITCH_KEY, readInEffect } from '../../shared/settings';
import { isObject } from '../../shared/protocol';
import type { Described, DescribedPath } from './description';
import { MATH_PACKAGE_PREFIX, PROFILE_PACKAGE_PREFIX, loadedAutomatically } from './description';
import type { Control, Edit, Option, Path, Source } from './settings';
import { MARKS_KEYWORD, admitsNull, changed, controlOf, isWritten, markLines, resolved, reverted, sourceOf } from './settings';

type Schema = Record<string, unknown>;

/** The value at `path` in `data`: `undefined` when a key of the path is missing. */
export function valueAt(data: unknown, path: Path): unknown {
  let current = data;
  for (const key of path) {
    if (!isObject(current) || !Object.hasOwn(current, key)) return undefined;
    current = current[key];
  }
  return current;
}

/** The alternatives of `anyOf` or `oneOf` that can hold keys, or `null` for a schema without them. */
function keyedAlternatives(schema: Schema): unknown[] | null {
  const branches = Array.isArray(schema.anyOf) ? schema.anyOf : Array.isArray(schema.oneOf) ? schema.oneOf : null;
  return branches === null ? null : branches.filter((branch) => !(isObject(branch) && ('const' in branch || branch.type === 'null')));
}

/**
 * The schema of the key `key` of a value of `schema`: a property, a property of the first
 * alternative that has the key, or the schema of every entry of a map.
 */
function childOf(root: Schema | null, schema: Schema, key: string): Schema | null {
  const own = resolved(root, schema);
  if (own === null) return null;
  if (isObject(own.properties) && Object.hasOwn(own.properties, key)) return resolved(root, own.properties[key]);
  const branches = keyedAlternatives(own);
  if (branches !== null) {
    for (const branch of branches) {
      const each = resolved(root, branch);
      const child = each === null ? null : childOf(root, each, key);
      if (child !== null) return child;
    }
    return null;
  }
  return isObject(own.additionalProperties) ? resolved(root, own.additionalProperties) : null;
}

/** The schema of the value at `path` in the schema `root` of a front matter, or `null` when it has none. */
export function schemaAt(root: Schema | null, path: Path): Schema | null {
  let current = root;
  for (const key of path) {
    if (current === null) return null;
    current = childOf(root, current, key);
  }
  return current;
}

/**
 * The default of the value at `path`: the default of its schema, else the value at the
 * rest of the path in the default of the nearest key of the path that has one.
 */
export function defaultAt(root: Schema | null, path: Path): unknown {
  let current = root;
  let found: { value: unknown; rest: Path } | null = null;
  for (let at = 0; at < path.length && current !== null; at += 1) {
    current = childOf(root, current, path[at]!);
    if (current !== null && 'default' in current) found = { value: current.default, rest: path.slice(at + 1) };
  }
  return found === null ? undefined : valueAt(found.value, found.rest);
}

/** Whether `path` begins with the keys of `start`. */
function startsWith(path: Path, start: Path): boolean {
  return start.every((key, at) => path[at] === key);
}

/** The feature whose entry holds `path`, or `null` for a path outside the features' settings. */
export function featureOf(path: Path): string | null {
  return path.length > FEATURES_PATH.length && startsWith(path, FEATURES_PATH) ? path[FEATURES_PATH.length]! : null;
}

/** A value in effect at a path, and its origin: `null` for a value of the feature's own default, which no source writes. */
export interface ValueInEffect {
  value: unknown;
  origin: Origin | null;
}

/**
 * The origin of the value at `rest`, the names below the key `key`, read down the origins
 * below the key (`entries`): a value inside one decided whole has the origin of the value
 * that holds it; a name that a map of origins lacks is a value of the feature's own default,
 * without an origin.
 */
function originBelow(key: InEffect, rest: Path): Origin | null {
  let origin: Origin = key.origin;
  let entries = key.entries;
  for (const name of rest) {
    if (entries === null) return origin;
    const entry = entries[name];
    if (entry === undefined) return null;
    origin = entry.origin;
    entries = entry.entries;
  }
  return origin;
}

/**
 * The value in effect at `path` and its origin, the origin that the state gives for that
 * value itself; `null` when no value is in effect there.
 */
export function inEffectAt(state: SettingsState, path: Path): ValueInEffect | null {
  const name = featureOf(path);
  if (name !== null) {
    const at = FEATURES_PATH.length + 1;
    const key = state.features[name]?.keys[path[at] ?? ''];
    if (key === undefined) return null;
    const rest = path.slice(at + 1);
    const value = valueAt(key.value, rest);
    return value === undefined ? null : { value, origin: originBelow(key, rest) };
  }
  const [, key, inner] = path;
  if (key === undefined || !startsWith(path, FEATURES_PATH.slice(0, 1))) return null;
  const value = state.flm[key];
  if (inner === undefined) return readInEffect(value);
  return path.length === 3 && isObject(value) ? readInEffect(value[inner]) : null;
}

/**
 * Whether the front matter writes the value at `path`. A `null` counts as not written,
 * except for a key whose schema admits `null` (`nullable`), where it is the value given.
 */
export function writtenAt(state: SettingsState, path: Path, nullable = false): boolean {
  const written = valueAt(state.frontMatter, path);
  return nullable ? written !== undefined : isWritten(written);
}

/** The value that a row at `path` shows: the one the front matter writes, else the one in effect, else the default. */
export function shownValue(state: SettingsState, path: Path, nullable = false): unknown {
  if (writtenAt(state, path, nullable)) return valueAt(state.frontMatter, path);
  const inEffect = inEffectAt(state, path);
  return inEffect !== null ? inEffect.value : defaultAt(state.schema, path);
}

/** Where the value of a row at `path` comes from. */
export function sourceAt(state: SettingsState, path: Path, nullable = false): Source {
  return sourceOf(writtenAt(state, path, nullable), inEffectAt(state, path)?.origin ?? null, !preparationFailed(state));
}

/** The control of the key at `path`, from its schema (`controlOf`). */
export function keyControl(state: SettingsState, path: Path): Control {
  return controlOf(state.schema, schemaAt(state.schema, path));
}

/** Whether the schema of the key at `path` admits `null`: its row can then be given no value. */
export function nullableAt(state: SettingsState, path: Path): boolean {
  return admitsNull(state.schema, schemaAt(state.schema, path));
}

/**
 * The write of the checkbox before the control of a key whose schema admits `null`:
 * unchecked, the key is given no value, `null`; checked, nothing is written until the
 * control gives a value (`null`).
 */
export function givenEdit(path: Path, given: boolean): Edit | null {
  return given ? null : changed(path, null);
}

/** The marks' names that `schema` writes, on itself and on its alternatives. */
function marksOf(root: Schema | null, schema: unknown, depth = 0): string[] {
  const own = resolved(root, schema);
  if (own === null || depth > 8) return [];
  const marks = Array.isArray(own[MARKS_KEYWORD]) ? own[MARKS_KEYWORD].filter((each): each is string => typeof each === 'string') : [];
  const branches = Array.isArray(own.anyOf) ? own.anyOf : Array.isArray(own.oneOf) ? own.oneOf : [];
  const inner = branches.flatMap((branch) => marksOf(root, branch, depth + 1));
  return [...new Set([...marks, ...inner])];
}

/**
 * The lines in words of the marks of the value at `path` and of the object that holds it,
 * as the schema writes them (`markLines`), for the tooltip of its row.
 */
export function marksAt(state: SettingsState, path: Path): string[] {
  const holder = path.length > 0 ? marksOf(state.schema, schemaAt(state.schema, path.slice(0, -1))) : [];
  return markLines(marksOf(state.schema, schemaAt(state.schema, path)), holder);
}

/**
 * Whether the rows at `path` are locked whatever the source of their value: the front matter
 * writes, at a key of the path but the last, a value that is neither a map nor `null`, such
 * as the entry `FEATURE: false` of a feature switched off, and the server refuses a write
 * through it. A row of a feature that a source other than the front matter switches off is
 * not locked, nor one that a higher source decides: its write applies under another build
 * type, or without that option.
 */
export function rowLocked(state: SettingsState, path: Path): boolean {
  return path.slice(0, -1).some((_, at) => {
    const value = valueAt(state.frontMatter, path.slice(0, at + 1));
    return value !== undefined && value !== null && !isObject(value);
  });
}

// --- after a preparation that failed ------------------------------------------------------

/**
 * Whether the last preparation failed: the values that the front matter does not write, and
 * the choices that need the registry, are then those of the last successful build.
 */
export function preparationFailed(state: SettingsState): boolean {
  return state.preparationFailed !== null;
}

/**
 * Whether the value that a row at `path` shows is that of the last successful build: the
 * last preparation failed, and the front matter does not write the path. An edit that starts
 * from that value would copy it into the front matter: the chips of a list of texts are
 * disabled then, and the YAML editor starts empty.
 */
export function outdatedAt(state: SettingsState, path: Path): boolean {
  return preparationFailed(state) && !isWritten(valueAt(state.frontMatter, path));
}

// --- the choices of the environment -------------------------------------------------------

/** The texts of a list of the choices, `choices[field]`; empty when it is not one. */
function namesOf(state: SettingsState, field: string): string[] {
  const value = state.choices?.[field];
  return Array.isArray(value) ? value.filter((each): each is string => typeof each === 'string') : [];
}

/** The options of a choice that the document's environment offers: the build types, or the math processors. */
export function environmentOptions(state: SettingsState, field: 'profiles' | 'math_processors'): Option[] {
  return namesOf(state, field).map((name) => ({ label: name, value: name }));
}

/**
 * The output that the build type shown is made from: the first output of the last
 * preparation that a compose template assembles (its `composed_as`), or `null`.
 */
export function shownOutput(state: SettingsState): string | null {
  const outputs = state.choices?.outputs;
  if (!Array.isArray(outputs)) return null;
  for (const output of outputs) {
    if (isObject(output) && typeof output.composed_as === 'string') return output.composed_as;
  }
  return null;
}

/** A package found: its name, and the description of its manifest, or `null`. */
export interface Found {
  name: string;
  description: string | null;
}

/** The packages found, in the order in which a name is looked up. */
export function packagesFound(state: SettingsState): Found[] {
  const packages = state.choices?.packages;
  if (!Array.isArray(packages)) return [];
  return packages.flatMap((each) => isObject(each) && typeof each.name === 'string'
    ? [{ name: each.name, description: typeof each.description === 'string' ? each.description : null }]
    : []);
}

/** A package that the packages list offers: as found, and whether it is added by hand. */
export interface Offered extends Found {
  byHand: boolean;
}

/**
 * The packages that the packages list offers, those found: first those added by hand, then
 * those that the math processor or the build type loads (`loadedAutomatically`), each in
 * the order found.
 */
export function packagesOffered(state: SettingsState): Offered[] {
  const offered = packagesFound(state).map((each) => ({ ...each, byHand: !loadedAutomatically(each.name) }));
  return [...offered.filter((each) => each.byHand), ...offered.filter((each) => !each.byHand)];
}

/** A package in use that the front matter does not list under `use`, and why it is in use, a line of text. */
export interface PackageInUse {
  name: string;
  why: string;
}

/**
 * The packages in use (the choice `packages_in_use`) that `written`, the names that the
 * front matter lists under `use`, does not hold, in their order, each with why it is in use:
 * it is the package of the math processor, or of the build type `profile` (the one in use);
 * it is in the initial list; another source lists it under `use` (the command line); or
 * else another package uses it.
 */
export function packagesInUse(state: SettingsState, written: readonly string[], profile: string | null): PackageInUse[] {
  const inUse = state.choices?.packages_in_use;
  if (!Array.isArray(inUse)) return [];
  const math = readInEffect(state.flm.math)?.value;
  const initial = readInEffect(state.flm.initial_use)?.value;
  const uses = Array.isArray(state.flm.use) ? state.flm.use.map(readInEffect) : [];
  const why = (name: string): string => {
    if (typeof math === 'string' && name === `${MATH_PACKAGE_PREFIX}${math}`) return `Via math processor ‘${math}’`;
    if (profile !== null && name === `${PROFILE_PACKAGE_PREFIX}${profile}`) return `Via build type ‘${profile}’`;
    if (Array.isArray(initial) && initial.includes(name)) return 'Via initial package list';
    const listed = uses.find((each) => each !== null && each.value === name);
    if (listed != null) return `Via ${listed.origin.text}`;
    return 'Used by another package';
  };
  return inUse.flatMap((name) => (typeof name === 'string' && !written.includes(name) ? [{ name, why: why(name) }] : []));
}

/**
 * Whether the packages list shows a package that the math processor or the build type
 * loads (`loadedAutomatically`), among `names`, the packages in use, or among those offered:
 * the list's note then says how they are loaded.
 */
export function automaticPackagesShown(state: SettingsState, names: readonly string[]): boolean {
  return [...names, ...packagesFound(state).map((each) => each.name)].some(loadedAutomatically);
}

// --- the rows of the description ----------------------------------------------------------

/** A row of the description with its paths, for the document's environment. */
export interface DescribedRow {
  row: Described;
  /** The keys of the row's path; empty for a row of links. */
  path: string[];
  /** For a choice: the keys of the path of "Custom…", or `null` for a choice without it. */
  custom: string[] | null;
  /** For a row of texts: the keys of the path of `unless`, or `null`. */
  unless: string[] | null;
  /** For the settings of a template: the keys of the path of the key that chooses the template; else `null`. */
  template: string[] | null;
  /** For a row of links: the features of the links that the environment has. */
  features: string[];
}

/** The keys of a described path, or `null` when the build type shown is made from no output that it can name. */
export function pathOf(state: SettingsState, path: DescribedPath): string[] | null {
  const output = shownOutput(state);
  const keys: string[] = [];
  for (const segment of path) {
    if (typeof segment === 'string') keys.push(segment);
    else if (output === null) return null;
    else keys.push(output);
  }
  return keys;
}

/** The keys of a described path that the schema of the document's environment has, or `null`. */
function presentPath(state: SettingsState, path: DescribedPath | undefined): string[] | null {
  const keys = path === undefined ? null : pathOf(state, path);
  return keys !== null && schemaAt(state.schema, keys) !== null ? keys : null;
}

/**
 * The rows of `rows` that the document's environment has: a row is left out when the schema
 * has no value at its path (the document uses no package with its feature); a row of links
 * keeps the links to the features it has; a choice loses "Custom…", and a row of texts its
 * `unless`, when the schema lacks their path; the settings of a template are left out when
 * it lacks the path of the key that chooses the template.
 */
export function describedRows(state: SettingsState, rows: readonly Described[]): DescribedRow[] {
  const result: DescribedRow[] = [];
  for (const row of rows) {
    if (row.kind === 'links') {
      const declared = declaredFeatures(state);
      const features = row.features.filter((name) => declared.includes(name));
      if (features.length > 0) result.push({ row, path: [], custom: null, unless: null, template: null, features });
      continue;
    }
    const path = presentPath(state, row.path);
    if (path === null) continue;
    const custom = row.kind === 'choice' ? presentPath(state, row.custom) : null;
    const unless = row.kind === 'texts' ? presentPath(state, row.unless) : null;
    const template = row.kind === 'template-settings' ? presentPath(state, row.template) : null;
    if (row.kind === 'template-settings' && template === null) continue;
    result.push({ row, path, custom, unless, template, features: [] });
  }
  return result;
}

/** The label of a curated row: its path from the feature's entry, keys joined by dots. */
export function curatedLabel(path: Path): string {
  return path.slice(FEATURES_PATH.length + 1).join('.');
}

// --- the switch of a feature --------------------------------------------------------------

/** The path of the feature `name`'s entry. */
export function entryPath(name: string): string[] {
  return [...FEATURES_PATH, name];
}

/** The origin of the source that switches the feature `name` off in effect, or `null` when it is on. */
export function switchedOff(state: SettingsState, name: string): Origin | null {
  return state.features[name]?.switchedOff ?? null;
}

/**
 * The switch that the front matter writes in the feature `name`'s entry, as the settings'
 * reader of a block reads it: off for `false` and for a map whose `enabled` is `false`; on
 * for `null`, for a map without `enabled`, and for one whose `enabled` is `true`; `null`
 * where the front matter writes no entry, or one that the reader refuses.
 */
export function writtenSwitch(state: SettingsState, name: string): boolean | null {
  const features = valueAt(state.frontMatter, FEATURES_PATH);
  if (!isObject(features) || !Object.hasOwn(features, name)) return null;
  const entry = features[name];
  if (entry === null) return true;
  if (entry === false) return false;
  if (!isObject(entry)) return null;
  if (!Object.hasOwn(entry, SWITCH_KEY)) return true;
  const on = entry[SWITCH_KEY];
  return typeof on === 'boolean' ? on : null;
}

/** The switch of a feature as the Settings tab shows it. */
export interface ShownSwitch {
  on: boolean;
  /** The source beside the switch: the higher source that switches it off, the mark of a value of the last successful build, or none. */
  source: Source | null;
}

/**
 * The switch of the feature `name`: the one the front matter writes, else the one in
 * effect. A source above the front matter that turns the feature off is named beside the
 * switch, which still writes the front matter. A switch not written in the front matter is
 * that of the last successful build after a preparation that failed.
 */
export function shownSwitch(state: SettingsState, name: string): ShownSwitch {
  const off = switchedOff(state, name);
  const written = writtenSwitch(state, name);
  const offSource = off === null ? null : sourceOf(written !== null, off);
  const decided = offSource !== null && offSource.kind === 'decided' ? offSource : null;
  if (written !== null) return { on: written, source: decided };
  if (preparationFailed(state)) return { on: off === null, source: { kind: 'outdated' } };
  return { on: off === null, source: decided };
}

/** Whether the feature `name` is on, as the Settings tab shows its switch. */
export function switchedOn(state: SettingsState, name: string): boolean {
  return shownSwitch(state, name).on;
}

/**
 * The write of the switch *Enabled* of the feature `name`, to `on`. Off writes `enabled:
 * false` in the feature's entry. On removes that key where the front matter writes it, or
 * the entry `FEATURE: false`; where a lower source switched the feature off, it writes
 * `enabled: true`.
 */
export function switchEdit(state: SettingsState, name: string, on: boolean): Edit {
  const entry = entryPath(name);
  const switchPath = [...entry, SWITCH_KEY];
  if (!on) return changed(switchPath, false);
  const written = valueAt(state.frontMatter, entry);
  if (written === false) return reverted(entry);
  if (isObject(written) && written[SWITCH_KEY] === false) return reverted(switchPath);
  return changed(switchPath, true);
}

// --- the template rows --------------------------------------------------------------------

/** A compose template registered for an output: its name, and its own settings with their defaults, in its order. */
interface Registered {
  name: string;
  config: Record<string, unknown>;
}

/** The compose templates registered for each output, by output. */
function templatesByOutput(state: SettingsState): Map<string, Registered[]> {
  const result = new Map<string, Registered[]>();
  const templates = state.choices?.templates;
  if (!isObject(templates)) return result;
  for (const [output, list] of Object.entries(templates)) {
    if (!Array.isArray(list)) continue;
    result.set(output, list.flatMap((each) => (isObject(each) && typeof each.name === 'string'
      ? [{ name: each.name, config: isObject(each.config) ? each.config : {} }]
      : [])));
  }
  return result;
}

/** The outputs of the template rows: the one that the build type shown is made from first, then the others in their order. */
export function templateOutputs(state: SettingsState): string[] {
  const outputs = [...templatesByOutput(state).keys()];
  const shown = shownOutput(state);
  return shown !== null && outputs.includes(shown) ? [shown, ...outputs.filter((each) => each !== shown)] : outputs;
}

/** The options of the template row of `output`. */
export function templateOptions(state: SettingsState, output: string): Option[] {
  return (templatesByOutput(state).get(output) ?? []).map(({ name }) => ({ label: name, value: name }));
}

/** The template's name that `choice` gives `output`: one name for all outputs, or the entry of a map. */
function templateIn(choice: unknown, output: string): unknown {
  if (typeof choice === 'string') return choice;
  return isObject(choice) ? choice[output] : undefined;
}

/**
 * The value of the template row of `output`, over the key at `path`: the name that the
 * front matter gives the output, else the one in effect, else the default. A map in effect
 * that does not list the output leaves it the default.
 */
export function templateValue(state: SettingsState, path: Path, output: string): unknown {
  const written = templateIn(valueAt(state.frontMatter, path), output);
  if (isWritten(written)) return written;
  const inEffect = inEffectAt(state, path);
  const given = inEffect === null ? undefined : templateIn(inEffect.value, output);
  return isWritten(given) ? given : templateIn(defaultAt(state.schema, path), output);
}

/** Where the value of the template row of `output` comes from. */
export function templateSource(state: SettingsState, path: Path, output: string): Source {
  const written = isWritten(templateIn(valueAt(state.frontMatter, path), output));
  const inEffect = inEffectAt(state, path);
  const given = inEffect !== null && isWritten(templateIn(inEffect.value, output));
  return sourceOf(written, given ? inEffect.origin : null, !preparationFailed(state));
}

/**
 * The write of a choice of the template row of `output`: the name for that output alone,
 * as the author chose it. The server writes out the names that the other outputs keep.
 */
export function templateEdit(path: Path, output: string, chosen: unknown): Edit {
  return changed([...path, output], chosen);
}

/** The write of the revert arrow of the template row of `output`: its entry removed. */
export function templateRevert(path: Path, output: string): Edit {
  return reverted([...path, output]);
}

// --- the settings of the template in use --------------------------------------------------

/** How many settings of a template the tab shows before the link "More…". */
export const TEMPLATE_SETTINGS_FIRST = 10;

/** The compose template in use: the one that assembles the output that the build type shown is made from. */
export interface TemplateInUse {
  /** The output that the build type shown is made from. */
  output: string;
  /** The template's name. */
  name: string;
  /**
   * The settings that the template declares, with their defaults, in its order; empty for a
   * name that no template registered for the output has.
   */
  defaults: Record<string, unknown>;
}

/**
 * The compose template in use, over the key at `choice`, which chooses each output's
 * template (`templateValue`); `null` when the build type shown is made from no output that
 * a compose template assembles.
 */
export function templateInUse(state: SettingsState, choice: Path): TemplateInUse | null {
  const output = shownOutput(state);
  if (output === null) return null;
  const name = templateValue(state, choice, output);
  if (typeof name !== 'string') return null;
  const registered = templatesByOutput(state).get(output)?.find((each) => each.name === name);
  return { output, name, defaults: registered?.config ?? {} };
}

/**
 * The path of the settings of `template` under the key at `path`, which holds every
 * template's settings: the output's name, then the template's.
 */
export function templateSettingsPath(path: Path, template: TemplateInUse): string[] {
  return [...path, template.output, template.name];
}

/** The settings of `template` that the front matter writes, under the key at `path`. */
export function writtenTemplateSettings(state: SettingsState, path: Path, template: TemplateInUse): string[] {
  const written = valueAt(state.frontMatter, templateSettingsPath(path, template));
  return isObject(written) ? Object.keys(written).filter((key) => isWritten(written[key])) : [];
}

/**
 * The settings of `template`: those that it declares, in its order, then those that the
 * front matter writes for it and it does not declare, which it is given as well.
 */
export function templateSettingKeys(state: SettingsState, path: Path, template: TemplateInUse): string[] {
  const declared = Object.keys(template.defaults);
  return [...declared, ...writtenTemplateSettings(state, path, template).filter((key) => !declared.includes(key))];
}

/**
 * The settings of `keys` that the tab shows before the link "More…": the first
 * `TEMPLATE_SETTINGS_FIRST`, and those of `pinned`, the settings that the front matter
 * writes, or wrote since the rows were first shown.
 */
export function firstTemplateSettings(keys: readonly string[], pinned: readonly string[]): string[] {
  return keys.filter((key, at) => at < TEMPLATE_SETTINGS_FIRST || pinned.includes(key));
}

// --- a group of checkboxes ----------------------------------------------------------------

/** Whether the checkbox of `entry`, an entry of the map at `path`, is checked: the entry is not `false`. */
export function boxChecked(state: SettingsState, path: Path, entry: string): boolean {
  const value = shownValue(state, [...path, entry]);
  return value !== false && value !== undefined && value !== null;
}

/**
 * The write of the checkbox of `entry`: unchecked writes `false` for the entry; checked
 * removes the `false` that the front matter writes, or, where a lower source removed the
 * entry, writes the entry's default. `null` when the box is checked and nothing can be
 * written: the entry has no default to write back.
 */
export function boxEdit(state: SettingsState, path: Path, entry: string, checked: boolean): Edit | null {
  const entryAt = [...path, entry];
  if (!checked) return changed(entryAt, false);
  if (valueAt(state.frontMatter, entryAt) === false) return reverted(entryAt);
  const fallback = defaultAt(state.schema, entryAt);
  return isWritten(fallback) && fallback !== false ? changed(entryAt, fallback) : null;
}

/** The entries of `entries` that the front matter writes in the map at `path`. */
export function boxesWritten(state: SettingsState, path: Path, entries: readonly string[]): string[] {
  return entries.filter((entry) => isWritten(valueAt(state.frontMatter, [...path, entry])));
}

/**
 * Where the value of a group of checkboxes comes from: the front matter, else the origin of
 * the map, else that of the first entry a source writes (the map of a feature's entry has no
 * origin of its own: each of its keys has one).
 */
export function boxesSource(state: SettingsState, path: Path, entries: readonly string[]): Source {
  const origin = inEffectAt(state, path)?.origin
    ?? entries.map((entry) => inEffectAt(state, [...path, entry])?.origin).find((each) => each != null)
    ?? null;
  return sourceOf(boxesWritten(state, path, entries).length > 0, origin, !preparationFailed(state));
}

/**
 * Whether checking the box of `entry` writes a default of the last successful build: it
 * writes the entry's default (`boxEdit`) while the schema is not that of the document as it
 * is. The box is disabled then.
 */
export function boxEditOutdated(state: SettingsState, path: Path, entry: string): boolean {
  const entryAt = [...path, entry];
  return !state.schemaCurrent && valueAt(state.frontMatter, entryAt) !== false && boxEdit(state, path, entry, true) !== null;
}

// --- the advanced list --------------------------------------------------------------------

/** The features that the schema declares (its `properties`): those of the document's packages, in its order. */
export function declaredFeatures(state: SettingsState): string[] {
  const features = schemaAt(state.schema, FEATURES_PATH);
  return features !== null && isObject(features.properties) ? Object.keys(features.properties) : [];
}

/** The features whose entries the front matter writes, in its order. */
function writtenFeatures(state: SettingsState): string[] {
  const features = valueAt(state.frontMatter, FEATURES_PATH);
  return isObject(features) ? Object.keys(features) : [];
}

/**
 * The features of the advanced list: those that the schema declares, in its order, then
 * those that the front matter writes and the schema lacks, in the front matter's order.
 */
export function featuresOf(state: SettingsState): string[] {
  const declared = declaredFeatures(state);
  return [...declared, ...writtenFeatures(state).filter((name) => !declared.includes(name))];
}

/**
 * Whether the row of the feature `name` carries the mark that no package in use has it: the
 * schema, which is that of the document as it is, does not declare it. The mark is left out
 * while the schema is that of the last successful build.
 */
export function unknownFeature(state: SettingsState, name: string): boolean {
  return state.schemaCurrent && !declaredFeatures(state).includes(name);
}

/** The settings keys that the schema gives the feature `name`, in its order, without its switch. */
function declaredKeys(state: SettingsState, name: string): string[] {
  if (!declaredFeatures(state).includes(name)) return [];
  const entry = schemaAt(state.schema, entryPath(name));
  if (entry === null) return [];
  const branches = keyedAlternatives(entry);
  const own = branches === null ? entry : resolved(state.schema, branches[0]);
  return own !== null && isObject(own.properties) ? Object.keys(own.properties).filter((key) => key !== SWITCH_KEY) : [];
}

/**
 * The settings keys of the feature `name`, without its switch: those of its schema, in its
 * order, then those that the front matter writes and the schema lacks.
 */
export function featureKeys(state: SettingsState, name: string): string[] {
  const declared = declaredKeys(state, name);
  return [...declared, ...writtenKeys(state, name).filter((key) => !declared.includes(key))];
}

/**
 * The feature that holds the settings of the option chosen at `path`, the feature whose name
 * is `prefix` followed by the value shown there, when that value is a name and the feature
 * has settings (`featureKeys`); else `null`.
 */
export function settingsFeatureOf(state: SettingsState, path: Path, prefix: string): string | null {
  const value = shownValue(state, path);
  if (typeof value !== 'string') return null;
  const name = `${prefix}${value}`;
  return featureKeys(state, name).length > 0 ? name : null;
}

/**
 * Whether the row of the key `key` of the feature `name` carries the mark that no package in
 * use has it: the feature is declared and its schema, which is that of the document as it
 * is, lacks the key. (A feature that the schema lacks carries the mark itself.)
 */
export function unknownKey(state: SettingsState, name: string, key: string): boolean {
  return state.schemaCurrent && declaredFeatures(state).includes(name) && !declaredKeys(state, name).includes(key);
}

/** The settings keys that the front matter writes in the feature `name`'s entry. */
export function writtenKeys(state: SettingsState, name: string): string[] {
  const entry = valueAt(state.frontMatter, entryPath(name));
  return isObject(entry) ? Object.keys(entry).filter((key) => key !== SWITCH_KEY && isWritten(entry[key])) : [];
}

/** A feature of the advanced list, and whether it is of the first group. */
export interface Listed {
  name: string;
  first: boolean;
}

/**
 * The advanced list: first the features that have a setting in the front matter or a
 * curated setting (`curated`, by feature, as the environment has them), and those that the
 * front matter writes and the schema lacks, then every other feature; each group in the
 * order of `featuresOf`.
 */
export function advancedList(state: SettingsState, curated: Readonly<Record<string, readonly unknown[]>>): Listed[] {
  const features = featuresOf(state);
  const declared = declaredFeatures(state);
  const isFirst = (name: string) => (curated[name]?.length ?? 0) > 0 || writtenKeys(state, name).length > 0 || !declared.includes(name);
  return [
    ...features.filter(isFirst).map((name) => ({ name, first: true })),
    ...features.filter((name) => !isFirst(name)).map((name) => ({ name, first: false })),
  ];
}

/**
 * The advanced list kept in the order fixed when the panel opened (`before`): the features
 * listed then keep their place and group, those that are no longer listed leave, and those
 * listed since come after them, in the order of `now`.
 */
export function keptList(before: readonly Listed[], now: readonly Listed[]): Listed[] {
  const names = new Set(now.map((each) => each.name));
  const kept = before.filter((each) => names.has(each.name));
  const known = new Set(kept.map((each) => each.name));
  return [...kept, ...now.filter((each) => !known.has(each.name))];
}

/** Whether a row matches the search `query`: its label or its key holds it, whatever the case. */
export function matches(query: string, ...texts: readonly string[]): boolean {
  const wanted = query.trim().toLowerCase();
  return wanted === '' || texts.some((text) => text.toLowerCase().includes(wanted));
}

/**
 * Whether a row of the advanced list matches each of `queries`, the texts of the tab's
 * search field and of the list's filter (`matches`): each may be held by another text of
 * the row.
 */
export function matchesEach(queries: readonly string[], ...texts: readonly string[]): boolean {
  return queries.every((query) => matches(query, ...texts));
}
