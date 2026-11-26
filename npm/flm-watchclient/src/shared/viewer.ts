// The viewer settings: how the author wants the preview shown, whatever the document. The
// field `viewer` of `hello` and the message `viewer` give them; `POST /.flm-preview/viewer`
// changes them. `PROTOCOL.md` specifies them. The tools page reads them all; the page
// script reads only whether it scrolls to changes.

import { isObject } from './protocol';

/** The request that changes the viewer settings. */
export const VIEWER = '/.flm-preview/viewer';

/** The tools page's themes: light or dark as the system is set, light, dark. */
export const THEMES = ['system', 'light', 'dark'] as const;

export type Theme = (typeof THEMES)[number];

/** The edges of the tools page at which its panel is docked. */
export const PANEL_SIDES = ['right', 'bottom'] as const;

export type PanelSide = (typeof PANEL_SIDES)[number];

/** An editor that the server knows. */
export interface Editor {
  /** The name a request gives, such as `code` or `custom`. */
  name: string;
  /** The name the author reads, such as "Visual Studio Code". */
  label: string;
  /** Whether the server can open a place in it: it was found, or, for `custom`, an editor command is given. */
  available: boolean;
}

/** The viewer settings. */
export interface ViewerSettings {
  /** The `name` of the editor in which a place opens, or `null` when there is none. */
  editor: string | null;
  theme: Theme;
  scrollToChanges: boolean;
  openProblemsOnErrors: boolean;
  wrapLines: boolean;
  panelSide: PanelSide;
  /** The panel's width at the right, in pixels, or `null` for the tools page's own. */
  panelWidth: number | null;
  /** The panel's height at the bottom, in pixels, or `null` for the tools page's own. */
  panelHeight: number | null;
  /** Why the file of the settings cannot be read, or `null`; meanwhile every change is refused. */
  unreadable: string | null;
}

/** The settings before the server gave any: the defaults, with no editor. */
export const DEFAULT_VIEWER: ViewerSettings = {
  editor: null,
  theme: 'system',
  scrollToChanges: true,
  openProblemsOnErrors: false,
  wrapLines: true,
  panelSide: 'right',
  panelWidth: null,
  panelHeight: null,
  unreadable: null,
};

/** The body of `POST /.flm-preview/viewer`: the settings to change, by their names in the protocol. */
export interface ViewerChange {
  editor?: string;
  theme?: Theme;
  scroll_to_changes?: boolean;
  open_problems_on_errors?: boolean;
  wrap_lines?: boolean;
  panel_side?: PanelSide;
  panel_width?: number;
  panel_height?: number;
}

function flag(value: unknown, otherwise: boolean): boolean {
  return typeof value === 'boolean' ? value : otherwise;
}

function size(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null;
}

/**
 * The viewer settings of `value` (the field `viewer` of `hello` and of the message
 * `viewer`), a field that is not understood taking its default; `null` when `value` is not
 * an object.
 */
export function readViewer(value: unknown): ViewerSettings | null {
  if (!isObject(value)) return null;
  const defaults = DEFAULT_VIEWER;
  return {
    editor: typeof value.editor === 'string' ? value.editor : null,
    theme: THEMES.find((theme) => theme === value.theme) ?? defaults.theme,
    scrollToChanges: flag(value.scroll_to_changes, defaults.scrollToChanges),
    openProblemsOnErrors: flag(value.open_problems_on_errors, defaults.openProblemsOnErrors),
    wrapLines: flag(value.wrap_lines, defaults.wrapLines),
    panelSide: PANEL_SIDES.find((side) => side === value.panel_side) ?? defaults.panelSide,
    panelWidth: size(value.panel_width),
    panelHeight: size(value.panel_height),
    unreadable: typeof value.unreadable === 'string' ? value.unreadable : null,
  };
}

/**
 * The editors of the field `editors` of `hello` and of the message `viewer`: those that are
 * understood.
 */
export function readEditors(value: unknown): Editor[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isObject).flatMap((each) =>
    typeof each.name === 'string' && typeof each.label === 'string' && typeof each.available === 'boolean'
      ? [{ name: each.name, label: each.label, available: each.available }]
      : []);
}
