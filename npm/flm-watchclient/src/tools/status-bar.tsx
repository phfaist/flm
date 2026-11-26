// The status bar, across the bottom of the window, always shown. At its left: the
// document's name, the selector of the build type, Build again, and the build state. At
// its right: Inspect, the problems, the shorthands of the tabs (in a wide window), and the
// panel button.

import { Icon } from './controls/icon';
import { InspectButton } from './inspect';
import { offeredTabs } from './panel';
import { chooseProfile, rebuild } from './requests';
import { buildLine, counts, shortName } from './state/build';
import { focusTab, panel, preview } from './state/store';

/** `count` and the word for it, `word` or its plural; the word hidden at phone width. */
function Count({ count, word }: { count: number; word: string }) {
  return (
    <span>
      {count}
      <span class="flm-words"> {count === 1 ? word : `${word}s`}</span>
    </span>
  );
}

/** The counts of the last build's problems, one per severity that occurs, or a check when it has none. */
function Counts() {
  const { problems } = preview.value;
  const { errors, warnings, notes } = counts(problems);
  if (problems.length === 0) {
    return (
      <span class="flm-counts flm-tone-success">
        <Icon name="check" size={14} />
        <span class="flm-words">No problems</span>
      </span>
    );
  }
  return (
    <span class="flm-counts">
      {errors > 0 && (
        <span class="flm-count flm-tone-error">
          <Icon name="circle-x" size={14} />
          <Count count={errors} word="error" />
        </span>
      )}
      {warnings > 0 && (
        <span class="flm-count flm-tone-warning">
          <Icon name="triangle-alert" size={14} />
          <Count count={warnings} word="warning" />
        </span>
      )}
      {notes > 0 && (
        <span class="flm-count flm-tone-note">
          <Icon name="info" size={14} />
          <Count count={notes} word="note" />
        </span>
      )}
    </span>
  );
}

/**
 * The problems: a click opens the panel on the Problems tab, or shows that tab, and moves
 * the focus into it. Nothing before the first build has finished.
 */
function Problems() {
  if (preview.value.durationMs === null) return null;
  return (
    <button type="button" class="flm-problems-button" title="Show the problems" onClick={() => focusTab('problems')}>
      <Counts />
    </button>
  );
}

/**
 * The shorthands of the tabs other than Problems and those at the end of the tab row: a
 * click on one opens the panel on its tab, or shows that tab, and moves the focus into it.
 * The stylesheet shows them in a wide window alone.
 */
function Shorthands() {
  const tabs = offeredTabs().filter((each) => each.id !== 'problems' && !each.end);
  return (
    <span class="flm-shorthands">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          class="flm-icon-button"
          aria-label={tab.title ?? tab.name}
          title={tab.title ?? tab.name}
          onClick={() => focusTab(tab.id)}
        >
          <Icon name={tab.icon} size={15} />
        </button>
      ))}
    </span>
  );
}

/** The document's short name, with its whole path as the tooltip; nothing before the server names it. */
function DocumentName() {
  const path = preview.value.document;
  if (path === null) return null;
  return (
    <span class="flm-document-name" title={path}>
      {shortName(path)}
    </span>
  );
}

/** Shows or hides the panel, on the tab it showed last. */
export function togglePanel(): void {
  panel.value = { ...panel.value, open: !panel.value.open };
}

/** The mark after the name of the default build type in the selector of the build type. */
const DEFAULT_MARK = '★';

/**
 * The selector of the build type: the build types the server reports, on the one in use,
 * the default build type marked (`DEFAULT_MARK`). A choice asks the server to build with
 * it; the selector then shows what the server reports, and goes back to the build type in
 * use when the request fails. Nothing before the server names a build type.
 */
function BuildType({ disabled }: { disabled: boolean }) {
  const { profile, defaultProfile, profiles } = preview.value;
  if (profile === null && profiles.length === 0) return null;
  // The build type in use is listed even when the server cannot prepare it.
  const names = profile === null || profiles.includes(profile) ? profiles : [...profiles, profile].sort();
  const choose = async (event: Event) => {
    const select = event.currentTarget as HTMLSelectElement;
    if (!(await chooseProfile(select.value))) select.value = preview.value.profile ?? '';
  };
  return (
    <label class="flm-build-type">
      <span class="flm-words">Showing</span>
      <select
        aria-label="Build type shown"
        title={`${DEFAULT_MARK} marks the default build type, the one built when none is chosen`}
        value={profile ?? ''}
        disabled={disabled}
        onChange={choose}
      >
        {profile === null && <option value="" disabled />}
        {names.map((name) => (
          <option key={name} value={name}>
            {name === defaultProfile ? `${name} ${DEFAULT_MARK}` : name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function StatusBar() {
  const { open } = panel.value;
  const line = buildLine(preview.value);
  const disconnected = preview.value.connection === 'disconnected';
  // The arrow points left while the panel is closed and up while it is open, at either side.
  const arrow = open ? 'chevron-up' : 'chevron-left';
  return (
    <footer class="flm-status">
      <DocumentName />
      <BuildType disabled={disconnected} />
      <button
        type="button"
        class="flm-icon-button"
        aria-label="Build again"
        title="Build again"
        disabled={disconnected}
        onClick={rebuild}
      >
        <Icon name="rotate-cw" size={14} />
      </button>
      <span class="flm-build-state" data-tone={line?.tone} role="status">
        {line?.text ?? ''}
      </span>
      <div class="flm-status-end">
        <InspectButton />
        <Problems />
        <Shorthands />
        <button
          type="button"
          class="flm-panel-button"
          aria-expanded={open}
          aria-label="Show or hide the panel"
          title="Show or hide the panel"
          onClick={togglePanel}
        >
          <Icon name={arrow} size={16} />
        </button>
      </div>
    </footer>
  );
}
