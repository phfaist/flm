// The settings state that the Metadata and Settings tabs show, and their changes of the
// front matter. A tab that shows the state reads it when it opens, and again whenever the
// stream's counter of the settings state differs from the one it was read at; the edits of
// each change of a control are sent in one request, with the revision of the state read, and
// the state is read again once the server has answered, so that the next change is made
// against the file as it then is (a change refused as made against an outdated revision is
// followed by the same reading).

import { signal } from '@preact/signals';
import { useEffect } from 'preact/hooks';
import type { SettingsState } from '../shared/settings';
import { changeFrontMatter, settingsState } from './requests';
import { FEATURES_PATH } from '../shared/settings';
import type { Edit } from './state/settings';
import { bodyOf, copiedLine } from './state/settings';
import { dispatch, preview } from './state/store';

/** The settings state read last; `null` before the first reading. */
export const settings = signal<SettingsState | null>(null);

/** Counts the readings started, so that the answer of an earlier one is dropped. */
let readings = 0;

/** The number of the reading that gave `settings`, counted as `readings` counts them. */
export const settingsReading = signal<number>(0);

/** The number of readings started so far. */
export function readingsStarted(): number {
  return readings;
}

/** Reads the settings state again. */
export async function readAgain(): Promise<void> {
  readings += 1;
  const reading = readings;
  const state = await settingsState();
  if (reading === readings && state !== null) {
    settings.value = state;
    settingsReading.value = reading;
  }
}

/**
 * The settings state, for a tab that shows it: read when the tab opens, and again when the
 * stream's counter changes, without closing the tab.
 */
export function useSettings(): SettingsState | null {
  const counter = preview.value.settings;
  useEffect(() => {
    void readAgain();
  }, [counter]);
  return settings.value;
}

/**
 * Whether the controls that write the front matter are enabled: the server is connected,
 * and the front matter of the state read can be read.
 */
export function canWrite(state: SettingsState | null): boolean {
  return state !== null && state.revision !== null && preview.value.connection === 'connected';
}

/**
 * Sends `edits`, the edits of one change of a control, as the author made it, in one
 * request against the revision of the state read: the server makes all of them or none,
 * and copies into the front matter the values that the change would otherwise replace,
 * which the status bar then names (`copiedLine`). Gives whether the server made them, once
 * the state has been read again.
 */
export async function write(edits: readonly Edit[]): Promise<boolean> {
  const revision = settings.value?.revision ?? null;
  if (revision === null) return false;
  const copied = await changeFrontMatter(bodyOf(revision, edits));
  const line = copied === null ? null : copiedLine(copied, FEATURES_PATH);
  if (line !== null) dispatch({ type: 'noted', text: line });
  await readAgain();
  return copied !== null;
}
