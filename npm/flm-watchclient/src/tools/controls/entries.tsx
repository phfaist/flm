// The controls of several entries: a group of checkboxes, each one entry of a map (a check
// writes that entry), and an ordered list of names with the names in use that it does not
// hold and the names that can be added. A name
// of the list moves by its handle, dragged with the pointer or moved by the up and down
// arrow keys, and the drop writes the order once; a name that is not added by hand is
// offered without *Add*. The row that holds one of these decides what a change writes.

import type { ComponentChildren } from 'preact';
import { useLayoutEffect, useRef, useState } from 'preact/hooks';
import { Icon } from './icon';

/**
 * A checkbox of a group: the entry it stands for, its label, whether it is checked, and
 * whether it cannot be changed apart from the group.
 */
export interface Checkbox {
  entry: string;
  label: ComponentChildren;
  checked: boolean;
  disabled?: boolean;
}

export function CheckboxGroup({ boxes, disabled, name, onChange }: {
  boxes: readonly Checkbox[];
  disabled: boolean;
  name: string;
  onChange: (entry: string, checked: boolean) => void;
}) {
  return (
    <div class="flm-checkboxes" role="group" aria-label={name}>
      {boxes.map((box) => (
        <label key={box.entry} class="flm-checkbox">
          <input
            type="checkbox"
            checked={box.checked}
            disabled={disabled || box.disabled === true}
            onChange={(event) => onChange(box.entry, (event.currentTarget as HTMLInputElement).checked)}
          />
          <span>{box.label}</span>
        </label>
      ))}
    </div>
  );
}

/**
 * A name that can be added, with its description or `null`; one not added by hand
 * (`byHand` false) is listed without the button *Add*.
 */
export interface Addable {
  name: string;
  description: string | null;
  byHand: boolean;
}

/** A name in use that the list does not hold, which cannot be removed, and why it is in use. */
export interface LockedName {
  name: string;
  why: string;
}

/** Where a row dragged by its handle lands: the row taken, and the place before which it goes (`names.length` for the end). */
interface Drag {
  from: number;
  to: number;
}

/** `names` once the name at `from` moves to the place before the name at `to` (`names.length` for the end). */
function reordered(names: readonly string[], from: number, to: number): string[] {
  const next = [...names];
  const [taken] = next.splice(from, 1);
  next.splice(to > from ? to - 1 : to, 0, taken!);
  return next;
}

/** Whether a row dragged from `from` and dropped before `to` changes the order. */
function moves({ from, to }: Drag): boolean {
  return to !== from && to !== from + 1;
}

/**
 * The name and the description of a row of a list of names: the description, when there is
 * one, on a line of its own under the name, cut to that line, with the whole description as
 * its tooltip.
 */
function NameText({ name, description }: { name: string; description: string | null }) {
  return (
    <span class="flm-name-body">
      <span class="flm-name-text">{name}</span>
      {description !== null && <span class="flm-name-description" title={description}>{description}</span>}
    </span>
  );
}

/**
 * An ordered list of names, such as the packages in use: each name can be removed, and
 * moves with its handle: dragged, a line showing where it lands, or with the up and down
 * arrow keys while the handle has the focus, which stays on it. Under it, the names in use
 * that the list does not hold (`locked`), in a list of the same look without the handles,
 * each with a padlock where the other list has the remove button, whose tooltip says why
 * the name is in use. Under the label "Available", the names that can be added, in the
 * order given, in a list of that look, each with a button *Add* where the first list has
 * the remove button when it is added by hand. A row of any list shows the name's
 * description under it. Then `note`, when given; then a text field that adds a name typed
 * in it. Each change gives the whole list.
 */
export function OrderedNames({ names, describe, locked = [], available, disabled, name, other, note, onChange }: {
  names: readonly string[];
  /** The description of a name in the lists, or `null`. */
  describe: (name: string) => string | null;
  locked?: readonly LockedName[];
  available: readonly Addable[];
  disabled: boolean;
  /** What the list holds, as "Packages": it names the list. */
  name: string;
  /** The placeholder and name of the field that adds a name typed in it, as "Another package, by name". */
  other: string;
  /** A note under the lists. */
  note?: ComponentChildren;
  /** Writes the list; what it gives settles once the list is written, with `false` when it is not. */
  onChange: (names: string[]) => Promise<boolean> | void;
}) {
  const [typed, setTypedState] = useState('');
  // The text typed, read by the handlers: the text typed just before Enter is added before
  // the field has rendered it.
  const current = useRef('');
  const setTyped = (text: string) => {
    current.current = text;
    setTypedState(text);
  };
  const list = useRef<HTMLOListElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  // The name whose handle keeps the focus after a move by the keys, and the order it moved
  // from: once the list shows another order, the focus goes back to the handle, which the
  // new order may have taken out of the page and put back.
  const keyed = useRef<{ name: string; order: string } | null>(null);
  // Whether a move is being written: the keys move a row once the last move is written.
  const writing = useRef(false);
  const order = names.join('\n');
  useLayoutEffect(() => {
    const wanted = keyed.current;
    if (wanted === null || wanted.order === order) return;
    keyed.current = null;
    const handle = [...(list.current?.querySelectorAll<HTMLElement>('[data-handle]') ?? [])].find((each) => each.dataset.handle === wanted.name);
    if (handle !== undefined && document.activeElement !== handle) handle.focus();
  }, [order]);
  const write = (next: string[]) => {
    const written = onChange(next);
    if (written === undefined) return;
    writing.current = true;
    const settled = (done: boolean) => {
      writing.current = false;
      // A move not written leaves the order as it is: no focus to give back.
      if (!done) keyed.current = null;
    };
    void written.then(settled, () => settled(false));
  };
  const keyMoved = (at: number, by: number) => {
    if (writing.current || at + by < 0 || at + by >= names.length) return;
    keyed.current = { name: names[at]!, order };
    write(reordered(names, at, by > 0 ? at + 2 : at - 1));
  };
  /** The place before which a row dragged at the height `y` lands: before the first row whose middle is below it. */
  const placeAt = (y: number) => {
    const rows = [...(list.current?.children ?? [])];
    const at = rows.findIndex((row) => {
      const box = row.getBoundingClientRect();
      return y < box.top + box.height / 2;
    });
    return at === -1 ? rows.length : at;
  };
  const dropped = () => {
    if (drag !== null && moves(drag)) write(reordered(names, drag.from, drag.to));
    setDrag(null);
  };
  const added = (each: string) => {
    if (each !== '' && !names.includes(each)) void onChange([...names, each]);
  };
  const addTyped = () => {
    added(current.current.trim());
    setTyped('');
  };
  const addable = available.filter((each) => !names.includes(each.name) && !locked.some((kept) => kept.name === each.name));
  // The line that shows where the row dragged lands: above the row at `to`, or below the last.
  const line = (at: number) => {
    if (drag === null || !moves(drag)) return undefined;
    if (drag.to === at) return 'before';
    return drag.to === names.length && at === names.length - 1 ? 'after' : undefined;
  };
  return (
    <div class="flm-names" role="group" aria-label={name}>
      {names.length > 0 && (
        <ol ref={list} class="flm-names-list">
          {names.map((each, at) => {
            const description = describe(each);
            return (
              <li key={each} class="flm-name" data-dragged={drag?.from === at ? 'true' : undefined} data-drop={line(at)}>
                <button
                  type="button"
                  class="flm-icon-button flm-name-handle"
                  data-handle={each}
                  aria-label={`Move ‘${each}’`}
                  title="Drag to move, or use the up and down arrow keys"
                  disabled={disabled || names.length < 2}
                  onPointerDown={(event) => {
                    if (event.button !== 0) return;
                    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
                    setDrag({ from: at, to: at });
                  }}
                  onPointerMove={(event) => {
                    if (drag === null) return;
                    const to = placeAt(event.clientY);
                    if (to !== drag.to) setDrag({ ...drag, to });
                  }}
                  onPointerUp={dropped}
                  onPointerCancel={() => setDrag(null)}
                  onKeyDown={(event) => {
                    const by = event.key === 'ArrowUp' ? -1 : event.key === 'ArrowDown' ? 1 : 0;
                    if (by === 0) return;
                    event.preventDefault();
                    keyMoved(at, by);
                  }}
                >
                  <Icon name="grip-vertical" size={13} />
                </button>
                <NameText name={each} description={description} />
                <button type="button" class="flm-icon-button" aria-label={`Remove ‘${each}’`} title="Remove" disabled={disabled} onClick={() => void onChange(names.filter((kept) => kept !== each))}>
                  <Icon name="x" size={13} />
                </button>
              </li>
            );
          })}
        </ol>
      )}
      {locked.length > 0 && (
        <ul class="flm-names-list flm-locked-list">
          {locked.map((each) => (
            <li key={each.name} class="flm-name">
              <NameText name={each.name} description={describe(each.name)} />
              <span class="flm-name-lock" role="img" aria-label={each.why} title={each.why}>
                <Icon name="lock" size={13} />
              </span>
            </li>
          ))}
        </ul>
      )}
      {addable.length > 0 && (
        <>
          <span class="flm-names-available">Available</span>
          <ul class="flm-names-list flm-addable-list">
            {addable.map((each) => (
              <li key={each.name} class="flm-name">
                <NameText name={each.name} description={each.description} />
                {each.byHand && <button type="button" class="flm-button" aria-label={`Add ‘${each.name}’`} disabled={disabled} onClick={() => added(each.name)}>Add</button>}
              </li>
            ))}
          </ul>
        </>
      )}
      {note !== undefined && <p class="flm-names-note">{note}</p>}
      <div class="flm-names-other">
        <input
          type="text"
          class="flm-field flm-text-field"
          aria-label={other}
          placeholder={other}
          spellcheck={false}
          disabled={disabled}
          value={typed}
          onInput={(event) => setTyped((event.currentTarget as HTMLInputElement).value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              addTyped();
            }
          }}
        />
        <button type="button" class="flm-button" disabled={disabled || typed.trim() === ''} onClick={addTyped}>Add</button>
      </div>
    </div>
  );
}
