// A list of texts shown as chips: a chip is edited in place (a click, then Enter or leaving
// it writes the list; an emptied chip is removed), removed by its button, and added by the
// button at the end. Each change writes the whole list.

import { useRef, useState } from 'preact/hooks';
import { Icon } from './icon';

/** The chip being edited: its position (the length of the list for a new one) and its text. */
interface Editing {
  at: number;
  text: string;
}

export function TextList({ texts, disabled, name, add, onChange }: {
  texts: readonly string[];
  disabled: boolean;
  /** What the list holds, as "Authors": it names the list and its buttons. */
  name: string;
  /** The text of the button that adds a text, as "Add author". */
  add: string;
  onChange: (texts: string[]) => Promise<unknown>;
}) {
  const [editing, setEditingState] = useState<Editing | null>(null);
  // The chip being edited, read by the handlers: the text typed just before Enter is written
  // before the field has rendered it, and the field's blur after Enter writes nothing more.
  const current = useRef<Editing | null>(null);
  const setEditing = (next: Editing | null) => {
    current.current = next;
    setEditingState(next);
  };
  const finish = () => {
    const edited = current.current;
    if (edited === null) return;
    setEditing(null);
    const text = edited.text.trim();
    const next = [...texts];
    if (edited.at >= texts.length) {
      if (text === '') return;
      next.push(text);
    } else if (text === '') {
      next.splice(edited.at, 1);
    } else if (text !== texts[edited.at]) {
      next[edited.at] = text;
    } else {
      return;
    }
    void onChange(next);
  };
  const field = (at: number) => (
    <input
      type="text"
      class="flm-field flm-chip-field"
      aria-label={at >= texts.length ? `${add}: its text` : `${name}: entry ${at + 1}`}
      value={editing?.text ?? ''}
      ref={(element) => element?.focus()}
      spellcheck={false}
      onInput={(event) => setEditing({ at, text: (event.currentTarget as HTMLInputElement).value })}
      onBlur={finish}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          finish();
        } else if (event.key === 'Escape') {
          event.preventDefault();
          setEditing(null);
        }
      }}
    />
  );
  return (
    <div class="flm-chips" role="group" aria-label={name}>
      {texts.map((text, at) => (editing?.at === at ? <span key={at}>{field(at)}</span> : (
        <span key={at} class="flm-chip">
          <button type="button" class="flm-chip-text" disabled={disabled} title={`Edit ‘${text}’`} onClick={() => setEditing({ at, text })}>
            {text}
          </button>
          <button
            type="button"
            class="flm-chip-remove"
            aria-label={`Remove ‘${text}’`}
            title={`Remove ‘${text}’`}
            disabled={disabled}
            onClick={() => void onChange(texts.filter((_, other) => other !== at))}
          >
            <Icon name="x" size={12} />
          </button>
        </span>
      )))}
      {editing !== null && editing.at >= texts.length ? field(texts.length) : (
        <button type="button" class="flm-add" disabled={disabled} onClick={() => setEditing({ at: texts.length, text: '' })}>
          {add}
        </button>
      )}
    </div>
  );
}
