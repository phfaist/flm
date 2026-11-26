// The controls of one value: a switch, a text field, a number field and a choice. A text or
// number field writes when the author leaves it or presses Enter; a switch and a choice at
// the change. A field that the author is editing keeps its text when the value it shows
// changes meanwhile (the settings state read again).

import { useRef, useState } from 'preact/hooks';
import type { Option } from '../state/settings';
import { sameValue } from '../state/settings';

/** A switch: a button with the role `switch`, which writes the other value at a click. */
export function Switch({ checked, disabled, name, small = false, onChange }: {
  checked: boolean;
  disabled: boolean;
  /** The switch's accessible name, when no `<label>` around it gives one. */
  name?: string;
  /** The smaller switch of a feature's row. */
  small?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      class={small ? 'flm-switch flm-switch-small' : 'flm-switch'}
      aria-checked={checked}
      aria-label={name}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span class="flm-switch-knob" />
    </button>
  );
}

/**
 * The text that a field shows and its handlers, given the value it shows and the function
 * that writes a text: the author's text while the field has the focus (the draft), then the
 * text written until the write is answered, then the value. The draft is also kept in a
 * reference, which the handlers read, so that Enter writes the text typed just before it,
 * before the field has rendered that text.
 */
function useDraft(value: string, commit: (text: string) => Promise<unknown>) {
  const [draft, setDraft] = useState<string | null>(null);
  const typed = useRef<string | null>(null);
  const [sent, setSent] = useState<string | null>(null);
  const change = (text: string | null) => {
    typed.current = text;
    setDraft(text);
  };
  const send = () => {
    const text = typed.current;
    change(null);
    if (text === null || text === value) return;
    setSent(text);
    void commit(text).finally(() => setSent(null));
  };
  return {
    value: draft ?? sent ?? value,
    onInput: (event: Event) => change((event.currentTarget as HTMLInputElement).value),
    onBlur: send,
    onKeyDown: (event: KeyboardEvent) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        send();
      } else if (event.key === 'Escape' && typed.current !== null) {
        event.preventDefault();
        change(null);
      }
    },
  };
}

/** A text field, which writes its text (`onCommit`) when the author leaves it or presses Enter. */
export function TextField({ value, disabled, name, placeholder, onCommit }: {
  value: string;
  disabled: boolean;
  name?: string;
  placeholder?: string;
  onCommit: (text: string) => Promise<unknown>;
}) {
  const draft = useDraft(value, onCommit);
  return (
    <input type="text" class="flm-field flm-text-field" aria-label={name} placeholder={placeholder} disabled={disabled} spellcheck={false} {...draft} />
  );
}

/**
 * A number field, 68 px wide, which writes its number when the author leaves it or presses
 * Enter; a text that is not a number (and an emptied field) writes nothing.
 */
export function NumberField({ value, integer, disabled, name, onCommit }: {
  value: number | null;
  integer: boolean;
  disabled: boolean;
  name?: string;
  onCommit: (value: number) => Promise<unknown>;
}) {
  const draft = useDraft(value === null ? '' : String(value), async (text) => {
    const number = Number(text);
    if (text.trim() !== '' && Number.isFinite(number)) await onCommit(number);
  });
  return (
    <input type="number" class="flm-field flm-number-field" aria-label={name} step={integer ? 1 : 'any'} disabled={disabled} {...draft} />
  );
}

/** The option "Custom…" of a choice: whether it is the option shown, and what choosing it does. */
export interface CustomOption {
  shown: boolean;
  onChoose: () => void;
}

/**
 * A choice among `options`, which writes the value of the option chosen; an option is the
 * value shown when it holds the same data. A value shown that is none of the options is
 * shown as one more option; with `custom`, the last option is "Custom…", shown in its place.
 */
export function Choice({ value, options, disabled, name, custom, onChange }: {
  value: unknown;
  options: readonly Option[];
  disabled: boolean;
  name?: string;
  custom?: CustomOption;
  onChange: (value: unknown) => void;
}) {
  const known = options.findIndex((option) => sameValue(option.value, value));
  const extra = custom === undefined && known < 0 && value !== undefined && value !== null;
  const all = extra ? [...options, { label: String(value), value }] : options;
  const customAt = all.length;
  const index = custom?.shown ? customAt : known >= 0 ? known : extra ? all.length - 1 : -1;
  return (
    <select
      class="flm-field flm-choice"
      aria-label={name}
      disabled={disabled}
      value={String(index)}
      onChange={(event) => {
        const at = Number((event.currentTarget as HTMLSelectElement).value);
        if (custom !== undefined && at === customAt) {
          custom.onChoose();
          return;
        }
        const chosen = all[at];
        if (chosen !== undefined && (custom?.shown || !sameValue(chosen.value, value))) onChange(chosen.value);
      }}
    >
      {index < 0 && <option value="-1" disabled />}
      {all.map((option, at) => (
        <option key={at} value={String(at)} disabled={option.disabled}>{option.label}</option>
      ))}
      {custom !== undefined && <option value={String(customAt)}>Custom…</option>}
    </select>
  );
}
