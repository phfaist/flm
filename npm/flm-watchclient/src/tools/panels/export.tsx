// The Export tab: the build type, the box that states the problems of the preview's last
// build, or the errors for which the server refused the last export (with the checkbox that
// confirms an export despite the errors), Save as sibling and Download, the result of the
// last export, and the command line that writes the same files from a terminal. The server
// names and places the export; the tab sends no path.

import { Fragment } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import type { ExportAction, ExportName } from '../../shared/export';
import { useCopy } from '../controls/copy-ref';
import { Icon } from '../controls/icon';
import type { Answer } from '../requests';
import { exportDocument, exportName, openExported } from '../requests';
import { counts } from '../state/build';
import type { ExportResult, ProblemsBox as Box } from '../state/export';
import { commandShown, exportForm, refusedForErrors, resultFor } from '../state/export';
import { exporting, preview, showTab } from '../state/store';

/** The build type the tab exports: the one chosen in it, else the one shown. */
function chosenProfile(): string | null {
  return exporting.value.profile ?? preview.value.profile;
}

/** The selector of the build type, among those the server reports. */
function BuildType({ profile, disabled }: { profile: string | null; disabled: boolean }) {
  const { profiles } = preview.value;
  const names = profile === null || profiles.includes(profile) ? profiles : [...profiles, profile].sort();
  return (
    <label class="flm-export-type">
      <span>Build type</span>
      <select
        class="flm-field"
        value={profile ?? ''}
        disabled={disabled}
        onChange={(event) => {
          const chosen = (event.currentTarget as HTMLSelectElement).value;
          exporting.value = { ...exporting.value, profile: chosen, last: null };
        }}
      >
        {profile === null && <option value="" disabled />}
        {names.map((name) => (
          <option key={name} value={name}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

/**
 * The box that states the problems, with the checkbox when the document has errors, and
 * the link that shows them: the Problems tab, or the log of the export refused for them.
 */
function ProblemsBox({ box }: { box: Box }) {
  const [log, setLog] = useState(false);
  const { link } = box;
  // Each box starts with its log hidden.
  useEffect(() => setLog(false), [box.text, link.kind === 'log' ? link.log : null]);
  return (
    <div class="flm-export-box" role="note">
      <Icon name="triangle-alert" size={15} />
      <span class="flm-export-box-text">{box.text}</span>
      {link.kind === 'problems' ? (
        <button type="button" class="flm-link" onClick={() => showTab('problems')}>
          Show problems
        </button>
      ) : (
        link.log !== null && (
          <button type="button" class="flm-link" aria-expanded={log} onClick={() => setLog(!log)}>
            {log ? 'Hide log' : 'Show log'}
          </button>
        )
      )}
      {box.confirm && (
        <label class="flm-checkbox flm-export-confirm">
          <input
            type="checkbox"
            checked={exporting.value.confirmed}
            onChange={(event) => {
              exporting.value = { ...exporting.value, confirmed: (event.currentTarget as HTMLInputElement).checked };
            }}
          />
          Export despite the errors
        </label>
      )}
      {log && link.kind === 'log' && link.log !== null && <pre class="flm-export-log">{link.log}</pre>}
    </div>
  );
}

/**
 * Exports the document with the build type `profile`, as `action` says. An export refused
 * for the document's errors clears the checkbox, so that the errors it states are confirmed
 * afresh.
 */
async function run(profile: string, action: ExportAction, continuing: boolean): Promise<void> {
  exporting.value = { ...exporting.value, running: action };
  const result = await exportDocument({ profile, action, continue_with_errors: continuing });
  const confirmed = refusedForErrors(result) ? false : exporting.value.confirmed;
  const last = result !== null ? { profile, result } : null;
  exporting.value = { ...exporting.value, running: null, last, confirmed };
}

/** Save as sibling and Download; while an export runs, the button that started it says so. */
function Buttons({ profile, name, enabled, continuing }: { profile: string; name: string | null; enabled: boolean; continuing: boolean }) {
  const { running } = exporting.value;
  const disabled = !enabled || running !== null;
  return (
    <div class="flm-export-buttons">
      <button type="button" class="flm-button flm-button-primary" disabled={disabled} onClick={() => void run(profile, 'save', continuing)}>
        {running === 'save' ? 'Saving…' : name !== null ? `Save as sibling “${name}”` : 'Save as sibling'}
      </button>
      <button type="button" class="flm-button" disabled={disabled} onClick={() => void run(profile, 'download', continuing)}>
        {running === 'download' ? 'Downloading…' : 'Download'}
      </button>
    </div>
  );
}

/**
 * The result of the last export: the name written, or why nothing was written. A refusal
 * for the document's errors shows in the box instead (`ProblemsBox`).
 */
function Result({ result, disconnected }: { result: ExportResult; disconnected: boolean }) {
  const [log, setLog] = useState(false);
  // Each result starts with its log hidden.
  useEffect(() => setLog(false), [result]);
  if (result.kind === 'saved') {
    return (
      <div class="flm-export-result flm-export-saved" role="status">
        <Icon name="check" size={14} />
        <span>Successfully wrote “{result.name}”.</span>
        <button type="button" class="flm-button flm-export-open" disabled={disconnected} onClick={openExported}>
          <Icon name="external-link" size={12} />
          Open in default app
        </button>
      </div>
    );
  }
  return (
    <div class="flm-export-result flm-export-refused" role="status">
      <p>
        <span>{result.reason}</span>
        {result.log !== null && (
          <button type="button" class="flm-link" aria-expanded={log} onClick={() => setLog(!log)}>
            {log ? 'Hide log' : 'Show log'}
          </button>
        )}
      </p>
      {log && result.log !== null && <pre class="flm-export-log">{result.log}</pre>}
    </div>
  );
}

/** The heading "Terminal command", and the command with its copy button. */
function Command({ command }: { command: string }) {
  const [copied, copy] = useCopy(command);
  return (
    <section aria-label="Terminal command">
      <h3 class="flm-section-heading">Terminal command</h3>
      <div class="flm-export-command">
        <code>
          {command.split(' ').map((word, at) => (
            // Each word on one line where it fits, so that a line breaks between two words,
            // never after the hyphens of an option.
            <Fragment key={at}>
              {at > 0 && ' '}
              <span class="flm-export-word">{word}</span>
            </Fragment>
          ))}
        </code>
        <button type="button" class="flm-icon-button" aria-label="Copy the command" title="Copy the command" onClick={copy}>
          <Icon name={copied ? 'check' : 'copy'} size={15} />
        </button>
      </div>
    </section>
  );
}

/**
 * The export's name and command lines for the build type `profile`, read again when the
 * build type or the document's environment changes; `null` while they are being read.
 */
function useExportName(profile: string | null, environment: string | null, connected: boolean): Answer<ExportName> | null {
  const [answer, setAnswer] = useState<Answer<ExportName> | null>(null);
  const asked = useRef(0);
  useEffect(() => {
    const ask = ++asked.current;
    setAnswer(null);
    if (profile === null || !connected) return;
    void exportName(profile).then((read) => {
      if (ask === asked.current) setAnswer(read);
    });
  }, [profile, environment, connected]);
  return answer;
}

export function ExportTab() {
  const state = preview.value;
  const { confirmed, last, running } = exporting.value;
  const disconnected = state.connection === 'disconnected';
  const profile = chosenProfile();
  const result = resultFor(last, profile);
  const answer = useExportName(profile, state.environment, state.connection === 'connected');
  const form = exportForm(counts(state.problems), confirmed, result);
  const name = answer !== null && 'value' in answer ? answer.value : null;
  const command = name !== null ? commandShown(name, form.continuing) : null;
  return (
    <div class="flm-export">
      {/* Disabled while an export runs, so that its result shows under its own build type. */}
      <BuildType profile={profile} disabled={disconnected || running !== null} />
      {form.box !== null && <ProblemsBox box={form.box} />}
      {answer !== null && 'failure' in answer ? (
        <p class="flm-export-unavailable">{answer.failure}</p>
      ) : (
        profile !== null && (
          <Buttons profile={profile} name={name?.name ?? null} enabled={form.enabled && name !== null && !disconnected} continuing={form.continuing} />
        )
      )}
      {result !== null && !refusedForErrors(result) && <Result result={result} disconnected={disconnected} />}
      {command !== null && <Command command={command} />}
    </div>
  );
}
