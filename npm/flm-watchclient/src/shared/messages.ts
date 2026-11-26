// The messages between the two browser parts of the preview: the tools page and the page
// script inside the document's page, which the tools page shows in a frame. Neither reads
// the other's document; they exchange these messages with `window.postMessage`, and each
// accepts a message only from the other's window and the server's origin. `PROTOCOL.md`
// specifies them.

import { isObject } from './protocol';

/** The field `channel` of every message, which sets the preview's apart from others. */
export const CHANNEL = 'flm-preview';

/** A part of a source: its file, and the byte offsets of its start and of its end. */
export interface Part {
  file: string;
  start: number;
  end: number;
}

/** A rectangle in the frame's window, in CSS pixels. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The messages of the tools page to the page script. */
export type ToPage =
  | { type: 'attach' }
  | { type: 'show-place'; place: Part }
  | { type: 'show-element'; id: string }
  | { type: 'inspect'; on: boolean }
  | { type: 'inspect-place'; place: Part };

/** The messages of the page script to the tools page. */
export type ToTools =
  | { type: 'loaded'; waiting: boolean }
  | { type: 'scrolled'; x: number; y: number }
  | { type: 'inspected'; place: Part | null; rect: Rect | null; label: string | null }
  | { type: 'open-failed'; text: string }
  | { type: 'unopened'; place: string; reason: string; copied: boolean };

/** The data of a message posted to the other part. */
export function envelope<M extends ToPage | ToTools>(message: M): M & { channel: typeof CHANNEL } {
  return { channel: CHANNEL, ...message };
}

/**
 * Whether a message event comes from the window `from` and the origin `origin`: the only
 * ones a part accepts its messages from.
 */
export function isFrom(event: { source: unknown; origin: string }, from: unknown, origin: string): boolean {
  return from !== null && event.source === from && event.origin === origin;
}

function isNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function readPart(value: unknown): Part | null {
  if (!isObject(value) || typeof value.file !== 'string' || !isNumber(value.start) || !isNumber(value.end)) return null;
  return { file: value.file, start: value.start, end: value.end };
}

function readRect(value: unknown): Rect | null {
  if (!isObject(value) || !isNumber(value.x) || !isNumber(value.y) || !isNumber(value.width) || !isNumber(value.height)) {
    return null;
  }
  return { x: value.x, y: value.y, width: value.width, height: value.height };
}

/** A message to the page script, read from its data; `null` when it is none. */
export function readToPage(data: unknown): ToPage | null {
  if (!isObject(data) || data.channel !== CHANNEL) return null;
  switch (data.type) {
    case 'attach':
      return { type: 'attach' };
    case 'show-place': {
      const place = readPart(data.place);
      return place === null ? null : { type: 'show-place', place };
    }
    case 'show-element':
      return typeof data.id === 'string' ? { type: 'show-element', id: data.id } : null;
    case 'inspect':
      return typeof data.on === 'boolean' ? { type: 'inspect', on: data.on } : null;
    case 'inspect-place': {
      const place = readPart(data.place);
      return place === null ? null : { type: 'inspect-place', place };
    }
    default:
      return null;
  }
}

/** A message to the tools page, read from its data; `null` when it is none. */
export function readToTools(data: unknown): ToTools | null {
  if (!isObject(data) || data.channel !== CHANNEL) return null;
  switch (data.type) {
    case 'loaded':
      return typeof data.waiting === 'boolean' ? { type: 'loaded', waiting: data.waiting } : null;
    case 'scrolled':
      return isNumber(data.x) && isNumber(data.y) ? { type: 'scrolled', x: data.x, y: data.y } : null;
    case 'inspected': {
      const place = data.place === null ? null : readPart(data.place);
      const rect = data.rect === null ? null : readRect(data.rect);
      const label = data.label === null || typeof data.label === 'string' ? data.label : undefined;
      if ((place === null) !== (data.place === null) || (rect === null) !== (data.rect === null) || label === undefined) {
        return null;
      }
      return { type: 'inspected', place, rect, label };
    }
    case 'open-failed':
      return typeof data.text === 'string' ? { type: 'open-failed', text: data.text } : null;
    case 'unopened':
      return typeof data.place === 'string' && typeof data.reason === 'string' && typeof data.copied === 'boolean'
        ? { type: 'unopened', place: data.place, reason: data.reason, copied: data.copied } : null;
    default:
      return null;
  }
}
