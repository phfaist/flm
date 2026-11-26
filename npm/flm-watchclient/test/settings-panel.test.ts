// The Settings tab: the rows of the description that the document's environment has, the
// order of the advanced list, the write of a template row, the settings of the template in
// use and their controls, a key given no value, the tooltip of a row's source, a row that a
// higher source decides, the writes of the switch of a feature, the packages offered, and
// what the tab shows and enables after a preparation that failed.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { FeatureInEffect, Origin, SettingsState } from '../src/shared/settings';
import { CURATED, TOPICS } from '../src/tools/state/description';
import { sourceLines } from '../src/tools/state/settings';
import {
  TEMPLATE_SETTINGS_FIRST, advancedList, automaticPackagesShown, boxEditOutdated, boxesSource, describedRows, featureKeys, featuresOf, firstTemplateSettings,
  givenEdit, keptList, keyControl, marksAt, matchesEach, nullableAt, outdatedAt, packagesInUse, packagesOffered, rowLocked, settingsFeatureOf, shownSwitch,
  shownValue, sourceAt, switchEdit, templateEdit, templateInUse, templateSettingKeys, templateSettingsPath, templateSource, templateValue,
  unknownFeature, unknownKey,
} from '../src/tools/state/settings-panel';

const defaults: Origin = { source: 'defaults', text: "the program's defaults", package: null, extension: null, workflow: null };
const notes: Origin = { source: 'package', text: 'the package `notes`', package: 'notes#notes', extension: 'notes', workflow: null };

/** The schema of a feature's entry, as the server gives it: `false`, `null`, or a reference to its `$defs` entry. */
function entry(name: string) {
  return { anyOf: [{ const: false }, { type: 'null' }, { $ref: `#/$defs/${name}` }] };
}

/** A schema of a front matter for the features `features`, each with the keys its `$defs` entry gives. */
function schemaOf(features: Record<string, Record<string, unknown>>) {
  const $defs: Record<string, unknown> = {};
  const properties: Record<string, unknown> = {};
  for (const [name, keys] of Object.entries(features)) {
    $defs[name] = { type: 'object', properties: { enabled: { type: 'boolean', default: true }, ...keys } };
    properties[name] = entry(name);
  }
  return {
    $defs,
    type: 'object',
    properties: {
      flm: {
        type: 'object',
        properties: {
          use: { type: 'array', items: { type: 'string' } },
          math: { anyOf: [{ type: 'string' }, { const: false }] },
          default_profile: { type: 'string' },
          parsing: { type: 'object', properties: { dollar_math: { type: 'boolean', default: false } } },
          // A feature that the schema does not describe is accepted, in the forms of a block.
          features: {
            type: ['object', 'null'],
            properties,
            additionalProperties: { anyOf: [{ const: false }, { type: 'null' }, { type: 'object', properties: { enabled: { type: 'boolean' } } }] },
          },
        },
      },
    },
  };
}

function stateOf(
  schema: Record<string, unknown>,
  frontMatter: Record<string, unknown> | null,
  features: Record<string, FeatureInEffect> = {},
  choices: Record<string, unknown> | null = null,
): SettingsState {
  return {
    revision: '0123456789abcdef', unreadable: null, frontMatter, schema, features, flm: {}, choices,
    preparationFailed: null, schemaCurrent: true,
  };
}

/** `state` after a preparation that failed, with a schema current or not. */
function failed(state: SettingsState, schemaCurrent = true): SettingsState {
  return { ...state, preparationFailed: 'paper.flm: The plan cannot be built', schemaCurrent };
}

const STANDARD = {
  compose: { template: { anyOf: [{ type: 'string' }, { type: 'object', additionalProperties: { type: 'string' } }] } },
  headings: { numbering: { type: 'string' } },
  toc: { depth: { type: 'integer', default: 3 } },
  citations: { mark: { type: 'string' }, style: { type: 'object' } },
  floats: { types: { type: 'object', additionalProperties: { type: 'object', properties: { numbering: { type: 'string' }, number_format: { type: 'object' } } } } },
  theorems: {},
};

test('a row of the description is left out for a document without its feature', () => {
  const state = stateOf(schemaOf(STANDARD), null);
  const rowsOf = (title: string) => describedRows(state, TOPICS.find((topic) => topic.title === title)!.rows);
  // Without the feature `citations-standard`, the checkboxes of the citation keys and the
  // files are left out; the citation marks stay, and the links keep `citations` alone.
  const bibliography = rowsOf('Bibliography');
  assert.deepEqual(bibliography.map((each) => each.row.kind), ['choice', 'links']);
  assert.deepEqual(bibliography[1]!.features, ['citations']);
  assert.deepEqual(rowsOf('Headings').map((each) => each.path.join('.')), ['flm.features.headings.numbering', 'flm.features.toc.depth']);
  // A curated setting of a feature that the document lacks is left out too.
  assert.deepEqual(describedRows(state, CURATED.graphics ?? []), []);
  const without = stateOf(schemaOf({ citations: STANDARD.citations }), null);
  assert.deepEqual(describedRows(without, TOPICS.find((topic) => topic.title === 'Headings')!.rows), []);
});

test('the advanced list gives first the features with a curated setting or a setting in the front matter, and keeps its order', () => {
  const schema = schemaOf({ theorems: { kinds: { type: 'object' } }, citations: STANDARD.citations, floats: STANDARD.floats, toc: STANDARD.toc, href: {} });
  const frontMatter = { flm: { features: { toc: { depth: 2 }, theorems: { enabled: false }, citations: false } } };
  const state = stateOf(schema, frontMatter);
  const curated = { floats: describedRows(state, CURATED.floats ?? []) };
  const list = advancedList(state, curated);
  // `floats` has curated settings and `toc` a setting in the front matter; `enabled` and the
  // shorthand `false` are not settings.
  assert.deepEqual(list, [
    { name: 'floats', first: true },
    { name: 'toc', first: true },
    { name: 'theorems', first: false },
    { name: 'citations', first: false },
    { name: 'href', first: false },
  ]);
  // A setting written since the panel opened leaves the order as it was; a feature listed
  // since comes last, and one no longer listed leaves.
  const later = stateOf(schemaOf({ theorems: { kinds: { type: 'object' } }, toc: STANDARD.toc, floats: STANDARD.floats, href: {}, refs: {} }), {
    flm: { features: { toc: { depth: 2 }, theorems: { kinds: {} } } },
  });
  assert.deepEqual(keptList(list, advancedList(later, curated)).map((each) => each.name), ['floats', 'toc', 'theorems', 'href', 'refs']);
});

test('a change of a compose template\'s name sends that change alone', () => {
  const path = ['flm', 'features', 'compose', 'template'];
  // The name of the output changed, whatever the front matter or a package writes for the
  // others: the server writes out the names that they keep, and its answer lists them.
  assert.deepEqual(templateEdit(path, 'latex-rendered', 'article'), { path: [...path, 'latex-rendered'], change: { value: 'article' } });
});

test('a template row shows the default template for an output that the map in effect does not list', () => {
  const path = ['flm', 'features', 'compose', 'template'];
  const schema = schemaOf({ ...STANDARD, compose: { template: { ...STANDARD.compose.template, default: 'simple' } } });
  const front: Origin = { ...defaults, source: 'front_matter', text: 'paper.flm:5:7' };
  // The front matter names a template for `text` alone: its map is in effect whole, over
  // the map of a package, and `html` takes the default template.
  const state = stateOf(
    schema,
    { flm: { features: { compose: { template: { text: 'none' } } } } },
    { compose: { switchedOff: null, keys: { template: { value: { text: 'none' }, origin: front, entries: null } } } },
  );
  assert.equal(templateValue(state, path, 'text'), 'none');
  assert.equal(templateValue(state, path, 'html'), 'simple');
  assert.deepEqual(templateSource(state, path, 'html'), { kind: 'default' });
});

test('the settings of the template in use are those of the template chosen for the output that the build type shown is made from', () => {
  const schema = schemaOf({
    ...STANDARD,
    compose: { template: { ...STANDARD.compose.template, default: 'simple' }, template_config: { type: 'object', additionalProperties: true } },
  });
  const choices = {
    templates: {
      html: [{ name: 'simple', config: {} }, { name: 'notes', config: { navigation_heading_level: 1, font_family: 'serif', links: [] } }],
      'latex-rendered': [{ name: 'simple', config: { documentclass: 'article' } }],
    },
    outputs: [{ name: 'pdf-latex', made_from: ['latex-rendered'], composed_as: null }, { name: 'latex-rendered', made_from: [], composed_as: 'latex-rendered' }],
  };
  const rowsOf = (state: SettingsState) => describedRows(state, TOPICS.find((topic) => topic.title === 'Current template settings')!.rows);
  // The build type shown is made from `latex-rendered`, whose template is the default one.
  const latex = stateOf(schema, { flm: { features: { compose: { template: { html: 'notes' } } } } }, {}, choices);
  const [row] = rowsOf(latex);
  assert.deepEqual(row!.path, ['flm', 'features', 'compose', 'template_config']);
  const simple = templateInUse(latex, row!.template!);
  assert.deepEqual(simple, { output: 'latex-rendered', name: 'simple', defaults: { documentclass: 'article' } });
  // A setting is written under the output's name, then the template's.
  assert.deepEqual(templateSettingsPath(row!.path, simple!), ['flm', 'features', 'compose', 'template_config', 'latex-rendered', 'simple']);
  // The settings are those that the template declares, in its order, then those that the
  // front matter writes for it and it does not declare.
  const html = stateOf(
    schema,
    { flm: { features: { compose: { template: 'notes', template_config: { html: { notes: { accent: 'red', font_family: 'sans-serif' }, simple: { other: 1 } } } } } } },
    {},
    { ...choices, outputs: [{ name: 'html', made_from: [], composed_as: 'html' }] },
  );
  const notesTemplate = templateInUse(html, row!.template!)!;
  assert.equal(notesTemplate.name, 'notes');
  assert.deepEqual(templateSettingKeys(html, row!.path, notesTemplate), ['navigation_heading_level', 'font_family', 'links', 'accent']);
  // No template is in use while the build type shown is made from no output that a template assembles.
  const none = stateOf(schema, null, {}, { ...choices, outputs: [{ name: 'bibtex', made_from: [], composed_as: null }] });
  assert.equal(templateInUse(none, row!.template!), null);
});

/** A schema whose compose template `notes` of the output `html` declares `settings`, its settings combined key by key. */
function templateSchema(settings: Record<string, unknown>) {
  const template = { type: 'object', default: {}, 'x-flm-marks': ['merge-object-key-values'], properties: settings };
  const config = { type: 'object', 'x-flm-marks': ['merge-object-key-values'], properties: { notes: template } };
  return schemaOf({ ...STANDARD, compose: { ...STANDARD.compose, template_config: { type: 'object', default: {}, properties: { html: config } } } });
}

const NOTES = ['flm', 'features', 'compose', 'template_config', 'html', 'notes'];

test('a setting of a compose template declared as a whole number, a choice among texts or a list of texts gets the control of its schema, with an empty default', () => {
  // Defaults that do not tell the type: `0`, an empty text, an empty list.
  const state = stateOf(templateSchema({
    level: { type: 'integer', default: 0 },
    theme: { type: 'string', enum: ['light', 'dark'], default: '' },
    stylesheets: { type: 'array', items: { type: 'string' }, default: [] },
  }), null);
  assert.deepEqual(keyControl(state, [...NOTES, 'level']), { kind: 'number', integer: true });
  assert.deepEqual(keyControl(state, [...NOTES, 'theme']), { kind: 'choice', options: [{ label: 'light', value: 'light' }, { label: 'dark', value: 'dark' }] });
  assert.deepEqual(keyControl(state, [...NOTES, 'stylesheets']), { kind: 'texts' });
});

test('a setting declared as a text or null can be given no value, and the change writes null', () => {
  const schema = templateSchema({ subtitle: { type: ['string', 'null'], default: null }, title: { type: 'string', default: '' } });
  const subtitle = [...NOTES, 'subtitle'];
  const state = stateOf(schema, null);
  assert.ok(nullableAt(state, subtitle) && !nullableAt(state, [...NOTES, 'title']));
  assert.deepEqual(keyControl(state, subtitle), { kind: 'text' });
  // Unchecked, the box writes `null`; checked, the control gives the value.
  assert.deepEqual(givenEdit(subtitle, false), { path: subtitle, change: { value: null } });
  assert.equal(givenEdit(subtitle, true), null);
  // The `null` that the front matter writes is its value, which the revert arrow removes.
  const written = stateOf(schema, { flm: { features: { compose: { template_config: { html: { notes: { subtitle: null } } } } } } });
  assert.equal(shownValue(written, subtitle, true), null);
  assert.deepEqual(sourceAt(written, subtitle, true), { kind: 'written' });
});

test('the tooltip of a row lists each source\'s value, the one in effect marked, then the marks of the object that holds it', () => {
  const mybase: Origin = { ...notes, text: 'the package `mybase`', package: 'mybase#mybase', extension: 'mybase' };
  const mytemplates: Origin = { ...notes, text: 'the package `mytemplates`', package: 'mytemplates#mytemplates', extension: 'mytemplates' };
  const front: Origin = { ...defaults, source: 'front_matter', text: 'paper.flm:5:7' };
  assert.deepEqual(sourceLines([
    { value: '0pt', origin: mybase, inEffect: false },
    { value: '1em', origin: mytemplates, inEffect: false },
    { value: '12pt', origin: front, inEffect: true },
  ]), ['in mybase: 0pt', 'in mytemplates: 1em', 'in the front matter: 12pt (in effect)']);
  const state = stateOf(templateSchema({ margin: { type: 'string', default: '0pt' } }), null);
  assert.deepEqual(marksAt(state, [...NOTES, 'margin']), ['The object that holds it is combined across sources, key by key.']);
});

test('a row that the command line decides is not locked and tells its source', () => {
  const depth = ['flm', 'features', 'toc', 'depth'];
  const option: Origin = { ...defaults, source: 'command_line', text: 'the option `-O toc.depth=2`' };
  const keys = { depth: { value: 2, origin: option, entries: null } };
  const state = stateOf(schemaOf(STANDARD), { flm: { features: { toc: { depth: 4 } } } }, { toc: { switchedOff: null, keys } });
  assert.deepEqual(sourceAt(state, depth), { kind: 'decided', line: 'Decided by the option ‘-O toc.depth=2’', written: true });
  assert.ok(!rowLocked(state, depth));
  // The row shows the front matter's value, which its control changes.
  assert.equal(shownValue(state, depth), 4);
});

test('the settings of a template beyond the first ones are shown when the front matter writes them', () => {
  const keys = Array.from({ length: TEMPLATE_SETTINGS_FIRST + 3 }, (_, at) => `key${at}`);
  const last = keys[keys.length - 1]!;
  assert.deepEqual(firstTemplateSettings(keys, []), keys.slice(0, TEMPLATE_SETTINGS_FIRST));
  assert.deepEqual(firstTemplateSettings(keys, [last, 'key0']), [...keys.slice(0, TEMPLATE_SETTINGS_FIRST), last]);
});

test('a row of the advanced list matches the search field and the filter of the list together', () => {
  assert.ok(matchesEach(['', ''], 'depth'));
  assert.ok(matchesEach(['toc', 'DEPTH'], 'depth', 'flm.features.toc.depth'));
  assert.ok(!matchesEach(['toc', 'style'], 'depth', 'flm.features.toc.depth'));
});

test('the switch of a feature writes `enabled: false`, and removes it, the shorthand, or writes `enabled: true` to switch it on', () => {
  const schema = schemaOf(STANDARD);
  const switchPath = ['flm', 'features', 'citations', 'enabled'];
  const off = (origin: Origin): Record<string, FeatureInEffect> => ({ citations: { switchedOff: origin, keys: {} } });
  // Over an entry with settings: off writes the key beside them, on removes it.
  const front: Origin = { ...defaults, source: 'front_matter', text: 'paper.flm:5:7' };
  const settings = stateOf(schema, { flm: { features: { citations: { mark: 'superscript' } } } });
  assert.deepEqual(switchEdit(settings, 'citations', false), { path: switchPath, change: { value: false } });
  const settingsOff = stateOf(schema, { flm: { features: { citations: { enabled: false, mark: 'superscript' } } } }, off(front));
  assert.deepEqual(switchEdit(settingsOff, 'citations', true), { path: switchPath, change: { remove: true } });
  // Over the shorthand `citations: false`: on removes the entry.
  const shorthand = stateOf(schema, { flm: { features: { citations: false } } }, off(front));
  assert.deepEqual(switchEdit(shorthand, 'citations', true), { path: ['flm', 'features', 'citations'], change: { remove: true } });
  // Over a feature that a package switched off: on writes `enabled: true`.
  const byPackage = stateOf(schema, { flm: { features: { citations: { mark: 'superscript' } } } }, off(notes));
  assert.deepEqual(switchEdit(byPackage, 'citations', true), { path: switchPath, change: { value: true } });
});

test('the switch of a feature shows what the front matter writes, else the switch in effect, unless a higher source fixes it', () => {
  const schema = schemaOf(STANDARD);
  const front: Origin = { ...defaults, source: 'front_matter', text: 'paper.flm:5:7' };
  const workflow: Origin = { ...defaults, source: 'workflow', text: 'the build type `html`', workflow: 'html' };
  const off = (origin: Origin): Record<string, FeatureInEffect> => ({ toc: { switchedOff: origin, keys: {} } });
  const on = (frontMatter: Record<string, unknown>, features: Record<string, FeatureInEffect> = {}) =>
    shownSwitch(failed(stateOf(schema, { flm: { features: frontMatter } }, features)), 'toc');
  // The forms that the reader of a block reads, after a failed preparation whose state in
  // effect is the last successful build's.
  assert.deepEqual(on({ toc: false }), { on: false, source: null });
  assert.deepEqual(on({ toc: { enabled: false, depth: 2 } }), { on: false, source: null });
  assert.deepEqual(on({ toc: null }, off(front)), { on: true, source: null });
  assert.deepEqual(on({ toc: { depth: 2 } }, off(front)), { on: true, source: null });
  assert.deepEqual(on({ toc: { enabled: true } }, off(front)), { on: true, source: null });
  // Not written: the switch in effect, of the last successful build.
  assert.deepEqual(on({}, off(notes)), { on: false, source: { kind: 'outdated' } });
  assert.deepEqual(on({ toc: { enabled: 'no' } }), { on: true, source: { kind: 'outdated' } });
  // Decided by a higher source: the front matter's switch, with the source that decides.
  assert.deepEqual(on({ toc: { enabled: true } }, off(workflow)), {
    on: true, source: { kind: 'decided', line: 'Decided by the build type ‘html’', written: true },
  });
  // After a preparation that succeeded, a switch not written carries no mark.
  assert.deepEqual(shownSwitch(stateOf(schema, {}, off(notes)), 'toc'), { on: false, source: null });
});

test('the math processor chosen leads to the feature of its settings when that feature has some', () => {
  const schema = schemaOf({ 'math-mathjax': { mode: { type: 'string' } }, 'math-bare': {} });
  const at = (math: unknown) => settingsFeatureOf(stateOf(schema, { flm: { math } }), ['flm', 'math'], 'math-');
  assert.equal(at('mathjax'), 'math-mathjax');
  assert.equal(at('bare'), null);
  assert.equal(at(false), null);
});

test('a group of checkboxes over a feature\'s entry takes the origin of the key that a source writes', () => {
  const schema = schemaOf({ blocks: { auto_unicode_quotes: { type: 'boolean', default: true }, ligature_unicode_dashes: { type: 'boolean', default: true } } });
  const keys = { ligature_unicode_dashes: { value: false, origin: notes, entries: null } };
  const state = stateOf(schema, null, { blocks: { switchedOff: null, keys } });
  assert.equal(boxesSource(state, ['flm', 'features', 'blocks'], ['auto_unicode_quotes', 'ligature_unicode_dashes']).kind, 'package');
});

test('the advanced list holds every feature and key that the front matter writes, and marks those that no package in use has', () => {
  const schema = schemaOf({ toc: STANDARD.toc, headings: STANDARD.headings });
  const frontMatter = { flm: { features: { nosuch: { a: 1 }, toc: { depth: 2, deep: true } } } };
  const state = failed(stateOf(schema, frontMatter));
  assert.deepEqual(featuresOf(state), ['toc', 'headings', 'nosuch']);
  assert.deepEqual(featureKeys(state, 'toc'), ['depth', 'deep']);
  assert.deepEqual(featureKeys(state, 'nosuch'), ['a']);
  assert.ok(unknownFeature(state, 'nosuch') && !unknownFeature(state, 'toc'));
  assert.ok(unknownKey(state, 'toc', 'deep') && !unknownKey(state, 'toc', 'depth'));
  // A feature that the schema lacks is of the first group.
  assert.deepEqual(advancedList(state, {}).map((each) => each.name), ['toc', 'nosuch', 'headings']);
  // While the schema is that of the last successful build, the lists hold them unmarked.
  const stale = failed(stateOf(schema, frontMatter), false);
  assert.deepEqual(featuresOf(stale), ['toc', 'headings', 'nosuch']);
  assert.ok(!unknownFeature(stale, 'nosuch') && !unknownKey(stale, 'toc', 'deep'));
});

test('a row of links keeps the features that the schema declares, whatever the schema accepts', () => {
  const state = stateOf(schemaOf({ citations: STANDARD.citations }), null);
  const bibliography = describedRows(state, TOPICS.find((topic) => topic.title === 'Bibliography')!.rows);
  assert.deepEqual(bibliography.find((each) => each.row.kind === 'links')?.features, ['citations']);
});

test('after a failed preparation, a value not written is of the last successful build, and a write that would copy one is refused', () => {
  const schema = schemaOf(STANDARD);
  const depth = ['flm', 'features', 'toc', 'depth'];
  const inEffect: Record<string, FeatureInEffect> = { toc: { switchedOff: null, keys: { depth: { value: 4, origin: notes, entries: null } } } };
  const written = failed(stateOf(schema, { flm: { features: { toc: { depth: 'x' } } } }, inEffect));
  // What the front matter writes is shown and current.
  assert.equal(shownValue(written, depth), 'x');
  assert.deepEqual(sourceAt(written, depth), { kind: 'written' });
  assert.ok(!outdatedAt(written, depth));
  // A value in effect, or a default, is of the last successful build.
  const unwritten = failed(stateOf(schema, {}, inEffect));
  assert.equal(shownValue(unwritten, depth), 4);
  assert.deepEqual(sourceAt(unwritten, depth), { kind: 'outdated' });
  assert.ok(outdatedAt(unwritten, depth));
  assert.deepEqual(sourceAt(failed(stateOf(schema, {})), depth), { kind: 'outdated' });
  // A checkbox whose check writes the entry's default, while the schema is not current.
  const boxes = schemaOf({ citations: { keys: { type: 'object', properties: { doi: { type: 'boolean', default: true } } } } });
  const path = ['flm', 'features', 'citations', 'keys'];
  assert.ok(boxEditOutdated(failed(stateOf(boxes, {}), false), path, 'doi'));
  assert.ok(!boxEditOutdated(failed(stateOf(boxes, {})), path, 'doi'));
  assert.ok(!boxEditOutdated(failed(stateOf(boxes, { flm: { features: { citations: { keys: { doi: false } } } } }), false), path, 'doi'));
});

test('the packages that the math processor or the build type loads are offered last, not to be added by hand', () => {
  const found = ['math-katex', 'notes', 'profile-pdf', 'graphics', 'mathtools'].map((name) => ({ name, description: null }));
  const state = stateOf(schemaOf(STANDARD), null, {}, { packages: found });
  assert.deepEqual(packagesOffered(state).map(({ name, byHand }) => [name, byHand]), [
    ['notes', true],
    ['graphics', true],
    ['mathtools', true],
    ['math-katex', false],
    ['profile-pdf', false],
  ]);
});

test('the packages in use that the front matter does not list are given in their order', () => {
  const choices = { packages_in_use: ['std', 'math-mathjax', 'notes', 'shared', 'profile-tty'] };
  const state = stateOf(schemaOf(STANDARD), { flm: { use: ['notes'] } }, {}, choices);
  assert.deepEqual(packagesInUse(state, ['notes'], 'tty').map((each) => each.name), ['std', 'math-mathjax', 'shared', 'profile-tty']);
});

test('the note on the packages loaded automatically shows when either list shows one', () => {
  const found = (...names: string[]) => stateOf(schemaOf(STANDARD), null, {}, { packages: names.map((name) => ({ name, description: null })) });
  // Offered among the packages found.
  assert.ok(automaticPackagesShown(found('notes', 'math-katex'), []));
  // Written in `use` alone, found or not.
  assert.ok(automaticPackagesShown(found('notes'), ['math-katex']));
  assert.ok(automaticPackagesShown(found('notes', 'profile-pdf'), ['profile-pdf']));
  assert.ok(!automaticPackagesShown(found('notes', 'graphics'), ['notes']));
});
