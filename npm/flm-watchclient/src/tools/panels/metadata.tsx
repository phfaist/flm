// The Metadata tab: the root keys of the front matter other than `flm`. The keys that the
// description gives rows of their own (`METADATA`) come first, each a text field, whose
// emptying removes its key, or a list of texts shown as chips, which the YAML editor edits
// when the value holds anything other than texts (an author written as a map), so that no
// part of it is lost; every other root key is a row with its key, a text field for a text
// and the YAML editor otherwise, and a button that removes the key; *Add a key* asks for the
// name, then opens the YAML editor of the new key. No row has a revert icon.

import { useState } from 'preact/hooks';
import type { SettingsState } from '../../shared/settings';
import { FLM_KEY } from '../../shared/settings';
import { canWrite, useSettings, write } from '../front-matter';
import { TextField } from '../controls/fields';
import { Icon } from '../controls/icon';
import { CheckSlot, Row, useSaved } from '../controls/row';
import { TextList } from '../controls/texts';
import { EditAsYaml, YamlEditor } from '../controls/yaml';
import type { MetadataRow } from '../state/description';
import { METADATA } from '../state/description';
import type { Edit } from '../state/settings';
import { changed, metadataControl, preview, reverted, savedYaml, textEdit, textsEdit, textsOf } from '../state/settings';
import { panel } from '../state/store';

/** The root key that the Settings tab shows, which *Add a key* does not add. */
const SETTINGS_KEY = FLM_KEY;

/** The root keys that have rows of their own; `flm` is the Settings tab's. */
const OWN_KEYS = [...METADATA.map((row) => row.key), SETTINGS_KEY];

/** A text of the front matter as a text field shows it: a number or a boolean as written. */
function textOf(value: unknown): string {
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean' ? String(value) : '';
}

/**
 * The row of its own of a root key, as the description gives it, or *Edit as YAML* where
 * `metadataControl` says so.
 */
function OwnRow({ row, value, disabled }: { row: MetadataRow; value: unknown; disabled: boolean }) {
  const [saved, run] = useSaved();
  const [editing, setEditing] = useState(false);
  const path = [row.key];
  const send = (edit: Edit) => run(write([edit]));
  if (metadataControl(row, value) === 'yaml') {
    return (
      <Row
        labels="metadata"
        label={row.label}
        named={false}
        saved={saved}
        control={<EditAsYaml preview={preview(value)} open={editing} disabled={disabled} onOpen={() => setEditing(true)} />}
        below={editing && (
          <YamlEditor path={path} disabled={disabled} onSave={(yaml) => send(savedYaml(path, yaml))} onClose={() => setEditing(false)} />
        )}
      />
    );
  }
  const control = row.kind === 'text'
    ? <TextField value={textOf(value)} placeholder={row.placeholder} disabled={disabled} onCommit={(text) => send(textEdit(path, text))} />
    : <TextList texts={textsOf(value)} disabled={disabled} name={row.label} add={row.add} onChange={(texts) => send(textsEdit(path, value, texts))} />;
  return <Row labels="metadata" label={row.label} named={row.kind === 'text'} saved={saved} control={control} />;
}

/** The button at the right end of a row of another key, which removes the key. */
function RemoveButton({ name, disabled, onRemove }: { name: string; disabled: boolean; onRemove: () => void }) {
  const label = `Remove the key ‘${name}’`;
  return (
    <button type="button" class="flm-icon-button flm-row-end" aria-label={label} title={label} disabled={disabled} onClick={onRemove}>
      <Icon name="trash-2" size={14} />
    </button>
  );
}

/** The row of another root key: a text field for a text, *Edit as YAML* otherwise. */
function OtherRow({ name, value, disabled }: { name: string; value: unknown; disabled: boolean }) {
  const [saved, run] = useSaved();
  const [editing, setEditing] = useState(false);
  const send = (edit: Edit) => run(write([edit]));
  const text = typeof value === 'string';
  return (
    <Row
      labels="metadata"
      label={<code>{name}</code>}
      named={text}
      saved={saved}
      control={text
        ? <TextField value={value} disabled={disabled} onCommit={(written) => send(changed([name], written))} />
        : <EditAsYaml preview={preview(value)} open={editing} disabled={disabled} onOpen={() => setEditing(true)} />}
      end={<RemoveButton name={name} disabled={disabled} onRemove={() => void send(reverted([name]))} />}
      below={editing && (
        <YamlEditor path={[name]} disabled={disabled} onSave={(yaml) => send(savedYaml([name], yaml))} onClose={() => setEditing(false)} />
      )}
    />
  );
}

/** Why *Add a key* cannot add the key `name` to the front matter `data`, or `null` when it can. */
function refusalOf(name: string, data: Record<string, unknown>): string | null {
  if (Object.hasOwn(data, name)) return `Duplicate key ‘${name}’`;
  if (name === SETTINGS_KEY) return `Reserved key ‘${name}’: its settings are in the Settings tab`;
  return null;
}

/**
 * *Add a key*: the name of the key asked for, then the YAML editor of its value; the green
 * check beside the button once the key is written.
 */
function AddKey({ data, disabled }: { data: Record<string, unknown>; disabled: boolean }) {
  const [saved, run] = useSaved();
  const [naming, setNaming] = useState<string | null>(null);
  const [named, setNamed] = useState<string | null>(null);
  if (named !== null) {
    return (
      <YamlEditor path={[named]} fresh disabled={disabled} onSave={(yaml) => run(write([savedYaml([named], yaml)]))} onClose={() => setNamed(null)} />
    );
  }
  if (naming === null) {
    return (
      <div class="flm-row-line">
        <button type="button" class="flm-add flm-add-key" disabled={disabled} onClick={() => setNaming('')}>Add a key</button>
        <CheckSlot saved={saved} />
      </div>
    );
  }
  const name = naming.trim();
  const refusal = name === '' ? null : refusalOf(name, data);
  const add = () => {
    if (name === '' || refusal !== null) return;
    setNaming(null);
    setNamed(name);
  };
  return (
    <div class="flm-add-key-form">
      <div class="flm-row-line">
        <input
          type="text"
          class="flm-field flm-text-field"
          aria-label="The name of the key"
          placeholder="Name of the key"
          value={naming}
          spellcheck={false}
          ref={(element) => element?.focus()}
          onInput={(event) => setNaming((event.currentTarget as HTMLInputElement).value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              add();
            } else if (event.key === 'Escape') {
              event.preventDefault();
              setNaming(null);
            }
          }}
        />
        <button type="button" class="flm-button" onClick={() => setNaming(null)}>Cancel</button>
        <button type="button" class="flm-button flm-button-primary" disabled={disabled || name === '' || refusal !== null} onClick={add}>Add</button>
      </div>
      {refusal !== null && <p class="flm-row-note">{refusal}</p>}
    </div>
  );
}

/** The line that says the front matter cannot be read, with a link to Problems. */
export function Unreadable({ reason }: { reason: string }) {
  return (
    <p class="flm-front-matter-unreadable">
      <span>{reason}</span>{' '}
      <button type="button" class="flm-link" onClick={() => (panel.value = { ...panel.value, tab: 'problems' })}>Show the problems</button>
    </p>
  );
}

function Fields({ state }: { state: SettingsState }) {
  const disabled = !canWrite(state);
  const data = state.frontMatter ?? {};
  const others = Object.keys(data).filter((key) => !OWN_KEYS.includes(key));
  return (
    <>
      {METADATA.map((row) => (
        <OwnRow key={row.key} row={row} value={data[row.key]} disabled={disabled} />
      ))}
      <h3 class="flm-section-heading">Other keys</h3>
      {others.map((key) => (
        <OtherRow key={key} name={key} value={data[key]} disabled={disabled} />
      ))}
      <AddKey data={data} disabled={disabled} />
    </>
  );
}

export function MetadataTab() {
  const state = useSettings();
  if (state === null) return <p class="flm-panel-line">Reading the front matter…</p>;
  return (
    <div class="flm-rows flm-metadata">
      {state.unreadable !== null && <Unreadable reason={state.unreadable} />}
      <Fields state={state} />
    </div>
  );
}
