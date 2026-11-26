// A row of the Metadata and Settings tabs: a label, a control, the slot of the green check,
// and at the right end the items that tell where the value comes from (`state/settings.ts`,
// `Source`): one icon, or for a value that the front matter writes over a package's, the
// package icon then the revert arrow. The green check shows for 1.5 s once the server has
// answered a write.

import type { ComponentChildren } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import { settingsReading } from '../front-matter';
import { settingSources } from '../requests';
import type { Answer } from '../requests';
import type { Sources } from '../../shared/settings';
import type { Path, Source } from '../state/settings';
import { OUTDATED_LINE, packageWrites, sourceLines } from '../state/settings';
import { Icon } from './icon';

/** How long the green check shows once a write is answered, in milliseconds. */
export const SAVED_MS = 1500;

/**
 * Whether a row's green check shows, and the function that runs a write of the row: the
 * check shows for `SAVED_MS` once the write gives `true`, the server's answer that it made
 * the change. The function gives the write's own answer.
 */
export function useSaved(): [boolean, (write: Promise<boolean>) => Promise<boolean>] {
  const [saved, setSaved] = useState(false);
  const timer = useRef<number | null>(null);
  useEffect(() => () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
  }, []);
  const run = async (write: Promise<boolean>) => {
    const done = await write;
    if (done) {
      if (timer.current !== null) window.clearTimeout(timer.current);
      setSaved(true);
      timer.current = window.setTimeout(() => setSaved(false), SAVED_MS);
    }
    return done;
  };
  return [saved, run];
}

/** The slot of the green check, 16 px wide whether the check shows or not. */
export function CheckSlot({ saved }: { saved: boolean }) {
  return (
    <span class="flm-check-slot">
      {saved && <Icon name="check" size={14} label="Saved" />}
    </span>
  );
}

/**
 * What the tooltip of a row's source tells beside its first line: the sources of the value
 * at `path`, read from the server, and `marks`, the words of its marks.
 */
export interface SourcesTip {
  path: Path;
  marks: readonly string[];
}

/** An answer of the sources of a row's value: the path asked, as a key, the reading of the settings state it was asked at, and the answer. */
interface SourcesRead {
  key: string;
  reading: number;
  answer: Answer<Sources>;
}

/**
 * The sources of the value at `tip.path`, read from the server once per reading of the
 * settings state (`settingsReading`), and the function that asks for them. With `eager`,
 * they are read when the row is shown and again at each new reading of the settings state;
 * otherwise when the function is called. The answer given is the last one for the path,
 * possibly of an earlier reading; a failure is asked again when the function is called.
 */
function useSources(tip: SourcesTip | undefined, eager: boolean): [SourcesRead | null, () => void] {
  const [read, setRead] = useState<SourcesRead | null>(null);
  const asked = useRef<{ key: string; reading: number } | null>(null);
  const reading = settingsReading.value;
  const key = tip === undefined ? '' : JSON.stringify(tip.path);
  const load = () => {
    if (tip === undefined) return;
    const last = asked.current;
    const failed = read !== null && read.key === key && read.reading === reading && 'failure' in read.answer;
    if (last !== null && last.key === key && last.reading === reading && !failed) return;
    const request = { key, reading };
    asked.current = request;
    void settingSources(tip.path).then((answer) => {
      if (asked.current === request) setRead({ key, reading, answer });
    });
  };
  useEffect(() => {
    if (eager) load();
  }, [eager, key, reading]);
  return [read !== null && read.key === key ? read : null, load];
}

/**
 * The lines of the tooltip of a row's source: `line`, until the sources of the value are
 * read at the current reading of the settings state, then each source's value
 * (`sourceLines`), or `line` and the failure's line when they cannot be read; below, the
 * words of the marks.
 */
function tipLines(line: string, read: SourcesRead | null, tip: SourcesTip | undefined): string[] {
  let lines = [line];
  if (read !== null && read.reading === settingsReading.value) {
    if ('failure' in read.answer) lines = [line, read.answer.failure];
    else if (read.answer.value.sources !== null && read.answer.value.sources.length > 0) lines = sourceLines(read.answer.value.sources);
  }
  return [...lines, ...(tip?.marks ?? [])];
}

/** The name of the package icon of a value that the front matter writes over a package's. */
const ALSO_PACKAGE_LINE = 'Also set by a package';

/**
 * The button that reveals a row's source: its tooltip, `lines`, and the same lines in a
 * small popover when it is activated; its name is `name`. `onWanted` runs when the tooltip
 * is first wanted (the pointer enters, the focus comes, a click).
 */
function OriginButton({ name, lines, onWanted, children }: { name: string; lines: string[]; onWanted: () => void; children: ComponentChildren }) {
  const [open, setOpen] = useState(false);
  const text = lines.join('\n');
  return (
    <span class="flm-origin">
      <button
        type="button"
        class="flm-icon-button flm-row-end"
        aria-label={name}
        title={text}
        aria-expanded={open}
        onPointerEnter={onWanted}
        onFocus={onWanted}
        onClick={() => {
          onWanted();
          setOpen(!open);
        }}
        onBlur={() => setOpen(false)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') setOpen(false);
        }}
      >
        {children}
      </button>
      {open && <span class="flm-popover" role="status">{lines.map((each, at) => <span key={at} class="flm-popover-line">{each}</span>)}</span>}
    </span>
  );
}

/** The revert arrow: active when the front matter writes the key (it removes the key), greyed out otherwise. */
function RevertButton({ disabled, onRevert }: { disabled: boolean; onRevert: () => void }) {
  return (
    <button type="button" class="flm-icon-button flm-row-end" aria-label="Revert to default" title="Revert to default" disabled={disabled} onClick={onRevert}>
      <Icon name="rotate-ccw" size={14} />
    </button>
  );
}

/**
 * The items at the right end of a row: the revert arrow, active when the front matter writes
 * the key (it removes the key) and greyed out at the default, and for a value that the front
 * matter writes, with `tip`, the package icon before it when a package also writes the
 * value (the sources are read when the row is shown); the package icon, whose
 * tooltip lists each source's value and the words of the marks (`tip`), and which shows
 * the same lines in a small popover when it is activated (for a touch screen and the
 * keyboard); for a value that a higher source decides, the words that name that source,
 * with the same tooltip, then the revert arrow; or the mark of a value of the last
 * successful build, a sign drawn in the error color of the tab's note, dimmed, with its
 * tooltip and its name.
 */
export function SourceIcon({ source, disabled, onRevert, tip }: {
  source: Source;
  disabled: boolean;
  onRevert: () => void;
  /** The path and the marks' words of the row's value, for the tooltip of its source. */
  tip?: SourcesTip;
}) {
  const [read, load] = useSources(tip, source.kind === 'written' && tip !== undefined);
  switch (source.kind) {
    case 'written': {
      const writes = read !== null && 'value' in read.answer ? read.answer.value.sources : null;
      return (
        <>
          {writes !== null && packageWrites(writes) && (
            <OriginButton name={ALSO_PACKAGE_LINE} lines={[...sourceLines(writes), ...(tip?.marks ?? [])]} onWanted={() => undefined}>
              <Icon name="package" size={14} />
            </OriginButton>
          )}
          <RevertButton disabled={disabled} onRevert={onRevert} />
        </>
      );
    }
    case 'default':
      return <RevertButton disabled onRevert={onRevert} />;
    case 'outdated':
      return (
        <span class="flm-outdated-mark" role="img" aria-label={OUTDATED_LINE} title={OUTDATED_LINE}>
          <Icon name="history" size={14} />
        </span>
      );
    case 'package':
      return <OriginButton name={source.line} lines={tipLines(source.line, read, tip)} onWanted={load}><Icon name="package" size={14} /></OriginButton>;
    case 'decided':
      return (
        <>
          <span class="flm-decided">
            <OriginButton name={source.line} lines={tipLines(source.line, read, tip)} onWanted={load}><span class="flm-decided-line">{source.line}</span></OriginButton>
          </span>
          <RevertButton disabled={disabled || !source.written} onRevert={onRevert} />
        </>
      );
  }
}

/** The discreet mark of a feature or a key that no package in use has. */
export function UnknownMark() {
  return <span class="flm-unknown-mark">no package in use</span>;
}

/** A label's text, where a name between backquotes is shown as code. */
export function Label({ text }: { text: string }) {
  return <>{text.split('`').map((part, at) => (at % 2 === 1 ? <code key={at}>{part}</code> : part))}</>;
}

/** The width of a row's label column: in Metadata, in the common settings, in the advanced list. */
export type Labels = 'metadata' | 'common' | 'advanced';

/**
 * A row: `label` then `control` (for a switch, the switch then its label, as a checkbox's),
 * `after` (a link that follows the control), the check slot, and `end`, the icon at the
 * right end; `below`, the YAML editor of the row, under it. `named` is `true` when `label` names the control as a `<label>` does; a
 * control that has several parts (chips, checkboxes) is named by itself. `outdated` dims
 * the value the control shows: it is that of the last successful build. `id` is the id of
 * the row's element, for a link that shows the row.
 */
export function Row({ label, control, after, saved, end, below, labels, switchFirst = false, named = true, disabled = false, outdated = false, id }: {
  label: ComponentChildren;
  control: ComponentChildren;
  after?: ComponentChildren;
  saved: boolean;
  end?: ComponentChildren;
  below?: ComponentChildren;
  labels: Labels;
  switchFirst?: boolean;
  named?: boolean;
  disabled?: boolean;
  outdated?: boolean;
  id?: string;
}) {
  const name = <span class="flm-row-label">{label}</span>;
  const parts = switchFirst ? <>{control}{name}</> : <>{name}{control}</>;
  return (
    <div id={id} class="flm-row" data-labels={labels} data-disabled={disabled ? 'true' : undefined} data-outdated={outdated ? 'true' : undefined}>
      <div class="flm-row-line">
        {named ? <label class="flm-row-main">{parts}</label> : <div class="flm-row-main">{parts}</div>}
        {after}
        <CheckSlot saved={saved} />
        {end}
      </div>
      {below}
    </div>
  );
}
