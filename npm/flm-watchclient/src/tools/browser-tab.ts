// The browser tab of the tools page: its title names the document, and its icon draws the
// state of the preview (building, errors, warnings, notes, none, disconnected). The help
// browser alone has an icon of its own.

import { effect } from '@preact/signals';
import type { IconName } from './controls/icon';
import { svg } from './controls/icon';
import type { Mark } from './state/build';
import { mark, title } from './state/build';
import { preview } from './state/store';

/**
 * The icon and its color for each state. The colors are the light set's: a browser draws
 * the tab's icon on a bar of its own, which the page's colors do not reach.
 */
const MARKS: Record<Mark, [IconName, string]> = {
  connecting: ['rotate-cw', '#5b6470'],
  building: ['rotate-cw', '#1f5fd1'],
  errors: ['circle-x', '#b3261e'],
  warnings: ['triangle-alert', '#8a5a00'],
  notes: ['info', '#5b6470'],
  none: ['circle-check', '#1a7f37'],
  disconnected: ['unplug', '#5b6470'],
};

/** The address of the icon `name` in `color`, drawn for a browser tab. */
function address(name: IconName, color: string): string {
  return `data:image/svg+xml,${encodeURIComponent(svg(name, 24, color))}`;
}

/** The address of the tab's icon for `state`. */
export function iconAddress(state: Mark): string {
  const [name, color] = MARKS[state];
  return address(name, color);
}

/** Adds the element that names the tab's icon. */
function iconLink(): HTMLLinkElement {
  const link = document.createElement('link');
  link.rel = 'icon';
  link.type = 'image/svg+xml';
  document.head.append(link);
  return link;
}

/** Gives the help browser alone its icon, so that the browser asks for no other. */
export function showHelpIcon(): void {
  iconLink().href = address('circle-help', '#1f5fd1');
}

/** Keeps the tab's title and icon in step with the state. */
export function followState(): void {
  const link = iconLink();
  effect(() => {
    const state = preview.value;
    document.title = title(state);
    const wanted = iconAddress(mark(state));
    if (link.href !== wanted) link.href = wanted;
  });
}
