// Inspect: the status bar's toggle and the card drawn over the frame. While Inspect is on,
// the page script takes the clicks on the document, outlines the element clicked and
// reports its place, its rectangle and the label that refers to it (`inspected`). The tools
// page asks the server for the constructs at the start of that place and draws the card
// beside the element: of the constructs that contain the whole place, the innermost as
// written and what it is, and the others around it (each inspected in turn when clicked),
// the place, Open in editor, Help, the copy of the code that refers to the element's label,
// and the problems on the element. The card closes on Escape, on a click elsewhere, when the
// document scrolls, changes or is resized, and when Inspect is turned off.

import { signal } from '@preact/signals';
import { useEffect, useLayoutEffect, useRef } from 'preact/hooks';
import type { ConstructsAt } from '../shared/help';
import type { Part, Rect, ToTools } from '../shared/messages';
import { CopyRef } from './controls/copy-ref';
import { Icon } from './controls/icon';
import { showHelp } from './help';
import { SEVERITY_ICONS } from './panels/problems';
import type { Answer } from './requests';
import { constructsAt, openPlace } from './requests';
import { view } from './state/build';
import { asWritten, cardPosition, constructsOf, problemsOn, whatItIs } from './state/inspect';
import { rowsOf } from './state/problems';
import { inspecting, preview } from './state/store';
import { toPage } from './views/channel';

/** The card: the part inspected, the element's rectangle and label, and the answer of `at`. */
interface Card {
  /** Which selection the card shows; each selection reported by the page has its own. */
  id: number;
  place: Part;
  rect: Rect;
  label: string | null;
  /** The constructs at the start of `place`, or `null` while they are read. */
  answer: Answer<ConstructsAt> | null;
}

/** The card, while it is open. */
const card = signal<Card | null>(null);

/** The number of the last selection reported. */
let selections = 0;

/** Closes the card; the page script then leaves no element selected. */
function closeCard(): void {
  if (card.value === null) return;
  card.value = null;
  toPage({ type: 'inspect', on: inspecting.value });
}

/** Turns Inspect on or off. */
function setInspecting(on: boolean): void {
  inspecting.value = on;
  card.value = null;
  toPage({ type: 'inspect', on });
}

/** Opens the card on a selection reported by the page script, and reads the constructs there. */
function open(place: Part, rect: Rect | null, label: string | null): void {
  const id = ++selections;
  // A construct around the one inspected that no element contains keeps the card where it is.
  const at = rect ?? card.value?.rect ?? { x: 0, y: 0, width: 0, height: 0 };
  card.value = { id, place, rect: at, label, answer: null };
  void constructsAt(place.file, place.start).then((answer) => {
    if (card.value?.id === id) card.value = { ...card.value, answer };
  });
}

/** The frame loaded a page: no element of it is selected, and Inspect is turned on in it again. */
export function inspectLoaded(): void {
  card.value = null;
  if (inspecting.value) toPage({ type: 'inspect', on: true });
}

/** The page scrolled: the element's rectangle is out of date. */
export function inspectScrolled(): void {
  closeCard();
}

/** The page reported a selection (`inspected`), or that nothing is selected. */
export function inspectSelected(message: Extract<ToTools, { type: 'inspected' }>): void {
  if (message.place === null) card.value = null;
  else open(message.place, message.rect, message.label);
}

/** The frame is gone: Inspect is off, with nothing to inspect. */
export function frameClosed(): void {
  inspecting.value = false;
  card.value = null;
}

/** The status bar's Inspect: a toggle, greyed out when the output is not an HTML page. */
export function InspectButton() {
  const html = view(preview.value).kind === 'frame';
  const pressed = html && inspecting.value;
  return (
    <button
      type="button"
      class="flm-inspect"
      aria-pressed={pressed}
      aria-disabled={!html}
      title={html ? 'Inspect' : 'Inspect works on an HTML page'}
      onClick={() => {
        if (html) setInspecting(!inspecting.value);
      }}
    >
      <Icon name="mouse-pointer-2" size={14} />
      <span class="flm-words">Inspect</span>
    </button>
  );
}

/** The card's content once the constructs are read. */
function CardContent({ selection, answer }: { selection: Card; answer: ConstructsAt }) {
  const { place, label } = selection;
  const [inner, ...outer] = constructsOf(answer.constructs, place);
  const disconnected = preview.value.connection === 'disconnected';
  const help = inner !== undefined && preview.value.help ? inner.help : null;
  // Problems alike share one item, as they share one row in Problems.
  const problems = rowsOf(problemsOn(preview.value.problems, place))
    .flatMap((group) => group.rows.map((row) => ({ key: row.key, problem: row.problems[0]! })));
  return (
    <>
      {inner !== undefined && (
        <div class="flm-inspect-head">
          <code class="flm-inspect-name">{asWritten(inner)}</code>
          <span class="flm-inspect-what">{whatItIs(inner)}</span>
        </div>
      )}
      <div class="flm-inspect-around">
        {outer.length > 0 && <span>Inside:</span>}
        {outer.map((construct) => (
          <button
            key={`${construct.start}-${construct.end}-${construct.name}`}
            type="button"
            class="flm-link flm-inspect-outer"
            onClick={() => toPage({ type: 'inspect-place', place: { file: construct.file, start: construct.start, end: construct.end } })}
          >
            {asWritten(construct)}
          </button>
        ))}
        <span class="flm-inspect-place">{`${place.file}:${answer.line}:${answer.column}`}</span>
      </div>
      <div class="flm-inspect-actions">
        <button type="button" class="flm-button flm-button-primary" disabled={disconnected} onClick={() => void openPlace({ file: place.file, start: place.start })}>
          Open in editor
        </button>
        {help !== null && (
          <button type="button" class="flm-button" onClick={() => showHelp(help)}>
            Help
          </button>
        )}
        {label !== null && <CopyRef label={label} className="flm-button flm-inspect-copy" />}
      </div>
      {problems.length > 0 && (
        <ul class="flm-inspect-problems">
          {problems.map(({ key, problem }) => (
            <li key={key} class={`flm-tone-${problem.severity}`}>
              <Icon name={SEVERITY_ICONS[problem.severity].icon} size={16} label={SEVERITY_ICONS[problem.severity].label} />
              <span>{problem.message}</span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

/**
 * The card, over the frame, beside the element inspected and inside the frame's window;
 * `room` is the output view, the frame's place. The card takes the focus when it opens on
 * a selection, so that Escape and the Tab key reach it.
 */
export function InspectCard({ room }: { room: () => HTMLElement | null }) {
  const element = useRef<HTMLDivElement>(null);
  const current = card.value;
  const build = preview.value.build;
  const opened = useRef<number | null>(null);

  // The document changed: the element inspected may be gone.
  const built = useRef(build);
  useEffect(() => {
    if (built.current !== build) closeCard();
    built.current = build;
  }, [build]);

  useLayoutEffect(() => {
    const box = element.current;
    const output = room();
    if (current === null || box === null || output === null) return;
    const { left, top } = cardPosition(
      current.rect,
      { width: box.offsetWidth, height: box.offsetHeight },
      { width: output.clientWidth, height: output.clientHeight },
    );
    box.style.left = `${left}px`;
    box.style.top = `${top}px`;
    if (opened.current !== current.id) {
      opened.current = current.id;
      box.focus({ preventScroll: true });
    }
  });

  // While the card is open: Escape and a click elsewhere close it, and so does a change of
  // the output view's size, which moves the document's elements.
  const open = current !== null;
  useEffect(() => {
    if (!open) return undefined;
    const output = room();
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      closeCard();
      document.querySelector<HTMLElement>('.flm-inspect')?.focus();
    };
    const onPointer = (event: PointerEvent) => {
      if (!(event.target instanceof Node && element.current?.contains(event.target))) closeCard();
    };
    const size = output === null ? null : { width: output.clientWidth, height: output.clientHeight };
    const observer = new ResizeObserver(() => {
      if (output !== null && size !== null && (output.clientWidth !== size.width || output.clientHeight !== size.height)) closeCard();
    });
    if (output !== null) observer.observe(output);
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer, true);
    return () => {
      observer.disconnect();
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer, true);
    };
  }, [open]);

  if (current === null) return null;
  const answer = current.answer;
  return (
    <div ref={element} class="flm-inspect-card" role="dialog" aria-label="Inspect" tabIndex={-1}>
      {answer === null ? null : 'value' in answer ? (
        <CardContent selection={current} answer={answer.value} />
      ) : (
        <p class="flm-inspect-failure">{answer.failure}</p>
      )}
    </div>
  );
}
