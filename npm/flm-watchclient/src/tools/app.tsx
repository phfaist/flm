// The tools page's application: the body (the output view, and the panel when it is open)
// above the status bar.

import { useLayoutEffect, useRef, useState } from 'preact/hooks';
import type { Room } from './panel';
import { Panel } from './panel';
import { panel } from './state/store';
import { StatusBar } from './status-bar';
import { Output } from './views/output';

export function App() {
  const body = useRef<HTMLDivElement>(null);
  const [room, setRoom] = useState<Room>({ width: window.innerWidth, height: window.innerHeight });
  // The body's size bounds the panel's; it changes with the window and with the panel's side.
  useLayoutEffect(() => {
    const element = body.current;
    if (element === null) return undefined;
    const observer = new ResizeObserver(() => setRoom({ width: element.clientWidth, height: element.clientHeight }));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return (
    <div class="flm-tools">
      <div ref={body} class="flm-body" data-side={panel.value.side}>
        <Output />
        <Panel room={room} body={() => body.current} />
      </div>
      <StatusBar />
    </div>
  );
}
