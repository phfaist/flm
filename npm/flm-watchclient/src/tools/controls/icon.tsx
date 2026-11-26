// The icons of the tools page, from the Lucide set: outline icons drawn with a 2 px stroke
// on a 24 px grid, in the color of the text around them.

import check from 'lucide-static/icons/check.svg';
import chevronLeft from 'lucide-static/icons/chevron-left.svg';
import chevronRight from 'lucide-static/icons/chevron-right.svg';
import chevronUp from 'lucide-static/icons/chevron-up.svg';
import circleAlert from 'lucide-static/icons/circle-alert.svg';
import circleCheck from 'lucide-static/icons/circle-check.svg';
import circleHelp from 'lucide-static/icons/circle-help.svg';
import circleX from 'lucide-static/icons/circle-x.svg';
import copy from 'lucide-static/icons/copy.svg';
import download from 'lucide-static/icons/download.svg';
import externalLink from 'lucide-static/icons/external-link.svg';
import gripVertical from 'lucide-static/icons/grip-vertical.svg';
import history from 'lucide-static/icons/history.svg';
import house from 'lucide-static/icons/house.svg';
import info from 'lucide-static/icons/info.svg';
import list from 'lucide-static/icons/list.svg';
import listFilter from 'lucide-static/icons/list-filter.svg';
import lock from 'lucide-static/icons/lock.svg';
import mousePointer from 'lucide-static/icons/mouse-pointer-2.svg';
import packageIcon from 'lucide-static/icons/package.svg';
import panelBottom from 'lucide-static/icons/panel-bottom.svg';
import panelRight from 'lucide-static/icons/panel-right.svg';
import plus from 'lucide-static/icons/plus.svg';
import rotateCcw from 'lucide-static/icons/rotate-ccw.svg';
import rotateCw from 'lucide-static/icons/rotate-cw.svg';
import search from 'lucide-static/icons/search.svg';
import settings from 'lucide-static/icons/settings.svg';
import slidersHorizontal from 'lucide-static/icons/sliders-horizontal.svg';
import squareArrowOutUpRight from 'lucide-static/icons/square-arrow-out-up-right.svg';
import tag from 'lucide-static/icons/tag.svg';
import trash from 'lucide-static/icons/trash-2.svg';
import triangleAlert from 'lucide-static/icons/triangle-alert.svg';
import unplug from 'lucide-static/icons/unplug.svg';
import x from 'lucide-static/icons/x.svg';
import zoomIn from 'lucide-static/icons/zoom-in.svg';
import zoomOut from 'lucide-static/icons/zoom-out.svg';

const ICONS = {
  check,
  'chevron-left': chevronLeft,
  'chevron-right': chevronRight,
  'chevron-up': chevronUp,
  'circle-alert': circleAlert,
  'circle-check': circleCheck,
  'circle-help': circleHelp,
  'circle-x': circleX,
  copy,
  download,
  'external-link': externalLink,
  'grip-vertical': gripVertical,
  history,
  house,
  info,
  list,
  'list-filter': listFilter,
  lock,
  'mouse-pointer-2': mousePointer,
  package: packageIcon,
  'panel-bottom': panelBottom,
  'panel-right': panelRight,
  plus,
  'rotate-ccw': rotateCcw,
  'rotate-cw': rotateCw,
  search,
  settings,
  'sliders-horizontal': slidersHorizontal,
  'square-arrow-out-up-right': squareArrowOutUpRight,
  tag,
  'trash-2': trash,
  'triangle-alert': triangleAlert,
  unplug,
  x,
  'zoom-in': zoomIn,
  'zoom-out': zoomOut,
};

export type IconName = keyof typeof ICONS;

/** The SVG text of the icon `name`, `size` pixels wide and high, its stroke in `stroke`. */
export function svg(name: IconName, size: number, stroke = 'currentColor'): string {
  return ICONS[name]
    .replace(/ width="24"/, ` width="${size}"`)
    .replace(/ height="24"/, ` height="${size}"`)
    .replace(/stroke="currentColor"/, `stroke="${stroke}"`);
}

/**
 * The icon `name`. With a `label`, it is an image of that name; without, it is hidden from
 * assistive technology, and the control around it has the name.
 */
export function Icon({ name, size, label }: { name: IconName; size: number; label?: string }) {
  return (
    <span
      class="flm-icon"
      aria-hidden={label === undefined ? true : undefined}
      role={label === undefined ? undefined : 'img'}
      aria-label={label}
      dangerouslySetInnerHTML={{ __html: svg(name, size) }}
    />
  );
}
