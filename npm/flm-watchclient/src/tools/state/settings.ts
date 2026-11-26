// What the rows of the Metadata and Settings tabs make of the settings state: the control
// that follows from the shape of a key's schema, the icon that tells where a row's value
// comes from and the lines of its tooltip, and the change of the front matter that a
// control sends. Nothing here names a feature or a key, and nothing touches the page, so
// that the tests can run each case.

import type { Change, Copied, FrontMatterWrite, Origin, SourceWrite } from '../../shared/settings';
import { isObject } from '../../shared/protocol';
import type { MetadataRow } from './description';

type Schema = Record<string, unknown>;

/** A path from the root of the front matter: its keys. */
export type Path = readonly string[];

/** The prefix of a reference to an entry of the schema's top-level `$defs`. */
const DEFS = '#/$defs/';

/**
 * The schema that `schema` refers to, when it is a reference into the top-level `$defs` of
 * `root`, with the reference's own keywords (its `description`, for one) beside the
 * target's; `schema` itself otherwise. The name after `#/$defs/` is a JSON Pointer's
 * segment, so `~1` stands for `/` and `~0` for `~`. A reference that leads nowhere gives
 * `null`; references are followed a few times at most, so that a loop ends.
 */
export function resolved(root: Schema | null, schema: unknown): Schema | null {
  let current: unknown = schema;
  for (let depth = 0; depth < 16; depth += 1) {
    if (!isObject(current)) return null;
    const ref = current.$ref;
    if (typeof ref !== 'string') return current;
    if (!ref.startsWith(DEFS) || root === null || !isObject(root.$defs)) return null;
    const name = ref.slice(DEFS.length).replace(/~1/g, '/').replace(/~0/g, '~');
    const target = root.$defs[name];
    if (!isObject(target)) return null;
    const { $ref: _, ...rest } = current;
    current = { ...target, ...rest };
  }
  return null;
}

/** `value` as JSON text whose maps list their keys in order, so that equal values give equal texts. */
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (isObject(value)) {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value) ?? 'undefined';
}

/** Whether two values of the front matter are the same data, whatever the order of a map's keys. */
export function sameValue(left: unknown, right: unknown): boolean {
  return canonical(left) === canonical(right);
}

/** An option of a choice: its label, the value it writes, and whether it is grayed out. */
export interface Option {
  label: string;
  value: unknown;
  disabled?: boolean;
}

/**
 * The control of a key's row in the advanced list, from the shape of its schema: a switch
 * for a boolean, a choice for an enumeration, a number field for a number (whole for an
 * integer), a text field for a text, the chips of a list of texts; and *Edit as YAML* for
 * any other shape.
 */
export type Control =
  | { kind: 'switch' }
  | { kind: 'choice'; options: Option[] }
  | { kind: 'number'; integer: boolean }
  | { kind: 'text' }
  | { kind: 'texts' }
  | { kind: 'yaml' };

/** The values of `schema` when it is an enumeration (`enum`, `const`, or alternatives of these). */
function enumerated(root: Schema | null, schema: Schema): unknown[] | null {
  if (Array.isArray(schema.enum)) return schema.enum;
  if ('const' in schema) return [schema.const];
  const branches = alternatives(schema);
  if (branches === null) return null;
  const values: unknown[] = [];
  for (const branch of branches) {
    const each = resolved(root, branch);
    const inner = each === null ? null : enumerated(root, each);
    if (inner === null) return null;
    values.push(...inner);
  }
  return values;
}

/** The alternatives of `anyOf` or `oneOf`, without the one that only admits `null`. */
function alternatives(schema: Schema): unknown[] | null {
  const branches = Array.isArray(schema.anyOf) ? schema.anyOf : Array.isArray(schema.oneOf) ? schema.oneOf : null;
  return branches === null ? null : branches.filter((branch) => !(isObject(branch) && branch.type === 'null' && !('const' in branch)));
}

/** The one type that `schema` names, leaving out `null`; `null` when it names none or several. */
function typeOf(schema: Schema): string | null {
  const types = Array.isArray(schema.type) ? schema.type.filter((each) => each !== 'null') : [schema.type];
  return types.length === 1 && typeof types[0] === 'string' ? types[0] : null;
}

function isScalar(value: unknown): boolean {
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';
}

/**
 * The control that follows from `schema`, a key's schema in the schema `root`. A schema
 * that admits `null` beside another type (`admitsNull`) gets the control of that type.
 */
export function controlOf(root: Schema | null, schema: unknown): Control {
  const own = resolved(root, schema);
  if (own === null) return { kind: 'yaml' };
  const values = enumerated(root, own)?.filter((value) => value !== null) ?? null;
  if (values !== null) {
    if (values.length === 0 || !values.every(isScalar)) return { kind: 'yaml' };
    if (values.length === 2 && values.includes(true) && values.includes(false)) return { kind: 'switch' };
    return { kind: 'choice', options: values.map((value) => ({ label: String(value), value })) };
  }
  const branches = alternatives(own);
  if (branches !== null) return branches.length === 1 ? controlOf(root, branches[0]) : { kind: 'yaml' };
  switch (typeOf(own)) {
    case 'boolean':
      return { kind: 'switch' };
    case 'integer':
      return { kind: 'number', integer: true };
    case 'number':
      return { kind: 'number', integer: false };
    case 'string':
      return { kind: 'text' };
    case 'array': {
      const items = resolved(root, own.items);
      return items !== null && controlOf(root, items).kind === 'text' ? { kind: 'texts' } : { kind: 'yaml' };
    }
    default:
      return { kind: 'yaml' };
  }
}

/**
 * Whether `schema`, a key's schema in the schema `root`, admits `null`: its `type` names
 * `null`, its enumeration holds `null`, or one of its alternatives admits it.
 */
export function admitsNull(root: Schema | null, schema: unknown): boolean {
  const own = resolved(root, schema);
  if (own === null) return false;
  if (own.type === 'null' || (Array.isArray(own.type) && own.type.includes('null'))) return true;
  if (Array.isArray(own.enum) && own.enum.includes(null)) return true;
  if ('const' in own) return own.const === null;
  const branches = Array.isArray(own.anyOf) ? own.anyOf : Array.isArray(own.oneOf) ? own.oneOf : [];
  return branches.some((branch) => admitsNull(root, branch));
}

/**
 * Where a row's value comes from, which decides the icon at the right end of the row:
 *
 * - `written`: the front matter writes the key; the revert arrow is active and removes it;
 *   before it, the package icon, when a package also writes the value (`packageWrites`);
 * - `default`: the value is the default (of the schema or of the program); the revert arrow
 *   is greyed out;
 * - `package`: a package gives the value; a package icon, and a change writes the key;
 * - `decided`: a source that ranks above the front matter (the workflow of the build type
 *   shown, the command line, the preview's own settings) decides the value at the moment;
 *   the row says so beside its control, which still writes the front matter (its value
 *   applies under another build type, or without that option), and the revert arrow is
 *   active when the front matter writes the key (`written`);
 * - `outdated`: after a preparation that failed, the value is not the front matter's, and
 *   is the one of the last successful build; one icon in place of the others, whose
 *   tooltip says so.
 */
export type Source =
  | { kind: 'written' }
  | { kind: 'default' }
  | { kind: 'package'; line: string }
  | { kind: 'decided'; line: string; written: boolean }
  | { kind: 'outdated' };

/** The words of the icon of a value of the last successful build. */
export const OUTDATED_LINE = 'The value of the last successful build: the last build failed';

/**
 * The source of a row's value: `written` tells whether the front matter writes the key,
 * `origin` is the origin of the value in effect, or `null` when no source writes it;
 * `current` is `false` after a preparation that failed, when a value the front matter does
 * not write is that of the last successful build.
 */
export function sourceOf(written: boolean, origin: Origin | null, current = true): Source {
  if (!written && !current) return { kind: 'outdated' };
  if (origin !== null && (origin.source === 'workflow' || origin.source === 'command_line')) {
    return { kind: 'decided', line: originLine(origin), written };
  }
  if (written) return { kind: 'written' };
  if (origin !== null && origin.source === 'package') return { kind: 'package', line: originLine(origin) };
  return { kind: 'default' };
}

/** `text` with each name the program writes between backquotes written between ‘…’. */
export function quoted(text: string): string {
  return text.replace(/`([^`]*)`/g, '‘$1’');
}

/**
 * The few words that reveal an origin, in the tooltip of the package icon and beside the
 * control of a row that a higher source decides: "From the package ‘lecturenotes’",
 * "Decided by the build type ‘html’", "Decided by the option ‘-O toc.depth=2’". They are
 * made from the program's text for the origin, whose names between backquotes are written
 * between ‘…’.
 */
export function originLine(origin: Origin): string {
  switch (origin.source) {
    case 'package':
    case 'front_matter':
    case 'defaults':
      return `From ${sourceName(origin)}`;
    default:
      return `Decided by ${sourceName(origin)}`;
  }
}

/**
 * The name of the source of `origin`, as the first line of a row's tooltip names it: "the
 * program's defaults", "the package ‘NAME’", "the front matter", "the build type ‘html’",
 * or the program's text for an option. `sourceLines` names a package by its use name alone.
 */
export function sourceName(origin: Origin): string {
  switch (origin.source) {
    case 'package':
      return `the package ‘${origin.extension ?? origin.package ?? ''}’`;
    case 'front_matter':
      return 'the front matter';
    case 'workflow':
      return origin.workflow !== null ? `the build type ‘${origin.workflow}’` : quoted(origin.text);
    default:
      return quoted(origin.text);
  }
}

/**
 * A value as a line of the tooltip shows it: a text as it is, any other value in YAML's
 * one-line form, `null` for none; cut with "…" as `preview` cuts.
 */
function valueLine(value: unknown): string {
  if (value === null || value === undefined) return 'null';
  if (typeof value !== 'string') return preview(value);
  return value.length > PREVIEW_LENGTH ? `${value.slice(0, PREVIEW_LENGTH - 1)}…` : value;
}

/**
 * The lines of the tooltip of a row's package icon: each source's value, one line per
 * source, lowest rank first ("in mybase: 0pt", "in the front matter: 12pt"), the one in
 * effect marked "(in effect)"; a package is named by its use name alone.
 */
export function sourceLines(writes: readonly SourceWrite[]): string[] {
  return writes.map((write) => {
    const name = write.origin.source === 'package' ? (write.origin.extension ?? write.origin.package ?? '') : sourceName(write.origin);
    return `in ${name}: ${valueLine(write.value)}${write.inEffect ? ' (in effect)' : ''}`;
  });
}

/** Whether the sources of a value, `writes`, list a write of a package. */
export function packageWrites(writes: readonly SourceWrite[]): boolean {
  return writes.some((write) => write.origin.source === 'package');
}

/** The names of the marks that a schema writes under `x-flm-marks`. */
export const MARKS_KEYWORD = 'x-flm-marks';

/** The words of a mark on the value itself, by the mark's name. */
const OWN_MARK_WORDS: Readonly<Record<string, string>> = {
  'merge-object-key-values': 'Its keys are combined across sources, key by key.',
  'remove-key-when-value-is-false': 'A key of it set to ‘false’ is removed.',
  'set-to-default-with-null': '‘null’ gives its default.',
  path: 'Its texts are paths of files.',
  markup: 'Its texts are written in FLM markup.',
};

/** The words of a mark on the object that holds the value, by the mark's name. */
const HOLDER_MARK_WORDS: Readonly<Record<string, string>> = {
  'merge-object-key-values': 'The object that holds it is combined across sources, key by key.',
  'remove-key-when-value-is-false': 'Set to ‘false’, it is removed from the object that holds it.',
};

/**
 * The lines in words of the marks of a value, `own`, and of the object that holds it,
 * `holder`, each a list of marks' names as a schema writes them: own marks first, in the
 * order given. A mark without words for its place gives no line.
 */
export function markLines(own: readonly string[], holder: readonly string[]): string[] {
  return [
    ...own.flatMap((name) => (Object.hasOwn(OWN_MARK_WORDS, name) ? [OWN_MARK_WORDS[name]!] : [])),
    ...holder.flatMap((name) => (Object.hasOwn(HOLDER_MARK_WORDS, name) ? [HOLDER_MARK_WORDS[name]!] : [])),
  ];
}

/**
 * The line of the status bar that names the values a write copied into the front matter:
 * each by its path from the feature's entry, or from the
 * root of the front matter outside the features, and its value; `null` when it copied none.
 */
export function copiedLine(copied: readonly Copied[], features: Path): string | null {
  if (copied.length === 0) return null;
  const named = copied.map(({ path, value }) => {
    const under = features.every((key, at) => path[at] === key) && path.length > features.length;
    return `‘${(under ? path.slice(features.length) : path).join('.')}: ${valueLine(value)}’`;
  });
  return `Copied into the front matter: ${named.join(', ')}`;
}

/**
 * Whether the front matter writes a key, given the value it holds at the key's path
 * (`undefined` when a key of the path is missing): a `null` counts as not written.
 */
export function isWritten(value: unknown): boolean {
  return value !== undefined && value !== null;
}

/** One change of the front matter at a path, as the row's control asks for it. */
export interface Edit {
  path: string[];
  change: Change;
}

/**
 * The body of the request that makes `edits`, the edits of one control's change, in order,
 * against the front matter's `revision`.
 */
export function bodyOf(revision: string, edits: readonly Edit[]): FrontMatterWrite {
  return { revision, changes: edits.map((edit) => ({ path: [...edit.path], ...edit.change })) };
}

/** The edit of a control's change: the value set at the row's path. */
export function changed(path: readonly string[], value: unknown): Edit {
  return { path: [...path], change: { value } };
}

/** The edit of the revert arrow: the key at the row's path removed. */
export function reverted(path: readonly string[]): Edit {
  return { path: [...path], change: { remove: true } };
}

/** The edit of the YAML editor's *Save*: the value at the path set from YAML text. */
export function savedYaml(path: readonly string[], yaml: string): Edit {
  return { path: [...path], change: { yaml } };
}

/** The edit of a text field of *Metadata*: the text set, or the key removed when it is empty. */
export function textEdit(path: readonly string[], text: string): Edit {
  return text === '' ? reverted(path) : changed(path, text);
}

/**
 * The edit of a list of texts shown as chips, given the value written before (`before`):
 * the list set, or the key removed when it is empty; one text stays one text when the front
 * matter wrote one text, not a list.
 */
export function textsEdit(path: readonly string[], before: unknown, texts: readonly string[]): Edit {
  if (texts.length === 0) return reverted(path);
  if (texts.length === 1 && isScalar(before)) return changed(path, texts[0]);
  return changed(path, [...texts]);
}

/**
 * The control of a row of its own of *Metadata*, for the value the front matter writes at
 * its key: the row's own control; but a list of texts whose value holds anything other than
 * texts, as an author written as a map, is edited as YAML, since its chips would lose the
 * parts that are not texts.
 */
export function metadataControl(row: MetadataRow, value: unknown): MetadataRow['kind'] | 'yaml' {
  if (row.kind === 'text') return 'text';
  const entries = Array.isArray(value) ? value : value === undefined || value === null ? [] : [value];
  return entries.every(isScalar) ? 'texts' : 'yaml';
}

/** The texts of a value shown as chips: one text, or the texts of a list. */
export function textsOf(value: unknown): string[] {
  if (value === undefined || value === null) return [];
  if (Array.isArray(value)) return value.map((each) => (isScalar(each) ? String(each) : JSON.stringify(each)));
  return [isScalar(value) ? String(value) : JSON.stringify(value)];
}

/** How many characters of a value the row of *Edit as YAML* shows beside its button. */
const PREVIEW_LENGTH = 40;

/**
 * A value in YAML's one-line form, as the author would write it: a text without quotes
 * unless it would read as another value or holds a sign of the one-line form, then in
 * double quotes.
 */
export function oneLine(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(oneLine).join(', ')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.entries(value).map(([key, entry]) => `${oneLine(key)}: ${oneLine(entry)}`).join(', ')}}`;
  }
  if (typeof value !== 'string') return String(value);
  const plain = value !== '' && value.trim() === value
    && !/[,:\[\]{}#&*!|>'"%@`]|^[-?]/.test(value)
    && !/^(true|false|null|~|[-+]?(\.?\d.*|\.inf|\.nan))$/i.test(value);
  return plain ? value : JSON.stringify(value);
}

/**
 * The beginning of a value, on one line, as the row of *Edit as YAML* shows it beside its
 * button: its text in YAML's one-line form, cut with "…"; empty for no value.
 */
export function preview(value: unknown): string {
  if (value === undefined || value === null) return '';
  const line = oneLine(value).replace(/\s+/g, ' ');
  return line.length > PREVIEW_LENGTH ? `${line.slice(0, PREVIEW_LENGTH - 1)}…` : line;
}
