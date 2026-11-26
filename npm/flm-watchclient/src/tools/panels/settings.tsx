// The Settings tab: a search field, the common settings by topic, then the advanced list of
// every feature of the document's packages. The topics, their rows and the curated
// settings of the advanced list are those of the description (`state/description.ts`); the
// rows that the document's environment lacks are left out. One topic holds the settings of
// the compose template in use, a row per setting that the template declares, with the
// control that follows from the setting's declared schema. A feature of the advanced list
// has an arrow that expands its settings and a switch *enabled*; the settings of a feature
// that is off are greyed out, and cannot be changed while the front matter writes the
// entry `FEATURE: false`, through which the server refuses a write. The order of the advanced list, and
// which features start expanded, are fixed when the tab opens. The search field filters
// every row of the tab; the filter beside the heading of the advanced list filters that
// list alone, and a row of the list is shown when it matches both. After a preparation that
// failed, a note at the top says so, and the rows mark what is of the last successful build
// (`state/settings-panel.ts`).

import { useEffect, useRef, useState } from 'preact/hooks';
import type { SettingsState } from '../../shared/settings';
import { canWrite, readingsStarted, settingsReading, useSettings, write } from '../front-matter';
import { CheckboxGroup, OrderedNames } from '../controls/entries';
import { Choice, Switch } from '../controls/fields';
import { Icon } from '../controls/icon';
import { KeyRow } from '../controls/key';
import { CheckSlot, Row, SourceIcon, UnknownMark, useSaved } from '../controls/row';
import type { Described } from '../state/description';
import { CURATED, MATH_PROCESSOR_PATH, TOPICS } from '../state/description';
import type { Control, Path } from '../state/settings';
import { controlOf, quoted, reverted, textsEdit, textsOf } from '../state/settings';
import type { DescribedRow, Listed, TemplateInUse } from '../state/settings-panel';
import {
  advancedList, automaticPackagesShown, boxChecked, boxEdit, boxEditOutdated, boxesSource, boxesWritten, curatedLabel, describedRows,
  entryPath, environmentOptions, featureKeys, firstTemplateSettings, keptList, marksAt, matches, matchesEach, packagesFound,
  packagesInUse, packagesOffered, rowLocked, schemaAt, settingsFeatureOf, shownSwitch, sourceAt, switchEdit, switchedOn,
  templateEdit, templateInUse, templateOptions, templateOutputs, templateRevert, templateSettingKeys,
  templateSettingsPath, templateSource, templateValue, unknownFeature, unknownKey, valueAt, writtenKeys, writtenTemplateSettings,
} from '../state/settings-panel';
import { panel, preview, settingsTarget } from '../state/store';
import { Unreadable } from './metadata';

/** Whether the rows at `path` cannot be changed: the front matter cannot be written, or the server refuses a write at the path (`rowLocked`). */
function lockedAt(state: SettingsState, path: Path): boolean {
  return !canWrite(state) || rowLocked(state, path);
}

/** The tooltip of the source of the row at `path`: the sources of its value, and the words of its marks. */
function tipAt(state: SettingsState, path: Path) {
  return { path, marks: marksAt(state, path) };
}

/** The key of a path, as the search matches it: its keys joined by dots. */
function keyOf(path: Path): string {
  return path.join('.');
}

/** The id of the element of a feature in the advanced list. */
function featureId(name: string): string {
  return `flm-feature-${name}`;
}

/** The control that a described row gives, for the rows whose control is one value's. */
function describedControl(state: SettingsState, row: Described, path: Path): Control | undefined {
  switch (row.kind) {
    case 'choice': {
      const environment = row.environment === undefined ? [] : environmentOptions(state, row.environment);
      return { kind: 'choice', options: [...environment, ...row.options] };
    }
    case 'switch':
      return { kind: 'switch' };
    case 'number': {
      const own = controlOf(state.schema, schemaAt(state.schema, path));
      return { kind: 'number', integer: own.kind === 'number' && own.integer };
    }
    case 'texts':
      return { kind: 'texts' };
    default:
      return undefined;
  }
}

/** The label that a row of the description shows, and that the search matches. */
function labelOf(described: DescribedRow): string {
  const { row, path } = described;
  if (row.kind === 'templates' || row.kind === 'template-settings') return '';
  if (row.kind === 'packages') return 'Packages';
  return row.label ?? curatedLabel(path);
}

/** Whether a row of the description matches the search `query`. */
function rowMatches(state: SettingsState, query: string, described: DescribedRow): boolean {
  const { row, path } = described;
  switch (row.kind) {
    case 'templates':
      return templateOutputs(state).some((output) => matches(query, output, keyOf([...path, output])));
    case 'template-settings': {
      const template = described.template === null ? null : templateInUse(state, described.template);
      return template !== null && matchingTemplateSettings(state, path, template, query).length > 0;
    }
    case 'links':
      return matches(query, row.label, ...described.features);
    case 'packages':
      return matches(query, labelOf(described), keyOf(path), ...textsOf(valueAt(state.frontMatter, path)));
    default:
      return matches(query, labelOf(described).replace(/`/g, ''), keyOf(path));
  }
}

function TemplateRow({ state, path, output, disabled }: { state: SettingsState; path: Path; output: string; disabled: boolean }) {
  const [saved, run] = useSaved();
  const source = templateSource(state, path, output);
  return (
    <Row
      labels="common"
      label={<code>{output}</code>}
      saved={saved}
      outdated={source.kind === 'outdated'}
      control={(
        <Choice
          value={templateValue(state, path, output)}
          options={templateOptions(state, output)}
          disabled={disabled}
          onChange={(chosen) => void run(write([templateEdit(path, output, chosen)]))}
        />
      )}
      end={(
        <SourceIcon
          source={source}
          disabled={disabled}
          tip={tipAt(state, [...path, output])}
          onRevert={() => void run(write([templateRevert(path, output)]))}
        />
      )}
    />
  );
}

/**
 * The template rows: the output that the build type shown is made from, then the others
 * behind a link; while the search field holds a text, the rows that match it.
 */
function TemplateRows({ state, path, query }: { state: SettingsState; path: Path; query: string }) {
  const [others, setOthers] = useState(false);
  const outputs = templateOutputs(state);
  const searching = query.trim() !== '';
  const shown = outputs.filter((output, at) => (searching ? matches(query, output, keyOf([...path, output])) : at === 0 || others));
  const disabled = lockedAt(state, path);
  return (
    <>
      {shown.map((output) => (
        <TemplateRow key={output} state={state} path={path} output={output} disabled={disabled} />
      ))}
      {!searching && outputs.length > 1 && (
        <button type="button" class="flm-link flm-more" aria-expanded={others} onClick={() => setOthers(!others)}>
          {others ? 'Hide the other outputs' : `Other outputs… (${outputs.length - 1})`}
        </button>
      )}
    </>
  );
}

/** The settings of `template`, under the key at `path`, that match the search `query`: by their key. */
function matchingTemplateSettings(state: SettingsState, path: Path, template: TemplateInUse, query: string): string[] {
  const settings = templateSettingsPath(path, template);
  return templateSettingKeys(state, path, template).filter((key) => matches(query, key, keyOf([...settings, key])));
}

/**
 * The rows of the settings of the compose template `template`, under the key at `path`: a
 * line that names the template and its output, then a row per setting, whose control
 * follows from the setting's declared schema, as any other key's does (*Edit as YAML* for a
 * setting that the template does not declare). The first settings are shown, with those
 * that the front matter writes, or wrote since the rows were first shown, and the others
 * behind a link; while the search field holds a text, the settings that match it.
 */
function TemplateSettings({ state, path, template, query }: { state: SettingsState; path: Path; template: TemplateInUse; query: string }) {
  const [more, setMore] = useState(false);
  const pinned = useRef(new Set<string>());
  for (const key of writtenTemplateSettings(state, path, template)) pinned.current.add(key);
  const settings = templateSettingsPath(path, template);
  const keys = templateSettingKeys(state, path, template);
  const first = firstTemplateSettings(keys, [...pinned.current]);
  const searching = query.trim() !== '';
  const shown = searching ? matchingTemplateSettings(state, path, template, query) : more ? keys : first;
  const disabled = lockedAt(state, path);
  return (
    <>
      <p class="flm-template-in-use">Template <code>{template.name}</code> for <code>{template.output}</code></p>
      {shown.map((key) => {
        const at = [...settings, key];
        const declared = Object.hasOwn(template.defaults, key);
        const fallback = declared ? template.defaults[key] : undefined;
        return (
          <KeyRow
            key={key}
            label={`\`${key}\``}
            path={at}
            state={state}
            fallback={fallback}
            labels="common"
            disabled={disabled}
          />
        );
      })}
      {!searching && first.length < keys.length && (
        <button type="button" class="flm-link flm-more" aria-expanded={more} onClick={() => setMore(!more)}>
          {more ? 'Hide additional settings' : `More… (${keys.length - first.length})`}
        </button>
      )}
    </>
  );
}

function BoxesRow({ state, row, path }: { state: SettingsState; row: Extract<Described, { kind: 'checkboxes' }>; path: Path }) {
  const [saved, run] = useSaved();
  const disabled = lockedAt(state, path);
  const entries = row.boxes.map((box) => box.entry);
  const source = boxesSource(state, path, entries);
  return (
    <Row
      labels="common"
      label={row.label}
      named={false}
      saved={saved}
      disabled={disabled}
      outdated={source.kind === 'outdated'}
      control={(
        <CheckboxGroup
          name={row.label}
          disabled={disabled}
          boxes={row.boxes.map((box) => {
            const checked = boxChecked(state, path, box.entry);
            return {
              entry: box.entry,
              label: <>{box.label}{box.note !== undefined && <span class="flm-checkbox-note"> {box.note}</span>}</>,
              checked,
              // An entry removed by a lower source, without a default to write back, or whose
              // default is of the last successful build.
              disabled: !checked && (boxEdit(state, path, box.entry, true) === null || boxEditOutdated(state, path, box.entry)),
            };
          })}
          onChange={(entry, checked) => {
            const edit = boxEdit(state, path, entry, checked);
            if (edit !== null) void run(write([edit]));
          }}
        />
      )}
      end={(
        <SourceIcon
          source={source}
          disabled={disabled}
          tip={tipAt(state, path)}
          onRevert={() => void run(write(boxesWritten(state, path, entries).map((entry) => reverted([...path, entry]))))}
        />
      )}
    />
  );
}

/**
 * The row of the packages in use: those that `use` lists, then those in use that it does
 * not list, locked, each with why it is in use. Under the lists, when one shows a package
 * that is loaded automatically, a note says so, with a link to the row of the math
 * processor when the tab shows it.
 */
function PackagesRow({ state, path, onShowRow }: { state: SettingsState; path: Path; onShowRow: (path: Path) => void }) {
  const [saved, run] = useSaved();
  const disabled = lockedAt(state, path);
  const written = valueAt(state.frontMatter, path);
  const names = textsOf(written);
  const found = packagesFound(state);
  const offered = packagesOffered(state);
  const source = sourceAt(state, path);
  const math = describedRows(state, MATH_ROW)[0];
  const processor = math === undefined
    ? 'math processor'
    : <button type="button" class="flm-link" onClick={() => onShowRow(math.path)}>math processor</button>;
  const note = automaticPackagesShown(state, names)
    ? <>Packages <code>math-*</code> and <code>profile-*</code> are loaded automatically according to the selected {processor} and build type.</>
    : undefined;
  return (
    <div class="flm-row" data-labels="common">
      <div class="flm-row-line flm-row-top">
        <OrderedNames
          names={names}
          describe={(name) => found.find((each) => each.name === name)?.description ?? null}
          locked={packagesInUse(state, names, preview.value.profile)}
          available={offered}
          disabled={disabled}
          name="Packages"
          other="Another package, by name"
          note={note}
          onChange={(next) => run(write([textsEdit(path, written, next)]))}
        />
        <CheckSlot saved={saved} />
        <SourceIcon source={source} disabled={disabled} tip={tipAt(state, path)} onRevert={() => void run(write([reverted(path)]))} />
      </div>
    </div>
  );
}

/** The row of the math processor, as the description gives it. */
const MATH_ROW = TOPICS.flatMap((topic) => topic.rows).filter((row) => row.kind === 'choice' && row.path === MATH_PROCESSOR_PATH);

/** The id of the element of a row of the common settings at `path`. */
function rowId(path: Path): string {
  return `flm-row-${keyOf(path)}`;
}

/** A row of the description, in the common settings or as a curated row of the advanced list. */
function DescribedRowView({ state, described, labels, query, onShow, onShowRow }: {
  state: SettingsState;
  described: DescribedRow;
  labels: 'common' | 'advanced';
  query: string;
  onShow: (feature: string) => void;
  onShowRow: (path: Path) => void;
}) {
  const { row, path } = described;
  switch (row.kind) {
    case 'templates':
      return <TemplateRows state={state} path={path} query={query} />;
    case 'template-settings': {
      // The rows start anew for another template: their link "More…" and the settings kept shown.
      const template = described.template === null ? null : templateInUse(state, described.template);
      return template === null
        ? null
        : <TemplateSettings key={`${template.output} ${template.name}`} state={state} path={path} template={template} query={query} />;
    }
    case 'checkboxes':
      return <BoxesRow state={state} row={row} path={path} />;
    case 'packages':
      return <PackagesRow state={state} path={path} onShowRow={onShowRow} />;
    case 'links':
      return (
        <div class="flm-links">
          <span class="flm-links-label">{row.label}</span>
          {described.features.map((feature) => (
            <button key={feature} type="button" class="flm-link" onClick={() => onShow(feature)}>{feature}</button>
          ))}
        </div>
      );
    default: {
      // A row of texts is greyed out while the value at `unless` is `false`.
      const { custom, unless } = described;
      const off = unless !== null && unless.length > 0 && !boxChecked(state, unless.slice(0, -1), unless[unless.length - 1]!);
      // The feature of the settings of the option chosen, which the link after the choice shows.
      const settings = row.kind === 'choice' && row.settingsOf !== undefined ? settingsFeatureOf(state, path, row.settingsOf) : null;
      return (
        <KeyRow
          label={labelOf(described)}
          path={path}
          state={state}
          control={describedControl(state, row, path)}
          custom={custom ?? undefined}
          after={settings !== null && (
            <button type="button" class="flm-link flm-settings-of" title={`The settings of ${settings}`} onClick={() => onShow(settings)}>
              options…
            </button>
          )}
          labels={labels}
          disabled={lockedAt(state, path) || off}
          id={labels === 'common' ? rowId(path) : undefined}
        />
      );
    }
  }
}

/** The common settings: each topic that has a row for the document's environment, and that matches the search. */
function CommonSettings({ state, query, onShow, onShowRow }: {
  state: SettingsState;
  query: string;
  onShow: (feature: string) => void;
  onShowRow: (path: Path) => void;
}) {
  return (
    <>
      {TOPICS.map((topic) => {
        const rows = describedRows(state, topic.rows).filter((each) => rowMatches(state, query, each));
        if (rows.length === 0) return null;
        return (
          <section key={topic.title} class="flm-rows-group" aria-label={topic.title}>
            <h3 class="flm-section-heading">{topic.title}</h3>
            <div class="flm-rows-topic">
              {rows.map((each) => (
                <DescribedRowView
                  key={keyOf(each.path) || each.features.join(' ')}
                  state={state}
                  described={each}
                  labels="common"
                  query={query}
                  onShow={onShow}
                  onShowRow={onShowRow}
                />
              ))}
            </div>
          </section>
        );
      })}
    </>
  );
}

/** The head of a feature of the advanced list: its arrow and name, then the switch *enabled*. */
function FeatureHead({ state, name, open, expandable, onToggle }: {
  state: SettingsState;
  name: string;
  open: boolean;
  expandable: boolean;
  onToggle: () => void;
}) {
  const [saved, run] = useSaved();
  const { on, source } = shownSwitch(state, name);
  const unknown = unknownFeature(state, name);
  return (
    <div class="flm-feature-head" data-outdated={source !== null && source.kind === 'outdated' ? 'true' : undefined}>
      <button type="button" class="flm-feature-name" aria-expanded={expandable ? open : undefined} disabled={!expandable} onClick={onToggle}>
        <span class="flm-feature-arrow" data-open={open ? 'true' : 'false'}>{expandable && <Icon name="chevron-right" size={14} />}</span>
        <code>{name}</code>
        {unknown && <UnknownMark />}
      </button>
      <Switch
        small
        checked={on}
        name={`Feature ‘${name}’ enabled`}
        disabled={!canWrite(state)}
        onChange={(checked) => void run(write([switchEdit(state, name, checked)]))}
      />
      <span class="flm-feature-enabled">enabled</span>
      <CheckSlot saved={saved} />
      {source !== null && <SourceIcon source={source} disabled onRevert={() => undefined} />}
    </div>
  );
}

/**
 * A feature of the advanced list, with its rows when it is expanded, or when the search and
 * the filter of the list match some: while either field holds a text, the feature shows the
 * rows that match both, and is left out when neither its name nor a row does.
 */
function Feature({ state, listed, pinned, open, more, query, filter, onToggle, onMore }: {
  state: SettingsState;
  listed: Listed;
  /** The keys shown at once in a feature of the first group: those the front matter writes. */
  pinned: readonly string[];
  open: boolean;
  more: boolean;
  /** The text of the tab's search field. */
  query: string;
  /** The text of the filter of the advanced list. */
  filter: string;
  onToggle: () => void;
  onMore: () => void;
}) {
  const { name, first } = listed;
  const curated = describedRows(state, CURATED[name] ?? []);
  const keys = featureKeys(state, name);
  const queries = [query, filter];
  const searching = queries.some((each) => each.trim() !== '');
  const keyMatches = (key: string) => matchesEach(queries, key, keyOf([...entryPath(name), key]));
  const curatedShown = searching
    ? curated.filter((each) => matchesEach(queries, curatedLabel(each.path), keyOf(each.path)))
    : curated;
  const hidden = first ? keys.filter((key) => !pinned.includes(key)) : [];
  let shownKeys: string[];
  if (searching) shownKeys = matchesEach(queries, name) ? keys : keys.filter(keyMatches);
  else shownKeys = first && !more ? keys.filter((key) => pinned.includes(key)) : keys;
  if (searching && !matchesEach(queries, name) && curatedShown.length === 0 && shownKeys.length === 0) return null;
  const expandable = keys.length > 0 || curated.length > 0;
  const expanded = expandable && (open || searching);
  const off = !switchedOn(state, name);
  return (
    <div class="flm-feature" id={featureId(name)} data-first={first ? 'true' : 'false'} data-off={off ? 'true' : 'false'}>
      <FeatureHead state={state} name={name} open={expanded} expandable={expandable} onToggle={onToggle} />
      {expanded && (
        <div class="flm-feature-body">
          {curatedShown.map((each) => (
            <DescribedRowView
              key={keyOf(each.path)}
              state={state}
              described={each}
              labels="advanced"
              query={query}
              onShow={() => undefined}
              onShowRow={() => undefined}
            />
          ))}
          {shownKeys.map((key) => (
            <KeyRow
              key={key}
              label={key}
              path={[...entryPath(name), key]}
              state={state}
              labels="advanced"
              disabled={lockedAt(state, [...entryPath(name), key])}
              unknown={unknownKey(state, name, key)}
            />
          ))}
          {!searching && hidden.length > 0 && (
            <button type="button" class="flm-link flm-more" aria-expanded={more} onClick={onMore}>
              {more ? 'Hide additional settings' : `More… (${hidden.length})`}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * The advanced list, in the order fixed when the tab opened, under its heading and its
 * filter, a small field that filters the list alone, as the author types. The order is
 * fixed from the first state read since then (`fresh`): the state kept from an earlier
 * opening of the tab may be older than the file.
 */
function AdvancedSettings({ state, fresh, query, filter, open, more, onFilter, onToggle, onMore }: {
  state: SettingsState;
  fresh: boolean;
  query: string;
  filter: string;
  open: Readonly<Record<string, boolean>>;
  more: Readonly<Record<string, boolean>>;
  onFilter: (filter: string) => void;
  onToggle: (name: string, open: boolean) => void;
  onMore: (name: string) => void;
}) {
  const order = useRef<Listed[]>([]);
  // The keys that the front matter wrote in each feature when the feature was first listed.
  const pinnedAtOpen = useRef(new Map<string, string[]>());
  const curated: Record<string, readonly unknown[]> = {};
  for (const name of Object.keys(CURATED)) curated[name] = describedRows(state, CURATED[name] ?? []);
  const now = advancedList(state, curated);
  if (fresh) order.current = keptList(order.current, now);
  return (
    <>
      <div class="flm-advanced-heading">
        <h3 class="flm-section-heading">Advanced feature settings</h3>
        <label class="flm-search flm-filter">
          <Icon name="list-filter" size={12} />
          <input
            type="search"
            aria-label="Filter the advanced feature settings"
            placeholder="Filter"
            spellcheck={false}
            value={filter}
            onInput={(event) => onFilter((event.currentTarget as HTMLInputElement).value)}
          />
        </label>
      </div>
      <div class="flm-features">
        {(fresh ? order.current : now).map((listed) => {
          const { name, first } = listed;
          const written = writtenKeys(state, name);
          if (fresh && !pinnedAtOpen.current.has(name)) pinnedAtOpen.current.set(name, written);
          const pinned = [...new Set([...(pinnedAtOpen.current.get(name) ?? []), ...written])];
          const isOpen = open[name] ?? first;
          return (
            <Feature
              key={name}
              state={state}
              listed={listed}
              pinned={pinned}
              open={isOpen}
              more={more[name] ?? false}
              query={query}
              filter={filter}
              onToggle={() => onToggle(name, !isOpen)}
              onMore={() => onMore(name)}
            />
          );
        })}
      </div>
    </>
  );
}

/**
 * The note at the top of the tab after a preparation that failed: that settings may show
 * values of the last successful build, a link to Problems, and the program's reason.
 */
function PreparationFailed({ reason }: { reason: string }) {
  return (
    <div class="flm-preparation-failed" role="status">
      <p>
        Build failed: Settings may show values from the last successful build.{' '}
        <button type="button" class="flm-link" onClick={() => (panel.value = { ...panel.value, tab: 'problems' })}>Show problems</button>
      </p>
      <p class="flm-preparation-reason">{quoted(reason)}</p>
    </div>
  );
}

function Search({ query, onChange }: { query: string; onChange: (query: string) => void }) {
  return (
    <div class="flm-settings-search">
      <label class="flm-search">
        <Icon name="search" size={14} />
        <input
          type="search"
          aria-label="Search the settings"
          placeholder="Search settings"
          spellcheck={false}
          value={query}
          onInput={(event) => onChange((event.currentTarget as HTMLInputElement).value)}
        />
      </label>
    </div>
  );
}

export function SettingsTab() {
  const state = useSettings();
  // The readings started before the tab opened: a state they gave may be older than the file.
  const opened = useRef<number | null>(null);
  if (opened.current === null) opened.current = readingsStarted();
  const fresh = settingsReading.value > opened.current;
  const [query, setQuery] = useState('');
  // The text of the filter of the advanced list.
  const [filter, setFilter] = useState('');
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [more, setMore] = useState<Record<string, boolean>>({});
  const [target, setTarget] = useState<string | null>(null);
  // The row of the common settings that a link asked for, shown once the search is emptied.
  const [rowTarget, setRowTarget] = useState<Path | null>(null);
  // A link of the common settings has expanded its feature: the feature is shown.
  useEffect(() => {
    if (target === null) return;
    document.getElementById(featureId(target))?.scrollIntoView({ block: 'start' });
    setTarget(null);
  }, [target]);
  // A link has asked for a row: the row is shown, and its control takes the focus.
  useEffect(() => {
    if (rowTarget === null) return;
    const row = document.getElementById(rowId(rowTarget));
    row?.scrollIntoView({ block: 'center' });
    row?.querySelector<HTMLElement>('select, input, button')?.focus({ preventScroll: true });
    setRowTarget(null);
  }, [rowTarget]);
  const showRow = (path: Path) => {
    setQuery('');
    setRowTarget(path);
  };
  const show = (feature: string) => {
    setQuery('');
    setFilter('');
    setOpen({ ...open, [feature]: true });
    setTarget(feature);
  };
  // Another tab asked to show a feature (the help page of its settings): it is shown once
  // the state is read.
  const asked = settingsTarget.value;
  useEffect(() => {
    if (asked === null || state === null) return;
    settingsTarget.value = null;
    show(asked);
  }, [asked, state === null]);
  return (
    <div class="flm-settings">
      <Search query={query} onChange={setQuery} />
      <div class="flm-settings-body">
        {state === null
          ? <p class="flm-panel-line">Reading the settings…</p>
          : (
            <>
              {state.unreadable !== null && <Unreadable reason={state.unreadable} />}
              {state.unreadable === null && state.preparationFailed !== null && <PreparationFailed reason={state.preparationFailed} />}
              <CommonSettings state={state} query={query} onShow={show} onShowRow={showRow} />
              <AdvancedSettings
                state={state}
                fresh={fresh}
                query={query}
                filter={filter}
                open={open}
                more={more}
                onFilter={setFilter}
                onToggle={(name, isOpen) => setOpen({ ...open, [name]: isOpen })}
                onMore={(name) => setMore({ ...more, [name]: !(more[name] ?? false) })}
              />
            </>
          )}
      </div>
    </div>
  );
}
