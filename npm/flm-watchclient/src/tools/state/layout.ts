// The panel's place and size: the edge it is docked at and its size at each edge, within
// the limits the window leaves. The sizes asked for are kept as they were asked; the size
// shown is the one asked for, brought within the limits of the body's current size.

/** The edge of the window the panel is docked at. */
export type Side = 'right' | 'bottom';

/** The panel's size at each edge, in pixels: its width at the right, its height at the bottom. */
export type Sizes = Record<Side, number>;

/** The sizes at first. */
export const FIRST_SIZES: Sizes = { right: 408, bottom: 320 };

/** The smallest width of the panel at the right. */
export const MIN_RIGHT = 360;

/** The largest part of the body's width the panel takes at the right. */
export const MAX_RIGHT_PART = 0.6;

/** The smallest height of the panel at the bottom. */
export const MIN_BOTTOM = 160;

/** The height the output view keeps above the panel at the bottom. */
export const OUTPUT_KEPT = 160;

/** How far an arrow key moves the resize bar, in pixels. */
export const KEY_STEP = 16;

/**
 * `size` brought within the limits of `side` for a body of `width` × `height` pixels. The
 * panel's smallest size wins over the room it leaves the output view.
 */
export function limited(side: Side, size: number, width: number, height: number): number {
  const [least, most] = side === 'right' ? [MIN_RIGHT, width * MAX_RIGHT_PART] : [MIN_BOTTOM, height - OUTPUT_KEPT];
  return Math.round(Math.max(least, Math.min(most, size)));
}

/** The body's box, as the drag reads it. */
export interface Box {
  right: number;
  bottom: number;
  width: number;
  height: number;
}

/** The size the panel takes when the resize bar is dragged to the pointer at `x`, `y`. */
export function dragged(side: Side, box: Box, x: number, y: number): number {
  const size = side === 'right' ? box.right - x : box.bottom - y;
  return limited(side, size, box.width, box.height);
}

/**
 * The size after the arrow key `key` on the resize bar, from the size `size` shown; `null`
 * for a key that does not move it. The arrow toward the output view makes the panel larger.
 */
export function keyed(side: Side, key: string, size: number, width: number, height: number): number | null {
  const moves: Record<Side, Record<string, number>> = {
    right: { ArrowLeft: KEY_STEP, ArrowRight: -KEY_STEP },
    bottom: { ArrowUp: KEY_STEP, ArrowDown: -KEY_STEP },
  };
  const move = moves[side][key];
  return move === undefined ? null : limited(side, size + move, width, height);
}

/** The other edge. */
export function other(side: Side): Side {
  return side === 'right' ? 'bottom' : 'right';
}
