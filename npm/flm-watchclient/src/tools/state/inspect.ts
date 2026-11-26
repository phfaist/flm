// The Inspect card's pure parts: the constructs of the part inspected, how a construct reads
// in the card, the problems on the part, and where the card is drawn over the frame.

import type { Construct } from '../../shared/help';
import type { Part, Rect } from '../../shared/messages';
import type { Problem } from '../../shared/protocol';
import { partOf } from '../../shared/protocol';

/** A construct as the author wrote it: `\emph`, `\begin{theorem}`, or the characters of a special. */
export function asWritten(construct: Construct): string {
  switch (construct.kind) {
    case 'macro':
      return `\\${construct.name}`;
    case 'environment':
      return `\\begin{${construct.name}}`;
    default:
      return construct.name;
  }
}

const KIND_WORDS: Record<string, string> = {
  macro: 'command',
  environment: 'environment',
  specials: 'special character sequence',
};

/** What a construct is: its kind, and the feature that declares it ("environment of the feature theorems"). */
export function whatItIs(construct: Construct): string {
  const kind = KIND_WORDS[construct.kind] ?? construct.kind;
  return construct.feature === null ? `${kind} that no feature declares` : `${kind} of the feature ${construct.feature}`;
}

/**
 * Of `constructs` (the answer of `at` at the start of `place`, innermost first), those that
 * contain the whole of `place`, innermost first: the first is the construct of the element
 * inspected, the others are around it. A construct that starts at `place` but ends inside
 * it, such as a command at the start of a paragraph, is inside the element, not the element.
 */
export function constructsOf(constructs: readonly Construct[], place: Part): Construct[] {
  return constructs.filter((construct) =>
    construct.file === place.file && construct.start <= place.start && place.end <= construct.end);
}

/** The end of `part`, a part that covers no bytes counting as covering the byte at its start. */
function endOf(part: Part): number {
  return Math.max(part.end, part.start + 1);
}

/** The problems of `problems` whose part of a source overlaps `place`, in their order. */
export function problemsOn(problems: readonly Problem[], place: Part): Problem[] {
  return problems.filter((problem) => {
    const part = partOf(problem);
    return part !== null && part.file === place.file && part.start < endOf(place) && place.start < endOf(part);
  });
}

/** The space between the card and the element, and between the card and the edges. */
export const CARD_GAP = 8;

/**
 * Where the card of size `card` is drawn, in a room of size `room` (the frame's window),
 * for the element at `rect`: under the element where the card fits there, else above it,
 * else on the side with more room, as near the element as the room allows; at the
 * element's left edge, moved left as much as needed to stay inside the room.
 */
export function cardPosition(
  rect: Rect,
  card: { width: number; height: number },
  room: { width: number; height: number },
): { left: number; top: number } {
  const within = (value: number, size: number, limit: number) =>
    Math.max(CARD_GAP, Math.min(value, limit - size - CARD_GAP));
  const left = within(rect.x, card.width, room.width);
  const below = rect.y + rect.height + CARD_GAP;
  const above = rect.y - CARD_GAP - card.height;
  if (below + card.height <= room.height - CARD_GAP) return { left, top: Math.max(CARD_GAP, below) };
  if (above >= CARD_GAP) return { left, top: above };
  // It fits on neither side: on the side with more room, as near the element as it fits.
  return rect.y > room.height - (rect.y + rect.height)
    ? { left, top: CARD_GAP }
    : { left, top: within(below, card.height, room.height) };
}
