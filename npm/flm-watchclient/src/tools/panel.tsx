// The panel and the bar that resizes it: the tab row (the tabs, then at its right end the
// Viewer tab, the dock button and the close button) and the open tab's content. The panel
// is docked at the right or at the bottom of the body; its size there is the one asked for,
// within the limits of the body's size (`state/layout.ts`). The side and the sizes are
// viewer settings: the page stores them once a drag, a move by the arrow keys or a change
// of side has ended, and keeps them in memory alone while the settings cannot be written or
// the connection is lost.

import type { ComponentChildren } from 'preact';
import { useEffect, useRef } from 'preact/hooks';
import type { IconName } from './controls/icon';
import { Icon } from './controls/icon';
import { ExportTab } from './panels/export';
import { HelpTab } from './panels/help';
import { MetadataTab } from './panels/metadata';
import { OutlineTab } from './panels/outline';
import { ProblemsTab, problemCount } from './panels/problems';
import { SettingsTab } from './panels/settings';
import { ViewerTab } from './panels/viewer';
import { changeViewer } from './requests';
import type { ViewerChange } from '../shared/viewer';
import type { Side } from './state/layout';
import { dragged, keyed, limited, other } from './state/layout';
import { focusTarget, panel, preview, viewer } from './state/store';

/**
 * A tab of the panel: its icon, its name, its tooltip (the name, when not said), a count
 * beside the name, its content, whether it is offered (always, when not said), and whether
 * it stands at the right end of the tab row.
 */
export interface Tab {
  id: string;
  name: string;
  title?: string;
  icon: IconName;
  count?: () => string | null;
  content: () => ComponentChildren;
  offered?: () => boolean;
  end?: boolean;
}

/** The tabs, in their order. */
export const TABS: Tab[] = [
  { id: 'problems', name: 'Problems', icon: 'circle-alert', count: problemCount, content: () => <ProblemsTab /> },
  { id: 'outline', name: 'Outline', icon: 'list', content: () => <OutlineTab /> },
  // Help, when the program that runs the preview gives the help pages.
  { id: 'help', name: 'Help', icon: 'circle-help', content: () => <HelpTab />, offered: () => preview.value.help },
  { id: 'metadata', name: 'Metadata', icon: 'tag', content: () => <MetadataTab /> },
  { id: 'settings', name: 'Settings', icon: 'sliders-horizontal', content: () => <SettingsTab /> },
  { id: 'export', name: 'Export', icon: 'download', content: () => <ExportTab /> },
  { id: 'viewer', name: 'Viewer', title: 'Viewer settings', icon: 'settings', content: () => <ViewerTab />, end: true },
];

/** The tabs offered, in their order. */
export function offeredTabs(): Tab[] {
  return TABS.filter((each) => each.offered?.() ?? true);
}

/** The body's size, in pixels, which bounds the panel's. */
export interface Room {
  width: number;
  height: number;
}

function setSize(side: Side, size: number): void {
  panel.value = { ...panel.value, sizes: { ...panel.value.sizes, [side]: size } };
}

/**
 * Stores `change` of the panel's place in the viewer settings, unless they cannot be written
 * or the connection is lost, when no request is sent.
 */
function storeLayout(change: ViewerChange): void {
  if (viewer.value.settings.unreadable === null && preview.value.connection !== 'disconnected') {
    void changeViewer(change);
  }
}

/** Stores the panel's size at `side`, as it is shown. */
function storeSize(side: Side): void {
  const size = panel.value.sizes[side];
  storeLayout(side === 'right' ? { panel_width: size } : { panel_height: size });
}

/** The bar between the output view and the panel: dragged, or moved with the arrow keys. */
function ResizeBar({ room, body }: { room: Room; body: () => HTMLElement | null }) {
  const { side, sizes } = panel.value;
  const dragging = useRef(false);
  // Whether the size changed since the last one stored.
  const moved = useRef(false);
  const move = (event: PointerEvent) => {
    const element = body();
    if (!dragging.current || element === null) return;
    const box = element.getBoundingClientRect();
    setSize(side, dragged(side, box, event.clientX, event.clientY));
    moved.current = true;
  };
  const end = () => {
    dragging.current = false;
    if (!moved.current) return;
    moved.current = false;
    storeSize(side);
  };
  return (
    <button
      type="button"
      class="flm-resize"
      aria-label="Resize the panel"
      title="Resize the panel"
      onPointerDown={(event) => {
        (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
        dragging.current = true;
      }}
      onPointerMove={move}
      onPointerUp={end}
      onPointerCancel={end}
      onKeyDown={(event) => {
        const size = keyed(side, event.key, limited(side, sizes[side], room.width, room.height), room.width, room.height);
        if (size === null) return;
        event.preventDefault();
        setSize(side, size);
        moved.current = true;
      }}
      onKeyUp={end}
      onBlur={end}
    />
  );
}

function TabButton({ tab, open }: { tab: Tab; open: boolean }) {
  const count = tab.count?.() ?? null;
  return (
    <button
      type="button"
      class={open ? 'flm-tab flm-tab-open' : 'flm-tab'}
      aria-pressed={open}
      title={tab.title ?? tab.name}
      onClick={() => (panel.value = { ...panel.value, tab: tab.id })}
    >
      <Icon name={tab.icon} size={15} />
      <span class="flm-tab-name">{tab.name}</span>
      {count !== null && <span class="flm-tab-count">{count}</span>}
    </button>
  );
}

/** Closes the panel, and gives the focus to the panel button that opens it again. */
function close(): void {
  panel.value = { ...panel.value, open: false };
  document.querySelector<HTMLElement>('.flm-panel-button')?.focus();
}

function TabRow({ shown }: { shown: Tab | undefined }) {
  const { side } = panel.value;
  const to = other(side);
  const dock = `Dock the panel at the ${to}`;
  const tabs = offeredTabs();
  const button = (each: Tab) => <TabButton key={each.id} tab={each} open={each === shown} />;
  return (
    <div class="flm-tab-row">
      {tabs.filter((each) => !each.end).map(button)}
      <span class="flm-tab-gap" />
      {tabs.filter((each) => each.end).map(button)}
      <button
        type="button"
        class="flm-icon-button flm-dock"
        aria-label={dock}
        title={dock}
        onClick={() => {
          panel.value = { ...panel.value, side: to };
          storeLayout({ panel_side: to });
        }}
      >
        <Icon name={to === 'bottom' ? 'panel-bottom' : 'panel-right'} size={16} />
      </button>
      <button type="button" class="flm-icon-button" aria-label="Close the panel" title="Close the panel" onClick={close}>
        <Icon name="x" size={16} />
      </button>
    </div>
  );
}

/** The controls that can take the focus, in the order of the keyboard. */
const FOCUSABLE =
  'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]';

/**
 * The content of the tab `tab`. When `focusTab` asked for that tab, its first control takes
 * the focus, or the content itself when the tab has none.
 */
function TabContent({ tab }: { tab: Tab | undefined }) {
  const element = useRef<HTMLDivElement>(null);
  const wanted = focusTarget.value;
  useEffect(() => {
    const content = element.current;
    if (tab === undefined || wanted !== tab.id || content === null) return;
    focusTarget.value = null;
    (content.querySelector<HTMLElement>(FOCUSABLE) ?? content).focus({ preventScroll: true });
  }, [wanted, tab?.id]);
  return (
    <div ref={element} class="flm-panel-content" tabIndex={-1}>
      {tab?.content()}
    </div>
  );
}

/** The resize bar and the panel, when the panel is open. */
export function Panel({ room, body }: { room: Room; body: () => HTMLElement | null }) {
  const { open, side, sizes, tab } = panel.value;
  if (!open) return null;
  const size = limited(side, sizes[side], room.width, room.height);
  // The tab used last; the first, Problems, before any was chosen.
  const tabs = offeredTabs();
  const shown = tabs.find((each) => each.id === tab) ?? tabs[0];
  return (
    <>
      <ResizeBar room={room} body={body} />
      <section
        class="flm-panel"
        aria-label="Panel"
        style={side === 'right' ? { width: `${size}px` } : { height: `${size}px` }}
      >
        <TabRow shown={shown} />
        <TabContent tab={shown} />
      </section>
    </>
  );
}
